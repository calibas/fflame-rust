use std::collections::HashMap;
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

    /// Color contribution (RGB)
    pub color: [f32; 3],

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
            color: [1.0, 1.0, 1.0],
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

    /// Reset transform to identity (unit triangle at origin)
    pub fn reset_to_identity(&mut self) {
        self.a = 1.0;
        self.b = 0.0;
        self.c = 0.0;
        self.d = 1.0;
        self.e = 0.0;
        self.f = 0.0;
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

/// Custom serialization - saves as HashMap
impl Serialize for Transform {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("Transform", 13)?;
        state.serialize_field("a", &self.a)?;
        state.serialize_field("b", &self.b)?;
        state.serialize_field("c", &self.c)?;
        state.serialize_field("d", &self.d)?;
        state.serialize_field("e", &self.e)?;
        state.serialize_field("f", &self.f)?;
        state.serialize_field("g", &self.g)?;
        state.serialize_field("weight", &self.weight)?;
        state.serialize_field("variations", &self.variations)?;
        state.serialize_field("variation_params", &self.variation_params)?;
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
                        Field::Color => color = Some(map.next_value()?),
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
                    color_speed: color_speed.ok_or_else(|| de::Error::missing_field("color_speed"))?,
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
    use crate::variations::VariationRegistry;

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

/// Projection type for 3D rendering
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ProjectionType {
    /// Orthographic projection (no perspective distortion)
    Orthographic,
    /// Perspective projection with configurable strength
    Perspective { strength: f32 },
}

impl Default for ProjectionType {
    fn default() -> Self {
        Self::Orthographic
    }
}

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flame {
    #[serde(default = "default_flame_name")]
    pub name: String,

    pub transforms: Vec<Transform>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_transform: Option<Transform>,

    /// Rendering mode (2D or 3D)
    #[serde(default)]
    pub render_mode: RenderMode,

    /// Projection type for 3D rendering
    #[serde(default)]
    pub projection: ProjectionType,
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
            projection: ProjectionType::default(),
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

        for transform in &self.transforms {
            for (name, weight) in &transform.variations {
                // Track max weight if variation used in multiple transforms
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
