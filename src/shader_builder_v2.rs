use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::variations::VariationRegistry;

/// Global flag to enable shader dumping (set via CLI --dump-shader flag)
static DUMP_SHADER_ENABLED: AtomicBool = AtomicBool::new(false);

/// Global flag to enable inlined constants for maximum performance
/// When enabled, the shader builder generates specialized shaders with
/// transform data compiled as constants instead of read from buffers.
/// This triggers shader recompilation on every flame parameter change,
/// so it should only be used for batch/CLI rendering (not interactive mode).
static INLINED_CONSTANTS_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable shader dumping (writes generated shaders to debug_shader_*.wgsl files)
pub fn enable_shader_dump() {
    DUMP_SHADER_ENABLED.store(true, Ordering::Relaxed);
}

/// Enable inlined constants mode for maximum shader performance.
/// WARNING: This triggers shader recompilation on every flame parameter change!
/// Only use for CLI/batch rendering, not interactive mode.
pub fn enable_inlined_constants() {
    INLINED_CONSTANTS_ENABLED.store(true, Ordering::Relaxed);
}

/// Check if shader dumping is enabled
fn should_dump_shader() -> bool {
    DUMP_SHADER_ENABLED.load(Ordering::Relaxed)
}

/// Check if inlined constants are enabled
pub fn should_use_inlined_constants() -> bool {
    INLINED_CONSTANTS_ENABLED.load(Ordering::Relaxed)
}

/// Simple template processor for shader conditional compilation
///
/// Supports:
/// - `{{#if CONDITION}}...{{/if}}` - Include block if condition is true
/// - `{{#if CONDITION}}...{{else}}...{{/if}}` - If/else blocks
/// - Nested conditionals supported
pub struct TemplateProcessor {
    conditions: HashMap<String, bool>,
}

impl TemplateProcessor {
    pub fn new() -> Self {
        Self {
            conditions: HashMap::new(),
        }
    }

    /// Set a condition value
    pub fn set(&mut self, name: &str, value: bool) -> &mut Self {
        self.conditions.insert(name.to_string(), value);
        self
    }

    /// Process template and return expanded source
    pub fn process(&self, template: &str) -> String {
        self.process_conditionals(template)
    }

    /// Process all conditional blocks in the template
    fn process_conditionals(&self, input: &str) -> String {
        let mut result = input.to_string();

        // Process conditionals from innermost to outermost
        // Keep processing until no more changes (handles nesting)
        loop {
            let new_result = self.process_single_pass(&result);
            if new_result == result {
                break;
            }
            result = new_result;
        }

        result
    }

    /// Process one level of conditionals
    fn process_single_pass(&self, input: &str) -> String {
        let mut result = String::new();
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '{' && chars.peek() == Some(&'{') {
                chars.next(); // consume second '{'

                // Check for #if
                let mut tag = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '}' {
                        break;
                    }
                    tag.push(chars.next().unwrap());
                }

                // Consume '}}'
                if chars.next() == Some('}') && chars.next() == Some('}') {
                    // Successfully consumed tag
                } else {
                    // Malformed tag, output as-is
                    result.push_str("{{");
                    result.push_str(&tag);
                    continue;
                }

                if tag.starts_with("#if ") {
                    let condition_name = tag[4..].trim();
                    let condition_value = self.conditions.get(condition_name).copied().unwrap_or(false);

                    // Find matching {{else}} and {{/if}}
                    let (if_block, else_block, remaining) = self.extract_if_else_blocks(&mut chars);

                    if condition_value {
                        result.push_str(&if_block);
                    } else if let Some(else_content) = else_block {
                        result.push_str(&else_content);
                    }

                    // The remaining content after {{/if}} is already consumed
                    result.push_str(&remaining);
                    return result + &chars.collect::<String>();
                } else {
                    // Unknown tag, output as-is
                    result.push_str("{{");
                    result.push_str(&tag);
                    result.push_str("}}");
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Extract if/else/endif blocks, handling nesting
    fn extract_if_else_blocks(&self, chars: &mut std::iter::Peekable<std::str::Chars>) -> (String, Option<String>, String) {
        let mut if_block = String::new();
        let mut else_block: Option<String> = None;
        let mut in_else = false;
        let mut depth = 1;

        while let Some(c) = chars.next() {
            if c == '{' && chars.peek() == Some(&'{') {
                chars.next();

                let mut tag = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '}' {
                        break;
                    }
                    tag.push(chars.next().unwrap());
                }

                // Consume '}}'
                if chars.next() == Some('}') {
                    chars.next();
                }

                if tag.starts_with("#if ") {
                    depth += 1;
                    // Include nested #if in current block
                    if in_else {
                        else_block.as_mut().unwrap().push_str("{{");
                        else_block.as_mut().unwrap().push_str(&tag);
                        else_block.as_mut().unwrap().push_str("}}");
                    } else {
                        if_block.push_str("{{");
                        if_block.push_str(&tag);
                        if_block.push_str("}}");
                    }
                } else if tag == "/if" {
                    depth -= 1;
                    if depth == 0 {
                        // Found matching {{/if}}
                        return (if_block, else_block, String::new());
                    } else {
                        // Nested {{/if}}
                        if in_else {
                            else_block.as_mut().unwrap().push_str("{{/if}}");
                        } else {
                            if_block.push_str("{{/if}}");
                        }
                    }
                } else if tag == "else" && depth == 1 {
                    in_else = true;
                    else_block = Some(String::new());
                } else {
                    // Other tag, include as-is
                    if in_else {
                        else_block.as_mut().unwrap().push_str("{{");
                        else_block.as_mut().unwrap().push_str(&tag);
                        else_block.as_mut().unwrap().push_str("}}");
                    } else {
                        if_block.push_str("{{");
                        if_block.push_str(&tag);
                        if_block.push_str("}}");
                    }
                }
            } else {
                if in_else {
                    else_block.as_mut().unwrap().push(c);
                } else {
                    if_block.push(c);
                }
            }
        }

        // If we get here, template was malformed (unclosed #if)
        (if_block, else_block, String::new())
    }
}

/// Inlined transform data for shader compilation
///
/// Contains all per-transform data that will be compiled as shader constants
/// instead of being read from buffers at runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct InlinedTransform {
    /// Affine coefficients: x' = ax + by + e, y' = cx + dy + f
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
    /// Z offset for 3D mode
    pub g: f32,
    /// Transform selection weight
    pub weight: f32,
    /// Color palette position (0-1)
    pub color: f32,
    /// Color speed / symmetry (-1 to 1)
    pub color_speed: f32,
    /// Opacity / visibility (0-1)
    pub opacity: f32,
    /// Variation weights by registry index (sparse - only non-zero)
    /// Key = variation registry index, Value = weight
    pub variation_weights: Vec<(u32, f32)>,
    /// Variation parameters by registry index and param slot
    /// Key = (variation_index, param_slot), Value = param value
    pub variation_params: Vec<((u32, u32), f32)>,
}

