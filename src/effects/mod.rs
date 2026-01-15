//! Effects System
//!
//! A flexible shader effects pipeline with two effect chains:
//! - Density Effects: Run before tonemap, have access to density (alpha channel)
//! - Color Effects: Run after tonemap, operate on final RGB colors
//!
//! Effects are registered by string name and stored in FractalConfig as dynamic lists.
//! Empty lists = zero cost (no render passes, no texture allocations).

use std::collections::HashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EffectParameter {
    /// Parameter name (e.g., "intensity", "radius")
    pub name: String,

    /// Display name for UI (e.g., "Intensity", "Radius")
    pub display_name: String,

    /// Parameter type (reuse from variations)
    pub param_type: ParamType,

    /// Default value
    pub default_value: f32,

    /// Minimum value (None = no limit)
    pub min_value: Option<f32>,

    /// Maximum value (None = no limit)
    pub max_value: Option<f32>,
}

/// Metadata for a registered effect
#[derive(Clone, Debug)]
pub struct EffectInfo {
    /// Unique name (e.g., "vignette", "density_blur")
    pub name: String,

    /// Display name for UI (e.g., "Vignette", "Density Blur")
    pub display_name: String,

    /// Category determines pipeline position
    pub category: EffectCategory,

    /// Path to shader file relative to shaders/ directory
    pub shader_path: String,

    /// Parameters for this effect
    pub parameters: Vec<EffectParameter>,
}

impl EffectInfo {
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
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EffectInstance {
    /// Effect type name (must match registered effect)
    pub effect_type: String,

    /// Whether this effect is currently active
    pub enabled: bool,

    /// Parameter values (name → value)
    /// Missing params use defaults from registry
    #[serde(default)]
    pub params: HashMap<String, f32>,
}

impl EffectInstance {
    /// Create a new effect instance with default parameters
    pub fn new(effect_type: &str) -> Self {
        let params = if let Some(info) = global_effect_registry().get(effect_type) {
            info.default_params()
        } else {
            HashMap::new()
        };

        Self {
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
        } else if let Some(info) = global_effect_registry().get(&self.effect_type) {
            info.get_param_default(param_name).unwrap_or(0.0)
        } else {
            0.0
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
static EFFECT_REGISTRY: Lazy<EffectRegistry> = Lazy::new(|| {
    let mut registry = EffectRegistry::new();
    register_builtin_effects(&mut registry);
    registry
});

/// Get the global effect registry
pub fn global_effect_registry() -> &'static EffectRegistry {
    &EFFECT_REGISTRY
}

/// Register all built-in effects
fn register_builtin_effects(registry: &mut EffectRegistry) {
    // === Color Effects (after tonemap) ===

    // Vignette - darken edges
    registry.register(EffectInfo {
        name: "vignette".to_string(),
        display_name: "Vignette".to_string(),
        category: EffectCategory::Color,
        shader_path: "effects/color/vignette.wgsl".to_string(),
        parameters: vec![
            EffectParameter {
                name: "intensity".to_string(),
                display_name: "Intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            EffectParameter {
                name: "radius".to_string(),
                display_name: "Radius".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            EffectParameter {
                name: "softness".to_string(),
                display_name: "Softness".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
        ],
    });

    // Film Grain - animated noise overlay
    registry.register(EffectInfo {
        name: "film_grain".to_string(),
        display_name: "Film Grain".to_string(),
        category: EffectCategory::Color,
        shader_path: "effects/color/film_grain.wgsl".to_string(),
        parameters: vec![
            EffectParameter {
                name: "intensity".to_string(),
                display_name: "Intensity".to_string(),
                param_type: ParamType::Float,
                default_value: 0.1,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            EffectParameter {
                name: "size".to_string(),
                display_name: "Grain Size".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.5),
                max_value: Some(4.0),
            },
        ],
    });

    // Chromatic Aberration - RGB channel offset
    registry.register(EffectInfo {
        name: "chromatic_aberration".to_string(),
        display_name: "Chromatic Aberration".to_string(),
        category: EffectCategory::Color,
        shader_path: "effects/color/chromatic_aberration.wgsl".to_string(),
        parameters: vec![
            EffectParameter {
                name: "amount".to_string(),
                display_name: "Amount".to_string(),
                param_type: ParamType::Float,
                default_value: 2.0,
                min_value: Some(0.0),
                max_value: Some(20.0),
            },
            EffectParameter {
                name: "radial".to_string(),
                display_name: "Radial".to_string(),
                param_type: ParamType::Boolean,
                default_value: 1.0, // true
                min_value: None,
                max_value: None,
            },
        ],
    });

    // Hue Cycle - psychedelic hue rotation
    registry.register(EffectInfo {
        name: "hue_cycle".to_string(),
        display_name: "Hue Cycle".to_string(),
        category: EffectCategory::Color,
        shader_path: "effects/color/hue_cycle.wgsl".to_string(),
        parameters: vec![
            EffectParameter {
                name: "offset".to_string(),
                display_name: "Offset".to_string(),
                param_type: ParamType::Angle,
                default_value: 0.0,
                min_value: Some(0.0),
                max_value: Some(360.0),
            },
            EffectParameter {
                name: "speed".to_string(),
                display_name: "Speed (deg/s)".to_string(),
                param_type: ParamType::Float,
                default_value: 30.0,
                min_value: Some(-360.0),
                max_value: Some(360.0),
            },
        ],
    });

    // === Density Effects (before tonemap) ===

    // Density Blur - blur weighted by density
    registry.register(EffectInfo {
        name: "density_blur".to_string(),
        display_name: "Density Blur".to_string(),
        category: EffectCategory::Density,
        shader_path: "effects/density/density_blur.wgsl".to_string(),
        parameters: vec![
            EffectParameter {
                name: "radius".to_string(),
                display_name: "Radius".to_string(),
                param_type: ParamType::Float,
                default_value: 3.0,
                min_value: Some(0.0),
                max_value: Some(10.0),
            },
            EffectParameter {
                name: "threshold".to_string(),
                display_name: "Density Threshold".to_string(),
                param_type: ParamType::Float,
                default_value: 0.25,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
            EffectParameter {
                name: "falloff".to_string(),
                display_name: "Falloff".to_string(),
                param_type: ParamType::Float,
                default_value: 0.5,
                min_value: Some(0.0),
                max_value: Some(1.0),
            },
        ],
    });

    // Sharpen - detail enhancement
    registry.register(EffectInfo {
        name: "sharpen".to_string(),
        display_name: "Sharpen".to_string(),
        category: EffectCategory::Density,
        shader_path: "effects/density/sharpen.wgsl".to_string(),
        parameters: vec![
            EffectParameter {
                name: "amount".to_string(),
                display_name: "Amount".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(2.0),
            },
            EffectParameter {
                name: "radius".to_string(),
                display_name: "Radius".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.5),
                max_value: Some(5.0),
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
        assert!(registry.len() >= 6);
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
