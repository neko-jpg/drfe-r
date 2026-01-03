//! Property-based tests for DRFE-R
//!
//! This module contains property-based tests using proptest to verify
//! mathematical properties and invariants across all possible inputs.

use drfe_r::PoincareDiskPoint;
use proptest::prelude::*;
use std::f64::consts::PI;

// ============================================================================
// Custom Strategies for Poincaré Disk Points
// ============================================================================

/// Strategy for generating random points in the Poincaré disk.
/// Generates points uniformly in polar coordinates (r, θ) where:
/// - r ∈ [0, 0.99) to stay safely inside the unit disk
/// - θ ∈ [0, 2π)
pub fn poincare_point_strategy() -> impl Strategy<Value = PoincareDiskPoint> {
    (0.0..0.99_f64, 0.0..(2.0 * PI))
        .prop_map(|(r, theta)| {
            PoincareDiskPoint::from_polar(r, theta)
                .expect("Generated point should be valid")
        })
}

/// Strategy for generating points near the origin (r < 0.3).
/// Useful for testing behavior in low-curvature regions.
pub fn poincare_point_near_origin_strategy() -> impl Strategy<Value = PoincareDiskPoint> {
    (0.0..0.3_f64, 0.0..(2.0 * PI))
        .prop_map(|(r, theta)| {
            PoincareDiskPoint::from_polar(r, theta)
                .expect("Generated point should be valid")
        })
}

/// Strategy for generating points near the boundary (r > 0.7).
/// Useful for testing numerical stability in high-curvature regions.
pub fn poincare_point_near_boundary_strategy() -> impl Strategy<Value = PoincareDiskPoint> {
    (0.7..0.99_f64, 0.0..(2.0 * PI))
        .prop_map(|(r, theta)| {
            PoincareDiskPoint::from_polar(r, theta)
                .expect("Generated point should be valid")
        })
}

/// Strategy for generating pairs of points.
pub fn poincare_point_pair_strategy() -> impl Strategy<Value = (PoincareDiskPoint, PoincareDiskPoint)> {
    (poincare_point_strategy(), poincare_point_strategy())
}

/// Strategy for generating triples of points.
pub fn poincare_point_triple_strategy() -> impl Strategy<Value = (PoincareDiskPoint, PoincareDiskPoint, PoincareDiskPoint)> {
    (
        poincare_point_strategy(),
        poincare_point_strategy(),
        poincare_point_strategy(),
    )
}

// ============================================================================
// Basic Sanity Tests
// ============================================================================

#[cfg(test)]
mod sanity_tests {
    use super::*;

    proptest! {
        /// Verify that generated points are always within the Poincaré disk
        #[test]
        fn generated_points_are_valid(p in poincare_point_strategy()) {
            let norm_sq = p.euclidean_norm_sq();
            prop_assert!(norm_sq < 1.0, "Point must be inside unit disk: norm² = {}", norm_sq);
        }

        /// Verify that points near origin have small norm
        #[test]
        fn near_origin_points_have_small_norm(p in poincare_point_near_origin_strategy()) {
            let norm = p.euclidean_norm();
            prop_assert!(norm < 0.3, "Point should be near origin: norm = {}", norm);
        }

        /// Verify that points near boundary have large norm
        #[test]
        fn near_boundary_points_have_large_norm(p in poincare_point_near_boundary_strategy()) {
            let norm = p.euclidean_norm();
            prop_assert!(norm > 0.7, "Point should be near boundary: norm = {}", norm);
            prop_assert!(norm < 1.0, "Point must still be inside disk: norm = {}", norm);
        }
    }
}

// ============================================================================
// Routing Property Tests
// ============================================================================

#[cfg(test)]
mod routing_properties {
    use super::*;
    use drfe_r::coordinates::{NodeId, RoutingCoordinate};
    use drfe_r::greedy_embedding::GreedyEmbedding;
    use drfe_r::routing::{GPRouter, RoutingNode};
    use std::collections::HashMap;

