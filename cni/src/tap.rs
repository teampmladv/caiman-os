//! tap.rs — TAP interface creation for VM networking
use anyhow::{Context, Result};
use std::process::Command;

/// Create a TAP interface and attach it to the bridge
pub fn create_tap(name: &str, bridge: &str) -> Result<()> {
    Command::new("ip").args(["tuntap", "add", "dev", name, "mode", "tap"])
        .status().context("ip tuntap add")?;
    Command::new("ip").args(["link", "set", name, "up"])
        .status().context("ip link set up")?;
    Command::new("ip").args(["link", "set", name, "master", bridge])
        .status().context("ip link set master")?;
    Ok(())
}

pub fn delete_tap(name: &str) -> Result<()> {
    Command::new("ip").args(["link", "del", name]).status()?;
    Ok(())
}
