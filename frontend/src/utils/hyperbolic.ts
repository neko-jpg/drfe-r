/**
 * Utility functions for hyperbolic geometry in the Poincaré disk model
 */

import type { PoincareDiskPoint } from '../types';

/**
 * Calculate the Euclidean distance between two points
 */
export function euclideanDistance(p1: PoincareDiskPoint, p2: PoincareDiskPoint): number {
  const dx = p2.x - p1.x;
  const dy = p2.y - p1.y;
  return Math.sqrt(dx * dx + dy * dy);
}

/**
 * Calculate the norm (magnitude) of a point
 */
export function norm(p: PoincareDiskPoint): number {
  return Math.sqrt(p.x * p.x + p.y * p.y);
}

/**
 * Calculate the hyperbolic distance between two points in the Poincaré disk
 * Using the formula: d(p1, p2) = acosh(1 + 2 * |p1 - p2|² / ((1 - |p1|²)(1 - |p2|²)))
 */
export function hyperbolicDistance(p1: PoincareDiskPoint, p2: PoincareDiskPoint): number {
  const norm1Sq = p1.x * p1.x + p1.y * p1.y;
  const norm2Sq = p2.x * p2.x + p2.y * p2.y;
  const diffNormSq = (p2.x - p1.x) ** 2 + (p2.y - p1.y) ** 2;
  
  const denominator = (1 - norm1Sq) * (1 - norm2Sq);
  if (denominator <= 0) return Infinity;
  
  const arg = 1 + (2 * diffNormSq) / denominator;
  return Math.acosh(arg);
}

/**
 * Check if a point is inside the Poincaré disk (|z| < 1)
 */
export function isInsideDisk(point: PoincareDiskPoint): boolean {
  return point.x * point.x + point.y * point.y < 1;
}

/**
 * Calculate the geodesic (hyperbolic line) between two points in the Poincaré disk.
 * Geodesics in the Poincaré disk are either:
 * 1. Diameters (straight lines through the origin)
 * 2. Circular arcs orthogonal to the boundary circle
 * 
 * Returns an array of points along the geodesic for rendering.
 */
export function calculateGeodesic(
  p1: PoincareDiskPoint,
  p2: PoincareDiskPoint,
  numPoints: number = 30
): PoincareDiskPoint[] {
  const points: PoincareDiskPoint[] = [];
  
  // Check if points are very close (use straight line)
  const dist = euclideanDistance(p1, p2);
  if (dist < 0.001) {
    return [p1, p2];
  }
  
  // Check if the geodesic passes through or near the origin (use straight line)
  // Cross product to check collinearity with origin
  const cross = p1.x * p2.y - p1.y * p2.x;
  if (Math.abs(cross) < 0.001) {
    // Points are collinear with origin - geodesic is a diameter
    for (let i = 0; i <= numPoints; i++) {
      const t = i / numPoints;
      points.push({
        x: p1.x + t * (p2.x - p1.x),
        y: p1.y + t * (p2.y - p1.y),
      });
    }
    return points;
  }
  
  // Calculate the center and radius of the geodesic arc
  // The geodesic is a circular arc orthogonal to the unit circle
  const geodesicCircle = calculateGeodesicCircle(p1, p2);
  if (!geodesicCircle) {
    // Fallback to straight line
    for (let i = 0; i <= numPoints; i++) {
      const t = i / numPoints;
      points.push({
        x: p1.x + t * (p2.x - p1.x),
        y: p1.y + t * (p2.y - p1.y),
      });
    }
    return points;
  }
  
  const { cx, cy, r } = geodesicCircle;
  
  // Calculate angles for p1 and p2 relative to the circle center
  const angle1 = Math.atan2(p1.y - cy, p1.x - cx);
  const angle2 = Math.atan2(p2.y - cy, p2.x - cx);
  
  // Determine the shorter arc direction
  const startAngle = angle1;
  const endAngle = angle2;
  let angleDiff = endAngle - startAngle;
  
  // Normalize angle difference to [-π, π]
  while (angleDiff > Math.PI) angleDiff -= 2 * Math.PI;
  while (angleDiff < -Math.PI) angleDiff += 2 * Math.PI;
  
  // Generate points along the arc
  for (let i = 0; i <= numPoints; i++) {
    const t = i / numPoints;
    const angle = startAngle + t * angleDiff;
    const point = {
      x: cx + r * Math.cos(angle),
      y: cy + r * Math.sin(angle),
    };
    // Only include points inside the disk
    if (norm(point) < 0.999) {
      points.push(point);
    }
  }
  
  return points.length > 1 ? points : [p1, p2];
}

/**
 * Calculate the circle that defines the geodesic between two points.
 * The geodesic circle is orthogonal to the unit circle.
 */
