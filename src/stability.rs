//! Dynamic Stability Module
//!
//! Implements proximal regularization to suppress coordinate oscillation
//! during topology changes and Ricci flow updates.

use crate::coordinates::RoutingCoordinate;
use crate::PoincareDiskPoint;
use std::collections::HashMap;

/// Proximal regularization controller
///
/// Adds a penalty term to prevent coordinates from drifting too far
/// from their previous positions:
///
/// min Σ (d_H(z_u, z_v) - ℓ_uv)² + λ Σ d_H(z_u, z_u^prev)²
pub struct ProximalRegularizer {
    /// Regularization weight λ
    pub lambda: f64,
    /// Previous coordinates for each node
    previous_coords: HashMap<String, PoincareDiskPoint>,
    /// Maximum allowed coordinate drift per update
    pub max_drift: f64,
}

impl ProximalRegularizer {
    pub fn new(lambda: f64, max_drift: f64) -> Self {
        Self {
            lambda,
            previous_coords: HashMap::new(),
            max_drift,
        }
    }

    /// Store current coordinates as previous (called before update)
    pub fn snapshot(&mut self, node_id: &str, coord: &RoutingCoordinate) {
        self.previous_coords.insert(node_id.to_string(), coord.point);
    }

    /// Get previous coordinate for a node
    pub fn get_previous(&self, node_id: &str) -> Option<&PoincareDiskPoint> {
        self.previous_coords.get(node_id)
    }

    /// Compute regularization penalty for a proposed coordinate update
    pub fn penalty(&self, node_id: &str, proposed: &PoincareDiskPoint) -> f64 {
        match self.previous_coords.get(node_id) {
            Some(prev) => {
                let drift = prev.hyperbolic_distance(proposed);
                self.lambda * drift * drift
            }
            None => 0.0, // No penalty for first update
        }
    }

    /// Apply proximal regularization to a proposed coordinate update
    ///
    /// Returns the regularized coordinate that balances:
    /// - Moving toward the target (from Ricci flow)
    /// - Staying close to previous position (stability)
    pub fn regularize(
        &self,
        node_id: &str,
        current: &PoincareDiskPoint,
        target: &PoincareDiskPoint,
    ) -> PoincareDiskPoint {
        let prev = match self.previous_coords.get(node_id) {
            Some(p) => p,
            None => return *target, // No regularization for first update
        };

        // Compute drift from previous
        let drift = prev.hyperbolic_distance(target);

        if drift <= self.max_drift {
            // Within allowed drift - accept target
            *target
        } else {
            // Limit drift by interpolating between prev and target
            let t = self.max_drift / drift;
            
            // Linear interpolation in Euclidean coordinates
            // (This is an approximation - true geodesic interpolation would be more accurate)
            let new_x = prev.x * (1.0 - t) + target.x * t;
            let new_y = prev.y * (1.0 - t) + target.y * t;

            // Ensure we stay inside the disk
            let norm_sq = new_x * new_x + new_y * new_y;
            if norm_sq >= 1.0 {
                // Scale back to keep inside disk
                let scale = 0.99 / norm_sq.sqrt();
                PoincareDiskPoint::new(new_x * scale, new_y * scale)
                    .unwrap_or(*current)
            } else {
                PoincareDiskPoint::new(new_x, new_y).unwrap_or(*current)
            }
        }
    }

    /// Compute objective function for coordinate optimization
    ///
    /// J(z) = Σ (d_H(z_u, z_v) - ℓ_uv)² + λ Σ d_H(z_u, z_u^prev)²
    pub fn objective_value(
        &self,
        coords: &HashMap<String, PoincareDiskPoint>,
        edges: &[(String, String, f64)], // (u, v, target_length)
    ) -> f64 {
        // Distance matching term
        let distance_term: f64 = edges
            .iter()
            .filter_map(|(u, v, target_len)| {
                let coord_u = coords.get(u)?;
                let coord_v = coords.get(v)?;
                let actual_dist = coord_u.hyperbolic_distance(coord_v);
                Some((actual_dist - target_len).powi(2))
            })
            .sum();

        // Proximal regularization term
        let proximal_term: f64 = coords
            .iter()
            .filter_map(|(node_id, coord)| {
                let prev = self.previous_coords.get(node_id)?;
                let drift = prev.hyperbolic_distance(coord);
                Some(drift * drift)
            })
            .sum();

        distance_term + self.lambda * proximal_term
    }

    /// Check if coordinates have converged (drift is small)
    pub fn has_converged(&self, coords: &HashMap<String, PoincareDiskPoint>, threshold: f64) -> bool {
        for (node_id, coord) in coords {
            if let Some(prev) = self.previous_coords.get(node_id) {
                if prev.hyperbolic_distance(coord) > threshold {
                    return false;
                }
            }
        }
        true
    }

    /// Update all previous coordinates from current
    pub fn update_all(&mut self, coords: &HashMap<String, PoincareDiskPoint>) {
        for (node_id, coord) in coords {
            self.previous_coords.insert(node_id.clone(), *coord);
        }
    }

    /// Clear stored previous coordinates
    pub fn clear(&mut self) {
        self.previous_coords.clear();
    }
}

impl Default for ProximalRegularizer {
    fn default() -> Self {
        Self::new(0.5, 0.1)
    }
}

/// Coordinate drift tracker for stability analysis
pub struct DriftTracker {
    /// History of coordinate positions
    history: HashMap<String, Vec<PoincareDiskPoint>>,
    /// Maximum history length
    max_history: usize,
}

