use serde::{Deserialize, Serialize};

/// Color mode determines how colors are assigned during iteration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMode {
    /// Use 1D palette texture lookup (Apophysis color coordinate evolution)
    Palette,
    /// Use speed-based coloring (distance traveled per iteration)
    Speed,
    /// Use transform path history as color (IFS tree visualization)
    PathMap,
}

impl Default for ColorMode {
    fn default() -> Self {
        Self::Palette
    }
}

/// PathMap capture mode - when to capture the path for each pixel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PathCaptureMode {
    /// Capture on first hit to the pixel (original behavior)
    #[default]
    FirstHit,
    /// Capture first hit after burn-in iterations complete
    FirstAfterBurnIn,
    /// Always overwrite - shows most recent path to hit pixel
    LastHit,
}

/// PathMap tracking mode - which iterations to store in the path
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PathTrackingMode {
    /// Store the first 32 iterations, then stop tracking
    #[default]
    First,
    /// Store the 32 most recent iterations (rolling window)
    Recent,
}

/// PathMap coloring style - how path hash maps to colors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PathMapStyle {
    /// Color by path beginning (first ~8 transforms), similar paths = similar colors
    #[default]
    Prefix,
    /// Color by path end (recent transforms), similar paths = similar colors
    Suffix,
    /// Color by path beginning with hash scrambling for distinct colors
    PrefixDistinct,
    /// Color by path end with hash scrambling for distinct colors
    SuffixDistinct,
    /// Color by iteration depth (burn_in to 32), uses palette gradient
    Depth,
    /// Color by distance from origin (0 to sqrt(2)), uses palette gradient
    OriginRadial,
    /// Color by horizontal position (-1 to 1), uses palette gradient
    OriginHorizontal,
    /// Color by vertical position (-1 to 1), uses palette gradient
    OriginVertical,
}

/// A single color stop in a gradient palette
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ColorStop {
    pub position: f32, // 0.0 to 1.0
    pub color: [f32; 3], // RGB
}

/// Palette definition with gradient stops
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub name: String,
    pub stops: Vec<ColorStop>,
    /// If true, palette is in fixed 256-color mode (positions locked)
    pub locked: bool,
    /// If true, palette is built-in and should not be edited directly (create copy instead)
    /// This flag is ONLY set at runtime in PaletteLibrary, never serialized
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

    /// Check if palette is in indexed 256-color mode (for compact serialization)
    fn is_indexed_256(&self) -> bool {
        if self.stops.len() != 256 {
            return false;
        }

        // Check if all stops are at exact positions i/255.0
        self.stops.iter().enumerate().all(|(i, stop)| {
            (stop.position - i as f32 / 255.0).abs() < 0.001
        })
    }

    /// Convert stops to compact hex string format (for indexed palettes)
    fn to_hex_string(&self) -> String {
        self.stops.iter()
            .map(|stop| format!("{:02X}{:02X}{:02X}",
                (stop.color[0] * 255.0).round() as u8,
                (stop.color[1] * 255.0).round() as u8,
                (stop.color[2] * 255.0).round() as u8))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Create palette from compact hex string format
    pub fn from_hex_string(name: String, hex_colors: &str, locked: bool) -> Result<Self, String> {
        let stops: Result<Vec<ColorStop>, String> = hex_colors.split(',')
            .enumerate()
            .map(|(i, hex)| {
                if hex.len() != 6 {
                    return Err(format!("Invalid hex color '{}' at index {}", hex, i));
                }

                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("Invalid red component in '{}'", hex))? as f32 / 255.0;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("Invalid green component in '{}'", hex))? as f32 / 255.0;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("Invalid blue component in '{}'", hex))? as f32 / 255.0;

                Ok(ColorStop {
                    position: i as f32 / 255.0,
                    color: [r, g, b],
                })
            })
            .collect();

        Ok(Palette {
            name,
            stops: stops?,
            locked,
            built_in: false,
        })
    }
}

