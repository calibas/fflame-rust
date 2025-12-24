use crate::config::FractalConfig;
use crate::renderer::compute_kernel::FlameRenderer;
use crate::scene::palette::PaletteLibrary;

/// Result of rendering a fractal to RGBA pixels
pub struct RenderResult {
    pub width: u32,
    pub height: u32,
    pub rgba_data: Vec<u8>,
    pub total_iterations: u64,
    pub render_time_ms: f64,
}

/// Shared render logic for both WASM and desktop headless export
///
/// This function handles:
/// - Creating the FlameRenderer
/// - Loading config into renderer
/// - Running the render loop (compute_pass + accumulate_pass)
/// - Running tonemap pass
/// - Reading pixels back
///
/// Platform-specific code (device/adapter creation, file I/O) stays in the caller.
pub async fn render_to_pixels(
    device: &egui_wgpu::wgpu::Device,
    queue: &egui_wgpu::wgpu::Queue,
    config: &FractalConfig,
    width: u32,
    height: u32,
    iterations_per_thread: u32,
    transparent: bool,
) -> Result<RenderResult, String> {
    // Track render time
    let start_time = web_time::Instant::now();

    // Create renderer
    log::info!("Export: Creating renderer for {}x{}", width, height);
    let surface_format = egui_wgpu::wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = FlameRenderer::new(device, queue, surface_format, width, height, &config.flame);

    // Load config into renderer
    let mut encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
        label: Some("Export Config Encoder"),
    });

    // Get palette
    let palette_library = PaletteLibrary::new();
    let palette = config.palette.as_ref()
        .or_else(|| palette_library.get(config.palette_index))
        .ok_or("No palette found")?;

    log::info!("Export: Loading config '{}' with {} transform(s), color_mode={:?}, iterations_per_thread={}",
        config.flame.name, config.flame.transforms.len(), config.color_mode, iterations_per_thread);
    renderer.load_config(device, &mut encoder, queue, config, palette, iterations_per_thread, 20);

    queue.submit(std::iter::once(encoder.finish()));

    // Render until max_iterations with chunked accumulation for consistent quality
    let mut total_rendered = 0u64;
    let target = config.max_iterations;

    const NUM_WORKGROUPS: u32 = 128;
    const THREADS_PER_WORKGROUP: u64 = 64;
    const BATCH_SIZE: u32 = 4;

    let mut batch_frame_count = 0;

    log::info!("Export: Starting render loop, target={} iterations", target);

    while total_rendered < target {
        let mut encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Render Frame"),
        });

        let clear_histogram = batch_frame_count == 0;
        // Clear paths only on very first batch of the entire export
        let clear_paths = total_rendered == 0 && clear_histogram;

        renderer.compute_pass(
            &mut encoder,
            queue,
            NUM_WORKGROUPS,
            iterations_per_thread,
            20, // burn_in default
            config.zoom,
            config.pan_x,
            config.pan_y,
            config.rotation,
            config.camera_rotation_x,
            config.camera_rotation_y,
            config.camera_z,
            config.speed_factor,
            clear_histogram,
            clear_paths,
        );

        let samples_this_frame = NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * iterations_per_thread as u64;
        total_rendered += samples_this_frame;
        batch_frame_count += 1;

        let should_accumulate = batch_frame_count >= BATCH_SIZE;
        if should_accumulate {
            let total_samples_in_batch = samples_this_frame * BATCH_SIZE as u64;
            renderer.accumulate_pass(&mut encoder, queue, device, total_samples_in_batch);
            batch_frame_count = 0;
        }

        queue.submit(std::iter::once(encoder.finish()));

        if total_rendered >= target {
            // Final accumulation if we have partial batch
            if batch_frame_count > 0 {
                let mut final_accum_encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                    label: Some("Final Batch Accumulation"),
                });
                let total_samples_in_batch = samples_this_frame * batch_frame_count as u64;
                renderer.accumulate_pass(&mut final_accum_encoder, queue, device, total_samples_in_batch);
                queue.submit(std::iter::once(final_accum_encoder.finish()));
            }
            break;
        }
    }

    log::info!("Export: Render loop complete, total_rendered={}", total_rendered);

    // Set transparent mode if requested
    if transparent {
        renderer.set_transparent_mode(queue, true, config, iterations_per_thread);
    }

    // Render tonemap pass
    let mut final_encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
        label: Some("Final Tonemap"),
    });
    renderer.tonemap_pass(&mut final_encoder);
    queue.submit(std::iter::once(final_encoder.finish()));

    log::info!("Export: Tonemap complete, reading pixels...");

    // Capture pixels
    let (width, height, rgba_data) = renderer.read_fractal_pixels(device, queue, transparent, config.background_color)
        .await
        .map_err(|e| format!("Failed to read pixels: {}", e))?;

    let render_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    log::info!("Export: Complete - {}x{}, {} iterations in {:.1}ms", width, height, total_rendered, render_time_ms);

    Ok(RenderResult {
        width,
        height,
        rgba_data,
        total_iterations: total_rendered,
        render_time_ms,
    })
}

