use serde::{Deserialize, Serialize};

/// Maximum number of variation types supported
pub const MAX_VARIATIONS: usize = 16;

/// A 2D point in fractal space
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Compute radius squared (r²)
    #[inline]
    pub fn r_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// Compute radius (r)
    #[inline]
    pub fn r(&self) -> f32 {
        self.r_squared().sqrt()
    }

    /// Compute angle (theta)
    #[inline]
    pub fn theta(&self) -> f32 {
        self.y.atan2(self.x)
    }

    /// Compute phi (reciprocal of radius)
    #[inline]
    pub fn phi(&self) -> f32 {
        self.x.atan2(self.y)
    }
}

/// IFS Transform with affine matrix and variation weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    // Affine transformation matrix: x' = ax + by + e, y' = cx + dy + f
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,

    /// Probability weight for selecting this transform
    pub weight: f32,

    /// Weights for each variation function (indexed by VariationType)
    pub variations: [f32; MAX_VARIATIONS],

    /// Color contribution (RGB)
    pub color: [f32; 3],

    /// Color speed (0.0 = parent color, 1.0 = transform color)
    pub color_speed: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
            weight: 1.0,
            variations: [0.0; MAX_VARIATIONS],
            color: [1.0, 1.0, 1.0],
            color_speed: 0.5,
        }
    }
}

impl Transform {
    /// Create a new transform with identity affine matrix
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply the affine transformation to a point
    #[inline]
    pub fn apply_affine(&self, p: Point) -> Point {
        Point {
            x: self.a * p.x + self.b * p.y + self.e,
            y: self.c * p.x + self.d * p.y + self.f,
        }
    }

    /// Apply all variation functions weighted and sum them
    pub fn apply_variations(&self, p: Point) -> Point {
        let mut result = Point::new(0.0, 0.0);

        for (i, &weight) in self.variations.iter().enumerate() {
            if weight != 0.0 {
                let variation_type = VariationType::from_index(i);
                let varied = variation_type.apply(p);
                result.x += weight * varied.x;
                result.y += weight * varied.y;
            }
        }

        result
    }
}

/// Enumeration of variation functions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum VariationType {
    Linear = 0,
    Sinusoidal = 1,
    Spherical = 2,
    Swirl = 3,
    Horseshoe = 4,
    Polar = 5,
    Handkerchief = 6,
    Heart = 7,
    Disc = 8,
    Spiral = 9,
    Hyperbolic = 10,
    Diamond = 11,
    Ex = 12,
    Julia = 13,
    Bent = 14,
    Waves = 15,
}

impl VariationType {
    /// Convert index to variation type
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Linear,
            1 => Self::Sinusoidal,
            2 => Self::Spherical,
            3 => Self::Swirl,
            4 => Self::Horseshoe,
            5 => Self::Polar,
            6 => Self::Handkerchief,
            7 => Self::Heart,
            8 => Self::Disc,
            9 => Self::Spiral,
            10 => Self::Hyperbolic,
            11 => Self::Diamond,
            12 => Self::Ex,
            13 => Self::Julia,
            14 => Self::Bent,
            15 => Self::Waves,
            _ => Self::Linear,
        }
    }

    /// Apply the variation function to a point
    pub fn apply(self, p: Point) -> Point {
        use std::f32::consts::PI;

        let r = p.r();
        let r_sq = p.r_squared();
        let theta = p.theta();
        let _phi = p.phi();

        match self {
            // V0: Linear - identity
            Self::Linear => p,

            // V1: Sinusoidal
            Self::Sinusoidal => Point::new(p.x.sin(), p.y.sin()),

            // V2: Spherical - inversion through unit circle
            Self::Spherical => {
                let r2 = r_sq + 1e-6; // Avoid division by zero
                Point::new(p.x / r2, p.y / r2)
            }

            // V3: Swirl
            Self::Swirl => {
                let r2 = r_sq;
                let sin_r2 = r2.sin();
                let cos_r2 = r2.cos();
                Point::new(p.x * sin_r2 - p.y * cos_r2, p.x * cos_r2 + p.y * sin_r2)
            }

            // V4: Horseshoe
            Self::Horseshoe => {
                let r_inv = 1.0 / (r + 1e-6);
                Point::new((p.x - p.y) * (p.x + p.y) * r_inv, 2.0 * p.x * p.y * r_inv)
            }

            // V5: Polar - convert to polar coordinates
            Self::Polar => Point::new(theta / PI, r - 1.0),

            // V6: Handkerchief
            Self::Handkerchief => {
                let theta_r = theta + r;
                Point::new(r * theta_r.sin(), r * theta_r.cos())
            }

            // V7: Heart
            Self::Heart => {
                let r_theta = r * theta;
                Point::new(r * r_theta.sin(), -r * r_theta.cos())
            }

            // V8: Disc
            Self::Disc => {
                let theta_pi = theta / PI;
                let pi_r = PI * r;
                Point::new(theta_pi * pi_r.sin(), theta_pi * pi_r.cos())
            }

            // V9: Spiral
            Self::Spiral => {
                let r_inv = 1.0 / (r + 1e-6);
                let cos_theta = theta.cos();
                let sin_theta = theta.sin();
                Point::new(r_inv * (cos_theta + sin_theta), r_inv * (cos_theta - sin_theta))
            }

            // V10: Hyperbolic
            Self::Hyperbolic => {
                let r_safe = r + 1e-6;
                Point::new(theta.sin() / r_safe, r_safe * theta.cos())
            }

            // V11: Diamond
            Self::Diamond => {
                let sin_theta = theta.sin();
                let cos_theta = theta.cos();
                Point::new(sin_theta * r.cos(), cos_theta * r.sin())
            }

            // V12: Ex
            Self::Ex => {
                let p0 = theta + r;
                let p1 = theta - r;
                let p0_sin = p0.sin();
                let p1_sin = p1.sin();
                let p0_cubed = p0_sin * p0_sin * p0_sin;
                let p1_cubed = p1_sin * p1_sin * p1_sin;
                Point::new(r * (p0_cubed + p1_cubed), r * (p0_cubed - p1_cubed))
            }

            // V13: Julia - square root with random rotation
            Self::Julia => {
                let sqrt_r = r.sqrt();
                let omega = if rand::random::<f32>() < 0.5 { 0.0 } else { PI };
                let half_theta = theta / 2.0 + omega;
                Point::new(sqrt_r * half_theta.cos(), sqrt_r * half_theta.sin())
            }

            // V14: Bent
            Self::Bent => {
                let nx = if p.x >= 0.0 { p.x } else { 2.0 * p.x };
                let ny = if p.y >= 0.0 { p.y } else { p.y / 2.0 };
                Point::new(nx, ny)
            }

            // V15: Waves (simplified version without parameters)
            Self::Waves => {
                let b = 0.5; // wave parameter
                let c = 0.5;
                let e = 0.5;
                let f = 0.5;
                Point::new(p.x + b * (p.y / (c * c + 1e-6)).sin(), p.y + e * (p.x / (f * f + 1e-6)).sin())
            }
        }
    }
}

