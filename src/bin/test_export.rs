//! Batch export test flames to PNG with metadata
//!
//! Usage:
//!   test_export --input tests/visual/configs --output tests/visual/current [OPTIONS]
//!
//! This tool renders .flame files for visual regression testing.

use anyhow::Result;
use clap::Parser;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "test_export")]
#[command(about = "Batch export test flames to PNG with metadata", long_about = None)]
struct Args {
    /// Input directory containing .flame files
    #[arg(short, long)]
    input: String,

    /// Output directory for PNG files
    #[arg(short, long)]
    output: String,

    /// Image width in pixels
    #[arg(short, long, default_value = "1920")]
    width: u32,

    /// Image height in pixels
    #[arg(short = 'H', long, default_value = "1080")]
    height: u32,

    /// Number of frames to render (more = better quality)
    #[arg(short, long, default_value = "100")]
    frames: u32,

    /// Number of workgroups per frame
    #[arg(long, default_value = "128")]
    workgroups: u32,

    /// Iterations per thread per frame
    #[arg(long, default_value = "256")]
    iterations: u32,

    /// Test category (for metadata)
    #[arg(short = 'c', long)]
    category: Option<String>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[pollster::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    if args.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    }

    println!("Fractal Flame Visual Test Exporter");
    println!("===================================");
    println!("Input: {}", args.input);
    println!("Output: {}", args.output);
    println!("Size: {}x{}", args.width, args.height);
    println!("Frames: {} ({}×{} workgroups, {} iterations)",
        args.frames, args.workgroups, args.iterations, args.workgroups * args.iterations);
    println!();

    // Create output directory
    std::fs::create_dir_all(&args.output)?;

    // Find all .flame files in input directory
    let flame_files = find_flame_files(&args.input)?;

    if flame_files.is_empty() {
        eprintln!("No .flame files found in {}", args.input);
        return Ok(());
    }

    println!("Found {} .flame files", flame_files.len());
    println!();

    // Create headless GPU instance
    println!("Initializing GPU...");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to find GPU adapter"))?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Test Export Device"),
                required_features: wgpu::Features::CLEAR_TEXTURE,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        )
        .await?;

    println!("GPU initialized: {}", adapter.get_info().name);
    println!();

    // Process each flame file
    let mut success_count = 0;
    let mut error_count = 0;

    for (idx, flame_path) in flame_files.iter().enumerate() {
        let file_name = flame_path.file_stem().unwrap().to_string_lossy();
        print!("[{}/{}] Rendering {}... ", idx + 1, flame_files.len(), file_name);
        std::io::Write::flush(&mut std::io::stdout()).unwrap();

        let render_start = Instant::now();

        match render_flame_file(
            &device,
            &queue,
            flame_path,
            &args,
            &file_name,
        ).await {
            Ok(output_path) => {
                let elapsed = render_start.elapsed();
                println!("✓ {:.2}s → {}", elapsed.as_secs_f64(), output_path.display());
                success_count += 1;
            }
            Err(e) => {
                println!("✗ Error: {}", e);
                error_count += 1;
            }
        }
    }

    println!();
    println!("Summary:");
    println!("  Success: {}", success_count);
    println!("  Errors:  {}", error_count);
    println!("  Total:   {}", flame_files.len());

    Ok(())
}

/// Find all .flame files in directory (recursive)
fn find_flame_files(dir: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let path = Path::new(dir);

    if !path.exists() {
        return Err(anyhow::anyhow!("Directory does not exist: {}", dir));
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "flame" {
                    files.push(path);
                }
            }
        } else if path.is_dir() {
            // Recurse into subdirectories
            if let Ok(mut sub_files) = find_flame_files(path.to_str().unwrap()) {
                files.append(&mut sub_files);
            }
        }
    }

    files.sort();
    Ok(files)
}

