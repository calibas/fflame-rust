//! Random flame generation
//!
//! Generates random but visually interesting fractal flames for exploration.

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::f32::consts::PI;
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

/// Symmetry type for generated flames
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum SymmetryType {
    #[default]
    None,
    /// Mirror across Y axis (flip X). Adds 1 transform with A=-1.
    BilateralHorizontal,
    /// Mirror across X axis (flip Y). Adds 1 transform with D=-1.
    BilateralVertical,
    /// N-fold rotational symmetry. Adds N-1 transforms rotated by k×(360°/N).
    Rotational(u8),
    /// Dihedral symmetry (rotation + reflection). Adds N transforms total.
    Dihedral(u8),
}

impl SymmetryType {
    /// Returns the number of symmetry transforms that will be added
    pub fn transform_count(&self) -> usize {
        match self {
            SymmetryType::None => 0,
            SymmetryType::BilateralHorizontal => 1,
            SymmetryType::BilateralVertical => 1,
            SymmetryType::Rotational(n) => (*n as usize).saturating_sub(1),
            SymmetryType::Dihedral(n) => *n as usize, // 1 bilateral + (n-1) rotational
        }
    }
}

/// Settings for random fractal generation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RandomGeneratorSettings {
    /// Name for this preset
    pub name: String,

    // Transform settings
    pub transform_count_min: usize,
    pub transform_count_max: usize,
    pub include_final_transform: bool,

    // Variation settings
    pub variations_per_transform_min: usize,
    pub variations_per_transform_max: usize,
    pub variation_weight_min: f32,
    pub variation_weight_max: f32,
    pub always_include_linear: bool,
    pub enabled_variations: HashSet<String>,

    // Affine ranges
    pub scale_min: f32,
    pub scale_max: f32,
    pub shear_min: f32,
    pub shear_max: f32,
    pub translate_min: f32,
    pub translate_max: f32,
    pub allow_negative_scale: bool,

    // Color & weight
    pub weight_min: f32,
    pub weight_max: f32,
    pub distribute_colors_evenly: bool,
    pub random_palette: bool,

    // Symmetry options
    pub symmetry: SymmetryType,

    // 3D options
    pub enable_3d: bool,
    pub include_3d_variations: bool,
    pub perspective_min: f32,
    pub perspective_max: f32,

    // Batch options
    pub batch_count: usize,
    pub seed: Option<u64>,
}

impl Default for RandomGeneratorSettings {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            transform_count_min: 2,
            transform_count_max: 5,
            include_final_transform: false,
            variations_per_transform_min: 1,
            variations_per_transform_max: 3,
            variation_weight_min: 0.2,
            variation_weight_max: 1.0,
            always_include_linear: false,
            enabled_variations: default_variations(),
            scale_min: 0.3,
            scale_max: 1.5,
            shear_min: -0.8,
            shear_max: 0.8,
            translate_min: -1.0,
            translate_max: 1.0,
            allow_negative_scale: true,
            weight_min: 0.5,
            weight_max: 1.5,
            distribute_colors_evenly: true,
            random_palette: true,
            symmetry: SymmetryType::None,
            enable_3d: false,
            include_3d_variations: false,
            perspective_min: 0.0,
            perspective_max: 5.0,
            batch_count: 10,
            seed: None,
        }
    }
}

/// Returns the default set of enabled variations
fn default_variations() -> HashSet<String> {
    GOOD_VARIATIONS.iter().map(|s| s.to_string()).collect()
}

/// Generate a random flame with visually interesting properties (using default settings)
pub fn generate_random_flame() -> Flame {
    generate_random_flame_with_settings(&RandomGeneratorSettings::default())
}

/// Generate a random flame using the provided settings
pub fn generate_random_flame_with_settings(settings: &RandomGeneratorSettings) -> Flame {
    let mut rng = rand::thread_rng();
    generate_random_flame_with_rng(settings, &mut rng)
}

