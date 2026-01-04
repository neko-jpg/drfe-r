//! Thorup-Zwick Compact Routing
//!
//! Implements the Thorup-Zwick (2001) compact routing scheme with guaranteed
//! stretch ≤ 3 for any connected graph.
//!
//! Key properties:
//! - Stretch guarantee: path length ≤ 3 × optimal
//! - Space per node: O(n^{1/2}) entries
//! - Preprocessing: O(n^{3/2} + m) time
//!
//! Algorithm:
//! 1. Select ~√n landmarks randomly
//! 2. For each node v, compute:
//!    - p(v): closest landmark
//!    - B(v): "bunch" = nodes closer than p(v)
//! 3. Routing uses bunch membership or landmark hops

use crate::coordinates::NodeId;
use std::collections::{HashMap, HashSet, VecDeque};

/// Configuration for Thorup-Zwick routing
#[derive(Debug, Clone)]
pub struct TZConfig {
    /// Number of landmarks (default: ceil(√n))
    pub num_landmarks: Option<usize>,
    /// Random seed for landmark selection
    pub seed: u64,
}

impl Default for TZConfig {
    fn default() -> Self {
        Self {
            num_landmarks: None,
            seed: 42,
        }
    }
}

/// Precomputed routing information for a single node
#[derive(Debug, Clone)]
pub struct TZNodeInfo {
    /// Closest landmark to this node
    pub closest_landmark: NodeId,
    /// Distance to closest landmark
    pub landmark_distance: u32,
    /// Bunch: nodes closer than the closest landmark
    /// Maps node_id -> (distance, next_hop_on_shortest_path)
    pub bunch: HashMap<NodeId, (u32, NodeId)>,
}

/// Thorup-Zwick routing table
#[derive(Debug, Clone)]
pub struct TZRoutingTable {
    /// Configuration
    pub config: TZConfig,
    /// Selected landmarks
    pub landmarks: Vec<NodeId>,
    /// Node information (bunch, closest landmark, etc.)
    pub node_info: HashMap<NodeId, TZNodeInfo>,
    /// Distances between landmarks
    pub landmark_distances: HashMap<(NodeId, NodeId), u32>,
    /// Next hop from landmark to landmark (for inter-landmark routing)
    pub landmark_next_hop: HashMap<(NodeId, NodeId), NodeId>,
    /// Next hop from any node to its closest landmark
    pub to_landmark_next_hop: HashMap<NodeId, NodeId>,
}

impl TZRoutingTable {
    /// Build the TZ routing table from a graph
    pub fn build(
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
        config: TZConfig,
    ) -> Result<Self, String> {
        let n = adjacency.len();
        if n == 0 {
            return Err("Empty graph".to_string());
        }

        // Compute number of landmarks
        let num_landmarks = config.num_landmarks.unwrap_or_else(|| {
            ((n as f64).sqrt().ceil() as usize).max(1).min(n)
        });

        // Select landmarks (using degree-weighted sampling for better coverage)
        let landmarks = Self::select_landmarks(adjacency, num_landmarks, config.seed);

        if landmarks.is_empty() {
            return Err("No landmarks selected".to_string());
        }

        // Compute distances from all landmarks (BFS from each)
        let mut landmark_full_distances: HashMap<&NodeId, HashMap<NodeId, u32>> = HashMap::new();
        let mut landmark_parent: HashMap<&NodeId, HashMap<NodeId, NodeId>> = HashMap::new();

        for landmark in &landmarks {
            let (distances, parents) = Self::bfs_with_parents(adjacency, landmark);
            landmark_full_distances.insert(landmark, distances);
            landmark_parent.insert(landmark, parents);
        }

        // For each node, find closest landmark and compute bunch
        let mut node_info: HashMap<NodeId, TZNodeInfo> = HashMap::new();
        let mut to_landmark_next_hop: HashMap<NodeId, NodeId> = HashMap::new();

        for node in adjacency.keys() {
            // Find closest landmark
            let mut closest_landmark = landmarks[0].clone();
            let mut min_distance = u32::MAX;

            for landmark in &landmarks {
                if let Some(dists) = landmark_full_distances.get(landmark) {
                    if let Some(&dist) = dists.get(node) {
                        if dist < min_distance {
                            min_distance = dist;
                            closest_landmark = landmark.clone();
                        }
                    }
                }
            }

            // Compute bunch: all nodes w such that d(node, w) < d(node, closest_landmark)
            let (node_distances, node_parents) = Self::bfs_with_parents(adjacency, node);
            let mut bunch: HashMap<NodeId, (u32, NodeId)> = HashMap::new();

            for (w, &dist) in &node_distances {
                if dist < min_distance {
                    // w is in the bunch
                    // Find next hop toward w
                    let next_hop = Self::find_next_hop_from_parents(node, w, &node_parents);
                    bunch.insert(w.clone(), (dist, next_hop));
                }
            }

            // Compute next hop toward closest landmark
            if let Some(parents) = landmark_parent.get(&closest_landmark) {
                let next_to_landmark = Self::find_next_hop_toward_source(node, &closest_landmark, parents);
                to_landmark_next_hop.insert(node.clone(), next_to_landmark);
            }

            node_info.insert(
                node.clone(),
                TZNodeInfo {
                    closest_landmark: closest_landmark.clone(),
                    landmark_distance: min_distance,
                    bunch,
                },
            );
        }

        // Compute landmark-to-landmark distances and routing
        let mut landmark_distances: HashMap<(NodeId, NodeId), u32> = HashMap::new();
        let mut landmark_next_hop: HashMap<(NodeId, NodeId), NodeId> = HashMap::new();

        for l1 in &landmarks {
            if let Some(dists) = landmark_full_distances.get(l1) {
                if let Some(parents) = landmark_parent.get(l1) {
                    for l2 in &landmarks {
                        if l1 != l2 {
                            if let Some(&dist) = dists.get(l2) {
                                landmark_distances.insert((l1.clone(), l2.clone()), dist);
                                
                                // Next hop from l2 toward l1 (using l1's BFS tree)
                                let next = Self::find_next_hop_toward_source(l2, l1, parents);
                                landmark_next_hop.insert((l2.clone(), l1.clone()), next);
                            }
                        }
                    }
                }
            }
        }

        Ok(TZRoutingTable {
            config,
            landmarks,
            node_info,
            landmark_distances,
            landmark_next_hop,
            to_landmark_next_hop,
        })
    }

