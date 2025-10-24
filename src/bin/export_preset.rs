//! Export fractal flame presets to PNG
//!
//! Usage:
//!   export_preset --preset Simple --output simple.png [OPTIONS]
//!
//! This tool renders flames deterministically for testing and comparison.

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "export_preset")]
#[command(about = "Export fractal flame presets to PNG", long_about = None)]
struct Args {
    /// Preset name to render (e.g. "Simple", "Fern", "Dragon")
    #[arg(short, long)]
    preset: String,

    /// Output PNG file path
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

    /// Enable tone curve
    #[arg(long)]
    use_curve: bool,

    /// Tone curve preset (Linear, S-Curve, BrightenShadows, DarkenHighlights)
    #[arg(long, default_value = "Linear")]
    curve_preset: String,

    /// Transparent background (instead of black)
    #[arg(short, long)]
    transparent: bool,

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
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();
    }

    println!("Fractal Flame Preset Exporter");
    println!("==============================");
    println!("Preset: {}", args.preset);
    println!("Output: {}", args.output);
    println!("Size: {}x{}", args.width, args.height);
    println!("Frames: {}", args.frames);
    println!("Tone Curve: {} ({})", if args.use_curve { "enabled" } else { "disabled" }, args.curve_preset);
    println!("Background: {}", if args.transparent { "transparent" } else { "opaque" });
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
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to find suitable GPU adapter"))?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Export Device"),
                required_features: wgpu::Features::CLEAR_TEXTURE,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        )
        .await?;

    println!("GPU: {}", adapter.get_info().name);
    println!();

    // Load preset
    println!("Loading preset '{}'...", args.preset);
    use fractal_flame_wgpu::scene::presets::PresetLibrary;
    let preset_library = PresetLibrary::new();

    // Find preset by name
    let config = preset_library
        .presets()
        .iter()
        .find(|p| p.flame.name == args.preset)
        .ok_or_else(|| anyhow::anyhow!("Preset '{}' not found", args.preset))?
        .clone();

    println!("Preset loaded: {} ({} transforms)", config.flame.name, config.flame.transforms.len());
    println!();

    // Set up tone curve
    use fractal_flame_wgpu::scene::tonemap::ToneCurve;
    let tonemap_curve = match args.curve_preset.as_str() {
        "Linear" => ToneCurve::linear(),
        "S-Curve" | "SCurve" => ToneCurve::s_curve(),
        "BrightenShadows" => ToneCurve::brighten_shadows(),
        "DarkenHighlights" => ToneCurve::darken_highlights(),
        _ => {
            eprintln!("Warning: Unknown curve preset '{}', using Linear", args.curve_preset);
            ToneCurve::linear()
        }
    };

    // Create renderer
    println!("Creating renderer...");
    use fractal_flame_wgpu::renderer::compute_kernel::FlameRenderer;
    let mut renderer = FlameRenderer::new(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8Unorm,
        args.width,
        args.height,
        &config.flame,
    );

    // Apply config settings
    let iterations_per_thread = 4096; // Default iteration count per thread (higher = better quality)
    renderer.update_iterations(
        &queue,
        iterations_per_thread,
        config.zoom,
        config.pan_x,
        config.pan_y,
        config.rotation,
        config.camera_rotation_x,
        config.camera_rotation_y,
        config.speed_factor,
    );

    renderer.set_color_mode(
        &queue,
        config.color_mode,
        iterations_per_thread,
        config.zoom,
        config.pan_x,
        config.pan_y,
        config.rotation,
        config.camera_rotation_x,
        config.camera_rotation_y,
        config.speed_factor,
    );

    // Load palette
    use fractal_flame_wgpu::scene::palette::PaletteLibrary;
    let palette_library = PaletteLibrary::new();
    let palette = palette_library.get(config.palette_index)
        .ok_or_else(|| anyhow::anyhow!("Palette index {} not found", config.palette_index))?;
    renderer.update_palette(&device, &queue, palette);
    renderer.update_density_scale(&queue, config.density_scale);

    // Use --transparent flag to control background mode
    // Black [0,0,0] triggers transparent mode (alpha = density)
    // Non-black triggers opaque mode (alpha = 1.0, blended with background)
    let background_color = if args.transparent {
        [0.0, 0.0, 0.0]  // Transparent mode
    } else {
        config.background_color  // Use preset's background (usually black but forces opaque)
    };
    // If preset has black background and not in transparent mode, use tiny epsilon to force opaque
    let background_color = if !args.transparent && background_color == [0.0, 0.0, 0.0] {
        [0.001, 0.001, 0.001]  // Force opaque mode with nearly-black background
    } else {
        background_color
    };
    renderer.update_background_color(&queue, background_color);

    // Update tone curve if enabled
    use fractal_flame_wgpu::scene::tonemap::ToneMapMode;
    renderer.update_tonemap(&queue, ToneMapMode::Logarithmic, args.use_curve, 1.0, 2.2);
    if args.use_curve {
        renderer.update_curve_lut(&queue, &tonemap_curve);
    }

    println!("Rendering {} frames...", args.frames);

    // Render frames
    for frame in 0..args.frames {
        if args.verbose || frame % 10 == 0 || frame == args.frames - 1 {
            let progress = (frame + 1) as f32 / args.frames as f32 * 100.0;
            println!("  Frame {}/{} ({:.1}%)", frame + 1, args.frames, progress);
        }

        // Create encoder for this frame
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Run compute pass (trajectory calculation)
        renderer.compute_pass(
            &mut encoder,
            &queue,
            128, // num_workgroups
            iterations_per_thread,
            config.zoom,
            config.pan_x,
            config.pan_y,
            config.rotation,
            config.camera_rotation_x,
            config.camera_rotation_y,
            config.speed_factor,
        );

        // Accumulate samples
        renderer.accumulate_pass(&mut encoder, &queue, &device, iterations_per_thread as u64);

        queue.submit(std::iter::once(encoder.finish()));
    }

    println!();
    println!("Rendering complete! Total iterations: {}", renderer.total_iterations());
    println!("Exporting to PNG...");

    // Create final tonemap encoder
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Export Encoder"),
    });

    // Create output texture for tonemap
    let output_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Output Texture"),
        size: wgpu::Extent3d {
            width: args.width,
            height: args.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Run tonemap pass
    renderer.tonemap_pass(&mut encoder, &output_view);

    // Copy to buffer for reading
    let bytes_per_pixel = 4; // RGBA8
    let unpadded_bytes_per_row = args.width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
    let buffer_size = (padded_bytes_per_row * args.height) as u64;

    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &output_texture,
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
            width: args.width,
            height: args.height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));

    // Read buffer
    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = futures::channel::oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });
    device.poll(wgpu::Maintain::Wait);
    rx.await??;

    let data = buffer_slice.get_mapped_range();

    // Remove padding and create image
    let mut rgba_data = Vec::with_capacity((args.width * args.height * 4) as usize);
    for y in 0..args.height {
        let row_start = (y * padded_bytes_per_row) as usize;
        let row_end = row_start + (args.width * 4) as usize;
        rgba_data.extend_from_slice(&data[row_start..row_end]);
    }

    drop(data);
    staging_buffer.unmap();

    // Save PNG
    use fractal_flame_wgpu::renderer::compute_kernel::encode_png_from_rgba;
    let png_bytes = encode_png_from_rgba(args.width, args.height, rgba_data, None)
        .map_err(|e| anyhow::anyhow!("PNG encoding failed: {}", e))?;

    std::fs::write(&args.output, png_bytes)?;

    println!("Saved to: {}", args.output);
    println!("Done!");

    Ok(())
}
