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

use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

use crate::config::FractalConfig;
use crate::scene::palette::{Palette, ColorMode};
use crate::scene::tonemap::{ToneMapMode, ToneCurve};
use crate::scene::transforms::{Flame, RenderMode, Transform, ProjectionType};
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
    let mut background = [0.0, 0.0, 0.0];
    let mut brightness = 1.0;
    let mut gamma = 2.2;

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
            "background" => {
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() == 3 {
                    background[0] = parts[0].parse::<f32>().unwrap_or(0.0) / 255.0;
                    background[1] = parts[1].parse::<f32>().unwrap_or(0.0) / 255.0;
                    background[2] = parts[2].parse::<f32>().unwrap_or(0.0) / 255.0;
                }
            }
            "brightness" => brightness = value.parse().unwrap_or(1.0),
            "gamma" => {
                // Apophysis gamma 1.0 ≈ our gamma 2.2 (sRGB standard)
                // Multiply by 2.2 to convert
                let apo_gamma: f32 = value.parse().unwrap_or(1.0);
                gamma = apo_gamma * 2.2;
            }
            _ => {} // Ignore unknown attributes for now
        }
    }

    // Parse child elements (xform and palette)
    let mut transforms_with_indices = Vec::new();
    let mut palette = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"xform" => {
                        let (transform, color_index) = parse_xform_element(reader, &e)?;
                        transforms_with_indices.push((transform, color_index));
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

    // Apply palette colors to transforms
    let mut transforms = Vec::new();
    for (mut transform, color_index) in transforms_with_indices {
        if let (Some(ref pal), Some(idx)) = (&palette, color_index) {
            if idx < pal.stops.len() {
                transform.color = pal.stops[idx].color;
                transform.color_speed = 0.5; // Default color speed
            }
        }
        transforms.push(transform);
    }

    // Build FractalConfig
    let flame = Flame {
        name,
        transforms,
        final_transform: None,
        render_mode: RenderMode::TwoD,
        projection: ProjectionType::Orthographic,
    };

    // Convert Apophysis scale/center to our zoom/pan
    // Apophysis: scale = pixels per unit, where scale 200 ≈ zoom 1.0
    // Our system: zoom and pan (pan is world offset)
    let zoom = scale / 200.0; // Apophysis scale 200.0 = our zoom 1.0
    let pan_x = center.0;
    let pan_y = center.1;

    Ok(FractalConfig {
        flame,
        zoom,
        pan_x,
        pan_y,
        rotation: 0.0,
        camera_rotation_x: 0.0,
        camera_rotation_y: 0.0,
        density_scale: brightness,
        speed_factor: 1.0,
        max_iterations: 1_000_000_000,
        color_mode: ColorMode::Transform,
        palette_index: 0,
        palette,
        background_color: background,
        tonemap_mode: ToneMapMode::Logarithmic,
        tonemap_curve: ToneCurve::linear(),
        use_curve: true,
        exposure: 1.0,
        gamma,
        deterministic_rng: false,
        histogram_color_scale: 10.0,
        low_density_smoothing: 0.5,
        density_compression_strength: 0.0,
        blend_factor: 0.1,
        target_iterations_per_pixel: 0,
        iterations_per_thread: 256,
        speed_multiplier: 1,
    })
}

/// Parse a single <xform> element (transform)
/// Returns (Transform, color_index) where color_index is the palette position
fn parse_xform_element(
    reader: &mut Reader<&[u8]>,
    element: &quick_xml::events::BytesStart,
) -> Result<(Transform, Option<usize>)> {
    let mut transform = Transform::new();
    let registry = global_registry();
    let mut color_index = None;

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
            "coefs" => {
                // Parse "a b c d e f" format
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() >= 6 {
                    transform.a = parts[0].parse().unwrap_or(1.0);
                    transform.b = parts[1].parse().unwrap_or(0.0);
                    transform.c = parts[2].parse().unwrap_or(0.0);
                    transform.d = parts[3].parse().unwrap_or(1.0);
                    transform.e = parts[4].parse().unwrap_or(0.0);
                    transform.f = parts[5].parse().unwrap_or(0.0);
                }
            }
            "opacity" => {
                // Store for future use, currently not used in our renderer
            }
            _ => {
                // Try to parse as variation
                if let Ok(weight_value) = value.parse::<f32>() {
                    if weight_value != 0.0 {
                        // Look up variation by name
                        if registry.get(key).is_some() {
                            transform.variations.insert(key.to_string(), weight_value);
                        }
                    }
                }
            }
        }
    }

    Ok((transform, color_index))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_spherical_example() {
        let xml = r#"
<flames name="spherical-apo">
<flame name="Spherical Test" version="Apophysis 7x Version 15D" size="1500 1000" center="0.12666568780208 0.0566529891945883" scale="208.227773031847" background="0 0 0" brightness="1" gamma="1">
   <xform weight="1" color="0" spherical="1" coefs="0.9 0 0 0.9 0 0" opacity="1" />
   <palette count="256" format="RGB">
      CC745ECB745ECA735EC8735DC7725DC6725DC5725DC3715C
   </palette>
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
}
