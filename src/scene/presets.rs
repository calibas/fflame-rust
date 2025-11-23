use super::transforms::{Flame, Transform};
use super::palette::{ColorMode, PaletteLibrary, Palette};
use super::tonemap::{ToneMapMode, ToneCurve};
use crate::config::FractalConfig;

pub fn create_jdisc_sea_flame() -> Flame {
    let mut flame = Flame::new();
    flame.name = "Julian Disc Sea".to_string();

    // Transform 1: mostly linear with slight sinusoidal
    let mut xform1 = Transform::new();
    xform1.a = -0.9499367;
    xform1.b = -0.06820806;
    xform1.c = 0.06820806;
    xform1.d = -0.9499367;
    xform1.e = 0.0;
    xform1.set_variation("disc", 1.0);
    xform1.color = 0.0;
    xform1.color_speed = 0.9;
    xform1.weight = 15.0;
    flame.add_transform(xform1);

    // Transform 2: rotation with swirl
    let mut xform2 = Transform::new();
    xform2.a = 0.36681697;
    xform2.b = 0.41341522;
    xform2.c =  -0.41341522;
    xform2.d = 0.36681697;
    xform2.set_variation("julian", 1.0);
    xform2.set_variation_param("julian", "power", 50.0);
    xform2.set_variation_param("julian", "dist", -1.0);
    xform2.color = 1.0; // Blue
    xform2.color_speed = 0.0;
    xform2.weight = 0.5;
    flame.add_transform(xform2);

    flame
}

/// Create a simple two-transform flame with linear and sinusoidal variations
#[allow(dead_code)]
pub fn create_simple_flame() -> Flame {
    let mut flame = Flame::new();
    flame.name = "Simple".to_string();

    // Transform 1: mostly linear with slight sinusoidal
    let mut xform1 = Transform::new();
    xform1.a = 0.8;
    xform1.d = 0.8;
    xform1.e = 0.1;
    xform1.set_variation("linear", 0.8);
    xform1.set_variation("sinusoidal", 0.2);
    xform1.color = 0.0; // Red
    xform1.weight = 1.0;
    flame.add_transform(xform1);

    // Transform 2: rotation with swirl
    let mut xform2 = Transform::new();
    xform2.a = 0.6;
    xform2.b = -0.3;
    xform2.c = 0.3;
    xform2.d = 0.6;
    xform2.set_variation("linear", 0.6);
    xform2.set_variation("swirl", 0.4);
    xform2.color = 0.7; // Blue
    xform2.weight = 1.0;
    flame.add_transform(xform2);

    flame
}

/// Create a flame with spherical variation (creates a circular inversion)
#[allow(dead_code)]
pub fn create_spherical_flame() -> Flame {
    let mut flame = Flame::new();
    flame.name = "Spherical".to_string();

    let mut xform1 = Transform::new();
    xform1.a = 0.9;
    xform1.d = 0.9;
    xform1.set_variation("spherical", 1.0);
    xform1.color = 0.1; // Orange
    xform1.weight = 1.0;
    flame.add_transform(xform1);

    let mut xform2 = Transform::new();
    xform2.a = -0.5;
    xform2.b = 0.5;
    xform2.c = -0.5;
    xform2.d = -0.5;
    xform2.set_variation("spherical", 1.0);
    xform2.color = 0.5; // Cyan
    xform2.weight = 1.0;
    flame.add_transform(xform2);

    flame
}

/// Create a flame with spiral variation
#[allow(dead_code)]
pub fn create_spiral_flame() -> Flame {
    let mut flame = Flame::new();
    flame.name = "Spiral".to_string();

    let mut xform1 = Transform::new();
    xform1.a = 0.7;
    xform1.d = 0.7;
    xform1.e = 0.2;
    xform1.set_variation("spiral", 1.0);
    xform1.color = 0.2; // Yellow
    xform1.weight = 1.0;
    flame.add_transform(xform1);

    let mut xform2 = Transform::new();
    xform2.a = 0.5;
    xform2.b = -0.5;
    xform2.c = 0.5;
    xform2.d = 0.5;
    xform2.set_variation("spiral", 0.7);
    xform2.set_variation("linear", 0.3);
    xform2.color = 0.8; // Magenta
    xform2.weight = 1.0;
    flame.add_transform(xform2);

    flame
}