/// Constants that get hard-coded into shaders
///
/// These values are compiled directly into the shader as `const` declarations,
/// eliminating uniform buffer reads and enabling the shader compiler to
/// optimize based on known values (dead code elimination, constant folding).
///
/// When any of these values change, shaders must be recompiled.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderConstants {
    /// Number of transforms (allows loop unrolling in select_transform)
    pub num_transforms: u32,

    /// Color mode: 0=Palette, 1=Speed, 2=PathMap
    /// Enables dead code elimination for unused color mode branches
    pub color_mode: u32,

    /// Whether final transform is enabled (eliminates branch if false)
    pub has_final_transform: bool,

    /// Index of final transform (only used if has_final_transform is true)
    pub final_transform_index: u32,

    /// Inlined transform data (eliminates buffer reads)
    /// When Some, transform data is compiled as constants
    /// When None, transform data is read from buffers (legacy behavior)
    pub inlined_transforms: Option<Vec<InlinedTransform>>,

    /// Whether any transform uses post-affine (eliminates branch if false)
    pub has_post_affine: bool,

    /// Precomputed cumulative weights for transform selection
    /// Eliminates the weight accumulation loops in select_transform
    pub cumulative_weights: Option<Vec<f32>>,
}

impl Default for ShaderConstants {
    fn default() -> Self {
        Self {
            num_transforms: 1,
            color_mode: 0,
            has_final_transform: false,
            final_transform_index: 0,
            has_post_affine: false,
            inlined_transforms: None,
            cumulative_weights: None,
        }
    }
}

