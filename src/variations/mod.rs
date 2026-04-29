use std::collections::HashMap;
use serde::{Deserialize, Serialize};

pub mod definition;
pub mod defs;

use definition::VariationDef;

/// Parameter type for variation parameters
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ParamType {
    /// Continuous floating-point value with min/max bounds
    Float,
    /// Unlimited floating-point value (full f32 range: -3.4E38 to +3.4E38)
    /// Uses min/max as slider range (default -10.0 to 10.0), but allows typing any value
    UnlimitedFloat,
    /// Integer value (stored as f32, cast for UI)
    Integer,
    /// Unlimited integer value (full i32 range: -2.1B to +2.1B)
    /// Uses min/max as slider range (default -100 to 100), but allows typing any integer
    UnlimitedInteger,
    /// Boolean value (0.0 = false, non-zero = true)
    Boolean,
    /// Angle in degrees (0-360, or custom range)
    Angle,
    /// Enum/choice value with discrete options
    /// Values stored as indices (0, 1, 2, ...)
    Enum {
        /// Display labels for each choice
        choices: Vec<String>,
    },
}

/// Helper function to simplify Enum creation
impl ParamType {
    pub fn enum_choices<S: AsRef<str>>(choices: &[S]) -> Self {
        ParamType::Enum {
            choices: choices.iter().map(|s| s.as_ref().to_string()).collect(),
        }
    }
}

/// Definition of a single variation parameter
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VariationParameter {
    /// Parameter name (e.g., "power", "dist")
    pub name: String,

    /// Display name for UI (e.g., "Power", "Distance")
    pub display_name: String,

    /// Parameter type
    pub param_type: ParamType,

    /// Default value
    pub default_value: f32,

    /// Minimum value (None = no limit)
    pub min_value: Option<f32>,

    /// Maximum value (None = no limit)
    pub max_value: Option<f32>,
}

/// Execution phase for variations (Apophysis XForm.pas:343-383)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VariationPhase {
    /// Pre-variations: Directly modify input coordinates FTx/FTy/FTz (NOT weighted sum)
    /// Execute before normal variations and precalculation
    Pre,

    /// Normal variations: Weighted sum accumulation to output FPx/FPy/FPz
    /// Execute after precalculation
    Normal,

    /// Post-variations: Directly modify output coordinates FPx/FPy/FPz (NOT weighted sum)
    /// Execute after all normal variations
    /// NOTE: Flatten is treated as post despite being index 1!
    Post,
}

/// Variation metadata and registration
#[derive(Clone, Debug)]
pub struct VariationInfo {
    /// Unique name (e.g., "linear", "sinusoidal", "curl_3d")
    pub name: String,

    /// Display name for UI
    pub display_name: String,

    /// Category for organization
    pub category: VariationCategory,

    /// Execution phase (pre/normal/post)
    pub phase: VariationPhase,

    /// WGSL function name (e.g., "variation_linear")
    pub wgsl_function: String,

    /// Whether this variation needs RNG
    pub needs_rng: bool,

    /// Whether this variation needs `xform_id` for reads from the
    /// per-transform `transforms[xform_id]` storage buffer (affine, weight,
    /// color, etc.). When true, the function signature includes
    /// `xform_id: u32` even for variations without parameters.
    pub needs_transform: bool,

    /// Whether this variation writes the iteration-local color register `vc`
    /// (Apophysis direct-color variations). When true, the WGSL signature
    /// gains `vc: ptr<function, f32>`. The shader builder uses this to
    /// detect whether any DC variation is active and emit the Step 3 lerp.
    pub writes_color: bool,

    /// Whether this is a core (built-in) or plugin variation
    pub is_core: bool,

    /// Optional: WGSL source code for 2D (for plugins loaded at runtime)
    pub wgsl_source: Option<String>,

    /// Optional: WGSL source code for 3D
    pub wgsl_source_3d: Option<String>,

    /// Optional: WGSL source code for the init function. When `Some`, a small
    /// GPU compute dispatch runs `init_<name>(user_params)` once per param
    /// change and writes derived values into the variation_params buffer.
    pub wgsl_source_init: Option<String>,

