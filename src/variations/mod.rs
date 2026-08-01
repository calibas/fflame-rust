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
    /// Every variant, for contract generation.
    pub const ALL: &'static [VariationPhase] = &[
        VariationPhase::Pre,
        VariationPhase::Normal,
        VariationPhase::Post,
        VariationPhase::Any,
    ];

    /// The canonical wire spelling.
    ///
    /// **`any` has no counterpart in `ApiVariationPhase` today**, which
    /// is a real gap rather than a naming detail: 545 of the 646
    /// shipped variations are `Any`, and the phase is what decides
    /// whether a variation honours JWildfire's per-instance
    /// `fx_priority` override (`ShaderBuilder` ignores the override for
    /// anything that is not `Any`). A downloaded variation serialized
    /// as `normal` therefore silently loses phase-override support.
    /// See §4.2 of `docs/projects/VARIATIONS_WIRE_FORMAT.md`.
    pub fn to_api_str(self) -> &'static str {
        match self {
            VariationPhase::Pre => "pre",
            VariationPhase::Normal => "normal",
            VariationPhase::Post => "post",
            VariationPhase::Any => "any",
        }
    }

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

    /// Where this came from: shipped, downloaded, or a local plugin.
    ///
    /// Replaced `is_core: bool` **and** a separate `version: u32`. The
    /// bool could not distinguish a download from a local plugin, and
    /// the two answer differently to "is this clearable cache" and "can
    /// this be updated". Keeping the version beside it duplicated what
    /// `Provenance::Api` already carries, and a duplicate is a thing to
    /// keep in step.
    pub provenance: crate::provenance::Provenance,

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
    pub fn from_download(
        dl: &crate::api::types::VariationDownload,
        provenance: crate::provenance::Provenance,
    ) -> Self {
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

        // The `features` array is authoritative when present; the three
        // legacy bools are the fallback for payloads predating it. That
        // ordering is what lets a newer server serve an older client and
        // vice versa without a flag day.
        //
        // An unrecognised feature name is IGNORED WITH A WARNING rather
        // than rejected: refusing the whole variation over one unknown
        // flag would make every future capability a breaking change.
        use crate::variations::definition::Feature;
        let mut features: Vec<Feature> = Vec::new();
        if dl.features.is_empty() {
            if dl.needs_rng { features.push(Feature::NeedsRng); }
            if dl.needs_transform { features.push(Feature::NeedsTransform); }
            if dl.writes_color { features.push(Feature::WritesColor); }
        } else {
            for name in &dl.features {
                match Feature::from_api_str(name) {
                    Some(f) => features.push(f),
                    None => log::warn!(
                        "Variation '{}': ignoring unknown feature `{name}` —                          this client does not know it yet",
                        dl.name
                    ),
                }
            }
        }
        // Payload-carrying, so it rides in its own field rather than the
        // array. 0 means "does not emit".
        if dl.plot_emits > 0 {
            features.push(Feature::PlotEmits(dl.plot_emits));
        }

        Self {
            name: dl.name.clone(),
            display_name: dl.display_name.clone(),
            category: VariationCategory::from_api_str(&dl.category),
            phase: api_phase_to_runtime(&dl.phase),
            wgsl_function,
            features,
            provenance,
            wgsl_source: dl.shader_2d.clone(),
            wgsl_source_3d: dl.shader_3d.clone(),
            wgsl_source_init: dl.shader_init.clone(),
            init_param_count: dl.init_param_count,
            // Now carried on the wire. Previously hardcoded to 0/None,
            // which silently mis-rendered any stateful server-hosted
            // variation: the slots were allocated and read as zeros.
            state_count: dl.state_count,
            wgsl_source_state_init: dl.shader_state_init.clone(),
            parameters,
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
            provenance: crate::provenance::Provenance::Builtin,
            wgsl_source: Some(def.wgsl_2d.to_string()),
            wgsl_source_3d: Some(def.wgsl_3d.to_string()),
            wgsl_source_init: def.wgsl_init.map(|s| s.to_string()),
            init_param_count: def.init_param_count,
            state_count: def.state_count,
            wgsl_source_state_init: def.wgsl_state_init.map(|s| s.to_string()),
            parameters: def.parameters_to_runtime(),
        }
    }

    /// True if this variation lists the given feature. Mirrors
    /// [`VariationDef::has_feature`] for the runtime side.
    pub fn has_feature(&self, f: crate::variations::definition::Feature) -> bool {
        self.features.contains(&f)
    }

    /// Max extra plot points this variation may emit per call
    /// (`Feature::PlotEmits`). 0 = plots normally only.
    pub fn plot_emit_cap(&self) -> u32 {
        self.features
            .iter()
            .find_map(|f| match f {
                crate::variations::definition::Feature::PlotEmits(n) => Some(*n as u32),
                _ => None,
            })
            .unwrap_or(0)
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

    /// Full 3D variations (Hemisphere, etc.) — 3D in character, but
    /// their `wgsl_2d` bodies are real 2D implementations, so they are
    /// compiled into 2D shaders like any other variation.
    Full3D,

    /// Variations with NO meaningful 2D reading at all — the ONLY
    /// category dropped from 2D shaders (see
    /// `ShaderBuilder::active_with_local_indices`).
    ///
    /// Currently empty, and that is the expected steady state: an
    /// audit of all 639 variations found none that needs excluding.
    /// A z-only variation returning `vec2(0.0)` and a pre/post 3D
    /// rotation returning `p` are both CORRECT 2D contributions, not
    /// broken ones. Reach for this only when a variation genuinely
    /// cannot be written in 2D — and prefer writing an honest 2D body
    /// instead, since dropping a variation silently changes the
    /// weighted sum of every transform that uses it.
    Only3D,

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
pub fn api_param_type_to_runtime_pub(api: &crate::api::types::ApiParamType) -> ParamType {
    api_param_type_to_runtime(api)
}

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
    ///
    /// Every variant must have an arm. `Only3D` had none, which made it
    /// unexpressable over the wire — and it is the one category that is
    /// FUNCTIONAL rather than cosmetic: it is dropped from 2D shaders
    /// (see `ShaderBuilder::active_with_local_indices`). A downloaded
    /// Only3D variation therefore arrived as `Plugin` and was compiled
    /// into 2D shaders, where by definition it has no meaningful
    /// reading. Latent so far only because no shipped variation uses
    /// the category.
    ///
    /// The bare `"3d"` arm is a legacy server spelling that collapses
    /// three categories into one; it resolves to `Depth3D`, which is
    /// wrong for the 114 `Full3D` variations. Kept so old payloads
    /// still parse, but the bulk import should send the real value —
    /// see §5 of `docs/projects/VARIATIONS_WIRE_FORMAT.md`.
    pub fn from_api_str(s: &str) -> Self {
        match s {
            "basic_2d" | "basic2d" => Self::Basic2D,
            "advanced_2d" | "advanced2d" => Self::Advanced2D,
            "depth_3d" | "depth3d" | "3d" => Self::Depth3D,
            "rotation_3d" | "rotation3d" => Self::Rotation3D,
            "full_3d" | "full3d" => Self::Full3D,
            "only_3d" | "only3d" => Self::Only3D,
            _ => Self::Plugin,
        }
    }

    /// The canonical wire spelling — the inverse of [`from_api_str`].
    ///
    /// Having both directions in one place is what lets a round-trip
    /// test assert every variant survives, which is how the missing
    /// `Only3D` arm would have been caught.
    pub fn to_api_str(self) -> &'static str {
        match self {
            Self::Basic2D => "basic_2d",
            Self::Advanced2D => "advanced_2d",
            Self::Depth3D => "depth_3d",
            Self::Rotation3D => "rotation_3d",
            Self::Full3D => "full_3d",
            Self::Only3D => "only_3d",
            Self::Plugin => "plugin",
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
    /// Register a variation from a download payload.
    ///
    /// Source-tagged rather than duplicated: a local plugin is **the
    /// same object from a different source**, so it takes this path with
    /// `Provenance::Local` instead of a parallel `register_from_local`
    /// that would have to be kept in step with every refusal added here.
    pub fn register_from_api(
        &mut self,
        dl: &crate::api::types::VariationDownload,
        provenance: crate::provenance::Provenance,
    ) {
        if let Some(existing) = self.variations.get(&dl.name) {
            if existing.provenance.is_builtin() {
                log::warn!(
                    "Cannot register API variation '{}' — name conflicts with built-in",
                    dl.name
                );
                return;
            }
            // Nor may a download displace the user's own plugin. §0
            // decision 3: collisions are reported, never shadowed — and
            // here the direction matters, because silently replacing
            // something the user wrote with something they did not is
            // the worse of the two failures.
            if matches!(existing.provenance, crate::provenance::Provenance::Local) {
                log::warn!(
                    "Cannot register API variation '{}' — you have a local plugin by that name",
                    dl.name
                );
                return;
            }
        }

        // A missing 2D body is legal for exactly one category.
        //
        // `only_3d` is filtered out of the active set in 2D builds
        // before any source lookup, so it never needs one. For every
        // other category a `None` would reach the emit loop's
        // `else { continue }` and become a silent no-op — the variation
        // would download, register, and contribute nothing. Refusing
        // here keeps that loud, which is the property that made
        // `shader_2d` required in the first place.
        let category = VariationCategory::from_api_str(&dl.category);
        if dl.shader_2d.is_none() && category != VariationCategory::Only3D {
            log::error!(
                "Refusing API variation '{}': no shader_2d, and its category \
                 `{}` is not `only_3d`. Without a 2D body it would render \
                 nothing in 2D flames with no error.",
                dl.name,
                dl.category
            );
            return;
        }
        // ...and an only_3d variation with no 3D body has nothing at all.
        if category == VariationCategory::Only3D && dl.shader_3d.is_none() {
            log::error!(
                "Refusing API variation '{}': category `only_3d` with no \
                 shader_3d — it would render nothing in any mode.",
                dl.name
            );
            return;
        }
        let info = VariationInfo::from_download(dl, provenance);
        if !self.ordered_names.contains(&info.name) {
            self.ordered_names.push(info.name.clone());
        }
        // Index foreign-app aliases, same rules as built-ins: first
        // registration wins, and an alias may not shadow a real
        // variation name. Without this a downloaded variation resolved
        // only by its canonical name, so a `.flame` importing it under a
        // foreign spelling silently found nothing.
        for alias in &dl.aliases {
            if let Some(existing) = self.aliases.get(alias) {
                log::warn!(
                    "Alias '{}' already maps to '{}'; ignoring duplicate from '{}'",
                    alias, existing, info.name
                );
                continue;
            }
            if self.variations.contains_key(alias) {
                log::warn!(
                    "Alias '{}' for '{}' conflicts with an existing variation name; ignoring",
                    alias, info.name
                );
                continue;
            }
            self.aliases.insert(alias.clone(), info.name.clone());
        }
        log::info!("Registered variation '{}' ({})", info.name, info.provenance.label());
        self.variations.insert(info.name.clone(), info);
        self.version = self.version.wrapping_add(1);
    }

    /// Remove one variation by name, whatever its provenance.
    ///
    /// Not used by Clear Cache — that is [`Self::clear_api`], which is
    /// provenance-aware. This is for uninstalling a plugin and for
    /// tests that must not leak into the global registry.
    pub fn remove_by_name(&mut self, name: &str) -> bool {
        let removed = self.variations.remove(name).is_some();
        if removed {
            self.aliases.retain(|_, target| target != name);
            self.version = self.version.wrapping_add(1);
        }
        removed
    }

    /// Remove every downloaded variation.
    ///
    /// Built-ins are compiled in, and **local plugins are the user's own
    /// files** — clearing a cache is not an invitation to delete either.
    /// The old filter was `!is_core`, which would have taken local
    /// plugins with it the moment they existed.
    pub fn clear_api(&mut self) {
        let removed: Vec<String> = self.variations.iter()
            .filter(|(_, info)| info.provenance.is_cached_download())
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
    /// Foreign-app aliases pointing at `canonical`, sorted.
    ///
    /// The alias index is stored alias → canonical because that is the
    /// direction lookups go; the corpus export needs the inverse, and
    /// sorting keeps the dump byte-stable across HashMap iteration
    /// order.
    pub fn aliases_for(&self, canonical: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .aliases
            .iter()
            .filter(|(_, v)| v.as_str() == canonical)
            .map(|(k, _)| k.clone())
            .collect();
        out.sort();
        out
    }

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

/// Resources a flame uses that nobody else can resolve.
///
/// A flame is shared as **names**, not definitions — so one that leans
/// on a local plugin renders correctly for its author and for nobody
/// else, including the same person on another device. There is no error
/// at the far end either: the name is simply unknown, and the fetch that
/// would normally rescue it has nothing to fetch.
///
/// Returned rather than warned about here, so the caller decides
/// whether this is a save (mention it) or an upload (mention it
/// louder).
pub fn local_plugin_dependencies(config: &crate::config::FractalConfig) -> Vec<String> {
    let registry = global_registry();
    let mut out: Vec<String> = config
        .flame
        .active_variation_names_ordered(&registry)
        .into_iter()
        .filter(|name| {
            registry
                .get(name)
                .is_some_and(|v| matches!(v.provenance, crate::provenance::Provenance::Local))
        })
        .collect();
    drop(registry);

    let effects = crate::effects::global_effect_registry();
    for e in config.color_effects.iter().chain(config.density_effects.iter()) {
        if effects
            .get(&e.effect_type)
            .is_some_and(|i| matches!(i.provenance, crate::provenance::Provenance::Local))
            && !out.contains(&e.effect_type)
        {
            out.push(e.effect_type.clone());
        }
    }
    out
}

/// Why a name a flame references cannot be resolved.
///
/// The distinction §8.4 asks for. "We can fetch this" and "you are
/// missing a plugin" look identical from the config — both are just a
/// name the registry does not know — but they need opposite responses,
/// and telling a user to wait for a download that will never come is
/// worse than telling them nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingReason {
    /// The catalog lists it and says it can be fetched.
    Downloadable,
    /// The catalog lists it but it is not fetchable — engine-integral,
    /// or not yet seeded server-side.
    KnownButNotFetchable,
    /// Nothing knows this name. Almost always a local plugin the sender
    /// has and the receiver does not.
    ProbablyAPlugin,
    /// No catalog has been fetched, so the question cannot be answered.
    /// Distinct from `ProbablyAPlugin` — being offline is not evidence.
    Unknown,
}

/// Classify a missing name against the cached catalog.
pub fn classify_missing(
    name: &str,
    catalog: Option<&crate::storage::variation_catalog::CachedCatalog>,
) -> MissingReason {
    let Some(catalog) = catalog else {
        return MissingReason::Unknown;
    };
    match catalog.items.iter().find(|i| i.name == name) {
        Some(item) if item.downloadable => MissingReason::Downloadable,
        Some(_) => MissingReason::KnownButNotFetchable,
        None if catalog.items.is_empty() => MissingReason::Unknown,
        None => MissingReason::ProbablyAPlugin,
    }
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

#[cfg(test)]
mod category_wire_tests {
    use super::VariationCategory as C;

    /// Every variant must survive a round trip.
    ///
    /// `Only3D` had no `from_api_str` arm, so it silently became
    /// `Plugin` — and `Only3D` is the one category the shader builder
    /// ACTS on (it is dropped from 2D shaders). A downloaded variation
    /// in that category was therefore compiled into 2D shaders where it
    /// has no meaningful reading. This test is the cheap thing that
    /// would have caught it.
    #[test]
    fn every_category_survives_the_wire() {
        for c in [
            C::Basic2D,
            C::Advanced2D,
            C::Depth3D,
            C::Rotation3D,
            C::Full3D,
            C::Only3D,
            C::Plugin,
        ] {
            assert_eq!(
                C::from_api_str(c.to_api_str()),
                c,
                "`{}` does not round-trip",
                c.to_api_str()
            );
        }
    }

    /// Unknown strings degrade to Plugin rather than failing — a newer
    /// server may serve a category this client has not learned yet.
    #[test]
    fn unknown_categories_fall_back_rather_than_fail() {
        assert_eq!(C::from_api_str("something_new"), C::Plugin);
        // Spellings the server has actually served. `parametric` and
        // `basic` are pre-import vocabulary; §5 wrongly claimed
        // `parameterized` was recognised — none of them are.
        for legacy in ["basic", "parametric", "parameterized", "pre", "post", "blur"] {
            assert_eq!(C::from_api_str(legacy), C::Plugin, "`{legacy}`");
        }
    }

    /// The legacy `3d` spelling collapses three categories into one.
    /// Documented, not endorsed: it resolves to Depth3D, which is wrong
    /// for the 114 Full3D variations.
    #[test]
    fn the_legacy_3d_spelling_is_lossy() {
        assert_eq!(C::from_api_str("3d"), C::Depth3D);
        assert_ne!(C::from_api_str("3d"), C::Full3D);
    }
}
