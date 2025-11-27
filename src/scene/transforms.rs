use std::collections::{HashMap, BTreeMap};
use serde::{Deserialize, Serialize, Deserializer, Serializer};
use serde::de::{self, Visitor, MapAccess};
use crate::variations::VariationRegistry;

/// IFS Transform with named variations (V2)
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

    /// Probability weight for selecting this transform
    pub weight: f32,

    /// Weights for each variation function (named)
    pub variations: HashMap<String, f32>,

    /// Variation parameters (key format: "variation_name.param_name")
    /// Example: "julian.power" -> 3.0
    pub variation_params: HashMap<String, f32>,

    /// Color palette position (0.0 to 1.0)
    /// Represents position in the palette for color coordinate evolution
    pub color: f32,

    /// Color speed / symmetry (-1.0 to 1.0, Apophysis compatibility)
    /// -1.0 = full transform color replacement
    ///  0.0 = 50/50 blend
    ///  1.0 = full inheritance (transform has no color influence)
    pub color_speed: f32,

    /// Opacity / visibility (0.0 to 1.0, Apophysis compatibility)
    /// Controls probability of plotting points from this transform
    /// 1.0 = always plot (default), 0.0 = never plot (invisible)
    pub opacity: f32,
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
        if weight.abs() < 1e-6 {
            self.variations.remove(name);
        } else {
            self.variations.insert(name.to_string(), weight);
        }
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

        let mut state = serializer.serialize_struct("Transform", 14)?;
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
                })
            }
        }

        const FIELDS: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "weight", "variations", "variation_params", "color", "color_speed", "opacity"];
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
                    }
                }

                Ok(Flame {
                    name: name.unwrap_or_else(|| default_flame_name()),
                    transforms: transforms.ok_or_else(|| de::Error::missing_field("transforms"))?,
                    final_transform: final_transform.unwrap_or(None),
                    render_mode: render_mode.unwrap_or_default(),
                    perspective_strength: perspective_strength.unwrap_or(0.0),
                })
            }
        }

        const FIELDS: &[&str] = &["name", "transforms", "final_transform", "render_mode", "perspective_strength", "projection"];
        deserializer.deserialize_struct("Flame", FIELDS, FlameVisitor)
    }
}
