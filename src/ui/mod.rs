mod config_dialog;
mod formatting;
mod help;
mod helpers;
mod lazy_undo;
mod menu_bar;
mod palette_editor;
mod panel_viewer;
mod performance;
mod response;
mod settings;
mod tone_mapping;
mod transforms;
mod triangle_editor;
mod undo_history;
mod variation_controls;
mod variation_params;
mod view;
mod workspace;

pub use lazy_undo::LazyUndoHelper;
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
    show_triangle_editor: bool,
    show_tone_mapping: bool,
    show_help: bool,
    show_undo_history: bool,
    // Lazy undo helpers for throttling undo captures during continuous drag
    lazy_undo_tone_mapping: LazyUndoHelper,
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
            show_triangle_editor: false,
            show_tone_mapping: true,  // Show by default
            show_help: false,
            show_undo_history: false,
            lazy_undo_tone_mapping: LazyUndoHelper::new(),
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
        config_manager: &mut crate::config::ConfigManager,
        flame_renderer: Option<&mut crate::renderer::compute_kernel::FlameRenderer>,
        flame: &mut crate::scene::transforms::Flame,
        palette_library: &crate::scene::palette::PaletteLibrary,
        preset_library: &crate::scene::presets::PresetLibrary,
        current_preset_index: &mut usize,
        paused: &mut bool,
        can_undo: bool,
        can_redo: bool,
    ) -> UiResponse {
        let raw_input = self.state.take_egui_input(window);

        // Note: Config change tracking now handled by ConfigManager.get_pending_actions()
        // Only non-config actions tracked here (file I/O, palette library, transforms, etc.)

        let mut pause_changed = false;
        let mut preset_changed = false;
        let mut add_transform = false;
        let mut delete_transform = None;

        // Config import/export
        let mut config_export_json = None;
        let mut config_import_json = None;
        let mut config_save_file = false;
        let mut config_load_file = false;
        let mut apophysis_import_file = false;

        // Palette library management
        let mut custom_palette = None;
        let mut palette_export_json = None;
        let mut palette_save_file = None;
        let mut palette_import_json = None;
        let mut palette_load_file = false;

        // Undo/redo
        let mut undo_requested = false;
        let mut redo_requested = false;

        // Export
        let mut png_export_with_background = false;
        let mut png_export_transparent = false;


        // Log ConfigManager state at start of UI render
        // log::debug!("render_ui start: ConfigManager has exposure={:.3}, gamma={:.3}",
        //     config_manager.config().exposure, config_manager.config().gamma);

        let full_output = self.ctx.run(raw_input, |ctx| {
            // Render menu bar
            menu_bar::render_menu_bar(
                ctx,
                &mut self.show_performance,
                &mut self.show_settings,
                &mut self.show_view,
                &mut self.show_transforms,
                &mut self.show_triangle_editor,
                &mut self.show_tone_mapping,
                &mut self.show_help,
                &mut self.show_palette_editor,
                &mut self.show_config_window,
                &mut self.show_undo_history,
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
            let _settings_update_type = settings::render_settings_window(
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
                flame_renderer.as_deref(),
                paused,
                &mut pause_changed,
                config_manager,
            );

            // Render View window
            let _view_update_type = view::render_view_window(
                ctx,
                &mut self.show_view,
                config_manager,
                flame,
            );

            // Render Help window
            help::render_help_window(ctx, &mut self.show_help);

            // Render Transforms window
            let _transforms_update_type = transforms::render_transforms_window(
                ctx,
                &mut self.show_transforms,
                config_manager,
                flame,
                &mut add_transform,
                &mut delete_transform,
            );

            // Render Triangle Editor window
            let _triangle_editor_update = triangle_editor::render_triangle_editor_window(
                ctx,
                &mut self.show_triangle_editor,
                config_manager,
                flame,
            );

            // Render Tone Mapping window
            let _tonemap_update = tone_mapping::render_tone_mapping_window(
                ctx,
                &mut self.show_tone_mapping,
                &mut self.show_palette_editor,
                config_manager,
                palette_library,
                &mut custom_palette,
                &mut self.lazy_undo_tone_mapping,
            );

            // Render Palette Editor window
            palette_editor::render_palette_editor_window(
                ctx,
                &mut self.show_palette_editor,
                &mut self.palette_editor,
                config_manager,
                &mut custom_palette,
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
                &mut apophysis_import_file,
            );

            // Render Undo/Redo History window
            undo_history::render_undo_history_window(
                ctx,
                &mut self.show_undo_history,
                config_manager,
                &mut undo_requested,
                &mut redo_requested,
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

        // Sync flame from ConfigManager AFTER UI updates (for live preview during drag)
        // This ensures app.rs gets the latest preview state when checking is_in_preview_mode()
        *flame = config_manager.active_config().flame.clone();

        UiResponse {
            pause_changed,
            config_export_requested: config_export_json,
            config_import_requested: config_import_json,
            config_save_file_requested: config_save_file,
            config_load_file_requested: config_load_file,
            apophysis_import_file_requested: apophysis_import_file,
            apophysis_import_configs: None,
            custom_palette,
            palette_export_json,
            palette_save_file,
            palette_import_json,
            palette_load_file,
            palette_imported: None,
            undo_requested,
            redo_requested,
            png_export_with_background,
            png_export_transparent,
            preset_changed,
            add_transform,
            delete_transform,
        }
    }
}
