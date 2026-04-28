//! vmm/src/kvm/loader.rs — Linux x86 boot protocol (bzImage)
//!
//! Implementa el protocolo de arranque de Linux para x86/x86_64.
//! Documentación oficial: Documentation/x86/boot.rst en el kernel de Linux.
//!
//! Flujo de arranque:
//!
//!   1. Leer el bzImage y parsear el boot header (sector 1, offset 0x1F1)
//!   2. Verificar magic number (0xAA55) y protocolo de arranque (>= 2.02)
//!   3. Configurar la "zero page" (boot_params en memoria del guest)
//!   4. Copiar el kernel descomprimido (vmlinux) a 1 MiB en memoria del guest
//!   5. Copiar la command line a memoria del guest
//!   6. Opcionalmente copiar el initrd (ramdisk inicial)
//!   7. Configurar los registros del vCPU para el entry point del kernel
//!
//! Mapa de memoria resultante en el guest:
//!
//!   0x0000_0000  →  0x0000_7FFF   Real mode IVT, BDA
//!   0x0000_7C00  →  0x0000_7DFF   Boot sector (no usado en protegido)
//!   0x0000_8000  →  0x0000_FFFF   zero page (boot_params struct)
//!   0x0001_0000  →  0x0009_FFFF   Command line (en 0x20000)
//!   0x0010_0000  →  0x????_????   Kernel (1 MiB = base de carga estándar)
//!   0x????_????  →  0x????_????   Initrd (si hay)

use std::path::Path;
use std::fs;
use anyhow::{bail, Context, Result};
use tracing::{debug, info};

use super::memory::GuestMemory;

// ── Constantes del boot protocol ──────────────────────────────────────────

/// Magic number en el boot sector (offset 0x1FE)
const BOOT_MAGIC: u16 = 0xAA55;

/// Magic "HdrS" en el setup header (offset 0x202)
const HDR_MAGIC: u32 = 0x53726448;

/// Dirección base de carga del kernel (1 MiB)
pub const KERNEL_LOAD_ADDR: u64 = 0x0010_0000;

/// Dirección de la zero page (boot_params)
pub const ZERO_PAGE_ADDR: u64 = 0x0000_7000;

/// Dirección de la command line
pub const CMDLINE_ADDR: u64 = 0x0002_0000;

/// Tamaño máximo de la command line
const CMDLINE_MAX: usize = 4096;

/// Dirección del initrd (al final del primer GiB)
const INITRD_ADDR: u64 = 0x3000_0000;

// ── Estructuras del boot header ───────────────────────────────────────────

/// Boot header del kernel de Linux (offset 0x1F1 en la imagen)
/// Ver: Documentation/x86/boot.rst, tabla "The Real-Mode Kernel Header"
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BootHeader {
    pub setup_sects:       u8,    // 0x1F1 número de sectores del setup
    pub root_flags:        u16,   // 0x1F2
    pub syssize:           u32,   // 0x1F4 tamaño del kernel en párrafos de 16 bytes
    pub ram_size:          u16,   // 0x1F8
    pub vid_mode:          u16,   // 0x1FA
    pub root_dev:          u16,   // 0x1FC
    pub boot_flag:         u16,   // 0x1FE → debe ser 0xAA55
    pub jump:              u16,   // 0x200 jump instruction
    pub header:            u32,   // 0x202 → debe ser "HdrS" = 0x53726448
    pub version:           u16,   // 0x206 boot protocol version
    pub realmode_swtch:    u32,   // 0x208
    pub start_sys_seg:     u16,   // 0x20C
    pub kernel_version:    u16,   // 0x20E
    pub type_of_loader:    u8,    // 0x210 → 0xFF = undefined bootloader
    pub loadflags:         u8,    // 0x211 flags de carga
    pub setup_move_size:   u16,   // 0x212
    pub code32_start:      u32,   // 0x214 entry point modo protegido
    pub ramdisk_image:     u32,   // 0x218 dirección initrd
    pub ramdisk_size:      u32,   // 0x21C tamaño initrd
    pub bootsect_kludge:   u32,   // 0x220
    pub heap_end_ptr:      u16,   // 0x224
    pub ext_loader_ver:    u8,    // 0x226
    pub ext_loader_type:   u8,    // 0x227
    pub cmd_line_ptr:      u32,   // 0x228 puntero a command line
    pub initrd_addr_max:   u32,   // 0x22C dirección máxima initrd
    pub kernel_alignment:  u32,   // 0x230
    pub relocatable_kernel:u8,    // 0x234
    pub min_alignment:     u8,    // 0x235
    pub xloadflags:        u16,   // 0x236
    pub cmdline_size:      u32,   // 0x238 tamaño máximo de cmdline
    pub hardware_subarch:  u32,   // 0x23C
    pub hardware_subarch_data: u64, // 0x240
    pub payload_offset:    u32,   // 0x248
    pub payload_length:    u32,   // 0x24C
    pub setup_data:        u64,   // 0x250
    pub pref_address:      u64,   // 0x258 dirección preferida de carga
    pub init_size:         u32,   // 0x260 espacio necesario en memoria
    pub handover_offset:   u32,   // 0x264 EFI handover offset
}

