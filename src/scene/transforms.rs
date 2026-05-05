use std::collections::{HashMap, BTreeMap};
use serde::{Deserialize, Serialize, Deserializer, Serializer};
use serde::de::{self, Visitor, MapAccess};
use crate::variations::VariationRegistry;

/// Maximum number of variations that can be active in a single flame.
/// This is the cap on the GPU-side `xform.variations` array and the
/// `variation_params` array layout (100 variations × 12 params = 1200 floats).
/// The variation registry itself is unbounded — this only limits the
/// per-flame active set.
pub const MAX_VARIATIONS_PER_FLAME: usize = 100;

/// Compute a per-flame local index map for the given active variation set.
///
/// Active variation names are sorted by their order in the registry (which is
/// append-only, so this is stable across runs and across registry growth) and
/// assigned sequential local indices `0..N`. If the active set exceeds
/// `MAX_VARIATIONS_PER_FLAME`, a warning is logged and the overflow is dropped.
///
/// Both the shader builder and the GPU buffer populator must use the same
/// mapping to keep buffer slots and shader code in agreement.
pub fn compute_local_index_map<I, S>(
    active_names: I,
    registry: &VariationRegistry,
) -> HashMap<String, u32>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let active_set: std::collections::HashSet<String> =
        active_names.into_iter().map(|s| s.as_ref().to_string()).collect();
    let mut active_in_order: Vec<String> = registry
        .names()
        .iter()
        .filter(|name| active_set.contains(*name))
        .cloned()
        .collect();
    if active_in_order.len() > MAX_VARIATIONS_PER_FLAME {
        log::warn!(
            "Flame has {} active variations; truncating to {}. Dropped: {:?}",
            active_in_order.len(),
            MAX_VARIATIONS_PER_FLAME,
            &active_in_order[MAX_VARIATIONS_PER_FLAME..],
        );
        active_in_order.truncate(MAX_VARIATIONS_PER_FLAME);
    }
    active_in_order
        .into_iter()
        .enumerate()
        .map(|(local, name)| (name, local as u32))
        .collect()
}

/// One entry in a flame's packed parameter layout: a variation, its
/// per-flame local index, the offset where its slots start in the
/// packed buffer, and how many slots it owns.
///
/// Returned by [`compute_packed_layout`] in local-index order, so the
/// total slot count of a flame is `entries.last().map(|e| e.offset +
/// e.slot_count).unwrap_or(0)`.
#[derive(Debug, Clone)]
pub struct PackedParamEntry {
    pub name: String,
    pub local_idx: u32,
    pub offset: u32,
    pub slot_count: u32,
}

/// Compute the packed parameter layout for a flame's active variation set.
///
/// Walks the local index map in order of `local_idx` (which matches
/// registry order, see [`compute_local_index_map`]) and assigns each
/// variation a contiguous slot range in the packed buffer:
///
/// ```text
///   variation A (local_idx=0, slot_count=3): offset 0, slots [0..3)
///   variation B (local_idx=1, slot_count=8): offset 3, slots [3..11)
///   variation C (local_idx=2, slot_count=2): offset 11, slots [11..13)
/// ```
///
/// Both the shader builder (for its generated `get_param` switch) and
/// the host packer ([`crate::gpu::buffers::GpuVariationParams`]) must
/// use this layout consistently — they're keyed by local_idx through
/// the same registry-order assignment.
///
/// Variations not found in the registry are skipped (this is unusual —
/// it would mean the flame references a variation that's been
/// unregistered).
pub fn compute_packed_layout(
    local_map: &HashMap<String, u32>,
    registry: &VariationRegistry,
) -> Vec<PackedParamEntry> {
    let mut entries: Vec<(&String, u32)> =
        local_map.iter().map(|(n, &i)| (n, i)).collect();
    entries.sort_by_key(|&(_, i)| i);

    let mut out = Vec::with_capacity(entries.len());
    let mut cursor: u32 = 0;
    for (name, local_idx) in entries {
        let slot_count = match registry.get(name) {
            Some(info) => info.slot_count() as u32,
            None => continue,
        };
        out.push(PackedParamEntry {
            name: name.clone(),
            local_idx,
            offset: cursor,
            slot_count,
        });
        cursor += slot_count;
    }
    out
}

/// Total number of slots needed to pack a flame's active variations.
///
/// Convenience wrapper that returns just the cursor value after walking
/// [`compute_packed_layout`].
pub fn total_packed_slots(
    local_map: &HashMap<String, u32>,
    registry: &VariationRegistry,
) -> u32 {
    compute_packed_layout(local_map, registry)
        .last()
        .map(|e| e.offset + e.slot_count)
        .unwrap_or(0)
}

/// One entry in a flame's per-thread state layout. Records which (xform,
/// variation) pair this entry belongs to, the offset in the flame's
/// `thread_state` array where its slots start, and how many slots it owns.
///
/// Returned by [`compute_state_layout`]. Unlike
/// [`PackedParamEntry`], state is keyed on `(xform_idx,
/// variation_local_id)` rather than just `variation_local_id` — the
/// same variation in different transforms gets independent state.
///
/// See [`docs/projects/intra-iteration-state-and-accum.md`](../../../docs/projects/intra-iteration-state-and-accum.md).
#[derive(Debug, Clone)]
pub struct PackedStateEntry {
    pub xform_idx: u32,
    pub variation_local_id: u32,
    pub variation_name: String,
    pub offset: u32,
    pub state_count: u32,
}

/// Soft cap on the total `var<private> thread_state` allocation per flame.
/// 1024 f32 = 4 KB per thread, well within the typical 32 KB per-thread
/// stack on desktop / mobile GPUs. Bumped here if we encounter flames that
/// legitimately need more.
pub const MAX_STATE_SLOTS_PER_FLAME: u32 = 1024;

