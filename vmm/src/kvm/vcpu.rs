//! vmm/src/kvm/vcpu.rs — vCPU thread with serial console (ttyS0)
//!
//! v0.3.0: wires up the 16550A serial to KVM_EXIT_IO.
//! The kernel writes ttyS0 output via outb to port 0x3F8.
//! We read the port/data from the kvm_run struct (mmapped from the vcpu fd)
//! and pass each byte to the Serial device, which prints to stdout.

use std::io::{Write, stdout};
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use anyhow::Result;
use kvm_bindings::{
    kvm_regs, kvm_sregs, CpuId, Msrs,
    KVM_EXIT_HLT, KVM_EXIT_IO, KVM_EXIT_MMIO,
    KVM_EXIT_SHUTDOWN, KVM_EXIT_SYSTEM_EVENT,
};
use kvm_ioctls::{VcpuFd, VmFd};
use tracing::{debug, info, warn};

use crate::device::serial::{Serial, SERIAL_BASE};
use crate::virtio::net::{NetState, VIRTIO_NET_MMIO_BASE, VIRTIO_NET_MMIO_SIZE};
use super::{loader::KernelLoadResult, memory::GuestMemory, vm::Vm};

// ── KVM_RUN mmap wrapper ──────────────────────────────────────────────────

/// Safe wrapper around the kvm_run memory-mapped structure.
/// kvm-ioctls mmaps kvm_run internally but doesn't expose it publicly.
/// Per KVM API docs: kvm_run is at mmap offset 0 of the vcpu fd.
struct KvmRunPtr {
    ptr:  *mut kvm_bindings::kvm_run,
    size: usize,
}

impl KvmRunPtr {
    fn new(vcpu_fd: &VcpuFd, mmap_size: usize) -> Result<Self> {
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                mmap_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                vcpu_fd.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            anyhow::bail!("mmap kvm_run failed: {}", std::io::Error::last_os_error());
        }
        Ok(Self { ptr: ptr as *mut kvm_bindings::kvm_run, size: mmap_size })
    }

    fn io_port(&self) -> u16 {
        unsafe { (*self.ptr).__bindgen_anon_1.io.port }
    }

    fn io_direction(&self) -> u8 {
        unsafe { (*self.ptr).__bindgen_anon_1.io.direction }
    }

    fn io_size(&self) -> u8 {
        unsafe { (*self.ptr).__bindgen_anon_1.io.size }
    }

    fn io_count(&self) -> u32 {
        unsafe { (*self.ptr).__bindgen_anon_1.io.count }
    }

    fn io_data_offset(&self) -> u64 {
        unsafe { (*self.ptr).__bindgen_anon_1.io.data_offset }
    }

    /// Read the first byte of IO data (for 1-byte PIO OUT)
    fn io_data_u8(&self) -> u8 {
        let offset = self.io_data_offset() as usize;
        unsafe { *((self.ptr as *const u8).add(offset)) }
    }
}

impl Drop for KvmRunPtr {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.ptr as *mut libc::c_void, self.size); }
    }
}

// Safety: KvmRunPtr is accessed only by the owning vCPU thread
unsafe impl Send for KvmRunPtr {}

// ── vCPU ──────────────────────────────────────────────────────────────────

pub struct Vcpu {
    id:  u64,
    fd:  VcpuFd,
    run: KvmRunPtr,
}

impl Vcpu {
    pub fn new(
        vm:     &Vm,
        id:     u64,
        mem:    &GuestMemory,
        kernel: &KernelLoadResult,
    ) -> Result<Self> {
        let fd = vm.vm_fd().create_vcpu(id)?;

        // Get the mmap size kvm reports for kvm_run
        let mmap_size = vm.kvm()
            .get_vcpu_mmap_size()
            .context("KVM_GET_VCPU_MMAP_SIZE")?;

        let run = KvmRunPtr::new(&fd, mmap_size)?;

        configure_cpuid(vm, &fd)?;
        configure_msrs(&fd)?;
        configure_regs(&fd, kernel)?;
        configure_sregs(&fd, mem)?;

        Ok(Self { id, fd, run })
    }

    /// Spawn the vCPU run loop in a dedicated OS thread.
    pub fn run(mut self, serial: Arc<Mutex<Serial>>, vnet: Arc<Mutex<NetState>>) -> JoinHandle<()> {
        thread::Builder::new()
            .name(format!("vcpu-{}", self.id))
            .spawn(move || run_loop(self.id, &mut self.fd, &self.run, serial, vnet))
            .expect("spawning vCPU thread")
    }
}

// ── Run loop ──────────────────────────────────────────────────────────────

fn run_loop(id: u64, fd: &mut VcpuFd, run: &KvmRunPtr, serial: Arc<Mutex<Serial>>, vnet: Arc<Mutex<NetState>>) {
    info!("vCPU {id} entering run loop");
    loop {
        match fd.run() {
            Ok(exit) => {
                use kvm_ioctls::VcpuExit::*;
                match exit {
                    Io => handle_io(id, run, &serial),
                    MmioRead(addr, data)  => handle_mmio_read(id, addr, data, &vnet),
                    MmioWrite(addr, data) => handle_mmio_write(id, addr, data, &serial, &vnet),
                    Hlt => {
                        debug!("vCPU {id}: HLT — idling");
                        std::thread::sleep(std::time::Duration::from_micros(100));
                    }
                    Shutdown => {
                        info!("vCPU {id}: SHUTDOWN");
                        break;
                    }
                    SystemEvent(ev, _) => {
                        info!("vCPU {id}: system event {ev:#x}");
                        break;
                    }
                    _ => {
                        debug!("vCPU {id}: unhandled exit {:?}", exit);
                    }
                }
            }
            Err(e) if e.errno() == libc::EINTR => continue,
            Err(e) => {
                warn!("vCPU {id}: KVM_RUN error: {e}");
                break;
            }
        }
    }
    // Flush serial buffer on exit
    serial.lock().unwrap().flush();
    info!("vCPU {id} exited run loop");
}