/// Generate a random flame using provided settings and RNG (for seeded generation)
pub fn generate_random_flame_with_rng<R: Rng>(settings: &RandomGeneratorSettings, rng: &mut R) -> Flame {
    // Random number of transforms within range
    let num_transforms = rng.gen_range(settings.transform_count_min..=settings.transform_count_max);

    let mut transforms = Vec::with_capacity(num_transforms + settings.symmetry.transform_count());

    // Generate random transforms
    for i in 0..num_transforms {
        transforms.push(random_transform_with_settings(rng, i, num_transforms, settings));
    }

    // Normalize weights of random transforms to sum to 1.0
    let total_weight: f32 = transforms.iter().map(|t| t.weight).sum();
    if total_weight > 0.0 {
        for transform in &mut transforms {
            transform.weight /= total_weight;
        }
    }

    // Add symmetry transforms after the random transforms
    add_symmetry_transforms(&mut transforms, settings.symmetry);

    // Determine render mode
    let render_mode = if settings.enable_3d {
        RenderMode::ThreeD
    } else {
        RenderMode::TwoD
    };

    // Random perspective strength if 3D enabled
    let perspective_strength = if settings.enable_3d {
        rng.gen_range(settings.perspective_min..=settings.perspective_max)
    } else {
        0.0
    };

    Flame {
        id: crate::scene::transforms::next_id(),
        name: "Random".to_string(),
        transforms,
        linked_transforms: Vec::new(),
        final_transforms: Vec::new(),
        render_mode,
        perspective_strength,
        depth_density_compensation: 0.0,
        far_density_fade: 0.0,
        far_density_fade_start: 0.0,
        xaos: None,
        solo_transform: None,
        subflames: Vec::new(),
        post_symmetry: crate::scene::transforms::PostSymmetry::default(),
        preserve_z: false, // Apo/JWF default — avoid Z-explosion trap.
    }
}

/// Generate a batch of random flames
pub fn generate_batch(settings: &RandomGeneratorSettings) -> Vec<Flame> {
    let mut results = Vec::with_capacity(settings.batch_count);

    if let Some(seed) = settings.seed {
        // Use seeded RNG for reproducibility
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        for i in 0..settings.batch_count {
            let mut flame = generate_random_flame_with_rng(settings, &mut rng);
            flame.name = format!("Random {}", i + 1);
            results.push(flame);
        }
    } else {
        // Use thread RNG
        let mut rng = rand::thread_rng();
        for i in 0..settings.batch_count {
            let mut flame = generate_random_flame_with_rng(settings, &mut rng);
            flame.name = format!("Random {}", i + 1);
            results.push(flame);
        }
    }

    results
}

/// Add symmetry transforms to the flame
fn add_symmetry_transforms(transforms: &mut Vec<Transform>, symmetry: SymmetryType) {
    let sym_count = symmetry.transform_count();
    if sym_count == 0 {
        return;
    }

    match symmetry {
        SymmetryType::None => {}

        SymmetryType::BilateralHorizontal => {
            transforms.push(create_bilateral_horizontal_transform(0.0));
        }

        SymmetryType::BilateralVertical => {
            transforms.push(create_bilateral_vertical_transform(0.0));
        }

        SymmetryType::Rotational(n) => {
            let n = n as usize;
            for k in 1..n {
                let color = k as f32 / sym_count as f32;
                transforms.push(create_rotation_transform(k, n, color));
            }
        }

        SymmetryType::Dihedral(n) => {
            let n = n as usize;
            // First: bilateral horizontal transform
            transforms.push(create_bilateral_horizontal_transform(0.0));
            // Then: rotational transforms
            for k in 1..n {
                let color = k as f32 / sym_count as f32;
                transforms.push(create_rotation_transform(k, n, color));
            }
        }
    }
}

/// Create a bilateral horizontal symmetry transform (mirror across Y axis, flip X)
fn create_bilateral_horizontal_transform(color: f32) -> Transform {
    let mut t = Transform::default();
    t.a = -1.0;  // Flip X
    t.b = 0.0;
    t.c = 0.0;
    t.d = 1.0;
    t.e = 0.0;
    t.f = 0.0;
    t.weight = 1.0;
    t.color = color;
    t.set_variation("linear", 1.0);
    t
}

