# WASM Testing Implementation Notes

## Current Blocker

The WASM build (`src/lib.rs::wasm_main()`) creates a full interactive app with egui UI, not a headless renderer. It:
1. Auto-starts on page load via `#[wasm_bindgen(start)]`
2. Runs winit event loop with egui UI
3. Doesn't expose any test APIs via `#[wasm_bindgen]`

This makes automated testing difficult because:
- Can't programmatically load config files
- Can't trigger headless renders
- Can't extract canvas data without UI interaction
- No way to know when render is "complete" (it runs continuously)

## Required Changes for WASM Testing

To enable automated WASM testing, we need to add:

### 1. Export API (`src/lib.rs`)

```rust
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WasmTestApi {
    // Headless renderer without UI
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WasmTestApi {
    #[wasm_bindgen(constructor)]
    pub async fn new(canvas: web_sys::HtmlCanvasElement) -> Result<WasmTestApi, JsValue> {
        // Create headless renderer
    }

    #[wasm_bindgen]
    pub fn load_config(&mut self, config_json: &str) -> Result<(), JsValue> {
        // Load FractalConfig from JSON
    }

    #[wasm_bindgen]
    pub async fn render(&mut self, iterations: u64) -> Result<(), JsValue> {
        // Render exact iteration count (deterministic)
    }

    #[wasm_bindgen]
    pub fn get_current_iterations(&self) -> u64 {
        // Query iteration progress
    }
}
```

### 2. Test Mode Flag

Add conditional compilation:
- Normal mode: Full UI app (current behavior)
- Test mode: Headless renderer with test API

```rust
#[cfg(all(target_arch = "wasm32", not(feature = "wasm_test")))]
#[wasm_bindgen(start)]
pub async fn wasm_main() {
    // Current implementation
}

#[cfg(all(target_arch = "wasm32", feature = "wasm_test"))]
// Export WasmTestApi instead
```

### 3. Deterministic Rendering

Ensure WASM render path:
- Respects `deterministic_rng: true` flag
- Renders exact `max_iterations` count
- Uses same RNG seed as desktop

### 4. Build Process

```bash
# Normal WASM build (with UI)
wasm-pack build --target web --release

# Test WASM build (headless API)
wasm-pack build --target web --release --features wasm_test
```

## Alternative: Visual Comparison Only

Instead of automated testing, we could:
1. Build WASM manually
2. Load test configs via UI
3. Manually inspect visual output
4. Compare against desktop screenshots

This is less ideal but doesn't require code changes.

## Recommendation

**Defer WASM testing to Phase 4** after core test coverage is expanded:
- Phase 2: Add 40+ desktop test configs
- Phase 3: GPU benchmarks
- Phase 4: WASM test infrastructure (requires wasm_bindgen exports)
- Phase 5: CI/CD integration

WASM testing requires significant refactoring and isn't critical since:
- Desktop and WASM use identical shader code
- Same FlameRenderer, same GPU pipelines
- Main difference is egui vs no-UI
- Most rendering bugs will be caught by desktop tests

## Status

**Not Implemented** - Requires wasm_bindgen export API.

Created scaffolding (`test_wasm.py`, `test.html`) but they won't work without the Rust-side changes above.
