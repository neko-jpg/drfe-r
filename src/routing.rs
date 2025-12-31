//! Gravity-Pressure (GP) Routing Algorithm
//!
//! Implements the routing algorithm that provides theoretical delivery guarantee
//! even when greedy forwarding fails due to local minima.
//!
//! Reference: Cvetkovski–Crovella (2009)

use crate::coordinates::{NodeId, RoutingCoordinate};
use crate::PoincareDiskPoint;
use std::collections::{HashMap, HashSet};

/// Routing mode for the GP algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingMode {
    /// Greedy mode: forward to neighbor closest to destination
    Gravity,
    /// Recovery mode: use pressure to escape local minimum
    Pressure,
}

/// Result of a routing decision
#[derive(Debug, Clone)]
pub enum RoutingDecision {
    /// Forward packet to specified next hop
    Forward { next_hop: NodeId, mode: RoutingMode },
    /// Packet has reached destination
    Delivered,
    /// Routing failed (should not happen in connected graph with sufficient TTL)
    Failed { reason: String },
}

/// Packet header for GP routing
#[derive(Debug, Clone)]
pub struct PacketHeader {
    /// Source node ID
    pub source: NodeId,
    /// Destination node ID
    pub destination: NodeId,
    /// Target coordinate (either anchor or routing coordinate)
    pub target_coord: PoincareDiskPoint,
    /// Current routing mode
    pub mode: RoutingMode,
    /// Time-to-live
    pub ttl: u32,
    /// Visited nodes (for pressure calculation)
    pub visited: HashSet<NodeId>,
    /// Pressure values accumulated during Pressure mode
    pub pressure_values: HashMap<NodeId, f64>,
}

impl PacketHeader {
    pub fn new(
        source: NodeId,
        destination: NodeId,
        target_coord: PoincareDiskPoint,
        ttl: u32,
    ) -> Self {
        Self {
            source,
            destination,
            target_coord,
            mode: RoutingMode::Gravity,
            ttl,
            visited: HashSet::new(),
            pressure_values: HashMap::new(),
        }
    }

    /// Check if a node has been visited
    pub fn has_visited(&self, node: &NodeId) -> bool {
        self.visited.contains(node)
    }

    /// Record a visit to a node
    pub fn record_visit(&mut self, node: NodeId) {
        self.visited.insert(node);
    }

    /// Get visit count (for pressure calculation)
    pub fn visit_count(&self) -> usize {
        self.visited.len()
    }
}

/// A node in the routing network
#[derive(Debug, Clone)]
pub struct RoutingNode {
    pub id: NodeId,
    pub coord: RoutingCoordinate,
    pub neighbors: Vec<NodeId>,
}

impl RoutingNode {
    pub fn new(id: NodeId, coord: RoutingCoordinate) -> Self {
        Self {
            id,
            coord,
            neighbors: Vec::new(),
        }
    }

    pub fn add_neighbor(&mut self, neighbor: NodeId) {
        if !self.neighbors.contains(&neighbor) {
            self.neighbors.push(neighbor);
        }
    }

    pub fn degree(&self) -> usize {
        self.neighbors.len()
    }
}

/// GP Router implementing Gravity-Pressure routing algorithm
pub struct GPRouter {
    /// All nodes in the network
    nodes: HashMap<NodeId, RoutingNode>,
    /// Pressure decay factor
    pressure_decay: f64,
    /// Pressure increment per visit
    pressure_increment: f64,
}

