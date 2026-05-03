//! caiman-vmm v0.6.0 -- KVM hypervisor without QEMU
//!
//! v0.6.0 adds: virtio-blk wired -- disco funcional dentro del guest

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
use virtio::net::{VirtioNet, VIRTIO_NET_MMIO_BASE};
use virtio::blk::{VirtioBlk, VIRTIO_BLK_MMIO_BASE};

#[derive(Parser)]
#[command(name = "caiman-vmm", version = "0.6.0", about = "KVM VMM -- no QEMU")]
struct Args {
    #[arg(long)] kernel:  String,
    #[arg(long)] initrd:  Option<String>,
    #[arg(long)] disk:    Option<String>,

    #[arg(long, default_value =
        "console=ttyS0,115200 reboot=k panic=1 nomodules \
         virtio_mmio.device=0x1000@0xd0000000:5 \
         virtio_mmio.device=0x1000@0xd0010000:6")]
    cmdline: String,

    #[arg(long, default_value_t = 256)] mem_mib: u64,
    #[arg(long, default_value_t = 1)]   cpus:    u8,
    #[arg(long, default_value = "eth0")] uplink:  String,
    #[arg(long, default_value = "tap0")] tap:     String,
    #[arg(long, default_value = "1")]    vm_id:   String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args = Args::parse();
    info!("caiman-vmm v{} -- vm_id={} mem={}MiB cpus={}", env!("CARGO_PKG_VERSION"), args.vm_id, args.mem_mib, args.cpus);
    run(args).await
}

async fn run(args: Args) -> Result<()> {
    // -- 1. Guest memory ---------------------------------------------------
    let kvm_fd = Kvm::new().context("opening /dev/kvm")?;
    let vm_fd  = kvm_fd.create_vm().context("KVM_CREATE_VM")?;
    let mem    = kvm::memory::GuestMemory::new(&vm_fd, args.mem_mib, false)?;
    let mem    = Arc::new(mem);

    // -- 2. Load kernel ----------------------------------------------------
    let lr = kvm::loader::load_bzimage(
        &*mem,
        std::path::Path::new(&args.kernel),
        &args.cmdline,
        args.initrd.as_deref().map(std::path::Path::new),
        args.mem_mib,
    )?;
    info!("Kernel entry={:#x}", lr.entry_point);

    // -- 3. VM -------------------------------------------------------------
    let vm = kvm::vm::Vm::new(&*mem)?;

    // -- 4. virtio-net -----------------------------------------------------
    // Parse vm_id to u32 for MAC/netlink -- strip "vm-" prefix, take decimal or hash
    let vm_num: u32 = args.vm_id.trim_start_matches("vm-")
        .parse::<u32>()
        .unwrap_or_else(|_| {
            // Hash the string to a u32
            args.vm_id.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32)) & 0xFF
        });
    let mac  = [0x02, 0xaa, 0xbb, 0x00, (vm_num >> 8) as u8, (vm_num & 0xFF) as u8];
    let vnet = VirtioNet::new(vm.vm_fd(), mac)?;

    // -- 5. virtio-blk (optional) ------------------------------------------
    let vblk = if let Some(ref disk) = args.disk {
        let blk = VirtioBlk::new(disk, false)?;
        blk.register_irq(vm.vm_fd())?;
        blk.start_dataplane(Arc::clone(&mem))?;
        info!("virtio-blk: {disk} at {:#x} irq={}", VIRTIO_BLK_MMIO_BASE, 6);
        Some(blk)
    } else {
        info!("virtio-blk: no disk provided (use --disk path.img)");
        None
    };

    // -- 6. Serial + vCPUs ------------------------------------------------
    let serial = Arc::new(Mutex::new(Serial::new()));
    let load   = kvm::loader::KernelLoadResult {
        kernel_load:      kvm::loader::KernelLoadOffset { offset: lr.entry_point },
        boot_params_addr: kvm::loader::ZERO_PAGE_ADDR,
    };

    let vnet_state = Arc::clone(&vnet.state);
    let vblk_state = vblk.as_ref().map(|b| Arc::clone(&b.state));

    let handles: Vec<_> = (0..args.cpus)
        .map(|id| {
            let s  = Arc::clone(&serial);
            let vn = Arc::clone(&vnet_state);
            let vb = vblk_state.clone();
            kvm::vcpu::Vcpu::new(&vm, id as u64, &*mem, &load)
                .map(|vcpu| vcpu.run(s, vn, vb))
        })
        .collect::<Result<_>>()?;

    info!("VM running:");
    println!("-----------------------------------------------------");

    // -- 7. virtio-net dataplane -------------------------------------------
    vnet.start_dataplane(&args.tap, Arc::clone(&mem))?;

    // -- 8. XDP -----------------------------------------------------------
    netlink_ctrl::vm_add(vm_num, &mac, &args.uplink).await.ok();

    for h in handles { let _ = h.join(); }
    println!("-----------------------------------------------------");
    netlink_ctrl::vm_del(vm_num).await.ok();
    info!("VM {} shutdown", args.vm_id);
    Ok(())
}

pub fn fmt_mac(mac: &[u8; 6]) -> String {
    mac.map(|b| format!("{b:02x}")).join(":")
}
