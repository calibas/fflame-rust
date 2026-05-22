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
    let mut cam_pitch = 0.0;  // Camera rotation X (radians)
    let mut cam_yaw = 0.0;    // Camera rotation Y (radians)
    let mut cam_zpos = 0.0;   // Camera Z position (height)
    let mut cam_perspective = 0.0;  // Perspective strength
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
            "cam_pitch" => cam_pitch = value.parse().unwrap_or(0.0),
            "cam_yaw" => cam_yaw = value.parse().unwrap_or(0.0),
            "cam_zpos" => cam_zpos = value.parse().unwrap_or(0.0),
            "cam_perspective" => cam_perspective = value.parse().unwrap_or(0.0),
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

    // Determine render mode: if any camera parameters are set, use 3D mode
    let has_camera_params = f32::abs(cam_pitch) > 0.0001
        || f32::abs(cam_yaw) > 0.0001
        || f32::abs(cam_zpos) > 0.0001
        || f32::abs(cam_perspective) > 0.0001;

    let render_mode = if has_camera_params {
        RenderMode::ThreeD
    } else {
        RenderMode::TwoD
    };

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
        dof_blur_strength: crate::config::defaults::DEFAULT_DOF_BLUR_STRENGTH,
        fog_strength: crate::config::defaults::DEFAULT_FOG_STRENGTH,
        fog_start: crate::config::defaults::DEFAULT_FOG_START,
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
        saturation: 1.5,  // Default saturation
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
        // Levels controls - use defaults (× mean density units)
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
}
