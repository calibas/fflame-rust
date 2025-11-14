# Viewport vs Export Rendering - Side-by-Side Comparison

## Overview

This document compares the viewport rendering code path (WORKS) with the export rendering code path (BROKEN) to identify differences.

## Initialization

### Viewport Renderer Creation
**File**: `src/app/mod.rs:82-93`
```rust
let flame_renderer = FlameRenderer::new(
    &gpu.device,
    &gpu.queue,
    gpu.config.format,  // Surface format from GPU context
    initial_viewport_size.0,
    initial_viewport_size.1,
    &flame,
);
```
**Key details**:
- Uses surface format from GPU context
- Uses initial viewport size (window size)
- Renderer persists for entire app lifetime

### Export Renderer Creation
**File**: `src/app/config.rs:115-122`
```rust
let surface_format = egui_wgpu::wgpu::TextureFormat::Rgba8Unorm;
let mut temp_renderer = FlameRenderer::new(
    &self.gpu.device,
    &self.gpu.queue,
    surface_format,
    self.export_width,
    self.export_height,
    &config.flame,
);
```
**Key details**:
- Hardcoded `Rgba8Unorm` format
- Uses custom export dimensions
- Temporary renderer (created and destroyed)

⚠️ **DIFFERENCE**: Surface format - viewport uses `gpu.config.format`, export uses hardcoded `Rgba8Unorm`

## Configuration Loading

### Viewport Config Loading
**File**: `src/app/mod.rs` (during startup and preset loading)
```rust
// Via import_config() which calls:
renderer.load_config(&self.gpu.device, &mut encoder, &self.gpu.queue, &config, palette, config.iterations_per_thread);
```

### Export Config Loading
**File**: `src/app/config.rs:136-137`
```rust
temp_renderer.load_config(&self.gpu.device, &mut encoder, &self.gpu.queue, &config, palette, config.iterations_per_thread);
self.gpu.queue.submit(std::iter::once(encoder.finish()));
```

✓ **SAME**: Both use identical `load_config()` call

## Render Loop

### Viewport Render Loop
**File**: `src/app/mod.rs:1081-1131`

**Per-frame sequence**:
```rust
const NUM_WORKGROUPS: u32 = 128;

// 1. Compute pass
let clear_histogram = self.frames_since_accumulation == 1;
let samples_this_frame = renderer.compute_pass(
    &mut render_encoder,
    &self.gpu.queue,
    NUM_WORKGROUPS,
    final_config.iterations_per_thread,  // e.g., 1000
    final_config.zoom,
    final_config.pan_x,
    final_config.pan_y,
    final_config.rotation,
    final_config.camera_rotation_x,
    final_config.camera_rotation_y,
    final_config.camera_z,
    final_config.speed_factor,
    clear_histogram  // Only clear on first frame of batch
);

// 2. Accumulate pass (conditional)
if should_accumulate {
    let total_samples_in_batch = samples_this_frame * self.accumulation_batch_size as u64;
    renderer.accumulate_pass(
        &mut render_encoder,
        &self.gpu.queue,
        &self.gpu.device,
        total_samples_in_batch  // e.g., samples_this_frame × 4
    );
    self.frames_since_accumulation = 0;
}

// 3. Tonemap pass
renderer.tonemap_pass(&mut render_encoder);

// 4. Submit
self.gpu.queue.submit(std::iter::once(render_encoder.finish()));
```

**Key details**:
- Runs continuously at 60 FPS
- Conditional histogram clear (batching enabled)
- Conditional accumulation (every N frames)
- `accumulation_batch_size = 4`
- Samples multiplied by batch size before accumulation
- Single encoder for all 3 passes

### Export Render Loop
**File**: `src/app/config.rs:144-188`

**Per-iteration sequence**:
```rust
const NUM_WORKGROUPS: u32 = 128;
const THREADS_PER_WORKGROUP: u64 = 64;
let iterations_per_frame = config.iterations_per_thread / config.speed_multiplier;  // 1000/4 = 250

while total_rendered < target {
    for _ in 0..config.speed_multiplier {  // Loop 4 times
        let mut encoder = self.gpu.device.create_command_encoder(...);

        // 1. Compute pass
        temp_renderer.compute_pass(
            &mut encoder,
            &self.gpu.queue,
            NUM_WORKGROUPS,
            iterations_per_frame,  // 250 (not 1000!)
            config.zoom,
            config.pan_x,
            config.pan_y,
            config.rotation,
            config.camera_rotation_x,
            config.camera_rotation_y,
            config.camera_z,
            config.speed_factor,
            true,  // ALWAYS clear histogram
        );

        // 2. Accumulate pass (always)
        let samples = NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * iterations_per_frame as u64;
        temp_renderer.accumulate_pass(
            &mut encoder,
            &self.gpu.queue,
            &self.gpu.device,
            samples  // 2,048,000 per iteration
        );

        total_rendered += samples;
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
    }
}

// 3. Final tonemap pass (separate encoder)
let mut final_encoder = self.gpu.device.create_command_encoder(...);
temp_renderer.tonemap_pass(&mut final_encoder);
self.gpu.queue.submit(std::iter::once(final_encoder.finish()));
```

