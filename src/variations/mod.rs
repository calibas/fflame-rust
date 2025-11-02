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

        log::info!("=== VARIATION REGISTRY INITIALIZATION ===");

        // Register core 2D variations (Basic) - Indices 0-4
        registry.register_core("linear", "Linear", VariationCategory::Basic2D, VariationPhase::Normal, false);          // 0
        registry.register_core("sinusoidal", "Sinusoidal", VariationCategory::Basic2D, VariationPhase::Normal, false);  // 1
        registry.register_core("spherical", "Spherical", VariationCategory::Basic2D, VariationPhase::Normal, false);    // 2
        registry.register_core("swirl", "Swirl", VariationCategory::Basic2D, VariationPhase::Normal, false);            // 3
        registry.register_core("horseshoe", "Horseshoe", VariationCategory::Basic2D, VariationPhase::Normal, false);    // 4

        // Register core 2D variations (Advanced) - Indices 5-15
        registry.register_core("polar", "Polar", VariationCategory::Advanced2D, VariationPhase::Normal, false);          // 5
        registry.register_core("handkerchief", "Handkerchief", VariationCategory::Advanced2D, VariationPhase::Normal, false); // 6
        registry.register_core("heart", "Heart", VariationCategory::Advanced2D, VariationPhase::Normal, false);          // 7
        registry.register_core("disc", "Disc", VariationCategory::Advanced2D, VariationPhase::Normal, false);            // 8
        registry.register_core("spiral", "Spiral", VariationCategory::Advanced2D, VariationPhase::Normal, false);        // 9
        registry.register_core("hyperbolic", "Hyperbolic", VariationCategory::Advanced2D, VariationPhase::Normal, false); // 10
        registry.register_core("diamond", "Diamond", VariationCategory::Advanced2D, VariationPhase::Normal, false);      // 11
        registry.register_core("ex", "Ex", VariationCategory::Advanced2D, VariationPhase::Normal, false);                // 12
        registry.register_core("julia", "Julia", VariationCategory::Advanced2D, VariationPhase::Normal, true);           // 13 (Needs RNG)
        registry.register_core("bent", "Bent", VariationCategory::Advanced2D, VariationPhase::Normal, false);            // 14
        registry.register_core("waves", "Waves", VariationCategory::Advanced2D, VariationPhase::Normal, false);          // 15

        // Register 3D depth variations - Indices 16-17, 23
        registry.register_core("zcone", "Z-Cone", VariationCategory::Depth3D, VariationPhase::Normal, false);            // 16
        registry.register_core("flatten", "Flatten", VariationCategory::Depth3D, VariationPhase::Post, false);           // 17 - POST! (Apophysis special case)

        // Register full 3D variations - Index 18
        registry.register_core("hemisphere", "Hemisphere", VariationCategory::Full3D, VariationPhase::Normal, false);    // 18

        // Register 3D rotation variations - Indices 19-22
        registry.register_core("pre_rotate_x", "Pre-Rotate X", VariationCategory::Rotation3D, VariationPhase::Pre, false);   // 19 - PRE!
        registry.register_core("pre_rotate_y", "Pre-Rotate Y", VariationCategory::Rotation3D, VariationPhase::Pre, false);   // 20 - PRE!
        registry.register_core("post_rotate_x", "Post-Rotate X", VariationCategory::Rotation3D, VariationPhase::Post, false); // 21 - POST!
        registry.register_core("post_rotate_y", "Post-Rotate Y", VariationCategory::Rotation3D, VariationPhase::Post, false); // 22 - POST!

        // Register 3D depth variations (continued) - Index 23
        registry.register_core("zscale", "Z-Scale", VariationCategory::Depth3D, VariationPhase::Normal, false);         // 23

        // NEW VARIATIONS (added after original 24) - Indices 24-25
        // IMPORTANT: Always add new variations at the end to preserve index compatibility
        registry.register_core("julian", "JuliaN", VariationCategory::Advanced2D, VariationPhase::Normal, true);        // 24 (Needs RNG)
        registry.register_core("blob", "Blob", VariationCategory::Advanced2D, VariationPhase::Normal, false);           // 25

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

        registry.add_parameters("blob", vec![
            VariationParameter {
                name: "high".to_string(),
                display_name: "High".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(3.0),
            },
            VariationParameter {
                name: "low".to_string(),
                display_name: "Low".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(3.0),
            },
            VariationParameter {
                name: "waves".to_string(),
                display_name: "Waves".to_string(),
                param_type: ParamType::Float,
                default_value: 6.0,
                min_value: Some(1.0),
                max_value: Some(20.0),
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

    /// Register a core (built-in) variation
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
