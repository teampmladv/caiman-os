//! caiman-vmm — KVM hypervisor without QEMU
use anyhow::{Context, Result};
use clap::Parser;
use kvm_ioctls::Kvm;
use tracing::info;

#[derive(Parser)]
#[command(name = "caiman-vmm", version, about = "KVM VMM — no QEMU")]
struct Args {
    #[arg(long)] kernel:  String,
    #[arg(long)] initrd:  Option<String>,
    #[arg(long, default_value = "console=ttyS0 reboot=k panic=1")] cmdline: String,
    #[arg(long, default_value_t = 256)] mem_mib: u64,
    #[arg(long, default_value_t = 1)]   cpus: u8,
    #[arg(long, default_value = "eth0")] uplink: String,
    #[arg(long, default_value_t = 1)]   vm_id: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    info!("caiman-vmm v{} starting — vm_id={} cpus={} mem={}MiB kernel={}",
        env!("CARGO_PKG_VERSION"), args.vm_id, args.cpus, args.mem_mib, args.kernel);

    let kvm = Kvm::new().context("Cannot open /dev/kvm — is KVM loaded?")?;
    let api_version = kvm.get_api_version();
    info!("KVM API version: {api_version}");

    let _vm = kvm.create_vm().context("KVM_CREATE_VM failed")?;
    info!("VM created — loader and vCPU loop coming in next release");

    // Block until Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("Shutting down vm_id={}", args.vm_id);
    Ok(())
}
