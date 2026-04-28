//! microseg/src/compiler.rs — label-selector policy → BPF map entries
//!
//! Translates MicroSegPolicy CRD objects into (src_id, dst_id, proto, port)
//! tuples that are written directly to the policy_map BPF hash map.
//!
//! Label → identity model:
//!   A VM's identity is the FNV-32 hash of its sorted label set.
//!   Example: {app=web, env=prod, tier=frontend} → identity=0xA3F2C1D4
//!   This means policies survive VM restarts as long as labels don't change.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc::Receiver;
use tracing::{info, warn};

use crate::maps::{self, PolicyKey, PolicyVal, Verdict};

// ── CRD types (mirrors microseg/k8s/microsegpolicy_crd.yaml) ──────────────

#[derive(Debug, Deserialize, Clone)]
pub struct MicroSegPolicy {
    pub metadata: PolicyMeta,
    pub spec:     PolicySpec,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PolicyMeta {
    pub name:      String,
    pub namespace: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PolicySpec {
    pub priority:    u8,
    pub action:      PolicyAction,
    pub from:        Vec<LabelSelector>,
    pub to:          Vec<LabelSelector>,
    pub ports:       Option<Vec<PortRule>>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PolicyAction {
    Allow,
    Deny,
    Log,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LabelSelector {
    #[serde(rename = "matchLabels")]
    pub match_labels: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PortRule {
    pub protocol: Option<String>,  // TCP, UDP, ICMP, or omit for any
    pub port:     Option<u16>,     // omit for any
}

// ── Main compilation loop ─────────────────────────────────────────────────

pub async fn policy_loop(
    mut rx:          Receiver<PolicyEvent>,
    bpf_pin_path:    &str,
) -> Result<()> {
    let mut policies: HashMap<String, MicroSegPolicy> = HashMap::new();

    loop {
        match rx.recv().await {
            Some(PolicyEvent::Added(p)) => {
                info!("Policy added: {}/{}", p.metadata.namespace, p.metadata.name);
                let key = format!("{}/{}", p.metadata.namespace, p.metadata.name);
                compile_policy(&p, bpf_pin_path).await
                    .unwrap_or_else(|e| warn!("compile {key}: {e}"));
                policies.insert(key, p);
            }
            Some(PolicyEvent::Deleted(name)) => {
                info!("Policy deleted: {name}");
                policies.remove(&name);
                // Recompile all remaining policies
                recompile_all(&policies, bpf_pin_path).await;
            }
            Some(PolicyEvent::Modified(p)) => {
                let key = format!("{}/{}", p.metadata.namespace, p.metadata.name);
                compile_policy(&p, bpf_pin_path).await
                    .unwrap_or_else(|e| warn!("recompile {key}: {e}"));
                policies.insert(key, p);
            }
            None => break,
        }
    }
    Ok(())
}

pub enum PolicyEvent {
    Added(MicroSegPolicy),
    Modified(MicroSegPolicy),
    Deleted(String),
}

// ── Compiler ─────────────────────────────────────────────────────────────

async fn compile_policy(policy: &MicroSegPolicy, bpf_pin_path: &str) -> Result<()> {
    let verdict = match policy.spec.action {
        PolicyAction::Allow => Verdict::Allow,
        PolicyAction::Deny  => Verdict::Deny,
        PolicyAction::Log   => Verdict::Log,
    };

    // Resolve all source and destination identities from label selectors
    let src_ids = resolve_identities(&policy.spec.from).await?;
    let dst_ids = resolve_identities(&policy.spec.to).await?;

    // Expand (src, dst, proto, port) combinations
    let ports = policy.spec.ports.as_deref().unwrap_or(&[]);

    for &src_id in &src_ids {
        for &dst_id in &dst_ids {
            if ports.is_empty() {
                // Any proto, any port
                let k = PolicyKey { src_id, dst_id, proto: 0, dst_port: 0 };
                let v = PolicyVal { verdict, priority: policy.spec.priority, rule_id: 0 };
                maps::update_policy_entry(bpf_pin_path, &k, &v)?;
            } else {
                for port_rule in ports {
                    let proto = proto_num(port_rule.protocol.as_deref());
                    let port  = port_rule.port.unwrap_or(0);
                    let k = PolicyKey { src_id, dst_id, proto, dst_port: port };
                    let v = PolicyVal { verdict, priority: policy.spec.priority, rule_id: 0 };
                    maps::update_policy_entry(bpf_pin_path, &k, &v)?;
                }
            }
        }
    }

    info!(
        "Compiled policy {}/{}: {} rules",
        policy.metadata.namespace, policy.metadata.name,
        src_ids.len() * dst_ids.len() * ports.len().max(1)
    );
    Ok(())
}

async fn recompile_all(policies: &HashMap<String, MicroSegPolicy>, bpf_pin_path: &str) {
    maps::clear_policy_map(bpf_pin_path).ok();
    for (_, policy) in policies {
        compile_policy(policy, bpf_pin_path).await
            .unwrap_or_else(|e| warn!("recompile: {e}"));
    }
}

/// Resolve label selectors to numeric identities by querying the
/// identity_map BPF map (populated by the caiman CNI when VMs start).
async fn resolve_identities(selectors: &[LabelSelector]) -> Result<Vec<u32>> {
    let mut ids = Vec::new();
    for sel in selectors {
        let id = labels_to_identity(&sel.match_labels);
        ids.push(id);
    }
    Ok(ids)
}

/// Deterministic identity from a label set (FNV-32 of sorted key=value pairs).
pub fn labels_to_identity(labels: &HashMap<String, String>) -> u32 {
    let mut pairs: Vec<String> = labels
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    pairs.sort();
    let combined = pairs.join(",");

    // FNV-32 hash
    let mut hash: u32 = 0x811c9dc5;
    for b in combined.bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    // Reserve 0 (unknown) and 1 (host)
    if hash < 2 { hash + 2 } else { hash }
}

fn proto_num(name: Option<&str>) -> u8 {
    match name {
        Some("TCP")  | Some("tcp")  => 6,
        Some("UDP")  | Some("udp")  => 17,
        Some("ICMP") | Some("icmp") => 1,
        _ => 0,  // any
    }
}
