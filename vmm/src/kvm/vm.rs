//! vmm/src/kvm/vm.rs — KVM virtual machine setup
//! Wraps KVM_CREATE_VM + IRQ chip + PIT2 + memory registration.

use anyhow::{Context, Result};
use kvm_bindings::{
    kvm_irq_routing_entry, kvm_pit_config,
    KVM_IRQ_ROUTING_IRQCHIP,
    KVM_IRQCHIP_IOAPIC, KVM_IRQCHIP_PIC_MASTER, KVM_IRQCHIP_PIC_SLAVE,
    KVM_PIT_SPEAKER_DUMMY,
};
use kvm_ioctls::{Kvm, VmFd};
use std::os::unix::io::AsRawFd;

use super::memory::GuestMemory;

pub struct Vm {
    kvm: Kvm,
    fd:  VmFd,
}

impl Vm {
    pub fn new(mem: &GuestMemory) -> Result<Self> {
        let kvm = Kvm::new().context("opening /dev/kvm")?;
        let api_ver = kvm.get_api_version();
        anyhow::ensure!(api_ver == 12, "unexpected KVM API version {api_ver}");

        let vm_fd = kvm.create_vm().context("KVM_CREATE_VM")?;

        // In-kernel irqchip + PIT2 (no userspace device emulation)
        vm_fd.create_irq_chip().context("KVM_CREATE_IRQCHIP")?;
        vm_fd.create_pit2(kvm_pit_config {
            flags: KVM_PIT_SPEAKER_DUMMY,
            ..Default::default()
        }).context("KVM_CREATE_PIT2")?;

        // Register RAM regions
        mem.register_with_vm(&vm_fd).context("KVM_SET_USER_MEMORY_REGION")?;

        // IRQ routing: wire PIC + IOAPIC
        setup_irq_routing(&vm_fd).context("KVM_SET_GSI_ROUTING")?;

        Ok(Self { kvm, fd: vm_fd })
    }

    pub fn fd(&self)     -> i32    { self.fd.as_raw_fd() }
    pub fn vm_fd(&self)  -> &VmFd  { &self.fd }
    pub fn kvm(&self)    -> &Kvm   { &self.kvm }
}

fn setup_irq_routing(vm_fd: &VmFd) -> Result<()> {
    const LEGACY: u32 = 16;
    const IOAPIC: u32 = 24;
    let total = LEGACY + IOAPIC;

    // Build flat Vec of entries then pass via kvm_bindings FAM wrapper
    let mut entries: Vec<kvm_irq_routing_entry> =
        vec![kvm_irq_routing_entry::default(); total as usize];

    for i in 0..LEGACY {
        let chip = if i < 8 { KVM_IRQCHIP_PIC_MASTER } else { KVM_IRQCHIP_PIC_SLAVE };
        entries[i as usize].gsi   = i;
        entries[i as usize].type_ = KVM_IRQ_ROUTING_IRQCHIP;
        unsafe {
            entries[i as usize].u.irqchip.irqchip = chip;
            entries[i as usize].u.irqchip.pin     = i % 8;
        }
    }
    for i in 0..IOAPIC {
        let idx = (LEGACY + i) as usize;
        entries[idx].gsi   = i;
        entries[idx].type_ = KVM_IRQ_ROUTING_IRQCHIP;
        unsafe {
            entries[idx].u.irqchip.irqchip = KVM_IRQCHIP_IOAPIC;
            entries[idx].u.irqchip.pin     = i;
        }
    }

    // Use FAM wrapper from kvm-bindings
    let mut routing = kvm_bindings::IrqRouting::new(total)
        .context("allocating IrqRouting")?;
    let slots = routing.as_mut_slice();
    slots.copy_from_slice(&entries);
    vm_fd.set_gsi_routing(&routing).context("KVM_SET_GSI_ROUTING")?;
    Ok(())
}
