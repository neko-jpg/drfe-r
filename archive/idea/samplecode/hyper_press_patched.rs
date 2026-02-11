//! HYPER-PRESS: H^2 Coordinates + Laplacian Potential Routing
//!
//! Implements the HYPER-PRESS algorithm from idea.md:
//! - Layer A: H^2 hyperbolic coordinates (degree-based radius + spectral angle)
//! - Layer B: Laplacian potential Φ_t for deadlock-free routing
//!
//! score(v) = d_H(v, t) + λ · Φ_t(v)

use crate::coordinates::NodeId;
use std::collections::HashMap;

/// Curvature parameter for hyperbolic space (controls how spread out nodes are)
const DEFAULT_ZETA: f64 = 1.0;

/// Maximum radius in Poincaré disk (nodes are placed between 0 and R)
const DEFAULT_MAX_RADIUS: f64 = 0.95;

/// Number of Gauss-Seidel iterations for potential approximation
const DEFAULT_POTENTIAL_ITERATIONS: usize = 20;

/// Default lambda for combining hyperbolic distance and potential
const DEFAULT_LAMBDA: f64 = 0.3;

/// H^2 coordinate for a node: (radius, angle) in Poincaré disk
#[derive(Debug, Clone)]
pub struct H2Coordinate {
    /// Radius r ∈ [0, R) - hubs have smaller radius (closer to center)
    pub radius: f64,
    /// Angle θ ∈ [0, 2π) - communities are grouped by angle
    pub angle: f64,
}

impl H2Coordinate {
    pub fn new(radius: f64, angle: f64) -> Self {
        Self { radius, angle }
    }
    
    /// Hyperbolic distance in Poincaré disk model
    /// cosh(ζ·d) = cosh(ζ·r1)·cosh(ζ·r2) - sinh(ζ·r1)·sinh(ζ·r2)·cos(Δθ)
    pub fn hyperbolic_distance(&self, other: &H2Coordinate, zeta: f64) -> f64 {
        let delta_theta = (self.angle - other.angle).abs();
        let delta_theta = if delta_theta > std::f64::consts::PI {
            2.0 * std::f64::consts::PI - delta_theta
        } else {
            delta_theta
        };
        
        let cosh_r1 = (zeta * self.radius).cosh();
        let cosh_r2 = (zeta * other.radius).cosh();
        let sinh_r1 = (zeta * self.radius).sinh();
        let sinh_r2 = (zeta * other.radius).sinh();
        
        let cosh_dist = cosh_r1 * cosh_r2 - sinh_r1 * sinh_r2 * delta_theta.cos();
        
        // Clamp to avoid numerical issues with acosh
        let cosh_dist = cosh_dist.max(1.0);
        cosh_dist.acosh() / zeta
    }
}

/// HYPER-PRESS routing engine
pub struct HyperPress {
    /// H^2 coordinates for each node
    coordinates: HashMap<NodeId, H2Coordinate>,
    /// Adjacency list
    adjacency: HashMap<NodeId, Vec<NodeId>>,
    /// Node degrees (cached)
    degrees: HashMap<NodeId, usize>,
    /// Curvature parameter
    zeta: f64,
    /// Lambda for potential weighting
    lambda: f64,
    /// Potential iterations
    potential_iterations: usize,
}

impl HyperPress {
    pub fn new() -> Self {
        Self {
            coordinates: HashMap::new(),
            adjacency: HashMap::new(),
            degrees: HashMap::new(),
            zeta: DEFAULT_ZETA,
            lambda: DEFAULT_LAMBDA,
            potential_iterations: DEFAULT_POTENTIAL_ITERATIONS,
        }
    }
    
    /// Set lambda parameter for potential weighting
    pub fn set_lambda(&mut self, lambda: f64) {
        self.lambda = lambda;
    }
    
