// caiman-qcow2: read-only qcow2 reader for cold import.
//
// Purpose: walk a qcow2 (v2/v3) image and stream its virtual disk content
// out as a sparse raw image, so an imported guest disk can be handed to the
// VMM as a plain raw file. This is read-only on purpose: importing never
// needs the allocation/write machinery (refcount tables, snapshots), which
// is where most of qcow2's complexity lives.
//
// v1 scope (handled):
//   - qcow2 version 2 and 3 headers (big-endian)
//   - standard (uncompressed) clusters
//   - unallocated and zero clusters (emitted as holes -> sparse output)
//   - sparse raw output (seeks over holes, exact virtual size at the end)
//
// v1 scope (refused with a clear error, by design):
//   - compressed clusters (Proxmox stores qcow2 uncompressed by default;
//     descriptor layout is documented below for the follow-up)
//   - backing files / backing chains (linked clones) -- flatten on source
//   - encryption (crypt_method != 0) and external data files
//
// Reference: QEMU docs/interop/qcow2.txt is the canonical spec.
//
// Threading: this is synchronous std::fs I/O. Call it from the async import
// flow via tokio::task::spawn_blocking.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

const QCOW2_MAGIC: u32 = 0x5146_49fb; // "QFI\xfb"

// Bits 9..55 of an L1/L2 entry hold the (cluster-aligned) host offset.
const OFFSET_MASK: u64 = 0x00ff_ffff_ffff_fe00;
// L2 entry flags.
const L2_COMPRESSED: u64 = 1 << 62;
const L2_ZERO: u64 = 1; // v3 "all zeroes" flag for standard clusters.

// Incompatible feature bits we are willing to tolerate on a read-only open.
// bit 0 = dirty. We refuse anything else (corrupt, external data file,
// non-default compression type, or unknown future bits).
const INCOMPAT_DIRTY: u64 = 1 << 0;
const INCOMPAT_SUPPORTED: u64 = INCOMPAT_DIRTY;

#[derive(Debug)]
pub enum Qcow2Error {
    Io(io::Error),
    BadMagic(u32),
    UnsupportedVersion(u32),
    BadClusterBits(u32),
    Encrypted(u32),
    HasBackingFile,
    CompressedCluster,
    UnsupportedIncompatibleFeatures(u64),
    TruncatedHeader,
    OffsetOutOfRange(u64),
}

impl From<io::Error> for Qcow2Error {
    fn from(e: io::Error) -> Self {
        Qcow2Error::Io(e)
    }
}

impl std::fmt::Display for Qcow2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Qcow2Error::Io(e) => write!(f, "io error: {}", e),
            Qcow2Error::BadMagic(m) => write!(f, "not a qcow2 image (magic {:#010x})", m),
            Qcow2Error::UnsupportedVersion(v) => write!(f, "unsupported qcow2 version {}", v),
            Qcow2Error::BadClusterBits(b) => write!(f, "unreasonable cluster_bits {}", b),
            Qcow2Error::Encrypted(m) => write!(f, "encrypted image (crypt_method {}) not supported", m),
            Qcow2Error::HasBackingFile => {
                write!(f, "image has a backing file; flatten it first (qemu-img convert on source)")
            }
            Qcow2Error::CompressedCluster => {
                write!(f, "compressed cluster found; re-export the image uncompressed")
            }
            Qcow2Error::UnsupportedIncompatibleFeatures(b) => {
                write!(f, "unsupported incompatible feature bits {:#018x}", b)
            }
            Qcow2Error::TruncatedHeader => write!(f, "file too small to be a valid qcow2 header"),
            Qcow2Error::OffsetOutOfRange(o) => write!(f, "table offset {:#x} past end of file", o),
        }
    }
}

impl std::error::Error for Qcow2Error {}

type Result<T> = std::result::Result<T, Qcow2Error>;

// Big-endian helpers reading from a fixed header buffer.
fn be_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn be_u64(buf: &[u8], off: usize) -> u64 {
    u64::from_be_bytes([
        buf[off], buf[off + 1], buf[off + 2], buf[off + 3],
        buf[off + 4], buf[off + 5], buf[off + 6], buf[off + 7],
    ])
}

pub struct Qcow2Reader {
    file: File,
    file_len: u64,
    cluster_size: u64,
    cluster_bits: u32,
    virtual_size: u64,
    l2_entries: u64,
    l1_table: Vec<u64>,
    // Single-entry L2 cache keyed by the L2 table's host offset. Enough for
    // sequential streaming, which is how convert_to_raw walks the disk.
    l2_cache_offset: u64,
    l2_cache: Vec<u64>,
}

