//! Variation definition types
//!
//! This module provides the `VariationDef` struct for defining variations
//! with their metadata and WGSL shader code in a single declaration.

use super::{ParamType, VariationCategory, VariationParameter, VariationPhase};

/// Capability/requirement flags a variation can opt into. Replaces what
/// used to be a growing pile of `pub <flag>: bool` fields on
/// `VariationDef` — adding a new feature is now a single enum variant
/// plus a `has_feature` check at the relevant codegen site rather than
/// a bulk edit across every variation file.
///
/// Each variation lists the features it uses in
/// `VariationDef::features: &'static [Feature]`; absence ⇒ doesn't
/// have / doesn't need that capability. Lookup is via
/// `VariationDef::has_feature` (and the mirror on `VariationInfo`) —
/// linear scan over the slice, which is fine because `Feature` has a
/// handful of variants and lookups happen at shader-build time, not
/// per-iteration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    /// Variation reads from the per-thread RNG state. When listed, the
    /// generated function signature gains `rng: ptr<function, RngState>`
    /// and the dispatch site passes the thread-local RNG pointer.
    NeedsRng,

    /// Variation reads fields from `transforms[xform_id]` (affine
    /// matrix, weight, color, opacity, direct_color). When listed, the
    /// function signature gains `xform_id: u32` even if the variation
    /// has no parameters — variations with parameters already get
    /// `xform_id` for the `get_param` indirection.
    NeedsTransform,

    /// Variation writes to the iteration-local palette-index register
    /// `vc` (Apophysis direct-color "DC" variations). When listed, the
    /// function signature gains `vc: ptr<function, f32>`; the main
    /// loop's color step lerps between standard color evolution and
    /// `vc` using the transform's `direct_color` field.
    WritesColor,

    /// Variation writes a direct RGB color into the iteration-local
    /// `vrc` register (vec3, parallel to `vc`'s palette-index path).
    /// When listed, the function signature gains
    /// `vrc: ptr<function, vec3<f32>>`; the main loop blends the
    /// variation's RGB output with the palette-sampled color via the
    /// transform's `direct_color`, bypassing the palette texture
    /// lookup for the RGB portion of the final color. Used by the
    /// `glsl_*` family of variations (JWildfire's shadertoy-style
    /// procedural shapes), which compute RGB directly from a
    /// per-pixel algorithm.
    ///
    /// The RGB override is gated per-iteration: `vrc` is sentinel-init
    /// to an out-of-gamut value (`-1e30`), and the plot only applies
    /// the override when a WritesRgb variation actually wrote a colour
    /// this iteration. So in a mixed flame the transforms that have no
    /// WritesRgb variation keep their palette colour instead of being
    /// blended toward the unwritten register (JWildfire gates this
    /// per-point via `pVarTP.rgbColor`). A WritesRgb variation MUST
    /// write a normal-range colour (components well within ±1e29).
    WritesRgb,

    /// Variation reads the running variation-accumulator (Apophysis
    /// `FPx/FPy/FPz`) so it can compose with prior variations in the
    /// same iteration. When listed, the function signature gains
    /// `accum: vec2<f32>` (or `vec3<f32>` in 3D) right after `p`, and
    /// the shader builder passes the current weighted-sum value.
    /// Effective only in normal and post phases.
    NeedsAccum,

    /// The variation's JWF source writes `pVarTP.z` UNCONDITIONALLY
    /// (true-3D variations: Julia3DFunc, ZConeFunc, Linear3DFunc, …).
    /// In a 3D shader its z contribution is kept under both
    /// `preserve_z` settings — this is what lets z compound across
    /// iterations the way JWF does.
    ///
    /// Absent (the default), the variation is treated as JWF's
    /// standard gated pattern — `if (isPreserveZCoordinate())
    /// pVarTP.z += pAmount·z` — and the 3D dispatch site zeroes the
    /// z component of its contribution when `preserve_z = false`.
    /// Our 3D bodies return `p.z` passthrough, which is exactly the
    /// gated add when kept and exactly JWF's skip when zeroed.
    ///
    /// Classified by `scripts/audit_z_write_semantics.py` against
    /// `output/variation-jwf-source/*.java`; cite the source when
    /// setting this by hand. Mutually exclusive with [`Self::NeverZ`].
    AlwaysZ,

    /// The variation's JWF source never writes `pVarTP.z` — not even
    /// the gated passthrough. Its z contribution is zeroed under BOTH
    /// `preserve_z` settings. NOTE: deliberately NOT auto-applied by
    /// the audit script — our hand-written 3D bodies sometimes extend
    /// 2D variations on purpose, so enforcement needs per-def review.
    /// Mutually exclusive with [`Self::AlwaysZ`].
    NeverZ,

    /// The variation **replaces** the working point rather than
    /// accumulating into it: its JWildfire source assigns `pVarTP.x = …`
    /// (not `+=`). Intrinsic to the variation and independent of its
    /// phase. It only changes the *combine* form of the pre/post
    /// emission when the variation is moved off its natural phase via
    /// `fx_priority` (see [`VariationPhase::Any`]):
    ///   - pre:  accumulate `temp = temp + w·body(temp)` vs Replace `temp = w·body(temp)`
    ///   - post: accumulate `result = result + w·body(result)` vs Replace `result = w·body(result)`
    /// The normal-phase dispatch is unchanged either way (replace-style
    /// normal variations already use the idisc pattern — read their own
    /// weight and pre-divide so the dispatcher's `result += w·body`
    /// cancels to the assigned value when they're the sole variation).
    /// Sourced from the JWF `=` vs `+=` classification; see
    /// `docs/projects/jwf-features.md`.
    Replace,
}