    /// Build H^2 coordinates from adjacency list
    /// - Radius: r_i = R - (2/ζ)·ln(k_i/k_min) where R = (2/ζ)·ln(N)
    /// - Angle: spectral ordering using Fiedler vector
    pub fn build_from_adjacency(&mut self, adjacency: &HashMap<NodeId, Vec<NodeId>>) {
        self.adjacency = adjacency.clone();
        
        // Cache degrees
        for (node, neighbors) in adjacency {
            self.degrees.insert(node.clone(), neighbors.len());
        }
        
        let n = adjacency.len() as f64;
        let k_min = self.degrees.values().copied().min().map(|d| d.max(1)).unwrap_or(1) as f64;
        
        // CRITICAL FIX: Use proper hyperbolic radius scale
        // R = (2/ζ) × ln(N) - this gives proper "hierarchy depth" for scale-free graphs
        // For N=78771, ζ=1: R ≈ 22.55
        let max_radius = (2.0 / self.zeta) * n.ln();
        
        let mut radii: HashMap<NodeId, f64> = HashMap::new();
        for (node, &degree) in &self.degrees {
            // r_i = R - (2/ζ)·ln(k_i)
            // High-degree nodes (hubs) get small radius (closer to center)
            // Low-degree nodes (leaves) get large radius (near boundary)
            let k = (degree as f64).max(1.0);
            let radius = max_radius - (2.0 / self.zeta) * (k / k_min).ln();
            
            // Clamp to valid range [0.1, R-0.1]
            let radius = radius.max(0.1).min(max_radius - 0.1);
            radii.insert(node.clone(), radius);
        }
        
        // Compute angles using spectral ordering (simplified: BFS from highest-degree node)
        let angles = self.compute_spectral_angles(adjacency);
        
        // Build final coordinates
        for (node, &radius) in &radii {
            let angle = angles.get(node).copied().unwrap_or(0.0);
            self.coordinates.insert(node.clone(), H2Coordinate::new(radius, angle));
        }
    }
    
