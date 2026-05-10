//! virtio/blk.rs -- virtio-blk MMIO device
//!
//! Fix: eventfd kick en lugar de busy polling -- elimina el deadlock
//! con el vCPU que tambien necesita el lock para MMIO writes.

use std::fs::OpenOptions;
use std::os::unix::fs::FileExt;
use std::sync::{Arc, Mutex};
use std::thread;
use anyhow::{Context, Result};
use vmm_sys_util::eventfd::EventFd;
use kvm_ioctls::VmFd;
use tracing::{debug, info, warn};

use crate::kvm::memory::GuestMemory;
use super::queue::{Virtqueue, VIRTQ_DESC_F_WRITE};

pub const VIRTIO_BLK_MMIO_BASE: u64 = 0xD001_0000;
pub const VIRTIO_BLK_MMIO_SIZE: u64 = 0x1000;
pub const VIRTIO_BLK_IRQ:       u32 = 6;

const VIRTIO_MAGIC:        u32 = 0x74726976;
const VIRTIO_VERSION:      u32 = 2;
const VIRTIO_DEV_BLK:      u32 = 2;
const VIRTIO_VENDOR:       u32 = 0x554D4551;
const VIRTIO_BLK_F_RO:    u64 = 1 << 5;
const VIRTIO_F_VERSION_1: u64 = 1 << 32;
const SECTOR_SIZE:         u64 = 512;

const T_IN:     u32 = 0;
const T_OUT:    u32 = 1;
const T_FLUSH:  u32 = 4;
const T_GET_ID: u32 = 8;
const S_OK:     u8  = 0;
const S_IOERR:  u8  = 1;
const S_UNSUPP: u8  = 2;

const REG_MAGIC:          u64 = 0x000;
const REG_VERSION:        u64 = 0x004;
const REG_DEVICE_ID:      u64 = 0x008;
const REG_VENDOR_ID:      u64 = 0x00C;
const REG_DEV_FEATURES:   u64 = 0x010;
const REG_FEATURES_SEL:   u64 = 0x014;
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

// ── MMIO state ─────────────────────────────────────────────────────────────

pub struct BlkState {
    sector_count: u64,
    read_only:    bool,
    features_sel: u32,
    queue_sel:    u32,
    status:       u32,
    irq_status:   u32,
    pub queue:    Virtqueue,
}

impl BlkState {
    pub fn new(sector_count: u64, read_only: bool) -> Self {
        Self {
            sector_count, read_only,
            features_sel: 0, queue_sel: 0,
            status: 0, irq_status: 0,
            queue: Virtqueue::new(128),
        }
    }

    pub fn mmio_read(&self, offset: u64) -> u32 {
        let features = VIRTIO_F_VERSION_1
            | if self.read_only { VIRTIO_BLK_F_RO } else { 0 };
        match offset {
            REG_MAGIC         => VIRTIO_MAGIC,
            REG_VERSION       => VIRTIO_VERSION,
            REG_DEVICE_ID     => VIRTIO_DEV_BLK,
            REG_VENDOR_ID     => VIRTIO_VENDOR,
            REG_DEV_FEATURES  => {
                if self.features_sel == 0 { features as u32 }
                else { (features >> 32) as u32 }
            }
            REG_QUEUE_NUM_MAX => 128,
            REG_QUEUE_READY   => self.queue.ready as u32,
            REG_IRQ_STATUS    => self.irq_status,
            REG_STATUS        => self.status,
            0x100             => (self.sector_count & 0xFFFF_FFFF) as u32,
            0x104             => (self.sector_count >> 32) as u32,
            _                 => 0,
        }
    }

    pub fn mmio_write(&mut self, offset: u64, val: u32, kick: &EventFd) {
        match offset {
            REG_FEATURES_SEL   => self.features_sel  = val,
            REG_QUEUE_SEL      => self.queue_sel      = val,
            REG_QUEUE_NUM      => self.queue.size     = val as u16,
            REG_QUEUE_READY    => self.queue.ready    = val != 0,
            REG_IRQ_ACK        => self.irq_status    &= !val,
            REG_STATUS         => self.status         = val,
            REG_QUEUE_DESC_LO  => self.queue.set_desc_table(val, 0),
            REG_QUEUE_DESC_HI  => self.queue.set_desc_table(0, val),
            REG_QUEUE_AVAIL_LO => self.queue.set_avail_ring(val, 0),
            REG_QUEUE_AVAIL_HI => self.queue.set_avail_ring(0, val),
            REG_QUEUE_USED_LO  => self.queue.set_used_ring(val, 0),
            REG_QUEUE_USED_HI  => self.queue.set_used_ring(0, val),
            // Guest kicked the device -- wake up dataplane
            REG_QUEUE_NOTIFY   => { tracing::info!("BLK QUEUE_NOTIFY kick"); let _ = kick.write(1); }
            _                  => {}
        }
    }
}

// ── VirtioBlk device ───────────────────────────────────────────────────────

pub struct VirtioBlk {
    pub state:  Arc<Mutex<BlkState>>,
    pub irqfd:  EventFd,
    pub kickfd: EventFd,
    image_path: String,
    read_only:  bool,
}

