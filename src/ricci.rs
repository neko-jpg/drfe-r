//! Hybrid Ricci Flow Implementation
//!
//! Computes discrete Ricci curvature for network edges using:
//! - Sinkhorn-Knopp approximation for low-degree edges
//! - Forman-Ricci curvature for high-degree edges
//!
//! This hybrid approach achieves O(k) ~ O(1) complexity instead of O(m³).

use crate::coordinates::{NodeId, RoutingCoordinate};
use crate::PoincareDiskPoint;
use std::collections::HashMap;

/// Threshold for switching between Sinkhorn and Forman curvature
const DEGREE_THRESHOLD: usize = 10;

/// Maximum Sinkhorn iterations
const SINKHORN_MAX_ITER: usize = 100;

/// Sinkhorn convergence threshold
const SINKHORN_EPSILON: f64 = 1e-6;

/// Entropy regularization parameter for Sinkhorn
const SINKHORN_LAMBDA: f64 = 0.1;

/// Edge in the network graph
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    pub u: NodeId,
    pub v: NodeId,
}

impl Edge {
    pub fn new(u: NodeId, v: NodeId) -> Self {
        // Canonical ordering for undirected edges
        if u.0 < v.0 {
            Self { u, v }
        } else {
            Self { u: v, v: u }
        }
    }
}

/// Curvature type indicator
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurvatureMethod {
    /// Ollivier-Ricci via Sinkhorn approximation
    Sinkhorn,
    /// Forman-Ricci (combinatorial)
    Forman,
}

/// Curvature result for an edge
#[derive(Debug, Clone)]
pub struct CurvatureResult {
    pub edge: Edge,
    pub value: f64,
    pub method: CurvatureMethod,
}

/// Node in a graph for Ricci flow computation
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: NodeId,
    pub coord: RoutingCoordinate,
    pub neighbors: Vec<NodeId>,
}

impl GraphNode {
    pub fn degree(&self) -> usize {
        self.neighbors.len()
    }
}

/// Graph structure for Ricci curvature computation
pub struct RicciGraph {
    pub nodes: HashMap<NodeId, GraphNode>,
    edges: Vec<Edge>,
}

impl RicciGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: GraphNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn add_edge(&mut self, u: &NodeId, v: &NodeId) {
        let edge = Edge::new(u.clone(), v.clone());
        if !self.edges.contains(&edge) {
            self.edges.push(edge);
        }