// Where a given guest cluster lives.
enum ClusterLoc {
    Zero,            // unallocated or explicit-zero: emit a hole
    Data(u64),       // host byte offset of cluster_size bytes
}

impl Qcow2Reader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;
        let file_len = file.metadata()?.len();

        // The header is always padded into the first cluster, so reading the
        // first 104 bytes (v3 header length) is safe for any real image.
        let mut hdr = [0u8; 104];
        if file_len < 72 {
            return Err(Qcow2Error::TruncatedHeader);
        }
        let to_read = if file_len >= 104 { 104 } else { 72 };
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut hdr[..to_read])?;

        let magic = be_u32(&hdr, 0);
        if magic != QCOW2_MAGIC {
            return Err(Qcow2Error::BadMagic(magic));
        }

        let version = be_u32(&hdr, 4);
        if version != 2 && version != 3 {
            return Err(Qcow2Error::UnsupportedVersion(version));
        }

        let backing_file_offset = be_u64(&hdr, 8);
        if backing_file_offset != 0 {
            return Err(Qcow2Error::HasBackingFile);
        }

        let cluster_bits = be_u32(&hdr, 20);
        // Sane range: 512 B .. 2 MiB clusters.
        if cluster_bits < 9 || cluster_bits > 21 {
            return Err(Qcow2Error::BadClusterBits(cluster_bits));
        }
        let cluster_size: u64 = 1u64 << cluster_bits;

        let virtual_size = be_u64(&hdr, 24);

        let crypt_method = be_u32(&hdr, 32);
        if crypt_method != 0 {
            return Err(Qcow2Error::Encrypted(crypt_method));
        }

        let l1_size = be_u32(&hdr, 36) as u64;
        let l1_table_offset = be_u64(&hdr, 40);

        if version >= 3 && to_read >= 80 {
            let incompatible = be_u64(&hdr, 72);
            if incompatible & !INCOMPAT_SUPPORTED != 0 {
                return Err(Qcow2Error::UnsupportedIncompatibleFeatures(
                    incompatible & !INCOMPAT_SUPPORTED,
                ));
            }
        }

        // Each L2 table holds cluster_size / 8 entries.
        let l2_entries = cluster_size / 8;

        // Load the L1 table (l1_size 8-byte big-endian entries).
        if l1_table_offset.checked_add(l1_size * 8).unwrap_or(u64::MAX) > file_len {
            return Err(Qcow2Error::OffsetOutOfRange(l1_table_offset));
        }
        let mut l1_raw = vec![0u8; (l1_size * 8) as usize];
        file.seek(SeekFrom::Start(l1_table_offset))?;
        file.read_exact(&mut l1_raw)?;
        let mut l1_table = Vec::with_capacity(l1_size as usize);
        for i in 0..(l1_size as usize) {
            l1_table.push(be_u64(&l1_raw, i * 8));
        }

        Ok(Qcow2Reader {
            file,
            file_len,
            cluster_size,
            cluster_bits,
            virtual_size,
            l2_entries,
            l1_table,
            l2_cache_offset: 0,
            l2_cache: Vec::new(),
        })
    }

    pub fn virtual_size(&self) -> u64 {
        self.virtual_size
    }

    pub fn cluster_size(&self) -> u64 {
        self.cluster_size
    }

    // Resolve which host location backs a given guest cluster index.
    fn resolve_cluster(&mut self, guest_cluster: u64) -> Result<ClusterLoc> {
        let l1_index = (guest_cluster / self.l2_entries) as usize;
        let l2_index = (guest_cluster % self.l2_entries) as usize;

        if l1_index >= self.l1_table.len() {
            return Ok(ClusterLoc::Zero);
        }
        let l2_offset = self.l1_table[l1_index] & OFFSET_MASK;
        if l2_offset == 0 {
            // No L2 table allocated for this range -> all zeroes.
            return Ok(ClusterLoc::Zero);
        }

        // Load the L2 table into the cache if needed.
        if l2_offset != self.l2_cache_offset || self.l2_cache.is_empty() {
            if l2_offset.checked_add(self.cluster_size).unwrap_or(u64::MAX) > self.file_len {
                return Err(Qcow2Error::OffsetOutOfRange(l2_offset));
            }
            let mut raw = vec![0u8; self.cluster_size as usize];
            self.file.seek(SeekFrom::Start(l2_offset))?;
            self.file.read_exact(&mut raw)?;
            let n = self.l2_entries as usize;
            let mut table = Vec::with_capacity(n);
            for i in 0..n {
                table.push(be_u64(&raw, i * 8));
            }
            self.l2_cache = table;
            self.l2_cache_offset = l2_offset;
        }

        let entry = self.l2_cache[l2_index];

        if entry & L2_COMPRESSED != 0 {
            // Deferred. Descriptor layout for the follow-up:
            //   let nbits = 62 - (cluster_bits - 8);
            //   host_offset = entry & ((1 << nbits) - 1);
            //   nb_sectors  = (entry >> nbits) & ((1 << (62 - nbits)) - 1);
            //   read nb_sectors*512 bytes from host_offset, inflate (deflate,
            //   or zstd if compression_type=1) to exactly cluster_size bytes.
            return Err(Qcow2Error::CompressedCluster);
        }

        let host_offset = entry & OFFSET_MASK;
        if entry & L2_ZERO != 0 || host_offset == 0 {
            // Explicit-zero cluster, or unallocated -> hole.
            return Ok(ClusterLoc::Zero);
        }
        Ok(ClusterLoc::Data(host_offset))
    }

    // Read a full cluster_size worth of bytes for a guest cluster into buf.
    // buf must be cluster_size long.
    fn read_cluster_into(&mut self, guest_cluster: u64, buf: &mut [u8]) -> Result<()> {
        match self.resolve_cluster(guest_cluster)? {
            ClusterLoc::Zero => {
                for b in buf.iter_mut() {
                    *b = 0;
                }
            }
            ClusterLoc::Data(host_offset) => {
                if host_offset.checked_add(self.cluster_size).unwrap_or(u64::MAX) > self.file_len {
                    return Err(Qcow2Error::OffsetOutOfRange(host_offset));
                }
                self.file.seek(SeekFrom::Start(host_offset))?;
                self.file.read_exact(buf)?;
            }
        }
        Ok(())
    }

    // Convert the whole virtual disk to a sparse raw file at out_path.
    // Zero/unallocated clusters are left as holes; the final file length is
    // set to the exact virtual size.
    pub fn convert_to_raw<P: AsRef<Path>>(&mut self, out_path: P) -> Result<()> {
        let mut out = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(out_path)?;

        let cluster_size = self.cluster_size as usize;
        let num_clusters = (self.virtual_size + self.cluster_size - 1) / self.cluster_size;
        let mut buf = vec![0u8; cluster_size];

        for c in 0..num_clusters {
            let guest_pos = c * self.cluster_size;
            let write_len = std::cmp::min(self.cluster_size, self.virtual_size - guest_pos) as usize;

            match self.resolve_cluster(c)? {
                ClusterLoc::Zero => {
                    // Leave a hole: do not write, sparseness is preserved.
                    continue;
                }
                ClusterLoc::Data(host_offset) => {
                    if host_offset.checked_add(self.cluster_size).unwrap_or(u64::MAX) > self.file_len {
                        return Err(Qcow2Error::OffsetOutOfRange(host_offset));
                    }
                    self.file.seek(SeekFrom::Start(host_offset))?;
                    self.file.read_exact(&mut buf)?;
                    out.seek(SeekFrom::Start(guest_pos))?;
                    out.write_all(&buf[..write_len])?;
                }
            }
        }

        // Ensure exact size, including any trailing hole.
        out.set_len(self.virtual_size)?;
        out.flush()?;
        Ok(())
    }

    // Random read of len bytes at a guest byte offset. Useful for inspection
    // (e.g. reading the partition table without a full conversion).
    pub fn read_at(&mut self, mut offset: u64, dst: &mut [u8]) -> Result<()> {
        let mut written = 0usize;
        let mut cluster_buf = vec![0u8; self.cluster_size as usize];
        while written < dst.len() {
            let guest_cluster = offset >> self.cluster_bits;
            let in_cluster = (offset & (self.cluster_size - 1)) as usize;
            self.read_cluster_into(guest_cluster, &mut cluster_buf)?;
            let take = std::cmp::min(self.cluster_size as usize - in_cluster, dst.len() - written);
            dst[written..written + take]
                .copy_from_slice(&cluster_buf[in_cluster..in_cluster + take]);
            written += take;
            offset += take as u64;
        }
        Ok(())
    }
}
