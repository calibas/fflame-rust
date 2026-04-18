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

/// Global variation registry singleton
/// This ensures the registry is initialized only once and shared across all code paths
pub fn global_registry() -> &'static VariationRegistry {
    use once_cell::sync::Lazy;
    static REGISTRY: Lazy<VariationRegistry> = Lazy::new(|| VariationRegistry::new());
    &REGISTRY
}
