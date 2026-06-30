use serde::{Deserialize, Serialize};
use crate::scene::transforms::{Flame, RenderMode};
use crate::scene::palette::{ColorMode, Palette, PathCaptureMode, PathMapStyle, PathTrackingMode};
use crate::scene::tonemap::{HighlightMode, ToneMapMode, ToneCurve};
use crate::effects::EffectInstance;

/// Current config format version.
///
/// v2 introduces the cloud "opaque blob" wire format (see
/// `docs/projects/api-v2.md`): the same JSON a `.fflame` file holds is stored
/// as a blob, so new config fields need no API/DB change. Both `.fflame` and
/// cloud loads run through `migrate_value` (version-keyed, on the raw JSON
/// *before* deserialize) so a version's old field defaults can be restored for
/// fields that were stripped at save time.
///
/// v3 moves the scene-level render fields (`render_mode`, `preserve_z`,
/// `perspective_strength`, `depth_density_compensation`, `far_density_fade`,
/// `far_density_fade_start`) off `Flame` and onto `FractalConfig` — they were
/// always whole-render settings (subflame copies were dead). The v2→v3
/// migration lifts them from `config.flame.*` to the top level.
pub const CURRENT_CONFIG_VERSION: u32 = 3;

/// Complete fractal configuration (excludes runtime-only settings)
/// All fields except `flame` have defaults for compact serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FractalConfig {
    /// The flame (transforms) - always required.
    /// Subflames (referenced by `subflame_wf` variations) live on the
    /// `Flame` struct itself, not here — see `Flame::subflames`.
    pub flame: Flame,

    /// Render mode (2D vs 3D). Scene-level, not per-flame: the whole render
    /// uses a single mode (one `RENDER_3D` shader flag), so subflames inherit
    /// it — a mixed 2D/3D nesting was never possible. Moved here from `Flame`
    /// in config v3. **Always serialized** (no skip) so the cloud blob's
    /// top-level `render_mode` is never absent — the server projects it into a
    /// typed column for catalog queries.
    #[serde(default)]
    pub render_mode: RenderMode,

    /// JWildfire's `preserve_z` flag — whether the chaos game's Z carries
    /// across iterations or resets each step. Scene-global (feeds the single
    /// `flatten_z_per_iter` shader flag), moved here from `Flame` in v3.
    /// Skipped when `true` (the pre-field default); absent ⇒ `true` on load so
    /// flames authored before the flag keep their look. New flames default to
    /// `false` (Apo/JWF semantics) and write it explicitly.
    #[serde(default = "default_preserve_z", skip_serializing_if = "is_default_preserve_z")]
    pub preserve_z: bool,

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
    pub camera_rotation_y: f32,  // Yaw (rotation around Z axis — Apo's ZXY Euler convention)
    /// JWildfire / Apophysis bank angle — rotation around the Y axis,
    /// which in the default camera pose (looking down −Z) tilts the
    /// camera horizontally and creates a perspective skew. Composed
    /// alongside pitch/yaw/roll inside the 3D camera matrix per
    /// JWildfire's `createProjectionMatrix(yaw, pitch, bank, roll)`.
    ///
    /// **XML rename quirk**: JWildfire serializes its `bank` field
    /// under the on-disk attribute name `cam_roll`, and writes the
    /// `roll` parameter as `rotate` (in degrees). So `cam_roll` in
    /// any imported `.flame` lands here, not on `rotation`. See
    /// `docs/projects/jwf-features.md` ("Camera rotation") for the
    /// full mapping table.
    ///
    /// In radians. Default 0. 3D-mode only — has no effect in 2D.
    #[serde(default, skip_serializing_if = "is_default_camera_bank")]
    pub camera_bank: f32,
    /// Camera X position in world space. JWildfire stores this as
    /// `cam_pos_x` on every flame; we round-trip via that attribute.
    /// 3D-mode only.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub camera_x: f32,
    /// Camera Y position in world space. Round-trips via JWildfire's
    /// `cam_pos_y`. 3D-mode only.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub camera_y: f32,
    /// Camera Z position (height). Round-trips via JWildfire's
    /// `cam_pos_z` (preferred) and the older `cam_zpos` (fallback —
    /// import only honors `cam_zpos` when `cam_pos_z` is absent; export
    /// writes both for backward compat with older JWildfire and Apo
    /// versions that only know `cam_zpos`).
    #[serde(default)]
    pub camera_z: f32,

    /// Perspective strength for 3D rendering (0.0 = flat/orthographic, higher
    /// = stronger). A projection/camera parameter — moved here from `Flame` in
    /// v3 to sit with the rest of the camera state. Round-trips through JWF's
    /// `cam_perspective`/`cam_persp` XML attribute.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub perspective_strength: f32,

    /// Saved image dimensions. Mirrors the `size` attribute on
    /// JWildfire / Apophysis `<flame>` elements — historically a
    /// canvas-extent concept that also participates in the zoom math
    /// (see docs/projects/jwf-features.md "size attribute"). Today
    /// we only consume this for one thing: pre-filling the
    /// "Custom Export Size" inputs in the Export PNG dialog so
    /// users get the flame's intended dimensions by default. The
    /// stored value round-trips through JSON and through XML
    /// import/export so flames opened in JWF/Apo see the same
    /// number.
    ///
    /// Default `(1920, 1080)` matches what we used to write
    /// unconditionally on export. Existing `.fflame` JSON files
    /// without the field deserialize identically to before.
    #[serde(default = "default_image_size", skip_serializing_if = "is_default_image_size")]
    pub image_size: (u32, u32),

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

    /// Depth-dependent density compensation for 3D perspective (0.0 = off,
    /// 1.0 = full radiance preservation). A render-time per-sample histogram
    /// weighting — moved here from `Flame` in v3 to sit with the other depth
    /// render effects (DoF, fog). Our own extension; `.fflame` only, dropped
    /// on `.flame` XML export. See `Flame`'s old doc for the full `zr^(−2·s)`
    /// derivation.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub depth_density_compensation: f32,
    /// Far density fade strength (Gaussian falloff of far samples' density
    /// weight). Render-time effect, moved here from `Flame` in v3. `.fflame`
    /// only. 0 = off.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub far_density_fade: f32,
    /// Camera-space depth where the far density fade starts. Only meaningful
    /// when `far_density_fade > 0`. Moved here from `Flame` in v3.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub far_density_fade_start: f32,

    /// Spatial filter — Gaussian blur applied to the per-batch histogram
    /// before accumulation. Mirrors Apophysis's `filter` attribute: a
    /// small per-sample-spread Gaussian that smooths per-iteration grain.
    /// Default 0.0 (off). Apo XML import picks up `filter="..."`.
    #[serde(default, skip_serializing_if = "is_default_filter_radius")]
    pub filter_radius: f32,

    /// Spatial-filter edge handling. Slider `0.0..=1.0`:
    /// - `0.0`: strict edge preservation (bilateral weighting tight) —
    ///   highlights stay sharp, only similar-density neighbors blur
    ///   together
    /// - `1.0`: uniform Gaussian (no density similarity check) —
    ///   highlights muddy into neighbors, same as raw box blur
    /// Default `0.0` so the filter does what users typically want
    /// (clean dim-area grain without blurring highlight detail).
    /// Mapped exponentially to a density-sigma value at shader time:
    /// `σ_d = mean_density × 1000^blur_edges`.
    #[serde(default, skip_serializing_if = "is_default_filter_blur_edges")]
    pub filter_blur_edges: f32,

    /// Rendering settings
    #[serde(default = "default_density_scale")]
    pub density_scale: f32,
    #[serde(default = "default_speed_factor")]
    pub speed_factor: f32,
    /// Maximum total iterations to render (default: 1 billion = ~infinite for interactive use)
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u64,
    /// Histogram color scale (precision vs overflow protection, default: 10.0)
    /// Blend factor for accumulation (0.01 = slow/smooth, 1.0 = fast/flickery, default: 0.1)
    #[serde(default = "default_blend_factor")]
    pub blend_factor: f32,
    /// Use dynamic blend (true = exponential convergence, false = fixed blend rate, default: true)
    #[serde(default = "default_use_dynamic_blend")]
    pub use_dynamic_blend: bool,

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

    /// Levels: opt-in density opacity remap. Apophysis has no Levels
    /// system; when off, the gamma/vibrancy pipeline's alpha runs
    /// straight through unmodified — Apo-matching default behavior.
    /// When on, the `levels_low`/`levels_high`/`levels_gamma` triplet
    /// gates per-pixel opacity by relative density.
    #[serde(default = "default_levels_enabled", skip_serializing_if = "is_default_levels_enabled")]
    pub levels_enabled: bool,

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