impl VirtioBlk {
    pub fn new(image_path: &str, read_only: bool) -> Result<Self> {
        let meta = std::fs::metadata(image_path)
            .with_context(|| format!("opening disk: {image_path}"))?;
        let sector_count = meta.len() / SECTOR_SIZE;

        let irqfd  = EventFd::new(0).context("blk irqfd")?;
        let kickfd = EventFd::new(0).context("blk kickfd")?;

        info!("virtio-blk: {} ({} MiB, {} sectors, ro={})",
            image_path, meta.len()/(1024*1024), sector_count, read_only);

        Ok(Self {
            state:      Arc::new(Mutex::new(BlkState::new(sector_count, read_only))),
            irqfd, kickfd,
            image_path: image_path.to_string(),
            read_only,
        })
    }

    pub fn register_irq(&self, vm_fd: &VmFd) -> Result<()> {
        vm_fd.register_irqfd(&self.irqfd, VIRTIO_BLK_IRQ)
            .map_err(|e| anyhow::anyhow!("KVM_IRQFD blk: {e}"))
    }

    pub fn start_dataplane(&self, mem: Arc<GuestMemory>) -> Result<()> {
        let state      = Arc::clone(&self.state);
        let irqfd      = self.irqfd.try_clone().context("clone blk irqfd")?;
        let kickfd     = self.kickfd.try_clone().context("clone blk kickfd")?;
        let image_path = self.image_path.clone();
        let read_only  = self.read_only;

        thread::Builder::new()
            .name("virtio-blk-dp".into())
            .spawn(move || {
                if let Err(e) = blk_dataplane(image_path, read_only, state, irqfd, kickfd, mem) {
                    warn!("virtio-blk dataplane exited: {e}");
                }
            })
            .context("spawn blk thread")?;
        Ok(())
    }
}

// ── Block request processing ────────────────────────────────────────────────

fn blk_dataplane(
    image_path: String,
    read_only:  bool,
    state:      Arc<Mutex<BlkState>>,
    irqfd:      EventFd,
    kickfd:     EventFd,
    mem:        Arc<GuestMemory>,
) -> Result<()> {
    let file = OpenOptions::new()
        .read(true)
        .write(!read_only)
        .open(&image_path)
        .with_context(|| format!("blk open {image_path}"))?;

    info!("virtio-blk dataplane running: {image_path}");

    loop {
        // Block until guest kicks us via QUEUE_NOTIFY -- no busy polling
        kickfd.read().ok();
        tracing::info!("BLK dataplane woke up -- processing queue");

        // Process all available requests in the queue
        loop {
            let (head, chain) = {
                let mut st = state.lock().unwrap();
                tracing::info!("BLK queue ready={} desc={:#x} avail={:#x}", st.queue.ready, st.queue.desc_table, st.queue.avail_ring);
                if !st.queue.ready || st.queue.avail_ring == 0 { break; }
                let Some(head) = st.queue.next_avail(&mem) else {
                    tracing::info!("BLK no requests available");
                    break;
                };
                tracing::info!("BLK processing head={head}");
                let chain = st.queue.read_chain(&mem, head);
                tracing::info!("BLK chain len={}", chain.len());
                (head, chain)
            };

            if chain.len() < 2 {
                let mut st = state.lock().unwrap();
                st.queue.add_used(&mem, head, 0);
                continue;
            }

            // Descriptor 0: request header (16 bytes)
            let hdr_desc  = chain[0];
            let hdr_bytes = match mem.read_slice(hdr_desc.addr, 16) {
                Ok(b) => b,
                Err(_) => { continue; }
            };
            let req_type = u32::from_le_bytes(hdr_bytes[0..4].try_into().unwrap_or([0u8;4]));
            let sector   = u64::from_le_bytes(hdr_bytes[8..16].try_into().unwrap_or([0u8;8]));
            let offset   = sector * SECTOR_SIZE;

            // Descriptor 1: data buffer
            let data_desc   = chain[1];
            let is_dev_write = data_desc.flags & VIRTQ_DESC_F_WRITE != 0;

            // Last descriptor: status byte
            let status_desc = chain[chain.len() - 1];

            // Execute I/O -- no lock held during file operations
            let status = match req_type {
                T_IN => {
                    if is_dev_write {
                        let mut buf = vec![0u8; data_desc.len as usize];
                        match file.read_at(&mut buf, offset) {
                            Ok(_) => { let _ = mem.write_slice(&buf, data_desc.addr); debug!("blk READ  sector={sector} len={}", buf.len()); S_OK }
                            Err(e) => { warn!("blk read err: {e}"); S_IOERR }
                        }
                    } else { S_IOERR }
                }
                T_OUT => {
                    if !read_only {
                        match mem.read_slice(data_desc.addr, data_desc.len as usize) {
                            Ok(buf) => match file.write_at(&buf, offset) {
                                Ok(_) => { debug!("blk WRITE sector={sector} len={}", buf.len()); S_OK }
                                Err(e) => { warn!("blk write err: {e}"); S_IOERR }
                            },
                            Err(_) => S_IOERR,
                        }
                    } else { S_IOERR }
                }
                T_FLUSH => S_OK,
                T_GET_ID => {
                    if is_dev_write && data_desc.len >= 20 {
                        let _ = mem.write_slice(b"caiman-blk-0        ", data_desc.addr);
                    }
                    S_OK
                }
                _ => S_UNSUPP,
            };

            // Write status byte + mark used + inject IRQ
            let _ = mem.write_slice(&[status], status_desc.addr);
            let used_len = if req_type == T_IN { data_desc.len } else { 0 };
            {
                let mut st = state.lock().unwrap();
                st.queue.add_used(&mem, head, used_len);
                st.irq_status |= 0x1;
            }
            tracing::info!("BLK IRQ injected status={status}");
            let _ = irqfd.write(1);
        }
    }
}
