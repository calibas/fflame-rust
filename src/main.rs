mod app;
mod gpu;
mod ui;
mod util;

use app::App;
use winit::dpi::PhysicalSize;
use pollster;

fn main() {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new().expect("Error creating EventLoop");

    // For now, we'll use a simpler approach with direct window creation
    // This is a workaround until we refactor to use ApplicationHandler properly
    let attributes = winit::window::Window::default_attributes()
        .with_title("Fractal Flame Renderer")
        .with_inner_size(PhysicalSize::new(1920, 1080));

    #[allow(deprecated)]
    let window = event_loop.create_window(attributes).unwrap();

    // Launch async GPU init
    pollster::block_on(async {
        App::run(event_loop, window).await.unwrap();
    });
}