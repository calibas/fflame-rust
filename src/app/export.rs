use crate::config::FractalConfig;

/// Headless PNG export for CLI mode
#[cfg(not(target_arch = "wasm32"))]
pub async fn export_headless(
    config: &FractalConfig,
    output_path: &std::path::Path,
    width: u32,
    height: u32,
    test_category: Option<String>,
    iterations_per_thread: u32,
) -> Result<bool, Box<dyn std::error::Error>> {
    use crate::renderer::compute_kernel::FlameRenderer;
    use crate::scene::palette::PaletteLibrary;
    use std::time::Instant;

    // Create headless GPU instance
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
        .ok_or("Failed to find adapter")?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Headless Device"),
                required_features: wgpu::Features::CLEAR_TEXTURE,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )
        .await?;

    // Create renderer
    let surface_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut renderer = FlameRenderer::new(&device, &queue, surface_format, width, height, &config.flame);

    // Load config into renderer
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Headless Export Encoder"),
    });

    // Get palette
    let palette_library = PaletteLibrary::new();
    let palette = config.palette.as_ref()
        .or_else(|| palette_library.get(config.palette_index))
        .ok_or("No palette found")?;

    renderer.load_config(&device, &mut encoder, &queue, config, palette, iterations_per_thread);

    queue.submit(std::iter::once(encoder.finish()));

    // Render until max_iterations
    let render_start = Instant::now();
    let mut total_rendered = 0u64;
    let target = config.max_iterations;

    while total_rendered < target {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Frame"),
        });

        renderer.compute_pass(
            &mut encoder,
            &queue,
            128,  // workgroups
            iterations_per_thread,
            config.zoom,
            config.pan_x,
            config.pan_y,
            config.rotation,
            config.camera_rotation_x,
            config.camera_rotation_y,
            config.speed_factor,
        );

        let samples = 128 * iterations_per_thread as u64;
        renderer.accumulate_pass(&mut encoder, &queue, &device, samples);

        queue.submit(std::iter::once(encoder.finish()));

        total_rendered += samples;

        // Progress indicator every 10M iterations
        if total_rendered % 10_000_000 < samples {
            print!("\r  Progress: {}/{} ({:.1}%)", total_rendered, target, (total_rendered as f64 / target as f64) * 100.0);
            use std::io::Write;
            std::io::stdout().flush().ok();
        }
    }

    println!("\r  Progress: {}/{} (100.0%)", total_rendered, target);

    let render_time_ms = render_start.elapsed().as_secs_f64() * 1000.0;

    // Capture pixels
    let (width, height, rgba_data) = renderer.capture_pixels(&device, &queue, false, surface_format).await?;

    // Build metadata
    let mut metadata = crate::png_metadata::PngMetadata::from_app_state(
        width,
        height,
        total_rendered,
        render_time_ms,
        config,
    );
    metadata.test_category = test_category;

    // Encode PNG
    let png_data = crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata))?;

    // Save to file
    std::fs::write(output_path, png_data)?;

    Ok(true)
}
