use serde::{Deserialize, Serialize};
use crate::scene::transforms::Flame;
use crate::scene::palette::{ColorMode, Palette, PathCaptureMode, PathMapStyle, PathTrackingMode};
use crate::scene::tonemap::{HighlightMode, ToneMapMode, ToneCurve};
use crate::effects::EffectInstance;

/// Current config format version
pub const CURRENT_CONFIG_VERSION: u32 = 1;

/// Complete fractal configuration (excludes runtime-only settings)
/// All fields except `flame` have defaults for compact serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FractalConfig {
    /// The flame (transforms) - always required.
    /// Subflames (referenced by `subflame_wf` variations) live on the
    /// `Flame` struct itself, not here — see `Flame::subflames`.
    pub flame: Flame,

    /// View settings
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    #[serde(default)]
    pub pan_x: f32,
    #[serde(default)]
    pub pan_y: f32,
    #[serde(default)]
    pub rotation: f32,  // 2D rotation (around Z axis)

    /// 3D Camera rotation (for 3D mode)
    #[serde(default)]
    pub camera_rotation_x: f32,  // Pitch (rotation around X axis)
    #[serde(default)]
    pub camera_rotation_y: f32,  // Yaw (rotation around Y axis)
    #[serde(default)]
    pub camera_z: f32,  // Camera Z position (height)

    /// Depth of Field settings (3D mode)
    #[serde(default = "default_dof_focus_distance")]
    pub dof_focus_distance: f32,  // Distance from origin where image is sharpest
    #[serde(default)]
    pub dof_blur_strength: f32,  // Blur amount (0.0 = disabled)

    /// Depth Fog settings (3D mode - atmospheric perspective)
    #[serde(default)]
    pub fog_strength: f32,  // Exponential fog density (0.0 = disabled)
    #[serde(default)]
    pub fog_start: f32,  // Depth where fog begins

    /// Rendering settings
    #[serde(default = "default_density_scale")]
    pub density_scale: f32,
    #[serde(default = "default_speed_factor")]
    pub speed_factor: f32,
    /// Maximum total iterations to render (default: 1 billion = ~infinite for interactive use)
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u64,
    /// Histogram color scale (precision vs overflow protection, default: 10.0)
    #[serde(default = "default_histogram_color_scale")]
    pub histogram_color_scale: f32,
    /// Blend factor for accumulation (0.01 = slow/smooth, 1.0 = fast/flickery, default: 0.1)
    #[serde(default = "default_blend_factor")]
    pub blend_factor: f32,
    /// Use dynamic blend (true = exponential convergence, false = fixed blend rate, default: true)
    #[serde(default = "default_use_dynamic_blend")]
    pub use_dynamic_blend: bool,
    /// Per-pixel iteration limit (0 = disabled, default: 0)
    #[serde(default)]
    pub target_iterations_per_pixel: u32,

    /// Color settings
    #[serde(default)]
    pub color_mode: ColorMode,
    /// PathMap coloring style (Prefix = color by path start, Suffix = color by path end)
    #[serde(default, skip_serializing_if = "PathMapStyle::is_default")]
    pub path_map_style: PathMapStyle,
    /// PathMap capture mode (FirstHit, FirstAfterBurnIn, LastHit)
    #[serde(default, skip_serializing_if = "PathCaptureMode::is_default")]
    pub path_capture_mode: PathCaptureMode,
    /// PathMap tracking mode (First = first 32 iterations, Recent = rolling window of 32 most recent)
    #[serde(default, skip_serializing_if = "PathTrackingMode::is_default")]
    pub path_tracking_mode: PathTrackingMode,
    /// The palette data - always present (required)
    /// This is the single source of truth for the active palette
    #[serde(default = "default_palette", deserialize_with = "deserialize_palette")]
    pub palette: Palette,
    /// Palette rotation: -1.0 to 1.0, shifts palette indices (Apophysis: -128 to 128)
    #[serde(default = "default_palette_rotation")]
    pub palette_rotation: f32,
    /// Palette texture size: 256-4096, higher values give smoother gradients
    #[serde(default = "default_palette_size")]
    pub palette_size: u32,
    /// Palette squeeze (linear-mode meaning): 1.0 = no change, >1 = repeat
    /// palette N times, <1 = show only N% of palette
    #[serde(default = "default_palette_squeeze")]
    pub palette_squeeze: f32,
    /// Which squeeze algorithm to use: `Linear` (the existing behavior,
    /// uniform N repeats) or `Geometric` (octave-based packing with the
    /// ratio held in `palette_squeeze_falloff`).
    #[serde(default, skip_serializing_if = "is_default_palette_squeeze_mode")]
    pub palette_squeeze_mode: crate::scene::palette::SqueezeMode,
    /// Geometric squeeze ratio. Only consulted when `palette_squeeze_mode`
    /// is `Geometric`. Typical values 0.3–0.7; default 0.5 reproduces the
    /// "first half, next quarter, next eighth" example.
    #[serde(default = "default_palette_squeeze_falloff", skip_serializing_if = "is_default_palette_squeeze_falloff")]
    pub palette_squeeze_falloff: f32,
    /// Logarithmic (exponential) redistribution of the squeezed lookup.
    /// 0.0 = identity (no-op). Positive bunches the palette toward
    /// the end of the input range; negative bunches toward the start.
    /// Composes with squeeze (applied after).
    #[serde(default, skip_serializing_if = "is_default_palette_log_strength")]
    pub palette_log_strength: f32,
    /// Flip the palette as the last step of the lookup pipeline.
    /// Composes with rotation/squeeze; does not modify the base palette stops.
    #[serde(default, skip_serializing_if = "is_default_palette_reverse")]
    pub palette_reverse: bool,
    #[serde(default)]
    pub background_color: [f32; 3],

    /// Tone mapping settings
    #[serde(default)]
    pub tonemap_mode: ToneMapMode,
    /// How to handle channel values that exceed [0,1] after tone mapping.
    /// `Clip` (default, Apophysis-compatible) clamps per-channel and shifts
    /// hue toward CMY/white at high brightness. `MaxNorm` divides all
    /// channels by their max to preserve hue when clipping would occur.
    #[serde(default)]
    pub highlight_mode: HighlightMode,
    #[serde(default)]
    pub tonemap_curve: ToneCurve,
    /// Whether to actually apply the tone curve
    #[serde(default = "default_true")]
    pub use_curve: bool,
    #[serde(default = "default_exposure")]
    pub exposure: f32,
    #[serde(default = "default_gamma")]
    pub gamma: f32,
    /// Gamma threshold: smooths gamma curve at low densities (Apophysis compatibility).
    /// See `DEFAULT_GAMMA_THRESHOLD` in `defaults.rs` for the current value.
    #[serde(default = "default_gamma_threshold")]
    pub gamma_threshold: f32,
    /// Brightness: logarithmic brightness scaling (Apophysis compatibility)
    /// 1.0 = standard brightness (default), higher = brighter
    #[serde(default = "default_brightness")]
    pub brightness: f32,
    /// Vibrancy: blend between old (gamma-only) and new (vibrant) color algorithms
    /// 1.0 = modern vibrant colors (default), 0.0 = classic gamma-only colors
    #[serde(default = "default_vibrancy")]
    pub vibrancy: f32,
    /// Highlights ("Fade to White" / Apophysis `white_level`): divides chroma
    /// in the log-density curve while leaving alpha alone, so bright/dense
    /// pixels bloom toward the background through alpha blend before RGB
    /// clips. Higher values bleach to white sooner; lower values keep
    /// highlights more saturated. See `DEFAULT_WHITE_LEVEL` in `defaults.rs`.
    #[serde(default = "default_white_level")]
    pub white_level: f32,
    /// Saturation: color saturation boost (1.0 = no change, >1.0 = more saturated)
    #[serde(default = "default_saturation")]
    pub saturation: f32,

    /// Hue shift: rotate hue in degrees (-180.0 to 180.0, 0.0 = no shift)
    #[serde(default = "default_hue_shift")]
    pub hue_shift: f32,

    /// Alpha blend low threshold: start blending toward linear alpha at this gamma-corrected value
    /// Lower = more gamma-corrected (no halos), Higher = more linear (more detail at edges)
    #[serde(default = "default_alpha_blend_low", skip_serializing_if = "is_default_alpha_blend_low")]
    pub alpha_blend_low: f32,

    /// Alpha blend high threshold: fully linear alpha above this gamma-corrected value
    /// Controls when mid-range density areas get full linear alpha (preserves detail)
    #[serde(default = "default_alpha_blend_high", skip_serializing_if = "is_default_alpha_blend_high")]
    pub alpha_blend_high: f32,

    /// Levels: density threshold for background/transparency
    /// Pixels with density below this become fully transparent (show background)
    #[serde(default, skip_serializing_if = "is_default_levels_low")]
    pub levels_low: f32,

    /// Levels: density threshold for full opacity
    /// Pixels with density above this become fully opaque (show fractal color)
    #[serde(default = "default_levels_high", skip_serializing_if = "is_default_levels_high")]
    pub levels_high: f32,

    /// Levels: gamma/midpoint for density curve (1.0 = linear)
    #[serde(default = "default_levels_gamma", skip_serializing_if = "is_default_levels_gamma")]
    pub levels_gamma: f32,

    /// Density effects chain (run before tonemap, have access to density in alpha)
    /// Empty = no effect passes, zero cost
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub density_effects: Vec<EffectInstance>,

    /// Color effects chain (run after tonemap, operate on final RGB)
    /// Empty = no effect passes, zero cost
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub color_effects: Vec<EffectInstance>,

    /// Optional: Deterministic RNG for reproducible renders
    #[serde(default)]
    pub deterministic_rng: bool,
}

