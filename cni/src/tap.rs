//! tap.rs -- TAP interface for VM networking
use anyhow::{Context, Result};
use std::process::Command;

fn run(args: &[&str]) -> Result<()> {
    Command::new(args[0]).args(&args[1..]).status()
        .with_context(|| format!("failed: {}", args.join(" ")))?;
    Ok(())
}

/// Create TAP interface and attach to bridge
pub fn create_tap(name: &str, bridge: &str) -> Result<()> {
    // Delete if exists (idempotent)
    let _ = Command::new("ip").args(["link", "del", name]).status();

    run(&["ip", "tuntap", "add", "dev", name, "mode", "tap"])?;
    run(&["ip", "link", "set", name, "up"])?;
    run(&["ip", "link", "set", name, "master", bridge])?;
    tracing::info!("TAP {name} created and attached to {bridge}");
    Ok(())
}

pub fn delete_tap(name: &str) -> Result<()> {
    let _ = Command::new("ip").args(["link", "del", name]).status();
    Ok(())
}

/// Set MAC address on TAP interface
pub fn set_mac(name: &str, mac: &str) -> Result<()> {
    run(&["ip", "link", "set", name, "address", mac])
}
