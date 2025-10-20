mod app;
mod gpu;
mod ui;
mod util;
pub mod scene;
mod renderer;
mod config;
mod undo;

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

            // Set initial size to fill the browser window
            let width = web_window.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = web_window.inner_height().unwrap().as_f64().unwrap() as u32;
            let _ = window.request_inner_size(PhysicalSize::new(width, height));

            log::info!("Initial window size: {}x{}", width, height);

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
