//! Apophysis .flame XML format parser and writer
//!
//! Handles import/export of Apophysis 7X .flame XML files for compatibility
//! with the Apophysis fractal flame editor.
//!
//! XML Structure:
//! ```xml
//! <flames name="collection_name">
//!   <flame name="..." version="..." size="W H" center="X Y" scale="..."
//!          background="R G B" brightness="..." gamma="..." ...>
//!     <xform weight="..." color="..." coefs="a b c d e f" opacity="..."
//!            linear="..." sinusoidal="..." spherical="..." ... />
//!     <palette count="256" format="RGB">HEXDATA</palette>
//!   </flame>
//! </flames>
//! ```

use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::config::FractalConfig;
use crate::scene::palette::{Palette, ColorMode, PathMapStyle, PathCaptureMode, PathTrackingMode};
use crate::scene::tonemap::{ToneMapMode, ToneCurve};
use crate::scene::transforms::{Flame, RenderMode, Transform};
use crate::variations::global_registry;

/// Parse Apophysis .flame XML file
/// Returns a vector of FractalConfig (one per <flame> element)
pub fn parse_flame_xml(xml_content: &str) -> Result<Vec<FractalConfig>> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut flames = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"flame" {
                    let config = parse_flame_element(&mut reader, &e)?;
                    flames.push(config);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error at position {}: {}", reader.buffer_position(), e)),
            _ => {}
        }
        buf.clear();
    }

    if flames.is_empty() {
        return Err(anyhow::anyhow!("No <flame> elements found in XML"));
    }

    Ok(flames)
}