/// Create a bilateral vertical symmetry transform (mirror across X axis, flip Y)
fn create_bilateral_vertical_transform(color: f32) -> Transform {
    let mut t = Transform::default();
    t.a = 1.0;
    t.b = 0.0;
    t.c = 0.0;
    t.d = -1.0;  // Flip Y
    t.e = 0.0;
    t.f = 0.0;
    t.weight = 1.0;
    t.color = color;
    t.set_variation("linear", 1.0);
    t
}

/// Create a rotation symmetry transform
/// k = rotation index (1 to n-1), n = total fold count
fn create_rotation_transform(k: usize, n: usize, color: f32) -> Transform {
    let angle = 2.0 * PI * (k as f32) / (n as f32);
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    let mut t = Transform::default();
    t.a = cos_a;
    t.b = -sin_a;
    t.c = sin_a;
    t.d = cos_a;
    t.e = 0.0;
    t.f = 0.0;
    t.weight = 1.0;
    t.color = color;
    t.set_variation("linear", 1.0);
    t
}

/// Generate a random transform using the provided settings
fn random_transform_with_settings<R: Rng>(
    rng: &mut R,
    index: usize,
    total: usize,
    settings: &RandomGeneratorSettings,
) -> Transform {
    let mut transform = Transform::default();

    // Random affine parameters within settings ranges
    let neg_scale = settings.allow_negative_scale && rng.gen_bool(0.3);
    transform.a = rng.gen_range(settings.scale_min..=settings.scale_max) * if neg_scale { -1.0 } else { 1.0 };
    transform.d = rng.gen_range(settings.scale_min..=settings.scale_max) * if neg_scale && rng.gen_bool(0.5) { -1.0 } else { 1.0 };

    // Shear components
    transform.b = rng.gen_range(settings.shear_min..=settings.shear_max);
    transform.c = rng.gen_range(settings.shear_min..=settings.shear_max);

    // Translation components
    transform.e = rng.gen_range(settings.translate_min..=settings.translate_max);
    transform.f = rng.gen_range(settings.translate_min..=settings.translate_max);

    // Weight
    transform.weight = rng.gen_range(settings.weight_min..=settings.weight_max);

    // Color: spread across palette based on transform index
    if settings.distribute_colors_evenly {
        transform.color = (index as f32 + 0.5) / total as f32;
    } else {
        transform.color = rng.gen_range(0.0..1.0);
    }

    // Build list of enabled variations
    let enabled: Vec<&str> = settings.enabled_variations
        .iter()
        .map(|s| s.as_str())
        .collect();

    if enabled.is_empty() {
        // Fallback to linear if no variations enabled
        transform.set_variation("linear", 1.0);
    } else {
        // Add random variations
        let num_variations = rng.gen_range(
            settings.variations_per_transform_min..=settings.variations_per_transform_max
        );

        for _ in 0..num_variations {
            let var_idx = rng.gen_range(0..enabled.len());
            let var_name = enabled[var_idx];
            let weight = rng.gen_range(settings.variation_weight_min..=settings.variation_weight_max);
            transform.set_variation(var_name, weight);
        }

        // Ensure first transform has linear if requested
        if index == 0 && settings.always_include_linear && transform.get_variation("linear") == 0.0 {
            transform.set_variation("linear", rng.gen_range(0.3..0.7));
        }
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

            // Weights should sum to approximately 1.0 (random transforms only)
            // Note: symmetry transforms have weight 1.0 and are added after normalization
            let total_weight: f32 = flame.transforms.iter().map(|t| t.weight).sum();
            assert!((total_weight - 1.0).abs() < 0.01);

            // Each transform should have at least one variation
            for transform in &flame.transforms {
                assert!(!transform.variations.is_empty());
            }
        }
    }

    #[test]
    fn test_symmetry_bilateral_horizontal() {
        let mut settings = RandomGeneratorSettings::default();
        settings.symmetry = SymmetryType::BilateralHorizontal;
        settings.transform_count_min = 2;
        settings.transform_count_max = 2;

        let flame = generate_random_flame_with_settings(&settings);

        // 2 random + 1 symmetry = 3 transforms
        assert_eq!(flame.transforms.len(), 3);

        // Last transform should be the symmetry transform
        let sym = &flame.transforms[2];
        assert_eq!(sym.a, -1.0);  // Flip X
        assert_eq!(sym.d, 1.0);
        assert_eq!(sym.weight, 1.0);
    }

    #[test]
    fn test_symmetry_bilateral_vertical() {
        let mut settings = RandomGeneratorSettings::default();
        settings.symmetry = SymmetryType::BilateralVertical;
        settings.transform_count_min = 2;
        settings.transform_count_max = 2;

        let flame = generate_random_flame_with_settings(&settings);

        // 2 random + 1 symmetry = 3 transforms
        assert_eq!(flame.transforms.len(), 3);

        // Last transform should be the symmetry transform
        let sym = &flame.transforms[2];
        assert_eq!(sym.a, 1.0);
        assert_eq!(sym.d, -1.0);  // Flip Y
        assert_eq!(sym.weight, 1.0);
    }

    #[test]
    fn test_symmetry_rotational_3() {
        let mut settings = RandomGeneratorSettings::default();
        settings.symmetry = SymmetryType::Rotational(3);
        settings.transform_count_min = 2;
        settings.transform_count_max = 2;

        let flame = generate_random_flame_with_settings(&settings);

        // 2 random + 2 symmetry (3-fold has 2 rotation transforms) = 4 transforms
        assert_eq!(flame.transforms.len(), 4);

        // Check rotation transforms have correct angles (120° and 240°)
        let sym1 = &flame.transforms[2];
        let sym2 = &flame.transforms[3];

        // 120° rotation: cos(120°) ≈ -0.5
        assert!((sym1.a - (-0.5)).abs() < 0.01);
        assert_eq!(sym1.weight, 1.0);

        // 240° rotation: cos(240°) ≈ -0.5
        assert!((sym2.a - (-0.5)).abs() < 0.01);
        assert_eq!(sym2.weight, 1.0);
    }

    #[test]
    fn test_symmetry_dihedral_3() {
        let mut settings = RandomGeneratorSettings::default();
        settings.symmetry = SymmetryType::Dihedral(3);
        settings.transform_count_min = 2;
        settings.transform_count_max = 2;

        let flame = generate_random_flame_with_settings(&settings);

        // 2 random + 3 symmetry (1 bilateral + 2 rotational) = 5 transforms
        assert_eq!(flame.transforms.len(), 5);

        // First symmetry transform should be bilateral horizontal
        let bilateral = &flame.transforms[2];
        assert_eq!(bilateral.a, -1.0);  // Flip X
        assert_eq!(bilateral.d, 1.0);

        // Next two should be rotations
        let rot1 = &flame.transforms[3];
        let rot2 = &flame.transforms[4];
        assert!((rot1.a - (-0.5)).abs() < 0.01);  // 120°
        assert!((rot2.a - (-0.5)).abs() < 0.01);  // 240°
    }

    #[test]
    fn test_symmetry_transform_count() {
        assert_eq!(SymmetryType::None.transform_count(), 0);
        assert_eq!(SymmetryType::BilateralHorizontal.transform_count(), 1);
        assert_eq!(SymmetryType::BilateralVertical.transform_count(), 1);
        assert_eq!(SymmetryType::Rotational(3).transform_count(), 2);
        assert_eq!(SymmetryType::Rotational(4).transform_count(), 3);
        assert_eq!(SymmetryType::Rotational(6).transform_count(), 5);
        assert_eq!(SymmetryType::Dihedral(3).transform_count(), 3);
        assert_eq!(SymmetryType::Dihedral(4).transform_count(), 4);
    }

    #[test]
    fn test_batch_generation() {
        let mut settings = RandomGeneratorSettings::default();
        settings.batch_count = 5;
        settings.seed = Some(12345);

        let batch = generate_batch(&settings);
        assert_eq!(batch.len(), 5);

        // With same seed, should get same results
        let batch2 = generate_batch(&settings);
        assert_eq!(batch.len(), batch2.len());
        for (f1, f2) in batch.iter().zip(batch2.iter()) {
            assert_eq!(f1.transforms.len(), f2.transforms.len());
        }
    }
}
