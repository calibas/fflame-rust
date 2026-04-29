//! Conversion between FractalConfig and API request/response types.
//!
//! The API uses a flat structure while FractalConfig has nested Flame/Palette/ToneCurve.
//! These functions handle the mapping in both directions.

use std::collections::HashMap;

use crate::config::FractalConfig;
use crate::effects::EffectInstance;
use crate::scene::palette::{ColorMode, ColorStop, Palette, PathCaptureMode, PathMapStyle, PathTrackingMode};
use crate::scene::tonemap::{ToneCurve, ToneMapMode};
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

impl From<ColorMode> for ApiColorMode {
    fn from(mode: ColorMode) -> Self {
        match mode {
            ColorMode::Palette => ApiColorMode::Palette,
            ColorMode::Speed => ApiColorMode::Speed,
            ColorMode::PathMap => ApiColorMode::PathMap,
        }
    }
}

impl From<ApiColorMode> for ColorMode {
    fn from(mode: ApiColorMode) -> Self {
        match mode {
            ApiColorMode::Palette => ColorMode::Palette,
            ApiColorMode::Speed => ColorMode::Speed,
            ApiColorMode::PathMap => ColorMode::PathMap,
        }
    }
}

impl From<ToneMapMode> for ApiToneMapMode {
    fn from(mode: ToneMapMode) -> Self {
        match mode {
            ToneMapMode::Linear => ApiToneMapMode::Linear,
            ToneMapMode::Logarithmic => ApiToneMapMode::Logarithmic,
            ToneMapMode::DensityVisualization => ApiToneMapMode::Density,
        }
    }
}

impl From<ApiToneMapMode> for ToneMapMode {
    fn from(mode: ApiToneMapMode) -> Self {
        match mode {
            ApiToneMapMode::Linear => ToneMapMode::Linear,
            ApiToneMapMode::Logarithmic => ToneMapMode::Logarithmic,
            ApiToneMapMode::Density => ToneMapMode::DensityVisualization,
        }
    }
}

impl From<PathMapStyle> for ApiPathMapStyle {
    fn from(style: PathMapStyle) -> Self {
        match style {
            PathMapStyle::Prefix => ApiPathMapStyle::Prefix,
            PathMapStyle::Suffix => ApiPathMapStyle::Suffix,
            PathMapStyle::PrefixDistinct => ApiPathMapStyle::PrefixDistinct,
            PathMapStyle::SuffixDistinct => ApiPathMapStyle::SuffixDistinct,
            PathMapStyle::Depth => ApiPathMapStyle::Depth,
            PathMapStyle::OriginRadial => ApiPathMapStyle::OriginRadial,
            PathMapStyle::OriginHorizontal => ApiPathMapStyle::OriginHorizontal,
            PathMapStyle::OriginVertical => ApiPathMapStyle::OriginVertical,
        }
    }
}

impl From<ApiPathMapStyle> for PathMapStyle {
    fn from(style: ApiPathMapStyle) -> Self {
        match style {
            ApiPathMapStyle::Prefix => PathMapStyle::Prefix,
            ApiPathMapStyle::Suffix => PathMapStyle::Suffix,
            ApiPathMapStyle::PrefixDistinct => PathMapStyle::PrefixDistinct,
            ApiPathMapStyle::SuffixDistinct => PathMapStyle::SuffixDistinct,
            ApiPathMapStyle::Depth => PathMapStyle::Depth,
            ApiPathMapStyle::OriginRadial => PathMapStyle::OriginRadial,
            ApiPathMapStyle::OriginHorizontal => PathMapStyle::OriginHorizontal,
            ApiPathMapStyle::OriginVertical => PathMapStyle::OriginVertical,
        }
    }
}

impl From<PathCaptureMode> for ApiPathCaptureMode {
    fn from(mode: PathCaptureMode) -> Self {
        match mode {
            PathCaptureMode::FirstHit => ApiPathCaptureMode::FirstHit,
            PathCaptureMode::FirstAfterBurnIn => ApiPathCaptureMode::FirstAfterBurnIn,
            PathCaptureMode::LastHit => ApiPathCaptureMode::LastHit,
        }
    }
}

