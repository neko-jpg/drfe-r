//! Greedy Embedding for Hyperbolic Routing
//!
//! Implements PIE (Polar Increasing-angle Embedding) algorithm based on
//! Kleinberg's theorem that guarantees 100% greedy routing success on trees.
//!
//! Key insight: By embedding a spanning tree into hyperbolic space where
//! parent-child relationships are preserved as distance relationships,
//! greedy forwarding is guaranteed to succeed.

use crate::coordinates::{NodeId, RoutingCoordinate};
use crate::PoincareDiskPoint;
use std::collections::{HashMap, HashSet, VecDeque};

/// Result of the greedy embedding process
#[derive(Debug, Clone)]
pub struct EmbeddingResult {
    /// Coordinates for each node
    pub coordinates: HashMap<NodeId, PoincareDiskPoint>,
    /// The spanning tree used (parent -> children)
    pub tree_children: HashMap<NodeId, Vec<NodeId>>,
    /// Root of the spanning tree
    pub root: NodeId,
    /// Maximum depth of the tree
    pub max_depth: usize,
}

/// Configuration for the PIE embedding
#[derive(Debug, Clone)]
pub struct PIEConfig {
    /// Base radius for root node (should be small, e.g., 0.0)
    pub root_radius: f64,
    /// Maximum radius (should be < 1.0 for Poincaré disk)
    pub max_radius: f64,
    /// Exponential base for radius growth (typically 0.5 for 1 - 2^(-depth))
    pub radius_base: f64,
}

impl Default for PIEConfig {
    fn default() -> Self {
        Self {
            root_radius: 0.05,  // Small but non-zero to avoid singularity at origin
            max_radius: 0.99,
            radius_base: 0.25,  // Very steep growth
        }
    }
}

/// Greedy Embedding using PIE (Polar Increasing-angle Embedding)
///
/// This algorithm guarantees that for any pair of nodes in the tree,
/// greedy forwarding (always moving to the neighbor closest to destination)
/// will successfully reach the destination.
pub struct GreedyEmbedding {
    config: PIEConfig,
}

impl GreedyEmbedding {
    pub fn new() -> Self {
        Self {
            config: PIEConfig::default(),
        }
    }

    pub fn with_config(config: PIEConfig) -> Self {
        Self { config }
    }

    /// Build a BFS spanning tree from the graph
    /// Returns (parent map, children map, root, depths)
    fn build_spanning_tree(
        &self,
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
        root: &NodeId,
    ) -> (
        HashMap<NodeId, Option<NodeId>>,
        HashMap<NodeId, Vec<NodeId>>,
        HashMap<NodeId, usize>,
    ) {
        let mut parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
        let mut children: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut depths: HashMap<NodeId, usize> = HashMap::new();
        let mut visited: HashSet<NodeId> = HashSet::new();

        // Initialize all nodes with empty children lists
        for node_id in adjacency.keys() {
            children.insert(node_id.clone(), Vec::new());
        }

        // BFS from root
        let mut queue = VecDeque::new();
        queue.push_back(root.clone());
        parent.insert(root.clone(), None);
        depths.insert(root.clone(), 0);
        visited.insert(root.clone());

        while let Some(current) = queue.pop_front() {
            let current_depth = depths[&current];

            if let Some(neighbors) = adjacency.get(&current) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        visited.insert(neighbor.clone());
                        parent.insert(neighbor.clone(), Some(current.clone()));
                        depths.insert(neighbor.clone(), current_depth + 1);

                        // Add as child
                        if let Some(child_list) = children.get_mut(&current) {
                            child_list.push(neighbor.clone());
                        }

                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }

        (parent, children, depths)
    }

    /// Compute radius for a node based on its depth
    /// Uses exponential spacing starting from root_radius
    fn compute_radius(&self, depth: usize, _max_depth: usize) -> f64 {
        if depth == 0 {
            return self.config.root_radius;
        }

        // Exponential growth starting from root_radius
        // r(d) = root_radius + (max_radius - root_radius) * (1 - base^d)
        let base = self.config.radius_base;
        let range = self.config.max_radius - self.config.root_radius;
        let r = self.config.root_radius + range * (1.0 - base.powi(depth as i32));

        // Clamp to valid range
        r.max(self.config.root_radius).min(self.config.max_radius - 0.001)
    }

