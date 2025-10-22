mod config_dialog;
mod formatting;
mod help;
mod menu_bar;
mod palette_editor;
mod performance;
mod response;
mod settings;
mod transforms;
mod view;

pub use palette_editor::PaletteEditor;
pub use response::UiResponse;

use egui_wgpu::Renderer as EguiRenderer;
use egui_winit::State as EguiWinitState;
use wgpu::*;
use winit::{event::WindowEvent, window::Window};

pub struct EguiLayer {
    state: EguiWinitState,
    pub ctx: egui::Context,
    renderer: EguiRenderer,
    config_json_buffer: String,
    show_config_window: bool,  // For Import/Export Config dialog
    show_palette_editor: bool,
    palette_editor: PaletteEditor,
    // Window visibility (no persistence between sessions)
    show_performance: bool,
    show_settings: bool,
    show_view: bool,
    show_transforms: bool,
    show_help: bool,
}

impl EguiLayer {
    pub fn new(window: &Window, device: &Device, format: TextureFormat) -> Self {
        let ctx = egui::Context::default();

        // Configure style to disable window shadows
        ctx.set_visuals(egui::Visuals {
            window_shadow: egui::epaint::Shadow::NONE,
            ..egui::Visuals::dark()
        });

        let viewport_id = ctx.viewport_id();
        let state = EguiWinitState::new(ctx.clone(), viewport_id, window, None, None, None);
        let renderer = EguiRenderer::new(device, format, None, 1, false);
        Self {
            state,
            ctx,
            renderer,
            config_json_buffer: String::new(),
            show_config_window: false,
            show_palette_editor: false,
            palette_editor: PaletteEditor::new(),
            // Window visibility - Performance, Transforms, Help minimized by default
            show_performance: false,
            show_settings: true,
            show_view: true,
            show_transforms: false,
            show_help: false,
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent, window: &Window) -> bool {
        let response = self.state.on_window_event(window, event);
        response.consumed
    }

    pub fn update_palette_editor(&mut self, palette: crate::scene::palette::Palette) {
        self.palette_editor.current_palette = palette;
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
        rotation: &mut f32,
        camera_rotation_x: &mut f32,
        camera_rotation_y: &mut f32,
        density_scale: &mut f32,
        palette_library: &crate::scene::palette::PaletteLibrary,
        current_palette_index: &mut usize,
        preset_library: &crate::scene::presets::PresetLibrary,
        current_preset_index: &mut usize,
        color_mode: &mut crate::scene::palette::ColorMode,
        paused: &mut bool,
        max_iterations: &mut Option<u64>,
        speed_factor: &mut f32,
        can_undo: bool,
        can_redo: bool,
        background_color: &mut [f32; 3],
    ) -> UiResponse {
        let raw_input = self.state.take_egui_input(window);

        let mut reset_requested = false;
        let mut flame_changed = false;
        let mut iterations_changed = false;
        let mut view_changed = false;
        let mut density_changed = false;
        let mut palette_changed = false;
        let mut color_mode_changed = false;
        let mut pause_changed = false;
        let mut preset_changed = false;
        let mut add_transform = false;
        let mut delete_transform = None;
        let mut render_mode_changed = false;
        let mut projection_changed = false;
        let mut camera_rotation_changed = false;
        // Config import/export window
        let mut config_export_json = None;
        let mut config_import_json = None;
        let mut config_save_file = false;
        let mut config_load_file = false;
        let mut custom_palette = None;
        let mut undo_requested = false;
        let mut redo_requested = false;
        let mut background_color_changed = false;
        let mut png_export_with_background = false;
        let mut png_export_transparent = false;
        // Palette import/export
        let mut palette_export_json = None;
        let mut palette_save_file = None;
        let mut palette_import_json = None;
        let mut palette_load_file = false;


        let full_output = self.ctx.run(raw_input, |ctx| {
            // Render menu bar
            menu_bar::render_menu_bar(
                ctx,
                &mut self.show_performance,
                &mut self.show_settings,
                &mut self.show_view,
                &mut self.show_transforms,
                &mut self.show_help,
                &mut self.show_palette_editor,
                &mut self.show_config_window,
            );

            // Render Performance window
            performance::render_performance_window(
                ctx,
                &mut self.show_performance,
                metrics,
                window_size,
                flame_renderer.as_deref(),
            );

            // Render Settings window
            settings::render_settings_window(
                ctx,
                &mut self.show_settings,
                &mut self.show_config_window,
                &mut self.show_palette_editor,
                can_undo,
                can_redo,
                &mut undo_requested,
                &mut redo_requested,
                &mut png_export_with_background,
                &mut png_export_transparent,
                preset_library,
                current_preset_index,
                &mut preset_changed,
                flame,
                &mut render_mode_changed,
                &mut projection_changed,
                &mut flame_changed,
                flame_renderer.as_deref(),
                paused,
                &mut pause_changed,
                &mut reset_requested,
                max_iterations,
                iterations_per_thread,
                &mut iterations_changed,
                density_scale,
                &mut density_changed,
                color_mode,
                &mut color_mode_changed,
                palette_library,
                current_palette_index,
                &mut palette_changed,
                &mut self.palette_editor.current_palette,
                speed_factor,
                background_color,
                &mut background_color_changed,
            );

            // Render View window
            view::render_view_window(
                ctx,
                &mut self.show_view,
                zoom,
                pan_x,
                pan_y,
                rotation,
                camera_rotation_x,
                camera_rotation_y,
                flame,
                &mut view_changed,
                &mut camera_rotation_changed,
            );

            // Render Help window
            help::render_help_window(ctx, &mut self.show_help);

            // Render Transforms window
            transforms::render_transforms_window(
                ctx,
                &mut self.show_transforms,
                flame,
                &mut flame_changed,
                &mut add_transform,
                &mut delete_transform,
            );

            // Render Palette Editor window
            palette_editor::render_palette_editor_window(
                ctx,
                &mut self.show_palette_editor,
                &mut self.palette_editor,
                &mut custom_palette,
                &mut palette_changed,
                &mut palette_export_json,
                &mut palette_save_file,
                &mut palette_import_json,
                &mut palette_load_file,
            );

            // Render Config Dialog window
            config_dialog::render_config_dialog(
                ctx,
                &mut self.show_config_window,
                &mut self.config_json_buffer,
                &mut config_export_json,
                &mut config_import_json,
                &mut config_save_file,
                &mut config_load_file,
            );
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
            palette_changed,
            color_mode_changed,
            pause_changed,
            config_export_requested: config_export_json,
            config_import_requested: config_import_json,
            config_save_file_requested: config_save_file,
            config_load_file_requested: config_load_file,
            custom_palette,
            undo_requested,
            redo_requested,
            background_color_changed,
            png_export_with_background,
            png_export_transparent,
            palette_export_json,
            palette_save_file,
            palette_import_json,
            palette_load_file,
            palette_imported: None,
            preset_changed,
            add_transform,
            delete_transform,
            render_mode_changed,
            projection_changed,
            camera_rotation_changed,
        }
    }
}
