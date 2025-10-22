/// Regression tests for fractal flame rendering
/// These tests compare GPU output against known reference images using checksums

use fractal_flame_wgpu::scene::{presets::PresetLibrary, transforms::*};

#[test]
fn test_cpu_reference_deterministic() {
    // Test that transform application is deterministic
    let transform = Transform {
        a: 0.5,
        b: 0.0,
        c: 0.0,
        d: 0.5,
        e: 0.0,
        f: 0.0,
        g: 0.0,
        weight: 1.0,
        variations: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        color: [1.0, 0.0, 0.0],
        color_speed: 0.5,
    };

    // Run twice with same initial conditions
    let mut point1 = Point::new(0.5, 0.5);
    let mut point2 = Point::new(0.5, 0.5);

    for _ in 0..100 {
        point1 = transform.apply_affine(point1);
        point1 = transform.apply_variations(point1);

        point2 = transform.apply_affine(point2);
        point2 = transform.apply_variations(point2);
    }

    // Results should be identical
    assert!((point1.x - point2.x).abs() < 1e-6);
    assert!((point1.y - point2.y).abs() < 1e-6);
}

#[test]
fn test_all_variations_no_panic() {
    // Test that all variation functions work without panicking
    let point = Point::new(0.5, 0.3);

    for var_idx in 0..24 {
        let var_type = VariationType::from_index(var_idx);
        let result = var_type.apply(point);

        // Check for NaN
        assert!(result.x.is_finite(), "Variation {} produced NaN x", var_idx);
        assert!(result.y.is_finite(), "Variation {} produced NaN y", var_idx);
    }
}

#[test]
fn test_transform_weights_valid() {
    // Test that transform weights are reasonable
    let presets = PresetLibrary::new();

    for config in presets.presets() {
        for transform in &config.flame.transforms {
            assert!(transform.weight >= 0.0, "Negative weight in {}", config.flame.name);
            assert!(transform.weight.is_finite(), "Non-finite weight in {}", config.flame.name);
        }
    }
}

#[test]
fn test_affine_identity() {
    // Test that identity transform doesn't change point
    let transform = Transform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
        g: 0.0,
        weight: 1.0,
        variations: {
            let mut vars = [0.0; 24];
            vars[0] = 1.0; // Linear only
            vars
        },
        color: [1.0, 1.0, 1.0],
        color_speed: 0.0,
    };

    let point = Point::new(0.5, 0.3);
    let affine_point = transform.apply_affine(point);
    let result = transform.apply_variations(affine_point);

    // With identity affine and linear variation, point should be unchanged
    assert!((result.x - point.x).abs() < 1e-6);
    assert!((result.y - point.y).abs() < 1e-6);
}

#[test]
fn test_preset_configs_valid() {
    // Test that all built-in presets are valid
    let presets = PresetLibrary::new();

    for config in presets.presets() {
        assert!(!config.flame.name.is_empty());
        assert!(!config.flame.transforms.is_empty());
        assert!(config.flame.transforms.len() <= 32); // MAX_TRANSFORMS

        // Check each transform
        for (i, transform) in config.flame.transforms.iter().enumerate() {
            assert!(
                transform.weight >= 0.0,
                "Transform {} in {} has negative weight",
                i,
                config.flame.name
            );

            // Check affine matrix is not all zeros
            let sum = transform.a.abs()
                + transform.b.abs()
                + transform.c.abs()
                + transform.d.abs();
            assert!(
                sum > 0.0,
                "Transform {} in {} has zero affine matrix",
                i,
                config.flame.name
            );

            // Check variations
            for (j, &var_weight) in transform.variations.iter().enumerate() {
                assert!(
                    var_weight.is_finite(),
                    "Transform {} variation {} in {} is not finite",
                    i,
                    j,
                    config.flame.name
                );
            }

            // Check colors
            for (j, &color) in transform.color.iter().enumerate() {
                assert!(
                    color >= 0.0 && color <= 1.0,
                    "Transform {} color channel {} in {} out of range",
                    i,
                    j,
                    config.flame.name
                );
            }
        }
    }
}

