//! Multiple Hyperbolic Geometry Models
//!
//! Provides numerical stability at the Poincaré disk boundary (r ≈ 1) by
//! supporting multiple models that can be switched based on computational needs:
//!
//! - **Poincaré Disk**: Conformal, good for visualization, unstable near boundary
//! - **Klein Disk**: Projective, straight lines are geodesics, faster computations
//! - **Hyperboloid (Lorentz)**: Most numerically stable, best for large distances
//! - **Upper Half Plane**: Alternative conformal model

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Trait for hyperbolic points in any model
pub trait HyperbolicPoint: Clone + Copy + std::fmt::Debug {
    /// Hyperbolic distance between two points
    fn distance(&self, other: &Self) -> f64;
    
    /// Convert to Poincaré disk representation
    fn to_poincare(&self) -> PoincareDisk;
    
    /// Create from Poincaré disk representation
    fn from_poincare(p: &PoincareDisk) -> Self;
}

/// Point in the Poincaré disk model
/// Domain: |z| < 1
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PoincareDisk {
    pub x: f64,
    pub y: f64,
}

impl PoincareDisk {
    pub fn new(x: f64, y: f64) -> Option<Self> {
        let r_sq = x * x + y * y;
        if r_sq >= 1.0 {
            None
        } else {
            Some(Self { x, y })
        }
    }

    pub fn origin() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn norm_sq(&self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    pub fn norm(&self) -> f64 {
        self.norm_sq().sqrt()
    }
}

impl HyperbolicPoint for PoincareDisk {
    fn distance(&self, other: &Self) -> f64 {
        let diff_x = self.x - other.x;
        let diff_y = self.y - other.y;
        let diff_sq = diff_x * diff_x + diff_y * diff_y;

        let norm1_sq = self.norm_sq();
        let norm2_sq = other.norm_sq();

        let denom = (1.0 - norm1_sq) * (1.0 - norm2_sq);
        if denom <= 0.0 {
            return f64::INFINITY;
        }

        let arg = 1.0 + 2.0 * diff_sq / denom;
        if arg < 1.0 {
            0.0
        } else {
            (arg + (arg * arg - 1.0).sqrt()).ln()
        }
    }

    fn to_poincare(&self) -> PoincareDisk {
        *self
    }

    fn from_poincare(p: &PoincareDisk) -> Self {
        *p
    }
}

/// Point in the Klein disk model (Beltrami-Klein)
/// Domain: |z| < 1
/// Advantage: Geodesics are straight lines (Euclidean)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KleinDisk {
    pub x: f64,
    pub y: f64,
}

impl KleinDisk {
    pub fn new(x: f64, y: f64) -> Option<Self> {
        let r_sq = x * x + y * y;
        if r_sq >= 1.0 {
            None
        } else {
            Some(Self { x, y })
        }
    }

    pub fn origin() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn norm_sq(&self) -> f64 {
        self.x * self.x + self.y * self.y
    }
}

impl HyperbolicPoint for KleinDisk {
    fn distance(&self, other: &Self) -> f64 {
        // Klein distance formula using cross-ratio
        let p1_sq = self.norm_sq();
        let p2_sq = other.norm_sq();
        let p1_p2 = self.x * other.x + self.y * other.y;

        // Cayley-Klein metric
        let num = 1.0 - p1_p2;
        let denom = ((1.0 - p1_sq) * (1.0 - p2_sq)).sqrt();

        if denom <= 1e-15 {
            return f64::INFINITY;
        }

        let cosh_d = num / denom;
        if cosh_d <= 1.0 {
            0.0
        } else {
            (cosh_d + (cosh_d * cosh_d - 1.0).sqrt()).ln()
        }
    }

    fn to_poincare(&self) -> PoincareDisk {
        // Klein to Poincaré: z_P = z_K / (1 + sqrt(1 - |z_K|²))
        let r_sq = self.norm_sq();
        if r_sq >= 1.0 - 1e-15 {
            // Near boundary, return boundary point
            let scale = 0.999 / r_sq.sqrt();
            return PoincareDisk { x: self.x * scale, y: self.y * scale };
        }

        let factor = 1.0 / (1.0 + (1.0 - r_sq).sqrt());
        PoincareDisk {
            x: self.x * factor,
            y: self.y * factor,
        }
    }

    fn from_poincare(p: &PoincareDisk) -> Self {
        // Poincaré to Klein: z_K = 2z_P / (1 + |z_P|²)
        let r_sq = p.norm_sq();
        let factor = 2.0 / (1.0 + r_sq);
        Self {
            x: p.x * factor,
            y: p.y * factor,
        }
    }
}

/// Point in the Hyperboloid (Lorentz) model
/// Domain: H² = {(t, x, y) ∈ ℝ³ : t² - x² - y² = 1, t > 0}
/// Advantage: Most numerically stable, especially for large distances
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Hyperboloid {
    /// Time-like coordinate (always > 1 for valid points)
    pub t: f64,
    pub x: f64,
    pub y: f64,
}