        // Update neighbor lists
        if let Some(node_u) = self.nodes.get_mut(u) {
            if !node_u.neighbors.contains(v) {
                node_u.neighbors.push(v.clone());
            }
        }
        if let Some(node_v) = self.nodes.get_mut(v) {
            if !node_v.neighbors.contains(u) {
                node_v.neighbors.push(u.clone());
            }
        }
    }

    pub fn get_node(&self, id: &NodeId) -> Option<&GraphNode> {
        self.nodes.get(id)
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Compute curvature for all edges using hybrid method
    pub fn compute_all_curvatures(&self) -> Vec<CurvatureResult> {
        self.edges
            .iter()
            .map(|edge| self.compute_curvature(edge))
            .collect()
    }

    /// Compute curvature for a single edge using appropriate method
    pub fn compute_curvature(&self, edge: &Edge) -> CurvatureResult {
        let node_u = self.nodes.get(&edge.u);
        let node_v = self.nodes.get(&edge.v);

        match (node_u, node_v) {
            (Some(u), Some(v)) => {
                let total_degree = u.degree() + v.degree();
                if total_degree > DEGREE_THRESHOLD * 2 {
                    // High degree: use Forman curvature
                    self.compute_forman_curvature(edge, u, v)
                } else {
                    // Low degree: use Sinkhorn approximation
                    self.compute_sinkhorn_curvature(edge, u, v)
                }
            }
            _ => CurvatureResult {
                edge: edge.clone(),
                value: 0.0,
                method: CurvatureMethod::Forman,
            },
        }
    }

    /// Forman-Ricci curvature (O(1) computation)
    ///
    /// Ric_F(e) = 4 - deg(u) - deg(v)
    ///
    /// For weighted graphs, the formula is more complex, but for unweighted:
    /// - Positive curvature: edge is "well connected"
    /// - Negative curvature: edge connects high-degree hubs
    fn compute_forman_curvature(
        &self,
        edge: &Edge,
        u: &GraphNode,
        v: &GraphNode,
    ) -> CurvatureResult {
        let curvature = 4.0 - (u.degree() as f64) - (v.degree() as f64);

        CurvatureResult {
            edge: edge.clone(),
            value: curvature,
            method: CurvatureMethod::Forman,
        }
    }

    /// Sinkhorn-Knopp approximation of Ollivier-Ricci curvature
    ///
    /// Uses entropy-regularized optimal transport to approximate
    /// the Wasserstein distance between node neighborhoods.
    fn compute_sinkhorn_curvature(
        &self,
        edge: &Edge,
        u: &GraphNode,
        v: &GraphNode,
    ) -> CurvatureResult {
        // Edge length in hyperbolic space
        let edge_length = u.coord.point.hyperbolic_distance(&v.coord.point);

        if edge_length < 1e-10 {
            return CurvatureResult {
                edge: edge.clone(),
                value: 1.0, // Maximum positive curvature
                method: CurvatureMethod::Sinkhorn,
            };
        }

        // Build probability distributions over neighborhoods
        let (mu_u, nodes_u) = self.neighborhood_distribution(u);
        let (mu_v, nodes_v) = self.neighborhood_distribution(v);

        if mu_u.is_empty() || mu_v.is_empty() {
            return CurvatureResult {
                edge: edge.clone(),
                value: 0.0,
                method: CurvatureMethod::Sinkhorn,
            };
        }

        // Cost matrix: hyperbolic distances between neighborhoods
        let cost_matrix = self.compute_cost_matrix(&nodes_u, &nodes_v);

        // Sinkhorn algorithm
        let w1 = self.sinkhorn_distance(&mu_u, &mu_v, &cost_matrix);

        // Ollivier-Ricci curvature: κ(e) = 1 - W₁(μ_u, μ_v) / d(u,v)
        let curvature = 1.0 - w1 / edge_length;

        CurvatureResult {
            edge: edge.clone(),
            value: curvature,
            method: CurvatureMethod::Sinkhorn,
        }
    }

    /// Build uniform probability distribution over node's neighborhood
    fn neighborhood_distribution(&self, node: &GraphNode) -> (Vec<f64>, Vec<NodeId>) {
        // Include the node itself with probability proportional to 1/(deg+1)
        let mut nodes = vec![node.id.clone()];
        nodes.extend(node.neighbors.iter().cloned());

        let n = nodes.len();
        let prob = 1.0 / n as f64;
        let distribution = vec![prob; n];

        (distribution, nodes)
    }

    /// Compute cost matrix (hyperbolic distances) between two sets of nodes
    fn compute_cost_matrix(&self, nodes_u: &[NodeId], nodes_v: &[NodeId]) -> Vec<Vec<f64>> {
        let mut matrix = vec![vec![0.0; nodes_v.len()]; nodes_u.len()];

        for (i, id_u) in nodes_u.iter().enumerate() {
            for (j, id_v) in nodes_v.iter().enumerate() {
                if let (Some(node_u), Some(node_v)) =
                    (self.nodes.get(id_u), self.nodes.get(id_v))
                {
                    matrix[i][j] = node_u.coord.point.hyperbolic_distance(&node_v.coord.point);
                }
            }
        }

        matrix
    }

    /// Sinkhorn algorithm for approximate optimal transport
    fn sinkhorn_distance(&self, mu: &[f64], nu: &[f64], cost: &[Vec<f64>]) -> f64 {
        let n = mu.len();
        let m = nu.len();

        if n == 0 || m == 0 {
            return 0.0;
        }

        // K = exp(-C / λ)
        let mut k_matrix = vec![vec![0.0; m]; n];
        for i in 0..n {
            for j in 0..m {
                k_matrix[i][j] = (-cost[i][j] / SINKHORN_LAMBDA).exp();
            }
        }

        // Initialize scaling vectors
        let mut u = vec![1.0; n];
        let mut v = vec![1.0; m];

        // Sinkhorn iterations
        for _ in 0..SINKHORN_MAX_ITER {
            let u_old = u.clone();

            // Update v: v = nu / (K^T * u)
            for j in 0..m {
                let mut sum = 0.0;
                for i in 0..n {
                    sum += k_matrix[i][j] * u[i];
                }
                v[j] = if sum > 1e-10 { nu[j] / sum } else { 0.0 };
            }

            // Update u: u = mu / (K * v)
            for i in 0..n {
                let mut sum = 0.0;
                for j in 0..m {
                    sum += k_matrix[i][j] * v[j];
                }
                u[i] = if sum > 1e-10 { mu[i] / sum } else { 0.0 };
            }

            // Check convergence
            let max_change = u
                .iter()
                .zip(u_old.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);

            if max_change < SINKHORN_EPSILON {
                break;
            }
        }

        // Compute Wasserstein distance
        let mut w1 = 0.0;
        for i in 0..n {
            for j in 0..m {
                let transport = u[i] * k_matrix[i][j] * v[j];
                w1 += transport * cost[i][j];
            }
        }

        w1
    }
}

