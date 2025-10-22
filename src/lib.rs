mod app;
pub mod gpu;
mod ui;
mod util;
pub mod scene;
mod renderer;
pub mod config;
mod undo;
pub mod profiler;
pub mod version;

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
