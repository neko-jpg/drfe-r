//! Byzantine Fault Tolerance for DRFE-R
//!
//! Defends against adversarial nodes that:
//! - Broadcast false coordinates
//! - Drop packets intentionally
//! - Attempt to partition the network

use std::collections::HashMap;
use std::sync::RwLock;

use crate::coordinates::NodeId;
use crate::PoincareDiskPoint;

/// Coordinate validator using neighbor consensus
pub struct CoordinateValidator {
    /// Maximum allowed deviation from consensus
    pub max_deviation: f64,
    /// Minimum number of neighbors required for validation
    pub quorum_size: usize,
    /// Trust decay rate for misbehaving nodes
    pub trust_decay: f64,
    /// Historical coordinates for anomaly detection
    history: RwLock<HashMap<NodeId, Vec<PoincareDiskPoint>>>,
    /// Trust scores for each node
    trust_scores: RwLock<HashMap<NodeId, f64>>,
}

impl CoordinateValidator {
    pub fn new(max_deviation: f64, quorum_size: usize) -> Self {
        Self {
            max_deviation,
            quorum_size,
            trust_decay: 0.1,
            history: RwLock::new(HashMap::new()),
            trust_scores: RwLock::new(HashMap::new()),
        }
    }

    /// Validate a claimed coordinate against neighbor reports
    pub fn validate_coordinate(
        &self,
        node_id: &NodeId,
        claimed: &PoincareDiskPoint,
        neighbor_reports: &[(NodeId, PoincareDiskPoint)],
    ) -> ValidationResult {
        if neighbor_reports.len() < self.quorum_size {
            return ValidationResult::InsufficientData {
                required: self.quorum_size,
                received: neighbor_reports.len(),
            };
        }

        // Calculate weighted consensus based on trust scores
        let trust_scores = self.trust_scores.read().unwrap();
        let mut weighted_x = 0.0;
        let mut weighted_y = 0.0;
        let mut total_weight = 0.0;

        for (neighbor_id, reported) in neighbor_reports {
            let trust = trust_scores.get(neighbor_id).copied().unwrap_or(1.0);
            weighted_x += reported.x * trust;
            weighted_y += reported.y * trust;
            total_weight += trust;
        }
        drop(trust_scores);

        if total_weight < 1e-10 {
            return ValidationResult::InsufficientTrust;
        }

        let consensus_x = weighted_x / total_weight;
        let consensus_y = weighted_y / total_weight;
        let consensus = PoincareDiskPoint::new(consensus_x, consensus_y)
            .unwrap_or(PoincareDiskPoint::new(0.0, 0.0).unwrap());

        // Check deviation from consensus
        let deviation = claimed.hyperbolic_distance(&consensus);

        if deviation > self.max_deviation {
            // Penalize the node
            self.decrease_trust(node_id);
            return ValidationResult::Invalid {
                claimed: *claimed,
                consensus,
                deviation,
                threshold: self.max_deviation,
            };
        }

        // Check for suspicious movement (too fast)
        if let Some(movement_issue) = self.check_movement(node_id, claimed) {
            return movement_issue;
        }

        // Valid coordinate
        self.record_coordinate(node_id, claimed);
        ValidationResult::Valid { deviation }
    }

    /// Check for suspicious rapid movement
    fn check_movement(&self, node_id: &NodeId, new_coord: &PoincareDiskPoint) -> Option<ValidationResult> {
        let history = self.history.read().unwrap();
        if let Some(coords) = history.get(node_id) {
            if let Some(last) = coords.last() {
                let movement = last.hyperbolic_distance(new_coord);
                // Movement greater than 2.0 per update is suspicious
                if movement > 2.0 {
                    return Some(ValidationResult::SuspiciousMovement {
                        from: *last,
                        to: *new_coord,
                        distance: movement,
                    });
                }
            }
        }
        None
    }

    /// Record a validated coordinate
    fn record_coordinate(&self, node_id: &NodeId, coord: &PoincareDiskPoint) {
        let mut history = self.history.write().unwrap();
        let coords = history.entry(node_id.clone()).or_insert_with(Vec::new);
        coords.push(*coord);
        // Keep only last 10 coordinates
        if coords.len() > 10 {
            coords.remove(0);
        }
    }

    /// Decrease trust score for misbehaving node
    fn decrease_trust(&self, node_id: &NodeId) {
        let mut trust_scores = self.trust_scores.write().unwrap();
        let score = trust_scores.entry(node_id.clone()).or_insert(1.0);
        *score *= 1.0 - self.trust_decay;
        if *score < 0.1 {
            *score = 0.1; // Minimum trust
        }
    }

    /// Get trust score for a node
    pub fn get_trust(&self, node_id: &NodeId) -> f64 {
        self.trust_scores.read().unwrap()
            .get(node_id)
            .copied()
            .unwrap_or(1.0)
    }

    /// Reset trust score for a node
    pub fn reset_trust(&self, node_id: &NodeId) {
        let mut trust_scores = self.trust_scores.write().unwrap();
        trust_scores.insert(node_id.clone(), 1.0);
    }
}

impl Default for CoordinateValidator {
    fn default() -> Self {
        Self::new(0.5, 3)
    }
}

/// Result of coordinate validation
#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid { deviation: f64 },
    Invalid {
        claimed: PoincareDiskPoint,
        consensus: PoincareDiskPoint,
        deviation: f64,
        threshold: f64,
    },
    SuspiciousMovement {
        from: PoincareDiskPoint,
        to: PoincareDiskPoint,
        distance: f64,
    },
    InsufficientData { required: usize, received: usize },
    InsufficientTrust,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid { .. })
    }
}