impl GPRouter {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            pressure_decay: 0.95,        // Slower decay to maintain pressure longer
            pressure_increment: 5.0,     // Stronger pressure to overcome distance
        }
    }

    /// Add a node to the network
    pub fn add_node(&mut self, node: RoutingNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Add a bidirectional edge between two nodes
    pub fn add_edge(&mut self, node1: &NodeId, node2: &NodeId) {
        if let Some(n1) = self.nodes.get_mut(node1) {
            n1.add_neighbor(node2.clone());
        }
        if let Some(n2) = self.nodes.get_mut(node2) {
            n2.add_neighbor(node1.clone());
        }
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &NodeId) -> Option<&RoutingNode> {
        self.nodes.get(id)
    }

    /// Get mutable reference to a node
    pub fn get_node_mut(&mut self, id: &NodeId) -> Option<&mut RoutingNode> {
        self.nodes.get_mut(id)
    }

    /// Make routing decision for a packet at the current node
    pub fn route(&self, current_node: &NodeId, packet: &mut PacketHeader) -> RoutingDecision {
        // Check TTL
        if packet.ttl == 0 {
            return RoutingDecision::Failed {
                reason: "TTL expired".to_string(),
            };
        }

        // Check if we reached destination
        if current_node == &packet.destination {
            return RoutingDecision::Delivered;
        }

        let current = match self.nodes.get(current_node) {
            Some(n) => n,
            None => {
                return RoutingDecision::Failed {
                    reason: format!("Node {} not found", current_node),
                }
            }
        };

        // Record visit
        packet.record_visit(current_node.clone());

        // Try Gravity mode first
        if let Some(decision) = self.try_gravity_routing(current, packet) {
            return decision;
        }

        // Fall back to Pressure mode
        self.pressure_routing(current, packet)
    }

    /// Gravity Mode: Greedy forwarding to neighbor closest to target
    fn try_gravity_routing(
        &self,
        current: &RoutingNode,
        packet: &PacketHeader,
    ) -> Option<RoutingDecision> {
        let current_distance = current.coord.point.hyperbolic_distance(&packet.target_coord);

        let mut best_neighbor: Option<&NodeId> = None;
        let mut best_distance = current_distance;

        for neighbor_id in &current.neighbors {
            if let Some(neighbor) = self.nodes.get(neighbor_id) {
                let distance = neighbor.coord.point.hyperbolic_distance(&packet.target_coord);
                if distance < best_distance {
                    best_distance = distance;
                    best_neighbor = Some(neighbor_id);
                }
            }
        }

        best_neighbor.map(|next_hop| RoutingDecision::Forward {
            next_hop: next_hop.clone(),
            mode: RoutingMode::Gravity,
        })
    }

    /// Pressure Mode: Use accumulated pressure to escape local minimum
    fn pressure_routing(&self, current: &RoutingNode, packet: &mut PacketHeader) -> RoutingDecision {
        // Calculate pressure-adjusted distances for each neighbor
        let mut best_neighbor: Option<NodeId> = None;
        let mut best_score = f64::INFINITY;

        for neighbor_id in &current.neighbors {
            if let Some(neighbor) = self.nodes.get(neighbor_id) {
                // Base distance (gravity component)
                let distance = neighbor.coord.point.hyperbolic_distance(&packet.target_coord);

                // Pressure component: penalize previously visited nodes
                let pressure = packet
                    .pressure_values
                    .get(neighbor_id)
                    .copied()
                    .unwrap_or(0.0);

                // Combined score: lower is better
                // Nodes with high pressure (many visits) get higher scores, making them less attractive
                let score = distance + pressure;

                if score < best_score {
                    best_score = score;
                    best_neighbor = Some(neighbor_id.clone());
                }
            }
        }

        match best_neighbor {
            Some(next_hop) => {
                // Update pressure for the current node
                let current_pressure = packet
                    .pressure_values
                    .get(&current.id)
                    .copied()
                    .unwrap_or(0.0);
                packet
                    .pressure_values
                    .insert(current.id.clone(), current_pressure + self.pressure_increment);

                // Decay all pressures
                for pressure in packet.pressure_values.values_mut() {
                    *pressure *= self.pressure_decay;
                }

                RoutingDecision::Forward {
                    next_hop,
                    mode: RoutingMode::Pressure,
                }
            }
            None => RoutingDecision::Failed {
                reason: "No neighbors available".to_string(),
            },
        }
    }

    /// Simulate packet delivery from source to destination
    pub fn simulate_delivery(
        &self,
        source: &NodeId,
        destination: &NodeId,
        target_coord: PoincareDiskPoint,
        max_ttl: u32,
    ) -> DeliveryResult {
        let mut packet = PacketHeader::new(
            source.clone(),
            destination.clone(),
            target_coord,
            max_ttl,
        );

        let mut current = source.clone();
        let mut path = vec![current.clone()];
        let mut hops = 0;
        let mut gravity_hops = 0;
        let mut pressure_hops = 0;

        loop {
            let decision = self.route(&current, &mut packet);

            match decision {
                RoutingDecision::Delivered => {
                    return DeliveryResult {
                        success: true,
                        hops,
                        gravity_hops,
                        pressure_hops,
                        path,
                        failure_reason: None,
                    };
                }
                RoutingDecision::Forward { next_hop, mode } => {
                    hops += 1;
                    packet.ttl -= 1;
                    match mode {
                        RoutingMode::Gravity => gravity_hops += 1,
                        RoutingMode::Pressure => pressure_hops += 1,
                    }
                    path.push(next_hop.clone());
                    current = next_hop;
                }
                RoutingDecision::Failed { reason } => {
                    return DeliveryResult {
                        success: false,
                        hops,
                        gravity_hops,
                        pressure_hops,
                        path,
                        failure_reason: Some(reason),
                    };
                }
            }
        }
    }

    /// Get total number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get total number of edges
    pub fn edge_count(&self) -> usize {
        self.nodes.values().map(|n| n.neighbors.len()).sum::<usize>() / 2
    }

    /// Get all node IDs
    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.keys().cloned().collect()
    }

    /// Get all edges as (NodeId, NodeId) pairs
    pub fn get_edges(&self) -> Vec<(NodeId, NodeId)> {
        let mut edges = Vec::new();
        for node in self.nodes.values() {
            for neighbor in &node.neighbors {
                if node.id.0 < neighbor.0 {
                    edges.push((node.id.clone(), neighbor.clone()));
                }
            }
        }
        edges
    }
}