impl ShaderConstants {
    /// Create constants with inlined transform data from a Flame
    ///
    /// This enables full constant inlining for maximum performance.
    /// The shader will be specialized for this exact flame configuration.
    pub fn with_inlined_transforms(
        flame: &crate::scene::transforms::Flame,
        registry: &crate::variations::VariationRegistry,
        color_mode: u32,
    ) -> Self {
        // Ensure at least 1 transform to prevent shader overflow (NUM_TRANSFORMS - 1u)
        let num_transforms = flame.transforms.len().max(1) as u32;
        let has_final = flame.final_transform.is_some();
        let final_idx = num_transforms; // Final comes after regular transforms

        // Per-flame local index map. Must match what the buffer populator
        // and the shader builder use, so that var_idx in the inlined weight
        // table aligns with `xform.variations[var_idx]` in the apply_variations
        // shader code.
        let id_map = crate::scene::transforms::compute_local_index_map(
            flame.extract_active_variations().into_keys(),
            registry,
        );

        // Inline all transforms
        let mut inlined = Vec::with_capacity(flame.transforms.len());
        let mut cumulative = Vec::with_capacity(flame.transforms.len());
        let mut total_weight = 0.0;

        for xform in &flame.transforms {
            // Convert variation weights to indexed form
            let mut var_weights = Vec::new();
            for (name, &weight) in &xform.variations {
                if weight.abs() > 1e-6 {
                    if let Some(&idx) = id_map.get(name) {
                        var_weights.push((idx, weight));
                    }
                }
            }
            var_weights.sort_by_key(|(idx, _)| *idx);

            // Convert variation params to indexed form
            let mut var_params = Vec::new();
            for (key, &value) in &xform.variation_params {
                // Key format: "variation_name.param_name"
                if let Some(dot_pos) = key.find('.') {
                    let var_name = &key[..dot_pos];
                    let param_name = &key[dot_pos + 1..];

                    if let Some(&var_idx) = id_map.get(var_name) {
                        if let Some(info) = registry.get(var_name) {
                            // Find param slot index
                            for (slot, param) in info.parameters.iter().enumerate() {
                                if param.name == param_name {
                                    var_params.push(((var_idx, slot as u32), value));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            var_params.sort_by_key(|((var_idx, slot), _)| (*var_idx, *slot));

            total_weight += xform.weight;
            cumulative.push(total_weight);

            inlined.push(InlinedTransform {
                a: xform.a,
                b: xform.b,
                c: xform.c,
                d: xform.d,
                e: xform.e,
                f: xform.f,
                g: xform.g,
                weight: xform.weight,
                color: xform.color,
                color_speed: xform.color_speed,
                opacity: xform.opacity,
                variation_weights: var_weights,
                variation_params: var_params,
            });
        }

        // Normalize cumulative weights to 0-1 range
        if total_weight > 0.0 {
            for w in &mut cumulative {
                *w /= total_weight;
            }
        }

        // Handle final transform if present
        if let Some(final_xform) = &flame.final_transform {
            let mut var_weights = Vec::new();
            for (name, &weight) in &final_xform.variations {
                if weight.abs() > 1e-6 {
                    if let Some(&idx) = id_map.get(name) {
                        var_weights.push((idx, weight));
                    }
                }
            }
            var_weights.sort_by_key(|(idx, _)| *idx);

            let mut var_params = Vec::new();
            for (key, &value) in &final_xform.variation_params {
                if let Some(dot_pos) = key.find('.') {
                    let var_name = &key[..dot_pos];
                    let param_name = &key[dot_pos + 1..];

                    if let Some(&var_idx) = id_map.get(var_name) {
                        if let Some(info) = registry.get(var_name) {
                            for (slot, param) in info.parameters.iter().enumerate() {
                                if param.name == param_name {
                                    var_params.push(((var_idx, slot as u32), value));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            var_params.sort_by_key(|((var_idx, slot), _)| (*var_idx, *slot));

            inlined.push(InlinedTransform {
                a: final_xform.a,
                b: final_xform.b,
                c: final_xform.c,
                d: final_xform.d,
                e: final_xform.e,
                f: final_xform.f,
                g: final_xform.g,
                weight: 0.0, // Final transform not selected by weight
                color: final_xform.color,
                color_speed: final_xform.color_speed,
                opacity: final_xform.opacity,
                variation_weights: var_weights,
                variation_params: var_params,
            });
        }

        Self {
            num_transforms,
            color_mode,
            has_final_transform: has_final,
            final_transform_index: final_idx,
            has_post_affine: flame.has_post_affine(),
            inlined_transforms: Some(inlined),
            cumulative_weights: Some(cumulative),
        }
    }
}

impl ShaderConstants {
    /// Generate WGSL const declarations
    pub fn to_wgsl(&self) -> String {
        let mut code = format!(
            "// Hard-coded shader constants (compiled at shader build time)\n\
             const NUM_TRANSFORMS: u32 = {}u;\n\
             const COLOR_MODE: u32 = {}u;\n\
             const HAS_FINAL_TRANSFORM: bool = {};\n\
             const FINAL_TRANSFORM_INDEX: u32 = {}u;\n\
             const HAS_POST_AFFINE: bool = {};\n",
            self.num_transforms,
            self.color_mode,
            self.has_final_transform,
            self.final_transform_index,
            self.has_post_affine,
        );

        // Generate cumulative weights for fast transform selection
        if let Some(ref cumulative) = self.cumulative_weights {
            code.push_str("\n// Precomputed cumulative weights (normalized 0-1)\n");
            code.push_str("const CUMULATIVE_WEIGHTS: array<f32, ");
            code.push_str(&cumulative.len().to_string());
            code.push_str("> = array<f32, ");
            code.push_str(&cumulative.len().to_string());
            code.push_str(">(");
            for (i, w) in cumulative.iter().enumerate() {
                if i > 0 {
                    code.push_str(", ");
                }
                code.push_str(&format!("{:.8}", w));
            }
            code.push_str(");\n");
            code.push_str("const USE_INLINED_WEIGHTS: bool = true;\n");
        } else {
            code.push_str("const USE_INLINED_WEIGHTS: bool = false;\n");
        }

        // Generate inlined transform data
        if let Some(ref transforms) = self.inlined_transforms {
            code.push_str("\n// Inlined transform data (eliminates buffer reads)\n");
            code.push_str("const USE_INLINED_TRANSFORMS: bool = true;\n\n");

            // Generate affine coefficients as struct array
            code.push_str("struct InlinedAffine {\n");
            code.push_str("    a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, g: f32,\n");
            code.push_str("    color: f32, color_speed: f32, opacity: f32,\n");
            code.push_str("}\n\n");

            code.push_str(&format!(
                "const INLINED_AFFINE: array<InlinedAffine, {}> = array<InlinedAffine, {}>(",
                transforms.len(), transforms.len()
            ));
            for (i, xform) in transforms.iter().enumerate() {
                if i > 0 {
                    code.push_str(",");
                }
                code.push_str(&format!(
                    "\n    InlinedAffine({:.8}, {:.8}, {:.8}, {:.8}, {:.8}, {:.8}, {:.8}, {:.8}, {:.8}, {:.8})",
                    xform.a, xform.b, xform.c, xform.d, xform.e, xform.f, xform.g,
                    xform.color, xform.color_speed, xform.opacity
                ));
            }
            code.push_str("\n);\n\n");

            // Generate variation weights lookup function
            // This allows the compiler to inline constant weights and eliminate dead code
            code.push_str("// Get inlined variation weight (enables dead code elimination)\n");
            code.push_str("fn get_inlined_var_weight(xform_id: u32, var_idx: u32) -> f32 {\n");
            code.push_str("    switch(xform_id) {\n");

            for (xform_idx, xform) in transforms.iter().enumerate() {
                code.push_str(&format!("        case {}u: {{\n", xform_idx));
                code.push_str("            switch(var_idx) {\n");
                for (var_idx, weight) in &xform.variation_weights {
                    code.push_str(&format!(
                        "                case {}u: {{ return {:.8}; }}\n",
                        var_idx, weight
                    ));
                }
                code.push_str("                default: { return 0.0; }\n");
                code.push_str("            }\n");
                code.push_str("        }\n");
            }
            code.push_str("        default: { return 0.0; }\n");
            code.push_str("    }\n");
            code.push_str("}\n\n");

            // Generate variation param lookup function
            code.push_str("// Get inlined variation parameter (eliminates buffer reads)\n");
            code.push_str("fn get_inlined_var_param(xform_id: u32, var_idx: u32, param_slot: u32) -> f32 {\n");
            code.push_str("    switch(xform_id) {\n");

            for (xform_idx, xform) in transforms.iter().enumerate() {
                if xform.variation_params.is_empty() {
                    continue;
                }
                code.push_str(&format!("        case {}u: {{\n", xform_idx));
                code.push_str("            switch(var_idx * 16u + param_slot) {\n");
                for ((var_idx, param_slot), value) in &xform.variation_params {
                    let combined_idx = var_idx * 16 + param_slot;
                    code.push_str(&format!(
                        "                case {}u: {{ return {:.8}; }}\n",
                        combined_idx, value
                    ));
                }
                code.push_str("                default: { return 0.0; }\n");
                code.push_str("            }\n");
                code.push_str("        }\n");
            }
            code.push_str("        default: { return 0.0; }\n");
            code.push_str("    }\n");
            code.push_str("}\n");
        } else {
            code.push_str("const USE_INLINED_TRANSFORMS: bool = false;\n");
        }

        code.push('\n');
        code
    }

    /// Check if constants use inlined transforms
    pub fn has_inlined_transforms(&self) -> bool {
        self.inlined_transforms.is_some()
    }
}

/// Builds WGSL shaders dynamically using named variations
pub struct ShaderBuilder {
    registry: VariationRegistry,
}

impl ShaderBuilder {
    pub fn new(registry: VariationRegistry) -> Self {
        Self { registry }
    }

    /// Build the per-flame active-variation list with LOCAL indices (`0..N`).
    /// The local map is shared across the GPU buffer populator and the shader
    /// builder so the slot in `xform.variations[idx]` and the `variation_id`
    /// passed to parameterized variation calls always match the buffer layout.
    ///
    /// `mode_filter`: if `Some(true)` for 2D, drops variations that aren't
    /// `Basic2D`/`Advanced2D`. This is purely a code-emission filter — the
    /// dropped variations still occupy their local slot in the buffer
    /// (harmless: nothing reads them in this shader).
    fn active_with_local_indices(
        &self,
        active_variations: &HashMap<String, f32>,
        render_3d: bool,
    ) -> Vec<(String, u32)> {
        use crate::variations::VariationCategory;
        let local_map = crate::scene::transforms::compute_local_index_map(
            active_variations.keys().cloned(),
            &self.registry,
        );
        // Iterate registry order so output is deterministic
        self.registry.names().iter()
            .filter_map(|name| {
                let local_idx = *local_map.get(name)?;
                if !render_3d {
                    let info = self.registry.get(name)?;
                    if !matches!(info.category, VariationCategory::Basic2D | VariationCategory::Advanced2D) {
                        return None;
                    }
                }
                Some((name.clone(), local_idx))
            })
            .collect()
    }

    /// Returns true if any variation in the active set writes to the
    /// iteration-local color register (has `writes_color: true`). Drives the
    /// `HAS_DC` template condition: the main loop's c_base/vc/Step3 lerp and
    /// the `vc` pointer parameter on `apply_variations` are emitted only when
    /// this is true. Flames without DC variations skip them entirely.
    fn has_dc_variation(&self, active_variations: &[(String, u32)]) -> bool {
        active_variations.iter().any(|(name, _)| {
            self.registry.get(name).is_some_and(|info| info.writes_color)
        })
    }

    /// Build the init compute shader for a flame's active variations.
    ///
    /// Returns `Some(wgsl)` if any active variation in the flame has a
    /// `wgsl_init` function; returns `None` otherwise (in which case no init
    /// dispatch is needed).
    ///
    /// The returned shader has a single bind group layout:
    ///   `@group(0) @binding(0) var<storage, read_write> variation_params`
    ///
    /// Dispatch with `ceil(pair_count / 64)` workgroups of size 64. Each
    /// thread handles one (xform_idx, init-bearing-variation) pair —
    /// reads user params from the buffer, runs the init function, writes
    /// derived values back into the same buffer at slots
    /// `local_idx * 16 + N..local_idx * 16 + N + M`.
    pub fn build_init_shader(
        &self,
        flame: &crate::scene::transforms::Flame,
        active_variations: &HashMap<String, f32>,
    ) -> Option<String> {
        use std::collections::HashSet;

        let local_map = crate::scene::transforms::compute_local_index_map(
            active_variations.keys().cloned(),
            &self.registry,
        );

        // Collect (xform_idx, var_name, local_idx) tuples for every transform
        // that uses a variation with `wgsl_init`. Order matters — pair_idx in
        // the dispatch is the index into this list.
        let mut pairs: Vec<(u32, String, u32)> = Vec::new();
        let mut emit_variation = |xform_idx: u32, xform: &crate::scene::transforms::Transform| {
            for (var_name, weight) in &xform.variations {
                if weight.abs() < 1e-6 {
                    continue;
                }
                let info = match self.registry.get(var_name) {
                    Some(i) => i,
                    None => continue,
                };
                if info.wgsl_source_init.is_none() {
                    continue;
                }
                let local_idx = match local_map.get(var_name) {
                    Some(&i) => i,
                    None => continue,
                };
                pairs.push((xform_idx, var_name.clone(), local_idx));
            }
        };
        for (i, xform) in flame.transforms.iter().enumerate() {
            emit_variation(i as u32, xform);
        }
        if let Some(final_xform) = &flame.final_transform {
            emit_variation(flame.transforms.len() as u32, final_xform);
        }

        if pairs.is_empty() {
            return None;
        }

        let pair_count = pairs.len();
        let mut shader = String::new();

        // 1. VariationParams struct + buffer binding (read_write).
        //    Mirror the layout used by the main shader's `variation_params`,
        //    but with read_write access mode.
        shader.push_str(
            "struct VariationParams {\n\
             \x20   params: array<f32, 1600>,\n\
             }\n\n\
             @group(0) @binding(0) var<storage, read_write> variation_params: array<VariationParams>;\n\n",
        );

        // 2. Emit each unique variation's init function (dedup by name).
        let mut emitted: HashSet<String> = HashSet::new();
        for (_xform_idx, var_name, _local_idx) in &pairs {
            if !emitted.insert(var_name.clone()) {
                continue;
            }
            if let Some(info) = self.registry.get(var_name) {
                if let Some(init_src) = &info.wgsl_source_init {
                    shader.push_str(init_src);
                    shader.push('\n');
                }
            }
        }

        // 3. Main entry point — switch on pair_idx, decode to (xform, var)
        //    and call the right init.
        shader.push_str(&format!(
            "const TOTAL_INIT_PAIRS: u32 = {}u;\n\n",
            pair_count
        ));
        shader.push_str(
            "@compute @workgroup_size(64)\n\
             fn init_main(@builtin(global_invocation_id) gid: vec3<u32>) {\n\
             \x20   let pair_idx = gid.x;\n\
             \x20   if (pair_idx >= TOTAL_INIT_PAIRS) { return; }\n\
             \x20   switch (pair_idx) {\n",
        );

        for (case_idx, (xform_idx, var_name, local_idx)) in pairs.iter().enumerate() {
            let info = match self.registry.get(var_name) {
                Some(i) => i,
                None => continue,
            };
            let n_user = info.parameters.len();
            let n_init = info.init_param_count;
            let base_slot = (*local_idx as usize) * 16;
            shader.push_str(&format!("        case {}u: {{\n", case_idx));
            // Read user params
            shader.push_str(&format!("            var user: array<f32, {}>;\n", n_user));
            for i in 0..n_user {
                shader.push_str(&format!(
                    "            user[{i}] = variation_params[{x}u].params[{slot}u];\n",
                    i = i,
                    x = xform_idx,
                    slot = base_slot + i,
                ));
            }
            // Call init
            shader.push_str(&format!(
                "            let derived = init_{name}(user);\n",
                name = var_name,
            ));
            // Write init params
            for i in 0..n_init {
                shader.push_str(&format!(
                    "            variation_params[{x}u].params[{slot}u] = derived[{i}];\n",
                    i = i,
                    x = xform_idx,
                    slot = base_slot + n_user + i,
                ));
            }
            shader.push_str("        }\n");
        }

        shader.push_str("        default: { /* unreachable */ }\n    }\n}\n");

        Some(shader)
    }

    /// Generate variation function code for ONLY active variations from embedded WGSL
    ///
    /// Only includes variation functions that are actually used in the current flame.
    /// This reduces shader size and compilation time significantly.
    fn generate_variation_code(&self, active_variations: &[(String, u32)], render_3d: bool) -> String {
        let mut code = String::new();

        // Generate code ONLY for active variations (not all variations in registry)
        // This is the key optimization - typical flames use 3-5 variations, not 80+
        for (name, _idx) in active_variations {
            if let Some(info) = self.registry.get(name) {
                if render_3d {
                    // For 3D mode, prefer wgsl_source_3d, fall back to wgsl_source
                    if let Some(source_3d) = &info.wgsl_source_3d {
                        code.push_str(source_3d);
                        code.push('\n');
                    } else if let Some(source) = &info.wgsl_source {
                        // 2D source as fallback
                        code.push_str(source);
                        code.push('\n');
                    }
                } else {
                    // For 2D mode, use wgsl_source only
                    if let Some(source) = &info.wgsl_source {
                        code.push_str(source);
                        code.push('\n');
                    }
                }
            }
        }

        code
    }

    /// Build trajectory shader from unified template
    ///
    /// This method uses the main_template.wgsl with conditional compilation
    /// to generate 2D/3D and simple/full variants from a single source file.
    ///
    /// Parameters:
    /// - `active_variations`: Map of active variation names to weights
    /// - `render_3d`: true for 3D mode (vec3), false for 2D mode (vec2)
    /// - `path_features_enabled`: true to include path tracking code
    /// - `xaos_enabled`: true to use xaos-weighted transform selection
    /// - `constants`: Hard-coded shader constants
    pub fn build_from_template(
        &self,
        active_variations: &HashMap<String, f32>,
        render_3d: bool,
        path_features_enabled: bool,
        xaos_enabled: bool,
        constants: &ShaderConstants,
    ) -> String {
        let active = self.active_with_local_indices(active_variations, render_3d);

        let mut shader = String::new();

        // 1. Hard-coded constants (must come first for use in later code)
        shader.push_str(&constants.to_wgsl());

        // 2. Header
        shader.push_str(include_str!("../shaders/core/header.wgsl"));
        shader.push('\n');

        // 3. RNG
        shader.push_str(include_str!("../shaders/core/rng.wgsl"));
        shader.push('\n');

        // 4. Affine transformations
        if render_3d {
            shader.push_str(include_str!("../shaders/core/affine_3d.wgsl"));
        } else {
            shader.push_str(include_str!("../shaders/core/affine.wgsl"));
        }
        shader.push('\n');

        // 5. Core variations from embedded VariationDef WGSL (only active ones)
        shader.push_str(&self.generate_variation_code(&active, render_3d));
        shader.push('\n');

        // Compute has_dc once: drives both the apply_variations signature
        // (with vs without `vc` param) and the HAS_DC template condition.
        let has_dc = self.has_dc_variation(&active);

        // 7. Generate apply_variations with fixed registry indices
        // Pass inlined transforms for dead code elimination if available
        if render_3d {
            shader.push_str(&self.build_apply_variations_3d(&active, constants.inlined_transforms.as_ref(), has_dc));
        } else {
            shader.push_str(&self.build_apply_variations_2d(&active, constants.inlined_transforms.as_ref(), has_dc));
        }
        shader.push('\n');

        // 8. Utilities
        shader.push_str(include_str!("../shaders/core/utilities.wgsl"));
        shader.push('\n');

        // 9. Path filter utilities (only needed when path features enabled)
        if path_features_enabled {
            shader.push_str(include_str!("../shaders/core/path_filter.wgsl"));
            shader.push('\n');
        }

        // 10. Main shader from template
        let template = include_str!("../shaders/core/main_template.wgsl");
        let mut processor = TemplateProcessor::new();
        processor.set("RENDER_3D", render_3d);
        processor.set("PATH_TRACKING", path_features_enabled);
        processor.set("XAOS_ENABLED", xaos_enabled);
        processor.set("HAS_DC", has_dc);
        shader.push_str(&processor.process(template));

        // DEBUG: Write shader to file for analysis (enabled via --dump-shader CLI flag)
        if should_dump_shader() {
            let filename = if render_3d { "debug_shader_3d.wgsl" } else { "debug_shader_2d.wgsl" };
            if let Err(e) = std::fs::write(filename, &shader) {
                log::error!("Failed to write debug shader: {}", e);
            } else {
                log::info!("Wrote shader to {} ({} bytes, {} lines)",
                    filename, shader.len(), shader.lines().count());
            }
        }

        shader
    }

    /// Build apply_variations function for 2D mode
    ///
    /// When `inlined_transforms` is provided, generates code with compile-time constant
    /// variation weights, enabling dead code elimination for unused variations per-transform.
    /// When `has_dc` is true, the function takes a `vc: ptr<function, f32>` parameter for
    /// direct-color variations to write to; when false, the parameter is omitted entirely
    /// (zero-cost path when no DC variation is in the active set).
    fn build_apply_variations_2d(
        &self,
        active_variations: &[(String, u32)],
        inlined_transforms: Option<&Vec<InlinedTransform>>,
        has_dc: bool,
    ) -> String {
        use crate::variations::VariationPhase;

        // When inlined, we generate per-transform specialized code
        let use_inlined = inlined_transforms.is_some();

        let signature = if has_dc {
            "fn apply_variations(xform: Transform, xform_id: u32, p: vec2<f32>, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {\n"
        } else {
            "fn apply_variations(xform: Transform, xform_id: u32, p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {\n"
        };
        let mut code = String::from(
            "// Apply all variations with Apophysis 4-phase execution model (XForm.pas:343-383)\n\
             // When has_dc=true, takes a `vc` pointer (the iteration-local color register\n\
             // Apophysis calls `vc`) so DC variations (writes_color: true) can write to it.\n\
             // When has_dc=false, the parameter is omitted — no DC variation in the active\n\
             // set means no inner call references vc, so it's pure overhead.\n",
        );
        code.push_str(signature);

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
                let mut params = String::new();

                // Variations with parameters get (xform_id, variation_id) for
                // get_param lookups. needs_transform variations also get both
                // so they can read transforms[xform_id].variations[variation_id]
                // (e.g., pre_rotate_x reads its own weight from the buffer).
                if !info.parameters.is_empty() || info.needs_transform {
                    params.push_str(&format!(", xform_id, {}u", idx));
                }
                if info.needs_rng {
                    params.push_str(", rng");
                }
                // DC variations get the iteration-local color register pointer.
                if info.writes_color {
                    params.push_str(", vc");
                }

                code.push_str(&format!(
                    "    // {}: {} (PRE)\n\
                     \x20   if (xform.variations[{}] != 0.0) {{\n\
                     \x20       temp = variation_{}(temp{});\n\
                     \x20   }}\n\n",
                    idx, info.display_name, idx, name, params
                ));
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
            let mut args = String::from("temp");
            if !info.parameters.is_empty() || info.needs_transform {
                args.push_str(&format!(", xform_id, {}u", idx));
            }
            if info.needs_rng {
                args.push_str(", rng");
            }
            if info.writes_color {
                args.push_str(", vc");
            }
            let call = format!("{}({})", info.wgsl_function, args);

            // Use inlined weights when available (enables dead code elimination)
            if use_inlined {
                code.push_str(&format!(
                    "    // {}: {} (NORMAL - INLINED)\n\
                     \x20   {{\n\
                     \x20       let w = get_inlined_var_weight(xform_id, {}u);\n\
                     \x20       if (w != 0.0) {{\n\
                     \x20           result += w * {};\n\
                     \x20       }}\n\
                     \x20   }}\n\n",
                    idx, info.display_name, idx, call
                ));
            } else {
                code.push_str(&format!(
                    "    // {}: {} (NORMAL)\n\
                     \x20   if (xform.variations[{}] != 0.0) {{\n\
                     \x20       result += xform.variations[{}] * {};\n\
                     \x20   }}\n\n",
                    idx, info.display_name, idx, idx, call
                ));
            }
        }

        // PHASE 4: Post-variations - directly modify output coordinates (rare in 2D)
        if !post_variations.is_empty() {
            code.push_str("    // Phase 4: Post-variations (modify output)\n\n");

            for (_name, idx, info) in &post_variations {
                // Post-variations directly modify result (NOT weighted sum!)
                let mut params = String::from("result");

                // has_params || needs_transform → pass (xform_id, variation_id).
                // Pure no-param no-needs_transform variations (e.g. flatten 2D
                // stub) get just `result`.
                if !info.parameters.is_empty() || info.needs_transform {
                    params.push_str(&format!(", xform_id, {}u", idx));
                }
                if info.needs_rng {
                    params.push_str(", rng");
                }
                if info.writes_color {
                    params.push_str(", vc");
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
    ///
    /// When `inlined_transforms` is provided, generates code with compile-time constant
    /// variation weights, enabling dead code elimination for unused variations per-transform.
    fn build_apply_variations_3d(
        &self,
        active_variations: &[(String, u32)],
        inlined_transforms: Option<&Vec<InlinedTransform>>,
        has_dc: bool,
    ) -> String {
        use crate::variations::VariationPhase;

        // When inlined, we generate per-transform specialized code
        let use_inlined = inlined_transforms.is_some();

        let signature = if has_dc {
            "fn apply_variations(xform: Transform, xform_id: u32, p: vec3<f32>, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {\n"
        } else {
            "fn apply_variations(xform: Transform, xform_id: u32, p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {\n"
        };
        let mut code = String::from(
            "// Apply all variations with Apophysis 4-phase execution model (XForm.pas:343-383)\n\
             // See 2D variant for the meaning of the `vc` pointer.\n",
        );
        code.push_str(signature);

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
                let mut params = String::new();

                if !info.parameters.is_empty() || info.needs_transform {
                    params.push_str(&format!(", xform_id, {}u", idx));
                }
                if info.needs_rng {
                    params.push_str(", rng");
                }
                if info.writes_color {
                    params.push_str(", vc");
                }

                code.push_str(&format!(
                    "    // {}: {} (PRE)\n\
                     \x20   if (xform.variations[{}] != 0.0) {{\n\
                     \x20       temp = variation_{}(temp{});\n\
                     \x20   }}\n\n",
                    idx, info.display_name, idx, name, params
                ));
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
                    if use_inlined {
                        code.push_str(&format!(
                            "    // {}: {} (NORMAL - Z-only - INLINED)\n\
                             \x20   {{\n\
                             \x20       let w = get_inlined_var_weight(xform_id, {}u);\n\
                             \x20       if (w != 0.0) {{\n\
                             \x20           let r = length(temp.xy);\n\
                             \x20           result.z += w * r;\n\
                             \x20       }}\n\
                             \x20   }}\n\n",
                            idx, info.display_name, idx
                        ));
                    } else {
                        code.push_str(&format!(
                            "    // {}: {} (NORMAL - Z-only)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       let r = length(temp.xy);\n\
                             \x20       result.z += xform.variations[{}] * r;\n\
                             \x20   }}\n\n",
                            idx, info.display_name, idx, idx
                        ));
                    }
                }
                "zscale" => {
                    if use_inlined {
                        code.push_str(&format!(
                            "    // {}: {} (NORMAL - Z-only - INLINED)\n\
                             \x20   {{\n\
                             \x20       let w = get_inlined_var_weight(xform_id, {}u);\n\
                             \x20       if (w != 0.0) {{\n\
                             \x20           result.z += w * temp.z;\n\
                             \x20       }}\n\
                             \x20   }}\n\n",
                            idx, info.display_name, idx
                        ));
                    } else {
                        code.push_str(&format!(
                            "    // {}: {} (NORMAL - Z-only)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result.z += xform.variations[{}] * temp.z;\n\
                             \x20   }}\n\n",
                            idx, info.display_name, idx, idx
                        ));
                    }
                }
                "ztranslate" => {
                    if use_inlined {
                        code.push_str(&format!(
                            "    // {}: {} (NORMAL - Z-only - INLINED)\n\
                             \x20   {{\n\
                             \x20       let w = get_inlined_var_weight(xform_id, {}u);\n\
                             \x20       if (w != 0.0) {{\n\
                             \x20           result.z += w;\n\
                             \x20       }}\n\
                             \x20   }}\n\n",
                            idx, info.display_name, idx
                        ));
                    } else {
                        code.push_str(&format!(
                            "    // {}: {} (NORMAL - Z-only)\n\
                             \x20   if (xform.variations[{}] != 0.0) {{\n\
                             \x20       result.z += xform.variations[{}];\n\
                             \x20   }}\n\n",
                            idx, info.display_name, idx, idx
                        ));
                    }
                }
                _ => {
                    // Standard variation with function call
                    let mut args = String::from("temp");
                    if !info.parameters.is_empty() || info.needs_transform {
                        args.push_str(&format!(", xform_id, {}u", idx));
                    }
                    if info.needs_rng {
                        args.push_str(", rng");
                    }
                    if info.writes_color {
                        args.push_str(", vc");
                    }
                    let call = format!("{}({})", info.wgsl_function, args);

                    // Use inlined weights when available
                    if use_inlined {
                        code.push_str(&format!(
                            "    // {}: {} (NORMAL - INLINED)\n\
                             \x20   {{\n\
                             \x20       let w = get_inlined_var_weight(xform_id, {}u);\n\
                             \x20       if (w != 0.0) {{\n\
                             \x20           result += w * {};\n\
                             \x20       }}\n\
                             \x20   }}\n\n",
                            idx, info.display_name, idx, call
                        ));
                    } else {
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
                        let mut params = String::from("result");

                        if !info.parameters.is_empty() || info.needs_transform {
                            params.push_str(&format!(", xform_id, {}u", idx));
                        }
                        if info.needs_rng {
                            params.push_str(", rng");
                        }
                        if info.writes_color {
                            params.push_str(", vc");
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
        let active_2d = self.active_with_local_indices(active_variations, false);

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

        // 4. Core variations (2D) from embedded VariationDef WGSL (only active ones)
        shader.push_str(&self.generate_variation_code(&active_2d, false));
        shader.push('\n');

        // 5. Generate apply_variations (no inlining for tiled shaders)
        let has_dc = self.has_dc_variation(&active_2d);
        shader.push_str(&self.build_apply_variations_2d(&active_2d, None, has_dc));
        shader.push('\n');

        // 7. Tiled utilities (uses full_width/full_height)
        shader.push_str(include_str!("../shaders/core/utilities_tiled.wgsl"));
        shader.push('\n');

        // 8. Tiled main — routed through TemplateProcessor with HAS_DC gate.
        let mut processor = TemplateProcessor::new();
        processor.set("HAS_DC", has_dc);
        shader.push_str(&processor.process(include_str!("../shaders/core/main_2d_tiled.wgsl")));

        shader
    }

    /// Build 3D TILED trajectory shader with active variations
    pub fn build_trajectory_3d_tiled(&self, active_variations: &HashMap<String, f32>) -> String {
        let active_3d = self.active_with_local_indices(active_variations, true);

        let mut shader = String::new();

        // 1. Tiled header
        shader.push_str(include_str!("../shaders/core/header_tiled.wgsl"));
        shader.push('\n');

        // 2. RNG
        shader.push_str(include_str!("../shaders/core/rng.wgsl"));
        shader.push('\n');

        // 3. Affine (3D)
        shader.push_str(include_str!("../shaders/core/affine_3d.wgsl"));
        shader.push('\n');

        // 4. Core variations (3D) from embedded VariationDef WGSL (only active ones)
        shader.push_str(&self.generate_variation_code(&active_3d, true));
        shader.push('\n');

        // 5. Generate apply_variations (no inlining for tiled shaders)
        let has_dc = self.has_dc_variation(&active_3d);
        shader.push_str(&self.build_apply_variations_3d(&active_3d, None, has_dc));
        shader.push('\n');

        // 6. Tiled utilities
        shader.push_str(include_str!("../shaders/core/utilities_tiled.wgsl"));
        shader.push('\n');

        // 7. Tiled main — routed through TemplateProcessor with HAS_DC gate.
        let mut processor = TemplateProcessor::new();
        processor.set("HAS_DC", has_dc);
        shader.push_str(&processor.process(include_str!("../shaders/core/main_3d_tiled.wgsl")));

        shader
    }

    /// Build the EXPORT shader — outputs samples to a buffer for CPU
    /// histogram accumulation. Uses the 3D code path regardless of the
    /// flame's render_mode: the 2D vs 3D distinction only matters for how Z
    /// is displayed, not for sample collection. Configs with 3D variations
    /// (e.g. flatten, hemisphere) need the 3D path even when render_mode is
    /// 2D, which is why there's no separate 2D export shader.
    pub fn build_export(&self, active_variations: &HashMap<String, f32>) -> String {
        let active = self.active_with_local_indices(active_variations, true);

        let mut shader = String::new();

        // 1. Export header
        shader.push_str(include_str!("../shaders/core/header_export.wgsl"));
        shader.push('\n');

        // 2. RNG
        shader.push_str(include_str!("../shaders/core/rng.wgsl"));
        shader.push('\n');

        // 3. Add NUM_TRANSFORMS constant (required by utilities.wgsl even though export uses runtime version)
        // Set to 32 (max transforms) - this is only used by select_transform_const which export doesn't call
        shader.push_str("const NUM_TRANSFORMS: u32 = 32u;\n");
        shader.push('\n');

        // 4. Standard utilities (MUST come before variations - defines get_param)
        shader.push_str(include_str!("../shaders/core/utilities.wgsl"));
        shader.push('\n');

        // 5. Affine (3D)
        shader.push_str(include_str!("../shaders/core/affine_3d.wgsl"));
        shader.push('\n');

        // 6. Core variations (3D) from embedded VariationDef WGSL (only active ones)
        shader.push_str(&self.generate_variation_code(&active, true));
        shader.push('\n');

        // 7. Generate apply_variations (no inlining for export shaders)
        let has_dc = self.has_dc_variation(&active);
        shader.push_str(&self.build_apply_variations_3d(&active, None, has_dc));
        shader.push('\n');

        // 8. Export main — routed through TemplateProcessor with HAS_DC gate.
        let mut processor = TemplateProcessor::new();
        processor.set("HAS_DC", has_dc);
        shader.push_str(&processor.process(include_str!("../shaders/core/main_export.wgsl")));

        shader
    }
}