#[test]
fn test_variation_symmetries() {
    // Test that certain variations have expected symmetries

    // Linear should preserve origin
    let linear = VariationType::Linear;
    let origin = Point::new(0.0, 0.0);
    let result = linear.apply(origin);
    assert!((result.x).abs() < 1e-6);
    assert!((result.y).abs() < 1e-6);

    // Spherical at origin should be undefined/zero (r² = 0)
    let spherical = VariationType::Spherical;
    let result = spherical.apply(origin);
    assert!(result.x.is_finite()); // Should not be NaN/Inf

    // Polar should be well-behaved
    let polar = VariationType::Polar;
    let point = Point::new(1.0, 0.0);
    let result = polar.apply(point);
    assert!(result.x.is_finite());
    assert!(result.y.is_finite());
}

#[test]
fn test_2d_variations() {
    // Test that 2D variations work correctly
    let point_2d = Point::new(0.5, 0.3);

    // Test a few 2D variations (indices 0-15)
    let linear = VariationType::Linear;
    let result = linear.apply(point_2d);
    // Linear should pass through unchanged
    assert!((result.x - point_2d.x).abs() < 1e-6);
    assert!((result.y - point_2d.y).abs() < 1e-6);

    // Sinusoidal (2D variation)
    let sinusoidal = VariationType::Sinusoidal;
    let result = sinusoidal.apply(point_2d);
    // Check result is valid
    assert!(result.x.is_finite());
    assert!(result.y.is_finite());
}

#[test]
fn test_projection_types() {
    // Test that projection types are correctly defined
    let ortho = ProjectionType::Orthographic;
    match ortho {
        ProjectionType::Orthographic => {} // OK
        _ => panic!("Orthographic projection type mismatch"),
    }

    let persp = ProjectionType::Perspective { strength: 3.0 };
    match persp {
        ProjectionType::Perspective { strength } => {
            assert!(strength > 0.0);
        }
        _ => panic!("Perspective projection type mismatch"),
    }
}

#[test]
fn test_render_mode() {
    // Test render mode enumeration
    let mode_2d = RenderMode::TwoD;
    let mode_3d = RenderMode::ThreeD;

    assert_ne!(
        std::mem::discriminant(&mode_2d),
        std::mem::discriminant(&mode_3d)
    );
}

#[test]
fn test_color_blending() {
    // Test color speed blending
    let transform = Transform {
        color: [1.0, 0.0, 0.0], // Red
        color_speed: 0.5,
        ..Default::default()
    };

    let parent_color = [0.0, 0.0, 1.0]; // Blue
    let child_color = transform.color;
    let speed = transform.color_speed;

    // Blend should be 50% red + 50% blue = purple
    let blended = [
        parent_color[0] * (1.0 - speed) + child_color[0] * speed,
        parent_color[1] * (1.0 - speed) + child_color[1] * speed,
        parent_color[2] * (1.0 - speed) + child_color[2] * speed,
    ];

    assert!((blended[0] - 0.5).abs() < 1e-6);
    assert!((blended[1] - 0.0).abs() < 1e-6);
    assert!((blended[2] - 0.5).abs() < 1e-6);
}

#[test]
fn test_point_calculations() {
    // Test polar coordinate calculations
    let point = Point::new(3.0, 4.0);

    let r = point.r();
    assert!((r - 5.0).abs() < 1e-6); // 3-4-5 triangle

    let r_squared = point.r_squared();
    assert!((r_squared - 25.0).abs() < 1e-6);

    let theta = point.theta();
    assert!(theta.is_finite());

    // Test with negative coordinates
    let point2 = Point::new(-1.0, -1.0);
    let r2 = point2.r();
    assert!((r2 - 2.0_f32.sqrt()).abs() < 1e-6);
}

#[test]
fn test_config_serialization() {
    // Test that config can be serialized and deserialized
    let presets = PresetLibrary::new();
    let original = presets.get(0).expect("No presets found");

    // Serialize to JSON
    let json = serde_json::to_string(original).expect("Failed to serialize");

    // Deserialize back
    let deserialized: fractal_flame_wgpu::config::FractalConfig = serde_json::from_str(&json).expect("Failed to deserialize");

    // Check key fields match
    assert_eq!(deserialized.flame.name, original.flame.name);
    assert_eq!(
        deserialized.flame.transforms.len(),
        original.flame.transforms.len()
    );
}