/// Apply a transform (affine + variations) to a point
pub fn apply_transform(transform: &Transform, p: Point) -> Point {
    let affine_p = transform.apply_affine(p);
    transform.apply_variations(affine_p)
}

/// Flame system - collection of transforms
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Flame {
    pub transforms: Vec<Transform>,
    pub final_transform: Option<Transform>,
}

impl Flame {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_transform(&mut self, transform: Transform) {
        self.transforms.push(transform);
    }

    /// Calculate cumulative weights for transform selection
    pub fn cumulative_weights(&self) -> Vec<f32> {
        let mut cumulative = Vec::with_capacity(self.transforms.len());
        let mut sum = 0.0;
        for transform in &self.transforms {
            sum += transform.weight;
            cumulative.push(sum);
        }
        cumulative
    }

    /// Select a transform index based on random value
    pub fn select_transform(&self, cumulative_weights: &[f32], rand_val: f32) -> usize {
        let total = cumulative_weights.last().copied().unwrap_or(1.0);
        let target = rand_val * total;

        for (i, &cum_weight) in cumulative_weights.iter().enumerate() {
            if target <= cum_weight {
                return i;
            }
        }
        self.transforms.len().saturating_sub(1)
    }
}

/// Run iterations of the flame algorithm (CPU reference)
pub fn iterate_flame(
    flame: &Flame,
    start_point: Point,
    iterations: usize,
    burn_in: usize,
) -> Vec<Point> {
    if flame.transforms.is_empty() {
        return Vec::new();
    }

    let cumulative_weights = flame.cumulative_weights();
    let mut points = Vec::with_capacity(iterations - burn_in);
    let mut current = start_point;

    for i in 0..iterations {
        // Select random transform
        let rand_val = rand::random::<f32>();
        let transform_idx = flame.select_transform(&cumulative_weights, rand_val);
        let transform = &flame.transforms[transform_idx];

        // Apply transform
        current = apply_transform(transform, current);

        // Skip burn-in iterations
        if i >= burn_in {
            points.push(current);
        }
    }

    // Apply final transform if present
    if let Some(ref final_xform) = flame.final_transform {
        points.iter_mut().for_each(|p| {
            *p = apply_transform(final_xform, *p);
        });
    }

    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_calculations() {
        let p = Point::new(3.0, 4.0);
        assert_eq!(p.r_squared(), 25.0);
        assert_eq!(p.r(), 5.0);
    }

    #[test]
    fn test_linear_variation() {
        let p = Point::new(2.0, 3.0);
        let result = VariationType::Linear.apply(p);
        assert_eq!(result, p);
    }

    #[test]
    fn test_spherical_variation() {
        let p = Point::new(2.0, 0.0);
        let result = VariationType::Spherical.apply(p);
        assert!((result.x - 0.5).abs() < 1e-6);
        assert!((result.y).abs() < 1e-6);
    }

    #[test]
    fn test_affine_transform() {
        let mut xform = Transform::new();
        xform.a = 2.0;
        xform.e = 1.0;

        let p = Point::new(1.0, 0.0);
        let result = xform.apply_affine(p);
        assert_eq!(result.x, 3.0);
        assert_eq!(result.y, 0.0);
    }

    #[test]
    fn test_flame_iteration() {
        let mut flame = Flame::new();
        let mut xform = Transform::new();
        xform.variations[0] = 1.0; // Linear variation
        flame.add_transform(xform);

        let start = Point::new(0.5, 0.5);
        let points = iterate_flame(&flame, start, 10, 5);
        assert_eq!(points.len(), 5);
    }
}
