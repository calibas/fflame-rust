# Palette Compact Format Optimization

## Problem

Current JSON format for indexed palettes (256 colors) is extremely wasteful:

```json
{
  "position": 0.0,
  "color": [0.7254902, 0.91764706, 0.92156863]
}
```

**~80 bytes × 256 = ~20KB per palette**

For 713 Apophysis palettes: **~14MB total**

## Solution

Use compact hex color format for indexed palettes:

```json
{
  "name": "Palette Name",
  "indexed_colors": "B9EAEB,B9FAFB,B9FAFF,..."
}
```

**7 bytes × 256 = ~1.8KB per palette** (91% reduction)

For 713 palettes: **~1.3MB total**

## Design

### Serialization (Save)

When saving a palette embedded in `.fflame` files:

1. **Check if palette is indexed**: Exactly 256 stops at positions `i/255.0` (i = 0..255)
2. **If indexed**: Serialize as `indexed_colors` hex string
3. **If gradient**: Serialize as `stops` array (legacy format)

```rust
// Pseudo-code
fn serialize_palette(palette: &Palette) -> Value {
    if is_indexed_256(palette) {
        json!({
            "name": palette.name,
            "indexed_colors": to_hex_string(palette.stops),  // "RRGGBB,RRGGBB,..."
            "built_in": palette.built_in
        })
    } else {
        // Use legacy stops format for gradients
        json!({
            "name": palette.name,
            "stops": palette.stops,
            "built_in": palette.built_in
        })
    }
}

fn is_indexed_256(palette: &Palette) -> bool {
    palette.stops.len() == 256 &&
    palette.stops.iter().enumerate().all(|(i, stop)| {
        (stop.position - i as f32 / 255.0).abs() < 0.001
    })
}

fn to_hex_string(stops: &[ColorStop]) -> String {
    stops.iter()
        .map(|stop| format!("{:02X}{:02X}{:02X}",
            (stop.color[0] * 255.0) as u8,
            (stop.color[1] * 255.0) as u8,
            (stop.color[2] * 255.0) as u8))
        .collect::<Vec<_>>()
        .join(",")
}
```

### Deserialization (Load)

Support **both formats** for backward compatibility:

```rust
// Pseudo-code
fn deserialize_palette(value: &Value) -> Result<Palette> {
    if let Some(indexed_hex) = value.get("indexed_colors") {
        // NEW: Load compact hex format
        from_hex_string(
            value["name"].as_str()?,
            indexed_hex.as_str()?
        )
    } else {
        // OLD: Load legacy stops format
        Ok(Palette {
            name: value["name"].as_str()?.to_string(),
            stops: serde_json::from_value(value["stops"])?,
            built_in: value.get("built_in").and_then(|v| v.as_bool()).unwrap_or(false),
        })
    }
}

fn from_hex_string(name: &str, hex_colors: &str) -> Result<Palette> {
    let stops: Vec<ColorStop> = hex_colors.split(',')
        .enumerate()
        .map(|(i, hex)| {
            let r = u8::from_str_radix(&hex[0..2], 16)? as f32 / 255.0;
            let g = u8::from_str_radix(&hex[2..4], 16)? as f32 / 255.0;
            let b = u8::from_str_radix(&hex[4..6], 16)? as f32 / 255.0;
            Ok(ColorStop {
                position: i as f32 / 255.0,
                color: [r, g, b],
            })
        })
        .collect::<Result<_>>()?;

    Ok(Palette {
        name: name.to_string(),
        stops,
        built_in: true,
    })
}
```

## Runtime Behavior

**No changes to internal representation:**
- Palette struct keeps `Vec<ColorStop>` internally
- GPU upload unchanged
- UI rendering unchanged
- Only serialization/deserialization affected

## Migration Strategy

**Phase 1: Add support (backward compatible)**
1. Implement `indexed_colors` deserialization (load new format)
2. Implement smart serialization (save indexed as compact, gradients as stops)
3. Keep all existing palette pack files unchanged

**Phase 2: Gradual conversion (optional, future)**
- Convert Apophysis palette packs to compact format
- Update embedded WASM palettes to compact format
- Old `.fflame` files with legacy format still load correctly

## Files Affected

- `src/scene/palette.rs` - Palette struct and ser/de logic
- `.fflame` config files - Embedded palette format (auto-optimized on save)
- `assets/palettes/packs/*.json` - Optional future conversion

## Benefits

- **91% smaller** `.fflame` files with indexed palettes
- **Faster** file I/O and JSON parsing
- **Smaller** WASM binary (embedded palettes)
- **Backward compatible** - old files still load
- **Zero runtime impact** - internal format unchanged

## Non-Goals

- Auto-conversion of existing palette pack files (not needed for compatibility)
- Changes to gradient palette format (only affects indexed palettes)
- Runtime palette representation changes (stays `Vec<ColorStop>`)
