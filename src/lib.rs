// Load I18n macro to allow use of `t!` macro anywhere
#[macro_use]
extern crate rust_i18n;

// Initialize rust-i18n with the locales directory
rust_i18n::i18n!("locales", fallback = "en");

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
// mod shader_builder; // Legacy - replaced by shader_builder_v2
mod shader_builder_v2;
mod shader_cache;

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
pub fn export_mode(input: &str, output: &str, width: Option<u32>, height: Option<u32>, category: Option<String>, iterations_per_thread: u32, speed_multiplier: u32) {
    env_logger::init();
    pollster::block_on(export_async(input, output, width, height, category, iterations_per_thread, speed_multiplier)).expect("Export failed");
}

#[cfg(not(target_arch = "wasm32"))]
async fn export_async(input: &str, output: &str, width: Option<u32>, height: Option<u32>, category: Option<String>, iterations_per_thread: u32, speed_multiplier: u32) -> Result<(), Box<dyn std::error::Error>> {
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

        // Call the existing PNG export logic from app
        // We'll need to add a headless export helper
        let success = app::export_headless(config, &output_file, w, h, category.clone(), iterations_per_thread, speed_multiplier).await?;

        if success {
            println!("  ✓ Saved to {}", output_file.display());
        }
    }

    println!("\nExport complete!");
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
                .with_title("Fractal Flame Renderer")
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

        App::run(event_loop, window).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let attributes = winit::window::Window::default_attributes()
            .with_title("Fractal Flame Renderer")
            .with_inner_size(PhysicalSize::new(1920, 1080));

        #[allow(deprecated)]
        let window = event_loop.create_window(attributes)?;

        App::run(event_loop, window).await
    }
}
