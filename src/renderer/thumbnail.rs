//! Thumbnails and gallery previews
//!
//! Both render a FractalConfig through the unified render API at a fixed
//! size and sample count: the thumbnail Save Online uploads (512, what
//! the API wants), and the Fractal Browser's previews (256, lit; see
//! [`render_preview_async`]).
//!
//! On desktop: Uses pollster::block_on() for synchronous rendering
//! On WASM: Uses async rendering with spawn_local()

use wgpu::*;

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
    // Where orbits are reset every dispatch, a dispatch is an orbit, so
    // the job keeps the length `RenderJob::new` gives it.
    let job = RenderJob::new(config, THUMBNAIL_SIZE, THUMBNAIL_SIZE).with_iterations(THUMBNAIL_ITERATIONS);
    let job = if config.resets_orbits() { job } else { job.with_iterations_per_thread(THUMBNAIL_ITERATIONS_PER_THREAD) };

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

/// The Fractal Browser's previews: the size the gallery shows a card at
/// most, and the cache keeps (`thumbnail_cache::CACHE_PIXELS`).
pub const PREVIEW_SIZE: u32 = 256;
/// A flame preview's samples: the thumbnail's density per pixel, scaled
/// to the preview's area.
pub const PREVIEW_ITERATIONS: u64 =
    THUMBNAIL_ITERATIONS * (PREVIEW_SIZE * PREVIEW_SIZE) as u64 / (THUMBNAIL_SIZE * THUMBNAIL_SIZE) as u64;

/// A gallery preview of `config`: smaller and cheaper than the
/// thumbnail Save Online uploads (`render_thumbnail_async`), which is
/// 512 because the API wants that size.
///
/// Rendered at [`PREVIEW_SIZE`] with [`PREVIEW_ITERATIONS`], and a lit 3D
/// view -- a solid, an escape terrain, a simulation's -- in the Lit tier
/// rather than path traced, which an `Auto` tier would give it (the
/// export's path; 256 samples by default). A simulation still runs its
/// whole step count: its picture is the run at that count.
pub async fn render_preview_async(device: &Device, queue: &Queue, config: &FractalConfig) -> image::RgbaImage {
    let config = preview_config(config);
    let job = RenderJob::new(&config, PREVIEW_SIZE, PREVIEW_SIZE).with_iterations(PREVIEW_ITERATIONS);
    let job = if config.resets_orbits() { job } else { job.with_iterations_per_thread(THUMBNAIL_ITERATIONS_PER_THREAD) };
    match render(device, queue, job, &mut NoProgress).await {
        Ok(output) => image::RgbaImage::from_raw(output.width, output.height, output.rgba_data)
            .unwrap_or_else(|| image::RgbaImage::new(PREVIEW_SIZE, PREVIEW_SIZE)),
        Err(e) => {
            log::error!("Failed to render a preview: {}", e);
            image::RgbaImage::new(PREVIEW_SIZE, PREVIEW_SIZE)
        }
    }
}

/// `config` as a preview renders it: every 3D view in the Lit tier.
fn preview_config(config: &FractalConfig) -> FractalConfig {
    use crate::config::escape::RenderTier;
    let mut c = config.clone();
    c.escape.solid_tier = RenderTier::Lit;
    c.escape.terrain.tier = RenderTier::Lit;
    c.sim.terrain.tier = RenderTier::Lit;
    c
}

/// Render a gallery preview (desktop only - blocks); see
/// [`render_preview_async`].
#[cfg(not(target_arch = "wasm32"))]
pub fn render_preview(device: &Device, queue: &Queue, config: &FractalConfig) -> image::RgbaImage {
    pollster::block_on(render_preview_async(device, queue, config))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A preview is the gallery's size at the thumbnail's density, and
    /// every 3D view in it lit: a solid at `Auto` -- the export's path
    /// tracing -- a path-traced terrain, a simulation's terrain.
    #[test]
    fn a_preview_is_small_and_lit() {
        use crate::config::escape::RenderTier;
        assert_eq!(THUMBNAIL_SIZE, 512, "the API's upload");
        assert_eq!(PREVIEW_SIZE, 256);
        assert_eq!(PREVIEW_ITERATIONS, THUMBNAIL_ITERATIONS / 4);
        let mut c = FractalConfig::default();
        c.escape.solid_tier = RenderTier::Auto;
        c.escape.terrain.tier = RenderTier::PathTraced;
        c.sim.terrain.tier = RenderTier::Auto;
        let p = preview_config(&c);
        assert_eq!((p.escape.solid_tier, p.escape.terrain.tier, p.sim.terrain.tier), (RenderTier::Lit, RenderTier::Lit, RenderTier::Lit));
    }

    /// A solid's preview renders, at the preview's size, through the lit
    /// tier: drawn, not the black an error leaves.
    #[test]
    fn a_solid_preview_renders_lit() {
        let instance = Instance::new(InstanceDescriptor { backends: Backends::all(), ..InstanceDescriptor::new_without_display_handle() });
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&RequestAdapterOptions::default())) else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let Ok((device, queue)) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("preview tests"),
            required_features: adapter.features() & Features::CLEAR_TEXTURE,
            required_limits: adapter.limits(),
            ..Default::default()
        })) else {
            eprintln!("no device; skipping");
            return;
        };
        let mut c = FractalConfig::default();
        c.render_mode = crate::scene::transforms::RenderMode::Escape;
        c.escape.formula = "quaternion_julia_solid".to_string();
        c.escape.solid_tier = crate::config::escape::RenderTier::Auto;
        let image = pollster::block_on(render_preview_async(&device, &queue, &c));
        assert_eq!(image.dimensions(), (PREVIEW_SIZE, PREVIEW_SIZE));
        let drawn = image.pixels().filter(|p| p.0[..3] != [0, 0, 0]).count();
        // The default solid fills about a tenth of its frame; an error
        // leaves none of it.
        assert!(drawn > (PREVIEW_SIZE * PREVIEW_SIZE / 64) as usize, "the solid shows in {drawn} pixels");
    }
}
