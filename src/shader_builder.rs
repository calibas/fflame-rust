use std::collections::HashSet;
use crate::scene::transforms::Flame;

/// Builds WGSL shaders dynamically by composing modular components
pub struct ShaderBuilder {
    /// Core shader components (always included)
    header: String,
    rng: String,
    affine: String,
    utilities: String,

    /// Core variations (indices 0-23, always compiled)
    core_variations_2d: String,
    core_variations_3d: String,

    /// Main loop template
    main_loop_2d: String,
    main_loop_3d: String,
}

impl ShaderBuilder {
    pub fn new() -> Self {
        Self {
            header: include_str!("../shaders/core/header.wgsl").to_string(),
            rng: include_str!("../shaders/core/rng.wgsl").to_string(),
            affine: include_str!("../shaders/core/affine.wgsl").to_string(),
            utilities: include_str!("../shaders/core/utilities.wgsl").to_string(),
            core_variations_2d: include_str!("../shaders/core/variations_2d.wgsl").to_string(),
            core_variations_3d: include_str!("../shaders/core/variations_3d.wgsl").to_string(),
            main_loop_2d: include_str!("../shaders/core/main_2d.wgsl").to_string(),
            main_loop_3d: include_str!("../shaders/core/main_3d.wgsl").to_string(),
        }
    }

    /// Build 2D trajectory shader
    pub fn build_trajectory_2d(&self, active_variations: &HashSet<u32>) -> String {
        let mut shader = String::new();

        // 1. Header (structs + bindings)
        shader.push_str(&self.header);
        shader.push('\n');

        // 2. RNG functions
        shader.push_str(&self.rng);
        shader.push('\n');

        // 3. Affine transform
        shader.push_str(&self.affine);
        shader.push('\n');

        // 4. Core variations (2D)
        shader.push_str(&self.core_variations_2d);
        shader.push('\n');

        // 5. Plugin variations (if any beyond 23)
        for var_id in active_variations.iter().filter(|&&id| id >= 24) {
            if let Some(code) = self.load_plugin_variation(*var_id) {
                shader.push_str(&code);
                shader.push('\n');
            }
        }

        // 6. Variation application function (dispatch)
        shader.push_str(&self.build_apply_variations_2d(active_variations));
        shader.push('\n');

        // 7. Utilities (select_transform, world_to_pixel, speed_to_color)
        shader.push_str(&self.utilities);
        shader.push('\n');

        // 8. Main loop
        shader.push_str(&self.main_loop_2d);

        shader
    }

    /// Build 3D trajectory shader
    pub fn build_trajectory_3d(&self, active_variations: &HashSet<u32>) -> String {
        let mut shader = String::new();

        // 1. Header (structs + bindings) - same as 2D
        shader.push_str(&self.header);
        shader.push('\n');

        // 2. RNG functions
        shader.push_str(&self.rng);
        shader.push('\n');

        // 3. Affine transform (3D version - defined in variations_3d.wgsl)
        // Skip affine here, it's in variations_3d

        // 4. Core variations (3D versions)
        shader.push_str(&self.core_variations_3d);
        shader.push('\n');

        // 5. Plugin variations (if any beyond 23)
        for var_id in active_variations.iter().filter(|&&id| id >= 24) {
            if let Some(code) = self.load_plugin_variation_3d(*var_id) {
                shader.push_str(&code);
                shader.push('\n');
            }
        }

        // 6. Variation application function (dispatch)
        shader.push_str(&self.build_apply_variations_3d(active_variations));
        shader.push('\n');

        // 7. Utilities (3D versions - projection, camera rotation)
        shader.push_str(&self.utilities);
        shader.push('\n');

        // 8. Main loop (3D)
        shader.push_str(&self.main_loop_3d);

        shader
    }

