//! bridge.rs -- Caiman bridge + NAT setup
//! Crea caiman0, habilita IP forwarding y masquerade automaticamente

use anyhow::{Context, Result};
use std::process::Command;

const BRIDGE:   &str = "caiman0";
const BRIDGE_IP: &str = "10.100.0.1/24";

fn run(args: &[&str]) -> Result<()> {
    let status = Command::new(args[0]).args(&args[1..]).status()
        .with_context(|| format!("failed to run: {}", args.join(" ")))?;
    if !status.success() {
        anyhow::bail!("command failed: {}", args.join(" "));
    }
    Ok(())
}

fn run_ok(args: &[&str]) {
    let _ = Command::new(args[0]).args(&args[1..]).status();
}

/// Detect the host's default network interface (eth0, wlan0, enp3s0, etc.)
pub fn detect_uplink() -> String {
    // Try ip route to find default interface
    let out = Command::new("ip").args(["route", "show", "default"]).output();
    if let Ok(out) = out {
        let s = String::from_utf8_lossy(&out.stdout);
        // "default via 192.168.1.1 dev eth0 ..."
        if let Some(dev) = s.split_whitespace()
            .skip_while(|w| *w != "dev")
            .nth(1)
        {
            return dev.to_string();
        }
    }
    // Fallback candidates
    for iface in &["eth0", "ens3", "enp3s0", "wlan0", "wlp2s0", "bond0"] {
        if std::path::Path::new(&format!("/sys/class/net/{iface}")).exists() {
            return iface.to_string();
        }
    }
    "eth0".to_string()
}

/// Ensure caiman0 bridge exists and is configured
pub fn ensure_bridge() -> Result<()> {
    // Check if bridge already exists
    let exists = std::path::Path::new(&format!("/sys/class/net/{BRIDGE}")).exists();

    if !exists {
        tracing::info!("Creating bridge {BRIDGE}");
        run(&["ip", "link", "add", BRIDGE, "type", "bridge"])?;
        run(&["ip", "link", "set", BRIDGE, "up"])?;
        run(&["ip", "addr", "add", BRIDGE_IP, "dev", BRIDGE])?;
    }

    Ok(())
}

/// Enable IP forwarding + NAT masquerade (NAT mode)
pub fn ensure_nat(uplink: &str) -> Result<()> {
    // Enable IP forwarding
    std::fs::write("/proc/sys/net/ipv4/ip_forward", "1")
        .context("cannot enable ip_forward")?;

    // Persist across reboots
    run_ok(&["sysctl", "-w", "net.ipv4.ip_forward=1"]);

    // Add masquerade rule (idempotent -- -C checks, -A adds if not present)
    let check = Command::new("iptables")
        .args(["-t", "nat", "-C", "POSTROUTING",
               "-s", "10.100.0.0/24", "-o", uplink, "-j", "MASQUERADE"])
        .status();

    if check.map(|s| !s.success()).unwrap_or(true) {
        run(&["iptables", "-t", "nat", "-A", "POSTROUTING",
              "-s", "10.100.0.0/24", "-o", uplink, "-j", "MASQUERADE"])?;
        tracing::info!("NAT masquerade enabled on {uplink}");
    }

    // Allow forwarding between bridge and uplink
    run_ok(&["iptables", "-A", "FORWARD", "-i", BRIDGE, "-o", uplink, "-j", "ACCEPT"]);
    run_ok(&["iptables", "-A", "FORWARD", "-i", uplink, "-o", BRIDGE,
             "-m", "state", "--state", "RELATED,ESTABLISHED", "-j", "ACCEPT"]);

    Ok(())
}

/// Setup bridge mode -- VM gets IP visible on LAN
pub fn ensure_bridge_mode(uplink: &str) -> Result<()> {
    ensure_bridge()?;

    // Add uplink to bridge (makes VMs visible on LAN)
    let check = Command::new("bridge")
        .args(["link", "show", "dev", uplink])
        .output();

    if check.map(|o| o.stdout.is_empty()).unwrap_or(true) {
        run_ok(&["ip", "link", "set", uplink, "master", BRIDGE]);
        tracing::info!("Bridge mode: {uplink} added to {BRIDGE}");
    }

    Ok(())
}

pub fn bridge_name() -> &'static str {
    BRIDGE
}
