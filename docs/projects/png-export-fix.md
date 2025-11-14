# PNG Export Rendering Fix

**Branch:** `fix/png-export-rendering`
**Status:** In Progress
**Created:** 2025-01-13

## Problem

PNG export (both app and CLI) produces incorrect output:
- Images are significantly darker than on-screen rendering (~4x darker)
- Tone curves don't work (should make output white/black, but have no effect)
- Palette may not be applied correctly

**Root Cause:**
After the frame synchronization refactor, `capture_from_tonemap_render()` creates a separate render pass that doesn't match the normal rendering pipeline. It has stale tonemap parameters and doesn't properly sync with the working render loop.

## Solution: Unified Fractal Texture

Instead of creating separate render passes for export, have the renderer maintain a `fractal_texture` that it always renders to. Both display and export read from this same texture.

### Current Architecture

```
FlameRenderer:
  - accumulation_buffer_a, accumulation_buffer_b (ping-pong)
  - tonemap_pass(target_view) -> renders to provided view

App Rendering:
  egui_layer.fractal_texture <- tonemap_pass() renders here
  Display shows this texture

PNG Export:
  capture_from_tonemap_render():
    - Creates temporary texture
    - Calls tonemap_pass() with temp texture
    - Reads pixels from temp texture
    - BROKEN: Stale params, wrong sync, different render path
```

### New Architecture

```
FlameRenderer:
  - accumulation_buffer_a, accumulation_buffer_b (ping-pong)
  - fractal_texture (Rgba8Unorm) <- NEW
  - fractal_texture_view <- NEW
  - tonemap_pass() -> always renders to fractal_texture

App Rendering:
  renderer.tonemap_pass() -> renders to internal texture
  Display shows renderer.get_fractal_texture_view()

PNG Export:
  renderer.read_fractal_pixels(transparent, bg_color):
    - Reads from fractal_texture (already rendered correctly)
    - Applies background blending on CPU if needed
    - Returns pixel data
```

### Benefits

1. **Single Source of Truth:** Display and export use the exact same rendered texture
2. **No Separate Render Path:** Export reads what was already rendered correctly
3. **Zero Overhead:** Same number of GPU operations during normal rendering
4. **Cleaner Ownership:** Renderer owns its output texture
5. **Flexible Dimensions:** Can render at different resolution than display
6. **Transparent + Opaque:** Background blending on CPU gives full control

## Implementation Plan

### Phase 1: Add Fractal Texture to Renderer

**File:** `src/renderer/compute_kernel.rs`

```rust
pub struct FlameRenderer {
    // ... existing fields ...

    // NEW: Output texture that tonemap renders to
    fractal_texture: Texture,
    fractal_texture_view: TextureView,
}

impl FlameRenderer {
    pub fn new(...) -> Self {
        // ... existing setup ...

        // Create fractal output texture (Rgba8Unorm for compatibility)
        let fractal_texture = device.create_texture(&TextureDescriptor {
            label: Some("Fractal Output"),
            size: Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let fractal_texture_view = fractal_texture.create_view(&TextureViewDescriptor::default());

        Self {
            // ... existing fields ...
            fractal_texture,
            fractal_texture_view,
        }
    }

    pub fn resize(...) {
        // Recreate fractal_texture with new dimensions
        // ... existing buffer recreation ...

        self.fractal_texture = device.create_texture(...);
        self.fractal_texture_view = self.fractal_texture.create_view(...);
    }

    // NEW: Public accessor for display
    pub fn get_fractal_texture_view(&self) -> &TextureView {
        &self.fractal_texture_view
    }
}
```

### Phase 2: Update tonemap_pass to Use Internal Texture

**File:** `src/renderer/compute_kernel.rs`

```rust
// OLD signature:
pub fn tonemap_pass(&self, encoder: &mut CommandEncoder, target_view: &TextureView)

// NEW signature:
pub fn tonemap_pass(&self, encoder: &mut CommandEncoder) {
    let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("Tonemap Pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: &self.fractal_texture_view,  // Use internal texture
            // ... rest same ...
        })],
        // ... rest same ...
    });
    // ... rest same ...
}
```

### Phase 3: Update App to Use Renderer's Texture

**File:** `src/app/mod.rs`

```rust
// OLD:
let fractal_view = self.egui_layer.ensure_fractal_texture(&self.gpu.device, ...);
renderer.tonemap_pass(&mut render_encoder, fractal_view);

// NEW:
renderer.tonemap_pass(&mut render_encoder);
// egui_layer displays renderer.get_fractal_texture_view() instead
```

**File:** `src/ui/mod.rs` or `src/ui/panel_viewer.rs`

```rust
// Remove fractal_texture management from egui_layer
// Display renderer's texture directly in the UI panel
```

### Phase 4: Replace capture_pixels with Simpler Read

**File:** `src/renderer/compute_kernel.rs`