    /// Compute angles θ ∈ [0, 2π) using a BFS-tree interval embedding.
    ///
    /// Why this helps:
    /// - Greedy routing in hyperbolic space needs *angular locality* (neighbors should have nearby angles).
    /// - The previous "community rank" scheme often destroys locality on real AS graphs, creating many local minima.
    ///
    /// Method (O(N+E)):
    /// 1) Build a BFS spanning forest, rooting each connected component at a high-degree node.
    /// 2) Compute subtree sizes on that forest.
    /// 3) Allocate each subtree a contiguous angular interval proportional to its size.
    fn compute_spectral_angles(&self, adjacency: &HashMap<NodeId, Vec<NodeId>>) -> HashMap<NodeId, f64> {
        use std::collections::{HashSet, VecDeque};
        
        let n_total = adjacency.len();
        if n_total == 0 {
            return HashMap::new();
        }
        
        // Sort nodes by degree (desc). Each new component root is the highest-degree unvisited node.
        let mut nodes_sorted: Vec<NodeId> = adjacency.keys().cloned().collect();
        nodes_sorted.sort_by(|a, b| {
            let da = adjacency.get(a).map(|v| v.len()).unwrap_or(0);
            let db = adjacency.get(b).map(|v| v.len()).unwrap_or(0);
            db.cmp(&da)
        });
        
        let two_pi = 2.0 * std::f64::consts::PI;
        let mut theta_cursor = 0.0;
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut angles: HashMap<NodeId, f64> = HashMap::new();
        
        for root in nodes_sorted {
            if visited.contains(&root) {
                continue;
            }
            // --- BFS spanning tree for this component ---
            let mut queue: VecDeque<NodeId> = VecDeque::new();
            queue.push_back(root.clone());
            visited.insert(root.clone());
            
            let mut order: Vec<NodeId> = Vec::new();
            let mut children: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
            
            while let Some(u) = queue.pop_front() {
                order.push(u.clone());
                if let Some(neigh) = adjacency.get(&u) {
                    for v in neigh {
                        if !visited.contains(v) {
                            visited.insert(v.clone());
                            queue.push_back(v.clone());
                            children.entry(u.clone()).or_default().push(v.clone());
                        }
                    }
                }
            }
            
            let comp_size = order.len();
            if comp_size == 0 {
                continue;
            }
            
            // --- Subtree sizes (postorder) ---
            let mut subtree_size: HashMap<NodeId, usize> = HashMap::new();
            for u in order.iter().rev() {
                let mut s = 1usize;
                if let Some(ch) = children.get(u) {
                    for v in ch {
                        s += *subtree_size.get(v).unwrap_or(&1);
                    }
                }
                subtree_size.insert(u.clone(), s);
            }
            // Sort children by descending subtree size so big clusters get contiguous blocks first.
            for (_u, ch) in children.iter_mut() {
                ch.sort_by(|a, b| {
                    let sa = *subtree_size.get(a).unwrap_or(&1);
                    let sb = *subtree_size.get(b).unwrap_or(&1);
                    sb.cmp(&sa)
                });
            }
            
            // Allocate an angular interval proportional to component size.
            let span = two_pi * (comp_size as f64) / (n_total as f64);
            let start = theta_cursor;
            let end = theta_cursor + span;
            theta_cursor = end;
            
            // --- Interval assignment (stack DFS) ---
            let mut stack: Vec<(NodeId, f64, f64)> = Vec::new();
            stack.push((root.clone(), start, end));
            while let Some((u, a0, a1)) = stack.pop() {
                angles.insert(u.clone(), (a0 + a1) / 2.0);
                if let Some(ch) = children.get(&u) {
                    if ch.is_empty() {
                        continue;
                    }
                    let total: usize = ch.iter().map(|v| *subtree_size.get(v).unwrap_or(&1)).sum();
                    if total == 0 {
                        continue;
                    }
                    let mut cursor = a0;
                    for v in ch {
                        let frac = (*subtree_size.get(v).unwrap_or(&1) as f64) / (total as f64);
                        let w = (a1 - a0) * frac;
                        let b0 = cursor;
                        let b1 = cursor + w;
                        cursor = b1;
                        stack.push((v.clone(), b0, b1));
                    }
                }
            }
        }
        
        angles
    }
    
    /// Get H^2 coordinate for a node
    pub fn get_coordinate(&self, node: &NodeId) -> Option<&H2Coordinate> {
        self.coordinates.get(node)
    }
    
    /// Compute Laplacian potential Φ_t using Gauss-Seidel iteration
    /// Compute Laplacian potential Φ_t (hitting-time form) using Gauss-Seidel iteration
    ///
    /// We use the discrete hitting-time equation (absorbing sink at `target`):
    ///   φ(target) = 0
    ///   φ(u) = 1 + (1/deg(u)) · Σ_{v ∈ N(u)} φ(v)   for u ≠ target
    ///
    /// Key reason: the plain averaging form collapses to φ≡0 and gives no gradient.
    /// This form guarantees (for the updated component) that every non-sink node has
    /// at least one neighbor with strictly smaller φ, so it can break deadlocks.
    pub fn compute_potential(&self, target: &NodeId) -> HashMap<NodeId, f64> {
        use std::collections::{HashSet, VecDeque};
        
        // Restrict to the connected component of the target (avoid divergence in other components).
        let mut reachable: HashSet<NodeId> = HashSet::new();
        if self.adjacency.contains_key(target) {
            let mut q: VecDeque<NodeId> = VecDeque::new();
            q.push_back(target.clone());
            reachable.insert(target.clone());
            while let Some(u) = q.pop_front() {
                if let Some(neigh) = self.adjacency.get(&u) {
                    for v in neigh {
                        if !reachable.contains(v) {
                            reachable.insert(v.clone());
                            q.push_back(v.clone());
                        }
                    }
                }
            }
        }
        
        let mut phi: HashMap<NodeId, f64> = HashMap::new();
        for node in self.adjacency.keys() {
            if node == target {
                phi.insert(node.clone(), 0.0);
            } else if reachable.is_empty() || reachable.contains(node) {
                phi.insert(node.clone(), 1.0);
            } else {
                phi.insert(node.clone(), f64::INFINITY);
            }
        }
        
        for _ in 0..self.potential_iterations {
            for (node, neighbors) in &self.adjacency {
                if node == target {
                    continue;
                }
                if !reachable.is_empty() && !reachable.contains(node) {
                    continue;
                }
                if neighbors.is_empty() {
                    continue;
                }
                let mut sum = 0.0;
                let mut deg = 0usize;
                for v in neighbors {
                    if let Some(&val) = phi.get(v) {
                        if val.is_finite() {
                            sum += val;
                            deg += 1;
                        }
                    }
                }
                if deg == 0 {
                    continue;
                }
                phi.insert(node.clone(), 1.0 + sum / (deg as f64));
            }
        }
        
        phi
    }
    