impl Hyperboloid {
    /// Create a new hyperboloid point (validates constraint)
    pub fn new(t: f64, x: f64, y: f64) -> Option<Self> {
        let constraint = t * t - x * x - y * y;
        // Allow small numerical error
        if (constraint - 1.0).abs() > 0.01 || t <= 0.0 {
            None
        } else {
            // Project onto hyperboloid
            let actual_t = (1.0 + x * x + y * y).sqrt();
            Some(Self { t: actual_t, x, y })
        }
    }

    /// Create from spatial coordinates, computing t
    pub fn from_spatial(x: f64, y: f64) -> Self {
        let t = (1.0 + x * x + y * y).sqrt();
        Self { t, x, y }
    }

    /// Origin on hyperboloid (t=1, x=0, y=0)
    pub fn origin() -> Self {
        Self { t: 1.0, x: 0.0, y: 0.0 }
    }

    /// Minkowski inner product: ⟨u, v⟩_M = t₁t₂ - x₁x₂ - y₁y₂
    pub fn minkowski_inner(&self, other: &Self) -> f64 {
        self.t * other.t - self.x * other.x - self.y * other.y
    }
}

impl HyperbolicPoint for Hyperboloid {
    fn distance(&self, other: &Self) -> f64 {
        // d(p, q) = arcosh(-⟨p, q⟩_M)
        let inner = self.minkowski_inner(other);
        // Note: for valid hyperboloid points, inner ≤ -1
        let cosh_d = -inner;
        if cosh_d <= 1.0 {
            0.0
        } else {
            (cosh_d + (cosh_d * cosh_d - 1.0).sqrt()).ln()
        }
    }

    fn to_poincare(&self) -> PoincareDisk {
        // Hyperboloid to Poincaré: (x, y) / (1 + t)
        let denom = 1.0 + self.t;
        if denom <= 1e-15 {
            return PoincareDisk::origin();
        }
        PoincareDisk {
            x: self.x / denom,
            y: self.y / denom,
        }
    }

    fn from_poincare(p: &PoincareDisk) -> Self {
        // Poincaré to Hyperboloid:
        // t = (1 + |z|²) / (1 - |z|²)
        // x = 2x / (1 - |z|²)
        // y = 2y / (1 - |z|²)
        let r_sq = p.norm_sq();
        let denom = 1.0 - r_sq;
        if denom <= 1e-15 {
            // Near boundary, use large t
            return Self { t: 1000.0, x: p.x * 500.0, y: p.y * 500.0 };
        }
        Self {
            t: (1.0 + r_sq) / denom,
            x: 2.0 * p.x / denom,
            y: 2.0 * p.y / denom,
        }
    }
}

/// Point in the Upper Half Plane model
/// Domain: {(x, y) ∈ ℝ² : y > 0}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UpperHalfPlane {
    pub x: f64,
    pub y: f64, // Must be positive
}

impl UpperHalfPlane {
    pub fn new(x: f64, y: f64) -> Option<Self> {
        if y <= 0.0 {
            None
        } else {
            Some(Self { x, y })
        }
    }

    /// Base point (0, 1)
    pub fn base() -> Self {
        Self { x: 0.0, y: 1.0 }
    }
}

impl HyperbolicPoint for UpperHalfPlane {
    fn distance(&self, other: &Self) -> f64 {
        // d(z₁, z₂) = arcosh(1 + |z₁ - z₂|² / (2y₁y₂))
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let diff_sq = dx * dx + dy * dy;

        let denom = 2.0 * self.y * other.y;
        if denom <= 1e-15 {
            return f64::INFINITY;
        }

        let arg = 1.0 + diff_sq / denom;
        if arg < 1.0 {
            0.0
        } else {
            (arg + (arg * arg - 1.0).sqrt()).ln()
        }
    }

    fn to_poincare(&self) -> PoincareDisk {
        // Upper half plane to Poincaré: w = (z - i) / (z + i)
        // where z = x + iy in upper half plane
        let num_re = self.x;
        let num_im = self.y - 1.0;
        let denom_re = self.x;
        let denom_im = self.y + 1.0;

        let denom_sq = denom_re * denom_re + denom_im * denom_im;
        if denom_sq <= 1e-15 {
            return PoincareDisk::origin();
        }

        let px = (num_re * denom_re + num_im * denom_im) / denom_sq;
        let py = (num_im * denom_re - num_re * denom_im) / denom_sq;

        // Ensure we're in the disk
        let r_sq = px * px + py * py;
        if r_sq >= 1.0 {
            let scale = 0.999 / r_sq.sqrt();
            PoincareDisk { x: px * scale, y: py * scale }
        } else {
            PoincareDisk { x: px, y: py }
        }
    }