```rust
// NEW: Simple read from fractal_texture
pub async fn read_fractal_pixels(
    &self,
    device: &Device,
    queue: &Queue,
    transparent: bool,
    background_color: [f32; 3],
) -> Result<(u32, u32, Vec<u8>), String> {
    // Wait for any pending rendering to complete
    let sync_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Pre-Read Sync"),
    });
    queue.submit(std::iter::once(sync_encoder.finish()));
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

    // Create staging buffer
    let bytes_per_pixel = 4; // RGBA8
    let unpadded_bytes_per_row = self.width * bytes_per_pixel;
    let align = COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
    let buffer_size = (padded_bytes_per_row * self.height) as u64;

    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Fractal Read Buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Copy texture to buffer
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Fractal Read Encoder"),
    });
    encoder.copy_texture_to_buffer(
        TexelCopyTextureInfo {
            texture: &self.fractal_texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfo {
            buffer: &buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(self.height),
            },
        },
        Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    // Map and read
    let buffer_slice = buffer.slice(..);
    let (tx, rx) = futures::channel::oneshot::channel();
    buffer_slice.map_async(MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    rx.await.map_err(|_| "Failed to map buffer".to_string())?
        .map_err(|e| format!("Buffer map error: {:?}", e))?;

    let data = buffer_slice.get_mapped_range();

    // Copy data, optionally blend background
    let mut rgba_data = Vec::with_capacity((self.width * self.height * 4) as usize);
    for y in 0..self.height {
        let row_start = (y * padded_bytes_per_row) as usize;
        let row_end = row_start + (self.width * bytes_per_pixel) as usize;
        let row_data = &data[row_start..row_end];

        for x in 0..self.width {
            let pixel_start = (x * bytes_per_pixel) as usize;
            let r = row_data[pixel_start];
            let g = row_data[pixel_start + 1];
            let b = row_data[pixel_start + 2];
            let a = row_data[pixel_start + 3];

            if transparent {
                // Transparent mode: keep original RGBA
                rgba_data.extend_from_slice(&[r, g, b, a]);
            } else {
                // Opaque mode: blend with background, set alpha=255
                let alpha = a as f32 / 255.0;
                let bg_r = (background_color[0] * 255.0) as u8;
                let bg_g = (background_color[1] * 255.0) as u8;
                let bg_b = (background_color[2] * 255.0) as u8;

                let out_r = ((r as f32 * alpha) + (bg_r as f32 * (1.0 - alpha))) as u8;
                let out_g = ((g as f32 * alpha) + (bg_g as f32 * (1.0 - alpha))) as u8;
                let out_b = ((b as f32 * alpha) + (bg_b as f32 * (1.0 - alpha))) as u8;

                rgba_data.extend_from_slice(&[out_r, out_g, out_b, 255]);
            }
        }
    }

    Ok((self.width, self.height, rgba_data))
}
```

### Phase 5: Update Export to Use New Method

**File:** `src/app/export.rs`

```rust
// OLD:
let (width, height, rgba_data) = renderer.capture_pixels(&device, &queue, false, surface_format).await?;

// NEW:
let background_color = config.background_color;
let (width, height, rgba_data) = renderer.read_fractal_pixels(&device, &queue, false, background_color).await?;
```

**File:** `src/app/mod.rs` (PNG export)

```rust
// OLD:
let pixels_future = renderer.capture_pixels(&self.gpu.device, &self.gpu.queue, transparent, self.gpu.config.format);

// NEW:
let background_color = export_config.background_color;
let pixels_future = renderer.read_fractal_pixels(&self.gpu.device, &self.gpu.queue, transparent, background_color);
```

### Phase 6: Remove Old capture_pixels Methods

**File:** `src/renderer/compute_kernel.rs`

Delete:
- `capture_pixels()`
- `capture_from_tonemap_render()`
- `capture_from_accumulation_buffer()`

These are replaced by the simpler `read_fractal_pixels()`.

## Testing Plan

1. **Visual Verification:**
   - Export simple-tcwhite.fflame → should be all white
   - Export simple-tcblack.fflame → should be all black
   - Export simple.fflame → compare with on-screen rendering (should match exactly)

2. **Transparency Testing:**
   - Export with transparent=true → verify alpha channel preserved
   - Export with transparent=false → verify background blending works

3. **Resolution Testing:**
   - Export at different dimensions than display
   - Verify no crashes or artifacts

4. **Performance Testing:**
   - Verify no FPS drop during normal rendering
   - Measure export time (should be same as before)

## Success Criteria

- [ ] Tone curves work correctly (tcwhite → white, tcblack → black)
- [ ] Export brightness matches on-screen rendering
- [ ] Palette is applied correctly
- [ ] Transparent PNG exports work
- [ ] Opaque PNG exports with background work
- [ ] No performance regression in normal rendering
- [ ] CLI export works correctly
- [ ] App PNG export works correctly

## Migration Notes

**Breaking Changes:**
- `tonemap_pass()` signature changes (removes `target_view` parameter)
- `capture_pixels()` removed, replaced with `read_fractal_pixels()`
- egui_layer no longer manages fractal_texture

**Backward Compatibility:**
- No config file changes needed
- No shader changes needed
- Only internal API changes

## References

- Original issue: PNG export produces dark images with broken tone curves
- Related: Frame synchronization refactor (commit 446da88)
- Related: Texture format fix (commit 9664a7a)