fn default_zoom() -> f32 {
    1.0
}

fn default_density_scale() -> f32 {
    super::defaults::DEFAULT_DENSITY_SCALE
}

fn default_speed_factor() -> f32 {
    super::defaults::DEFAULT_SPEED_FACTOR
}

fn default_exposure() -> f32 {
    super::defaults::DEFAULT_EXPOSURE
}

fn default_gamma() -> f32 {
    super::defaults::DEFAULT_GAMMA
}

fn default_gamma_threshold() -> f32 {
    super::defaults::DEFAULT_GAMMA_THRESHOLD
}

fn default_white_level() -> f32 {
    super::defaults::DEFAULT_WHITE_LEVEL
}

fn default_brightness() -> f32 {
    super::defaults::DEFAULT_BRIGHTNESS
}

fn default_vibrancy() -> f32 {
    1.0  // Modern vibrant colors by default
}

fn default_saturation() -> f32 {
    super::defaults::DEFAULT_SATURATION
}

fn default_hue_shift() -> f32 {
    super::defaults::DEFAULT_HUE_SHIFT
}

fn default_alpha_blend_low() -> f32 {
    super::defaults::DEFAULT_ALPHA_BLEND_LOW
}

fn default_alpha_blend_high() -> f32 {
    super::defaults::DEFAULT_ALPHA_BLEND_HIGH
}