/// Walk a flame's active variations and assign each `(xform_idx,
/// variation_local_id)` pair with `state_count > 0` a contiguous offset
/// in the per-thread state array.
///
/// Walk order:
///   1. Each transform in declaration order (`flame.transforms`).
///   2. The final transform last (if present), at index `transforms.len()`.
///   3. Within each transform, active variations sorted by local_idx so
///      the layout matches the shader builder's emit order.
///
/// Variations not in the active set, with weight ≈ 0, or with
/// `state_count == 0` are skipped.
pub fn compute_state_layout(
    flame: &Flame,
    local_map: &HashMap<String, u32>,
    registry: &VariationRegistry,
) -> Vec<PackedStateEntry> {
    let mut out: Vec<PackedStateEntry> = Vec::new();
    let mut cursor: u32 = 0;

    let mut emit_xform = |xform_idx: u32, xform: &Transform, cursor: &mut u32| {
        let mut active: Vec<(&String, u32)> = xform
            .variations
            .iter()
            .filter(|(_, &w)| w.abs() > 1e-6)
            .filter_map(|(name, _)| local_map.get(name).map(|&id| (name, id)))
            .collect();
        active.sort_by_key(|&(_, id)| id);
        for (name, local_id) in active {
            let info = match registry.get(name) {
                Some(i) => i,
                None => continue,
            };
            if info.state_count == 0 {
                continue;
            }
            let state_count = info.state_count as u32;
            out.push(PackedStateEntry {
                xform_idx,
                variation_local_id: local_id,
                variation_name: name.clone(),
                offset: *cursor,
                state_count,
            });
            *cursor += state_count;
        }
    };

    for (idx, xform) in flame.transforms.iter().enumerate() {
        emit_xform(idx as u32, xform, &mut cursor);
    }
    if let Some(ref final_xform) = flame.final_transform {
        emit_xform(flame.transforms.len() as u32, final_xform, &mut cursor);
    }

    if cursor > MAX_STATE_SLOTS_PER_FLAME {
        log::warn!(
            "Flame '{}' needs {} state slots; soft cap is {}. Consider raising MAX_STATE_SLOTS_PER_FLAME.",
            flame.name,
            cursor,
            MAX_STATE_SLOTS_PER_FLAME,
        );
    }

    out
}

/// Total number of state slots needed for a flame's active variations.
/// Returns 0 if no active variation declares state.
pub fn total_state_slots(
    flame: &Flame,
    local_map: &HashMap<String, u32>,
    registry: &VariationRegistry,
) -> u32 {
    compute_state_layout(flame, local_map, registry)
        .last()
        .map(|e| e.offset + e.state_count)
        .unwrap_or(0)
}

/// IFS Transform with named variations (V2)
///
/// This struct is used for both regular transforms AND the final transform.
/// When used as the final transform, only these fields are used:
/// - Affine matrix (a, b, c, d, e, f, g)
/// - Variations and variation_params
///
/// The following fields are IGNORED for final transforms (color is computed
/// during the iteration loop before the final transform is applied):
/// - weight (final transform is always applied, not selected by probability)
/// - color (final transform doesn't affect color index)
/// - color_speed (final transform doesn't blend colors)
/// - opacity (final transform doesn't affect visibility)
#[derive(Debug, Clone)]
pub struct Transform {
    // Affine transformation matrix: x' = ax + by + e, y' = cx + dy + f
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,

    /// Z offset for 3D mode (z' = z + g)
    pub g: f32,

    /// Probability weight for selecting this transform.
    /// NOTE: Ignored for final transforms.
    pub weight: f32,

    /// Weights for each variation function (named)
    pub variations: HashMap<String, f32>,

    /// Variation parameters (key format: "variation_name.param_name")
    /// Example: "julian.power" -> 3.0
    pub variation_params: HashMap<String, f32>,

    /// Color palette position (0.0 to 1.0)
    /// Represents position in the palette for color coordinate evolution.
    /// NOTE: Ignored for final transforms.
    pub color: f32,

    /// Color speed / symmetry (-1.0 to 1.0, Apophysis compatibility)
    /// -1.0 = full transform color replacement
    ///  0.0 = 50/50 blend
    ///  1.0 = full inheritance (transform has no color influence)
    /// NOTE: Ignored for final transforms.
    pub color_speed: f32,

    /// Opacity / visibility (0.0 to 1.0, Apophysis compatibility)
    /// Controls probability of plotting points from this transform
    /// 1.0 = always plot (default), 0.0 = never plot (invisible)
    /// NOTE: Ignored for final transforms.
    pub opacity: f32,

    /// Direct-color blend strength (0.0 to 1.0, Apophysis `pluginColor`).
    /// 0.0 = standard color evolution; 1.0 = direct-color variations fully
    /// override the iteration color. No effect when no direct-color
    /// variations are active in the flame. Default 0.0.
    pub direct_color: f32,

    // Post-affine transformation matrix (optional, applied after variations)
    // Same formula as pre-affine: x' = ax + by + e, y' = cx + dy + f, z' = z + g
    // When disabled, post-affine is skipped entirely (zero shader cost).
    /// Whether post-affine is enabled for this transform
    pub post_affine_enabled: bool,
    /// Post-affine matrix coefficient a (default: 1.0 = identity)
    pub post_a: f32,
    /// Post-affine matrix coefficient b (default: 0.0 = identity)
    pub post_b: f32,
    /// Post-affine matrix coefficient c (default: 0.0 = identity)
    pub post_c: f32,
    /// Post-affine matrix coefficient d (default: 1.0 = identity)
    pub post_d: f32,
    /// Post-affine translation X (default: 0.0 = identity)
    pub post_e: f32,
    /// Post-affine translation Y (default: 0.0 = identity)
    pub post_f: f32,
    /// Post-affine Z offset for 3D mode (default: 0.0 = identity)
    pub post_g: f32,

    /// Indexes into `flame.linked_transforms` — Linked transforms that
    /// run sequentially after this normal transform's variations.
    /// Linked transforms are part of dynamics: their output feeds the
    /// next iteration. Empty for transforms in the linked/final pools
    /// themselves. See `docs/projects/per-transform-linked-and-final.md`.
    pub linked_attachments: Vec<usize>,

    /// Indexes into `flame.final_transforms` — Final transforms that
    /// run sequentially after the Linked chain to produce the plotted
    /// point. Output is NOT fed forward (filter only). Empty for
    /// transforms in the linked/final pools themselves.
    pub final_attachments: Vec<usize>,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
            g: 0.0,
            weight: 1.0,
            variations: HashMap::new(),
            variation_params: HashMap::new(),
            color: 0.5,        // Mid-palette position (neutral default)
            color_speed: 0.0,  // Apophysis default: 50/50 blend
            opacity: 1.0,      // Apophysis default: always visible
            direct_color: 0.0, // Apophysis default: no direct-color blending
            post_affine_enabled: false,
            post_a: 1.0,
            post_b: 0.0,
            post_c: 0.0,
            post_d: 1.0,
            post_e: 0.0,
            post_f: 0.0,
            post_g: 0.0,
            linked_attachments: Vec::new(),
            final_attachments: Vec::new(),
        }
    }
}

impl Transform {
    /// Create a new transform with identity affine matrix
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a variation weight by name
    pub fn set_variation(&mut self, name: &str, weight: f32) {
        // Always insert/update the weight - don't auto-remove at zero
        // This allows variations to remain visible in UI at weight 0
        // Use remove_variation() to explicitly remove a variation
        self.variations.insert(name.to_string(), weight);
    }