    /// Build the apply_variations function for 2D
    fn build_apply_variations_2d(&self, active: &HashSet<u32>) -> String {
        let mut code = String::from(
            "// Apply all variations with weights\n\
             fn apply_variations(xform: Transform, p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {\n\
             \x20   var result = vec2<f32>(0.0, 0.0);\n\n"
        );

        // Include all core variations (0-23) and any plugin variations
        let mut var_indices: Vec<u32> = active.iter().copied().collect();
        var_indices.sort();

        for var_id in var_indices.iter().filter(|&&id| id < 24) {
            let var_name = variation_name_2d(*var_id);
            if var_name == "julia" {
                code.push_str(&format!(
                    "    // Variation {}: {}\n\
                     \x20   if (xform.variations[{}] != 0.0) {{\n\
                     \x20       result += xform.variations[{}] * {}(p, rng);\n\
                     \x20   }}\n",
                    var_id, var_name, var_id, var_id, var_name
                ));
            } else {
                code.push_str(&format!(
                    "    // Variation {}: {}\n\
                     \x20   if (xform.variations[{}] != 0.0) {{\n\
                     \x20       result += xform.variations[{}] * {}(p);\n\
                     \x20   }}\n",
                    var_id, var_name, var_id, var_id, var_name
                ));
            }
        }

        // Plugin variations (24+)
        for var_id in var_indices.iter().filter(|&&id| id >= 24) {
            code.push_str(&format!(
                "    // Variation {}: plugin_{}\n\
                 \x20   if (xform.variations[{}] != 0.0) {{\n\
                 \x20       result += xform.variations[{}] * variation_plugin_{}(p, rng);\n\
                 \x20   }}\n",
                var_id, var_id, var_id, var_id, var_id
            ));
        }

        code.push_str("\n    return result;\n}\n");
        code
    }

    /// Build the apply_variations function for 3D
    fn build_apply_variations_3d(&self, active: &HashSet<u32>) -> String {
        let mut code = String::from(
            "// Apply all variations with weights (3D version)\n\
             fn apply_variations(xform: Transform, p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {\n\
             \x20   var result = vec3<f32>(0.0, 0.0, 0.0);\n\n"
        );

        let mut var_indices: Vec<u32> = active.iter().copied().collect();
        var_indices.sort();

        // 2D variations (0-15) - pass Z through
        for var_id in var_indices.iter().filter(|&&id| id < 16) {
            let var_name = variation_name_2d(*var_id);
            if var_name == "julia" {
                code.push_str(&format!(
                    "    // Variation {}: {} (2D, Z pass-through)\n\
                     \x20   if (xform.variations[{}] != 0.0) {{\n\
                     \x20       result += xform.variations[{}] * {}(p, rng);\n\
                     \x20   }}\n",
                    var_id, var_name, var_id, var_id, var_name
                ));
            } else {
                code.push_str(&format!(
                    "    // Variation {}: {} (2D, Z pass-through)\n\
                     \x20   if (xform.variations[{}] != 0.0) {{\n\
                     \x20       result += xform.variations[{}] * {}(p);\n\
                     \x20   }}\n",
                    var_id, var_name, var_id, var_id, var_name
                ));
            }
        }

        // 3D variations (16-23) - special handling for Z-only vs full 3D
        code.push_str("\n    // === 3D Variations (16-23) ===\n");
        code.push_str("    // Note: Z-only variations modify result.z directly instead of adding to result\n\n");

        if var_indices.contains(&16) {
            code.push_str(
                "    // Variation 16: Zcone - Z becomes distance from origin in XY\n\
                 \x20   if (xform.variations[16] != 0.0) {\n\
                 \x20       let r = length(p.xy);\n\
                 \x20       result.z += xform.variations[16] * r;\n\
                 \x20   }\n\n"
            );
        }

        if var_indices.contains(&17) {
            code.push_str(
                "    // Variation 17: Flatten - compress Z toward zero\n\
                 \x20   if (xform.variations[17] != 0.0) {\n\
                 \x20       result.z *= (1.0 - xform.variations[17] * 0.5);\n\
                 \x20   }\n\n"
            );
        }

        if var_indices.contains(&18) {
            code.push_str(
                "    // Variation 18: Hemisphere - project onto hemisphere (affects all axes)\n\
                 \x20   if (xform.variations[18] != 0.0) {\n\
                 \x20       result += xform.variations[18] * variation_hemisphere(p);\n\
                 \x20   }\n\n"
            );
        }

        if var_indices.contains(&19) {
            code.push_str(
                "    // Variation 19: PreRotateX - rotate around X before other variations\n\
                 \x20   if (xform.variations[19] != 0.0) {\n\
                 \x20       result = rotate_x(result, xform.variations[19]);\n\
                 \x20   }\n\n"
            );
        }

        if var_indices.contains(&20) {
            code.push_str(
                "    // Variation 20: PreRotateY - rotate around Y before other variations\n\
                 \x20   if (xform.variations[20] != 0.0) {\n\
                 \x20       result = rotate_y(result, xform.variations[20]);\n\
                 \x20   }\n\n"
            );
        }

        if var_indices.contains(&21) {
            code.push_str(
                "    // Variation 21: PostRotateX - rotate around X after other variations\n\
                 \x20   if (xform.variations[21] != 0.0) {\n\
                 \x20       result = rotate_x(result, xform.variations[21]);\n\
                 \x20   }\n\n"
            );
        }

        if var_indices.contains(&22) {
            code.push_str(
                "    // Variation 22: PostRotateY - rotate around Y after other variations\n\
                 \x20   if (xform.variations[22] != 0.0) {\n\
                 \x20       result = rotate_y(result, xform.variations[22]);\n\
                 \x20   }\n\n"
            );
        }

        if var_indices.contains(&23) {
            code.push_str(
                "    // Variation 23: ZScale - scale Z coordinate\n\
                 \x20   if (xform.variations[23] != 0.0) {\n\
                 \x20       result.z *= (1.0 + xform.variations[23]);\n\
                 \x20   }\n\n"
            );
        }

        // Plugin variations (24+)
        for var_id in var_indices.iter().filter(|&&id| id >= 24) {
            code.push_str(&format!(
                "    // Variation {}: plugin_{}\n\
                 \x20   if (xform.variations[{}] != 0.0) {{\n\
                 \x20       result += xform.variations[{}] * variation_plugin_{}(p, rng);\n\
                 \x20   }}\n",
                var_id, var_id, var_id, var_id, var_id
            ));
        }

        code.push_str("\n    return result;\n}\n");
        code
    }