    /// Compute hybrid score: d_H(v, t) + λ · Φ_t(v)
    pub fn compute_score(
        &self,
        node: &NodeId,
        target: &NodeId,
        target_coord: &H2Coordinate,
        potential: &HashMap<NodeId, f64>,
    ) -> f64 {
        let node_coord = match self.coordinates.get(node) {
            Some(c) => c,
            None => return f64::INFINITY,
        };
        
        let h2_distance = node_coord.hyperbolic_distance(target_coord, self.zeta);
        let phi = potential.get(node).copied().unwrap_or(1.0);
        
        h2_distance + self.lambda * phi
    }
    
    /// Find best next hop using HYPER-PRESS scoring
    pub fn find_best_neighbor(
        &self,
        current: &NodeId,
        target: &NodeId,
        visited: &std::collections::HashSet<NodeId>,
    ) -> Option<NodeId> {
        let neighbors = self.adjacency.get(current)?;
        let target_coord = self.coordinates.get(target)?;
        
        // Compute potential for this target (can be cached for efficiency)
        let potential = self.compute_potential(target);
        
        let current_score = self.compute_score(current, target, target_coord, &potential);
        
        let mut best_neighbor: Option<NodeId> = None;
        let mut best_score = current_score;
        
        for neighbor in neighbors {
            if visited.contains(neighbor) {
                continue;
            }
            
            let score = self.compute_score(neighbor, target, target_coord, &potential);
            if score < best_score {
                best_score = score;
                best_neighbor = Some(neighbor.clone());
            }
        }
        
        best_neighbor
    }
    
    /// 1-hop lookahead: evaluate neighbors' neighbors
    pub fn find_best_neighbor_lookahead(
        &self,
        current: &NodeId,
        target: &NodeId,
        visited: &std::collections::HashSet<NodeId>,
    ) -> Option<NodeId> {
        let neighbors = self.adjacency.get(current)?;
        let target_coord = self.coordinates.get(target)?;
        let potential = self.compute_potential(target);
        
        let current_score = self.compute_score(current, target, target_coord, &potential);
        
        let mut best_first_hop: Option<NodeId> = None;
        let mut best_two_hop_score = current_score;
        
        for neighbor in neighbors {
            if visited.contains(neighbor) {
                continue;
            }
            
            // Evaluate best 2-hop path through this neighbor
            let neighbor_score = self.compute_score(neighbor, target, target_coord, &potential);
            
            // Check neighbor's neighbors
            let mut best_continuation = neighbor_score;
            if let Some(second_neighbors) = self.adjacency.get(neighbor) {
                for second in second_neighbors {
                    if second == current || visited.contains(second) {
                        continue;
                    }
                    let second_score = self.compute_score(second, target, target_coord, &potential);
                    if second_score < best_continuation {
                        best_continuation = second_score;
                    }
                }
            }
            
            // Use average of direct and best continuation as score
            let combined_score = (neighbor_score + best_continuation) / 2.0;
            
            if combined_score < best_two_hop_score {
                best_two_hop_score = combined_score;
                best_first_hop = Some(neighbor.clone());
            }
        }
        
        best_first_hop
    }
    
