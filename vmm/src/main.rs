//! caiman-vmm v0.6.0 -- KVM hypervisor without QEMU
//!
//! v0.6.0 adds: virtio-blk wired -- disco funcional dentro del guest

use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use std::io::Write;
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
#[command(name = "caiman-vmm", version = env!("CARGO_PKG_VERSION"), about = "KVM VMM -- no QEMU")]
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
    // -- 1. Guest memory (pre-allocate, register in Vm::new) ---------------
    let mem = kvm::memory::GuestMemory::alloc(args.mem_mib, false)?;
    let mem = Arc::new(mem);

    // -- 2. VM (creates KVM fd + VM fd + registers memory) -----------------
    let vm = kvm::vm::Vm::new(&*mem)?;

    // -- 3. Load kernel ----------------------------------------------------
    let lr = kvm::loader::load_bzimage(
        &*mem,
        std::path::Path::new(&args.kernel),
        &args.cmdline,
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

    // Put terminal in raw mode so stdin goes directly to serial
    let _raw = {
        use std::os::unix::io::AsRawFd;
        let fd = std::io::stdin().as_raw_fd();
        let mut termios = unsafe { std::mem::zeroed::<libc::termios>() };
        unsafe { libc::tcgetattr(fd, &mut termios) };
        let old = termios;
        termios.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
        termios.c_iflag &= !(libc::IXON | libc::ICRNL);
        termios.c_cc[libc::VMIN] = 1;
        termios.c_cc[libc::VTIME] = 0;
        unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) };
        struct RestoreTermios { fd: i32, old: libc::termios }
        impl Drop for RestoreTermios {
            fn drop(&mut self) {
                unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.old); }
            }
        }
        RestoreTermios { fd, old }
    };

    // Forward stdin -> serial RX
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
                            let byte = if b == b'\r' { b'\n' } else { b };
                            // Local echo
                            let _ = std::io::stdout().write_all(&[byte]);
                            let _ = std::io::stdout().flush();
                            // Inject into VM serial RX -- keep signaling until consumed
                            {
                                let mut s = serial_stdin.lock().unwrap();
                                s.rbr = byte;
                                s.lsr |= 0x01;
                                s.iir = 0x04; // IIR_RDI -- force RX interrupt
                            }
                            // Inject IRQ multiple times to ensure vCPU wakes
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
    Ok(())
}

pub fn fmt_mac(mac: &[u8; 6]) -> String {
    mac.map(|b| format!("{b:02x}")).join(":")
}
