//! Conversion between FractalConfig and API request/response types.
//!
//! The API uses a flat structure while FractalConfig has nested Flame/Palette/ToneCurve.
//! These functions handle the mapping in both directions.

use crate::config::FractalConfig;
use crate::scene::palette::{ColorStop, Palette};
use crate::scene::transforms::{Flame, RenderMode, Transform};

use super::types::*;

// ============================================================================
// Enum conversions: App → API
// ============================================================================

impl From<RenderMode> for ApiRenderMode {
    fn from(mode: RenderMode) -> Self {
        match mode {
            RenderMode::TwoD => ApiRenderMode::TwoD,
            RenderMode::ThreeD => ApiRenderMode::ThreeD,
        }
    }
}

impl From<ApiRenderMode> for RenderMode {
    fn from(mode: ApiRenderMode) -> Self {
        match mode {
            ApiRenderMode::TwoD => RenderMode::TwoD,
            ApiRenderMode::ThreeD => RenderMode::ThreeD,
        }
    }
}

// ============================================================================
// Palette conversion
// ============================================================================

/// Convert our Palette stops to the API stops format (JSON array of {position, color}).
fn palette_stops_to_json(palette: &Palette) -> serde_json::Value {
    let stops: Vec<serde_json::Value> = palette
        .stops
        .iter()
        .map(|s| {
            serde_json::json!({
                "position": s.position,
                "color": s.color,
            })
        })
        .collect();
    serde_json::Value::Array(stops)
}

/// Reconstruct a `Palette` from any content-carrying source (an inline
/// `ApiPalette`, a library entry, etc.). Prefers `stops` for fidelity,
/// falls back to a 256-position interpolation of `color_data`, and
/// finally yields the default palette when neither is present.
fn palette_from_parts(
    name: String,
    stops_value: Option<&serde_json::Value>,
    color_data: Option<&Vec<u32>>,
) -> Palette {
    if let Some(stops_value) = stops_value {
        if let Ok(stops) = serde_json::from_value::<Vec<ColorStop>>(stops_value.clone()) {
            if !stops.is_empty() {
                return Palette {
                    name,
                    stops,
                    locked: false,
                    built_in: false,
                };
            }
        }
    }

    if let Some(color_data) = color_data {
        if color_data.len() >= 3 {
            let num_colors = color_data.len() / 3;
            let stops: Vec<ColorStop> = (0..num_colors)
                .map(|i| {
                    let r = color_data[i * 3] as f32 / 255.0;
                    let g = color_data[i * 3 + 1] as f32 / 255.0;
                    let b = color_data[i * 3 + 2] as f32 / 255.0;
                    ColorStop {
                        position: if num_colors > 1 {
                            i as f32 / (num_colors - 1) as f32
                        } else {
                            0.0
                        },
                        color: [r, g, b],
                    }
                })
                .collect();

            return Palette {
                name,
                stops,
                locked: num_colors == 256,
                built_in: false,
            };
        }
    }

    Palette::fire()
}

/// Reconstruct a `Palette` from an inline `ApiPalette` payload.
pub fn palette_from_api(api: &ApiPalette) -> Palette {
    let name = api.name.clone().unwrap_or_else(|| "API Palette".to_string());
    palette_from_parts(name, api.stops.as_ref(), api.color_data.as_ref())
}

/// Reconstruct a `Palette` from a library entry. Uses the caller's
/// personal `nickname` as the display name, or "Untitled" when none.
pub fn palette_from_library_entry(entry: &LibraryPaletteEntry) -> Palette {
    let name = entry.nickname.clone().unwrap_or_else(|| "Untitled".to_string());
    palette_from_parts(name, entry.stops.as_ref(), entry.color_data.as_ref())
}

// ============================================================================
// FractalConfig → API request
// ============================================================================

/// Pool tag for a root transform, in the order the server stores them.
const TRANSFORM_POOLS: [&str; 3] = ["normal", "linked", "final"];

/// Names of the variations with non-zero weight on a transform. Search/index
/// metadata only — the authoritative variation map lives in the transform's
/// `data` blob. Order is unspecified (HashMap iteration); the server treats
/// `variation_names` as a set.
fn nonzero_variation_names(t: &Transform) -> Vec<String> {
    t.variations
        .iter()
        .filter(|(_, &w)| w != 0.0)
        .map(|(name, _)| name.clone())
        .collect()
}

