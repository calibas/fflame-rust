use serde::{Deserialize, Serialize};

/// Tone mapping mode (affects how HDR is compressed to display range)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToneMapMode {
    /// Linear tone mapping (simple clamping)
    Linear,
    /// Logarithmic tone mapping (compresses bright areas)
    Logarithmic,
    /// Raw density visualization (shows accumulated density as grayscale)
    /// Useful for analyzing density distribution and fine detail
    DensityVisualization,
}

impl Default for ToneMapMode {
    fn default() -> Self {
        Self::Logarithmic
    }
}

/// How to handle color values that exceed [0,1] after exposure/gamma/vibrancy.
/// The shader's clamp-to-[0,1] cliff at the end of the tonemap pass is the
/// Apophysis/JWildfire-compatible default; `MaxNorm` divides all channels by
/// the maximum so hue is preserved exactly when any channel would clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HighlightMode {
    /// Per-channel clamp to [0,1]. Causes hue shift toward CMY/white at high
    /// brightness (orange (1, 0.5, 0) × 5 → (5, 2.5, 0) → clamps to pure
    /// yellow). Matches Apophysis / JWildfire.
    Clip,
    /// Hue-preserving rescale. If `max(r,g,b) > 1`, divide all channels by
    /// the max so the brightest channel lands at 1.0 and the others stay in
    /// ratio. No hue shift; bright pixels desaturate by lowering value
    /// rather than blowing out per-channel.
    MaxNorm,
}

impl Default for HighlightMode {
    fn default() -> Self {
        // Preserve existing visual regression baselines.
        Self::Clip
    }
}

/// A single control point on the tone curve
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CurvePoint {
    /// Input value (0.0 to 1.0)
    pub x: f32,
    /// Output value (0.0 to 1.0)
    pub y: f32,
}

impl CurvePoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
        }
    }
}

/// Tone curve for fine-grained control over tone mapping
/// Similar to Photoshop/Apophysis curves system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToneCurve {
    /// Control points defining the curve (always includes (0,0) and (1,1))
    /// Sorted by x coordinate
    pub points: Vec<CurvePoint>,
}

impl Default for ToneCurve {
    fn default() -> Self {
        Self::linear()
    }
}

impl ToneCurve {
    /// Create a linear curve (identity: output = input)
    pub fn linear() -> Self {
        Self {
            points: vec![
                CurvePoint::new(0.0, 0.0),
                CurvePoint::new(1.0, 1.0),
            ],
        }
    }

    /// Create an S-curve (enhances contrast)
    pub fn s_curve() -> Self {
        Self {
            points: vec![
                CurvePoint::new(0.0, 0.0),
                CurvePoint::new(0.25, 0.15),
                CurvePoint::new(0.5, 0.5),
                CurvePoint::new(0.75, 0.85),
                CurvePoint::new(1.0, 1.0),
            ],
        }
    }

    /// Create a curve that brightens shadows
    pub fn brighten_shadows() -> Self {
        Self {
            points: vec![
                CurvePoint::new(0.0, 0.0),
                CurvePoint::new(0.25, 0.4),
                CurvePoint::new(0.5, 0.6),
                CurvePoint::new(0.75, 0.8),
                CurvePoint::new(1.0, 1.0),
            ],
        }
    }

    /// Create a curve that darkens highlights
    pub fn darken_highlights() -> Self {
        Self {
            points: vec![
                CurvePoint::new(0.0, 0.0),
                CurvePoint::new(0.25, 0.2),
                CurvePoint::new(0.5, 0.4),
                CurvePoint::new(0.75, 0.6),
                CurvePoint::new(1.0, 1.0),
            ],
        }
    }

    /// Evaluate the curve at a given input value using monotonic cubic Hermite interpolation
    /// This creates smooth curves that pass through all control points without overshooting
    pub fn evaluate(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);

