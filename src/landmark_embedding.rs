//! Landmark-MDS Hyperbolic Embedding
//!
//! Implements graph-distance-aware embedding into the Poincaré disk.
//! Unlike PIE which only considers tree structure, this approach uses
//! actual graph shortest-path distances via landmark sampling.
//!
//! Algorithm:
//! 1. Select k landmarks using farthest-point sampling
//! 2. Compute all-pairs distances from landmarks using BFS
//! 3. Use classical MDS to position landmarks in Euclidean space
//! 4. Map to Poincaré disk and refine via hyperbolic stress minimization
//! 5. Triangulate remaining nodes using landmark distances

use crate::coordinates::{NodeId, RoutingCoordinate};
use crate::PoincareDiskPoint;
use std::collections::{HashMap, HashSet, VecDeque};

/// Configuration for Landmark-MDS embedding
#[derive(Debug, Clone)]
pub struct LandmarkConfig {
    /// Number of landmarks to select (default: min(2*√n, 64))
    pub num_landmarks: Option<usize>,
    /// Number of stress minimization iterations for landmarks
    pub landmark_iterations: usize,
    /// Number of iterations for triangulating each node
    pub triangulation_iterations: usize,
    /// Step size for gradient descent
    pub step_size: f64,
    /// Regularization to prevent boundary collapse
    pub boundary_margin: f64,
}

impl Default for LandmarkConfig {
    fn default() -> Self {
        Self {
            num_landmarks: None, // Will be computed as min(2*√n, 64)
            landmark_iterations: 100,
            triangulation_iterations: 50,
            step_size: 0.1,
            boundary_margin: 0.02,
        }
    }
}

/// Result of landmark embedding
#[derive(Debug, Clone)]
pub struct LandmarkEmbeddingResult {
    /// Coordinates for each node in the Poincaré disk
    pub coordinates: HashMap<NodeId, PoincareDiskPoint>,
    /// Selected landmark node IDs
    pub landmarks: Vec<NodeId>,
    /// Covering radius (max distance from any node to nearest landmark)
    pub covering_radius: u32,
    /// Distance matrix from each node to each landmark
    pub landmark_distances: HashMap<NodeId, Vec<u32>>,
}

/// Landmark-MDS Hyperbolic Embedding
pub struct LandmarkEmbedding {
    config: LandmarkConfig,
}

impl LandmarkEmbedding {
    pub fn new() -> Self {
        Self {
            config: LandmarkConfig::default(),
        }
    }

    pub fn with_config(config: LandmarkConfig) -> Self {
        Self { config }
    }

    /// Compute the optimal number of landmarks based on network size
    fn compute_num_landmarks(&self, n: usize) -> usize {
        match self.config.num_landmarks {
            Some(k) => k.min(n),
            None => {
                let sqrt_n = (n as f64).sqrt() as usize;
                (2 * sqrt_n).min(64).max(4).min(n)
            }
        }
    }

    /// Select landmarks using farthest-point sampling
    /// This maximizes the minimum distance from any node to its nearest landmark
    fn select_landmarks(
        &self,
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
        num_landmarks: usize,
    ) -> Vec<NodeId> {
        let nodes: Vec<&NodeId> = adjacency.keys().collect();
        if nodes.is_empty() || num_landmarks == 0 {
            return Vec::new();
        }

        let mut landmarks = Vec::with_capacity(num_landmarks);
        
        // Start with highest-degree node (hub)
        let first_landmark = adjacency
            .iter()
            .max_by_key(|(_, neighbors)| neighbors.len())
            .map(|(id, _)| id.clone())
            .unwrap();
        landmarks.push(first_landmark.clone());

        // Distance from each node to nearest landmark
        let mut min_distances: HashMap<NodeId, u32> = HashMap::new();
        
        // Initialize with distances from first landmark
        let distances_from_first = self.bfs_distances(adjacency, &first_landmark);
        for (node, dist) in distances_from_first {
            min_distances.insert(node, dist);
        }

        // Greedily select remaining landmarks
        while landmarks.len() < num_landmarks {
            // Find node with maximum distance to nearest landmark
            let next_landmark = min_distances
                .iter()
                .filter(|(id, _)| !landmarks.contains(id))
                .max_by_key(|(_, &dist)| dist)
                .map(|(id, _)| id.clone());

            match next_landmark {
                Some(landmark) => {
                    // Update min_distances with new landmark
                    let new_distances = self.bfs_distances(adjacency, &landmark);
                    for (node, dist) in new_distances {
                        let current = min_distances.entry(node).or_insert(u32::MAX);
                        *current = (*current).min(dist);
                    }
                    landmarks.push(landmark);
                }
                None => break,
            }
        }

        landmarks
    }