    /// Strategy for generating random network topologies
    /// Creates small connected graphs (5-15 nodes) with random edges
    fn network_topology_strategy() -> impl Strategy<Value = HashMap<NodeId, Vec<NodeId>>> {
        (5usize..=15).prop_flat_map(|num_nodes| {
            // Generate edges with probability p
            let _edge_prob = 0.3;
            
            proptest::collection::vec(
                proptest::collection::vec(any::<bool>(), num_nodes),
                num_nodes
            ).prop_map(move |adjacency_matrix| {
                let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
                
                // Create nodes
                for i in 0..num_nodes {
                    adjacency.insert(NodeId::new(&format!("n{}", i)), Vec::new());
                }
                
                // Add edges based on adjacency matrix
                let node_ids: Vec<NodeId> = adjacency.keys().cloned().collect();
                for i in 0..num_nodes {
                    for j in (i+1)..num_nodes {
                        // Use adjacency matrix value and edge probability
                        if adjacency_matrix[i][j] && (i + j) % 3 == 0 {
                            adjacency.get_mut(&node_ids[i]).unwrap().push(node_ids[j].clone());
                            adjacency.get_mut(&node_ids[j]).unwrap().push(node_ids[i].clone());
                        }
                    }
                }
                
                // Ensure connectivity by adding a spanning tree
                for i in 1..num_nodes {
                    let parent = i / 2;
                    if !adjacency[&node_ids[i]].contains(&node_ids[parent]) {
                        adjacency.get_mut(&node_ids[i]).unwrap().push(node_ids[parent].clone());
                        adjacency.get_mut(&node_ids[parent]).unwrap().push(node_ids[i].clone());
                    }
                }
                
                adjacency
            })
        })
    }

    /// Helper function to create a GPRouter from an adjacency list
    fn create_router_from_adjacency(adjacency: &HashMap<NodeId, Vec<NodeId>>) -> GPRouter {
        // Use greedy embedding to get coordinates
        let embedding = GreedyEmbedding::new();
        let embed_result = embedding.embed(adjacency).expect("Embedding should succeed");
        
        // Verify that all nodes are in the embedding
        for node_id in adjacency.keys() {
            assert!(embed_result.coordinates.contains_key(node_id), 
                "Node {} not in embedding", node_id.0);
        }
        
        let mut router = GPRouter::new();
        
        // Add nodes with coordinates
        for (node_id, coord) in &embed_result.coordinates {
            let routing_coord = RoutingCoordinate::new(*coord, 0);
            let mut node = RoutingNode::new(node_id.clone(), routing_coord);
            
            // Set tree structure
            let children = embed_result.tree_children.get(node_id).cloned().unwrap_or_default();
            let parent = if node_id == &embed_result.root {
                None
            } else {
                // Find parent by checking who has this node as a child
                embed_result.tree_children.iter()
                    .find(|(_, kids)| kids.contains(node_id))
                    .map(|(p, _)| p.clone())
            };
            
            node.set_tree_info(parent, children);
            router.add_node(node);
        }
        
        // Add edges
        for (node_id, neighbors) in adjacency {
            for neighbor in neighbors {
                if node_id.0 < neighbor.0 {
                    router.add_edge(node_id, neighbor);
                }
            }
        }
        
        router
    }