impl From<ApiPathCaptureMode> for PathCaptureMode {
    fn from(mode: ApiPathCaptureMode) -> Self {
        match mode {
            ApiPathCaptureMode::FirstHit => PathCaptureMode::FirstHit,
            ApiPathCaptureMode::FirstAfterBurnIn => PathCaptureMode::FirstAfterBurnIn,
            ApiPathCaptureMode::LastHit => PathCaptureMode::LastHit,
        }
    }
}

impl From<PathTrackingMode> for ApiPathTrackingMode {
    fn from(mode: PathTrackingMode) -> Self {
        match mode {
            PathTrackingMode::First => ApiPathTrackingMode::First,
            PathTrackingMode::Recent => ApiPathTrackingMode::Recent,
        }
    }
}

impl From<ApiPathTrackingMode> for PathTrackingMode {
    fn from(mode: ApiPathTrackingMode) -> Self {
        match mode {
            ApiPathTrackingMode::First => PathTrackingMode::First,
            ApiPathTrackingMode::Recent => PathTrackingMode::Recent,
        }
    }
}

// ============================================================================
// Transform conversion
// ============================================================================

fn transform_to_api(t: &Transform) -> CreateTransformInput {
    CreateTransformInput {
        a: Some(t.a),
        b: Some(t.b),
        c: Some(t.c),
        d: Some(t.d),
        e: Some(t.e),
        f: Some(t.f),
        g: Some(t.g),
        weight: Some(t.weight),
        color: Some(t.color),
        color_speed: Some(t.color_speed),
        opacity: Some(t.opacity),
        direct_color: if t.direct_color.abs() > 1e-6 { Some(t.direct_color) } else { None },
        variations: if t.variations.is_empty() { None } else { Some(t.variations.clone()) },
        variation_params: if t.variation_params.is_empty() { None } else { Some(t.variation_params.clone()) },
        post_affine_enabled: Some(t.post_affine_enabled),
        post_a: Some(t.post_a),
        post_b: Some(t.post_b),
        post_c: Some(t.post_c),
        post_d: Some(t.post_d),
        post_e: Some(t.post_e),
        post_f: Some(t.post_f),
        post_g: Some(t.post_g),
    }
}

fn transform_from_api(resp: &TransformResponse) -> Transform {
    Transform {
        a: resp.a,
        b: resp.b,
        c: resp.c,
        d: resp.d,
        e: resp.e,
        f: resp.f,
        g: resp.g,
        weight: resp.weight,
        color: resp.color,
        color_speed: resp.color_speed,
        opacity: resp.opacity,
        direct_color: resp.direct_color,
        variations: resp.variations.clone(),
        variation_params: resp.variation_params.clone(),
        post_affine_enabled: resp.post_affine_enabled,
        post_a: resp.post_a,
        post_b: resp.post_b,
        post_c: resp.post_c,
        post_d: resp.post_d,
        post_e: resp.post_e,
        post_f: resp.post_f,
        post_g: resp.post_g,
    }
}

// ============================================================================
// Effect conversion
// ============================================================================

fn effect_to_api(e: &EffectInstance) -> CreateEffectInput {
    let params = if e.params.is_empty() {
        None
    } else {
        // Convert HashMap<String, f32> to JSON value
        Some(serde_json::to_value(&e.params).unwrap_or(serde_json::Value::Null))
    };

    CreateEffectInput {
        effect_name: e.effect_type.clone(),
        params,
        enabled: e.enabled,
    }
}

fn effect_from_api(resp: &ConfigEffectResponse) -> EffectInstance {
    let params = resp
        .params
        .as_ref()
        .and_then(|v| serde_json::from_value::<HashMap<String, f32>>(v.clone()).ok())
        .unwrap_or_default();

    EffectInstance {
        effect_type: resp.effect_name.clone(),
        enabled: resp.enabled,
        params,
    }
}