    /// Number of init-derived parameters this variation produces. Stored
    /// alongside user parameters in the buffer at slots
    /// `parameters.len()..parameters.len() + init_param_count`.
    pub init_param_count: usize,

    /// Parameters for this variation
    pub parameters: Vec<VariationParameter>,

    /// Version number. Built-in variations use 0; API-loaded variations
    /// use the server's version. Used for cache invalidation.
    pub version: u32,
}

impl VariationInfo {
    /// Get the default value for a parameter by name
    pub fn get_param_default(&self, param_name: &str) -> Option<f32> {
        self.parameters
            .iter()
            .find(|p| p.name == param_name)
            .map(|p| p.default_value)
    }

    /// Get parameter definition by name
    pub fn get_param(&self, param_name: &str) -> Option<&VariationParameter> {
        self.parameters.iter().find(|p| p.name == param_name)
    }

    /// Create from an API VariationDownload response
    pub fn from_download(dl: &crate::api::types::VariationDownload) -> Self {
        let parameters = dl.parameters.iter().map(|p| VariationParameter {
            name: p.name.clone(),
            display_name: p.display_name.clone(),
            param_type: api_param_type_to_runtime(&p.param_type),
            default_value: p.default_value,
            min_value: p.min_value,
            max_value: p.max_value,
        }).collect();

        let wgsl_function = format!("variation_{}", dl.name);

        Self {
            name: dl.name.clone(),
            display_name: dl.display_name.clone(),
            category: VariationCategory::from_api_str(&dl.category),
            phase: api_phase_to_runtime(&dl.phase),
            wgsl_function,
            needs_rng: dl.needs_rng,
            needs_transform: dl.needs_transform,
            writes_color: dl.writes_color,
            is_core: false,
            wgsl_source: Some(dl.shader_2d.clone()),
            wgsl_source_3d: dl.shader_3d.clone(),
            wgsl_source_init: dl.shader_init.clone(),
            init_param_count: dl.init_param_count,
            parameters,
            version: dl.version,
        }
    }

