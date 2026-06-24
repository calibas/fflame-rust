use std::collections::HashMap;
use serde::{Deserialize, Serialize};

pub mod definition;
pub mod defs;
pub mod analytic_blur;

use definition::VariationDef;
pub use definition::Feature;

/// Parameter type for variation parameters.
///
/// Note on `Enum.choices`: `&'static [&'static str]` so the type is
/// `const`-compatible — enum params can live in `pub static
/// VariationDef`. For API-loaded variations, the conversion in
/// `api_param_type_to_runtime` leaks owned strings to obtain
/// `&'static` lifetimes (memory cost is bounded by the number of
/// distinct enum variations loaded — trivial in practice).
#[derive(Clone, Debug, Serialize, PartialEq)]
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
        choices: &'static [&'static str],
    },
}

/// Definition of a single variation parameter
#[derive(Clone, Debug, Serialize)]
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

    /// Free-form help / tooltip prose shown under the parameter
    /// control. Populated from `VariationParamDef.description` for
    /// built-ins, from `ApiVariationParameter.description` for API
    /// loads. `None` renders the control with no tooltip.
    pub description: Option<String>,
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

    /// Phase-agnostic: the variation may be assigned to any phase per
    /// instance via JWildfire's `fx_priority` (see
    /// [`crate::scene::transforms::Transform::variation_priorities`]).
    /// This is the **opt-in** marker — `Pre`/`Normal`/`Post` variations
    /// stay locked to their phase and ignore `fx_priority`; only `Any`
    /// variations honour it. With no override the variation defaults to
    /// the **normal** bucket. The combine form when moved to pre/post is
    /// chosen by [`crate::variations::definition::Feature::Replace`].
    /// See `docs/projects/jwf-features.md`.
    Any,
}

impl VariationPhase {
    /// The JWildfire `fx_priority` integer a variation runs at in its
    /// natural (no-override) phase: `Pre`→−1, `Normal`/`Any`→0, `Post`→1.
    /// Used on import to decide whether a parsed `fx_priority` is an
    /// actual override worth storing (sparse model) and at shader-build
    /// time as the default bucket.
    pub fn natural_priority(&self) -> i32 {
        match self {
            VariationPhase::Pre => -1,
            VariationPhase::Normal | VariationPhase::Any => 0,
            VariationPhase::Post => 1,
        }
    }
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

    /// Capability/requirement flags. See [`crate::variations::definition::Feature`]
    /// for variant docs. Mirrors `VariationDef::features` but as a `Vec`
    /// because it's the runtime side (API-loaded variations build it at
    /// download time). Lookup via [`Self::has_feature`].
    pub features: Vec<crate::variations::definition::Feature>,

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

    /// Number of f32 state slots this variation owns per (xform, variation)
    /// instance. State is per-(thread, xform, variation), zero-initialized
    /// each shader invocation, and persists across the inner iteration loop.
    /// Default 0 (no state). See `intra-iteration-state-and-accum.md`.
    pub state_count: usize,

    /// Optional WGSL fragment that runs at thread start to initialize state
    /// slots beyond zero-fill. Default None.
    pub wgsl_source_state_init: Option<String>,

    /// Parameters for this variation
    pub parameters: Vec<VariationParameter>,

    /// Version number. Built-in variations use 0; API-loaded variations
    /// use the server's version. Used for cache invalidation.
    pub version: u32,
}

