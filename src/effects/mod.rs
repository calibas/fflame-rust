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

use std::sync::LazyLock;
use serde::{Deserialize, Serialize};
use rust_i18n::t;

use crate::params::{ParamDef, Spec};

/// Effects written in Rhai, compiled by the formula language (P2).
pub mod custom;

/// Register the effects a config carries (`FractalConfig::custom_effects`)
/// before its chains are built: every path that renders a config's effects
/// calls this.
pub fn ensure_custom(config: &crate::config::FractalConfig) {
    custom::ensure(config);
}

/// The blend modes a colour effect's `blend_mode` picks, in the order
/// `apply_blend` (shaders/effects/common/blend_modes.wgsl) numbers them.
pub const BLEND_MODE_NAMES: &[&str] = &[
    "Normal",
    "Add",
    "Multiply",
    "Screen",
    "Overlay",
    "Soft Light",
    "Hard Light",
    "Color Dodge",
    "Color Burn",
    "Hue",
    "Saturation",
    "Color",
    "Luminosity",
];

/// Effect category determines when in the pipeline the effect runs
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum EffectCategory {
    /// Runs before tonemap, has access to density in alpha channel
    Density,
    /// Runs after tonemap, operates on final RGB colors
    Color,
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
    pub const DISTANCE_FIELD: &str = include_str!("../../shaders/effects/color/distance_field.wgsl");

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
                if let Ok(from_disk) = std::fs::read_to_string(crate::resources::resource_path(format!("shaders/{path}"))) {
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

    /// The name to show.
    ///
    /// Built-in effects leave this empty and are labelled from
    /// `locales/*.yml`, where the curated names live (`sobel_edges` is
    /// "Edge Glow", so deriving one from the key would be wrong) and
    /// where translations can exist at all.
    ///
    /// Anything that did not ship must carry its own: a plugin author
    /// cannot add a locale entry, and a downloaded effect's label
    /// arrives on the wire. See [`Self::translated_name`].
    pub display_name: String,

    /// Where the WGSL comes from.
    pub source: EffectSource,

    /// Parameters for this effect
    pub parameters: Vec<ParamDef>,

    /// Where this effect came from, and therefore whether it is
    /// third-party code, cache, or updatable. See
    /// [`crate::provenance::Provenance`] for why those are three
    /// questions rather than one bool.
    pub provenance: crate::provenance::Provenance,

    /// A Rhai effect's labels as its source writes them (design rule 8):
    /// shown in the current language whenever they are shown
    /// ([`Self::translated_name`], [`Self::param_label`]). `None` for an
    /// effect labelled from the locale files or by plain strings.
    pub labels: Option<std::sync::Arc<crate::scripted::Labels>>,
}

impl EffectInfo {
    /// A parameter's label and hint in the current language: its source's
    /// when it has labels, else the definition's own.
    pub fn param_label(&self, p: &ParamDef) -> (String, Option<String>) {
        if let Some(l) = self.labels.as_ref().and_then(|l| l.param(&p.name)) {
            let caption = l.caption.resolve();
            let hint = l.hint.resolve();
            return (
                if caption.is_empty() { p.label.to_string() } else { caption },
                if hint.is_empty() { p.hint().map(str::to_string) } else { Some(hint) },
            );
        }
        (p.label.to_string(), p.hint().map(str::to_string))
    }

    /// The label to show for this effect.
    ///
    /// Locale first, so a built-in keeps its curated and translatable
    /// name. `rust-i18n` returns the **key** when there is no entry, so
    /// a miss has to be detected rather than trusted — otherwise
    /// `effects.plugin_example_tint.name` is what reaches the user, which
    /// is exactly what happened before this fell back.
    ///
    /// A plugin can never have a locale entry, so the fallback is not an
    /// edge case for them; it is the only path they take.
    pub fn translated_name(&self) -> String {
        let key = format!("effects.{}.name", self.name);
        let translated = t!(&key);
        if translated != key {
            return translated.to_string();
        }
        // A Rhai effect's title, by locale as written.
        if let Some(title) = self.labels.as_ref().map(|l| l.title.resolve()).filter(|t| !t.is_empty()) {
            return title;
        }
        if !self.display_name.is_empty() {
            return self.display_name.clone();
        }
        // Nothing declared a label. The raw name is a poor title but an
        // honest one, and it is at least the thing the user typed.
        self.name.clone()
    }

    /// Get the default value for a parameter by name
    pub fn get_param_default(&self, param_name: &str) -> Option<f32> {
        self.parameters
            .iter()
            .find(|p| p.name == param_name)
            .map(|p| p.slot_default())
    }

    /// Get parameter definition by name
    pub fn get_param(&self, param_name: &str) -> Option<&ParamDef> {
        self.parameters.iter().find(|p| p.name == param_name)
    }

    /// Get all default parameter values as a HashMap
    pub fn default_params(&self) -> HashMap<String, f32> {
        self.parameters
            .iter()
            .map(|p| (p.name.to_string(), p.slot_default()))
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

    /// Each name's registration, counted: what a compiled pipeline was
    /// built from, so a re-registered effect (a Rhai effect edited, a
    /// download updated) is compiled again ([`Self::revision`]).
    revisions: HashMap<String, u64>,
    registrations: u64,
}

impl EffectRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            effects: HashMap::new(),
            ordered_names: Vec::new(),
            revisions: HashMap::new(),
            registrations: 0,
        }
    }

    /// Register an effect
    pub fn register(&mut self, info: EffectInfo) {
        let name = info.name.clone();
        if !self.effects.contains_key(&name) {
            self.ordered_names.push(name.clone());
        }
        self.registrations += 1;
        self.revisions.insert(name.clone(), self.registrations);
        self.effects.insert(name, info);
    }

    /// Which registration of `name` is current (0: none). A pipeline built
    /// from an older one is stale.
    pub fn revision(&self, name: &str) -> u64 {
        self.revisions.get(name).copied().unwrap_or(0)
    }

    /// Remove an effect, any provenance; a pipeline built from it is then
    /// stale (its revision is gone).
    pub fn remove_by_name(&mut self, name: &str) -> Option<EffectInfo> {
        self.ordered_names.retain(|n| n != name);
        self.revisions.remove(name);
        self.effects.remove(name)
    }

    /// Remove every downloaded effect, what Clear Cache does (the
    /// variations' `clear_api`); a pipeline built from one is then stale.
    /// Gives how many went.
    pub fn clear_api(&mut self) -> usize {
        let names: Vec<String> = self.effects.values().filter(|i| i.provenance.is_cached_download()).map(|i| i.name.clone()).collect();
        for n in &names {
            self.remove_by_name(n);
        }
        names.len()
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
static EFFECT_REGISTRY: LazyLock<RwLock<EffectRegistry>> = LazyLock::new(|| {
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
static MISSING_EFFECTS: LazyLock<std::sync::Mutex<std::collections::BTreeSet<String>>> =
    LazyLock::new(Default::default);

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
    static SYMBOLS: LazyLock<Vec<String>> = LazyLock::new(|| {
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
            // The `&'static` comes from the leaked LazyLock, which lives for
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

    // An unranged float as the Effects panel has always drawn one: 0..1.
    let parameters = dl.parameters.iter().map(|p| crate::params::from_api(p, (0.0, 1.0))).collect();

    Ok(EffectInfo {
        name,
        // Was dropped on the floor here. `EffectInfo` had no field for
        // it, so a downloaded or local effect fell through to the locale
        // lookup, missed, and displayed the raw key.
        display_name: dl.display_name.clone(),
        category,
        source: EffectSource::Owned(shader),
        parameters,
        provenance: crate::provenance::Provenance::Api { version: dl.version },
        labels: None,
    })
}

impl EffectRegistry {
    /// Register an effect from a download payload.
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
            // Never shadow a built-in: displacing one would change what
            // a shared flame renders.
            if existing.provenance.is_builtin() {
                return Err(format!(
                    "effect `{}` is built in; it cannot be replaced",
                    dl.name
                ));
            }
        }
        let mut info = check_download(dl)?;
        info.provenance = provenance;
        self.register(info);
        Ok(())
    }
}

/// Register the built-in effects: each one-pass effect as its Rhai twin
/// (D1, docs/archive/projects/rhai-plugins.md P2 step 7: [`custom::builtin_twin`]);
/// `distance_field`, three passes, as its WGSL.
fn register_builtin_effects(registry: &mut EffectRegistry) {
    for info in wgsl_builtins() {
        if let Some(twin) = custom::builtin_twin(&info.name) {
            registry.register(twin);
            continue;
        }
        registry.register(info);
    }
}

/// The built-in effects as their WGSL, in the order the lists show them:
/// the reference each Rhai twin is held to
/// (`the_rhai_twins_match_their_wgsl`).
pub fn wgsl_builtins() -> Vec<EffectInfo> {
    let mut out = Vec::new();
    // === Color Effects (after tonemap) ===

    // Vignette - darken edges
    out.push(EffectInfo {
        name: "vignette".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::VIGNETTE,
            path: "effects/color/vignette.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(0.5, 0.0, 1.0),
            ),
            ParamDef::new(
                "radius",
                "Radius",
                "",
                Spec::float(0.5, 0.0, 1.0),
            ),
            ParamDef::new(
                "softness",
                "Softness",
                "",
                Spec::float(0.5, 0.0, 1.0),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(0, BLEND_MODE_NAMES),
            ),
        ],
    });

    // Film Grain - per-pixel random noise
    out.push(EffectInfo {
        name: "film_grain".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::FILM_GRAIN,
            path: "effects/color/film_grain.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(0.1, 0.0, 1.0),
            ),
            ParamDef::new(
                "seed",
                "Seed",
                "",
                Spec::float(0.0, 0.0, 256.0),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(4, BLEND_MODE_NAMES),
            ),
        ],
    });

    // Chromatic Aberration - RGB channel offset
    out.push(EffectInfo {
        name: "chromatic_aberration".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::CHROMATIC_ABERRATION,
            path: "effects/color/chromatic_aberration.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "amount",
                "Amount",
                "",
                Spec::float(2.0, 0.0, 20.0),
            ),
            ParamDef::new(
                "radial",
                "Radial",
                "",
                Spec::bool(true),
            ),
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(1.0, 0.0, 1.0),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(0, BLEND_MODE_NAMES),
            ),
        ],
    });

    // Hue Shift - static hue rotation (animate via keyframes)
    out.push(EffectInfo {
        name: "hue_shift".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::HUE_CYCLE,
            path: "effects/color/hue_cycle.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "offset",
                "Hue Offset",
                "",
                Spec::angle(0.0, 0.0, 360.0),
            ),
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(1.0, 0.0, 1.0),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(0, BLEND_MODE_NAMES),
            ),
        ],
    });

    // === Density Effects (before tonemap) ===

    // Density Blur - blur weighted by density
    out.push(EffectInfo {
        name: "density_blur".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Density,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::DENSITY_BLUR,
            path: "effects/density/density_blur.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "radius",
                "Radius",
                "",
                Spec::float(3.0, 0.0, 10.0),
            ),
            ParamDef::new(
                "threshold",
                "Density Threshold",
                "Density, in multiples of the frame's mean density, below which a pixel blurs at the full radius. 0.25 blurs the parts sparser than a quarter of the average.",
                Spec::float(0.25, 0.0, 1.0),
            ),
            ParamDef::new(
                "falloff",
                "Falloff",
                "How far above the threshold, in multiples of the mean density, the blur fades out to none.",
                Spec::float(0.5, 0.0, 1.0),
            ),
        ],
    });

    // Sharpen - detail enhancement
    out.push(EffectInfo {
        name: "sharpen".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Density,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::SHARPEN,
            path: "effects/density/sharpen.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "amount",
                "Amount",
                "",
                Spec::float(1.0, 0.0, 2.0),
            ),
            ParamDef::new(
                "radius",
                "Radius",
                "",
                Spec::float(1.0, 0.5, 5.0),
            ),
        ],
    });

    // Bilateral Blur - edge-preserving blur
    out.push(EffectInfo {
        name: "bilateral_blur".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Density,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::BILATERAL_BLUR,
            path: "effects/density/bilateral_blur.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "radius",
                "Radius",
                "",
                Spec::float(3.0, 1.0, 10.0),
            ),
            ParamDef::new(
                "sigma_spatial",
                "Spatial Sigma",
                "",
                Spec::float(3.0, 1.0, 10.0),
            ),
            ParamDef::new(
                "sigma_range",
                "Range Sigma",
                "",
                Spec::float(0.1, 0.05, 0.5),
            ),
        ],
    });

    // === Psychedelic Color Effects ===

    // Kaleidoscope - N-fold rotational symmetry
    out.push(EffectInfo {
        name: "kaleidoscope".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::KALEIDOSCOPE,
            path: "effects/color/kaleidoscope.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "segments",
                "Segments",
                "",
                Spec::float(6.0, 2.0, 16.0),
            ),
            ParamDef::new(
                "rotation",
                "Source Angle",
                "",
                Spec::angle(0.0, 0.0, 360.0),
            ),
            ParamDef::new(
                "zoom",
                "Zoom",
                "",
                Spec::float(1.0, 0.1, 3.0),
            ),
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(1.0, 0.0, 1.0),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(0, BLEND_MODE_NAMES),
            ),
            ParamDef::new(
                "square_mode",
                "Square Mode",
                "",
                Spec::choice(1, &["Screen ratio (stretched)", "Square"]),
            ),
            ParamDef::new(
                "edge_offset",
                "Edge Offset",
                "",
                Spec::int(0, 0, 2000),
            ),
        ],
    });

    // Plasma - classic demoscene effect
    out.push(EffectInfo {
        name: "plasma".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::PLASMA,
            path: "effects/color/plasma.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(0.3, 0.0, 1.0),
            ),
            ParamDef::new(
                "scale",
                "Scale",
                "",
                Spec::float(3.0, 0.1, 100.0),
            ),
            ParamDef::new(
                "speed",
                "Speed",
                "",
                Spec::float(1.0, 0.0, 10.0),
            ),
            ParamDef::new(
                "time",
                "Time",
                "",
                Spec::float(0.0, 0.0, 1000.0),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(1, BLEND_MODE_NAMES),
            ),
            ParamDef::new(
                "direction",
                "Direction",
                "",
                Spec::angle(225.0, 0.0, 360.0),
            ),
        ],
    });

    // Tunnel - infinite tunnel effect
    out.push(EffectInfo {
        name: "tunnel".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::TUNNEL,
            path: "effects/color/tunnel.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "speed",
                "Speed",
                "",
                Spec::float(1.0, 0.0, 5.0),
            ),
            ParamDef::new(
                "rotation_speed",
                "Rotation Speed",
                "",
                Spec::float(0.5, 0.0, 5.0),
            ),
            ParamDef::new(
                "distortion",
                "Distortion",
                "",
                Spec::float(0.0, 0.0, 1.0),
            ),
            ParamDef::new(
                "time",
                "Time",
                "",
                Spec::float(0.0, 0.0, 1000.0),
            ),
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(1.0, 0.0, 1.0),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(0, BLEND_MODE_NAMES),
            ),
        ],
    });

    // Sobel Edges - neon edge detection
    out.push(EffectInfo {
        name: "sobel_edges".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::SOBEL_EDGES,
            path: "effects/color/sobel_edges.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(1.0, 0.0, 2.0),
            ),
            ParamDef::new(
                "threshold",
                "Threshold",
                "",
                Spec::float(0.1, 0.0, 1.0),
            ),
            ParamDef::new(
                "glow",
                "Glow",
                "",
                Spec::float(0.5, 0.0, 1.0),
            ),
            ParamDef::new(
                "preserve_color",
                "Preserve Color",
                "",
                Spec::float(0.5, 0.0, 1.0),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(1, BLEND_MODE_NAMES),
            ),
        ],
    });

    // Domain Warp - organic noise distortion
    out.push(EffectInfo {
        name: "domain_warp".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::DOMAIN_WARP,
            path: "effects/color/domain_warp.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(0.1, 0.0, 0.5),
            ),
            ParamDef::new(
                "scale",
                "Scale",
                "",
                Spec::float(3.0, 0.5, 10.0),
            ),
            ParamDef::new(
                "octaves",
                "Octaves",
                "",
                Spec::int(4, 1, 6),
            ),
            ParamDef::new(
                "time",
                "Time",
                "",
                Spec::float(0.0, 0.0, 1000.0),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(0, BLEND_MODE_NAMES),
            ),
            ParamDef::new(
                "direction",
                "Direction",
                "",
                Spec::angle(225.0, 0.0, 360.0),
            ),
        ],
    });

    // Simplex Noise - psychedelic noise overlay
    out.push(EffectInfo {
        name: "simplex_noise".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::SIMPLEX_NOISE,
            path: "effects/color/simplex_noise.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(0.5, 0.0, 1.0),
            ),
            ParamDef::new(
                "scale",
                "Scale",
                "",
                Spec::float(5.0, 0.5, 20.0),
            ),
            ParamDef::new(
                "octaves",
                "Octaves",
                "",
                Spec::int(4, 1, 6),
            ),
            ParamDef::new(
                "time",
                "Time",
                "",
                Spec::float(0.0, 0.0, 1000.0),
            ),
            ParamDef::new(
                "mode",
                "Mode",
                "",
                Spec::choice(0, &["Color", "Distort", "Mask"]),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(4, BLEND_MODE_NAMES),
            ),
            ParamDef::new(
                "direction",
                "Direction",
                "",
                Spec::angle(225.0, 0.0, 360.0),
            ),
        ],
    });

    // Worley Noise - cellular patterns
    out.push(EffectInfo {
        name: "worley_noise".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::WORLEY_NOISE,
            path: "effects/color/worley_noise.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(0.5, 0.0, 1.0),
            ),
            ParamDef::new(
                "scale",
                "Scale",
                "",
                Spec::float(8.0, 1.0, 20.0),
            ),
            ParamDef::new(
                "time",
                "Time",
                "",
                Spec::float(0.0, 0.0, 1000.0),
            ),
            ParamDef::new(
                "mode",
                "Pattern",
                "",
                Spec::choice(0, &["Cells", "Edges", "Organic", "Crystal"]),
            ),
            ParamDef::new(
                "color_mode",
                "Color Mode",
                "",
                Spec::choice(1, &["Grayscale", "Rainbow", "Original"]),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(4, BLEND_MODE_NAMES),
            ),
            ParamDef::new(
                "direction",
                "Direction",
                "",
                Spec::angle(225.0, 0.0, 360.0),
            ),
        ],
    });

    // Julia/Mandelbrot - fractal overlay
    out.push(EffectInfo {
        name: "julia".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::JULIA,
            path: "effects/color/julia.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "mode",
                "Mode",
                "",
                Spec::choice(1, &["Mandelbrot", "Julia"]),
            ),
            ParamDef::new(
                "julia_cx",
                "Julia C (Real)",
                "",
                Spec::float(-0.7, -2.0, 2.0),
            ),
            ParamDef::new(
                "julia_cy",
                "Julia C (Imag)",
                "",
                Spec::float(0.27, -2.0, 2.0),
            ),
            ParamDef::new(
                "max_iter",
                "Max Iterations",
                "",
                Spec::int(100, 10, 500),
            ),
            ParamDef::new(
                "zoom",
                "Zoom",
                "",
                Spec::float(1.0, 0.1, 100.0),
            ),
            ParamDef::new(
                "center_x",
                "Center X",
                "",
                Spec::float(0.0, -3.0, 3.0),
            ),
            ParamDef::new(
                "center_y",
                "Center Y",
                "",
                Spec::float(0.0, -3.0, 3.0),
            ),
            ParamDef::new(
                "color_mode",
                "Color Mode",
                "",
                Spec::choice(1, &["Escape", "Smooth", "Original mask"]),
            ),
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(1.0, 0.0, 1.0),
            ),
            ParamDef::new(
                "blend_mode",
                "Blend Mode",
                "",
                Spec::choice(0, BLEND_MODE_NAMES),
            ),
        ],
    });

    // Distance Field - jump-flood the rendered attractor into an
    // exterior distance field, then shade it (glow / contour bands /
    // nearest fill). The escape-time plan's §7.3 bridge: DE-style
    // looks for ARBITRARY flames, no invertibility required. The
    // chain runner special-cases its execution into a multi-pass
    // pipeline (seed + ~log2(max dim) floods + composite).
    out.push(EffectInfo {
        name: "distance_field".to_string(),
        // Empty on purpose: built-ins are labelled from
        // `locales/*.yml`, where the curated names live.
        display_name: String::new(),
        category: EffectCategory::Color,
        source: EffectSource::Builtin {
            embedded: embedded_shaders::DISTANCE_FIELD,
            path: "effects/color/distance_field.wgsl",
        },
        provenance: crate::provenance::Provenance::Builtin,
        labels: None,
        parameters: vec![
            ParamDef::new(
                "threshold",
                "Seed Threshold",
                "Pixels brighter than this seed the distance field.",
                Spec::float(0.15, 0.0, 1.0),
            ),
            ParamDef::new(
                "spread",
                "Spread",
                "Field range as a fraction of the image's larger dimension.",
                Spec::float(0.25, 0.01, 1.0),
            ),
            ParamDef::new(
                "mode",
                "Mode",
                "0 = glow, 1 = contour bands, 2 = nearest-color fill.",
                Spec::choice(0, &["Glow", "Contours", "Nearest fill"]),
            ),
            ParamDef::new(
                "intensity",
                "Intensity",
                "",
                Spec::float(1.0, 0.0, 2.0),
            ),
            ParamDef::new(
                "band_count",
                "Band Count",
                "Contour rings across the range (mode 1).",
                Spec::float(12.0, 1.0, 64.0),
            ),
        ],
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dropdown's names are `apply_blend`'s cases, in its order: a
    /// mode added to the shader, or a reordering, fails here instead of
    /// labelling every blend wrongly.
    #[test]
    fn blend_mode_names_match_apply_blend() {
        let cases: Vec<(usize, String)> = embedded_shaders::BLEND_MODES
            .lines()
            .filter_map(|l| l.trim().strip_prefix("case "))
            .filter_map(|rest| {
                let (n, body) = rest.split_once(':')?;
                let f = body.split("blend_").nth(1)?.split('(').next()?;
                Some((n.trim().parse().ok()?, f.to_string()))
            })
            .collect();
        let named: Vec<(usize, String)> = BLEND_MODE_NAMES
            .iter()
            .enumerate()
            .map(|(i, n)| (i, n.to_lowercase().replace(' ', "_")))
            .collect();
        assert_eq!(cases, named);
    }

    /// An enum's range is its choice list, so a value the slider used
    /// to write is the same entry in the dropdown.
    #[test]
    fn enum_effect_params_span_their_choices() {
        let registry = global_effect_registry();
        let mut seen = 0;
        for info in registry.all() {
            for p in &info.parameters {
                let Some(choices) = p.options() else { continue };
                seen += 1;
                let last = choices.len() as f32 - 1.0;
                assert_eq!(p.slot_range(), (0.0, last), "{}.{}", info.name, p.name);
                assert!((0.0..=last).contains(&p.slot_default()) && p.slot_default().fract() == 0.0, "{}.{}", info.name, p.name);
            }
        }
        assert!(seen >= 19, "{seen}");
    }

    #[test]
    fn distance_field_shader_validates_all_entry_points() {
        // The chain runner builds three pipelines from this one
        // module (fs_seed / fs_flood / fs_composite); a WGSL error in
        // any of them is a runtime pipeline panic, so validate at
        // test time like the escape templates do.
        use wgpu::naga;
        let src = embedded_shaders::DISTANCE_FIELD;
        let module = naga::front::wgsl::parse_str(src)
            .unwrap_or_else(|e| panic!("distance_field parse: {e}"));
        for entry in ["vs_main", "fs_seed", "fs_flood", "fs_composite"] {
            assert!(
                module.entry_points.iter().any(|ep| ep.name == entry),
                "missing entry point {entry}"
            );
        }
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("distance_field validation: {e:?}"));
        use crate::variations::shader_lint;
        assert!(shader_lint::self_operations(src).is_empty());
        assert!(shader_lint::subnormal_literals(src).is_empty());
        assert!(shader_lint::mixed_operators(src).is_empty(), "{:?}", shader_lint::mixed_operators(src));
    }

    /// Effects sample with an explicit level, never an implicit
    /// derivative.
    ///
    /// `textureSample` (like `textureSampleBias` and
    /// `textureSampleCompare`) takes screen-space derivatives, which WGSL
    /// allows only in uniform control flow. Chrome's Tint enforces that;
    /// naga, which validates the desktop build, does not check fragment
    /// uniformity. So density_blur -- a per-pixel early return on a
    /// density-dependent radius, then samples in its blur loop -- and
    /// tunnel -- a return at the centre pixel, then a sample -- compiled
    /// on desktop and failed in the browser. The effect textures have
    /// one mip, so `textureSampleLevel(.., 0.0)` reads exactly what
    /// `textureSample` did, in any control flow; every effect uses it
    /// now, so a new branch cannot reintroduce the failure.
    ///
    /// Effects are also published to the API, whose publish check could
    /// apply the same rule to downloaded ones.
    #[test]
    fn effects_never_sample_with_implicit_derivatives() {
        fn visit(dir: &std::path::Path, f: &mut impl FnMut(&std::path::Path, &str)) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, f);
                } else if path.extension().and_then(|e| e.to_str()) == Some("wgsl") {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        f(&path, &text);
                    }
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders/effects");
        let mut violations = Vec::new();
        let mut checked = 0usize;
        visit(&root, &mut |path, text| {
            checked += 1;
            let code = crate::variations::shader_lint::strip_wgsl_comments(text);
            for builtin in ["textureSample(", "textureSampleBias(", "textureSampleCompare("] {
                for (at, _) in code.match_indices(builtin) {
                    let before = code[..at].bytes().next_back();
                    if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_') {
                        continue; // part of a longer identifier
                    }
                    let line = code[..at].matches('\n').count() + 1;
                    violations.push(format!("{}:{line}: {builtin}..)", path.display()));
                }
            }
        });
        assert!(checked > 10, "scanned only {checked} effect shaders -- the walk is broken");
        assert!(
            violations.is_empty(),
            "implicit-derivative sampling in an effect (a uniformity error in Chrome \
             wherever control flow depends on the pixel); use textureSampleLevel(.., 0.0):\n  {}",
            violations.join("\n  ")
        );
    }

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

    /// Clear Cache removes the downloads, and only them.
    #[test]
    fn clear_api_removes_the_downloads_alone() {
        let mut reg = EffectRegistry::new();
        register_builtin_effects(&mut reg);
        let builtins = reg.len();
        reg.register_from_api(&dl("fetched_one", Some("fn main() {}")), crate::provenance::Provenance::Api { version: 1 }).unwrap();
        let mut mine = check_download(&dl("mine_one", Some("fn main() {}"))).unwrap();
        mine.provenance = crate::provenance::Provenance::Local;
        reg.register(mine);
        assert_eq!(reg.clear_api(), 1);
        assert!(!reg.contains("fetched_one") && reg.contains("mine_one") && reg.contains("vignette"));
        assert_eq!(reg.len(), builtins + 1);
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

#[cfg(test)]
mod label_tests {
    use super::*;

    /// A built-in is labelled from the locale, where the curated name
    /// lives. `sobel_edges` is "Edge Glow", so a name derived from the
    /// registry key would be wrong, not merely ugly. A built-in that is
    /// its Rhai twin carries a title too, which is the locale's English,
    /// so a copy of it opened in the Formula Editor reads the same.
    #[test]
    fn a_builtin_takes_its_curated_locale_name() {
        let reg = global_effect_registry();
        let sobel = reg.get("sobel_edges").expect("ships");
        assert_eq!(sobel.translated_name(), "Edge Glow");
        let wgsl = wgsl_builtins().into_iter().find(|i| i.name == "distance_field").expect("ships");
        assert!(wgsl.display_name.is_empty(), "a WGSL built-in carries no inline label");
        for (name, source) in custom::all_twins() {
            let twin = custom::compile(source, crate::provenance::Provenance::Builtin).expect("compiles");
            let info = reg.get(name).expect("ships");
            assert_eq!(twin.display_name, info.translated_name(), "{name}: its title is the locale's");
        }
    }

    /// A plugin or downloaded effect uses the label it declares.
    ///
    /// It has no other option: `rust-i18n` returns the KEY on a miss,
    /// and a plugin author cannot add a locale entry. Before this fell
    /// back, `effects.plugin_example_tint.name` is what appeared in the
    /// effect picker — found by using the app, not by a test.
    #[test]
    fn a_plugin_uses_the_label_it_declares() {
        let info = EffectInfo {
            name: "plugin_example_tint".into(),
            display_name: "Plugin Example (Tint)".into(),
            category: EffectCategory::Color,
            source: EffectSource::Owned("fn main() {}".into()),
            parameters: Vec::new(),
            provenance: crate::provenance::Provenance::Local,
            labels: None,
        };
        assert_eq!(info.translated_name(), "Plugin Example (Tint)");
        assert!(
            !info.translated_name().contains("effects."),
            "a locale key must never reach the user"
        );
    }

    /// Declaring nothing at all still gives a usable label rather than
    /// a key.
    #[test]
    fn a_nameless_effect_falls_back_to_its_registry_name() {
        let info = EffectInfo {
            name: "unnamed_thing".into(),
            display_name: String::new(),
            category: EffectCategory::Color,
            source: EffectSource::Owned("fn main() {}".into()),
            parameters: Vec::new(),
            provenance: crate::provenance::Provenance::Local,
            labels: None,
        };
        assert_eq!(info.translated_name(), "unnamed_thing");
    }

    /// Every shipped effect must have its locale entry. A missing one
    /// now degrades quietly to the raw name instead of showing a key,
    /// which is better for the user and worse for noticing — so this
    /// checks the whole corpus.
    #[test]
    fn every_shipped_effect_has_a_curated_label() {
        let reg = global_effect_registry();
        for info in reg.all() {
            let label = info.translated_name();
            assert_ne!(
                label, info.name,
                "`{}` has no `effects.{}.name` in locales/en.yml",
                info.name, info.name
            );
            assert!(!label.starts_with("effects."), "{label}");
        }
    }
}
