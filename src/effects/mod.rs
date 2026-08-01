//! Effects System
//!
//! A flexible shader effects pipeline with two effect chains:
//! - Density Effects: Run before tonemap, have access to density (alpha channel)
//! - Color Effects: Run after tonemap, operate on final RGB colors
//!
//! Effects are registered by string name and stored in FractalConfig as dynamic lists.
//! Empty lists = zero cost (no render passes, no texture allocations).

use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use rust_i18n::t;

use crate::variations::ParamType;

/// Effect category determines when in the pipeline the effect runs
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum EffectCategory {
    /// Runs before tonemap, has access to density in alpha channel
    Density,
    /// Runs after tonemap, operates on final RGB colors
    Color,
}

/// Definition of a single effect parameter
#[derive(Clone, Debug, Serialize)]
pub struct EffectParameter {
    /// Parameter name (canonical, lowercase, e.g. "intensity", "radius")
    pub name: String,

    /// Display name for UI (e.g. "Intensity", "Hue Offset"). Single
    /// English locale by policy — these are technical labels, not
    /// subject to i18n. Replaces the previous
    /// `EffectInfo::translated_param_name()` lookup pattern.
    pub display_name: String,

    /// Parameter type (reuse from variations)
    pub param_type: ParamType,

    /// Default value
    pub default_value: f32,

    /// Minimum value (None = no limit)
    pub min_value: Option<f32>,

    /// Maximum value (None = no limit)
    pub max_value: Option<f32>,

    /// Free-form help / tooltip prose shown under the parameter
    /// control. `None` renders the control without a tooltip. Single
    /// English locale by policy.
    pub description: Option<String>,
}

/// Every shipped effect shader, compiled in.
///
/// Always present, on every target. Desktop prefers the on-disk copy
/// when there is one (see [`EffectSource`]) so a shader can be edited
/// without a rebuild; this is what makes a binary run from anywhere.
pub mod embedded_shaders {
    // Common includes
    pub const BLEND_MODES: &str = include_str!("../../shaders/effects/common/blend_modes.wgsl");

    // Color effects
    pub const CHROMATIC_ABERRATION: &str = include_str!("../../shaders/effects/color/chromatic_aberration.wgsl");
    pub const DOMAIN_WARP: &str = include_str!("../../shaders/effects/color/domain_warp.wgsl");
    pub const FILM_GRAIN: &str = include_str!("../../shaders/effects/color/film_grain.wgsl");
    pub const HUE_CYCLE: &str = include_str!("../../shaders/effects/color/hue_cycle.wgsl");
    pub const KALEIDOSCOPE: &str = include_str!("../../shaders/effects/color/kaleidoscope.wgsl");
    pub const PLASMA: &str = include_str!("../../shaders/effects/color/plasma.wgsl");
    pub const SIMPLEX_NOISE: &str = include_str!("../../shaders/effects/color/simplex_noise.wgsl");
    pub const SOBEL_EDGES: &str = include_str!("../../shaders/effects/color/sobel_edges.wgsl");
    pub const TUNNEL: &str = include_str!("../../shaders/effects/color/tunnel.wgsl");
    pub const VIGNETTE: &str = include_str!("../../shaders/effects/color/vignette.wgsl");
    pub const WORLEY_NOISE: &str = include_str!("../../shaders/effects/color/worley_noise.wgsl");
    pub const JULIA: &str = include_str!("../../shaders/effects/color/julia.wgsl");

    // Density effects
    pub const BILATERAL_BLUR: &str = include_str!("../../shaders/effects/density/bilateral_blur.wgsl");
    pub const DENSITY_BLUR: &str = include_str!("../../shaders/effects/density/density_blur.wgsl");
    pub const SHARPEN: &str = include_str!("../../shaders/effects/density/sharpen.wgsl");
}

/// Where an effect's WGSL comes from.
///
/// # The arrangement, and the bug it fixes
///
/// Built-in effects are **embedded** with `include_str!` and, on
/// desktop, superseded by the on-disk copy under `shaders/` when one is
/// there. That is the same arrangement shipped scripts have: edit the
/// file, restart, see the change, with no recompile — while a binary
/// run from a directory that has no `shaders/` still works.
///
/// It did not work before. `embedded_shaders` was already compiled into
/// every build, but the desktop path read the filesystem and **errored
/// if the file was missing** rather than falling back to the copy it was
/// already carrying. Every effect then failed to compile with a log line
/// and rendered nothing.
///
/// The web path was worse than missing: a hardcoded `match` over the
/// fifteen shipped paths with `_ => Err("Unknown effect shader")`, so a
/// downloaded effect was not unimplemented but *inexpressible*.
#[derive(Clone, Debug)]
pub enum EffectSource {
    /// Ships with the app. `path` is only a desktop override hint.
    Builtin { embedded: &'static str, path: &'static str },
    /// Downloaded or locally installed: the WGSL travels with it.
    ///
    /// There is no path to fall back to, which is correct — a resource
    /// the app did not ship has no place under `shaders/`.
    Owned(String),
}