    /// Remove a variation from this transform
    pub fn remove_variation(&mut self, name: &str) {
        self.variations.remove(name);
    }

    /// Get a variation weight by name
    pub fn get_variation(&self, name: &str) -> f32 {
        self.variations.get(name).copied().unwrap_or(0.0)
    }

    /// Get all active variation names
    pub fn active_variations(&self) -> Vec<String> {
        self.variations.keys().cloned().collect()
    }

    // === VARIATION PARAMETER METHODS ===

    /// Set a parameter for a specific variation
    /// Key format: "variation_name.param_name" (e.g., "julian.power")
    pub fn set_variation_param(&mut self, variation: &str, param: &str, value: f32) {
        let key = format!("{}.{}", variation, param);
        self.variation_params.insert(key, value);
    }

    /// Get a parameter value for a specific variation
    /// Returns None if not set
    pub fn get_variation_param(&self, variation: &str, param: &str) -> Option<f32> {
        let key = format!("{}.{}", variation, param);
        self.variation_params.get(&key).copied()
    }

    /// Get a parameter value with fallback to default from registry
    pub fn get_variation_param_or_default(
        &self,
        variation: &str,
        param: &str,
        registry: &VariationRegistry,
    ) -> f32 {
        self.get_variation_param(variation, param)
            .or_else(|| {
                registry.get(variation)
                    .and_then(|info| info.get_param_default(param))
            })
            .unwrap_or(0.0)
    }

    /// Convert from legacy array format to HashMap
    pub fn from_array(
        array: &[f32],
        registry: &VariationRegistry,
    ) -> HashMap<String, f32> {
        let mut map = HashMap::new();
        let names = registry.names();

        for (i, &weight) in array.iter().enumerate() {
            if weight.abs() > 1e-6 {
                if let Some(name) = names.get(i) {
                    map.insert(name.clone(), weight);
                }
            }
        }

        map
    }

    /// Convert to GPU array format with runtime ID mapping
    pub fn to_gpu_array(
        &self,
        id_map: &HashMap<String, u32>,
        max_variations: usize,
    ) -> Vec<f32> {
        let mut array = vec![0.0; max_variations];

        for (name, &weight) in &self.variations {
            if let Some(&id) = id_map.get(name) {
                if (id as usize) < max_variations {
                    array[id as usize] = weight;
                }
            }
        }

        array
    }

    // === TRIANGLE EDITOR METHODS ===

    /// Convert affine coefficients to triangle representation (O, X, Y points)
    /// Returns (Origin, X-axis endpoint, Y-axis endpoint)
    ///
    /// Note: Apophysis uses Y-down coordinate system for triangle display.
    /// This matches Apophysis behavior by negating f, b, and c appropriately.
    pub fn to_triangle(&self) -> ([f32; 2], [f32; 2], [f32; 2]) {
        let o = [self.e, -self.f];
        let x = [self.e + self.a, -self.f - self.b];
        let y = [self.e - self.c, -self.f + self.d];
        (o, x, y)
    }

    /// Update affine coefficients from triangle representation
    /// Takes (Origin, X-axis endpoint, Y-axis endpoint)
    ///
    /// Note: Inverse of to_triangle(), accounts for Apophysis Y-down coordinate system.
    pub fn from_triangle(&mut self, o: [f32; 2], x: [f32; 2], y: [f32; 2]) {
        self.a = x[0] - o[0];
        self.b = -(x[1] - o[1]);
        self.c = -(y[0] - o[0]);
        self.d = y[1] - o[1];
        self.e = o[0];
        self.f = -o[1];
    }

    /// Convert affine coefficients to triangle using Apophysis sign convention
    /// Matches Apophysis triangle editor exactly
    pub fn to_triangle_apophysis(&self) -> ([f32; 2], [f32; 2], [f32; 2]) {
        // Apophysis displays b, c, f with opposite sign from our internal representation
        let display_f = -self.f;

        // Apophysis formulas (verified to match exactly):
        // O = (e, -f)
        // X = (e + a, -f - c)
        // Y = (e - b, -f + d)
        let o = [self.e, display_f];
        let x = [self.e + self.a, display_f - self.c];
        let y = [self.e - self.b, display_f + self.d];
        (o, x, y)
    }

    /// Update affine coefficients from triangle using Apophysis sign convention
    /// Inverse of to_triangle_apophysis()
    pub fn from_triangle_apophysis(&mut self, o: [f32; 2], x: [f32; 2], y: [f32; 2]) {
        // Inverse of:
        // O = (e, -f)
        // X = (e + a, -f - c)
        // Y = (e - b, -f + d)
        //
        // Solve for coefficients:
        // e = O[0]
        // f = -O[1]
        // a = X[0] - O[0]
        // c = O[1] - X[1]  (since X[1] = O[1] - c)
        // b = O[0] - Y[0]  (since Y[0] = O[0] - b)
        // d = Y[1] - O[1]

        self.e = o[0];
        self.f = -o[1];
        self.a = x[0] - o[0];
        self.c = o[1] - x[1];
        self.b = o[0] - y[0];
        self.d = y[1] - o[1];
    }

    // === POST-AFFINE TRIANGLE EDITOR METHODS ===

    /// Convert post-affine coefficients to triangle using Apophysis sign convention
    pub fn post_to_triangle_apophysis(&self) -> ([f32; 2], [f32; 2], [f32; 2]) {
        let display_f = -self.post_f;
        let o = [self.post_e, display_f];
        let x = [self.post_e + self.post_a, display_f - self.post_c];
        let y = [self.post_e - self.post_b, display_f + self.post_d];
        (o, x, y)
    }

    /// Update post-affine coefficients from triangle using Apophysis sign convention
    pub fn post_from_triangle_apophysis(&mut self, o: [f32; 2], x: [f32; 2], y: [f32; 2]) {
        self.post_e = o[0];
        self.post_f = -o[1];
        self.post_a = x[0] - o[0];
        self.post_c = o[1] - x[1];
        self.post_b = o[0] - y[0];
        self.post_d = y[1] - o[1];
    }

    /// Reset post-affine to identity (no-op transform)
    pub fn reset_post_affine_to_identity(&mut self) {
        self.post_a = 1.0;
        self.post_b = 0.0;
        self.post_c = 0.0;
        self.post_d = 1.0;
        self.post_e = 0.0;
        self.post_f = 0.0;
        self.post_g = 0.0;
    }

    /// Reset transform to identity (unit triangle at origin)
    pub fn reset_to_identity(&mut self) {
        self.a = 1.0;
        self.b = 0.0;
        self.c = 0.0;
        self.d = 1.0;
        self.e = 0.0;
        self.f = 0.0;
    }