// Custom serialization/deserialization for compact indexed palette format
impl serde::Serialize for Palette {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        // Check if this is an indexed 256-color palette
        if self.is_indexed_256() {
            // Serialize in compact hex format
            let mut state = serializer.serialize_struct("Palette", 3)?;
            state.serialize_field("name", &self.name)?;
            state.serialize_field("indexed_colors", &self.to_hex_string())?;
            state.serialize_field("locked", &self.locked)?;
            state.end()
        } else {
            // Serialize as gradient with stops (legacy format)
            let mut state = serializer.serialize_struct("Palette", 3)?;
            state.serialize_field("name", &self.name)?;
            state.serialize_field("stops", &self.stops)?;
            state.serialize_field("locked", &self.locked)?;
            state.end()
        }
    }
}

impl<'de> serde::Deserialize<'de> for Palette {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct PaletteVisitor;

        impl<'de> Visitor<'de> for PaletteVisitor {
            type Value = Palette;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a palette with either 'indexed_colors' or 'stops'")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut name: Option<String> = None;
                let mut indexed_colors: Option<String> = None;
                let mut stops: Option<Vec<ColorStop>> = None;
                let mut locked: Option<bool> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "name" => {
                            name = Some(map.next_value()?);
                        }
                        "indexed_colors" => {
                            indexed_colors = Some(map.next_value()?);
                        }
                        "stops" => {
                            stops = Some(map.next_value()?);
                        }
                        "locked" => {
                            locked = Some(map.next_value()?);
                        }
                        _ => {
                            // Skip unknown fields
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                let locked = locked.unwrap_or(false);

                // Try compact format first, fallback to legacy stops format
                if let Some(hex_string) = indexed_colors {
                    Palette::from_hex_string(name, &hex_string, locked)
                        .map_err(de::Error::custom)
                } else if let Some(stops) = stops {
                    Ok(Palette {
                        name,
                        stops,
                        locked,
                        built_in: false,
                    })
                } else {
                    Err(de::Error::missing_field("indexed_colors or stops"))
                }
            }
        }

        deserializer.deserialize_struct("Palette", &["name", "indexed_colors", "stops", "locked"], PaletteVisitor)
    }
}

/// Collection of available palettes organized into packs
#[derive(Clone)]
pub struct PaletteLibrary {
    /// Flat list of all palettes (backward compatibility)
    palettes: Vec<Palette>,
    /// Palette packs with load state tracking
    packs: Vec<crate::resources::PalettePackInfo>,
    /// Index of the Custom pack (user-saved palettes)
    custom_pack_index: Option<usize>,
    /// Generation counter - increments when custom library is modified (for cache invalidation)
    generation: u32,
}

