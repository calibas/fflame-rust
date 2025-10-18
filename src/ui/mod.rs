use egui_wgpu::Renderer as EguiRenderer;
use egui_winit::State as EguiWinitState;
use wgpu::*;
use winit::{event::WindowEvent, window::Window};

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
    ) {
        let raw_input = self.state.take_egui_input(window);

        let full_output = self.ctx.run(raw_input, |ctx| {
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
    }
}