    /// BFS to compute shortest-path distances from a source node
    fn bfs_distances(
        &self,
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
        source: &NodeId,
    ) -> HashMap<NodeId, u32> {
        let mut distances = HashMap::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        distances.insert(source.clone(), 0);
        visited.insert(source.clone());
        queue.push_back((source.clone(), 0));

        while let Some((current, dist)) = queue.pop_front() {
            if let Some(neighbors) = adjacency.get(&current) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        visited.insert(neighbor.clone());
                        distances.insert(neighbor.clone(), dist + 1);
                        queue.push_back((neighbor.clone(), dist + 1));
                    }
                }
            }
        }

        distances
    }

    /// Compute all landmark distances for all nodes
    fn compute_all_landmark_distances(
        &self,
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
        landmarks: &[NodeId],
    ) -> HashMap<NodeId, Vec<u32>> {
        let mut all_distances: HashMap<NodeId, Vec<u32>> = HashMap::new();
        
        // Initialize distance vectors for all nodes
        for node in adjacency.keys() {
            all_distances.insert(node.clone(), vec![u32::MAX; landmarks.len()]);
        }

        // Compute distances from each landmark
        for (i, landmark) in landmarks.iter().enumerate() {
            let distances = self.bfs_distances(adjacency, landmark);
            for (node, dist) in distances {
                if let Some(vec) = all_distances.get_mut(&node) {
                    vec[i] = dist;
                }
            }
        }

        all_distances
    }

    /// Classical MDS to get initial Euclidean coordinates for landmarks
    /// Returns 2D coordinates for each landmark
    fn classical_mds(
        &self,
        landmarks: &[NodeId],
        landmark_distances: &HashMap<NodeId, Vec<u32>>,
    ) -> HashMap<NodeId, (f64, f64)> {
        let k = landmarks.len();
        if k < 2 {
            return landmarks
                .iter()
                .map(|l| (l.clone(), (0.0, 0.0)))
                .collect();
        }

        // Build distance matrix between landmarks
        let mut dist_matrix = vec![vec![0.0; k]; k];
        for (i, li) in landmarks.iter().enumerate() {
            if let Some(dists) = landmark_distances.get(li) {
                for (j, _lj) in landmarks.iter().enumerate() {
                    // Distance from landmark i to landmark j
                    // = distance from li to lj's position = dists[j]
                    dist_matrix[i][j] = dists[j] as f64;
                }
            }
        }

        // Symmetrize (in case of any numerical issues)
        for i in 0..k {
            for j in (i + 1)..k {
                let avg = (dist_matrix[i][j] + dist_matrix[j][i]) / 2.0;
                dist_matrix[i][j] = avg;
                dist_matrix[j][i] = avg;
            }
        }

        // Double centering: B = -1/2 * J * D² * J where J = I - 1/k * 11^T
        // First compute D²
        let mut d_squared = vec![vec![0.0; k]; k];
        for i in 0..k {
            for j in 0..k {
                d_squared[i][j] = dist_matrix[i][j] * dist_matrix[i][j];
            }
        }

        // Row and column means
        let mut row_means = vec![0.0; k];
        let mut col_means = vec![0.0; k];
        let mut grand_mean = 0.0;

        for i in 0..k {
            for j in 0..k {
                row_means[i] += d_squared[i][j];
                col_means[j] += d_squared[i][j];
                grand_mean += d_squared[i][j];
            }
        }
        for i in 0..k {
            row_means[i] /= k as f64;
            col_means[i] /= k as f64;
        }
        grand_mean /= (k * k) as f64;

        // B_ij = -1/2 * (D²_ij - row_mean_i - col_mean_j + grand_mean)
        let mut b = vec![vec![0.0; k]; k];
        for i in 0..k {
            for j in 0..k {
                b[i][j] = -0.5 * (d_squared[i][j] - row_means[i] - col_means[j] + grand_mean);
            }
        }

        // Power iteration to find top 2 eigenvectors
        let (coords_x, coords_y) = self.power_iteration_2d(&b);

        // Scale coordinates to fit in unit disk with margin
        let max_coord = coords_x
            .iter()
            .chain(coords_y.iter())
            .map(|x| x.abs())
            .fold(0.0f64, f64::max);
        
        let scale = if max_coord > 1e-10 {
            (1.0 - self.config.boundary_margin * 2.0) / max_coord
        } else {
            1.0
        };

        landmarks
            .iter()
            .enumerate()
            .map(|(i, l)| (l.clone(), (coords_x[i] * scale, coords_y[i] * scale)))
            .collect()
    }

    /// Power iteration to find top 2 eigenvectors of a symmetric matrix
    fn power_iteration_2d(&self, matrix: &[Vec<f64>]) -> (Vec<f64>, Vec<f64>) {
        let n = matrix.len();
        if n == 0 {
            return (vec![], vec![]);
        }

        let mut v1 = vec![1.0 / (n as f64).sqrt(); n];
        let mut v2 = vec![0.0; n];
        
        // Initialize v2 orthogonal to v1
        for i in 0..n {
            v2[i] = if i % 2 == 0 { 1.0 } else { -1.0 };
        }
        let norm2: f64 = v2.iter().map(|x| x * x).sum::<f64>().sqrt();
        for x in v2.iter_mut() {
            *x /= norm2;
        }

        // Power iteration for first eigenvector
        for _ in 0..100 {
            let mut new_v = vec![0.0; n];
            for i in 0..n {
                for j in 0..n {
                    new_v[i] += matrix[i][j] * v1[j];
                }
            }
            let norm: f64 = new_v.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 1e-10 {
                for x in new_v.iter_mut() {
                    *x /= norm;
                }
            }
            v1 = new_v;
        }

        // Deflate matrix and find second eigenvector
        let lambda1: f64 = (0..n)
            .map(|i| {
                let mv: f64 = (0..n).map(|j| matrix[i][j] * v1[j]).sum();
                mv * v1[i]
            })
            .sum();

        for _ in 0..100 {
            let mut new_v = vec![0.0; n];
            for i in 0..n {
                for j in 0..n {
                    // Deflated matrix: A - lambda1 * v1 * v1^T
                    let deflated = matrix[i][j] - lambda1 * v1[i] * v1[j];
                    new_v[i] += deflated * v2[j];
                }
            }
            // Gram-Schmidt orthogonalization against v1
            let dot: f64 = (0..n).map(|i| new_v[i] * v1[i]).sum();
            for i in 0..n {
                new_v[i] -= dot * v1[i];
            }
            let norm: f64 = new_v.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 1e-10 {
                for x in new_v.iter_mut() {
                    *x /= norm;
                }
            }
            v2 = new_v;
        }

        // Scale by eigenvalues (sqrt of eigenvalue for coordinates)
        let lambda2: f64 = (0..n)
            .map(|i| {
                let mv: f64 = (0..n).map(|j| matrix[i][j] * v2[j]).sum();
                mv * v2[i]
            })
            .sum();

        let scale1 = lambda1.max(0.0).sqrt();
        let scale2 = lambda2.max(0.0).sqrt();

        let coords_x: Vec<f64> = v1.iter().map(|x| x * scale1).collect();
        let coords_y: Vec<f64> = v2.iter().map(|x| x * scale2).collect();

        (coords_x, coords_y)
    }

    /// Refine landmark coordinates using hyperbolic stress minimization
    fn refine_landmark_coords(
        &self,
        landmarks: &[NodeId],
        init_coords: &HashMap<NodeId, (f64, f64)>,
        landmark_distances: &HashMap<NodeId, Vec<u32>>,
    ) -> HashMap<NodeId, (f64, f64)> {
        let mut coords = init_coords.clone();
        let k = landmarks.len();

        for _ in 0..self.config.landmark_iterations {
            let mut gradients: HashMap<NodeId, (f64, f64)> = HashMap::new();
            for l in landmarks {
                gradients.insert(l.clone(), (0.0, 0.0));
            }

            // Compute stress gradients between all landmark pairs
            for i in 0..k {
                for j in (i + 1)..k {
                    let li = &landmarks[i];
                    let lj = &landmarks[j];

                    let (xi, yi) = coords.get(li).copied().unwrap_or((0.0, 0.0));
                    let (xj, yj) = coords.get(lj).copied().unwrap_or((0.0, 0.0));

                    // Target distance (graph distance)
                    let target_dist = landmark_distances
                        .get(li)
                        .map(|d| d[j] as f64)
                        .unwrap_or(1.0);

                    // Current hyperbolic distance
                    let point_i = PoincareDiskPoint::new(xi, yi);
                    let point_j = PoincareDiskPoint::new(xj, yj);

                    let current_dist = match (point_i, point_j) {
                        (Some(pi), Some(pj)) => pi.hyperbolic_distance(&pj),
                        _ => continue,
                    };

                    if current_dist < 1e-10 {
                        continue;
                    }

                    // Stress and gradient
                    let stress = current_dist - target_dist;
                    let dx = xj - xi;
                    let dy = yj - yi;
                    let eucl_dist = (dx * dx + dy * dy).sqrt().max(1e-10);

                    // Conformal factors
                    let ri_sq = xi * xi + yi * yi;
                    let rj_sq = xj * xj + yj * yj;
                    let conf_i = 2.0 / (1.0 - ri_sq).max(0.01);
                    let conf_j = 2.0 / (1.0 - rj_sq).max(0.01);

                    let grad_mag = stress * conf_i * conf_j / eucl_dist * self.config.step_size;
                    let gx = dx / eucl_dist * grad_mag;
                    let gy = dy / eucl_dist * grad_mag;

                    if let Some(g) = gradients.get_mut(li) {
                        g.0 += gx;
                        g.1 += gy;
                    }
                    if let Some(g) = gradients.get_mut(lj) {
                        g.0 -= gx;
                        g.1 -= gy;
                    }
                }
            }

            // Apply gradients with Riemannian metric
            for (id, coord) in coords.iter_mut() {
                if let Some(&(gx, gy)) = gradients.get(id) {
                    let (x, y) = *coord;
                    let r_sq = x * x + y * y;
                    let metric_scale = ((1.0 - r_sq) * (1.0 - r_sq)) / 4.0;
                    let metric_scale = metric_scale.max(0.001);

                    let new_x = x - gx * metric_scale;
                    let new_y = y - gy * metric_scale;

                    // Project back into disk with margin
                    let new_r_sq = new_x * new_x + new_y * new_y;
                    let max_r = 1.0 - self.config.boundary_margin;
                    if new_r_sq >= max_r * max_r {
                        let scale = (max_r - 0.01) / new_r_sq.sqrt();
                        coord.0 = new_x * scale;
                        coord.1 = new_y * scale;
                    } else {
                        coord.0 = new_x;
                        coord.1 = new_y;
                    }
                }
            }
        }

        coords
    }

    /// Triangulate a single node using its distances to landmarks
    fn triangulate_node(
        &self,
        node_distances: &[u32],
        landmark_coords: &[(f64, f64)],
    ) -> (f64, f64) {
        if landmark_coords.is_empty() {
            return (0.0, 0.0);
        }

        // Start from centroid of landmarks weighted by inverse distance
        let mut x = 0.0;
        let mut y = 0.0;
        let mut total_weight = 0.0;

        for (i, &(lx, ly)) in landmark_coords.iter().enumerate() {
            let dist = node_distances.get(i).copied().unwrap_or(u32::MAX);
            if dist == 0 {
                // Node is at this landmark
                return (lx, ly);
            }
            let weight = 1.0 / (dist as f64 + 1.0);
            x += lx * weight;
            y += ly * weight;
            total_weight += weight;
        }

        if total_weight > 1e-10 {
            x /= total_weight;
            y /= total_weight;
        }

        // Gradient descent to minimize triangulation stress
        for _ in 0..self.config.triangulation_iterations {
            let mut gx = 0.0;
            let mut gy = 0.0;

            let point = match PoincareDiskPoint::new(x, y) {
                Some(p) => p,
                None => break,
            };

            for (i, &(lx, ly)) in landmark_coords.iter().enumerate() {
                let target_dist = node_distances.get(i).copied().unwrap_or(1) as f64;
                
                let landmark_point = match PoincareDiskPoint::new(lx, ly) {
                    Some(p) => p,
                    None => continue,
                };

                let current_dist = point.hyperbolic_distance(&landmark_point);
                if current_dist < 1e-10 {
                    continue;
                }

                let stress = current_dist - target_dist;
                let dx = lx - x;
                let dy = ly - y;
                let eucl_dist = (dx * dx + dy * dy).sqrt().max(1e-10);

                // Weight by inverse target distance (closer landmarks matter more)
                let weight = 1.0 / (target_dist + 1.0);
                let grad_mag = stress * weight * self.config.step_size * 0.5;

                gx += dx / eucl_dist * grad_mag;
                gy += dy / eucl_dist * grad_mag;
            }

            // Apply gradient with Riemannian metric
            let r_sq = x * x + y * y;
            let metric_scale = ((1.0 - r_sq) * (1.0 - r_sq)) / 4.0;
            let metric_scale = metric_scale.max(0.001);

            x += gx * metric_scale;
            y += gy * metric_scale;

            // Project back into disk
            let new_r_sq = x * x + y * y;
            let max_r = 1.0 - self.config.boundary_margin;
            if new_r_sq >= max_r * max_r {
                let scale = (max_r - 0.01) / new_r_sq.sqrt();
                x *= scale;
                y *= scale;
            }
        }

        (x, y)
    }

    /// Main embedding function
    pub fn embed(
        &self,
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
    ) -> Result<LandmarkEmbeddingResult, String> {
        let n = adjacency.len();
        if n == 0 {
            return Err("Empty graph".to_string());
        }

        // 1. Select landmarks
        let num_landmarks = self.compute_num_landmarks(n);
        let landmarks = self.select_landmarks(adjacency, num_landmarks);
        
        if landmarks.is_empty() {
            return Err("No landmarks selected".to_string());
        }

        // 2. Compute all landmark distances
        let landmark_distances = self.compute_all_landmark_distances(adjacency, &landmarks);

        // 3. Compute covering radius
        let covering_radius = landmark_distances
            .values()
            .map(|dists| dists.iter().copied().min().unwrap_or(u32::MAX))
            .max()
            .unwrap_or(0);

        // 4. Classical MDS for initial landmark positions
        let init_coords = self.classical_mds(&landmarks, &landmark_distances);

        // 5. Refine landmark coordinates in hyperbolic space
        let landmark_coords = self.refine_landmark_coords(&landmarks, &init_coords, &landmark_distances);

        // 6. Convert landmark coords to vector for triangulation
        let landmark_coord_vec: Vec<(f64, f64)> = landmarks
            .iter()
            .map(|l| landmark_coords.get(l).copied().unwrap_or((0.0, 0.0)))
            .collect();

        // 7. Triangulate all nodes
        let mut coordinates: HashMap<NodeId, PoincareDiskPoint> = HashMap::new();
        
        for (node_id, dists) in &landmark_distances {
            let (x, y) = if landmarks.contains(node_id) {
                landmark_coords.get(node_id).copied().unwrap_or((0.0, 0.0))
            } else {
                self.triangulate_node(dists, &landmark_coord_vec)
            };

            let point = PoincareDiskPoint::new(x, y)
                .unwrap_or_else(|| PoincareDiskPoint::origin());
            coordinates.insert(node_id.clone(), point);
        }

        Ok(LandmarkEmbeddingResult {
            coordinates,
            landmarks,
            covering_radius,
            landmark_distances,
        })
    }

    /// Embed and return as RoutingCoordinates
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
}

