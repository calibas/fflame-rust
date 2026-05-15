//! Export functions for headless rendering
//!
//! This module provides platform-specific device creation and delegates
//! actual rendering to the unified `renderer::render()` API.

use crate::config::FractalConfig;
use crate::renderer::{render, NoProgress, RenderJob};

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
            instance
                .request_adapter(&fallback_options)
                .await
                .map_err(|e| format!("Failed to find GPU adapter: {:?}", e))?
        }
    };

    let adapter_info = adapter.get_info();
    log::info!(
        "WASM Export: Adapter: {} (backend: {:?})",
        adapter_info.name,
        adapter_info.backend
    );

    // Use WebGL2-compatible limits with storage buffer override
    let adapter_limits = adapter.limits();
    let mut limits = egui_wgpu::wgpu::Limits::downlevel_webgl2_defaults();
    limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
    // Compute bind group needs 10 storage buffers per stage
    // (subflame_transforms + subflame_metadata pushed the count to 10).
    limits.max_storage_buffers_per_shader_stage =
        adapter_limits.max_storage_buffers_per_shader_stage;

    let (device, queue) = adapter
        .request_device(&egui_wgpu::wgpu::DeviceDescriptor {
            label: Some("WASM Headless Device"),
            required_features: egui_wgpu::wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            memory_hints: egui_wgpu::wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        })
        .await
        .map_err(|e| format!("Failed to create device: {:?}", e))?;

    // Use unified render API
    let job = RenderJob::new(config, width, height)
        .with_iterations_per_thread(iterations_per_thread)
        .with_transparent(transparent);

    let result = render(&device, &queue, job, &mut NoProgress)
        .await
        .map_err(|e| e.to_string())?;

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
    crate::renderer::compute_kernel::encode_png_from_rgba(
        result.width,
        result.height,
        result.rgba_data,
        Some(metadata),
    )
    .map_err(|e| format!("Failed to encode PNG: {}", e))
}

/// Headless PNG export for CLI mode.
///
/// Routes between FlameRenderer's direct-histogram path (the
/// interactive renderer, single GPU buffer) and HighResExporter's
/// sample-emit path (GPU compute samples → CPU histogram for
/// resolutions whose histogram exceeds one storage-buffer binding).
/// Decision is made against the *adapter's actual*
/// `max_storage_buffer_binding_size`, not the WebGPU spec floor of
/// 128 MB — modern desktops report 2 GB+ which means 8K renders
/// (1 GB histogram) can take the fast direct-histogram path.
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
    let max_binding = probe_max_binding_size().await
        .unwrap_or(128 * 1024 * 1024);
    let hist_size = crate::export::histogram_size_bytes(width, height);

    if hist_size > max_binding {
        log::info!(
            "Routing through HighResExporter for {}x{} (histogram {} MB > device binding limit {} MB)",
            width, height,
            hist_size / (1024 * 1024),
            max_binding / (1024 * 1024)
        );
        return export_headless_cpu(
            config,
            output_path,
            width,
            height,
            test_category,
            iterations_per_thread,
            transparent,
        )
        .await;
    }

    log::info!(
        "Routing through FlameRenderer for {}x{} (histogram {} MB ≤ device binding limit {} MB)",
        width, height,
        hist_size / (1024 * 1024),
        max_binding / (1024 * 1024)
    );
    export_headless_gpu(
        config,
        output_path,
        width,
        height,
        test_category,
        iterations_per_thread,
        transparent,
    )
    .await
}

/// Probe the default adapter to discover how large a single storage
/// buffer binding it supports. The export router uses this to decide
/// whether the full-resolution histogram fits in one binding (use
/// FlameRenderer) or has to be tile-fallbacked (use HighResExporter).
#[cfg(not(target_arch = "wasm32"))]
async fn probe_max_binding_size() -> Option<u64> {
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
        .ok()?;
    Some(adapter.limits().max_storage_buffer_binding_size as u64)
}

/// GPU-based export using unified render API
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

    // Request the adapter's actual storage-buffer + total-buffer
    // + texture-dimension limits. The WebGPU spec defaults are
    // 128 MB binding, 256 MB total, 8K texture dim — those force
    // 4K+ histograms to fail allocation and 8K+ images to fail
    // texture creation, even on desktops that natively support
    // gigabyte buffers and 16K+ textures. Mirrors the limits
    // expansion HighResExporter::new does for the same reason.
    let adapter_limits = adapter.limits();
    let mut limits = egui_wgpu::wgpu::Limits::default();
    limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
    limits.max_buffer_size = adapter_limits.max_buffer_size;
    limits.max_texture_dimension_2d = adapter_limits.max_texture_dimension_2d;
    // Compute bind group needs 10 storage buffers (see gpu/device.rs
    // for the running count); WebGPU spec floor is 8.
    limits.max_storage_buffers_per_shader_stage =
        adapter_limits.max_storage_buffers_per_shader_stage;

    let (device, queue) = adapter
        .request_device(&egui_wgpu::wgpu::DeviceDescriptor {
            label: Some("Headless Device"),
            required_features: egui_wgpu::wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            memory_hints: egui_wgpu::wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        })
        .await?;

    // Use unified render API
    let job = RenderJob::new(config, width, height)
        .with_iterations_per_thread(iterations_per_thread)
        .with_transparent(transparent);

    let result = render(&device, &queue, job, &mut NoProgress)
        .await
        .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;

    // Progress indicator (desktop only)
    println!(
        "\r  Progress: {}/{} (100.0%)",
        result.total_iterations, config.max_iterations
    );

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
        result.width,
        result.height,
        result.rgba_data,
        Some(metadata),
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
    transparent: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    use crate::export::{CliExportProgress, HighResExporter};
    use std::time::Instant;

    let export_start = Instant::now();

    // Create exporter
    let mut exporter = HighResExporter::new(config, width, height, None).await?;

    // Calculate total iterations
    let total_iterations = config.max_iterations;

    // Run export with progress reporting
    let mut progress = CliExportProgress;
    let rgba_data = exporter
        .export(config, total_iterations, transparent, &mut progress)
        .await?;

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
    let png_data =
        crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata))?;

    // Save to file
    std::fs::write(output_path, png_data)?;

    Ok(true)
}
