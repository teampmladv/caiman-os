//! vmm/src/kvm/vcpu.rs -- vCPU thread with serial console (ttyS0)

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
use crate::virtio::blk::{BlkState, VIRTIO_BLK_MMIO_BASE, VIRTIO_BLK_MMIO_SIZE};
use super::{loader::KernelLoadResult, memory::GuestMemory, vm::Vm};

struct KvmRunPtr {
    pub ptr:  *mut kvm_bindings::kvm_run,
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
    fn io_port(&self) -> u16 { unsafe { (*self.ptr).__bindgen_anon_1.io.port } }
    fn io_direction(&self) -> u8 { unsafe { (*self.ptr).__bindgen_anon_1.io.direction } }
    fn io_size(&self) -> u8 { unsafe { (*self.ptr).__bindgen_anon_1.io.size } }
    fn io_count(&self) -> u32 { unsafe { (*self.ptr).__bindgen_anon_1.io.count } }
    pub fn io_data_offset(&self) -> u64 { unsafe { (*self.ptr).__bindgen_anon_1.io.data_offset } }
    fn io_data_u8(&self) -> u8 {
        let offset = self.io_data_offset() as usize;
        unsafe { *((self.ptr as *const u8).add(offset)) }
    }
}
impl Drop for KvmRunPtr {
    fn drop(&mut self) { unsafe { libc::munmap(self.ptr as *mut libc::c_void, self.size); } }
}
unsafe impl Send for KvmRunPtr {}

pub struct Vcpu { id: u64, fd: VcpuFd, run: KvmRunPtr }

impl Vcpu {
    pub fn new(vm: &Vm, id: u64, mem: &GuestMemory, kernel: &KernelLoadResult) -> Result<Self> {
        let fd = vm.vm_fd().create_vcpu(id)?;
        let mmap_size = vm.kvm().get_vcpu_mmap_size().context("KVM_GET_VCPU_MMAP_SIZE")?;
        let run = KvmRunPtr::new(&fd, mmap_size)?;
        configure_cpuid(vm, &fd)?;
        configure_msrs(&fd)?;
        configure_regs(&fd, kernel)?;
        configure_sregs(&fd, mem)?;
        Ok(Self { id, fd, run })
    }
    pub fn run(mut self, serial: Arc<Mutex<Serial>>, vnet: Arc<Mutex<NetState>>, vblk: Option<Arc<Mutex<BlkState>>>) -> JoinHandle<()> {
        thread::Builder::new()
            .name(format!("vcpu-{}", self.id))
            .spawn(move || run_loop(self.id, &mut self.fd, &self.run, serial, vnet, vblk))
            .expect("spawning vCPU thread")
    }
}

fn run_loop(id: u64, fd: &mut VcpuFd, run: &KvmRunPtr, serial: Arc<Mutex<Serial>>, vnet: Arc<Mutex<NetState>>, vblk: Option<Arc<Mutex<BlkState>>>) {
    info!("vCPU {id} entering run loop");
    loop {
        match fd.run() {
            Ok(exit) => {
                use kvm_ioctls::VcpuExit::*;
                match exit {
                    Io => handle_io(id, run, &serial),
                    MmioRead(addr, data) => handle_mmio_read(id, addr, data, &vnet, &vblk),
                    MmioWrite(addr, data) => handle_mmio_write(id, addr, data, &serial, &vnet, &vblk),
                    Hlt => { debug!("vCPU {id}: HLT"); std::thread::sleep(std::time::Duration::from_micros(100)); }
                    Shutdown => { info!("vCPU {id}: SHUTDOWN"); break; }
                    SystemEvent(ev, _) => { info!("vCPU {id}: system event {ev:#x}"); break; }
                    _ => { debug!("vCPU {id}: unhandled exit {:?}", exit); }
                }
            }
            Err(e) if e.errno() == libc::EINTR => continue,
            Err(e) => { warn!("vCPU {id}: KVM_RUN error: {e}"); break; }
        }
    }
    serial.lock().unwrap().flush();
    info!("vCPU {id} exited run loop");
}

const KVM_IO_OUT: u8 = 1;

fn handle_io(id: u64, run: &KvmRunPtr, serial: &Arc<Mutex<Serial>>) {
    let port = run.io_port();
    let direction = run.io_direction();
    let size = run.io_size();
    if port >= SERIAL_BASE && port < SERIAL_BASE + 8 {
        if direction == KVM_IO_OUT {
            let byte = run.io_data_u8();
            serial.lock().unwrap().write_port(port, byte);
        } else {
            let val = serial.lock().unwrap().read_port(port);
            let offset = run.io_data_offset() as usize;
            unsafe {
                let dst = (run.ptr as *mut u8).add(offset);
                let count = run.io_count() as usize;
                for i in 0..count { *dst.add(i) = val; }
            }
        }
        return;
    }
    if port == 0x604 && direction == KVM_IO_OUT {
        info!("vCPU {id}: ACPI reset");
        std::process::exit(0);
    }
    debug!("vCPU {id}: unhandled IO port={port:#x} dir={direction} size={size}");
}

