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
        use crate::variations::VariationPhase;

        let mut code = String::from(
            "// Apply all variations with Apophysis 4-phase execution model (XForm.pas:343-383)\n\
             fn apply_variations(xform: Transform, xform_id: u32, p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {\n"
        );

        // Separate variations by phase
        let mut pre_variations = Vec::new();
        let mut normal_variations = Vec::new();
        let mut post_variations = Vec::new();

        for (name, idx) in active_variations {
            if let Some(info) = self.registry.get(name) {
                match info.phase {
                    VariationPhase::Pre => pre_variations.push((name.clone(), *idx, info)),
                    VariationPhase::Normal => normal_variations.push((name.clone(), *idx, info)),
                    VariationPhase::Post => post_variations.push((name.clone(), *idx, info)),
                }
            }
        }

        // Sort each phase by registry index for determinism
        pre_variations.sort_by_key(|(_, idx, _)| *idx);
        normal_variations.sort_by_key(|(_, idx, _)| *idx);
        post_variations.sort_by_key(|(_, idx, _)| *idx);

        // PHASE 1: Pre-variations - directly modify input coordinates (rare in 2D)
        if !pre_variations.is_empty() {
            code.push_str("    // Phase 1: Pre-variations (modify input)\n");
            code.push_str("    var temp = p;\n\n");

            for (name, idx, info) in &pre_variations {
                // Pre-variations directly modify temp (NOT weighted sum!)

                // Handle rotation variations (hardcoded function names)
                if name.contains("rotate") {
                    let rotate_fn = if name.contains("_x") { "rotate_x" } else { "rotate_y" };
                    code.push_str(&format!(
                        "    // {}: {} (PRE)\n\
                         \x20   if (xform.variations[{}] != 0.0) {{\n\
                         \x20       temp = {}(temp, xform.variations[{}]);\n\
                         \x20   }}\n\n",
                        idx, info.display_name, idx, rotate_fn, idx
                    ));
                } else {
                    // Generic Pre-phase variation handling
                    let needs_rng = info.needs_rng;
                    let has_params = !info.parameters.is_empty();

                    // Build parameter list based on variation needs
                    let mut params = String::new();

                    // Pre-phase variations that need weight parameter
                    if name.contains("zscale") || name.contains("ztranslate")
                        || name.contains("sinusoidal") || name.contains("disc") {
                        params.push_str(&format!(", xform.variations[{}]", idx));
                    }

                    // Add xform_id and variation_id if variation has parameters
                    if has_params {
                        params.push_str(&format!(", xform_id, {}u", idx));
                    }

                    // Add RNG if needed
                    if needs_rng {
                        params.push_str(", rng");
                    }

                    code.push_str(&format!(
                        "    // {}: {} (PRE)\n\
                         \x20   if (xform.variations[{}] != 0.0) {{\n\
                         \x20       temp = variation_{}(temp{});\n\
                         \x20   }}\n\n",
                        idx, info.display_name, idx, name, params
                    ));
                }
            }
        } else {
            code.push_str("    var temp = p;\n\n");
        }

        // PHASE 2: Precalculation (handled per-variation)
        code.push_str("    // Phase 2: Precalculation handled per-variation\n\n");

        // PHASE 3: Normal variations - weighted sum accumulation
        code.push_str("    // Phase 3: Normal variations (weighted sum from modified input)\n");
        code.push_str("    var result = vec2<f32>(0.0, 0.0);\n\n");

        for (_name, idx, info) in &normal_variations {
            let call = if !info.parameters.is_empty() {
                if info.needs_rng {
                    format!("{}(temp, xform_id, {}u, rng)", info.wgsl_function, idx)
                } else {
                    format!("{}(temp, xform_id, {}u)", info.wgsl_function, idx)
                }
            } else {
                if info.needs_rng {
                    format!("{}(temp, rng)", info.wgsl_function)
                } else {
                    format!("{}(temp)", info.wgsl_function)
                }
            };

            code.push_str(&format!(
                "    // {}: {} (NORMAL)\n\
                 \x20   if (xform.variations[{}] != 0.0) {{\n\
                 \x20       result += xform.variations[{}] * {};\n\
                 \x20   }}\n\n",
                idx, info.display_name, idx, idx, call
            ));
        }

        // PHASE 4: Post-variations - directly modify output coordinates (rare in 2D)
        if !post_variations.is_empty() {
            code.push_str("    // Phase 4: Post-variations (modify output)\n\n");

            for (_name, idx, info) in &post_variations {
                // Post-variations directly modify result (NOT weighted sum!)
                let needs_rng = info.needs_rng;
                let has_params = !info.parameters.is_empty();

                // Build parameter list
                let mut params = String::from("result");

                // Post-variations with parameters (like post_bwraps)
                if has_params {
                    params.push_str(&format!(", xform_id, {}u", idx));
                } else {
                    // Traditional post-variations (rotate_x, rotate_y, flatten) use weight
                    params.push_str(&format!(", xform.variations[{}]", idx));
                }

                // Add RNG if needed
                if needs_rng {
                    params.push_str(", rng");
                }

                code.push_str(&format!(
                    "    // {}: {} (POST)\n\
                     \x20   if (xform.variations[{}] != 0.0) {{\n\
                     \x20       result = {}({});\n\
                     \x20   }}\n\n",
                    idx, info.display_name, idx, info.wgsl_function, params
                ));
            }
        }

        code.push_str("    return result;\n}\n");
        code
    }

    /// Build apply_variations function for 3D mode
    fn build_apply_variations_3d(&self, active_variations: &[(String, u32)]) -> String {
        use crate::variations::VariationPhase;

        let mut code = String::from(
            "// Apply all variations with Apophysis 4-phase execution model (XForm.pas:343-383)\n\
             fn apply_variations(xform: Transform, xform_id: u32, p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {\n"
        );

        // Separate variations by phase
        let mut pre_variations = Vec::new();
        let mut normal_variations = Vec::new();
        let mut post_variations = Vec::new();

        for (name, idx) in active_variations {
            if let Some(info) = self.registry.get(name) {
                match info.phase {
                    VariationPhase::Pre => pre_variations.push((name.clone(), *idx, info)),
                    VariationPhase::Normal => normal_variations.push((name.clone(), *idx, info)),
                    VariationPhase::Post => post_variations.push((name.clone(), *idx, info)),
                }
            }
        }

        // Sort each phase by registry index for determinism
        pre_variations.sort_by_key(|(_, idx, _)| *idx);
        normal_variations.sort_by_key(|(_, idx, _)| *idx);
        post_variations.sort_by_key(|(_, idx, _)| *idx);

        // PHASE 1: Pre-variations - directly modify input coordinates (lines 343-349)
        if !pre_variations.is_empty() {
            code.push_str("    // Phase 1: Pre-variations (modify input)\n");
            code.push_str("    var temp = p;\n\n");

            for (name, idx, info) in &pre_variations {
                // Pre-variations directly modify temp (NOT weighted sum!)

                // Handle rotation variations (hardcoded function names)
                if name.contains("rotate") {
                    let rotate_fn = if name.contains("_x") { "rotate_x" } else { "rotate_y" };
                    code.push_str(&format!(
                        "    // {}: {} (PRE)\n\
                         \x20   if (xform.variations[{}] != 0.0) {{\n\
                         \x20       temp = {}(temp, xform.variations[{}]);\n\
                         \x20   }}\n\n",
                        idx, info.display_name, idx, rotate_fn, idx
                    ));
                } else {
                    // Generic Pre-phase variation handling
                    let needs_rng = info.needs_rng;
                    let has_params = !info.parameters.is_empty();

                    // Build parameter list based on variation needs
                    let mut params = String::new();

                    // Pre-phase variations that need weight parameter
                    if name.contains("zscale") || name.contains("ztranslate")
                        || name.contains("sinusoidal") || name.contains("disc") {
                        params.push_str(&format!(", xform.variations[{}]", idx));
                    }

                    // Add xform_id and variation_id if variation has parameters
                    if has_params {
                        params.push_str(&format!(", xform_id, {}u", idx));
                    }

                    // Add RNG if needed
                    if needs_rng {
                        params.push_str(", rng");
                    }

                    code.push_str(&format!(
                        "    // {}: {} (PRE)\n\
                         \x20   if (xform.variations[{}] != 0.0) {{\n\
                         \x20       temp = variation_{}(temp{});\n\
                         \x20   }}\n\n",
                        idx, info.display_name, idx, name, params
                    ));
                }
            }
        } else {
            code.push_str("    var temp = p;\n\n");
        }

        // PHASE 2: Precalculation (would go here if needed - currently in shader functions)
        code.push_str("    // Phase 2: Precalculation handled per-variation\n\n");

        // PHASE 3: Normal variations - weighted sum accumulation (lines 363-373)
        code.push_str("    // Phase 3: Normal variations (weighted sum from modified input)\n");
        code.push_str("    var result = vec3<f32>(0.0, 0.0, 0.0);\n\n");

        for (name, idx, info) in &normal_variations {
            // Special inline implementations for Z-only variations
            match name.as_str() {
                "zcone" => {
                    code.push_str(&format!(
                        "    // {}: {} (NORMAL - Z-only)\n\
                         \x20   if (xform.variations[{}] != 0.0) {{\n\
                         \x20       let r = length(temp.xy);\n\
                         \x20       result.z += xform.variations[{}] * r;\n\
                         \x20   }}\n\n",
                        idx, info.display_name, idx, idx
                    ));
                }
                "zscale" => {
                    code.push_str(&format!(
                        "    // {}: {} (NORMAL - Z-only)\n\
                         \x20   if (xform.variations[{}] != 0.0) {{\n\
                         \x20       result.z += xform.variations[{}] * temp.z;\n\
                         \x20   }}\n\n",
                        idx, info.display_name, idx, idx
                    ));
                }
                "ztranslate" => {
                    code.push_str(&format!(
                        "    // {}: {} (NORMAL - Z-only)\n\
                         \x20   if (xform.variations[{}] != 0.0) {{\n\
                         \x20       result.z += xform.variations[{}];\n\
                         \x20   }}\n\n",
                        idx, info.display_name, idx, idx
                    ));
                }
                _ => {
                    // Standard variation with function call
                    let call = if !info.parameters.is_empty() {
                        if info.needs_rng {
                            format!("{}(temp, xform_id, {}u, rng)", info.wgsl_function, idx)
                        } else {
                            format!("{}(temp, xform_id, {}u)", info.wgsl_function, idx)
                        }
                    } else {
                        if info.needs_rng {
                            format!("{}(temp, rng)", info.wgsl_function)
                        } else {
                            format!("{}(temp)", info.wgsl_function)
                        }
                    };

                    code.push_str(&format!(
                        "    // {}: {} (NORMAL)\n\
                         \x20   if (xform.variations[{}] != 0.0) {{\n\
                         \x20       result += xform.variations[{}] * {};\n\
                         \x20   }}\n\n",
                        idx, info.display_name, idx, idx, call
                    ));
                }
            }
        }

        // PHASE 4: Post-variations - directly modify output coordinates (lines 375-383)
        if !post_variations.is_empty() {
            code.push_str("    // Phase 4: Post-variations (modify output)\n\n");

            for (name, idx, info) in &post_variations {
                // Post-variations directly modify result (NOT weighted sum!)
                match name.as_str() {
                    "flatten" => {
                        // Flatten sets Z to zero (Apophysis: FPz := 0)
                        code.push_str(&format!(
                            "    // {}: {} (POST - Z-only)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result.z = 0.0;\n\
                             \x20   }}\n\n",
                            idx, info.display_name, idx
                        ));
                    }
                    _ => {
                        // Generic post-variations (rotation, post_bwraps, etc.)
                        let needs_rng = info.needs_rng;
                        let has_params = !info.parameters.is_empty();

                        // Build parameter list
                        let mut params = String::from("result");

                        // Post-variations with parameters (like post_bwraps)
                        if has_params {
                            params.push_str(&format!(", xform_id, {}u", idx));
                        } else if name.contains("rotate") {
                            // Traditional rotate variations use weight
                            let rotate_fn = if name.contains("_x") { "rotate_x" } else { "rotate_y" };
                            code.push_str(&format!(
                                "    // {}: {} (POST - Rotation)\n\
                                 \x20   if (xform.variations[{}] != 0.0) {{\n\
                                 \x20       result = {}(result, xform.variations[{}]);\n\
                                 \x20   }}\n\n",
                                idx, info.display_name, idx, rotate_fn, idx
                            ));
                            continue;  // Skip the generic code below
                        } else {
                            // Other traditional post-variations use weight
                            params.push_str(&format!(", xform.variations[{}]", idx));
                        }

                        // Add RNG if needed
                        if needs_rng {
                            params.push_str(", rng");
                        }

                        code.push_str(&format!(
                            "    // {}: {} (POST)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result = {}({});\n\
                             \x20   }}\n\n",
                            idx, info.display_name, idx, info.wgsl_function, params
                        ));
                    }
                }
            }
        }

        code.push_str("    return result;\n}\n");
        code
    }

    /// Build 2D TILED trajectory shader with active variations
    /// Uses full-resolution coordinates and routes samples to tile buffers
    pub fn build_trajectory_2d_tiled(&self, active_variations: &HashMap<String, f32>) -> String {
        use crate::variations::VariationCategory;
        use std::collections::HashMap;

        let mut index_map: HashMap<String, u32> = HashMap::new();
        for (i, name) in self.registry.names().iter().enumerate() {
            if let Some(info) = self.registry.get(name) {
                if matches!(info.category, VariationCategory::Basic2D | VariationCategory::Advanced2D) {
                    index_map.insert(name.clone(), i as u32);
                }
            }
        }

        let active_2d: Vec<(String, u32)> = index_map
            .iter()
            .filter(|(name, _)| active_variations.contains_key(*name))
            .map(|(name, idx)| (name.clone(), *idx))
            .collect();

        let mut shader = String::new();

        // 1. Tiled header (includes TileParams binding)
        shader.push_str(include_str!("../shaders/core/header_tiled.wgsl"));
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

        // 6. Generate apply_variations
        shader.push_str(&self.build_apply_variations_2d(&active_2d));
        shader.push('\n');

        // 7. Tiled utilities (uses full_width/full_height)
        shader.push_str(include_str!("../shaders/core/utilities_tiled.wgsl"));
        shader.push('\n');

        // 8. Tiled main
        shader.push_str(include_str!("../shaders/core/main_2d_tiled.wgsl"));

        shader
    }

    /// Build 3D TILED trajectory shader with active variations
    pub fn build_trajectory_3d_tiled(&self, active_variations: &HashMap<String, f32>) -> String {
        use std::collections::HashMap;

        let mut index_map: HashMap<String, u32> = HashMap::new();
        for (i, name) in self.registry.names().iter().enumerate() {
            index_map.insert(name.clone(), i as u32);
        }

        let active_3d: Vec<(String, u32)> = index_map
            .iter()
            .filter(|(name, _)| active_variations.contains_key(*name))
            .map(|(name, idx)| (name.clone(), *idx))
            .collect();

        let mut shader = String::new();

        // 1. Tiled header
        shader.push_str(include_str!("../shaders/core/header_tiled.wgsl"));
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

        // 5. Generate apply_variations
        shader.push_str(&self.build_apply_variations_3d(&active_3d));
        shader.push('\n');

        // 6. Tiled utilities
        shader.push_str(include_str!("../shaders/core/utilities_tiled.wgsl"));
        shader.push('\n');

        // 7. Tiled main
        shader.push_str(include_str!("../shaders/core/main_3d_tiled.wgsl"));

        shader
    }

    /// Build 2D EXPORT shader - outputs samples to buffer for CPU histogram
    /// NOTE: Uses 3D shader internally to support configs with 3D variations like "flatten"
    /// even when render_mode is 2D. The 3D shader handles 2D correctly (Z is just ignored).
    pub fn build_export_2d(&self, active_variations: &HashMap<String, f32>) -> String {
        // Use 3D shader for export - it handles all variation types correctly
        // The 2D vs 3D distinction only matters for how Z is displayed, not for export
        self.build_export_3d(active_variations)
    }

    /// Build 3D EXPORT shader - outputs samples to buffer for CPU histogram
    pub fn build_export_3d(&self, active_variations: &HashMap<String, f32>) -> String {
        use std::collections::HashMap;

        let mut index_map: HashMap<String, u32> = HashMap::new();
        for (i, name) in self.registry.names().iter().enumerate() {
            index_map.insert(name.clone(), i as u32);
        }

        let active_3d: Vec<(String, u32)> = index_map
            .iter()
            .filter(|(name, _)| active_variations.contains_key(*name))
            .map(|(name, idx)| (name.clone(), *idx))
            .collect();

        let mut shader = String::new();

        // 1. Export header
        shader.push_str(include_str!("../shaders/core/header_export.wgsl"));
        shader.push('\n');

        // 2. RNG
        shader.push_str(include_str!("../shaders/core/rng.wgsl"));
        shader.push('\n');

        // 3. Standard utilities (MUST come before variations - defines get_param)
        shader.push_str(include_str!("../shaders/core/utilities.wgsl"));
        shader.push('\n');

        // 4. Core variations (3D)
        shader.push_str(include_str!("../shaders/core/variations_3d.wgsl"));
        shader.push('\n');

        // 5. Plugin variations
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

        // 6. Generate apply_variations
        shader.push_str(&self.build_apply_variations_3d(&active_3d));
        shader.push('\n');

        // 7. Export main
        shader.push_str(include_str!("../shaders/core/main_3d_export.wgsl"));

        shader
    }
}
