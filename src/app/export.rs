use crate::config::FractalConfig;

/// Headless PNG export - WASM version
#[cfg(target_arch = "wasm32")]
pub async fn export_headless_wasm(
    config: &FractalConfig,
    width: u32,
    height: u32,
    iterations_per_thread: u32,
    _speed_multiplier: u32, // Reserved for future use
) -> Result<Vec<u8>, String> {
    use crate::renderer::compute_kernel::FlameRenderer;
    use crate::scene::palette::PaletteLibrary;

    // Create headless GPU instance
    // WebGL not supported - compute shaders required for fractal generation
    let instance = egui_wgpu::wgpu::Instance::new(&egui_wgpu::wgpu::InstanceDescriptor {
        backends: egui_wgpu::wgpu::Backends::BROWSER_WEBGPU,
        ..Default::default()
    });

    // Try high-performance adapter first, then fallback
    let adapter_options = egui_wgpu::wgpu::RequestAdapterOptions {
        power_preference: egui_wgpu::wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    };

    let adapter = instance.request_adapter(&adapter_options).await;

    let adapter = match adapter {
        Ok(a) => a,
        Err(_) => {
            // Try again with fallback adapter
            let fallback_options = egui_wgpu::wgpu::RequestAdapterOptions {
                power_preference: egui_wgpu::wgpu::PowerPreference::default(),
                force_fallback_adapter: true,
                compatible_surface: None,
            };
            instance.request_adapter(&fallback_options)
                .await
                .map_err(|e| format!("Failed to find GPU adapter: {:?}", e))?
        }
    };

    let (device, queue) = adapter
        .request_device(
            &egui_wgpu::wgpu::DeviceDescriptor {
                label: Some("WASM Headless Device"),
                required_features: egui_wgpu::wgpu::Features::CLEAR_TEXTURE,
                // Use WebGL2-compatible limits for WASM/WebGPU compatibility
                required_limits: egui_wgpu::wgpu::Limits::downlevel_webgl2_defaults(),
                memory_hints: egui_wgpu::wgpu::MemoryHints::Performance,
                experimental_features: Default::default(),
                trace: Default::default(),
            },
        )
        .await
        .map_err(|e| format!("Failed to create device: {:?}", e))?;

    // Create renderer
    let surface_format = egui_wgpu::wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = FlameRenderer::new(&device, &queue, surface_format, width, height, &config.flame);

    // Load config into renderer
    let mut encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
        label: Some("WASM Export Encoder"),
    });

    // Get palette
    let palette_library = PaletteLibrary::new();
    let palette = config.palette.as_ref()
        .or_else(|| palette_library.get(config.palette_index))
        .ok_or("No palette found")?;

    renderer.load_config(&device, &mut encoder, &queue, config, palette, iterations_per_thread);

    queue.submit(std::iter::once(encoder.finish()));

    // Render until max_iterations with chunked accumulation for consistent quality
    let mut total_rendered = 0u64;
    let target = config.max_iterations;

    const NUM_WORKGROUPS: u32 = 128;
    const THREADS_PER_WORKGROUP: u64 = 64;
    const BATCH_SIZE: u32 = 4;

    let mut batch_frame_count = 0;

    // Track render time
    let start_time = web_time::Instant::now();

    while total_rendered < target {
        let mut encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Render Frame"),
        });

        let clear_histogram = batch_frame_count == 0;

        renderer.compute_pass(
            &mut encoder,
            &queue,
            NUM_WORKGROUPS,
            iterations_per_thread,
            config.zoom,
            config.pan_x,
            config.pan_y,
            config.rotation,
            config.camera_rotation_x,
            config.camera_rotation_y,
            config.camera_z,
            config.speed_factor,
            clear_histogram,
        );

        let samples_this_frame = NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * iterations_per_thread as u64;
        total_rendered += samples_this_frame;
        batch_frame_count += 1;

        let should_accumulate = batch_frame_count >= BATCH_SIZE;
        if should_accumulate {
            let total_samples_in_batch = samples_this_frame * BATCH_SIZE as u64;
            renderer.accumulate_pass(&mut encoder, &queue, &device, total_samples_in_batch);
            batch_frame_count = 0;
        }

        queue.submit(std::iter::once(encoder.finish()));

        if total_rendered >= target {
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
    }

    // Render tonemap pass
    let mut final_encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
        label: Some("Final Tonemap"),
    });
    renderer.tonemap_pass(&mut final_encoder);
    queue.submit(std::iter::once(final_encoder.finish()));

    // Capture pixels
    let (width, height, rgba_data) = renderer.read_fractal_pixels(&device, &queue, false, config.background_color)
        .await
        .map_err(|e| format!("Failed to read pixels: {}", e))?;

    // Calculate render time
    let render_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    // Build metadata
    let metadata = crate::png_metadata::PngMetadata::from_app_state(
        width,
        height,
        total_rendered,
        render_time_ms,
        iterations_per_thread,
        config.speed_factor,
        config,
    );

    // Encode PNG
    let png_data = crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata))
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(png_data)
}

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

    // Start timing from the very beginning (includes all overhead)
    let export_start = Instant::now();

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

    // Render tonemap pass to fractal_texture before reading pixels
    let mut final_encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
        label: Some("Final Tonemap"),
    });
    renderer.tonemap_pass(&mut final_encoder);
    queue.submit(std::iter::once(final_encoder.finish()));

    // Capture pixels from fractal_texture (what was actually rendered and displayed)
    let (width, height, rgba_data) = renderer.read_fractal_pixels(&device, &queue, false, config.background_color).await?;

    // Encode PNG
    let png_data = crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data.clone(), None)?;

    // Save to file
    std::fs::write(output_path, &png_data)?;

    // Calculate total export time (includes device creation, rendering, tonemap, pixel readback, PNG encoding, file write)
    let total_export_time_ms = export_start.elapsed().as_secs_f64() * 1000.0;

    // Build metadata with total export time
    let mut metadata = crate::png_metadata::PngMetadata::from_app_state(
        width,
        height,
        total_rendered,
        total_export_time_ms,
        iterations_per_thread,
        config.speed_factor,
        config,
    );
    metadata.test_category = test_category;

    // Re-encode PNG with metadata
    let png_data_with_metadata = crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata))?;

    // Overwrite file with metadata version
    std::fs::write(output_path, png_data_with_metadata)?;

    Ok(true)
}