/// loadflags bits
const LOADED_HIGH:   u8 = 1 << 0; // kernel cargado sobre 1MiB
const CAN_USE_HEAP:  u8 = 1 << 7; // bootloader puede usar heap

/// E820 memory map entry types
const E820_RAM:      u32 = 1;
const E820_RESERVED: u32 = 2;

/// E820 entry en la zero page
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
struct E820Entry {
    addr: u64,
    size: u64,
    entry_type: u32,
}

// ── Loader ────────────────────────────────────────────────────────────────

pub struct KernelLoader {
    pub entry_point:   u64,
    pub kernel_end:    u64,
    pub initrd_start:  Option<u64>,
    pub initrd_size:   u64,
}

/// Carga un bzImage en la memoria del guest y configura la zero page.
/// Devuelve la dirección del entry point para los registros del vCPU.
pub fn load_bzimage(
    mem:        &mut GuestMemory,
    kernel_path:&Path,
    cmdline:    &str,
    initrd_path:Option<&Path>,
    mem_mib:    u64,
) -> Result<KernelLoader> {
    info!("Loading kernel: {}", kernel_path.display());

    let kernel_bytes = fs::read(kernel_path)
        .with_context(|| format!("reading kernel: {}", kernel_path.display()))?;

    // ── 1. Parsear boot header ────────────────────────────────────────────
    let header = parse_boot_header(&kernel_bytes)?;

    // Verificar magic numbers
    if header.boot_flag != BOOT_MAGIC {
        bail!("Invalid boot magic: {:#x} (expected {:#x})", header.boot_flag, BOOT_MAGIC);
    }
    if header.header != HDR_MAGIC {
        bail!("Not a valid bzImage (HdrS magic missing)");
    }

    let proto_ver = header.version;
    if proto_ver < 0x0202 {
        bail!("Boot protocol {:#x} too old — need >= 2.02", proto_ver);
    }

    debug!("Boot protocol: {:#x}", proto_ver);
    debug!("Kernel loadflags: {:#b}", header.loadflags);

    let is_bzimage = header.loadflags & LOADED_HIGH != 0;
    if !is_bzimage {
        bail!("Only bzImage (LOADED_HIGH) is supported");
    }

    // ── 2. Extraer el kernel de la imagen ─────────────────────────────────
    // El bzImage tiene: setup sectors + kernel comprimido
    let setup_sects = if header.setup_sects == 0 { 4 } else { header.setup_sects as usize };
    let kernel_offset = (setup_sects + 1) * 512;

    if kernel_offset >= kernel_bytes.len() {
        bail!("Invalid kernel offset {kernel_offset} (file size {})", kernel_bytes.len());
    }

    let kernel_data = &kernel_bytes[kernel_offset..];
    let kernel_size = kernel_data.len() as u64;

    // ── 3. Copiar kernel a 1 MiB en la memoria del guest ─────────────────
    let load_addr = if header.relocatable_kernel != 0 {
        // Kernel relocatable: usar dirección preferida o KERNEL_LOAD_ADDR
        std::cmp::max(
            header.pref_address,
            KERNEL_LOAD_ADDR,
        )
    } else {
        KERNEL_LOAD_ADDR
    };

    info!("Copying kernel ({} MiB) to guest address {:#x}",
          kernel_size / (1024*1024), load_addr);

    mem.write_slice(kernel_data, load_addr)
        .context("writing kernel to guest memory")?;

    let kernel_end = load_addr + kernel_size;

    // ── 4. Cargar initrd si existe ────────────────────────────────────────
    let (initrd_start, initrd_size) = if let Some(path) = initrd_path {
        let initrd_data = fs::read(path)
            .with_context(|| format!("reading initrd: {}", path.display()))?;
        let size = initrd_data.len() as u64;

        // Poner el initrd debajo del límite initrd_addr_max, alineado a 4K
        let max_addr = if header.initrd_addr_max > 0 {
            header.initrd_addr_max as u64
        } else {
            0x3800_0000  // 896 MiB default
        };
        let addr = (std::cmp::min(INITRD_ADDR, max_addr) - size) & !0xFFF;

        info!("Copying initrd ({} KiB) to guest address {:#x}", size / 1024, addr);
        mem.write_slice(&initrd_data, addr)
            .context("writing initrd to guest memory")?;

        (Some(addr), size)
    } else {
        (None, 0)
    };

    // ── 5. Copiar command line ────────────────────────────────────────────
    if cmdline.len() > CMDLINE_MAX {
        bail!("Command line too long ({} > {})", cmdline.len(), CMDLINE_MAX);
    }

    let mut cmdline_bytes = cmdline.as_bytes().to_vec();
    cmdline_bytes.push(0); // null-terminate
    mem.write_slice(&cmdline_bytes, CMDLINE_ADDR)
        .context("writing cmdline to guest memory")?;

    debug!("Command line at {:#x}: {}", CMDLINE_ADDR, cmdline);

    // ── 6. Configurar la zero page (boot_params) ──────────────────────────
    let boot_params = build_boot_params(
        &header,
        mem_mib,
        initrd_start,
        initrd_size,
    );

    // Serializar boot_params a bytes y escribir en guest memory
    let params_bytes = unsafe {
        std::slice::from_raw_parts(
            &boot_params as *const BootParams as *const u8,
            std::mem::size_of::<BootParams>(),
        )
    };
    mem.write_slice(params_bytes, ZERO_PAGE_ADDR)
        .context("writing boot_params to guest memory")?;

    // ── 7. Configurar GDT (descriptor tables mínimas) ────────────────────
    setup_gdt(mem)?;

    info!("Kernel loaded: entry={:#x}, kernel_end={:#x}", load_addr, kernel_end);

    Ok(KernelLoader {
        entry_point: load_addr,
        kernel_end,
        initrd_start,
        initrd_size,
    })
}