    // === TRANSFORM OPERATIONS (for animation) ===

    /// Get the origin X position (translation component)
    pub fn origin_x(&self) -> f32 {
        self.e
    }

    /// Get the origin Y position (translation component, Apophysis convention)
    pub fn origin_y(&self) -> f32 {
        -self.f
    }

    /// Set the origin X position (translation component)
    pub fn set_origin_x(&mut self, x: f32) {
        // Get current triangle
        let (mut o, mut x_pt, mut y_pt) = self.to_triangle_apophysis();
        let dx = x - o[0];
        // Translate all points
        o[0] = x;
        x_pt[0] += dx;
        y_pt[0] += dx;
        self.from_triangle_apophysis(o, x_pt, y_pt);
    }

    /// Set the origin Y position (translation component, Apophysis convention)
    pub fn set_origin_y(&mut self, y: f32) {
        // Get current triangle
        let (mut o, mut x_pt, mut y_pt) = self.to_triangle_apophysis();
        let dy = y - o[1];
        // Translate all points
        o[1] = y;
        x_pt[1] += dy;
        y_pt[1] += dy;
        self.from_triangle_apophysis(o, x_pt, y_pt);
    }

    /// Get the rotation angle in radians (from the X-axis arm)
    pub fn rotation(&self) -> f32 {
        let (o, x_pt, _) = self.to_triangle_apophysis();
        let dx = x_pt[0] - o[0];
        let dy = x_pt[1] - o[1];
        dy.atan2(dx)
    }

    /// Set the rotation angle in radians (rotates around origin, preserving scale)
    pub fn set_rotation(&mut self, angle: f32) {
        let (o, x_pt, y_pt) = self.to_triangle_apophysis();

        // Get current vectors from origin
        let x_vec = [x_pt[0] - o[0], x_pt[1] - o[1]];
        let y_vec = [y_pt[0] - o[0], y_pt[1] - o[1]];

        // Get current lengths (scales)
        let x_len = (x_vec[0] * x_vec[0] + x_vec[1] * x_vec[1]).sqrt();
        let y_len = (y_vec[0] * y_vec[0] + y_vec[1] * y_vec[1]).sqrt();

        // Get current angle between X and Y arms (to preserve shear)
        let current_x_angle = x_vec[1].atan2(x_vec[0]);
        let current_y_angle = y_vec[1].atan2(y_vec[0]);
        let angle_diff = current_y_angle - current_x_angle;

        // New X arm at the target angle
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let new_x = [o[0] + x_len * cos_a, o[1] + x_len * sin_a];

        // New Y arm at target angle + preserved angle difference
        let y_angle = angle + angle_diff;
        let new_y = [o[0] + y_len * y_angle.cos(), o[1] + y_len * y_angle.sin()];

        self.from_triangle_apophysis(o, new_x, new_y);
    }

    /// Get the uniform scale factor (average of X and Y arm lengths)
    pub fn scale(&self) -> f32 {
        let (o, x_pt, y_pt) = self.to_triangle_apophysis();
        let x_vec = [x_pt[0] - o[0], x_pt[1] - o[1]];
        let y_vec = [y_pt[0] - o[0], y_pt[1] - o[1]];
        let x_len = (x_vec[0] * x_vec[0] + x_vec[1] * x_vec[1]).sqrt();
        let y_len = (y_vec[0] * y_vec[0] + y_vec[1] * y_vec[1]).sqrt();
        (x_len + y_len) / 2.0
    }

    /// Set uniform scale (scales both arms equally, preserving rotation)
    pub fn set_scale(&mut self, scale: f32) {
        let (o, x_pt, y_pt) = self.to_triangle_apophysis();

        // Get current vectors from origin
        let x_vec = [x_pt[0] - o[0], x_pt[1] - o[1]];
        let y_vec = [y_pt[0] - o[0], y_pt[1] - o[1]];

        // Get current lengths
        let x_len = (x_vec[0] * x_vec[0] + x_vec[1] * x_vec[1]).sqrt();
        let y_len = (y_vec[0] * y_vec[0] + y_vec[1] * y_vec[1]).sqrt();

        // Avoid division by zero
        if x_len < 1e-6 || y_len < 1e-6 {
            return;
        }

        // Scale both arms to the target scale
        let x_scale = scale / x_len;
        let y_scale = scale / y_len;

        let new_x = [o[0] + x_vec[0] * x_scale, o[1] + x_vec[1] * x_scale];
        let new_y = [o[0] + y_vec[0] * y_scale, o[1] + y_vec[1] * y_scale];

        self.from_triangle_apophysis(o, new_x, new_y);
    }

    // === COMPATIBILITY METHODS (for gradual migration) ===

    /// COMPATIBILITY: Set variation by index (for old code)
    pub fn set_variation_by_index(&mut self, index: usize, weight: f32, registry: &VariationRegistry) {
        if let Some(name) = registry.names().get(index) {
            self.set_variation(name, weight);
        }
    }

    /// COMPATIBILITY: Get variation by index
    pub fn get_variation_by_index(&self, index: usize, registry: &VariationRegistry) -> f32 {
        if let Some(name) = registry.names().get(index) {
            self.get_variation(name)
        } else {
            0.0
        }
    }

    /// Convert this transform's variation weights into the GPU's fixed-size
    /// `[f32; 100]` slot array, using the supplied per-flame local index map.
    /// Variations not present in `local_map` (either not active anywhere in the
    /// flame, or dropped past the cap) contribute zero.
    pub fn to_fixed_array(&self, local_map: &HashMap<String, u32>) -> [f32; MAX_VARIATIONS_PER_FLAME] {
        let mut array = [0.0; MAX_VARIATIONS_PER_FLAME];
        for (name, weight) in &self.variations {
            if let Some(&local_idx) = local_map.get(name) {
                let slot = local_idx as usize;
                if slot < MAX_VARIATIONS_PER_FLAME {
                    array[slot] = *weight;
                }
            }
        }
        array
    }
}

/// Custom serialization - saves as sorted map for deterministic output
impl Serialize for Transform {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        // Convert HashMaps to BTreeMaps for deterministic key ordering
        // (HashMap iteration order is random, breaking content-addressable caching)
        let variations_sorted: BTreeMap<_, _> = self.variations.iter().collect();
        let params_sorted: BTreeMap<_, _> = self.variation_params.iter().collect();

