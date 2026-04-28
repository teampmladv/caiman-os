//! caiman-vmm v0.3.0 — KVM hypervisor without QEMU
//!
//! v0.3.0 adds:
//!   - Serial console ttyS0 (16550A) — see the kernel boot log
//!   - kvm_run mmap for PIO exit data
//!   - virtio-blk (disk image)
//!   - XDP registration with caiman_net.ko

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

#[derive(Parser)]
#[command(name = "caiman-vmm", version = "0.3.0", about = "KVM VMM — no QEMU")]
struct Args {
    /// Linux bzImage
    #[arg(long)]
    kernel: String,

    /// Initial ramdisk
    #[arg(long)]
    initrd: Option<String>,

    /// virtio-blk disk image
    #[arg(long)]
    disk: Option<String>,

    /// Kernel command line
    #[arg(long, default_value = "console=ttyS0,115200 reboot=k panic=1 nomodules")]
    cmdline: String,

    /// Guest RAM in MiB
    #[arg(long, default_value_t = 256)]
    mem_mib: u64,

    /// Number of vCPUs
    #[arg(long, default_value_t = 1)]
    cpus: u8,

    /// Host NIC for XDP
    #[arg(long, default_value = "eth0")]
    uplink: String,

    /// VM identifier
    #[arg(long, default_value_t = 1)]
    vm_id: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("caiman_vmm=info".parse()?),
        )
        .init();

    let args = Args::parse();
    info!("caiman-vmm v0.3.0 — vm_id={} cpus={} mem={}MiB", args.vm_id, args.cpus, args.mem_mib);
    run(args).await
}

async fn run(args: Args) -> Result<()> {
    // ── 1. Guest memory ───────────────────────────────────────────────────
    let kvm_fd = Kvm::new().context("opening /dev/kvm")?;
    let vm_fd  = kvm_fd.create_vm().context("KVM_CREATE_VM")?;
    let mut mem = kvm::memory::GuestMemory::new(&vm_fd, args.mem_mib, false)
        .context("allocating guest memory")?;
    info!("Guest memory: {} MiB", args.mem_mib);

    // ── 2. Load kernel ────────────────────────────────────────────────────
    let loader_result = kvm::loader::load_bzimage(
        &mut mem,
        std::path::Path::new(&args.kernel),
        &args.cmdline,
        args.initrd.as_deref().map(std::path::Path::new),
        args.mem_mib,
    ).context("loading bzImage")?;
    info!("Kernel loaded: entry={:#x}", loader_result.entry_point);

    // ── 3. Create VM ──────────────────────────────────────────────────────
    let vm = kvm::vm::Vm::new(&mem).context("creating VM")?;

    // ── 4. Serial console ─────────────────────────────────────────────────
    let serial = Arc::new(Mutex::new(Serial::new()));
    info!("Serial console: COM1 (0x3F8) → stdout");

    // ── 5. Create vCPUs ───────────────────────────────────────────────────
    let load = kvm::loader::KernelLoadResult {
        kernel_load:      kvm::loader::KernelLoadOffset { offset: loader_result.entry_point },
        boot_params_addr: kvm::loader::ZERO_PAGE_ADDR,
    };

    let handles: Vec<_> = (0..args.cpus)
        .map(|id| {
            let s = Arc::clone(&serial);
            kvm::vcpu::Vcpu::new(&vm, id as u64, &mem, &load)
                .map(|vcpu| vcpu.run(s))
        })
        .collect::<Result<_>>()?;

    info!("{} vCPU(s) running — serial output below:", args.cpus);
    println!("─────────────────────────────────────────────────────────────");

    // ── 6. XDP + BPF ──────────────────────────────────────────────────────
    let mac = [0x02, 0xaa, 0xbb, 0x00, 0x00, args.vm_id as u8];
    netlink_ctrl::vm_add(args.vm_id, &mac, &args.uplink).await.ok();
    ebpf::setup_vm_maps(args.vm_id, &mac,
        &format!("/sys/fs/bpf/caiman/vm{}", args.vm_id)).ok();

    // ── 7. virtio-blk ─────────────────────────────────────────────────────
    if let Some(ref disk) = args.disk {
        let _blk = virtio::blk::VirtioBlk::new(disk, false)?;
        info!("virtio-blk: {disk}");
    }

    // ── 8. Wait for VM exit ───────────────────────────────────────────────
    for h in handles { let _ = h.join(); }

    println!("─────────────────────────────────────────────────────────────");
    netlink_ctrl::vm_del(args.vm_id).await.ok();
    info!("VM {} shutdown", args.vm_id);
    Ok(())
}

pub fn fmt_mac(mac: &[u8; 6]) -> String {
    mac.map(|b| format!("{b:02x}")).join(":")
}
