//! caiman-vmm — KVM hypervisor without QEMU
use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;

mod device;
mod ebpf;
mod kvm;
mod netlink_ctrl;
mod virtio;

use kvm::{memory::GuestMemory, vcpu::Vcpu, vm::Vm};

#[derive(Parser, Debug)]
#[command(name = "caiman-vmm", about = "Minimal KVM VMM — no QEMU")]
struct Args {
    #[arg(long)] kernel:  String,
    #[arg(long)] initrd:  Option<String>,
    #[arg(long, default_value = "console=ttyS0 reboot=k panic=1 nomodules")]
    cmdline: String,
    #[arg(long, default_value_t = 256)] mem_mib: u64,
    #[arg(long, default_value_t = 1)]   cpus: u8,
    #[arg(long, default_value = "eth0")] uplink: String,
    #[arg(long)] mac: Option<String>,
    #[arg(long, default_value_t = 1)]   vm_id: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("caiman_vmm=info".parse()?))
        .init();

    let args = Args::parse();
    info!("caiman-vmm: vm_id={} cpus={} mem={}MiB", args.vm_id, args.cpus, args.mem_mib);
    run(args).await
}

async fn run(args: Args) -> Result<()> {
    use kvm_ioctls::Kvm;

    // 1. Guest memory
    let kvm = Kvm::new().context("opening /dev/kvm")?;
    let vm_fd = kvm.create_vm().context("KVM_CREATE_VM")?;
    let mut mem = GuestMemory::new(&vm_fd, args.mem_mib, false)
        .context("allocating guest memory")?;

    // 2. Create VM wrapper
    let vm = Vm::new(&mem).context("creating VM")?;

    // 3. Load kernel
    let result = kvm::loader::load_bzimage(
        &mut mem,
        std::path::Path::new(&args.kernel),
        &args.cmdline,
        args.initrd.as_deref().map(std::path::Path::new),
        args.mem_mib,
    ).context("loading bzImage")?;
    info!("Kernel loaded: entry={:#x}", result.entry_point);

    // 4. Register with caiman_net
    let mac = gen_mac(args.vm_id);
    netlink_ctrl::vm_add(args.vm_id, &mac, &args.uplink).await.ok();

    // 5. Setup BPF maps
    let bpf_pin = format!("/sys/fs/bpf/caiman/vm{}", args.vm_id);
    ebpf::setup_vm_maps(args.vm_id, &mac, &bpf_pin).ok();

    // 6. Run vCPUs
    info!("Starting {} vCPU(s)…", args.cpus);
    let load_result = kvm::loader::KernelLoadResult {
        kernel_load:      kvm::loader::KernelLoadOffset { offset: result.entry_point },
        boot_params_addr: kvm::loader::ZERO_PAGE_ADDR,
    };
    let handles: Vec<_> = (0..args.cpus)
        .map(|id| {
            Vcpu::new(&vm, id as u64, &mem, &load_result)
                .map(|mut v| v.run())
        })
        .collect::<Result<_>>()?;

    for h in handles { let _ = h.join(); }

    netlink_ctrl::vm_del(args.vm_id).await.ok();
    info!("VMM shutdown");
    Ok(())
}

fn gen_mac(vm_id: u32) -> [u8; 6] {
    let b = vm_id.to_le_bytes();
    [0x02, 0xaa, 0xbb, b[0], b[1], b[2]]
}