impl EffectSource {
    /// The WGSL, before include processing.
    ///
    /// Never fails: a built-in always has its embedded copy, and an
    /// owned one holds its source outright. That is a deliberate change
    /// from the previous `Result` — every failure mode it modelled was
    /// a missing file the binary already contained.
    pub fn wgsl(&self) -> String {
        match self {
            Self::Builtin { embedded, path } => {
                // Desktop only: prefer the working copy so a shader can
                // be edited without a rebuild. Absent or unreadable
                // falls through to the embedded copy rather than
                // failing, which is the fix.
                #[cfg(not(target_arch = "wasm32"))]
                if let Ok(from_disk) = std::fs::read_to_string(format!("shaders/{path}")) {
                    return from_disk;
                }
                let _ = path;
                (*embedded).to_string()
            }
            Self::Owned(src) => src.clone(),
        }
    }

    /// The shader path, for the corpus exporter and for diagnostics.
    /// `None` for anything that did not ship.
    pub fn path(&self) -> Option<&'static str> {
        match self {
            Self::Builtin { path, .. } => Some(path),
            Self::Owned(_) => None,
        }
    }
}

/// Metadata for a registered effect
#[derive(Clone, Debug)]
pub struct EffectInfo {
    /// Unique name (e.g., "vignette", "density_blur")
    pub name: String,

    /// Category determines pipeline position
    pub category: EffectCategory,

    /// Where the WGSL comes from.
    pub source: EffectSource,

    /// Parameters for this effect
    pub parameters: Vec<EffectParameter>,

    /// Where this effect came from, and therefore whether it is
    /// third-party code, cache, or updatable. See
    /// [`crate::provenance::Provenance`] for why those are three
    /// questions rather than one bool.
    pub provenance: crate::provenance::Provenance,
}

impl EffectInfo {
    /// Get the translated display name for this effect
    pub fn translated_name(&self) -> String {
        let key = format!("effects.{}.name", self.name);
        t!(&key).to_string()
    }

    /// Get the default value for a parameter by name
    pub fn get_param_default(&self, param_name: &str) -> Option<f32> {
        self.parameters
            .iter()
            .find(|p| p.name == param_name)
            .map(|p| p.default_value)
    }

    /// Get parameter definition by name
    pub fn get_param(&self, param_name: &str) -> Option<&EffectParameter> {
        self.parameters.iter().find(|p| p.name == param_name)
    }

    /// Get all default parameter values as a HashMap
    pub fn default_params(&self) -> HashMap<String, f32> {
        self.parameters
            .iter()
            .map(|p| (p.name.clone(), p.default_value))
            .collect()
    }
}

/// A single effect instance in a config
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EffectInstance {
    /// Session-local identity used by the animation system to bind
    /// tracks stably across effect add / delete / reorder. Never
    /// serialized — see `crate::scene::transforms::next_id` docs.
    #[serde(skip)]
    pub id: u64,

    /// Effect type name (must match registered effect)
    pub effect_type: String,

    /// Whether this effect is currently active
    pub enabled: bool,

    /// Parameter values (name → value)
    /// Missing params use defaults from registry
    #[serde(default)]
    pub params: HashMap<String, f32>,
}

// Manual PartialEq that ignores the runtime-only `id` field. Two
// effects with the same data are equal regardless of session identity.
impl PartialEq for EffectInstance {
    fn eq(&self, other: &Self) -> bool {
        self.effect_type == other.effect_type
            && self.enabled == other.enabled
            && self.params == other.params
    }
}

impl EffectInstance {
    /// Create a new effect instance with default parameters and a fresh
    /// session-local ID. Editor code paths should use this. Code that
    /// expects `fixup_ids` to assign an ID later (deserialize) can
    /// construct with `id: 0`.
    pub fn new(effect_type: &str) -> Self {
        let registry = global_effect_registry();
        let params = if let Some(info) = registry.get(effect_type) {
            info.default_params()
        } else {
            HashMap::new()
        };
        drop(registry);

        Self {
            id: crate::scene::transforms::next_id(),
            effect_type: effect_type.to_string(),
            enabled: true,
            params,
        }
    }

    /// Create a new disabled effect instance
    pub fn new_disabled(effect_type: &str) -> Self {
        let mut instance = Self::new(effect_type);
        instance.enabled = false;
        instance
    }

    /// Get a parameter value, falling back to registry default
    pub fn get_param(&self, param_name: &str) -> f32 {
        if let Some(&value) = self.params.get(param_name) {
            value
        } else {
            let registry = global_effect_registry();
            match registry.get(&self.effect_type) {
                Some(info) => info.get_param_default(param_name).unwrap_or(0.0),
                None => 0.0,
            }
        }
    }

    /// Set a parameter value
    pub fn set_param(&mut self, param_name: &str, value: f32) {
        self.params.insert(param_name.to_string(), value);
    }
}

