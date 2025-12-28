# PR: Unified Rendering API + WASM Fix

## Summary

- Consolidate 7 duplicate rendering code paths into a single unified `render()` API
- Fix WASM headless export producing black images (shader uniform control flow violation)
- Add debugging infrastructure for WASM browser tests

## Changes

### Unified Rendering API (`src/renderer/render.rs`)

Created a single entry point for all rendering operations:

```rust
pub async fn render(
    device: &Device,
    queue: &Queue,
    job: RenderJob<'_>,
    progress: &mut dyn RenderProgress,
) -> Result<RenderOutput, RenderError>
```

**Benefits:**
- Reduces code duplication from ~7 render loops to 1
- Consistent behavior across CLI export, WASM export, thumbnails, animation frames
- Builder pattern for configuration (`RenderJob`)
- Progress callback trait for long-running renders

**Migrated paths:**
- `app/export.rs` - Desktop and WASM headless export
- `renderer/thumbnail.rs` - Preset thumbnail generation
- `app/config.rs` - Custom size export
- `animation/export.rs` - Animation frame rendering

### WASM Black Image Fix (`shaders/tonemap.wgsl`)

**Root cause:** `textureSample()` was called inside non-uniform control flow (an `if` block depending on per-pixel path data). This violates the WGSL specification.

- Desktop GPU drivers are lenient and allow this
- Browser WebGPU strictly enforces the spec, causing shader compilation to fail silently

**Fix:** Replace `textureSample` with `textureLoad` for palette lookup in path map coloring:

```wgsl
// Before (broken in WASM):
fractal_color = textureSample(palette_texture, palette_sampler, vec2<f32>(t, 0.5)).rgb;

// After (works everywhere):
let palette_idx = u32(clamp(t * 255.0, 0.0, 255.0));
fractal_color = textureLoad(palette_texture, vec2<i32>(i32(palette_idx), 0), 0).rgb;
```

### Testing Infrastructure

**`scripts/run_benchmarks.py`:**
- Added `--wasm-only` flag to skip CPU/desktop benchmarks
- Added `--no-save` flag to skip saving results (for quick iteration)

**`tests/visual/wasm/test_wasm.py`:**
- Added browser console log capture for debugging WASM issues
- Filters logs to show only render-related messages

### Test Fixes

**`src/export/tiled.rs`:**
- Fixed test assertions that assumed 1M pixel threshold instead of actual 4M

## Test Plan

- [x] `cargo test --release` - All 119 tests pass
- [x] `cargo build --release` - Desktop builds successfully
- [x] Desktop headless export produces correct images
- [x] WASM visual regression tests - All 7 tests pass
- [x] Interactive WASM app still works (verified in browser)

## Files Changed

- `src/renderer/render.rs` (new) - Unified render API
- `src/renderer/mod.rs` - Export new types
- `src/app/export.rs` - Use unified API
- `src/renderer/thumbnail.rs` - Use unified API
- `src/app/config.rs` - Use unified API
- `src/animation/export.rs` - Partial migration (keeps renderer reuse optimization)
- `shaders/tonemap.wgsl` - Fix textureSample uniform control flow
- `scripts/run_benchmarks.py` - Add --wasm-only and --no-save flags
- `tests/visual/wasm/test_wasm.py` - Add browser console logging
- `src/export/tiled.rs` - Fix test assertions
- `tests/visual/baseline/wasm/*.png` - Updated baselines
