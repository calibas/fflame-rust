//! TabViewer implementation for rendering docked panels

use egui_dock::{egui, TabViewer};
use super::workspace::PanelType;

/// Context needed by panels to render
///
/// Holds references to all UI state from EguiLayer that panels might need.
/// This avoids passing 20+ parameters to each panel.
pub struct PanelContext<'a> {
    // Core state
    pub config_manager: &'a mut crate::config::ConfigManager,
    pub flame: &'a mut crate::scene::transforms::Flame,

    // Libraries
    pub preset_library: &'a crate::scene::presets::PresetLibrary,
    pub palette_library: &'a crate::scene::palette::PaletteLibrary,

    // Renderer (optional, might not exist during init)
    pub flame_renderer: Option<&'a crate::renderer::compute_kernel::FlameRenderer>,

    // Window visibility flags
    pub show_config_window: &'a mut bool,

    // Action flags
    pub add_transform: &'a mut bool,
    pub delete_transform: &'a mut Option<usize>,
    pub undo_requested: &'a mut bool,
    pub redo_requested: &'a mut bool,
    pub preset_changed: &'a mut bool,
    pub pause_changed: &'a mut bool,
    pub open_palette_editor: &'a mut bool,

    // UI state
    pub current_preset_index: &'a mut usize,
    pub paused: &'a mut bool,
    pub png_export_with_background: &'a mut bool,
    pub png_export_transparent: &'a mut bool,
    pub custom_palette: &'a mut Option<crate::scene::palette::Palette>,
    pub palette_editor: &'a mut crate::ui::palette_editor::PaletteEditor,
    pub palette_export_json: &'a mut Option<crate::scene::palette::Palette>,
    pub palette_save_file: &'a mut Option<crate::scene::palette::Palette>,
    pub palette_import_json: &'a mut Option<String>,
    pub palette_load_file: &'a mut bool,

    // Performance metrics
    pub metrics: &'a crate::util::PerformanceMetrics,
    pub window_size: winit::dpi::PhysicalSize<u32>,

    // Fractal texture for display
    pub fractal_texture_id: Option<egui::TextureId>,

    // Config dialog state
    pub config_json_buffer: &'a mut String,
    pub config_export_json: &'a mut Option<String>,
    pub config_import_json: &'a mut Option<String>,
    pub config_save_file: &'a mut bool,
    pub config_load_file: &'a mut bool,
    pub apophysis_import_file: &'a mut bool,
    pub open_config_dialog: &'a mut bool,
}

/// Viewer for rendering each panel type
pub struct PanelViewer<'a> {
    pub context: PanelContext<'a>,
}

impl<'a> TabViewer for PanelViewer<'a> {
    type Tab = PanelType;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.to_string().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            PanelType::FractalViewport => {
                self.render_fractal_viewport(ui);
            }
            PanelType::Transforms => {
                self.render_transforms_panel(ui);
            }
            PanelType::TriangleEditor => {
                self.render_triangle_editor_panel(ui);
            }
            PanelType::Colors => {
                self.render_colors_panel(ui);
            }
            PanelType::PaletteEditor => {
                self.render_palette_editor_panel(ui);
            }
            PanelType::View => {
                self.render_view_panel(ui);
            }
            PanelType::Rendering => {
                self.render_rendering_panel(ui);
            }
            PanelType::History => {
                self.render_history_panel(ui);
            }
            PanelType::Performance => {
                self.render_performance_panel(ui);
            }
            PanelType::Help => {
                self.render_help_panel(ui);
            }
            PanelType::ConfigDialog => {
                self.render_config_dialog_panel(ui);
            }
        }
    }
}

impl<'a> PanelViewer<'a> {
    /// Render Transforms panel (transform list, affine, variations)
    fn render_transforms_panel(&mut self, ui: &mut egui::Ui) {
        let _ = super::transforms::render_transforms_content(
            ui,
            self.context.config_manager,
            self.context.flame,
            self.context.add_transform,
            self.context.delete_transform,
        );
    }

