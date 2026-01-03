//! Rendezvous Mechanism for DRFE-R
//!
//! Implements the distributed protocol for resolving node coordinates
//! when only the destination ID is known.

use crate::coordinates::{AnchorCoordinate, HomeNodeRegistry, NodeId, RoutingCoordinate};
use crate::routing::{GPRouter, RoutingNode};
use crate::PoincareDiskPoint;

/// Protocol phase for rendezvous routing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendezvousPhase {
    /// Phase 1: Route toward anchor coordinate
    TowardAnchor,
    /// Phase 2: Route toward actual destination
    TowardDestination,
}

/// Rendezvous packet for the two-phase routing protocol
#[derive(Debug, Clone)]
pub struct RendezvousPacket {
    /// Source node ID
    pub source: NodeId,
    /// Destination node ID
    pub destination: NodeId,
    /// Current routing phase
    pub phase: RendezvousPhase,
    /// Current target coordinate
    pub target_coord: PoincareDiskPoint,
    /// TTL
    pub ttl: u32,
    /// Payload (opaque bytes)
    pub payload: Vec<u8>,
}

impl RendezvousPacket {
    pub fn new(source: NodeId, destination: NodeId, ttl: u32, payload: Vec<u8>) -> Self {
        // Initially target the anchor coordinate
        let anchor = AnchorCoordinate::from_id(&destination);

        Self {
            source,
            destination,
            phase: RendezvousPhase::TowardAnchor,
            target_coord: anchor.point,
            ttl,
            payload,
        }
    }

    /// Transition to Phase 2 with actual destination coordinate
    pub fn switch_to_destination(&mut self, dest_coord: PoincareDiskPoint) {
        self.phase = RendezvousPhase::TowardDestination;
        self.target_coord = dest_coord;
    }
}

/// Registration message sent by nodes to their home nodes
#[derive(Debug, Clone)]
pub struct RegistrationMessage {
    /// Node being registered
    pub node_id: NodeId,
    /// Current routing coordinate
    pub routing_coord: RoutingCoordinate,
    /// TTL for soft-state
    pub ttl: u64,
    /// Timestamp
    pub timestamp: u64,
}

impl RegistrationMessage {
    pub fn new(node_id: NodeId, coord: RoutingCoordinate, ttl: u64, timestamp: u64) -> Self {
        Self {
            node_id,
            routing_coord: coord,
            ttl,
            timestamp,
        }
    }
}

/// Rendezvous controller managing the two-phase routing protocol
pub struct RendezvousController {
    /// Home node registry
    registry: HomeNodeRegistry,
    /// GP Router for actual packet forwarding
    router: GPRouter,
    /// Registration TTL (in time units)
    registration_ttl: u64,
    /// Registration interval
    #[allow(dead_code)]
    registration_interval: u64,
}

impl RendezvousController {
    pub fn new(registration_ttl: u64, registration_interval: u64) -> Self {
        Self {
            registry: HomeNodeRegistry::new(),
            router: GPRouter::new(),
            registration_ttl,
            registration_interval,
        }
    }

    /// Add a node to the network
    pub fn add_node(&mut self, id: NodeId, coord: RoutingCoordinate) {
        let routing_node = RoutingNode::new(id.clone(), coord);
        self.router.add_node(routing_node);
        self.registry.register_node(id, coord);
    }

    /// Add an edge between two nodes
    pub fn add_edge(&mut self, u: &NodeId, v: &NodeId) {
        self.router.add_edge(u, v);
    }

    /// Process a registration message (at home node)
    pub fn process_registration(
        &mut self,
        msg: RegistrationMessage,
        current_time: u64,
    ) {
        self.registry.register_at_home(
            &msg.node_id,
            msg.routing_coord,
            msg.ttl,
            current_time,
        );
    }

    /// Simulate registration of a node to its home node
    pub fn register_node_to_home(
        &mut self,
        node_id: &NodeId,
        current_time: u64,
    ) -> Option<NodeId> {
        let coord = self.registry.get_routing(node_id)?.clone();
        
        // Find home node
        let home = self.registry.find_home_node(node_id)?;

        // Register at home node
        self.registry.register_at_home(
            node_id,
            coord,
            self.registration_ttl,
            current_time,
        );

        Some(home)
    }

    /// Route a rendezvous packet
    pub fn route_packet(
        &self,
        packet: &mut RendezvousPacket,
        current_node: &NodeId,
        current_time: u64,
    ) -> RendezvousRoutingResult {
        // Check TTL
        if packet.ttl == 0 {
            return RendezvousRoutingResult::Failed {
                reason: "TTL expired".to_string(),
            };
        }

        // Check if we're at destination
        if current_node == &packet.destination {
            return RendezvousRoutingResult::Delivered;
        }

        match packet.phase {
            RendezvousPhase::TowardAnchor => {
                // Check if current node is the home node
                if let Some(home) = self.registry.find_home_node(&packet.destination) {
                    if &home == current_node {
                        // We're at home node - lookup destination coordinate
                        if let Some(dest_coord) =
                            self.registry.lookup_registration(&packet.destination, current_time)
                        {
                            // Switch to Phase 2
                            packet.switch_to_destination(dest_coord.point);
                            return self.route_toward_destination(packet, current_node);
                        } else {
                            // Destination not registered - this shouldn't happen in normal operation
                            return RendezvousRoutingResult::Failed {
                                reason: format!(
                                    "Destination {} not registered at home node",
                                    packet.destination
                                ),
                            };
                        }
                    }
                }

                // Continue routing toward anchor
                self.route_toward_anchor(packet, current_node)
            }
            RendezvousPhase::TowardDestination => {
                self.route_toward_destination(packet, current_node)
            }
        }
    }