/// Headless PNG export - WASM version
#[cfg(target_arch = "wasm32")]
pub async fn export_headless_wasm(
    config: &FractalConfig,
    width: u32,
    height: u32,
    iterations_per_thread: u32,
    transparent: bool,
) -> Result<Vec<u8>, String> {
    // Create headless GPU instance
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

    let adapter = match instance.request_adapter(&adapter_options).await {
        Ok(a) => {
            log::info!("WASM Export: Got high-performance adapter");
            a
        }
        Err(_) => {
            log::warn!("WASM Export: High-performance adapter not found, trying fallback...");
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

    let adapter_info = adapter.get_info();
    log::info!("WASM Export: Adapter: {} (backend: {:?})", adapter_info.name, adapter_info.backend);

    // Use WebGL2-compatible limits with storage buffer override
    let adapter_limits = adapter.limits();
    let mut limits = egui_wgpu::wgpu::Limits::downlevel_webgl2_defaults();
    limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;

    let (device, queue) = adapter
        .request_device(
            &egui_wgpu::wgpu::DeviceDescriptor {
                label: Some("WASM Headless Device"),
                required_features: egui_wgpu::wgpu::Features::CLEAR_TEXTURE,
                required_limits: limits,
                memory_hints: egui_wgpu::wgpu::MemoryHints::Performance,
                experimental_features: Default::default(),
                trace: Default::default(),
            },
        )
        .await
        .map_err(|e| format!("Failed to create device: {:?}", e))?;

    // Use shared render logic
    let result = render_to_pixels(&device, &queue, config, width, height, iterations_per_thread, transparent).await?;

    // Build metadata
    let metadata = crate::png_metadata::PngMetadata::from_app_state(
        result.width,
        result.height,
        result.total_iterations,
        result.render_time_ms,
        iterations_per_thread,
        config.speed_factor,
        config,
    );

    // Encode PNG
    let png_data = crate::renderer::compute_kernel::encode_png_from_rgba(
        result.width, result.height, result.rgba_data, Some(metadata)
    ).map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(png_data)
}

/// Headless PNG export for CLI mode
///
/// Automatically selects between:
/// - GPU histogram (fast, for resolutions up to ~4000x4000)
/// - CPU histogram (unlimited resolution, for larger exports)
#[cfg(not(target_arch = "wasm32"))]
pub async fn export_headless(
    config: &FractalConfig,
    output_path: &std::path::Path,
    width: u32,
    height: u32,
    test_category: Option<String>,
    iterations_per_thread: u32,
    transparent: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Check if we need CPU-based export for large resolutions
    if crate::export::needs_cpu_export(width, height) {
        let histogram_size = crate::export::calculate_histogram_size(width, height);
        log::info!(
            "Using CPU histogram export for {}x{} (histogram would be {} MB)",
            width, height, histogram_size / (1024 * 1024)
        );
        return export_headless_cpu(config, output_path, width, height, test_category, iterations_per_thread, transparent).await;
    }

    // Use standard GPU-based export for smaller resolutions
    export_headless_gpu(config, output_path, width, height, test_category, iterations_per_thread, transparent).await
}

/// GPU-based export (original implementation, fast but limited by buffer size)
#[cfg(not(target_arch = "wasm32"))]
async fn export_headless_gpu(
    config: &FractalConfig,
    output_path: &std::path::Path,
    width: u32,
    height: u32,
    test_category: Option<String>,
    iterations_per_thread: u32,
    transparent: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    use std::time::Instant;

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

    // Use shared render logic
    let result = render_to_pixels(&device, &queue, config, width, height, iterations_per_thread, transparent).await
        .map_err(|e| Box::<dyn std::error::Error>::from(e))?;

    // Progress indicator (desktop only)
    println!("\r  Progress: {}/{} (100.0%)", result.total_iterations, config.max_iterations);

    // Calculate total export time
    let total_export_time_ms = export_start.elapsed().as_secs_f64() * 1000.0;

    // Build metadata
    let mut metadata = crate::png_metadata::PngMetadata::from_app_state(
        result.width,
        result.height,
        result.total_iterations,
        total_export_time_ms,
        iterations_per_thread,
        config.speed_factor,
        config,
    );
    metadata.test_category = test_category;

    // Encode PNG with metadata
    let png_data = crate::renderer::compute_kernel::encode_png_from_rgba(
        result.width, result.height, result.rgba_data, Some(metadata)
    )?;

    // Save to file
    std::fs::write(output_path, png_data)?;

    Ok(true)
}

/// CPU-based export for large resolutions (histogram on CPU, no buffer size limits)
#[cfg(not(target_arch = "wasm32"))]
async fn export_headless_cpu(
    config: &FractalConfig,
    output_path: &std::path::Path,
    width: u32,
    height: u32,
    test_category: Option<String>,
    iterations_per_thread: u32,
    _transparent: bool, // TODO: implement transparent mode for CPU export
) -> Result<bool, Box<dyn std::error::Error>> {
    use crate::export::{HighResExporter, CliExportProgress};
    use std::time::Instant;

    let export_start = Instant::now();

    // Create exporter
    let mut exporter = HighResExporter::new(config, width, height, None).await?;

    // Calculate total iterations
    let total_iterations = config.max_iterations;

    // Run export with progress reporting
    let mut progress = CliExportProgress;
    let rgba_data = exporter.export(config, total_iterations, &mut progress).await?;

    // Calculate total export time
    let total_export_time_ms = export_start.elapsed().as_secs_f64() * 1000.0;

    // Build metadata
    let mut metadata = crate::png_metadata::PngMetadata::from_app_state(
        width,
        height,
        total_iterations,
        total_export_time_ms,
        iterations_per_thread,
        config.speed_factor,
        config,
    );
    metadata.test_category = test_category;

    // Encode PNG with metadata
    let png_data = crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata))?;

    // Save to file
    std::fs::write(output_path, png_data)?;

    Ok(true)
}
