use serde::{Deserialize, Serialize};

/// Color mode determines how colors are assigned during iteration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMode {
    /// Use per-transform colors
    Transform,
    /// Use 1D palette texture lookup
    Palette,
    /// Use speed-based coloring (distance traveled per iteration)
    Speed,
}

impl Default for ColorMode {
    fn default() -> Self {
        Self::Transform
    }
}

/// A single color stop in a gradient palette
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ColorStop {
    pub position: f32, // 0.0 to 1.0
    pub color: [f32; 3], // RGB
}

/// Palette definition with gradient stops
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Palette {
    pub name: String,
    pub stops: Vec<ColorStop>,
    /// If true, palette is in fixed 256-color mode (positions locked)
    #[serde(default)]
    pub locked: bool,
    /// If true, palette is built-in and should not be edited directly (create copy instead)
    /// This flag is ONLY set at runtime in PaletteLibrary, never serialized
    #[serde(skip)]
    pub built_in: bool,
}

impl Default for Palette {
    fn default() -> Self {
        Self::grayscale()
    }
}

impl Palette {
    /// Create a new palette with given name and stops
    pub fn new(name: impl Into<String>, stops: Vec<ColorStop>) -> Self {
        Self {
            name: name.into(),
            stops,
            locked: false,
            built_in: false,
        }
    }

    /// Create a new built-in palette (prevents direct editing)
    fn new_builtin(name: impl Into<String>, stops: Vec<ColorStop>) -> Self {
        Self {
            name: name.into(),
            stops,
            locked: false,
            built_in: true,
        }
    }

    /// Create a simple grayscale palette
    pub fn grayscale() -> Self {
        Self::new_builtin(
            "Grayscale",
            vec![
                ColorStop {
                    position: 0.0,
                    color: [0.0, 0.0, 0.0],
                },
                ColorStop {
                    position: 1.0,
                    color: [1.0, 1.0, 1.0],
                },
            ],
        )
    }

    /// Create a fire palette (black -> red -> orange -> yellow -> white)
    pub fn fire() -> Self {
        Self::new_builtin(
            "Fire",
            vec![
                ColorStop {
                    position: 0.0,
                    color: [0.0, 0.0, 0.0],
                },
                ColorStop {
                    position: 0.25,
                    color: [0.5, 0.0, 0.0],
                },
                ColorStop {
                    position: 0.5,
                    color: [1.0, 0.3, 0.0],
                },
                ColorStop {
                    position: 0.75,
                    color: [1.0, 0.8, 0.0],
                },
                ColorStop {
                    position: 1.0,
                    color: [1.0, 1.0, 0.8],
                },
            ],
        )
    }

    /// Create a cool (blue-cyan-white) palette
    pub fn cool() -> Self {
        Self::new_builtin(
            "Cool",
            vec![
                ColorStop {
                    position: 0.0,
                    color: [0.0, 0.0, 0.2],
                },
                ColorStop {
                    position: 0.33,
                    color: [0.0, 0.2, 0.6],
                },
                ColorStop {
                    position: 0.66,
                    color: [0.0, 0.6, 0.8],
                },
                ColorStop {
                    position: 1.0,
                    color: [0.8, 1.0, 1.0],
                },
            ],
        )
    }

    /// Create a rainbow palette
    pub fn rainbow() -> Self {
        Self::new_builtin(
            "Rainbow",
            vec![
                ColorStop {
                    position: 0.0,
                    color: [1.0, 0.0, 0.0],
                },
                ColorStop {
                    position: 0.17,
                    color: [1.0, 0.5, 0.0],
                },
                ColorStop {
                    position: 0.33,
                    color: [1.0, 1.0, 0.0],
                },
                ColorStop {
                    position: 0.5,
                    color: [0.0, 1.0, 0.0],
                },
                ColorStop {
                    position: 0.67,
                    color: [0.0, 0.5, 1.0],
                },
                ColorStop {
                    position: 0.83,
                    color: [0.5, 0.0, 1.0],
                },
                ColorStop {
                    position: 1.0,
                    color: [1.0, 0.0, 0.5],
                },
            ],
        )
    }

    /// Create a purple-pink palette
    pub fn purple_pink() -> Self {
        Self::new_builtin(
            "Purple Pink",
            vec![
                ColorStop {
                    position: 0.0,
                    color: [0.1, 0.0, 0.2],
                },
                ColorStop {
                    position: 0.33,
                    color: [0.4, 0.0, 0.6],
                },
                ColorStop {
                    position: 0.66,
                    color: [0.8, 0.2, 0.8],
                },
                ColorStop {
                    position: 1.0,
                    color: [1.0, 0.7, 1.0],
                },
            ],
        )
    }

