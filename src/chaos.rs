//! Chaos Engineering for DRFE-R
//!
//! Injects network failures to test system resilience:
//! - Network partitions
//! - Packet drops
//! - Delays and jitter
//! - Clock drift
//! - Node crashes

use std::collections::HashSet;
use std::sync::RwLock;

use crate::coordinates::NodeId;

/// Random number generator for chaos
fn random_f64() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    ((nanos % 10000) as f64) / 10000.0
}

/// Chaos injection engine
pub struct ChaosEngine {
    /// Probability of network partition (0.0 to 1.0)
    pub partition_probability: f64,
    /// Probability of packet drop (0.0 to 1.0)
    pub packet_drop_rate: f64,
    /// Delay range in milliseconds (min, max)
    pub delay_range_ms: (u64, u64),
    /// Clock drift in milliseconds (can be negative)
    pub clock_drift_ms: i64,
    /// Currently partitioned node groups
    partitions: RwLock<Vec<HashSet<NodeId>>>,
    /// Currently crashed nodes
    crashed_nodes: RwLock<HashSet<NodeId>>,
    /// Whether chaos is enabled
    pub enabled: bool,
    /// Statistics
    stats: RwLock<ChaosStats>,
}

/// Chaos injection statistics
#[derive(Debug, Clone, Default)]
pub struct ChaosStats {
    pub packets_dropped: u64,
    pub packets_delayed: u64,
    pub partitions_created: u64,
    pub nodes_crashed: u64,
    pub connections_broken: u64,
}

impl ChaosEngine {
    pub fn new() -> Self {
        Self {
            partition_probability: 0.0,
            packet_drop_rate: 0.0,
            delay_range_ms: (0, 0),
            clock_drift_ms: 0,
            partitions: RwLock::new(Vec::new()),
            crashed_nodes: RwLock::new(HashSet::new()),
            enabled: false,
            stats: RwLock::new(ChaosStats::default()),
        }
    }

    /// Create a chaos engine with aggressive settings for testing
    pub fn aggressive() -> Self {
        Self {
            partition_probability: 0.1,
            packet_drop_rate: 0.05,
            delay_range_ms: (10, 500),
            clock_drift_ms: 100,
            partitions: RwLock::new(Vec::new()),
            crashed_nodes: RwLock::new(HashSet::new()),
            enabled: true,
            stats: RwLock::new(ChaosStats::default()),
        }
    }

    /// Should this packet be dropped?
    pub fn should_drop_packet(&self, _from: &NodeId, _to: &NodeId) -> bool {
        if !self.enabled {
            return false;
        }
        
        if random_f64() < self.packet_drop_rate {
            self.stats.write().unwrap().packets_dropped += 1;
            return true;
        }
        false
    }

    /// Get delay for a packet in milliseconds
    pub fn get_delay_ms(&self, _from: &NodeId, _to: &NodeId) -> u64 {
        if !self.enabled || self.delay_range_ms.1 == 0 {
            return 0;
        }
        
        let range = self.delay_range_ms.1 - self.delay_range_ms.0;
        let delay = self.delay_range_ms.0 + (random_f64() * range as f64) as u64;
        
        if delay > 0 {
            self.stats.write().unwrap().packets_delayed += 1;
        }
        delay
    }

    /// Create a network partition
    pub fn create_partition(&self, group_a: Vec<NodeId>, group_b: Vec<NodeId>) {
        if !self.enabled {
            return;
        }
        
        let mut partitions = self.partitions.write().unwrap();
        partitions.push(group_a.into_iter().collect());
        partitions.push(group_b.into_iter().collect());
        
        self.stats.write().unwrap().partitions_created += 1;
    }

    /// Check if two nodes can communicate
    pub fn can_communicate(&self, from: &NodeId, to: &NodeId) -> bool {
        if !self.enabled {
            return true;
        }
        
        // Check if either node is crashed
        let crashed = self.crashed_nodes.read().unwrap();
        if crashed.contains(from) || crashed.contains(to) {
            return false;
        }
        
        // Check partitions
        let partitions = self.partitions.read().unwrap();
        for i in 0..partitions.len() {
            if partitions[i].contains(from) {
                // From is in partition i, check if to is in a different partition
                for j in 0..partitions.len() {
                    if i != j && partitions[j].contains(to) {
                        return false; // In different partitions
                    }
                }
            }
        }
        
        // Random partition check based on probability
        if random_f64() < self.partition_probability {
            self.stats.write().unwrap().connections_broken += 1;
            return false;
        }
        
        true
    }

    /// Crash a node
    pub fn crash_node(&self, node_id: NodeId) {
        if !self.enabled {
            return;
        }
        
        self.crashed_nodes.write().unwrap().insert(node_id);
        self.stats.write().unwrap().nodes_crashed += 1;
    }

    /// Recover a crashed node
    pub fn recover_node(&self, node_id: &NodeId) {
        self.crashed_nodes.write().unwrap().remove(node_id);
    }

    /// Heal all partitions
    pub fn heal_partitions(&self) {
        self.partitions.write().unwrap().clear();
    }

    /// Get current time with drift
    pub fn get_time_with_drift(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        
        (now + self.clock_drift_ms).max(0) as u64
    }

    /// Is a node crashed?
    pub fn is_crashed(&self, node_id: &NodeId) -> bool {
        self.crashed_nodes.read().unwrap().contains(node_id)
    }

    /// Get statistics
    pub fn get_stats(&self) -> ChaosStats {
        self.stats.read().unwrap().clone()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        *self.stats.write().unwrap() = ChaosStats::default();
    }