fn default_dof_focus_distance() -> f32 {
    super::defaults::DEFAULT_DOF_FOCUS_DISTANCE
}

fn default_levels_high() -> f32 {
    // Clip at `× mean density` units after the scale-invariance change,
    // independent of total iteration count. The 10× default was
    // recalibrated alongside DEFAULT_EXPOSURE / DEFAULT_GAMMA /
    // DEFAULT_GAMMA_THRESHOLD to produce a sensible image for flames
    // that don't override tonemap fields.
    super::defaults::DEFAULT_LEVELS_HIGH
}

fn default_levels_gamma() -> f32 {
    1.0  // Linear (no gamma adjustment)
}

fn default_palette_rotation() -> f32 {
    super::defaults::DEFAULT_PALETTE_ROTATION
}

fn default_palette_size() -> u32 {
    super::defaults::DEFAULT_PALETTE_SIZE
}

fn default_palette_squeeze() -> f32 {
    super::defaults::DEFAULT_PALETTE_SQUEEZE
}

fn is_default_palette_reverse(v: &bool) -> bool {
    !*v
}

fn default_palette_squeeze_falloff() -> f32 {
    0.5
}

fn is_default_palette_squeeze_falloff(v: &f32) -> bool {
    (*v - 0.5).abs() < FLOAT_EPSILON
}

fn is_default_palette_squeeze_mode(v: &crate::scene::palette::SqueezeMode) -> bool {
    matches!(v, crate::scene::palette::SqueezeMode::Linear)
}

fn is_default_palette_log_strength(v: &f32) -> bool {
    v.abs() < FLOAT_EPSILON
}

fn default_palette() -> Palette {
    Palette::fire()
}

/// Custom deserializer to handle old configs with `palette: null` or missing palette
fn deserialize_palette<'de, D>(deserializer: D) -> Result<Palette, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Try to deserialize as Option<Palette> for backward compatibility
    let opt: Option<Palette> = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_else(default_palette))
}