    /// Create from a static VariationDef
    pub fn from_def(def: &VariationDef) -> Self {
        Self {
            name: def.name.to_string(),
            display_name: def.display_name.to_string(),
            category: def.category.clone(),
            phase: def.phase.clone(),
            wgsl_function: def.wgsl_function_name(),
            needs_rng: def.needs_rng,
            needs_transform: def.needs_transform,
            writes_color: def.writes_color,
            is_core: true, // All VariationDef are core variations
            wgsl_source: Some(def.wgsl_2d.to_string()),
            wgsl_source_3d: def.wgsl_3d.map(|s| s.to_string()),
            wgsl_source_init: def.wgsl_init.map(|s| s.to_string()),
            init_param_count: def.init_param_count,
            parameters: def.parameters_to_runtime(),
            version: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum VariationCategory {
    /// Basic 2D variations (Linear, Sinusoidal, etc.)
    Basic2D,

    /// Advanced 2D variations (Polar, Julia, etc.)
    Advanced2D,

    /// 3D depth variations (Zcone, Flatten, etc.)
    Depth3D,

    /// 3D rotation variations (PreRotateX, etc.)
    Rotation3D,

    /// Full 3D variations (Hemisphere, etc.)
    Full3D,

    /// Plugin variations
    Plugin,
}

/// Convert API param type to runtime ParamType
fn api_param_type_to_runtime(api: &crate::api::types::ApiParamType) -> ParamType {
    use crate::api::types::ApiParamType;
    match api {
        ApiParamType::Float => ParamType::Float,
        ApiParamType::UnlimitedFloat => ParamType::UnlimitedFloat,
        ApiParamType::Integer => ParamType::Integer,
        ApiParamType::UnlimitedInteger => ParamType::UnlimitedInteger,
        ApiParamType::Boolean => ParamType::Boolean,
        ApiParamType::Angle => ParamType::Angle,
        ApiParamType::Enum { choices } => ParamType::Enum { choices: choices.clone() },
    }
}

/// Convert API phase to runtime VariationPhase
fn api_phase_to_runtime(api: &crate::api::types::ApiVariationPhase) -> VariationPhase {
    use crate::api::types::ApiVariationPhase;
    match api {
        ApiVariationPhase::Pre => VariationPhase::Pre,
        ApiVariationPhase::Normal => VariationPhase::Normal,
        ApiVariationPhase::Post => VariationPhase::Post,
    }
}

impl VariationCategory {
    /// Parse from API string (matches API's snake_case).
    /// Unknown values map to Plugin as a safe default.
    pub fn from_api_str(s: &str) -> Self {
        match s {
            "basic_2d" | "basic2d" => Self::Basic2D,
            "advanced_2d" | "advanced2d" => Self::Advanced2D,
            "depth_3d" | "depth3d" | "3d" => Self::Depth3D,
            "rotation_3d" | "rotation3d" => Self::Rotation3D,
            "full_3d" | "full3d" => Self::Full3D,
            _ => Self::Plugin,
        }
    }
}

/// Registry of all available variations
#[derive(Clone, Debug)]
pub struct VariationRegistry {
    /// Map of variation name -> info
    variations: HashMap<String, VariationInfo>,

    /// Ordered list of variation names (for consistent ID assignment)
    ordered_names: Vec<String>,

    /// Bumped whenever a variation is added, replaced, or removed at runtime.
    /// Used by the shader cache to detect when a rebuild is needed even if
    /// the flame's referenced variation names haven't changed.
    version: u64,
}

impl VariationRegistry {
    /// Create a new registry with core variations
    ///
    /// Loads variations from static VariationDef definitions in defs module.
    /// All variations with embedded WGSL code are loaded from there.
    pub fn new() -> Self {
        let mut registry = Self {
            variations: HashMap::new(),
            ordered_names: Vec::new(),
            version: 0,
        };

        log::info!("=== VARIATION REGISTRY INITIALIZATION ===");

        // Load all variations from static definitions
        // The order in ALL_VARIATIONS determines the variation indices
        for def in defs::ALL_VARIATIONS.iter() {
            registry.register_from_def(def);
        }

        // All variations are now loaded from VariationDef static definitions
        // Parameters are defined directly in their respective VariationDef

        // DEBUG: Print final registry order
        log::info!("Final variation registry (name -> index):");
        for (i, name) in registry.ordered_names.iter().enumerate() {
            log::info!("  [{}] = {}", i, name);
        }
        log::info!("Total variations: {}", registry.ordered_names.len());

        registry
    }

    /// Register a core (built-in) variation from a static definition
    fn register_from_def(&mut self, def: &VariationDef) {
        let info = VariationInfo::from_def(def);
        self.ordered_names.push(info.name.clone());
        self.variations.insert(info.name.clone(), info);
    }

    /// Register or replace an API-loaded variation.
    /// If a variation with the same name already exists, it's replaced
    /// (e.g., when a newer version is fetched). Built-in variations
    /// can't be replaced — the call is rejected with a logged warning.
    pub fn register_from_api(&mut self, dl: &crate::api::types::VariationDownload) {
        if let Some(existing) = self.variations.get(&dl.name) {
            if existing.is_core {
                log::warn!(
                    "Cannot register API variation '{}' — name conflicts with built-in",
                    dl.name
                );
                return;
            }
        }
        let info = VariationInfo::from_download(dl);
        if !self.ordered_names.contains(&info.name) {
            self.ordered_names.push(info.name.clone());
        }
        log::info!("Registered API variation '{}' v{}", info.name, info.version);
        self.variations.insert(info.name.clone(), info);
        self.version = self.version.wrapping_add(1);
    }

    /// Remove all API-loaded (non-core) variations.
    /// Built-in variations are preserved. Used by the "Clear Variation Cache" action.
    pub fn clear_api(&mut self) {
        let removed: Vec<String> = self.variations.iter()
            .filter(|(_, info)| !info.is_core)
            .map(|(name, _)| name.clone())
            .collect();
        for name in &removed {
            self.variations.remove(name);
        }
        self.ordered_names.retain(|name| self.variations.contains_key(name));
        log::info!("Cleared {} API-loaded variations from registry", removed.len());
        if !removed.is_empty() {
            self.version = self.version.wrapping_add(1);
        }
    }

    /// Get the registry's version counter. Bumped on any runtime add/remove.
    /// Used by the shader cache to detect when a rebuild is needed.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Check if a variation is registered (built-in or API).
    pub fn has(&self, name: &str) -> bool {
        self.variations.contains_key(name)
    }

    /// Get variation info by name
    pub fn get(&self, name: &str) -> Option<&VariationInfo> {
        self.variations.get(name)
    }

    /// Get all variation names in order
    pub fn names(&self) -> &[String] {
        &self.ordered_names
    }

    /// Get variations by category (in registration order)
    pub fn by_category(&self, category: VariationCategory) -> Vec<&VariationInfo> {
        // Iterate ordered_names to preserve registration order (numerical ID order)
        // This ensures UI displays variations in consistent order
        self.ordered_names
            .iter()
            .filter_map(|name| self.variations.get(name))
            .filter(|v| v.category == category)
            .collect()
    }

    /// Assign runtime IDs to active variations
    /// Returns a map of variation name -> shader ID
    pub fn assign_ids(&self, active_names: &[String]) -> HashMap<String, u32> {
        active_names
            .iter()
            .enumerate()
            .map(|(id, name)| (name.clone(), id as u32))
            .collect()
    }

    /// Get all variations (for UI)
    pub fn all(&self) -> Vec<&VariationInfo> {
        self.ordered_names
            .iter()
            .filter_map(|name| self.variations.get(name))
            .collect()
    }

}

impl Default for VariationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

fn registry_lock() -> &'static RwLock<VariationRegistry> {
    use once_cell::sync::Lazy;
    static REGISTRY: Lazy<RwLock<VariationRegistry>> = Lazy::new(|| RwLock::new(VariationRegistry::new()));
    &REGISTRY
}

/// Get a read guard to the global variation registry singleton.
/// Initialized once, shared across all code paths.
pub fn global_registry() -> RwLockReadGuard<'static, VariationRegistry> {
    registry_lock().read().expect("variation registry RwLock poisoned")
}

/// Get a write guard to the global variation registry. Use sparingly —
/// only for adding/removing API-loaded variations at runtime.
pub fn global_registry_mut() -> RwLockWriteGuard<'static, VariationRegistry> {
    registry_lock().write().expect("variation registry RwLock poisoned")
}