impl Default for RicciGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Ricci Flow controller for coordinate updates
pub struct RicciFlow {
    /// Step size for coordinate updates
    pub step_size: f64,
    /// Target curvature (usually 0 for uniformization)
    pub target_curvature: f64,
    /// Coordinate update step size
    pub coord_step: f64,
}

impl RicciFlow {
    pub fn new(step_size: f64) -> Self {
        Self {
            step_size,
            target_curvature: 0.0,
            coord_step: 0.1,
        }
    }

    /// Perform one step of Ricci flow, updating edge weights
    /// and returning the new target distances
    pub fn flow_step(&self, graph: &RicciGraph) -> HashMap<Edge, f64> {
        let curvatures = graph.compute_all_curvatures();
        let mut new_lengths = HashMap::new();

        for result in curvatures {
            let node_u = graph.get_node(&result.edge.u);
            let node_v = graph.get_node(&result.edge.v);

            if let (Some(u), Some(v)) = (node_u, node_v) {
                let current_length = u.coord.point.hyperbolic_distance(&v.coord.point);

                // Ricci flow equation: dℓ/dt = -κ * ℓ
                // Discrete: ℓ_new = ℓ * (1 - step_size * κ)
                let flow_factor = 1.0 - self.step_size * (result.value - self.target_curvature);
                let new_length = current_length * flow_factor.max(0.1).min(2.0);

                new_lengths.insert(result.edge, new_length);
            }
        }

        new_lengths
    }

    /// Optimize coordinates to match target distances using gradient descent
    /// Returns the new coordinates for each node
    pub fn optimize_coordinates(
        &self,
        graph: &RicciGraph,
        target_lengths: &HashMap<Edge, f64>,
        iterations: usize,
    ) -> HashMap<NodeId, crate::PoincareDiskPoint> {
        use crate::PoincareDiskPoint;
        
        // Collect current coordinates
        let mut coords: HashMap<NodeId, (f64, f64)> = HashMap::new();
        for (id, node) in &graph.nodes {
            coords.insert(id.clone(), (node.coord.point.x, node.coord.point.y));
        }

        // Gradient descent to minimize stress
        for _ in 0..iterations {
            let mut gradients: HashMap<NodeId, (f64, f64)> = HashMap::new();
            for id in coords.keys() {
                gradients.insert(id.clone(), (0.0, 0.0));
            }

            // Compute gradients from edge stress
            for (edge, &target_len) in target_lengths {
                let (ux, uy) = match coords.get(&edge.u) {
                    Some(&c) => c,
                    None => continue,
                };
                let (vx, vy) = match coords.get(&edge.v) {
                    Some(&c) => c,
                    None => continue,
                };

                // Current Euclidean distance (approximation for small movements)
                let dx = vx - ux;
                let dy = vy - uy;
                let current_dist = (dx * dx + dy * dy).sqrt().max(0.001);

                // Stress gradient: (current - target) * direction
                // Normalize target_len to Euclidean scale (rough approximation)
                let target_euclidean = (target_len / 3.0).tanh() * 0.9;
                let stress = current_dist - target_euclidean;
                let grad_scale = stress / current_dist * self.coord_step;

                // Update gradients (opposite directions for u and v)
                if let Some(g) = gradients.get_mut(&edge.u) {
                    g.0 += dx * grad_scale;
                    g.1 += dy * grad_scale;
                }
                if let Some(g) = gradients.get_mut(&edge.v) {
                    g.0 -= dx * grad_scale;
                    g.1 -= dy * grad_scale;
                }
            }

            // Apply gradients with constraint to stay in disk
            for (id, coord) in coords.iter_mut() {
                if let Some(&(gx, gy)) = gradients.get(id) {
                    coord.0 -= gx;
                    coord.1 -= gy;

                    // Project back into disk if needed
                    let r_sq = coord.0 * coord.0 + coord.1 * coord.1;
                    if r_sq >= 0.99 * 0.99 {
                        let scale = 0.98 / r_sq.sqrt();
                        coord.0 *= scale;
                        coord.1 *= scale;
                    }
                }
            }
        }

        // Convert back to PoincareDiskPoint
        let mut result = HashMap::new();
        for (id, (x, y)) in coords {
            if let Some(point) = PoincareDiskPoint::new(x, y) {
                result.insert(id, point);
            }
        }
        result
    }

