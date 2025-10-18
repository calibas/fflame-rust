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
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    view_changed_by_keyboard: bool,
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
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            view_changed_by_keyboard: false,
            metrics: PerformanceMetrics::new(),
        };

        #[allow(deprecated)]
        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent { event, window_id } if window_id == window.id() => {
                    // Let egui handle events first
                    let consumed = app.egui_layer.handle_event(&event, &window);

                    match event {
                        WindowEvent::CloseRequested => elwt.exit(),
                        WindowEvent::Resized(size) => app.gpu.resize(size),
                        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                            // Handle scale factor changes if needed
                            let _ = scale_factor;
                        },
                        WindowEvent::KeyboardInput { event: key_event, .. } if !consumed => {
                            // Handle keyboard input only if egui didn't consume it
                            app.handle_keyboard(&key_event);
                        }
                        WindowEvent::RedrawRequested => {
                            app.update();
                            match app.render(&window) {
                                Ok(_) => {},
                                Err(SurfaceError::Lost) => app.gpu.resize(app.gpu.size),
                                Err(SurfaceError::OutOfMemory) => elwt.exit(),
                                Err(e) => eprintln!("Render error: {:?}", e),
                            }
                        }
                        _ => {}
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

    fn handle_keyboard(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{KeyCode, PhysicalKey};

        // Only handle key press (not release)
        if !event.state.is_pressed() {
            return;
        }

        let pan_step = 0.1 / self.zoom;

        match event.physical_key {
            PhysicalKey::Code(KeyCode::ArrowUp) => {
                self.pan_y -= pan_step;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowDown) => {
                self.pan_y += pan_step;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                self.pan_x -= pan_step;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowRight) => {
                self.pan_x += pan_step;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::Equal) | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                self.zoom *= 1.5;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::Minus) | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                self.zoom /= 1.5;
                self.view_changed_by_keyboard = true;
            }
            _ => {}
        }
    }

    fn render(&mut self, window: &Window) -> Result<(), SurfaceError> {
        let frame = self.gpu.surface.get_current_texture()?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Run flame compute shader with progressive refinement
        if let Some(ref mut renderer) = self.flame_renderer {
            // 1. Compute new samples with fresh random seed
            renderer.compute_pass(&mut encoder, &self.gpu.queue, 128, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y);

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
            &mut self.zoom,
            &mut self.pan_x,
            &mut self.pan_y,
        );

        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        // Handle UI responses and keyboard input (needs to be after submit since we need a new encoder)
        let view_changed = ui_response.view_changed || self.view_changed_by_keyboard;
        if ui_response.reset_requested || ui_response.flame_changed || ui_response.iterations_changed || view_changed {
            if let Some(ref mut renderer) = self.flame_renderer {
                let mut update_encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Update Encoder"),
                });

                if ui_response.flame_changed {
                    renderer.update_flame(&self.gpu.queue, &self.flame, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y);
                }

                if ui_response.iterations_changed || view_changed {
                    renderer.update_iterations(&self.gpu.queue, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y);
                }

                // Reset accumulation when view changes or user requests it
                if ui_response.reset_requested || view_changed {
                    renderer.reset(&mut update_encoder, &self.gpu.queue, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y);
                }

                self.gpu.queue.submit(std::iter::once(update_encoder.finish()));
            }
        }

        // Clear keyboard flag for next frame
        self.view_changed_by_keyboard = false;
        frame.present();

        Ok(())
    }
}