    /// Load plugin variation code (2D version)
    fn load_plugin_variation(&self, _var_id: u32) -> Option<String> {
        // In the future, this will load from files or a plugin registry
        // For now, just a placeholder
        None
    }

    /// Load plugin variation code (3D version)
    fn load_plugin_variation_3d(&self, _var_id: u32) -> Option<String> {
        // In the future, this will load from files or a plugin registry
        None
    }

    /// Extract active variation indices from a flame
    pub fn extract_active_variations(flame: &Flame) -> HashSet<u32> {
        let mut active = HashSet::new();

        // Convert named variations to indices using the fixed array mapping
        for transform in &flame.transforms {
            let fixed_array = transform.to_fixed_array(&flame.variation_registry);
            for (idx, &weight) in fixed_array.iter().enumerate() {
                if weight.abs() > 1e-6 {
                    active.insert(idx as u32);
                }
            }
        }

        // Always include at least Linear (index 0) to avoid empty shaders
        active.insert(0);

        active
    }
}

impl Default for ShaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Get variation name for 2D variations (0-23)
fn variation_name_2d(id: u32) -> &'static str {
    match id {
        0 => "variation_linear",
        1 => "variation_sinusoidal",
        2 => "variation_spherical",
        3 => "variation_swirl",
        4 => "variation_horseshoe",
        5 => "variation_polar",
        6 => "variation_handkerchief",
        7 => "variation_heart",
        8 => "variation_disc",
        9 => "variation_spiral",
        10 => "variation_hyperbolic",
        11 => "variation_diamond",
        12 => "variation_ex",
        13 => "julia", // Special case: needs RNG
        14 => "variation_bent",
        15 => "variation_waves",
        _ => "variation_linear", // Fallback
    }
}