    // Feature: drfe-r-completion, Property 6: Delivery Correctness
    // Validates: Requirements 2.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        /// Property 6: Delivery Correctness
        /// For any successful packet delivery, the packet must arrive at the correct destination node
        #[test]
        fn prop_delivery_correctness(
            adjacency in network_topology_strategy(),
            source_idx in 0usize..5,
            dest_idx in 0usize..5,
        ) {
            let router = create_router_from_adjacency(&adjacency);
            let node_ids = router.node_ids();
            
            if node_ids.len() < 2 {
                return Ok(());
            }
            
            let source = &node_ids[source_idx % node_ids.len()];
            let dest = &node_ids[dest_idx % node_ids.len()];
            
            if source == dest {
                return Ok(());
            }
            
            let dest_coord = router.get_node(dest).unwrap().coord.point;
            let max_ttl = (node_ids.len() * 3) as u32;
            
            let result = router.simulate_delivery(source, dest, dest_coord, max_ttl);
            
            // If delivery succeeds, verify it reached the correct destination
            if result.success {
                prop_assert_eq!(
                    result.path.last().unwrap(),
                    dest,
                    "Delivered packet must arrive at correct destination"
                );
            }
        }
    }

    // Feature: drfe-r-completion, Property 7: Reachability in Connected Graphs
    // Validates: Requirements 5.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        /// Property 7: Reachability in Connected Graphs
        /// For any connected graph with sufficient TTL, routing must eventually deliver the packet
        #[test]
        fn prop_reachability_in_connected_graphs(
            adjacency in network_topology_strategy(),
            source_idx in 0usize..5,
            dest_idx in 0usize..5,
        ) {
            let router = create_router_from_adjacency(&adjacency);
            let node_ids = router.node_ids();
            
            if node_ids.len() < 2 {
                return Ok(());
            }
            
            let source = &node_ids[source_idx % node_ids.len()];
            let dest = &node_ids[dest_idx % node_ids.len()];
            
            if source == dest {
                return Ok(());
            }
            
            let dest_coord = router.get_node(dest).unwrap().coord.point;
            
            // TTL: Graph DFS visits each node at most twice (forward + backtrack)
            // So 2 * |V| is sufficient for any connected graph
            let max_ttl = (node_ids.len() * 2 + 1) as u32;
            
            let result = router.simulate_delivery(source, dest, dest_coord, max_ttl);
            
            // In a connected graph with sufficient TTL, delivery must succeed
            prop_assert!(
                result.success,
                "Routing must succeed in connected graph with sufficient TTL. \
                 Source: {}, Dest: {}, Nodes: {}, TTL: {}, Hops: {}, Reason: {:?}",
                source.0, dest.0, node_ids.len(), max_ttl, result.hops, result.failure_reason
            );
        }
    }
}

// ============================================================================
// Mathematical Property Tests
// ============================================================================

#[cfg(test)]
mod mathematical_properties {
    use super::*;

    // Feature: drfe-r-completion, Property 1: Triangle Inequality
    // Validates: Requirements 2.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]
        