impl Default for PaletteLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl PaletteLibrary {
    pub fn new() -> Self {
        use crate::resources::PalettePackInfo;

        let mut packs: Vec<PalettePackInfo> = Vec::new();

        // Always load the embedded built-in pack first (offline fallback)
        match crate::resources::palettes::load_builtin_pack() {
            Ok(pack) => {
                log::info!("Loaded embedded Built-in pack ({} palettes)", pack.palettes.len());
                packs.push(PalettePackInfo::from_pack(pack, true));
            }
            Err(e) => {
                log::error!("Failed to parse embedded Built-in pack: {}", e);
            }
        }

        // Load manifest to discover available packs (lazy loading)
        #[cfg(not(target_arch = "wasm32"))]
        {
            match crate::resources::palettes::load_manifest() {
                Ok(manifest) => {
                    log::info!("Loaded palette manifest with {} packs", manifest.packs.len());
                    for pack_meta in manifest.packs {
                        // Skip builtin - already loaded embedded version
                        if pack_meta.id == "builtin" {
                            continue;
                        }
                        log::info!("  - {} ({} palettes, enabled: {})",
                            pack_meta.name, pack_meta.item_count, pack_meta.enabled_by_default);
                        packs.push(PalettePackInfo::from_metadata(pack_meta));
                    }
                }
                Err(e) => {
                    log::warn!("Failed to load palette manifest: {}", e);
                    log::info!("Falling back to filesystem scan...");

                    // Fallback: scan filesystem directly
                    use std::fs;
                    use std::path::Path;

                    let packs_dir = Path::new("assets/palettes/packs");
                    if packs_dir.exists() {
                        if let Ok(entries) = fs::read_dir(packs_dir) {
                            for entry in entries.flatten() {
                                let path = entry.path();
                                let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

                                // Skip manifest and builtin (already loaded)
                                if filename == "manifest.json" || filename == "builtin.json" {
                                    continue;
                                }

                                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                                    match fs::read_to_string(&path) {
                                        Ok(content) => {
                                            match serde_json::from_str::<PalettePack>(&content) {
                                                Ok(pack) => {
                                                    log::info!("Loaded palette pack: {} ({} palettes)",
                                                        pack.pack_name, pack.palettes.len());
                                                    packs.push(PalettePackInfo::from_pack(pack, false));
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
            }
        }

        // WASM: Only use embedded builtin pack, manifest packs loaded on-demand
        #[cfg(target_arch = "wasm32")]
        {
            // TODO: Could spawn async manifest fetch here for future enhancement
            log::info!("WASM: Using embedded builtin pack only, other packs loaded on demand");
        }

        // Create library instance with empty palette list
        let mut library = Self {
            palettes: Vec::new(),
            packs,
            custom_pack_index: None,
            generation: 0,
        };

        // Add Grayscale as fallback (always first)
        library.add_or_update(Palette::grayscale());

        // Fallback: If no packs loaded, use hardcoded palettes
        if library.packs.is_empty() {
            library.add_or_update(Palette::fire());
            library.add_or_update(Palette::cool());
            library.add_or_update(Palette::rainbow());
            library.add_or_update(Palette::purple_pink());
        }

        // Load user's custom palettes from storage (as a Custom pack)
        library.load_custom_pack();

        // Build flat palette list from enabled packs (for random flame, etc.)
        library.rebuild_palette_list();

        library
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

    /// Add palette if name doesn't exist (case-insensitive), otherwise skip with warning
    /// Returns the index of the palette (existing or newly added)
    pub fn add_or_update(&mut self, palette: Palette) -> usize {
        // Search for existing palette with same name (case-insensitive)
        for (i, lib_palette) in self.palettes.iter().enumerate() {
            if lib_palette.name.to_lowercase() == palette.name.to_lowercase() {
                // Duplicate found - skip and warn
                log::warn!("Skipping duplicate palette '{}' (already exists as '{}')",
                    palette.name, lib_palette.name);
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

    /// Get pack at index (returns the inner PalettePack if loaded)
    pub fn get_pack(&self, index: usize) -> Option<&PalettePack> {
        self.packs.get(index).and_then(|info| info.pack.as_ref())
    }

    /// Get pack info at index (includes load state)
    pub fn get_pack_info(&self, index: usize) -> Option<&crate::resources::PalettePackInfo> {
        self.packs.get(index)
    }

    /// Get mutable access to packs (for retry state reset)
    pub fn packs_mut(&mut self) -> &mut Vec<crate::resources::PalettePackInfo> {
        &mut self.packs
    }

    /// Check if pack is enabled
    pub fn is_pack_enabled(&self, index: usize) -> bool {
        self.packs.get(index).map(|info| info.enabled).unwrap_or(false)
    }

    /// Toggle pack enabled state and rebuild palette list
    pub fn set_pack_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(info) = self.packs.get_mut(index) {
            info.enabled = enabled;
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
            .filter_map(|info| info.pack.as_ref())
            .flat_map(|pack| pack.palettes.iter().map(|p| p.name.clone()))
            .collect();

        self.palettes.retain(|p| !pack_names.contains(&p.name));

        // Add all enabled pack palettes (use add_or_update to prevent duplicates)
        // Collect first to avoid borrow checker issues
        // Include pack index to know if it's the Custom pack
        let palettes_to_add: Vec<(usize, Palette)> = self.packs.iter().enumerate()
            .filter(|(_, info)| info.enabled && info.pack.is_some())
            .flat_map(|(pack_idx, info)| {
                info.pack.as_ref().unwrap().palettes.iter().map(move |p| (pack_idx, p.clone()))
            })
            .collect();

        for (pack_idx, palette) in palettes_to_add {
            let mut pal = palette;
            // Custom pack palettes are user-saved, not built-in
            pal.built_in = self.custom_pack_index != Some(pack_idx);
            self.add_or_update(pal);
        }
    }

    // ===== CUSTOM PALETTE METHODS =====

    /// Load custom palettes from storage and create the Custom pack
    fn load_custom_pack(&mut self) {
        use crate::storage::CustomPaletteLibrary;
        use crate::resources::PalettePackInfo;

        let custom_lib = CustomPaletteLibrary::load();

        // Create the Custom pack (even if empty)
        let custom_pack = PalettePack {
            pack_name: "Custom".to_string(),
            description: "Your saved palettes".to_string(),
            enabled_by_default: true,
            palettes: custom_lib.palettes,
        };

        let palette_count = custom_pack.palettes.len();

        // Add as first pack (so it appears at the top)
        self.packs.insert(0, PalettePackInfo::from_pack(custom_pack, false));
        self.custom_pack_index = Some(0);

        if palette_count > 0 {
            log::info!("Loaded Custom pack with {} palette(s)", palette_count);
        }
    }

    /// Save a palette to the Custom library (persists across sessions)
    /// If a palette with the same name exists, it will be overwritten
    /// Returns Ok(()) on success, Err with message on failure
    pub fn save_to_custom_library(&mut self, palette: Palette) -> Result<(), String> {
        use crate::storage::CustomPaletteLibrary;

        // Load current custom palettes from storage
        let mut custom_lib = CustomPaletteLibrary::load();

        // Prepare the palette
        let mut palette = palette;
        palette.built_in = false; // Ensure it's editable

        // Check if this is an overwrite
        let is_overwrite = custom_lib.has_palette_by_name(&palette.name);

        // Add (or overwrite) the palette and save
        custom_lib.add_palette(palette.clone())
            .map_err(|e| format!("Failed to save: {}", e))?;

        // Update the Custom pack in memory
        if let Some(idx) = self.custom_pack_index {
            if let Some(info) = self.packs.get_mut(idx) {
                if let Some(pack) = info.pack.as_mut() {
                    if is_overwrite {
                        // Find and replace existing palette
                        let name_lower = palette.name.to_lowercase();
                        if let Some(pos) = pack.palettes.iter().position(|p| p.name.to_lowercase() == name_lower) {
                            pack.palettes[pos] = palette;
                        }
                    } else {
                        pack.palettes.push(palette);
                    }
                }
            }
        }

        // Rebuild palette list
        self.rebuild_palette_list();

        // Increment generation for cache invalidation
        self.generation = self.generation.wrapping_add(1);

        Ok(())
    }

    /// Check if a palette with the given name exists in the Custom pack
    pub fn has_custom_palette_named(&self, name: &str) -> bool {
        use crate::storage::CustomPaletteLibrary;
        let custom_lib = CustomPaletteLibrary::load();
        custom_lib.has_palette_by_name(name)
    }

    /// Delete a palette from the Custom library by name
    /// Returns Ok(true) if deleted, Ok(false) if not found
    pub fn delete_from_custom_library(&mut self, name: &str) -> Result<bool, String> {
        use crate::storage::CustomPaletteLibrary;

        // Load and delete from storage
        let mut custom_lib = CustomPaletteLibrary::load();
        let deleted = custom_lib.delete_palette_by_name(name)
            .map_err(|e| format!("Failed to delete: {}", e))?;

        if deleted {
            // Update in-memory Custom pack
            if let Some(idx) = self.custom_pack_index {
                if let Some(info) = self.packs.get_mut(idx) {
                    if let Some(pack) = info.pack.as_mut() {
                        let name_lower = name.to_lowercase();
                        pack.palettes.retain(|p| p.name.to_lowercase() != name_lower);
                    }
                }
            }

            // Rebuild palette list
            self.rebuild_palette_list();

            // Increment generation for cache invalidation
            self.generation = self.generation.wrapping_add(1);
        }

        Ok(deleted)
    }

    /// Check if a Custom pack exists and has palettes
    pub fn has_custom_palettes(&self) -> bool {
        self.custom_pack_index
            .and_then(|idx| self.packs.get(idx))
            .and_then(|info| info.pack.as_ref())
            .map(|pack| !pack.palettes.is_empty())
            .unwrap_or(false)
    }

    /// Get the index of the Custom pack (if it exists)
    pub fn custom_pack_index(&self) -> Option<usize> {
        self.custom_pack_index
    }

    /// Get generation counter (for cache invalidation)
    pub fn generation(&self) -> u32 {
        self.generation
    }

    // ===== PACK LOADING METHODS =====

    /// Start loading a pack by index (desktop - synchronous)
    ///
    /// Returns true if pack was loaded successfully, false if already loaded or failed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn start_pack_load(&mut self, pack_idx: usize) -> bool {
        use crate::resources::LoadState;

        let info = match self.packs.get_mut(pack_idx) {
            Some(info) => info,
            None => return false,
        };

        // Skip if already loaded or loading
        if info.is_loaded() || info.is_loading() {
            return false;
        }

        // Get file path from metadata
        let file_path = match &info.metadata {
            Some(meta) => meta.file.clone(),
            None => {
                log::warn!("Cannot load pack {}: no metadata", pack_idx);
                return false;
            }
        };

        // Mark as loading
        info.load_state = LoadState::Loading;

        // Synchronous load on desktop
        log::info!("Loading palette pack: {}", file_path);
        match crate::resources::palettes::load_pack(&file_path) {
            Ok(pack) => {
                log::info!("Loaded pack {} with {} palettes", pack.pack_name, pack.palettes.len());
                if let Some(info) = self.packs.get_mut(pack_idx) {
                    info.pack = Some(pack);
                    info.load_state = LoadState::Loaded;
                }
                // Rebuild palette list if pack is enabled
                if self.packs.get(pack_idx).map(|i| i.enabled).unwrap_or(false) {
                    self.rebuild_palette_list();
                }
                true
            }
            Err(e) => {
                log::error!("Failed to load pack {}: {}", file_path, e);
                if let Some(info) = self.packs.get_mut(pack_idx) {
                    info.load_state = LoadState::Failed(e.to_string());
                }
                false
            }
        }
    }

    /// Start loading a pack by index (WASM - marks as loading, returns file path for async fetch)
    ///
    /// Returns Some(file_path) if pack should be fetched, None if already loaded or no metadata.
    #[cfg(target_arch = "wasm32")]
    pub fn start_pack_load(&mut self, pack_idx: usize) -> Option<String> {
        use crate::resources::LoadState;

        let info = match self.packs.get_mut(pack_idx) {
            Some(info) => info,
            None => return None,
        };

        // Skip if already loaded or loading
        if info.is_loaded() || info.is_loading() {
            return None;
        }

        // Get file path from metadata
        let file_path = match &info.metadata {
            Some(meta) => meta.file.clone(),
            None => {
                log::warn!("Cannot load pack {}: no metadata", pack_idx);
                return None;
            }
        };

        // Mark as loading
        info.load_state = LoadState::Loading;
        log::info!("Starting fetch for palette pack: {}", file_path);

        Some(file_path)
    }

    /// Set pack as loaded (for async completion on WASM)
    pub fn set_pack_loaded(&mut self, pack_idx: usize, pack: PalettePack) {
        use crate::resources::LoadState;

        if let Some(info) = self.packs.get_mut(pack_idx) {
            log::info!("Pack {} loaded with {} palettes", pack.pack_name, pack.palettes.len());
            info.pack = Some(pack);
            info.load_state = LoadState::Loaded;

            // Rebuild palette list if pack is enabled
            if info.enabled {
                self.rebuild_palette_list();
            }
        }
    }

    /// Set pack as failed (for async completion on WASM)
    pub fn set_pack_failed(&mut self, pack_idx: usize, error: String) {
        use crate::resources::LoadState;

        if let Some(info) = self.packs.get_mut(pack_idx) {
            log::error!("Pack {} failed to load: {}", info.name(), error);
            info.load_state = LoadState::Failed(error);
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

    #[test]
    fn test_compact_serialization_indexed() {
        // Create an indexed 256-color palette
        let mut stops = Vec::with_capacity(256);
        for i in 0..256 {
            stops.push(ColorStop {
                position: i as f32 / 255.0,
                color: [
                    (i as f32 / 255.0),
                    ((255 - i) as f32 / 255.0),
                    0.5,
                ],
            });
        }

        let palette = Palette {
            name: "Test Indexed".to_string(),
            stops,
            locked: true,
            built_in: false,
        };

        // Serialize to JSON
        let json = serde_json::to_string(&palette).unwrap();

        // Should use compact format (contains "indexed_colors")
        assert!(json.contains("indexed_colors"), "Should use compact format");
        assert!(!json.contains("stops"), "Should not contain 'stops' field");

        // Deserialize back
        let loaded: Palette = serde_json::from_str(&json).unwrap();

        // Should match original
        assert_eq!(loaded.name, palette.name);
        assert_eq!(loaded.stops.len(), 256);
        assert_eq!(loaded.locked, true);

        // Check a few color values (with rounding tolerance)
        for i in [0, 64, 128, 192, 255] {
            let orig_color = palette.stops[i].color;
            let loaded_color = loaded.stops[i].color;
            for c in 0..3 {
                assert!((orig_color[c] - loaded_color[c]).abs() < 0.01,
                    "Color mismatch at index {} component {}: {} vs {}",
                    i, c, orig_color[c], loaded_color[c]);
            }
        }
    }

    #[test]
    fn test_compact_serialization_gradient() {
        // Create a gradient palette (not indexed)
        let palette = Palette::fire();

        // Serialize to JSON
        let json = serde_json::to_string(&palette).unwrap();

        // Should use legacy format (contains "stops")
        assert!(json.contains("stops"), "Should use legacy format with stops");
        assert!(!json.contains("indexed_colors"), "Should not use compact format");

        // Deserialize back
        let loaded: Palette = serde_json::from_str(&json).unwrap();

        // Should match original
        assert_eq!(loaded.name, palette.name);
        assert_eq!(loaded.stops.len(), palette.stops.len());
    }

    #[test]
    fn test_backward_compatibility_legacy_format() {
        // Test that old JSON format still loads correctly
        let legacy_json = r#"{
            "name": "Legacy Palette",
            "stops": [
                {"position": 0.0, "color": [1.0, 0.0, 0.0]},
                {"position": 0.5, "color": [0.0, 1.0, 0.0]},
                {"position": 1.0, "color": [0.0, 0.0, 1.0]}
            ],
            "locked": false
        }"#;

        let loaded: Palette = serde_json::from_str(legacy_json).unwrap();

        assert_eq!(loaded.name, "Legacy Palette");
        assert_eq!(loaded.stops.len(), 3);
        assert_eq!(loaded.locked, false);
        assert_eq!(loaded.stops[0].color, [1.0, 0.0, 0.0]);
        assert_eq!(loaded.stops[2].color, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_compact_format_size() {
        // Create indexed palette
        let mut stops = Vec::with_capacity(256);
        for i in 0..256 {
            stops.push(ColorStop {
                position: i as f32 / 255.0,
                color: [(i as f32 / 255.0), 0.5, 0.5],
            });
        }

        let palette = Palette {
            name: "Size Test".to_string(),
            stops,
            locked: true,
            built_in: false,
        };

        let compact_json = serde_json::to_string(&palette).unwrap();

        // Create same palette but force legacy format by making it non-indexed
        let mut legacy_palette = palette.clone();
        legacy_palette.stops[100].position = 0.5; // Break indexing

        let legacy_json = serde_json::to_string(&legacy_palette).unwrap();

        // Compact should be much smaller
        println!("Compact size: {} bytes", compact_json.len());
        println!("Legacy size: {} bytes", legacy_json.len());
        assert!(compact_json.len() < legacy_json.len() / 2,
            "Compact format should be at least 50% smaller");
    }
}

// ===== GLOBAL SINGLETON =====

use once_cell::sync::Lazy;
use std::sync::RwLock;

/// Global palette library singleton (mutable via RwLock)
///
/// This provides a single shared instance of PaletteLibrary for the entire application.
/// Use RwLock because PaletteLibrary needs to be mutable (add custom palettes, enable/disable packs).
///
/// # Usage
/// ```ignore
/// // Read access (most common)
/// let library = global_palette_library().read().unwrap();
/// let palette = library.get(index);
///
/// // Write access (rare - adding custom palette)
/// let mut library = global_palette_library().write().unwrap();
/// library.add(custom_palette);
/// ```
pub fn global_palette_library() -> &'static RwLock<PaletteLibrary> {
    static LIBRARY: Lazy<RwLock<PaletteLibrary>> = Lazy::new(|| {
        RwLock::new(PaletteLibrary::new())
    });
    &LIBRARY
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
