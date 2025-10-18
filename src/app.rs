use winit::{event::*, event_loop::{EventLoop, ControlFlow}, window::Window};
use wgpu::SurfaceError;

use crate::gpu::device::GpuContext;
use crate::ui::EguiLayer;
use crate::renderer::FlameRenderer;
use crate::scene::{presets, transforms::Flame};
use crate::util::PerformanceMetrics;

pub struct App {
    gpu: GpuContext,
    egui_layer: EguiLayer,
    flame_renderer: Option<FlameRenderer>,
    flame: Flame,
    iterations_per_thread: u32,
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
            flame,
            iterations_per_thread: 256,
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

        // Run flame compute shader with progressive refinement
        if let Some(ref mut renderer) = self.flame_renderer {
            // 1. Compute new samples
            renderer.compute_pass(&mut encoder, 128); // 128 workgroups * 64 threads = 8192 trajectories per frame

            // 2. Accumulate samples (blend with previous frames)
            renderer.accumulate_pass(&mut encoder, &self.gpu.queue, &self.gpu.device);

            // 3. Tonemap and render to screen
            renderer.tonemap_pass(&mut encoder, &view);
        }

        // Render UI on top and handle updates
        let ui_response = self.egui_layer.render_ui(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &view,
            window,
            self.gpu.size,
            &self.metrics,
            self.flame_renderer.as_mut(),
            &mut self.flame,
            &mut self.iterations_per_thread,
        );

        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        // Handle UI responses (needs to be after submit since we need a new encoder)
        if ui_response.reset_requested || ui_response.flame_changed || ui_response.iterations_changed {
            if let Some(ref mut renderer) = self.flame_renderer {
                let mut update_encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Update Encoder"),
                });

                if ui_response.flame_changed {
                    renderer.update_flame(&self.gpu.queue, &self.flame);
                }

                if ui_response.iterations_changed {
                    renderer.update_iterations(&self.gpu.queue, self.iterations_per_thread);
                }

                if ui_response.reset_requested {
                    renderer.reset(&mut update_encoder, &self.gpu.queue);
                }

                self.gpu.queue.submit(std::iter::once(update_encoder.finish()));
            }
        }
        frame.present();

        Ok(())
    }
}