// ── boot_params (zero page) ───────────────────────────────────────────────

/// Subset del boot_params struct que necesitamos llenar
/// (0x1000 bytes total — definido en arch/x86/include/uapi/asm/bootparam.h)
#[repr(C, packed)]
struct BootParams {
    _pad0:          [u8; 0x1E8],    // 0x000 – 0x1E7: screen_info, etc. (ceros)
    e820_entries:   u8,             // 0x1E8
    _pad1:          [u8; 8],        // 0x1E9 – 0x1F0
    // Offset 0x1F1: setup header (copiamos del bzImage)
    hdr:            BootHeader,     // 0x1F1 – ~0x268
    _pad2:          [u8; 0x290 - (0x1F1 + std::mem::size_of::<BootHeader>())],
    // 0x290: E820 memory map (máximo 128 entradas × 20 bytes)
    e820_table:     [E820Entry; 128],
}

fn build_boot_params(
    hdr:          &BootHeader,
    mem_mib:      u64,
    initrd_addr:  Option<u64>,
    initrd_size:  u64,
) -> BootParams {
    let mut params = BootParams {
        _pad0:        [0u8; 0x1E8],
        e820_entries: 0,
        _pad1:        [0u8; 8],
        hdr:          *hdr,
        _pad2:        [0u8; _],
        e820_table:   [E820Entry::default(); 128],
    };

    // Actualizar el header con nuestros valores
    params.hdr.type_of_loader  = 0xFF;    // bootloader desconocido
    params.hdr.loadflags       |= CAN_USE_HEAP;
    params.hdr.heap_end_ptr    = 0xFE00;  // heap justo antes del final del segmento
    params.hdr.cmd_line_ptr    = CMDLINE_ADDR as u32;
    params.hdr.cmdline_size    = CMDLINE_MAX as u32;

    if let (Some(addr), size) = (initrd_addr, initrd_size) {
        params.hdr.ramdisk_image = addr as u32;
        params.hdr.ramdisk_size  = size as u32;
    }

    // E820 memory map
    // Entrada 1: [0, 640 KiB] — RAM convencional
    let mut n = 0usize;
    params.e820_table[n] = E820Entry {
        addr:       0x0000_0000,
        size:       0x0009_F000,  // 636 KiB (deja 4K para BIOS data area)
        entry_type: E820_RAM,
    };
    n += 1;

    // Entrada 2: [640 KiB – 1 MiB] — reservado (VGA, ROM)
    params.e820_table[n] = E820Entry {
        addr:       0x000A_0000,
        size:       0x0006_0000,  // 384 KiB
        entry_type: E820_RESERVED,
    };
    n += 1;

    // Entrada 3: [1 MiB – RAM total] — RAM del guest
    let mem_bytes = mem_mib * 1024 * 1024;
    params.e820_table[n] = E820Entry {
        addr:       0x0010_0000,
        size:       mem_bytes - 0x0010_0000,
        entry_type: E820_RAM,
    };
    n += 1;

    params.e820_entries = n as u8;
    params
}