**Key details**:
- Runs in tight loop until target iterations
- ALWAYS clears histogram every iteration
- ALWAYS accumulates (no batching)
- Uses `iterations_per_frame = iterations_per_thread / speed_multiplier`
- Separate encoder for each iteration
- Final tonemap uses different encoder

⚠️ **DIFFERENCES FOUND**:

| Aspect | Viewport | Export |
|--------|----------|--------|
| iterations_per_thread | 1000 | 250 (divided by speed_multiplier) |
| Histogram clear | Conditional (batching) | Always (every iteration) |
| Accumulation | Conditional (batching) | Always |
| Samples passed to accumulate | `samples × batch_size` | `samples` only |
| Encoder usage | Single encoder for 3 passes | New encoder per iteration |
| Tonemap timing | After each batch | Once at the end |

## Accumulation Parameters

### Viewport
**File**: `src/app/mod.rs:1103`
```rust
let total_samples_in_batch = samples_this_frame * self.accumulation_batch_size as u64;
renderer.accumulate_pass(&mut render_encoder, &self.gpu.queue, &self.gpu.device, total_samples_in_batch);
```
- `samples_this_frame = 128 × 64 × 1000 = 8,192,000`
- `accumulation_batch_size = 4`
- `total_samples_in_batch = 8,192,000 × 4 = 32,768,000`

### Export
**File**: `src/app/config.rs:176`
```rust
let samples = NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * iterations_per_frame as u64;
temp_renderer.accumulate_pass(&mut encoder, &self.gpu.queue, &self.gpu.device, samples);
```
- `iterations_per_frame = 1000 / 4 = 250`
- `samples = 128 × 64 × 250 = 2,048,000`
- No multiplication by batch size

⚠️ **CRITICAL DIFFERENCE**: Viewport passes 16× more samples per accumulation call!

## Blend Factor Calculation

Both viewport and export use the same `accumulate_pass()` internal logic:

**File**: `src/renderer/compute_kernel.rs:240-256`
```rust
pub fn accumulate_pass(&mut self, encoder: &mut CommandEncoder, queue: &Queue, device: &Device, samples_this_frame: u64) {
    self.samples_accumulated += samples_this_frame;

    let blend_factor = if self.overwrite_mode {
        1.0
    } else if self.use_dynamic_blend {
        samples_this_frame as f32 / self.samples_accumulated as f32
    } else {
        self.blend_factor  // 0.1 default
    };

    // ... creates AccumulateParams with blend_factor
}
```

**Viewport** (with batching):
- First accumulate: `32,768,000 / 32,768,000 = 1.0` (full blend)
- Second accumulate: `32,768,000 / 65,536,000 = 0.5`
- Third accumulate: `32,768,000 / 98,304,000 = 0.33`
- Etc.

**Export** (fixed blend after our change):
- All accumulates: `0.1` (fixed blend factor)

## Summary of Critical Differences

1. **iterations_per_thread value**: Viewport uses full value (1000), Export divides by speed_multiplier (250)
2. **Samples per accumulation**: Viewport multiplies by batch_size (×4), Export doesn't
3. **Histogram clearing**: Viewport batches (clear every 4 frames), Export clears every iteration
4. **Accumulation frequency**: Viewport batches (every 4 frames), Export every iteration
5. **Tonemap frequency**: Viewport after each batch, Export once at end
6. **Blend factor**: Viewport uses dynamic (decreasing), Export uses fixed 0.1

## Hypothesis

The brightness issue may be caused by the **different accumulation strategy**:

- **Viewport**: Accumulates large batches (32M samples) infrequently → Higher density per accumulation
- **Export**: Accumulates small chunks (2M samples) frequently → Lower density per accumulation

Even though the total iteration count is the same, the **density distribution** in the accumulation buffer may differ, affecting the tonemap brightness calculation.

## Next Test

Export using viewport's exact parameters:
- Use full `iterations_per_thread` (don't divide)
- Multiply samples by batch size before accumulation
- See if brightness matches