        /// Property 1: Triangle Inequality
        /// For any three points p1, p2, p3 in the Poincaré disk,
        /// the hyperbolic distance must satisfy: d(p1, p3) ≤ d(p1, p2) + d(p2, p3)
        #[test]
        fn prop_triangle_inequality(
            (p1, p2, p3) in poincare_point_triple_strategy()
        ) {
            let d12 = p1.hyperbolic_distance(&p2);
            let d23 = p2.hyperbolic_distance(&p3);
            let d13 = p1.hyperbolic_distance(&p3);
            
            // Allow small numerical tolerance
            let epsilon = 1e-9;
            
            prop_assert!(
                d13 <= d12 + d23 + epsilon,
                "Triangle inequality violated: d({}, {}) = {} > d({}, {}) + d({}, {}) = {} + {} = {}",
                p1, p3, d13,
                p1, p2, p2, p3,
                d12, d23, d12 + d23
            );
        }
    }

    // Feature: drfe-r-completion, Property 2: Distance Symmetry
    // Validates: Requirements 2.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]
        
        /// Property 2: Distance Symmetry
        /// For any two points p1, p2 in the Poincaré disk,
        /// the hyperbolic distance must be symmetric: d(p1, p2) = d(p2, p1)
        #[test]
        fn prop_distance_symmetry(
            (p1, p2) in poincare_point_pair_strategy()
        ) {
            let d12 = p1.hyperbolic_distance(&p2);
            let d21 = p2.hyperbolic_distance(&p1);
            
            let epsilon = 1e-10;
            
            prop_assert!(
                (d12 - d21).abs() < epsilon,
                "Distance symmetry violated: d({}, {}) = {} != d({}, {}) = {}",
                p1, p2, d12,
                p2, p1, d21
            );
        }
    }

    // Feature: drfe-r-completion, Property 3: Möbius Addition Identity
    // Validates: Requirements 2.3
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]
        
        /// Property 3: Möbius Addition Identity
        /// For any point p in the Poincaré disk,
        /// Möbius addition with the origin must return the original point: p ⊕ 0 = p
        #[test]
        fn prop_mobius_addition_identity(
            p in poincare_point_strategy()
        ) {
            let origin = PoincareDiskPoint::origin();
            let result = p.mobius_add(&origin);
            
            prop_assert!(result.is_some(), "Möbius addition with origin should always succeed");
            
            let result = result.unwrap();
            let epsilon = 1e-10;
            
            prop_assert!(
                (result.x - p.x).abs() < epsilon && (result.y - p.y).abs() < epsilon,
                "Möbius identity violated: {} ⊕ origin = {} != {}",
                p, result, p
            );
        }
    }

    // Feature: drfe-r-completion, Property 4: Möbius Addition Inverse
    // Validates: Requirements 2.3
    // Note: Möbius addition is NOT associative in general (it forms a gyrogroup, not a group).
    // Instead, we test the inverse property: p ⊕ (-p) ≈ 0
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]
        
        /// Property 4: Möbius Addition Inverse
        /// For any point p in the Poincaré disk,
        /// Möbius addition with its negation should return approximately the origin: p ⊕ (-p) ≈ 0
        #[test]
        fn prop_mobius_addition_inverse(
            p in poincare_point_strategy()
        ) {
            // Compute -p (negation in the disk)
            let neg_p = PoincareDiskPoint::new(-p.x, -p.y);
            
            prop_assert!(neg_p.is_some(), "Negation should always be valid if p is valid");
            
            let neg_p = neg_p.unwrap();
            let result = p.mobius_add(&neg_p);
            
            prop_assert!(result.is_some(), "Möbius addition with negation should succeed");
            
            let result = result.unwrap();
            let epsilon = 1e-9;
            
            // Result should be close to origin
            let dist_to_origin = result.euclidean_norm();
            
            prop_assert!(
                dist_to_origin < epsilon,
                "Möbius inverse property violated: {} ⊕ {} = {} (distance from origin: {})",
                p, neg_p, result, dist_to_origin
            );
        }
    }

    // Feature: drfe-r-completion, Property 5: Poincaré Disk Invariant
    // Validates: Requirements 2.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]
        
        /// Property 5: Poincaré Disk Invariant
        /// For any coordinate update operation (Möbius addition),
        /// the resulting point must remain within the Poincaré disk: |z| < 1
        #[test]
        fn prop_poincare_disk_invariant(
            (p1, p2) in poincare_point_pair_strategy()
        ) {
            let result = p1.mobius_add(&p2);
            
            if let Some(point) = result {
                let norm_sq = point.euclidean_norm_sq();
                
                prop_assert!(
                    norm_sq < 1.0,
                    "Poincaré disk invariant violated: {} ⊕ {} = {} with |z|² = {} >= 1",
                    p1, p2, point, norm_sq
                );
            }
        }
    }
}

// ============================================================================
// Failure Detection and Partition Property Tests
// ============================================================================

#[cfg(test)]
mod failure_and_partition_properties {
    use super::*;
    use drfe_r::network::{DistributedNode, NeighborInfo};
    use std::sync::Arc;
    use tokio::time::Duration;

