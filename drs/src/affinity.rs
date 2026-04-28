//! drs/src/affinity.rs — VM affinity / anti-affinity rules
//!
//! Equivalent to vSphere DRS affinity rules. Two rule types:
//!
//!   Affinity      — VMs in group MUST/SHOULD run on the same host
//!   Anti-affinity — VMs in group MUST/SHOULD run on different hosts
//!
//! Rule scopes:
//!   Hard (MUST)   — migration rejected if it violates the rule
//!   Soft (SHOULD) — migration penalized in scoring if it violates
//!
//! Defined via VmAffinityRule CRD or as pod topologySpreadConstraints.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::monitor::ClusterSnapshot;
use crate::types::VmMetrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffinityRule {
    pub name:       String,
    pub rule_type:  AffinityType,
    pub scope:      AffinityScope,
    pub vm_ids:     Vec<u32>,
    pub vm_labels:  HashMap<String, String>,  // label selector alternative
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AffinityType {
    Affinity,      // VMs should be co-located
    AntiAffinity,  // VMs should be separated
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AffinityScope {
    Hard,   // Violation = reject migration
    Soft,   // Violation = penalize score
}

pub struct AffinityChecker {
    rules: Vec<AffinityRule>,
}

impl AffinityChecker {
    pub fn new() -> Self {
        // Rules loaded from Kubernetes CRDs at startup, refreshed periodically
        Self { rules: Vec::new() }
    }

    pub fn load_from_k8s(rules: Vec<AffinityRule>) -> Self {
        Self { rules }
    }

    /// Returns true if migrating `vm` to `target_node` is allowed.
    pub fn migration_allowed(
        &self,
        vm:          &VmMetrics,
        target_node: &str,
        snap:        &ClusterSnapshot,
    ) -> bool {
        for rule in &self.rules {
            if !self.vm_in_rule(vm.vm_id, rule) { continue; }

            match rule.rule_type {
                AffinityType::AntiAffinity if rule.scope == AffinityScope::Hard => {
                    // Check if any co-rule VM is already on target_node
                    for other_vm_id in &rule.vm_ids {
                        if *other_vm_id == vm.vm_id { continue; }
                        if self.vm_on_node(*other_vm_id, target_node, snap) {
                            return false;  // Hard anti-affinity violation
                        }
                    }
                }
                AffinityType::Affinity if rule.scope == AffinityScope::Hard => {
                    // Check that all co-rule VMs are on target_node (or being migrated there)
                    // Simplified: at least one must be there
                    let any_on_target = rule.vm_ids.iter()
                        .filter(|&&id| id != vm.vm_id)
                        .any(|&id| self.vm_on_node(id, target_node, snap));
                    if !any_on_target && !rule.vm_ids.is_empty() {
                        return false;  // Hard affinity violation
                    }
                }
                _ => {}  // Soft rules handled in scoring
            }
        }
        true
    }

    /// Soft affinity penalty score adjustment [-1.0, +1.0]
    pub fn soft_score_adjustment(&self, vm: &VmMetrics, target_node: &str, snap: &ClusterSnapshot) -> f64 {
        let mut adjustment = 0.0f64;
        for rule in &self.rules {
            if !self.vm_in_rule(vm.vm_id, rule) { continue; }
            if rule.scope != AffinityScope::Soft  { continue; }

            let affinity_satisfied = rule.vm_ids.iter()
                .filter(|&&id| id != vm.vm_id)
                .any(|&id| self.vm_on_node(id, target_node, snap));

            match rule.rule_type {
                AffinityType::Affinity     => adjustment += if affinity_satisfied { 0.1 } else { -0.1 },
                AffinityType::AntiAffinity => adjustment += if affinity_satisfied { -0.1 } else { 0.1 },
            }
        }
        adjustment
    }

    fn vm_in_rule(&self, vm_id: u32, rule: &AffinityRule) -> bool {
        rule.vm_ids.contains(&vm_id)
    }

    fn vm_on_node(&self, vm_id: u32, node: &str, snap: &ClusterSnapshot) -> bool {
        snap.nodes.iter()
            .find(|n| n.hostname == node)
            .map(|n| n.vms.iter().any(|v| v.vm_id == vm_id))
            .unwrap_or(false)
    }
}
