# Unified Rendering API Design

## Problem Statement

The codebase has **7 different code paths** that create FlameRenderer and run render loops:

| Location | Purpose | Creates Device? |
|----------|---------|-----------------|
| `app/mod.rs:103` | Main interactive app | No (uses App's GPU) |
| `app/config.rs:129` | UI "Export PNG" button | No (uses App's GPU) |
| `app/export.rs:39` | Headless CLI/WASM export | Yes (creates own) |
| `renderer/thumbnail.rs:31` | Gallery thumbnails | No (uses App's GPU) |
| `animation/export.rs:903` | Video export (per frame) | Yes (creates own) |
| `animation/export.rs:1242` | Fast video export | Yes (creates own) |
| `export/high_res.rs` | Tiled CPU histogram | Yes (creates own) |

### Duplicated Code

Each path duplicates:
1. **FlameRenderer creation** - `FlameRenderer::new(device, queue, format, w, h, flame)`
2. **Config loading** - `renderer.load_config(device, encoder, queue, config, palette, ipt, burn_in)`
3. **Render loop** - The exact same pattern:
   ```rust
   while total_rendered < target {
       compute_pass(..., clear_histogram, clear_paths);
       batch_frame_count += 1;
       if batch_frame_count >= BATCH_SIZE {
           accumulate_pass(...);
           batch_frame_count = 0;
       }
   }
   // Final partial batch
   if batch_frame_count > 0 {
       accumulate_pass(...);
   }
   tonemap_pass();
   read_pixels();
   ```
4. **Constants** - `NUM_WORKGROUPS=128`, `THREADS_PER_WORKGROUP=64`, `BATCH_SIZE=4`

### Current Issues

1. **WASM headless export produces black images** - Bug exists only in headless path, not interactive
2. **Hard to maintain** - Fixing a bug requires changes in 5+ places
3. **Inconsistent behavior** - Some paths batch, some don't; burn_in values differ
4. **Code bloat** - ~500 lines of duplicated render loop logic

---

## Proposed Solution: Unified Rendering API

### Core Principle

**Separate concerns:**
1. **GPU Resource Management** - Who owns device/queue
2. **Rendering** - The actual compute/accumulate/tonemap loop
3. **Output Handling** - What to do with pixels (display, save, encode)

### API Design

```rust
/// Configuration for a render job
pub struct RenderJob {
    /// Fractal configuration (transforms, colors, view settings)
    pub config: FractalConfig,

    /// Output dimensions
    pub width: u32,
    pub height: u32,

    /// How many iterations to render (None = use config.max_iterations)
    pub target_iterations: Option<u64>,

    /// Iterations per GPU thread per dispatch
    pub iterations_per_thread: u32,

    /// Burn-in iterations (skipped before plotting)
    pub burn_in: u32,

    /// Transparent background (for PNG export)
    pub transparent: bool,
}

impl RenderJob {
    /// Create from FractalConfig with defaults
    pub fn from_config(config: FractalConfig, width: u32, height: u32) -> Self;

    /// Builder methods
    pub fn with_iterations(self, iterations: u64) -> Self;
    pub fn with_transparent(self, transparent: bool) -> Self;
}

/// Result of a completed render
pub struct RenderOutput {
    pub width: u32,
    pub height: u32,
    pub rgba_data: Vec<u8>,
    pub total_iterations: u64,
    pub render_time_ms: f64,
}

/// Progress callback for long-running renders
pub trait RenderProgress {
    fn on_progress(&mut self, current: u64, total: u64);
    fn is_cancelled(&self) -> bool { false }
}

/// The unified rendering function
///
/// Caller provides device/queue - this keeps platform-specific GPU setup separate.
pub async fn render(
    device: &Device,
    queue: &Queue,
    job: RenderJob,
    progress: Option<&mut dyn RenderProgress>,
) -> Result<RenderOutput, RenderError>;
```

### Usage Examples

**Headless CLI Export:**
```rust
let (device, queue) = create_headless_device().await?;
let job = RenderJob::from_config(config, 1920, 1080);
let output = render(&device, &queue, job, Some(&mut CliProgress)).await?;
save_png(&output, "output.png")?;
```

**Thumbnail Generation:**
```rust
// Uses app's existing device/queue
let job = RenderJob::from_config(config, 256, 256)
    .with_iterations(10_000_000);
let output = render(&app.device, &app.queue, job, None).await?;
```

**Video Export (per frame):**
```rust
for frame in 0..total_frames {
    let frame_config = apply_animation(base_config, frame);
    let job = RenderJob::from_config(frame_config, 1920, 1080);
    let output = render(&device, &queue, job, None).await?;
    ffmpeg_stdin.write_all(&output.rgba_data)?;
}
```

**Interactive App:**
```rust
// Interactive mode is special - needs incremental rendering
// Could use a different API or a streaming variant:
pub struct IncrementalRenderer {
    renderer: FlameRenderer,
    // ...
}

impl IncrementalRenderer {
    /// Render one batch, return texture view for display
    pub fn render_batch(&mut self) -> &TextureView;

    /// Check if target reached
    pub fn is_complete(&self) -> bool;
}
```

---

## Implementation Plan

### Phase 1: Create Unified `render()` Function

1. Move `render_to_pixels()` from `app/export.rs` to new `src/renderer/render.rs`
2. Add `RenderJob` struct with builder pattern
3. Add optional progress callback
4. Add cancellation support

### Phase 2: Migrate Export Paths

1. **`app/export.rs`** - Already uses `render_to_pixels()`, just update to new API
2. **`renderer/thumbnail.rs`** - Replace duplicated loop with `render()` call
3. **`app/config.rs`** - Replace UI export code with `render()` call
4. **`animation/export.rs`** - Replace per-frame rendering with `render()` call

### Phase 3: Handle Special Cases

1. **HighResExporter** - Keep separate (different architecture: GPU samples → CPU histogram → GPU tonemap)
2. **Interactive App** - Create `IncrementalRenderer` wrapper or keep current approach

### Phase 4: Cleanup

1. Remove duplicated render loops
2. Consolidate constants (`NUM_WORKGROUPS`, `BATCH_SIZE`, etc.)
3. Update documentation

---

## Benefits

1. **Single point of truth** - One render loop to maintain and debug
2. **WASM bug isolation** - If interactive WASM works, headless should too (same render code)
3. **Consistent quality** - Same batching, burn-in, accumulation across all paths
4. **Easier testing** - Test render() once, all paths benefit
5. **~400 lines removed** - Less code to maintain

---

## Open Questions

1. **Should interactive mode use the same API?**
   - Pro: Maximum consistency
   - Con: Interactive needs frame-by-frame control, texture views for display
   - Recommendation: Keep interactive separate but share FlameRenderer internals

2. **Where should device/queue creation live?**
   - Current: Scattered across export paths
   - Proposal: Separate `gpu::create_headless_device()` function
   - WASM vs Desktop differences isolated there

3. **How to handle HighResExporter?**
   - Different architecture (CPU histogram)
   - Could share some code (GPU sample generation) but not render loop
   - Recommendation: Keep separate for now, possibly unify later

---

## Files to Modify

| File | Change |
|------|--------|
| `src/renderer/mod.rs` | Add `pub mod render;` |
| `src/renderer/render.rs` | New file with unified API |
| `src/app/export.rs` | Use `render()`, remove duplicated loop |
| `src/renderer/thumbnail.rs` | Use `render()`, remove duplicated loop |
| `src/app/config.rs` | Use `render()`, remove duplicated loop |
| `src/animation/export.rs` | Use `render()`, remove duplicated loop |