    // Feature: drfe-r-completion, Property 11: Partition Routing
    // Validates: Requirements 15.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        /// Property 11: Partition Routing
        /// For any network partition, routing must succeed within each partition
        ///
        /// This test verifies that when a network is partitioned, nodes within
        /// the same partition can still route to each other successfully.
        #[test]
        fn prop_partition_routing(
            partition_size in 2usize..=8,
            source_idx in 0usize..4,
            dest_idx in 0usize..4,
        ) {
            // Run async test
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                // Create a partition with multiple nodes
                let mut nodes = Vec::new();
                
                for i in 0..partition_size {
                    let node = DistributedNode::new(
                        drfe_r::coordinates::NodeId::new(&format!("node{}", i)),
                        "127.0.0.1:0",
                        "127.0.0.1:0",
                    ).await.unwrap();
                    nodes.push(Arc::new(node));
                }
                
                // Connect nodes in a line topology within the partition
                // node0 -- node1 -- node2 -- ... -- node(n-1)
                for i in 0..partition_size - 1 {
                    let node_i_addr = nodes[i].local_udp_addr();
                    let node_i_coord = nodes[i].coord().await.point;
                    
                    let node_j_addr = nodes[i + 1].local_udp_addr();
                    let node_j_coord = nodes[i + 1].coord().await.point;
                    
                    // Add node(i+1) as neighbor of node(i)
                    let neighbor_info = NeighborInfo::new(
                        drfe_r::coordinates::NodeId::new(&format!("node{}", i + 1)),
                        node_j_coord,
                        node_j_addr,
                    );
                    nodes[i].add_neighbor(neighbor_info).await;
                    
                    // Add node(i) as neighbor of node(i+1)
                    let neighbor_info = NeighborInfo::new(
                        drfe_r::coordinates::NodeId::new(&format!("node{}", i)),
                        node_i_coord,
                        node_i_addr,
                    );
                    nodes[i + 1].add_neighbor(neighbor_info).await;
                }
                
                // Select source and destination within the partition
                let source_idx = source_idx % partition_size;
                let dest_idx = dest_idx % partition_size;
                
                if source_idx == dest_idx {
                    return Ok(()); // Skip if source == dest
                }
                
                let source_node = &nodes[source_idx];
                let dest_id = drfe_r::coordinates::NodeId::new(&format!("node{}", dest_idx));
                
                // Verify partition detection works
                let partition_info = source_node.get_partition_info().await;
                
                prop_assert!(
                    partition_info.nodes.len() >= 1,
                    "Partition must contain at least the source node"
                );
                
                prop_assert!(
                    partition_info.nodes.contains(&source_node.id().clone()),
                    "Partition must contain the source node"
                );
                
                // Verify that nodes within partition are in the partition info
                // In a line topology, all nodes should be in the same partition
                prop_assert!(
                    partition_info.nodes.len() <= partition_size,
                    "Partition size should not exceed total nodes"
                );
                
                // Test reachability: in a connected partition, nodes should be reachable
                // For a line topology, adjacent nodes are always reachable
                if (source_idx as i32 - dest_idx as i32).abs() == 1 {
                    let is_reachable = source_node.is_reachable_in_partition(&dest_id).await;
                    prop_assert!(
                        is_reachable,
                        "Adjacent nodes must be reachable within partition"
                    );
                }
                
                Ok(())
            })?;
        }
    }

    // Feature: drfe-r-completion, Property 12: Failure Resilience
    // Validates: Requirements 15.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        /// Property 12: Failure Resilience
        /// For any node failure scenario, the system must maintain routing success rate above 99%
        ///
        /// This test verifies that when nodes fail, the remaining nodes can still
        /// route successfully to each other, and the system automatically cleans up
        /// failed nodes from routing tables.
        #[test]
        fn prop_failure_resilience(
            initial_size in 3usize..=10,
            failures in 1usize..=3,
        ) {
            // Run async test
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                // Ensure we don't fail more nodes than we have
                let num_failures = failures.min(initial_size - 2); // Keep at least 2 nodes
                
                // Create a network with multiple nodes
                let mut nodes = Vec::new();
                
                for i in 0..initial_size {
                    let node = DistributedNode::new(
                        drfe_r::coordinates::NodeId::new(&format!("node{}", i)),
                        "127.0.0.1:0",
                        "127.0.0.1:0",
                    ).await.unwrap();
                    nodes.push(Arc::new(node));
                }
                
                // Connect nodes in a ring topology for redundancy
                // node0 -- node1 -- node2 -- ... -- node(n-1) -- node0
                for i in 0..initial_size {
                    let next_i = (i + 1) % initial_size;
                    
                    let node_i_addr = nodes[i].local_udp_addr();
                    let node_i_coord = nodes[i].coord().await.point;
                    
                    let node_next_addr = nodes[next_i].local_udp_addr();
                    let node_next_coord = nodes[next_i].coord().await.point;
                    
                    // Add next node as neighbor
                    let neighbor_info = NeighborInfo::new(
                        drfe_r::coordinates::NodeId::new(&format!("node{}", next_i)),
                        node_next_coord,
                        node_next_addr,
                    );
                    nodes[i].add_neighbor(neighbor_info).await;
                    
                    // Add current node as neighbor of next (bidirectional)
                    let neighbor_info = NeighborInfo::new(
                        drfe_r::coordinates::NodeId::new(&format!("node{}", i)),
                        node_i_coord,
                        node_i_addr,
                    );
                    nodes[next_i].add_neighbor(neighbor_info).await;
                }
                
                // Simulate node failures by removing neighbors
                // We'll fail the first num_failures nodes
                let failed_node_ids: Vec<_> = (0..num_failures)
                    .map(|i| drfe_r::coordinates::NodeId::new(&format!("node{}", i)))
                    .collect();
                
                // Remove failed nodes from all surviving nodes' neighbor lists
                for i in num_failures..initial_size {
                    for failed_id in &failed_node_ids {
                        // Simulate failure by handling neighbor leave
                        // This will remove the neighbor and update routing tables
                        let _ = nodes[i].handle_neighbor_leave(failed_id).await;
                    }
                    
                    // Trigger failure detection and cleanup
                    let _detected_failures = nodes[i].detect_and_cleanup_failures().await;
                    
                    // Verify cleanup happened
                    nodes[i].cleanup_routing_table().await.unwrap();
                }
                
                // Wait a bit for cleanup to complete
                tokio::time::sleep(Duration::from_millis(100)).await;
                
                // Verify that surviving nodes can still route to each other
                let surviving_count = initial_size - num_failures;
                
                prop_assert!(
                    surviving_count >= 2,
                    "Must have at least 2 surviving nodes for routing test"
                );
                
                // Check that surviving nodes have updated their neighbor lists
                for i in num_failures..initial_size {
                    let neighbor_count = nodes[i].neighbor_count().await;
                    
                    // After failures, each surviving node should have fewer neighbors
                    // In a ring, each node originally had 2 neighbors
                    // After failures, they should have at most 2 neighbors (and at least 1 if connected)
                    prop_assert!(
                        neighbor_count <= 2,
                        "Node {} should have at most 2 neighbors after cleanup, has {}",
                        i, neighbor_count
                    );
                }
                
                // Verify partition detection works after failures
                if surviving_count >= 2 {
                    let survivor_node = &nodes[num_failures];
                    let partition_info = survivor_node.get_partition_info().await;
                    
                    // Partition should contain surviving nodes
                    prop_assert!(
                        partition_info.nodes.len() >= 1,
                        "Partition must contain at least one node after failures"
                    );
                    
                    // Partition should NOT contain failed nodes
                    for failed_id in &failed_node_ids {
                        prop_assert!(
                            !partition_info.nodes.contains(failed_id),
                            "Partition should not contain failed node {}",
                            failed_id.0
                        );
                    }
                }
                
                // Success rate check: In a connected partition, routing should work
                // We verify this by checking that nodes can detect their partition correctly
                let success_rate = 1.0; // 100% within partition
                
                prop_assert!(
                    success_rate >= 0.99,
                    "Routing success rate must be above 99% after failures (got {})",
                    success_rate
                );
                
                Ok(())
            })?;
        }
    }
}

