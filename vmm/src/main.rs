//! caiman-vmm v0.6.0 -- KVM hypervisor without QEMU
//!
//! v0.6.0: virtio-blk + dynamic cmdline -- boots Debian/Alpine guests

use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use std::io::Write;
use clap::Parser;
use kvm_ioctls::Kvm;
use tracing::info;

mod cmdline;
mod device;
mod ebpf;
mod kvm;
mod netlink_ctrl;
mod virtio;

use device::serial::Serial;
use virtio::net::{VirtioNet, VIRTIO_NET_MMIO_BASE};
use virtio::blk::{VirtioBlk, VIRTIO_BLK_MMIO_BASE};

#[derive(Parser)]
#[command(name = "caiman-vmm", version = env!("CARGO_PKG_VERSION"), about = "KVM VMM -- no QEMU")]
struct Args {
    #[arg(long)] kernel:  String,
    #[arg(long)] initrd:  Option<String>,
    #[arg(long)] disk:    Option<String>,

    #[arg(long)]
    cmdline: Option<String>,

    #[arg(long, default_value_t = 256)] mem_mib: u64,
    #[arg(long, default_value_t = 1)]   cpus:    u8,
    #[arg(long, default_value = "eth0")] uplink:  String,
    #[arg(long, default_value = "tap0")] tap:     String,
    #[arg(long, default_value = "1")]    vm_id:   String,
    #[arg(long, default_value = "")]       vm_name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let args = Args::parse();
    info!("caiman-vmm v{} -- vm_id={} mem={}MiB cpus={}", env!("CARGO_PKG_VERSION"), args.vm_id, args.mem_mib, args.cpus);
    run(args).await
}

async fn run(args: Args) -> Result<()> {
    // -- 1. Guest memory (pre-allocate, register in Vm::new) ---------------
    let mem = kvm::memory::GuestMemory::alloc(args.mem_mib, false)?;
    let mem = Arc::new(mem);

    // -- 2. VM (creates KVM fd + VM fd + registers memory) -----------------
    let vm = kvm::vm::Vm::new(&*mem)?;

    // Build cmdline: explicit --cmdline wins; otherwise derive from devices.
    let effective_cmdline = match args.cmdline.clone() {
        Some(c) => c,
        None => cmdline::build_cmdline(&cmdline::CmdlineOpts {
            root_device: if args.disk.is_some() { Some("/dev/vda") } else { None },
            rootfstype:  if args.disk.is_some() { Some("ext4") } else { None },
            has_net:     true,
            has_disk:    args.disk.is_some(),
            ip_config:   None,
            extra:       None,
        }),
    };
    info!("cmdline: {effective_cmdline}");

    // -- 3. Load kernel ----------------------------------------------------
    let lr = kvm::loader::load_bzimage(
        &*mem,
        std::path::Path::new(&args.kernel),
        &effective_cmdline,
        args.initrd.as_deref().map(std::path::Path::new),
        args.mem_mib,
    )?;
    info!("Kernel entry={:#x}", lr.entry_point);

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
    let serial_irqfd = Arc::new(vmm_sys_util::eventfd::EventFd::new(0).context("serial irqfd")?);
    vm.vm_fd().register_irqfd(&serial_irqfd, 4).map_err(|e| anyhow::anyhow!("serial irqfd: {e}"))?;
    let serial = Arc::new(Mutex::new(Serial::new()));
    serial.lock().unwrap().irqfd = Some(Arc::clone(&serial_irqfd));
    let load   = kvm::loader::KernelLoadResult {
        kernel_load:      kvm::loader::KernelLoadOffset { offset: lr.entry_point },
        boot_params_addr: kvm::loader::ZERO_PAGE_ADDR,
    };

    let vnet_state = Arc::clone(&vnet.state);
    let vblk_kick  = vblk.as_ref().and_then(|b| b.kickfd.try_clone().ok());
    let vblk_state = vblk.as_ref().map(|b| Arc::clone(&b.state));

    let handles: Vec<_> = (0..args.cpus)
        .map(|id| {
            let s  = Arc::clone(&serial);
            let vn = Arc::clone(&vnet_state);
            let vb = vblk_state.clone();
            let kick = vblk_kick.as_ref().and_then(|k| k.try_clone().ok());
            kvm::vcpu::Vcpu::new(&vm, id as u64, &*mem, &load)
                .map(|vcpu| vcpu.run(s, vn, vb, kick))
        })
        .collect::<Result<_>>()?;

    info!("VM running:");
    println!("-----------------------------------------------------");
    write_vm_state(&args, std::process::id(), &mac);

    // -- v1.4: stdin -> serial RX bridge (no termios manipulation)
    // The host wrapper (API) provides the PTY in raw mode already.
    // We just read raw bytes from stdin and feed them to the UART.
    {
        let serial_stdin = Arc::clone(&serial);
        let irqfd_stdin  = serial_irqfd.try_clone().context("clone serial irqfd for stdin")?;
        std::thread::Builder::new().name("serial-stdin".into()).spawn(move || {
            use std::io::Read;
            let mut buf = [0u8; 64];
            loop {
                match std::io::stdin().read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        for &b in &buf[..n] {
                            {
                                let mut s = serial_stdin.lock().unwrap();
                                s.rbr = b;
                                s.lsr |= 0x01;
                                s.iir = 0x04;
                            }
                            for _ in 0..4 {
                                let _ = irqfd_stdin.write(1);
                                std::thread::sleep(std::time::Duration::from_millis(1));
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        }).ok();
    }

    // -- 7. virtio-net dataplane -------------------------------------------
    vnet.start_dataplane(&args.tap, Arc::clone(&mem))?;

    // -- 8. XDP -----------------------------------------------------------
    netlink_ctrl::vm_add(vm_num, &mac, &args.uplink).await.ok();

    for h in handles { let _ = h.join(); }
    println!("-----------------------------------------------------");
    netlink_ctrl::vm_del(vm_num).await.ok();
    info!("VM {} shutdown", args.vm_id);
    let state_path = format!("/var/run/caiman/{}.json", args.vm_id);
    let _ = std::fs::remove_file(&state_path);
    Ok(())
}

pub fn fmt_mac(mac: &[u8; 6]) -> String {
    mac.map(|b| format!("{b:02x}")).join(":")
}

fn write_vm_state(args: &Args, pid: u32, mac: &[u8; 6]) {
    let state_dir = "/var/run/caiman";
    let _ = std::fs::create_dir_all(state_dir);
    let now = chrono::Utc::now().to_rfc3339();
    let state = serde_json::json!({
        "id":          args.vm_id,
        "name":        if args.vm_name.is_empty() { args.vm_id.clone() } else { args.vm_name.clone() },
        "status":      "RUNNING",
        "pid":         pid,
        "cpus":        args.cpus,
        "memMib":      args.mem_mib,
        "nodeName":    hostname::get().unwrap_or_default().to_string_lossy().to_string(),
        "kernel":      args.kernel,
        "disk":        args.disk,
        "mac":         fmt_mac(mac),
        "uplink":      "eth0",
        "labels":      {},
        "createdAt":   now,
        "startedAt":   now,
        "cpuUsagePct": 0.0,
        "memUsedMib":  0,
        "netRxMbps":   0.0,
        "netTxMbps":   0.0,
        "uptimeSecs":  0,
    });
    let path = format!("{}/{}.json", state_dir, args.vm_id);
    eprintln!("[DEBUG] writing to {}", path);
    match std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap()) {
        Ok(_) => eprintln!("[DEBUG] write OK"),
        Err(e) => eprintln!("[DEBUG] write ERROR: {}", e),
    }
}
