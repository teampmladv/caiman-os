//! virtio/queue.rs -- minimal virtqueue (split-ring, virtio 1.x spec)
//!
//! Implements the virtqueue ring buffer shared between guest and host.
//! No external crate dependencies -- reads directly from GuestMemory.
//!
//! Layout in guest RAM (set by driver via MMIO writes):
//!
//!   desc_table   -> array of 16-byte descriptors
//!   avail_ring   -> flags(2) + idx(2) + ring[N](2)
//!   used_ring    -> flags(2) + idx(2) + elem[N](8)

use anyhow::{bail, Result};
use crate::kvm::memory::GuestMemory;

// Descriptor flags
pub const VIRTQ_DESC_F_NEXT:     u16 = 0x1;
pub const VIRTQ_DESC_F_WRITE:    u16 = 0x2;  // guest-writable (device writes here)

const MAX_QUEUE_SIZE: u16 = 1024;

/// A single descriptor in the descriptor table
#[derive(Debug, Clone, Copy, Default)]
pub struct Descriptor {
    pub addr:  u64,
    pub len:   u32,
    pub flags: u16,
    pub next:  u16,
}

/// Virtqueue state -- configured by the guest via MMIO writes
pub struct Virtqueue {
    pub size:       u16,
    pub ready:      bool,

    pub desc_table:  u64,    // GPA of descriptor table
    pub avail_ring:  u64,    // GPA of available ring
    used_ring:   u64,    // GPA of used ring

    last_avail_idx: u16, // last index we processed from avail ring
    used_idx:       u16, // current index in used ring
}

impl Virtqueue {
    pub fn new(size: u16) -> Self {
        Self {
            size: size.min(MAX_QUEUE_SIZE),
            ready: false,
            desc_table:     0,
            avail_ring:     0,
            used_ring:      0,
            last_avail_idx: 0,
            used_idx:       0,
        }
    }

    // -- MMIO configuration (called by MMIO write handler) -----------------

    pub fn set_desc_table(&mut self, low: u32, high: u32) {
        if low  != 0 { self.desc_table = (self.desc_table & 0xFFFF_FFFF_0000_0000) | low as u64; }
        if high != 0 { self.desc_table = (self.desc_table & 0x0000_0000_FFFF_FFFF) | (high as u64) << 32; }
    }
    pub fn set_avail_ring(&mut self, low: u32, high: u32) {
        if low  != 0 { self.avail_ring = (self.avail_ring & 0xFFFF_FFFF_0000_0000) | low as u64; }
        if high != 0 { self.avail_ring = (self.avail_ring & 0x0000_0000_FFFF_FFFF) | (high as u64) << 32; }
    }
    pub fn set_used_ring(&mut self, low: u32, high: u32) {
        if low  != 0 { self.used_ring = (self.used_ring & 0xFFFF_FFFF_0000_0000) | low as u64; }
        if high != 0 { self.used_ring = (self.used_ring & 0x0000_0000_FFFF_FFFF) | (high as u64) << 32; }
    }

    // -- Descriptor iteration ----------------------------------------------

    /// Returns the next available descriptor chain head, if any.
    pub fn next_avail(&mut self, mem: &GuestMemory) -> Option<u16> {
        let avail_idx = self.read_avail_idx(mem)?;
        if self.last_avail_idx == avail_idx {
            return None; // no new descriptors
        }
        let ring_idx = self.last_avail_idx % self.size;
        let desc_idx = self.read_avail_ring_entry(mem, ring_idx)?;
        self.last_avail_idx = self.last_avail_idx.wrapping_add(1);
        Some(desc_idx)
    }

    /// Read a full descriptor chain starting at `head`.
    pub fn read_chain(&self, mem: &GuestMemory, head: u16) -> Vec<Descriptor> {
        let mut chain = Vec::new();
        let mut idx = head;
        loop {
            let desc = match self.read_desc(mem, idx) {
                Some(d) => d,
                None    => break,
            };
            chain.push(desc);
            if desc.flags & VIRTQ_DESC_F_NEXT == 0 || chain.len() > 64 {
                break;
            }
            idx = desc.next;
        }
        chain
    }

    /// Mark descriptor chain as used, notify guest.
    pub fn add_used(&mut self, mem: &GuestMemory, head: u16, len: u32) -> bool {
        if self.used_ring == 0 { return false; }
        let ring_idx = self.used_idx % self.size;
        let elem_offset = self.used_ring
            + 4 // flags(2) + idx(2)
            + ring_idx as u64 * 8;

        // Write used element: id(4) + len(4)
        let _ = mem.write_slice(&(head as u32).to_le_bytes(), elem_offset);
        let _ = mem.write_slice(&len.to_le_bytes(), elem_offset + 4);

        self.used_idx = self.used_idx.wrapping_add(1);
        // Update used ring idx
        let _ = mem.write_slice(&self.used_idx.to_le_bytes(), self.used_ring + 2);
        true
    }

    // -- Private helpers ---------------------------------------------------

    fn read_avail_idx(&self, mem: &GuestMemory) -> Option<u16> {
        if self.avail_ring == 0 { return None; }
        let bytes = mem.read_slice(self.avail_ring + 2, 2).ok()?;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_avail_ring_entry(&self, mem: &GuestMemory, ring_idx: u16) -> Option<u16> {
        let offset = self.avail_ring + 4 + ring_idx as u64 * 2;
        let bytes = mem.read_slice(offset, 2).ok()?;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_desc(&self, mem: &GuestMemory, idx: u16) -> Option<Descriptor> {
        if self.desc_table == 0 || idx >= self.size { return None; }
        let offset = self.desc_table + idx as u64 * 16;
        let bytes = mem.read_slice(offset, 16).ok()?;
        Some(Descriptor {
            addr:  u64::from_le_bytes(bytes[0..8].try_into().ok()?),
            len:   u32::from_le_bytes(bytes[8..12].try_into().ok()?),
            flags: u16::from_le_bytes(bytes[12..14].try_into().ok()?),
            next:  u16::from_le_bytes(bytes[14..16].try_into().ok()?),
        })
    }
}