/// Create a flame with julia set variation
#[allow(dead_code)]
pub fn create_julia_flame() -> Flame {
    let mut flame = Flame::new();
    flame.name = "Julia".to_string();

    let mut xform1 = Transform::new();
    xform1.a = 0.8;
    xform1.d = 0.8;
    xform1.set_variation("julia", 1.0);
    xform1.color = 0.75; // Purple
    xform1.weight = 1.0;
    flame.add_transform(xform1);

    flame
}

/// Create Simple2 - a test preset with specific view settings
#[allow(dead_code)]
pub fn create_simple2_flame() -> Flame {
    let mut flame = Flame::new();
    flame.name = "Simple2".to_string();

    // Transform 1: Linear + Sinusoidal
    let mut xform1 = Transform::new();
    xform1.a = 0.8;
    xform1.b = 0.0;
    xform1.c = 0.0;
    xform1.d = 0.8;
    xform1.e = 0.1;
    xform1.f = 0.0;
    xform1.g = 0.0;
    xform1.weight = 1.0;
    xform1.set_variation("sinusoidal", 0.2);
    xform1.set_variation("linear", 0.8);
    xform1.color = 0.0;
    xform1.color_speed = 0.5;
    flame.add_transform(xform1);

    // Transform 2: Swirl + Linear
    let mut xform2 = Transform::new();
    xform2.a = 0.6;
    xform2.b = -0.3;
    xform2.c = 0.3;
    xform2.d = 0.6;
    xform2.e = 0.0;
    xform2.f = 0.0;
    xform2.g = 0.0;
    xform2.weight = 1.0;
    xform2.set_variation("swirl", 0.4);
    xform2.set_variation("linear", 0.6);
    xform2.color = 0.7;
    xform2.color_speed = 0.5;
    flame.add_transform(xform2);

    flame
}

/// Create a complex multi-transform flame
#[allow(dead_code)]
pub fn create_complex_flame() -> Flame {
    let mut flame = Flame::new();
    flame.name = "Complex".to_string();

    // Transform 1: Linear base
    let mut xform1 = Transform::new();
    xform1.a = 0.5;
    xform1.b = 0.2;
    xform1.c = -0.2;
    xform1.d = 0.5;
    xform1.e = 0.3;
    xform1.set_variation("linear", 0.5);
    xform1.set_variation("sinusoidal", 0.5);
    xform1.color = 0.0;
    xform1.weight = 2.0;
    flame.add_transform(xform1);

    // Transform 2: Spherical
    let mut xform2 = Transform::new();
    xform2.a = 0.7;
    xform2.d = 0.7;
    xform2.set_variation("spherical", 1.0);
    xform2.color = 0.4;
    xform2.weight = 1.5;
    flame.add_transform(xform2);

    // Transform 3: Horseshoe
    let mut xform3 = Transform::new();
    xform3.a = 0.4;
    xform3.b = -0.4;
    xform3.c = 0.4;
    xform3.d = 0.4;
    xform3.set_variation("horseshoe", 0.8);
    xform3.set_variation("linear", 0.2);
    xform3.color = 0.7;
    xform3.weight = 1.0;
    flame.add_transform(xform3);

    // Transform 4: Heart
    let mut xform4 = Transform::new();
    xform4.a = 0.6;
    xform4.d = 0.6;
    xform4.f = -0.2;
    xform4.set_variation("heart", 1.0);
    xform4.color = 0.2;
    xform4.weight = 0.8;
    flame.add_transform(xform4);

    flame
}

/// Create a 3D flame using z-manipulating variations
#[allow(dead_code)]
pub fn create_3d_flame() -> Flame {
    use super::transforms::RenderMode;

    let mut flame = Flame::new();
    flame.name = "3D Spiral Tower".to_string();
    flame.render_mode = RenderMode::ThreeD;
    flame.perspective_strength = 3.0;

    // Transform 1: Linear with Zcone - creates a cone in Z
    let mut xform1 = Transform::new();
    xform1.a = 0.7;
    xform1.d = 0.7;
    xform1.e = 0.1;
    xform1.g = -0.3; // Z offset
    xform1.set_variation("linear", 0.5);
    xform1.set_variation("zcone", 0.5);
    xform1.color = 0.05; // Red
    xform1.weight = 1.0;
    flame.add_transform(xform1);

    // Transform 2: Spherical with PostRotateY - twist in 3D
    let mut xform2 = Transform::new();
    xform2.a = 0.6;
    xform2.b = -0.3;
    xform2.c = 0.3;
    xform2.d = 0.6;
    xform2.g = 0.2; // Z offset
    xform2.set_variation("spherical", 0.7);
    xform2.set_variation("post_rotate_y", 0.3);
    xform2.color = 0.65; // Blue
    xform2.weight = 1.0;
    flame.add_transform(xform2);

    // Transform 3: Hemisphere - creates 3D sphere structure
    let mut xform3 = Transform::new();
    xform3.a = 0.5;
    xform3.d = 0.5;
    xform3.g = 0.1;
    xform3.set_variation("hemisphere", 1.0);
    xform3.color = 0.35; // Green
    xform3.weight = 0.8;
    flame.add_transform(xform3);

    flame
}

