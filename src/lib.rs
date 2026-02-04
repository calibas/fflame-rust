// Load I18n macro to allow use of `t!` macro anywhere
#[macro_use]
extern crate rust_i18n;

// Initialize rust-i18n with the locales directory
i18n!("locales", fallback = "en");

mod app;
pub mod gpu;
mod ui;
mod util;
pub mod scene;
pub mod renderer;
pub mod config;
pub mod profiler;
pub mod version;
pub mod variations;
pub mod png_metadata;
pub mod apophysis_xml;
pub mod i18n;
pub mod animation;
pub mod signal;
pub mod storage;
pub mod export;
pub mod resources;
pub mod effects;
#[cfg(feature = "audio")]
pub mod audio;
// mod shader_builder; // Legacy - replaced by shader_builder_v2
mod shader_builder_v2;
mod shader_cache;

#[cfg(target_arch = "wasm32")]
pub mod wasm_api;
#[cfg(target_arch = "wasm32")]
mod web_clipboard;

// Prelude for convenient imports
pub mod prelude {
    pub use crate::scene::presets::PresetLibrary;
    pub use crate::scene::palette::{PaletteLibrary, ColorMode};
    pub use crate::scene::tonemap::{ToneCurve, ToneMapMode};
    pub use crate::renderer::compute_kernel::FlameRenderer;
    pub use crate::config::FractalConfig;
}

use app::App;
use winit::dpi::PhysicalSize;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn wasm_main() {
    // Set up panic hook for better error messages in browser console
    console_error_panic_hook::set_once();

    // Initialize logging
    console_log::init_with_level(log::Level::Info).expect("Failed to initialize logger");

    run().await.expect("Failed to run app");
}

