//! Default values for configuration parameters
//!
//! Single source of truth for all default values used throughout the codebase.
//! Changing a value here updates all usages.

// Histogram & Color
pub const DEFAULT_HISTOGRAM_COLOR_SCALE: f32 = 100.0; // Max precision: 100 color levels
pub const DEFAULT_EXPOSURE: f32 = 1.0;
pub const DEFAULT_GAMMA: f32 = 1.0;
pub const DEFAULT_BRIGHTNESS: f32 = 1.0;
pub const DEFAULT_SATURATION: f32 = 1.0; // 1.0 = no change, >1.0 = more saturated
pub const DEFAULT_HUE_SHIFT: f32 = 0.0; // 0.0 = no shift, range -180.0 to 180.0 degrees
pub const DEFAULT_VALUE_SCALE: f32 = 1.0; // 1.0 = no change, >1.0 = brighter

// Apophysis brightness constants (ControlPoint.pas:37-39)
pub const BRIGHT_ADJUST: f32 = 2.3;
pub const PREFILTER_WHITE: f32 = 67108864.0; // (1 shl 26)
pub const DEFAULT_WHITE_LEVEL: f32 = 200.0;

// Accumulation
pub const DEFAULT_LOW_DENSITY_SMOOTHING: f32 = 0.5; // Moderate smoothing
pub const DEFAULT_DENSITY_COMPRESSION: f32 = 0.0; // No compression
pub const DEFAULT_BLEND_FACTOR: f32 = 0.1; // 10% blend rate
pub const DEFAULT_USE_DYNAMIC_BLEND: bool = true; // Exponential convergence

// Performance
pub const DEFAULT_ITERATIONS_PER_THREAD: u32 = 256;
pub const DEFAULT_SPEED_MULTIPLIER: u32 = 1;
pub const DEFAULT_TARGET_ITERATIONS_PER_PIXEL: u64 = 0; // Disabled

// Other
pub const DEFAULT_MAX_ITERATIONS: u64 = 1_000_000_000;
pub const DEFAULT_SPEED_FACTOR: f32 = 0.5;
pub const DEFAULT_DENSITY_SCALE: f32 = 1.0;
