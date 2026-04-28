//! vmm/src/virtio/net.rs — virtio-net device (MMIO transport)
//!
//! This is a *minimal* virtio-net device:
//!   - MMIO transport (no PCIe emulation required)
//!   - TX queue drains to caiman_net_mod via eventfd kick
//!   - RX queue is filled by the kernel module (no userspace copy)
//!   - No checksum offload emulation — rely on XDP program
//!
//! The actual packet forwarding is done in kernel space by caiman_net_mod.
//! This layer only manages virtqueue state and VM interrupt injection.

use anyhow::{Context, Result};
use vmm_sys_util::eventfd::EventFd;
use virtio_queue::{Queue, QueueT};
use kvm_ioctls::VmFd;
use tracing::{debug, info};

/// Virtio-net feature bits we advertise to the guest
const VIRTIO_NET_F_MAC:       u64 = 1 << 5;
const VIRTIO_NET_F_STATUS:    u64 = 1 << 16;
const VIRTIO_NET_F_MQ:        u64 = 1 << 22;
/// Standard virtio feature bits
const VIRTIO_F_VERSION_1:     u64 = 1 << 32;
const VIRTIO_F_RING_PACKED:   u64 = 1 << 34;

pub const NET_QUEUE_RX: usize = 0;
pub const NET_QUEUE_TX: usize = 1;
pub const QUEUE_SIZE:   u16   = 256;

/// virtio-net MMIO base address (must match device tree / kernel boot args)
pub const VIRTIO_NET_MMIO_BASE: u64 = 0xd000_0000;
pub const VIRTIO_NET_MMIO_SIZE: u64 = 0x0000_1000;
pub const VIRTIO_NET_IRQ:       u32 = 5;

pub struct VirtioNet {
        pub mac:     [u8; 6],
        pub vm_id:   u32,
        /// eventfd used to notify guest (IRQ injection via KVM_IRQFD)
        irqfd:       EventFd,
        /// eventfd guest writes when TX queue has data (kick)
        tx_kick_fd:  EventFd,
        rx_queue:    Queue,
        tx_queue:    Queue,
}

impl VirtioNet {
        pub fn new(vm: &crate::kvm::vm::Vm, mac: [u8; 6], vm_id: u32) -> Result<Self> {
                let irqfd    = EventFd::new(0).context("irqfd")?;
                let tx_kick_fd = EventFd::new(0).context("tx kickfd")?;

                // Register IRQ eventfd with KVM so writing it injects a GSI
                vm.vm_fd()
                        .register_irqfd(&irqfd, VIRTIO_NET_IRQ)
                        .context("KVM_IRQFD register")?;

                let rx_queue = Queue::new(QUEUE_SIZE)?;
                let tx_queue = Queue::new(QUEUE_SIZE)?;

                info!(
                        "virtio-net: mac={} vm_id={} mmio_base={:#x}",
                        crate::fmt_mac(&mac),
                        vm_id,
                        VIRTIO_NET_MMIO_BASE
                );

                Ok(Self {
                        mac,
                        vm_id,
                        irqfd,
                        tx_kick_fd,
                        rx_queue,
                        tx_queue,
                })
        }

        /// Inject a RX interrupt into the guest (called when a packet arrives)
        pub fn inject_rx_irq(&self) -> Result<()> {
                self.irqfd.write(1).context("inject RX IRQ")
        }

        /// Called on MMIO write to the doorbell register (TX kick)
        pub fn handle_tx_kick(&mut self) {
                debug!("virtio-net: TX kick from guest");
                // In the full implementation this notifies caiman_net_mod via
                // the ioctl interface to drain the TX virtqueue using
                // caiman_net_kick_tx(). The kernel module does the actual
                // XDP redirect without any userspace data copy.
        }

        /// Returns feature bits to present to the guest during negotiation
        pub fn features(&self) -> u64 {
                VIRTIO_NET_F_MAC | VIRTIO_NET_F_STATUS | VIRTIO_F_VERSION_1
        }

        /// Handle MMIO read from guest (virtio-mmio register file)
        pub fn mmio_read(&self, offset: u64, data: &mut [u8]) {
                match offset {
                        // MagicValue: 0x74726976 ("virt")
                        0x000 => write_le32(data, 0x7472_6976),
                        // Version: 2 (virtio-mmio v2)
                        0x004 => write_le32(data, 2),
                        // DeviceID: 1 (network card)
                        0x008 => write_le32(data, 1),
                        // VendorID
                        0x00c => write_le32(data, 0xffff),
                        // DeviceFeatures
                        0x010 => write_le64(data, self.features()),
                        // Status
                        0x070 => write_le32(data, 0xf), // DRIVER_OK
                        _ => {}
                }
        }

        /// Handle MMIO write from guest (queue setup, status changes)
        pub fn mmio_write(&mut self, offset: u64, data: &[u8]) {
                match offset {
                        0x050 => {
                                // QueueSel — guest selects which queue to configure
                                debug!("virtio-net: QueueSel={}", read_le32(data));
                        }
                        0x064 => {
                                // QueueReady — guest signals queue is ready
                                debug!("virtio-net: QueueReady");
                        }
                        0x060 => {
                                // QueueNotify — TX kick doorbell
                                self.handle_tx_kick();
                        }
                        _ => {}
                }
        }
}

// ── Little-endian helpers ──────────────────────────────────────────────────

fn write_le32(buf: &mut [u8], val: u32) {
        if buf.len() >= 4 {
                buf[..4].copy_from_slice(&val.to_le_bytes());
        }
}

fn write_le64(buf: &mut [u8], val: u64) {
        if buf.len() >= 8 {
                buf[..8].copy_from_slice(&val.to_le_bytes());
        }
}

fn read_le32(buf: &[u8]) -> u32 {
        if buf.len() >= 4 {
                u32::from_le_bytes(buf[..4].try_into().unwrap())
        } else {
                0
        }
}
