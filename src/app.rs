use winit::{event::*, event_loop::{EventLoop, ControlFlow}, window::Window};
use wgpu::SurfaceError;

use crate::gpu::device::GpuContext;
use crate::ui::EguiLayer;

pub struct App {
    gpu: GpuContext,
    egui_layer: EguiLayer,
}

impl App {
    pub async fn run(event_loop: EventLoop<()>, window: Window) -> Result<(), Box<dyn std::error::Error>> {
        let gpu = GpuContext::new(&window).await.expect("GPU init failed");
        let egui_layer = EguiLayer::new(&window, &gpu.device, gpu.config.format);

        let mut app = Self { gpu, egui_layer };

        #[allow(deprecated)]
        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent { event, window_id } if window_id == window.id() => {
                    match event {
                        WindowEvent::CloseRequested => elwt.exit(),
                        WindowEvent::Resized(size) => app.gpu.resize(size),
                        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                            // Handle scale factor changes if needed
                            let _ = scale_factor;
                        },
                        WindowEvent::RedrawRequested => {
                            app.update();
                            match app.render(&window) {
                                Ok(_) => {},
                                Err(SurfaceError::Lost) => app.gpu.resize(app.gpu.size),
                                Err(SurfaceError::OutOfMemory) => elwt.exit(),
                                Err(e) => eprintln!("Render error: {:?}", e),
                            }
                        }
                        _ => {
                            app.egui_layer.handle_event(&event, &window);
                        }
                    }
                }
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        })?;

        Ok(())
    }

    fn update(&mut self) {
        // Update app state here (UI, animation, etc.)
    }

    fn render(&mut self, window: &Window) -> Result<(), SurfaceError> {
        self.gpu.begin_frame();
        self.egui_layer.render_ui(&self.gpu, window);
        Ok(())
    }
}