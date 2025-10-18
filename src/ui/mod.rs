use egui_wgpu::Renderer as EguiRenderer;
use egui_winit::State as EguiWinitState;
use wgpu::*;
use winit::{event::WindowEvent, window::Window};

pub struct UiResponse {
    pub reset_requested: bool,
    pub flame_changed: bool,
    pub iterations_changed: bool,
    pub view_changed: bool,
    pub density_changed: bool,
}

pub struct EguiLayer {
    state: EguiWinitState,
    ctx: egui::Context,
    renderer: EguiRenderer,
}

impl EguiLayer {
    pub fn new(window: &Window, device: &Device, format: TextureFormat) -> Self {
        let ctx = egui::Context::default();
        let viewport_id = ctx.viewport_id();
        let state = EguiWinitState::new(ctx.clone(), viewport_id, window, None, None, None);
        let renderer = EguiRenderer::new(device, format, None, 1, false);
        Self {
            state,
            ctx,
            renderer,
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent, window: &Window) -> bool {
        let response = self.state.on_window_event(window, event);
        response.consumed
    }

    pub fn render_ui(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        window: &Window,
        window_size: winit::dpi::PhysicalSize<u32>,
        metrics: &crate::util::PerformanceMetrics,
        flame_renderer: Option<&mut crate::renderer::compute_kernel::FlameRenderer>,
        flame: &mut crate::scene::transforms::Flame,
        iterations_per_thread: &mut u32,
        zoom: &mut f32,
        pan_x: &mut f32,
        pan_y: &mut f32,
        density_scale: &mut f32,
    ) -> UiResponse {
        let raw_input = self.state.take_egui_input(window);

        let mut reset_requested = false;
        let mut flame_changed = false;
        let mut iterations_changed = false;
        let mut view_changed = false;
        let mut density_changed = false;

        let full_output = self.ctx.run(raw_input, |ctx| {
            // Performance window
            egui::Window::new("Performance").show(ctx, |ui| {
                ui.heading("Fractal Flame Renderer");
                ui.separator();

                ui.label(format!("FPS: {:.1}", metrics.fps()));
                ui.label(format!("Frame Time: {:.2} ms", metrics.frame_time_ms()));

                let (min, max) = metrics.frame_time_range();
                ui.label(format!("Frame Time Range: {:.2} - {:.2} ms", min, max));

                ui.separator();
                ui.label(format!("Total Frames: {}", metrics.frame_count()));
                ui.label(format!("Resolution: {}x{}", window_size.width, window_size.height));

                // Accumulation controls
                if let Some(renderer) = &flame_renderer {
                    ui.separator();
                    ui.label(format!("Samples Accumulated: {}", renderer.samples_accumulated()));

                    if ui.button("Reset Accumulation").clicked() {
                        reset_requested = true;
                    }
                }

                // Render settings
                ui.separator();
                ui.label("Render Settings");
                if ui.add(egui::Slider::new(iterations_per_thread, 64..=4096).text("Iterations per Thread")).changed() {
                    iterations_changed = true;
                }
                if ui.add(egui::Slider::new(density_scale, 0.01..=10.0).text("Density Scale")).changed() {
                    density_changed = true;
                }

                // View settings
                ui.separator();
                ui.label("View");
                ui.horizontal(|ui| {
                    ui.label("Zoom:");
                    if ui.add(egui::DragValue::new(zoom).speed(0.01).range(0.01..=10000.0)).changed() {
                        view_changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Zoom In").clicked() {
                        *zoom *= 1.5;
                        view_changed = true;
                    }
                    if ui.button("Zoom Out").clicked() {
                        *zoom /= 1.5;
                        view_changed = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Pan X:");
                    if ui.add(egui::DragValue::new(pan_x).speed(0.01)).changed() {
                        view_changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Pan Y:");
                    if ui.add(egui::DragValue::new(pan_y).speed(0.01)).changed() {
                        view_changed = true;
                    }
                });

                // Pan step size depends on zoom (more zoomed in = smaller steps)
                let pan_step = 0.1 / *zoom;

                // Arrow keys layout
                ui.horizontal(|ui| {
                    ui.add_space(30.0);
                    if ui.button("  ^  ").clicked() {
                        *pan_y -= pan_step;
                        view_changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("  <  ").clicked() {
                        *pan_x -= pan_step;
                        view_changed = true;
                    }
                    if ui.button("  v  ").clicked() {
                        *pan_y += pan_step;
                        view_changed = true;
                    }
                    if ui.button("  >  ").clicked() {
                        *pan_x += pan_step;
                        view_changed = true;
                    }
                });

                if ui.button("Reset View").clicked() {
                    *zoom = 1.0;
                    *pan_x = 0.0;
                    *pan_y = 0.0;
                    view_changed = true;
                }
            });

            // Transforms window
            egui::Window::new("Transforms").show(ctx, |ui| {
                ui.heading(format!("Transforms ({})", flame.transforms.len()));
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, transform) in flame.transforms.iter_mut().enumerate() {
                        ui.push_id(i, |ui| {
                            egui::CollapsingHeader::new(format!("Transform {}", i))
                                .default_open(i == 0)
                                .show(ui, |ui| {
                                    ui.label("Affine Matrix");

                                    ui.horizontal(|ui| {
                                        ui.label("a:");
                                        if ui.add(egui::DragValue::new(&mut transform.a).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                        ui.label("b:");
                                        if ui.add(egui::DragValue::new(&mut transform.b).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("c:");
                                        if ui.add(egui::DragValue::new(&mut transform.c).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                        ui.label("d:");
                                        if ui.add(egui::DragValue::new(&mut transform.d).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("e:");
                                        if ui.add(egui::DragValue::new(&mut transform.e).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                        ui.label("f:");
                                        if ui.add(egui::DragValue::new(&mut transform.f).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                    });

                                    ui.separator();
                                    ui.label("Weight");
                                    if ui.add(egui::Slider::new(&mut transform.weight, 0.0..=2.0)).changed() {
                                        flame_changed = true;
                                    }

                                    ui.separator();
                                    ui.label("Color");
                                    if ui.horizontal(|ui| {
                                        ui.label("R:");
                                        let r_changed = ui.add(egui::Slider::new(&mut transform.color[0], 0.0..=1.0)).changed();
                                        ui.label("G:");
                                        let g_changed = ui.add(egui::Slider::new(&mut transform.color[1], 0.0..=1.0)).changed();
                                        ui.label("B:");
                                        let b_changed = ui.add(egui::Slider::new(&mut transform.color[2], 0.0..=1.0)).changed();
                                        r_changed || g_changed || b_changed
                                    }).inner {
                                        flame_changed = true;
                                    }

                                    if ui.add(egui::Slider::new(&mut transform.color_speed, 0.0..=1.0).text("Color Speed")).changed() {
                                        flame_changed = true;
                                    }

                                    ui.separator();
                                    ui.label("Variations");

                                    let variation_names = [
                                        "Linear", "Sinusoidal", "Spherical", "Swirl",
                                        "Horseshoe", "Polar", "Handkerchief", "Heart",
                                        "Disc", "Spiral", "Hyperbolic", "Diamond",
                                        "Ex", "Julia", "Bent", "Waves"
                                    ];

                                    for (idx, name) in variation_names.iter().enumerate() {
                                        if ui.add(egui::Slider::new(&mut transform.variations[idx], 0.0..=2.0).text(*name)).changed() {
                                            flame_changed = true;
                                        }
                                    }
                                });
                        });
                    }
                });
            });
        });

        self.state.handle_platform_output(window, full_output.platform_output);

        let tris = self.ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [window_size.width, window_size.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer.update_texture(device, queue, *id, image_delta);
        }

        self.renderer.update_buffers(device, queue, encoder, &tris, &screen_descriptor);

        {
            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load, // Load existing content (flame rendering)
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // SAFETY: egui-wgpu's render method has an overly restrictive 'static lifetime.
            // This transmute is safe because we immediately drop the render pass after calling render.
            let rpass_static: &mut wgpu::RenderPass<'static> = unsafe {
                std::mem::transmute(&mut rpass)
            };

            self.renderer.render(rpass_static, &tris, &screen_descriptor);
        } // Render pass is dropped here

        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }

        UiResponse {
            reset_requested,
            flame_changed,
            iterations_changed,
            view_changed,
            density_changed,
        }
    }
}
