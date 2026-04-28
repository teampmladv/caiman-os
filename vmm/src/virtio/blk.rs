//! vmm/src/virtio/blk.rs — virtio-blk dispositivo de bloque
//!
//! Implementa un dispositivo virtio-blk MMIO que expone un archivo
//! del host como disco del guest (imagen raw o qcow2 via qemu-nbd).
//!
//! El guest ve un disco virtio estándar — usa el driver virtio_blk
//! del kernel de Linux. Las peticiones llegan por virtqueues y
//! las servimos leyendo/escribiendo el archivo de imagen.
//!
//! Virtqueues:
//!   Queue 0: requestq — todas las operaciones de I/O

use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::os::unix::fs::FileExt;
use anyhow::{bail, Context, Result};
use tracing::{debug, warn};

// Tipos de petición virtio-blk
const VIRTIO_BLK_T_IN:        u32 = 0;  // read
const VIRTIO_BLK_T_OUT:       u32 = 1;  // write
const VIRTIO_BLK_T_FLUSH:     u32 = 4;  // flush
const VIRTIO_BLK_T_GET_ID:    u32 = 8;  // device ID

// Status bytes de respuesta
const VIRTIO_BLK_S_OK:        u8 = 0;
const VIRTIO_BLK_S_IOERR:     u8 = 1;
const VIRTIO_BLK_S_UNSUPP:    u8 = 2;

// Tamaño de sector (siempre 512 bytes en virtio-blk)
const SECTOR_SIZE: u64 = 512;

/// Header de petición virtio-blk (descriptor 0 del guest)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct BlkReqHeader {
    req_type: u32,
    reserved: u32,
    sector:   u64,
}

pub struct VirtioBlk {
    image:     File,
    disk_size: u64,  // bytes
    read_only: bool,
}

impl VirtioBlk {
    pub fn new(image_path: &str, read_only: bool) -> Result<Self> {
        let image = OpenOptions::new()
            .read(true)
            .write(!read_only)
            .open(image_path)
            .with_context(|| format!("opening disk image: {image_path}"))?;

        let disk_size = image.metadata()?.len();
        if disk_size == 0 {
            bail!("Disk image {image_path} is empty");
        }

        debug!("VirtioBlk: {image_path} ({} MiB, read_only={read_only})",
               disk_size / (1024*1024));

        Ok(Self { image, disk_size, read_only })
    }

    /// Número de sectores del disco
    pub fn sector_count(&self) -> u64 {
        self.disk_size / SECTOR_SIZE
    }

    /// Procesar una petición I/O del guest.
    /// - header:   descriptor 0 (BlkReqHeader)
    /// - data:     descriptor 1 (buffer de datos, IN o OUT)
    /// - status:   descriptor 2 (1 byte de respuesta)
    pub fn process_request(
        &mut self,
        header_bytes: &[u8],
        data:         &mut [u8],
        is_write:     bool,
    ) -> u8 {
        if header_bytes.len() < std::mem::size_of::<BlkReqHeader>() {
            warn!("VirtioBlk: request header too short");
            return VIRTIO_BLK_S_IOERR;
        }

        let hdr: BlkReqHeader = unsafe {
            std::ptr::read_unaligned(header_bytes.as_ptr() as *const BlkReqHeader)
        };
        // Copy packed fields to local vars to avoid unaligned reference
        let req_type = hdr.req_type;
        let sector   = hdr.sector;
        let offset   = sector * SECTOR_SIZE;

        match req_type {
            VIRTIO_BLK_T_IN => {
                // READ: leer del disco → data buffer
                match self.image.read_at(data, offset) {
                    Ok(n) if n == data.len() => VIRTIO_BLK_S_OK,
                    Ok(n) => {
                        // Rellenar el resto con ceros (final de disco)
                        data[n..].fill(0);
                        VIRTIO_BLK_S_OK
                    }
                    Err(e) => {
                        warn!("VirtioBlk read error at offset {offset}: {e}");
                        VIRTIO_BLK_S_IOERR
                    }
                }
            }

            VIRTIO_BLK_T_OUT => {
                // WRITE: data buffer → disco
                if self.read_only {
                    return VIRTIO_BLK_S_IOERR;
                }
                match self.image.write_at(data, offset) {
                    Ok(_) => VIRTIO_BLK_S_OK,
                    Err(e) => {
                        warn!("VirtioBlk write error at offset {offset}: {e}");
                        VIRTIO_BLK_S_IOERR
                    }
                }
            }

            VIRTIO_BLK_T_FLUSH => {
                // FLUSH: asegurar que los datos están en disco
                match self.image.flush() {
                    Ok(_) => VIRTIO_BLK_S_OK,
                    Err(e) => {
                        warn!("VirtioBlk flush error: {e}");
                        VIRTIO_BLK_S_IOERR
                    }
                }
            }

            VIRTIO_BLK_T_GET_ID => {
                // Devolver ID del dispositivo (20 bytes)
                let id = b"caiman-blk-0        ";
                let n  = std::cmp::min(data.len(), id.len());
                data[..n].copy_from_slice(&id[..n]);
                VIRTIO_BLK_S_OK
            }

            _ => {
                warn!("VirtioBlk: unsupported request type {}", req_type);
                VIRTIO_BLK_S_UNSUPP
            }
        }
    }
}