        // Find the segment containing x
        for i in 0..self.points.len() - 1 {
            let p0 = &self.points[i];
            let p1 = &self.points[i + 1];

            if x >= p0.x && x <= p1.x {
                // Current segment slope
                let delta = (p1.y - p0.y) / (p1.x - p0.x);

                // Compute tangents using Catmull-Rom finite differences
                let m0 = if i == 0 {
                    // First point: use current segment slope
                    delta
                } else {
                    let p_prev = &self.points[i - 1];
                    // Average of left and right slopes
                    let slope_left = (p0.y - p_prev.y) / (p0.x - p_prev.x);
                    (slope_left + delta) * 0.5
                };

                let m1 = if i == self.points.len() - 2 {
                    // Last point: use current segment slope
                    delta
                } else {
                    let p_next = &self.points[i + 2];
                    // Average of left and right slopes
                    let slope_right = (p_next.y - p1.y) / (p_next.x - p1.x);
                    (delta + slope_right) * 0.5
                };

                // Apply monotonicity constraint to prevent overshoot
                let (m0_final, m1_final) = if delta.abs() < 1e-6 {
                    // Flat segment - zero tangents
                    (0.0, 0.0)
                } else {
                    // Fritsch-Carlson monotonicity constraints
                    let alpha = m0 / delta;
                    let beta = m1 / delta;

                    // Check if tangents would cause non-monotonic behavior
                    if alpha < 0.0 || beta < 0.0 {
                        // One tangent points wrong direction - use linear
                        (delta, delta)
                    } else {
                        // Apply constraint: alpha^2 + beta^2 <= 9
                        let sum_sq = alpha * alpha + beta * beta;
                        if sum_sq > 9.0 {
                            let tau = 3.0 / sum_sq.sqrt();
                            (tau * alpha * delta, tau * beta * delta)
                        } else {
                            // No constraint needed
                            (m0, m1)
                        }
                    }
                };

                // Cubic Hermite interpolation
                let t = (x - p0.x) / (p1.x - p0.x);
                let t2 = t * t;
                let t3 = t2 * t;

                let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                let h10 = t3 - 2.0 * t2 + t;
                let h01 = -2.0 * t3 + 3.0 * t2;
                let h11 = t3 - t2;

                let dx = p1.x - p0.x;
                return h00 * p0.y + h10 * dx * m0_final + h01 * p1.y + h11 * dx * m1_final;
            }
        }

        // Fallback (shouldn't reach here if points include 0 and 1)
        x
    }

    /// Generate a 256-sample lookup texture for fast GPU evaluation
    /// Returns Rgba16Float data as bytes (R channel = curve value, GBA = 0)
    pub fn generate_lut(&self) -> Vec<u8> {
        use half::f16;
        let mut bytes = Vec::with_capacity(256 * 4 * 2); // 256 pixels * 4 components * 2 bytes
        for i in 0..256 {
            // Generate LUT at edge values (i / 255) for proper range coverage
            // This ensures LUT[0] = f(0.0) and LUT[255] = f(1.0)
            let x = i as f32 / 255.0;
            let y = self.evaluate(x);
            // Convert to f16 and write bytes
            bytes.extend_from_slice(&f16::from_f32(y).to_le_bytes());    // R
            bytes.extend_from_slice(&f16::from_f32(0.0).to_le_bytes());  // G
            bytes.extend_from_slice(&f16::from_f32(0.0).to_le_bytes());  // B
            bytes.extend_from_slice(&f16::from_f32(1.0).to_le_bytes());  // A
        }
        bytes
    }

    /// Add a control point and maintain sorted order
    pub fn add_point(&mut self, point: CurvePoint) {
        // Don't allow modifying endpoints
        if point.x <= 0.01 || point.x >= 0.99 {
            return;
        }

        // Check if a point exists at this x coordinate
        if let Some(existing) = self.points.iter_mut().find(|p| (p.x - point.x).abs() < 0.01) {
            existing.y = point.y;
        } else {
            self.points.push(point);
            self.points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        }
    }

    /// Remove a control point (can't remove endpoints)
    pub fn remove_point(&mut self, index: usize) {
        // Don't allow removing endpoints
        if index == 0 || index == self.points.len() - 1 {
            return;
        }
        if index < self.points.len() {
            self.points.remove(index);
        }
    }

    /// Move an existing point
    pub fn move_point(&mut self, index: usize, new_x: f32, new_y: f32) {
        if index >= self.points.len() {
            return;
        }

        // Don't allow moving endpoints horizontally
        if index == 0 || index == self.points.len() - 1 {
            self.points[index].y = new_y.clamp(0.0, 1.0);
        } else {
            self.points[index].x = new_x.clamp(0.0, 1.0);
            self.points[index].y = new_y.clamp(0.0, 1.0);
            self.points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_curve() {
        let curve = ToneCurve::linear();
        assert_eq!(curve.evaluate(0.0), 0.0);
        assert_eq!(curve.evaluate(0.5), 0.5);
        assert_eq!(curve.evaluate(1.0), 1.0);
    }

    #[test]
    fn test_s_curve() {
        let curve = ToneCurve::s_curve();
        // S-curve should darken shadows and brighten highlights
        assert!(curve.evaluate(0.25) < 0.25);
        assert_eq!(curve.evaluate(0.5), 0.5);
        assert!(curve.evaluate(0.75) > 0.75);
    }

    #[test]
    fn test_add_point() {
        let mut curve = ToneCurve::linear();
        curve.add_point(CurvePoint::new(0.5, 0.7));
        assert_eq!(curve.points.len(), 3);
        assert_eq!(curve.evaluate(0.5), 0.7);
    }

    #[test]
    fn test_lut_generation() {
        let curve = ToneCurve::linear();
        let lut = curve.generate_lut();
        // LUT is now f16 (2 bytes per component): 256 pixels × 4 components × 2 bytes
        assert_eq!(lut.len(), 256 * 4 * 2);
    }
}