function calculateGeodesicCircle(
  p1: PoincareDiskPoint,
  p2: PoincareDiskPoint
): { cx: number; cy: number; r: number } | null {
  // Using the formula for the geodesic circle in the Poincaré disk
  // The center lies on the perpendicular bisector of p1 and p2
  // and the circle is orthogonal to the unit circle
  
  const x1 = p1.x, y1 = p1.y;
  const x2 = p2.x, y2 = p2.y;
  
  // Midpoint
  const mx = (x1 + x2) / 2;
  const my = (y1 + y2) / 2;
  
  // Direction perpendicular to p1-p2
  const dx = x2 - x1;
  const dy = y2 - y1;
  const perpX = -dy;
  const perpY = dx;
  
  // The center must satisfy: |center|² - r² = 1 (orthogonality to unit circle)
  // and the center lies on the line: (mx, my) + t * (perpX, perpY)
  
  // |p1 - center|² = r²
  // |center|² = 1 + r²
  
  // Let center = (mx + t*perpX, my + t*perpY)
  // (x1 - mx - t*perpX)² + (y1 - my - t*perpY)² = r²
  // (mx + t*perpX)² + (my + t*perpY)² = 1 + r²
  
  // Simplifying:
  // (dx/2 - t*perpX)² + (dy/2 - t*perpY)² = r²
  // (mx + t*perpX)² + (my + t*perpY)² - 1 = r²
  
  // Setting them equal:
  // (dx/2)² - dx*t*perpX + t²*perpX² + (dy/2)² - dy*t*perpY + t²*perpY² 
  //   = mx² + 2*mx*t*perpX + t²*perpX² + my² + 2*my*t*perpY + t²*perpY² - 1
  
  // (dx² + dy²)/4 - t*(dx*perpX + dy*perpY) = mx² + my² + 2*t*(mx*perpX + my*perpY) - 1
  
  const d2 = dx * dx + dy * dy;
  const m2 = mx * mx + my * my;
  // Note: dotDP = dx * perpX + dy * perpY is always 0 since perp is perpendicular to (dx, dy)
  const dotMP = mx * perpX + my * perpY;
  
  // d2/4 = m2 + 2*t*dotMP - 1
  // t = (d2/4 - m2 + 1) / (2 * dotMP)
  
  if (Math.abs(dotMP) < 1e-10) {
    return null; // Degenerate case
  }
  
  const t = (d2 / 4 - m2 + 1) / (2 * dotMP);
  
  const cx = mx + t * perpX;
  const cy = my + t * perpY;
  
  // Calculate radius
  const r = Math.sqrt((x1 - cx) ** 2 + (y1 - cy) ** 2);
  
  if (!isFinite(r) || r < 0.001 || r > 1000) {
    return null;
  }
  
  return { cx, cy, r };
}

/**
 * Convert Poincaré disk coordinates to canvas coordinates
 */
export function toCanvasCoords(
  point: PoincareDiskPoint,
  canvasWidth: number,
  canvasHeight: number,
  scale: number = 1,
  offsetX: number = 0,
  offsetY: number = 0
): { x: number; y: number } {
  const centerX = canvasWidth / 2;
  const centerY = canvasHeight / 2;
  const radius = Math.min(canvasWidth, canvasHeight) / 2 - 20;
  
  return {
    x: centerX + (point.x * radius * scale) + offsetX,
    y: centerY - (point.y * radius * scale) + offsetY, // Flip Y for canvas
  };
}

/**
 * Convert canvas coordinates to Poincaré disk coordinates
 */
export function fromCanvasCoords(
  canvasX: number,
  canvasY: number,
  canvasWidth: number,
  canvasHeight: number,
  scale: number = 1,
  offsetX: number = 0,
  offsetY: number = 0
): PoincareDiskPoint {
  const centerX = canvasWidth / 2;
  const centerY = canvasHeight / 2;
  const radius = Math.min(canvasWidth, canvasHeight) / 2 - 20;
  
  return {
    x: ((canvasX - offsetX) - centerX) / (radius * scale),
    y: -((canvasY - offsetY) - centerY) / (radius * scale), // Flip Y back
  };
}

/**
 * Get color for curvature value (heatmap)
 * Negative curvature: blue, Zero: white, Positive curvature: red
 * Uses a smooth gradient for better visualization
 */
export function curvatureToColor(curvature: number): string {
  // Clamp curvature to [-1, 1] range
  const normalized = Math.max(-1, Math.min(1, curvature));
  
  if (normalized < 0) {
    // Blue for negative curvature (hyperbolic-like regions)
    // Interpolate from white (0) to blue (-1)
    const t = -normalized; // 0 to 1
    const r = Math.floor(255 * (1 - t));
    const g = Math.floor(255 * (1 - t));
    const b = 255;
    return `rgb(${r}, ${g}, ${b})`;
  } else if (normalized > 0) {
    // Red for positive curvature (spherical-like regions)
    // Interpolate from white (0) to red (+1)
    const t = normalized; // 0 to 1
    const r = 255;
    const g = Math.floor(255 * (1 - t));
    const b = Math.floor(255 * (1 - t));
    return `rgb(${r}, ${g}, ${b})`;
  } else {
    // White for zero curvature (flat/Euclidean)
    return 'rgb(255, 255, 255)';
  }
}

/**
 * Get color for curvature value with alpha channel
 * Useful for overlay effects
 */
export function curvatureToColorWithAlpha(curvature: number, alpha: number = 1): string {
  // Clamp curvature to [-1, 1] range
  const normalized = Math.max(-1, Math.min(1, curvature));
  const a = Math.max(0, Math.min(1, alpha));
  
  if (normalized < 0) {
    // Blue for negative curvature
    const t = -normalized;
    const r = Math.floor(255 * (1 - t));
    const g = Math.floor(255 * (1 - t));
    const b = 255;
    return `rgba(${r}, ${g}, ${b}, ${a})`;
  } else if (normalized > 0) {
    // Red for positive curvature
    const t = normalized;
    const r = 255;
    const g = Math.floor(255 * (1 - t));
    const b = Math.floor(255 * (1 - t));
    return `rgba(${r}, ${g}, ${b}, ${a})`;
  } else {
    return `rgba(255, 255, 255, ${a})`;
  }
}
