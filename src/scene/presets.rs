//! Preset library for fractal configurations
//!
//! Presets are loaded from `assets/presets.fae` on both Desktop and WASM.
//! Falls back to a simple default preset if loading fails.

use crate::config::FractalConfig;

/// **The Presets tab's categories**, each its own gallery and tab. A
/// gallery makes its missing previews only while it is shown, those on
/// screen first, so a category's are made when it is opened -- not every
/// preset's at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PresetCategory {
    #[default]
    Flame2d,
    Flame3d,
    Escape2d,
    Escape3d,
    Simulation,
}

impl PresetCategory {
    pub const ALL: [PresetCategory; 5] =
        [Self::Flame2d, Self::Flame3d, Self::Escape2d, Self::Escape3d, Self::Simulation];

    /// The category a config falls in: by the camera its picture is seen
    /// through (`camera::gesture::view_kind`), an escape solid or terrain
    /// being a 3D escape picture. A Rhai solid is told from its source,
    /// which is not compiled yet when the presets are sorted, so the
    /// solids' registry does not know it.
    pub fn of(config: &FractalConfig) -> Self {
        use crate::camera::gesture::{view_kind, ViewKind};
        match view_kind(config) {
            ViewKind::Flame2d => Self::Flame2d,
            ViewKind::Flame3d => Self::Flame3d,
            ViewKind::EscapePlane if !runs_a_rhai_solid(config) => Self::Escape2d,
            ViewKind::EscapePlane | ViewKind::Solid | ViewKind::EscapeTerrain => Self::Escape3d,
            ViewKind::Sim2d | ViewKind::SimTerrain => Self::Simulation,
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Flame2d => "browser.presets_flame_2d",
            Self::Flame3d => "browser.presets_flame_3d",
            Self::Escape2d => "browser.presets_escape_2d",
            Self::Escape3d => "browser.presets_escape_3d",
            Self::Simulation => "browser.presets_simulation",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// Whether the config's formula is its own Rhai source, and that source a
/// solid. The source stays in the config when another formula is picked
/// (undo finds it), so the formula's name has to be a custom one too.
fn runs_a_rhai_solid(config: &FractalConfig) -> bool {
    use crate::config::escape::CustomDialect;
    use crate::lang::{rhai, EntryKind};
    let escape = &config.escape;
    escape.formula.starts_with("custom-")
        && escape
            .custom_formula
            .as_ref()
            .is_some_and(|src| src.dialect == CustomDialect::Rhai && rhai::kind_of(&src.source) == Some(EntryKind::Solid))
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
    /// Create a new preset library by loading from assets/presets.fae
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

use std::sync::LazyLock;

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
    static LIBRARY: LazyLock<PresetLibrary> = LazyLock::new(PresetLibrary::new);
    &LIBRARY
}