    /// Extract k-hop local subgraph centered at `center`
    /// Returns adjacency list of the subgraph and mapping from NodeId to local index
    fn extract_local_subgraph(
        &self,
        center: &NodeId,
        k_hops: usize,
    ) -> (Vec<NodeId>, HashMap<NodeId, usize>, HashMap<usize, Vec<usize>>) {
        use std::collections::{VecDeque, HashSet};
        
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut local_nodes = Vec::new();
        
        queue.push_back((center.clone(), 0));
        visited.insert(center.clone());
        
        while let Some((node, depth)) = queue.pop_front() {
            local_nodes.push(node.clone());
            
            if depth < k_hops {
                if let Some(neighbors) = self.adjacency.get(&node) {
                    for neighbor in neighbors {
                        if !visited.contains(neighbor) {
                            visited.insert(neighbor.clone());
                            queue.push_back((neighbor.clone(), depth + 1));
                        }
                    }
                }
            }
        }
        
        // Build local index mapping
        let node_to_idx: HashMap<NodeId, usize> = local_nodes.iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), i))
            .collect();
        
        // Build local adjacency (only edges within the subgraph)
        let mut local_adj: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, node) in local_nodes.iter().enumerate() {
            if let Some(neighbors) = self.adjacency.get(node) {
                let local_neighbors: Vec<usize> = neighbors.iter()
                    .filter_map(|n| node_to_idx.get(n).copied())
                    .collect();
                local_adj.insert(i, local_neighbors);
            }
        }
        
        (local_nodes, node_to_idx, local_adj)
    }
    
    /// Compute Laplacian potential on a LOCAL subgraph only (fast!)
    /// target_in_subgraph: the local index of the target (or closest boundary node to target)
    fn compute_local_potential(
        &self,
        local_nodes: &[NodeId],
        node_to_idx: &HashMap<NodeId, usize>,
        local_adj: &HashMap<usize, Vec<usize>>,
        target: &NodeId,
        target_coord: &H2Coordinate,
    ) -> HashMap<NodeId, f64> {
        let n = local_nodes.len();
        if n == 0 {
            return HashMap::new();
        }
        
        // Find sink node: target if in subgraph, else boundary node closest to target
        let sink_idx = if let Some(&idx) = node_to_idx.get(target) {
            idx
        } else {
            // Find boundary node closest to target in H² space
            let mut best_idx = 0;
            let mut best_dist = f64::INFINITY;
            for (i, node) in local_nodes.iter().enumerate() {
                if let Some(coord) = self.coordinates.get(node) {
                    let dist = coord.hyperbolic_distance(target_coord, self.zeta);
                    if dist < best_dist {
                        best_dist = dist;
                        best_idx = i;
                    }
                }
            }
            best_idx
        };
        
        // Initialize local potential (hitting time型: 遠いほど大きくなる)
        let mut phi = vec![1.0; n];
        phi[sink_idx] = 0.0;
        
        // CRITICAL FIX: Use hitting time formulation (add +1)
        // φ(t) = 0, φ(u) = 1 + (1/deg) × Σφ(v)
        // This prevents values from collapsing to zero
        for _ in 0..self.potential_iterations {
            for i in 0..n {
                if i == sink_idx {
                    continue;
                }
                if let Some(neighbors) = local_adj.get(&i) {
                    if !neighbors.is_empty() {
                        let sum: f64 = neighbors.iter().map(|&j| phi[j]).sum();
                        // +1 term makes this hitting time (expected steps to reach sink)
                        phi[i] = 1.0 + sum / (neighbors.len() as f64);
                    }
                }
            }
        }
        
        // Map back to NodeId
        let mut result = HashMap::new();
        for (i, node) in local_nodes.iter().enumerate() {
            result.insert(node.clone(), phi[i]);
        }
        result
    }
    
    /// FAST HYPER-PRESS routing for large graphs:
    /// 1. Try H² distance only (no potential computation)
    /// 2. If stuck (no improving neighbor), compute LOCAL potential and use it
    pub fn find_best_neighbor_fast(
        &self,
        current: &NodeId,
        target: &NodeId,
        visited: &std::collections::HashSet<NodeId>,
    ) -> Option<NodeId> {
        let neighbors = self.adjacency.get(current)?;
        let target_coord = self.coordinates.get(target)?;
        let current_coord = self.coordinates.get(current)?;
        
        let current_h2_dist = current_coord.hyperbolic_distance(target_coord, self.zeta);
        
        // === Phase 1: Try pure H² greedy (no potential) ===
        let mut best_h2_neighbor: Option<NodeId> = None;
        let mut best_h2_dist = current_h2_dist;
        
        for neighbor in neighbors {
            if visited.contains(neighbor) || neighbor == target {
                if neighbor == target {
                    return Some(neighbor.clone()); // Destination is neighbor!
                }
                continue;
            }
            
            if let Some(neighbor_coord) = self.coordinates.get(neighbor) {
                let dist = neighbor_coord.hyperbolic_distance(target_coord, self.zeta);
                if dist < best_h2_dist {
                    best_h2_dist = dist;
                    best_h2_neighbor = Some(neighbor.clone());
                }
            }
        }
        
        // If we found an improving neighbor using H² only, use it (fast path)
        if best_h2_neighbor.is_some() {
            return best_h2_neighbor;
        }
        
        // === Phase 2: Stuck in local minimum. Use LOCAL potential ===
        // IMPROVEMENT A: Increase k-hop from 2 to 3 for better coverage
        const K_HOPS: usize = 3;
        
        let (local_nodes, node_to_idx, local_adj) = self.extract_local_subgraph(current, K_HOPS);
        let local_potential = self.compute_local_potential(
            &local_nodes,
            &node_to_idx,
            &local_adj,
            target,
            target_coord,
        );
        
        // IMPROVEMENT C: Auto-normalize λ based on local H² and φ scales
        // This prevents λ from being useless (too small) or dominating (too large)
        let (adaptive_lambda, _h2_scale) = {
            let mut h2_vals: Vec<f64> = Vec::new();
            let mut phi_vals: Vec<f64> = Vec::new();
            
            for neighbor in neighbors {
                if let Some(coord) = self.coordinates.get(neighbor) {
                    h2_vals.push(coord.hyperbolic_distance(target_coord, self.zeta));
                }
                if let Some(&phi) = local_potential.get(neighbor) {
                    phi_vals.push(phi);
                }
            }
            
            if h2_vals.len() >= 2 && phi_vals.len() >= 2 {
                h2_vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
                phi_vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
                
                let h2_range = h2_vals.last().unwrap() - h2_vals.first().unwrap();
                let phi_range = phi_vals.last().unwrap() - phi_vals.first().unwrap();
                
                if phi_range > 1e-6 {
                    // λ = α × (d_H range) / (φ range)
                    (0.5 * h2_range / phi_range, h2_range)
                } else {
                    (self.lambda, h2_vals.last().copied().unwrap_or(1.0))
                }
            } else {
                (self.lambda, 1.0)
            }
        };
        
        // Phase 2 routing policy:
        //   (a) Prefer a strict descent in the local potential Φ (escape local minima).
        //   (b) If Φ descent is not found (rare / numerical), pick the smallest hybrid score
        //       even if it does not improve the current score.
        //       Returning None here triggers random fallback and kills success rate.
        
        let current_phi = local_potential.get(current).copied().unwrap_or(f64::INFINITY);
        
        // (a) Strict Φ descent
        let mut best_phi_neighbor: Option<NodeId> = None;
        let mut best_phi = current_phi;
        for neighbor in neighbors {
            if visited.contains(neighbor) {
                continue;
            }
            if let Some(&phi) = local_potential.get(neighbor) {
                if phi + 1e-9 < best_phi {
                    best_phi = phi;
                    best_phi_neighbor = Some(neighbor.clone());
                }
            }
        }
        if best_phi_neighbor.is_some() {
            return best_phi_neighbor;
        }
        
        // (b) Fallback inside HyperPress: smallest hybrid score (no "must improve" constraint)
        let mut best_neighbor: Option<NodeId> = None;
        let mut best_score = f64::INFINITY;
        for neighbor in neighbors {
            if visited.contains(neighbor) {
                continue;
            }
            if let Some(neighbor_coord) = self.coordinates.get(neighbor) {
                let h2_dist = neighbor_coord.hyperbolic_distance(target_coord, self.zeta);
                let phi = local_potential.get(neighbor).copied().unwrap_or(1.0);
                let score = h2_dist + adaptive_lambda * phi;
                if score < best_score {
                    best_score = score;
                    best_neighbor = Some(neighbor.clone());
                }
            }
        }
        
        best_neighbor
    }
    
    /// Get statistics about the embedding
    pub fn stats(&self) -> HyperPressStats {
        let radii: Vec<f64> = self.coordinates.values().map(|c| c.radius).collect();
        let min_radius = radii.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_radius = radii.iter().cloned().fold(0.0, f64::max);
        let avg_radius = radii.iter().sum::<f64>() / radii.len() as f64;
        
        HyperPressStats {
            node_count: self.coordinates.len(),
            min_radius,
            max_radius,
            avg_radius,
            zeta: self.zeta,
            lambda: self.lambda,
        }
    }
}

