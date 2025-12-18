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
    pub palette_library: &'a mut crate::scene::palette::PaletteLibrary,

    // Renderer (optional, might not exist during init)
    pub flame_renderer: Option<&'a crate::renderer::compute_kernel::FlameRenderer>,
    pub tiled_renderer: Option<&'a crate::export::TiledRenderer>,

    // Animation controller
    pub animation_controller: &'a mut crate::animation::AnimationController,

    // Action flags
    pub add_transform: &'a mut bool,
    pub delete_transform: &'a mut Option<usize>,
    pub undo_requested: &'a mut bool,
    pub redo_requested: &'a mut bool,
    pub preset_changed: &'a mut bool,
    pub open_palette_editor: &'a mut bool,

    // UI state
    pub current_preset_index: &'a mut usize,
    pub paused: &'a mut bool,
    pub png_export_with_background: &'a mut bool,
    pub png_export_transparent: &'a mut bool,
    pub export_width: &'a mut u32,
    pub export_height: &'a mut u32,
    pub use_custom_export_size: &'a mut bool,
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
    pub fractal_viewport_size: &'a mut Option<(u32, u32)>,

    // Config dialog state
    pub config_json_buffer: &'a mut String,
    pub config_export_json: &'a mut Option<String>,
    pub config_import_json: &'a mut Option<String>,
    pub config_save_file: &'a mut bool,
    pub config_load_file: &'a mut bool,
    pub apophysis_import_file: &'a mut bool,
    pub open_config_dialog: &'a mut bool,

    // Preset library panel state
    pub preset_library_panel: &'a mut Option<super::preset_library::PresetLibraryPanel>,
    pub selected_preset_config: &'a mut Option<crate::config::FractalConfig>,

    // File browser panel state
    pub file_browser_panel: &'a mut Option<super::file_browser::FileBrowserPanel>,
    pub file_browser_open_requested: &'a mut bool,

    // Animation export settings
    pub animation_export_settings: &'a mut super::animation_panel::AnimationExportSettings,
    pub animation_export_requested: &'a mut Option<super::animation_panel::AnimationExportSettings>,
    pub animation_export_progress: &'a super::animation_panel::ExportProgress,

    // Track editor state
    pub track_editor_state: &'a mut super::track_editor::TrackEditorState,

    // Animation seek changed flag (timeline was scrubbed)
    pub animation_seek_changed: &'a mut bool,
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
            PanelType::PaletteLibrary => {
                self.render_palette_library_panel(ui);
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
            PanelType::Animation => {
                self.render_animation_panel(ui);
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
            PanelType::PresetLibrary => {
                self.render_preset_library_panel(ui);
            }
            PanelType::FileBrowser => {
                self.render_file_browser_panel(ui);
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

    /// Render Palette Library panel (browse and manage palette packs)
    fn render_palette_library_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(selected_palette) = super::palette_library::render_palette_library(
            ui,
            self.context.palette_library,
        ) {
            // Update custom palette (will be applied to config via ConfigManager)
            *self.context.custom_palette = Some(selected_palette);
        }
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
            self.context.png_export_with_background,
            self.context.png_export_transparent,
            self.context.export_width,
            self.context.export_height,
            self.context.use_custom_export_size,
            self.context.preset_library,
            self.context.current_preset_index,
            self.context.preset_changed,
            self.context.flame,
            self.context.flame_renderer,
            self.context.tiled_renderer,
            self.context.paused,
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

    /// Render Animation panel (playback controls, timeline)
    fn render_animation_panel(&mut self, ui: &mut egui::Ui) {
        let response = super::animation_panel::render_animation_content(
            ui,
            self.context.animation_controller,
            self.context.animation_export_settings,
            self.context.animation_export_progress,
        );

        // Handle new animation request
        if response.new_animation {
            let new_anim = crate::animation::Animation::new("New Animation".to_string(), 10.0);
            self.context.animation_controller.load(new_anim);
        }

        // Handle animation load response
        if let Some(animation) = response.load_animation {
            // If animation has embedded config, load it first
            if let Some(ref config) = animation.base_config {
                log::info!("Animation '{}' has embedded config, loading it", animation.name);
                let description = format!("Load Animation: {}", animation.name);
                if let Err(e) = self.context.config_manager.load_config(config.clone(), description) {
                    log::error!("Failed to load animation's embedded config: {}", e);
                }
                // Mark that preset changed (triggers GPU update)
                *self.context.preset_changed = true;
            }
            self.context.animation_controller.load(animation);
        }

        // Handle animation save response
        if response.save_animation {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(ref animation) = self.context.animation_controller.animation {
                // Clone animation and embed current config
                let mut animation_with_config = animation.clone();
                animation_with_config.set_base_config(self.context.config_manager.active_config().clone());

                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Animation", &["anim", "json"])
                    .set_file_name(&format!("{}.anim", animation_with_config.name))
                    .save_file()
                {
                    match animation_with_config.to_json() {
                        Ok(json) => {
                            if let Err(e) = std::fs::write(&path, json) {
                                log::error!("Failed to save animation: {}", e);
                            } else {
                                log::info!("Saved animation with embedded config to {:?}", path);
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to serialize animation: {}", e);
                        }
                    }
                }
            }
        }

        // Handle animation export request
        if let Some(settings) = response.export_animation {
            *self.context.animation_export_requested = Some(settings);
        }

        // Handle timeline scrubbing
        if response.seek_changed {
            *self.context.animation_seek_changed = true;
        }

        // Track editor section
        ui.separator();
        super::track_editor::render_track_editor(
            ui,
            self.context.animation_controller,
            self.context.track_editor_state,
            self.context.flame,
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
        // Set panel background to match fractal background color
        // This allows the fractal texture to have transparent edges that blend naturally
        let bg_color = self.context.config_manager.active_config().background_color;
        let bg_color32 = egui::Color32::from_rgb(
            (bg_color[0] * 255.0) as u8,
            (bg_color[1] * 255.0) as u8,
            (bg_color[2] * 255.0) as u8,
        );

        // Override the panel's background color
        ui.visuals_mut().panel_fill = bg_color32;

        if let Some(texture_id) = self.context.fractal_texture_id {
            // Get the actual panel size and report it for texture sizing
            let available_size = ui.available_size();
            let width = available_size.x.max(1.0) as u32;
            let height = available_size.y.max(1.0) as u32;

            // Report the size back so texture can be resized to match
            *self.context.fractal_viewport_size = Some((width, height));

            // Display the fractal texture with drag and scroll interaction
            let image = egui::Image::new(egui::load::SizedTexture::new(texture_id, available_size))
                .fit_to_exact_size(available_size)
                .maintain_aspect_ratio(false) // Fill entire panel
                .sense(egui::Sense::click_and_drag()); // Enable drag interaction

            let response = ui.add(image);

            // Handle mouse drag for panning
            if response.dragged_by(egui::PointerButton::Primary) {
                let drag_delta = response.drag_delta();
                self.handle_fractal_drag(drag_delta, available_size);
            }

            // Handle mouse wheel for zooming
            if response.hovered() {
                let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
                if scroll_delta.abs() > 0.1 {
                    self.handle_fractal_scroll(scroll_delta, response.hover_pos(), response.rect, available_size);
                }
            }
        } else {
            // Fallback if texture not available yet
            ui.centered_and_justified(|ui| {
                ui.label("Initializing fractal renderer...");
            });
        }
    }

    /// Handle fractal panning via mouse drag
    fn handle_fractal_drag(&mut self, drag_delta: egui::Vec2, panel_size: egui::Vec2) {
        let config = self.context.config_manager.active_config();

        // Convert screen space delta to fractal space
        // Invert both X and Y (drag right = pan left, drag down = pan up)
        let screen_dx = -drag_delta.x / panel_size.x;
        let screen_dy = -drag_delta.y / panel_size.y;

        // Apply rotation (negate to convert screen to fractal space)
        let cos_r = (-config.rotation).cos();
        let sin_r = (-config.rotation).sin();

        // Scale by zoom (4.0 is the full visible range: -2 to +2)
        let scale = 4.0 / config.zoom;
        let fractal_dx = (screen_dx * cos_r - screen_dy * sin_r) * scale;
        let fractal_dy = (screen_dx * sin_r + screen_dy * cos_r) * scale;

        let new_pan_x = config.pan_x + fractal_dx;
        let new_pan_y = config.pan_y + fractal_dy;

        let _ = self.context.config_manager.update_param(
            crate::config::ConfigPath::Pan,
            (new_pan_x, new_pan_y).into()
        );
    }

    /// Handle fractal zooming via mouse wheel
    fn handle_fractal_scroll(
        &mut self,
        scroll_delta: f32,
        mouse_pos: Option<egui::Pos2>,
        panel_rect: egui::Rect,
        panel_size: egui::Vec2,
    ) {
        let config = self.context.config_manager.active_config();

        // Use power-based zoom for smooth scrolling (matches original code)
        let zoom_factor = if scroll_delta.abs() > 0.1 {
            1.1f32.powf(scroll_delta * 0.05)
        } else {
            1.0
        };

        if zoom_factor != 1.0 {
            // Zoom in toward cursor, zoom out from center
            if zoom_factor > 1.0 {
                // Zooming in - zoom toward mouse cursor position
                if let Some(mouse_pos) = mouse_pos {
                    // Convert mouse position from panel space to fractal space
                    // Panel center
                    let center_x = panel_rect.center().x;
                    let center_y = panel_rect.center().y;

                    // Mouse offset from center in panel pixels
                    let mouse_offset_x = mouse_pos.x - center_x;
                    let mouse_offset_y = mouse_pos.y - center_y;

                    // Convert to fractal space (account for current zoom, scale, and rotation)
                    let scale = f32::min(panel_size.x, panel_size.y) * 0.25;

                    // Apply rotation to convert screen space to fractal space
                    let cos_r = (-config.rotation).cos();
                    let sin_r = (-config.rotation).sin();
                    let rotated_offset_x = mouse_offset_x * cos_r - mouse_offset_y * sin_r;
                    let rotated_offset_y = mouse_offset_x * sin_r + mouse_offset_y * cos_r;

                    let fractal_offset_x = rotated_offset_x / (scale * config.zoom);
                    let fractal_offset_y = rotated_offset_y / (scale * config.zoom);

                    // Calculate the point in fractal space that the mouse is pointing at
                    let point_x = config.pan_x + fractal_offset_x;
                    let point_y = config.pan_y + fractal_offset_y;

                    // Apply zoom and adjust pan (also need rotation for new zoom level)
                    let new_zoom = (config.zoom * zoom_factor).clamp(0.01, 1000.0);
                    let new_rotated_offset_x = mouse_offset_x * cos_r - mouse_offset_y * sin_r;
                    let new_rotated_offset_y = mouse_offset_x * sin_r + mouse_offset_y * cos_r;
                    let new_fractal_offset_x = new_rotated_offset_x / (scale * new_zoom);
                    let new_fractal_offset_y = new_rotated_offset_y / (scale * new_zoom);
                    let new_pan_x = point_x - new_fractal_offset_x;
                    let new_pan_y = point_y - new_fractal_offset_y;

                    // Update zoom and pan atomically
                    let _ = self.context.config_manager.update_batch(
                        vec![
                            (crate::config::ConfigPath::Zoom, new_zoom.into()),
                            (crate::config::ConfigPath::Pan, (new_pan_x, new_pan_y).into()),
                        ],
                        "Zoom In (Wheel)".to_string()
                    );
                } else {
                    // No mouse position, zoom to center
                    let new_zoom = (config.zoom * zoom_factor).clamp(0.01, 1000.0);
                    let _ = self.context.config_manager.update_param(
                        crate::config::ConfigPath::Zoom,
                        new_zoom.into()
                    );
                }
            } else {
                // Zooming out - always zoom from center
                let new_zoom = (config.zoom * zoom_factor).clamp(0.01, 1000.0);
                let _ = self.context.config_manager.update_param(
                    crate::config::ConfigPath::Zoom,
                    new_zoom.into()
                );
            }
        }
    }

    /// Render Preset Library panel (browse and select presets with thumbnails)
    fn render_preset_library_panel(&mut self, ui: &mut egui::Ui) {
        // Initialize panel if not already created
        if self.context.preset_library_panel.is_none() {
            *self.context.preset_library_panel = Some(
                super::preset_library::PresetLibraryPanel::new(self.context.preset_library)
            );
        }

        if let Some(panel) = self.context.preset_library_panel.as_mut() {
            let response = panel.render(ui);

            // Handle selection
            if let Some(config) = response.selected {
                *self.context.selected_preset_config = Some(config);
            }
        }
    }

    /// Render File Browser panel (browse and load .fflame files)
    fn render_file_browser_panel(&mut self, ui: &mut egui::Ui) {
        // Initialize panel if not already created
        if self.context.file_browser_panel.is_none() {
            *self.context.file_browser_panel = Some(super::file_browser::FileBrowserPanel::new());
        }

        if let Some(panel) = self.context.file_browser_panel.as_mut() {
            let response = panel.render(ui);

            // Handle file open request
            if panel.take_open_file_request() {
                *self.context.file_browser_open_requested = true;
            }

            // Handle selection (load the config)
            if let Some(config) = response.selected {
                *self.context.selected_preset_config = Some(config);
            }
        }
    }
}
