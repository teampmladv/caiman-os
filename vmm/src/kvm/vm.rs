//! vmm/src/kvm/vm.rs -- KVM virtual machine setup

use anyhow::{Context, Result};
use kvm_bindings::{kvm_pit_config, KVM_PIT_SPEAKER_DUMMY};
use kvm_ioctls::{Kvm, VmFd};
use std::os::unix::io::AsRawFd;
use std::os::unix::io::AsRawFd as _;

use super::memory::GuestMemory;

pub struct Vm {
    kvm: Kvm,
    fd:  VmFd,
}

impl Vm {
    pub fn new(mem: &GuestMemory) -> Result<Self> {
        let kvm = Kvm::new().context("opening /dev/kvm")?;
        let api = kvm.get_api_version();
        anyhow::ensure!(api == 12, "unexpected KVM API version {api}");

        let fd = kvm.create_vm().context("KVM_CREATE_VM")?;

        // AMD requires TSS + identity map before irqchip creation
        // Without both, KVM_RUN returns ENOSPC on AMD processors
        fd.set_tss_address(0xfffbd000).context("KVM_SET_TSS_ADDRESS")?;
        unsafe {
            // KVM_SET_IDENTITY_MAP_ADDR -- required on AMD alongside TSS
            let identity_addr: u64 = 0xfffbc000;
            let ret = libc::ioctl(
                fd.as_raw_fd(),
                0x4008ae48u64 as libc::c_ulong, // KVM_SET_IDENTITY_MAP_ADDR
                &identity_addr as *const u64,
            );
            if ret < 0 {
                // Not fatal -- some kernels don't support this ioctl
                tracing::warn!("KVM_SET_IDENTITY_MAP_ADDR not supported (ok on Intel)");
            }
        }

        // In-kernel irqchip: no userspace interrupt emulation needed
        fd.create_irq_chip().context("KVM_CREATE_IRQCHIP")?;

        // In-kernel PIT2 for timer
        fd.create_pit2(kvm_pit_config {
            flags: KVM_PIT_SPEAKER_DUMMY,
            ..Default::default()
        }).context("KVM_CREATE_PIT2")?;

        // Regions are registered at GuestMemory construction time
        mem.register_with_vm(&fd).context("register memory")?;

        Ok(Self { kvm, fd })
    }

    pub fn fd(&self)    -> i32   { self.fd.as_raw_fd() }
    pub fn vm_fd(&self) -> &VmFd { &self.fd }
    pub fn kvm(&self)   -> &Kvm  { &self.kvm }
}
