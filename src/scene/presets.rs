//! Preset library for fractal configurations
//!
//! Presets are loaded from `assets/presets.fflame` on both Desktop and WASM.
//! Falls back to a simple default preset if loading fails.

use crate::config::FractalConfig;

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
    /// Create a new preset library by loading from assets/presets.fflame
    ///
    /// This works identically on Desktop and WASM - both load from the same file.
    /// Falls back to a simple default preset if loading fails.
    pub fn new() -> Self {
        let presets = crate::resources::load_presets_with_fallback();
        Self { presets }
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

// ===== GLOBAL SINGLETON =====

use once_cell::sync::Lazy;

/// Global preset library singleton (immutable)
///
/// This provides a single shared instance of PresetLibrary for the entire application.
/// Since PresetLibrary is immutable after creation, no locking is needed.
///
/// # Usage
/// ```ignore
/// let library = global_preset_library();
/// let preset = library.get(0);
/// ```
pub fn global_preset_library() -> &'static PresetLibrary {
    static LIBRARY: Lazy<PresetLibrary> = Lazy::new(PresetLibrary::new);
    &LIBRARY
}