/// Split a flame's three root transform pools into the flat wire array. Each
/// transform's full state serializes opaquely into `data` (the same JSON a
/// `Transform` holds in a `.fflame`); `kind` + `sort_order` let the reader
/// rebuild the pools.
fn root_transforms_to_wire(flame: &Flame) -> Result<Vec<ApiTransformWire>, serde_json::Error> {
    let pools = [
        &flame.transforms,
        &flame.linked_transforms,
        &flame.final_transforms,
    ];
    let mut out = Vec::new();
    for (kind, pool) in TRANSFORM_POOLS.iter().zip(pools.iter()) {
        for (i, t) in pool.iter().enumerate() {
            out.push(ApiTransformWire {
                kind: kind.to_string(),
                sort_order: i as i32,
                variation_names: nonzero_variation_names(t),
                data: serde_json::to_value(t)?,
            });
        }
    }
    Ok(out)
}

/// Convert a `FractalConfig` to a `CreateFlameRequest`. The config becomes an
/// opaque blob (the `.fflame` JSON minus the root transforms and minus the
/// palette); the root transforms split into the flat `transforms[]` array;
/// the palette travels inline (server hashes by content). Subflames — and
/// their own transforms — ride inside the blob untouched.
pub fn config_to_create_request(
    config: &FractalConfig,
    name: Option<&str>,
) -> Result<CreateFlameRequest, serde_json::Error> {
    let flame = &config.flame;
    let transforms = root_transforms_to_wire(flame)?;

    // Blob = canonical config value (versioned, defaults stripped) with the
    // palette removed (sent inline) and the ROOT transform pools emptied
    // (sent as `transforms[]`). Subflames keep their inline transforms.
    let mut blob = config.to_json_value()?;
    if let Some(obj) = blob.as_object_mut() {
        obj.remove("palette");
        if let Some(flame_obj) = obj.get_mut("flame").and_then(|f| f.as_object_mut()) {
            flame_obj.insert("transforms".to_string(), serde_json::json!([]));
            flame_obj.insert("linked_transforms".to_string(), serde_json::json!([]));
            flame_obj.insert("final_transforms".to_string(), serde_json::json!([]));
        }
    }

    Ok(CreateFlameRequest {
        name: name
            .map(|n| n.to_string())
            .unwrap_or_else(|| flame.name.clone()),
        visibility: None, // Set by caller if needed
        palette: Some(palette_to_api(&config.palette)),
        config: blob,
        transforms,
    })
}

/// Encode a palette as either compact color_data (for 256-stop indexed palettes)
/// or as JSON stops (for gradient palettes with arbitrary positions).
/// Returns (stops, color_data).
fn encode_palette_data(palette: &Palette) -> (Option<serde_json::Value>, Option<Vec<u32>>) {
    if palette.stops.len() == 256 && palette.stops.iter().enumerate().all(|(i, s)| {
        (s.position - i as f32 / 255.0).abs() < 0.001
    }) {
        // Packed R,G,B,R,G,B,... as u32 (0-255)
        let color_data: Vec<u32> = palette
            .stops
            .iter()
            .flat_map(|s| {
                [
                    (s.color[0] * 255.0).round() as u32,
                    (s.color[1] * 255.0).round() as u32,
                    (s.color[2] * 255.0).round() as u32,
                ]
            })
            .collect();
        (None, Some(color_data))
    } else {
        (Some(palette_stops_to_json(palette)), None)
    }
}

/// Build an inline `ApiPalette` payload from a local `Palette`. The
/// server computes the content hash and assigns it on the response, so
/// the client never sends `hash`. The local palette name travels as the
/// flame-specific display label.
pub fn palette_to_api(palette: &Palette) -> ApiPalette {
    let (stops, color_data) = encode_palette_data(palette);
    ApiPalette {
        hash: None,
        name: Some(palette.name.clone()),
        color_data,
        stops,
        avg_color_r: None,
        avg_color_g: None,
        avg_color_b: None,
        dominant_hue: None,
        color_count: None,
    }
}

// ============================================================================
// API response → FractalConfig
// ============================================================================