    fn from_poincare(p: &PoincareDisk) -> Self {
        // Poincaré to Upper half plane: z = i(1 + w) / (1 - w)
        let one_minus_w_re = 1.0 - p.x;
        let one_minus_w_im = -p.y;
        let one_plus_w_re = 1.0 + p.x;
        let one_plus_w_im = p.y;

        let denom_sq = one_minus_w_re * one_minus_w_re + one_minus_w_im * one_minus_w_im;
        if denom_sq <= 1e-15 {
            return Self { x: 0.0, y: 1000.0 }; // Near boundary
        }

        // i * (1 + w) / (1 - w)
        // = (i * one_plus) / one_minus
        // Multiply by i: (a + bi) -> (-b + ai)
        let i_plus_re = -one_plus_w_im;
        let i_plus_im = one_plus_w_re;

        let x = (i_plus_re * one_minus_w_re + i_plus_im * one_minus_w_im) / denom_sq;
        let y = (i_plus_im * one_minus_w_re - i_plus_re * one_minus_w_im) / denom_sq;

        Self { x, y: y.max(0.001) }
    }
}

/// Enum for dynamic model selection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HyperbolicModel {
    Poincare,
    Klein,
    Hyperboloid,
    UpperHalfPlane,
}

/// Adaptive model selector based on numerical stability needs
pub struct AdaptiveModelSelector {
    /// Threshold for switching from Poincaré to Hyperboloid (r² value)
    pub boundary_threshold: f64,
    /// Current preferred model
    pub default_model: HyperbolicModel,
}

impl AdaptiveModelSelector {
    pub fn new() -> Self {
        Self {
            boundary_threshold: 0.95 * 0.95, // r > 0.95
            default_model: HyperbolicModel::Poincare,
        }
    }

    /// Select best model for computing distance between two Poincaré disk points
    pub fn select_for_distance(&self, p1: &PoincareDisk, p2: &PoincareDisk) -> HyperbolicModel {
        let r1_sq = p1.norm_sq();
        let r2_sq = p2.norm_sq();

        // If either point is near boundary, use Hyperboloid for stability
        if r1_sq > self.boundary_threshold || r2_sq > self.boundary_threshold {
            HyperbolicModel::Hyperboloid
        } else {
            self.default_model
        }
    }

    /// Compute distance using adaptive model selection
    pub fn compute_distance(&self, p1: &PoincareDisk, p2: &PoincareDisk) -> f64 {
        match self.select_for_distance(p1, p2) {
            HyperbolicModel::Poincare => p1.distance(p2),
            HyperbolicModel::Klein => {
                let k1 = KleinDisk::from_poincare(p1);
                let k2 = KleinDisk::from_poincare(p2);
                k1.distance(&k2)
            }
            HyperbolicModel::Hyperboloid => {
                let h1 = Hyperboloid::from_poincare(p1);
                let h2 = Hyperboloid::from_poincare(p2);
                h1.distance(&h2)
            }
            HyperbolicModel::UpperHalfPlane => {
                let u1 = UpperHalfPlane::from_poincare(p1);
                let u2 = UpperHalfPlane::from_poincare(p2);
                u1.distance(&u2)
            }
        }
    }
}

impl Default for AdaptiveModelSelector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poincare_to_klein_roundtrip() {
        let p = PoincareDisk::new(0.3, 0.4).unwrap();
        let k = KleinDisk::from_poincare(&p);
        let p2 = k.to_poincare();
        
        assert!((p.x - p2.x).abs() < 1e-10);
        assert!((p.y - p2.y).abs() < 1e-10);
    }

    #[test]
    fn test_poincare_to_hyperboloid_roundtrip() {
        let p = PoincareDisk::new(0.3, 0.4).unwrap();
        let h = Hyperboloid::from_poincare(&p);
        let p2 = h.to_poincare();
        
        assert!((p.x - p2.x).abs() < 1e-10);
        assert!((p.y - p2.y).abs() < 1e-10);
    }

    #[test]
    fn test_hyperboloid_constraint() {
        let h = Hyperboloid::from_spatial(0.5, 0.3);
        let constraint = h.t * h.t - h.x * h.x - h.y * h.y;
        assert!((constraint - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_distance_consistency() {
        let p1 = PoincareDisk::new(0.2, 0.1).unwrap();
        let p2 = PoincareDisk::new(0.4, 0.3).unwrap();

        let d_poincare = p1.distance(&p2);

        let h1 = Hyperboloid::from_poincare(&p1);
        let h2 = Hyperboloid::from_poincare(&p2);
        let d_hyperboloid = h1.distance(&h2);

        // Distances should match within numerical tolerance
        let diff = (d_poincare - d_hyperboloid).abs();
        assert!(diff < 0.05, "Distance mismatch: Poincare={}, Hyperboloid={}, Diff={}", d_poincare, d_hyperboloid, diff);
    }

    #[test]
    fn test_adaptive_selector_near_boundary() {
        let selector = AdaptiveModelSelector::new();
        let near_boundary = PoincareDisk::new(0.98, 0.0).unwrap();
        let at_origin = PoincareDisk::new(0.1, 0.1).unwrap();

        let model = selector.select_for_distance(&near_boundary, &at_origin);
        assert_eq!(model, HyperbolicModel::Hyperboloid);
    }
}
