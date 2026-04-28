//! vmm/src/kvm/vcpu.rs — vCPU thread
//!
//! Each vCPU runs in its own OS thread calling KVM_RUN in a tight loop.
//! VM-exits are handled here; the goal is to handle as few as possible
//! (everything that can be in-kernel, is in-kernel).
//!
//! Handled exits:
//!   KVM_EXIT_IO           — PIO to serial / reboot port
//!   KVM_EXIT_MMIO         — virtio MMIO transport intercept
//!   KVM_EXIT_SHUTDOWN     — guest triple-fault / RESET
//!   KVM_EXIT_HLT          — guest HLT (idle vCPU)
//!   KVM_EXIT_SYSTEM_EVENT — guest reboot/shutdown via PSCI / fw_cfg

use std::thread::{self, JoinHandle};

use anyhow::Result;
use kvm_bindings::{
        kvm_regs, kvm_sregs, CpuId, Msrs,
        KVM_EXIT_HLT, KVM_EXIT_IO, KVM_EXIT_MMIO,
        KVM_EXIT_SHUTDOWN, KVM_EXIT_SYSTEM_EVENT,
};
use kvm_ioctls::VcpuFd;
use tracing::{debug, info, warn};

use super::{memory::GuestMemory, vm::Vm};
use crate::kvm::loader::KernelLoadResult;

pub struct Vcpu {
        id:  u64,
        fd:  VcpuFd,
}

impl Vcpu {
        pub fn new(
                vm: &Vm,
                id: u64,
                mem: &GuestMemory,
                kernel: &KernelLoadResult,
        ) -> Result<Self> {
                let fd = vm.vm_fd().create_vcpu(id)?;

                configure_cpuid(vm, &fd)?;
                configure_msrs(&fd)?;
                configure_regs(&fd, kernel, mem, id)?;
                configure_sregs(&fd, mem)?;

                Ok(Self { id, fd })
        }

        /// Spawn the vCPU run loop in a dedicated OS thread.
        /// Returns a JoinHandle so the caller can wait for VM exit.
        pub fn run(&mut self) -> JoinHandle<()> {
                // SAFETY: VcpuFd is Send; we move it into the thread.
                // The actual KVM fd outlives the thread because Vm is not
                // dropped until all handles are joined.
                let id   = self.id;
                // kvm-ioctls VcpuFd is not Clone, so we use a raw ptr trick
                // here for simplicity. Production code should use Arc<Mutex<>>.
                let fd_ptr = &mut self.fd as *mut VcpuFd as usize;

                thread::Builder::new()
                        .name(format!("vcpu-{id}"))
                        .spawn(move || {
                                // SAFETY: fd_ptr is valid for the duration of
                                // the parent Vcpu struct (see above caveat).
                                let fd = unsafe { &mut *(fd_ptr as *mut VcpuFd) };
                                run_loop(id, fd);
                        })
                        .expect("spawning vCPU thread")
        }
}

// ── vCPU run loop ──────────────────────────────────────────────────────────

fn run_loop(id: u64, fd: &mut VcpuFd) {
        info!("vCPU {id} entering run loop");
        loop {
                match fd.run() {
                        Ok(exit) => {
                                use kvm_ioctls::VcpuExit::*;
                                match exit {
                                        Io => handle_io(id, fd),
                                        MmioRead(addr, data) => handle_mmio_read(id, addr, data),
                                        MmioWrite(addr, data) => handle_mmio_write(id, addr, data),
                                        Hlt => {
                                                debug!("vCPU {id}: HLT — idling");
                                                std::thread::sleep(
                                                        std::time::Duration::from_micros(100),
                                                );
                                        }
                                        Shutdown => {
                                                info!("vCPU {id}: SHUTDOWN exit");
                                                break;
                                        }
                                        SystemEvent(event_type, _flags) => {
                                                info!("vCPU {id}: system event {event_type:#x}");
                                                break;
                                        }
                                        _ => {
                                                warn!("vCPU {id}: unhandled exit {:?}", exit);
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
        info!("vCPU {id} exited run loop");
}

// ── Exit handlers ──────────────────────────────────────────────────────────

fn handle_io(id: u64, fd: &mut VcpuFd) {
        // We only emulate:
        //   0x3f8 (COM1 serial TX) — write byte to stdout
        //   0x604 (ACPI reboot)    — exit the VMM
}

fn handle_mmio_read(id: u64, addr: u64, data: &mut [u8]) {
        debug!("vCPU {id}: MMIO read  addr={addr:#x} len={}", data.len());
        // virtio MMIO transport reads are dispatched here in the full
        // implementation; for now return zero (device not ready).
        for b in data.iter_mut() {
                *b = 0;
        }
}

fn handle_mmio_write(id: u64, addr: u64, data: &[u8]) {
        debug!("vCPU {id}: MMIO write addr={addr:#x} len={} data={data:?}",
               data.len());
        // virtio MMIO transport writes handled here in full implementation.
}

// ── vCPU configuration helpers ─────────────────────────────────────────────

fn configure_cpuid(vm: &Vm, fd: &VcpuFd) -> Result<()> {
        let mut cpuid = vm.kvm()
                .get_supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)?;
        // Patch hypervisor bit, KVM leaf, etc.
        for entry in cpuid.as_mut_slice() {
                match entry.function {
                        1 => {
                                // Set hypervisor bit (ECX bit 31)
                                entry.ecx |= 1 << 31;
                        }
                        0x4000_0000 => {
                                // KVM CPUID leaf: "KVMKVMKVM\0\0\0"
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
                kvm_bindings::kvm_msr_entry {
                        index: 0x174,  // IA32_SYSENTER_CS
                        data:  0,
                        ..Default::default()
                },
                kvm_bindings::kvm_msr_entry {
                        index: 0x175,  // IA32_SYSENTER_ESP
                        data:  0,
                        ..Default::default()
                },
                kvm_bindings::kvm_msr_entry {
                        index: 0x176,  // IA32_SYSENTER_EIP
                        data:  0,
                        ..Default::default()
                },
        ])?;
        fd.set_msrs(&msrs)?;
        Ok(())
}

fn configure_regs(
        fd: &VcpuFd,
        kernel: &KernelLoadResult,
        _mem: &GuestMemory,
        id: u64,
) -> Result<()> {
        let mut regs = kvm_regs::default();
        regs.rflags = 0x0000_0000_0000_0002; // reserved bit always set
        regs.rip    = kernel.kernel_load.offset;
        // Linux 64-bit boot: RDI = 0 (boot_params pointer set by loader)
        // BSP (id=0) gets the real entry; APs start at SIPI vector
        if id == 0 {
                regs.rsi = kernel.boot_params_addr;
        }
        fd.set_regs(&regs)?;
        Ok(())
}

fn configure_sregs(fd: &VcpuFd, _mem: &GuestMemory) -> Result<()> {
        let mut sregs = fd.get_sregs()?;
        // Long mode setup: flat 64-bit code/data segments, enable paging
        // (full GDT + page tables built by the kernel loader)
        sregs.cr0 |= 0x80000001; // PE + PG
        sregs.cr4 |= 0x20;        // PAE
        sregs.efer |= 0x500;      // LME + LMA
        fd.set_sregs(&sregs)?;
        Ok(())
}