/// Registry of all available effects
pub struct EffectRegistry {
    /// Effects by name
    effects: HashMap<String, EffectInfo>,

    /// Ordered list of effect names (registration order)
    ordered_names: Vec<String>,
}

impl EffectRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            effects: HashMap::new(),
            ordered_names: Vec::new(),
        }
    }

    /// Register an effect
    pub fn register(&mut self, info: EffectInfo) {
        let name = info.name.clone();
        if !self.effects.contains_key(&name) {
            self.ordered_names.push(name.clone());
        }
        self.effects.insert(name, info);
    }

    /// Get effect info by name
    pub fn get(&self, name: &str) -> Option<&EffectInfo> {
        self.effects.get(name)
    }

    /// Get all effects in registration order
    pub fn all(&self) -> impl Iterator<Item = &EffectInfo> {
        self.ordered_names
            .iter()
            .filter_map(|name| self.effects.get(name))
    }

    /// Get all effects in a category
    pub fn by_category(&self, category: EffectCategory) -> impl Iterator<Item = &EffectInfo> {
        self.all().filter(move |info| info.category == category)
    }

    /// Get effect names in a category
    pub fn names_by_category(&self, category: EffectCategory) -> Vec<&str> {
        self.by_category(category)
            .map(|info| info.name.as_str())
            .collect()
    }

    /// Check if an effect exists
    pub fn contains(&self, name: &str) -> bool {
        self.effects.contains_key(name)
    }

    /// Get the number of registered effects
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}

impl Default for EffectRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global effect registry singleton
static EFFECT_REGISTRY: Lazy<RwLock<EffectRegistry>> = Lazy::new(|| {
    let mut registry = EffectRegistry::new();
    register_builtin_effects(&mut registry);
    RwLock::new(registry)
});

/// Read guard on the global effect registry.
///
/// Behind a lock because effects are no longer a closed set: a
/// downloaded or locally installed effect is registered at runtime, the
/// way `register_from_api` already does for variations. Registration
/// used to be compile-time only, which is why this returned a plain
/// reference.
///
/// **Bind the guard to a variable** before using what it lends out.
/// `global_effect_registry().get(x)` drops the guard at the end of the
/// statement, so anything borrowed from it dies with it.
pub fn global_effect_registry() -> RwLockReadGuard<'static, EffectRegistry> {
    EFFECT_REGISTRY.read().expect("effect registry RwLock poisoned")
}

/// Write guard on the global effect registry. Use sparingly — only for
/// adding or removing effects that did not ship with the app.
pub fn global_effect_registry_mut() -> RwLockWriteGuard<'static, EffectRegistry> {
    EFFECT_REGISTRY.write().expect("effect registry RwLock poisoned")
}

/// Effect names a flame asked for that this build does not have.
///
/// Recorded at compile time (in the shader sense) rather than scanned
/// out of the config, because that is where the answer is already known
/// — `EffectChain` looks each one up and finds nothing. The variation
/// equivalent, `missing_variations_in`, scans the flame instead; it can,
/// because a variation's absence is visible from the config alone,
/// whereas an effect's depends on what the registry holds right now.
///
/// Drained by the app, which turns it into a fetch.
static MISSING_EFFECTS: Lazy<std::sync::Mutex<std::collections::BTreeSet<String>>> =
    Lazy::new(Default::default);

/// Note that a flame referenced an effect that is not registered.
pub fn note_missing_effect(name: &str) {
    if let Ok(mut set) = MISSING_EFFECTS.lock() {
        set.insert(name.to_string());
    }
}

/// Take the recorded names, leaving the set empty.
///
/// Draining rather than reading keeps a failed fetch from re-triggering
/// every frame: the render records the name again next time it tries to
/// compile, so a genuinely still-missing effect comes back, but a
/// failure the user has already been told about does not spin.
pub fn take_missing_effects() -> Vec<String> {
    match MISSING_EFFECTS.lock() {
        Ok(mut set) => std::mem::take(&mut *set).into_iter().collect(),
        Err(_) => Vec::new(),
    }
}

// ============================================================================
// Registering an effect that did not ship with the app
// ============================================================================

/// Every function the shared blend-mode library defines.
///
/// Parsed from the library itself rather than listed here. A
/// transcribed list is the same trap as the name-gated helper table and
/// the reserved script stems: it goes stale silently, and the failure
/// shows up as a downloaded effect that will not compile for a reason
/// nobody can see. Add a function to `blend_modes.wgsl` and this
/// follows.
fn blend_library_symbols() -> &'static [String] {
    static SYMBOLS: Lazy<Vec<String>> = Lazy::new(|| {
        embedded_shaders::BLEND_MODES
            .lines()
            .filter_map(|l| l.trim().strip_prefix("fn "))
            .filter_map(|rest| rest.split('(').next())
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect()
    });
    &SYMBOLS
}

