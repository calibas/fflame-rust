use winit::{event::*, event_loop::{EventLoop, ControlFlow}, window::Window};
use wgpu::SurfaceError;

use crate::gpu::device::GpuContext;
use crate::ui::EguiLayer;
use crate::renderer::FlameRenderer;
use crate::scene::presets;
use crate::util::PerformanceMetrics;

pub struct App {
    gpu: GpuContext,
    egui_layer: EguiLayer,
    flame_renderer: Option<FlameRenderer>,
    metrics: PerformanceMetrics,
}

impl App {
    pub async fn run(event_loop: EventLoop<()>, window: Window) -> Result<(), Box<dyn std::error::Error>> {
        let gpu = GpuContext::new(&window).await.expect("GPU init failed");
        let egui_layer = EguiLayer::new(&window, &gpu.device, gpu.config.format);

        // Create initial flame (use simple preset)
        let flame = presets::create_simple_flame();
        let flame_renderer = FlameRenderer::new(
            &gpu.device,
            gpu.config.format,
            gpu.size.width,
            gpu.size.height,
            &flame,
        );

        let mut app = Self {
            gpu,
            egui_layer,
            flame_renderer: Some(flame_renderer),
            metrics: PerformanceMetrics::new(),
        };

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
        // Update performance metrics
        self.metrics.update();
    }

    fn render(&mut self, window: &Window) -> Result<(), SurfaceError> {
        let frame = self.gpu.surface.get_current_texture()?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Run flame compute shader
        if let Some(ref mut renderer) = self.flame_renderer {
            // Dispatch compute work (progressive refinement)
            renderer.compute_pass(&mut encoder, 128); // 128 workgroups * 64 threads = 8192 trajectories per frame

            // Tonemap and render to screen
            renderer.tonemap_pass(&mut encoder, &view);
        }

        // Render UI on top
        self.egui_layer.render_ui(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &view,
            window,
            self.gpu.size,
            &self.metrics,
        );

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }
}