    /// Render Triangle Editor panel (visual triangle editing)
    fn render_triangle_editor_panel(&mut self, ui: &mut egui::Ui) {
        let _ = super::triangle_editor::render_triangle_editor_content(
            ui,
            self.context.config_manager,
            self.context.flame,
        );
    }

    /// Render Colors panel (color mode, palette, tone mapping)
    fn render_colors_panel(&mut self, ui: &mut egui::Ui) {
        let _ = super::tone_mapping::render_colors_content(
            ui,
            self.context.config_manager,
            self.context.palette_library,
            self.context.custom_palette,
            self.context.open_palette_editor,
        );
    }

    /// Render Palette Editor panel (palette editing)
    fn render_palette_editor_panel(&mut self, ui: &mut egui::Ui) {
        super::palette_editor::render_palette_editor_content(
            ui,
            self.context.palette_editor,
            self.context.config_manager,
            self.context.custom_palette,
            self.context.palette_export_json,
            self.context.palette_save_file,
            self.context.palette_import_json,
            self.context.palette_load_file,
        );
    }

    /// Render the View panel (zoom, pan, rotation)
    fn render_view_panel(&mut self, ui: &mut egui::Ui) {
        super::view::render_view_content(
            ui,
            self.context.config_manager,
            self.context.flame,
        );
    }

    /// Render the Rendering panel (iterations, accumulation)
    fn render_rendering_panel(&mut self, ui: &mut egui::Ui) {
        super::settings::render_settings_content(
            ui,
            self.context.show_config_window,
            self.context.config_manager.can_undo(),
            self.context.config_manager.can_redo(),
            self.context.undo_requested,
            self.context.redo_requested,
            self.context.png_export_with_background,
            self.context.png_export_transparent,
            self.context.preset_library,
            self.context.current_preset_index,
            self.context.preset_changed,
            self.context.flame,
            self.context.flame_renderer,
            self.context.paused,
            self.context.pause_changed,
            self.context.config_manager,
            self.context.open_config_dialog,
        );
    }


    /// Render the History panel (undo/redo browser)
    fn render_history_panel(&mut self, ui: &mut egui::Ui) {
        super::undo_history::render_undo_history_content(
            ui,
            self.context.config_manager,
            self.context.undo_requested,
            self.context.redo_requested,
        );
    }

    /// Render the Performance panel (stats and version info)
    fn render_performance_panel(&mut self, ui: &mut egui::Ui) {
        super::performance::render_performance_content(
            ui,
            self.context.metrics,
            self.context.window_size,
            self.context.flame_renderer,
        );
    }

    /// Render Help panel (keyboard shortcuts and documentation)
    fn render_help_panel(&mut self, ui: &mut egui::Ui) {
        super::help::render_help_content(ui);
    }

    /// Render Config Dialog panel (import/export configuration)
    fn render_config_dialog_panel(&mut self, ui: &mut egui::Ui) {
        super::config_dialog::render_config_dialog_content(
            ui,
            self.context.config_json_buffer,
            self.context.config_export_json,
            self.context.config_import_json,
            self.context.config_save_file,
            self.context.config_load_file,
            self.context.apophysis_import_file,
        );
    }

    /// Render Fractal Viewport (main fractal rendering area)
    fn render_fractal_viewport(&mut self, ui: &mut egui::Ui) {
        if let Some(texture_id) = self.context.fractal_texture_id {
            // Display the fractal texture, scaled to fill available space
            let available_size = ui.available_size();

            // Use SizeHint::Size to maintain aspect ratio while filling space
            let image = egui::Image::new(egui::load::SizedTexture::new(texture_id, available_size))
                .fit_to_exact_size(available_size)
                .maintain_aspect_ratio(false); // Fill entire panel

            ui.add(image);
        } else {
            // Fallback if texture not available yet
            ui.centered_and_justified(|ui| {
                ui.label("Initializing fractal renderer...");
            });
        }
    }
}