/// Create a Flower of Life sacred geometry pattern
/// Uses central spiral with 6 linear petals arranged in a hexagon
#[allow(dead_code)]
pub fn create_flower_of_life() -> Flame {
    let mut flame = Flame::new();
    flame.name = "Flower of Life".to_string();

    use std::f32::consts::PI;

    // Center transform with spiral to create the core circle
    let mut center = Transform::new();
    center.a = 0.5;
    center.d = 0.5;
    center.set_variation("spiral", 1.0);
    center.color = 0.5; // White center
    center.weight = 1.0;
    flame.add_transform(center);

    // Create 6 transforms arranged in a hexagon (60-degree increments)
    // Each uses linear variation to create clean circular petals
    for i in 0..6 {
        let angle = (i as f32) * PI / 3.0; // 60 degrees in radians
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let mut xform = Transform::new();

        // Small scale with rotation
        xform.a = cos_a * 0.4;
        xform.b = -sin_a * 0.4;
        xform.c = sin_a * 0.4;
        xform.d = cos_a * 0.4;

        // Offset to position this petal
        xform.e = cos_a * 0.35;
        xform.f = sin_a * 0.35;

        // Use linear variation for clean circles
        xform.set_variation("linear", 1.0);

        // Create rainbow colors around the circle
        // Map to palette positions evenly distributed
        xform.color = (i as f32) / 6.0;

        xform.weight = 1.0;
        flame.add_transform(xform);
    }

    flame
}

/// Get all preset flames
#[allow(dead_code)]
pub fn get_all_presets() -> Vec<(&'static str, Flame)> {
    vec![
        ("Simple", create_simple_flame()),
        ("Spherical", create_spherical_flame()),
        ("Spiral", create_spiral_flame()),
        ("Julia", create_julia_flame()),
        ("Complex", create_complex_flame()),
        ("Flower of Life", create_flower_of_life()),
    ]
}

/// Collection of available fractal presets
pub struct PresetLibrary {
    presets: Vec<FractalConfig>,
}