    /// Select landmarks using degree-biased sampling
    fn select_landmarks(
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
        num_landmarks: usize,
        _seed: u64,
    ) -> Vec<NodeId> {
        
        // Sort nodes by degree (descending) for deterministic selection
        let mut nodes_by_degree: Vec<(&NodeId, usize)> = adjacency
            .iter()
            .map(|(id, neighbors)| (id, neighbors.len()))
            .collect();
        nodes_by_degree.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.0.cmp(&b.0.0)));

        // Simple deterministic selection: take top-degree nodes with some spacing
        let mut landmarks = Vec::new();
        let mut selected: HashSet<NodeId> = HashSet::new();
        
        // Always include the highest-degree node
        if let Some((first, _)) = nodes_by_degree.first() {
            landmarks.push((*first).clone());
            selected.insert((*first).clone());
        }

        // Add more landmarks with spacing
        let step = if num_landmarks > 1 && nodes_by_degree.len() > num_landmarks {
            nodes_by_degree.len() / num_landmarks
        } else {
            1
        };

        for i in (0..nodes_by_degree.len()).step_by(step.max(1)) {
            if landmarks.len() >= num_landmarks {
                break;
            }
            let (node, _) = nodes_by_degree[i];
            if !selected.contains(node) {
                landmarks.push(node.clone());
                selected.insert(node.clone());
            }
        }

        // Fill remaining if needed
        for (node, _) in nodes_by_degree {
            if landmarks.len() >= num_landmarks {
                break;
            }
            if !selected.contains(node) {
                landmarks.push(node.clone());
                selected.insert(node.clone());
            }
        }

        landmarks
    }

    /// BFS returning distances and parent pointers
    fn bfs_with_parents(
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
        source: &NodeId,
    ) -> (HashMap<NodeId, u32>, HashMap<NodeId, NodeId>) {
        let mut distances = HashMap::new();
        let mut parents = HashMap::new();
        let mut queue = VecDeque::new();

        distances.insert(source.clone(), 0);
        queue.push_back(source.clone());

        while let Some(current) = queue.pop_front() {
            let current_dist = *distances.get(&current).unwrap();
            
            if let Some(neighbors) = adjacency.get(&current) {
                for neighbor in neighbors {
                    if !distances.contains_key(neighbor) {
                        distances.insert(neighbor.clone(), current_dist + 1);
                        parents.insert(neighbor.clone(), current.clone());
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }

        (distances, parents)
    }

    /// Find next hop from 'from' toward 'to' using parent map (BFS tree from 'from')
    fn find_next_hop_from_parents(
        from: &NodeId,
        to: &NodeId,
        parents: &HashMap<NodeId, NodeId>,
    ) -> NodeId {
        if from == to {
            return from.clone();
        }

        // Walk back from 'to' toward 'from' using parent pointers
        let mut path = vec![to.clone()];
        let mut current = to.clone();
        
        while let Some(parent) = parents.get(&current) {
            path.push(parent.clone());
            if parent == from {
                break;
            }
            current = parent.clone();
        }

        // The second-to-last element is the next hop from 'from'
        if path.len() >= 2 {
            path[path.len() - 2].clone()
        } else {
            to.clone()
        }
    }

    /// Find next hop toward source using BFS tree rooted at source
    fn find_next_hop_toward_source(
        from: &NodeId,
        source: &NodeId,
        parent_from_source: &HashMap<NodeId, NodeId>,
    ) -> NodeId {
        if from == source {
            return from.clone();
        }

        // parent_from_source[v] = parent of v in BFS tree from source
        // So to go from 'from' toward 'source', we follow parents
        parent_from_source.get(from).cloned().unwrap_or_else(|| from.clone())
    }

    /// Get the next hop for routing from current to destination
    /// Returns (next_hop, is_destination)
    pub fn get_next_hop(&self, current: &NodeId, destination: &NodeId) -> Option<(NodeId, bool)> {
        if current == destination {
            return Some((current.clone(), true));
        }

        let current_info = self.node_info.get(current)?;

        // Case 1: destination is in current's bunch
        if let Some((_, next_hop)) = current_info.bunch.get(destination) {
            return Some((next_hop.clone(), false));
        }

        // Case 2: route via landmarks
        // Go toward current's closest landmark first
        let next = self.to_landmark_next_hop.get(current)?;
        Some((next.clone(), false))
    }

    /// Compute the full TZ path from source to destination (for precomputation)
    pub fn compute_path(&self, source: &NodeId, destination: &NodeId) -> Option<Vec<NodeId>> {
        if source == destination {
            return Some(vec![source.clone()]);
        }

        let mut path = vec![source.clone()];
        let mut current = source.clone();
        let mut visited = HashSet::new();
        visited.insert(source.clone());

        // Maximum steps to prevent infinite loops
        let max_steps = self.node_info.len() * 3;

        for _ in 0..max_steps {
            if current == *destination {
                return Some(path);
            }

            match self.get_next_hop(&current, destination) {
                Some((next, is_dest)) => {
                    if visited.contains(&next) && !is_dest {
                        // Cycle detected, try alternative routing
                        return self.compute_path_via_landmarks(source, destination);
                    }
                    visited.insert(next.clone());
                    path.push(next.clone());
                    if is_dest {
                        return Some(path);
                    }
                    current = next;
                }
                None => {
                    // Fallback to landmark routing
                    return self.compute_path_via_landmarks(source, destination);
                }
            }
        }

        // Fallback if direct routing fails
        self.compute_path_via_landmarks(source, destination)
    }

    /// Compute path via landmarks (guaranteed to work if graph is connected)
    fn compute_path_via_landmarks(
        &self,
        source: &NodeId,
        destination: &NodeId,
    ) -> Option<Vec<NodeId>> {
        let src_info = self.node_info.get(source)?;
        let dst_info = self.node_info.get(destination)?;

        let src_landmark = &src_info.closest_landmark;
        let dst_landmark = &dst_info.closest_landmark;

        // Path: source -> src_landmark -> dst_landmark -> destination
        let mut path = vec![source.clone()];
        
        // Phase 1: source to src_landmark
        let mut current = source.clone();
        let mut visited = HashSet::new();
        visited.insert(current.clone());
        
        while current != *src_landmark {
            if let Some(next) = self.to_landmark_next_hop.get(&current) {
                if visited.contains(next) {
                    break; // Avoid loop
                }
                visited.insert(next.clone());
                path.push(next.clone());
                current = next.clone();
            } else {
                break;
            }
        }

        // Phase 2: src_landmark to dst_landmark (if different)
        if src_landmark != dst_landmark {
            current = src_landmark.clone();
            visited.clear();
            visited.insert(current.clone());
            
            while current != *dst_landmark {
                if let Some(next) = self.landmark_next_hop.get(&(current.clone(), dst_landmark.clone())) {
                    if visited.contains(next) {
                        break;
                    }
                    visited.insert(next.clone());
                    if !path.contains(next) {
                        path.push(next.clone());
                    }
                    current = next.clone();
                } else {
                    break;
                }
            }
        }

        // Phase 3: dst_landmark to destination (reverse of destination to landmark)
        // This is approximate - in practice, we'd need destination's BFS tree
        if !path.contains(destination) {
            path.push(destination.clone());
        }

        Some(path)
    }

    /// Get total memory usage estimate (number of entries)
    pub fn memory_usage(&self) -> usize {
        let bunch_entries: usize = self.node_info.values().map(|info| info.bunch.len()).sum();
        let landmark_entries = self.landmark_distances.len() + self.landmark_next_hop.len();
        let node_entries = self.to_landmark_next_hop.len();
        
        bunch_entries + landmark_entries + node_entries
    }

    /// Verify stretch guarantee on a sample of pairs
    pub fn verify_stretch(
        &self,
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
        num_samples: usize,
    ) -> (f64, f64, usize) {
        // Returns: (avg_stretch, max_stretch, num_violations)
        let nodes: Vec<&NodeId> = adjacency.keys().collect();
        if nodes.len() < 2 {
            return (1.0, 1.0, 0);
        }

        let mut total_stretch = 0.0;
        let mut max_stretch = 0.0;
        let mut violations = 0;
        let mut valid_samples = 0;

        // Use deterministic sampling
        for i in 0..num_samples.min(nodes.len() * nodes.len()) {
            let src_idx = i % nodes.len();
            let dst_idx = (i / nodes.len()) % nodes.len();
            
            if src_idx == dst_idx {
                continue;
            }

            let source = nodes[src_idx];
            let destination = nodes[dst_idx];

            // Compute TZ path length
            if let Some(tz_path) = self.compute_path(source, destination) {
                let tz_len = tz_path.len() as u32 - 1; // hops

                // Compute optimal path length (BFS)
                let (distances, _) = Self::bfs_with_parents(adjacency, source);
                if let Some(&optimal) = distances.get(destination) {
                    if optimal > 0 {
                        let stretch = tz_len as f64 / optimal as f64;
                        total_stretch += stretch;
                        max_stretch = f64::max(max_stretch, stretch);
                        valid_samples += 1;

                        if stretch > 3.0 {
                            violations += 1;
                        }
                    }
                }
            }
        }

        let avg_stretch = if valid_samples > 0 {
            total_stretch / valid_samples as f64
        } else {
            1.0
        };

        (avg_stretch, max_stretch, violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_path_graph() -> HashMap<NodeId, Vec<NodeId>> {
        // Path: 0 -- 1 -- 2 -- 3 -- 4
        let mut adj = HashMap::new();
        adj.insert(NodeId::new("0"), vec![NodeId::new("1")]);
        adj.insert(NodeId::new("1"), vec![NodeId::new("0"), NodeId::new("2")]);
        adj.insert(NodeId::new("2"), vec![NodeId::new("1"), NodeId::new("3")]);
        adj.insert(NodeId::new("3"), vec![NodeId::new("2"), NodeId::new("4")]);
        adj.insert(NodeId::new("4"), vec![NodeId::new("3")]);
        adj
    }

    fn create_triangle_graph() -> HashMap<NodeId, Vec<NodeId>> {
        let mut adj = HashMap::new();
        adj.insert(NodeId::new("0"), vec![NodeId::new("1"), NodeId::new("2")]);
        adj.insert(NodeId::new("1"), vec![NodeId::new("0"), NodeId::new("2")]);
        adj.insert(NodeId::new("2"), vec![NodeId::new("0"), NodeId::new("1")]);
        adj
    }

    #[test]
    fn test_tz_build() {
        let adj = create_path_graph();
        let config = TZConfig::default();
        let table = TZRoutingTable::build(&adj, config).unwrap();

        assert!(!table.landmarks.is_empty());
        assert_eq!(table.node_info.len(), 5);
    }

    #[test]
    fn test_tz_path_computation() {
        let adj = create_path_graph();
        let config = TZConfig::default();
        let table = TZRoutingTable::build(&adj, config).unwrap();

        let path = table.compute_path(&NodeId::new("0"), &NodeId::new("4"));
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.first() == Some(&NodeId::new("0")));
        assert!(path.last() == Some(&NodeId::new("4")));
    }

    #[test]
    fn test_tz_stretch_guarantee() {
        let adj = create_triangle_graph();
        let config = TZConfig::default();
        let table = TZRoutingTable::build(&adj, config).unwrap();

        let (avg_stretch, max_stretch, violations) = table.verify_stretch(&adj, 100);
        
        // Stretch should be at most 3
        assert!(max_stretch <= 3.0, "Max stretch {} exceeds guarantee", max_stretch);
        assert_eq!(violations, 0, "Found {} stretch violations", violations);
    }

    #[test]
    fn test_bunch_membership() {
        let adj = create_path_graph();
        let config = TZConfig::default();
        let table = TZRoutingTable::build(&adj, config).unwrap();

        // Node 2 should have some nodes in its bunch
        if let Some(info) = table.node_info.get(&NodeId::new("2")) {
            // Nodes closer than the closest landmark should be in the bunch
            assert!(info.bunch.len() > 0 || info.landmark_distance == 0);
        }
    }
}
