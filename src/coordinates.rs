//! Dual Coordinate System: Anchor Coordinates and Routing Coordinates
//!
//! This module implements the core solution to the Coordinate-ID Paradox:
//! - Anchor Coordinate: Topology-independent, derived deterministically from ID
//! - Routing Coordinate: Topology-dependent, updated dynamically via Ricci flow

use crate::PoincareDiskPoint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Node identifier (could be IP address, UUID, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Anchor Coordinate: Static, topology-independent coordinate derived from node ID.
///
/// Properties:
/// - Deterministic: same ID always produces same coordinate
/// - Computable by any node: no network state required
/// - Located near the boundary of the Poincaré disk (r ≈ 0.95)
#[derive(Debug, Clone, Copy)]
pub struct AnchorCoordinate {
    pub point: PoincareDiskPoint,
}

impl AnchorCoordinate {
    /// Default radius for anchor coordinates (near boundary but inside disk)
    const DEFAULT_RADIUS: f64 = 0.95;

    /// Compute anchor coordinate from node ID using SHA-256 hash.
    /// The hash determines the angle θ on the disk boundary.
    pub fn from_id(id: &NodeId) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(id.0.as_bytes());
        let hash = hasher.finalize();

        // Use first 8 bytes of hash to determine angle
        let hash_value = u64::from_be_bytes([
            hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
        ]);

        let theta = (hash_value as f64 / u64::MAX as f64) * 2.0 * std::f64::consts::PI;

        let point = PoincareDiskPoint::from_polar(Self::DEFAULT_RADIUS, theta)
            .expect("DEFAULT_RADIUS should always be valid");

        Self { point }
    }

    /// Compute anchor coordinate with custom radius
    pub fn from_id_with_radius(id: &NodeId, radius: f64) -> Option<Self> {
        if radius <= 0.0 || radius >= 1.0 {
            return None;
        }

        let mut hasher = Sha256::new();
        hasher.update(id.0.as_bytes());
        let hash = hasher.finalize();

        let hash_value = u64::from_be_bytes([
            hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
        ]);

        let theta = (hash_value as f64 / u64::MAX as f64) * 2.0 * std::f64::consts::PI;
        let point = PoincareDiskPoint::from_polar(radius, theta)?;

        Some(Self { point })
    }
}

/// Routing Coordinate: Dynamic, topology-dependent coordinate.
///
/// Properties:
/// - Updated via Ricci flow embedding
/// - Known only to the node itself and its neighbors
/// - Used for actual packet forwarding (Greedy Forwarding)
#[derive(Debug, Clone, Copy)]
pub struct RoutingCoordinate {
    pub point: PoincareDiskPoint,
    /// Timestamp of last update
    pub updated_at: u64,
}

impl RoutingCoordinate {
    pub fn new(point: PoincareDiskPoint, timestamp: u64) -> Self {
        Self {
            point,
            updated_at: timestamp,
        }
    }

    /// Create initial routing coordinate (same as anchor coordinate)
    pub fn from_anchor(anchor: &AnchorCoordinate, timestamp: u64) -> Self {
        Self {
            point: anchor.point,
            updated_at: timestamp,
        }
    }
}

/// Home Node: The node whose routing coordinate is closest to a given anchor coordinate.
///
/// h(t) = argmin_{v ∈ V} d_H(z_v, a(ID_t))
#[derive(Debug, Clone)]
pub struct HomeNodeRegistry {
    /// Map from NodeId to its anchor coordinate
    anchor_coords: HashMap<NodeId, AnchorCoordinate>,
    /// Map from NodeId to its current routing coordinate
    routing_coords: HashMap<NodeId, RoutingCoordinate>,
    /// Soft-state registration: Map from NodeId to registered routing coordinate (for rendezvous)
    registrations: HashMap<NodeId, (RoutingCoordinate, u64)>, // (coord, expiry_time)
}

impl HomeNodeRegistry {
    pub fn new() -> Self {
        Self {
            anchor_coords: HashMap::new(),
            routing_coords: HashMap::new(),
            registrations: HashMap::new(),
        }
    }

    /// Register a node with its anchor coordinate
    pub fn register_node(&mut self, id: NodeId, routing_coord: RoutingCoordinate) {
        let anchor = AnchorCoordinate::from_id(&id);
        self.anchor_coords.insert(id.clone(), anchor);
        self.routing_coords.insert(id, routing_coord);
    }

    /// Get anchor coordinate for a node ID (any node can compute this)
    pub fn get_anchor(&self, id: &NodeId) -> AnchorCoordinate {
        AnchorCoordinate::from_id(id)
    }

    /// Get routing coordinate for a node (only if registered)
    pub fn get_routing(&self, id: &NodeId) -> Option<&RoutingCoordinate> {
        self.routing_coords.get(id)
    }