    /// Perform PIE embedding
    ///
    /// Algorithm:
    /// 1. Build BFS spanning tree from highest-degree node (hub)
    /// 2. Assign root to origin (or small radius)
    /// 3. For each node, assign angle range based on parent's range
    /// 4. Children divide their parent's angle range equally
    /// 5. Radius increases exponentially with depth
    pub fn embed(
        &self,
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
    ) -> Result<EmbeddingResult, String> {
        if adjacency.is_empty() {
            return Err("Empty graph".to_string());
        }

        // Find the highest-degree node as root (hub)
        let root = adjacency
            .iter()
            .max_by_key(|(_, neighbors)| neighbors.len())
            .map(|(id, _)| id.clone())
            .ok_or("No nodes in graph")?;

        // Build spanning tree
        let (_parent, children, depths) = self.build_spanning_tree(adjacency, &root);

        let max_depth = *depths.values().max().unwrap_or(&1);

        // Assign coordinates using DFS traversal
        let mut coordinates: HashMap<NodeId, PoincareDiskPoint> = HashMap::new();

        // Angle ranges: each node has (start_angle, end_angle)
        let mut angle_ranges: HashMap<NodeId, (f64, f64)> = HashMap::new();

        // Root gets the full circle
        angle_ranges.insert(root.clone(), (0.0, 2.0 * std::f64::consts::PI));

        // Process nodes in BFS order (to ensure parents are processed before children)
        let mut queue = VecDeque::new();
        queue.push_back(root.clone());

        while let Some(current) = queue.pop_front() {
            let depth = depths.get(&current).copied().unwrap_or(0);
            let (start_angle, end_angle) = angle_ranges.get(&current).copied().unwrap_or((0.0, 2.0 * std::f64::consts::PI));

            // Compute this node's coordinate
            let radius = self.compute_radius(depth, max_depth);
            let angle = (start_angle + end_angle) / 2.0; // Center of angle range

            let point = if radius < 0.001 {
                // Near origin
                PoincareDiskPoint::origin()
            } else {
                PoincareDiskPoint::from_polar(radius, angle)
                    .unwrap_or_else(|| PoincareDiskPoint::origin())
            };

            coordinates.insert(current.clone(), point);

            // Assign angle ranges to children
            if let Some(child_list) = children.get(&current) {
                let num_children = child_list.len();
                if num_children > 0 {
                    let child_angle_span = (end_angle - start_angle) / num_children as f64;

                    for (i, child) in child_list.iter().enumerate() {
                        let child_start = start_angle + i as f64 * child_angle_span;
                        let child_end = child_start + child_angle_span;
                        angle_ranges.insert(child.clone(), (child_start, child_end));
                        queue.push_back(child.clone());
                    }
                }
            }
        }

        Ok(EmbeddingResult {
            coordinates,
            tree_children: children,
            root,
            max_depth,
        })
    }

    /// Embed graph and return coordinates as RoutingCoordinates
    pub fn embed_as_routing_coords(
        &self,
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
    ) -> Result<HashMap<NodeId, RoutingCoordinate>, String> {
        let result = self.embed(adjacency)?;

        let coords = result
            .coordinates
            .into_iter()
            .map(|(id, point)| (id, RoutingCoordinate::new(point, 0)))
            .collect();

        Ok(coords)
    }

