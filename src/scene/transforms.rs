use std::collections::{HashMap, BTreeMap};
use serde::{Deserialize, Serialize, Deserializer, Serializer};
use serde::de::{self, Visitor, MapAccess};
use crate::variations::VariationRegistry;

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
            post_affine_enabled: false,
            post_a: 1.0,
            post_b: 0.0,
            post_c: 0.0,
            post_d: 1.0,
            post_e: 0.0,
            post_f: 0.0,
            post_g: 0.0,
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

    /// COMPATIBILITY: Convert to fixed 100-element array for GPU
    pub fn to_fixed_array(&self, registry: &VariationRegistry) -> [f32; 100] {
        let mut array = [0.0; 100];
        for (i, name) in registry.names().iter().enumerate().take(100) {
            array[i] = self.get_variation(name);
        }
        array
    }

    /// COMPATIBILITY: Set from fixed array
    pub fn from_fixed_array(&mut self, array: [f32; 100], registry: &VariationRegistry) {
        self.variations.clear();
        for (i, &weight) in array.iter().enumerate() {
            if weight.abs() > 1e-6 {
                if let Some(name) = registry.names().get(i) {
                    self.set_variation(name, weight);
                }
            }
        }
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

        // Count fields: 13 base + up to 8 post-affine
        let has_post = self.post_affine_enabled;
        let field_count = 13 + if has_post { 8 } else { 0 };

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
            PostAffineEnabled, PostA, PostB, PostC, PostD, PostE, PostF, PostG,
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
                let mut post_affine_enabled = None;
                let mut post_a = None;
                let mut post_b = None;
                let mut post_c = None;
                let mut post_d = None;
                let mut post_e = None;
                let mut post_f = None;
                let mut post_g = None;

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
                        Field::PostAffineEnabled => post_affine_enabled = Some(map.next_value()?),
                        Field::PostA => post_a = Some(map.next_value()?),
                        Field::PostB => post_b = Some(map.next_value()?),
                        Field::PostC => post_c = Some(map.next_value()?),
                        Field::PostD => post_d = Some(map.next_value()?),
                        Field::PostE => post_e = Some(map.next_value()?),
                        Field::PostF => post_f = Some(map.next_value()?),
                        Field::PostG => post_g = Some(map.next_value()?),
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
                    // Post-affine defaults to disabled + identity (backward compatible)
                    post_affine_enabled: post_affine_enabled.unwrap_or(false),
                    post_a: post_a.unwrap_or(1.0),
                    post_b: post_b.unwrap_or(0.0),
                    post_c: post_c.unwrap_or(0.0),
                    post_d: post_d.unwrap_or(1.0),
                    post_e: post_e.unwrap_or(0.0),
                    post_f: post_f.unwrap_or(0.0),
                    post_g: post_g.unwrap_or(0.0),
                })
            }
        }

        const FIELDS: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "weight", "variations", "variation_params", "color", "color_speed", "opacity", "post_affine_enabled", "post_a", "post_b", "post_c", "post_d", "post_e", "post_f", "post_g"];
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

    // === LINKED TRANSFORMS TESTS ===

    fn make_flame_with_xaos(num_transforms: usize, xaos: Vec<Vec<f32>>) -> Flame {
        let mut flame = Flame::new();
        for _ in 0..num_transforms {
            let mut t = Transform::new();
            t.set_variation("linear", 1.0);
            flame.transforms.push(t);
        }
        flame.xaos = Some(xaos);
        flame
    }

    #[test]
    fn test_detect_no_links() {
        // All weights 1.0 - no links
        let flame = make_flame_with_xaos(3, vec![
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
        ]);
        assert!(flame.detect_linked_pairs().is_empty());
        assert!(flame.detect_linked_chains().is_empty());
    }

    #[test]
    fn test_detect_no_links_without_xaos() {
        // No xaos at all (None) - no links
        let mut flame = Flame::new();
        for _ in 0..3 {
            let mut t = Transform::new();
            t.set_variation("linear", 1.0);
            flame.transforms.push(t);
        }
        assert!(flame.detect_linked_pairs().is_empty());
        assert!(flame.detect_linked_chains().is_empty());
    }

    #[test]
    fn test_detect_simple_link() {
        // T0 -> T1 link: T0 routes only to T1, T1 only reachable from T0
        let flame = make_flame_with_xaos(3, vec![
            vec![0.0, 1.0, 0.0], // T0: only goes to T1
            vec![1.0, 0.0, 1.0], // T1: goes to T0 and T2
            vec![1.0, 0.0, 1.0], // T2: goes to T0 and T2 (NOT T1)
        ]);

        let pairs = flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].pre, 0);
        assert_eq!(pairs[0].post, 1);

        let chains = flame.detect_linked_chains();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0], vec![0, 1]);
    }

    #[test]
    fn test_detect_chain() {
        // T0 -> T1 -> T2 chain
        let flame = make_flame_with_xaos(4, vec![
            vec![0.0, 1.0, 0.0, 0.0], // T0: only to T1
            vec![0.0, 0.0, 1.0, 0.0], // T1: only to T2
            vec![1.0, 0.0, 0.0, 1.0], // T2: to T0 and T3
            vec![1.0, 0.0, 0.0, 1.0], // T3: to T0 and T3 (NOT T1 or T2)
        ]);

        let pairs = flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], LinkedPair { pre: 0, post: 1 });
        assert_eq!(pairs[1], LinkedPair { pre: 1, post: 2 });

        let chains = flame.detect_linked_chains();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0], vec![0, 1, 2]);
    }

    #[test]
    fn test_detect_multiple_links() {
        // Two independent links: T0->T1 and T2->T3
        // T1 only reachable from T0, T3 only reachable from T2
        let flame = make_flame_with_xaos(4, vec![
            vec![0.0, 1.0, 0.0, 0.0], // T0: only to T1
            vec![1.0, 0.0, 1.0, 0.0], // T1: to T0 and T2 (NOT T3)
            vec![0.0, 0.0, 0.0, 1.0], // T2: only to T3
            vec![1.0, 0.0, 1.0, 0.0], // T3: to T0 and T2 (NOT T1)
        ]);

        let pairs = flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], LinkedPair { pre: 0, post: 1 });
        assert_eq!(pairs[1], LinkedPair { pre: 2, post: 3 });

        let chains = flame.detect_linked_chains();
        assert_eq!(chains.len(), 2);
        assert_eq!(chains[0], vec![0, 1]);
        assert_eq!(chains[1], vec![2, 3]);
    }

    #[test]
    fn test_link_transforms_changes() {
        // Start with 3 transforms, all default xaos (1.0)
        let flame = make_flame_with_xaos(3, vec![
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
        ]);

        let changes = flame.link_transforms_changes(0, 1);
        assert!(!changes.is_empty());

        // Apply changes manually to verify
        let mut test_flame = flame.clone();
        for (path, value) in &changes {
            if let crate::config::ConfigPath::Xaos { src, dst } = path {
                let weight: f32 = match value {
                    crate::config::ConfigValue::Float(v) => *v,
                    _ => panic!("Expected float"),
                };
                test_flame.set_xaos(*src, *dst, weight);
            }
        }

        // Verify the link was created
        let pairs = test_flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].pre, 0);
        assert_eq!(pairs[0].post, 1);

        // T0 should route only to T1
        assert!(test_flame.get_xaos(0, 0) < 1e-6);
        assert!(test_flame.get_xaos(0, 1) > 0.9);
        assert!(test_flame.get_xaos(0, 2) < 1e-6);

        // T1 should not be reachable from T2
        assert!(test_flame.get_xaos(2, 1) < 1e-6);
    }

    #[test]
    fn test_unlink_chain() {
        // Set up a linked pair T0 -> T1
        let flame = make_flame_with_xaos(3, vec![
            vec![0.0, 1.0, 0.0], // T0: only to T1
            vec![1.0, 0.0, 1.0], // T1: to T0 and T2
            vec![1.0, 0.0, 1.0], // T2: to T0 and T2
        ]);

        let chains = flame.detect_linked_chains();
        assert_eq!(chains.len(), 1);

        let changes = flame.unlink_chain_changes(&chains[0]);
        assert!(!changes.is_empty());

        // Apply changes
        let mut test_flame = flame.clone();
        for (path, value) in &changes {
            if let crate::config::ConfigPath::Xaos { src, dst } = path {
                let weight: f32 = match value {
                    crate::config::ConfigValue::Float(v) => *v,
                    _ => panic!("Expected float"),
                };
                test_flame.set_xaos(*src, *dst, weight);
            }
        }

        // After unlinking, no linked pairs should exist
        let pairs = test_flame.detect_linked_pairs();
        assert!(pairs.is_empty(), "Expected no linked pairs after unlink, got: {:?}", pairs);

        // Verify all chain members have all weights restored to 1.0
        for &idx in &chains[0] {
            for dst in 0..3 {
                assert!((test_flame.get_xaos(idx, dst) - 1.0).abs() < 1e-6,
                    "Expected weight 1.0 for xaos[{}][{}], got {}", idx, dst, test_flame.get_xaos(idx, dst));
            }
        }
    }

    #[test]
    fn test_unlink_restores_pre_to_post_weight() {
        // Regression: linking T1->T2 then unlinking left xaos[T1][T2] = 0.0
        let mut flame = make_flame_with_xaos(3, vec![
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
        ]);

        // Link T1 -> T2 (indices 1 -> 2)
        let link_changes = flame.link_transforms_changes(1, 2);
        for (path, value) in &link_changes {
            if let crate::config::ConfigPath::Xaos { src, dst } = path {
                let weight: f32 = match value {
                    crate::config::ConfigValue::Float(v) => *v,
                    _ => panic!("Expected float"),
                };
                flame.set_xaos(*src, *dst, weight);
            }
        }

        // Verify link exists
        let pairs = flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 1);

        // Unlink
        let chains = flame.detect_linked_chains();
        let unlink_changes = flame.unlink_chain_changes(&chains[0]);
        for (path, value) in &unlink_changes {
            if let crate::config::ConfigPath::Xaos { src, dst } = path {
                let weight: f32 = match value {
                    crate::config::ConfigValue::Float(v) => *v,
                    _ => panic!("Expected float"),
                };
                flame.set_xaos(*src, *dst, weight);
            }
        }

        // THE BUG: xaos[1][2] was 0.0 instead of 1.0
        assert!((flame.get_xaos(1, 2) - 1.0).abs() < 1e-6,
            "xaos[1][2] should be 1.0 after unlink, got {}", flame.get_xaos(1, 2));

        // All weights should be 1.0
        for src in 0..3 {
            for dst in 0..3 {
                assert!((flame.get_xaos(src, dst) - 1.0).abs() < 1e-6,
                    "xaos[{}][{}] should be 1.0, got {}", src, dst, flame.get_xaos(src, dst));
            }
        }
    }

    #[test]
    fn test_on_transform_added_preserves_links() {
        // Set up linked pair T0 -> T1
        let mut flame = make_flame_with_xaos(3, vec![
            vec![0.0, 1.0, 0.0], // T0: only to T1
            vec![1.0, 0.0, 1.0], // T1: to T0 and T2
            vec![1.0, 0.0, 1.0], // T2: to T0 and T2
        ]);

        // Verify link exists
        let pairs = flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 1);

        // Add a new transform at index 3 (end)
        flame.on_transform_added(3);
        flame.transforms.push(Transform::new());

        // Matrix should be 4x4 now
        let xaos = flame.xaos.as_ref().unwrap();
        assert_eq!(xaos.len(), 4);
        assert_eq!(xaos[0].len(), 4);

        // The link should still be detected (T0 -> T1)
        let pairs_after = flame.detect_linked_pairs();
        assert_eq!(pairs_after.len(), 1, "Link should be preserved after adding transform");
        assert_eq!(pairs_after[0].pre, 0);
        assert_eq!(pairs_after[0].post, 1);

        // New transform (T3) should have weight 0 to T1 (post-transform)
        assert!(flame.get_xaos(3, 1) < 1e-6, "New transform should not route to post-transform");
    }

    #[test]
    fn test_on_transform_deleted_updates_xaos() {
        let mut flame = make_flame_with_xaos(3, vec![
            vec![0.5, 1.0, 0.0],
            vec![1.0, 0.5, 1.0],
            vec![0.0, 1.0, 0.5],
        ]);

        // Delete transform at index 1
        flame.transforms.remove(1);
        flame.on_transform_deleted(1);

        // Matrix should be 2x2 now with correct indices
        let xaos = flame.xaos.as_ref().unwrap();
        assert_eq!(xaos.len(), 2);
        assert_eq!(xaos[0].len(), 2);

        // Row 0 (was T0): kept columns 0 and 2 (now 0 and 1)
        assert_eq!(flame.get_xaos(0, 0), 0.5); // T0->T0
        assert_eq!(flame.get_xaos(0, 1), 0.0); // T0->T2 (was column 2)

        // Row 1 (was T2): kept columns 0 and 2 (now 0 and 1)
        assert_eq!(flame.get_xaos(1, 0), 0.0); // T2->T0
        assert_eq!(flame.get_xaos(1, 1), 0.5); // T2->T2 (was column 2)
    }

    #[test]
    fn test_too_few_transforms_for_links() {
        // 0 or 1 transforms - can't have links
        let flame = Flame::new();
        assert!(flame.detect_linked_pairs().is_empty());

        let mut flame = Flame::new();
        flame.transforms.push(Transform::new());
        assert!(flame.detect_linked_pairs().is_empty());
    }

    #[test]
    fn test_link_same_transform_returns_empty() {
        let flame = make_flame_with_xaos(2, vec![
            vec![1.0, 1.0],
            vec![1.0, 1.0],
        ]);
        let changes = flame.link_transforms_changes(0, 0);
        assert!(changes.is_empty());
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_transform: Option<Transform>,
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
            render_mode: RenderMode::default(),
            perspective_strength: 0.0,  // Default to orthographic (flat)
            xaos: None,  // Default: no xaos (all weights implicitly 1.0)
            solo_transform: None,  // Default: no solo (all transforms active)
        }
    }
}