/// Render a single .flame file to PNG with metadata
async fn render_flame_file(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    flame_path: &Path,
    args: &Args,
    file_name: &str,
) -> Result<PathBuf> {
    use fractal_flame_wgpu::config::FractalConfig;
    use fractal_flame_wgpu::renderer::compute_kernel::FlameRenderer;
    use fractal_flame_wgpu::png_metadata::PngMetadata;
    use fractal_flame_wgpu::scene::palette::PaletteLibrary;

    // Load config from file
    let config_json = std::fs::read_to_string(flame_path)?;
    let config: FractalConfig = serde_json::from_str(&config_json)
        .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", flame_path.display(), e))?;

    // Create renderer with loaded flame
    let surface_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut renderer = FlameRenderer::new(
        device,
        queue,
        surface_format,
        args.width,
        args.height,
        &config.flame,
    );

    // Apply config settings
    renderer.update_iterations(
        queue,
        args.iterations,
        config.zoom,
        config.pan_x,
        config.pan_y,
        config.rotation,
        config.camera_rotation_x,
        config.camera_rotation_y,
        config.speed_factor,
    );

    renderer.set_color_mode(
        queue,
        config.color_mode,
        args.iterations,
        config.zoom,
        config.pan_x,
        config.pan_y,
        config.rotation,
        config.camera_rotation_x,
        config.camera_rotation_y,
        config.speed_factor,
    );

    // Load and apply palette
    let palette_library = PaletteLibrary::new();
    let palette = palette_library.get(config.palette_index)
        .ok_or_else(|| anyhow::anyhow!("Palette index {} not found", config.palette_index))?;
    renderer.update_palette(device, queue, palette);
    renderer.update_density_scale(queue, config.density_scale);
    renderer.update_background_color(queue, config.background_color);

    // Create encoder for rendering
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Test Export Encoder"),
    });

    // Render frames progressively
    let render_start = Instant::now();

    for _ in 0..args.frames {
        renderer.compute_pass(
            &mut encoder,
            queue,
            args.workgroups,
            args.iterations,
            config.zoom,
            config.pan_x,
            config.pan_y,
            config.rotation,
            config.camera_rotation_x,
            config.camera_rotation_y,
            config.speed_factor,
        );

        let samples = args.workgroups * args.iterations;
        renderer.accumulate_pass(&mut encoder, queue, device, samples as u64);
    }

    // Apply tone mapping
    renderer.update_tonemap(queue, config.tonemap_mode, true, config.exposure, config.gamma);
    renderer.update_curve_lut(queue, &config.tonemap_curve);

    // Submit all GPU commands
    queue.submit(Some(encoder.finish()));

    // Wait for GPU to finish
    device.poll(wgpu::Maintain::Wait);

    let render_time_ms = render_start.elapsed().as_secs_f64() * 1000.0;

    // Capture pixels
    let (width, height, rgba_data) = capture_pixels(device, queue, &renderer, surface_format).await?;

    // Build metadata
    let mut metadata = PngMetadata::from_app_state(
        width,
        height,
        renderer.total_iterations(),
        render_time_ms,
        &config,
    );

    // Add test metadata
    metadata.test_name = Some(file_name.to_string());
    metadata.test_category = args.category.clone();

    // Encode PNG with metadata
    let png_bytes = fractal_flame_wgpu::png_metadata::encode_png_with_metadata(
        width,
        height,
        rgba_data,
        &metadata,
    ).map_err(|e| anyhow::anyhow!("{}", e))?;

    // Save to output directory
    let output_path = Path::new(&args.output).join(format!("{}.png", file_name));
    std::fs::write(&output_path, png_bytes)?;

    Ok(output_path)
}

/// Capture rendered pixels from GPU
async fn capture_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &fractal_flame_wgpu::renderer::compute_kernel::FlameRenderer,
    surface_format: wgpu::TextureFormat,
) -> Result<(u32, u32, Vec<u8>)> {
    // Create staging texture for readback
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Capture Texture"),
        size: wgpu::Extent3d {
            width: renderer.width,
            height: renderer.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: surface_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Render to texture
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Capture Encoder"),
    });

    renderer.tonemap_pass(&mut encoder, &texture_view);

    // Copy texture to staging buffer
    let padded_bytes_per_row = {
        let bytes_per_row = renderer.width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        ((bytes_per_row + align - 1) / align) * align
    };

    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Capture Staging Buffer"),
        size: (padded_bytes_per_row * renderer.height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: renderer.width,
            height: renderer.height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    // Map buffer and read pixels
    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = futures::channel::oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });

    device.poll(wgpu::Maintain::Wait);
    rx.await??;

    // Copy to RGBA buffer (strip padding)
    let data = buffer_slice.get_mapped_range();
    let mut rgba_data = Vec::with_capacity((renderer.width * renderer.height * 4) as usize);

    for y in 0..renderer.height {
        let row_start = (y * padded_bytes_per_row) as usize;
        let row_end = row_start + (renderer.width * 4) as usize;
        rgba_data.extend_from_slice(&data[row_start..row_end]);
    }

    drop(data);
    staging_buffer.unmap();

    Ok((renderer.width, renderer.height, rgba_data))
}
