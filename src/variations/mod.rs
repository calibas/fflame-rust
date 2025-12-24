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

    /// Whether this is a core (built-in) or plugin variation
    pub is_core: bool,

    /// Optional: WGSL source code for 2D (for plugins loaded at runtime)
    pub wgsl_source: Option<String>,

    /// Optional: WGSL source code for 3D
    pub wgsl_source_3d: Option<String>,

    /// Parameters for this variation
    pub parameters: Vec<VariationParameter>,
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

    /// Create from a static VariationDef
    pub fn from_def(def: &VariationDef) -> Self {
        Self {
            name: def.name.to_string(),
            display_name: def.display_name.to_string(),
            category: def.category.clone(),
            phase: def.phase.clone(),
            wgsl_function: def.wgsl_function_name(),
            needs_rng: def.needs_rng,
            is_core: true, // All VariationDef are core variations
            wgsl_source: Some(def.wgsl_2d.to_string()),
            wgsl_source_3d: def.wgsl_3d.map(|s| s.to_string()),
            parameters: def.parameters_to_runtime(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

/// Registry of all available variations
#[derive(Clone, Debug)]
pub struct VariationRegistry {
    /// Map of variation name -> info
    variations: HashMap<String, VariationInfo>,

    /// Ordered list of variation names (for consistent ID assignment)
    ordered_names: Vec<String>,
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
        };

        log::info!("=== VARIATION REGISTRY INITIALIZATION ===");

        // Load all variations from static definitions
        // The order in ALL_VARIATIONS determines the variation indices
        for def in defs::ALL_VARIATIONS.iter() {
            registry.register_from_def(def);
        }

        // Legacy variations not yet converted to VariationDef
        // These will be migrated incrementally to static definitions
        // TODO: Convert remaining variations to VariationDef format with embedded WGSL

        // 3D blur variations
        registry.register_core("zblur", "Z-Blur", VariationCategory::Depth3D, VariationPhase::Normal, true);
        registry.register_core("blur3d", "Blur 3D", VariationCategory::Full3D, VariationPhase::Normal, true);

        // Pre-phase variations
        registry.register_core("pre_blur", "Pre-Blur", VariationCategory::Advanced2D, VariationPhase::Pre, true);
        registry.register_core("pre_zscale", "Pre-ZScale", VariationCategory::Depth3D, VariationPhase::Pre, false);
        registry.register_core("pre_ztranslate", "Pre-ZTranslate", VariationCategory::Depth3D, VariationPhase::Pre, false);
        registry.register_core("pre_spherical", "Pre-Spherical", VariationCategory::Advanced2D, VariationPhase::Pre, false);
        registry.register_core("pre_sinusoidal", "Pre-Sinusoidal", VariationCategory::Advanced2D, VariationPhase::Pre, false);
        registry.register_core("pre_disc", "Pre-Disc", VariationCategory::Advanced2D, VariationPhase::Pre, false);
        registry.register_core("pre_bwraps", "Pre Bwraps", VariationCategory::Advanced2D, VariationPhase::Pre, false);
        registry.register_core("pre_crop", "Pre Crop", VariationCategory::Advanced2D, VariationPhase::Pre, true);
        registry.register_core("pre_falloff2", "Pre Falloff2", VariationCategory::Advanced2D, VariationPhase::Pre, true);

        // Normal-phase variations
        registry.register_core("ztranslate", "ZTranslate", VariationCategory::Depth3D, VariationPhase::Normal, false);
        registry.register_core("julia3d", "Julia3D", VariationCategory::Full3D, VariationPhase::Normal, true);
        registry.register_core("falloff2", "Falloff2", VariationCategory::Advanced2D, VariationPhase::Normal, true);
        registry.register_core("wedge", "Wedge", VariationCategory::Advanced2D, VariationPhase::Normal, false);
        registry.register_core("epispiral", "Epispiral", VariationCategory::Advanced2D, VariationPhase::Normal, true);
        registry.register_core("bwraps", "BWraps", VariationCategory::Advanced2D, VariationPhase::Normal, false);
        registry.register_core("juliascope", "JuliaScope", VariationCategory::Advanced2D, VariationPhase::Normal, true);
        registry.register_core("julia3dz", "Julia3Dz", VariationCategory::Full3D, VariationPhase::Normal, true);
        registry.register_core("curl3d", "Curl3D", VariationCategory::Full3D, VariationPhase::Normal, false);
        registry.register_core("radial_blur", "Radial Blur", VariationCategory::Advanced2D, VariationPhase::Normal, true);
        registry.register_core("blur_circle", "Blur Circle", VariationCategory::Advanced2D, VariationPhase::Normal, true);
        registry.register_core("blur_zoom", "Blur Zoom", VariationCategory::Advanced2D, VariationPhase::Normal, true);
        registry.register_core("blur_pixelize", "Blur Pixelize", VariationCategory::Advanced2D, VariationPhase::Normal, true);
        registry.register_core("separation", "Separation", VariationCategory::Advanced2D, VariationPhase::Normal, false);
        registry.register_core("mobius", "Mobius", VariationCategory::Advanced2D, VariationPhase::Normal, false);
        registry.register_core("crop", "Crop", VariationCategory::Advanced2D, VariationPhase::Normal, true);

        // Post-phase variations
        registry.register_core("post_bwraps", "Post Bwraps", VariationCategory::Advanced2D, VariationPhase::Post, false);
        registry.register_core("post_crop", "Post Crop", VariationCategory::Advanced2D, VariationPhase::Post, true);
        registry.register_core("post_falloff2", "Post Falloff2", VariationCategory::Advanced2D, VariationPhase::Post, true);
        registry.register_core("post_curl", "Post Curl", VariationCategory::Advanced2D, VariationPhase::Post, false);
        registry.register_core("post_curl3d", "Post Curl 3D", VariationCategory::Full3D, VariationPhase::Post, false);

        // Add parameters to legacy variations that need them
        // (Variations from ALL_VARIATIONS already have parameters defined in their VariationDef)

        registry.add_parameters("julia3d", vec![
            VariationParameter {
                name: "power".to_string(),
                display_name: "Power".to_string(),
                param_type: ParamType::UnlimitedInteger,
                default_value: 2.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
        ]);

        registry.add_parameters("falloff2", vec![
            VariationParameter {
                name: "scatter".to_string(),
                display_name: "Scatter".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(0.000001),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "mindist".to_string(),
                display_name: "Min Distance".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "mul_x".to_string(),
                display_name: "Multiply X".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "mul_y".to_string(),
                display_name: "Multiply Y".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "mul_z".to_string(),
                display_name: "Multiply Z".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "mul_c".to_string(),
                display_name: "Multiply Color".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "x0".to_string(),
                display_name: "X Center".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "y0".to_string(),
                display_name: "Y Center".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "z0".to_string(),
                display_name: "Z Center".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "invert".to_string(),
                display_name: "Invert".to_string(),
                param_type: ParamType::Boolean,
                default_value: 0.0,
                min_value: None,
                max_value: None,
            },
            VariationParameter {
                name: "type".to_string(),
                display_name: "Blur Type".to_string(),
                param_type: ParamType::enum_choices(&["Linear", "Radial", "Gaussian"]),
                default_value: 0.0,
                min_value: None,
                max_value: None,
            },
        ]);

        registry.add_parameters("wedge", vec![
            VariationParameter {
                name: "angle".to_string(),
                display_name: "Angle".to_string(),
                param_type: ParamType::Angle,
                default_value: 90.0, // π/2 radians = 90 degrees
                min_value: Some(-360.0),
                max_value: Some(360.0),
            },
            VariationParameter {
                name: "hole".to_string(),
                display_name: "Hole".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "count".to_string(),
                display_name: "Count".to_string(),
                param_type: ParamType::Integer,
                default_value: 2.0,
                min_value: Some(1.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "swirl".to_string(),
                display_name: "Swirl".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-30.0),
                max_value: Some(30.0),
            },
        ]);

        registry.add_parameters("epispiral", vec![
            VariationParameter {
                name: "n".to_string(),
                display_name: "N".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 6.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "thickness".to_string(),
                display_name: "Thickness".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-2.0),
                max_value: Some(2.0),
            },
            VariationParameter {
                name: "holes".to_string(),
                display_name: "Holes".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
        ]);

        registry.add_parameters("bwraps", vec![
            VariationParameter {
                name: "cellsize".to_string(),
                display_name: "Cell Size".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "space".to_string(),
                display_name: "Space".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-1.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "gain".to_string(),
                display_name: "Gain".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "inner_twist".to_string(),
                display_name: "Inner Twist".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "outer_twist".to_string(),
                display_name: "Outer Twist".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
        ]);

        registry.add_parameters("juliascope", vec![
            VariationParameter {
                name: "power".to_string(),
                display_name: "Power".to_string(),
                param_type: ParamType::UnlimitedInteger,
                default_value: 2.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "dist".to_string(),
                display_name: "Distance".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
        ]);

        registry.add_parameters("julia3dz", vec![
            VariationParameter {
                name: "power".to_string(),
                display_name: "Power".to_string(),
                param_type: ParamType::UnlimitedInteger,
                default_value: 2.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
        ]);

        registry.add_parameters("curl3d", vec![
            VariationParameter {
                name: "cx".to_string(),
                display_name: "CX".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "cy".to_string(),
                display_name: "CY".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "cz".to_string(),
                display_name: "CZ".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
        ]);

        registry.add_parameters("radial_blur", vec![
            VariationParameter {
                name: "angle".to_string(),
                display_name: "Angle".to_string(),
                param_type: ParamType::Angle,
                default_value: 0.0,
                min_value: Some(-360.0),
                max_value: Some(360.0),
            },
        ]);

        registry.add_parameters("blur_zoom", vec![
            VariationParameter {
                name: "length".to_string(),
                display_name: "Length".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "x".to_string(),
                display_name: "X".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "y".to_string(),
                display_name: "Y".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
        ]);

        registry.add_parameters("blur_pixelize", vec![
            VariationParameter {
                name: "size".to_string(),
                display_name: "Size".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.1,
                min_value: Some(0.0000001),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "scale".to_string(),
                display_name: "Scale".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
        ]);

        registry.add_parameters("separation", vec![
            VariationParameter {
                name: "x".to_string(),
                display_name: "X".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "y".to_string(),
                display_name: "Y".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "xinside".to_string(),
                display_name: "X Inside".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "yinside".to_string(),
                display_name: "Y Inside".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
        ]);

        registry.add_parameters("mobius", vec![
            VariationParameter {
                name: "re_a".to_string(),
                display_name: "Re A".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "im_a".to_string(),
                display_name: "Im A".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "re_b".to_string(),
                display_name: "Re B".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "im_b".to_string(),
                display_name: "Im B".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "re_c".to_string(),
                display_name: "Re C".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "im_c".to_string(),
                display_name: "Im C".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "re_d".to_string(),
                display_name: "Re D".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
            VariationParameter {
                name: "im_d".to_string(),
                display_name: "Im D".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-20.0),
                max_value: Some(20.0),
            },
        ]);

        registry.add_parameters("crop", vec![
            VariationParameter {
                name: "left".to_string(),
                display_name: "Left".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: -1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "top".to_string(),
                display_name: "Top".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: -1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "right".to_string(),
                display_name: "Right".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "bottom".to_string(),
                display_name: "Bottom".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "scatter_area".to_string(),
                display_name: "Scatter Area".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(-1.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "zero".to_string(),
                display_name: "Zero".to_string(),
                param_type: ParamType::Boolean,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
        ]);

        registry.add_parameters("pre_bwraps", vec![
            VariationParameter {
                name: "cellsize".to_string(),
                display_name: "Cell Size".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "space".to_string(),
                display_name: "Space".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-1.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "gain".to_string(),
                display_name: "Gain".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "inner_twist".to_string(),
                display_name: "Inner Twist".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "outer_twist".to_string(),
                display_name: "Outer Twist".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
        ]);

        registry.add_parameters("post_bwraps", vec![
            VariationParameter {
                name: "cellsize".to_string(),
                display_name: "Cell Size".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "space".to_string(),
                display_name: "Space".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-1.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "gain".to_string(),
                display_name: "Gain".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "inner_twist".to_string(),
                display_name: "Inner Twist".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "outer_twist".to_string(),
                display_name: "Outer Twist".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
        ]);

        registry.add_parameters("pre_crop", vec![
            VariationParameter {
                name: "left".to_string(),
                display_name: "Left".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: -1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "top".to_string(),
                display_name: "Top".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: -1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "right".to_string(),
                display_name: "Right".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "bottom".to_string(),
                display_name: "Bottom".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "scatter_area".to_string(),
                display_name: "Scatter Area".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(-1.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "zero".to_string(),
                display_name: "Zero".to_string(),
                param_type: ParamType::Boolean,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
        ]);

        registry.add_parameters("post_crop", vec![
            VariationParameter {
                name: "left".to_string(),
                display_name: "Left".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: -1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "top".to_string(),
                display_name: "Top".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: -1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "right".to_string(),
                display_name: "Right".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "bottom".to_string(),
                display_name: "Bottom".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "scatter_area".to_string(),
                display_name: "Scatter Area".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(-1.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "zero".to_string(),
                display_name: "Zero".to_string(),
                param_type: ParamType::Boolean,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
        ]);

        registry.add_parameters("pre_falloff2", vec![
            VariationParameter {
                name: "scatter".to_string(),
                display_name: "Scatter".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(0.000001),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "mindist".to_string(),
                display_name: "Min Distance".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "mul_x".to_string(),
                display_name: "Multiply X".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "mul_y".to_string(),
                display_name: "Multiply Y".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "mul_z".to_string(),
                display_name: "Multiply Z".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "mul_c".to_string(),
                display_name: "Multiply Color".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "x0".to_string(),
                display_name: "X Center".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "y0".to_string(),
                display_name: "Y Center".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "z0".to_string(),
                display_name: "Z Center".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "invert".to_string(),
                display_name: "Invert".to_string(),
                param_type: ParamType::Boolean,
                default_value: 0.0,
                min_value: None,
                max_value: None,
            },
            VariationParameter {
                name: "type".to_string(),
                display_name: "Blur Type".to_string(),
                param_type: ParamType::enum_choices(&["Linear", "Radial", "Gaussian"]),
                default_value: 0.0,
                min_value: None,
                max_value: None,
            },
        ]);

        registry.add_parameters("post_falloff2", vec![
            VariationParameter {
                name: "scatter".to_string(),
                display_name: "Scatter".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 1.0,
                min_value: Some(0.000001),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "mindist".to_string(),
                display_name: "Min Distance".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "mul_x".to_string(),
                display_name: "Multiply X".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "mul_y".to_string(),
                display_name: "Multiply Y".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "mul_z".to_string(),
                display_name: "Multiply Z".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "mul_c".to_string(),
                display_name: "Multiply Color".to_string(),
                param_type: ParamType::Float,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            VariationParameter {
                name: "x0".to_string(),
                display_name: "X Center".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "y0".to_string(),
                display_name: "Y Center".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "z0".to_string(),
                display_name: "Z Center".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "invert".to_string(),
                display_name: "Invert".to_string(),
                param_type: ParamType::Boolean,
                default_value: 0.0,
                min_value: None,
                max_value: None,
            },
            VariationParameter {
                name: "type".to_string(),
                display_name: "Blur Type".to_string(),
                param_type: ParamType::enum_choices(&["Linear", "Radial", "Gaussian"]),
                default_value: 0.0,
                min_value: None,
                max_value: None,
            },
        ]);

        registry.add_parameters("post_curl", vec![
            VariationParameter {
                name: "c1".to_string(),
                display_name: "C1".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "c2".to_string(),
                display_name: "C2".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
        ]);

        registry.add_parameters("post_curl3d", vec![
            VariationParameter {
                name: "cx".to_string(),
                display_name: "CX".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "cy".to_string(),
                display_name: "CY".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
            VariationParameter {
                name: "cz".to_string(),
                display_name: "CZ".to_string(),
                param_type: ParamType::UnlimitedFloat,
                default_value: 0.0,
                min_value: Some(-5.0),
                max_value: Some(5.0),
            },
        ]);

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

    /// Register a core (built-in) variation (legacy method for backward compatibility)
    fn register_core(&mut self, name: &str, display_name: &str, category: VariationCategory, phase: VariationPhase, needs_rng: bool) {
        let wgsl_function = if name == "julia" {
            // Julia is special - doesn't have "variation_" prefix in shader
            name.to_string()
        } else {
            format!("variation_{}", name)
        };

        let info = VariationInfo {
            name: name.to_string(),
            display_name: display_name.to_string(),
            category,
            phase,
            wgsl_function,
            needs_rng,
            is_core: true,
            wgsl_source: None,
            wgsl_source_3d: None,
            parameters: Vec::new(),  // No parameters by default
        };

        self.variations.insert(name.to_string(), info);
        self.ordered_names.push(name.to_string());
    }

    /// Register a plugin variation
    pub fn register_plugin(&mut self, name: String, display_name: String, category: VariationCategory, phase: VariationPhase, wgsl_source: String, needs_rng: bool) {
        let wgsl_function = format!("variation_{}", name);

        let info = VariationInfo {
            name: name.clone(),
            display_name,
            category,
            phase,
            wgsl_function,
            needs_rng,
            is_core: false,
            wgsl_source: Some(wgsl_source),
            wgsl_source_3d: None, // Plugin 3D source can be added separately
            parameters: Vec::new(),  // Parameters can be added later
        };

        self.variations.insert(name.clone(), info);
        if !self.ordered_names.contains(&name) {
            self.ordered_names.push(name);
        }
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

    /// Add parameters to an existing variation (helper for defining parameters after registration)
    fn add_parameters(&mut self, name: &str, parameters: Vec<VariationParameter>) {
        if let Some(info) = self.variations.get_mut(name) {
            info.parameters = parameters;
        }
    }
}

impl Default for VariationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global variation registry singleton
/// This ensures the registry is initialized only once and shared across all code paths
pub fn global_registry() -> &'static VariationRegistry {
    use once_cell::sync::Lazy;
    static REGISTRY: Lazy<VariationRegistry> = Lazy::new(|| VariationRegistry::new());
    &REGISTRY
}