/// Static definition of a variation
///
/// Each variation is defined as a const with all its metadata and WGSL code.
/// The 2D and 3D implementations are in the same definition.
///
/// # Example
/// ```ignore
/// pub const SPHERICAL: VariationDef = VariationDef {
///     name: "spherical",
///     display_name: "Spherical",
///     category: VariationCategory::Basic2D,
///     phase: VariationPhase::Normal,
///     needs_rng: false,
///     parameters: &[],
///     wgsl_2d: r#"
/// fn variation_spherical(p: vec2<f32>) -> vec2<f32> {
///     let r2 = dot(p, p) + 1e-6;
///     return p / r2;
/// }
/// "#,
///     wgsl_3d: r#"
/// fn variation_spherical(p: vec3<f32>) -> vec3<f32> {
///     let r2 = dot(p.xy, p.xy) + 1e-6;
///     return vec3(p.xy / r2, p.z);
/// }
/// "#,
/// };
/// ```
pub struct VariationDef {
    /// Unique identifier (lowercase, snake_case)
    pub name: &'static str,

    /// Additional names this variation is known by in other apps
    /// (Apophysis 7X, JWildfire, Chaotica). Used during `.flame` XML
    /// import to map foreign names to our canonical `name`.
    ///
    /// Example: `linear` lists `&["linear3D"]` because Apo 7X / JWildfire
    /// have a separate `linear3D` variation while ours handles both 2D
    /// and 3D modes from the same definition. Without the alias the
    /// `linear3D="…"` attribute gets silently dropped on import.
    ///
    /// Default `&[]` for variations with no foreign-app aliases. Add as
    /// the import path encounters drops; no exhaustive research needed
    /// upfront.
    pub aliases: &'static [&'static str],

    /// Display name for UI
    pub display_name: &'static str,

    /// Category for organization
    pub category: VariationCategory,

    /// Execution phase (pre/normal/post)
    pub phase: VariationPhase,

    /// Capability/requirement flags. See [`Feature`] for individual
    /// variant docs. Replaces what used to be a growing set of
    /// `pub <name>: bool` fields (`needs_rng`, `needs_transform`,
    /// `writes_color`, `needs_accum`, ...). Adding a future feature
    /// is a new enum variant + the codegen site that consumes it; no
    /// bulk edit across every variation file.
    ///
    /// Empty slice ⇒ a "plain" variation that just reads `p` and
    /// writes its return value (the linear/sinusoidal/etc. shape).
    pub features: &'static [Feature],

    /// Parameters for this variation
    pub parameters: &'static [VariationParamDef],

    /// Number of init-derived ("private") parameters this variation produces.
    /// Stored alongside user parameters in the variation_params buffer at slots
    /// `parameters.len()..parameters.len() + init_param_count`.
    pub init_param_count: usize,

    /// Optional WGSL init function. When `Some`, a small GPU compute dispatch
    /// runs once per param change and writes derived values into the buffer.
    /// The variation body reads them via `get_param(...)` like any other slot.
    /// Function signature: `fn init_NAME(user: array<f32, N>) -> array<f32, M>`
    /// where N = parameters.len() and M = init_param_count.
    pub wgsl_init: Option<&'static str>,

    /// Number of f32 state slots this variation owns per (xform, variation)
    /// instance. Slots are zero-initialized at the start of each shader
    /// invocation (one main() call = one compute dispatch) and persist
    /// across the inner iteration loop within that invocation. Variations
    /// access their slots via the generated `get_state` / `set_state`
    /// accessors. Default 0 (no state).
    ///
    /// See [`docs/projects/intra-iteration-state-and-accum.md`](../../docs/projects/intra-iteration-state-and-accum.md).
    pub state_count: usize,

    /// Optional WGSL fragment that runs once at thread start (inside main(),
    /// before the iteration loop) to initialize this variation's state slots
    /// beyond zero-fill. Has `xform_id`, `variation_id`, and `set_state` in
    /// scope. Default None (zero-init suffices).
    pub wgsl_state_init: Option<&'static str>,

    /// 2D WGSL implementation
    /// Function signature should match one of:
    /// - `fn variation_NAME(p: vec2<f32>) -> vec2<f32>`
    /// - `fn variation_NAME(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32>` (if needs_rng)
    /// - `fn variation_NAME(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32>` (if has params)
    /// - Full signature with both rng and params
    pub wgsl_2d: &'static str,

    /// 3D WGSL implementation. Required for every variation — the shader
    /// builder cannot synthesize a sensible 3D body from the 2D one
    /// because the function-return type would mismatch (`vec2<f32>` vs
    /// `vec3<f32>`) and trigger a WGSL validation error.
    /// For variations whose math is 2D-shaped, the 3D body typically
    /// just mirrors the 2D math with `vec3<f32>` in/out and passes
    /// `p.z` through unchanged on the return.
    pub wgsl_3d: &'static str,
}

