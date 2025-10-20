use super::transforms::{Flame, Transform, VariationType};
use super::palette::ColorMode;
use crate::config::FractalConfig;

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
    xform1.variations[VariationType::Linear as usize] = 0.8;
    xform1.variations[VariationType::Sinusoidal as usize] = 0.2;
    xform1.color = [1.0, 0.2, 0.2]; // Red
    xform1.weight = 1.0;
    flame.add_transform(xform1);

    // Transform 2: rotation with swirl
    let mut xform2 = Transform::new();
    xform2.a = 0.6;
    xform2.b = -0.3;
    xform2.c = 0.3;
    xform2.d = 0.6;
    xform2.variations[VariationType::Linear as usize] = 0.6;
    xform2.variations[VariationType::Swirl as usize] = 0.4;
    xform2.color = [0.2, 0.2, 1.0]; // Blue
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
    xform1.variations[VariationType::Spherical as usize] = 1.0;
    xform1.color = [1.0, 0.5, 0.0]; // Orange
    xform1.weight = 1.0;
    flame.add_transform(xform1);

    let mut xform2 = Transform::new();
    xform2.a = -0.5;
    xform2.b = 0.5;
    xform2.c = -0.5;
    xform2.d = -0.5;
    xform2.variations[VariationType::Spherical as usize] = 1.0;
    xform2.color = [0.0, 1.0, 0.5]; // Cyan
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
    xform1.variations[VariationType::Spiral as usize] = 1.0;
    xform1.color = [1.0, 1.0, 0.0]; // Yellow
    xform1.weight = 1.0;
    flame.add_transform(xform1);

    let mut xform2 = Transform::new();
    xform2.a = 0.5;
    xform2.b = -0.5;
    xform2.c = 0.5;
    xform2.d = 0.5;
    xform2.variations[VariationType::Spiral as usize] = 0.7;
    xform2.variations[VariationType::Linear as usize] = 0.3;
    xform2.color = [1.0, 0.0, 1.0]; // Magenta
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
    xform1.variations[VariationType::Julia as usize] = 1.0;
    xform1.color = [0.8, 0.2, 0.8]; // Purple
    xform1.weight = 1.0;
    flame.add_transform(xform1);

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
    xform1.variations[VariationType::Linear as usize] = 0.5;
    xform1.variations[VariationType::Sinusoidal as usize] = 0.5;
    xform1.color = [1.0, 0.0, 0.0];
    xform1.weight = 2.0;
    flame.add_transform(xform1);

    // Transform 2: Spherical
    let mut xform2 = Transform::new();
    xform2.a = 0.7;
    xform2.d = 0.7;
    xform2.variations[VariationType::Spherical as usize] = 1.0;
    xform2.color = [0.0, 1.0, 0.0];
    xform2.weight = 1.5;
    flame.add_transform(xform2);

    // Transform 3: Horseshoe
    let mut xform3 = Transform::new();
    xform3.a = 0.4;
    xform3.b = -0.4;
    xform3.c = 0.4;
    xform3.d = 0.4;
    xform3.variations[VariationType::Horseshoe as usize] = 0.8;
    xform3.variations[VariationType::Linear as usize] = 0.2;
    xform3.color = [0.0, 0.0, 1.0];
    xform3.weight = 1.0;
    flame.add_transform(xform3);

    // Transform 4: Heart
    let mut xform4 = Transform::new();
    xform4.a = 0.6;
    xform4.d = 0.6;
    xform4.f = -0.2;
    xform4.variations[VariationType::Heart as usize] = 1.0;
    xform4.color = [1.0, 1.0, 0.0];
    xform4.weight = 0.8;
    flame.add_transform(xform4);

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
        let mut presets = vec![];

        // Load presets from assets folder (desktop only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            let assets_configs = super::assets::load_configs_from_dir(
                std::path::Path::new("assets/presets")
            );
            presets.extend(assets_configs);
        }

        // WASM: use built-in presets (no file system access)
        #[cfg(target_arch = "wasm32")]
        {
            presets.extend(vec![
                Self::flame_to_config(create_simple_flame()),
                Self::flame_to_config(create_spherical_flame()),
                Self::flame_to_config(create_spiral_flame()),
                Self::flame_to_config(create_julia_flame()),
                Self::flame_to_config(create_complex_flame()),
            ]);
        }

        Self { presets }
    }

    /// Helper to convert old Flame to FractalConfig with sensible defaults
    fn flame_to_config(flame: Flame) -> FractalConfig {
        FractalConfig {
            flame,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            density_scale: 1.0,
            speed_factor: 0.5,
            color_mode: ColorMode::Transform,
            palette_index: 1,
            background_color: [0.0, 0.0, 0.0],
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