    /// Refine embedding using stress minimization
    /// This adjusts coordinates so that adjacent nodes are closer in hyperbolic space
    pub fn refine_embedding(
        coordinates: &mut HashMap<NodeId, PoincareDiskPoint>,
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
        iterations: usize,
        step_size: f64,
    ) {
        let node_ids: Vec<NodeId> = coordinates.keys().cloned().collect();
        
        // Target distance for adjacent nodes (should be small in hyperbolic space)
        let target_neighbor_dist = 0.5;
        
        for _iter in 0..iterations {
            // Compute gradients for each node
            let mut gradients: HashMap<NodeId, (f64, f64)> = HashMap::new();
            for id in &node_ids {
                gradients.insert(id.clone(), (0.0, 0.0));
            }

            // For each edge, compute contribution to gradient
            for (node_id, neighbors) in adjacency {
                let node_coord = match coordinates.get(node_id) {
                    Some(c) => *c,
                    None => continue,
                };

                for neighbor_id in neighbors {
                    let neighbor_coord = match coordinates.get(neighbor_id) {
                        Some(c) => *c,
                        None => continue,
                    };

                    let current_dist = node_coord.hyperbolic_distance(&neighbor_coord);
                    if current_dist < 1e-10 {
                        continue;
                    }

                    // Stress: want neighbor distance to be close to target
                    let stress = current_dist - target_neighbor_dist;
                    
                    // Euclidean direction (approximation for small movements in Poincaré disk)
                    let dx = neighbor_coord.x - node_coord.x;
                    let dy = neighbor_coord.y - node_coord.y;
                    let euclidean_dist = (dx * dx + dy * dy).sqrt().max(1e-10);
                    
                    // Gradient: move toward/away from neighbor based on stress
                    // Positive stress = too far apart = move closer
                    let grad_scale = stress * step_size / euclidean_dist;
                    
                    if let Some(g) = gradients.get_mut(node_id) {
                        g.0 += dx * grad_scale;
                        g.1 += dy * grad_scale;
                    }
                }
            }

            // Apply gradients with Poincaré disk constraint
            for (id, (gx, gy)) in &gradients {
                if let Some(coord) = coordinates.get_mut(id) {
                    // Riemannian scaling factor for Poincaré disk
                    let r_sq = coord.x * coord.x + coord.y * coord.y;
                    let scale = (1.0 - r_sq).powi(2) / 4.0;
                    
                    coord.x += gx * scale;
                    coord.y += gy * scale;

                    // Project back into disk if needed
                    let new_r_sq = coord.x * coord.x + coord.y * coord.y;
                    if new_r_sq >= 0.98 * 0.98 {
                        let norm = new_r_sq.sqrt();
                        coord.x = coord.x / norm * 0.97;
                        coord.y = coord.y / norm * 0.97;
                    }
                }
            }
        }
    }
}

impl Default for GreedyEmbedding {
    fn default() -> Self {
        Self::new()
    }
}

