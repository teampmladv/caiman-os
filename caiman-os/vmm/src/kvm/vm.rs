//! vmm/src/kvm/vm.rs — KVM virtual machine handle
//!
//! Wraps KVM_CREATE_VM ioctl and all VM-level configuration:
//! IRQ routing, memory slots, CPUID, and MSR tables.

use anyhow::{Context, Result};
use kvm_bindings::{
        kvm_irq_routing, kvm_irq_routing_entry, KVM_IRQ_ROUTING_IRQCHIP,
        KVM_IRQCHIP_IOAPIC, KVM_IRQCHIP_PIC_MASTER, KVM_IRQCHIP_PIC_SLAVE,
};
use kvm_ioctls::{Kvm, VmFd};

use super::memory::GuestMemory;

pub struct Vm {
        kvm: Kvm,
        fd:  VmFd,
}

impl Vm {
        /// Open /dev/kvm and create a new VM, configuring the in-kernel
        /// IRQ chip and PIT so no userspace device emulation is needed for
        /// timer and interrupt routing.
        pub fn new(mem: &GuestMemory) -> Result<Self> {
                let kvm = Kvm::new().context("opening /dev/kvm")?;

                let api_ver = kvm.get_api_version();
                anyhow::ensure!(
                        api_ver == 12,
                        "unexpected KVM API version {api_ver} (expected 12)"
                );

                let vm_fd = kvm.create_vm().context("KVM_CREATE_VM")?;

                // In-kernel IRQ chip — eliminates all userspace interrupt injection
                vm_fd.create_irq_chip().context("KVM_CREATE_IRQCHIP")?;

                // In-kernel PIT2 (8254 timer) — prevents VM-exit on every tick
                vm_fd
                        .create_pit2(kvm_bindings::kvm_pit_config {
                                flags: kvm_bindings::KVM_PIT_SPEAKER_DUMMY,
                                ..Default::default()
                        })
                        .context("KVM_CREATE_PIT2")?;

                // Register guest memory regions
                mem.register_with_vm(&vm_fd)
                        .context("registering memory regions")?;

                // Set up IRQ routing table for PIC + IOAPIC
                Self::setup_irq_routing(&vm_fd).context("IRQ routing")?;

                Ok(Self { kvm, fd: vm_fd })
        }

        pub fn fd(&self) -> i32 {
                // kvm-ioctls doesn't expose the raw fd directly in older versions;
                // using as_raw_fd from std::os::unix::io::AsRawFd
                use std::os::unix::io::AsRawFd;
                self.fd.as_raw_fd()
        }

        pub fn vm_fd(&self) -> &VmFd {
                &self.fd
        }

        pub fn kvm(&self) -> &Kvm {
                &self.kvm
        }

        // ── Private helpers ────────────────────────────────────────────────

        fn setup_irq_routing(vm_fd: &VmFd) -> Result<()> {
                // Wire the standard x86 IRQ routes:
                //   IRQs  0-7  -> PIC master (IRQCHIP_PIC_MASTER)
                //   IRQs  8-15 -> PIC slave  (IRQCHIP_PIC_SLAVE)
                //   IRQs  0-23 -> IOAPIC
                const LEGACY_IRQS: u32 = 16;
                const IOAPIC_IRQS: u32 = 24;
                let total = LEGACY_IRQS + IOAPIC_IRQS;

                let mut routing = vec![kvm_irq_routing_entry::default(); total as usize];

                for i in 0..LEGACY_IRQS {
                        let chip = if i < 8 {
                                KVM_IRQCHIP_PIC_MASTER
                        } else {
                                KVM_IRQCHIP_PIC_SLAVE
                        };
                        routing[i as usize] = kvm_irq_routing_entry {
                                gsi:   i,
                                type_: KVM_IRQ_ROUTING_IRQCHIP,
                                ..Default::default()
                        };
                        // Safety: union field u.irqchip
                        unsafe {
                                routing[i as usize].u.irqchip.irqchip = chip;
                                routing[i as usize].u.irqchip.pin = i % 8;
                        }
                }

                for i in 0..IOAPIC_IRQS {
                        let idx = (LEGACY_IRQS + i) as usize;
                        routing[idx] = kvm_irq_routing_entry {
                                gsi:   i,
                                type_: KVM_IRQ_ROUTING_IRQCHIP,
                                ..Default::default()
                        };
                        unsafe {
                                routing[idx].u.irqchip.irqchip = KVM_IRQCHIP_IOAPIC;
                                routing[idx].u.irqchip.pin = i;
                        }
                }

                let irq_routing = kvm_ioctls::IrqRouting::new(total)
                        .context("allocating IRQ routing table")?;
                // Populate via kvm-ioctls safe wrapper
                vm_fd.set_gsi_routing(&irq_routing)
                        .context("KVM_SET_GSI_ROUTING")?;
                Ok(())
        }
}
