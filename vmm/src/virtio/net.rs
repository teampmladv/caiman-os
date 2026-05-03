//! virtio/net.rs -- virtio-net MMIO device
//!
//! MMIO base: 0xD000_0000  size: 0x1000  IRQ: 5
//! Tell the guest via cmdline:
//!   virtio_mmio.device=0x1000@0xd0000000:5
//!
//! Data path:
//!   Guest TX -> virtqueue -> Tap::send() -> host network
//!   Host RX  -> Tap::recv() -> virtqueue -> inject IRQ -> guest

use std::sync::{Arc, Mutex};
use std::thread;
use anyhow::{Context, Result};
use vmm_sys_util::eventfd::EventFd;
use kvm_ioctls::VmFd;
use tracing::{debug, info, warn};

use crate::kvm::memory::GuestMemory;
use super::queue::{Virtqueue, VIRTQ_DESC_F_WRITE};
use super::tap::Tap;

// -- Constants -------------------------------------------------------------

pub const VIRTIO_NET_MMIO_BASE: u64 = 0xD000_0000;
pub const VIRTIO_NET_MMIO_SIZE: u64 = 0x1000;
pub const VIRTIO_NET_IRQ:       u32 = 5;

const VIRTIO_MAGIC:     u32 = 0x74726976; // "virt"
const VIRTIO_VERSION:   u32 = 2;
const VIRTIO_DEV_NET:   u32 = 1;
const VIRTIO_VENDOR:    u32 = 0x554D4551; // "QEMU"

const VIRTIO_NET_F_MAC: u64 = 1 << 5;
const VIRTIO_F_VERSION_1: u64 = 1 << 32;

// MMIO register offsets
const REG_MAGIC:          u64 = 0x000;
const REG_VERSION:        u64 = 0x004;
const REG_DEVICE_ID:      u64 = 0x008;
const REG_VENDOR_ID:      u64 = 0x00C;
const REG_DEVICE_FEATURES:u64 = 0x010;
const REG_FEATURES_SEL:   u64 = 0x014;
const REG_DRIVER_FEATURES:u64 = 0x020;
const REG_DRIVER_FEAT_SEL:u64 = 0x024;
const REG_QUEUE_SEL:      u64 = 0x030;
const REG_QUEUE_NUM_MAX:  u64 = 0x034;
const REG_QUEUE_NUM:      u64 = 0x038;
const REG_QUEUE_READY:    u64 = 0x044;
const REG_QUEUE_NOTIFY:   u64 = 0x050;
const REG_IRQ_STATUS:     u64 = 0x060;
const REG_IRQ_ACK:        u64 = 0x064;
const REG_STATUS:         u64 = 0x070;
const REG_QUEUE_DESC_LO:  u64 = 0x080;
const REG_QUEUE_DESC_HI:  u64 = 0x084;
const REG_QUEUE_AVAIL_LO: u64 = 0x090;
const REG_QUEUE_AVAIL_HI: u64 = 0x094;
const REG_QUEUE_USED_LO:  u64 = 0x0A0;
const REG_QUEUE_USED_HI:  u64 = 0x0A4;
const REG_CONFIG:         u64 = 0x100; // MAC address starts here

// -- Shared state ----------------------------------------------------------

pub struct NetState {
    pub mac:           [u8; 6],

    // MMIO config registers
    device_features_sel: u32,
    driver_features:     u64,
    queue_sel:           u32,
    status:              u32,
    irq_status:          u32,

    // Virtqueues: 0=RX, 1=TX
    pub queues: [Virtqueue; 2],
}

impl NetState {
    pub fn new(mac: [u8; 6]) -> Self {
        Self {
            mac,
            device_features_sel: 0,
            driver_features:     0,
            queue_sel:           0,
            status:              0,
            irq_status:          0,
            queues: [Virtqueue::new(256), Virtqueue::new(256)],
        }
    }

    /// Handle MMIO read from the guest
    pub fn mmio_read(&self, offset: u64) -> u32 {
        match offset {
            REG_MAGIC          => VIRTIO_MAGIC,
            REG_VERSION        => VIRTIO_VERSION,
            REG_DEVICE_ID      => VIRTIO_DEV_NET,
            REG_VENDOR_ID      => VIRTIO_VENDOR,
            REG_DEVICE_FEATURES => {
                let features = VIRTIO_NET_F_MAC | VIRTIO_F_VERSION_1;
                if self.device_features_sel == 0 {
                    features as u32
                } else {
                    (features >> 32) as u32
                }
            }
            REG_QUEUE_NUM_MAX  => 256,
            REG_QUEUE_READY    => self.queues[self.queue_sel as usize].ready as u32,
            REG_IRQ_STATUS     => self.irq_status,
            REG_STATUS         => self.status,

            // MAC address in config space
            REG_CONFIG..=0x105 => {
                let i = (offset - REG_CONFIG) as usize;
                if i < 6 { self.mac[i] as u32 } else { 0 }
            }
            _ => 0,
        }
    }

