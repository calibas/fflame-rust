//! Default values for configuration parameters
//!
//! Single source of truth for all default values used throughout the codebase.
//! Changing a value here updates all usages.
//!
//! ⚠️ WARNING: Changing defaults affects config serialization!
//! With compact serialization (omitting defaults), old configs rely on these values.
//! If you change a default:
//!   1. Bump CURRENT_CONFIG_VERSION in fractal_config.rs
//!   2. Add migration to preserve old default for existing configs
//!   3. Document the change in VERSION_HISTORY
//! See docs/projects/config-versioning.md for details.
//!
//! See also `docs/projects/config-v2-migration.md` for the running list
//! of changes pending a v1 → v2 bump (tonemap defaults shift, levels_high
//! units conversion, etc.).

// Histogram & Color
pub const DEFAULT_HISTOGRAM_COLOR_SCALE: f32 = 100.0; // Max precision: 100 color levels
pub const DEFAULT_EXPOSURE: f32 = 0.5; // Was 1.0 — calibrated against scale-invariant Levels
pub const DEFAULT_GAMMA: f32 = 1.5; // Was 1.0 — same recalibration
pub const DEFAULT_BRIGHTNESS: f32 = 1.0;
pub const DEFAULT_SATURATION: f32 = 1.0; // 1.0 = no change, >1.0 = more saturated
pub const DEFAULT_HUE_SHIFT: f32 = 0.0; // 0.0 = no shift, range -180.0 to 180.0 degrees
pub const DEFAULT_PALETTE_ROTATION: f32 = 0.0; // -1.0 to 1.0, shifts palette indices (Apophysis: -128 to 128)
pub const DEFAULT_PALETTE_SIZE: u32 = 256; // 256-4096, palette texture resolution
pub const DEFAULT_PALETTE_SQUEEZE: f32 = 1.0; // 1.0 = no change, >1 = repeat palette, <1 = show portion

// Levels (in "× mean density" units since the scale-invariance change)
pub const DEFAULT_LEVELS_HIGH: f32 = 10.0; // Was 1.0 — clip at 10× mean instead of at mean
pub const DEFAULT_LEVELS_GAMMA: f32 = 1.0; // Linear; <1 compresses toward opaque, >1 toward transparent

// Apophysis brightness constants (ControlPoint.pas:37-39)
pub const BRIGHT_ADJUST: f32 = 2.3;
pub const PREFILTER_WHITE: f32 = 67108864.0; // (1 shl 26)
pub const DEFAULT_WHITE_LEVEL: f32 = 200.0;
pub const DEFAULT_GAMMA_THRESHOLD: f32 = 5.0; // Was 0.0025 — recalibrated alongside other tonemap defaults

// Alpha blending curve (background compositing)
// Controls transition from gamma-corrected alpha (edges) to linear alpha (detail)
pub const DEFAULT_ALPHA_BLEND_LOW: f32 = 0.3; // Start blending toward linear at this alpha
pub const DEFAULT_ALPHA_BLEND_HIGH: f32 = 0.8; // Full linear alpha above this value

// Accumulation
pub const DEFAULT_BLEND_FACTOR: f32 = 0.1; // 10% blend rate (used when dynamic blend is off)
pub const DEFAULT_USE_DYNAMIC_BLEND: bool = true; // Clamped exponential: 0.8 → 0.01

// Performance
pub const DEFAULT_ITERATIONS_PER_THREAD: u32 = 256;
pub const DEFAULT_TARGET_ITERATIONS_PER_PIXEL: u64 = 0; // Disabled

// Depth of Field
pub const DEFAULT_DOF_FOCUS_DISTANCE: f32 = 1.0; // Distance from origin where image is sharpest
pub const DEFAULT_DOF_BLUR_STRENGTH: f32 = 0.0; // 0.0 = disabled, up to ~10.0 for strong blur

// Depth Fog (atmospheric perspective)
pub const DEFAULT_FOG_STRENGTH: f32 = 0.0; // 0.0 = disabled, exponential fog density
pub const DEFAULT_FOG_START: f32 = 0.0; // Depth where fog begins (camera-space Z)

// Other
pub const DEFAULT_MAX_ITERATIONS: u64 = 1_000_000_000;
pub const DEFAULT_SPEED_FACTOR: f32 = 0.5;
pub const DEFAULT_DENSITY_SCALE: f32 = 1.0;