    /// Update routing coordinate for a node
    pub fn update_routing(&mut self, id: &NodeId, coord: RoutingCoordinate) {
        self.routing_coords.insert(id.clone(), coord);
    }

    /// Find the home node for a given target ID.
    /// Returns the NodeId of the node whose routing coordinate is closest to the target's anchor.
    pub fn find_home_node(&self, target_id: &NodeId) -> Option<NodeId> {
        let target_anchor = AnchorCoordinate::from_id(target_id);

        self.routing_coords
            .iter()
            .min_by(|(_, coord_a), (_, coord_b)| {
                let dist_a = coord_a.point.hyperbolic_distance(&target_anchor.point);
                let dist_b = coord_b.point.hyperbolic_distance(&target_anchor.point);
                dist_a
                    .partial_cmp(&dist_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| id.clone())
    }

    /// Register destination info at home node (soft-state with TTL)
    pub fn register_at_home(
        &mut self,
        target_id: &NodeId,
        routing_coord: RoutingCoordinate,
        ttl: u64,
        current_time: u64,
    ) {
        let expiry = current_time + ttl;
        self.registrations
            .insert(target_id.clone(), (routing_coord, expiry));
    }

    /// Lookup registered routing coordinate (used by home node)
    pub fn lookup_registration(
        &self,
        target_id: &NodeId,
        current_time: u64,
    ) -> Option<&RoutingCoordinate> {
        self.registrations.get(target_id).and_then(|(coord, expiry)| {
            if current_time < *expiry {
                Some(coord)
            } else {
                None
            }
        })
    }

    /// Clean up expired registrations
    pub fn cleanup_expired(&mut self, current_time: u64) {
        self.registrations
            .retain(|_, (_, expiry)| current_time < *expiry);
    }

    /// Get all registered nodes
    pub fn get_all_nodes(&self) -> Vec<&NodeId> {
        self.routing_coords.keys().collect()
    }

    /// Get number of registered nodes
    pub fn node_count(&self) -> usize {
        self.routing_coords.len()
    }
}

impl Default for HomeNodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anchor_coordinate_deterministic() {
        let id = NodeId::new("node_123");
        let anchor1 = AnchorCoordinate::from_id(&id);
        let anchor2 = AnchorCoordinate::from_id(&id);

        assert!((anchor1.point.x - anchor2.point.x).abs() < 1e-10);
        assert!((anchor1.point.y - anchor2.point.y).abs() < 1e-10);
    }

    #[test]
    fn test_anchor_coordinate_different_ids() {
        let id1 = NodeId::new("node_a");
        let id2 = NodeId::new("node_b");
        let anchor1 = AnchorCoordinate::from_id(&id1);
        let anchor2 = AnchorCoordinate::from_id(&id2);

        // Different IDs should produce different coordinates (with high probability)
        assert!(
            (anchor1.point.x - anchor2.point.x).abs() > 1e-6
                || (anchor1.point.y - anchor2.point.y).abs() > 1e-6
        );
    }

    #[test]
    fn test_anchor_near_boundary() {
        let id = NodeId::new("test_node");
        let anchor = AnchorCoordinate::from_id(&id);
        let r = anchor.point.euclidean_norm();

        assert!((r - 0.95).abs() < 1e-10);
    }

    #[test]
    fn test_home_node_selection() {
        let mut registry = HomeNodeRegistry::new();

        // Register three nodes at different positions
        let node1 = NodeId::new("node_1");
        let node2 = NodeId::new("node_2");
        let node3 = NodeId::new("node_3");

        let coord1 = RoutingCoordinate::new(PoincareDiskPoint::new(0.1, 0.0).unwrap(), 0);
        let coord2 = RoutingCoordinate::new(PoincareDiskPoint::new(0.0, 0.1).unwrap(), 0);
        let coord3 = RoutingCoordinate::new(PoincareDiskPoint::new(-0.1, -0.1).unwrap(), 0);

        registry.register_node(node1.clone(), coord1);
        registry.register_node(node2.clone(), coord2);
        registry.register_node(node3.clone(), coord3);

        // Find home node for some target
        let target = NodeId::new("target_x");
        let home = registry.find_home_node(&target);

        assert!(home.is_some());
    }

    #[test]
    fn test_registration_with_ttl() {
        let mut registry = HomeNodeRegistry::new();
        let target = NodeId::new("target");
        let coord = RoutingCoordinate::new(PoincareDiskPoint::new(0.5, 0.5).unwrap(), 0);

        registry.register_at_home(&target, coord, 100, 0);

        // Before expiry
        assert!(registry.lookup_registration(&target, 50).is_some());

        // After expiry
        assert!(registry.lookup_registration(&target, 150).is_none());
    }
}