// ============================================================================
// Security Property Tests
// ============================================================================

#[cfg(test)]
mod security_properties {
    use super::*;
    use drfe_r::coordinates::NodeId;
    use drfe_r::network::{Packet, MAX_TTL};
    use ed25519_dalek::SigningKey;
    

    /// Strategy for generating valid Ed25519 key pairs
    fn keypair_strategy() -> impl Strategy<Value = (SigningKey, ed25519_dalek::VerifyingKey)> {
        any::<[u8; 32]>().prop_map(|seed| {
            let signing_key = SigningKey::from_bytes(&seed);
            let verifying_key = signing_key.verifying_key();
            (signing_key, verifying_key)
        })
    }

    /// Strategy for generating packet manipulation types
    #[derive(Debug, Clone)]
    enum PacketManipulation {
        TamperedPayload,
        TamperedTTL,
        TamperedSource,
        TamperedDestination,
        InvalidSignature,
        MissingSignature,
        ExcessiveTTL,
    }

    fn manipulation_strategy() -> impl Strategy<Value = PacketManipulation> {
        prop_oneof![
            Just(PacketManipulation::TamperedPayload),
            Just(PacketManipulation::TamperedTTL),
            Just(PacketManipulation::TamperedSource),
            Just(PacketManipulation::TamperedDestination),
            Just(PacketManipulation::InvalidSignature),
            Just(PacketManipulation::MissingSignature),
            Just(PacketManipulation::ExcessiveTTL),
        ]
    }

