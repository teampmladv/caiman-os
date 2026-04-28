//! vmm/src/kvm/memory.rs — Guest memory management
//!
//! Gestiona la memoria del guest via KVM_SET_USER_MEMORY_REGION.
//! Usa mmap anónimo para alocar la RAM del guest — KVM mapea ese
//! rango de memoria del proceso host como RAM visible por el guest.
//!
//! Layout:
//!   0x0000_0000 – 0x0009_FFFF  640 KiB RAM convencional
//!   0x000A_0000 – 0x000F_FFFF  384 KiB reservado (VGA/ROM)
//!   0x0010_0000 – <mem_end>    RAM extendida (kernel, initrd, heap)

use std::num::NonZeroUsize;
use anyhow::{Context, Result};
use kvm_ioctls::VmFd;
use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES};
use tracing::debug;

/// Región de memoria del guest
pub struct MemoryRegion {
    pub host_ptr: *mut u8,
    pub guest_addr: u64,
    pub size: usize,
    pub slot: u32,
}

unsafe impl Send for MemoryRegion {}
unsafe impl Sync for MemoryRegion {}

impl Drop for MemoryRegion {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.host_ptr as *mut libc::c_void, self.size);
        }
    }
}

/// Gestión completa de la memoria del guest
pub struct GuestMemory {
    regions: Vec<MemoryRegion>,
}

impl GuestMemory {
    /// Crear y registrar la memoria del guest en KVM.
    pub fn new(vm: &VmFd, mem_mib: u64, track_dirty: bool) -> Result<Self> {
        let mut regions = Vec::new();

        // Región 1: RAM convencional [0 – 640 KiB)
        let low_size = 640 * 1024usize;
        let low = alloc_region(vm, 0, 0x0000_0000, low_size, 0, track_dirty)
            .context("allocating low RAM")?;
        regions.push(low);

        // Región 2: RAM extendida [1 MiB – mem_end)
        // (el rango 640K–1M está reservado para VGA/ROM y no lo mapeamos)
        let high_size = (mem_mib * 1024 * 1024 - 0x0010_0000) as usize;
        let high = alloc_region(vm, 1, 0x0010_0000, high_size, 1, track_dirty)
            .context("allocating high RAM")?;
        regions.push(high);

        debug!("Guest memory: {} MiB ({} regions)", mem_mib, regions.len());
        Ok(Self { regions })
    }

    /// Escribir un slice de bytes en una dirección guest.
    pub fn write_slice(&mut self, data: &[u8], guest_addr: u64) -> Result<()> {
        let region = self.region_for(guest_addr, data.len())?;
        let offset  = (guest_addr - region.guest_addr) as usize;
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                region.host_ptr.add(offset),
                data.len(),
            );
        }
        Ok(())
    }

    /// Leer un slice de bytes desde una dirección guest.
    pub fn read_slice(&self, guest_addr: u64, len: usize) -> Result<Vec<u8>> {
        let region = self.region_for(guest_addr, len)?;
        let offset  = (guest_addr - region.guest_addr) as usize;
        let mut buf = vec![0u8; len];
        unsafe {
            std::ptr::copy_nonoverlapping(
                region.host_ptr.add(offset),
                buf.as_mut_ptr(),
                len,
            );
        }
        Ok(buf)
    }

    /// Obtener el puntero host para una dirección guest.
    pub fn host_ptr(&self, guest_addr: u64) -> Result<*const u8> {
        let region = self.region_for(guest_addr, 1)?;
        let offset  = (guest_addr - region.guest_addr) as usize;
        Ok(unsafe { region.host_ptr.add(offset) as *const u8 })
    }

    fn region_for(&self, guest_addr: u64, len: usize) -> Result<&MemoryRegion> {
        self.regions.iter().find(|r| {
            guest_addr >= r.guest_addr
                && guest_addr + len as u64 <= r.guest_addr + r.size as u64
        })
        .ok_or_else(|| anyhow::anyhow!(
            "guest address {:#x} (len {}) not in any memory region",
            guest_addr, len
        ))
    }

    fn region_for_mut(&mut self, guest_addr: u64, len: usize) -> Result<&mut MemoryRegion> {
        self.regions.iter_mut().find(|r| {
            guest_addr >= r.guest_addr
                && guest_addr + len as u64 <= r.guest_addr + r.size as u64
        })
        .ok_or_else(|| anyhow::anyhow!(
            "guest address {:#x} (len {}) not in any memory region",
            guest_addr, len
        ))
    }
}

fn alloc_region(
    vm:          &VmFd,
    slot:        u32,
    guest_addr:  u64,
    size:        usize,
    numa_node:   u32,
    track_dirty: bool,
) -> Result<MemoryRegion> {
    // Alocar memoria anónima con mmap
    let host_ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
            -1, 0,
        )
    };
    if host_ptr == libc::MAP_FAILED {
        anyhow::bail!("mmap failed for {} MiB region", size / (1024 * 1024));
    }

    // Registrar con KVM via KVM_SET_USER_MEMORY_REGION
    let flags = if track_dirty { KVM_MEM_LOG_DIRTY_PAGES } else { 0 };
    let region = kvm_userspace_memory_region {
        slot,
        flags,
        guest_phys_addr: guest_addr,
        memory_size:     size as u64,
        userspace_addr:  host_ptr as u64,
    };

    unsafe {
        vm.set_user_memory_region(region)
            .context("KVM_SET_USER_MEMORY_REGION")?;
    }

    debug!("Memory region slot={} guest={:#x} size={}MiB dirty_tracking={}",
           slot, guest_addr, size/(1024*1024), track_dirty);

    Ok(MemoryRegion {
        host_ptr: host_ptr as *mut u8,
        guest_addr,
        size,
        slot,
    })
}

    /// Register all memory regions with the VM.
    /// Called from Vm::new() after creating the VmFd.
    pub fn register_with_vm(&self, _vm: &kvm_ioctls::VmFd) -> anyhow::Result<()> {
        // Regions were already registered in alloc_region() via KVM_SET_USER_MEMORY_REGION
        // This is a no-op — regions are set up at construction time
        Ok(())
    }