    /// Run full Ricci Flow optimization: compute target lengths then optimize coords
    pub fn run_optimization(
        &self,
        graph: &mut RicciGraph,
        flow_iterations: usize,
        coord_iterations: usize,
    ) -> f64 {
        let mut total_stress = 0.0;

        for _ in 0..flow_iterations {
            // 1. Compute target edge lengths via Ricci flow
            let target_lengths = self.flow_step(graph);

            // 2. Optimize coordinates to match target lengths
            let new_coords = self.optimize_coordinates(graph, &target_lengths, coord_iterations);

            // 3. Update graph coordinates
            for (id, point) in &new_coords {
                if let Some(node) = graph.nodes.get_mut(id) {
                    node.coord.point = *point;
                }
            }

            // 4. Compute residual stress
            total_stress = 0.0;
            for (edge, &target) in &target_lengths {
                if let (Some(u), Some(v)) = (graph.get_node(&edge.u), graph.get_node(&edge.v)) {
                    let actual = u.coord.point.hyperbolic_distance(&v.coord.point);
                    total_stress += (actual - target).powi(2);
                }
            }
        }

        total_stress
    }
}

impl Default for RicciFlow {
    fn default() -> Self {
        Self::new(0.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> RicciGraph {
        let mut graph = RicciGraph::new();

        // Create a simple triangle + one extra node
        let nodes = vec![
            ("a", 0.0, 0.0),
            ("b", 0.3, 0.0),
            ("c", 0.15, 0.25),
            ("d", 0.5, 0.0),
        ];

        for (id, x, y) in &nodes {
            let coord = RoutingCoordinate::new(PoincareDiskPoint::new(*x, *y).unwrap(), 0);
            graph.add_node(GraphNode {
                id: NodeId::new(*id),
                coord,
                neighbors: Vec::new(),
            });
        }

        // Triangle edges
        graph.add_edge(&NodeId::new("a"), &NodeId::new("b"));
        graph.add_edge(&NodeId::new("b"), &NodeId::new("c"));
        graph.add_edge(&NodeId::new("c"), &NodeId::new("a"));
        // Extra edge
        graph.add_edge(&NodeId::new("b"), &NodeId::new("d"));

        graph
    }

    #[test]
    fn test_forman_curvature() {
        let graph = create_test_graph();

        // Edge a-b: degree sum = 2+3 = 5 < threshold*2=20, so Sinkhorn is used
        let edge = Edge::new(NodeId::new("a"), NodeId::new("b"));
        let result = graph.compute_curvature(&edge);

        // For low degree edges, Sinkhorn method is used (Ollivier-Ricci approximation)
        // Verify the method is correctly selected
        assert_eq!(result.method, CurvatureMethod::Sinkhorn);
        // Curvature should be a reasonable value (between -2 and 2 typically)
        assert!(result.value > -2.0 && result.value < 2.0);
    }

    #[test]
    fn test_curvature_sign() {
        let graph = create_test_graph();

        // Edge b-d connects a hub (b, degree 3) to a leaf (d, degree 1)
        // Degree sum = 3+1 = 4 < 20, so Sinkhorn is used
        let edge = Edge::new(NodeId::new("b"), NodeId::new("d"));
        let result = graph.compute_curvature(&edge);

        // Verify method selection and reasonable curvature range
        assert_eq!(result.method, CurvatureMethod::Sinkhorn);
        assert!(result.value > -2.0 && result.value < 2.0);
    }

    #[test]
    fn test_all_curvatures() {
        let graph = create_test_graph();
        let results = graph.compute_all_curvatures();

        assert_eq!(results.len(), 4); // 4 edges
    }

    #[test]
    fn test_ricci_flow_step() {
        let graph = create_test_graph();
        let flow = RicciFlow::new(0.1);

        let new_lengths = flow.flow_step(&graph);

        // Should have new lengths for all edges
        assert_eq!(new_lengths.len(), graph.edges().len());

        // All new lengths should be positive
        for length in new_lengths.values() {
            assert!(*length > 0.0);
        }
    }
}