fn handle_mmio_read(id: u64, addr: u64, data: &mut [u8], vnet: &Arc<Mutex<NetState>>, vblk: &Option<Arc<Mutex<BlkState>>>) {
    if addr >= VIRTIO_BLK_MMIO_BASE && addr < VIRTIO_BLK_MMIO_BASE + VIRTIO_BLK_MMIO_SIZE {
        if let Some(blk) = vblk.as_ref() {
            let val = blk.lock().unwrap().mmio_read(addr - VIRTIO_BLK_MMIO_BASE);
            let bytes = val.to_le_bytes();
            let n = data.len().min(4);
            data[..n].copy_from_slice(&bytes[..n]);
            return;
        }
    }
    if addr >= VIRTIO_NET_MMIO_BASE && addr < VIRTIO_NET_MMIO_BASE + VIRTIO_NET_MMIO_SIZE {
        let val = vnet.lock().unwrap().mmio_read(addr - VIRTIO_NET_MMIO_BASE);
        let bytes = val.to_le_bytes();
        let n = data.len().min(4);
        data[..n].copy_from_slice(&bytes[..n]);
        return;
    }
    debug!("vCPU {id}: MMIO read {addr:#x} len={}", data.len());
    data.fill(0);
}

fn handle_mmio_write(id: u64, addr: u64, data: &[u8], serial: &Arc<Mutex<Serial>>, vnet: &Arc<Mutex<NetState>>, vblk: &Option<Arc<Mutex<BlkState>>>) {
    let val32 = if data.len() >= 4 { u32::from_le_bytes([data[0], data[1], data[2], data[3]]) }
                else if !data.is_empty() { data[0] as u32 } else { 0 };
    if addr >= 0x09000000 && addr < 0x09001000 && !data.is_empty() {
        serial.lock().unwrap().write_port(SERIAL_BASE, data[0]);
        return;
    }
    if addr >= VIRTIO_BLK_MMIO_BASE && addr < VIRTIO_BLK_MMIO_BASE + VIRTIO_BLK_MMIO_SIZE {
        if let Some(blk) = vblk.as_ref() { blk.lock().unwrap().mmio_write(addr - VIRTIO_BLK_MMIO_BASE, val32); return; }
    }
    if addr >= VIRTIO_NET_MMIO_BASE && addr < VIRTIO_NET_MMIO_BASE + VIRTIO_NET_MMIO_SIZE {
        vnet.lock().unwrap().mmio_write(addr - VIRTIO_NET_MMIO_BASE, val32);
        return;
    }
    debug!("vCPU {id}: MMIO write {addr:#x} val={val32:#x}");
}

fn configure_cpuid(vm: &Vm, fd: &VcpuFd) -> Result<()> {
    let mut cpuid = vm.kvm().get_supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)?;
    for entry in cpuid.as_mut_slice() {
        match entry.function {
            1 => { entry.ecx |= 1 << 31; }
            0x4000_0000 => {
                entry.eax = 0x4000_0001;
                entry.ebx = 0x4b4d_564b;
                entry.ecx = 0x564b_4d56;
                entry.edx = 0x4d00_0000;
            }
            _ => {}
        }
    }
    fd.set_cpuid2(&cpuid)?;
    Ok(())
}

fn configure_msrs(fd: &VcpuFd) -> Result<()> {
    let msrs = Msrs::from_entries(&[
        kvm_bindings::kvm_msr_entry { index: 0x174, data: 0, ..Default::default() },
        kvm_bindings::kvm_msr_entry { index: 0x175, data: 0, ..Default::default() },
        kvm_bindings::kvm_msr_entry { index: 0x176, data: 0, ..Default::default() },
    ])?;
    fd.set_msrs(&msrs)?;
    Ok(())
}

fn configure_regs(fd: &VcpuFd, kernel: &KernelLoadResult) -> Result<()> {
    let mut regs = kvm_regs::default();
    regs.rflags = 0x0000_0000_0000_0002;
    regs.rip    = kernel.kernel_load.offset;
    regs.rsi    = kernel.boot_params_addr;
    regs.rsp    = 0x0008_0000;
    fd.set_regs(&regs)?;
    Ok(())
}

fn configure_sregs(fd: &VcpuFd, _mem: &GuestMemory) -> Result<()> {
    let mut sregs = fd.get_sregs()?;
    let code_seg = kvm_bindings::kvm_segment {
        base: 0, limit: 0xffff_ffff, selector: 0x08,
        type_: 0xb, present: 1, dpl: 0, db: 1, s: 1, l: 0, g: 1, avl: 0,
        ..Default::default()
    };
    let data_seg = kvm_bindings::kvm_segment {
        base: 0, limit: 0xffff_ffff, selector: 0x10,
        type_: 0x3, present: 1, dpl: 0, db: 1, s: 1, l: 0, g: 1, avl: 0,
        ..Default::default()
    };
    sregs.cs = code_seg;
    sregs.ds = data_seg;
    sregs.es = data_seg;
    sregs.fs = data_seg;
    sregs.gs = data_seg;
    sregs.ss = data_seg;
    sregs.gdt.base  = 0x5000;
    sregs.gdt.limit = 4 * 8 - 1;
    sregs.cr0  = 0x0000_0011;
    sregs.cr4  = 0;
    sregs.efer = 0;
    fd.set_sregs(&sregs)?;
    Ok(())
}

trait Context<T> { fn context(self, msg: &str) -> Result<T>; }
impl<T> Context<T> for std::result::Result<T, kvm_ioctls::Error> {
    fn context(self, msg: &str) -> Result<T> { self.map_err(|e| anyhow::anyhow!("{msg}: {e}")) }
}
impl<T> Context<T> for std::result::Result<T, std::io::Error> {
    fn context(self, msg: &str) -> Result<T> { self.map_err(|e| anyhow::anyhow!("{msg}: {e}")) }
}