fn default_true() -> bool {
    true
}

fn default_max_iterations() -> u64 {
    super::defaults::DEFAULT_MAX_ITERATIONS
}

// === Helper functions for skip_serializing_if with approximate float comparison ===
// These handle f32 precision issues (e.g., 0.8 becomes 0.800000011920929)

const FLOAT_EPSILON: f32 = 1e-5;

fn is_default_alpha_blend_low(v: &f32) -> bool {
    (*v - super::defaults::DEFAULT_ALPHA_BLEND_LOW).abs() < FLOAT_EPSILON
}

fn is_default_alpha_blend_high(v: &f32) -> bool {
    (*v - super::defaults::DEFAULT_ALPHA_BLEND_HIGH).abs() < FLOAT_EPSILON
}

fn is_default_levels_low(v: &f32) -> bool {
    v.abs() < FLOAT_EPSILON  // Default is 0.0
}

fn is_default_levels_high(v: &f32) -> bool {
    (*v - super::defaults::DEFAULT_LEVELS_HIGH).abs() < FLOAT_EPSILON
}

fn is_default_levels_gamma(v: &f32) -> bool {
    (*v - 1.0).abs() < FLOAT_EPSILON  // Default is 1.0
}

fn default_histogram_color_scale() -> f32 {
    super::defaults::DEFAULT_HISTOGRAM_COLOR_SCALE
}

fn default_blend_factor() -> f32 {
    super::defaults::DEFAULT_BLEND_FACTOR
}

fn default_use_dynamic_blend() -> bool {
    super::defaults::DEFAULT_USE_DYNAMIC_BLEND
}

impl Default for FractalConfig {
    fn default() -> Self {
        use crate::scene::transforms::Flame;
        use crate::scene::tonemap::ToneCurve;

        Self {
            flame: Flame::default(),
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            camera_rotation_x: 0.0,
            camera_rotation_y: 0.0,
            camera_z: 0.0,
            dof_focus_distance: default_dof_focus_distance(),
            dof_blur_strength: 0.0,
            fog_strength: 0.0,
            fog_start: 0.0,
            density_scale: 1.0,
            speed_factor: 0.5,
            max_iterations: default_max_iterations(),
            histogram_color_scale: default_histogram_color_scale(),
            blend_factor: default_blend_factor(),
            use_dynamic_blend: default_use_dynamic_blend(),
            target_iterations_per_pixel: 0,
            color_mode: ColorMode::Palette,
            path_map_style: PathMapStyle::default(),
            path_capture_mode: PathCaptureMode::default(),
            path_tracking_mode: PathTrackingMode::default(),
            palette: default_palette(),
            palette_rotation: default_palette_rotation(),
            palette_size: default_palette_size(),
            palette_squeeze: default_palette_squeeze(),
            palette_squeeze_mode: crate::scene::palette::SqueezeMode::Linear,
            palette_squeeze_falloff: default_palette_squeeze_falloff(),
            palette_log_strength: 0.0,
            palette_reverse: false,
            background_color: [0.0, 0.0, 0.0],
            tonemap_mode: ToneMapMode::default(),
            highlight_mode: HighlightMode::default(),
            tonemap_curve: ToneCurve::default(),
            use_curve: default_true(),
            exposure: default_exposure(),
            gamma: default_gamma(),
            gamma_threshold: default_gamma_threshold(),
            brightness: default_brightness(),
            vibrancy: default_vibrancy(),
            white_level: default_white_level(),
            saturation: default_saturation(),
            hue_shift: default_hue_shift(),
            alpha_blend_low: default_alpha_blend_low(),
            alpha_blend_high: default_alpha_blend_high(),
            levels_low: 0.0,
            levels_high: default_levels_high(),
            levels_gamma: default_levels_gamma(),
            density_effects: Vec::new(),
            color_effects: Vec::new(),
            deterministic_rng: false,
        }
    }
}

impl FractalConfig {
    /// Export configuration to JSON string with version header
    /// Omits fields that match defaults for compact output
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        // Serialize to JSON value
        let mut value = serde_json::to_value(self)?;