// ============================================================================
// ToneCurve conversion
// ============================================================================

fn tonecurve_to_json(curve: &ToneCurve) -> serde_json::Value {
    serde_json::to_value(curve).unwrap_or(serde_json::Value::Null)
}

fn tonecurve_from_json(value: Option<&serde_json::Value>) -> ToneCurve {
    value
        .and_then(|v| serde_json::from_value::<ToneCurve>(v.clone()).ok())
        .unwrap_or_default()
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

/// Reconstruct a Palette from API palette response data.
pub fn palette_from_api(resp: &PaletteResponse) -> Palette {
    let name = resp.name.clone().unwrap_or_else(|| "API Palette".to_string());

    // Try to reconstruct from stops first
    if let Some(stops_value) = &resp.stops {
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

    // Fall back to color_data (packed u32 array: R,G,B,R,G,B,...)
    if let Some(color_data) = &resp.color_data {
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

    // Last resort: default palette
    Palette::fire()
}

// ============================================================================
// FractalConfig → API request
// ============================================================================

/// Convert a FractalConfig to an API CreateFlameRequest.
///
/// Returns the flame request and optionally a palette request if the palette
/// should be saved separately (when no palette_id exists yet).
pub fn config_to_create_request(config: &FractalConfig, name: Option<&str>) -> CreateFlameRequest {
    let flame = &config.flame;

    CreateFlameRequest {
        name: name.map(|n| n.to_string()).unwrap_or_else(|| flame.name.clone()),
        transforms: flame.transforms.iter().map(transform_to_api).collect(),
        visibility: None, // Set by caller if needed
        final_transform: flame.final_transform.as_ref().map(transform_to_api),
        render_mode: Some(flame.render_mode.into()),
        perspective_strength: Some(flame.perspective_strength),
        xaos: flame.xaos.as_ref().map(|x| serde_json::to_value(x).unwrap_or(serde_json::Value::Null)),
        solo_transform: flame.solo_transform.map(|i| i as i32),

        zoom: Some(config.zoom),
        pan_x: Some(config.pan_x),
        pan_y: Some(config.pan_y),
        rotation: Some(config.rotation),
        camera_rotation_x: Some(config.camera_rotation_x),
        camera_rotation_y: Some(config.camera_rotation_y),
        camera_z: Some(config.camera_z),

        dof_focus_distance: Some(config.dof_focus_distance),
        dof_blur_strength: Some(config.dof_blur_strength),
        fog_strength: Some(config.fog_strength),
        fog_start: Some(config.fog_start),

        density_scale: Some(config.density_scale),
        speed_factor: Some(config.speed_factor),
        max_iterations: Some(config.max_iterations),
        histogram_color_scale: Some(config.histogram_color_scale),
        blend_factor: Some(config.blend_factor),
        use_dynamic_blend: Some(config.use_dynamic_blend),
        target_iterations_per_pixel: Some(config.target_iterations_per_pixel as i32),

        color_mode: Some(config.color_mode.into()),
        path_map_style: Some(config.path_map_style.into()),
        path_capture_mode: Some(config.path_capture_mode.into()),
        path_tracking_mode: Some(config.path_tracking_mode.into()),
        palette_id: None, // Will be set after palette is saved
        palette_rotation: Some(config.palette_rotation),
        palette_size: Some(config.palette_size as i32),
        palette_squeeze: Some(config.palette_squeeze),
        background_color: Some(config.background_color.to_vec()),

        tonemap_mode: Some(config.tonemap_mode.into()),
        tonemap_curve: Some(tonecurve_to_json(&config.tonemap_curve)),
        use_curve: Some(config.use_curve),
        exposure: Some(config.exposure),
        gamma: Some(config.gamma),
        gamma_threshold: Some(config.gamma_threshold),
        brightness: Some(config.brightness),
        vibrancy: Some(config.vibrancy),
        saturation: Some(config.saturation),
        hue_shift: Some(config.hue_shift),
        alpha_blend_low: Some(config.alpha_blend_low),
        alpha_blend_high: Some(config.alpha_blend_high),
        levels_low: Some(config.levels_low),
        levels_high: Some(config.levels_high),
        levels_gamma: Some(config.levels_gamma),

        density_effects: if config.density_effects.is_empty() {
            None
        } else {
            Some(config.density_effects.iter().map(effect_to_api).collect())
        },
        color_effects: if config.color_effects.is_empty() {
            None
        } else {
            Some(config.color_effects.iter().map(effect_to_api).collect())
        },

        deterministic_rng: Some(config.deterministic_rng),
    }
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

/// Compute the palette content hash matching the server's `palette_content_hash()`.
///
/// SHA-256 over:
///   (0x63 + color_data_bytes | 0x43) || (0x73 + stops_utf8 | 0x53)
///
/// `color_data` bytes are raw u8 channel values (R,G,B,R,G,B,...).
/// `stops` text must match PostgreSQL's `jsonb::text` canonical format
/// (sorted keys, space after `:` and `,`).
pub fn compute_palette_hash(palette: &Palette) -> String {
    use sha2::{Sha256, Digest};

    let (stops_json, color_data) = encode_palette_data(palette);

    let mut hasher = Sha256::new();

    // color_data component
    match color_data {
        Some(ref data) => {
            hasher.update([0x63]); // tag: color_data present
            // Convert u32 values (0-255) to raw bytes — matches BYTEA storage
            let bytes: Vec<u8> = data.iter().map(|&v| v as u8).collect();
            hasher.update(&bytes);
        }
        None => {
            hasher.update([0x43]); // tag: color_data NULL
        }
    }

    // stops component
    match stops_json {
        Some(ref val) => {
            hasher.update([0x73]); // tag: stops present
            let text = jsonb_text(val);
            hasher.update(text.as_bytes());
        }
        None => {
            hasher.update([0x53]); // tag: stops NULL
        }
    }

    // Zero-pad each byte to 2 hex digits — matches PostgreSQL encode(..., 'hex')
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Format a serde_json::Value to match PostgreSQL's `jsonb::text` canonical output.
///
/// PostgreSQL JSONB text format: sorted keys, space after `:` and `,`, no newlines.
/// e.g. `{"color": [1.0, 0.0, 0.0], "position": 0.0}`
fn jsonb_text(val: &serde_json::Value) -> String {
    use serde::Serialize;

    struct PostgresJsonFormatter;

    impl serde_json::ser::Formatter for PostgresJsonFormatter {
        fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
        where W: ?Sized + std::io::Write {
            if first { Ok(()) } else { writer.write_all(b", ") }
        }

        fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
        where W: ?Sized + std::io::Write {
            if first { Ok(()) } else { writer.write_all(b", ") }
        }

        fn begin_object_value<W>(&mut self, writer: &mut W) -> std::io::Result<()>
        where W: ?Sized + std::io::Write {
            writer.write_all(b": ")
        }
    }

    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, PostgresJsonFormatter);
    val.serialize(&mut ser).unwrap();
    String::from_utf8(buf).unwrap()
}

/// Create a palette request from a FractalConfig's palette.
pub fn palette_to_create_request(palette: &Palette, visibility: Option<ApiPaletteVisibility>) -> CreatePaletteRequest {
    let vis = visibility.unwrap_or(ApiPaletteVisibility::Private);
    let (stops, color_data) = encode_palette_data(palette);
    CreatePaletteRequest {
        visibility: vis,
        name: Some(palette.name.clone()),
        stops,
        color_data,
        metadata: None,
    }
}

// ============================================================================
// API response → FractalConfig
// ============================================================================

/// Convert an API FlameResponse to a FractalConfig.
///
/// Optionally provide a PaletteResponse to reconstruct the palette.
/// If no palette is provided, the default palette is used.
pub fn flame_response_to_config(
    resp: &FlameResponse,
    palette_resp: Option<&PaletteResponse>,
) -> FractalConfig {
    // Reconstruct transforms (sorted by sort_order, excluding final transform)
    let mut transforms: Vec<TransformResponse> = resp
        .transforms
        .iter()
        .filter(|t| !t.is_final_transform)
        .cloned()
        .collect();
    transforms.sort_by_key(|t| t.sort_order);

    let final_transform = resp.final_transform.as_ref().map(transform_from_api);

    // Reconstruct xaos from JSON value
    let xaos: Option<Vec<Vec<f32>>> = resp
        .xaos
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let flame = Flame {
        name: resp.name.clone(),
        transforms: transforms.iter().map(transform_from_api).collect(),
        final_transform,
        render_mode: resp.render_mode.into(),
        perspective_strength: resp.perspective_strength,
        xaos,
        solo_transform: resp.solo_transform.map(|i| i as usize),
    };

    // Reconstruct palette
    let palette = palette_resp
        .map(palette_from_api)
        .unwrap_or_else(Palette::fire);

    // Background color: API sends Vec<f32>, we need [f32; 3]
    let background_color = if resp.background_color.len() >= 3 {
        [
            resp.background_color[0],
            resp.background_color[1],
            resp.background_color[2],
        ]
    } else {
        [0.0, 0.0, 0.0]
    };

    FractalConfig {
        flame,
        zoom: resp.zoom,
        pan_x: resp.pan_x,
        pan_y: resp.pan_y,
        rotation: resp.rotation,
        camera_rotation_x: resp.camera_rotation_x,
        camera_rotation_y: resp.camera_rotation_y,
        camera_z: resp.camera_z,
        dof_focus_distance: resp.dof_focus_distance,
        dof_blur_strength: resp.dof_blur_strength,
        fog_strength: resp.fog_strength,
        fog_start: resp.fog_start,
        density_scale: resp.density_scale,
        speed_factor: resp.speed_factor,
        max_iterations: resp.max_iterations,
        histogram_color_scale: resp.histogram_color_scale,
        blend_factor: resp.blend_factor,
        use_dynamic_blend: resp.use_dynamic_blend,
        target_iterations_per_pixel: resp.target_iterations_per_pixel as u32,
        color_mode: resp.color_mode.into(),
        path_map_style: resp.path_map_style.into(),
        path_capture_mode: resp.path_capture_mode.into(),
        path_tracking_mode: resp.path_tracking_mode.into(),
        palette,
        palette_rotation: resp.palette_rotation,
        palette_size: resp.palette_size as u32,
        palette_squeeze: resp.palette_squeeze,
        background_color,
        tonemap_mode: resp.tonemap_mode.into(),
        tonemap_curve: tonecurve_from_json(resp.tonemap_curve.as_ref()),
        use_curve: resp.use_curve,
        exposure: resp.exposure,
        gamma: resp.gamma,
        gamma_threshold: resp.gamma_threshold,
        brightness: resp.brightness,
        vibrancy: resp.vibrancy,
        saturation: resp.saturation,
        hue_shift: resp.hue_shift,
        alpha_blend_low: resp.alpha_blend_low,
        alpha_blend_high: resp.alpha_blend_high,
        levels_low: resp.levels_low,
        levels_high: resp.levels_high,
        levels_gamma: resp.levels_gamma,
        density_effects: resp.density_effects.iter().map(effect_from_api).collect(),
        color_effects: resp.color_effects.iter().map(effect_from_api).collect(),
        deterministic_rng: resp.deterministic_rng,
    }
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
    fn test_color_mode_roundtrip() {
        assert_eq!(ColorMode::from(ApiColorMode::from(ColorMode::Palette)), ColorMode::Palette);
        assert_eq!(ColorMode::from(ApiColorMode::from(ColorMode::Speed)), ColorMode::Speed);
        assert_eq!(ColorMode::from(ApiColorMode::from(ColorMode::PathMap)), ColorMode::PathMap);
    }

    #[test]
    fn test_tonemap_mode_roundtrip() {
        assert_eq!(ToneMapMode::from(ApiToneMapMode::from(ToneMapMode::Linear)), ToneMapMode::Linear);
        assert_eq!(ToneMapMode::from(ApiToneMapMode::from(ToneMapMode::Logarithmic)), ToneMapMode::Logarithmic);
        assert_eq!(ToneMapMode::from(ApiToneMapMode::from(ToneMapMode::DensityVisualization)), ToneMapMode::DensityVisualization);
    }

    #[test]
    fn test_api_enum_serialization() {
        // Verify API enums serialize to the exact strings the API expects
        assert_eq!(serde_json::to_string(&ApiRenderMode::TwoD).unwrap(), "\"2d\"");
        assert_eq!(serde_json::to_string(&ApiRenderMode::ThreeD).unwrap(), "\"3d\"");
        assert_eq!(serde_json::to_string(&ApiColorMode::Palette).unwrap(), "\"palette\"");
        assert_eq!(serde_json::to_string(&ApiColorMode::PathMap).unwrap(), "\"path_map\"");
        assert_eq!(serde_json::to_string(&ApiToneMapMode::Logarithmic).unwrap(), "\"logarithmic\"");
        assert_eq!(serde_json::to_string(&ApiToneMapMode::Density).unwrap(), "\"density\"");
        assert_eq!(serde_json::to_string(&ApiPathMapStyle::PrefixDistinct).unwrap(), "\"prefix_distinct\"");
        assert_eq!(serde_json::to_string(&ApiPathMapStyle::OriginRadial).unwrap(), "\"origin_radial\"");
    }

    #[test]
    fn test_transform_roundtrip() {
        let t = Transform::new();
        let api = transform_to_api(&t);

        // Simulate API response (all fields required)
        let resp = TransformResponse {
            id: "test-id".to_string(),
            sort_order: 0,
            is_final_transform: false,
            a: api.a.unwrap(),
            b: api.b.unwrap(),
            c: api.c.unwrap(),
            d: api.d.unwrap(),
            e: api.e.unwrap(),
            f: api.f.unwrap(),
            g: api.g.unwrap(),
            weight: api.weight.unwrap(),
            color: api.color.unwrap(),
            color_speed: api.color_speed.unwrap(),
            opacity: api.opacity.unwrap(),
            direct_color: api.direct_color.unwrap_or(0.0),
            variations: api.variations.unwrap_or_default(),
            variation_params: api.variation_params.unwrap_or_default(),
            post_affine_enabled: api.post_affine_enabled.unwrap(),
            post_a: api.post_a.unwrap(),
            post_b: api.post_b.unwrap(),
            post_c: api.post_c.unwrap(),
            post_d: api.post_d.unwrap(),
            post_e: api.post_e.unwrap(),
            post_f: api.post_f.unwrap(),
            post_g: api.post_g.unwrap(),
        };

        let restored = transform_from_api(&resp);
        assert_eq!(t.a, restored.a);
        assert_eq!(t.weight, restored.weight);
        assert_eq!(t.opacity, restored.opacity);
        assert_eq!(t.direct_color, restored.direct_color);
    }

    #[test]
    fn test_config_to_request_basic() {
        use crate::scene::transforms::Transform;

        let mut config = FractalConfig::default();
        // Default Flame has empty transforms, add one for testing
        config.flame.transforms.push(Transform::new());

        let req = config_to_create_request(&config, Some("Test Flame"));

        assert_eq!(req.name, "Test Flame");
        assert_eq!(req.render_mode, Some(ApiRenderMode::TwoD));
        assert_eq!(req.color_mode, Some(ApiColorMode::Palette));
        assert_eq!(req.tonemap_mode, Some(ApiToneMapMode::Logarithmic));
        assert!(!req.transforms.is_empty());
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
        base_config: animation.base_config.as_ref().and_then(|c| serde_json::to_value(c).ok()),
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

    let base_config = resp.base_config.as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    crate::animation::Animation {
        name: resp.name.clone(),
        base_config,
        duration: resp.duration,
        tracks,
        generators,
        loop_mode: resp.loop_mode.into(),
    }
}