/// Packet delivery verifier for detecting intentional drops
pub struct DeliveryVerifier {
    /// Expected packets per source-destination pair
    pending: RwLock<HashMap<(NodeId, NodeId), Vec<PendingPacket>>>,
    /// Drop counts per node
    drop_counts: RwLock<HashMap<NodeId, usize>>,
    /// Timeout before packet is considered dropped (nanoseconds)
    timeout_ns: u64,
}

#[derive(Debug, Clone)]
struct PendingPacket {
    packet_id: String,
    path: Vec<NodeId>,
    sent_time_ns: u64,
}

impl DeliveryVerifier {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            pending: RwLock::new(HashMap::new()),
            drop_counts: RwLock::new(HashMap::new()),
            timeout_ns: timeout_ms * 1_000_000,
        }
    }

    /// Record a packet being sent
    pub fn record_send(&self, source: NodeId, dest: NodeId, packet_id: &str, path: Vec<NodeId>) {
        let mut pending = self.pending.write().unwrap();
        let packets = pending.entry((source, dest)).or_insert_with(Vec::new);
        packets.push(PendingPacket {
            packet_id: packet_id.to_string(),
            path,
            sent_time_ns: Self::now_ns(),
        });
    }

    /// Record a packet being delivered
    pub fn record_delivery(&self, source: &NodeId, dest: &NodeId, packet_id: &str) {
        let mut pending = self.pending.write().unwrap();
        if let Some(packets) = pending.get_mut(&(source.clone(), dest.clone())) {
            packets.retain(|p| p.packet_id != packet_id);
        }
    }

    /// Check for timed-out packets and identify suspected droppers
    pub fn check_drops(&self) -> Vec<(NodeId, usize)> {
        let now = Self::now_ns();
        let mut pending = self.pending.write().unwrap();
        let mut drop_counts = self.drop_counts.write().unwrap();

        let mut dropped = Vec::new();

        for (_, packets) in pending.iter_mut() {
            let timed_out: Vec<_> = packets.iter()
                .filter(|p| now - p.sent_time_ns > self.timeout_ns)
                .cloned()
                .collect();

            for packet in &timed_out {
                // Blame the last node in the known path
                if let Some(last_node) = packet.path.last() {
                    *drop_counts.entry(last_node.clone()).or_insert(0) += 1;
                }
            }

            dropped.extend(timed_out.iter().map(|p| p.packet_id.clone()));
            packets.retain(|p| now - p.sent_time_ns <= self.timeout_ns);
        }

        drop_counts.iter()
            .map(|(id, count)| (id.clone(), *count))
            .collect()
    }

    fn now_ns() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
}

impl Default for DeliveryVerifier {
    fn default() -> Self {
        Self::new(5000) // 5 second timeout
    }
}

/// Byzantine-tolerant routing decision
pub struct ByzantineRouter {
    pub validator: CoordinateValidator,
    pub verifier: DeliveryVerifier,
    /// Blacklisted nodes
    blacklist: RwLock<Vec<NodeId>>,
    /// Maximum drop count before blacklisting
    max_drops: usize,
}

impl ByzantineRouter {
    pub fn new() -> Self {
        Self {
            validator: CoordinateValidator::default(),
            verifier: DeliveryVerifier::default(),
            blacklist: RwLock::new(Vec::new()),
            max_drops: 5,
        }
    }

    /// Check if a node is blacklisted
    pub fn is_blacklisted(&self, node_id: &NodeId) -> bool {
        self.blacklist.read().unwrap().contains(node_id)
    }

    /// Add node to blacklist
    pub fn blacklist_node(&self, node_id: NodeId) {
        let mut blacklist = self.blacklist.write().unwrap();
        if !blacklist.contains(&node_id) {
            blacklist.push(node_id);
        }
    }

    /// Update blacklist based on drop counts
    pub fn update_blacklist(&self) {
        let drops = self.verifier.check_drops();
        for (node_id, count) in drops {
            if count > self.max_drops {
                self.blacklist_node(node_id);
            }
        }
    }

    /// Get all blacklisted nodes
    pub fn get_blacklist(&self) -> Vec<NodeId> {
        self.blacklist.read().unwrap().clone()
    }
}

impl Default for ByzantineRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_validation_valid() {
        let validator = CoordinateValidator::new(1.0, 2);
        
        let claimed = PoincareDiskPoint::new(0.3, 0.4).unwrap();
        let neighbor_reports = vec![
            (NodeId::new("n1"), PoincareDiskPoint::new(0.31, 0.39).unwrap()),
            (NodeId::new("n2"), PoincareDiskPoint::new(0.29, 0.41).unwrap()),
        ];
        
        let result = validator.validate_coordinate(
            &NodeId::new("test"),
            &claimed,
            &neighbor_reports,
        );
        
        assert!(result.is_valid());
    }

    #[test]
    fn test_coordinate_validation_invalid() {
        let validator = CoordinateValidator::new(0.1, 2);
        
        let claimed = PoincareDiskPoint::new(0.9, 0.0).unwrap(); // Far from consensus
        let neighbor_reports = vec![
            (NodeId::new("n1"), PoincareDiskPoint::new(0.1, 0.1).unwrap()),
            (NodeId::new("n2"), PoincareDiskPoint::new(0.15, 0.05).unwrap()),
        ];
        
        let result = validator.validate_coordinate(
            &NodeId::new("test"),
            &claimed,
            &neighbor_reports,
        );
        
        assert!(!result.is_valid());
    }

    #[test]
    fn test_blacklist() {
        let router = ByzantineRouter::new();
        
        assert!(!router.is_blacklisted(&NodeId::new("test")));
        router.blacklist_node(NodeId::new("test"));
        assert!(router.is_blacklisted(&NodeId::new("test")));
    }
}