        // Count fields: 13 base + 1 if direct_color != 0 + up to 8 post-affine
        // + up to 2 attachment lists (only when non-empty)
        let has_post = self.post_affine_enabled;
        let has_direct_color = self.direct_color.abs() > 1e-6;
        let has_linked = !self.linked_attachments.is_empty();
        let has_final = !self.final_attachments.is_empty();
        let field_count = 13
            + if has_direct_color { 1 } else { 0 }
            + if has_post { 8 } else { 0 }
            + if has_linked { 1 } else { 0 }
            + if has_final { 1 } else { 0 };

        let mut state = serializer.serialize_struct("Transform", field_count)?;
        state.serialize_field("a", &self.a)?;
        state.serialize_field("b", &self.b)?;
        state.serialize_field("c", &self.c)?;
        state.serialize_field("d", &self.d)?;
        state.serialize_field("e", &self.e)?;
        state.serialize_field("f", &self.f)?;
        state.serialize_field("g", &self.g)?;
        state.serialize_field("weight", &self.weight)?;
        state.serialize_field("variations", &variations_sorted)?;
        state.serialize_field("variation_params", &params_sorted)?;
        state.serialize_field("color", &self.color)?;
        state.serialize_field("color_speed", &self.color_speed)?;
        state.serialize_field("opacity", &self.opacity)?;
        // Only serialize direct_color when non-zero (keeps .fflame files clean)
        if has_direct_color {
            state.serialize_field("direct_color", &self.direct_color)?;
        }
        // Only serialize post-affine fields when enabled (keeps .fflame files clean)
        if has_post {
            state.serialize_field("post_affine_enabled", &self.post_affine_enabled)?;
            state.serialize_field("post_a", &self.post_a)?;
            state.serialize_field("post_b", &self.post_b)?;
            state.serialize_field("post_c", &self.post_c)?;
            state.serialize_field("post_d", &self.post_d)?;
            state.serialize_field("post_e", &self.post_e)?;
            state.serialize_field("post_f", &self.post_f)?;
            state.serialize_field("post_g", &self.post_g)?;
        }
        if has_linked {
            state.serialize_field("linked_attachments", &self.linked_attachments)?;
        }
        if has_final {
            state.serialize_field("final_attachments", &self.final_attachments)?;
        }
        state.end()
    }
}

