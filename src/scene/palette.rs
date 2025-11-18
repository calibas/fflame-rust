use serde::{Deserialize, Serialize};

/// Color mode determines how colors are assigned during iteration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMode {
    /// Use 1D palette texture lookup (Apophysis color coordinate evolution)
    Palette,
    /// Use speed-based coloring (distance traveled per iteration)
    Speed,
}

impl Default for ColorMode {
    fn default() -> Self {
        Self::Palette
    }
}

/// A single color stop in a gradient palette
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ColorStop {
    pub position: f32, // 0.0 to 1.0
    pub color: [f32; 3], // RGB
}

/// Palette definition with gradient stops
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

    /// Find the closest palette position for a given RGB color
    /// Returns position in range 0.0-1.0
    ///
    /// Uses brute force search by sampling the palette at regular intervals
    /// and finding the position with minimum Euclidean distance in RGB space.
    pub fn find_position(&self, target_rgb: [f32; 3]) -> f32 {
        const SAMPLES: usize = 256;

        let mut best_position = 0.5;
        let mut best_distance = f32::MAX;

        for i in 0..SAMPLES {
            let position = i as f32 / (SAMPLES - 1) as f32;
            let color = self.sample_color(position);

            // Calculate Euclidean distance in RGB space
            let dr = color[0] - target_rgb[0];
            let dg = color[1] - target_rgb[1];
            let db = color[2] - target_rgb[2];
            let distance = (dr * dr + dg * dg + db * db).sqrt();

            if distance < best_distance {
                best_distance = distance;
                best_position = position;
            }
        }

        best_position
    }
}

/// Collection of available palettes organized into packs
pub struct PaletteLibrary {
    /// Flat list of all palettes (backward compatibility)
    palettes: Vec<Palette>,
    /// Palette packs (new system)
    packs: Vec<PalettePack>,
    /// Runtime enabled state for each pack
    enabled_packs: Vec<bool>,
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

        // Load packs from assets/palettes/packs/
        let mut packs = Vec::new();
        let mut enabled_packs = Vec::new();

        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::fs;
            use std::path::Path;

            // Load old individual palette files for backward compatibility
            let mut assets_palettes = super::assets::load_palettes_from_dir(
                Path::new("assets/palettes")
            );
            // Mark all asset palettes as built-in (shipped with the application)
            for pal in &mut assets_palettes {
                pal.built_in = true;
            }
            palettes.extend(assets_palettes);

            // Load new palette packs
            let packs_dir = Path::new("assets/palettes/packs");
            if packs_dir.exists() {
                if let Ok(entries) = fs::read_dir(packs_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("json") {
                            match fs::read_to_string(&path) {
                                Ok(content) => {
                                    match serde_json::from_str::<PalettePack>(&content) {
                                        Ok(pack) => {
                                            log::info!("Loaded palette pack: {} ({} palettes)",
                                                pack.pack_name, pack.palettes.len());
                                            let enabled = pack.enabled_by_default;
                                            packs.push(pack);
                                            enabled_packs.push(enabled);
                                        }
                                        Err(e) => {
                                            log::error!("Failed to parse palette pack {:?}: {}", path, e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to read palette pack {:?}: {}", path, e);
                                }
                            }
                        }
                    }
                }
            }
        }

        // WASM or fallback: Use built-in palettes if no assets were loaded
        if palettes.len() == 1 && packs.is_empty() {
            palettes.extend(vec![
                Palette::fire(),
                Palette::cool(),
                Palette::rainbow(),
                Palette::purple_pink(),
            ]);
        }

        // Add all enabled pack palettes to the main palette list
        // This ensures they appear in the Colors panel dropdown
        // Mark all pack palettes as built-in (shipped with the application)
        for (pack_idx, pack) in packs.iter().enumerate() {
            if enabled_packs.get(pack_idx).copied().unwrap_or(false) {
                for palette in &pack.palettes {
                    let mut pal = palette.clone();
                    pal.built_in = true; // Pack palettes are shipped assets
                    palettes.push(pal);
                }
            }
        }

        Self {
            palettes,
            packs,
            enabled_packs,
        }
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

    /// Add palette if name doesn't exist, otherwise update existing
    /// Returns the index of the palette (existing or newly added)
    pub fn add_or_update(&mut self, palette: Palette) -> usize {
        // Search for existing palette with same name
        for (i, lib_palette) in self.palettes.iter_mut().enumerate() {
            if lib_palette.name == palette.name {
                // Update existing
                *lib_palette = palette;
                return i;
            }
        }
        // Add new
        self.palettes.push(palette);
        self.palettes.len() - 1
    }

    pub fn iter(&self) -> impl Iterator<Item = &Palette> {
        self.palettes.iter()
    }

    pub fn len(&self) -> usize {
        self.palettes.len()
    }

    // ===== PACK-RELATED METHODS =====

    /// Get number of packs
    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }

    /// Get pack at index
    pub fn get_pack(&self, index: usize) -> Option<&PalettePack> {
        self.packs.get(index)
    }

