# PNG Export and Metadata

Complete guide to PNG export functionality, metadata embedding, and CLI batch export mode.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overview and data flow
- [RENDERER.md](RENDERER.md) - Rendering pipeline details
- [CONFIG.md](CONFIG.md) - FractalConfig structure
- [TESTING-GUIDE.md](../TESTING-GUIDE.md) - Visual regression testing

---

## PNG Export Overview

The renderer supports multiple export modes:

1. **Interactive Export** - Save button in UI (transparent or opaque)
2. **CLI Batch Export** - Headless rendering for testing and automation
3. **High-Resolution Export** - Any resolution via hybrid GPU/CPU architecture (Added 2025-12-18)

Standard resolutions (up to 4K) use fast GPU-only rendering (~0.5s for 10M iterations @ 800×600).
Larger resolutions automatically use the CPU histogram path (~24s for 4000×4000 @ 10M iterations).

---

## Interactive PNG Export

### UI Controls

**Location:** [src/ui/mod.rs](../../src/ui/mod.rs) - Export section

```rust
if ui.button("💾 Save PNG (Transparent)").clicked() {
    ui_response.save_png_requested = true;
    ui_response.transparent_export = true;
}
if ui.button("💾 Save PNG (Opaque)").clicked() {
    ui_response.save_png_requested = true;
    ui_response.transparent_export = false;
}
```

### Export Handler