/// Custom deserialization - supports both HashMap and array formats
impl<'de> Deserialize<'de> for Transform {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            A, B, C, D, E, F, G, Weight, Variations, VariationParams, Color, ColorSpeed, Opacity,
            DirectColor,
            PostAffineEnabled, PostA, PostB, PostC, PostD, PostE, PostF, PostG,
            LinkedAttachments, FinalAttachments,
        }

        struct TransformVisitor;

        impl<'de> Visitor<'de> for TransformVisitor {
            type Value = Transform;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Transform")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Transform, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut a = None;
                let mut b = None;
                let mut c = None;
                let mut d = None;
                let mut e = None;
                let mut f = None;
                let mut g = None;
                let mut weight = None;
                let mut variations = None;
                let mut variation_params = None;
                let mut color = None;
                let mut color_speed = None;
                let mut opacity = None;
                let mut direct_color = None;
                let mut post_affine_enabled = None;
                let mut post_a = None;
                let mut post_b = None;
                let mut post_c = None;
                let mut post_d = None;
                let mut post_e = None;
                let mut post_f = None;
                let mut post_g = None;
                let mut linked_attachments: Option<Vec<usize>> = None;
                let mut final_attachments: Option<Vec<usize>> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::A => a = Some(map.next_value()?),
                        Field::B => b = Some(map.next_value()?),
                        Field::C => c = Some(map.next_value()?),
                        Field::D => d = Some(map.next_value()?),
                        Field::E => e = Some(map.next_value()?),
                        Field::F => f = Some(map.next_value()?),
                        Field::G => g = Some(map.next_value()?),
                        Field::Weight => weight = Some(map.next_value()?),
                        Field::Variations => {
                            // Try to deserialize as HashMap first
                            let value: serde_json::Value = map.next_value()?;

                            let var_map = match value {
                                // New format: HashMap
                                serde_json::Value::Object(obj) => {
                                    let mut map = HashMap::new();
                                    for (k, v) in obj {
                                        if let serde_json::Value::Number(num) = v {
                                            if let Some(f) = num.as_f64() {
                                                map.insert(k, f as f32);
                                            }
                                        }
                                    }
                                    map
                                }
                                // Old format: Array - convert using global registry
                                serde_json::Value::Array(arr) => {
                                    let mut map = HashMap::new();
                                    let registry = crate::variations::global_registry();
                                    let names = registry.names();

                                    for (i, val) in arr.iter().enumerate() {
                                        if let serde_json::Value::Number(num) = val {
                                            if let Some(weight) = num.as_f64() {
                                                let weight = weight as f32;
                                                if weight.abs() > 1e-6 {
                                                    if let Some(name) = names.get(i) {
                                                        map.insert(name.clone(), weight);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    map
                                }
                                _ => {
                                    return Err(de::Error::custom(
                                        "variations must be an object (new format) or array (legacy format)"
                                    ));
                                }
                            };

                            variations = Some(var_map);
                        }
                        Field::VariationParams => {
                            variation_params = Some(map.next_value()?);
                        }
                        Field::Color => {
                            // Handle both old format [f32; 3] and new format f32
                            let value: serde_json::Value = map.next_value()?;
                            let color_value = match value {
                                // New format: single float (palette position)
                                serde_json::Value::Number(num) => {
                                    num.as_f64().map(|f| f as f32).unwrap_or(0.5)
                                }
                                // Old format: RGB array → average to single value
                                serde_json::Value::Array(arr) if arr.len() >= 3 => {
                                    let r = arr[0].as_f64().unwrap_or(0.0) as f32;
                                    let g = arr[1].as_f64().unwrap_or(0.0) as f32;
                                    let b = arr[2].as_f64().unwrap_or(0.0) as f32;
                                    (r + g + b) / 3.0
                                }
                                _ => 0.5  // Default to mid-palette
                            };
                            color = Some(color_value);
                        }
                        Field::ColorSpeed => color_speed = Some(map.next_value()?),
                        Field::Opacity => opacity = Some(map.next_value()?),
                        Field::DirectColor => direct_color = Some(map.next_value()?),
                        Field::PostAffineEnabled => post_affine_enabled = Some(map.next_value()?),
                        Field::PostA => post_a = Some(map.next_value()?),
                        Field::PostB => post_b = Some(map.next_value()?),
                        Field::PostC => post_c = Some(map.next_value()?),
                        Field::PostD => post_d = Some(map.next_value()?),
                        Field::PostE => post_e = Some(map.next_value()?),
                        Field::PostF => post_f = Some(map.next_value()?),
                        Field::PostG => post_g = Some(map.next_value()?),
                        Field::LinkedAttachments => linked_attachments = Some(map.next_value()?),
                        Field::FinalAttachments => final_attachments = Some(map.next_value()?),
                    }
                }

                Ok(Transform {
                    a: a.ok_or_else(|| de::Error::missing_field("a"))?,
                    b: b.ok_or_else(|| de::Error::missing_field("b"))?,
                    c: c.ok_or_else(|| de::Error::missing_field("c"))?,
                    d: d.ok_or_else(|| de::Error::missing_field("d"))?,
                    e: e.ok_or_else(|| de::Error::missing_field("e"))?,
                    f: f.ok_or_else(|| de::Error::missing_field("f"))?,
                    g: g.unwrap_or(0.0),
                    weight: weight.ok_or_else(|| de::Error::missing_field("weight"))?,
                    variations: variations.ok_or_else(|| de::Error::missing_field("variations"))?,
                    variation_params: variation_params.unwrap_or_else(HashMap::new), // Default to empty if missing
                    color: color.ok_or_else(|| de::Error::missing_field("color"))?,
                    color_speed: color_speed.unwrap_or(0.0), // Default to 0.0 for backward compatibility
                    opacity: opacity.unwrap_or(1.0), // Default to 1.0 for backward compatibility
                    direct_color: direct_color.unwrap_or(0.0), // Default 0.0 (no direct-color blending)
                    // Post-affine defaults to disabled + identity (backward compatible)
                    post_affine_enabled: post_affine_enabled.unwrap_or(false),
                    post_a: post_a.unwrap_or(1.0),
                    post_b: post_b.unwrap_or(0.0),
                    post_c: post_c.unwrap_or(0.0),
                    post_d: post_d.unwrap_or(1.0),
                    post_e: post_e.unwrap_or(0.0),
                    post_f: post_f.unwrap_or(0.0),
                    post_g: post_g.unwrap_or(0.0),
                    linked_attachments: linked_attachments.unwrap_or_default(),
                    final_attachments: final_attachments.unwrap_or_default(),
                })
            }
        }

        const FIELDS: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "weight", "variations", "variation_params", "color", "color_speed", "opacity", "direct_color", "post_affine_enabled", "post_a", "post_b", "post_c", "post_d", "post_e", "post_f", "post_g", "linked_attachments", "final_attachments"];
        deserializer.deserialize_struct("Transform", FIELDS, TransformVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_named_variations() {
        let mut xform = Transform::new();
        xform.set_variation("linear", 0.5);
        xform.set_variation("swirl", 0.3);

        assert_eq!(xform.get_variation("linear"), 0.5);
        assert_eq!(xform.get_variation("swirl"), 0.3);
        assert_eq!(xform.get_variation("nonexistent"), 0.0);
    }

    #[test]
    fn test_legacy_final_migration_attaches_to_every_normal() {
        let mut flame = Flame::new();
        flame.transforms.push(Transform::new());
        flame.transforms.push(Transform::new());
        flame.transforms.push(Transform::new());
        flame.final_transform = Some({
            let mut t = Transform::new();
            t.set_variation("spherical", 0.5);
            t
        });

        flame.migrate_legacy_final();

        // Pool should now contain the legacy final at index 0.
        assert_eq!(flame.final_transforms.len(), 1);
        assert_eq!(flame.final_transforms[0].get_variation("spherical"), 0.5);
        // Every normal transform should reference final_transforms[0].
        for t in &flame.transforms {
            assert_eq!(t.final_attachments, vec![0]);
            assert_eq!(t.linked_attachments, Vec::<usize>::new());
        }
    }

    #[test]
    fn test_migration_is_idempotent() {
        let mut flame = Flame::new();
        flame.transforms.push(Transform::new());
        flame.final_transform = Some(Transform::new());

        flame.migrate_legacy_final();
        let first_pool_len = flame.final_transforms.len();
        let first_attach = flame.transforms[0].final_attachments.clone();

        // Calling again should not duplicate the attachment on already-attached
        // normals (it WILL append a second copy to the pool, since we don't
        // dedup pool contents — but per-normal attachments are guarded). The
        // second call's attachments come back as [0, 1] though, since the new
        // pool index is 1. So idempotency is per-normal-deduplication only.
        flame.migrate_legacy_final();
        // Pool grows by 1 (we don't dedup pool contents — calling twice is a
        // bug we want to catch, not silently ignore).
        assert_eq!(flame.final_transforms.len(), first_pool_len + 1);
        // The attachment list grows by 1 (new pool index).
        assert_eq!(flame.transforms[0].final_attachments.len(), first_attach.len() + 1);
    }

    #[test]
    fn test_migration_no_legacy_final_is_noop() {
        let mut flame = Flame::new();
        flame.transforms.push(Transform::new());
        // No final_transform set.
        flame.migrate_legacy_final();
        assert!(flame.final_transforms.is_empty());
        assert!(flame.transforms[0].final_attachments.is_empty());
    }

    #[test]
    fn test_array_conversion() {
        let array = [0.5, 0.0, 0.0, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let registry = crate::variations::global_registry();

        let map = Transform::from_array(&array, &registry);

        assert_eq!(map.get("linear"), Some(&0.5));
        assert_eq!(map.get("swirl"), Some(&0.3));
    }

    #[test]
    fn test_gpu_array_conversion() {
        let mut xform = Transform::new();
        xform.set_variation("linear", 0.5);
        xform.set_variation("swirl", 0.3);

        let mut id_map = HashMap::new();
        id_map.insert("linear".to_string(), 0);
        id_map.insert("swirl".to_string(), 1);

        let gpu_array = xform.to_gpu_array(&id_map, 10);

        assert_eq!(gpu_array[0], 0.5);
        assert_eq!(gpu_array[1], 0.3);
        assert_eq!(gpu_array[2], 0.0);
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut xform = Transform::new();
        xform.set_variation("linear", 0.5);
        xform.set_variation("swirl", 0.3);

        let json = serde_json::to_string(&xform).unwrap();
        let deserialized: Transform = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.get_variation("linear"), 0.5);
        assert_eq!(deserialized.get_variation("swirl"), 0.3);
    }

    #[test]
    fn test_deserialize_legacy_array() {
        let json = r#"{
            "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "e": 0.0, "f": 0.0, "g": 0.0,
            "weight": 1.0,
            "variations": [0.5, 0.0, 0.0, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            "color": [1.0, 1.0, 1.0],
            "color_speed": 0.5
        }"#;

        let xform: Transform = serde_json::from_str(json).unwrap();

        assert_eq!(xform.get_variation("linear"), 0.5);
        assert_eq!(xform.get_variation("swirl"), 0.3);
    }

    #[test]
    fn test_variation_params_set_get() {
        let mut xform = Transform::new();

        // Set parameters
        xform.set_variation_param("julian", "power", 5.0);
        xform.set_variation_param("julian", "dist", 1.5);

        // Get parameters
        assert_eq!(xform.get_variation_param("julian", "power"), Some(5.0));
        assert_eq!(xform.get_variation_param("julian", "dist"), Some(1.5));

        // Non-existent parameter
        assert_eq!(xform.get_variation_param("julian", "nonexistent"), None);
        assert_eq!(xform.get_variation_param("nonexistent", "power"), None);
    }

    #[test]
    fn test_variation_params_serialize() {
        let mut xform = Transform::new();
        xform.set_variation("julian", 0.8);
        xform.set_variation_param("julian", "power", 3.0);
        xform.set_variation_param("julian", "dist", 1.0);

        let json = serde_json::to_string(&xform).unwrap();
        let deserialized: Transform = serde_json::from_str(&json).unwrap();

        // Verify variation weight
        assert_eq!(deserialized.get_variation("julian"), 0.8);

        // Verify parameters
        assert_eq!(deserialized.get_variation_param("julian", "power"), Some(3.0));
        assert_eq!(deserialized.get_variation_param("julian", "dist"), Some(1.0));
    }

    #[test]
    fn test_variation_params_backward_compat() {
        // Old config without variation_params field
        let json = r#"{
            "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "e": 0.0, "f": 0.0, "g": 0.0,
            "weight": 1.0,
            "variations": {"julian": 0.8},
            "color": [1.0, 1.0, 1.0],
            "color_speed": 0.5
        }"#;

        let xform: Transform = serde_json::from_str(json).unwrap();

        // Should deserialize successfully
        assert_eq!(xform.get_variation("julian"), 0.8);

        // variation_params should be empty (defaults to empty HashMap)
        assert_eq!(xform.get_variation_param("julian", "power"), None);
        assert!(xform.variation_params.is_empty());
    }

}
// === Additional code from legacy transforms.rs ===