impl Default for PresetLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl PresetLibrary {
    pub fn new() -> Self {
        // Always start with built-in presets
        let mut presets = vec![
        // Julian Disc Sea
            {
                // let palette_library = PaletteLibrary::new();
                // let palette = palette_library.get(1).cloned();
                let palette = Palette::from_hex_string("South Sea Bather".to_string(), "B9EAEB,C1EEEB,C5F2EB,C9F2EB,C9F6EB,CDF6EB,CDF6EB,CDF2EB,D1F2EB,D2EEEB,D1F2E1,D6F2EB,DDF6FE,D5F2F4,F2FAF4,E2F2EB,DEF2EB,D6F2EB,D6F2F4,D1EEF4,D1EEF4,CDEEF4,CDEEEB,C9EEEB,C9EEEB,C9EEF4,C9EEF4,C9F2F4,CDF2F4,D1F2F4,D2F2F4,D1F6F4,CDF2F4,C5F2F4,BDF2F4,BDF2F4,B9EEF4,B5F2F4,BDF2F4,C1F2F4,C5F2FE,C5F2FE,BDF2F4,B5F2F4,B1F2F4,B5EEF4,BDEAEB,BDEAEB,C1E6EB,C1E6EB,BDE6E1,B5E6E1,A5E6E1,A5E2E1,A1E2EB,9DE6EA,99DEF4,A5E2F4,A5E6F4,A5E6F4,A9E2F4,ADE2EB,B1E2EB,B1DEEB,B1E2EB,B1E6F4,B1E2F4,B1E2F4,B1E2F4,ADE2F4,A9E2F4,A1E6FE,9DE6FE,A5EAF4,ADEAF4,B1EEEA,B9EEEB,C1EEEB,C5EEEB,C5EEEB,C9EEEB,C9F2F4,C5F2F4,C5EEF4,C5EAF4,C5EAF4,C5EAF4,C1E6EB,C1E6EB,C5EAE1,C5E6E2,C2E2CF,CE9B84,B27F71,A68455,918055,9A8055,A27255,A2764B,B6724B,BA7F67,D2A484,C6E2C6,C9EAE1,CEEED8,DEAB83,CE9B7A,BE907A,CA977A,DBA384,E6B796,FAE9CE,DEEAEB,D1EEEB,C1E2EB,BDDEEB,B5DEEB,ADE2F4,A9E2F4,A9E2F4,ADE6F4,ADEAEB,ADEAEA,ADE6EB,ADE2EB,ADE2EA,B1E6E2,B5E6E2,BDE6EB,C1E6F4,C5EAF4,C9EAF4,C9EEF4,C9EEF4,C9EEF4,C9EEF4,C9EEF4,C9F2F4,C9F2F4,C9F2F4,C9F2F4,C9F2F4,C5EEF4,C1EEFE,B1EEFE,ADE6F4,B1E6F4,B1EAF4,B5EEF4,BDEEF4,C1F2F4,C9F6F4,CDF6F4,CDF6F4,CDF6F4,CDF2F4,CDEEF4,CDEEFE,C9EEFE,C5EEFE,C1EEF4,C1EEF4,C1EAF4,C1EAEB,C1EAEB,C1EAEB,BDEAF4,B9EAF4,B5EAF4,B5E6F4,B5E6F4,B5EAF4,BDEAF4,C1EAF4,C5EAEB,C5EAEB,C9EAEB,C9EAF4,CDEAF4,CDEEF4,CDEEF4,CDEEF4,CDF2F4,C9F2EB,C9F2EB,C5F2EB,C1EAF4,B9E6F4,B5E2F4,B5E2F4,B5E6F4,B5E6F4,B9EAEB,BDEEEB,BDF2EB,C1EEEB,C5EEEB,C5EEEB,C5EEE1,C1EAE1,BDDED8,AA9471,756842,483725,0B0C09,242C25,4C7567,9E9171,B1CEC5,BDE2D8,BDEAE2,C1EEEB,BDEEF4,BDEEF4,BDEEF4,B9EAF4,B9EAF4,B9E6F4,BDE6EB,BDE6EB,BDEAF4,C1EEF4,C5F2FE,C9F6FE,C9F2FE,C5EEFE,C1EEF4,BDEEEB,B9EAEB,B1E6EA,B5E6EB,B5E6EB,B9E2EB,B5E6EB,BDE6EB,C1EAEB,C1EAF4,BDEAF4,B9E6F4,B5E6F4,B1E6F4,B1E6EB,A9E2EB,A9E2EB,A1DEE1,89BEC5,9E917A,957C67,857967,8D6A4B,8D5F42,856342,796C42,796438,756841,5D5938", true).unwrap_or_default();
                FractalConfig {
                    flame: create_jdisc_sea_flame(),
                    zoom: 15.262974,
                    pan_x: -0.013063544,
                    pan_y: 0.008007533,
                    rotation: -0.06981317,
                    camera_rotation_x: 0.0,
                    camera_rotation_y: 0.0,
                    camera_z: 0.0,
                    density_scale: 1.0,
                    speed_factor: 1.0,
                    max_iterations: 1_000_000_000,
                    color_mode: ColorMode::Palette,
                    palette_index: 1,
                    palette: Some(palette),
                    palette_rotation: 0.0,
                    background_color: [0.0, 0.0, 0.0],
                    tonemap_mode: ToneMapMode::Logarithmic,
                    tonemap_curve: ToneCurve::linear(),
                    use_curve: true,
                    exposure: 1.0,
                    gamma: 4.0,
                    brightness: 10.0,
                    vibrancy: 1.0,
                    saturation: 3.0,
                    hue_shift: 0.0,
                    value_scale: 1.0,
                    gamma_threshold: 150.0,
                    deterministic_rng: false,
                    histogram_color_scale: 100.0,
                    low_density_smoothing: 0.5,
                    density_compression_strength: 0.0,
                    blend_factor: 0.1,
                    use_dynamic_blend: true,
                    target_iterations_per_pixel: 0,
                    iterations_per_thread: 256,
                    vsync_enabled: true,
                    target_fps: 60.0,
                }
            },
            Self::flame_to_config(create_simple_flame()),
            Self::flame_to_config(create_spherical_flame()),
            Self::flame_to_config(create_spiral_flame()),
            Self::flame_to_config(create_julia_flame()),
            Self::flame_to_config(create_complex_flame()),
            Self::flame_to_config(create_flower_of_life()),
            Self::flame_to_config(create_3d_flame()),
            // Simple2 with specific view settings for testing
            {
                let palette_library = PaletteLibrary::new();
                let palette = palette_library.get(1).cloned();
                FractalConfig {
                    flame: create_simple2_flame(),
                    zoom: 54.76374,
                    pan_x: 0.03848837,
                    pan_y: 0.11393361,
                    rotation: 0.0,
                    camera_rotation_x: 0.0,
                    camera_rotation_y: 0.0,
                    camera_z: 0.0,
                    density_scale: 1.2,
                    speed_factor: 0.5,
                    max_iterations: 1_000_000_000,
                    color_mode: ColorMode::Palette,
                    palette_index: 1,
                    palette,
                    palette_rotation: 0.0,
                    background_color: [0.0, 0.0, 0.0],
                    tonemap_mode: ToneMapMode::Logarithmic,
                    tonemap_curve: ToneCurve::linear(),
                    use_curve: true,
                    exposure: 1.0,
                    gamma: 2.2,
                    brightness: 1.0,
                    vibrancy: 1.0,
                    saturation: 1.0,
                    hue_shift: 0.0,
                    value_scale: 1.0,
                    gamma_threshold: 0.0025,
                    deterministic_rng: false,
                    histogram_color_scale: 100.0,
                    low_density_smoothing: 0.5,
                    density_compression_strength: 0.0,
                    blend_factor: 0.1,
                    use_dynamic_blend: true,
                    target_iterations_per_pixel: 0,
                    iterations_per_thread: 256,
                    vsync_enabled: true,
                    target_fps: 60.0,
                }
            },
        ];

        // Desktop: Load additional presets from assets folder (copied to target/ by build.rs)
        #[cfg(not(target_arch = "wasm32"))]
        {
            let assets_configs = super::assets::load_configs_from_dir(
                std::path::Path::new("assets/presets")
            );
            // Add any presets from assets that aren't already built-in
            for config in assets_configs {
                if !presets.iter().any(|p| p.flame.name == config.flame.name) {
                    presets.push(config);
                }
            }
        }

        Self { presets }
    }

