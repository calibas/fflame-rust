//! Random flame generation
//!
//! Generates random but visually interesting fractal flames for exploration.

use rand::Rng;
use crate::scene::transforms::{Flame, Transform, RenderMode};

/// Basic 2D variations that tend to produce visually interesting results
const GOOD_VARIATIONS: &[&str] = &[
    "linear",
    "sinusoidal",
    "spherical",
    "swirl",
    "horseshoe",
    "polar",
    "handkerchief",
    "heart",
    "disc",
    "spiral",
    "hyperbolic",
    "diamond",
    "julia",
    "bent",
    "waves",
];

/// Generate a random flame with visually interesting properties
pub fn generate_random_flame() -> Flame {
    let mut rng = rand::thread_rng();

    // Random number of transforms (2-5)
    let num_transforms = rng.gen_range(2..=5);

    let mut transforms = Vec::with_capacity(num_transforms);

    for i in 0..num_transforms {
        transforms.push(random_transform(&mut rng, i, num_transforms));
    }

    // Normalize weights to sum to 1.0
    let total_weight: f32 = transforms.iter().map(|t| t.weight).sum();
    if total_weight > 0.0 {
        for transform in &mut transforms {
            transform.weight /= total_weight;
        }
    }

    Flame {
        name: "Random".to_string(),
        transforms,
        final_transform: None,
        render_mode: RenderMode::TwoD,
        perspective_strength: 0.0,
    }
}

/// Generate a random transform with reasonable parameters
fn random_transform<R: Rng>(rng: &mut R, index: usize, total: usize) -> Transform {
    let mut transform = Transform::default();

    // Random affine parameters with reasonable ranges
    // Scale components (a, d): 0.3 to 1.5
    transform.a = rng.gen_range(0.3..1.5) * if rng.gen_bool(0.3) { -1.0 } else { 1.0 };
    transform.d = rng.gen_range(0.3..1.5) * if rng.gen_bool(0.3) { -1.0 } else { 1.0 };

    // Shear components (b, c): -0.8 to 0.8
    transform.b = rng.gen_range(-0.8..0.8);
    transform.c = rng.gen_range(-0.8..0.8);

    // Translation components (e, f): -1.0 to 1.0
    transform.e = rng.gen_range(-1.0..1.0);
    transform.f = rng.gen_range(-1.0..1.0);

    // Weight: slightly random but not too extreme
    transform.weight = rng.gen_range(0.5..1.5);

    // Color: spread across palette based on transform index
    transform.color = (index as f32 + rng.gen_range(0.0..0.5)) / total as f32;

    // Add 1-3 random variations
    let num_variations = rng.gen_range(1..=3);
    for _ in 0..num_variations {
        let var_idx = rng.gen_range(0..GOOD_VARIATIONS.len());
        let var_name = GOOD_VARIATIONS[var_idx];

        // Weight between 0.2 and 1.0
        let weight = rng.gen_range(0.2..1.0);
        transform.set_variation(var_name, weight);
    }

    // Ensure at least one transform has linear to provide some structure
    if index == 0 && transform.get_variation("linear") == 0.0 {
        transform.set_variation("linear", rng.gen_range(0.3..0.7));
    }

    transform
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_flame() {
        // Generate a few flames and check basic properties
        for _ in 0..10 {
            let flame = generate_random_flame();

            // Should have 2-5 transforms
            assert!(flame.transforms.len() >= 2);
            assert!(flame.transforms.len() <= 5);

            // Weights should sum to approximately 1.0
            let total_weight: f32 = flame.transforms.iter().map(|t| t.weight).sum();
            assert!((total_weight - 1.0).abs() < 0.01);

            // Each transform should have at least one variation
            for transform in &flame.transforms {
                assert!(!transform.variations.is_empty());
            }
        }
    }
}