    fn route_toward_anchor(
        &self,
        packet: &RendezvousPacket,
        current_node: &NodeId,
    ) -> RendezvousRoutingResult {
        let current = match self.router.get_node(current_node) {
            Some(n) => n,
            None => {
                return RendezvousRoutingResult::Failed {
                    reason: format!("Node {} not found", current_node),
                }
            }
        };

        // Find neighbor closest to anchor coordinate
        let mut best_neighbor: Option<NodeId> = None;
        let mut best_distance = current.coord.point.hyperbolic_distance(&packet.target_coord);

        for neighbor_id in &current.neighbors {
            if let Some(neighbor) = self.router.get_node(neighbor_id) {
                let dist = neighbor.coord.point.hyperbolic_distance(&packet.target_coord);
                if dist < best_distance {
                    best_distance = dist;
                    best_neighbor = Some(neighbor_id.clone());
                }
            }
        }

        match best_neighbor {
            Some(next_hop) => RendezvousRoutingResult::Forward {
                next_hop,
                phase: RendezvousPhase::TowardAnchor,
            },
            None => {
                // Local minimum - we might be at the home node
                // Check if current node is closest to anchor
                RendezvousRoutingResult::AtHomeNode {
                    home_node: current_node.clone(),
                }
            }
        }
    }

    fn route_toward_destination(
        &self,
        packet: &RendezvousPacket,
        current_node: &NodeId,
    ) -> RendezvousRoutingResult {
        let current = match self.router.get_node(current_node) {
            Some(n) => n,
            None => {
                return RendezvousRoutingResult::Failed {
                    reason: format!("Node {} not found", current_node),
                }
            }
        };

        // Check if we've arrived (within some tolerance)
        let dist_to_target = current.coord.point.hyperbolic_distance(&packet.target_coord);
        if dist_to_target < 0.01 {
            // Very close to target - check if this is the destination
            if current_node == &packet.destination {
                return RendezvousRoutingResult::Delivered;
            }
        }

        // Greedy forwarding toward destination
        let mut best_neighbor: Option<NodeId> = None;
        let mut best_distance = dist_to_target;

        for neighbor_id in &current.neighbors {
            // Direct check for destination
            if neighbor_id == &packet.destination {
                return RendezvousRoutingResult::Forward {
                    next_hop: neighbor_id.clone(),
                    phase: RendezvousPhase::TowardDestination,
                };
            }

            if let Some(neighbor) = self.router.get_node(neighbor_id) {
                let dist = neighbor.coord.point.hyperbolic_distance(&packet.target_coord);
                if dist < best_distance {
                    best_distance = dist;
                    best_neighbor = Some(neighbor_id.clone());
                }
            }
        }

        match best_neighbor {
            Some(next_hop) => RendezvousRoutingResult::Forward {
                next_hop,
                phase: RendezvousPhase::TowardDestination,
            },
            None => RendezvousRoutingResult::Failed {
                reason: "Local minimum in Phase 2".to_string(),
            },
        }
    }

    /// Get the underlying router
    pub fn router(&self) -> &GPRouter {
        &self.router
    }

    /// Get the registry
    pub fn registry(&self) -> &HomeNodeRegistry {
        &self.registry
    }

    /// Get mutable registry
    pub fn registry_mut(&mut self) -> &mut HomeNodeRegistry {
        &mut self.registry
    }
}

/// Result of rendezvous routing decision
#[derive(Debug, Clone)]
pub enum RendezvousRoutingResult {
    /// Forward to next hop
    Forward {
        next_hop: NodeId,
        phase: RendezvousPhase,
    },
    /// Arrived at home node (Phase 1 complete)
    AtHomeNode { home_node: NodeId },
    /// Packet delivered to destination
    Delivered,
    /// Routing failed
    Failed { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registration_message() {
        let coord = RoutingCoordinate::new(PoincareDiskPoint::new(0.5, 0.5).unwrap(), 0);
        let msg = RegistrationMessage::new(NodeId::new("test"), coord, 100, 0);

        assert_eq!(msg.node_id.0, "test");
        assert_eq!(msg.ttl, 100);
    }

    #[test]
    fn test_rendezvous_packet_creation() {
        let packet = RendezvousPacket::new(
            NodeId::new("src"),
            NodeId::new("dst"),
            100,
            vec![1, 2, 3],
        );

        assert_eq!(packet.phase, RendezvousPhase::TowardAnchor);
        assert_eq!(packet.ttl, 100);
    }

    #[test]
    fn test_anchor_coordinate_computation() {
        let id = NodeId::new("test_node");
        let anchor = AnchorCoordinate::from_id(&id);

        // Anchor should be near boundary (r ≈ 0.95)
        let r = anchor.point.euclidean_norm();
        assert!((r - 0.95).abs() < 0.01);
    }
}