/// Default saved image dimensions, matching what we historically
/// wrote as a fixed `size="1920 1080"` on every XML export. Old
/// `.fflame` JSON files (no `image_size` field) deserialize to
/// this value, so behavior is unchanged for them.
fn default_image_size() -> (u32, u32) {
    (1920, 1080)
}

/// Skip-serialize helper for `image_size`. Keeps existing flame
/// JSON files free of the field unless the user actually changes
/// it from the default — same pattern as `is_default_filter_radius`.
fn is_default_image_size(v: &(u32, u32)) -> bool {
    *v == default_image_size()
}

/// Skip-serialize helper for `camera_bank`. Almost every flame has
/// no bank applied, so the default skip keeps `.fflame` JSON files
/// free of the field unless a user explicitly tilts the camera
/// (or imports a JWF flame that wrote a non-zero `cam_roll`).
/// Skip-serialize helper — keeps fields free from existing flame
/// JSON files unless the user actually changes them from zero.
/// Shared by `camera_x` / `camera_y` and other zero-defaulted f32s.
fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

fn is_default_camera_bank(v: &f32) -> bool {
    *v == 0.0
}

/// Serde-absent default for `preserve_z`: `true`. Flames authored before the
/// flag existed omit it, and historically defaulted to `true` (Z carries) to
/// preserve their look. New flames default to `false` (via
/// `FractalConfig::default`) and write the field explicitly.
fn default_preserve_z() -> bool {
    true
}