/// A library function this shader calls but does not define.
///
/// Returns the first one found, for the error message. A shader that
/// *defines* `fn luminance(...)` itself is not calling ours, so it is
/// not reported.
fn missing_blend_library_symbol(shader: &str) -> Option<&'static str> {
    blend_library_symbols().iter().find_map(|sym| {
        let call = format!("{sym}(");
        let definition = format!("fn {sym}(");
        if shader.contains(&call) && !shader.contains(&definition) {
            // The `&'static` comes from the leaked Lazy, which lives for
            // the process.
            Some(sym.as_str())
        } else {
            None
        }
    })
}

/// Why an effect from the API cannot be registered.
///
/// Separated from the registry so the rules are testable without a
/// registry singleton, and so each one can say what it actually
/// objects to.
pub fn check_download(dl: &crate::api::types::EffectDownload) -> Result<EffectInfo, String> {
    let name = dl.name.clone();

    let category = match dl.category.as_deref() {
        Some("density") => EffectCategory::Density,
        Some("color") => EffectCategory::Color,
        other => {
            return Err(format!(
                "effect `{name}`: category must be `density` or `color`, got {other:?}. \
                 The category IS the pipeline position, so there is no safe default."
            ))
        }
    };

    // Null until the server's shaders are seeded. Registering one would
    // produce an effect that appears in the panel, accepts parameters,
    // and renders nothing.
    let shader = dl.shader.clone().filter(|s| !s.trim().is_empty()).ok_or_else(|| {
        format!(
            "effect `{name}` carries no shader (downloadable={}), so there is nothing \
             to compile",
            dl.downloadable
        )
    })?;

    // Physical uniform capacity, not policy: the params live in a fixed
    // `[[f32; 4]; 12]`. Over it, the tail would be silently dropped.
    if dl.parameters.len() > crate::renderer::effect_chain::MAX_EFFECT_PARAMS {
        return Err(format!(
            "effect `{name}` declares {} parameters; the uniform holds {}",
            dl.parameters.len(),
            crate::renderer::effect_chain::MAX_EFFECT_PARAMS
        ));
    }

    // The splice happens on the marker, not on the flag. A shader that
    // calls into the shared library without the marker compiles against
    // nothing and fails naming a function its author never wrote — the
    // exact confusing failure `load_blend_modes` used to produce for
    // built-ins.
    const MARKER: &str = "// INCLUDE_BLEND_MODES";
    if !shader.contains(MARKER) {
        if let Some(sym) = missing_blend_library_symbol(&shader) {
            return Err(format!(
                "effect `{name}` calls `{sym}` from the shared blend-mode library but \
                 does not include it — add `{MARKER}` to the shader"
            ));
        }
    }

    let parameters = dl
        .parameters
        .iter()
        .map(|p| EffectParameter {
            name: p.name.clone(),
            display_name: p.display_name.clone(),
            param_type: crate::variations::api_param_type_to_runtime_pub(&p.param_type),
            default_value: p.default_value,
            min_value: p.min_value,
            max_value: p.max_value,
            description: p.description.clone(),
        })
        .collect();

    Ok(EffectInfo {
        name,
        category,
        source: EffectSource::Owned(shader),
        parameters,
        provenance: crate::provenance::Provenance::Api { version: dl.version },
    })
}

impl EffectRegistry {
    /// Register an effect from a download payload.
    ///
    /// Source-tagged rather than duplicated: a local plugin is the same
    /// object from a different source, so it takes this path with
    /// `Provenance::Local` instead of a parallel entry point that would
    /// have to be kept in step with every refusal added here.
    ///
    /// Refuses rather than degrades, for the reason variations do: an
    /// effect that registers and renders nothing looks like a broken
    /// feature, and the user has no way to find out why.
    pub fn register_from_api(
        &mut self,
        dl: &crate::api::types::EffectDownload,
        provenance: crate::provenance::Provenance,
    ) -> Result<(), String> {
        if let Some(existing) = self.get(&dl.name) {
            // Never shadow, in either direction. Displacing a built-in
            // would change what a shared flame renders; displacing a
            // local plugin would replace the user's own work with
            // somebody else's, which is the worse of the two.
            match existing.provenance {
                crate::provenance::Provenance::Builtin => {
                    return Err(format!(
                        "effect `{}` is built in; it cannot be replaced",
                        dl.name
                    ))
                }
                crate::provenance::Provenance::Local
                    if !matches!(provenance, crate::provenance::Provenance::Local) =>
                {
                    return Err(format!(
                        "effect `{}` is a local plugin of yours; a download cannot replace it",
                        dl.name
                    ))
                }
                _ => {}
            }
        }
        let mut info = check_download(dl)?;
        info.provenance = provenance;
        self.register(info);
        Ok(())
    }
}