/// Bucket the flat root-transform wire array back into the three pools,
/// each sorted by `sort_order`, returning the transforms' opaque `data`
/// values ready to slot into the flame blob. Unknown `kind` values fall
/// into the normal pool (forward-compatible with a future server that
/// adds pool kinds).
fn wire_transforms_to_pools(
    wires: &[ApiTransformWire],
) -> (Vec<serde_json::Value>, Vec<serde_json::Value>, Vec<serde_json::Value>) {
    let mut pools: [Vec<&ApiTransformWire>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for w in wires {
        let idx = match w.kind.as_str() {
            "linked" => 1,
            "final" => 2,
            _ => 0,
        };
        pools[idx].push(w);
    }
    for pool in &mut pools {
        pool.sort_by_key(|w| w.sort_order);
    }
    let [normal, linked, finals] = pools;
    let data = |pool: Vec<&ApiTransformWire>| pool.into_iter().map(|w| w.data.clone()).collect();
    (data(normal), data(linked), data(finals))
}

/// Convert an API `FlameResponse` back into a `FractalConfig`. Re-injects the
/// root transforms (from the flat `transforms[]`) and the palette (from the
/// inline `palette` payload) into the opaque config blob, then deserializes
/// through the shared version-keyed migration path (`from_json_value`) so
/// cloud blobs and local `.fflame` files agree. Subflame transforms already
/// ride inside the blob and need no special handling.
pub fn flame_response_to_config(resp: &FlameResponse) -> Result<FractalConfig, serde_json::Error> {
    let mut blob = resp.config.clone();
    let obj = blob.as_object_mut().ok_or_else(|| {
        serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Flame config blob is not a JSON object",
        ))
    })?;

    // Recovery for the flattened-v2 API data bug: a migration merged the
    // flame's fields into the config top level, so there is no "flame" object
    // (a real v3 blob always has one). The scene-render fields — including
    // `render_mode` — already sit at the top level (their v3 home), so we just
    // rebuild a flame to hold the root transforms (still carried in
    // `resp.transforms`) and let everything else deserialize from the top
    // level. Non-transform flame state (xaos, solo_transform, post_symmetry,
    // subflames) was merged into junk top-level keys and is unrecoverable — it
    // falls back to defaults. Stamp the current version so the v2→v3 lift does
    // NOT run: it would look for `render_mode` under the now-empty flame, miss
    // it, default to "2d", and clobber the correct top-level value.
    if !obj.contains_key("flame") {
        obj.insert("flame".to_string(), serde_json::json!({}));
        obj.insert(
            "version".to_string(),
            serde_json::json!(crate::config::CURRENT_CONFIG_VERSION),
        );
        obj.remove("config_version");
    }

    let (normal, linked, finals) = wire_transforms_to_pools(&resp.transforms);
    if let Some(flame_obj) = obj.get_mut("flame").and_then(|f| f.as_object_mut()) {
        flame_obj.insert("transforms".to_string(), serde_json::Value::Array(normal));
        flame_obj.insert(
            "linked_transforms".to_string(),
            serde_json::Value::Array(linked),
        );
        flame_obj.insert(
            "final_transforms".to_string(),
            serde_json::Value::Array(finals),
        );
        // The server `name` column is authoritative — in v2 the cloud title
        // and `Flame::name` are a single field.
        flame_obj.insert("name".to_string(), serde_json::json!(resp.name));
    }

    // Re-attach the palette (sent inline, kept out of the blob).
    if let Some(api_palette) = &resp.palette {
        let palette = palette_from_api(api_palette);
        obj.insert("palette".to_string(), serde_json::to_value(&palette)?);
    }

    FractalConfig::from_json_value(blob)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_mode_roundtrip() {
        assert_eq!(RenderMode::from(ApiRenderMode::from(RenderMode::TwoD)), RenderMode::TwoD);
        assert_eq!(RenderMode::from(ApiRenderMode::from(RenderMode::ThreeD)), RenderMode::ThreeD);
    }

    #[test]
    fn test_render_mode_serialization() {
        assert_eq!(serde_json::to_string(&ApiRenderMode::TwoD).unwrap(), "\"2d\"");
        assert_eq!(serde_json::to_string(&ApiRenderMode::ThreeD).unwrap(), "\"3d\"");
    }

    /// Mirror a `CreateFlameRequest` back the way the server would on read:
    /// blob + transforms stored verbatim, palette echoed. Lets the tests
    /// exercise the full save → load round trip without a live server.
    fn mirror_as_response(req: CreateFlameRequest) -> FlameResponse {
        FlameResponse {
            id: "test-id".to_string(),
            user_id: "test-user".to_string(),
            name: req.name,
            visibility: req.visibility,
            palette: req.palette,
            config: req.config,
            transforms: req.transforms,
            animation_count: 0,
            animations: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn test_blob_request_strips_palette_and_root_transforms() {
        let mut config = FractalConfig::default();
        config.flame.transforms.push(Transform::new());
        config.flame.final_transforms.push(Transform::new());

        let req = config_to_create_request(&config, Some("Test Flame")).unwrap();

        assert_eq!(req.name, "Test Flame");
        // Palette left the blob (sent inline).
        assert!(req.palette.is_some());
        assert!(req.config.get("palette").is_none());
        // Root transform pools emptied in the blob, moved to `transforms[]`.
        let blob_flame = req.config.get("flame").unwrap();
        assert_eq!(blob_flame["transforms"].as_array().unwrap().len(), 0);
        assert_eq!(blob_flame["final_transforms"].as_array().unwrap().len(), 0);
        assert_eq!(req.transforms.iter().filter(|t| t.kind == "normal").count(), 1);
        assert_eq!(req.transforms.iter().filter(|t| t.kind == "final").count(), 1);
        // Blob carries the version header for migration.
        assert!(req.config.get("version").is_some());
    }

    #[test]
    fn test_blob_roundtrip_preserves_drift_fields() {
        let mut config = FractalConfig::default();
        let mut t = Transform::new();
        t.variations.insert("linear".to_string(), 1.0);
        t.variations.insert("spherical".to_string(), 0.5);
        config.flame.transforms.push(t);
        let mut tf = Transform::new();
        tf.variations.insert("swirl".to_string(), 1.0);
        config.flame.final_transforms.push(tf);
        config.flame.name = "Round Trip".to_string();
        config.zoom = 3.5;
        config.gamma = 2.2;
        // Fields the old per-field wire format silently dropped — must
        // survive now that the whole config rides in the blob.
        config.camera_bank = 0.25;
        config.camera_x = 1.5;
        config.preserve_z = true;

        let req = config_to_create_request(&config, Some("Round Trip")).unwrap();
        let resp = mirror_as_response(req);
        let restored = flame_response_to_config(&resp).unwrap();

        assert_eq!(restored.flame.name, "Round Trip");
        assert_eq!(restored.zoom, 3.5);
        assert_eq!(restored.gamma, 2.2);
        assert_eq!(restored.camera_bank, 0.25);
        assert_eq!(restored.camera_x, 1.5);
        assert!(restored.preserve_z);
        assert_eq!(restored.flame.transforms.len(), 1);
        assert_eq!(restored.flame.final_transforms.len(), 1);
        assert_eq!(
            restored.flame.transforms[0].variations.get("spherical"),
            Some(&0.5)
        );
        assert_eq!(
            restored.flame.final_transforms[0].variations.get("swirl"),
            Some(&1.0)
        );
    }

    #[test]
    fn test_response_migrates_v1_blob() {
        // A blob written by an older (v1) client still loads: the shared
        // version-keyed migration runs before deserialize.
        let mut t = Transform::new();
        t.variations.insert("linear".to_string(), 1.0);
        let resp = FlameResponse {
            id: "x".into(),
            user_id: "x".into(),
            name: "Old".into(),
            visibility: None,
            palette: None,
            config: serde_json::json!({ "version": 1, "flame": { "name": "ignored" } }),
            transforms: vec![ApiTransformWire {
                kind: "normal".into(),
                sort_order: 0,
                variation_names: vec!["linear".into()],
                data: serde_json::to_value(&t).unwrap(),
            }],
            animation_count: 0,
            animations: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let config = flame_response_to_config(&resp).unwrap();
        // Server `name` column is authoritative for the flame name.
        assert_eq!(config.flame.name, "Old");
        assert_eq!(config.flame.transforms.len(), 1);
        assert_eq!(
            config.flame.transforms[0].variations.get("linear"),
            Some(&1.0)
        );
    }

    #[test]
    fn test_response_accepts_api_snake_case_enums() {
        // The server's migrated blobs carry enum values in the API's
        // snake_case casing (`"palette"`, `"density"`, `"2d"`, ...). Those
        // must still deserialize into the config's PascalCase enums.
        use crate::scene::palette::{
            ColorMode, PathCaptureMode, PathMapStyle, PathTrackingMode, SqueezeMode,
        };
        use crate::scene::tonemap::{HighlightMode, ToneMapMode};

        let mut t = Transform::new();
        t.variations.insert("linear".to_string(), 1.0);
        let resp = FlameResponse {
            id: "x".into(),
            user_id: "x".into(),
            name: "Snake".into(),
            visibility: None,
            palette: None,
            config: serde_json::json!({
                "version": 2,
                "flame": { "name": "Snake", "render_mode": "3d" },
                "color_mode": "path_map",
                "tonemap_mode": "density",
                "highlight_mode": "max_norm",
                "palette_squeeze_mode": "geometric",
                "path_map_style": "origin_radial",
                "path_capture_mode": "first_after_burn_in",
                "path_tracking_mode": "recent"
            }),
            transforms: vec![ApiTransformWire {
                kind: "normal".into(),
                sort_order: 0,
                variation_names: vec!["linear".into()],
                data: serde_json::to_value(&t).unwrap(),
            }],
            animation_count: 0,
            animations: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let config = flame_response_to_config(&resp).unwrap();
        assert_eq!(config.render_mode, RenderMode::ThreeD);
        assert_eq!(config.color_mode, ColorMode::PathMap);
        assert_eq!(config.tonemap_mode, ToneMapMode::DensityVisualization);
        assert_eq!(config.highlight_mode, HighlightMode::MaxNorm);
        assert_eq!(config.palette_squeeze_mode, SqueezeMode::Geometric);
        assert_eq!(config.path_map_style, PathMapStyle::OriginRadial);
        assert_eq!(config.path_capture_mode, PathCaptureMode::FirstAfterBurnIn);
        assert_eq!(config.path_tracking_mode, PathTrackingMode::Recent);
    }

    #[test]
    fn test_flattened_v2_blob_recovered() {
        // Simulate the flattened-v2 API data bug: the flame's fields were
        // merged into the config top level, so there is no "flame" object.
        // render_mode survived at the top level (its v3 home); the root
        // transforms still arrive in `resp.transforms`.
        let mut flat = serde_json::to_value(FractalConfig::default()).unwrap();
        let obj = flat.as_object_mut().unwrap();
        obj.remove("flame"); // merged up by the bug
        obj.remove("version");
        obj.insert("config_version".into(), serde_json::json!(2));
        obj.insert("render_mode".into(), serde_json::json!("3d"));
        obj.insert("zoom".into(), serde_json::json!(2.5));
        // Junk flame fields that got merged to the top level — must be ignored.
        obj.insert("xaos".into(), serde_json::json!([[1.0]]));
        obj.insert("solo_transform".into(), serde_json::json!(0));

        let mut t = Transform::new();
        t.variations.insert("linear".to_string(), 1.0);
        let resp = FlameResponse {
            id: "x".into(),
            user_id: "x".into(),
            name: "Recovered".into(),
            visibility: None,
            palette: None,
            config: flat,
            transforms: vec![ApiTransformWire {
                kind: "normal".into(),
                sort_order: 0,
                variation_names: vec!["linear".into()],
                data: serde_json::to_value(&t).unwrap(),
            }],
            animation_count: 0,
            animations: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let config = flame_response_to_config(&resp).unwrap();
        // Flame rebuilt with the root transforms; render_mode preserved (NOT
        // clobbered to 2d by a spurious v2→v3 lift); other config fields kept.
        assert_eq!(config.flame.transforms.len(), 1);
        assert_eq!(
            config.flame.transforms[0].variations.get("linear"),
            Some(&1.0)
        );
        assert_eq!(config.render_mode, RenderMode::ThreeD);
        assert_eq!(config.zoom, 2.5);
        assert_eq!(config.flame.name, "Recovered");
    }
}

// ============================================================================
// Animation conversion: App ↔ API
// ============================================================================

/// Convert an Animation to a CreateAnimationRequest.
///
/// Serializes tracks, generators, and base_config as opaque JSON values —
/// the API stores them verbatim without interpreting the structure.
pub fn animation_to_create_request(
    animation: &crate::animation::Animation,
    name: Option<&str>,
    flame_id: Option<&str>,
    visibility: Option<ApiVisibility>,
) -> CreateAnimationRequest {
    CreateAnimationRequest {
        name: name.unwrap_or(&animation.name).to_string(),
        flame_id: flame_id.map(|s| s.to_string()),
        duration: Some(animation.duration),
        loop_mode: Some(animation.loop_mode.into()),
        tracks: serde_json::to_value(&animation.tracks).ok(),
        generators: serde_json::to_value(&animation.generators).ok(),
        // Stamp the config version (v3) via the canonical serializer so the
        // embedded base_config round-trips through the migration on load.
        base_config: animation.base_config.as_ref().and_then(|c| c.to_json_value().ok()),
        visibility,
    }
}

/// Convert an AnimationResponse back to an Animation.
///
/// Deserializes tracks, generators, and base_config from the opaque JSON
/// values stored on the server.
pub fn animation_response_to_animation(resp: &AnimationResponse) -> crate::animation::Animation {
    let tracks = resp.tracks.as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let generators = resp.generators.as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // Embedded base_config carries a config version (v3+) when saved by a
    // current client; legacy animations have no version ⇒ assume v2 (the
    // format in use when the raw struct serializer was the only path), then
    // migrate to current.
    let base_config = resp.base_config.as_ref()
        .and_then(|v| FractalConfig::from_json_value_with_default_version(v.clone(), 2).ok());

    crate::animation::Animation {
        name: resp.name.clone(),
        base_config,
        duration: resp.duration,
        tracks,
        generators,
        loop_mode: resp.loop_mode.into(),
    }
}