impl Flame {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_transform(&mut self, transform: Transform) {
        self.transforms.push(transform);
    }

    /// Extract all active variation names from all transforms
    pub fn extract_active_variations(&self) -> HashMap<String, f32> {
        let mut all_variations = HashMap::new();

        // Extract from regular transforms
        for transform in &self.transforms {
            for (name, weight) in &transform.variations {
                // Track max weight if variation used in multiple transforms
                if weight.abs() > 1e-6 {
                    let existing = all_variations.entry(name.clone()).or_insert(0.0);
                    *existing = f32::max(*existing, *weight);
                }
            }
        }

        // Extract from final transform if present
        if let Some(final_xform) = &self.final_transform {
            for (name, weight) in &final_xform.variations {
                if weight.abs() > 1e-6 {
                    let existing = all_variations.entry(name.clone()).or_insert(0.0);
                    *existing = f32::max(*existing, *weight);
                }
            }
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

    /// Get runtime ID mapping for active variations
    /// Uses registry order to ensure deterministic ID assignment
    pub fn get_id_mapping(&self) -> HashMap<String, u32> {
        let active_set: std::collections::HashSet<String> =
            self.extract_active_variations().keys().cloned().collect();

        // Use global registry order for deterministic ID assignment
        let registry = crate::variations::global_registry();
        let active: Vec<String> = registry.names()
            .iter()
            .filter(|name| active_set.contains(*name))
            .cloned()
            .collect();

        registry.assign_ids(&active)
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

    // === XAOS (WEIGHTED TRANSFORM TRANSITIONS) ===

    /// Check if this flame has non-default xaos weights
    /// Returns false if xaos is None or all weights are 1.0
    pub fn has_xaos(&self) -> bool {
        if let Some(ref xaos) = self.xaos {
            // Check if any weight differs from 1.0
            for row in xaos {
                for &weight in row {
                    if (weight - 1.0).abs() > 1e-6 {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get xaos weight for transition from src to dst
    /// Returns 1.0 if xaos is not enabled or indices are out of bounds
    pub fn get_xaos(&self, src: usize, dst: usize) -> f32 {
        if let Some(ref xaos) = self.xaos {
            if src < xaos.len() && dst < xaos[src].len() {
                return xaos[src][dst];
            }
        }
        1.0
    }

    /// Set xaos weight for transition from src to dst
    /// Automatically initializes xaos matrix if needed
    pub fn set_xaos(&mut self, src: usize, dst: usize, weight: f32) {
        self.ensure_xaos_size();
        if let Some(ref mut xaos) = self.xaos {
            if src < xaos.len() && dst < xaos[src].len() {
                xaos[src][dst] = weight;
            }
        }
    }

    /// Ensure xaos matrix exists and has correct size for current transform count
    /// Initializes all weights to 1.0 (default behavior)
    pub fn ensure_xaos_size(&mut self) {
        let n = self.transforms.len();
        if n == 0 {
            self.xaos = None;
            return;
        }

        match &mut self.xaos {
            None => {
                // Initialize with all 1.0 weights
                self.xaos = Some(vec![vec![1.0; n]; n]);
            }
            Some(xaos) => {
                // Resize if needed
                let current_size = xaos.len();
                if current_size != n {
                    // Resize rows
                    xaos.resize(n, vec![1.0; n]);
                    // Resize columns in each row
                    for row in xaos.iter_mut() {
                        row.resize(n, 1.0);
                    }
                }
            }
        }
    }

    /// Clear xaos (set to None, reverting to default behavior)
    pub fn clear_xaos(&mut self) {
        self.xaos = None;
    }

    /// Reset all xaos weights to 1.0 (keeping the matrix allocated)
    pub fn reset_xaos(&mut self) {
        if let Some(ref mut xaos) = self.xaos {
            for row in xaos.iter_mut() {
                for weight in row.iter_mut() {
                    *weight = 1.0;
                }
            }
        }
    }

    /// Update xaos matrix after a transform is added at the given index.
    ///
    /// Inserts a new row and column at the given index with default weight 1.0.
    /// Then preserves existing linked transforms by zeroing the new transform's
    /// weight to any existing post-transforms (transforms with exactly one
    /// non-zero incoming weight).
    pub fn on_transform_added(&mut self, index: usize) {
        // Detect linked pairs BEFORE modifying the matrix (needs immutable borrow)
        let pairs = self.detect_linked_pairs();
        let pre_indices: std::collections::HashSet<usize> = pairs.iter().map(|p| p.pre).collect();
        let post_indices: std::collections::HashSet<usize> = pairs.iter().map(|p| p.post).collect();

        if let Some(ref mut xaos) = self.xaos {
            let n = xaos.len(); // size before insertion

            // Insert new column at `index` in each existing row
            for row in xaos.iter_mut() {
                if index <= row.len() {
                    row.insert(index, 1.0);
                }
            }

            // Insert new row at `index` (length is now n+1)
            let new_row = vec![1.0; n + 1];
            if index <= xaos.len() {
                xaos.insert(index, new_row);
            }

            // Preserve linked transforms:
            // 1. New transform should not route to post-transforms (would break "sole incoming")
            for post in &post_indices {
                let adjusted_post = if *post >= index { *post + 1 } else { *post };
                if adjusted_post < xaos[index].len() {
                    xaos[index][adjusted_post] = 0.0;
                }
            }

            // 2. Pre-transforms should not route to new transform (would break "sole outgoing")
            for pre in &pre_indices {
                let adjusted_pre = if *pre >= index { *pre + 1 } else { *pre };
                if adjusted_pre < xaos.len() {
                    xaos[adjusted_pre][index] = 0.0;
                }
            }
        }
    }

    /// Update xaos matrix after a transform is deleted at the given index.
    ///
    /// Removes the row and column at the given index. If the resulting matrix
    /// is all 1.0, clears xaos entirely.
    pub fn on_transform_deleted(&mut self, index: usize) {
        if let Some(ref mut xaos) = self.xaos {
            // Remove the row at `index`
            if index < xaos.len() {
                xaos.remove(index);
            }

            // Remove the column at `index` from each remaining row
            for row in xaos.iter_mut() {
                if index < row.len() {
                    row.remove(index);
                }
            }

            // If matrix is now empty or all 1.0, clear it
            if xaos.is_empty() {
                self.xaos = None;
            } else {
                let all_default = xaos.iter().all(|row| row.iter().all(|&w| (w - 1.0).abs() < 1e-6));
                if all_default {
                    self.xaos = None;
                }
            }
        }
    }

    /// Get xaos matrix as flat array for GPU upload
    /// Returns None if xaos is not enabled
    /// Layout: row-major, xaos_flat[src * n + dst] = weight for src→dst
    pub fn xaos_flat(&self) -> Option<Vec<f32>> {
        self.xaos.as_ref().map(|xaos| {
            let n = xaos.len();
            let mut flat = Vec::with_capacity(n * n);
            for row in xaos {
                flat.extend_from_slice(row);
            }
            flat
        })
    }

    // === LINKED TRANSFORMS (CONVENIENCE LAYER ON XAOS) ===

    /// Detect all linked transform pairs from the xaos matrix.
    ///
    /// A linked pair (pre, post) exists when:
    /// - pre has exactly one non-zero outgoing xaos weight, pointing to post
    /// - post has exactly one non-zero incoming xaos weight, coming from pre
    ///
    /// Returns pairs sorted by pre index.
    pub fn detect_linked_pairs(&self) -> Vec<LinkedPair> {
        let n = self.transforms.len();
        if n < 2 {
            return Vec::new();
        }

        // For each transform, find its sole non-zero outgoing target (if any)
        let mut sole_outgoing: Vec<Option<usize>> = Vec::with_capacity(n);
        for src in 0..n {
            let mut target = None;
            let mut count = 0;
            for dst in 0..n {
                if self.get_xaos(src, dst) > 1e-6 {
                    count += 1;
                    target = Some(dst);
                }
            }
            if count == 1 {
                sole_outgoing.push(target);
            } else {
                sole_outgoing.push(None);
            }
        }

        // For each transform, count non-zero incoming weights
        let mut incoming_count: Vec<usize> = vec![0; n];
        let mut sole_incoming_src: Vec<Option<usize>> = vec![None; n];
        for dst in 0..n {
            for src in 0..n {
                if self.get_xaos(src, dst) > 1e-6 {
                    incoming_count[dst] += 1;
                    sole_incoming_src[dst] = Some(src);
                }
            }
        }

        // A linked pair exists when pre has sole outgoing to post,
        // AND post has sole incoming from pre
        let mut pairs = Vec::new();
        for pre in 0..n {
            if let Some(post) = sole_outgoing[pre] {
                if incoming_count[post] == 1 {
                    if let Some(src) = sole_incoming_src[post] {
                        if src == pre {
                            pairs.push(LinkedPair { pre, post });
                        }
                    }
                }
            }
        }

        pairs
    }

    /// Detect linked chains from linked pairs.
    ///
    /// A chain is a sequence [A, B, C, ...] where A->B and B->C are linked pairs.
    /// Returns chains sorted by first element. Each transform appears in at most one chain.
    pub fn detect_linked_chains(&self) -> Vec<Vec<usize>> {
        let pairs = self.detect_linked_pairs();
        if pairs.is_empty() {
            return Vec::new();
        }

        // Build lookup: pre -> post
        let mut pre_to_post: HashMap<usize, usize> = HashMap::new();
        let mut is_post: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for pair in &pairs {
            pre_to_post.insert(pair.pre, pair.post);
            is_post.insert(pair.post);
        }

        // Find chain starts: transforms that are a pre but not a post in any pair
        let mut chains = Vec::new();
        for pair in &pairs {
            if !is_post.contains(&pair.pre) {
                // This is a chain start
                let mut chain = vec![pair.pre];
                let mut current = pair.pre;
                while let Some(&next) = pre_to_post.get(&current) {
                    chain.push(next);
                    current = next;
                }
                chains.push(chain);
            }
        }

        chains
    }

    /// Generate xaos changes to link pre -> post.
    ///
    /// This modifies the xaos matrix so that:
    /// - pre routes exclusively to post (all other outgoing weights = 0)
    /// - post is only reachable from pre (all other incoming weights to post = 0)
    /// - post inherits pre's original outgoing weights
    /// - post does not route to itself
    ///
    /// Returns the batch of (ConfigPath, ConfigValue) changes to apply via ConfigManager.
    pub fn link_transforms_changes(&self, pre: usize, post: usize) -> Vec<(crate::config::ConfigPath, crate::config::ConfigValue)> {
        use crate::config::{ConfigPath, ConfigValue};

        let n = self.transforms.len();
        if pre >= n || post >= n || pre == post {
            return Vec::new();
        }

        let mut changes = Vec::new();

        // 1. Copy pre's current outgoing weights to post's row
        for dst in 0..n {
            let weight = self.get_xaos(pre, dst);
            changes.push((
                ConfigPath::Xaos { src: post, dst },
                ConfigValue::Float(weight),
            ));
        }

        // 2. Set pre's row: all zeros except post = 1.0
        for dst in 0..n {
            let weight = if dst == post { 1.0 } else { 0.0 };
            changes.push((
                ConfigPath::Xaos { src: pre, dst },
                ConfigValue::Float(weight),
            ));
        }

        // 3. Set all other transforms' weight to post = 0 (post only reachable from pre)
        for src in 0..n {
            if src != pre {
                changes.push((
                    ConfigPath::Xaos { src, dst: post },
                    ConfigValue::Float(0.0),
                ));
            }
        }

        // 4. Post does not route to itself
        changes.push((
            ConfigPath::Xaos { src: post, dst: post },
            ConfigValue::Float(0.0),
        ));

        changes
    }

    /// Generate xaos changes to unlink a chain.
    ///
    /// For a chain [A, B, C]:
    /// - A gets C's outgoing weights (the last element's routing)
    /// - All intermediate and final elements become freely reachable (incoming weights = 1.0)
    /// - All elements get self-weight restored to 1.0
    ///
    /// For a simple pair [A, B]:
    /// - A gets B's outgoing weights
    /// - B becomes freely reachable
    ///
    /// Returns the batch of (ConfigPath, ConfigValue) changes to apply via ConfigManager.
    pub fn unlink_chain_changes(&self, chain: &[usize]) -> Vec<(crate::config::ConfigPath, crate::config::ConfigValue)> {
        use crate::config::{ConfigPath, ConfigValue};

        let n = self.transforms.len();
        if chain.len() < 2 {
            return Vec::new();
        }

        let mut changes = Vec::new();

        // Restore all chain members to normal routing (all weights = 1.0).
        // The old algorithm copied last's outgoing to first's row, but that
        // produced duplicate keys (first→last = 0.0 from copy, then 1.0 from
        // restore) and the first write won in the batch delta, leaving a stale 0.
        for &idx in chain {
            for dst in 0..n {
                changes.push((
                    ConfigPath::Xaos { src: idx, dst },
                    ConfigValue::Float(1.0),
                ));
            }
            for src in 0..n {
                changes.push((
                    ConfigPath::Xaos { src, dst: idx },
                    ConfigValue::Float(1.0),
                ));
            }
        }

        changes
    }
}

/// A detected linked transform pair
#[derive(Debug, Clone, PartialEq)]
pub struct LinkedPair {
    /// Index of the "pre" transform (routes exclusively to post)
    pub pre: usize,
    /// Index of the "post" transform (only reachable from pre)
    pub post: usize,
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
                let mut transforms = None;
                let mut final_transform = None;
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
                            final_transform = Some(map.next_value()?);
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

                Ok(Flame {
                    name: name.unwrap_or_else(|| default_flame_name()),
                    transforms: transforms.ok_or_else(|| de::Error::missing_field("transforms"))?,
                    final_transform: final_transform.unwrap_or(None),
                    render_mode: render_mode.unwrap_or_default(),
                    perspective_strength: perspective_strength.unwrap_or(0.0),
                    xaos,
                    solo_transform: solo_transform.unwrap_or(None),
                })
            }
        }

        const FIELDS: &[&str] = &["name", "transforms", "final_transform", "render_mode", "perspective_strength", "projection", "xaos", "solo_transform"];
        deserializer.deserialize_struct("Flame", FIELDS, FlameVisitor)
    }
}
