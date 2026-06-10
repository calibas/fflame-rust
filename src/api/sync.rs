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

impl From<crate::scene::tonemap::HighlightMode> for ApiHighlightMode {
    fn from(mode: crate::scene::tonemap::HighlightMode) -> Self {
        use crate::scene::tonemap::HighlightMode;
        match mode {
            HighlightMode::Clip => ApiHighlightMode::Clip,
            HighlightMode::MaxNorm => ApiHighlightMode::MaxNorm,
            HighlightMode::Reinhard => ApiHighlightMode::Reinhard,
            HighlightMode::Filmic => ApiHighlightMode::Filmic,
        }
    }
}

impl From<ApiHighlightMode> for crate::scene::tonemap::HighlightMode {
    fn from(mode: ApiHighlightMode) -> Self {
        use crate::scene::tonemap::HighlightMode;
        match mode {
            ApiHighlightMode::Clip => HighlightMode::Clip,
            ApiHighlightMode::MaxNorm => HighlightMode::MaxNorm,
            ApiHighlightMode::Reinhard => HighlightMode::Reinhard,
            ApiHighlightMode::Filmic => HighlightMode::Filmic,
        }
    }
}

impl From<crate::scene::palette::SqueezeMode> for ApiSqueezeMode {
    fn from(mode: crate::scene::palette::SqueezeMode) -> Self {
        use crate::scene::palette::SqueezeMode;
        match mode {
            SqueezeMode::Linear => ApiSqueezeMode::Linear,
            SqueezeMode::Geometric => ApiSqueezeMode::Geometric,
        }
    }
}