    /// Check if pack is enabled
    pub fn is_pack_enabled(&self, index: usize) -> bool {
        self.enabled_packs.get(index).copied().unwrap_or(false)
    }

    /// Toggle pack enabled state and rebuild palette list
    pub fn set_pack_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(state) = self.enabled_packs.get_mut(index) {
            *state = enabled;
            self.rebuild_palette_list();
        }
    }

    /// Rebuild the main palette list from packs
    /// Called when packs are enabled/disabled
    fn rebuild_palette_list(&mut self) {
        // Keep only non-pack palettes:
        // - Hardcoded built-ins (grayscale, fire, etc.)
        // - Legacy assets/palettes/*.palette files
        // - User-created/imported palettes
        // Remove pack palettes (we'll re-add enabled ones)
        let pack_names: std::collections::HashSet<String> = self.packs
            .iter()
            .flat_map(|pack| pack.palettes.iter().map(|p| p.name.clone()))
            .collect();

        self.palettes.retain(|p| !pack_names.contains(&p.name));

        // Add all enabled pack palettes
        for (pack_idx, pack) in self.packs.iter().enumerate() {
            if self.enabled_packs.get(pack_idx).copied().unwrap_or(false) {
                for palette in &pack.palettes {
                    let mut pal = palette.clone();
                    pal.built_in = true; // Pack palettes are shipped assets
                    self.palettes.push(pal);
                }
            }
        }
    }

    /// Generate preview image for a palette
    /// Returns ColorImage suitable for egui texture rendering
    pub fn generate_preview(palette: &Palette, width: usize, height: usize) -> egui::ColorImage {
        let mut pixels = vec![egui::Color32::BLACK; width * height];

        for x in 0..width {
            let t = x as f32 / (width - 1).max(1) as f32;
            let color = palette.sample_color(t);
            let color32 = egui::Color32::from_rgb(
                (color[0] * 255.0) as u8,
                (color[1] * 255.0) as u8,
                (color[2] * 255.0) as u8,
            );

            // Fill vertical column
            for y in 0..height {
                pixels[y * width + x] = color32;
            }
        }

        egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels.iter()
            .flat_map(|c| [c.r(), c.g(), c.b(), c.a()])
            .collect::<Vec<u8>>())
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

    #[test]
    fn test_find_position_exact_stops() {
        let palette = Palette::rainbow();

        // Test exact stop colors - should find exact or very close position
        let red_pos = palette.find_position([1.0, 0.0, 0.0]);
        assert!((red_pos - 0.0).abs() < 0.01, "Red should be at 0.0, got {}", red_pos);

        let yellow_pos = palette.find_position([1.0, 1.0, 0.0]);
        assert!((yellow_pos - 0.33).abs() < 0.05, "Yellow should be near 0.33, got {}", yellow_pos);

        let green_pos = palette.find_position([0.0, 1.0, 0.0]);
        assert!((green_pos - 0.5).abs() < 0.01, "Green should be at 0.5, got {}", green_pos);
    }

    #[test]
    fn test_find_position_interpolated() {
        let palette = Palette::grayscale();

        // Test interpolated colors
        let gray = [0.5, 0.5, 0.5];
        let pos = palette.find_position(gray);
        assert!((pos - 0.5).abs() < 0.05, "Gray should be near 0.5, got {}", pos);

        let dark_gray = [0.25, 0.25, 0.25];
        let pos_dark = palette.find_position(dark_gray);
        assert!(pos_dark < 0.5, "Dark gray should be < 0.5, got {}", pos_dark);

        let light_gray = [0.75, 0.75, 0.75];
        let pos_light = palette.find_position(light_gray);
        assert!(pos_light > 0.5, "Light gray should be > 0.5, got {}", pos_light);
    }

    #[test]
    fn test_find_position_boundaries() {
        let palette = Palette::fire();

        // Black (start)
        let black_pos = palette.find_position([0.0, 0.0, 0.0]);
        assert!((black_pos - 0.0).abs() < 0.01, "Black should be at 0.0, got {}", black_pos);

        // Whitish (end)
        let white_pos = palette.find_position([1.0, 1.0, 0.8]);
        assert!(white_pos > 0.9, "Light color should be near 1.0, got {}", white_pos);
    }

    #[test]
    fn test_find_position_roundtrip() {
        let palette = Palette::cool();

        // Test roundtrip: position -> color -> position
        let original_pos = 0.42;
        let color = palette.sample_color(original_pos);
        let found_pos = palette.find_position(color);

        // Should be close (within sampling resolution)
        assert!((found_pos - original_pos).abs() < 0.01,
            "Roundtrip failed: {} -> {:?} -> {}", original_pos, color, found_pos);
    }
}

// ===== PALETTE PACK SYSTEM =====

/// A pack of related palettes loaded from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PalettePack {
    pub pack_name: String,
    pub description: String,
    #[serde(default)]
    pub enabled_by_default: bool,
    pub palettes: Vec<Palette>,
}
