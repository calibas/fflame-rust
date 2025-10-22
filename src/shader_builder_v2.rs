use std::collections::HashMap;
use crate::variations::{VariationRegistry, VariationInfo};

/// Builds WGSL shaders dynamically using named variations
pub struct ShaderBuilder {
    registry: VariationRegistry,
}

impl ShaderBuilder {
    pub fn new(registry: VariationRegistry) -> Self {
        Self { registry }
    }

    /// Build 2D trajectory shader with active variations
    pub fn build_trajectory_2d(&self, active_variations: &HashMap<String, f32>) -> String {
        let active_names: Vec<String> = active_variations.keys().cloned().collect();
        let id_map = self.registry.assign_ids(&active_names);

        let mut shader = String::new();

        // 1. Header
        shader.push_str(include_str!("../shaders/core/header.wgsl"));
        shader.push('\n');

        // 2. RNG
        shader.push_str(include_str!("../shaders/core/rng.wgsl"));
        shader.push('\n');

        // 3. Affine
        shader.push_str(include_str!("../shaders/core/affine.wgsl"));
        shader.push('\n');

        // 4. Core variations (2D)
        shader.push_str(include_str!("../shaders/core/variations_2d.wgsl"));
        shader.push('\n');

        // 5. Plugin variations
        for name in &active_names {
            if let Some(info) = self.registry.get(name) {
                if !info.is_core {
                    if let Some(source) = &info.wgsl_source {
                        shader.push_str(source);
                        shader.push('\n');
                    }
                }
            }
        }

        // 6. Generate apply_variations with name->ID mapping
        shader.push_str(&self.build_apply_variations_2d(&id_map));
        shader.push('\n');

        // 7. Utilities
        shader.push_str(include_str!("../shaders/core/utilities.wgsl"));
        shader.push('\n');

        // 8. Main
        shader.push_str(include_str!("../shaders/core/main_2d.wgsl"));

        shader
    }

    /// Build 3D trajectory shader with active variations
    pub fn build_trajectory_3d(&self, active_variations: &HashMap<String, f32>) -> String {
        let active_names: Vec<String> = active_variations.keys().cloned().collect();
        let id_map = self.registry.assign_ids(&active_names);

        let mut shader = String::new();

        // 1. Header
        shader.push_str(include_str!("../shaders/core/header.wgsl"));
        shader.push('\n');

        // 2. RNG
        shader.push_str(include_str!("../shaders/core/rng.wgsl"));
        shader.push('\n');

        // 3. Core variations (3D)
        shader.push_str(include_str!("../shaders/core/variations_3d.wgsl"));
        shader.push('\n');

        // 4. Plugin variations
        for name in &active_names {
            if let Some(info) = self.registry.get(name) {
                if !info.is_core {
                    if let Some(source) = &info.wgsl_source {
                        shader.push_str(source);
                        shader.push('\n');
                    }
                }
            }
        }

        // 5. Generate apply_variations with name->ID mapping
        shader.push_str(&self.build_apply_variations_3d(&id_map));
        shader.push('\n');

        // 6. Utilities
        shader.push_str(include_str!("../shaders/core/utilities.wgsl"));
        shader.push('\n');

        // 7. Main
        shader.push_str(include_str!("../shaders/core/main_3d.wgsl"));

        shader
    }

    /// Build apply_variations function for 2D mode
    fn build_apply_variations_2d(&self, id_map: &HashMap<String, u32>) -> String {
        let mut code = String::from(
            "// Apply all variations with weights\n\
             fn apply_variations(xform: Transform, p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {\n\
             \x20   var result = vec2<f32>(0.0, 0.0);\n\n"
        );

        // Sort by ID for deterministic shader generation
        let mut entries: Vec<_> = id_map.iter().collect();
        entries.sort_by_key(|(_, id)| *id);

        for (name, id) in entries {
            if let Some(info) = self.registry.get(name) {
                let call = if info.needs_rng {
                    format!("{}(p, rng)", info.wgsl_function)
                } else {
                    format!("{}(p)", info.wgsl_function)
                };

                code.push_str(&format!(
                    "    // {}: {}\n\
                     \x20   if (xform.variations[{}] != 0.0) {{\n\
                     \x20       result += xform.variations[{}] * {};\n\
                     \x20   }}\n",
                    id, info.display_name, id, id, call
                ));
            }
        }

        code.push_str("\n    return result;\n}\n");
        code
    }

    /// Build apply_variations function for 3D mode
    fn build_apply_variations_3d(&self, id_map: &HashMap<String, u32>) -> String {
        let mut code = String::from(
            "// Apply all variations with weights (3D)\n\
             fn apply_variations(xform: Transform, p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {\n\
             \x20   var result = vec3<f32>(0.0, 0.0, 0.0);\n\n"
        );

        // Sort by ID
        let mut entries: Vec<_> = id_map.iter().collect();
        entries.sort_by_key(|(_, id)| *id);

        for (name, id) in entries {
            if let Some(info) = self.registry.get(name) {
                // Special handling for Z-only and rotation variations
                match name.as_str() {
                    "zcone" => {
                        code.push_str(&format!(
                            "    // {}: {} (Z-only)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       let r = length(p.xy);\n\
                             \x20       result.z += xform.variations[{}] * r;\n\
                             \x20   }}\n",
                            id, info.display_name, id, id
                        ));
                    }
                    "flatten" => {
                        code.push_str(&format!(
                            "    // {}: {} (Z-only)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result.z *= (1.0 - xform.variations[{}] * 0.5);\n\
                             \x20   }}\n",
                            id, info.display_name, id, id
                        ));
                    }
                    "zscale" => {
                        code.push_str(&format!(
                            "    // {}: {} (Z-only)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result.z *= (1.0 + xform.variations[{}]);\n\
                             \x20   }}\n",
                            id, info.display_name, id, id
                        ));
                    }
                    "pre_rotate_x" | "pre_rotate_y" | "post_rotate_x" | "post_rotate_y" => {
                        let rotate_fn = if name.contains("_x") { "rotate_x" } else { "rotate_y" };
                        code.push_str(&format!(
                            "    // {}: {} (Rotation)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result = {}(result, xform.variations[{}]);\n\
                             \x20   }}\n",
                            id, info.display_name, id, rotate_fn, id
                        ));
                    }
                    _ => {
                        // Standard variation
                        let call = if info.needs_rng {
                            format!("{}(p, rng)", info.wgsl_function)
                        } else {
                            format!("{}(p)", info.wgsl_function)
                        };

                        code.push_str(&format!(
                            "    // {}: {}\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result += xform.variations[{}] * {};\n\
                             \x20   }}\n",
                            id, info.display_name, id, id, call
                        ));
                    }
                }
            }
        }

        code.push_str("\n    return result;\n}\n");
        code
    }

    /// Get the registry
    pub fn registry(&self) -> &VariationRegistry {
        &self.registry
    }
}