/// Static parameter definition for variations
pub struct VariationParamDef {
    /// Parameter name (lowercase)
    pub name: &'static str,

    /// Display name for UI
    pub display_name: &'static str,

    /// Parameter type
    pub param_type: ParamType,

    /// Default value
    pub default_value: f32,

    /// Minimum value (None = no limit for slider, typing still allowed)
    pub min_value: Option<f32>,

    /// Maximum value (None = no limit for slider, typing still allowed)
    pub max_value: Option<f32>,

    /// Free-form help / tooltip prose shown under the parameter
    /// control. `None` renders the control without a tooltip. Single
    /// English locale by policy (technical descriptions, not subject
    /// to i18n).
    pub description: Option<&'static str>,
}

impl VariationParamDef {
    /// Convert to runtime VariationParameter
    pub fn to_runtime(&self) -> VariationParameter {
        VariationParameter {
            name: self.name.to_string(),
            display_name: self.display_name.to_string(),
            param_type: self.param_type.clone(),
            default_value: self.default_value,
            min_value: self.min_value,
            max_value: self.max_value,
            description: self.description.map(|s| s.to_string()),
        }
    }
}

impl VariationDef {
    /// True if this variation lists the given feature in `features`.
    /// Linear-scan over a tiny slice — cheap, and only called at
    /// shader-build time.
    pub fn has_feature(&self, f: Feature) -> bool {
        self.features.contains(&f)
    }

    /// Total slots this variation occupies in the packed parameter buffer.
    ///
    /// Equals `parameters.len() + init_param_count`. User params live in
    /// slots `[0, parameters.len())` and init-derived params live in slots
    /// `[parameters.len(), parameters.len() + init_param_count)`. A
    /// variation with no parameters and no init slots takes 0 slots.
    pub fn slot_count(&self) -> usize {
        self.parameters.len() + self.init_param_count
    }

    /// Get the WGSL function name
    pub fn wgsl_function_name(&self) -> String {
        if self.name == "julia" {
            // Julia is special - no "variation_" prefix
            self.name.to_string()
        } else {
            format!("variation_{}", self.name)
        }
    }

    /// Convert parameters to runtime format
    pub fn parameters_to_runtime(&self) -> Vec<VariationParameter> {
        self.parameters.iter().map(|p| p.to_runtime()).collect()
    }

    /// Get the complete 2D WGSL source
    pub fn wgsl_source_2d(&self) -> &'static str {
        self.wgsl_2d
    }

    /// Get the 3D WGSL source verbatim. The field is required at type
    /// level — every variation provides its own 3D body.
    pub fn wgsl_source_3d(&self) -> &'static str {
        self.wgsl_3d
    }
}

