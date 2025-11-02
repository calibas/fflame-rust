/// Configuration and state management module
///
/// This module implements a delta-based state management system where all
/// configuration changes flow through a central ConfigManager. Changes are
/// tracked as deltas (what changed) rather than storing full configs, enabling:
///
/// - Smart undo/redo with delta storage
/// - Selective updates (only recompute what's needed)
/// - Human-readable change descriptions
/// - Centralized lazy undo throttling

pub mod defaults;
pub mod delta;
pub mod fractal_config;
pub mod manager;
pub mod slider;

pub use defaults::*;
pub use delta::{
    AffineParam, ColorComponent, ConfigChange, ConfigDelta, ConfigPath, ConfigValue, UpdateType,
};
pub use fractal_config::FractalConfig;
pub use manager::{ConfigError, ConfigManager, UpdateAction};
pub use slider::{ConfigSlider, ConfigSliderResult, ConfigSliderUi, LazyUndoUi};
