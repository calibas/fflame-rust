//! Variation definition types
//!
//! This module provides the `VariationDef` struct for defining variations
//! with their metadata and WGSL shader code in a single declaration.

use super::{ParamType, VariationCategory, VariationParameter, VariationPhase};

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
///     wgsl_3d: Some(r#"
/// fn variation_spherical(p: vec3<f32>) -> vec3<f32> {
///     let r2 = dot(p.xy, p.xy) + 1e-6;
///     return vec3(p.xy / r2, p.z);
/// }
/// "#),
/// };
/// ```
pub struct VariationDef {
    /// Unique identifier (lowercase, snake_case)
    pub name: &'static str,

    /// Display name for UI
    pub display_name: &'static str,

    /// Category for organization
    pub category: VariationCategory,

    /// Execution phase (pre/normal/post)
    pub phase: VariationPhase,

    /// Whether this variation requires RNG
    pub needs_rng: bool,

    /// Whether this variation needs `xform_id` so it can read fields from
    /// the per-transform `transforms[xform_id]` storage buffer — affine
    /// matrix (a/b/c/d/e/f), weight, color, opacity, direct_color. When
    /// true, the function signature includes `xform_id: u32` even for
    /// variations without parameters.
    pub needs_transform: bool,

    /// Whether this variation writes to the iteration-local color register
    /// `vc` (Apophysis direct-color "DC" variations). When true, the WGSL
    /// signature gains `vc: ptr<function, f32>` so the body can do
    /// `*vc = palette_position;` based on geometry. The main loop lerps
    /// between standard color evolution and `vc` using the transform's
    /// `direct_color` field.
    pub writes_color: bool,

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

    /// Whether the variation reads the running variation accumulator (cpp's
    /// `FPx/FPy/FPz`). When true, the function signature gains
    /// `accum: vec2<f32>` (or `vec3<f32>` in 3D) after `p`, and the shader
    /// builder passes the current `result` value so the variation sees the
    /// sum of contributions from prior variations in this iteration.
    /// Effective only in normal and post phases. Default false.
    pub needs_accum: bool,

    /// 2D WGSL implementation
    /// Function signature should match one of:
    /// - `fn variation_NAME(p: vec2<f32>) -> vec2<f32>`
    /// - `fn variation_NAME(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32>` (if needs_rng)
    /// - `fn variation_NAME(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32>` (if has params)
    /// - Full signature with both rng and params
    pub wgsl_2d: &'static str,

    /// 3D WGSL implementation (optional - uses 2D with Z pass-through if None)
    /// For 2D variations, this should typically be:
    /// `return vec3(variation_NAME_2d(p.xy), p.z);`
    /// or a full 3D implementation for variations that modify Z
    pub wgsl_3d: Option<&'static str>,
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

    /// Get the 3D WGSL source (or 2D with Z pass-through wrapper)
    pub fn wgsl_source_3d(&self) -> String {
        if let Some(wgsl_3d) = self.wgsl_3d {
            wgsl_3d.to_string()
        } else {
            // Generate a wrapper that passes Z through unchanged
            // This is for pure 2D variations
            self.generate_3d_wrapper()
        }
    }

    /// Generate a 3D wrapper for a 2D-only variation
    fn generate_3d_wrapper(&self) -> String {
        let func_name = self.wgsl_function_name();

        // Build the wrapper signature dynamically to handle the full
        // dimension matrix (needs_rng × has_params × needs_transform ×
        // writes_color × needs_accum). The forwarded call drops the Z
        // component of `p` and `accum` since the 2D body takes vec2.
        let mut wrapper_params: Vec<&str> = vec!["p: vec3<f32>"];
        let mut call_args: Vec<String> = vec!["p.xy".to_string()];
        if self.needs_accum {
            wrapper_params.push("accum: vec3<f32>");
            call_args.push("accum.xy".to_string());
        }
        if !self.parameters.is_empty() || self.needs_transform {
            wrapper_params.push("xform_id: u32");
            wrapper_params.push("variation_id: u32");
            call_args.push("xform_id".to_string());
            call_args.push("variation_id".to_string());
        }
        if self.needs_rng {
            wrapper_params.push("rng: ptr<function, RngState>");
            call_args.push("rng".to_string());
        }
        if self.writes_color {
            wrapper_params.push("vc: ptr<function, f32>");
            call_args.push("vc".to_string());
        }

        format!(
            r#"
fn {func_name}({params}) -> vec3<f32> {{
    let result_2d = {func_name}_2d({call_args});
    return vec3(result_2d, p.z);
}}
"#,
            func_name = func_name,
            params = wrapper_params.join(", "),
            call_args = call_args.join(", ")
        )
    }
}

/// Macro to simplify variation parameter definition
#[macro_export]
macro_rules! param {
    // Float with full range
    ($name:expr, $display:expr, float, $default:expr, $min:expr, $max:expr) => {
        VariationParamDef {
            name: $name,
            display_name: $display,
            param_type: ParamType::Float,
            default_value: $default,
            min_value: Some($min),
            max_value: Some($max),
            description: None,
        }
    };
    // UnlimitedFloat
    ($name:expr, $display:expr, unlimited_float, $default:expr, $min:expr, $max:expr) => {
        VariationParamDef {
            name: $name,
            display_name: $display,
            param_type: ParamType::UnlimitedFloat,
            default_value: $default,
            min_value: Some($min),
            max_value: Some($max),
            description: None,
        }
    };
    // Integer
    ($name:expr, $display:expr, int, $default:expr, $min:expr, $max:expr) => {
        VariationParamDef {
            name: $name,
            display_name: $display,
            param_type: ParamType::Integer,
            default_value: $default,
            min_value: Some($min),
            max_value: Some($max),
            description: None,
        }
    };
    // UnlimitedInteger
    ($name:expr, $display:expr, unlimited_int, $default:expr, $min:expr, $max:expr) => {
        VariationParamDef {
            name: $name,
            display_name: $display,
            param_type: ParamType::UnlimitedInteger,
            default_value: $default,
            min_value: Some($min),
            max_value: Some($max),
            description: None,
        }
    };
    // Angle
    ($name:expr, $display:expr, angle, $default:expr) => {
        VariationParamDef {
            name: $name,
            display_name: $display,
            param_type: ParamType::Angle,
            default_value: $default,
            min_value: Some(-360.0),
            max_value: Some(360.0),
            description: None,
        }
    };
    // Boolean
    ($name:expr, $display:expr, bool, $default:expr) => {
        VariationParamDef {
            name: $name,
            display_name: $display,
            param_type: ParamType::Boolean,
            default_value: if $default { 1.0 } else { 0.0 },
            min_value: None,
            max_value: None,
            description: None,
        }
    };
}
