//! ebpf/mod.rs -- BPF map setup helpers
//! Creates per-VM pin directories used by the caiman_net.ko XDP program.
use anyhow::Result;
use std::fs;

/// Create BPF pin directory and write identity files for this VM.
/// caiman_net.ko reads these on XDP program attach.
pub fn setup_vm_maps(vm_id: u32, mac: &[u8; 6], pin_path: &str) -> Result<()> {
    fs::create_dir_all(pin_path)?;
    fs::write(format!("{}/vm_id", pin_path), vm_id.to_string())?;
    fs::write(
        format!("{}/mac", pin_path),
        mac.map(|b| format!("{b:02x}")).join(":"),
    )?;
    Ok(())
}