    /// Generate a 1D texture data array (RGBA32Float format)
    /// size: number of samples in the 1D texture (e.g., 256, 512, 1024)
    pub fn generate_texture_data(&self, size: usize) -> Vec<f32> {
        let mut data = Vec::with_capacity(size * 4);

        for i in 0..size {
            let t = i as f32 / (size - 1) as f32;
            let color = self.sample_color(t);
            data.push(color[0]);
            data.push(color[1]);
            data.push(color[2]);
            data.push(1.0); // Alpha
        }

        data
    }

    /// Sample color at position t (0.0 to 1.0) using linear interpolation
    pub fn sample_color(&self, t: f32) -> [f32; 3] {
        if self.stops.is_empty() {
            return [1.0, 1.0, 1.0];
        }

        let t = t.clamp(0.0, 1.0);

        // Find the two stops to interpolate between
        let mut idx = 0;
        for (i, stop) in self.stops.iter().enumerate() {
            if stop.position > t {
                break;
            }
            idx = i;
        }

        // Handle edge cases
        if idx == self.stops.len() - 1 {
            return self.stops[idx].color;
        }

        let stop1 = &self.stops[idx];
        let stop2 = &self.stops[idx + 1];

        // Linear interpolation
        let range = stop2.position - stop1.position;
        if range < 1e-6 {
            return stop1.color;
        }

        let local_t = (t - stop1.position) / range;

        [
            stop1.color[0] + (stop2.color[0] - stop1.color[0]) * local_t,
            stop1.color[1] + (stop2.color[1] - stop1.color[1]) * local_t,
            stop1.color[2] + (stop2.color[2] - stop1.color[2]) * local_t,
        ]
    }

    /// Convert palette to fixed 256-color mode
    /// Samples the current gradient at 256 evenly-spaced positions
    pub fn convert_to_fixed(&mut self) {
        if self.locked {
            return; // Already in fixed mode
        }

        // Sample current gradient at 256 positions
        let mut new_stops = Vec::with_capacity(256);
        for i in 0..256 {
            let position = i as f32 / 255.0;
            let color = self.sample_color(position);
            new_stops.push(ColorStop { position, color });
        }

        self.stops = new_stops;
        self.locked = true;
    }

    /// Convert palette to free gradient mode
    /// Just unlocks the palette, doesn't change stops
    pub fn convert_to_free(&mut self) {
        self.locked = false;
    }

    /// Create a new locked 256-color palette
    pub fn new_locked(name: impl Into<String>, stops: Vec<ColorStop>) -> Self {
        Self {
            name: name.into(),
            stops,
            locked: true,
            built_in: false,
        }
    }
}

/// Collection of available palettes
pub struct PaletteLibrary {
    palettes: Vec<Palette>,
}

impl Default for PaletteLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl PaletteLibrary {
    pub fn new() -> Self {
        let mut palettes = vec![
            Palette::grayscale(),
        ];

        // Desktop: Load palettes from assets folder (copied to target/ by build.rs)
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut assets_palettes = super::assets::load_palettes_from_dir(
                std::path::Path::new("assets/palettes")
            );
            // Mark all asset palettes as built-in (shipped with the application)
            for pal in &mut assets_palettes {
                pal.built_in = true;
            }
            palettes.extend(assets_palettes);
        }

        // WASM or fallback: Use built-in palettes if no assets were loaded
        if palettes.len() == 1 {
            palettes.extend(vec![
                Palette::fire(),
                Palette::cool(),
                Palette::rainbow(),
                Palette::purple_pink(),
            ]);
        }

        Self { palettes }
    }

    pub fn palettes(&self) -> &[Palette] {
        &self.palettes
    }

    pub fn get(&self, index: usize) -> Option<&Palette> {
        self.palettes.get(index)
    }

    pub fn add(&mut self, palette: Palette) {
        self.palettes.push(palette);
    }

    pub fn update(&mut self, index: usize, palette: Palette) {
        if index < self.palettes.len() {
            self.palettes[index] = palette;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Palette> {
        self.palettes.iter()
    }

    pub fn len(&self) -> usize {
        self.palettes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_sampling() {
        let palette = Palette::grayscale();
        let black = palette.sample_color(0.0);
        let white = palette.sample_color(1.0);
        let gray = palette.sample_color(0.5);

        assert_eq!(black, [0.0, 0.0, 0.0]);
        assert_eq!(white, [1.0, 1.0, 1.0]);
        assert!((gray[0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_texture_generation() {
        let palette = Palette::fire();
        let data = palette.generate_texture_data(256);

        // Should have 256 * 4 floats (RGBA)
        assert_eq!(data.len(), 256 * 4);

        // First pixel should be blackish (start of fire)
        assert!(data[0] < 0.1); // R
        assert!(data[1] < 0.1); // G
        assert!(data[2] < 0.1); // B
        assert_eq!(data[3], 1.0); // A
    }
}