/// Parse a single <flame> element
fn parse_flame_element(
    reader: &mut Reader<&[u8]>,
    start_element: &quick_xml::events::BytesStart,
) -> Result<FractalConfig> {
    // Parse flame attributes
    let mut name = String::from("Untitled");
    let mut size = (1920, 1080);
    let mut center = (0.0, 0.0);
    let mut scale = 100.0;
    let mut rotate = 0.0;  // View rotation in degrees
    let mut background = [0.0, 0.0, 0.0];
    let mut brightness = 1.0;
    let mut gamma = 2.2;
    let mut vibrancy = 1.0;
    let mut gamma_threshold = 0.0025;
    let mut filter_radius = 0.0_f32;  // Apo's `filter` attribute
    let mut cam_pitch = 0.0;  // Camera rotation X (radians)
    let mut cam_yaw = 0.0;    // Camera rotation Y (radians)
    let mut cam_zpos = 0.0;   // Camera Z position (height)
    let mut cam_perspective = 0.0;  // Perspective strength
    let mut cam_dof = 0.0;    // Depth-of-field blur strength (Apo's `cam_dof`)
    let mut curves: Option<Vec<f32>> = None;  // Tone curve data (48 floats)
    let mut solo_xform: Option<usize> = None;  // Solo transform index (0-indexed)

    for attr in start_element.attributes() {
        let attr = attr?;
        let key = std::str::from_utf8(attr.key.as_ref())?;
        let value = std::str::from_utf8(&attr.value)?;

        match key {
            "name" => name = value.to_string(),
            "size" => {
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() == 2 {
                    size.0 = parts[0].parse().unwrap_or(1920);
                    size.1 = parts[1].parse().unwrap_or(1080);
                }
            }
            "center" => {
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() == 2 {
                    center.0 = parts[0].parse().unwrap_or(0.0);
                    center.1 = parts[1].parse().unwrap_or(0.0);
                }
            }
            "scale" => scale = value.parse().unwrap_or(100.0),
            "rotate" => rotate = value.parse().unwrap_or(0.0),
            "background" => {
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() == 3 {
                    background[0] = parts[0].parse::<f32>().unwrap_or(0.0) / 255.0;
                    background[1] = parts[1].parse::<f32>().unwrap_or(0.0) / 255.0;
                    background[2] = parts[2].parse::<f32>().unwrap_or(0.0) / 255.0;
                }
            }
            "brightness" => brightness = value.parse().unwrap_or(4.0),
            "gamma" => {
                gamma = value.parse().unwrap_or(4.0);
            }
            "vibrancy" => vibrancy = value.parse().unwrap_or(1.0),
            "gamma_threshold" => gamma_threshold = value.parse().unwrap_or(0.0025),
            "filter" => filter_radius = value.parse().unwrap_or(0.0),
            "cam_pitch" => cam_pitch = value.parse().unwrap_or(0.0),
            "cam_yaw" => cam_yaw = value.parse().unwrap_or(0.0),
            "cam_zpos" => cam_zpos = value.parse().unwrap_or(0.0),
            "cam_perspective" => cam_perspective = value.parse().unwrap_or(0.0),
            "cam_dof" => cam_dof = value.parse().unwrap_or(0.0),
            "curves" => {
                // Parse space-separated floats (48 values: 4 curves × 12 points)
                let parsed: Vec<f32> = value.split_whitespace()
                    .filter_map(|s| s.parse::<f32>().ok())
                    .collect();
                if parsed.len() == 48 {
                    curves = Some(parsed);
                }
            }
            "soloxform" => {
                // Solo transform index (0-indexed in Apophysis)
                solo_xform = value.parse::<usize>().ok();
            }
            _ => {} // Ignore unknown attributes for now
        }
    }

    // Parse child elements (xform, finalxform, and palette)
    let mut xform_results = Vec::new();
    let mut final_transform_with_index: Option<(Transform, Option<usize>)> = None;
    let mut palette = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"xform" => {
                        let result = parse_xform_element(&e)?;
                        xform_results.push(result);
                    }
                    b"finalxform" => {
                        let (transform, color_index) = parse_finalxform_element(&e)?;
                        final_transform_with_index = Some((transform, color_index));
                    }
                    b"palette" => {
                        palette = Some(parse_palette_element(reader, &e)?);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"flame" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    // Always use Palette mode (ColorMode::Transform has been removed)
    let color_mode = ColorMode::Palette;

    // Extract transforms, color indices, and chaos weights from parse results
    let mut transforms = Vec::new();
    let mut all_chaos_weights: Vec<Option<Vec<f32>>> = Vec::new();

    for result in xform_results {
        let mut transform = result.transform;
        if let Some(idx) = result.color_index {
            if palette.is_some() {
                // Palette mode: Store color coordinate (0-1) as averaged RGB
                // This will be used as palette position in shader
                let color_coord = idx as f32 / 255.0;
                transform.color = color_coord;
            }
            // Note: color_speed comes from XML parsing, don't override here
        }
        transforms.push(transform);
        all_chaos_weights.push(result.chaos_weights);
    }

    // Build xaos matrix if any transform has chaos weights
    let xaos = if all_chaos_weights.iter().any(|w| w.is_some()) {
        let n = transforms.len();
        let mut matrix = vec![vec![1.0; n]; n];

        for (src, weights_opt) in all_chaos_weights.iter().enumerate() {
            if let Some(weights) = weights_opt {
                for (dst, &weight) in weights.iter().enumerate() {
                    if dst < n {
                        matrix[src][dst] = weight;
                    }
                }
            }
            // If no chaos weights for this transform, row stays at 1.0 (identity)
        }

        Some(matrix)
    } else {
        None
    };

    // Process final transform if present
    let final_transform = if let Some((mut final_xform, color_index)) = final_transform_with_index {
        if let Some(idx) = color_index {
            if palette.is_some() {
                // Palette mode: Store color coordinate (0-1)
                let color_coord = idx as f32 / 255.0;
                final_xform.color = color_coord;
            }
        }
        Some(final_xform)
    } else {
        None
    };

    // Apophysis 7X and JWildfire write 3D flames exclusively (every
    // variation in their corpus is treated as 3D-capable; the camera
    // params are optional with zero defaults for "flat" views). We
    // default to 3D mode on import so 3D-only variations (zcone,
    // hemisphere, pre_rotate_x, etc.) work correctly. The render mode
    // can still be toggled in the UI after import.
    //
    // Chaotica's situation is less clear — its corpus has 3D variations
    // but no obvious 3D camera controls. Treating it as 3D-on-import
    // for now and revisiting if a Chaotica-source-detection path lands.
    //
    // `has_camera_params` is kept here for the previous heuristic's
    // information value (logging) and any future per-source overrides.
    let _has_camera_params = f32::abs(cam_pitch) > 0.0001
        || f32::abs(cam_yaw) > 0.0001
        || f32::abs(cam_zpos) > 0.0001
        || f32::abs(cam_perspective) > 0.0001;

    let render_mode = RenderMode::ThreeD;

    // Determine perspective strength from cam_perspective
    let perspective_strength = f32::abs(cam_perspective);

    // Apophysis XML always carries a singular global Final (or none).
    // `Flame::migrate_legacy_final` pushes it into the new
    // `final_transforms` pool and auto-attaches to every normal so the
    // rest of the pipeline sees a consistent shape.
    // See `docs/projects/per-transform-linked-and-final.md`.
    let mut flame = Flame {
        id: crate::scene::transforms::next_id(),
        name,
        transforms,
        linked_transforms: Vec::new(),
        final_transforms: Vec::new(),
        render_mode,
        perspective_strength,
        xaos,
        solo_transform: solo_xform,
        // Subflames populated by the subflame_wf importer in Phase 5;
        // empty Vec here is the post-import default for flames that
        // don't use subflame_wf.
        subflames: Vec::new(),
    };
    flame.migrate_legacy_final(final_transform);

    // Convert Apophysis scale/center to our zoom/pan
    // Apophysis: scale = pixels per unit, where scale 200 ≈ zoom 1.0
    // Our system: zoom and pan (pan is world offset)
    let zoom = scale / 200.0; // Apophysis scale 200.0 = our zoom 1.0
    let pan_x = center.0;
    let pan_y = center.1;

    // Convert rotation from degrees to radians
    let rotation = rotate * std::f32::consts::PI / 180.0;

    // Convert gamma_threshold from Apophysis scale to UI scale
    // Apophysis default: 0.0025 (very small values)
    // UI scale: 0-1000 (larger range for better slider precision)
    // Formula: ui_value = apophysis_value * 2000.0 + 50.0
    // This ensures default 0.0025 becomes 55.0 in UI
    let gamma_threshold = gamma_threshold * 2000.0 + 50.0;

    // Parse tone curve from Apophysis curves data
    // Apophysis: 48 floats = 4 curves (X, R, G, B) × 12 points each
    // Indices: 0-11=X, 12-23=R, 24-35=G, 36-47=B
    // We use the average of R, G, B curves
    let tonemap_curve = if let Some(curve_data) = curves {
        parse_apophysis_curves(&curve_data)
    } else {
        ToneCurve::linear()
    };

    Ok(FractalConfig {
        flame,
        zoom,
        pan_x,
        pan_y,
        rotation,
        camera_rotation_x: cam_pitch,
        camera_rotation_y: cam_yaw,
        camera_z: cam_zpos,
        dof_focus_distance: crate::config::defaults::DEFAULT_DOF_FOCUS_DISTANCE,
        // Direct copy of Apo's `cam_dof` after the step-3 strength rescale —
        // shader divides by 10 internally, so the stored value carries the
        // same magnitude as Apo's attribute.
        dof_blur_strength: cam_dof,
        fog_strength: crate::config::defaults::DEFAULT_FOG_STRENGTH,
        fog_start: crate::config::defaults::DEFAULT_FOG_START,
        // Apo's `filter` is the sample-time Gaussian sigma in pixels; we
        // apply it on the per-batch histogram before accumulation.
        filter_radius,
        // Edge preservation defaults to 0 (strict) for fresh imports;
        // user can dial up if they want uniform Gaussian behavior.
        filter_blur_edges: 0.0,
        density_scale: 1.0,  // Use default, brightness is handled by Apophysis brightness parameter
        speed_factor: 1.0,
        max_iterations: 1_000_000_000,
        color_mode,  // Detected based on palette presence
        path_map_style: PathMapStyle::default(),
        path_capture_mode: PathCaptureMode::default(),
        path_tracking_mode: PathTrackingMode::default(),
        // Use parsed palette, or default if not present in XML
        palette: palette.unwrap_or_default(),
        palette_rotation: 0.0,  // Default, could parse from XML if present
        background_color: background,
        tonemap_mode: ToneMapMode::Logarithmic,
        tonemap_curve,
        use_curve: true,
        exposure: 1.0,
        gamma,
        brightness,  // Use parsed Apophysis brightness value
        vibrancy,  // Use parsed Apophysis vibrancy
        // TODO: parse Apophysis `white_level` XML attribute (default 200)
        white_level: crate::config::defaults::DEFAULT_WHITE_LEVEL,
        highlight_mode: crate::scene::tonemap::HighlightMode::Clip,  // Apophysis-compatible
        // saturation 1.0 — Apo has no saturation control. Previously 1.5
        // to compensate for sRGB-as-linear palette washout; that became
        // wrong once the palette-decode fix landed (oversaturates past
        // Apo's reference).
        saturation: crate::config::defaults::DEFAULT_SATURATION,
        hue_shift: 0.0,  // Default hue shift
        gamma_threshold,  // Use parsed Apophysis gamma_threshold
        deterministic_rng: false,
        blend_factor: 0.1,
        use_dynamic_blend: true,
        alpha_blend_low: crate::config::defaults::DEFAULT_ALPHA_BLEND_LOW,
        alpha_blend_high: crate::config::defaults::DEFAULT_ALPHA_BLEND_HIGH,
        palette_size: crate::config::defaults::DEFAULT_PALETTE_SIZE,
        palette_squeeze: crate::config::defaults::DEFAULT_PALETTE_SQUEEZE,
        palette_squeeze_mode: crate::scene::palette::SqueezeMode::Linear,
        palette_squeeze_falloff: 0.5,
        palette_log_strength: 0.0,
        palette_reverse: false,
        // Apo has no Levels system, but enabling our Levels with the
        // tuned defaults (gamma 0.5) empirically lands closer to Apo's
        // brightness response on the flames tested. Treating Levels as
        // a calibration layer rather than a strict Apo-feature mapping.
        levels_enabled: crate::config::defaults::DEFAULT_LEVELS_ENABLED,
        levels_low: 0.0,
        levels_high: crate::config::defaults::DEFAULT_LEVELS_HIGH,
        levels_gamma: crate::config::defaults::DEFAULT_LEVELS_GAMMA,
        // Effects - empty by default (zero cost)
        density_effects: Vec::new(),
        color_effects: Vec::new(),
    })
}

/// Parse a single <xform> element (transform)
/// Returns (Transform, color_index) where color_index is the palette position
/// Result of parsing an xform element
/// Contains the transform, optional color index, and optional chaos weights
struct XformParseResult {
    transform: Transform,
    color_index: Option<usize>,
    chaos_weights: Option<Vec<f32>>,
}

fn parse_xform_element(
    element: &quick_xml::events::BytesStart,
) -> Result<XformParseResult> {
    let mut transform = Transform::new();
    let registry = global_registry();
    let mut color_index = None;
    let mut chaos_weights = None;

    // Storage for variation parameters (applied after all attributes parsed)
    let mut pending_params: Vec<(String, String, f32)> = Vec::new(); // (var_name, param_name, value)

    for attr in element.attributes() {
        let attr = attr?;
        let key = std::str::from_utf8(attr.key.as_ref())?;
        let value = std::str::from_utf8(&attr.value)?;

        match key {
            "weight" => transform.weight = value.parse().unwrap_or(1.0),
            "color" => {
                // In Apophysis, color is a palette index (0.0-1.0 mapped to 0-255)
                if let Ok(color_value) = value.parse::<f32>() {
                    color_index = Some((color_value * 255.0) as usize);
                }
            }
            "color_speed" | "symmetry" => {
                // Apophysis calls this "symmetry" in XML, we call it color_speed
                // Range: -1.0 to 1.0 (Apophysis symmetry parameter)
                transform.color_speed = value.parse().unwrap_or(0.0);
            }
            "coefs" => {
                // Parse "a c b d e f" format (Apophysis order!)
                // Apophysis stores matrix column-major, XML writes: c[0,0] c[0,1] c[1,0] c[1,1] c[2,0] c[2,1]
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() >= 6 {
                    transform.a = parts[0].parse().unwrap_or(1.0);  // c[0,0]
                    transform.c = parts[1].parse().unwrap_or(0.0);  // c[0,1]
                    transform.b = parts[2].parse().unwrap_or(0.0);  // c[1,0]
                    transform.d = parts[3].parse().unwrap_or(1.0);  // c[1,1]
                    transform.e = parts[4].parse().unwrap_or(0.0);  // c[2,0]
                    transform.f = parts[5].parse().unwrap_or(0.0);  // c[2,1]
                }
            }
            "opacity" => {
                // Parse opacity (0.0 to 1.0, default 1.0)
                transform.opacity = value.parse().unwrap_or(1.0);
            }
            "pluginColor" | "plugin_color" => {
                // Apophysis direct-color blend strength (0.0 = standard, 1.0 = full DC)
                transform.direct_color = value.parse().unwrap_or(0.0);
            }
            "post" => {
                // Parse post-affine: "pa pc pb pd pe pf" (same column-major order as coefs)
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() >= 6 {
                    transform.post_affine_enabled = true;
                    transform.post_a = parts[0].parse().unwrap_or(1.0);  // c[0,0]
                    transform.post_c = parts[1].parse().unwrap_or(0.0);  // c[0,1]
                    transform.post_b = parts[2].parse().unwrap_or(0.0);  // c[1,0]
                    transform.post_d = parts[3].parse().unwrap_or(1.0);  // c[1,1]
                    transform.post_e = parts[4].parse().unwrap_or(0.0);  // c[2,0]
                    transform.post_f = parts[5].parse().unwrap_or(0.0);  // c[2,1]
                }
            }
            "chaos" => {
                // Parse chaos/xaos weights (space-separated floats)
                // chaos="1.0 0.5 0.75" means P(this→0)=1.0, P(this→1)=0.5, P(this→2)=0.75
                let weights: Vec<f32> = value.split_whitespace()
                    .filter_map(|s| s.parse::<f32>().ok())
                    .collect();
                if !weights.is_empty() {
                    chaos_weights = Some(weights);
                }
            }
            _ => {
                // Try to parse as variation or variation parameter
                if let Ok(parsed_value) = value.parse::<f32>() {
                    // First, try direct variation lookup (e.g., "julian", "spherical")
                    if registry.get(key).is_some() {
                        if parsed_value != 0.0 {
                            transform.variations.insert(key.to_string(), parsed_value);
                        }
                    } else if key.contains('_') {
                        // Try to parse as variation parameter (e.g., "julian_power", "blob_high")
                        // Try progressively longer prefixes to find matching variation
                        // This handles cases like "pre_blur_param" correctly
                        if let Some((var_name, param_name)) = find_variation_and_param(key, &registry) {
                            pending_params.push((var_name, param_name, parsed_value));
                        }
                    }
                }
            }
        }
    }

    // Apply collected parameters after all variations are known
    for (var_name, param_name, value) in pending_params {
        transform.set_variation_param(&var_name, &param_name, value);
    }

    Ok(XformParseResult {
        transform,
        color_index,
        chaos_weights,
    })
}

/// Parse a <finalxform> element (same as xform but without weight/opacity)
fn parse_finalxform_element(
    element: &quick_xml::events::BytesStart,
) -> Result<(Transform, Option<usize>)> {
    let mut transform = Transform::new();
    let registry = global_registry();
    let mut color_index = None;

    // Storage for variation parameters (applied after all attributes parsed)
    let mut pending_params: Vec<(String, String, f32)> = Vec::new();

    for attr in element.attributes() {
        let attr = attr?;
        let key = std::str::from_utf8(attr.key.as_ref())?;
        let value = std::str::from_utf8(&attr.value)?;

        match key {
            // NO "weight" attribute - final transform is not part of random selection
            // NO "opacity" attribute - final transform is always applied
            "color" => {
                // In Apophysis, color is a palette index (0.0-1.0 mapped to 0-255)
                if let Ok(color_value) = value.parse::<f32>() {
                    color_index = Some((color_value * 255.0) as usize);
                }
            }
            "color_speed" | "symmetry" => {
                // Apophysis calls this "symmetry" in XML
                transform.color_speed = value.parse().unwrap_or(0.0);
            }
            "pluginColor" | "plugin_color" => {
                // Apophysis direct-color blend strength
                transform.direct_color = value.parse().unwrap_or(0.0);
            }
            "coefs" => {
                // Parse "a c b d e f" format (Apophysis order)
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() >= 6 {
                    transform.a = parts[0].parse().unwrap_or(1.0);
                    transform.c = parts[1].parse().unwrap_or(0.0);
                    transform.b = parts[2].parse().unwrap_or(0.0);
                    transform.d = parts[3].parse().unwrap_or(1.0);
                    transform.e = parts[4].parse().unwrap_or(0.0);
                    transform.f = parts[5].parse().unwrap_or(0.0);
                }
            }
            "post" => {
                // Parse post-affine: "pa pc pb pd pe pf" (same column-major order as coefs)
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() >= 6 {
                    transform.post_affine_enabled = true;
                    transform.post_a = parts[0].parse().unwrap_or(1.0);
                    transform.post_c = parts[1].parse().unwrap_or(0.0);
                    transform.post_b = parts[2].parse().unwrap_or(0.0);
                    transform.post_d = parts[3].parse().unwrap_or(1.0);
                    transform.post_e = parts[4].parse().unwrap_or(0.0);
                    transform.post_f = parts[5].parse().unwrap_or(0.0);
                }
            }
            _ => {
                // Try to parse as variation or variation parameter
                if let Ok(parsed_value) = value.parse::<f32>() {
                    // First, try direct variation lookup
                    if registry.get(key).is_some() {
                        if parsed_value != 0.0 {
                            transform.variations.insert(key.to_string(), parsed_value);
                        }
                    } else if key.contains('_') {
                        // Try to parse as variation parameter
                        if let Some((var_name, param_name)) = find_variation_and_param(key, &registry) {
                            pending_params.push((var_name, param_name, parsed_value));
                        }
                    }
                }
            }
        }
    }

    // Apply collected parameters after all variations are known
    for (var_name, param_name, value) in pending_params {
        transform.set_variation_param(&var_name, &param_name, value);
    }

    Ok((transform, color_index))
}

/// Try to split an attribute key into variation name and parameter name
/// Handles cases like "julian_power" → ("julian", "power")
/// and "pre_blur_strength" → ("pre_blur", "strength")
fn find_variation_and_param(key: &str, registry: &crate::variations::VariationRegistry) -> Option<(String, String)> {
    // Try progressively longer prefixes until we find a matching variation
    let parts: Vec<&str> = key.split('_').collect();

    // Try from longest to shortest prefix (e.g., "pre_blur_strength" tries "pre_blur_strength", then "pre_blur")
    for i in (1..parts.len()).rev() {
        let potential_var = parts[..i].join("_");

        if let Some(var_info) = registry.get(&potential_var) {
            // Found matching variation, remaining part is parameter name
            let param_name = parts[i..].join("_");

            // Validate that this parameter exists for this variation
            if var_info.parameters.iter().any(|p| p.name == param_name) {
                return Some((potential_var, param_name));
            }
        }
    }

    None
}

/// Parse a <palette> element (256 RGB hex colors)
fn parse_palette_element(
    reader: &mut Reader<&[u8]>,
    _element: &quick_xml::events::BytesStart,
) -> Result<Palette> {
    let mut buf = Vec::new();
    let mut hex_data = String::new();

    // Read text content
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                hex_data.push_str(&e.unescape()?);
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"palette" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    // Parse hex data: 256 colors × 3 channels × 2 hex chars = 1536 chars
    let hex_data = hex_data.chars().filter(|c| !c.is_whitespace()).collect::<String>();

    if hex_data.len() < 1536 {
        return Err(anyhow::anyhow!(
            "Invalid palette data: expected 1536 hex characters, got {}",
            hex_data.len()
        ));
    }

    let mut stops = Vec::new();

    for i in 0..256 {
        let offset = i * 6;
        let r_str = &hex_data[offset..offset + 2];
        let g_str = &hex_data[offset + 2..offset + 4];
        let b_str = &hex_data[offset + 4..offset + 6];

        let r = u8::from_str_radix(r_str, 16)? as f32 / 255.0;
        let g = u8::from_str_radix(g_str, 16)? as f32 / 255.0;
        let b = u8::from_str_radix(b_str, 16)? as f32 / 255.0;

        stops.push(crate::scene::palette::ColorStop {
            position: i as f32 / 255.0,
            color: [r, g, b],
        });
    }

    Ok(Palette::new_locked("Imported from Apophysis", stops))
}

/// Parse Apophysis curves data into a ToneCurve
///
/// Apophysis stores 48 floats representing 4 curves × 12 values:
/// - Indices 0-11: Combined/luminance curve (the one we use)
/// - Indices 12-23: Red channel curve
/// - Indices 24-35: Green channel curve
/// - Indices 36-47: Blue channel curve
///
/// Each curve is a Weighted Cubic Bezier (Rational Bezier) with 4 control points:
/// - 12 values = 4 points × (x, y, weight)
/// - Formula: B(t) = Σ[w[i] × B³ᵢ(t) × P[i]] / Σ[w[i] × B³ᵢ(t)]
///
/// We sample the Bezier curve at 3 intermediate points (t=0.25, 0.5, 0.75) and
/// combine with the endpoints to create a 5-point linear approximation.
fn parse_apophysis_curves(data: &[f32]) -> ToneCurve {
    if data.len() != 48 {
        // Invalid data, return linear curve
        return ToneCurve::linear();
    }

    // Extract first 12 values (combined curve) as 4 control points × (x, y, w)
    let control_points = [
        (data[0], data[1], data[2]),   // Point 0
        (data[3], data[4], data[5]),   // Point 1
        (data[6], data[7], data[8]),   // Point 2
        (data[9], data[10], data[11]), // Point 3
    ];

    // Sample the Bezier curve at fixed t values
    let mut points = Vec::new();

    // Start point (t=0)
    points.push(crate::scene::tonemap::CurvePoint::new(0.0, 0.0));

    // Sample at t=0.25, 0.5, 0.75
    for &t in &[0.25, 0.5, 0.75] {
        if let Some((x, y)) = eval_rational_bezier(t, &control_points) {
            points.push(crate::scene::tonemap::CurvePoint::new(x, y));
        }
    }

    // End point (t=1)
    points.push(crate::scene::tonemap::CurvePoint::new(1.0, 1.0));

    // Sort by x coordinate (should already be sorted, but ensure it)
    points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

    ToneCurve { points }
}

/// Evaluate a rational cubic Bezier curve at parameter t
///
/// control_points: 4 tuples of (x, y, weight)
/// Returns: Some((x, y)) or None if denominator is zero
fn eval_rational_bezier(t: f32, control_points: &[(f32, f32, f32); 4]) -> Option<(f32, f32)> {
    let s = 1.0 - t;
    let s2 = s * s;
    let s3 = s2 * s;
    let t2 = t * t;
    let t3 = t2 * t;

    // Cubic Bernstein polynomials with weights
    let b0 = control_points[0].2 * s3;
    let b1 = control_points[1].2 * 3.0 * s2 * t;
    let b2 = control_points[2].2 * 3.0 * s * t2;
    let b3 = control_points[3].2 * t3;

    let nom_x = b0 * control_points[0].0 + b1 * control_points[1].0 +
                b2 * control_points[2].0 + b3 * control_points[3].0;
    let nom_y = b0 * control_points[0].1 + b1 * control_points[1].1 +
                b2 * control_points[2].1 + b3 * control_points[3].1;
    let denom = b0 + b1 + b2 + b3;

    if denom.abs() < 1e-10 {
        return None; // Avoid division by zero
    }

    Some((nom_x / denom, nom_y / denom))
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Serialize a `FractalConfig` to an Apophysis 7X `.flame` XML document.
///
/// The output is the inverse of `parse_flame_xml`. Round-tripping the
/// fields the parser reads is the design target — anything we read on
/// import gets written back. Fields we don't read are written with
/// Apo-reasonable defaults (`quality`, `oversample`, `enable_de`, etc.)
/// so the file looks complete to other tools.
///
/// Things this does NOT preserve, by design:
///   - Our extended UI state (palette squeeze, levels, effects, etc.) —
///     those live in `.fflame`, not `.flame`. Use `.fflame` for lossless
///     round-trip with ourselves.
///   - Tone curves — `curves` is always written as the 48-value Apo
///     identity. Re-importing one of our exports loses any non-linear
///     tonemap_curve. Could be reversed by inverting the Bezier sampler
///     in `parse_apophysis_curves`, but that's non-trivial and not
///     needed for the export → Apo workflow.
///   - Subflames and linked transforms — `subflame_wf` references are
///     written as the variation attribute, but child flames are not
///     emitted in this XML.
///   - Variation **init params** (`_dx`, `_dy`, etc.) — these are
///     derived values our GPU recomputes on load, not user-facing, and
///     Apo doesn't have the concept.
pub fn write_flame_xml(config: &FractalConfig) -> String {
    let mut out = String::with_capacity(8192);
    let version = format!(
        "Fractal Flame WGPU {}",
        env!("CARGO_PKG_VERSION")
    );

    // Inverse of importer's conversions:
    //   - importer: zoom = scale / 200.0           → apo_scale = zoom × 200
    //   - importer: rotation = rotate × π / 180    → rotate_deg = rotation × 180 / π
    //   - importer: ui_gt = apo_gt × 2000 + 50     → apo_gt = (ui_gt − 50) / 2000
    let apo_scale = config.zoom * 200.0;
    let rotate_deg = config.rotation * 180.0 / std::f32::consts::PI;
    let apo_gamma_threshold = ((config.gamma_threshold - 50.0) / 2000.0).max(0.0);

    // 1920×1080 default — FractalConfig doesn't carry a render size.
    // Apo just uses this for the preview canvas; doesn't affect math.
    let size = (1920, 1080);

    let bg = config.background_color;
    let bg_str = format!(
        "{} {} {}",
        (bg[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (bg[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (bg[2].clamp(0.0, 1.0) * 255.0).round() as u8,
    );

    // `plugins` — Apo wants the union of variation names across all
    // xforms (and the finalxform if any). Sorted for determinism.
    let plugins = collect_plugin_names(&config.flame);

    out.push_str("<flames name=\"");
    out.push_str(&xml_escape_attr(&config.flame.name));
    out.push_str("\">\n");

    out.push_str("<flame name=\"");
    out.push_str(&xml_escape_attr(&config.flame.name));
    out.push_str(&format!("\" version=\"{}\"", xml_escape_attr(&version)));
    out.push_str(&format!(" size=\"{} {}\"", size.0, size.1));
    out.push_str(&format!(" center=\"{} {}\"", fmt_f32(config.pan_x), fmt_f32(config.pan_y)));
    out.push_str(&format!(" scale=\"{}\"", fmt_f32(apo_scale)));
    if rotate_deg.abs() > 1e-6 {
        out.push_str(&format!(" rotate=\"{}\"", fmt_f32(rotate_deg)));
    }
    // Camera (3D). Always written when nonzero; zero defaults are dropped
    // so 2D flames don't carry dead camera attributes.
    if config.camera_rotation_x.abs() > 1e-6 {
        out.push_str(&format!(" cam_pitch=\"{}\"", fmt_f32(config.camera_rotation_x)));
    }
    if config.camera_rotation_y.abs() > 1e-6 {
        out.push_str(&format!(" cam_yaw=\"{}\"", fmt_f32(config.camera_rotation_y)));
    }
    if config.camera_z.abs() > 1e-6 {
        out.push_str(&format!(" cam_zpos=\"{}\"", fmt_f32(config.camera_z)));
    }
    if config.flame.perspective_strength.abs() > 1e-6 {
        out.push_str(&format!(" cam_perspective=\"{}\"", fmt_f32(config.flame.perspective_strength)));
    }
    if config.dof_blur_strength.abs() > 1e-6 {
        out.push_str(&format!(" cam_dof=\"{}\"", fmt_f32(config.dof_blur_strength)));
    }
    // Standard Apo attrs that the importer reads.
    out.push_str(" oversample=\"1\"");
    if config.filter_radius.abs() > 1e-6 {
        out.push_str(&format!(" filter=\"{}\"", fmt_f32(config.filter_radius)));
    }
    out.push_str(" quality=\"50\"");
    out.push_str(&format!(" background=\"{}\"", bg_str));
    out.push_str(&format!(" brightness=\"{}\"", fmt_f32(config.brightness)));
    out.push_str(&format!(" gamma=\"{}\"", fmt_f32(config.gamma)));
    if (config.vibrancy - 1.0).abs() > 1e-6 {
        out.push_str(&format!(" vibrancy=\"{}\"", fmt_f32(config.vibrancy)));
    }
    out.push_str(&format!(" gamma_threshold=\"{}\"", fmt_f32(apo_gamma_threshold)));
    // Apo density-estimator block — we don't use it, write its defaults
    // so the file looks complete.
    out.push_str(" estimator_radius=\"9\" estimator_minimum=\"0\" estimator_curve=\"0.4\" enable_de=\"0\"");
    out.push_str(&format!(" plugins=\"{}\"", xml_escape_attr(&plugins)));
    out.push_str(" new_linear=\"1\"");
    // Identity tone curves — see module note above on why we don't
    // attempt to round-trip the Bezier.
    out.push_str(" curves=\"");
    let identity_curve = "0 0 1 0 0 1 1 1 1 1 1 1";
    for i in 0..4 {
        if i > 0 { out.push(' '); }
        out.push_str(identity_curve);
    }
    out.push('"');
    if let Some(idx) = config.flame.solo_transform {
        out.push_str(&format!(" soloxform=\"{}\"", idx));
    }
    out.push_str(" >\n");

    // Normal transforms (`<xform>`).
    let registry = global_registry();
    let xaos = config.flame.xaos.as_ref();
    for (i, xform) in config.flame.transforms.iter().enumerate() {
        let chaos_row = xaos.and_then(|m| m.get(i));
        write_xform(&mut out, xform, false, chaos_row, &*registry);
    }

    // Final transforms. Apo's `<finalxform>` is singular — when we have
    // multiple, write the first and drop the rest with a log warning.
    // (Multi-final is a per-transform feature we added on top of Apo;
    // there's no Apo-side concept for it.)
    if let Some(final_xform) = config.flame.final_transforms.first() {
        write_xform(&mut out, final_xform, true, None, &*registry);
        if config.flame.final_transforms.len() > 1 {
            log::warn!(
                "write_flame_xml: dropping {} extra final transform(s) — Apo XML only supports a single <finalxform>",
                config.flame.final_transforms.len() - 1
            );
        }
    }

    // Palette: 256 colors, RGB hex, 8 colors per line.
    write_palette(&mut out, &config.palette);

    out.push_str("</flame>\n");
    out.push_str("</flames>\n");

    out
}

/// Format an f32 using Rust's default formatter, which produces the
/// shortest decimal that round-trips exactly. Mirrors what serde_json
/// does and matches Apo's mixed-precision output style.
fn fmt_f32(v: f32) -> String {
    if v == 0.0 {
        "0".to_string()
    } else {
        format!("{}", v)
    }
}

/// Minimal XML attribute-value escape. Variation names and numbers
/// don't need this, but flame `name`s come from user input.
fn xml_escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Build the `plugins` attribute value — space-separated unique
/// variation names across all xforms (and the final, if any). Sorted
/// for determinism.
fn collect_plugin_names(flame: &Flame) -> String {
    let mut names: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for xform in &flame.transforms {
        for name in xform.variations.keys() {
            names.insert(name.as_str());
        }
    }
    for xform in &flame.final_transforms {
        for name in xform.variations.keys() {
            names.insert(name.as_str());
        }
    }
    names.into_iter().collect::<Vec<_>>().join(" ")
}

/// Write one `<xform>` or `<finalxform>` element. `is_final = true`
/// suppresses the `weight` and `opacity` attributes (Apo's finalxform
/// has neither). `chaos_row` is this transform's row of the xaos matrix
/// (write only if it has any non-1.0 entry).
fn write_xform(
    out: &mut String,
    xform: &Transform,
    is_final: bool,
    chaos_row: Option<&Vec<f32>>,
    registry: &crate::variations::VariationRegistry,
) {
    out.push_str(if is_final { "   <finalxform" } else { "   <xform" });

    if !is_final {
        out.push_str(&format!(" weight=\"{}\"", fmt_f32(xform.weight)));
    }
    out.push_str(&format!(" color=\"{}\"", fmt_f32(xform.color)));
    if xform.color_speed.abs() > 1e-6 {
        out.push_str(&format!(" color_speed=\"{}\"", fmt_f32(xform.color_speed)));
    }
    if !is_final {
        // Apo always writes opacity; we follow suit so a re-import sees
        // the same value rather than the parser default.
        out.push_str(&format!(" opacity=\"{}\"", fmt_f32(xform.opacity)));
    }
    if xform.direct_color.abs() > 1e-6 {
        out.push_str(&format!(" pluginColor=\"{}\"", fmt_f32(xform.direct_color)));
    }

    // Variations — sorted for deterministic output.
    let mut variation_names: Vec<&String> = xform.variations.keys().collect();
    variation_names.sort();
    for name in &variation_names {
        let weight = xform.variations[*name];
        out.push_str(&format!(" {}=\"{}\"", name, fmt_f32(weight)));
    }

    // Coefs: importer parses "a c b d e f" (column-major), so we
    // serialize in that exact order.
    out.push_str(&format!(
        " coefs=\"{} {} {} {} {} {}\"",
        fmt_f32(xform.a), fmt_f32(xform.c),
        fmt_f32(xform.b), fmt_f32(xform.d),
        fmt_f32(xform.e), fmt_f32(xform.f),
    ));

    if xform.post_affine_enabled {
        out.push_str(&format!(
            " post=\"{} {} {} {} {} {}\"",
            fmt_f32(xform.post_a), fmt_f32(xform.post_c),
            fmt_f32(xform.post_b), fmt_f32(xform.post_d),
            fmt_f32(xform.post_e), fmt_f32(xform.post_f),
        ));
    }

    // Variation parameters: `variation_params` keys are
    // "varname.paramname". Only emit entries whose variation is active
    // in this xform (orphan params from a removed variation don't get
    // written) and whose param is registered as a user-facing parameter
    // (skips derived/init slots, which aren't in `parameters` anyway).
    let mut param_entries: Vec<(&String, f32)> = xform.variation_params.iter()
        .map(|(k, v)| (k, *v))
        .collect();
    param_entries.sort_by_key(|(k, _)| k.as_str());
    for (key, value) in param_entries {
        let Some((var_name, param_name)) = key.split_once('.') else { continue };
        if !xform.variations.contains_key(var_name) {
            continue;
        }
        // Validate the param is user-facing (defensive — `variation_params`
        // shouldn't contain non-registered entries, but check anyway).
        let is_registered = registry.get(var_name)
            .map(|info| info.parameters.iter().any(|p| p.name == param_name))
            .unwrap_or(false);
        if !is_registered {
            continue;
        }
        out.push_str(&format!(" {}_{}=\"{}\"", var_name, param_name, fmt_f32(value)));
    }

    if let Some(row) = chaos_row {
        // Only emit `chaos` if any entry differs from the default 1.0
        // (importer fills missing rows with all-1.0).
        if row.iter().any(|w| (w - 1.0).abs() > 1e-6) {
            out.push_str(" chaos=\"");
            for (i, w) in row.iter().enumerate() {
                if i > 0 { out.push(' '); }
                out.push_str(&fmt_f32(*w));
            }
            out.push('"');
        }
    }

    out.push_str(" />\n");
}

/// Write the 256-entry palette as Apo-formatted hex: 8 colors per line,
/// 48 hex chars each, lowercase. Apo emits uppercase but its parser is
/// case-insensitive; we use uppercase to match the sample files.
fn write_palette(out: &mut String, palette: &crate::scene::palette::Palette) {
    out.push_str("   <palette count=\"256\" format=\"RGB\">\n");
    for row in 0..32 {
        out.push_str("      ");
        for col in 0..8 {
            let i = row * 8 + col;
            let position = i as f32 / 255.0;
            let color = palette.sample_color(position);
            let r = (color[0].clamp(0.0, 1.0) * 255.0).round() as u8;
            let g = (color[1].clamp(0.0, 1.0) * 255.0).round() as u8;
            let b = (color[2].clamp(0.0, 1.0) * 255.0).round() as u8;
            out.push_str(&format!("{:02X}{:02X}{:02X}", r, g, b));
        }
        out.push('\n');
    }
    out.push_str("   </palette>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_spherical_example() {
        // Minimal test without palette (palette parsing is tested elsewhere)
        let xml = r#"
<flames name="spherical-apo">
<flame name="Spherical Test" version="Apophysis 7x Version 15D" size="1500 1000" center="0.12666568780208 0.0566529891945883" scale="208.227773031847" background="0 0 0" brightness="1" gamma="1">
   <xform weight="1" color="0" spherical="1" coefs="0.9 0 0 0.9 0 0" opacity="1" />
</flame>
</flames>
        "#;

        let result = parse_flame_xml(xml);
        assert!(result.is_ok(), "Failed to parse XML: {:?}", result.err());

        let configs = result.unwrap();
        assert_eq!(configs.len(), 1);

        let config = &configs[0];
        assert_eq!(config.flame.name, "Spherical Test");
        assert_eq!(config.flame.transforms.len(), 1);

        let xform = &config.flame.transforms[0];
        assert_eq!(xform.weight, 1.0);
        assert_eq!(xform.a, 0.9);
        assert_eq!(xform.d, 0.9);
        assert!(xform.variations.contains_key("spherical"));
        assert_eq!(xform.variations["spherical"], 1.0);
    }

    #[test]
    fn test_coef_order_column_major() {
        // Test that we're parsing column-major order correctly
        // Apophysis XML: coefs="a c b d e f"
        let xml = r#"
<flames name="test">
<flame name="Rotation Test" size="800 600" center="0 0" scale="200">
   <xform weight="1" linear="1" coefs="0.34284 0.564847 -0.564847 0.34284 1.5 2.5" />
</flame>
</flames>
        "#;

        let result = parse_flame_xml(xml);
        assert!(result.is_ok());

        let config = &result.unwrap()[0];
        let xform = &config.flame.transforms[0];

        // coefs="0.34284 0.564847 -0.564847 0.34284 1.5 2.5"
        // Should parse as: a c b d e f
        assert_eq!(xform.a, 0.34284);      // parts[0]
        assert_eq!(xform.c, 0.564847);     // parts[1]
        assert_eq!(xform.b, -0.564847);    // parts[2]
        assert_eq!(xform.d, 0.34284);      // parts[3]
        assert_eq!(xform.e, 1.5);          // parts[4]
        assert_eq!(xform.f, 2.5);          // parts[5]
    }

    #[test]
    fn test_variation_parameters() {
        // Test parsing variation parameters like julian_power and julian_dist
        let xml = r#"
<flames name="test">
<flame name="Julian Test" size="800 600" center="0 0" scale="200">
   <xform weight="0.5" color="1" bubble="0.2" pre_blur="10" coefs="1 0 0 1 0 0" opacity="1" />
   <xform weight="6" color="0" flatten="1" julian="1" coefs="0.707107 -0.707107 0.707107 0.707107 0 -0.3" julian_power="2" julian_dist="-1" opacity="1" />
   <xform weight="1" color="0.5" blob="0.5" coefs="1 0 0 1 0 0" blob_high="1.5" blob_low="0.8" blob_waves="6" opacity="1" />
</flame>
</flames>
        "#;

        let result = parse_flame_xml(xml);
        assert!(result.is_ok(), "Failed to parse XML: {:?}", result.err());

        let config = &result.unwrap()[0];
        assert_eq!(config.flame.transforms.len(), 3);

        // First transform: bubble + pre_blur (no parameters on these)
        let xform0 = &config.flame.transforms[0];
        assert_eq!(xform0.weight, 0.5);
        assert!(xform0.variations.contains_key("bubble"));
        assert!(xform0.variations.contains_key("pre_blur"));
        assert_eq!(xform0.variations["bubble"], 0.2);
        assert_eq!(xform0.variations["pre_blur"], 10.0);

        // Second transform: julian with power and dist parameters
        let xform1 = &config.flame.transforms[1];
        assert_eq!(xform1.weight, 6.0);
        assert!(xform1.variations.contains_key("flatten"));
        assert!(xform1.variations.contains_key("julian"));
        assert_eq!(xform1.variations["julian"], 1.0);

        // Check julian parameters
        assert_eq!(xform1.get_variation_param_or_default("julian", "power", &global_registry()), 2.0);
        assert_eq!(xform1.get_variation_param_or_default("julian", "dist", &global_registry()), -1.0);

        // Third transform: blob with high, low, waves parameters
        let xform2 = &config.flame.transforms[2];
        assert_eq!(xform2.weight, 1.0);
        assert!(xform2.variations.contains_key("blob"));
        assert_eq!(xform2.variations["blob"], 0.5);

        // Check blob parameters
        assert_eq!(xform2.get_variation_param_or_default("blob", "high", &global_registry()), 1.5);
        assert_eq!(xform2.get_variation_param_or_default("blob", "low", &global_registry()), 0.8);
        assert_eq!(xform2.get_variation_param_or_default("blob", "waves", &global_registry()), 6.0);
    }

    #[test]
    fn test_post_affine_import() {
        // Test that post attribute is parsed correctly
        let xml = r#"
<flames name="test">
<flame name="Post-Affine Test" size="800 600" center="0 0" scale="200">
   <xform weight="0.5" color="1" bubble="0.2" pre_blur="10" coefs="1 0 0 1 0 0" post="0.8 0 0 1 0 0" opacity="1" />
   <xform weight="1" color="0" linear="1" coefs="0.9 0 0 0.9 0 0" opacity="1" />
   <finalxform color="0" linear="1" coefs="1 0 0 1 0 0" post="0.5 0.1 -0.1 0.5 0.2 0.3" />
</flame>
</flames>
        "#;

        let result = parse_flame_xml(xml);
        assert!(result.is_ok(), "Failed to parse XML: {:?}", result.err());

        let config = &result.unwrap()[0];
        assert_eq!(config.flame.transforms.len(), 2);

        // First transform has post-affine
        let xform0 = &config.flame.transforms[0];
        assert!(xform0.post_affine_enabled);
        assert_eq!(xform0.post_a, 0.8);   // parts[0]
        assert_eq!(xform0.post_c, 0.0);   // parts[1]
        assert_eq!(xform0.post_b, 0.0);   // parts[2]
        assert_eq!(xform0.post_d, 1.0);   // parts[3]
        assert_eq!(xform0.post_e, 0.0);   // parts[4]
        assert_eq!(xform0.post_f, 0.0);   // parts[5]

        // Second transform has no post-affine
        let xform1 = &config.flame.transforms[1];
        assert!(!xform1.post_affine_enabled);
        assert_eq!(xform1.post_a, 1.0);   // identity
        assert_eq!(xform1.post_d, 1.0);   // identity

        // Final transform has post-affine (now lives in the final_transforms pool).
        let final_xform = config.flame.final_transforms.first().unwrap();
        assert!(final_xform.post_affine_enabled);
        assert_eq!(final_xform.post_a, 0.5);
        assert_eq!(final_xform.post_c, 0.1);   // parts[1]
        assert_eq!(final_xform.post_b, -0.1);  // parts[2]
        assert_eq!(final_xform.post_d, 0.5);
        assert_eq!(final_xform.post_e, 0.2);
        assert_eq!(final_xform.post_f, 0.3);
    }

    #[test]
    fn test_find_variation_and_param() {
        let registry = global_registry();

        // Simple case: julian_power
        let result = find_variation_and_param("julian_power", &registry);
        assert_eq!(result, Some(("julian".to_string(), "power".to_string())));

        // Simple case: blob_high
        let result = find_variation_and_param("blob_high", &registry);
        assert_eq!(result, Some(("blob".to_string(), "high".to_string())));

        // Case with underscores in variation name (if pre_blur had params)
        // For now we don't have pre_blur params, but the logic should handle it
        let result = find_variation_and_param("pre_blur_strength", &registry);
        // pre_blur has no "strength" param, so this should return None
        assert_eq!(result, None);

        // Invalid parameter name
        let result = find_variation_and_param("julian_invalid", &registry);
        assert_eq!(result, None);

        // Invalid variation name
        let result = find_variation_and_param("invalid_param", &registry);
        assert_eq!(result, None);
    }

    /// Round-trip: write a flame to XML and re-parse it, asserting the
    /// fields the parser reads survive byte-for-byte through
    /// `write_flame_xml`. Anything the parser doesn't read is not
    /// asserted here.
    #[test]
    fn test_roundtrip_basic() {
        let xml_in = r#"
<flames name="rt-test">
<flame name="RoundTrip" version="Apophysis 7x Version 15D" size="1500 1000" center="0.123 -0.456" scale="375" cam_pitch="0.943" cam_dof="0.194" oversample="1" filter="0.5" quality="50" background="0 0 0" brightness="4" gamma="4" gamma_threshold="0.01" plugins="linear julian" new_linear="1" >
   <xform weight="0.5" color="0" linear="1" coefs="0.125 0 0 0.125 -0.002 0.002" opacity="1" />
   <xform weight="6" color="0.481" julian="1" coefs="0.707107 -0.707107 0.707107 0.707107 0 -0.3" julian_power="2" julian_dist="-1" opacity="0.8" />
</flame>
</flames>
        "#;

        let configs = parse_flame_xml(xml_in).expect("parse in");
        assert_eq!(configs.len(), 1);
        let original = &configs[0];

        let xml_out = write_flame_xml(original);
        let configs_back = parse_flame_xml(&xml_out).expect("parse out");
        assert_eq!(configs_back.len(), 1);
        let back = &configs_back[0];

        assert_eq!(back.flame.name, "RoundTrip");
        assert_eq!(back.flame.transforms.len(), 2);

        // Affine: importer parses "a c b d e f"; exporter writes the
        // same order. Spot-check the second xform's rotation matrix.
        let x1 = &back.flame.transforms[1];
        assert!((x1.a - 0.707107).abs() < 1e-5, "a: {}", x1.a);
        assert!((x1.b - 0.707107).abs() < 1e-5, "b: {}", x1.b);
        assert!((x1.c - -0.707107).abs() < 1e-5, "c: {}", x1.c);
        assert!((x1.d - 0.707107).abs() < 1e-5, "d: {}", x1.d);
        assert_eq!(x1.weight, 6.0);
        assert!((x1.opacity - 0.8).abs() < 1e-6, "opacity: {}", x1.opacity);
        assert_eq!(x1.variations["julian"], 1.0);
        assert_eq!(
            x1.get_variation_param_or_default("julian", "power", &global_registry()),
            2.0
        );
        assert_eq!(
            x1.get_variation_param_or_default("julian", "dist", &global_registry()),
            -1.0
        );

        // Camera attrs survived.
        assert!((back.camera_rotation_x - 0.943).abs() < 1e-4);
        assert!((back.dof_blur_strength - 0.194).abs() < 1e-4);

        // Scale → zoom conversion is the inverse of the importer's
        // `zoom = scale / 200`: 375 / 200 = 1.875.
        assert!((back.zoom - 1.875).abs() < 1e-4, "zoom: {}", back.zoom);

        // Tonemap basics.
        assert!((back.brightness - 4.0).abs() < 1e-4);
        assert!((back.gamma - 4.0).abs() < 1e-4);

        // Pan, exactly preserved.
        assert!((back.pan_x - 0.123).abs() < 1e-4);
        assert!((back.pan_y - -0.456).abs() < 1e-4);
    }

    /// Exporting a flame with post-affine and chaos should produce XML
    /// that re-imports those features faithfully.
    #[test]
    fn test_roundtrip_post_and_chaos() {
        let xml_in = r#"
<flames name="pc-test">
<flame name="PostChaos" size="800 600" center="0 0" scale="200" background="0 0 0" brightness="1" gamma="2.2">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" post="0.8 0.1 -0.1 0.5 0.2 0.3" opacity="1" chaos="1 0.5" />
   <xform weight="1" color="0.5" spherical="1" coefs="0.9 0 0 0.9 0 0" opacity="1" chaos="0 1" />
</flame>
</flames>
        "#;

        let original = parse_flame_xml(xml_in).expect("parse").into_iter().next().unwrap();
        let xml_out = write_flame_xml(&original);
        let back = parse_flame_xml(&xml_out).expect("re-parse").into_iter().next().unwrap();

        // Post-affine on xform 0.
        let x0 = &back.flame.transforms[0];
        assert!(x0.post_affine_enabled);
        assert!((x0.post_a - 0.8).abs() < 1e-5);
        assert!((x0.post_b - -0.1).abs() < 1e-5);
        assert!((x0.post_c - 0.1).abs() < 1e-5);
        assert!((x0.post_d - 0.5).abs() < 1e-5);
        assert!((x0.post_e - 0.2).abs() < 1e-5);
        assert!((x0.post_f - 0.3).abs() < 1e-5);

        // Chaos matrix survived (2×2 with non-default off-diagonals).
        let xaos = back.flame.xaos.as_ref().expect("xaos should be set");
        assert_eq!(xaos.len(), 2);
        assert!((xaos[0][0] - 1.0).abs() < 1e-6);
        assert!((xaos[0][1] - 0.5).abs() < 1e-6);
        assert!((xaos[1][0] - 0.0).abs() < 1e-6);
        assert!((xaos[1][1] - 1.0).abs() < 1e-6);
    }

    /// Real-world Apo file (`output/ship-on-the-sea.flame`, exported
    /// from Apophysis 7X v15C.9 with 3 xforms, julia3D, zscale, custom
    /// palette, cam_pitch, cam_dof) — should round-trip through our
    /// import/export without losing any field the parser reads.
    #[test]
    fn test_roundtrip_real_apo_file() {
        let xml_in = include_str!("../output/ship-on-the-sea.flame");

        let original = parse_flame_xml(xml_in).expect("parse real Apo file")
            .into_iter().next().unwrap();
        let xml_out = write_flame_xml(&original);
        let back = parse_flame_xml(&xml_out).expect("re-parse our export")
            .into_iter().next().unwrap();

        // Structural parity: same xform count, same variation set,
        // same palette length.
        assert_eq!(original.flame.transforms.len(), back.flame.transforms.len());
        assert_eq!(original.palette.stops.len(), back.palette.stops.len());

        // View state.
        assert!((original.zoom - back.zoom).abs() < 1e-3, "zoom: {} vs {}", original.zoom, back.zoom);
        assert!((original.pan_x - back.pan_x).abs() < 1e-5);
        assert!((original.pan_y - back.pan_y).abs() < 1e-5);
        assert!((original.camera_rotation_x - back.camera_rotation_x).abs() < 1e-4);
        assert!((original.dof_blur_strength - back.dof_blur_strength).abs() < 1e-4);

        // Tonemap.
        assert!((original.brightness - back.brightness).abs() < 1e-4);
        assert!((original.gamma - back.gamma).abs() < 1e-4);

        // Per-xform: weight, color, affine, variations preserved.
        for (i, (orig, rt)) in original.flame.transforms.iter()
            .zip(back.flame.transforms.iter()).enumerate()
        {
            assert!((orig.weight - rt.weight).abs() < 1e-5, "xform {} weight", i);
            assert!((orig.opacity - rt.opacity).abs() < 1e-5, "xform {} opacity", i);
            assert!((orig.a - rt.a).abs() < 1e-5, "xform {} a", i);
            assert!((orig.b - rt.b).abs() < 1e-5, "xform {} b", i);
            assert!((orig.c - rt.c).abs() < 1e-5, "xform {} c", i);
            assert!((orig.d - rt.d).abs() < 1e-5, "xform {} d", i);
            assert!((orig.e - rt.e).abs() < 1e-5, "xform {} e", i);
            assert!((orig.f - rt.f).abs() < 1e-5, "xform {} f", i);
            // Variation names + weights identical.
            assert_eq!(
                orig.variations.keys().collect::<std::collections::BTreeSet<_>>(),
                rt.variations.keys().collect::<std::collections::BTreeSet<_>>(),
                "xform {} variation set", i,
            );
            for (name, &w) in &orig.variations {
                let rt_w = rt.variations.get(name).copied().unwrap_or(0.0);
                assert!((w - rt_w).abs() < 1e-5, "xform {} var {}: {} vs {}", i, name, w, rt_w);
            }
        }
    }

    /// The palette path: write a custom palette out and read it back,
    /// checking that the 256 RGB triplets survive the hex round-trip.
    #[test]
    fn test_roundtrip_palette() {
        use crate::scene::palette::{ColorStop, Palette};

        // Two-stop palette: black at 0, red at 1. Sampling yields a
        // smooth fade with predictable midpoint.
        let stops = vec![
            ColorStop { position: 0.0, color: [0.0, 0.0, 0.0] },
            ColorStop { position: 1.0, color: [1.0, 0.0, 0.0] },
        ];
        let palette = Palette::new_locked("rt", stops);

        let mut config = FractalConfig::default();
        config.palette = palette;
        config.flame.name = "PaletteRT".to_string();

        let xml = write_flame_xml(&config);
        let back = parse_flame_xml(&xml).expect("parse").into_iter().next().unwrap();

        // Imported palettes have 256 stops on linear positions.
        assert_eq!(back.palette.stops.len(), 256);
        // Endpoints survive (within 1/255 quantization).
        let r0 = back.palette.stops[0].color[0];
        let r255 = back.palette.stops[255].color[0];
        assert!(r0 < 0.01, "stop 0 red: {}", r0);
        assert!(r255 > 0.99, "stop 255 red: {}", r255);
    }
}
