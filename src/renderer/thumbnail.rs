//! Thumbnail rendering for gallery previews
//!
//! Renders FractalConfig thumbnails using the unified render API,
//! at a smaller size and fixed iteration count.
//!
//! On desktop: Uses pollster::block_on() for synchronous rendering
//! On WASM: Uses async rendering with spawn_local()

use egui_wgpu::wgpu::*;

use crate::config::FractalConfig;
use crate::renderer::{render, NoProgress, RenderJob};

/// Thumbnail rendering configuration
pub const THUMBNAIL_SIZE: u32 = 512;
pub const THUMBNAIL_ITERATIONS: u64 = 50_000_000; // 50M iterations
pub const THUMBNAIL_ITERATIONS_PER_THREAD: u32 = 256;

/// Async thumbnail rendering (used by both desktop and WASM)
pub async fn render_thumbnail_async(
    device: &Device,
    queue: &Queue,
    config: &FractalConfig,
) -> image::RgbaImage {
    // Use unified render API
    let job = RenderJob::new(config, THUMBNAIL_SIZE, THUMBNAIL_SIZE)
        .with_iterations(THUMBNAIL_ITERATIONS)
        .with_iterations_per_thread(THUMBNAIL_ITERATIONS_PER_THREAD);

    let result = render(device, queue, job, &mut NoProgress).await;

    match result {
        Ok(output) => {
            image::RgbaImage::from_raw(output.width, output.height, output.rgba_data)
                .unwrap_or_else(|| image::RgbaImage::new(THUMBNAIL_SIZE, THUMBNAIL_SIZE))
        }
        Err(e) => {
            log::error!("Failed to render thumbnail: {}", e);
            // Return empty image on error
            image::RgbaImage::new(THUMBNAIL_SIZE, THUMBNAIL_SIZE)
        }
    }
}

/// Render a thumbnail for a FractalConfig (desktop only - blocks)
///
/// Uses the unified render API at thumbnail resolution.
///
/// Note: This is a blocking operation that takes ~1-2 seconds.
#[cfg(not(target_arch = "wasm32"))]
pub fn render_thumbnail(
    device: &Device,
    queue: &Queue,
    config: &FractalConfig,
    _palette_library: &crate::scene::palette::PaletteLibrary,
) -> image::RgbaImage {
    pollster::block_on(render_thumbnail_async(device, queue, config))
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