impl Default for LandmarkEmbedding {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_adjacency() -> HashMap<NodeId, Vec<NodeId>> {
        // Simple graph: 0 -- 1 -- 2 -- 3 -- 4
        let mut adj = HashMap::new();
        adj.insert(NodeId::new("0"), vec![NodeId::new("1")]);
        adj.insert(NodeId::new("1"), vec![NodeId::new("0"), NodeId::new("2")]);
        adj.insert(NodeId::new("2"), vec![NodeId::new("1"), NodeId::new("3")]);
        adj.insert(NodeId::new("3"), vec![NodeId::new("2"), NodeId::new("4")]);
        adj.insert(NodeId::new("4"), vec![NodeId::new("3")]);
        adj
    }

    #[test]
    fn test_landmark_selection() {
        let adj = create_test_adjacency();
        let embedder = LandmarkEmbedding::new();
        let landmarks = embedder.select_landmarks(&adj, 2);
        
        assert_eq!(landmarks.len(), 2);
        // Should select endpoints (0 and 4) as they maximize coverage
    }

    #[test]
    fn test_bfs_distances() {
        let adj = create_test_adjacency();
        let embedder = LandmarkEmbedding::new();
        let distances = embedder.bfs_distances(&adj, &NodeId::new("0"));
        
        assert_eq!(distances.get(&NodeId::new("0")), Some(&0));
        assert_eq!(distances.get(&NodeId::new("1")), Some(&1));
        assert_eq!(distances.get(&NodeId::new("4")), Some(&4));
    }

    #[test]
    fn test_embedding_creates_valid_coordinates() {
        let adj = create_test_adjacency();
        let embedder = LandmarkEmbedding::new();
        let result = embedder.embed(&adj).unwrap();

        assert_eq!(result.coordinates.len(), 5);
        for (_, point) in &result.coordinates {
            assert!(point.euclidean_norm() < 1.0);
        }
    }

    #[test]
    fn test_embedding_with_cycle() {
        // Triangle graph
        let mut adj = HashMap::new();
        adj.insert(NodeId::new("0"), vec![NodeId::new("1"), NodeId::new("2")]);
        adj.insert(NodeId::new("1"), vec![NodeId::new("0"), NodeId::new("2")]);
        adj.insert(NodeId::new("2"), vec![NodeId::new("0"), NodeId::new("1")]);

        let embedder = LandmarkEmbedding::new();
        let result = embedder.embed(&adj).unwrap();

        assert_eq!(result.coordinates.len(), 3);
    }
}