impl Default for HyperPress {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct HyperPressStats {
    pub node_count: usize,
    pub min_radius: f64,
    pub max_radius: f64,
    pub avg_radius: f64,
    pub zeta: f64,
    pub lambda: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_h2_distance() {
        let c1 = H2Coordinate::new(0.5, 0.0);
        let c2 = H2Coordinate::new(0.5, std::f64::consts::PI);
        
        let dist = c1.hyperbolic_distance(&c2, 1.0);
        assert!(dist > 0.0);
        
        // Same point should have zero distance
        let dist_self = c1.hyperbolic_distance(&c1, 1.0);
        assert!(dist_self < 1e-10);
    }
    
    #[test]
    fn test_build_from_adjacency() {
        let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        adj.insert(NodeId("a".into()), vec![NodeId("b".into()), NodeId("c".into())]);
        adj.insert(NodeId("b".into()), vec![NodeId("a".into())]);
        adj.insert(NodeId("c".into()), vec![NodeId("a".into())]);
        
        let mut hp = HyperPress::new();
        hp.build_from_adjacency(&adj);
        
        // Hub (node a) should have smaller radius
        let r_a = hp.get_coordinate(&NodeId("a".into())).unwrap().radius;
        let r_b = hp.get_coordinate(&NodeId("b".into())).unwrap().radius;
        
        assert!(r_a < r_b, "Hub should be closer to center");
    }
    
    #[test]
    fn test_potential() {
        let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        adj.insert(NodeId("a".into()), vec![NodeId("b".into())]);
        adj.insert(NodeId("b".into()), vec![NodeId("a".into()), NodeId("c".into())]);
        adj.insert(NodeId("c".into()), vec![NodeId("b".into())]);
        
        let mut hp = HyperPress::new();
        hp.build_from_adjacency(&adj);
        
        let potential = hp.compute_potential(&NodeId("c".into()));
        
        // φ(c) should be 0
        assert_eq!(*potential.get(&NodeId("c".into())).unwrap(), 0.0);
        
        // φ(a) >= φ(b) since a is farther from c (may be equal if converged)
        let phi_a = *potential.get(&NodeId("a".into())).unwrap();
        let phi_b = *potential.get(&NodeId("b".into())).unwrap();
        assert!(phi_a >= phi_b || (phi_a - phi_b).abs() < 0.1, 
            "Expected φ(a) >= φ(b), got φ(a)={}, φ(b)={}", phi_a, phi_b);
    }
}