impl From<ApiSqueezeMode> for crate::scene::palette::SqueezeMode {
    fn from(mode: ApiSqueezeMode) -> Self {
        use crate::scene::palette::SqueezeMode;
        match mode {
            ApiSqueezeMode::Linear => SqueezeMode::Linear,
            ApiSqueezeMode::Geometric => SqueezeMode::Geometric,
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
        linked_attachments: if t.linked_attachments.is_empty() {
            None
        } else {
            Some(t.linked_attachments.clone())
        },
        final_attachments: if t.final_attachments.is_empty() {
            None
        } else {
            Some(t.final_attachments.clone())
        },
    }
}

fn transform_from_api(resp: &TransformResponse) -> Transform {
    Transform {
        id: crate::scene::transforms::next_id(),
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
        // JWildfire-extension plane affines. The API contract doesn't
        // carry these yet — default to identity so API-loaded flames
        // stay Apophysis-semantics. When the API contract grows fields
        // for them we'll wire them through here.
        yz_coefs: crate::scene::transforms::IDENTITY_PLANE_COEFS,
        zx_coefs: crate::scene::transforms::IDENTITY_PLANE_COEFS,
        yz_post_coefs: crate::scene::transforms::IDENTITY_PLANE_COEFS,
        zx_post_coefs: crate::scene::transforms::IDENTITY_PLANE_COEFS,
        linked_attachments: resp.linked_attachments.clone(),
        final_attachments: resp.final_attachments.clone(),
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
        id: crate::scene::transforms::next_id(),
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

/// Convert a FractalConfig to an API CreateFlameRequest. The palette
/// travels inline in `palette`; the server deduplicates by content hash.
///
/// Convert a nested `Flame` (subflame) into the `SubflameRequest` wire
/// shape. Recursive — subflames can contain subflames up to depth 4
/// (server-enforced). Only the `Flame` state crosses the wire; tonemap
/// and palette belong to the parent `FractalConfig` and are inherited at
/// render time.
fn flame_to_subflame_request(flame: &Flame) -> SubflameRequest {
    SubflameRequest {
        flame_name: Some(flame.name.clone()),
        render_mode: flame.render_mode.into(),
        perspective_strength: flame.perspective_strength,
        solo_transform: flame.solo_transform.map(|i| i as i32),
        xaos: flame.xaos.as_ref().map(|x| {
            serde_json::to_value(x).unwrap_or(serde_json::Value::Null)
        }),
        transforms: flame.transforms.iter().map(transform_to_api).collect(),
        linked_transforms: flame
            .linked_transforms
            .iter()
            .map(transform_to_api)
            .collect(),
        final_transforms: flame
            .final_transforms
            .iter()
            .map(transform_to_api)
            .collect(),
        subflames: flame
            .subflames
            .iter()
            .map(flame_to_subflame_request)
            .collect(),
    }
}

pub fn config_to_create_request(config: &FractalConfig, name: Option<&str>) -> CreateFlameRequest {
    let flame = &config.flame;

    CreateFlameRequest {
        name: name.map(|n| n.to_string()).unwrap_or_else(|| flame.name.clone()),
        flame_name: Some(flame.name.clone()),
        transforms: flame.transforms.iter().map(transform_to_api).collect(),
        linked_transforms: flame
            .linked_transforms
            .iter()
            .map(transform_to_api)
            .collect(),
        final_transforms: flame
            .final_transforms
            .iter()
            .map(transform_to_api)
            .collect(),
        subflames: flame
            .subflames
            .iter()
            .map(flame_to_subflame_request)
            .collect(),
        visibility: None, // Set by caller if needed
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
        filter_radius: Some(config.filter_radius),
        filter_blur_edges: Some(config.filter_blur_edges),

        density_scale: Some(config.density_scale),
        speed_factor: Some(config.speed_factor),
        max_iterations: Some(config.max_iterations),
        blend_factor: Some(config.blend_factor),
        use_dynamic_blend: Some(config.use_dynamic_blend),

        color_mode: Some(config.color_mode.into()),
        path_map_style: Some(config.path_map_style.into()),
        path_capture_mode: Some(config.path_capture_mode.into()),
        path_tracking_mode: Some(config.path_tracking_mode.into()),
        palette: Some(palette_to_api(&config.palette)),
        palette_rotation: Some(config.palette_rotation),
        palette_size: Some(config.palette_size as i32),
        palette_squeeze: Some(config.palette_squeeze),
        palette_squeeze_mode: Some(config.palette_squeeze_mode.into()),
        palette_squeeze_falloff: Some(config.palette_squeeze_falloff),
        palette_log_strength: Some(config.palette_log_strength),
        palette_reverse: Some(config.palette_reverse),
        background_color: Some(config.background_color.to_vec()),

        tonemap_mode: Some(config.tonemap_mode.into()),
        highlight_mode: Some(config.highlight_mode.into()),
        white_level: Some(config.white_level),
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
        levels_enabled: Some(config.levels_enabled),
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

/// Convert an API FlameResponse to a FractalConfig. The palette is
/// reconstructed from the inline `resp.palette` payload; when the flame
/// has no palette, the default palette is used.
/// Recursively assemble a nested `Flame` from a subflame `FlameResponse`.
/// Subflames don't carry their own tonemap/palette/visibility — those are
/// inherited from the parent's `FractalConfig` at render time. Server
/// stores defaults on subflame rows but we ignore those fields here.
fn flame_from_subflame_response(resp: &FlameResponse) -> Flame {
    let xaos: Option<Vec<Vec<f32>>> = resp
        .xaos
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let mut transforms: Vec<&TransformResponse> = resp.transforms.iter().collect();
    transforms.sort_by_key(|t| t.sort_order);
    let mut linked: Vec<&TransformResponse> = resp.linked_transforms.iter().collect();
    linked.sort_by_key(|t| t.sort_order);
    let mut finals: Vec<&TransformResponse> = resp.final_transforms.iter().collect();
    finals.sort_by_key(|t| t.sort_order);

    Flame {
        id: crate::scene::transforms::next_id(),
        name: resp.flame_name.clone().unwrap_or_else(|| resp.name.clone()),
        transforms: transforms.iter().map(|t| transform_from_api(t)).collect(),
        linked_transforms: linked.iter().map(|t| transform_from_api(t)).collect(),
        final_transforms: finals.iter().map(|t| transform_from_api(t)).collect(),
        render_mode: resp.render_mode.into(),
        perspective_strength: resp.perspective_strength,
        depth_density_compensation: 0.0,
        xaos,
        solo_transform: resp.solo_transform.map(|i| i as usize),
        subflames: resp.subflames.iter().map(flame_from_subflame_response).collect(),
        // API doesn't carry post_symmetry yet; default until the server
        // schema gains the field.
        post_symmetry: crate::scene::transforms::PostSymmetry::default(),
        preserve_z: false,
    }
}

pub fn flame_response_to_config(resp: &FlameResponse) -> FractalConfig {
    // Bucket the three pools by `transform_kind` and sort by sort_order
    // within each. Server-side guarantees `transforms` only holds
    // `normal` rows (linked / final pools come in their own arrays), but
    // we filter defensively in case an older server returns a flat list.
    let mut transforms: Vec<&TransformResponse> = resp
        .transforms
        .iter()
        .filter(|t| matches!(t.transform_kind, ApiTransformKind::Normal))
        .collect();
    transforms.sort_by_key(|t| t.sort_order);
    let mut linked: Vec<&TransformResponse> = resp.linked_transforms.iter().collect();
    linked.sort_by_key(|t| t.sort_order);
    let mut finals: Vec<&TransformResponse> = resp.final_transforms.iter().collect();
    finals.sort_by_key(|t| t.sort_order);

    // Reconstruct xaos from JSON value
    let xaos: Option<Vec<Vec<f32>>> = resp
        .xaos
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let flame = Flame {
        id: crate::scene::transforms::next_id(),
        name: resp.flame_name.clone().unwrap_or_else(|| resp.name.clone()),
        transforms: transforms.iter().map(|t| transform_from_api(t)).collect(),
        linked_transforms: linked.iter().map(|t| transform_from_api(t)).collect(),
        final_transforms: finals.iter().map(|t| transform_from_api(t)).collect(),
        render_mode: resp.render_mode.into(),
        perspective_strength: resp.perspective_strength,
        depth_density_compensation: 0.0,
        xaos,
        solo_transform: resp.solo_transform.map(|i| i as usize),
        subflames: resp.subflames.iter().map(flame_from_subflame_response).collect(),
        // API doesn't carry post_symmetry yet; default until the server
        // schema gains the field.
        post_symmetry: crate::scene::transforms::PostSymmetry::default(),
        preserve_z: false,
    };

    // Reconstruct palette from the inline payload on the flame response.
    let palette = resp
        .palette
        .as_ref()
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
        // API contract doesn't carry camera_bank yet — defaults to 0
        // (no bank applied). Wire through when the API grows a field.
        camera_bank: 0.0,
        // API contract doesn't carry camera_x/y yet — default to 0
        // (camera at origin). Wire through when the API grows fields.
        camera_x: 0.0,
        camera_y: 0.0,
        camera_z: resp.camera_z,
        // API contract doesn't carry image_size yet; default to the
        // historical 1920×1080 so API-loaded flames behave the same
        // as a fresh `FractalConfig::default()`. Wire through when
        // the API grows a field for it.
        image_size: (1920, 1080),
        dof_focus_distance: resp.dof_focus_distance,
        dof_blur_strength: resp.dof_blur_strength,
        fog_strength: resp.fog_strength,
        fog_start: resp.fog_start,
        filter_radius: resp.filter_radius.unwrap_or(0.0),
        filter_blur_edges: resp.filter_blur_edges.unwrap_or(0.0),
        density_scale: resp.density_scale,
        speed_factor: resp.speed_factor,
        max_iterations: resp.max_iterations,
        blend_factor: resp.blend_factor,
        use_dynamic_blend: resp.use_dynamic_blend,
        color_mode: resp.color_mode.into(),
        path_map_style: resp.path_map_style.into(),
        path_capture_mode: resp.path_capture_mode.into(),
        path_tracking_mode: resp.path_tracking_mode.into(),
        palette,
        palette_rotation: resp.palette_rotation,
        palette_size: resp.palette_size as u32,
        palette_squeeze: resp.palette_squeeze,
        palette_squeeze_mode: resp
            .palette_squeeze_mode
            .map(Into::into)
            .unwrap_or(crate::scene::palette::SqueezeMode::Linear),
        palette_squeeze_falloff: resp.palette_squeeze_falloff.unwrap_or(0.5),
        palette_log_strength: resp.palette_log_strength.unwrap_or(0.0),
        palette_reverse: resp.palette_reverse.unwrap_or(false),
        background_color,
        tonemap_mode: resp.tonemap_mode.into(),
        tonemap_curve: tonecurve_from_json(resp.tonemap_curve.as_ref()),
        use_curve: resp.use_curve,
        exposure: resp.exposure,
        gamma: resp.gamma,
        gamma_threshold: resp.gamma_threshold,
        brightness: resp.brightness,
        vibrancy: resp.vibrancy,
        white_level: resp
            .white_level
            .unwrap_or(crate::config::defaults::DEFAULT_WHITE_LEVEL),
        highlight_mode: resp
            .highlight_mode
            .map(Into::into)
            .unwrap_or_default(),
        saturation: resp.saturation,
        hue_shift: resp.hue_shift,
        alpha_blend_low: resp.alpha_blend_low,
        alpha_blend_high: resp.alpha_blend_high,
        levels_enabled: resp.levels_enabled,
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
            transform_kind: ApiTransformKind::Normal,
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
            // Fallback stays at 0.0 even though `Transform::default()` is
            // now 1.0 — API responses for flames saved before the flip
            // omit this field (line 207 serializes only when non-zero),
            // and we want those server-stored flames to keep their look.
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
            linked_attachments: api.linked_attachments.clone().unwrap_or_default(),
            final_attachments: api.final_attachments.clone().unwrap_or_default(),
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
