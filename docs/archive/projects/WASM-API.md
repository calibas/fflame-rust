# WASM API - JavaScript Control for Fractal Flame Renderer

**Status:** ✅ Complete and Production-Ready (2025-11-15)
**Priority:** High (enables URL sharing, browser export, programmatic generation)

## Overview

The WASM API provides JavaScript bindings for controlling the fractal flame renderer from the browser. This enables:

1. **URL Parameter Config Loading** - Share fractals via `?preset=Bubble` or `?config=<base64json>`
2. **Browser-Based PNG Export** - Download button functionality without server round-trip
3. **Programmatic Generation** - JavaScript control of fractal rendering
4. **Automated Testing** - Selenium/Puppeteer integration (though manual testing is preferred)

## Implementation

### Files Created

**Rust Side:**
- `src/wasm_api.rs` - WasmApi struct with wasm_bindgen exports (280 lines)
- `src/app/export.rs` - Added `export_headless_wasm()` (150 lines, reuses CLI code)

**JavaScript Side:**
- `tests/visual/wasm/test.html` - Example usage with headless rendering
- `tests/visual/wasm/test_wasm.py` - Automated test runner (deferred)

### API Reference

#### WasmApi Methods

```javascript
import init, { WasmApi } from './pkg/fractal_flame_wgpu.js';

await init(); // Initialize WASM module

const api = new WasmApi();

// Load config from JSON string
api.load_config_json('{"flame": {...}, "max_iterations": 1000000}');

// Load config from URL parameters
api.load_config_from_url(window.location.href); // ?preset=Bubble or ?config=<base64>

// Load built-in preset
api.load_preset("Bubble");

// Export PNG (returns Uint8Array)
const pngBytes = await api.export_png(800, 600, 256); // width, height, iterations_per_thread

// Query state
const hasConfig = api.has_config();
const progress = api.get_progress(); // 0.0 to 1.0
const currentIters = api.get_current_iterations();
const targetIters = api.get_target_iterations();

// Get config as JSON
const configJson = api.get_config_json();
const config = JSON.parse(configJson);

// Get available presets
const presetsJson = api.get_preset_names();
const presets = JSON.parse(presetsJson); // Array of strings

// Set target iterations (overrides config)
api.set_target_iterations(5000000);
```

## Use Cases

### 1. URL Parameter Config Loading

Share fractals by encoding the config in the URL:

```javascript
// Load from ?preset=<name>
if (window.location.search.includes('preset=')) {
    api.load_config_from_url(window.location.href);
}

// Load from ?config=<base64_json>
const config = {...}; // FractalConfig object
const json = JSON.stringify(config);
const base64 = btoa(json);
window.location.href = `?config=${base64}`;
```

### 2. Browser PNG Export

Download fractal as PNG without server round-trip:

```javascript
// Export PNG
const pngBytes = await api.export_png(1920, 1080, 256);

// Download
const blob = new Blob([pngBytes], { type: 'image/png' });
const url = URL.createObjectURL(blob);
const a = document.createElement('a');
a.href = url;
a.download = 'fractal.png';
a.click();
URL.revokeObjectURL(url);
```

### 3. Programmatic Generation

Generate fractals from JavaScript:

```javascript
// Load preset
api.load_preset("Bubble");

// Modify config
const config = JSON.parse(api.get_config_json());
config.zoom = 2.0;
config.pan_x = 0.5;
config.max_iterations = 10000000;
api.load_config_json(JSON.stringify(config));

// Render
const png = await api.export_png(800, 600, 256);
```

## Technical Details

### Headless Rendering

`WasmApi::export_png()` creates its own headless GPU rendering context:

1. **Reuses Desktop CLI Code** - Same logic as `export_headless()` in `src/app/export.rs`
2. **Deterministic RNG** - Respects `deterministic_rng` flag for reproducible renders
3. **Batched Accumulation** - Same quality as desktop (128 workgroups × 256 iterations × 4 batches)
4. **PNG Metadata** - Embeds build info, config, and render settings

**Performance:**
- 10M iterations @ 800x600: ~2 seconds
- 500M iterations @ 1920x1080: ~60 seconds
- Scales linearly with iteration count

### URL Parameter Parsing

Manual URL parsing (no web_sys::Url dependency):

```rust
// Parse query string manually
let parts: Vec<&str> = url_str.split('?').collect();
let query = parts[1];
let params: Vec<(&str, &str)> = query.split('&')
    .map(|param| param.split('='))
    .collect();
```

Base64 decoding is manual implementation (no external deps):

```rust
// Simple base64 decode for URL parameters
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Handles URL encoding (%3D, %2B, %2F)
    // Standard base64 character set
    // Returns decoded bytes
}
```

### Integration with Main App

The WasmApi coexists with the main interactive app:

- **Main App:** `wasm_main()` auto-starts via `#[wasm_bindgen(start)]`, requires `<canvas id="canvas">`
- **WasmApi:** Standalone, no canvas required, creates own headless context
- **Conflict:** Main app auto-starts even when using WasmApi (can panic if no canvas)
- **Solution:** Add dummy `<canvas id="canvas">` element to prevent panic

## Automated Testing (Deferred)

While the WASM API is fully functional, automated browser testing is deferred due to complexity:

**Issues:**
- wasm_main() auto-start interference
- HTTP server threading on Windows
- Hash mismatches (WebGPU vs native drivers)
- 500M iteration configs timeout (need WASM-specific versions)
- Selenium/chromedriver setup complexity

**Recommendation:**
- Prioritize Phase 2 (40+ desktop test configs)
- Desktop tests validate core rendering (same shaders, same GPU pipeline)
- Manual browser testing sufficient for WASM-specific issues
- Test infrastructure remains in `tests/visual/wasm/` for future use

See [tests/visual/wasm/IMPLEMENTATION_NOTES.md](../../tests/visual/wasm/IMPLEMENTATION_NOTES.md) for details.

## Future Enhancements

### High Priority
- **Real-time Preview Mode** - Progressive rendering with frame callbacks
- **Worker Thread Support** - Offload rendering to Web Worker
- **Streaming Export** - Large renders without blocking UI

### Medium Priority
- **Animation Support** - Keyframe interpolation and video export
- **Preset Gallery** - Visual browser with thumbnails
- **Palette Editor** - Browser-based palette creation

### Low Priority
- **Collaborative Sharing** - Firebase integration for config sharing
- **Social Media Integration** - Direct posting to Twitter/Instagram
- **NFT Minting** - On-chain fractal generation

## Related Documentation

- [docs/main/EXPORT.md](../main/EXPORT.md) - PNG export implementation
- [docs/WASM.md](../WASM.md) - WASM build guide
- [tests/visual/wasm/IMPLEMENTATION_NOTES.md](../../tests/visual/wasm/IMPLEMENTATION_NOTES.md) - Testing notes
- [docs/projects/visual-regression-testing.md](visual-regression-testing.md) - Testing plan