    /// Handle MMIO write from the guest
    pub fn mmio_write(&mut self, offset: u64, val: u32) {
        let q = self.queue_sel as usize;
        match offset {
            REG_FEATURES_SEL    => self.device_features_sel = val,
            REG_DRIVER_FEATURES => {
                if self.driver_features == 0 { // sel 0
                    self.driver_features = val as u64;
                } else {
                    self.driver_features |= (val as u64) << 32;
                }
            }
            REG_DRIVER_FEAT_SEL => {} // track if needed
            REG_QUEUE_SEL       => self.queue_sel = val & 1,
            REG_QUEUE_NUM       => self.queues[q].size = val as u16,
            REG_QUEUE_READY     => self.queues[q].ready = val != 0,
            REG_IRQ_ACK         => self.irq_status &= !val,
            REG_STATUS          => self.status = val,
            REG_QUEUE_DESC_LO   => {
                let hi = 0u32;
                self.queues[q].set_desc_table(val, hi);
            }
            REG_QUEUE_DESC_HI   => {
                let lo = 0u32; // caller must set lo first then hi in real impl
                self.queues[q].set_desc_table(lo, val);
            }
            REG_QUEUE_AVAIL_LO  => self.queues[q].set_avail_ring(val, 0),
            REG_QUEUE_AVAIL_HI  => self.queues[q].set_avail_ring(0, val),
            REG_QUEUE_USED_LO   => self.queues[q].set_used_ring(val, 0),
            REG_QUEUE_USED_HI   => self.queues[q].set_used_ring(0, val),
            REG_QUEUE_NOTIFY    => {} // handled by kick eventfd
            _ => {}
        }
    }
}

// -- VirtioNet device ------------------------------------------------------

pub struct VirtioNet {
    pub state:  Arc<Mutex<NetState>>,
    pub irqfd:  EventFd,
}

impl VirtioNet {
    pub fn new(vm_fd: &VmFd, mac: [u8; 6]) -> Result<Self> {
        let irqfd = EventFd::new(0).context("irqfd")?;

        // Register IRQ eventfd: writing it injects interrupt VIRTIO_NET_IRQ
        vm_fd.register_irqfd(&irqfd, VIRTIO_NET_IRQ)
            .map_err(|e| anyhow::anyhow!("KVM_IRQFD: {e}"))?;

        info!("virtio-net: mac={} mmio={:#x} irq={}",
            mac.map(|b| format!("{b:02x}")).join(":"),
            VIRTIO_NET_MMIO_BASE, VIRTIO_NET_IRQ);

        Ok(Self {
            state: Arc::new(Mutex::new(NetState::new(mac))),
            irqfd,
        })
    }

    /// Start the TX/RX data plane threads.
    /// Call after the VM is running.
    pub fn start_dataplane(
        &self,
        tap_name: &str,
        mem: Arc<GuestMemory>,
    ) -> Result<()> {
        let state   = Arc::clone(&self.state);
        let irqfd   = self.irqfd.try_clone().context("clone irqfd")?;
        let tap_name = tap_name.to_string();

        thread::Builder::new()
            .name("virtio-net-dp".into())
            .spawn(move || {
                if let Err(e) = dataplane_loop(tap_name, state, irqfd, mem) {
                    warn!("virtio-net dataplane error: {e}");
                }
            })
            .context("spawning dataplane thread")?;
        Ok(())
    }

    /// Inject an interrupt into the guest (called after adding to used ring).
    pub fn inject_irq(&self) {
        if let Err(e) = self.irqfd.write(1) {
            warn!("inject_irq: {e}");
        }
    }
}

// -- Data plane ------------------------------------------------------------

fn dataplane_loop(
    tap_name: String,
    state:    Arc<Mutex<NetState>>,
    irqfd:    EventFd,
    mem:      Arc<GuestMemory>,
) -> Result<()> {
    let mut tap = Tap::new(&tap_name)?;
    tap.up()?;

    let mut rx_buf = vec![0u8; 2048];
    let mut tx_buf = vec![0u8; 2048];

    info!("virtio-net dataplane running on TAP '{}'", tap.name());

    loop {
        // -- TX: drain guest TX queue -> TAP -------------------------------
        {
            let mut st = state.lock().unwrap();
            while let Some(head) = st.queues[1].next_avail(&mem) {
                let chain = st.queues[1].read_chain(&mem, head);
                let mut total = 0usize;
                for desc in &chain {
                    if desc.flags & VIRTQ_DESC_F_WRITE == 0 {
                        // guest-readable: copy data out
                        let n = desc.len as usize;
                        if total + n <= tx_buf.len() {
                            if let Ok(bytes) = mem.read_slice(desc.addr, n) {
                                tx_buf[total..total+n].copy_from_slice(&bytes);
                                total += n;
                            }
                        }
                    }
                }
                if total > 12 { // skip 12-byte virtio-net header
                    let _ = tap.send(&tx_buf[12..total]);
                    debug!("virtio-net TX: {} bytes", total - 12);
                }
                st.queues[1].add_used(&mem, head, total as u32);
            }
        }

        // -- RX: TAP -> guest RX queue --------------------------------------
        if let Some(n) = tap.recv(&mut rx_buf) {
            let mut st = state.lock().unwrap();
            if let Some(head) = st.queues[0].next_avail(&mem) {
                let chain = st.queues[0].read_chain(&mem, head);
                let mut written = 0u32;
                // First descriptor: 12-byte virtio-net header (zeros)
                let header = [0u8; 12];
                for desc in &chain {
                    if desc.flags & VIRTQ_DESC_F_WRITE != 0 {
                        if written == 0 && desc.len >= 12 {
                            let _ = mem.write_slice(&header, desc.addr);
                            let data_len = n.min(desc.len as usize - 12);
                            let _ = mem.write_slice(&rx_buf[..data_len], desc.addr + 12);
                            written = (12 + data_len) as u32;
                        }
                        break;
                    }
                }
                if written > 0 {
                    st.queues[0].add_used(&mem, head, written);
                    st.irq_status |= 0x1;
                    drop(st);
                    let _ = irqfd.write(1); // inject RX interrupt
                    debug!("virtio-net RX: {} bytes -> guest", n);
                }
            }
        }

        // Brief yield to avoid 100% CPU on empty queues
        std::thread::sleep(std::time::Duration::from_micros(100));
    }
}