/// Skip-serialize helper for `preserve_z`: omit when `true` (the pre-field
/// default). Paired with `default_preserve_z` so omitted ⇒ `true` on load.
/// **Not** listed in `remove_default_fields` — that strips against
/// `Self::default()` (`false`), which would wrongly drop genuine `false`
/// values; the skip+default pair is the single source of truth here.
fn is_default_preserve_z(v: &bool) -> bool {
    *v
}

/// Convert the pre-rename legacy `projection` field (an enum: the string
/// `"Orthographic"`, or `{ "Perspective": { "strength": X } }`) into a plain
/// `perspective_strength` number, used by the v2→v3 migration. Mirrors the
/// translation that used to live in `Flame`'s manual deserializer.
fn legacy_projection_to_strength(v: serde_json::Value) -> serde_json::Value {
    let strength = match &v {
        serde_json::Value::String(s) if s == "Orthographic" => 0.0,
        serde_json::Value::Object(obj) => obj
            .get("Perspective")
            .and_then(|p| p.get("strength"))
            .and_then(|s| s.as_f64())
            .unwrap_or(0.0),
        _ => 0.0,
    };
    serde_json::json!(strength)
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

fn default_levels_enabled() -> bool {
    super::defaults::DEFAULT_LEVELS_ENABLED
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
    super::defaults::DEFAULT_LEVELS_GAMMA  // Linear (no gamma adjustment)
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

fn is_default_levels_enabled(v: &bool) -> bool {
    *v == super::defaults::DEFAULT_LEVELS_ENABLED
}

fn is_default_filter_radius(v: &f32) -> bool {
    v.abs() < FLOAT_EPSILON  // Default is 0.0 (filter off)
}

fn is_default_filter_blur_edges(v: &f32) -> bool {
    v.abs() < FLOAT_EPSILON  // Default is 0.0 (strict edge preservation)
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
            // Scene-level render state (moved off Flame in v3). New flames
            // default to 2D / preserve_z=false (Apo/JWF semantics).
            render_mode: RenderMode::TwoD,
            preserve_z: false,
            perspective_strength: 0.0,
            depth_density_compensation: 0.0,
            far_density_fade: 0.0,
            far_density_fade_start: 0.0,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            camera_rotation_x: 0.0,
            camera_rotation_y: 0.0,
            camera_bank: 0.0,
            camera_x: 0.0,
            camera_y: 0.0,
            camera_z: 0.0,
            image_size: default_image_size(),
            dof_focus_distance: default_dof_focus_distance(),
            dof_blur_strength: 0.0,
            fog_strength: 0.0,
            fog_start: 0.0,
            filter_radius: 0.0,
            filter_blur_edges: 0.0,
            density_scale: 1.0,
            speed_factor: 0.5,
            max_iterations: default_max_iterations(),
            blend_factor: default_blend_factor(),
            use_dynamic_blend: default_use_dynamic_blend(),
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
            levels_enabled: default_levels_enabled(),
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
    /// Convert a screen-aligned XY delta into the pan coordinate frame.
    ///
    /// Both render pipelines apply pan BEFORE the screen rotation
    /// (`world_to_pixel` and `world_to_pixel_3d` share the
    /// pan → rotate → zoom composition — the Apophysis convention,
    /// where Pan X/Y are a position in the fractal plane). Screen-
    /// space movement therefore rotates by `−rotation` to land in
    /// pan coordinates, identically in 2D and 3D.
    ///
    /// Every input path that turns screen motion into a pan change
    /// (mouse drag, arrow keys, zoom-to-cursor, pinch) must go
    /// through this so the inputs stay consistent with the
    /// pipelines and with each other.
    pub fn screen_delta_to_pan_frame(&self, dx: f32, dy: f32) -> (f32, f32) {
        let cos_r = (-self.rotation).cos();
        let sin_r = (-self.rotation).sin();
        (dx * cos_r - dy * sin_r, dx * sin_r + dy * cos_r)
    }

    /// Build the compact, version-headed JSON **value**: defaults stripped,
    /// `version` first, `flame` next. This is the canonical serialized form
    /// — `to_json` is just this stringified, and the cloud config blob
    /// (`api::sync`) is this value minus the palette and root transforms.
    pub(crate) fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
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
            Ok(serde_json::Value::Object(ordered_obj))
        } else {
            Ok(value)
        }
    }

    /// Export configuration to JSON string with version header
    /// Omits fields that match defaults for compact output
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.to_json_value()?)
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
        if config.blend_factor == defaults.blend_factor { obj.remove("blend_factor"); }
        if config.use_dynamic_blend == defaults.use_dynamic_blend { obj.remove("use_dynamic_blend"); }

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

    /// Import configuration from a JSON string, with version-keyed migration.
    /// Both `.fflame` files and cloud config blobs deserialize through here
    /// (and `from_json_value`), so they share one migration path.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        Self::from_json_value(serde_json::from_str(json)?)
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

    /// Version-keyed migration on the raw JSON **value**, run BEFORE
    /// `from_value`. This is the crucial ordering: a field stripped at save
    /// time (because it equalled its default at *that* version) is *absent*
    /// here, so an arm can restore the version's old default with
    /// `obj.entry("field").or_insert(json!(old_default))`. If we migrated the
    /// typed struct instead, serde would already have filled the absent field
    /// with the *current* default and the old value would be unrecoverable.
    ///
    /// Each arm upgrades exactly one version; `version` is rewritten to current
    /// on success. Arms for bumps that change no defaults/shape are empty —
    /// serde's current default is already correct for every field.
    fn migrate_value(from_version: u32, value: &mut serde_json::Value) -> Result<(), serde_json::Error> {
        let obj = match value.as_object_mut() {
            Some(o) => o,
            // Non-object: let `from_value` surface the real type error.
            None => return Ok(()),
        };

        let mut version = from_version;
        while version < CURRENT_CONFIG_VERSION {
            match version {
                // v0 -> v1: pre-versioning configs. No structural/default
                // changes; serde defaults handle missing fields.
                0 => {}
                // v1 -> v2: opaque-blob format (docs/projects/api-v2.md). No
                // field defaults changed, so this is a no-op for now. The arm
                // exists so a *future* default change lands here as an explicit
                // `obj.entry("field").or_insert(json!(v1_default))` rather than
                // silently re-rendering old flames at the new default.
                1 => {}
                // v2 -> v3: lift the scene-level render fields from the nested
                // `flame` object up to the config top level (see
                // CURRENT_CONFIG_VERSION docs). Source of truth for these is
                // now `config.*`; the `flame` object loses them. Subflames'
                // (always-ignored) copies are left in place — `Flame`'s
                // deserializer ignores unknown fields and they drop on re-save.
                2 => {
                    // Extract from the flame object, then insert at top level
                    // (can't hold the `flame` borrow while mutating `obj`).
                    let (render_mode, perspective, depth_dc, far_fade, far_fade_start, preserve_z) =
                        if let Some(flame) = obj.get_mut("flame").and_then(|f| f.as_object_mut()) {
                            let render_mode = flame
                                .remove("render_mode")
                                .unwrap_or_else(|| serde_json::json!("2d"));
                            // perspective_strength, or the pre-rename legacy
                            // `projection` enum form.
                            let perspective = flame
                                .remove("perspective_strength")
                                .or_else(|| flame.remove("projection").map(legacy_projection_to_strength))
                                .unwrap_or_else(|| serde_json::json!(0.0));
                            let depth_dc = flame.remove("depth_density_compensation");
                            let far_fade = flame.remove("far_density_fade");
                            let far_fade_start = flame.remove("far_density_fade_start");
                            // Absent in pre-flag flames ⇒ true (kept their look).
                            let preserve_z = flame
                                .remove("preserve_z")
                                .unwrap_or_else(|| serde_json::json!(true));
                            (render_mode, perspective, depth_dc, far_fade, far_fade_start, preserve_z)
                        } else {
                            (
                                serde_json::json!("2d"),
                                serde_json::json!(0.0),
                                None,
                                None,
                                None,
                                serde_json::json!(true),
                            )
                        };
                    obj.insert("render_mode".to_string(), render_mode);
                    obj.insert("perspective_strength".to_string(), perspective);
                    if let Some(v) = depth_dc {
                        obj.insert("depth_density_compensation".to_string(), v);
                    }
                    if let Some(v) = far_fade {
                        obj.insert("far_density_fade".to_string(), v);
                    }
                    if let Some(v) = far_fade_start {
                        obj.insert("far_density_fade_start".to_string(), v);
                    }
                    obj.insert("preserve_z".to_string(), preserve_z);
                }
                other => {
                    return Err(serde_json::Error::io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Unknown config version {} during migration", other),
                    )));
                }
            }
            version += 1;
        }

        obj.insert("version".to_string(), serde_json::json!(CURRENT_CONFIG_VERSION));
        if from_version < CURRENT_CONFIG_VERSION {
            log::info!("Migrated config from version {} to {}", from_version, CURRENT_CONFIG_VERSION);
        }
        Ok(())
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

    /// Import a single configuration from a JSON value, migrating it (on the
    /// value, before deserialize) to the current version. Shared by `from_json`
    /// and the array path; the cloud-blob path (`api::sync`) reuses it too,
    /// which is why it's `pub(crate)`.
    pub(crate) fn from_json_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        // `.fflame` / cloud blobs without a version are genuinely pre-versioning
        // (v0).
        Self::from_json_value_with_default_version(value, 0)
    }

    /// Like [`from_json_value`] but uses `default_version` when the value has no
    /// `version` field, instead of assuming v0.
    ///
    /// The animation `base_config` path passes **2**: those configs were embedded
    /// by the raw struct serializer (which never wrote a `version`) at a time
    /// when the format was already v2, so an absent version means v2 — run only
    /// the v2→current migrations, not the v0/v1 ones.
    pub(crate) fn from_json_value_with_default_version(
        mut value: serde_json::Value,
        default_version: u32,
    ) -> Result<Self, serde_json::Error> {
        let version = value.get("version")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(default_version);

        if version > CURRENT_CONFIG_VERSION {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Config version {} is newer than supported version {}. Please update the application.",
                    version, CURRENT_CONFIG_VERSION
                ),
            )));
        }

        // Migrate on the value (restores version-specific defaults), THEN
        // deserialize — serde fills any still-absent field with the current
        // default, correct for every field a migration arm didn't touch.
        Self::migrate_value(version, &mut value)?;
        let mut config: Self = serde_json::from_value(value)?;

        // Assign session-local IDs to every Transform / Flame / Effect that
        // came in without one (all of them — IDs are serde-skipped).
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
        assert!(json.contains(&format!("\"version\": {}", CURRENT_CONFIG_VERSION)));
    }

    #[test]
    fn test_enums_serialize_snake_case_read_pascal_case() {
        use crate::scene::palette::ColorMode;
        use crate::scene::tonemap::ToneMapMode;
        use crate::scene::transforms::RenderMode;

        // Emit: the wire/blob form is snake_case (the server casts
        // `render_mode` straight into a Postgres enum of '2d'/'3d').
        let mut config = FractalConfig::default();
        config.render_mode = RenderMode::ThreeD;
        config.color_mode = ColorMode::PathMap;
        config.tonemap_mode = ToneMapMode::DensityVisualization;
        let json = config.to_json().unwrap();
        assert!(json.contains("\"3d\""), "render_mode must emit \"3d\"");
        assert!(json.contains("\"path_map\""), "color_mode must emit snake_case");
        assert!(json.contains("\"density\""), "tonemap_mode must emit \"density\"");
        assert!(!json.contains("ThreeD") && !json.contains("PathMap"));

        // Read: legacy PascalCase enum values still load via the serde
        // aliases. (render_mode is top-level config since v3.)
        let mut legacy = serde_json::to_value(FractalConfig::default()).unwrap();
        legacy["version"] = serde_json::json!(CURRENT_CONFIG_VERSION);
        legacy["render_mode"] = serde_json::json!("ThreeD");
        legacy["color_mode"] = serde_json::json!("PathMap");
        legacy["tonemap_mode"] = serde_json::json!("DensityVisualization");
        let restored = FractalConfig::from_json_value(legacy).unwrap();
        assert_eq!(restored.render_mode, RenderMode::ThreeD);
        assert_eq!(restored.color_mode, ColorMode::PathMap);
        assert_eq!(restored.tonemap_mode, ToneMapMode::DensityVisualization);
    }

    /// v2→v3 migration: a v2 blob with the scene-render fields nested under
    /// `flame` must lift them to the config top level, with `preserve_z`
    /// absent ⇒ true and the legacy `projection` form mapped to perspective.
    #[test]
    fn test_v2_to_v3_lifts_scene_render_fields() {
        use crate::scene::transforms::RenderMode;

        // Build a v2-shaped value: start from a default config, then move the
        // render fields back under `flame` and stamp version 2.
        let mut v2 = serde_json::to_value(FractalConfig::default()).unwrap();
        let obj = v2.as_object_mut().unwrap();
        obj.insert("version".into(), serde_json::json!(2));
        // These belong at top level in v3; put them under flame as a v2 file would.
        obj.remove("render_mode");
        obj.remove("preserve_z");
        obj.remove("perspective_strength");
        let flame = obj.get_mut("flame").unwrap().as_object_mut().unwrap();
        flame.insert("render_mode".into(), serde_json::json!("3d"));
        flame.insert("depth_density_compensation".into(), serde_json::json!(0.5));
        // preserve_z intentionally absent ⇒ should migrate to true.
        // Legacy `projection` enum form ⇒ perspective_strength 2.0.
        flame.insert(
            "projection".into(),
            serde_json::json!({ "Perspective": { "strength": 2.0 } }),
        );

        let cfg = FractalConfig::from_json_value(v2).unwrap();
        assert_eq!(cfg.render_mode, RenderMode::ThreeD, "render_mode lifted");
        assert_eq!(cfg.depth_density_compensation, 0.5, "depth lifted");
        assert!(cfg.preserve_z, "absent preserve_z ⇒ true");
        assert_eq!(cfg.perspective_strength, 2.0, "legacy projection ⇒ perspective");
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
