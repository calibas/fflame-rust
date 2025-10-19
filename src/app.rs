use winit::{event::*, event_loop::{EventLoop, ControlFlow}, window::Window};
use wgpu::SurfaceError;

use crate::gpu::device::GpuContext;
use crate::ui::EguiLayer;
use crate::renderer::FlameRenderer;
use crate::scene::{presets, transforms::Flame};
use crate::scene::palette::{PaletteLibrary, ColorMode};
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
    density_scale: f32,
    view_changed_by_keyboard: bool,
    mouse_dragging: bool,
    last_mouse_pos: Option<(f32, f32)>,
    metrics: PerformanceMetrics,
    palette_library: PaletteLibrary,
    current_palette_index: usize,
    color_mode: ColorMode,
    paused: bool,
    max_iterations: Option<u64>,
    speed_factor: f32,
}

impl App {
    pub async fn run(event_loop: EventLoop<()>, window: Window) -> Result<(), Box<dyn std::error::Error>> {
        let gpu = GpuContext::new(&window).await.expect("GPU init failed");
        let egui_layer = EguiLayer::new(&window, &gpu.device, gpu.config.format);

        // Create initial flame (use simple preset)
        let flame = presets::create_simple_flame();
        let flame_renderer = FlameRenderer::new(
            &gpu.device,
            &gpu.queue,
            gpu.config.format,
            gpu.size.width,
            gpu.size.height,
            &flame,
        );

        let palette_library = PaletteLibrary::new();

        let mut app = Self {
            gpu,
            egui_layer,
            flame_renderer: Some(flame_renderer),
            flame,
            iterations_per_thread: 256,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            density_scale: 1.0,
            view_changed_by_keyboard: false,
            mouse_dragging: false,
            last_mouse_pos: None,
            metrics: PerformanceMetrics::new(),
            palette_library,
            current_palette_index: 1, // Start with Fire palette
            color_mode: ColorMode::Transform,
            paused: false,
            max_iterations: None,
            speed_factor: 0.5,
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
                        WindowEvent::MouseInput { state, button, .. } if !consumed => {
                            app.handle_mouse_button(state, button);
                        }
                        WindowEvent::CursorMoved { position, .. } if !consumed => {
                            app.handle_mouse_move(position.x as f32, position.y as f32);
                        }
                        WindowEvent::MouseWheel { delta, .. } if !consumed => {
                            app.handle_mouse_wheel(delta);
                        }
                        WindowEvent::RedrawRequested => {
                            app.update();
                            match app.render(&window) {
                                Ok(_) => {},
                                Err(SurfaceError::Lost | SurfaceError::Outdated) => app.gpu.resize(app.gpu.size),
                                Err(SurfaceError::OutOfMemory) => elwt.exit(),
                                Err(e) => eprintln!("Render error: {:?}", e),
                            }
                        }
                        _ => {}
                    }
                }
                Event::Resumed => {
                    // On WASM, request a resize to ensure proper canvas dimensions
                    #[cfg(target_arch = "wasm32")]
                    {
                        let size = window.inner_size();
                        if size.width > 0 && size.height > 0 {
                            log::info!("Resumed event - resizing to {}x{}", size.width, size.height);
                            app.gpu.resize(size);
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

    fn handle_mouse_button(&mut self, state: winit::event::ElementState, button: winit::event::MouseButton) {
        use winit::event::{ElementState, MouseButton};

        if button == MouseButton::Left {
            match state {
                ElementState::Pressed => {
                    self.mouse_dragging = true;
                }
                ElementState::Released => {
                    self.mouse_dragging = false;
                    self.last_mouse_pos = None;
                }
            }
        }
    }

    fn handle_mouse_move(&mut self, x: f32, y: f32) {
        if self.mouse_dragging {
            if let Some((last_x, last_y)) = self.last_mouse_pos {
                // Calculate delta in screen pixels
                let dx = x - last_x;
                let dy = y - last_y;

                // Convert to fractal space - scale inversely with zoom and window size
                let scale = f32::min(self.gpu.size.width as f32, self.gpu.size.height as f32) * 0.25;
                let pan_dx = -dx / (scale * self.zoom);
                let pan_dy = -dy / (scale * self.zoom); // Negative to match drag direction

                self.pan_x += pan_dx;
                self.pan_y += pan_dy;
                self.view_changed_by_keyboard = true; // Reuse same flag for mouse
            }
            self.last_mouse_pos = Some((x, y));
        }
    }

    fn handle_mouse_wheel(&mut self, delta: winit::event::MouseScrollDelta) {
        use winit::event::MouseScrollDelta;

        let zoom_factor = match delta {
            MouseScrollDelta::LineDelta(_x, y) => {
                // Each line is typically one "click" of the wheel
                // Positive y = scroll up = zoom in, negative y = scroll down = zoom out
                if y > 0.0 {
                    1.1f32.powf(y)
                } else if y < 0.0 {
                    1.1f32.powf(y)
                } else {
                    1.0
                }
            }
            MouseScrollDelta::PixelDelta(pos) => {
                // Pixel delta for touchpad scrolling
                // Positive y = scroll up = zoom in
                let y = pos.y as f32;
                if y.abs() > 0.1 {
                    1.1f32.powf(y * 0.01)
                } else {
                    1.0
                }
            }
        };

        if zoom_factor != 1.0 {
            self.zoom *= zoom_factor;
            self.view_changed_by_keyboard = true; // Reuse same flag
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
            // Check if we should continue iterating
            let should_iterate = !self.paused &&
                self.max_iterations.map_or(true, |max| renderer.total_iterations() < max);

            if should_iterate {
                // 1. Compute new samples with fresh random seed
                renderer.compute_pass(&mut encoder, &self.gpu.queue, 128, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.speed_factor);

                // 2. Accumulate samples (blend with previous frames)
                renderer.accumulate_pass(&mut encoder, &self.gpu.queue, &self.gpu.device);
            }

            // 3. Tonemap and render to screen (always render)
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
            &mut self.density_scale,
            &self.palette_library,
            &mut self.current_palette_index,
            &mut self.color_mode,
            &mut self.paused,
            &mut self.max_iterations,
            &mut self.speed_factor,
        );

        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        // Handle UI responses and keyboard input (needs to be after submit since we need a new encoder)
        let view_changed = ui_response.view_changed || self.view_changed_by_keyboard;
        let needs_update = ui_response.reset_requested || ui_response.flame_changed || ui_response.iterations_changed
            || view_changed || ui_response.density_changed || ui_response.palette_changed || ui_response.color_mode_changed || ui_response.pause_changed;

        if needs_update {
            if let Some(ref mut renderer) = self.flame_renderer {
                let mut update_encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Update Encoder"),
                });

                if ui_response.flame_changed {
                    renderer.update_flame(&self.gpu.queue, &self.flame, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.speed_factor);
                }

                if ui_response.iterations_changed || view_changed {
                    renderer.update_iterations(&self.gpu.queue, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.speed_factor);
                }

                if ui_response.density_changed {
                    renderer.update_density_scale(&self.gpu.queue, self.density_scale);
                }

                if ui_response.color_mode_changed {
                    renderer.set_color_mode(&self.gpu.queue, self.color_mode, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.speed_factor);
                }

                if ui_response.palette_changed {
                    if let Some(palette) = self.palette_library.get(self.current_palette_index) {
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, palette);
                    }
                }

                // Reset accumulation when view changes, palette changes, color mode changes, or user requests it
                if ui_response.reset_requested || view_changed || ui_response.palette_changed || ui_response.color_mode_changed {
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