impl VariationInfo {
    /// Total slots this variation occupies in the packed parameter buffer.
    ///
    /// Equals `parameters.len() + init_param_count`. User params live in
    /// slots `[0, parameters.len())` and init-derived params live in slots
    /// `[parameters.len(), parameters.len() + init_param_count)`.
    pub fn slot_count(&self) -> usize {
        self.parameters.len() + self.init_param_count
    }

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
            description: p.description.clone(),
        }).collect();

        let wgsl_function = format!("variation_{}", dl.name);

        // API contract still exposes the old bool fields. Derive the
        // runtime `features` slice from them here so the rest of the
        // codebase only ever sees the consolidated representation.
        // (When the API contract gets extended to ship a Vec<Feature>
        // directly, drop this derivation and read it through.)
        use crate::variations::definition::Feature;
        let mut features: Vec<Feature> = Vec::new();
        if dl.needs_rng { features.push(Feature::NeedsRng); }
        if dl.needs_transform { features.push(Feature::NeedsTransform); }
        if dl.writes_color { features.push(Feature::WritesColor); }
        // dl.needs_accum / writes_rgb don't exist in the API contract yet
        // — default to absent. Same reason state_count defaults to 0 below.

        Self {
            name: dl.name.clone(),
            display_name: dl.display_name.clone(),
            category: VariationCategory::from_api_str(&dl.category),
            phase: api_phase_to_runtime(&dl.phase),
            wgsl_function,
            features,
            is_core: false,
            wgsl_source: Some(dl.shader_2d.clone()),
            wgsl_source_3d: dl.shader_3d.clone(),
            wgsl_source_init: dl.shader_init.clone(),
            init_param_count: dl.init_param_count,
            // API-loaded variations do not yet carry state metadata.
            // Default to stateless until the API contract is extended
            // (separate project — only adds fields, no breakage).
            state_count: 0,
            wgsl_source_state_init: None,
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
            features: def.features.to_vec(),
            is_core: true, // All VariationDef are core variations
            wgsl_source: Some(def.wgsl_2d.to_string()),
            wgsl_source_3d: Some(def.wgsl_3d.to_string()),
            wgsl_source_init: def.wgsl_init.map(|s| s.to_string()),
            init_param_count: def.init_param_count,
            state_count: def.state_count,
            wgsl_source_state_init: def.wgsl_state_init.map(|s| s.to_string()),
            parameters: def.parameters_to_runtime(),
            version: 0,
        }
    }

    /// True if this variation lists the given feature. Mirrors
    /// [`VariationDef::has_feature`] for the runtime side.
    pub fn has_feature(&self, f: crate::variations::definition::Feature) -> bool {
        self.features.contains(&f)
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

/// Convert API param type to runtime ParamType.
///
/// For `Enum`, leaks the choice strings to obtain the `&'static`
/// lifetime that `ParamType::Enum.choices` requires. Memory is
/// bounded by the count of distinct enum-bearing variations loaded
/// from the API — small in practice (one allocation per choice plus
/// one for the slice, freed never but never re-allocated either).
fn api_param_type_to_runtime(api: &crate::api::types::ApiParamType) -> ParamType {
    use crate::api::types::ApiParamType;
    match api {
        ApiParamType::Float => ParamType::Float,
        ApiParamType::UnlimitedFloat => ParamType::UnlimitedFloat,
        ApiParamType::Integer => ParamType::Integer,
        ApiParamType::UnlimitedInteger => ParamType::UnlimitedInteger,
        ApiParamType::Boolean => ParamType::Boolean,
        ApiParamType::Angle => ParamType::Angle,
        ApiParamType::Enum { choices } => {
            let leaked_strs: Vec<&'static str> = choices
                .iter()
                .map(|s| &*Box::leak(s.clone().into_boxed_str()))
                .collect();
            ParamType::Enum {
                choices: Box::leak(leaked_strs.into_boxed_slice()),
            }
        }
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

    /// Map of foreign-app alias name -> our canonical variation name.
    /// Populated from each `VariationDef::aliases` slice at register
    /// time. Consulted by `get()` and `has()` so XML imports that use
    /// other apps' names (e.g. `linear3D` from Apo 7X / JWildfire when
    /// we only have `linear`) resolve to the right variation.
    aliases: HashMap<String, String>,

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
            aliases: HashMap::new(),
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
        // Index foreign-app aliases pointing to this variation's canonical
        // name so XML import lookups by alias resolve correctly.
        for alias in def.aliases {
            let alias = (*alias).to_string();
            if let Some(existing) = self.aliases.get(&alias) {
                log::warn!(
                    "Alias '{}' already maps to '{}'; ignoring duplicate from '{}'",
                    alias, existing, info.name
                );
                continue;
            }
            if self.variations.contains_key(&alias) {
                log::warn!(
                    "Alias '{}' for '{}' conflicts with an existing variation name; ignoring",
                    alias, info.name
                );
                continue;
            }
            self.aliases.insert(alias, info.name.clone());
        }
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
    /// Resolves foreign-app aliases to the canonical name.
    pub fn has(&self, name: &str) -> bool {
        if self.variations.contains_key(name) {
            return true;
        }
        if let Some(canonical) = self.aliases.get(name) {
            return self.variations.contains_key(canonical);
        }
        false
    }

    /// Get variation info by name. If the lookup misses, also tries the
    /// alias table so foreign-app names (e.g. `linear3D` from Apo 7X /
    /// JWildfire) resolve to our canonical variation.
    pub fn get(&self, name: &str) -> Option<&VariationInfo> {
        if let Some(info) = self.variations.get(name) {
            return Some(info);
        }
        if let Some(canonical) = self.aliases.get(name) {
            return self.variations.get(canonical);
        }
        None
    }

    /// Resolve a name through the alias table without doing the
    /// VariationInfo lookup. Useful at XML import time when the caller
    /// wants to record the canonical name on the Transform (so the rest
    /// of the pipeline never sees the alias).
    pub fn resolve_alias<'a>(&'a self, name: &'a str) -> &'a str {
        if self.variations.contains_key(name) {
            return name;
        }
        if let Some(canonical) = self.aliases.get(name) {
            return canonical.as_str();
        }
        name
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
    let mut scan = |xform: &crate::scene::transforms::Transform| {
        for name in xform.variations.keys() {
            if xform.variations.get(name).copied().unwrap_or(0.0) == 0.0 {
                continue; // weight 0 — not actually used
            }
            if !registry.has(name) {
                missing.insert(name.clone());
            }
        }
    };
    for xform in &flame.transforms { scan(xform); }
    for xform in &flame.linked_transforms { scan(xform); }
    for xform in &flame.final_transforms { scan(xform); }
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
