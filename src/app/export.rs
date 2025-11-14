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
    speed_multiplier: u32,
) -> Result<bool, Box<dyn std::error::Error>> {
    use crate::renderer::compute_kernel::FlameRenderer;
    use crate::scene::palette::PaletteLibrary;
    use std::time::Instant;

    // Create headless GPU instance
    let instance = egui_wgpu::wgpu::Instance::new(&egui_wgpu::wgpu::InstanceDescriptor {
        backends: egui_wgpu::wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&egui_wgpu::wgpu::RequestAdapterOptions {
            power_preference: egui_wgpu::wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .map_err(|e| format!("Failed to find adapter: {:?}", e))?;

    let (device, queue) = adapter
        .request_device(
            &egui_wgpu::wgpu::DeviceDescriptor {
                label: Some("Headless Device"),
                required_features: egui_wgpu::wgpu::Features::CLEAR_TEXTURE,
                required_limits: egui_wgpu::wgpu::Limits::default(),
                memory_hints: egui_wgpu::wgpu::MemoryHints::Performance,
                experimental_features: Default::default(),
                trace: Default::default(),
            },
        )
        .await?;

    // Create renderer
    let surface_format = egui_wgpu::wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = FlameRenderer::new(&device, &queue, surface_format, width, height, &config.flame);

    // Load config into renderer
    let mut encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
        label: Some("Headless Export Encoder"),
    });

    // Get palette
    let palette_library = PaletteLibrary::new();
    let palette = config.palette.as_ref()
        .or_else(|| palette_library.get(config.palette_index))
        .ok_or("No palette found")?;

    renderer.load_config(&device, &mut encoder, &queue, config, palette, iterations_per_thread);

    queue.submit(std::iter::once(encoder.finish()));

    // Render until max_iterations with chunked accumulation for consistent quality
    let render_start = Instant::now();
    let mut total_rendered = 0u64;
    let target = config.max_iterations;

    const NUM_WORKGROUPS: u32 = 128;
    const THREADS_PER_WORKGROUP: u64 = 64;
    const BATCH_SIZE: u32 = 4; // Match viewport's accumulation_batch_size

    let mut batch_frame_count = 0;

    while total_rendered < target {
        let mut encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Render Frame"),
        });

        // Clear histogram only on first frame of batch (match viewport behavior)
        let clear_histogram = batch_frame_count == 0;

        // Use FULL iterations_per_thread like viewport does (not divided by speed_multiplier)
        renderer.compute_pass(
            &mut encoder,
            &queue,
            NUM_WORKGROUPS,
            iterations_per_thread, // CHANGED: Use full value like viewport
            config.zoom,
            config.pan_x,
            config.pan_y,
            config.rotation,
            config.camera_rotation_x,
            config.camera_rotation_y,
            config.camera_z,
            config.speed_factor,
            clear_histogram, // CHANGED: Conditional clear like viewport
        );

        let samples_this_frame = NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * iterations_per_thread as u64;
        total_rendered += samples_this_frame;
        batch_frame_count += 1;

        // Accumulate only when batch is complete (match viewport behavior)
        let should_accumulate = batch_frame_count >= BATCH_SIZE;
        if should_accumulate {
            // Pass total samples in batch like viewport does (multiply by batch size)
            let total_samples_in_batch = samples_this_frame * BATCH_SIZE as u64;
            renderer.accumulate_pass(&mut encoder, &queue, &device, total_samples_in_batch);

            batch_frame_count = 0; // Reset batch counter
        }

        queue.submit(std::iter::once(encoder.finish()));

        if total_rendered >= target {
            // Final accumulation if we have partial batch
            if batch_frame_count > 0 {
                let mut final_accum_encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                    label: Some("Final Batch Accumulation"),
                });
                let total_samples_in_batch = samples_this_frame * batch_frame_count as u64;
                renderer.accumulate_pass(&mut final_accum_encoder, &queue, &device, total_samples_in_batch);
                queue.submit(std::iter::once(final_accum_encoder.finish()));
            }
            break;
        }

        // Progress indicator every 10M iterations
        if total_rendered % 10_000_000 < (NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * iterations_per_thread as u64) {
            print!("\r  Progress: {}/{} ({:.1}%)", total_rendered, target, (total_rendered as f64 / target as f64) * 100.0);
            use std::io::Write;
            std::io::stdout().flush().ok();
        }
    }

    println!("\r  Progress: {}/{} (100.0%)", total_rendered, target);

    let render_time_ms = render_start.elapsed().as_secs_f64() * 1000.0;

    // Render tonemap pass to fractal_texture before reading pixels
    let mut final_encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
        label: Some("Final Tonemap"),
    });
    renderer.tonemap_pass(&mut final_encoder);
    queue.submit(std::iter::once(final_encoder.finish()));

    // Capture pixels from fractal_texture (what was actually rendered and displayed)
    let (width, height, rgba_data) = renderer.read_fractal_pixels(&device, &queue, false, config.background_color).await?;

    // Build metadata
    let mut metadata = crate::png_metadata::PngMetadata::from_app_state(
        width,
        height,
        total_rendered,
        render_time_ms,
        iterations_per_thread,
        config.speed_factor,
        config,
    );
    metadata.test_category = test_category;

    // Encode PNG
    let png_data = crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata))?;

    // Save to file
    std::fs::write(output_path, png_data)?;

    Ok(true)
}