**Location:** [src/app/mod.rs](../../src/app/mod.rs#L684-L723)

```rust
if ui_response.save_png_requested {
    // Open file dialog
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("PNG Image", &["png"])
        .save_file()
    {
        // Capture PNG with metadata
        let config = self.export_config();
        let metadata = create_png_metadata(&config, /* render stats */);

        if ui_response.transparent_export {
            self.renderer.capture_from_accumulation_buffer(
                &self.gpu.device,
                &self.gpu.queue,
                &path,
                Some(metadata),
            );
        } else {
            self.renderer.capture_from_tonemap_render(
                &self.gpu.device,
                &self.gpu.queue,
                &path,
                Some(metadata),
            );
        }
    }
}
```

---

## Dual Export Paths

### Why Two Export Methods?

The renderer uses **two different GPU pipelines** for transparent vs opaque export:

**Problem:** The tonemap shader applies tone mapping AND blends RGB with `background_color` before outputting. Even though the shader outputs an alpha channel, the RGB values are already blended with the background.

**Solution:** Transparent export reads from the **accumulation buffer** (raw fractal colors) and applies tone mapping on the CPU, preserving unblended RGB values.

### Path 1: Transparent Export (Preserves Alpha)

**Location:** [src/renderer/compute_kernel.rs:351-453](../../src/renderer/compute_kernel.rs#L351-L453)

**Function:** `capture_from_accumulation_buffer()`

**Pipeline:**
```
1. Copy Rgba16Float accumulation buffer → CPU buffer (staging buffer)
2. CPU reads f16 RGBA values (half-precision floats)
3. Apply tone mapping on CPU:
   - Logarithmic: color = log(density * density_scale + 1.0) / log_scale
   - Linear: color = density * density_scale
   - Apply gamma correction
4. Calculate alpha channel:
   - alpha = clamp(density * density_scale, 0.0, 1.0)
   - Density stored in accumulation buffer's alpha channel
5. Convert to Rgba8 (8-bit per channel)
6. Encode PNG with metadata
```

**Key Code:**
```rust
pub fn capture_from_accumulation_buffer(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &std::path::Path,
    metadata: Option<PngMetadata>,
) -> Result<(), Box<dyn std::error::Error>> {
    let width = self.buffers.width;
    let height = self.buffers.height;
    let bytes_per_row = width * 8;  // Rgba16Float = 8 bytes per pixel

    // Create staging buffer
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("PNG Export Staging Buffer"),
        size: (bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Copy accumulation texture → staging buffer
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &self.buffers.accum_texture_0,  // Current accumulation buffer
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit(Some(encoder.finish()));

    // Map buffer for CPU read
    let buffer_slice = staging_buffer.slice(..);
    buffer_slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);

    // Read f16 data and convert to Rgba8
    let data = buffer_slice.get_mapped_range();
    let f16_data: &[half::f16] = bytemuck::cast_slice(&data);

    let mut rgba8_data = vec![0u8; (width * height * 4) as usize];
    for i in 0..(width * height) as usize {
        let r = f16_data[i * 4 + 0].to_f32();
        let g = f16_data[i * 4 + 1].to_f32();
        let b = f16_data[i * 4 + 2].to_f32();
        let density = f16_data[i * 4 + 3].to_f32();

        // Apply tone mapping (simplified - actual code handles log/linear modes)
        let scale = self.params.density_scale;
        let tone_mapped_r = (r * scale).powf(1.0 / self.params.gamma);
        let tone_mapped_g = (g * scale).powf(1.0 / self.params.gamma);
        let tone_mapped_b = (b * scale).powf(1.0 / self.params.gamma);

        // Calculate alpha from density
        let alpha = (density * scale).clamp(0.0, 1.0);

        rgba8_data[i * 4 + 0] = (tone_mapped_r.clamp(0.0, 1.0) * 255.0) as u8;
        rgba8_data[i * 4 + 1] = (tone_mapped_g.clamp(0.0, 1.0) * 255.0) as u8;
        rgba8_data[i * 4 + 2] = (tone_mapped_b.clamp(0.0, 1.0) * 255.0) as u8;
        rgba8_data[i * 4 + 3] = (alpha * 255.0) as u8;
    }

    // Encode PNG with metadata
    encode_png_with_metadata(path, &rgba8_data, width, height, metadata)?;

    Ok(())
}
```

**Pros:**
- Preserves transparency (alpha channel)
- RGB values not pre-blended with background
- Suitable for compositing in other tools

**Cons:**
- Requires CPU tone mapping (slower)
- Limited to current tone mapping modes (log/linear)

### Path 2: Opaque Export (Background Blended)

**Location:** [src/renderer/compute_kernel.rs:455-543](../../src/renderer/compute_kernel.rs#L455-L543)

**Function:** `capture_from_tonemap_render()`

**Pipeline:**
```
1. Create temporary Rgba8Unorm texture (same size as viewport)
2. Run tonemap_pass() → render to temp texture
   - Full GPU tone mapping (log + gamma + background blend)
   - RGB values pre-blended with background_color
3. Copy temp texture → CPU staging buffer
4. Map buffer, read Rgba8 data directly
5. Encode PNG with metadata
```

**Key Code:**
```rust
pub fn capture_from_tonemap_render(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &std::path::Path,
    metadata: Option<PngMetadata>,
) -> Result<(), Box<dyn std::error::Error>> {
    let width = self.buffers.width;
    let height = self.buffers.height;

    // Create temporary render target
    let temp_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Tonemap Capture Texture"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let temp_view = temp_texture.create_view(&Default::default());

    // Render via tonemap pass
    let mut encoder = device.create_command_encoder(&Default::default());
    self.tonemap_pass(&mut encoder, &temp_view);
    queue.submit(Some(encoder.finish()));

    // Copy texture → staging buffer
    let bytes_per_row = width * 4;  // Rgba8 = 4 bytes per pixel
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("PNG Export Staging Buffer"),
        size: (bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &temp_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit(Some(encoder.finish()));

    // Map and read Rgba8 data
    let buffer_slice = staging_buffer.slice(..);
    buffer_slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);

    let data = buffer_slice.get_mapped_range();
    let rgba8_data = data.to_vec();

    // Encode PNG with metadata
    encode_png_with_metadata(path, &rgba8_data, width, height, metadata)?;

    Ok(())
}
```

**Pros:**
- Full GPU tone mapping (fast)
- Uses exact same shader as viewport (WYSIWYG)
- No CPU tone mapping code duplication

**Cons:**
- RGB values pre-blended with background
- Alpha channel present but RGB not compositable
- Essentially opaque despite having alpha

---

## CLI Batch Export Mode

The main app supports headless batch PNG export for testing and automation.

**Entry Point:** [src/main.rs](../../src/main.rs)

### CLI Interface

**Command syntax:**
```bash
fractal_flame_wgpu export [OPTIONS]
```

**Options:**
- `-i, --input <PATH>` - Input config file or directory (required)
- `-o, --output <PATH>` - Output PNG file or directory (required)
- `-w, --width <WIDTH>` - Output width in pixels (optional, uses config default)
- `-h, --height <HEIGHT>` - Output height in pixels (optional, uses config default)
- `--category <NAME>` - Test category for metadata (optional)
- `--iterations-per-thread <N>` - Iterations per thread (optional, uses config default)
- `--speed-multiplier <N>` - Speed multiplier for quality control (optional, default: 1)

**Examples:**

**Single file export:**
```bash
fractal_flame_wgpu export \
  -i config.fflame \
  -o output.png \
  --width 1920 \
  --height 1080
```

**Batch directory export:**
```bash
fractal_flame_wgpu export \
  -i tests/visual/configs \
  -o tests/visual/current
```

**With test category metadata:**
```bash
fractal_flame_wgpu export \
  -i tests/visual/configs/variations \
  -o tests/visual/current \
  --category variations
```

**With speed multiplier (quality control):**
```bash
fractal_flame_wgpu export \
  -i config.fflame \
  -o output.png \
  --iterations-per-thread 4096 \
  --speed-multiplier 16
```

### Implementation

**Location:** [src/app/export.rs](../../src/app/export.rs)

**Main function:** `export_headless()`

**Flow:**
```
1. Parse CLI args (clap)
2. Create headless GPU instance (no window)
3. Load config(s) from input path
4. For each config:
   a. Calculate dispatch count from max_iterations
   b. Run compute passes (chunked by speed_multiplier)
   c. Run accumulate passes (ping-pong swap each chunk)
   d. Show progress indicator (iteration count + percentage)
   e. Capture PNG with full metadata
5. Print summary (total configs, total time)
```

**Key code:**
```rust
pub async fn export_headless(
    config_path: &Path,
    output_path: &Path,
    width: Option<u32>,
    height: Option<u32>,
    test_category: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load config
    let json = std::fs::read_to_string(config_path)?;
    let mut config = FractalConfig::from_json(&json)?;

    // Override resolution if specified
    if let Some(w) = width { config.width = w; }
    if let Some(h) = height { config.height = h; }

    // Create headless GPU instance
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,  // Headless
        force_fallback_adapter: false,
    }).await.ok_or("Failed to find adapter")?;

    let (device, queue) = adapter.request_device(&Default::default(), None).await?;

    // Create renderer
    let mut renderer = FlameRenderer::new(&device, &queue, config.width, config.height);
    renderer.load_config(&config);

    // Calculate dispatch count
    let iterations_per_dispatch = renderer.workgroups * renderer.workgroup_size * renderer.iterations_per_thread;
    let total_dispatches = (config.max_iterations / iterations_per_dispatch as u64) as usize;

    // Render with progress indicator
    let start_time = std::time::Instant::now();
    for dispatch_idx in 0..total_dispatches {
        // Run compute + accumulate (one frame)
        let mut encoder = device.create_command_encoder(&Default::default());
        renderer.compute_pass(&mut encoder);
        renderer.accumulate_pass(&mut encoder);
        queue.submit(Some(encoder.finish()));

        // Progress indicator
        let completed = (dispatch_idx + 1) * iterations_per_dispatch as usize;
        let percent = (completed as f64 / config.max_iterations as f64) * 100.0;
        print!("\rRendering: {} iterations ({:.1}%)...", completed, percent);
        std::io::stdout().flush()?;
    }
    println!(" Done in {:.2}s", start_time.elapsed().as_secs_f64());

    // Create metadata
    let metadata = PngMetadata {
        build_version: VERSION,
        git_hash: GIT_HASH,
        // ... more fields
        total_iterations: config.max_iterations,
        render_time_ms: start_time.elapsed().as_millis() as f64,
        config_json: config.to_json()?,
        config_checksum: sha256(&config.to_json()?),
        test_category,
    };

    // Export PNG (opaque)
    renderer.capture_from_tonemap_render(&device, &queue, output_path, Some(metadata))?;

    Ok(())
}
```

### Performance

**Rendering speed:**
- Default: 128 workgroups × 64 threads × 256 iterations = 2,097,152 iterations per dispatch
- Example: 10M iterations @ 800×600 renders in ~0.5 seconds
- GPU-bound: Most time spent in compute shader

**Progress tracking:**
- Calculates dispatch count from `config.max_iterations`
- Prints iteration count and percentage each dispatch
- Flushes stdout for real-time updates

### Output Files

**Naming convention:**
- Single file: User-specified filename
- Batch export: `{flame_name}.png` (lowercase, spaces → underscores)

**Example:**
```
Input:  simple3.fflame (flame.name = "Simple 3")
Output: simple_3.png
```

**Metadata:**
- All PNGs include full metadata (see PNG Metadata section below)
- Test category included if `--category` specified
- Exact config JSON embedded for reproducibility

---

## PNG Metadata

All exported PNGs include comprehensive metadata in **tEXt chunks** (PNG standard).

**Location:** [src/png_metadata.rs](../../src/png_metadata.rs)

### PngMetadata Structure

```rust
pub struct PngMetadata {
    // Build Information
    pub build_version: &'static str,      // Version from Cargo.toml
    pub git_hash: &'static str,           // Git commit hash
    pub git_branch: &'static str,         // Git branch name
    pub build_timestamp: &'static str,    // Build date and time
    pub platform: &'static str,           // OS and architecture
    pub rustc_version: &'static str,      // Rustc version
    pub build_profile: &'static str,      // debug/release

    // Render Settings
    pub width: u32,                       // Output width
    pub height: u32,                      // Output height
    pub total_iterations: u64,            // Total iteration count
    pub render_time_ms: f64,              // Render time in milliseconds
    pub frame_count: u32,                 // Number of frames (dispatches)
    pub workgroups: u32,                  // Workgroups per dispatch
    pub iterations_per_dispatch: u64,     // Iterations per dispatch

    // Flame Configuration
    pub config_json: String,              // Complete FractalConfig as JSON
    pub config_checksum: String,          // SHA256 of config_json

    // Display Settings
    pub background_color: [f32; 3],       // RGB background
    pub exposure: f32,                    // Exposure adjustment
    pub gamma: f32,                       // Gamma correction
    pub use_curve: bool,                  // Tone curve enabled
    pub tonemap_mode: String,             // "Logarithmic" or "Linear"

    // Test Support (optional)
    pub test_name: Option<String>,        // Test name (e.g., "sierpinski_triangle")
    pub test_category: Option<String>,    // Test category (e.g., "variations")
}
```

### Encoding Metadata

**Function:** `encode_png_with_metadata()`

**Process:**
1. Create PNG encoder with RGBA color type
2. Write PNG header and image data
3. Write tEXt chunks for each metadata field
4. Finalize PNG file

**Key code:**
```rust
pub fn encode_png_with_metadata(
    path: &std::path::Path,
    data: &[u8],
    width: u32,
    height: u32,
    metadata: Option<PngMetadata>,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::create(path)?;
    let encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header()?;

    // Write metadata as tEXt chunks
    if let Some(meta) = metadata {
        writer.write_text_chunk("build_version", &meta.build_version)?;
        writer.write_text_chunk("git_hash", &meta.git_hash)?;
        writer.write_text_chunk("git_branch", &meta.git_branch)?;
        writer.write_text_chunk("build_timestamp", &meta.build_timestamp)?;
        writer.write_text_chunk("platform", &meta.platform)?;
        writer.write_text_chunk("rustc_version", &meta.rustc_version)?;
        writer.write_text_chunk("build_profile", &meta.build_profile)?;

        writer.write_text_chunk("width", &meta.width.to_string())?;
        writer.write_text_chunk("height", &meta.height.to_string())?;
        writer.write_text_chunk("total_iterations", &meta.total_iterations.to_string())?;
        writer.write_text_chunk("render_time_ms", &meta.render_time_ms.to_string())?;

        writer.write_text_chunk("config", &meta.config_json)?;
        writer.write_text_chunk("config_checksum", &meta.config_checksum)?;

        // ... more fields
    }

    // Write image data
    writer.write_image_data(data)?;

    Ok(())
}
```

### Reading Metadata

**Function:** `read_png_metadata()`

**Usage:**
```rust
use fractal_flame_wgpu::png_metadata::read_png_metadata;

let metadata = read_png_metadata("output.png")?;
println!("Rendered {} iterations in {:.2}ms",
    metadata.total_iterations, metadata.render_time_ms);
println!("Config checksum: {}", metadata.config_checksum);

// Verify config integrity
let config = FractalConfig::from_json(&metadata.config_json)?;
assert_eq!(sha256(&metadata.config_json), metadata.config_checksum);
```

**Key code:**
```rust
pub fn read_png_metadata(path: &std::path::Path) -> Result<PngMetadata, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let decoder = png::Decoder::new(file);
    let reader = decoder.read_info()?;

    let info = reader.info();
    let mut metadata = PngMetadata::default();

    // Read tEXt chunks
    for chunk in &info.text {
        match chunk.keyword.as_str() {
            "build_version" => metadata.build_version = chunk.text.clone(),
            "git_hash" => metadata.git_hash = chunk.text.clone(),
            "config" => metadata.config_json = chunk.text.clone(),
            "config_checksum" => metadata.config_checksum = chunk.text.clone(),
            // ... more fields
            _ => {}
        }
    }

    Ok(metadata)
}
```

### Metadata Use Cases

**Visual Regression Testing:**
- Compare config checksums to verify identical configs
- Extract iteration count and render time for performance tracking
- Test category grouping for organized test suites

**Debugging:**
- Check build version and git hash for reproducibility
- Verify exact config used to generate image
- Inspect render settings (iterations, workgroups, etc.)

**Automation:**
- Batch export with embedded test metadata
- Automated comparison scripts using checksums
- Performance profiling using render time data

---

## WASM Export Support

**Status:** Fully functional (100% complete)

### Implementation Details

**Location:** [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs)

**Issue:** WASM has strict lifetime requirements for async operations.

**Solution:** Use `unsafe` lifetime extension (safe in practice):

```rust
#[cfg(target_arch = "wasm32")]
{
    // WASM requires 'static lifetime for async block
    // Safe because GPU resources live for program lifetime
    let buffer_slice_static: &'static [u8] = unsafe {
        std::mem::transmute(buffer_slice.get_mapped_range().as_ref())
    };

    // Use buffer_slice_static in wasm_bindgen_futures::spawn_local
    // ...
}
```

**Why safe:**
- GPU device, queue, and buffers live for entire program lifetime
- WASM single-threaded execution model
- No concurrent access possible
- Resources never dropped until program exit

**WASM-specific behavior:**
- File dialogs use browser native APIs (via `rfd` crate)
- PNG encoding identical to desktop
- Metadata embedding works identically
- No CLI export mode (interactive only)

---

## High-Resolution Export System (Added 2025-12-18)

The renderer now supports PNG export at **any resolution** via a hybrid GPU/CPU architecture.

### Architecture Overview

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  GPU Compute    │────▶│  CPU Histogram   │────▶│  GPU Tonemap    │
│  (samples)      │     │  (row-binned)    │     │  (final pass)   │
└─────────────────┘     └──────────────────┘     └─────────────────┘
```

**Two export paths based on resolution:**

| Path | Condition | Method |
|------|-----------|--------|
| **GPU** | Histogram ≤128MB | Standard GPU-only rendering |
| **CPU** | Histogram >128MB | GPU compute + CPU histogram + GPU tonemap |

### Resolution Thresholds

| Resolution | Histogram Size | Export Path |
|------------|---------------|-------------|
| 1920×1080 (1080p) | 31.6 MB | GPU |
| 2560×1440 (1440p) | 56.2 MB | GPU |
| 3840×2160 (4K) | 126.6 MB | GPU |
| 4096×2160 (4K DCI) | 135.0 MB | **CPU** |
| 4000×4000 | 244.1 MB | **CPU** |
| 7680×4320 (8K) | 506.2 MB | **CPU** |

### Implementation Details

**Location:** [src/export/high_res.rs](../../src/export/high_res.rs)

**Key components:**
- `HighResExporter` - Main export struct with GPU/CPU hybrid pipeline
- `needs_cpu_export(width, height)` - Threshold check function
- Row-based parallel histogram accumulation using rayon
- GPU tonemapping via tonemap.wgsl shader

**CPU Histogram Accumulation:**
```rust
// Row-based binning eliminates lock contention
let row_bins: Vec<Vec<HistogramPixel>> = (0..height)
    .into_par_iter()
    .map(|y| {
        let mut row = vec![HistogramPixel::default(); width];
        // Process samples for this row
        row
    })
    .collect();
```

**GPU Tonemapping:**
- Uploads CPU histogram as f16 texture (Rgba16Float)
- Runs same tonemap.wgsl shader as interactive rendering
- Ensures visual consistency between preview and export

### Performance

- **4000×4000 @ 10M iterations**: ~24 seconds
- **Bottleneck**: CPU histogram accumulation (~60% of time)
- **Parallelization**: rayon for row-based accumulation and texture conversion

### UI Integration

Both UI and CLI exports automatically use the high-res path when needed:

```rust
// In src/app/config.rs
if crate::export::needs_cpu_export(self.export_width, self.export_height) {
    self.export_high_res_cpu(transparent, config);
} else {
    // Standard GPU export
}
```

---

## Current Limitations

### Export Formats

**Current:** PNG only (Rgba8)

**Future possibilities:**
- EXR/HDR for high dynamic range
- TIFF for 16-bit per channel
- Video export for animations

---

## Common Tasks

### Export PNG from Interactive App

1. Render fractal to desired quality (wait for convergence)
2. Click "💾 Save PNG (Transparent)" or "💾 Save PNG (Opaque)"
3. Choose filename in file dialog
4. PNG saved with full metadata

### Batch Export Presets

```bash
# Export all presets to separate PNGs
for config in assets/presets/*.fflame; do
    name=$(basename "$config" .fflame)
    fractal_flame_wgpu export -i "$config" -o "output/$name.png"
done
```

### Extract Config from PNG

```rust
use fractal_flame_wgpu::png_metadata::read_png_metadata;
use fractal_flame_wgpu::config::FractalConfig;

let metadata = read_png_metadata("output.png")?;
let config = FractalConfig::from_json(&metadata.config_json)?;

// Now you can load this config into the app
app.import_config(config);
```

### Verify PNG Integrity

```rust
use fractal_flame_wgpu::png_metadata::read_png_metadata;
use sha2::{Sha256, Digest};

let metadata = read_png_metadata("output.png")?;

// Calculate checksum of config JSON
let mut hasher = Sha256::new();
hasher.update(metadata.config_json.as_bytes());
let calculated = format!("{:x}", hasher.finalize());

// Compare with embedded checksum
assert_eq!(calculated, metadata.config_checksum);
println!("PNG integrity verified!");
```

### Debug Export Issues

Enable verbose logging:

```rust
pub fn capture_from_accumulation_buffer(...) {
    println!("Export starting: {}×{}", width, height);
    println!("Bytes per row: {}", bytes_per_row);

    // ... copy texture

    println!("Texture copied to staging buffer");

    // ... map buffer

    println!("Buffer mapped, reading data");

    // ... encode PNG

    println!("PNG encoded: {}", path.display());
}
```

---

## Performance Tips

### Fast Preview Export

For quick previews without metadata:

```rust
// Disable metadata embedding
renderer.capture_from_tonemap_render(&device, &queue, &path, None)?;
```

### High-Quality Export

For publication-quality images:

1. Let render converge (1 billion+ iterations)
2. Use high `density_scale` (100-200)
3. Enable tone curve (`use_curve = true`)
4. Export at full resolution
5. Use transparent export for compositing flexibility

### Batch Export Optimization

For processing many configs:

1. Reuse GPU instance across exports
2. Batch configs by similar resolution (avoid buffer recreation)
3. Use `--speed-multiplier` for consistent quality
4. Consider parallel processing (multiple GPU contexts)

---

**Last Updated:** 2025-12-18
**Related Docs:** [ARCHITECTURE.md](../ARCHITECTURE.md), [RENDERER.md](RENDERER.md), [CONFIG.md](CONFIG.md), [TESTING-GUIDE.md](../TESTING-GUIDE.md)
