use serde::{Deserialize, Serialize};

/// Tone mapping mode (affects how HDR is compressed to display range)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToneMapMode {
    /// Linear tone mapping (simple clamping)
    Linear,
    /// Logarithmic tone mapping (compresses bright areas)
    Logarithmic,
}

impl Default for ToneMapMode {
    fn default() -> Self {
        Self::Logarithmic
    }
}

/// A single control point on the tone curve
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Evaluate the curve at a given input value using linear interpolation
    pub fn evaluate(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);

        // Find the two points to interpolate between
        for i in 0..self.points.len() - 1 {
            let p0 = &self.points[i];
            let p1 = &self.points[i + 1];

            if x >= p0.x && x <= p1.x {
                // Linear interpolation between p0 and p1
                let t = (x - p0.x) / (p1.x - p0.x);
                return p0.y + t * (p1.y - p0.y);
            }
        }

        // Fallback (shouldn't reach here if points include 0 and 1)
        x
    }

    /// Generate a 256-sample lookup texture for fast GPU evaluation
    /// Returns RGBA8 data (R channel = curve value, GBA = 0)
    pub fn generate_lut(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(256 * 4);
        for i in 0..256 {
            let x = i as f32 / 255.0;
            let y = self.evaluate(x);
            let y_u8 = (y * 255.0).clamp(0.0, 255.0) as u8;
            data.push(y_u8);  // R
            data.push(0);      // G
            data.push(0);      // B
            data.push(255);    // A (unused, but some GPUs need it)
        }
        data
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
        assert_eq!(lut.len(), 256 * 4);
        assert_eq!(lut[0], 0);    // First R value
        assert_eq!(lut[1020], 255); // Last R value (255 * 4)
    }
}
