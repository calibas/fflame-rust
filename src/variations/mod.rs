use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Parameter type for variation parameters
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ParamType {
    /// Continuous floating-point value
    Float,
    /// Integer value (stored as f32, cast for UI)
    Integer,
    /// Angle in degrees (0-360, or custom range)
    Angle,
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

/// Variation metadata and registration
#[derive(Clone, Debug)]
pub struct VariationInfo {
    /// Unique name (e.g., "linear", "sinusoidal", "curl_3d")
    pub name: String,

    /// Display name for UI
    pub display_name: String,

    /// Category for organization
    pub category: VariationCategory,

    /// WGSL function name (e.g., "variation_linear")
    pub wgsl_function: String,

    /// Whether this variation needs RNG
    pub needs_rng: bool,

    /// Whether this is a core (built-in) or plugin variation
    pub is_core: bool,

    /// Optional: WGSL source code (for plugins loaded at runtime)
    pub wgsl_source: Option<String>,

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
    pub fn new() -> Self {
        let mut registry = Self {
            variations: HashMap::new(),
            ordered_names: Vec::new(),
        };

        // Register core 2D variations (Basic)
        registry.register_core("linear", "Linear", VariationCategory::Basic2D, false);
        registry.register_core("sinusoidal", "Sinusoidal", VariationCategory::Basic2D, false);
        registry.register_core("spherical", "Spherical", VariationCategory::Basic2D, false);
        registry.register_core("swirl", "Swirl", VariationCategory::Basic2D, false);
        registry.register_core("horseshoe", "Horseshoe", VariationCategory::Basic2D, false);

        // Register core 2D variations (Advanced)
        registry.register_core("polar", "Polar", VariationCategory::Advanced2D, false);
        registry.register_core("handkerchief", "Handkerchief", VariationCategory::Advanced2D, false);
        registry.register_core("heart", "Heart", VariationCategory::Advanced2D, false);
        registry.register_core("disc", "Disc", VariationCategory::Advanced2D, false);
        registry.register_core("spiral", "Spiral", VariationCategory::Advanced2D, false);
        registry.register_core("hyperbolic", "Hyperbolic", VariationCategory::Advanced2D, false);
        registry.register_core("diamond", "Diamond", VariationCategory::Advanced2D, false);
        registry.register_core("ex", "Ex", VariationCategory::Advanced2D, false);
        registry.register_core("julia", "Julia", VariationCategory::Advanced2D, true); // Needs RNG
        registry.register_core("julian", "JuliaN", VariationCategory::Advanced2D, true); // Needs RNG
        registry.register_core("bent", "Bent", VariationCategory::Advanced2D, false);
        registry.register_core("waves", "Waves", VariationCategory::Advanced2D, false);

        // Register 3D depth variations
        registry.register_core("zcone", "Z-Cone", VariationCategory::Depth3D, false);
        registry.register_core("flatten", "Flatten", VariationCategory::Depth3D, false);

        // Register full 3D variations
        registry.register_core("hemisphere", "Hemisphere", VariationCategory::Full3D, false);

        // Register 3D rotation variations
        registry.register_core("pre_rotate_x", "Pre-Rotate X", VariationCategory::Rotation3D, false);
        registry.register_core("pre_rotate_y", "Pre-Rotate Y", VariationCategory::Rotation3D, false);
        registry.register_core("post_rotate_x", "Post-Rotate X", VariationCategory::Rotation3D, false);
        registry.register_core("post_rotate_y", "Post-Rotate Y", VariationCategory::Rotation3D, false);

        // Register 3D depth variations (continued)
        registry.register_core("zscale", "Z-Scale", VariationCategory::Depth3D, false);

        // Add parameters to variations that need them
        registry.add_parameters("julian", vec![
            VariationParameter {
                name: "power".to_string(),
                display_name: "Power".to_string(),
                param_type: ParamType::Integer,
                default_value: 2.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "dist".to_string(),
                display_name: "Distance".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.1),
                max_value: Some(5.0),
            },
        ]);

        registry
    }

    /// Register a core (built-in) variation
    fn register_core(&mut self, name: &str, display_name: &str, category: VariationCategory, needs_rng: bool) {
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
            wgsl_function,
            needs_rng,
            is_core: true,
            wgsl_source: None,
            parameters: Vec::new(),  // No parameters by default
        };

        self.variations.insert(name.to_string(), info);
        self.ordered_names.push(name.to_string());
    }

    /// Register a plugin variation
    pub fn register_plugin(&mut self, name: String, display_name: String, category: VariationCategory, wgsl_source: String, needs_rng: bool) {
        let wgsl_function = format!("variation_{}", name);

        let info = VariationInfo {
            name: name.clone(),
            display_name,
            category,
            wgsl_function,
            needs_rng,
            is_core: false,
            wgsl_source: Some(wgsl_source),
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

    /// Get variations by category
    pub fn by_category(&self, category: VariationCategory) -> Vec<&VariationInfo> {
        self.variations
            .values()
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
