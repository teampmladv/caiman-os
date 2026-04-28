//! caiman-vmm v0.4.0 — KVM hypervisor without QEMU
//! v0.4.0 adds: virtio-net (TX/RX via TAP), full network inside guest

use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use clap::Parser;
use kvm_ioctls::Kvm;
use tracing::info;

mod device;
mod ebpf;
mod kvm;
mod netlink_ctrl;
mod virtio;

use device::serial::Serial;
use virtio::net::{VirtioNet, VIRTIO_NET_MMIO_BASE, VIRTIO_NET_MMIO_SIZE, VIRTIO_NET_IRQ};

#[derive(Parser)]
#[command(name = "caiman-vmm", version = "0.4.0", about = "KVM VMM — no QEMU")]
struct Args {
    #[arg(long)] kernel:  String,
    #[arg(long)] initrd:  Option<String>,
    #[arg(long)] disk:    Option<String>,

    /// Kernel cmdline — includes virtio_mmio device spec
    #[arg(long, default_value =
        "console=ttyS0,115200 reboot=k panic=1 nomodules \
         virtio_mmio.device=0x1000@0xd0000000:5")]
    cmdline: String,

    #[arg(long, default_value_t = 256)] mem_mib: u64,
    #[arg(long, default_value_t = 1)]   cpus:    u8,
    #[arg(long, default_value = "eth0")] uplink:  String,
    #[arg(long, default_value = "tap0")] tap:     String,
    #[arg(long, default_value_t = 1)]   vm_id:   u32,
    #[arg(long, default_value = "10.0.0.1/24")] tap_ip: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args = Args::parse();
    info!("caiman-vmm v0.4.0 — vm_id={} mem={}MiB cpus={}", args.vm_id, args.mem_mib, args.cpus);
    run(args).await
}

async fn run(args: Args) -> Result<()> {
    // ── 1. Guest memory ───────────────────────────────────────────────────
    let kvm_fd = Kvm::new().context("opening /dev/kvm")?;
    let vm_fd  = kvm_fd.create_vm().context("KVM_CREATE_VM")?;
    let mem = kvm::memory::GuestMemory::new(&vm_fd, args.mem_mib, false)?;

    // ── 2. Load kernel ────────────────────────────────────────────────────
    let loader_result = kvm::loader::load_bzimage(
        &mem,
        std::path::Path::new(&args.kernel),
        &args.cmdline,
        args.initrd.as_deref().map(std::path::Path::new),
        args.mem_mib,
    )?;
    info!("Kernel loaded: entry={:#x}", loader_result.entry_point);

    // ── 3. VM + virtio-net ────────────────────────────────────────────────
    let vm = kvm::vm::Vm::new(&mem)?;

    let mac = [0x02, 0xaa, 0xbb, 0x00, 0x00, args.vm_id as u8];
    let vnet = VirtioNet::new(vm.vm_fd(), mac)?;

    // ── 4. Serial + vCPUs ────────────────────────────────────────────────
    let serial = Arc::new(Mutex::new(Serial::new()));
    let load = kvm::loader::KernelLoadResult {
        kernel_load:      kvm::loader::KernelLoadOffset { offset: loader_result.entry_point },
        boot_params_addr: kvm::loader::ZERO_PAGE_ADDR,
    };

    // Wrap mem in Arc early so it can be shared between vCPUs and dataplane
    let mem_arc = Arc::new(mem);

    let vnet_state = Arc::clone(&vnet.state);
    let handles: Vec<_> = (0..args.cpus)
        .map(|id| {
            let s  = Arc::clone(&serial);
            let vs = Arc::clone(&vnet_state);
            kvm::vcpu::Vcpu::new(&vm, id as u64, &*mem_arc, &load)
                .map(|vcpu| vcpu.run(s, vs))
        })
        .collect::<Result<_>>()?;

    info!("VM running — serial output:");
    println!("─────────────────────────────────────────────────────");

    // ── 5. virtio-net TAP dataplane ───────────────────────────────────────
    vnet.start_dataplane(&args.tap, Arc::clone(&mem_arc))?;
    info!("virtio-net dataplane started on TAP '{}'", args.tap);

    // ── 6. XDP ───────────────────────────────────────────────────────────
    netlink_ctrl::vm_add(args.vm_id, &mac, &args.uplink).await.ok();

    // ── 7. Wait ───────────────────────────────────────────────────────────
    for h in handles { let _ = h.join(); }
    println!("─────────────────────────────────────────────────────");
    netlink_ctrl::vm_del(args.vm_id).await.ok();
    info!("VM {} shutdown", args.vm_id);
    Ok(())
}

pub fn fmt_mac(mac: &[u8; 6]) -> String {
    mac.map(|b| format!("{b:02x}")).join(":")
}