// ── GDT mínima para modo protegido ────────────────────────────────────────

const GDT_ADDR: u64 = 0x0000_5000;

fn setup_gdt(mem: &mut GuestMemory) -> Result<()> {
    // Descriptores GDT: null, código 32-bit, datos 32-bit, código 64-bit
    let gdt: [u64; 4] = [
        0x0000_0000_0000_0000,  // null descriptor
        0x00CF_9A00_0000_FFFF,  // código 32-bit: base=0, limit=4G, DPL=0, execute/read
        0x00CF_9200_0000_FFFF,  // datos 32-bit: base=0, limit=4G, DPL=0, read/write
        0x00AF_9A00_0000_FFFF,  // código 64-bit: L=1 (long mode)
    ];

    let gdt_bytes = unsafe {
        std::slice::from_raw_parts(
            gdt.as_ptr() as *const u8,
            gdt.len() * 8,
        )
    };

    mem.write_slice(gdt_bytes, GDT_ADDR)
        .context("writing GDT to guest memory")?;

    debug!("GDT written at {:#x}", GDT_ADDR);
    Ok(())
}

// ── Parse del boot header ─────────────────────────────────────────────────

fn parse_boot_header(data: &[u8]) -> Result<BootHeader> {
    const HDR_OFFSET: usize = 0x1F1;
    const HDR_SIZE:   usize = std::mem::size_of::<BootHeader>();

    if data.len() < HDR_OFFSET + HDR_SIZE {
        bail!("File too small to be a valid bzImage ({} bytes)", data.len());
    }

    let hdr = unsafe {
        let ptr = data[HDR_OFFSET..].as_ptr() as *const BootHeader;
        *ptr
    };

    Ok(hdr)
}

// ── Configurar registros del vCPU para arrancar el kernel ─────────────────

/// Registros que el vCPU debe tener al entrar al kernel.
/// Ver: Documentation/x86/boot.rst, "Running the kernel"
pub struct BootRegs {
    pub rip:  u64,    // entry point del kernel
    pub rsp:  u64,    // stack pointer inicial
    pub rbp:  u64,    // base pointer
    pub rsi:  u64,    // puntero a boot_params (zero page)
    pub rflags: u64,  // flags iniciales
    // Segmentos en modo protegido / long mode
    pub cs_selector:  u16,
    pub ds_selector:  u16,
    pub es_selector:  u16,
    pub ss_selector:  u16,
    pub fs_selector:  u16,
    pub gs_selector:  u16,
    pub cr0: u64,     // CR0: PE=1 (protected mode), PG=0 (paging off inicialmente)
    pub cr3: u64,     // CR3: page table base
    pub cr4: u64,     // CR4
    pub efer: u64,    // EFER: LME para long mode
    pub gdt_base: u64,
    pub gdt_limit: u16,
}

pub fn boot_regs(entry: u64) -> BootRegs {
    BootRegs {
        rip:    entry,
        rsp:    0x0008_0000,     // stack en 512 KiB
        rbp:    0,
        rsi:    ZERO_PAGE_ADDR,  // RSI → boot_params (requerido por el kernel)
        rflags: 0x0000_0002,     // Reserved bit siempre 1

        // Modo protegido de 32-bit (el kernel hace la transición a 64-bit)
        cs_selector: 0x10,   // índice 2 en GDT (código 32-bit)
        ds_selector: 0x18,   // índice 3 en GDT (datos 32-bit)
        es_selector: 0x18,
        ss_selector: 0x18,
        fs_selector: 0x18,
        gs_selector: 0x18,

        // CR0: PE=1 (protected mode enable), WP=1
        cr0:  0x0000_0011,
        cr3:  0,
        cr4:  0,
        efer: 0,

        gdt_base:  GDT_ADDR,
        gdt_limit: 4 * 8 - 1,   // 4 descriptores × 8 bytes - 1
    }
}