/// Rendering mode for the fractal flame
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderMode {
    /// 2D rendering (traditional fractal flames)
    TwoD,
    /// 3D rendering with pseudo-3D projection
    ThreeD,
}

impl Default for RenderMode {
    fn default() -> Self {
        Self::TwoD
    }
}

// ProjectionType enum removed - now using perspective_strength f32 directly
// 0.0 = orthographic (flat), higher values = increasing perspective distortion

/// A 2D point in fractal space
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Compute radius squared (r²)
    #[inline]
    pub fn r_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// Compute radius (r)
    #[inline]
    pub fn r(&self) -> f32 {
        self.r_squared().sqrt()
    }

    /// Compute angle (theta)
    #[inline]
    pub fn theta(&self) -> f32 {
        self.y.atan2(self.x)
    }

    /// Compute phi (reciprocal of radius)
    #[inline]
    pub fn phi(&self) -> f32 {
        self.x.atan2(self.y)
    }
}

/// Flame system - collection of transforms
#[derive(Debug, Clone, Serialize)]
pub struct Flame {
    pub name: String,
    pub transforms: Vec<Transform>,
    /// LEGACY: singular global Final transform. Loaded by the Flame
    /// deserializer if present in the JSON, then migrated into
    /// `final_transforms[0]` (with `final_attachments = [0]` on every
    /// normal). Kept here for now so the existing renderer and UI
    /// callers continue to work; will be removed once the new chain
    /// model fully replaces them. Always `None` after migration.
    /// See `docs/projects/per-transform-linked-and-final.md`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_transform: Option<Transform>,
    /// Pool of Linked transforms — referenced by index from each
    /// normal transform's `linked_attachments`. Linked transforms are
    /// part of dynamics (their output feeds the next iteration) and
    /// run in declaration order after the normal transform's variations.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub linked_transforms: Vec<Transform>,
    /// Pool of Final transforms — referenced by index from each
    /// normal transform's `final_attachments`. Final transforms are
    /// pure plot-time filters (output is plotted but NOT fed forward)
    /// and run in declaration order after the Linked chain.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub final_transforms: Vec<Transform>,
    /// Rendering mode (2D or 3D)
    pub render_mode: RenderMode,
    /// Perspective strength for 3D rendering (0.0 = flat/orthographic, 10.0 = strong perspective)
    pub perspective_strength: f32,
    /// Xaos transition weights: xaos[src][dst] = modifier for src→dst transition
    /// None when all weights are 1.0 (default behavior, no memory allocated)
    /// When Some, outer Vec has len = transforms.len(), inner Vec has len = transforms.len()
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xaos: Option<Vec<Vec<f32>>>,

    /// Solo transform index (0-indexed). When Some(n), only transform n has weight,
    /// all others effectively have weight 0. Used for debugging individual transforms.
    /// Matches Apophysis XML attribute: soloxform="N"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solo_transform: Option<usize>,
}

fn default_flame_name() -> String {
    "Untitled".to_string()
}

impl Default for Flame {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            transforms: Vec::new(),
            final_transform: None,
            linked_transforms: Vec::new(),
            final_transforms: Vec::new(),
            render_mode: RenderMode::default(),
            perspective_strength: 0.0,  // Default to orthographic (flat)
            xaos: None,  // Default: no xaos (all weights implicitly 1.0)
            solo_transform: None,  // Default: no solo (all transforms active)
        }
    }
}

impl Flame {
    /// Index in the GPU concatenated transforms buffer where the
    /// LEGACY `flame.final_transform` is appended (see
    /// `GpuTransform::from_flame` doc). Equals
    /// `transforms.len() + linked_transforms.len() + final_transforms.len()`.
    /// Only meaningful while `final_transform` is `Some`; the renderer
    /// gates on `has_final_transform`. Removed in Phase 4 along with
    /// the legacy field.
    pub fn legacy_final_slot(&self) -> u32 {
        (self.transforms.len() + self.linked_transforms.len() + self.final_transforms.len()) as u32
    }

    /// Total GPU transform slot count: normals + linkeds + finals + 1
    /// (legacy final, if present).
    pub fn total_gpu_transform_slots(&self) -> usize {
        self.transforms.len()
            + self.linked_transforms.len()
            + self.final_transforms.len()
            + if self.final_transform.is_some() { 1 } else { 0 }
    }