// ── Exit handlers ─────────────────────────────────────────────────────────

const KVM_IO_OUT: u8 = 1;

/// Handle KVM_EXIT_IO — route to serial or reboot port
fn handle_io(id: u64, run: &KvmRunPtr, serial: &Arc<Mutex<Serial>>) {
    let port      = run.io_port();
    let direction = run.io_direction();
    let size      = run.io_size();

    // Serial COM1: ports 0x3F8..0x3FF
    if port >= SERIAL_BASE && port < SERIAL_BASE + 8 {
        if direction == KVM_IO_OUT && size == 1 {
            let byte = run.io_data_u8();
            serial.lock().unwrap().write_port(port, byte);
        } else if direction != KVM_IO_OUT {
            // IN from serial: return 0 for now
        }
        return;
    }

    // ACPI reboot port 0x604
    if port == 0x604 && direction == KVM_IO_OUT {
        info!("vCPU {id}: ACPI reset — shutting down");
        std::process::exit(0);
    }

    debug!("vCPU {id}: unhandled IO port={port:#x} dir={direction} size={size}");
}

fn handle_mmio_read(id: u64, addr: u64, data: &mut [u8], vnet: &Arc<Mutex<NetState>>) {
    debug!("vCPU {id}: MMIO read {addr:#x} len={}", data.len());
    data.fill(0);
}

fn handle_mmio_write(id: u64, addr: u64, data: &[u8], serial: &Arc<Mutex<Serial>>, vnet: &Arc<Mutex<NetState>>) {
    debug!("vCPU {id}: MMIO write {addr:#x} len={}", data.len());
    // PL011 serial fallback at 0x09000000
    if addr >= 0x09000000 && addr < 0x09001000 && !data.is_empty() {
        serial.lock().unwrap().write_port(SERIAL_BASE, data[0]);
        return;
    }
    // virtio-net MMIO config
    if addr >= VIRTIO_NET_MMIO_BASE && addr < VIRTIO_NET_MMIO_BASE + VIRTIO_NET_MMIO_SIZE {
        let offset = addr - VIRTIO_NET_MMIO_BASE;
        let val = if data.len() >= 4 {
            u32::from_le_bytes([data[0], data[1], data[2], data[3]])
        } else if !data.is_empty() {
            data[0] as u32
        } else { 0 };
        vnet.lock().unwrap().mmio_write(offset, val);
        return;
    }
}

// ── vCPU configuration ────────────────────────────────────────────────────

fn configure_cpuid(vm: &Vm, fd: &VcpuFd) -> Result<()> {
    let mut cpuid = vm.kvm()
        .get_supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)?;
    for entry in cpuid.as_mut_slice() {
        match entry.function {
            1 => { entry.ecx |= 1 << 31; } // hypervisor bit
            0x4000_0000 => {
                entry.eax = 0x4000_0001;
                entry.ebx = 0x4b4d_564b; // "KVMK"
                entry.ecx = 0x564b_4d56; // "VKMV"
                entry.edx = 0x4d00_0000; // "M\0\0\0"
            }
            _ => {}
        }
    }
    fd.set_cpuid2(&cpuid)?;
    Ok(())
}

fn configure_msrs(fd: &VcpuFd) -> Result<()> {
    let msrs = Msrs::from_entries(&[
        kvm_bindings::kvm_msr_entry { index: 0x174, data: 0, ..Default::default() }, // SYSENTER_CS
        kvm_bindings::kvm_msr_entry { index: 0x175, data: 0, ..Default::default() }, // SYSENTER_ESP
        kvm_bindings::kvm_msr_entry { index: 0x176, data: 0, ..Default::default() }, // SYSENTER_EIP
    ])?;
    fd.set_msrs(&msrs)?;
    Ok(())
}

fn configure_regs(fd: &VcpuFd, kernel: &KernelLoadResult) -> Result<()> {
    let mut regs = kvm_regs::default();
    regs.rflags = 0x0000_0000_0000_0002; // reserved bit always set
    regs.rip    = kernel.kernel_load.offset;
    regs.rsi    = kernel.boot_params_addr; // → boot_params
    fd.set_regs(&regs)?;
    Ok(())
}

fn configure_sregs(fd: &VcpuFd, _mem: &GuestMemory) -> Result<()> {
    let mut sregs = fd.get_sregs()?;
    sregs.cr0  |= 0x80000001; // PE + PG
    sregs.cr4  |= 0x20;       // PAE
    sregs.efer |= 0x500;      // LME + LMA
    fd.set_sregs(&sregs)?;
    Ok(())
}

// Helper trait for context()
trait Context<T> {
    fn context(self, msg: &str) -> Result<T>;
}
impl<T> Context<T> for std::result::Result<T, kvm_ioctls::Error> {
    fn context(self, msg: &str) -> Result<T> {
        self.map_err(|e| anyhow::anyhow!("{msg}: {e}"))
    }
}
impl<T> Context<T> for std::result::Result<T, std::io::Error> {
    fn context(self, msg: &str) -> Result<T> {
        self.map_err(|e| anyhow::anyhow!("{msg}: {e}"))
    }
}
