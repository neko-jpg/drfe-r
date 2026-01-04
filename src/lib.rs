//! DRFE-R: Distributed Ricci Flow Embedding with Rendezvous Mechanism
//!
//! Core library for hyperbolic geometry operations and distributed routing protocol.

pub mod api;
pub mod audit;
pub mod baselines;
pub mod byzantine;
pub mod chat;
pub mod chaos;
pub mod coordinates;
pub mod greedy_embedding;
pub mod grpc;
pub mod hierarchical;
pub mod hyperbolic_models;
pub mod landmark_embedding;
pub mod lockfree;
pub mod network;
pub mod network_tls;
pub mod rendezvous;
pub mod ricci;
pub mod routing;
pub mod stability;
pub mod sybil;
pub mod telemetry;
pub mod tls;
pub mod tz_routing;


use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// A point in the Poincaré disk model of hyperbolic space.
/// The disk is the unit disk {z ∈ ℂ : |z| < 1}.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PoincareDiskPoint {
    /// x coordinate (real part)
    pub x: f64,
    /// y coordinate (imaginary part)
    pub y: f64,
}

impl PoincareDiskPoint {
    /// Create a new point in the Poincaré disk.
    /// Returns None if the point is outside the open unit disk.
    pub fn new(x: f64, y: f64) -> Option<Self> {
        let r_sq = x * x + y * y;
        if r_sq >= 1.0 {
            None
        } else {
            Some(Self { x, y })
        }
    }

    /// Create a point from polar coordinates (r, θ).
    /// r must be in [0, 1).
    pub fn from_polar(r: f64, theta: f64) -> Option<Self> {
        if r < 0.0 || r >= 1.0 {
            return None;
        }
        Some(Self {
            x: r * theta.cos(),
            y: r * theta.sin(),
        })
    }

    /// Origin of the Poincaré disk
    pub fn origin() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Euclidean distance from origin (|z|)
    pub fn euclidean_norm(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Euclidean distance squared from origin (|z|²)
    pub fn euclidean_norm_sq(&self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    /// Polar angle θ
    pub fn angle(&self) -> f64 {
        self.y.atan2(self.x)
    }

    /// Hyperbolic distance between two points in the Poincaré disk.
    /// Formula: d_H(z1, z2) = arcosh(1 + 2|z1 - z2|² / ((1 - |z1|²)(1 - |z2|²)))
    pub fn hyperbolic_distance(&self, other: &Self) -> f64 {
        let diff_x = self.x - other.x;
        let diff_y = self.y - other.y;
        let diff_sq = diff_x * diff_x + diff_y * diff_y;

        let norm1_sq = self.euclidean_norm_sq();
        let norm2_sq = other.euclidean_norm_sq();

        let denom = (1.0 - norm1_sq) * (1.0 - norm2_sq);

        // Avoid numerical issues
        if denom <= 0.0 {
            return f64::INFINITY;
        }

        let arg = 1.0 + 2.0 * diff_sq / denom;

        // arcosh(x) = ln(x + sqrt(x² - 1))
        if arg < 1.0 {
            0.0
        } else {
            (arg + (arg * arg - 1.0).sqrt()).ln()
        }
    }

    /// Möbius addition: a ⊕ b in the Poincaré disk
    /// Used for coordinate transformations
    pub fn mobius_add(&self, other: &Self) -> Option<Self> {
        let a = num_complex::Complex64::new(self.x, self.y);
        let b = num_complex::Complex64::new(other.x, other.y);

        let a_conj = a.conj();
        let numerator = a + b;
        let denominator = 1.0 + a_conj * b;

        if denominator.norm() < 1e-10 {
            return None;
        }

        let result = numerator / denominator;

        Self::new(result.re, result.im)
    }

    /// Geodesic interpolation between two points.
    /// t = 0 returns self, t = 1 returns other.
    pub fn geodesic_interpolate(&self, other: &Self, t: f64) -> Option<Self> {
        if t < 0.0 || t > 1.0 {
            return None;
        }

        // Simple linear interpolation in the disk (not exact geodesic, but close for nearby points)
        let x = self.x * (1.0 - t) + other.x * t;
        let y = self.y * (1.0 - t) + other.y * t;

        Self::new(x, y)
    }
}

impl std::fmt::Display for PoincareDiskPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.4}, {:.4})", self.x, self.y)
    }
}

/// Convert angle to range [0, 2π)
pub fn normalize_angle(theta: f64) -> f64 {
    let mut result = theta % (2.0 * PI);
    if result < 0.0 {
        result += 2.0 * PI;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_creation() {
        assert!(PoincareDiskPoint::new(0.0, 0.0).is_some());
        assert!(PoincareDiskPoint::new(0.5, 0.5).is_some());
        assert!(PoincareDiskPoint::new(1.0, 0.0).is_none()); // On boundary
        assert!(PoincareDiskPoint::new(0.8, 0.8).is_none()); // Outside
    }

    #[test]
    fn test_hyperbolic_distance_origin() {
        let origin = PoincareDiskPoint::origin();
        let p = PoincareDiskPoint::new(0.5, 0.0).unwrap();
        let d = origin.hyperbolic_distance(&p);
        // d_H(0, r) = 2 * arctanh(r) = ln((1+r)/(1-r))
        let expected: f64 = ((1.0 + 0.5) / (1.0 - 0.5_f64)).ln();
        assert!((d - expected).abs() < 1e-10);
    }

    #[test]
    fn test_hyperbolic_distance_symmetry() {
        let p1 = PoincareDiskPoint::new(0.3, 0.2).unwrap();
        let p2 = PoincareDiskPoint::new(-0.1, 0.4).unwrap();
        let d1 = p1.hyperbolic_distance(&p2);
        let d2 = p2.hyperbolic_distance(&p1);
        assert!((d1 - d2).abs() < 1e-10);
    }

    #[test]
    fn test_hyperbolic_distance_self() {
        let p = PoincareDiskPoint::new(0.3, 0.4).unwrap();
        assert!(p.hyperbolic_distance(&p) < 1e-10);
    }

    #[test]
    fn test_polar_coordinates() {
        let p = PoincareDiskPoint::from_polar(0.5, PI / 4.0).unwrap();
        let expected_x = 0.5 * (PI / 4.0).cos();
        let expected_y = 0.5 * (PI / 4.0).sin();
        assert!((p.x - expected_x).abs() < 1e-10);
        assert!((p.y - expected_y).abs() < 1e-10);
    }

    #[test]
    fn test_mobius_addition_origin() {
        let origin = PoincareDiskPoint::origin();
        let p = PoincareDiskPoint::new(0.3, 0.4).unwrap();
        let result = origin.mobius_add(&p).unwrap();
        assert!((result.x - p.x).abs() < 1e-10);
        assert!((result.y - p.y).abs() < 1e-10);
    }
}