    /// Helper to convert old Flame to FractalConfig with sensible defaults
    fn flame_to_config(flame: Flame) -> FractalConfig {
        use crate::scene::palette::PaletteLibrary;

        // Get palette from library for complete export
        let palette_library = PaletteLibrary::new();
        let palette = palette_library.get(1).cloned(); // Fire palette

        FractalConfig {
            flame,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            camera_rotation_x: 0.0,
            camera_rotation_y: 0.0,
            camera_z: 0.0,
            density_scale: 1.0,
            speed_factor: 0.5,
            max_iterations: 1_000_000_000,
            color_mode: ColorMode::Palette,
            palette_index: 1,
            palette,
            palette_rotation: 0.0,
            background_color: [0.0, 0.0, 0.0],
            tonemap_mode: ToneMapMode::Logarithmic,
            tonemap_curve: ToneCurve::linear(),
            use_curve: true,
            exposure: 1.0,
            gamma: 2.2,
            brightness: 1.0,
            vibrancy: 1.0,
            saturation: 1.0,
            hue_shift: 0.0,
            value_scale: 1.0,
            gamma_threshold: 0.0025,
            deterministic_rng: false,
            histogram_color_scale: 100.0,
            low_density_smoothing: 0.5,
            density_compression_strength: 0.0,
            blend_factor: 0.1,
            use_dynamic_blend: true,
            target_iterations_per_pixel: 0,
            iterations_per_thread: 256,
            vsync_enabled: true,
            target_fps: 60.0,
        }
    }

    pub fn presets(&self) -> &[FractalConfig] {
        &self.presets
    }

    pub fn get(&self, index: usize) -> Option<&FractalConfig> {
        self.presets.get(index)
    }

    pub fn len(&self) -> usize {
        self.presets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.presets.is_empty()
    }
}
