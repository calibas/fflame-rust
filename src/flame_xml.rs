//! `.flame` XML format parser and writer — Apophysis 7X / JWildfire /
//! Chaotica compatible.
//!
//! The three flame editors share an XML core (`<flame>` with attribute
//! camera/tonemap/palette settings, `<xform>` children with affine
//! coefficients and variation weights, `<palette>` with 256 hex RGB
//! triplets). Apophysis is the original; JWildfire and Chaotica added
//! their own attributes on top. We parse the common core plus the
//! JWF/Chaotica extras we have a home for (e.g. `cam_persp`,
//! `saturation`, `white_level`, `subflame_wf_flame`'s hex-encoded
//! child); unknown attributes are silently dropped.
//!
//! On export we write the Apophysis dialect plus JWF's subflame
//! companion attrs when needed — that gets us files JWF (and usually
//! Apo) can re-open.
//!
//! XML Structure:
//! ```xml
//! <flames name="collection_name">
//!   <flame name="..." version="..." size="W H" center="X Y" scale="..."
//!          background="R G B" brightness="..." gamma="..." ...>
//!     <xform weight="..." color="..." coefs="a c b d e f" opacity="..."
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
                // Apophysis/older-JWF use `<flame>`; newer JWildfire saves
                // a layered `<jwf-flame>` whose flame attributes live on the
                // root element and whose xforms/palette are nested in
                // `<layer>` children. We parse the FIRST layer as the flame
                // and ignore the rest (and `<bgxform>`) — see
                // parse_flame_element.
                let n = e.name();
                if n.as_ref() == b"flame" || n.as_ref() == b"jwf-flame" {
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
        return Err(anyhow::anyhow!("No <flame> or <jwf-flame> elements found in XML"));
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
    // `size` lands on `FractalConfig::image_size` below. Default
    // matches both the pre-feature behavior (we used to write a
    // fixed `size="1920 1080"` on export) and the FractalConfig
    // default, so missing-attribute flames behave unchanged.
    let mut size: (u32, u32) = (1920, 1080);
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
    let mut cam_bank: f32 = 0.0;  // Camera bank — lands here from XML `cam_roll` (rename quirk; see comment below)
    // Camera world-space position. JWildfire writes `cam_pos_x/y/z`
    // on every flame; the older `cam_zpos` single-axis form also
    // appears in Apo and older JWildfire saves. We honor `cam_pos_z`
    // when present and fall back to `cam_zpos` otherwise.
    //
    // SEMANTICS CAVEAT (deliberate divergence from JWildfire): JWF
    // applies cam_pos AFTER the camera rotation, in camera space,
    // ADDED (`FlameRendererView.project`: `camPoint = Mᵀ·p + camPos`,
    // plus an extra `+ camPosZ` term in the perspective denominator).
    // Ours is a true world-space camera position applied before
    // rotation (`M·(p − camera_pos)`) — what free-fly navigation
    // needs. Flames with non-zero cam_pos therefore render
    // differently here vs JWF, in both round-trip directions.
    //
    // TODO: an exact position conversion exists for the rotation
    // part — import as `c_ours = −M_eff^T·c_jwf`, export as
    // `c_jwf = −M_eff·c_ours`, with M_eff built from the four camera
    // angles — but JWF's extra `+camPosZ` perspective term has no
    // counterpart in our projection, so the conversion is only
    // faithful for orthographic flames (or cam_pos_z = 0). Skipped
    // until someone actually hits this; JWF's own random flames all
    // write cam_pos as zeros.
    let mut cam_pos_x: f32 = 0.0;
    let mut cam_pos_y: f32 = 0.0;
    let mut cam_pos_z_opt: Option<f32> = None;
    let mut cam_zpos: f32 = 0.0;   // Camera Z position — legacy single-axis
    let mut cam_perspective = 0.0;  // Perspective strength
    let mut cam_dof = 0.0;    // Depth-of-field blur strength (Apo's `cam_dof`)
    let mut curves: Option<Vec<f32>> = None;  // Tone curve data (48 floats)
    let mut solo_xform: Option<usize> = None;  // Solo transform index (0-indexed)

    // JWF-specific attrs that have a home in our config.
    // `cam_zoom` is a multiplier on top of `scale` in JWF — final
    // visible scale is `scale × cam_zoom`. We fold it into the zoom
    // conversion so a JWF flame opens at the same magnification it
    // would in JWF.
    let mut cam_zoom_factor: f32 = 1.0;
    // JWildfire solid-rendering block (`sld_render_*`): collected
    // verbatim here, mapped onto our splat-native solid model after
    // the attribute loop (see solid_shading_from_sld).
    let mut sld_attrs: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut saturation: f32 = crate::config::defaults::DEFAULT_SATURATION;
    let mut white_level: f32 = crate::config::defaults::DEFAULT_WHITE_LEVEL;

    // JWildfire `post_symmetry_*` attrs. Apo files don't write these,
    // so the defaults stay until JWF tokens overwrite them.
    let mut post_symmetry = crate::scene::transforms::PostSymmetry::default();

    // JWildfire `preserve_z` — matches Apo/JWF default of false (Z
    // reset each iteration). The attr is omitted entirely when JWF
    // wants the default, so we keep the false initial value unless
    // we encounter the token explicitly.
    let mut preserve_z = false;

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
            // JWildfire rename quirk: the XML attribute named `cam_roll`
            // actually carries the internal *bank* field (Y-axis
            // rotation), not the roll. JWildfire's `roll` parameter
            // goes out as the top-level `rotate` attribute in degrees,
            // which we already parse into `rotation`. Confirmed from
            // upstream JWF source `AbstractFlameWriter.java`:
            //   createAttr("rotate", pFlame.getCamRoll())
            //   createAttr("cam_roll", (pFlame.getCamBank() * Math.PI) / 180.0)
            // So `cam_roll` here lands on `camera_bank`, in radians.
            // See docs/projects/jwf-features.md ("Camera rotation").
            "cam_roll" => cam_bank = value.parse().unwrap_or(0.0),
            "cam_zpos" => cam_zpos = value.parse().unwrap_or(0.0),
            // JWildfire's modern 3-axis camera position attributes.
            // `cam_pos_z` takes priority over the legacy `cam_zpos`
            // when both appear (the fallback happens at the
            // FractalConfig construction site below).
            "cam_pos_x" => cam_pos_x = value.parse().unwrap_or(0.0),
            "cam_pos_y" => cam_pos_y = value.parse().unwrap_or(0.0),
            "cam_pos_z" => cam_pos_z_opt = value.parse().ok(),
            // Apo uses `cam_perspective`; JWF uses `cam_persp`.
            "cam_perspective" | "cam_persp" => cam_perspective = value.parse().unwrap_or(0.0),
            "cam_dof" => cam_dof = value.parse().unwrap_or(0.0),
            // JWF post-scale multiplier; folded into our zoom below.
            "cam_zoom" => cam_zoom_factor = value.parse().unwrap_or(1.0),
            // JWF/Apo color knobs. Apo files usually don't set
            // saturation; JWF always does. white_level is JWF's
            // tonemap white point (default 220).
            "saturation" => saturation = value.parse().unwrap_or(crate::config::defaults::DEFAULT_SATURATION),
            "white_level" => white_level = value.parse().unwrap_or(crate::config::defaults::DEFAULT_WHITE_LEVEL),
            "post_symmetry_type" => {
                post_symmetry.ty = crate::scene::transforms::PostSymmetryType::from_xml_token(value);
            }
            "post_symmetry_order" => {
                post_symmetry.order = value.parse::<u32>().unwrap_or(3);
            }
            "post_symmetry_centre_x" | "post_symmetry_center_x" => {
                post_symmetry.center_x = value.parse().unwrap_or(0.0);
            }
            "post_symmetry_centre_y" | "post_symmetry_center_y" => {
                post_symmetry.center_y = value.parse().unwrap_or(0.0);
            }
            "post_symmetry_distance" => {
                post_symmetry.distance = value.parse().unwrap_or(1.25);
            }
            "post_symmetry_rotation" => {
                post_symmetry.rotation_deg = value.parse().unwrap_or(6.0);
            }
            "preserve_z" => {
                // JWildfire encodes as `"1"` / `"0"`. Treat anything
                // non-zero as true; everything else (including the
                // empty string) as false.
                preserve_z = !value.trim().is_empty() && value.trim() != "0";
            }
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
            name if name.starts_with("sld_render_") => {
                sld_attrs.insert(name.to_string(), value.to_string());
            }
            _ => {} // Ignore unknown attributes for now
        }
    }

    // Parse child elements (xform, finalxform, and palette)
    let mut xform_results = Vec::new();
    // JWildfire allows multiple <finalxform> elements per flame and they
    // are chained at plot time. Apophysis 7X also supports this. Collect
    // all of them rather than overwriting; if `migrate_legacy_final`
    // pushes them in order, the chain ends up in the same order as the
    // source XML.
    let mut final_transforms_with_index: Vec<(Transform, Option<usize>)> = Vec::new();
    let mut palette = None;
    let mut buf = Vec::new();

    // The loop terminates on the End tag matching this element (`flame`
    // or `jwf-flame`).
    let element_name = start_element.name().as_ref().to_vec();

    // Layered `<jwf-flame>` support: xforms/palette live inside `<layer>`
    // children. We accept only the FIRST layer's content and skip the
    // rest. `<flame>` (Apophysis) has no layers, so `skip_layer` stays
    // false and every xform is a direct child as before.
    let mut layers_seen = 0usize;
    let mut skip_layer = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"layer" => {
                        // First layer = the flame; later layers ignored.
                        skip_layer = layers_seen > 0;
                        layers_seen += 1;
                    }
                    // Skip content of non-first layers (and `<bgxform>` is
                    // never matched here, so it's ignored automatically).
                    b"xform" if !skip_layer => {
                        let result = parse_xform_element(&e)?;
                        xform_results.push(result);
                    }
                    b"finalxform" if !skip_layer => {
                        let (transform, color_index) = parse_finalxform_element(&e)?;
                        final_transforms_with_index.push((transform, color_index));
                    }
                    b"palette" if !skip_layer => {
                        palette = Some(parse_palette_element(reader, &e)?);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let n = e.name();
                if n.as_ref() == element_name.as_slice() {
                    break;
                }
                if n.as_ref() == b"layer" {
                    // Back at the flame level; a following layer's Start
                    // will re-evaluate `skip_layer`.
                    skip_layer = false;
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

    // Extract transforms, color indices, chaos weights, and any
    // hex-decoded subflames from the parse results. Subflames are
    // pushed onto a parallel `imported_subflames` Vec and the owning
    // transform's `subflame_wf.subflame_id` param is updated to point
    // at the assigned index. We hand the vec to the `Flame` below.
    let mut transforms = Vec::new();
    let mut all_chaos_weights: Vec<Option<Vec<f32>>> = Vec::new();
    let mut imported_subflames: Vec<Flame> = Vec::new();

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
        if let Some(child) = result.imported_subflame {
            let assigned_id = imported_subflames.len() as f32;
            imported_subflames.push(child);
            transform.set_variation_param("subflame_wf", "subflame_id", assigned_id);
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

    // Process final transforms — JWF chains multiple finals at plot
    // time so we need to keep them all. Each gets its color coordinate
    // promoted to the palette position the same way normal xforms do.
    let final_transforms_processed: Vec<Transform> = final_transforms_with_index
        .into_iter()
        .map(|(mut final_xform, color_index)| {
            if let Some(idx) = color_index {
                if palette.is_some() {
                    let color_coord = idx as f32 / 255.0;
                    final_xform.color = color_coord;
                }
            }
            final_xform
        })
        .collect();

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

    // Apophysis XML originally carried a singular global Final; JWildfire
    // extended the format with multiple <finalxform> elements that chain
    // at plot time. `Flame::migrate_legacy_final` pushes each into the
    // `final_transforms` pool and auto-attaches them to every normal so
    // the rest of the pipeline sees a consistent shape.
    // See `docs/projects/per-transform-linked-and-final.md`.
    let mut flame = Flame {
        id: crate::scene::transforms::next_id(),
        name,
        transforms,
        linked_transforms: Vec::new(),
        final_transforms: Vec::new(),
        xaos,
        solo_transform: solo_xform,
        // JWF `subflame_wf_flame` payloads decoded above; empty when no
        // xform carried one (the common Apo case).
        subflames: imported_subflames,
        post_symmetry,
    };
    for final_xform in final_transforms_processed {
        flame.migrate_legacy_final(Some(final_xform));
    }

    // Convert Apophysis scale/center to our zoom/pan
    // Apophysis: scale = pixels per unit, where scale 200 ≈ zoom 1.0
    // Our system: zoom and pan (pan is world offset)
    // Apo: zoom = scale / 200. JWF adds a `cam_zoom` multiplier on
    // top, defaulting to 1.0 when absent.
    let zoom = (scale / 200.0) * cam_zoom_factor;
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

    let solid_import = solid_shading_from_sld(&sld_attrs);

    Ok(FractalConfig {
        // .flame XML carries no escape-time state (no Apo/JWF
        // equivalent) — deliberate, see the plan's serialization notes.
        escape: Default::default(),
        // Apophysis/JWildfire XML has no simulation concept, so an
        // imported .flame always carries the defaults.
        sim: Default::default(),
        flame,
        // Scene-level render state (config-level since v3).
        render_mode,
        perspective_strength,
        preserve_z,
        // JWildfire's `sld_render_*` block maps onto our splat-native
        // solid model where a correspondence exists (lights, material,
        // AO intensity, shadows); everything else keeps our defaults.
        solid_strength: solid_import.as_ref().map_or(crate::config::DEFAULT_SOLID_STRENGTH, |(ss, _)| *ss),
        surface_thickness: crate::config::DEFAULT_SURFACE_THICKNESS,
        solid_shading: solid_import.map_or_else(crate::config::SolidShadingSettings::default, |(_, sh)| sh),
        // Our own extensions; no .flame XML attribute (see FractalConfig docs).
        depth_density_compensation: 0.0,
        far_density_fade: 0.0,
        far_density_fade_start: 0.0,
        zoom,
        pan_x,
        pan_y,
        rotation,
        camera_rotation_x: cam_pitch,
        camera_rotation_y: cam_yaw,
        camera_bank: cam_bank,
        camera_x: cam_pos_x,
        camera_y: cam_pos_y,
        // Prefer `cam_pos_z` (modern JWildfire) over `cam_zpos`
        // (legacy single-axis form). If both are present, JWildfire
        // itself uses `cam_pos_z`; if only `cam_zpos` appears (older
        // saves), fall back to that.
        camera_z: cam_pos_z_opt.unwrap_or(cam_zpos),
        image_size: size,
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
        white_level,
        highlight_mode: crate::scene::tonemap::HighlightMode::Clip,  // Apophysis-compatible
        saturation,
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
    /// JWildfire's `subflame_wf_flame` attribute — hex-encoded XML
    /// bytes of the embedded child flame. When present (and the xform
    /// has the `subflame_wf` variation), we decode it as a sub-`<flame>`
    /// element and push the resulting `Flame` onto the parent's
    /// `subflames` pool; the xform's `subflame_wf.subflame_id` param
    /// gets pointed at the assigned index. Empty/missing on Apo files.
    imported_subflame: Option<Flame>,
}

/// Parse a 6-float space-separated value (as used by JWildfire's
/// `yzCoefs`, `zxCoefs`, `yzPost`, `zxPost` attributes) into the
/// provided buffer. The buffer is passed pre-initialized to the
/// identity so a malformed or short value leaves the corresponding
/// plane at identity (effectively a no-op).
fn parse_plane_coefs(value: &str, out: &mut [f32; 6]) {
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.len() >= 6 {
        for i in 0..6 {
            if let Ok(v) = parts[i].parse::<f32>() {
                out[i] = v;
            }
        }
    }
}

fn parse_xform_element(
    element: &quick_xml::events::BytesStart,
) -> Result<XformParseResult> {
    let mut transform = Transform::new();
    let registry = global_registry();
    let mut color_index = None;
    let mut chaos_weights = None;
    // JWF subflame_wf payload — hex-encoded child flame XML. Captured
    // during attribute iteration, decoded + parsed after the loop so
    // we can confirm the xform actually has `subflame_wf` active before
    // doing the recursive parse work.
    let mut subflame_wf_hex: Option<String> = None;
    let mut imported_subflame: Option<Flame> = None;

    // Storage for variation parameters (applied after all attributes parsed)
    let mut pending_params: Vec<(String, String, f32)> = Vec::new(); // (var_name, param_name, value)
    // fx_priority overrides, applied after all variations are known.
    let mut pending_priorities: Vec<(String, i32)> = Vec::new();

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
            // JWildfire extension: per-transform plane affines for the
            // YZ and ZX planes (and post-affine variants). Stored
            // positionally as 6 floats `a c b d e f` matching the JWF
            // XML write order — same indexing convention as
            // TransformationAffineFullStep reads them. Identity
            // (no-op) is `1 0 0 1 0 0`; flames that don't write the
            // attribute leave the plane at identity.
            "yzCoefs" => parse_plane_coefs(value, &mut transform.yz_coefs),
            "zxCoefs" => parse_plane_coefs(value, &mut transform.zx_coefs),
            "yzPost"  => parse_plane_coefs(value, &mut transform.yz_post_coefs),
            "zxPost"  => parse_plane_coefs(value, &mut transform.zx_post_coefs),
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
            // JWF hex-encoded child flame. Capture the raw string; we
            // hex-decode and recursively parse after the loop, only if
            // the xform turned out to have `subflame_wf` active. We
            // also intercept the companion attrs that would otherwise
            // try (and fail) to deserialize as variation params via the
            // generic `_` fallback.
            "subflame_wf_flame" => {
                subflame_wf_hex = Some(value.to_string());
            }
            "subflame_wf_flame_filename" | "subflame_wf_flame_is_sequence"
            | "subflame_wf_flame_sequence_start" | "subflame_wf_flame_sequence_end"
            | "subflame_wf_flame_sequence_repeat" | "subflame_wf_flame_sequence_digits" => {
                // JWF book-keeping for subflame sequences — not modeled.
            }
            _ => {
                // JWildfire per-variation phase override `<var>_fx_priority`.
                // Intercept before the variation-param path so it isn't
                // mistaken for a parameter named "fx_priority".
                if let Some(pair) = parse_fx_priority(key, value, &registry) {
                    pending_priorities.push(pair);
                    continue;
                }
                // Try to parse as variation or variation parameter
                if let Ok(parsed_value) = value.parse::<f32>() {
                    // First, try direct variation lookup (e.g., "julian", "spherical")
                    if let Some(info) = registry.get(key) {
                        if parsed_value != 0.0 {
                            // Store under the CANONICAL name — `key` may be a
                            // foreign alias (e.g. JWF `linear3D` → our
                            // `linear`). The rest of the pipeline (per-flame
                            // local index map, GPU weight upload, UI) only
                            // knows canonical names; an alias key passes
                            // `registry.get()` here but is silently dropped
                            // at shader build. Sum rather than overwrite so a
                            // flame carrying both `linear` and `linear3D`
                            // keeps both contributions (JWF treats them as
                            // separate variations whose outputs add).
                            // Record import order (XML attribute order) the
                            // first time we see this variation, so the
                            // dispatch matches JWildfire's per-xform order.
                            if !transform.variations.contains_key(&info.name) {
                                transform.variation_order.push(info.name.clone());
                            }
                            *transform
                                .variations
                                .entry(info.name.clone())
                                .or_insert(0.0) += parsed_value;
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
        let value = variation_param_from_xml(&var_name, &param_name, value);
        transform.set_variation_param(&var_name, &param_name, value);
    }
    // Apply fx_priority overrides (sparse; needs the full variation set).
    apply_fx_priorities(&mut transform, pending_priorities, &registry);

    // Recurse into the JWF subflame_wf_flame payload, if any. We
    // require `subflame_wf` to be active on this xform so we don't
    // accidentally surface a stray attribute as a child flame.
    if let Some(hex) = subflame_wf_hex {
        if transform.variations.contains_key("subflame_wf") {
            match decode_hex_bytes(&hex) {
                Ok(bytes) => match std::str::from_utf8(&bytes) {
                    Ok(child_xml) => match parse_flame_xml(child_xml) {
                        Ok(mut child_configs) if !child_configs.is_empty() => {
                            imported_subflame = Some(child_configs.remove(0).flame);
                        }
                        Ok(_) => log::warn!("subflame_wf_flame decoded to XML with no <flame> elements"),
                        Err(e) => log::warn!("subflame_wf_flame inner parse failed: {}", e),
                    },
                    Err(e) => log::warn!("subflame_wf_flame hex decoded to non-UTF8: {}", e),
                },
                Err(e) => log::warn!("subflame_wf_flame hex decode failed: {}", e),
            }
        } else {
            log::debug!("ignoring subflame_wf_flame attr: subflame_wf variation not active on this xform");
        }
    }

    Ok(XformParseResult {
        transform,
        color_index,
        chaos_weights,
        imported_subflame,
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
    // fx_priority overrides, applied after all variations are known.
    let mut pending_priorities: Vec<(String, i32)> = Vec::new();

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
            // JWildfire plane affines — same as the normal xform parser.
            // See the xform branch for the convention details.
            "yzCoefs" => parse_plane_coefs(value, &mut transform.yz_coefs),
            "zxCoefs" => parse_plane_coefs(value, &mut transform.zx_coefs),
            "yzPost"  => parse_plane_coefs(value, &mut transform.yz_post_coefs),
            "zxPost"  => parse_plane_coefs(value, &mut transform.zx_post_coefs),
            _ => {
                // JWildfire per-variation phase override `<var>_fx_priority`.
                if let Some(pair) = parse_fx_priority(key, value, &registry) {
                    pending_priorities.push(pair);
                    continue;
                }
                // Try to parse as variation or variation parameter
                if let Ok(parsed_value) = value.parse::<f32>() {
                    // First, try direct variation lookup
                    if let Some(info) = registry.get(key) {
                        if parsed_value != 0.0 {
                            // Canonical name + summed merge — same
                            // alias-canonicalization as the xform parser
                            // above (JWF `linear3D` → our `linear`).
                            // Record import order (see the xform parser).
                            if !transform.variations.contains_key(&info.name) {
                                transform.variation_order.push(info.name.clone());
                            }
                            *transform
                                .variations
                                .entry(info.name.clone())
                                .or_insert(0.0) += parsed_value;
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
        let value = variation_param_from_xml(&var_name, &param_name, value);
        transform.set_variation_param(&var_name, &param_name, value);
    }
    // Apply fx_priority overrides (sparse; needs the full variation set).
    apply_fx_priorities(&mut transform, pending_priorities, &registry);

    Ok((transform, color_index))
}

/// Parse a JWildfire `<var>_fx_priority` attribute. Returns
/// `Some((canonical_name, priority))` when `key` is an fx_priority
/// attribute for a known variation and `value` parses as a number, else
/// `None`. The `i32` is the raw JWF priority; the caller stores it
/// sparsely (only when it actually overrides the variation's natural
/// phase) after all variations on the xform are known.
fn parse_fx_priority(
    key: &str,
    value: &str,
    registry: &crate::variations::VariationRegistry,
) -> Option<(String, i32)> {
    let var_key = key.strip_suffix("_fx_priority")?;
    let info = registry.get(var_key)?;
    // JWF writes integers ("-1", "0", "2"); tolerate a stray ".0".
    let prio = value.parse::<f32>().ok()?.round() as i32;
    Some((info.name.clone(), prio))
}

/// Store collected fx_priority overrides on `transform`, keeping only the
/// ones that (a) belong to a variation actually active on this xform and
/// (b) differ from that variation def's natural-phase priority — the
/// sparse-override model (see `Transform::variation_priorities`).
fn apply_fx_priorities(
    transform: &mut Transform,
    pending: Vec<(String, i32)>,
    registry: &crate::variations::VariationRegistry,
) {
    for (name, prio) in pending {
        if !transform.variations.contains_key(&name) {
            continue; // priority for an inactive variation — ignore
        }
        let natural = registry
            .get(&name)
            .map(|i| i.phase.natural_priority())
            .unwrap_or(0);
        if prio != natural {
            transform.variation_priorities.insert(name, prio);
        }
    }
}

/// Per-parameter unit conversion: .flame XML representation → our
/// internal value. flam3/Apo/JWF sometimes store parameters in units
/// that differ from our UI semantics; converting at the boundary keeps
/// on-disk .flame values meaning the same thing in every app.
///
/// Currently only radial_blur's angle: flam3/JWF store half-turn units
/// (`spin = sin(a·π/2)`, 1.0 = pure spin, JWF default 0.5 = balanced);
/// our parameter is degrees with `spin = sin(deg·π/360)`, so
/// `degrees = xml_value × 180`.
fn variation_param_from_xml(var_name: &str, param_name: &str, value: f32) -> f32 {
    match (var_name, param_name) {
        ("radial_blur", "angle") => value * 180.0,
        // The *plot_wf variations plot a baked formula chosen by
        // `preset_id`. JWF's `preset_id = -1` means "use the custom
        // `formula` string" — which we can't evaluate, so importing it
        // verbatim renders the transform at the origin (a dead flame).
        // Clamp -1 → 0 on import: the flame still renders, and preset 0
        // is JWF's own default, so there's a chance it matches.
        (
            "yplot2d_wf" | "yplot3d_wf" | "parplot2d_wf" | "polarplot2d_wf"
            | "polarplot3d_wf",
            "preset_id",
        ) if value < 0.0 => 0.0,
        _ => value,
    }
}

/// Inverse of [`variation_param_from_xml`] for export.
fn variation_param_to_xml(var_name: &str, param_name: &str, value: f32) -> f32 {
    match (var_name, param_name) {
        ("radial_blur", "angle") => value / 180.0,
        _ => value,
    }
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
                // Return the CANONICAL variation name — `potential_var`
                // may be a foreign alias, and `set_variation_param`
                // keys must match the canonical `variations` entry.
                return Some((var_info.name.clone(), param_name));
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
/// Map JWildfire's `sld_render_*` attribute block onto our solid
/// rendering + lighting model. Returns None unless
/// `sld_render_enabled="1"`.
///
/// Correspondences (from a JWildfire 8.50 solid flame; unknown
/// semantics deliberately NOT guessed):
/// - material 0 ambient/diffuse/phong/phong_size → our
///   ambient/diffuse/specular/shininess (single global material; the
///   per-xform `material` index has no home here).
/// - `sld_render_ao_intensity` → ssao_strength when AO is enabled.
///   JWF's AO radius/samples/falloff describe a different sampler —
///   our screen-space radius keeps its default.
/// - lights: `altitude`/`azimuth` are degrees → our
///   elevation/azimuth; intensity + rgb map directly. JWF has no
///   per-light enabled flag — existence up to the count is enabled.
///   We hold 4 light slots; extra JWF lights are dropped with a warn.
/// - shadows: any light with `shadows=1` (and shadow_type != OFF)
///   turns our shadow maps on; strength = max of those lights'
///   `shadow_intensity` (ours is global, JWF's per-light).
/// - JWF solid is a hard surface: solid_strength = 1,
///   shading_strength = 1.
///
/// Note the JWildfire-side attribute typos, preserved faithfully:
/// `sld_render_ligtht_count` (light count) — we accept the corrected
/// spelling too in case a future JWF fixes it.
fn solid_shading_from_sld(
    attrs: &std::collections::HashMap<String, String>,
) -> Option<(f32, crate::config::SolidShadingSettings)> {
    if attrs.get("sld_render_enabled").map(String::as_str) != Some("1") {
        return None;
    }
    let getf = |k: &str| attrs.get(k).and_then(|v| v.parse::<f32>().ok());

    let mut sh = crate::config::SolidShadingSettings::default();
    sh.shading_strength = 1.0;
    if let Some(v) = getf("sld_render_material_ambient0") {
        sh.ambient = v.clamp(0.0, 1.0);
    }
    if let Some(v) = getf("sld_render_material_diffuse0") {
        sh.diffuse = v.clamp(0.0, 2.0);
    }
    if let Some(v) = getf("sld_render_material_phong0") {
        sh.specular = v.clamp(0.0, 2.0);
    }
    if let Some(v) = getf("sld_render_material_phong_size0") {
        sh.shininess = v.clamp(1.0, 128.0);
    }
    let ao_enabled = attrs.get("sld_render_ao_enabled").map(String::as_str) == Some("1");
    sh.ssao_strength = if ao_enabled {
        getf("sld_render_ao_intensity").unwrap_or(0.6).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // JWF typo: `ligtht`. Accept the corrected spelling too.
    let count = attrs
        .get("sld_render_ligtht_count")
        .or_else(|| attrs.get("sld_render_light_count"))
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    if count > 4 {
        log::warn!("JWF solid import: {} lights, keeping the first 4", count);
    }
    let shadow_off = attrs.get("sld_render_shadow_type").map(String::as_str) == Some("OFF");
    let mut shadow_strength = 0.0f32;
    for i in 0..4 {
        let light = &mut sh.lights[i];
        if i >= count {
            light.enabled = false;
            continue;
        }
        light.enabled = true;
        if let Some(v) = getf(&format!("sld_render_light_azimuth{i}")) {
            light.azimuth = v;
        }
        if let Some(v) = getf(&format!("sld_render_light_altitude{i}")) {
            light.elevation = v.clamp(-90.0, 90.0);
        }
        if let Some(v) = getf(&format!("sld_render_light_intensity{i}")) {
            light.intensity = v.max(0.0);
        }
        light.color = [
            getf(&format!("sld_render_light_red{i}")).unwrap_or(1.0).clamp(0.0, 1.0),
            getf(&format!("sld_render_light_green{i}")).unwrap_or(1.0).clamp(0.0, 1.0),
            getf(&format!("sld_render_light_blue{i}")).unwrap_or(1.0).clamp(0.0, 1.0),
        ];
        let casts = attrs
            .get(&format!("sld_render_light_shadows{i}"))
            .map(String::as_str)
            == Some("1");
        if casts && !shadow_off {
            let intensity = getf(&format!("sld_render_light_shadow_intensity{i}")).unwrap_or(0.8);
            shadow_strength = shadow_strength.max(intensity.clamp(0.0, 1.0));
        }
    }
    sh.shadow_strength = shadow_strength;

    Some((1.0, sh))
}

/// Write our solid rendering + lighting as JWildfire's `sld_render_*`
/// block (inverse of `solid_shading_from_sld`). Only enabled lights
/// are exported (JWF has no per-light enabled flag — only a count), so
/// they renumber to 0..n. The material's unmapped attributes get JWF's
/// own defaults so JWF 8.50 parses the block exactly as it writes it —
/// including its `ligtht`/`mappping`/`diif` attribute-name typos.
fn write_solid_rendering(out: &mut String, config: &FractalConfig) {
    let active = matches!(config.render_mode, crate::scene::transforms::RenderMode::ThreeD)
        && (config.solid_strength > 0.0 || config.solid_shading.active());
    if !active {
        return;
    }
    let sh = &config.solid_shading;
    out.push_str(" sld_render_enabled=\"1\"");
    out.push_str(&format!(
        " sld_render_ao_enabled=\"{}\"",
        u8::from(sh.ssao_strength > 0.0)
    ));
    if sh.ssao_strength > 0.0 {
        out.push_str(&format!(" sld_render_ao_intensity=\"{}\"", fmt_f32(sh.ssao_strength)));
    }
    let shadows_on = sh.shadow_strength > 0.0;
    out.push_str(&format!(
        " sld_render_shadow_type=\"{}\"",
        if shadows_on { "FAST" } else { "OFF" }
    ));
    if shadows_on {
        // Our splat-resolution maps are 1024² per light.
        out.push_str(" sld_render_shadowmap_size=\"1024\"");
    }
    out.push_str(" sld_render_material_count=\"1\"");
    out.push_str(&format!(" sld_render_material_ambient0=\"{}\"", fmt_f32(sh.ambient)));
    out.push_str(&format!(" sld_render_material_diffuse0=\"{}\"", fmt_f32(sh.diffuse)));
    out.push_str(&format!(" sld_render_material_phong0=\"{}\"", fmt_f32(sh.specular)));
    out.push_str(&format!(" sld_render_material_phong_size0=\"{}\"", fmt_f32(sh.shininess)));
    // JWF defaults for the material attrs we don't model (specular
    // color, reflection map, diffuse response) — written so JWF's
    // parser sees a complete material.
    out.push_str(" sld_render_material_phong_red0=\"1.0\"");
    out.push_str(" sld_render_material_phong_green0=\"1.0\"");
    out.push_str(" sld_render_material_phong_blue0=\"1.0\"");
    out.push_str(" sld_render_material_refl_map_intensity0=\"0.5\"");
    out.push_str(" sld_render_material_refl_map_filename0=\"\"");
    out.push_str(" sld_render_material_refl_mappping0=\"BLINN_NEWELL\"");
    out.push_str(" sld_render_material_light_diif_func0=\"COSA_SQUARE\"");

    let enabled: Vec<&crate::config::SolidLight> =
        sh.lights.iter().filter(|l| l.enabled && l.intensity > 0.0).collect();
    out.push_str(&format!(" sld_render_ligtht_count=\"{}\"", enabled.len()));
    for (i, l) in enabled.iter().enumerate() {
        out.push_str(&format!(" sld_render_light_altitude{i}=\"{}\"", fmt_f32(l.elevation)));
        out.push_str(&format!(" sld_render_light_azimuth{i}=\"{}\"", fmt_f32(l.azimuth)));
        out.push_str(&format!(" sld_render_light_intensity{i}=\"{}\"", fmt_f32(l.intensity)));
        out.push_str(&format!(" sld_render_light_red{i}=\"{}\"", fmt_f32(l.color[0])));
        out.push_str(&format!(" sld_render_light_green{i}=\"{}\"", fmt_f32(l.color[1])));
        out.push_str(&format!(" sld_render_light_blue{i}=\"{}\"", fmt_f32(l.color[2])));
        out.push_str(&format!(
            " sld_render_light_shadows{i}=\"{}\"",
            u8::from(shadows_on)
        ));
        out.push_str(&format!(
            " sld_render_light_shadow_intensity{i}=\"{}\"",
            fmt_f32(sh.shadow_strength)
        ));
    }
}

pub fn write_flame_xml(config: &FractalConfig) -> String {
    let mut out = String::with_capacity(8192);
    out.push_str("<flames name=\"");
    out.push_str(&xml_escape_attr(&config.flame.name));
    out.push_str("\">\n");
    write_single_flame(&mut out, config, &config.flame);
    out.push_str("</flames>\n");
    out
}

/// Emit one `<flame>...</flame>` element. Reused by both the top-level
/// writer (wrapped in `<flames>`) and the subflame embed path (the
/// returned string gets hex-encoded into a `subflame_wf_flame` attr).
///
/// `config` supplies the shared tonemap/palette/camera/etc. settings.
/// `flame` is the IFS structure being emitted — usually
/// `config.flame`, but for subflames it's the child flame from
/// `config.flame.subflames[id]`. Children inherit the parent's
/// tonemap/palette since our model has no per-subflame FractalConfig.
fn write_single_flame(out: &mut String, config: &FractalConfig, flame: &Flame) {
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

    // Canvas dimensions live on the FractalConfig now (round-tripped
    // through both XML import and JSON serde). Pre-feature flames
    // default to (1920, 1080) — same value we used to write
    // unconditionally — so on-disk output is unchanged for them.
    let size = config.image_size;

    let bg = config.background_color;
    let bg_str = format!(
        "{} {} {}",
        (bg[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (bg[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (bg[2].clamp(0.0, 1.0) * 255.0).round() as u8,
    );

    // `plugins` — Apo wants the union of variation names across all
    // xforms (and the finalxform if any). Sorted for determinism.
    let plugins = collect_plugin_names(flame);

    out.push_str("<flame name=\"");
    out.push_str(&xml_escape_attr(&flame.name));
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
    // JWildfire's rename quirk: bank is serialized under the on-disk
    // attribute name `cam_roll`. See the import-side comment for the
    // full explanation. This export side mirrors JWildfire's
    // `createAttr("cam_roll", getCamBank() * π/180)` — we already
    // store the value in radians so there's no unit conversion.
    if config.camera_bank.abs() > 1e-6 {
        out.push_str(&format!(" cam_roll=\"{}\"", fmt_f32(config.camera_bank)));
    }
    // Camera world-space position. Emit JWildfire's modern
    // `cam_pos_x/y/z` triple when any axis is non-zero. Also emit the
    // legacy `cam_zpos` (Z only) for backward compat — older JWildfire
    // and Apo versions only understand `cam_zpos`, and ignoring the
    // attributes they don't know is a no-op for them. On re-import,
    // `cam_pos_z` takes priority over `cam_zpos` so the round-trip
    // value is the one we wrote.
    if config.camera_x.abs() > 1e-6 {
        out.push_str(&format!(" cam_pos_x=\"{}\"", fmt_f32(config.camera_x)));
    }
    if config.camera_y.abs() > 1e-6 {
        out.push_str(&format!(" cam_pos_y=\"{}\"", fmt_f32(config.camera_y)));
    }
    if config.camera_z.abs() > 1e-6 {
        out.push_str(&format!(" cam_pos_z=\"{}\"", fmt_f32(config.camera_z)));
        out.push_str(&format!(" cam_zpos=\"{}\"", fmt_f32(config.camera_z)));
    }
    if config.perspective_strength.abs() > 1e-6 {
        // Apo reads `cam_perspective`; JWF reads `cam_persp`. Emit both
        // so re-import lands the perspective in either editor.
        out.push_str(&format!(" cam_perspective=\"{}\"", fmt_f32(config.perspective_strength)));
        out.push_str(&format!(" cam_persp=\"{}\"", fmt_f32(config.perspective_strength)));
    }
    if config.dof_blur_strength.abs() > 1e-6 {
        out.push_str(&format!(" cam_dof=\"{}\"", fmt_f32(config.dof_blur_strength)));
    }
    // JWildfire solid rendering + lighting (sld_render_* block) —
    // top-level flame only (subflame <flame> elements are IFS payloads;
    // carrying scene-level solid attrs there would set solid mode on
    // the child if opened standalone in JWF).
    if std::ptr::eq(flame, &config.flame) {
        write_solid_rendering(out, config);
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
    // JWF-side knobs. Apo silently ignores both. Always emit so JWF
    // sees the right white point and saturation on re-import.
    out.push_str(&format!(" saturation=\"{}\"", fmt_f32(config.saturation)));
    out.push_str(&format!(" white_level=\"{}\"", fmt_f32(config.white_level)));
    out.push_str(&format!(" gamma_threshold=\"{}\"", fmt_f32(apo_gamma_threshold)));

    // Post-symmetry — JWF reads, Apo ignores. Always emit so a JWF
    // round-trip preserves the symmetry settings; the defaults match
    // JWF's per-file fallbacks (NONE / 3 / 0 / 0 / 1.25 / 6.0).
    let ps = &flame.post_symmetry;
    out.push_str(&format!(" post_symmetry_type=\"{}\"", ps.ty.xml_token()));
    out.push_str(&format!(" post_symmetry_order=\"{}\"", ps.order));
    out.push_str(&format!(" post_symmetry_centre_x=\"{}\"", fmt_f32(ps.center_x)));
    out.push_str(&format!(" post_symmetry_centre_y=\"{}\"", fmt_f32(ps.center_y)));
    out.push_str(&format!(" post_symmetry_distance=\"{}\"", fmt_f32(ps.distance)));
    out.push_str(&format!(" post_symmetry_rotation=\"{}\"", fmt_f32(ps.rotation_deg)));

    // JWildfire convention: `preserve_z` is omitted when default
    // (false) and emitted as `"1"` when set. Apo ignores the attr.
    if config.preserve_z {
        out.push_str(" preserve_z=\"1\"");
    }
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
    if let Some(idx) = flame.solo_transform {
        out.push_str(&format!(" soloxform=\"{}\"", idx));
    }
    out.push_str(" >\n");

    // Normal transforms (`<xform>`).
    let registry = global_registry();
    let xaos = flame.xaos.as_ref();
    for (i, xform) in flame.transforms.iter().enumerate() {
        let chaos_row = xaos.and_then(|m| m.get(i));
        write_xform(out, xform, false, chaos_row, &*registry, config, &flame.subflames);
    }

    // Final transforms. JWildfire (and Apophysis 7X with the JWF
    // extension) allow multiple `<finalxform>` elements chained at
    // plot time. The importer fix in a8bf010 collected all of them
    // into `final_transforms: Vec<Transform>`; mirror that on export
    // by writing each in source order. Round-trips through JWF
    // cleanly.
    for final_xform in &flame.final_transforms {
        write_xform(out, final_xform, true, None, &*registry, config, &flame.subflames);
    }

    // Palette: 256 colors, RGB hex, 8 colors per line.
    write_palette(out, &config.palette);

    out.push_str("</flame>\n");
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

/// Uppercase hex encoding of arbitrary bytes — two hex chars per byte,
/// no whitespace. Matches JWildfire's `subflame_wf_flame` format
/// exactly (see `tests/test_configs/JWF-rando13.flame` for an example payload).
fn hex_encode_uppercase(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0F) as usize] as char);
    }
    out
}

/// Decode a hex string (case-insensitive, whitespace tolerated). Used
/// for JWF's `subflame_wf_flame` payload. Errors on odd length or
/// non-hex characters — both indicate the writer didn't follow the
/// JWF convention.
fn decode_hex_bytes(s: &str) -> anyhow::Result<Vec<u8>> {
    let mut digits: Vec<u8> = Vec::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_whitespace() {
            continue;
        }
        let v = match c {
            '0'..='9' => c as u8 - b'0',
            'a'..='f' => c as u8 - b'a' + 10,
            'A'..='F' => c as u8 - b'A' + 10,
            _ => anyhow::bail!("non-hex char in payload: {:?}", c),
        };
        digits.push(v);
    }
    if digits.len() % 2 != 0 {
        anyhow::bail!("odd number of hex digits ({}) — expected pairs", digits.len());
    }
    Ok(digits.chunks_exact(2).map(|c| (c[0] << 4) | c[1]).collect())
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
    parent_config: &FractalConfig,
    subflame_pool: &[Flame],
) {
    out.push_str(if is_final { "   <finalxform" } else { "   <xform" });

    if !is_final {
        out.push_str(&format!(" weight=\"{}\"", fmt_f32(xform.weight)));
    }
    out.push_str(&format!(" color=\"{}\"", fmt_f32(xform.color)));
    if xform.color_speed.abs() > 1e-6 {
        // Apophysis and JWildfire both name this attribute `symmetry`
        // in the on-disk XML even though their internal field for it
        // is `colorSpeed` / `color_speed`. Importers above accept
        // either spelling; we write `symmetry` so flames opened in
        // Apo/JWF show the value in the right field rather than
        // appearing as an unknown attribute.
        out.push_str(&format!(" symmetry=\"{}\"", fmt_f32(xform.color_speed)));
    }
    if !is_final {
        // Apo always writes opacity; we follow suit so a re-import sees
        // the same value rather than the parser default.
        out.push_str(&format!(" opacity=\"{}\"", fmt_f32(xform.opacity)));
    }
    if xform.direct_color.abs() > 1e-6 {
        out.push_str(&format!(" pluginColor=\"{}\"", fmt_f32(xform.direct_color)));
    }

    // Variations — emit in the transform's recorded order
    // (`variation_order`) so the dispatch order survives a `.flame`
    // round-trip (JWildfire applies variations in xform-list order); then
    // any active variations not in the hint, sorted, for determinism.
    let mut emitted_vars: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for name in &xform.variation_order {
        if let Some(weight) = xform.variations.get(name) {
            out.push_str(&format!(" {}=\"{}\"", name, fmt_f32(*weight)));
            emitted_vars.insert(name.as_str());
        }
    }
    let mut variation_names: Vec<&String> = xform
        .variations
        .keys()
        .filter(|n| !emitted_vars.contains(n.as_str()))
        .collect();
    variation_names.sort();
    for name in &variation_names {
        let weight = xform.variations[*name];
        out.push_str(&format!(" {}=\"{}\"", name, fmt_f32(weight)));
    }

    // Per-variation phase overrides (JWildfire `<var>_fx_priority`).
    // Sparse: `variation_priorities` only holds genuine overrides, and
    // we emit only those whose variation is active on this xform. JWF
    // writes the integer priority; Apophysis ignores unknown attributes.
    let mut priority_names: Vec<&String> = xform.variation_priorities.keys().collect();
    priority_names.sort();
    for name in &priority_names {
        if !xform.variations.contains_key(*name) {
            continue; // override for a removed variation — don't emit
        }
        out.push_str(&format!(" {}_fx_priority=\"{}\"", name, xform.variation_priorities[*name]));
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

    // JWildfire-extension plane affines. Written conditionally — only
    // when the plane isn't identity — matching JWildfire's own
    // `if (xForm.isHasYZCoeffs()) { ... }` conditional XML output.
    // Apophysis readers ignore unknown attributes, so this is also
    // safe for Apo round-trip. Layout matches JWF's XML write block
    // in `XForm.java`: six positional floats per attribute.
    let write_plane = |out: &mut String, name: &str, coefs: &[f32; 6]| {
        out.push_str(&format!(
            " {}=\"{} {} {} {} {} {}\"",
            name,
            fmt_f32(coefs[0]), fmt_f32(coefs[1]),
            fmt_f32(coefs[2]), fmt_f32(coefs[3]),
            fmt_f32(coefs[4]), fmt_f32(coefs[5]),
        ));
    };
    if !xform.is_yz_identity() {
        write_plane(out, "yzCoefs", &xform.yz_coefs);
    }
    if !xform.is_zx_identity() {
        write_plane(out, "zxCoefs", &xform.zx_coefs);
    }
    if !xform.is_yz_post_identity() {
        write_plane(out, "yzPost", &xform.yz_post_coefs);
    }
    if !xform.is_zx_post_identity() {
        write_plane(out, "zxPost", &xform.zx_post_coefs);
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
        let xml_value = variation_param_to_xml(var_name, param_name, value);
        out.push_str(&format!(" {}_{}=\"{}\"", var_name, param_name, fmt_f32(xml_value)));
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

    // JWF subflame_wf payload. When this xform has `subflame_wf`
    // active, recurse: write the referenced subflame as a single
    // <flame> element, hex-encode the bytes, and emit as the
    // `subflame_wf_flame` attribute. JWF also requires a fixed set of
    // companion attrs (sequence flags, filename); we write the empty
    // defaults — these are the values our import path tolerates.
    if xform.variations.contains_key("subflame_wf") {
        let id = xform.get_variation_param("subflame_wf", "subflame_id")
            .unwrap_or(0.0) as usize;
        if let Some(child_flame) = subflame_pool.get(id) {
            let mut child_xml = String::with_capacity(4096);
            write_single_flame(&mut child_xml, parent_config, child_flame);
            let hex = hex_encode_uppercase(child_xml.as_bytes());
            out.push_str(&format!(" subflame_wf_flame=\"{}\"", hex));
            out.push_str(" subflame_wf_flame_filename=\"\"");
            out.push_str(" subflame_wf_flame_is_sequence=\"0\"");
        } else {
            log::warn!(
                "write_xform: subflame_wf references subflame_id={} but parent has only {} subflame(s)",
                id, subflame_pool.len()
            );
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

// Every test here parses a .flame naming real variations, so the
// whole module is about the flame catalog.
#[cfg(feature = "engine-flame")]
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
    fn test_variation_order_captured_from_xml() {
        // Variation order must follow XML attribute order so the dispatch
        // matches JWildfire's per-xform order (matters for NeedsAccum /
        // Replace variations). roundspher3D-then-waves2 vs the reverse must
        // produce opposite `variation_order`.
        let mk = |attrs: &str| {
            let xml = format!(
                r#"<flames name="t"><flame name="O" size="100 100" center="0 0" scale="50">
                   <xform weight="1" color="0.5" {} coefs="1 0 0 1 0 0"/></flame></flames>"#,
                attrs
            );
            parse_flame_xml(&xml).expect("parse").remove(0).flame.transforms[0].variation_order.clone()
        };
        assert_eq!(
            mk(r#"roundspher3D="0.5" waves2="0.9""#),
            vec!["roundspher3D".to_string(), "waves2".to_string()],
        );
        assert_eq!(
            mk(r#"waves2="0.9" roundspher3D="0.5""#),
            vec!["waves2".to_string(), "roundspher3D".to_string()],
        );
    }

    #[test]
    fn test_jwf_flame_layered_first_layer_only() {
        // Newer JWildfire saves a layered `<jwf-flame>`: flame attributes on
        // the root, xforms/palette inside `<layer>` children, plus a
        // `<bgxform>`. We parse the FIRST layer as the flame and ignore the
        // rest. Here the first layer has linear+spherical; the second layer
        // (julian) and the bgxform must be ignored.
        let xml = r#"
<jwf-flame name="Layered" size="800 600" center="0 0" scale="200" brightness="4">
  <layer weight="1.0" visible="true">
    <xform weight="1" color="0.2" linear="1" coefs="1 0 0 1 0 0"/>
    <xform weight="0.5" color="0.7" spherical="1" coefs="1 0 0 1 0 0"/>
  </layer>
  <layer weight="0.5" visible="true">
    <xform weight="1" color="0.9" julian="1" julian_power="3" coefs="1 0 0 1 0 0"/>
  </layer>
  <bgxform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0"/>
</jwf-flame>
        "#;

        let config = &parse_flame_xml(xml).expect("parse jwf-flame")[0];
        assert_eq!(config.flame.name, "Layered");
        // Only the first layer's two xforms.
        assert_eq!(config.flame.transforms.len(), 2);
        let names: std::collections::BTreeSet<_> = config.flame.transforms.iter()
            .flat_map(|t| t.variations.keys().cloned())
            .collect();
        assert!(names.contains("linear") && names.contains("spherical"));
        assert!(!names.contains("julian"), "second layer must be ignored");
        // Flame attributes came off the <jwf-flame> root.
        assert_eq!(config.image_size, (800, 600));
    }

    #[test]
    fn test_fx_priority_import_sparse_override() {
        // combimirror is an `Any` variation (natural priority 0); a stored
        // fx_priority="-1" is a genuine override and must be kept.
        // pre_blur is natively Pre (natural -1); fx_priority="-1" equals its
        // natural phase, so it is NOT stored (sparse model). The other
        // variations at priority 0 store nothing.
        let xml = r#"
<flames name="test">
<flame name="FX Priority Test" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0.5" linear="1" combimirror="0.5" combimirror_fx_priority="-1"
          pre_blur="2" pre_blur_fx_priority="-1" bubble="0.3" bubble_fx_priority="0"
          coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;

        let config = &parse_flame_xml(xml).expect("parse")[0];
        let xform = &config.flame.transforms[0];

        // Only the genuine override survives.
        assert_eq!(xform.variation_priorities.get("combimirror"), Some(&-1));
        assert_eq!(xform.variation_priorities.get("pre_blur"), None,
            "pre_blur fx_priority equals its natural phase — not an override");
        assert_eq!(xform.variation_priorities.get("bubble"), None,
            "priority 0 on a normal-default var is not an override");
        assert_eq!(xform.variation_priorities.len(), 1);
    }

    #[test]
    fn test_fx_priority_roundtrip() {
        // An override survives export → re-import unchanged.
        let xml = r#"
<flames name="test">
<flame name="RT" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0.5" combimirror="0.5" combimirror_fx_priority="-1"
          coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let config = parse_flame_xml(xml).expect("parse").remove(0);
        let exported = write_flame_xml(&config);
        assert!(exported.contains("combimirror_fx_priority=\"-1\""),
            "export must write the override; got:\n{}", exported);

        let reimported = &parse_flame_xml(&exported).expect("re-parse")[0];
        assert_eq!(
            reimported.flame.transforms[0].variation_priorities.get("combimirror"),
            Some(&-1)
        );
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

    /// JWF's `*plot_wf` variations use `preset_id = -1` to mean "custom
    /// formula string" — which we can't evaluate, so it plots at the
    /// origin (dead flame). Import clamps -1 → 0 (JWF's default preset)
    /// so the flame renders; a valid preset_id passes through unchanged.
    #[test]
    fn test_plot_wf_preset_id_clamped_on_import() {
        let xml = r#"
<flames name="test">
<flame name="Plot Test" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0" yplot2d_wf="0.5" yplot2d_wf_preset_id="-1" coefs="1 0 0 1 0 0" opacity="1" />
   <xform weight="1" color="0" polarplot3d_wf="0.5" polarplot3d_wf_preset_id="4" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;

        let config = &parse_flame_xml(xml).expect("parse")[0];

        // preset_id = -1 (custom formula) clamped to 0.
        let x0 = &config.flame.transforms[0];
        assert_eq!(
            x0.get_variation_param_or_default("yplot2d_wf", "preset_id", &global_registry()),
            0.0
        );
        // A real preset survives untouched.
        let x1 = &config.flame.transforms[1];
        assert_eq!(
            x1.get_variation_param_or_default("polarplot3d_wf", "preset_id", &global_registry()),
            4.0
        );
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
    fn test_multiple_finalxform_import() {
        // JWildfire (and Apophysis 7X with the JWF extension) allow
        // multiple <finalxform> elements per flame, chained at plot time.
        // Pre-fix, the importer kept only the last one. Verify all of
        // them now land in `final_transforms` in source order.
        let xml = r#"
<flames name="test">
<flame name="Multi-Final Test" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
   <finalxform weight="0" color="0.1" spherical="1" coefs="1 0 0 1 0 0" />
   <finalxform weight="0" color="0.5" linear="1" coefs="2 0 0 2 0 0" />
   <finalxform weight="0" color="0.9" bubble="1" coefs="1 0 0 1 3 4" />
</flame>
</flames>
        "#;
        let configs = parse_flame_xml(xml).expect("parse must succeed");
        let flame = &configs[0].flame;
        assert_eq!(flame.final_transforms.len(), 3,
            "expected three finalxforms, got {}", flame.final_transforms.len());
        // Order preserved — each final's variation + affine round-trip.
        // (Color isn't checked because the existing palette-conditional
        // promotion path only writes the color coordinate when a
        // `<palette>` is present in the XML, and this fixture omits one.
        // That's pre-existing behavior, not a multi-final regression.)
        let f0 = &flame.final_transforms[0];
        assert!(f0.active_variations().iter().any(|n| n == "spherical"));
        let f1 = &flame.final_transforms[1];
        assert!(f1.active_variations().iter().any(|n| n == "linear"));
        assert_eq!(f1.a, 2.0);
        assert_eq!(f1.d, 2.0);
        let f2 = &flame.final_transforms[2];
        assert!(f2.active_variations().iter().any(|n| n == "bubble"));
        // coefs="1 0 0 1 3 4" → a,c,b,d,e,f order (Apophysis column-major),
        // so the translation lands in (e, f), not (c, f).
        assert_eq!(f2.e, 3.0);
        assert_eq!(f2.f, 4.0);
        // migrate_legacy_final should have auto-attached every final to
        // every normal so the plot-time chain sees all of them.
        let normal = &flame.transforms[0];
        assert_eq!(normal.final_attachments, vec![0, 1, 2],
            "expected normal xform to attach to all 3 finals");
    }

    #[test]
    fn test_multiple_finalxform_roundtrip() {
        // Companion to test_multiple_finalxform_import. The exporter
        // previously wrote only the first final and logged a warning
        // about the rest — round-trip through XML lost them silently.
        // Verify all of them survive write_flame_xml → parse_flame_xml.
        let xml = r#"
<flames name="test">
<flame name="Multi-Final Roundtrip" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
   <finalxform weight="0" color="0.1" spherical="1" coefs="1 0 0 1 0 0" />
   <finalxform weight="0" color="0.5" linear="1" coefs="2 0 0 2 0 0" />
   <finalxform weight="0" color="0.9" bubble="1" coefs="1 0 0 1 3 4" />
</flame>
</flames>
        "#;
        let configs = parse_flame_xml(xml).expect("parse must succeed");
        let exported = write_flame_xml(&configs[0]);
        // Substring check is enough — count of <finalxform tags must be 3.
        let final_count = exported.matches("<finalxform").count();
        assert_eq!(final_count, 3,
            "expected 3 <finalxform> elements in export, got {}\n--- output ---\n{}",
            final_count, exported);
        // Round-trip back through parser to confirm each survives.
        let reimported = parse_flame_xml(&exported).expect("re-parse must succeed");
        let flame = &reimported[0].flame;
        assert_eq!(flame.final_transforms.len(), 3,
            "expected 3 finalxforms after round-trip, got {}", flame.final_transforms.len());
        assert!(flame.final_transforms[0].active_variations().iter().any(|n| n == "spherical"));
        assert!(flame.final_transforms[1].active_variations().iter().any(|n| n == "linear"));
        assert!(flame.final_transforms[2].active_variations().iter().any(|n| n == "bubble"));
        assert_eq!(flame.final_transforms[1].a, 2.0);
        assert_eq!(flame.final_transforms[2].e, 3.0);
        assert_eq!(flame.final_transforms[2].f, 4.0);
    }

    #[test]
    fn test_jwf_plane_affines_roundtrip() {
        // JWildfire-extension per-xform plane affines (yzCoefs, zxCoefs,
        // yzPost, zxPost). Verify: (a) import populates the arrays in
        // positional order, (b) identity values are skipped on export
        // (matching JWF's `if (xForm.isHasYZCoeffs())` conditional),
        // (c) non-identity values round-trip through import → export
        // → re-import preserved.
        let xml = r#"
<flames name="test">
<flame name="3D Affine Test" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0" linear="1"
          coefs="1 0 0 1 0 0"
          yzCoefs="0.9 0.1 -0.1 0.9 0.5 0.7"
          zxCoefs="0.8 0.2 -0.2 0.8 -0.3 0.4"
          opacity="1" />
   <xform weight="1" color="0.5" spherical="1"
          coefs="1 0 0 1 0 0"
          yzPost="1.5 0 0 1.5 0 0"
          zxPost="1.2 0.3 -0.3 1.2 0.1 -0.1"
          opacity="1" />
   <xform weight="1" color="1" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;

        // Import: arrays populate in positional order.
        let configs = parse_flame_xml(xml).expect("parse must succeed");
        let flame = &configs[0].flame;
        assert_eq!(flame.transforms.len(), 3);

        // Xform 0: yzCoefs + zxCoefs (pre-affine only).
        let x0 = &flame.transforms[0];
        assert_eq!(x0.yz_coefs, [0.9, 0.1, -0.1, 0.9, 0.5, 0.7]);
        assert_eq!(x0.zx_coefs, [0.8, 0.2, -0.2, 0.8, -0.3, 0.4]);
        assert!(x0.is_yz_post_identity(), "x0 has no yzPost");
        assert!(x0.is_zx_post_identity(), "x0 has no zxPost");
        assert!(!x0.is_yz_identity());
        assert!(!x0.is_zx_identity());

        // Xform 1: yzPost + zxPost (post-affine only).
        let x1 = &flame.transforms[1];
        assert!(x1.is_yz_identity());
        assert!(x1.is_zx_identity());
        assert_eq!(x1.yz_post_coefs, [1.5, 0.0, 0.0, 1.5, 0.0, 0.0]);
        assert_eq!(x1.zx_post_coefs, [1.2, 0.3, -0.3, 1.2, 0.1, -0.1]);

        // Xform 2: no plane attrs at all → all identity.
        let x2 = &flame.transforms[2];
        assert!(x2.is_yz_identity());
        assert!(x2.is_zx_identity());
        assert!(x2.is_yz_post_identity());
        assert!(x2.is_zx_post_identity());

        // Export → re-import. Identity-plane attributes must be skipped
        // (Apophysis-style flame stays clean of the extension noise);
        // non-identity attributes must survive byte-for-byte.
        let exported = write_flame_xml(&configs[0]);
        // Xform 2 (all identity) must not have any of the four attrs.
        // Pull the third <xform line — color may be formatted in various
        // ways, so index by position rather than by content.
        let xform_lines: Vec<&str> = exported.lines()
            .filter(|line| line.trim_start().starts_with("<xform"))
            .collect();
        assert_eq!(xform_lines.len(), 3, "three xform lines in export");
        let x2_block = xform_lines[2];
        assert!(!x2_block.contains("yzCoefs="), "identity yz_coefs must not be written: {}", x2_block);
        assert!(!x2_block.contains("zxCoefs="), "identity zx_coefs must not be written: {}", x2_block);
        assert!(!x2_block.contains("yzPost="), "identity yz_post must not be written: {}", x2_block);
        assert!(!x2_block.contains("zxPost="), "identity zx_post must not be written: {}", x2_block);
        // Xform 0 must have yzCoefs and zxCoefs but not yzPost/zxPost.
        assert!(exported.contains("yzCoefs=\"0.9 0.1 -0.1 0.9 0.5 0.7\""), "yzCoefs preserved: {}", exported);
        assert!(exported.contains("zxCoefs=\"0.8 0.2 -0.2 0.8 -0.3 0.4\""), "zxCoefs preserved: {}", exported);

        // Re-import and re-check the round-tripped values land exactly.
        let reimport = parse_flame_xml(&exported).expect("re-parse must succeed");
        let r0 = &reimport[0].flame.transforms[0];
        assert_eq!(r0.yz_coefs, [0.9, 0.1, -0.1, 0.9, 0.5, 0.7]);
        assert_eq!(r0.zx_coefs, [0.8, 0.2, -0.2, 0.8, -0.3, 0.4]);
        let r1 = &reimport[0].flame.transforms[1];
        assert_eq!(r1.yz_post_coefs, [1.5, 0.0, 0.0, 1.5, 0.0, 0.0]);
        assert_eq!(r1.zx_post_coefs, [1.2, 0.3, -0.3, 1.2, 0.1, -0.1]);

        // Gating regression (JWF-rando4-rotated): xform 1 carries
        // yzPost/zxPost but NO `post=` attribute. JWF gates the three
        // post planes independently, so the post step must still run —
        // post_affine_enabled stays false (no XY post) but
        // has_post_step() and the flame-level shader gate must be true.
        // This used to be gated on post_affine_enabled alone, which
        // silently dropped plane-only posts (seen as a missing ~44°
        // Y-axis rotation).
        assert!(!x1.post_affine_enabled, "no post= attr → XY post stays disabled");
        assert!(x1.has_post_step(), "plane-only post must still run the post step");
        assert!(!x0.has_post_step(), "pre-only planes don't enable the post step");
        assert!(flame.has_post_affine(), "flame-level post gate must see plane-only posts");
    }

    #[test]
    fn test_color_speed_exports_as_symmetry() {
        // Apophysis and JWildfire both name the per-transform color
        // speed/symmetry value `symmetry` in the on-disk XML. Our
        // importer accepts both `color_speed` and `symmetry`; the
        // exporter must write `symmetry` so the round-trip stays
        // compatible with Apo/JWF (they show it as an unknown
        // attribute under `color_speed`).
        let xml = r#"
<flames name="test">
<flame name="ColorSpeed" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" symmetry="0.42" opacity="1" />
</flame>
</flames>
        "#;
        let configs = parse_flame_xml(xml).expect("parse must succeed");
        assert!((configs[0].flame.transforms[0].color_speed - 0.42).abs() < 1e-4);

        let exported = write_flame_xml(&configs[0]);
        assert!(exported.contains(r#"symmetry="0.42""#), "must write `symmetry=`, got: {}", exported);
        assert!(!exported.contains("color_speed="), "must NOT write `color_speed=`, got: {}", exported);

        // Round-trip the value through re-parse to make sure the
        // exported form parses back to the same internal value.
        let reimport = parse_flame_xml(&exported).expect("re-parse must succeed");
        assert!((reimport[0].flame.transforms[0].color_speed - 0.42).abs() < 1e-4);
    }

    #[test]
    fn test_image_size_roundtrip() {
        // The `size` XML attribute now lands on `FractalConfig::image_size`
        // instead of being discarded. Verify (a) import populates the
        // field, (b) export writes the stored value (not the historic
        // fixed `1920 1080`), and (c) flames without the attribute
        // get the (1920, 1080) default.
        let with_size = r#"
<flames name="t">
<flame name="Custom" size="1024 768" center="0 0" scale="200">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let configs = parse_flame_xml(with_size).expect("parse must succeed");
        assert_eq!(configs[0].image_size, (1024, 768));

        // Export writes the stored value, not 1920 1080.
        let exported = write_flame_xml(&configs[0]);
        assert!(exported.contains(r#"size="1024 768""#), "must write `size=\"1024 768\"`, got: {}", exported);

        // Round-trip preserves it.
        let reimport = parse_flame_xml(&exported).expect("re-parse must succeed");
        assert_eq!(reimport[0].image_size, (1024, 768));

        // A flame without the size attribute defaults to (1920, 1080).
        let no_size = r#"
<flames name="t">
<flame name="Default" center="0 0" scale="200">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let default_configs = parse_flame_xml(no_size).expect("parse must succeed");
        assert_eq!(default_configs[0].image_size, (1920, 1080));
    }

    #[test]
    fn test_cam_roll_maps_to_camera_bank() {
        // JWildfire's rename quirk: the XML attribute `cam_roll`
        // carries the *bank* parameter, not the roll. Verify (a)
        // import lands `cam_roll` on `camera_bank` and leaves
        // `rotation` (which would be JWF's roll, via `rotate`)
        // untouched, (b) export writes `camera_bank` as `cam_roll`,
        // (c) round-trip preserves the value byte-for-byte, (d)
        // identity bank is skipped on export so existing flames
        // stay clean.
        let xml = r#"
<flames name="t">
<flame name="WithBank" size="800 600" center="0 0" scale="200" cam_roll="0.3927" cam_pitch="0.5" cam_yaw="0.2">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let configs = parse_flame_xml(xml).expect("parse must succeed");
        let cfg = &configs[0];

        // cam_roll → camera_bank (in radians, as-stored).
        assert!((cfg.camera_bank - 0.3927).abs() < 1e-4, "camera_bank should be 0.3927, got {}", cfg.camera_bank);
        // Sanity: pitch and yaw land where expected.
        assert!((cfg.camera_rotation_x - 0.5).abs() < 1e-4);
        assert!((cfg.camera_rotation_y - 0.2).abs() < 1e-4);
        // The XML had no `rotate` attribute, so `rotation` stays at
        // the import default. The cam_roll value must NOT have
        // landed there.
        assert!(cfg.rotation.abs() < 1e-4, "rotation should be 0, got {}", cfg.rotation);

        // Export writes the bank as `cam_roll` (not as `rotate`),
        // mirroring JWildfire's serializer.
        let exported = write_flame_xml(cfg);
        assert!(exported.contains("cam_roll=\"0.3927\""), "must write `cam_roll=`, got: {}", exported);

        // Round-trip preserves it.
        let reimport = parse_flame_xml(&exported).expect("re-parse must succeed");
        assert!((reimport[0].camera_bank - 0.3927).abs() < 1e-4);

        // A flame without `cam_roll` defaults camera_bank to 0 and
        // the export skips writing the attribute (keeps Apo files
        // clean).
        let no_bank = r#"
<flames name="t">
<flame name="NoBank" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let no_bank_cfg = &parse_flame_xml(no_bank).expect("parse must succeed")[0];
        assert_eq!(no_bank_cfg.camera_bank, 0.0);
        let no_bank_xml = write_flame_xml(no_bank_cfg);
        assert!(!no_bank_xml.contains("cam_roll="), "identity bank must not be written: {}", no_bank_xml);
    }

    #[test]
    fn test_variation_alias_canonicalization() {
        // Aliased variation attributes must be stored under the
        // CANONICAL name: the per-flame local index map is built from
        // canonical registry names only, so an alias key passes the
        // import lookup but is silently dropped at shader build —
        // observed originally as renders matching
        // JWF-minus-the-variation. `cylinder_apo` aliases `cylinder`.
        let xml = r#"
<flames name="t">
<flame name="Alias" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0" cylinder_apo="0.8" spherical="0.2" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>"#;
        let cfg = &parse_flame_xml(xml).expect("parse must succeed")[0];
        let vars = &cfg.flame.transforms[0].variations;
        assert!(
            !vars.contains_key("cylinder_apo"),
            "alias key must not be stored: {vars:?}"
        );
        let w = vars.get("cylinder").copied().expect("canonical key present");
        assert!((w - 0.8).abs() < 1e-6, "cylinder weight = {w}");

        // The canonicalized weight must actually reach the shader's
        // local index map (the original failure point).
        let id_map = cfg.flame.get_id_mapping();
        assert!(
            id_map.contains_key("cylinder"),
            "cylinder missing from shader id map: {id_map:?}"
        );

        // Alias + canonical on the same xform sum their weights (the
        // foreign app treats them as separate variations whose
        // outputs add; in our merged-def model that's a summed
        // weight).
        let xml_both = r#"
<flames name="t">
<flame name="Both" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0" cylinder="0.25" cylinder_apo="0.5" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>"#;
        let cfg = &parse_flame_xml(xml_both).expect("parse must succeed")[0];
        let w = cfg.flame.transforms[0]
            .variations
            .get("cylinder")
            .copied()
            .expect("merged canonical key present");
        assert!((w - 0.75).abs() < 1e-6, "summed cylinder weight = {w}");
    }

    #[test]
    fn test_linear3d_imports_as_own_variation() {
        // `linear3D` used to be an alias of `linear`; it is now its
        // own def because JWF gives them different z semantics
        // (linear3D writes z unconditionally — Feature::AlwaysZ —
        // while linear gates z on preserve_z). Both must import as
        // separate entries and both must reach the shader id map.
        let xml = r#"
<flames name="t">
<flame name="L3D" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0" linear="0.25" linear3D="0.5" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>"#;
        let cfg = &parse_flame_xml(xml).expect("parse must succeed")[0];
        let vars = &cfg.flame.transforms[0].variations;
        let lin = vars.get("linear").copied().expect("linear present");
        let lin3d = vars.get("linear3D").copied().expect("linear3D present");
        assert!((lin - 0.25).abs() < 1e-6, "linear weight = {lin}");
        assert!((lin3d - 0.5).abs() < 1e-6, "linear3D weight = {lin3d}");

        let id_map = cfg.flame.get_id_mapping();
        assert!(id_map.contains_key("linear"), "linear in id map");
        assert!(id_map.contains_key("linear3D"), "linear3D in id map");
    }

    #[test]
    fn test_radial_blur_angle_unit_conversion() {
        // flam3/JWF store radial_blur's angle in half-turn units
        // (0.5 = balanced, 1.0 = pure spin); ours is degrees with the
        // same effective formula (spin = sin(deg·π/360)), so the XML
        // boundary converts ×180 on import, ÷180 on export.
        let xml = r#"
<flames name="t">
<flame name="RB" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0" radial_blur="0.7" radial_blur_angle="0.5" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>"#;
        let cfg = &parse_flame_xml(xml).expect("parse must succeed")[0];
        let xform = &cfg.flame.transforms[0];
        let angle = xform
            .variation_params
            .get("radial_blur.angle")
            .copied()
            .expect("angle param imported");
        assert!(
            (angle - 90.0).abs() < 1e-3,
            "JWF 0.5 half-turns should import as 90 degrees, got {angle}"
        );

        // Export converts back to half-turn units.
        let exported = write_flame_xml(cfg);
        assert!(
            exported.contains("radial_blur_angle=\"0.5\""),
            "expected half-turn units on export: {exported}"
        );
    }

    #[test]
    fn test_camera_position_roundtrip() {
        // JWildfire's modern `cam_pos_x/y/z` triple now round-trips
        // through `FractalConfig::camera_x/y/z`. Verify:
        //   (a) import populates all three when present
        //   (b) `cam_pos_z` takes priority over the legacy `cam_zpos`
        //       (Apo / older JWildfire single-axis form)
        //   (c) when only `cam_zpos` is present, it falls back into z
        //   (d) export writes the `cam_pos_*` triple AND keeps emitting
        //       `cam_zpos` for legacy backward compat
        //   (e) identity-zero values are skipped on export so existing
        //       flames stay clean
        let xml_all = r#"
<flames name="t">
<flame name="WithPos" size="800 600" center="0 0" scale="200" cam_pos_x="1.5" cam_pos_y="-2.25" cam_pos_z="3.75" cam_zpos="999">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let cfg = &parse_flame_xml(xml_all).expect("parse must succeed")[0];

        // (a) all three populated
        assert!((cfg.camera_x - 1.5).abs() < 1e-4, "camera_x = {}", cfg.camera_x);
        assert!((cfg.camera_y - (-2.25)).abs() < 1e-4, "camera_y = {}", cfg.camera_y);
        // (b) `cam_pos_z` (3.75) wins over `cam_zpos` (999) when both present.
        assert!((cfg.camera_z - 3.75).abs() < 1e-4, "camera_z should be 3.75 (cam_pos_z), got {}", cfg.camera_z);

        // (d) export writes the modern triple AND legacy cam_zpos
        let exported = write_flame_xml(cfg);
        assert!(exported.contains("cam_pos_x=\"1.5\""), "missing cam_pos_x: {}", exported);
        assert!(exported.contains("cam_pos_y=\"-2.25\""), "missing cam_pos_y: {}", exported);
        assert!(exported.contains("cam_pos_z=\"3.75\""), "missing cam_pos_z: {}", exported);
        assert!(exported.contains("cam_zpos=\"3.75\""), "missing cam_zpos legacy fallback: {}", exported);

        // Round-trip preserves the values
        let reimport = &parse_flame_xml(&exported).expect("re-parse must succeed")[0];
        assert!((reimport.camera_x - 1.5).abs() < 1e-4);
        assert!((reimport.camera_y - (-2.25)).abs() < 1e-4);
        assert!((reimport.camera_z - 3.75).abs() < 1e-4);

        // (c) legacy `cam_zpos` only — falls back into z
        let xml_legacy = r#"
<flames name="t">
<flame name="Legacy" size="800 600" center="0 0" scale="200" cam_zpos="2.5">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let legacy = &parse_flame_xml(xml_legacy).expect("parse must succeed")[0];
        assert_eq!(legacy.camera_x, 0.0);
        assert_eq!(legacy.camera_y, 0.0);
        assert!((legacy.camera_z - 2.5).abs() < 1e-4, "cam_zpos should fall back to z: {}", legacy.camera_z);

        // (e) identity-zero export stays clean — no cam_pos_x/y/z written
        let xml_default = r#"
<flames name="t">
<flame name="Default" size="800 600" center="0 0" scale="200">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let default_cfg = &parse_flame_xml(xml_default).expect("parse must succeed")[0];
        let default_xml = write_flame_xml(default_cfg);
        assert!(!default_xml.contains("cam_pos_x="), "default x must not be written: {}", default_xml);
        assert!(!default_xml.contains("cam_pos_y="), "default y must not be written: {}", default_xml);
        assert!(!default_xml.contains("cam_pos_z="), "default z must not be written: {}", default_xml);
        assert!(!default_xml.contains("cam_zpos="), "default zpos must not be written: {}", default_xml);
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

    /// JWildfire `sld_render_*` block (attribute names verbatim from a
    /// JWF 8.50 solid flame, including the `ligtht` typo): import maps
    /// onto solid_strength + SolidShadingSettings; export round-trips.
    #[test]
    fn jwf_solid_rendering_roundtrip() {
        let xml = r#"<flame name="Solid" size="800 600" center="0 0" scale="200" cam_pitch="0.5" sld_render_enabled="1" sld_render_ao_enabled="1" sld_render_ao_intensity="0.6" sld_render_shadow_type="FAST" sld_render_shadowmap_size="4096" sld_render_material_count="1" sld_render_material_diffuse0="0.42" sld_render_material_ambient0="0.71" sld_render_material_phong0="0.75" sld_render_material_phong_size0="4.5" sld_render_ligtht_count="2" sld_render_light_altitude0="39.7" sld_render_light_azimuth0="3.44" sld_render_light_intensity0="0.8" sld_render_light_red0="1.0" sld_render_light_green0="0.9" sld_render_light_blue0="0.8" sld_render_light_shadows0="1" sld_render_light_shadow_intensity0="0.8" sld_render_light_altitude1="64.0" sld_render_light_azimuth1="55.0" sld_render_light_intensity1="0.6" sld_render_light_red1="1.0" sld_render_light_green1="1.0" sld_render_light_blue1="1.0" sld_render_light_shadows1="0" sld_render_light_shadow_intensity1="0.7">
  <xform weight="0.5" color="0" coefs="1 0 0 1 0 0"/>
  <palette count="256" format="RGB">000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF</palette>
</flame>"#;
        let configs = parse_flame_xml(xml).expect("parse");
        let c = &configs[0];
        assert_eq!(c.solid_strength, 1.0);
        let sh = &c.solid_shading;
        assert_eq!(sh.shading_strength, 1.0);
        assert!((sh.ambient - 0.71).abs() < 1e-5);
        assert!((sh.diffuse - 0.42).abs() < 1e-5);
        assert!((sh.specular - 0.75).abs() < 1e-5);
        assert!((sh.shininess - 4.5).abs() < 1e-5);
        assert!((sh.ssao_strength - 0.6).abs() < 1e-5);
        // Light 0 casts shadows at 0.8; light 1 doesn't — global max.
        assert!((sh.shadow_strength - 0.8).abs() < 1e-5);
        assert!(sh.lights[0].enabled && sh.lights[1].enabled);
        assert!(!sh.lights[2].enabled && !sh.lights[3].enabled);
        assert!((sh.lights[0].elevation - 39.7).abs() < 1e-4);
        assert!((sh.lights[0].azimuth - 3.44).abs() < 1e-4);
        assert!((sh.lights[0].intensity - 0.8).abs() < 1e-5);
        assert!((sh.lights[0].color[1] - 0.9).abs() < 1e-5);
        assert!((sh.lights[1].elevation - 64.0).abs() < 1e-4);

        // Export → re-import round-trip.
        let exported = write_flame_xml(c);
        assert!(exported.contains("sld_render_enabled=\"1\""), "missing sld block: {}", exported);
        assert!(exported.contains("sld_render_ligtht_count=\"2\""), "JWF light-count typo must be preserved: {}", exported);
        let back = &parse_flame_xml(&exported).expect("reparse")[0];
        let bh = &back.solid_shading;
        assert_eq!(back.solid_strength, 1.0);
        assert!((bh.ambient - 0.71).abs() < 1e-4);
        assert!((bh.diffuse - 0.42).abs() < 1e-4);
        assert!((bh.specular - 0.75).abs() < 1e-4);
        assert!((bh.shininess - 4.5).abs() < 1e-4);
        assert!((bh.ssao_strength - 0.6).abs() < 1e-4);
        assert!((bh.shadow_strength - 0.8).abs() < 1e-4);
        assert!(bh.lights[0].enabled && bh.lights[1].enabled && !bh.lights[2].enabled);
        assert!((bh.lights[1].azimuth - 55.0).abs() < 1e-4);

        // Non-solid flames must not emit the block.
        let mut plain = c.clone();
        plain.solid_strength = 0.0;
        plain.solid_shading = crate::config::SolidShadingSettings::default();
        let plain_xml = write_flame_xml(&plain);
        assert!(!plain_xml.contains("sld_render"), "plain flame leaked sld block");
    }

    /// Real-world Apo file (exported from Apophysis 7X v15C.9 with 3
    /// xforms, julia3D, zscale, custom palette, cam_pitch, cam_dof) —
    /// should round-trip through our import/export without losing any
    /// field the parser reads.
    ///
    /// The fixture lives in `tests/test_configs/` beside the other
    /// `.flame` samples. It used to be `include_str!`'d from `output/`,
    /// which is **gitignored** — so on any machine but the one it was
    /// written on, the crate did not compile and NO test could run.
    #[test]
    fn test_roundtrip_real_apo_file() {
        let xml_in = include_str!("../tests/test_configs/ship-on-the-sea.flame");

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

    // -----------------------------------------------------------------
    // JWildfire compatibility tests
    // -----------------------------------------------------------------

    /// JWF aliases — `cam_persp` should populate perspective_strength
    /// the same way `cam_perspective` does, and `cam_zoom` should
    /// multiply into our `zoom` field on top of `scale`.
    #[test]
    fn test_jwf_camera_aliases() {
        let xml = r#"
<flames name="jwf-cam">
<flame name="JWFCam" size="800 600" center="0 0" scale="200" cam_persp="1.5" cam_zoom="2.0" background="0 0 0" brightness="4" gamma="4">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let cfg = parse_flame_xml(xml).expect("parse").into_iter().next().unwrap();

        // cam_persp landed in perspective_strength.
        assert!((cfg.perspective_strength - 1.5).abs() < 1e-5);
        // zoom = (scale / 200) × cam_zoom = (200/200) × 2.0 = 2.0
        assert!((cfg.zoom - 2.0).abs() < 1e-5, "zoom: {}", cfg.zoom);
    }

    /// JWF's `saturation` and `white_level` are written directly into
    /// the corresponding FractalConfig fields; we don't override with
    /// defaults the way the pre-JWF importer did.
    #[test]
    fn test_jwf_tonemap_fields() {
        let xml = r#"
<flames name="jwf-tm">
<flame name="JWFTm" size="800 600" center="0 0" scale="200" background="0 0 0" brightness="4" gamma="4" saturation="0.75" white_level="180.0">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let cfg = parse_flame_xml(xml).expect("parse").into_iter().next().unwrap();
        assert!((cfg.saturation - 0.75).abs() < 1e-5);
        assert!((cfg.white_level - 180.0).abs() < 1e-5);
    }

    /// Hex codec — round-trip arbitrary bytes through encode/decode
    /// without loss. Includes the JWF subflame_wf_flame leading
    /// signature `3C666C616D65` = ASCII `<flame` to verify the case
    /// and ordering match the format used by the editor.
    #[test]
    fn test_hex_codec_roundtrip() {
        // ASCII spot-check.
        let original = b"<flame name=\"x\" />";
        let encoded = hex_encode_uppercase(original);
        // Matches the JWF dump prefix.
        assert!(encoded.starts_with("3C666C616D65"), "encoded: {}", encoded);
        let decoded = decode_hex_bytes(&encoded).expect("decode");
        assert_eq!(decoded, original);

        // Full byte range round-trip.
        let bytes: Vec<u8> = (0..=255u8).collect();
        let encoded = hex_encode_uppercase(&bytes);
        let decoded = decode_hex_bytes(&encoded).expect("decode");
        assert_eq!(decoded, bytes);

        // Whitespace and lowercase are tolerated on decode.
        let mixed = "3c66 6c61\n6D65";
        let decoded = decode_hex_bytes(mixed).expect("decode mixed");
        assert_eq!(&decoded, b"<flame");

        // Odd length errors out cleanly.
        assert!(decode_hex_bytes("3C6").is_err());
        // Non-hex char errors out cleanly.
        assert!(decode_hex_bytes("3CZZ").is_err());
    }

    /// Full subflame round-trip: build a parent FractalConfig whose
    /// flame has a `subflame_wf` xform pointing at one child Flame,
    /// export to XML, re-import, assert the child survived.
    #[test]
    fn test_subflame_roundtrip() {
        use crate::scene::transforms::{Flame, Transform};

        let mut parent = FractalConfig::default();
        parent.flame.name = "ParentFlame".to_string();
        parent.flame.transforms.clear();

        // Xform 0: linear, ordinary.
        let mut x0 = Transform::new();
        x0.variations.insert("linear".to_string(), 1.0);
        x0.weight = 1.0;
        parent.flame.transforms.push(x0);

        // Xform 1: subflame_wf pointing at subflame index 0.
        let mut x1 = Transform::new();
        x1.variations.insert("subflame_wf".to_string(), 0.5);
        x1.set_variation_param("subflame_wf", "subflame_id", 0.0);
        x1.weight = 0.5;
        parent.flame.transforms.push(x1);

        // The child flame.
        let mut child = Flame::new();
        child.name = "ChildFlame".to_string();
        child.transforms.clear();
        let mut cx = Transform::new();
        cx.variations.insert("spherical".to_string(), 1.0);
        cx.weight = 1.0;
        cx.a = 0.5;
        cx.d = 0.5;
        child.transforms.push(cx);
        parent.flame.subflames.push(child);

        // Export and re-import.
        let xml = write_flame_xml(&parent);
        // Sanity: the XML contains the hex-encoded subflame payload.
        assert!(xml.contains("subflame_wf_flame=\""), "no subflame_wf_flame attr in: {}", xml);

        let back = parse_flame_xml(&xml).expect("re-parse")
            .into_iter().next().unwrap();

        // Parent structure preserved.
        assert_eq!(back.flame.transforms.len(), 2);
        assert!(back.flame.transforms[0].variations.contains_key("linear"));
        assert!(back.flame.transforms[1].variations.contains_key("subflame_wf"));

        // Child survived the hex round-trip.
        assert_eq!(back.flame.subflames.len(), 1, "subflame missing after re-parse");
        let child_back = &back.flame.subflames[0];
        assert_eq!(child_back.name, "ChildFlame");
        assert_eq!(child_back.transforms.len(), 1);
        assert!(child_back.transforms[0].variations.contains_key("spherical"));
        assert!((child_back.transforms[0].a - 0.5).abs() < 1e-5);
        assert!((child_back.transforms[0].d - 0.5).abs() < 1e-5);

        // Xform 1's subflame_id points at the new (index 0) subflame.
        let id = back.flame.transforms[1]
            .get_variation_param("subflame_wf", "subflame_id")
            .expect("subflame_id missing");
        assert_eq!(id as usize, 0);
    }

    /// Importing the real JWF-rando13 file — has a subflame_wf xform
    /// with a hex-encoded child flame. Verifies the importer extracts
    /// it cleanly without crashing on JWF's many extra attributes.
    #[test]
    fn test_import_jwf_subflame_file() {
        let xml = include_str!("../tests/test_configs/JWF-rando13.flame");
        let cfg = parse_flame_xml(xml).expect("parse JWF rando13")
            .into_iter().next().unwrap();

        // Top-level structure: at least one xform, and the subflame
        // pool got populated by the hex-encoded payload.
        assert!(!cfg.flame.transforms.is_empty(), "no xforms in parent");
        assert_eq!(
            cfg.flame.subflames.len(), 1,
            "expected exactly one decoded subflame from this file"
        );

        // The subflame_wf xform's subflame_id matches the assigned
        // index (0, since only one was imported).
        let subflame_wf_xform = cfg.flame.transforms.iter()
            .find(|x| x.variations.contains_key("subflame_wf"))
            .expect("no subflame_wf xform found");
        let id = subflame_wf_xform.get_variation_param("subflame_wf", "subflame_id")
            .expect("subflame_id missing");
        assert_eq!(id as usize, 0);

        // The child flame has its own xforms (rando13's subflame is
        // a single-xform truchet).
        let child = &cfg.flame.subflames[0];
        assert!(!child.transforms.is_empty(), "child subflame has no xforms");
    }

    /// Post-symmetry round-trip: import → export → re-import preserves
    /// type, order, center, distance, rotation. Also confirms the JWF
    /// `centre_*` spelling alias works on import.
    #[test]
    fn test_post_symmetry_roundtrip() {
        use crate::scene::transforms::PostSymmetryType;
        let xml = r#"
<flames name="ps-test">
<flame name="PS" size="800 600" center="0 0" scale="200" background="0 0 0" brightness="4" gamma="4" post_symmetry_type="POINT" post_symmetry_order="5" post_symmetry_centre_x="0.25" post_symmetry_centre_y="-0.5" post_symmetry_distance="2.0" post_symmetry_rotation="15.0">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;

        let original = parse_flame_xml(xml).expect("parse").into_iter().next().unwrap();
        assert_eq!(original.flame.post_symmetry.ty, PostSymmetryType::Point);
        assert_eq!(original.flame.post_symmetry.order, 5);
        assert!((original.flame.post_symmetry.center_x - 0.25).abs() < 1e-5);
        assert!((original.flame.post_symmetry.center_y - -0.5).abs() < 1e-5);
        assert!((original.flame.post_symmetry.distance - 2.0).abs() < 1e-5);
        assert!((original.flame.post_symmetry.rotation_deg - 15.0).abs() < 1e-4);

        let xml_out = write_flame_xml(&original);
        let back = parse_flame_xml(&xml_out).expect("re-parse").into_iter().next().unwrap();

        assert_eq!(back.flame.post_symmetry.ty, PostSymmetryType::Point);
        assert_eq!(back.flame.post_symmetry.order, 5);
        assert!((back.flame.post_symmetry.center_x - 0.25).abs() < 1e-5);
        assert!((back.flame.post_symmetry.center_y - -0.5).abs() < 1e-5);
        assert!((back.flame.post_symmetry.distance - 2.0).abs() < 1e-5);
        assert!((back.flame.post_symmetry.rotation_deg - 15.0).abs() < 1e-4);
    }

    /// X-axis and Y-axis symmetry tokens import cleanly. The default
    /// case (no `post_symmetry_*` attrs on the flame) leaves the field
    /// at `None`.
    #[test]
    fn test_post_symmetry_axis_modes_and_default() {
        use crate::scene::transforms::PostSymmetryType;

        // Default: no attrs → None.
        let xml_none = r#"
<flames name="none">
<flame name="None" size="800 600" center="0 0" scale="200" background="0 0 0" brightness="4" gamma="4">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let cfg = parse_flame_xml(xml_none).expect("parse").into_iter().next().unwrap();
        assert_eq!(cfg.flame.post_symmetry.ty, PostSymmetryType::None);

        for (token, expected) in [
            // Note the cross-mapping: JWildfire's `X_AXIS` flips X
            // (left/right mirror) which is our `YAxis` under the
            // standard math convention (axis-of-symmetry = line of
            // reflection). See `PostSymmetryType::xml_token` for why.
            ("X_AXIS", PostSymmetryType::YAxis),
            ("Y_AXIS", PostSymmetryType::XAxis),
        ] {
            let xml = format!(
                r#"<flames name="t"><flame name="T" size="800 600" center="0 0" scale="200" background="0 0 0" brightness="4" gamma="4" post_symmetry_type="{}" post_symmetry_distance="0.5" post_symmetry_rotation="30.0"><xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" /></flame></flames>"#,
                token,
            );
            let cfg = parse_flame_xml(&xml).expect("parse").into_iter().next().unwrap();
            assert_eq!(cfg.flame.post_symmetry.ty, expected, "token {}", token);
            assert!((cfg.flame.post_symmetry.distance - 0.5).abs() < 1e-5);
            assert!((cfg.flame.post_symmetry.rotation_deg - 30.0).abs() < 1e-4);
        }
    }

    /// `preserve_z` XML round-trip: importer reads `"1"` as true,
    /// missing attr stays at false (JWF default). Writer emits
    /// `preserve_z="1"` when true and omits it when false.
    #[test]
    fn test_preserve_z_roundtrip() {
        // Missing attr → false (JWF default).
        let xml_none = r#"
<flames name="pz">
<flame name="None" size="800 600" center="0 0" scale="200" background="0 0 0" brightness="4" gamma="4">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let cfg = parse_flame_xml(xml_none).expect("parse").into_iter().next().unwrap();
        assert!(!cfg.preserve_z, "missing attr should default to false");
        let xml_out = write_flame_xml(&cfg);
        assert!(!xml_out.contains("preserve_z"), "should not emit when false: {}", xml_out);

        // preserve_z="1" → true. Round-trip preserves the flag and
        // re-emits the attr on export.
        let xml_set = r#"
<flames name="pz">
<flame name="Preserved" size="800 600" center="0 0" scale="200" background="0 0 0" brightness="4" gamma="4" preserve_z="1">
   <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1" />
</flame>
</flames>
        "#;
        let cfg = parse_flame_xml(xml_set).expect("parse").into_iter().next().unwrap();
        assert!(cfg.preserve_z, "preserve_z=\"1\" should map to true");
        let xml_out = write_flame_xml(&cfg);
        assert!(xml_out.contains("preserve_z=\"1\""), "should re-emit when true: {}", xml_out);

        // Re-import after export — value survives.
        let cfg_back = parse_flame_xml(&xml_out).expect("re-parse").into_iter().next().unwrap();
        assert!(cfg_back.preserve_z);
    }

    /// Diagnostic: print the rando13 subflame's structure (xform count,
    /// variations, perspective, transform G values, render_mode-ish
    /// signals). Run with `-- --nocapture` to see output. Helps verify
    /// what we're actually importing when chasing through-parent
    /// behavior differences.
    #[test]
    #[ignore = "diagnostic-only; run with --ignored --nocapture when needed"]
    fn diag_dump_rando13_subflame() {
        let xml = include_str!("../tests/test_configs/JWF-rando13.flame");
        let cfg = parse_flame_xml(xml).unwrap().into_iter().next().unwrap();
        let child = &cfg.flame.subflames[0];
        println!("--- subflame ---");
        println!("name: {}", child.name);
        println!("transforms: {}", child.transforms.len());
        for (i, x) in child.transforms.iter().enumerate() {
            let vars: Vec<_> = x.variations.iter().collect();
            println!(
                "  xf{}: weight={} g={} direct_color={} vars={:?} params={:?}",
                i, x.weight, x.g, x.direct_color,
                vars,
                x.variation_params,
            );
        }
        println!("--- parent subflame_wf ---");
        let pxf = cfg.flame.transforms.iter()
            .find(|x| x.variations.contains_key("subflame_wf")).unwrap();
        println!(
            "  weight={} direct_color={} variations={:?} params={:?}",
            pxf.weight, pxf.direct_color,
            pxf.variations.iter().collect::<Vec<_>>(),
            pxf.variation_params,
        );
    }
}