/// Verify that the embedding satisfies greedy routing property
/// Returns (success_count, total_pairs, failure_details)
pub fn verify_greedy_property(
    coordinates: &HashMap<NodeId, PoincareDiskPoint>,
    adjacency: &HashMap<NodeId, Vec<NodeId>>,
) -> (usize, usize, Vec<(NodeId, NodeId)>) {
    let node_ids: Vec<&NodeId> = coordinates.keys().collect();
    let mut success_count = 0;
    let mut total_pairs = 0;
    let mut failures = Vec::new();

    for &source in &node_ids {
        for &dest in &node_ids {
            if source == dest {
                continue;
            }

            total_pairs += 1;

            // Simulate greedy routing
            let mut current = source.clone();
            let mut visited = HashSet::new();
            let dest_coord = coordinates.get(dest).unwrap();
            let mut success = false;

            for _ in 0..1000 {
                // Max iterations to prevent infinite loops
                if &current == dest {
                    success = true;
                    break;
                }

                if visited.contains(&current) {
                    break; // Loop detected
                }
                visited.insert(current.clone());

                let current_coord = match coordinates.get(&current) {
                    Some(c) => c,
                    None => break,
                };

                let current_dist = current_coord.hyperbolic_distance(dest_coord);

                // Find neighbor closest to destination
                let neighbors = match adjacency.get(&current) {
                    Some(n) => n,
                    None => break,
                };

                let mut best_neighbor: Option<NodeId> = None;
                let mut best_dist = current_dist;

                for neighbor in neighbors {
                    if let Some(neighbor_coord) = coordinates.get(neighbor) {
                        let dist = neighbor_coord.hyperbolic_distance(dest_coord);
                        if dist < best_dist {
                            best_dist = dist;
                            best_neighbor = Some(neighbor.clone());
                        }
                    }
                }

                match best_neighbor {
                    Some(next) => current = next,
                    None => break, // Local minimum
                }
            }

            if success {
                success_count += 1;
            } else {
                failures.push((source.clone(), dest.clone()));
            }
        }
    }

    (success_count, total_pairs, failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_adjacency() -> HashMap<NodeId, Vec<NodeId>> {
        // Simple tree: 0 -> 1, 2; 1 -> 3, 4; 2 -> 5
        let mut adj = HashMap::new();

        adj.insert(NodeId::new("0"), vec![NodeId::new("1"), NodeId::new("2")]);
        adj.insert(NodeId::new("1"), vec![NodeId::new("0"), NodeId::new("3"), NodeId::new("4")]);
        adj.insert(NodeId::new("2"), vec![NodeId::new("0"), NodeId::new("5")]);
        adj.insert(NodeId::new("3"), vec![NodeId::new("1")]);
        adj.insert(NodeId::new("4"), vec![NodeId::new("1")]);
        adj.insert(NodeId::new("5"), vec![NodeId::new("2")]);

        adj
    }

    #[test]
    fn test_embedding_creates_valid_coordinates() {
        let adj = create_test_adjacency();
        let embedder = GreedyEmbedding::new();
        let result = embedder.embed(&adj).unwrap();

        // All nodes should have coordinates
        assert_eq!(result.coordinates.len(), 6);

        // All coordinates should be inside the Poincaré disk
        for (_, point) in &result.coordinates {
            assert!(point.euclidean_norm() < 1.0);
        }
    }

    #[test]
    fn test_radius_increases_with_depth() {
        let adj = create_test_adjacency();
        let embedder = GreedyEmbedding::new();
        let result = embedder.embed(&adj).unwrap();

        // Root should be at or near origin
        let root_coord = result.coordinates.get(&result.root).unwrap();
        assert!(root_coord.euclidean_norm() < 0.1);

        // Leaf nodes should be at larger radii
        let leaf_coord = result.coordinates.get(&NodeId::new("3")).unwrap();
        assert!(leaf_coord.euclidean_norm() > root_coord.euclidean_norm());
    }

    #[test]
    fn test_root_node_radius_within_expected_range() {
        let adj = create_test_adjacency();
        let embedder = GreedyEmbedding::new();
        let result = embedder.embed(&adj).unwrap();

        // Root node should have radius equal to PIEConfig.root_radius (0.05)
        let root_coord = result.coordinates.get(&result.root).unwrap();
        let root_radius = root_coord.euclidean_norm();
        
        // Should be very close to the configured root_radius (0.05)
        assert!(root_radius < 0.1, "Root radius {} should be less than 0.1", root_radius);
        assert!(root_radius >= 0.0, "Root radius {} should be non-negative", root_radius);
        
        // More specifically, should be close to 0.05 (the default root_radius)
        assert!((root_radius - 0.05).abs() < 0.01, 
                "Root radius {} should be close to 0.05", root_radius);
    }

    #[test]
    fn test_greedy_routing_on_tree() {
        let adj = create_test_adjacency();
        let embedder = GreedyEmbedding::new();
        let result = embedder.embed(&adj).unwrap();

        let (success, total, failures) = verify_greedy_property(&result.coordinates, &adj);

        // On a tree, PIE embedding should give 100% success
        assert_eq!(
            success, total,
            "Greedy routing failed for {} pairs: {:?}",
            failures.len(),
            failures.iter().take(5).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_embedding_with_cycle() {
        // Graph with a cycle: 0-1-2-0
        let mut adj = HashMap::new();
        adj.insert(NodeId::new("0"), vec![NodeId::new("1"), NodeId::new("2")]);
        adj.insert(NodeId::new("1"), vec![NodeId::new("0"), NodeId::new("2")]);
        adj.insert(NodeId::new("2"), vec![NodeId::new("1"), NodeId::new("0")]);

        let embedder = GreedyEmbedding::new();
        let result = embedder.embed(&adj).unwrap();

        // Should still create valid coordinates
        assert_eq!(result.coordinates.len(), 3);
    }
}
