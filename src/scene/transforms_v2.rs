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

    /// Color contribution (RGB)
    pub color: [f32; 3],

    /// Color speed (0.0 = parent color, 1.0 = transform color)
    pub color_speed: f32,
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
            color: [1.0, 1.0, 1.0],
            color_speed: 0.5,
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
}

/// Custom serialization - saves as HashMap
impl Serialize for Transform {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("Transform", 11)?;
        state.serialize_field("a", &self.a)?;
        state.serialize_field("b", &self.b)?;
        state.serialize_field("c", &self.c)?;
        state.serialize_field("d", &self.d)?;
        state.serialize_field("e", &self.e)?;
        state.serialize_field("f", &self.f)?;
        state.serialize_field("g", &self.g)?;
        state.serialize_field("weight", &self.weight)?;
        state.serialize_field("variations", &self.variations)?;
        state.serialize_field("color", &self.color)?;
        state.serialize_field("color_speed", &self.color_speed)?;
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
            A, B, C, D, E, F, G, Weight, Variations, Color, ColorSpeed,
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
                let mut color = None;
                let mut color_speed = None;

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
                        Field::Color => color = Some(map.next_value()?),
                        Field::ColorSpeed => color_speed = Some(map.next_value()?),
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
                    color: color.ok_or_else(|| de::Error::missing_field("color"))?,
                    color_speed: color_speed.ok_or_else(|| de::Error::missing_field("color_speed"))?,
                })
            }
        }

        const FIELDS: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "weight", "variations", "color", "color_speed"];
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
}