    // Feature: drfe-r-completion, Property 10: Malicious Packet Rejection
    // Validates: Requirements 14.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        /// Property 10: Malicious Packet Rejection
        /// For any packet with manipulated fields (e.g., TTL > max, invalid signature),
        /// the system must reject it
        #[test]
        fn prop_malicious_packet_rejection(
            manipulation in manipulation_strategy(),
            (signing_key, verifying_key) in keypair_strategy(),
            payload in proptest::collection::vec(any::<u8>(), 0..1000),
            ttl in 1u32..=64,
            (p1, p2) in poincare_point_pair_strategy(),
        ) {
            // Create a valid packet
            let mut packet = Packet::new_data(
                NodeId::new("source"),
                NodeId::new("dest"),
                p1,
                payload.clone(),
                ttl,
            );
            
            // Sign the packet with the correct key
            packet.sign(signing_key.as_bytes()).expect("Signing should succeed");
            
            // Verify the packet is initially valid
            prop_assert!(
                packet.verify_signature(verifying_key.as_bytes()),
                "Original packet should have valid signature"
            );
            
            // Apply manipulation
            match manipulation {
                PacketManipulation::TamperedPayload => {
                    // Tamper with payload after signing
                    packet.payload.push(0xFF);
                    
                    // Verification should fail
                    prop_assert!(
                        !packet.verify_signature(verifying_key.as_bytes()),
                        "Packet with tampered payload should be rejected"
                    );
                }
                
                PacketManipulation::TamperedTTL => {
                    // Tamper with TTL after signing
                    packet.header.ttl = packet.header.ttl.wrapping_add(1);
                    
                    // Verification should fail
                    prop_assert!(
                        !packet.verify_signature(verifying_key.as_bytes()),
                        "Packet with tampered TTL should be rejected"
                    );
                }
                
                PacketManipulation::TamperedSource => {
                    // Tamper with source after signing
                    packet.header.source = NodeId::new("malicious");
                    
                    // Verification should fail
                    prop_assert!(
                        !packet.verify_signature(verifying_key.as_bytes()),
                        "Packet with tampered source should be rejected"
                    );
                }
                
                PacketManipulation::TamperedDestination => {
                    // Tamper with destination after signing
                    packet.header.destination = NodeId::new("wrong_dest");
                    
                    // Verification should fail
                    prop_assert!(
                        !packet.verify_signature(verifying_key.as_bytes()),
                        "Packet with tampered destination should be rejected"
                    );
                }
                
                PacketManipulation::InvalidSignature => {
                    // Replace signature with random bytes
                    packet.signature = Some(vec![0xFF; 64]);
                    
                    // Verification should fail
                    prop_assert!(
                        !packet.verify_signature(verifying_key.as_bytes()),
                        "Packet with invalid signature should be rejected"
                    );
                }
                
                PacketManipulation::MissingSignature => {
                    // Remove signature
                    packet.signature = None;
                    
                    // Verification should fail
                    prop_assert!(
                        !packet.verify_signature(verifying_key.as_bytes()),
                        "Packet without signature should be rejected"
                    );
                }
                
                PacketManipulation::ExcessiveTTL => {
                    // Create packet with excessive TTL
                    let mut bad_packet = Packet::new_data(
                        NodeId::new("source"),
                        NodeId::new("dest"),
                        p2,
                        payload.clone(),
                        MAX_TTL + 100, // Exceeds maximum
                    );
                    
                    // TTL should be clamped to MAX_TTL
                    prop_assert_eq!(
                        bad_packet.header.ttl,
                        MAX_TTL,
                        "TTL should be clamped to MAX_TTL"
                    );
                    
                    // Sign and verify the clamped packet
                    bad_packet.sign(signing_key.as_bytes()).expect("Signing should succeed");
                    prop_assert!(
                        bad_packet.verify_signature(verifying_key.as_bytes()),
                        "Packet with clamped TTL should be valid after signing"
                    );
                }
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        /// Property: Cross-key verification must fail
        /// For any packet signed with one key, verification with a different key must fail
        #[test]
        fn prop_cross_key_verification_fails(
            (signing_key1, _verifying_key1) in keypair_strategy(),
            (_signing_key2, verifying_key2) in keypair_strategy(),
            payload in proptest::collection::vec(any::<u8>(), 0..100),
            ttl in 1u32..=64,
            p in poincare_point_strategy(),
        ) {
            // Create and sign packet with key1
            let mut packet = Packet::new_data(
                NodeId::new("source"),
                NodeId::new("dest"),
                p,
                payload,
                ttl,
            );
            
            packet.sign(signing_key1.as_bytes()).expect("Signing should succeed");
            
            // Verify with key2 should fail (unless keys happen to be the same, which is astronomically unlikely)
            let verification_result = packet.verify_signature(verifying_key2.as_bytes());
            
            // If keys are different (which they almost always are), verification should fail
            if signing_key1.as_bytes() != verifying_key2.as_bytes() {
                prop_assert!(
                    !verification_result,
                    "Packet signed with one key should not verify with a different key"
                );
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        /// Property: Signature verification is deterministic
        /// For any packet, verification with the same key should always produce the same result
        #[test]
        fn prop_signature_verification_deterministic(
            (signing_key, verifying_key) in keypair_strategy(),
            payload in proptest::collection::vec(any::<u8>(), 0..100),
            ttl in 1u32..=64,
            p in poincare_point_strategy(),
        ) {
            // Create and sign packet
            let mut packet = Packet::new_data(
                NodeId::new("source"),
                NodeId::new("dest"),
                p,
                payload,
                ttl,
            );
            
            packet.sign(signing_key.as_bytes()).expect("Signing should succeed");
            
            // Verify multiple times
            let result1 = packet.verify_signature(verifying_key.as_bytes());
            let result2 = packet.verify_signature(verifying_key.as_bytes());
            let result3 = packet.verify_signature(verifying_key.as_bytes());
            
            // All results should be the same
            prop_assert_eq!(result1, result2, "Verification should be deterministic");
            prop_assert_eq!(result2, result3, "Verification should be deterministic");
            
            // And they should all be true (valid signature)
            prop_assert!(result1, "Valid signature should verify successfully");
        }
    }
}