/// Macro to simplify variation parameter definition.
///
/// Each typed form has two arms: one without description (default
/// `None`), one with a trailing `$desc:expr` arg. Pick whichever matches
/// the call site — macro_rules dispatches by arity.
#[macro_export]
macro_rules! param {
    // ---- Float ----
    ($name:expr, $display:expr, float, $default:expr, $min:expr, $max:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::Float,
            default_value: $default, min_value: Some($min), max_value: Some($max),
            description: None,
        }
    };
    ($name:expr, $display:expr, float, $default:expr, $min:expr, $max:expr, $desc:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::Float,
            default_value: $default, min_value: Some($min), max_value: Some($max),
            description: Some($desc),
        }
    };
    // ---- UnlimitedFloat ----
    ($name:expr, $display:expr, unlimited_float, $default:expr, $min:expr, $max:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::UnlimitedFloat,
            default_value: $default, min_value: Some($min), max_value: Some($max),
            description: None,
        }
    };
    ($name:expr, $display:expr, unlimited_float, $default:expr, $min:expr, $max:expr, $desc:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::UnlimitedFloat,
            default_value: $default, min_value: Some($min), max_value: Some($max),
            description: Some($desc),
        }
    };
    // ---- Integer ----
    ($name:expr, $display:expr, int, $default:expr, $min:expr, $max:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::Integer,
            default_value: $default, min_value: Some($min), max_value: Some($max),
            description: None,
        }
    };
    ($name:expr, $display:expr, int, $default:expr, $min:expr, $max:expr, $desc:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::Integer,
            default_value: $default, min_value: Some($min), max_value: Some($max),
            description: Some($desc),
        }
    };
    // ---- UnlimitedInteger ----
    ($name:expr, $display:expr, unlimited_int, $default:expr, $min:expr, $max:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::UnlimitedInteger,
            default_value: $default, min_value: Some($min), max_value: Some($max),
            description: None,
        }
    };
    ($name:expr, $display:expr, unlimited_int, $default:expr, $min:expr, $max:expr, $desc:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::UnlimitedInteger,
            default_value: $default, min_value: Some($min), max_value: Some($max),
            description: Some($desc),
        }
    };
    // ---- Angle ----
    ($name:expr, $display:expr, angle, $default:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::Angle,
            default_value: $default, min_value: Some(-360.0), max_value: Some(360.0),
            description: None,
        }
    };
    ($name:expr, $display:expr, angle, $default:expr, $desc:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::Angle,
            default_value: $default, min_value: Some(-360.0), max_value: Some(360.0),
            description: Some($desc),
        }
    };
    // ---- Boolean ----
    ($name:expr, $display:expr, bool, $default:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::Boolean,
            default_value: if $default { 1.0 } else { 0.0 },
            min_value: None, max_value: None,
            description: None,
        }
    };
    ($name:expr, $display:expr, bool, $default:expr, $desc:expr) => {
        VariationParamDef {
            name: $name, display_name: $display, param_type: ParamType::Boolean,
            default_value: if $default { 1.0 } else { 0.0 },
            min_value: None, max_value: None,
            description: Some($desc),
        }
    };
    // ---- Enum ----
    // Choices is a `&'static [&'static str]` slice literal.
    // Default value is the index of the initially selected choice.
    // Example: param!("mode", "Mode", enum, 0, &["Wrap", "Clamp", "Zero"])
    ($name:expr, $display:expr, enum, $default:expr, $choices:expr) => {
        VariationParamDef {
            name: $name, display_name: $display,
            param_type: ParamType::Enum { choices: $choices },
            default_value: $default as f32,
            min_value: Some(0.0),
            max_value: Some(($choices.len() as f32) - 1.0),
            description: None,
        }
    };
    ($name:expr, $display:expr, enum, $default:expr, $choices:expr, $desc:expr) => {
        VariationParamDef {
            name: $name, display_name: $display,
            param_type: ParamType::Enum { choices: $choices },
            default_value: $default as f32,
            min_value: Some(0.0),
            max_value: Some(($choices.len() as f32) - 1.0),
            description: Some($desc),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::param;

    // Smoke test: the enum arm must produce a const-evaluable value
    // so it can live in a `pub static VariationDef`. If
    // ParamType::Enum.choices ever drifts back to a non-const-compatible
    // type (Vec<String>, etc.), this declaration fails to compile.
    static _ENUM_ARM_COMPILES_IN_STATIC: VariationParamDef = param!(
        "mode", "Mode", enum, 1, &["Wrap", "Clamp", "Zero"]
    );

    static _ENUM_ARM_WITH_DESC: VariationParamDef = param!(
        "mode", "Mode", enum, 0, &["Off", "On"],
        "Toggles the thing."
    );

    #[test]
    fn enum_arm_fields_correct() {
        assert_eq!(_ENUM_ARM_COMPILES_IN_STATIC.default_value, 1.0);
        assert_eq!(_ENUM_ARM_COMPILES_IN_STATIC.min_value, Some(0.0));
        assert_eq!(_ENUM_ARM_COMPILES_IN_STATIC.max_value, Some(2.0));
        match _ENUM_ARM_COMPILES_IN_STATIC.param_type {
            ParamType::Enum { choices } => {
                assert_eq!(choices, &["Wrap", "Clamp", "Zero"]);
            }
            _ => panic!("expected Enum param_type"),
        }
        assert_eq!(_ENUM_ARM_WITH_DESC.description, Some("Toggles the thing."));
    }
}
