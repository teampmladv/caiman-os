//! cmdline.rs -- centralized kernel command line construction
//!
//! Single source of truth for the guest kernel cmdline. Only announces
//! virtio-mmio devices that actually exist, to avoid "Wrong magic value"
//! errors when the guest probes a device the VMM did not instantiate.

use crate::virtio::net::VIRTIO_NET_MMIO_BASE;
use crate::virtio::blk::VIRTIO_BLK_MMIO_BASE;

const VIRTIO_NET_IRQ: u8 = 5;
const VIRTIO_BLK_IRQ: u8 = 6;

#[derive(Default)]
pub struct CmdlineOpts<'a> {
    /// Root device, e.g. Some("/dev/vda") for boot-from-disk; None when
    /// the rootfs lives in the initrd.
    pub root_device: Option<&'a str>,
    /// Filesystem type for root, e.g. Some("ext4").
    pub rootfstype: Option<&'a str>,
    /// Announce virtio-net device on the cmdline.
    pub has_net: bool,
    /// Announce virtio-blk device on the cmdline (set true only when a
    /// disk is actually attached).
    pub has_disk: bool,
    /// IP autoconfig string for the NAT path, e.g. "ip=10.100.0.5::...".
    pub ip_config: Option<&'a str>,
    /// Extra per-distro flags, e.g. "i8042.noaux i8042.nokbd nomodules".
    pub extra: Option<&'a str>,
}

pub fn build_cmdline(opts: &CmdlineOpts) -> String {
    let mut parts: Vec<String> = vec![
        "earlycon=uart8250,io,0x3f8,115200n8".to_string(),
        "console=ttyS0,115200".to_string(),
    ];

    if let Some(root) = opts.root_device {
        parts.push(format!("root={root}"));
        if let Some(fstype) = opts.rootfstype {
            parts.push(format!("rootfstype={fstype}"));
        }
    }

    parts.push("rw".to_string());
    parts.push("reboot=k".to_string());
    parts.push("panic=1".to_string());

    if opts.has_net {
        parts.push(format!(
            "virtio_mmio.device=0x1000@{:#x}:{}",
            VIRTIO_NET_MMIO_BASE, VIRTIO_NET_IRQ
        ));
    }
    if opts.has_disk {
        parts.push(format!(
            "virtio_mmio.device=0x1000@{:#x}:{}",
            VIRTIO_BLK_MMIO_BASE, VIRTIO_BLK_IRQ
        ));
    }

    if let Some(ip) = opts.ip_config {
        parts.push(ip.to_string());
    }
    if let Some(extra) = opts.extra {
        parts.push(extra.to_string());
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_disk_omits_blk() {
        let cl = build_cmdline(&CmdlineOpts { has_net: true, has_disk: false, ..Default::default() });
        assert!(cl.contains("0xd0000000:5"));
        assert!(!cl.contains("0xd0010000:6"));
        assert!(!cl.contains("root="));
    }

    #[test]
    fn with_disk_includes_blk_and_root() {
        let cl = build_cmdline(&CmdlineOpts {
            root_device: Some("/dev/vda"),
            rootfstype: Some("ext4"),
            has_net: true,
            has_disk: true,
            ..Default::default()
        });
        assert!(cl.contains("0xd0010000:6"));
        assert!(cl.contains("root=/dev/vda"));
        assert!(cl.contains("rootfstype=ext4"));
    }

    #[test]
    fn extra_flags_appended() {
        let cl = build_cmdline(&CmdlineOpts { has_net: true, extra: Some("i8042.noaux"), ..Default::default() });
        assert!(cl.ends_with("i8042.noaux"));
    }
}