    /// Reset all chaos state
    pub fn reset(&self) {
        self.partitions.write().unwrap().clear();
        self.crashed_nodes.write().unwrap().clear();
        self.reset_stats();
    }

    /// Enable or disable chaos injection
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl Default for ChaosEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Chaos test scenario
#[derive(Debug, Clone)]
pub struct ChaosScenario {
    pub name: String,
    pub description: String,
    pub actions: Vec<ChaosAction>,
}

/// Individual chaos action
#[derive(Debug, Clone)]
pub enum ChaosAction {
    /// Wait for specified duration
    Wait { ms: u64 },
    /// Create network partition
    Partition { groups: Vec<Vec<String>> },
    /// Heal all partitions
    HealPartitions,
    /// Crash specified nodes
    CrashNodes { node_ids: Vec<String> },
    /// Recover specified nodes
    RecoverNodes { node_ids: Vec<String> },
    /// Set packet drop rate
    SetDropRate { rate: f64 },
    /// Set delay range
    SetDelayRange { min_ms: u64, max_ms: u64 },
    /// Verify condition
    Verify { condition: VerifyCondition },
}

/// Verification conditions
#[derive(Debug, Clone)]
pub enum VerifyCondition {
    /// All nodes can reach destination
    AllReachable { destination: String },
    /// Delivery success rate is above threshold
    SuccessRateAbove { threshold: f64 },
    /// Average hops is below threshold
    AvgHopsBelow { threshold: f64 },
}

/// Preset chaos scenarios for testing
pub struct ChaosScenarios;

impl ChaosScenarios {
    /// Network partition scenario
    pub fn network_partition() -> ChaosScenario {
        ChaosScenario {
            name: "Network Partition".to_string(),
            description: "Split network into two groups and verify recovery".to_string(),
            actions: vec![
                ChaosAction::Partition { 
                    groups: vec![
                        vec!["node_0".into(), "node_1".into(), "node_2".into()],
                        vec!["node_3".into(), "node_4".into()],
                    ]
                },
                ChaosAction::Wait { ms: 5000 },
                ChaosAction::HealPartitions,
                ChaosAction::Wait { ms: 2000 },
                ChaosAction::Verify { 
                    condition: VerifyCondition::SuccessRateAbove { threshold: 0.95 }
                },
            ],
        }
    }

    /// Cascading node failures
    pub fn cascading_failures() -> ChaosScenario {
        ChaosScenario {
            name: "Cascading Failures".to_string(),
            description: "Crash nodes sequentially and verify system stability".to_string(),
            actions: vec![
                ChaosAction::CrashNodes { node_ids: vec!["node_0".into()] },
                ChaosAction::Wait { ms: 1000 },
                ChaosAction::CrashNodes { node_ids: vec!["node_1".into()] },
                ChaosAction::Wait { ms: 1000 },
                ChaosAction::CrashNodes { node_ids: vec!["node_2".into()] },
                ChaosAction::Wait { ms: 3000 },
                ChaosAction::Verify { 
                    condition: VerifyCondition::SuccessRateAbove { threshold: 0.80 }
                },
                ChaosAction::RecoverNodes { 
                    node_ids: vec!["node_0".into(), "node_1".into(), "node_2".into()] 
                },
            ],
        }
    }

    /// High latency scenario
    pub fn high_latency() -> ChaosScenario {
        ChaosScenario {
            name: "High Latency".to_string(),
            description: "Introduce extreme network delays".to_string(),
            actions: vec![
                ChaosAction::SetDelayRange { min_ms: 100, max_ms: 2000 },
                ChaosAction::Wait { ms: 10000 },
                ChaosAction::Verify { 
                    condition: VerifyCondition::SuccessRateAbove { threshold: 0.90 }
                },
                ChaosAction::SetDelayRange { min_ms: 0, max_ms: 0 },
            ],
        }
    }

    /// Packet loss scenario
    pub fn packet_loss() -> ChaosScenario {
        ChaosScenario {
            name: "Packet Loss".to_string(),
            description: "Simulate unreliable network with packet drops".to_string(),
            actions: vec![
                ChaosAction::SetDropRate { rate: 0.1 },
                ChaosAction::Wait { ms: 5000 },
                ChaosAction::Verify { 
                    condition: VerifyCondition::SuccessRateAbove { threshold: 0.85 }
                },
                ChaosAction::SetDropRate { rate: 0.0 },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chaos_disabled() {
        let chaos = ChaosEngine::new();
        assert!(!chaos.should_drop_packet(&NodeId::new("a"), &NodeId::new("b")));
        assert!(chaos.can_communicate(&NodeId::new("a"), &NodeId::new("b")));
    }

    #[test]
    fn test_node_crash() {
        let mut chaos = ChaosEngine::new();
        chaos.set_enabled(true);
        
        let node = NodeId::new("test");
        assert!(!chaos.is_crashed(&node));
        
        chaos.crash_node(node.clone());
        assert!(chaos.is_crashed(&node));
        assert!(!chaos.can_communicate(&node, &NodeId::new("other")));
        
        chaos.recover_node(&node);
        assert!(!chaos.is_crashed(&node));
    }

    #[test]
    fn test_partition() {
        let mut chaos = ChaosEngine::new();
        chaos.set_enabled(true);
        
        let a1 = NodeId::new("a1");
        let a2 = NodeId::new("a2");
        let b1 = NodeId::new("b1");
        
        chaos.create_partition(vec![a1.clone(), a2.clone()], vec![b1.clone()]);
        
        assert!(!chaos.can_communicate(&a1, &b1));
        
        chaos.heal_partitions();
        // After healing, should be able to communicate
    }
}