impl Default for GPRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a packet delivery simulation
#[derive(Debug, Clone)]
pub struct DeliveryResult {
    pub success: bool,
    pub hops: u32,
    pub gravity_hops: u32,
    pub pressure_hops: u32,
    pub path: Vec<NodeId>,
    pub failure_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_network() -> GPRouter {
        let mut router = GPRouter::new();

        // Create a simple network
        //     1
        //    / \
        //   0---2---3
        //    \ /
        //     4

        let nodes = vec![
            ("0", 0.0, 0.0),
            ("1", 0.0, 0.3),
            ("2", 0.3, 0.0),
            ("3", 0.5, 0.0),
            ("4", 0.15, -0.2),
        ];

        for (id, x, y) in &nodes {
            let coord = RoutingCoordinate::new(PoincareDiskPoint::new(*x, *y).unwrap(), 0);
            router.add_node(RoutingNode::new(NodeId::new(*id), coord));
        }

        // Add edges
        router.add_edge(&NodeId::new("0"), &NodeId::new("1"));
        router.add_edge(&NodeId::new("0"), &NodeId::new("2"));
        router.add_edge(&NodeId::new("0"), &NodeId::new("4"));
        router.add_edge(&NodeId::new("1"), &NodeId::new("2"));
        router.add_edge(&NodeId::new("2"), &NodeId::new("3"));
        router.add_edge(&NodeId::new("2"), &NodeId::new("4"));

        router
    }

    #[test]
    fn test_gravity_routing_success() {
        let router = create_test_network();
        let source = NodeId::new("0");
        let dest = NodeId::new("3");

        let dest_coord = router.get_node(&dest).unwrap().coord.point;
        let result = router.simulate_delivery(&source, &dest, dest_coord, 10);

        assert!(result.success);
        assert!(result.hops <= 3);
        assert_eq!(result.path.first().unwrap(), &source);
        assert_eq!(result.path.last().unwrap(), &dest);
    }

    #[test]
    fn test_routing_self() {
        let router = create_test_network();
        let node = NodeId::new("0");
        let coord = router.get_node(&node).unwrap().coord.point;

        let result = router.simulate_delivery(&node, &node, coord, 10);

        assert!(result.success);
        assert_eq!(result.hops, 0);
    }

    #[test]
    fn test_ttl_expiry() {
        let router = create_test_network();
        let source = NodeId::new("0");
        let dest = NodeId::new("3");
        let dest_coord = router.get_node(&dest).unwrap().coord.point;

        // TTL = 1, not enough to reach destination
        let result = router.simulate_delivery(&source, &dest, dest_coord, 1);

        assert!(!result.success);
        assert!(result.failure_reason.is_some());
    }
}