#[cfg(not(target_arch = "wasm32"))]
pub fn desktop_main() {
    env_logger::init();
    pollster::block_on(run()).expect("Failed to run app");
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_mode(input: &str, output: &str, width: Option<u32>, height: Option<u32>, category: Option<String>, iterations_per_thread: Option<u32>, dump_shader: bool) {
    env_logger::init();
    if dump_shader {
        shader_builder_v2::enable_shader_dump();
    }
    // Enable inlined constants for CLI export - compiles flame data as shader constants
    // for maximum performance (eliminates buffer reads, enables dead code elimination)
    shader_builder_v2::enable_inlined_constants();
    pollster::block_on(export_async(input, output, width, height, category, iterations_per_thread)).expect("Export failed");
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_animation_mode(
    config_path: &str,
    animation_path: &str,
    output_path: &str,
    width: u32,
    height: u32,
    fps: u32,
    iterations_per_thread: u32,
    video_settings: animation::export::VideoEncodingSettings,
) {
    env_logger::init();
    // Enable inlined constants for animation export - maximum shader performance
    shader_builder_v2::enable_inlined_constants();
    pollster::block_on(export_animation_async(config_path, animation_path, output_path, width, height, fps, iterations_per_thread, video_settings)).expect("Animation export failed");
}

#[cfg(not(target_arch = "wasm32"))]
async fn export_async(input: &str, output: &str, width: Option<u32>, height: Option<u32>, category: Option<String>, iterations_per_thread: Option<u32>) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path;

    println!("Fractal Flame Batch Export");
    println!("===========================");
    println!("Input: {}", input);
    println!("Output: {}", output);
    println!();

    // Find all .fflame files
    let input_path = Path::new(input);
    let flame_files = if input_path.is_dir() {
        scene::assets::load_configs_from_dir(input_path)
    } else {
        vec![config::FractalConfig::load_from_file(input_path)?]
    };

    if flame_files.is_empty() {
        eprintln!("No .fflame files found");
        return Ok(());
    }

    println!("Found {} config(s)\n", flame_files.len());

    // Create output directory if needed
    let output_path = Path::new(output);
    if flame_files.len() > 1 {
        std::fs::create_dir_all(output_path)?;
    }

    // Export each config
    for (i, config) in flame_files.iter().enumerate() {
        let flame_name = &config.flame.name;
        println!("[{}/{}] Exporting {}...", i + 1, flame_files.len(), flame_name);

        // Determine output file path
        let output_file = if flame_files.len() == 1 && output_path.extension().is_some() {
            output_path.to_path_buf()
        } else {
            output_path.join(format!("{}.png", flame_name.to_lowercase().replace(" ", "_")))
        };

        // Use config dimensions or provided dimensions
        let (w, h) = (
            width.unwrap_or(1920),
            height.unwrap_or(1080),
        );

        // Use CLI override or default (device-specific setting not in FractalConfig)
        let ipt = iterations_per_thread.unwrap_or(crate::config::defaults::DEFAULT_ITERATIONS_PER_THREAD);

        // Call the existing PNG export logic from app
        // We'll need to add a headless export helper
        // CLI export uses opaque (non-transparent) mode by default
        let success = app::export_headless(config, &output_file, w, h, category.clone(), ipt, false).await?;

        if success {
            println!("  ✓ Saved to {}", output_file.display());
        }
    }

    println!("\nExport complete!");
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
async fn export_animation_async(
    config_path: &str,
    animation_path: &str,
    output_path: &str,
    width: u32,
    height: u32,
    fps: u32,
    iterations_per_thread: u32,
    video_settings: animation::export::VideoEncodingSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path;
    use animation::export::{AnimationExportConfig, CliProgressCallback, export_animation, is_ffmpeg_available};
    use animation::Animation;

    println!("Fractal Flame Animation Export");
    println!("===============================");
    println!("Config: {}", config_path);
    println!("Animation: {}", animation_path);
    println!("Output: {}", output_path);
    println!("Resolution: {}x{} @ {} FPS", width, height, fps);
    println!("Iterations per thread: {}", iterations_per_thread);
    println!("Codec: {} (CRF {})", video_settings.codec.display_name(), video_settings.quality);
    println!();

    // Check ffmpeg availability
    if !is_ffmpeg_available() {
        eprintln!("Error: FFmpeg not found.");
        eprintln!("Install FFmpeg and ensure it's in your PATH to export animations.");
        return Err("FFmpeg not found".into());
    }

    // Load config file
    let config = config::FractalConfig::load_from_file(Path::new(config_path))?;
    println!("Loaded config: {}", config.flame.name);

    // Load animation file
    let animation_contents = std::fs::read_to_string(animation_path)?;
    let anim = Animation::from_json(&animation_contents)?;
    println!("Loaded animation: {} ({:.1}s duration)", anim.name, anim.duration);

    let total_frames = (anim.duration * fps as f64).ceil() as u32;
    println!("Total frames: {}", total_frames);
    println!();

    // Create export config
    let export_config = AnimationExportConfig {
        config,
        animation: anim,
        output_path: Path::new(output_path).to_path_buf(),
        width,
        height,
        fps,
        iterations_per_thread,
        video_settings,
    };

    // Run export (pipes directly to FFmpeg)
    let mut progress = CliProgressCallback::new();
    let result = export_animation(export_config, &mut progress).await?;

    println!();
    println!("Animation export complete!");
    println!("  Total frames: {}", result.total_frames);
    println!("  Total time: {:.1}s", result.total_time_ms / 1000.0);
    println!("  Average per frame: {:.1}s", result.avg_frame_time_ms / 1000.0);
    println!("  Output: {}", result.output_path.display());

    Ok(())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = winit::event_loop::EventLoop::new()?;

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowAttributesExtWebSys;

        let window = {
            let web_window = web_sys::window().unwrap();
            let document = web_window.document().unwrap();
            let canvas = document
                .get_element_by_id("canvas")
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();

            let attributes = winit::window::Window::default_attributes()
                .with_title("FAR")
                .with_canvas(Some(canvas));

            #[allow(deprecated)]
            let window = event_loop.create_window(attributes)?;

            // Set canvas size to match display size with device pixel ratio
            // This ensures 1:1 pixel mapping for crisp rendering
            let dpr = web_window.device_pixel_ratio();
            let width = web_window.inner_width().unwrap().as_f64().unwrap();
            let height = web_window.inner_height().unwrap().as_f64().unwrap();

            // Physical pixels = CSS pixels × device pixel ratio
            let physical_width = (width * dpr) as u32;
            let physical_height = (height * dpr) as u32;

            let _ = window.request_inner_size(PhysicalSize::new(physical_width, physical_height));

            // DEBUG: Log all size-related information
            let actual_inner_size = window.inner_size();
            let canvas_element = document.get_element_by_id("canvas").unwrap();
            let canvas = canvas_element.dyn_into::<web_sys::HtmlCanvasElement>().unwrap();

            log::info!("=== WASM Canvas Size Debug ===");
            log::info!("  Browser window (CSS): {}x{}", width as u32, height as u32);
            log::info!("  Device Pixel Ratio: {}", dpr);
            log::info!("  Calculated physical: {}x{}", physical_width, physical_height);
            log::info!("  Window inner_size(): {}x{}", actual_inner_size.width, actual_inner_size.height);
            log::info!("  Canvas element width: {}", canvas.width());
            log::info!("  Canvas element height: {}", canvas.height());
            log::info!("  Canvas clientWidth: {}", canvas.client_width());
            log::info!("  Canvas clientHeight: {}", canvas.client_height());
            log::info!("  Canvas offsetWidth: {}", canvas.offset_width());
            log::info!("  Canvas offsetHeight: {}", canvas.offset_height());
            log::info!("===============================");

            window
        };

        App::run(event_loop, std::sync::Arc::new(window)).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let attributes = winit::window::Window::default_attributes()
            .with_title("FAR")
            .with_inner_size(PhysicalSize::new(1920, 1080));

        #[allow(deprecated)]
        let window = event_loop.create_window(attributes)?;

        App::run(event_loop, std::sync::Arc::new(window)).await
    }
}