        if let Some(obj) = value.as_object_mut() {
            // Remove fields that match defaults (compact serialization)
            let defaults = Self::default();
            Self::remove_default_fields(obj, self, &defaults);

            // Build ordered object with version first
            let mut ordered_obj = serde_json::Map::new();
            ordered_obj.insert("version".to_string(), serde_json::json!(CURRENT_CONFIG_VERSION));

            // Add flame first (always required), then other non-default fields
            if let Some(flame) = obj.remove("flame") {
                ordered_obj.insert("flame".to_string(), flame);
            }
            for (k, v) in obj.iter() {
                if k != "version" {
                    ordered_obj.insert(k.clone(), v.clone());
                }
            }
            serde_json::to_string_pretty(&ordered_obj)
        } else {
            serde_json::to_string_pretty(&value)
        }
    }

    /// Remove fields from JSON object that match default values
    fn remove_default_fields(obj: &mut serde_json::Map<String, serde_json::Value>, config: &Self, defaults: &Self) {
        // View settings
        if config.zoom == defaults.zoom { obj.remove("zoom"); }
        if config.pan_x == defaults.pan_x { obj.remove("pan_x"); }
        if config.pan_y == defaults.pan_y { obj.remove("pan_y"); }
        if config.rotation == defaults.rotation { obj.remove("rotation"); }
        if config.camera_rotation_x == defaults.camera_rotation_x { obj.remove("camera_rotation_x"); }
        if config.camera_rotation_y == defaults.camera_rotation_y { obj.remove("camera_rotation_y"); }
        if config.camera_z == defaults.camera_z { obj.remove("camera_z"); }
        if config.dof_focus_distance == defaults.dof_focus_distance { obj.remove("dof_focus_distance"); }
        if config.dof_blur_strength == defaults.dof_blur_strength { obj.remove("dof_blur_strength"); }
        if config.fog_strength == defaults.fog_strength { obj.remove("fog_strength"); }
        if config.fog_start == defaults.fog_start { obj.remove("fog_start"); }

        // Rendering settings
        if config.density_scale == defaults.density_scale { obj.remove("density_scale"); }
        if config.speed_factor == defaults.speed_factor { obj.remove("speed_factor"); }
        if config.max_iterations == defaults.max_iterations { obj.remove("max_iterations"); }
        if config.histogram_color_scale == defaults.histogram_color_scale { obj.remove("histogram_color_scale"); }
        if config.blend_factor == defaults.blend_factor { obj.remove("blend_factor"); }
        if config.use_dynamic_blend == defaults.use_dynamic_blend { obj.remove("use_dynamic_blend"); }
        if config.target_iterations_per_pixel == defaults.target_iterations_per_pixel { obj.remove("target_iterations_per_pixel"); }

        // Color settings
        if config.color_mode == defaults.color_mode { obj.remove("color_mode"); }
        // Always include palette in output (it's required)
        if config.palette_rotation == defaults.palette_rotation { obj.remove("palette_rotation"); }
        if config.palette_size == defaults.palette_size { obj.remove("palette_size"); }
        if config.palette_squeeze == defaults.palette_squeeze { obj.remove("palette_squeeze"); }
        if config.palette_squeeze_mode == defaults.palette_squeeze_mode { obj.remove("palette_squeeze_mode"); }
        if config.palette_squeeze_falloff == defaults.palette_squeeze_falloff { obj.remove("palette_squeeze_falloff"); }
        if config.palette_log_strength == defaults.palette_log_strength { obj.remove("palette_log_strength"); }
        if config.palette_reverse == defaults.palette_reverse { obj.remove("palette_reverse"); }
        if config.background_color == defaults.background_color { obj.remove("background_color"); }

        // Tone mapping settings
        if config.tonemap_mode == defaults.tonemap_mode { obj.remove("tonemap_mode"); }
        if config.highlight_mode == defaults.highlight_mode { obj.remove("highlight_mode"); }
        if config.tonemap_curve == defaults.tonemap_curve { obj.remove("tonemap_curve"); }
        if config.use_curve == defaults.use_curve { obj.remove("use_curve"); }
        if config.exposure == defaults.exposure { obj.remove("exposure"); }
        if config.gamma == defaults.gamma { obj.remove("gamma"); }
        if config.gamma_threshold == defaults.gamma_threshold { obj.remove("gamma_threshold"); }
        if config.brightness == defaults.brightness { obj.remove("brightness"); }
        if config.vibrancy == defaults.vibrancy { obj.remove("vibrancy"); }
        if config.white_level == defaults.white_level { obj.remove("white_level"); }
        if config.saturation == defaults.saturation { obj.remove("saturation"); }
        if config.hue_shift == defaults.hue_shift { obj.remove("hue_shift"); }

        // Other
        if config.deterministic_rng == defaults.deterministic_rng { obj.remove("deterministic_rng"); }
    }

    /// Import configuration from JSON string with version checking and migration
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        // Parse to check version first
        let value: serde_json::Value = serde_json::from_str(json)?;

        // Check version if present
        let version = value.get("version")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0); // Pre-versioning configs are version 0

        if version > CURRENT_CONFIG_VERSION {
            let msg = format!(
                "Config version {} is newer than supported version {}. Please update the application.",
                version, CURRENT_CONFIG_VERSION
            );
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                msg
            )));
        }

        // Deserialize first (serde defaults will apply)
        let mut config: Self = serde_json::from_value(value)?;

        // Apply migrations if needed
        if version < CURRENT_CONFIG_VERSION {
            config = Self::migrate(config, version)?;
        }

        // Assign session-local IDs to every Transform / Flame / Effect
        // that came in without one (i.e. all of them — IDs are
        // serde-skipped). This makes the deserialized config immediately
        // usable by the animation rebind machinery.
        config.fixup_ids();

        Ok(config)
    }

    /// Walk every list-item (transforms in all three pools on Main and
    /// every subflame, every subflame itself, and every color/density
    /// effect) and assign a fresh session-local ID to anything whose
    /// `id` is the zero sentinel. Idempotent — items that already have
    /// a non-zero ID are left alone.
    ///
    /// Called automatically after deserialize. Also safe to call after
    /// any operation that produces a config without IDs (e.g. preset
    /// clones, paste from clipboard).
    pub fn fixup_ids(&mut self) {
        fixup_flame_ids(&mut self.flame);
        for effect in &mut self.density_effects {
            if effect.id == 0 {
                effect.id = crate::scene::transforms::next_id();
            }
        }
        for effect in &mut self.color_effects {
            if effect.id == 0 {
                effect.id = crate::scene::transforms::next_id();
            }
        }
    }

    /// Migrate config from old version to current version
    /// Returns error as serde_json::Error for API consistency
    fn migrate(mut config: Self, from_version: u32) -> Result<Self, serde_json::Error> {
        let mut current_version = from_version;

        // Migration chain: apply each migration in sequence
        while current_version < CURRENT_CONFIG_VERSION {
            config = match current_version {
                0 => Self::migrate_v0_to_v1(config),
                // Future migrations:
                // 1 => Self::migrate_v1_to_v2(config),
                // 2 => Self::migrate_v2_to_v3(config),
                _ => {
                    let msg = format!("Unknown config version {} during migration", current_version);
                    return Err(serde_json::Error::io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        msg
                    )));
                }
            };
            current_version += 1;
        }

        log::info!("Migrated config from version {} to {}", from_version, CURRENT_CONFIG_VERSION);
        Ok(config)
    }

    /// Migrate pre-versioning configs (version 0) to version 1
    /// These are configs saved before versioning was implemented
    fn migrate_v0_to_v1(config: Self) -> Self {
        // No structural changes needed - serde defaults handle missing fields
        // This migration exists to document that v0 -> v1 is a no-op
        config
    }

    /// Export to JSON file
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let json = self.to_json()?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Import from JSON file
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&json)?)
    }

    /// Import multiple configurations from JSON string
    /// Supports both single object (backward compatible) and JSON array formats
    pub fn from_json_multi(json: &str) -> Result<Vec<Self>, serde_json::Error> {
        // Parse as generic JSON value to detect format
        let value: serde_json::Value = serde_json::from_str(json)?;

        if value.is_array() {
            // Array format: multiple configs
            let configs: Vec<serde_json::Value> = serde_json::from_value(value)?;
            configs
                .into_iter()
                .enumerate()
                .map(|(i, v)| {
                    Self::from_json_value(v).map_err(|e| {
                        serde_json::Error::io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Error in config[{}]: {}", i, e),
                        ))
                    })
                })
                .collect()
        } else if value.is_object() {
            // Single object format (backward compatible)
            Ok(vec![Self::from_json_value(value)?])
        } else {
            Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Expected JSON object or array",
            )))
        }
    }

    /// Import a single configuration from JSON value (internal helper)
    fn from_json_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        // Check version if present
        let version = value.get("version")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0);

        if version > CURRENT_CONFIG_VERSION {
            let msg = format!(
                "Config version {} is newer than supported version {}. Please update the application.",
                version, CURRENT_CONFIG_VERSION
            );
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                msg,
            )));
        }

        // Deserialize first (serde defaults will apply)
        let mut config: Self = serde_json::from_value(value)?;

        // Apply migrations if needed
        if version < CURRENT_CONFIG_VERSION {
            config = Self::migrate(config, version)?;
        }

        // Assign session-local IDs (see `fixup_ids` doc).
        config.fixup_ids();

        Ok(config)
    }

    /// Export multiple configurations to JSON array string
    pub fn to_json_array(configs: &[Self]) -> Result<String, serde_json::Error> {
        let values: Result<Vec<serde_json::Value>, _> = configs
            .iter()
            .map(|config| {
                let mut value = serde_json::to_value(config)?;
                if let Some(obj) = value.as_object_mut() {
                    let defaults = Self::default();
                    Self::remove_default_fields(obj, config, &defaults);

                    // Build ordered object with version first
                    let mut ordered_obj = serde_json::Map::new();
                    ordered_obj.insert("version".to_string(), serde_json::json!(CURRENT_CONFIG_VERSION));

                    // Add flame first (always required), then other non-default fields
                    if let Some(flame) = obj.remove("flame") {
                        ordered_obj.insert("flame".to_string(), flame);
                    }
                    for (k, v) in obj.iter() {
                        if k != "version" {
                            ordered_obj.insert(k.clone(), v.clone());
                        }
                    }
                    Ok(serde_json::Value::Object(ordered_obj))
                } else {
                    Ok(value)
                }
            })
            .collect();

        serde_json::to_string_pretty(&values?)
    }

    /// Load multiple configurations from file
    /// Supports both single object and JSON array formats
    pub fn load_multi_from_file(path: &std::path::Path) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        Ok(Self::from_json_multi(&json)?)
    }

    /// Save multiple configurations to file as JSON array
    pub fn save_multi_to_file(configs: &[Self], path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let json = Self::to_json_array(configs)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

/// Recursively assign session-local IDs to a flame, its transform
/// pools, and every subflame. Items with non-zero IDs are left alone
/// so this is safe to re-run.
fn fixup_flame_ids(flame: &mut Flame) {
    use crate::scene::transforms::next_id;
    if flame.id == 0 {
        flame.id = next_id();
    }
    for t in &mut flame.transforms {
        if t.id == 0 {
            t.id = next_id();
        }
    }
    for t in &mut flame.linked_transforms {
        if t.id == 0 {
            t.id = next_id();
        }
    }
    for t in &mut flame.final_transforms {
        if t.id == 0 {
            t.id = next_id();
        }
    }
    for sub in &mut flame.subflames {
        fixup_flame_ids(sub);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_included_in_json() {
        let config = FractalConfig::default();
        let json = config.to_json().unwrap();
        assert!(json.contains("\"version\": 1"));
    }

    #[test]
    fn test_compact_serialization_omits_defaults() {
        let config = FractalConfig::default();
        let json = config.to_json().unwrap();

        // Default values should NOT be in the output
        assert!(!json.contains("\"zoom\""));
        assert!(!json.contains("\"pan_x\""));
        assert!(!json.contains("\"exposure\""));
        assert!(!json.contains("\"gamma\""));

        // Required fields should still be present
        assert!(json.contains("\"version\""));
        assert!(json.contains("\"flame\""));
    }

    #[test]
    fn test_non_default_values_included() {
        let mut config = FractalConfig::default();
        config.zoom = 2.5;
        config.exposure = 1.5;

        let json = config.to_json().unwrap();

        // Non-default values should be included
        assert!(json.contains("\"zoom\": 2.5"));
        assert!(json.contains("\"exposure\": 1.5"));
    }

    #[test]
    fn test_roundtrip_with_defaults() {
        let original = FractalConfig::default();
        let json = original.to_json().unwrap();
        let loaded = FractalConfig::from_json(&json).unwrap();

        // All values should match after roundtrip
        assert_eq!(original.zoom, loaded.zoom);
        assert_eq!(original.pan_x, loaded.pan_x);
        assert_eq!(original.exposure, loaded.exposure);
        assert_eq!(original.gamma, loaded.gamma);
    }

    #[test]
    fn test_roundtrip_with_non_defaults() {
        let mut original = FractalConfig::default();
        original.zoom = 3.0;
        original.pan_x = 1.5;
        original.exposure = 2.0;
        original.gamma = 1.8;

        let json = original.to_json().unwrap();
        let loaded = FractalConfig::from_json(&json).unwrap();

        assert_eq!(original.zoom, loaded.zoom);
        assert_eq!(original.pan_x, loaded.pan_x);
        assert_eq!(original.exposure, loaded.exposure);
        assert_eq!(original.gamma, loaded.gamma);
    }

    #[test]
    fn test_future_version_rejected() {
        let json = r#"{"version": 999, "flame": {"name": "test", "transforms": []}}"#;
        let result = FractalConfig::from_json(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("newer than supported"));
    }

    #[test]
    fn test_missing_version_treated_as_v0() {
        // Pre-versioning config (no version field)
        let json = r#"{"flame": {"name": "test", "transforms": []}}"#;
        let result = FractalConfig::from_json(json);
        // Should succeed - v0 migrates to v1
        assert!(result.is_ok());
    }

    #[test]
    fn test_version_1_loads_without_migration() {
        let json = r#"{"version": 1, "flame": {"name": "test", "transforms": []}}"#;
        let result = FractalConfig::from_json(json);
        assert!(result.is_ok());
    }

    // Multi-config tests

    #[test]
    fn test_from_json_multi_single_object() {
        // Single object format should return vec with one element
        let json = r#"{"version": 1, "flame": {"name": "test", "transforms": []}}"#;
        let configs = FractalConfig::from_json_multi(json).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].flame.name, "test");
    }

    #[test]
    fn test_from_json_multi_array() {
        // Array format should return all configs
        let json = r#"[
            {"version": 1, "flame": {"name": "config1", "transforms": []}},
            {"version": 1, "flame": {"name": "config2", "transforms": []}}
        ]"#;
        let configs = FractalConfig::from_json_multi(json).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].flame.name, "config1");
        assert_eq!(configs[1].flame.name, "config2");
    }

    #[test]
    fn test_from_json_multi_empty_array() {
        let json = "[]";
        let configs = FractalConfig::from_json_multi(json).unwrap();
        assert!(configs.is_empty());
    }

    #[test]
    fn test_from_json_multi_invalid_format() {
        // Neither object nor array
        let json = "\"just a string\"";
        let result = FractalConfig::from_json_multi(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_to_json_array() {
        let mut config1 = FractalConfig::default();
        config1.flame.name = "first".to_string();
        config1.zoom = 2.0;

        let mut config2 = FractalConfig::default();
        config2.flame.name = "second".to_string();
        config2.exposure = 1.5;

        let json = FractalConfig::to_json_array(&[config1, config2]).unwrap();

        // Should be a JSON array
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));

        // Both configs should be present
        assert!(json.contains("\"first\""));
        assert!(json.contains("\"second\""));

        // Non-default values should be included
        assert!(json.contains("\"zoom\": 2"));
        assert!(json.contains("\"exposure\": 1.5"));
    }

    #[test]
    fn test_multi_config_roundtrip() {
        let mut config1 = FractalConfig::default();
        config1.flame.name = "alpha".to_string();
        config1.zoom = 3.0;

        let mut config2 = FractalConfig::default();
        config2.flame.name = "beta".to_string();
        config2.gamma = 1.8;

        let json = FractalConfig::to_json_array(&[config1.clone(), config2.clone()]).unwrap();
        let loaded = FractalConfig::from_json_multi(&json).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].flame.name, "alpha");
        assert_eq!(loaded[0].zoom, 3.0);
        assert_eq!(loaded[1].flame.name, "beta");
        assert_eq!(loaded[1].gamma, 1.8);
    }

    #[test]
    fn test_multi_config_array_migration() {
        // Array with pre-versioned configs (no version field)
        let json = r#"[
            {"flame": {"name": "old1", "transforms": []}},
            {"flame": {"name": "old2", "transforms": []}}
        ]"#;
        let configs = FractalConfig::from_json_multi(json).unwrap();
        assert_eq!(configs.len(), 2);
        // Should have been migrated from v0 to v1
    }

    #[test]
    fn test_multi_config_array_future_version_rejected() {
        // Array with future version should fail
        let json = r#"[
            {"version": 1, "flame": {"name": "ok", "transforms": []}},
            {"version": 999, "flame": {"name": "future", "transforms": []}}
        ]"#;
        let result = FractalConfig::from_json_multi(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("config[1]"));
    }
}