impl DriftTracker {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: HashMap::new(),
            max_history,
        }
    }

    /// Record a coordinate position
    pub fn record(&mut self, node_id: &str, coord: PoincareDiskPoint) {
        let entry = self.history.entry(node_id.to_string()).or_insert_with(Vec::new);
        entry.push(coord);
        
        // Trim history if too long
        if entry.len() > self.max_history {
            entry.remove(0);
        }
    }

    /// Compute average drift rate for a node
    pub fn average_drift(&self, node_id: &str) -> Option<f64> {
        let history = self.history.get(node_id)?;
        if history.len() < 2 {
            return None;
        }

        let mut total_drift = 0.0;
        for i in 1..history.len() {
            total_drift += history[i - 1].hyperbolic_distance(&history[i]);
        }

        Some(total_drift / (history.len() - 1) as f64)
    }

    /// Compute maximum drift for a node
    pub fn max_drift(&self, node_id: &str) -> Option<f64> {
        let history = self.history.get(node_id)?;
        if history.len() < 2 {
            return None;
        }

        let mut max = 0.0f64;
        for i in 1..history.len() {
            let drift = history[i - 1].hyperbolic_distance(&history[i]);
            max = f64::max(max, drift);
        }

        Some(max)
    }

    /// Check if coordinates are oscillating (high variance in drift)
    pub fn is_oscillating(&self, node_id: &str, threshold: f64) -> bool {
        let history = match self.history.get(node_id) {
            Some(h) if h.len() >= 3 => h,
            _ => return false,
        };

        // Compute drifts
        let drifts: Vec<f64> = (1..history.len())
            .map(|i| history[i - 1].hyperbolic_distance(&history[i]))
            .collect();

        // Compute variance
        let mean = drifts.iter().sum::<f64>() / drifts.len() as f64;
        let variance = drifts.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / drifts.len() as f64;

        variance.sqrt() > threshold
    }

    /// Get statistics for all nodes
    pub fn get_stats(&self) -> StabilityStats {
        let mut total_avg_drift = 0.0;
        let mut total_max_drift = 0.0;
        let mut oscillating_count = 0;
        let mut node_count = 0;

        for node_id in self.history.keys() {
            if let Some(avg) = self.average_drift(node_id) {
                total_avg_drift += avg;
                node_count += 1;
            }
            if let Some(max) = self.max_drift(node_id) {
                total_max_drift = f64::max(total_max_drift, max);
            }
            if self.is_oscillating(node_id, 0.01) {
                oscillating_count += 1;
            }
        }

        StabilityStats {
            average_drift: if node_count > 0 {
                total_avg_drift / node_count as f64
            } else {
                0.0
            },
            max_drift: total_max_drift,
            oscillating_nodes: oscillating_count,
            total_nodes: self.history.len(),
        }
    }
}

/// Statistics about coordinate stability
#[derive(Debug, Clone)]
pub struct StabilityStats {
    pub average_drift: f64,
    pub max_drift: f64,
    pub oscillating_nodes: usize,
    pub total_nodes: usize,
}

impl std::fmt::Display for StabilityStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Stability: avg_drift={:.4}, max_drift={:.4}, oscillating={}/{}",
            self.average_drift, self.max_drift, self.oscillating_nodes, self.total_nodes
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proximal_regularizer() {
        let mut reg = ProximalRegularizer::new(0.5, 0.1);

        let node_id = "test";
        let initial = PoincareDiskPoint::new(0.0, 0.0).unwrap();
        let coord = RoutingCoordinate::new(initial, 0);
        
        reg.snapshot(node_id, &coord);

        // Small move - should be accepted
        let small_move = PoincareDiskPoint::new(0.05, 0.0).unwrap();
        let result = reg.regularize(node_id, &initial, &small_move);
        assert!((result.x - small_move.x).abs() < 0.01);

        // Large move - should be limited
        let large_move = PoincareDiskPoint::new(0.5, 0.0).unwrap();
        let result = reg.regularize(node_id, &initial, &large_move);
        let drift = initial.hyperbolic_distance(&result);
        assert!(drift <= reg.max_drift + 0.01);
    }

    #[test]
    fn test_drift_tracker() {
        let mut tracker = DriftTracker::new(10);

        // Record some positions
        tracker.record("node1", PoincareDiskPoint::new(0.0, 0.0).unwrap());
        tracker.record("node1", PoincareDiskPoint::new(0.1, 0.0).unwrap());
        tracker.record("node1", PoincareDiskPoint::new(0.15, 0.0).unwrap());

        let avg = tracker.average_drift("node1");
        assert!(avg.is_some());
        assert!(avg.unwrap() > 0.0);
    }

    #[test]
    fn test_stability_stats() {
        let mut tracker = DriftTracker::new(10);

        // Add stable node
        tracker.record("stable", PoincareDiskPoint::new(0.0, 0.0).unwrap());
        tracker.record("stable", PoincareDiskPoint::new(0.001, 0.0).unwrap());

        // Add drifting node
        tracker.record("drifting", PoincareDiskPoint::new(0.0, 0.0).unwrap());
        tracker.record("drifting", PoincareDiskPoint::new(0.2, 0.0).unwrap());

        let stats = tracker.get_stats();
        assert_eq!(stats.total_nodes, 2);
        assert!(stats.max_drift > 0.0);
    }
}