/// Register all built-in effects
fn register_builtin_effects(registry: &mut EffectRegistry) {
    // === Color Effects (after tonemap) ===

    // Vignette - darken edges
    registry.register(EffectInfo {
        name: "vignette".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::VIGNETTE,
            path: "effects/color/vignette.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "radius".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Radius".to_string(),
                description: None,
            },
            EffectParameter {
                name: "softness".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Softness".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 0.0, // Normal
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
        ],
    });

    // Film Grain - per-pixel random noise
    registry.register(EffectInfo {
        name: "film_grain".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::FILM_GRAIN,
            path: "effects/color/film_grain.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 0.1,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "seed".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(256.0),
                display_name: "Seed".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 4.0, // Overlay (good for grain)
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
        ],
    });

    // Chromatic Aberration - RGB channel offset
    registry.register(EffectInfo {
        name: "chromatic_aberration".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::CHROMATIC_ABERRATION,
            path: "effects/color/chromatic_aberration.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "amount".to_string(),
                param_type: ParamType::Float,
                default_value: 2.0,
                min_value: Some(0.0),
                max_value: Some(20.0),
                display_name: "Amount".to_string(),
                description: None,
            },
            EffectParameter {
                name: "radial".to_string(),
                param_type: ParamType::Boolean,
                default_value: 1.0, // true
                min_value: None,
                max_value: None,
                display_name: "Radial".to_string(),
                description: None,
            },
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 0.0, // Normal
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
        ],
    });

    // Hue Shift - static hue rotation (animate via keyframes)
    registry.register(EffectInfo {
        name: "hue_shift".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::HUE_CYCLE,
            path: "effects/color/hue_cycle.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "offset".to_string(),
                param_type: ParamType::Angle,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(360.0),
                display_name: "Hue Offset".to_string(),
                description: None,
            },
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 0.0, // Normal
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
        ],
    });

    // === Density Effects (before tonemap) ===

    // Density Blur - blur weighted by density
    registry.register(EffectInfo {
        name: "density_blur".to_string(),
        category: EffectCategory::Density,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::DENSITY_BLUR,
            path: "effects/density/density_blur.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "radius".to_string(),
                param_type: ParamType::Float,
                default_value: 3.0,
                min_value: Some(0.0),
                max_value: Some(10.0),
                display_name: "Radius".to_string(),
                description: None,
            },
            EffectParameter {
                name: "threshold".to_string(),
                param_type: ParamType::Float,
                default_value: 0.25,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Density Threshold".to_string(),
                description: None,
            },
            EffectParameter {
                name: "falloff".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Falloff".to_string(),
                description: None,
            },
        ],
    });

    // Sharpen - detail enhancement
    registry.register(EffectInfo {
        name: "sharpen".to_string(),
        category: EffectCategory::Density,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::SHARPEN,
            path: "effects/density/sharpen.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "amount".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(2.0),
                display_name: "Amount".to_string(),
                description: None,
            },
            EffectParameter {
                name: "radius".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.5),
                max_value: Some(5.0),
                display_name: "Radius".to_string(),
                description: None,
            },
        ],
    });

    // Bilateral Blur - edge-preserving blur
    registry.register(EffectInfo {
        name: "bilateral_blur".to_string(),
        category: EffectCategory::Density,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::BILATERAL_BLUR,
            path: "effects/density/bilateral_blur.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "radius".to_string(),
                param_type: ParamType::Float,
                default_value: 3.0,
                min_value: Some(1.0),
                max_value: Some(10.0),
                display_name: "Radius".to_string(),
                description: None,
            },
            EffectParameter {
                name: "sigma_spatial".to_string(),
                param_type: ParamType::Float,
                default_value: 3.0,
                min_value: Some(1.0),
                max_value: Some(10.0),
                display_name: "Spatial Sigma".to_string(),
                description: None,
            },
            EffectParameter {
                name: "sigma_range".to_string(),
                param_type: ParamType::Float,
                default_value: 0.1,
                min_value: Some(0.05),
                max_value: Some(0.5),
                display_name: "Range Sigma".to_string(),
                description: None,
            },
        ],
    });

    // === Psychedelic Color Effects ===

    // Kaleidoscope - N-fold rotational symmetry
    registry.register(EffectInfo {
        name: "kaleidoscope".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::KALEIDOSCOPE,
            path: "effects/color/kaleidoscope.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "segments".to_string(),
                param_type: ParamType::Float,
                default_value: 6.0,
                min_value: Some(2.0),
                max_value: Some(16.0),
                display_name: "Segments".to_string(),
                description: None,
            },
            EffectParameter {
                name: "rotation".to_string(),
                param_type: ParamType::Angle,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(360.0),
                display_name: "Source Angle".to_string(),
                description: None,
            },
            EffectParameter {
                name: "zoom".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.1),
                max_value: Some(3.0),
                display_name: "Zoom".to_string(),
                description: None,
            },
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 0.0, // Normal
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
            EffectParameter {
                name: "square_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 1.0, // 1 = Square (1:1 perfect symmetry), 0 = Screen ratio (stretched)
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Square Mode".to_string(),
                description: None,
            },
            EffectParameter {
                name: "edge_offset".to_string(),
                param_type: ParamType::Integer,
                default_value: 0.0, // 0 = no offset (existing behavior)
                min_value: Some(0.0),
                max_value: Some(2000.0),
                display_name: "Edge Offset".to_string(),
                description: None,
            },
        ],
    });

    // Plasma - classic demoscene effect
    registry.register(EffectInfo {
        name: "plasma".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::PLASMA,
            path: "effects/color/plasma.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 0.3,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "scale".to_string(),
                param_type: ParamType::Float,
                default_value: 3.0,
                min_value: Some(0.1),
                max_value: Some(100.0),
                display_name: "Scale".to_string(),
                description: None,
            },
            EffectParameter {
                name: "speed".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(10.0),
                display_name: "Speed".to_string(),
                description: None,
            },
            EffectParameter {
                name: "time".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1000.0),
                display_name: "Time".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 1.0, // Add (classic plasma look)
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
            EffectParameter {
                name: "direction".to_string(),
                param_type: ParamType::Angle,
                default_value: 225.0, // Default: up-left (legacy behavior)
                min_value: Some(0.0),
                max_value: Some(360.0),
                display_name: "Direction".to_string(),
                description: None,
            },
        ],
    });

    // Tunnel - infinite tunnel effect
    registry.register(EffectInfo {
        name: "tunnel".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::TUNNEL,
            path: "effects/color/tunnel.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "speed".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(5.0),
                display_name: "Speed".to_string(),
                description: None,
            },
            EffectParameter {
                name: "rotation_speed".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(5.0),
                display_name: "Rotation Speed".to_string(),
                description: None,
            },
            EffectParameter {
                name: "distortion".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Distortion".to_string(),
                description: None,
            },
            EffectParameter {
                name: "time".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1000.0),
                display_name: "Time".to_string(),
                description: None,
            },
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 0.0, // Normal
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
        ],
    });

    // Sobel Edges - neon edge detection
    registry.register(EffectInfo {
        name: "sobel_edges".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::SOBEL_EDGES,
            path: "effects/color/sobel_edges.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(2.0),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "threshold".to_string(),
                param_type: ParamType::Float,
                default_value: 0.1,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Threshold".to_string(),
                description: None,
            },
            EffectParameter {
                name: "glow".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Glow".to_string(),
                description: None,
            },
            EffectParameter {
                name: "preserve_color".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Preserve Color".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 1.0, // Add (good for neon glow)
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
        ],
    });

    // Domain Warp - organic noise distortion
    registry.register(EffectInfo {
        name: "domain_warp".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::DOMAIN_WARP,
            path: "effects/color/domain_warp.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 0.1,
                min_value: Some(0.0),
                max_value: Some(0.5),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "scale".to_string(),
                param_type: ParamType::Float,
                default_value: 3.0,
                min_value: Some(0.5),
                max_value: Some(10.0),
                display_name: "Scale".to_string(),
                description: None,
            },
            EffectParameter {
                name: "octaves".to_string(),
                param_type: ParamType::Float,
                default_value: 4.0,
                min_value: Some(1.0),
                max_value: Some(6.0),
                display_name: "Octaves".to_string(),
                description: None,
            },
            EffectParameter {
                name: "time".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1000.0),
                display_name: "Time".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 0.0, // Normal
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
            EffectParameter {
                name: "direction".to_string(),
                param_type: ParamType::Angle,
                default_value: 225.0, // Default: up-left (legacy behavior)
                min_value: Some(0.0),
                max_value: Some(360.0),
                display_name: "Direction".to_string(),
                description: None,
            },
        ],
    });

    // Simplex Noise - psychedelic noise overlay
    registry.register(EffectInfo {
        name: "simplex_noise".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::SIMPLEX_NOISE,
            path: "effects/color/simplex_noise.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "scale".to_string(),
                param_type: ParamType::Float,
                default_value: 5.0,
                min_value: Some(0.5),
                max_value: Some(20.0),
                display_name: "Scale".to_string(),
                description: None,
            },
            EffectParameter {
                name: "octaves".to_string(),
                param_type: ParamType::Float,
                default_value: 4.0,
                min_value: Some(1.0),
                max_value: Some(6.0),
                display_name: "Octaves".to_string(),
                description: None,
            },
            EffectParameter {
                name: "time".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1000.0),
                display_name: "Time".to_string(),
                description: None,
            },
            EffectParameter {
                name: "mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 0.0, // 0=Color, 1=Distort, 2=Mask
                min_value: Some(0.0),
                max_value: Some(2.0),
                display_name: "Mode".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 4.0, // Overlay (good for noise)
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
            EffectParameter {
                name: "direction".to_string(),
                param_type: ParamType::Angle,
                default_value: 225.0, // Default: up-left (legacy behavior)
                min_value: Some(0.0),
                max_value: Some(360.0),
                display_name: "Direction".to_string(),
                description: None,
            },
        ],
    });

    // Worley Noise - cellular patterns
    registry.register(EffectInfo {
        name: "worley_noise".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::WORLEY_NOISE,
            path: "effects/color/worley_noise.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "scale".to_string(),
                param_type: ParamType::Float,
                default_value: 8.0,
                min_value: Some(1.0),
                max_value: Some(20.0),
                display_name: "Scale".to_string(),
                description: None,
            },
            EffectParameter {
                name: "time".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1000.0),
                display_name: "Time".to_string(),
                description: None,
            },
            EffectParameter {
                name: "mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 0.0, // 0=Cells, 1=Edges, 2=Organic, 3=Crystal
                min_value: Some(0.0),
                max_value: Some(3.0),
                display_name: "Pattern".to_string(),
                description: None,
            },
            EffectParameter {
                name: "color_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 1.0, // 0=Grayscale, 1=Rainbow, 2=Original
                min_value: Some(0.0),
                max_value: Some(2.0),
                display_name: "Color Mode".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 4.0, // Overlay
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
            EffectParameter {
                name: "direction".to_string(),
                param_type: ParamType::Angle,
                default_value: 225.0, // Default: up-left (legacy behavior)
                min_value: Some(0.0),
                max_value: Some(360.0),
                display_name: "Direction".to_string(),
                description: None,
            },
        ],
    });

    // Julia/Mandelbrot - fractal overlay
    registry.register(EffectInfo {
        name: "julia".to_string(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::JULIA,
            path: "effects/color/julia.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        parameters: vec![
            EffectParameter {
                name: "mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 1.0, // 0=Mandelbrot, 1=Julia
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Mode".to_string(),
                description: None,
            },
            EffectParameter {
                name: "julia_cx".to_string(),
                param_type: ParamType::Float,
                default_value: -0.7,
                min_value: Some(-2.0),
                max_value: Some(2.0),
                display_name: "Julia C (Real)".to_string(),
                description: None,
            },
            EffectParameter {
                name: "julia_cy".to_string(),
                param_type: ParamType::Float,
                default_value: 0.27,
                min_value: Some(-2.0),
                max_value: Some(2.0),
                display_name: "Julia C (Imag)".to_string(),
                description: None,
            },
            EffectParameter {
                name: "max_iter".to_string(),
                param_type: ParamType::Integer,
                default_value: 100.0,
                min_value: Some(10.0),
                max_value: Some(500.0),
                display_name: "Max Iterations".to_string(),
                description: None,
            },
            EffectParameter {
                name: "zoom".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.1),
                max_value: Some(100.0),
                display_name: "Zoom".to_string(),
                description: None,
            },
            EffectParameter {
                name: "center_x".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(-3.0),
                max_value: Some(3.0),
                display_name: "Center X".to_string(),
                description: None,
            },
            EffectParameter {
                name: "center_y".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(-3.0),
                max_value: Some(3.0),
                display_name: "Center Y".to_string(),
                description: None,
            },
            EffectParameter {
                name: "color_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 1.0, // 0=Escape, 1=Smooth, 2=Original mask
                min_value: Some(0.0),
                max_value: Some(2.0),
                display_name: "Color Mode".to_string(),
                description: None,
            },
            EffectParameter {
                name: "intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
                display_name: "Intensity".to_string(),
                description: None,
            },
            EffectParameter {
                name: "blend_mode".to_string(),
                param_type: ParamType::Integer,
                default_value: 0.0, // Normal
                min_value: Some(0.0),
                max_value: Some(12.0),
                display_name: "Blend Mode".to_string(),
                description: None,
            },
        ],
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_builtin_effects() {
        let registry = global_effect_registry();
        assert!(registry.contains("vignette"));
        assert!(registry.contains("density_blur"));
        assert!(registry.contains("kaleidoscope"));
        assert!(registry.contains("bilateral_blur"));
        assert!(registry.contains("simplex_noise"));
        assert!(registry.contains("worley_noise"));
        assert!(registry.len() >= 14);
    }

    #[test]
    fn test_effect_categories() {
        let registry = global_effect_registry();

        let color_effects: Vec<_> = registry.by_category(EffectCategory::Color).collect();
        assert!(color_effects.iter().any(|e| e.name == "vignette"));

        let density_effects: Vec<_> = registry.by_category(EffectCategory::Density).collect();
        assert!(density_effects.iter().any(|e| e.name == "density_blur"));
    }

    #[test]
    fn test_effect_instance_creation() {
        let instance = EffectInstance::new("vignette");
        assert_eq!(instance.effect_type, "vignette");
        assert!(instance.enabled);
        assert!((instance.get_param("intensity") - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_effect_instance_params() {
        let mut instance = EffectInstance::new("vignette");
        instance.set_param("intensity", 0.8);
        assert!((instance.get_param("intensity") - 0.8).abs() < 0.001);
    }
}

#[cfg(test)]
mod download_tests {
    use super::*;
    use crate::api::types::{ApiParamType, ApiVariationParameter, EffectDownload};

    fn dl(name: &str, shader: Option<&str>) -> EffectDownload {
        EffectDownload {
            id: name.into(),
            name: name.into(),
            display_name: name.into(),
            category: Some("color".into()),
            authors: Vec::new(),
            description: None,
            description_plain: None,
            version: 1,
            parameters: Vec::new(),
            shader: shader.map(|s| s.into()),
            requires_blend_modes: false,
            downloadable: true,
        }
    }

    fn param(n: &str) -> ApiVariationParameter {
        ApiVariationParameter {
            name: n.into(),
            display_name: n.into(),
            param_type: ApiParamType::Float,
            default_value: 0.0,
            min_value: None,
            max_value: None,
            description: None,
        }
    }

    /// The ordinary case still works, or every refusal below is just a
    /// broken feature.
    #[test]
    fn a_well_formed_effect_registers() {
        let info = check_download(&dl("swirl", Some("fn main() {}"))).expect("registers");
        assert_eq!(info.name, "swirl");
        assert_eq!(info.category, EffectCategory::Color);
        assert_eq!(info.provenance, crate::provenance::Provenance::Api { version: 1 });
        assert!(info.source.path().is_none(), "a download has no shipped path");
    }

    /// `shader` is null until the server seeds them. Registering one
    /// would put an effect in the panel that accepts parameters and
    /// renders nothing.
    #[test]
    fn an_effect_with_no_shader_is_refused() {
        let e = check_download(&dl("empty", None)).expect_err("must refuse");
        assert!(e.contains("no shader"), "{e}");
        // Whitespace is not a shader either.
        assert!(check_download(&dl("blank", Some("   \n"))).is_err());
    }

    /// The category IS the pipeline position, so there is no safe
    /// default to fall back on.
    #[test]
    fn a_bad_category_is_refused_rather_than_defaulted() {
        let mut d = dl("odd", Some("fn main() {}"));
        d.category = Some("sideways".into());
        assert!(check_download(&d).unwrap_err().contains("density"));
        d.category = None;
        assert!(check_download(&d).is_err());
    }

    /// Over the uniform's physical capacity the tail would be silently
    /// dropped — the effect would work, just not all of it.
    #[test]
    fn too_many_parameters_are_refused() {
        let mut d = dl("greedy", Some("fn main() {}"));
        d.parameters = (0..crate::renderer::effect_chain::MAX_EFFECT_PARAMS + 1)
            .map(|i| param(&format!("p{i}")))
            .collect();
        let e = check_download(&d).expect_err("must refuse");
        assert!(e.contains("uniform holds"), "{e}");

        // Exactly at capacity is fine.
        d.parameters.pop();
        assert!(check_download(&d).is_ok());
    }

    /// A shader that calls the shared library without including it
    /// compiles against nothing and fails naming a function its author
    /// never wrote.
    #[test]
    fn calling_the_blend_library_without_including_it_is_refused() {
        let d = dl("needy", Some("fn main() { let c = blend_screen(a, b); }"));
        let e = check_download(&d).expect_err("must refuse");
        assert!(e.contains("blend_screen"), "{e}");
        assert!(e.contains("INCLUDE_BLEND_MODES"), "{e}");

        // With the marker it is fine — the splice happens on the marker.
        let ok = dl(
            "polite",
            Some("// INCLUDE_BLEND_MODES\nfn main() { let c = blend_screen(a, b); }"),
        );
        assert!(check_download(&ok).is_ok());
    }

    /// The guard covers the WHOLE library, not just `blend_*`. A shader
    /// calling `luminance` fails exactly as hard.
    #[test]
    fn the_guard_covers_every_symbol_the_library_defines() {
        let syms = blend_library_symbols();
        assert!(syms.len() > 15, "parsed {} symbols", syms.len());
        assert!(syms.iter().any(|s| s == "luminance"), "{syms:?}");
        assert!(syms.iter().any(|s| s == "rgb_to_hsl"), "{syms:?}");

        let d = dl("lumen", Some("fn main() { let l = luminance(c); }"));
        assert!(check_download(&d).unwrap_err().contains("luminance"));
    }

    /// A shader that defines its own function of the same name is not
    /// calling ours, so it must not be refused.
    #[test]
    fn a_shader_that_defines_the_symbol_itself_is_fine() {
        let d = dl(
            "selfsufficient",
            Some("fn luminance(c: vec3<f32>) -> f32 { return c.g; }\nfn main() { luminance(x); }"),
        );
        assert!(check_download(&d).is_ok());
    }

    /// A built-in cannot be replaced by a download, matching variations.
    #[test]
    fn a_download_cannot_replace_a_builtin() {
        let mut reg = EffectRegistry::new();
        register_builtin_effects(&mut reg);
        let e = reg
            .register_from_api(
                &dl("vignette", Some("fn main() {}")),
                crate::provenance::Provenance::Api { version: 1 },
            )
            .expect_err("must refuse");
        assert!(e.contains("built in"), "{e}");
        assert!(
            reg.get("vignette").unwrap().provenance.is_builtin(),
            "the built-in must survive"
        );
    }
}
