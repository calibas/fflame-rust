//! Thumbnail rendering for gallery previews
//!
//! Renders FractalConfig thumbnails using the same rendering pipeline
//! as the main application, but at a smaller size and fixed iteration count.

use egui_wgpu::wgpu::*;

use crate::config::FractalConfig;
use crate::renderer::compute_kernel::FlameRenderer;
use crate::scene::palette::PaletteLibrary;

/// Thumbnail rendering configuration
pub const THUMBNAIL_SIZE: u32 = 512;
pub const THUMBNAIL_ITERATIONS: u64 = 50_000_000; // 50M iterations
pub const THUMBNAIL_ITERATIONS_PER_THREAD: u32 = 256;

/// Render a thumbnail for a FractalConfig
///
/// This creates a temporary FlameRenderer at thumbnail resolution,
/// renders the config, and returns the RGBA image data.
///
/// Note: This is a blocking operation that takes ~1-2 seconds.
pub fn render_thumbnail(
    device: &Device,
    queue: &Queue,
    config: &FractalConfig,
    palette_library: &PaletteLibrary,
) -> image::RgbaImage {
    // Create thumbnail-sized renderer
    let surface_format = TextureFormat::Rgba8Unorm;
    let mut renderer = FlameRenderer::new(
        device,
        queue,
        surface_format,
        THUMBNAIL_SIZE,
        THUMBNAIL_SIZE,
        &config.flame,
    );

    // Load config into renderer
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Thumbnail Config Encoder"),
    });

    // Get palette from config or library
    let palette = config
        .palette
        .as_ref()
        .or_else(|| palette_library.get(config.palette_index))
        .cloned()
        .unwrap_or_default();

    renderer.load_config(
        device,
        &mut encoder,
        queue,
        config,
        &palette,
        THUMBNAIL_ITERATIONS_PER_THREAD,
    );

    queue.submit(std::iter::once(encoder.finish()));

    // Render loop
    const NUM_WORKGROUPS: u32 = 128;
    const THREADS_PER_WORKGROUP: u64 = 64;
    const BATCH_SIZE: u32 = 4;

    let mut total_rendered = 0u64;
    let mut batch_frame_count = 0;

    while total_rendered < THUMBNAIL_ITERATIONS {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Thumbnail Render Frame"),
        });

        let clear_histogram = batch_frame_count == 0;

        renderer.compute_pass(
            &mut encoder,
            queue,
            NUM_WORKGROUPS,
            THUMBNAIL_ITERATIONS_PER_THREAD,
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

        let samples_this_frame =
            NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * THUMBNAIL_ITERATIONS_PER_THREAD as u64;
        total_rendered += samples_this_frame;
        batch_frame_count += 1;

        let should_accumulate = batch_frame_count >= BATCH_SIZE;
        if should_accumulate {
            let total_samples_in_batch = samples_this_frame * BATCH_SIZE as u64;
            renderer.accumulate_pass(&mut encoder, queue, device, total_samples_in_batch);
            batch_frame_count = 0;
        }

        queue.submit(std::iter::once(encoder.finish()));

        if total_rendered >= THUMBNAIL_ITERATIONS {
            // Final accumulation if we have partial batch
            if batch_frame_count > 0 {
                let mut final_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Thumbnail Final Accumulation"),
                });
                let total_samples_in_batch = samples_this_frame * batch_frame_count as u64;
                renderer.accumulate_pass(&mut final_encoder, queue, device, total_samples_in_batch);
                queue.submit(std::iter::once(final_encoder.finish()));
            }
            break;
        }
    }

    // Tonemap pass
    let mut final_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Thumbnail Tonemap"),
    });
    renderer.tonemap_pass(&mut final_encoder);
    queue.submit(std::iter::once(final_encoder.finish()));

    // Read pixels (blocking)
    let rgba_data = pollster::block_on(async {
        renderer
            .read_fractal_pixels(device, queue, false, config.background_color)
            .await
    });

    match rgba_data {
        Ok((width, height, data)) => {
            image::RgbaImage::from_raw(width, height, data)
                .unwrap_or_else(|| image::RgbaImage::new(THUMBNAIL_SIZE, THUMBNAIL_SIZE))
        }
        Err(e) => {
            log::error!("Failed to read thumbnail pixels: {}", e);
            // Return empty image on error
            image::RgbaImage::new(THUMBNAIL_SIZE, THUMBNAIL_SIZE)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require GPU access and are marked as ignored by default
    // Run with: cargo test -- --ignored

    #[test]
    #[ignore]
    fn test_thumbnail_render() {
        // This test would need GPU access
        // For now, just verify the constants are reasonable
        assert_eq!(THUMBNAIL_SIZE, 512);
        assert!(THUMBNAIL_ITERATIONS >= 10_000_000);
    }
}
