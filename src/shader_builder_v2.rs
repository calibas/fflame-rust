use std::collections::HashMap;
use crate::variations::VariationRegistry;

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
        // Filter to only 2D variations (exclude 3D-only variations)
        use crate::variations::VariationCategory;
        use std::collections::HashMap;

        // Build a map of variation name -> registry index (0-23)
        let mut index_map: HashMap<String, u32> = HashMap::new();
        for (i, name) in self.registry.names().iter().enumerate() {
            // Only include 2D variations
            if let Some(info) = self.registry.get(name) {
                if matches!(info.category, VariationCategory::Basic2D | VariationCategory::Advanced2D) {
                    index_map.insert(name.clone(), i as u32);
                }
            }
        }

        // Only include active 2D variations
        let active_2d: Vec<(String, u32)> = index_map
            .iter()
            .filter(|(name, _)| active_variations.contains_key(*name))
            .map(|(name, idx)| (name.clone(), *idx))
            .collect();

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

        // 5. Plugin variations (2D only)
        for (name, _) in &active_2d {
            if let Some(info) = self.registry.get(name) {
                if !info.is_core {
                    if let Some(source) = &info.wgsl_source {
                        shader.push_str(source);
                        shader.push('\n');
                    }
                }
            }
        }

        // 6. Generate apply_variations with fixed registry indices
        shader.push_str(&self.build_apply_variations_2d(&active_2d));
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
        use std::collections::HashMap;

        // Build a map of variation name -> registry index (0-23)
        // For 3D, include ALL variations (2D and 3D)
        let mut index_map: HashMap<String, u32> = HashMap::new();
        for (i, name) in self.registry.names().iter().enumerate() {
            index_map.insert(name.clone(), i as u32);
        }

        // Only include active variations
        let active_3d: Vec<(String, u32)> = index_map
            .iter()
            .filter(|(name, _)| active_variations.contains_key(*name))
            .map(|(name, idx)| (name.clone(), *idx))
            .collect();

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
        for (name, _) in &active_3d {
            if let Some(info) = self.registry.get(name) {
                if !info.is_core {
                    if let Some(source) = &info.wgsl_source {
                        shader.push_str(source);
                        shader.push('\n');
                    }
                }
            }
        }

        // 5. Generate apply_variations with fixed registry indices
        shader.push_str(&self.build_apply_variations_3d(&active_3d));
        shader.push('\n');

        // 6. Utilities
        shader.push_str(include_str!("../shaders/core/utilities.wgsl"));
        shader.push('\n');

        // 7. Main
        shader.push_str(include_str!("../shaders/core/main_3d.wgsl"));

        shader
    }

    /// Build apply_variations function for 2D mode
    fn build_apply_variations_2d(&self, active_variations: &[(String, u32)]) -> String {
        let mut code = String::from(
            "// Apply all variations with weights\n\
             fn apply_variations(xform: Transform, xform_id: u32, p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {\n\
             \x20   // Precalculate common values (Apophysis optimization)\n\
             \x20   let r = length(p);\n\
             \x20   let r2 = dot(p, p);\n\
             \x20   let theta = atan2(p.x, p.y);\n\
             \x20   let sin_theta = sin(theta);\n\
             \x20   let cos_theta = cos(theta);\n\n\
             \x20   var result = vec2<f32>(0.0, 0.0);\n\n"
        );

        // Sort by registry index for deterministic shader generation
        let mut entries = active_variations.to_vec();
        entries.sort_by_key(|(_, idx)| *idx);

        for (name, idx) in entries {
            if let Some(info) = self.registry.get(&name) {
                // Determine function call signature based on variation needs
                let call = match name.as_str() {
                    // Variations that don't use precalculated values
                    "linear" | "sinusoidal" => {
                        format!("{}(p)", info.wgsl_function)
                    }
                    // Variations that use r2
                    "spherical" | "swirl" => {
                        format!("{}(p, r2)", info.wgsl_function)
                    }
                    // Variations that use r only
                    "horseshoe" => {
                        format!("{}(p, r)", info.wgsl_function)
                    }
                    // Variations that use theta and r (polar coordinates)
                    "polar" => {
                        format!("{}(theta, r)", info.wgsl_function)
                    }
                    "handkerchief" | "heart" => {
                        format!("{}(r, theta)", info.wgsl_function)
                    }
                    "disc" => {
                        format!("{}(theta, r)", info.wgsl_function)
                    }
                    "ex" => {
                        format!("{}(r, theta)", info.wgsl_function)
                    }
                    // Variations that use r, sin_theta, cos_theta
                    "spiral" | "hyperbolic" | "diamond" => {
                        format!("{}(r, sin_theta, cos_theta)", info.wgsl_function)
                    }
                    // Variations with RNG (not optimized yet, keep standard call)
                    "julia" => {
                        format!("{}(p, rng)", info.wgsl_function)
                    }
                    // Blob: parameterized + optimized (uses precalculated values)
                    "blob" => {
                        format!("{}(r, theta, sin_theta, cos_theta, xform_id)", info.wgsl_function)
                    }
                    // Parameterized variations
                    _ if !info.parameters.is_empty() => {
                        if info.needs_rng {
                            format!("{}(p, xform_id, rng)", info.wgsl_function)
                        } else {
                            format!("{}(p, xform_id)", info.wgsl_function)
                        }
                    }
                    // Default: standard variations
                    _ => {
                        if info.needs_rng {
                            format!("{}(p, rng)", info.wgsl_function)
                        } else {
                            format!("{}(p)", info.wgsl_function)
                        }
                    }
                };

                code.push_str(&format!(
                    "    // {}: {}\n\
                     \x20   if (xform.variations[{}] != 0.0) {{\n\
                     \x20       result += xform.variations[{}] * {};\n\
                     \x20   }}\n",
                    idx, info.display_name, idx, idx, call
                ));
            }
        }

        code.push_str("\n    return result;\n}\n");
        code
    }

    /// Build apply_variations function for 3D mode
    fn build_apply_variations_3d(&self, active_variations: &[(String, u32)]) -> String {
        let mut code = String::from(
            "// Apply all variations with weights (3D)\n\
             fn apply_variations(xform: Transform, xform_id: u32, p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {\n\
             \x20   // Precalculate common values (Apophysis optimization)\n\
             \x20   let r = length(p.xy);\n\
             \x20   let r2 = dot(p.xy, p.xy);\n\
             \x20   let theta = atan2(p.x, p.y);\n\
             \x20   let sin_theta = sin(theta);\n\
             \x20   let cos_theta = cos(theta);\n\n\
             \x20   var result = vec3<f32>(0.0, 0.0, 0.0);\n\n"
        );

        // Sort by registry index
        let mut entries = active_variations.to_vec();
        entries.sort_by_key(|(_, idx)| *idx);

        for (name, idx) in entries {
            if let Some(info) = self.registry.get(&name) {
                // Special handling for Z-only and rotation variations
                match name.as_str() {
                    "zcone" => {
                        code.push_str(&format!(
                            "    // {}: {} (Z-only)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       let r = length(p.xy);\n\
                             \x20       result.z += xform.variations[{}] * r;\n\
                             \x20   }}\n",
                            idx, info.display_name, idx, idx
                        ));
                    }
                    "flatten" => {
                        code.push_str(&format!(
                            "    // {}: {} (Z-only)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result.z *= (1.0 - xform.variations[{}] * 0.5);\n\
                             \x20   }}\n",
                            idx, info.display_name, idx, idx
                        ));
                    }
                    "zscale" => {
                        code.push_str(&format!(
                            "    // {}: {} (Z-only)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result.z *= (1.0 + xform.variations[{}]);\n\
                             \x20   }}\n",
                            idx, info.display_name, idx, idx
                        ));
                    }
                    "pre_rotate_x" | "pre_rotate_y" | "post_rotate_x" | "post_rotate_y" => {
                        let rotate_fn = if name.contains("_x") { "rotate_x" } else { "rotate_y" };
                        code.push_str(&format!(
                            "    // {}: {} (Rotation)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result = {}(result, xform.variations[{}]);\n\
                             \x20   }}\n",
                            idx, info.display_name, idx, rotate_fn, idx
                        ));
                    }
                    _ => {
                        // Standard variation - same call pattern matching as 2D
                        let call = match name.as_str() {
                            // Basic variations that don't use precalc
                            "linear" | "sinusoidal" | "bent" | "waves" => {
                                format!("{}(p)", info.wgsl_function)
                            }
                            // Variations using r2
                            "spherical" | "swirl" => {
                                format!("{}(p, r2)", info.wgsl_function)
                            }
                            // Variations using r
                            "horseshoe" => {
                                format!("{}(p, r)", info.wgsl_function)
                            }
                            // Variations using theta, r, z
                            "polar" => {
                                format!("{}(theta, r, p.z)", info.wgsl_function)
                            }
                            "handkerchief" | "heart" => {
                                format!("{}(r, theta, p.z)", info.wgsl_function)
                            }
                            "disc" => {
                                format!("{}(theta, r, p.z)", info.wgsl_function)
                            }
                            "ex" => {
                                format!("{}(r, theta, p.z)", info.wgsl_function)
                            }
                            // Variations using r, sin_theta, cos_theta, z
                            "spiral" | "hyperbolic" | "diamond" => {
                                format!("{}(r, sin_theta, cos_theta, p.z)", info.wgsl_function)
                            }
                            // RNG variations
                            "julia" => {
                                format!("{}(p, rng)", info.wgsl_function)
                            }
                            // Blob: parameterized + optimized
                            "blob" => {
                                format!("{}(r, theta, sin_theta, cos_theta, p.z, xform_id)", info.wgsl_function)
                            }
                            // Parameterized variations
                            _ if !info.parameters.is_empty() => {
                                if info.needs_rng {
                                    format!("{}(p, xform_id, rng)", info.wgsl_function)
                                } else {
                                    format!("{}(p, xform_id)", info.wgsl_function)
                                }
                            }
                            // Default fallback
                            _ => {
                                if info.needs_rng {
                                    format!("{}(p, rng)", info.wgsl_function)
                                } else {
                                    format!("{}(p)", info.wgsl_function)
                                }
                            }
                        };

                        code.push_str(&format!(
                            "    // {}: {}\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result += xform.variations[{}] * {};\n\
                             \x20   }}\n",
                            idx, info.display_name, idx, idx, call
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
