/// Regression tests for fractal flame rendering
/// These tests verify core functionality hasn't broken

use fractal_flame_wgpu::scene::{presets::PresetLibrary, transforms::*};
use fractal_flame_wgpu::variations::global_registry;

#[test]
fn test_transform_default() {
    // Test that default transform is valid
    let transform = Transform::default();

    assert_eq!(transform.a, 1.0);
    assert_eq!(transform.d, 1.0);
    assert_eq!(transform.weight, 1.0);
    assert_eq!(transform.color, 0.5);  // Mid-palette position
    assert_eq!(transform.color_speed, 0.0);  // Apophysis default
    assert_eq!(transform.opacity, 1.0);
}

#[test]
fn test_transform_new() {
    // Test that Transform::new() creates valid transform
    let transform = Transform::new();

    assert!(transform.variations.is_empty());
    assert!(transform.variation_params.is_empty());
    assert_eq!(transform.weight, 1.0);
}

#[test]
fn test_all_variations_registered() {
    // Test that all core variations are registered
    let registry = global_registry();

    // Core 2D variations (0-15)
    assert!(registry.get("linear").is_some());
    assert!(registry.get("sinusoidal").is_some());
    assert!(registry.get("spherical").is_some());
    assert!(registry.get("swirl").is_some());
    assert!(registry.get("horseshoe").is_some());
    assert!(registry.get("polar").is_some());
    assert!(registry.get("handkerchief").is_some());
    assert!(registry.get("heart").is_some());
    assert!(registry.get("disc").is_some());
    assert!(registry.get("spiral").is_some());
    assert!(registry.get("hyperbolic").is_some());
    assert!(registry.get("diamond").is_some());
    assert!(registry.get("ex").is_some());
    assert!(registry.get("julia").is_some());
    assert!(registry.get("bent").is_some());
    assert!(registry.get("waves").is_some());

    // 3D variations (16-23)
    assert!(registry.get("zcone").is_some());
    assert!(registry.get("flatten").is_some());
    assert!(registry.get("hemisphere").is_some());
    assert!(registry.get("pre_rotate_x").is_some());
    assert!(registry.get("pre_rotate_y").is_some());
    assert!(registry.get("post_rotate_x").is_some());
    assert!(registry.get("post_rotate_y").is_some());
    assert!(registry.get("zscale").is_some());

    // Parameterized variations (24-25)
    assert!(registry.get("julian").is_some());
    assert!(registry.get("blob").is_some());
}

#[test]
fn test_preset_library_loads() {
    // Test that all presets can be loaded without error
    let library = PresetLibrary::new();

    // Should have at least the built-in presets
    assert!(library.len() > 0, "No presets loaded");

    // Should be able to get presets by index
    assert!(library.get(0).is_some(), "First preset missing");
}

#[test]
fn test_preset_configs_valid() {
    // Test that preset configs have valid structure
    let library = PresetLibrary::new();

    for (i, config) in library.presets().iter().enumerate() {
        // Every preset should have at least one transform
        assert!(config.flame.transforms.len() > 0, "Preset {} has no transforms", i);

        // Every transform should have valid weight
        for (j, transform) in config.flame.transforms.iter().enumerate() {
            assert!(transform.weight > 0.0, "Preset {} transform {} has invalid weight", i, j);
            assert!(transform.color >= 0.0 && transform.color <= 1.0,
                "Preset {} transform {} has invalid color", i, j);
        }
    }
}

#[test]
fn test_transform_serialization() {
    // Test that transforms can be serialized and deserialized
    let mut transform = Transform::new();
    transform.a = 0.5;
    transform.d = 0.5;
    transform.variations.insert("linear".to_string(), 0.5);
    transform.variations.insert("sinusoidal".to_string(), 0.3);
    transform.color = 0.6;

    let json = serde_json::to_string(&transform).expect("Failed to serialize");
    let deserialized: Transform = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(deserialized.a, transform.a);
    assert_eq!(deserialized.d, transform.d);
    assert_eq!(deserialized.color, transform.color);
    assert_eq!(deserialized.variations.get("linear"), Some(&0.5));
    assert_eq!(deserialized.variations.get("sinusoidal"), Some(&0.3));
}

#[test]
fn test_flame_serialization() {
    // Test that Flame can be serialized and deserialized
    let mut flame = Flame::default();
    flame.transforms.push(Transform::new());
    flame.transforms[0].variations.insert("linear".to_string(), 1.0);

    let json = serde_json::to_string(&flame).expect("Failed to serialize");
    let deserialized: Flame = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(deserialized.transforms.len(), 1);
    assert_eq!(deserialized.transforms[0].variations.get("linear"), Some(&1.0));
}

#[test]
fn test_variation_parameters() {
    // Test that variation parameters can be set
    let mut transform = Transform::new();
    transform.variations.insert("julian".to_string(), 1.0);
    transform.variation_params.insert("julian.power".to_string(), 3.0);
    transform.variation_params.insert("julian.dist".to_string(), 2.0);

    assert_eq!(transform.variation_params.get("julian.power"), Some(&3.0));
    assert_eq!(transform.variation_params.get("julian.dist"), Some(&2.0));
}

#[test]
fn test_config_manager_creation() {
    // Test that ConfigManager can be created
    use fractal_flame_wgpu::config::{ConfigManager, FractalConfig};

    let config = FractalConfig::default();
    let manager = ConfigManager::new(config);

    // Default flame has no transforms (empty)
    assert_eq!(manager.active_config().flame.transforms.len(), 0);
}

#[test]
fn test_palette_interpolation() {
    // Test that palette can interpolate colors
    use fractal_flame_wgpu::scene::palette::Palette;

    let palette = Palette::fire();

    // Sample at various positions
    let color_0 = palette.sample_color(0.0);
    let color_mid = palette.sample_color(0.5);
    let color_1 = palette.sample_color(1.0);

    // Check all colors are valid RGB
    for color in [color_0, color_mid, color_1] {
        assert!(color[0] >= 0.0 && color[0] <= 1.0);
        assert!(color[1] >= 0.0 && color[1] <= 1.0);
        assert!(color[2] >= 0.0 && color[2] <= 1.0);
    }
}
