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
        let histogram_size = crate::export::histogram_size_bytes(width, height);
        log::info!(
            "Using CPU histogram export for {}x{} (histogram would be {} MB)",
            width,
            height,
            histogram_size / (1024 * 1024)
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

    // Use standard GPU-based export for smaller resolutions
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

    let (device, queue) = adapter
        .request_device(&egui_wgpu::wgpu::DeviceDescriptor {
            label: Some("Headless Device"),
            required_features: egui_wgpu::wgpu::Features::CLEAR_TEXTURE,
            required_limits: egui_wgpu::wgpu::Limits::default(),
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