/// Scan a flame's transforms for variation names not registered.
/// Returns the deduplicated list of missing names (empty if all are present).
pub fn missing_variations_in(flame: &crate::scene::transforms::Flame) -> Vec<String> {
    let registry = global_registry();
    let mut missing = std::collections::HashSet::new();
    for xform in &flame.transforms {
        for name in xform.variations.keys() {
            if xform.variations.get(name).copied().unwrap_or(0.0) == 0.0 {
                continue; // weight 0 — not actually used
            }
            if !registry.has(name) {
                missing.insert(name.clone());
            }
        }
    }
    if let Some(ref final_xform) = flame.final_transform {
        for name in final_xform.variations.keys() {
            if final_xform.variations.get(name).copied().unwrap_or(0.0) == 0.0 {
                continue;
            }
            if !registry.has(name) {
                missing.insert(name.clone());
            }
        }
    }
    missing.into_iter().collect()
}

/// Load all cached API variations from disk/storage and register them.
/// Call once at app startup, after the global registry is initialized.
/// Errors for individual variations are logged but don't fail the load.
pub fn load_cached_api_variations() {
    let cached = crate::storage::variation_cache::load_all();
    if cached.is_empty() {
        return;
    }
    let mut registry = global_registry_mut();
    for download in cached {
        registry.register_from_api(&download);
    }
}