    /// Migrate a legacy singular `final_transform` into the new
    /// `final_transforms` pool with an attachment on every normal
    /// transform. The legacy field is left populated for now (so the
    /// existing renderer that reads `flame.final_transform` keeps
    /// working) and will be cleared in Phase 4 of the project.
    /// See `docs/projects/per-transform-linked-and-final.md`.
    pub fn migrate_legacy_final(&mut self) {
        let Some(ref legacy) = self.final_transform else { return };
        let new_idx = self.final_transforms.len();
        self.final_transforms.push(legacy.clone());
        for t in &mut self.transforms {
            if !t.final_attachments.contains(&new_idx) {
                t.final_attachments.push(new_idx);
            }
        }
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_transform(&mut self, transform: Transform) {
        self.transforms.push(transform);
    }

    /// Extract all active variation names from all three pools.
    /// Used by the shader builder to compile only the variations a
    /// flame actually uses.
    pub fn extract_active_variations(&self) -> HashMap<String, f32> {
        let mut all_variations = HashMap::new();
        let mut absorb = |t: &Transform, all: &mut HashMap<String, f32>| {
            for (name, weight) in &t.variations {
                if weight.abs() > 1e-6 {
                    let existing = all.entry(name.clone()).or_insert(0.0);
                    *existing = f32::max(*existing, *weight);
                }
            }
        };

        for t in &self.transforms { absorb(t, &mut all_variations); }
        for t in &self.linked_transforms { absorb(t, &mut all_variations); }
        for t in &self.final_transforms { absorb(t, &mut all_variations); }

        // Legacy global Final — kept in extract until Phase 4 drops the field.
        if let Some(final_xform) = &self.final_transform {
            absorb(final_xform, &mut all_variations);
        }

        all_variations
    }

    /// Check if any transform (regular or final) has post-affine enabled
    pub fn has_post_affine(&self) -> bool {
        for xform in &self.transforms {
            if xform.post_affine_enabled {
                return true;
            }
        }
        if let Some(ref final_xform) = self.final_transform {
            if final_xform.post_affine_enabled {
                return true;
            }
        }
        false
    }

    /// Get runtime ID mapping for active variations.
    /// Delegates to `compute_local_index_map` for stable registry-order assignment
    /// and the per-flame cap.
    pub fn get_id_mapping(&self) -> HashMap<String, u32> {
        let registry = crate::variations::global_registry();
        compute_local_index_map(
            self.extract_active_variations().into_keys(),
            &registry,
        )
    }

    /// Calculate cumulative weights for transform selection
    pub fn cumulative_weights(&self) -> Vec<f32> {
        let mut cumulative = Vec::with_capacity(self.transforms.len());
        let mut sum = 0.0;
        for transform in &self.transforms {
            sum += transform.weight;
            cumulative.push(sum);
        }
        cumulative
    }

    /// Select a transform index based on random value
    pub fn select_transform(&self, cumulative_weights: &[f32], rand_val: f32) -> usize {
        let total = cumulative_weights.last().copied().unwrap_or(1.0);
        let target = rand_val * total;

        for (i, &cum_weight) in cumulative_weights.iter().enumerate() {
            if target <= cum_weight {
                return i;
            }
        }
        self.transforms.len().saturating_sub(1)
    }

}

// Custom deserializer for Flame to handle backward compatibility with old ProjectionType enum
impl<'de> Deserialize<'de> for Flame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Name,
            Transforms,
            FinalTransform,
            LinkedTransforms,
            FinalTransforms,
            RenderMode,
            PerspectiveStrength,
            Projection, // Old field name for backward compatibility
            Xaos,
            SoloTransform,
        }

        struct FlameVisitor;

        impl<'de> Visitor<'de> for FlameVisitor {
            type Value = Flame;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Flame")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Flame, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut name = None;
                let mut transforms: Option<Vec<Transform>> = None;
                let mut final_transform: Option<Transform> = None;
                let mut linked_transforms: Option<Vec<Transform>> = None;
                let mut final_transforms: Option<Vec<Transform>> = None;
                let mut render_mode = None;
                let mut perspective_strength = None;
                let mut xaos = None;
                let mut solo_transform = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            name = Some(map.next_value()?);
                        }
                        Field::Transforms => {
                            transforms = Some(map.next_value()?);
                        }
                        Field::FinalTransform => {
                            final_transform = map.next_value()?;
                        }
                        Field::LinkedTransforms => {
                            linked_transforms = Some(map.next_value()?);
                        }
                        Field::FinalTransforms => {
                            final_transforms = Some(map.next_value()?);
                        }
                        Field::RenderMode => {
                            render_mode = Some(map.next_value()?);
                        }
                        Field::PerspectiveStrength => {
                            perspective_strength = Some(map.next_value()?);
                        }
                        Field::Projection => {
                            // Old format: enum ProjectionType
                            // { "Orthographic": null } or { "Perspective": { "strength": 2.0 } }
                            let value: serde_json::Value = map.next_value()?;

                            // Extract strength from old ProjectionType enum
                            perspective_strength = Some(match value {
                                serde_json::Value::String(ref s) if s == "Orthographic" => 0.0,
                                serde_json::Value::Object(ref obj) => {
                                    if let Some(persp) = obj.get("Perspective") {
                                        if let Some(strength_obj) = persp.as_object() {
                                            if let Some(strength) = strength_obj.get("strength") {
                                                strength.as_f64().unwrap_or(0.0) as f32
                                            } else {
                                                2.0 // Default if strength missing
                                            }
                                        } else {
                                            2.0 // Default
                                        }
                                    } else {
                                        0.0 // Orthographic
                                    }
                                }
                                _ => 0.0, // Default to orthographic
                            });
                        }
                        Field::Xaos => {
                            xaos = Some(map.next_value()?);
                        }
                        Field::SoloTransform => {
                            solo_transform = Some(map.next_value()?);
                        }
                    }
                }

                let transforms = transforms
                    .ok_or_else(|| de::Error::missing_field("transforms"))?;
                let linked_transforms = linked_transforms.unwrap_or_default();
                let final_transforms = final_transforms.unwrap_or_default();

                let mut flame = Flame {
                    name: name.unwrap_or_else(|| default_flame_name()),
                    transforms,
                    final_transform,
                    linked_transforms,
                    final_transforms,
                    render_mode: render_mode.unwrap_or_default(),
                    perspective_strength: perspective_strength.unwrap_or(0.0),
                    xaos,
                    solo_transform: solo_transform.unwrap_or(None),
                };
                // Migrate legacy singular `final_transform` into the new
                // `final_transforms` pool. See
                // `docs/projects/per-transform-linked-and-final.md`
                // §"File format / migration".
                flame.migrate_legacy_final();
                Ok(flame)
            }
        }

        const FIELDS: &[&str] = &["name", "transforms", "final_transform", "linked_transforms", "final_transforms", "render_mode", "perspective_strength", "projection", "xaos", "solo_transform"];
        deserializer.deserialize_struct("Flame", FIELDS, FlameVisitor)
    }
}
