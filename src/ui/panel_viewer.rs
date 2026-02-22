//! TabViewer implementation for rendering docked panels

use egui_dock::{egui, TabViewer};
use rust_i18n::t;
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

    // Animation controller
    pub animation_controller: &'a mut crate::animation::AnimationController,

    // Action flags
    pub add_transform: &'a mut bool,
    pub delete_transform: &'a mut Option<usize>,
    pub clone_transform: &'a mut Option<usize>,
    pub undo_requested: &'a mut bool,
    pub redo_requested: &'a mut bool,
    pub open_palette_editor: &'a mut bool,
    pub open_palette_library: &'a mut bool,
    pub open_triangle_editor: &'a mut bool,
    pub open_preset_library: &'a mut bool,
    pub open_random_generator: &'a mut bool,

    // UI state
    pub paused: &'a mut bool,
    pub png_export_with_background: &'a mut bool,
    pub png_export_transparent: &'a mut bool,
    pub export_width: &'a mut u32,
    pub export_height: &'a mut u32,
    pub use_custom_export_size: &'a mut bool,
    pub palette_editor: &'a mut crate::ui::palette_editor::PaletteEditor,
    pub palette_export_json: &'a mut Option<crate::scene::palette::Palette>,
    pub palette_save_file: &'a mut Option<crate::scene::palette::Palette>,
    pub palette_save_to_library: &'a mut Option<crate::scene::palette::Palette>,
    pub palette_delete_from_library: &'a mut Option<String>,
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

    // Selected preset config (from any browser)
    pub selected_preset_config: &'a mut Option<crate::config::FractalConfig>,

    // API flame ID loaded from Online tab
    pub loaded_api_flame_id: &'a mut Option<String>,

    // API notification from browser panel (e.g., delete result)
    pub api_notification: &'a mut Option<(String, bool)>,

    // Login dialog state
    pub login_dialog_state: &'a mut super::login_dialog::LoginDialogState,

    // Save Online dialog state
    pub save_online_dialog_state: &'a mut super::save_online_dialog::SaveOnlineDialogState,

    // Cloud palette state (for Palette Library cloud section)
    pub cloud_palette_state: &'a mut super::CloudPaletteState,

    // File browser open request (shared by FractalBrowser)
    pub file_browser_open_requested: &'a mut bool,

    // Animation export settings
    pub animation_export_settings: &'a mut super::animation_panel::AnimationExportSettings,
    pub animation_export_requested: &'a mut Option<super::animation_panel::AnimationExportSettings>,
    pub animation_export_progress: &'a super::animation_panel::ExportProgress,

    // Export Animation panel state (Phase 5)
    pub export_panel_state: &'a mut super::animation_panel::ExportPanelState,

    // Track editor state
    pub track_editor_state: &'a mut super::track_editor::TrackEditorState,

    // Animation seek changed flag (timeline was scrubbed)
    pub animation_seek_changed: &'a mut bool,

    // Animation scrubber drag stopped or discrete seek action (frame step) - reset accumulation
    pub animation_seek_drag_stopped: &'a mut bool,

    // PathMap mode: hovered pixel coordinates and cached path info
    pub hovered_pixel: &'a mut Option<(u32, u32)>,
    pub path_click_info: &'a Option<super::PathClickInfo>,
    pub close_path_overlay: &'a mut bool,

    // Path editor state
    pub path_editor_state: &'a mut super::path_editor::PathEditorState,
    pub path_filters_changed: &'a mut Option<Vec<crate::gpu::buffers::GpuPathFilter>>,

    // PNG export progress
    pub png_export_progress: &'a super::export_panel::PngExportProgress,

    // Random generator panel state
    pub random_generator_panel: &'a mut Option<super::random_generator::RandomGeneratorPanel>,
    pub generated_flame: &'a mut Option<crate::scene::transforms::Flame>,
    pub generated_batch: &'a mut Option<Vec<crate::config::FractalConfig>>,

    // Fractal browser panel state (unified presets/batch/files)
    pub fractal_browser_panel: &'a mut Option<super::fractal_browser::FractalBrowserPanel>,

    // Histogram for density visualization (levels now in ConfigManager)
    pub density_histogram: &'a crate::renderer::DensityHistogram,

    // Xaos editor state
    pub xaos_editor_state: &'a mut super::xaos_editor::XaosEditorState,

    // Signal panel state
    pub audio_manager: &'a mut crate::audio::AudioManager,
    pub audio_player: &'a mut crate::audio::AudioPlayer,
    pub audio_capture: &'a mut crate::audio::AudioCapture,
    pub signal_panel_state: &'a mut super::signal_panel::SignalPanelState,
    pub signal_manager: &'a mut crate::signal::SignalManager,
    pub current_time: f64,
    pub load_audio_file: &'a mut bool,
    pub load_signal_file: &'a mut bool,
    pub save_signal_file: &'a mut Option<String>,
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
            PanelType::KeyboardShortcuts => {
                self.render_keyboard_shortcuts_panel(ui);
            }
            PanelType::ConfigDialog => {
                self.render_config_dialog_panel(ui);
            }
            PanelType::FractalBrowser => {
                self.render_fractal_browser_panel(ui);
            }
            PanelType::PathEditor => {
                self.render_path_editor_panel(ui);
            }
            PanelType::Export => {
                self.render_export_panel(ui);
            }
            PanelType::RandomGenerator => {
                self.render_random_generator_panel(ui);
            }
            PanelType::Effects => {
                self.render_effects_panel(ui);
            }
            PanelType::XaosEditor => {
                self.render_xaos_editor_panel(ui);
            }
            PanelType::Signal => {
                self.render_signal_panel(ui);
            }
            PanelType::LoginDialog => {
                self.render_login_dialog(ui);
            }
            PanelType::SaveOnlineDialog => {
                self.render_save_online_dialog(ui);
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
            self.context.clone_transform,
            self.context.open_triangle_editor,
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
            self.context.open_palette_editor,
            self.context.open_palette_library,
            self.context.density_histogram,
        );
    }

    /// Render Palette Editor panel (palette editing)
    fn render_palette_editor_panel(&mut self, ui: &mut egui::Ui) {
        // Capture palette_library reference for the closure
        let palette_library = &self.context.palette_library;
        super::palette_editor::render_palette_editor_content(
            ui,
            self.context.palette_editor,
            self.context.config_manager,
            self.context.palette_export_json,
            self.context.palette_save_file,
            self.context.palette_save_to_library,
            self.context.palette_delete_from_library,
            self.context.palette_import_json,
            self.context.palette_load_file,
            self.context.open_palette_library,
            |name| palette_library.has_custom_palette_named(name),
        );
    }

    /// Render Palette Library panel (browse and manage palette packs)
    fn render_palette_library_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            super::palette_library::render_palette_library(
                ui,
                self.context.palette_library,
                self.context.config_manager,
                self.context.open_palette_editor,
            );

            let (online_mode, auth_pair) = {
                let settings = self.context.config_manager.system_settings();
                let online = settings.online_mode;
                let auth = settings.auth_token.as_ref().map(|token| {
                    (settings.api_base_url.clone(), token.clone())
                });
                (online, auth)
            };
            if online_mode {
                let auth = auth_pair.as_ref().map(|(b, t)| (b.as_str(), t.as_str()));
                super::palette_library::render_cloud_palettes_section(
                    ui,
                    self.context.cloud_palette_state,
                    self.context.config_manager,
                    auth,
                );
            }
        });
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
            self.context.flame_renderer,
            self.context.paused,
            self.context.config_manager,
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
        // Ensure animation always exists (animation is always present with 0 tracks by default)
        if self.context.animation_controller.animation.is_none() {
            let new_anim = crate::animation::Animation::new("New Animation".to_string(), 10.0);
            self.context.animation_controller.load(new_anim);
        }

        let mut response = super::animation_panel::render_animation_content(
            ui,
            self.context.animation_controller,
            self.context.animation_export_settings,
        );

        // Handle timeline scrubbing (from render_animation_content)
        if response.seek_changed {
            *self.context.animation_seek_changed = true;
        }
        if response.seek_drag_stopped {
            *self.context.animation_seek_drag_stopped = true;
        }

        // Track editor section with visual bars aligned to timeline
        ui.separator();
        let track_response = super::track_editor::render_track_editor(
            ui,
            self.context.animation_controller,
            self.context.track_editor_state,
            self.context.config_manager.active_config(),
            response.timeline_layout,
        );

        // Handle seek from clicking on track bars (Phase 3) - discrete action, needs reset
        if let Some(time) = track_response.seek_to_time {
            self.context.animation_controller.seek(time);
            *self.context.animation_seek_changed = true;
            *self.context.animation_seek_drag_stopped = true;
        }

        // Export progress (after tracks section)
        if self.context.animation_export_progress.is_exporting {
            ui.separator();
            super::animation_panel::render_export_progress(ui, self.context.animation_export_progress);
        }

        // File controls (after tracks section)
        ui.separator();
        super::animation_panel::render_file_controls(ui, self.context.animation_controller, &mut response);

        // Handle file control responses (must be after render_file_controls)
        // Handle open export panel request (Phase 5)
        if response.open_export_panel {
            self.context.export_panel_state.is_open = true;
            // Auto-fill export settings from current config
            self.context.animation_export_settings.iterations_per_thread =
                self.context.config_manager.system_settings().iterations_per_thread;
            self.context.animation_export_settings.max_iterations =
                self.context.config_manager.active_config().max_iterations;
        }

        // Handle animation load response
        if let Some(animation) = response.load_animation.take() {
            // If animation has embedded config, load it via selected_preset_config
            // This ensures proper GPU sync and undo/redo handling
            if let Some(config) = animation.base_config.clone() {
                log::info!("Animation '{}' has embedded config, loading it", animation.name);
                *self.context.selected_preset_config = Some(config);
            }
            let generators = animation.generators.clone();
            let duration = animation.duration;
            self.context.animation_controller.load(animation);
            self.context.signal_panel_state.restore_generators(
                generators, self.context.signal_manager, duration,
            );
        }

        // Handle animation load trigger (WASM only - uses native file picker)
        #[cfg(target_arch = "wasm32")]
        if response.trigger_animation_load {
            let ctx = ui.ctx().clone();
            crate::app::trigger_browser_file_picker(".anim,.json", ctx, "pending_animation_load_raw");
        }

        // Handle animation save response
        if response.save_animation {
            // Sync generators from panel state → animation before saving
            if let Some(ref mut animation) = self.context.animation_controller.animation {
                animation.generators = self.context.signal_panel_state.generators.clone();
            }

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

            #[cfg(target_arch = "wasm32")]
            if let Some(ref animation) = self.context.animation_controller.animation {
                // Clone animation and embed current config
                let mut animation_with_config = animation.clone();
                animation_with_config.set_base_config(self.context.config_manager.active_config().clone());

                match animation_with_config.to_json() {
                    Ok(json) => {
                        let filename = format!("{}.anim", animation_with_config.name.to_lowercase().replace(' ', "_"));
                        if let Err(e) = crate::app::trigger_browser_download(json.as_bytes(), &filename, "application/json") {
                            log::error!("Failed to trigger animation download: {}", e);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to serialize animation: {}", e);
                    }
                }
            }
        }

        // Handle animation export request
        if let Some(settings) = response.export_animation.take() {
            *self.context.animation_export_requested = Some(settings);
        }
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

    /// Render Help panel (intro and links)
    fn render_help_panel(&mut self, ui: &mut egui::Ui) {
        super::help::render_help_panel_content(
            ui,
            self.context.config_manager,
            self.context.open_preset_library,
            self.context.open_random_generator,
        );
    }

    /// Render Keyboard Shortcuts panel
    fn render_keyboard_shortcuts_panel(&mut self, ui: &mut egui::Ui) {
        super::help::render_keyboard_shortcuts_content(ui);
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

            // Handle right-click (or drag with right button held) to query path at pixel (PathMap mode)
            // Use down() to detect when button is held, allowing continuous updates while dragging
            let secondary_held = ui.input(|i| i.pointer.secondary_down());
            if secondary_held && response.hovered() {
                if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    // Convert from panel coordinates to texture coordinates
                    let local_x = pointer_pos.x - response.rect.min.x;
                    let local_y = pointer_pos.y - response.rect.min.y;
                    let pixel_x = (local_x / available_size.x * width as f32) as u32;
                    let pixel_y = (local_y / available_size.y * height as f32) as u32;
                    *self.context.hovered_pixel = Some((pixel_x.min(width - 1), pixel_y.min(height - 1)));
                }
            }

            // Display path info overlay when available (PathMap mode only)
            let is_path_map_mode = self.context.config_manager.active_config().color_mode
                == crate::scene::palette::ColorMode::PathMap;
            if is_path_map_mode {
                if let Some(click_info) = self.context.path_click_info {
                    self.render_path_overlay(ui, &response, click_info);
                }
            }
        } else {
            // Fallback if texture not available yet
            ui.centered_and_justified(|ui| {
                ui.label(t!("viewport.initializing"));
            });
        }
    }

    /// Handle fractal panning via mouse drag
    fn handle_fractal_drag(&mut self, drag_delta: egui::Vec2, panel_size: egui::Vec2) {
        let config = self.context.config_manager.active_config();

        // Convert screen pixel delta to fractal space
        // Use width for both axes so drag speed is uniform regardless of aspect ratio
        // (dividing Y by panel_size.y made vertical panning slow on tall viewports)
        let scale = 4.0 / (config.zoom * panel_size.x);
        let dx = -drag_delta.x * scale;
        let dy = -drag_delta.y * scale;

        // Apply rotation (negate to convert screen to fractal space)
        let cos_r = (-config.rotation).cos();
        let sin_r = (-config.rotation).sin();

        let fractal_dx = dx * cos_r - dy * sin_r;
        let fractal_dy = dx * sin_r + dy * cos_r;

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
                        "history.action.wheel_zoom".to_string()
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

    /// Render path overlay showing pixel info, coordinates, path, and color preview
    fn render_path_overlay(
        &mut self,
        ui: &mut egui::Ui,
        _image_response: &egui::Response,
        click_info: &super::PathClickInfo,
    ) {
        // Get transform names for display
        let flame = &self.context.config_manager.active_config().flame;
        let transform_count = flame.transforms.len();

        // Build path string
        let path_vec = click_info.path_entry.to_vec();

        // Format path: show transform indices and names
        let path_str: Vec<String> = path_vec.iter().map(|&idx| {
            let idx = idx as usize;
            if idx < transform_count {
                format!("T{}", idx)
            } else {
                format!("?{}", idx)
            }
        }).collect();

        // Create overlay window anchored to top-left of viewport
        egui::Area::new(egui::Id::new("path_overlay"))
            .fixed_pos(ui.min_rect().min + egui::vec2(10.0, 10.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 220))
                    .inner_margin(egui::Margin::same(10))
                    .corner_radius(egui::CornerRadius::same(6))
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);

                        // Header row with close button
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(t!("path_overlay.title")).strong().color(egui::Color32::WHITE));

                            // Push close button to the right
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("X").clicked() {
                                    *self.context.close_path_overlay = true;
                                }
                            });
                        });

                        ui.add_space(6.0);

                        // Two-column layout: info on left, color preview on right
                        ui.horizontal(|ui| {
                            // Left column: coordinates and path info
                            ui.vertical(|ui| {
                                ui.set_min_width(280.0);

                                // Pixel coordinates section
                                ui.label(egui::RichText::new(t!("path_overlay.coordinates")).strong().color(egui::Color32::LIGHT_GRAY));
                                ui.add_space(2.0);

                                // View space (pixel) coordinates
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.pixel")).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("({}, {})",
                                        click_info.found_pixel.0, click_info.found_pixel.1))
                                        .color(egui::Color32::WHITE));
                                    if click_info.search_distance > 0.0 {
                                        ui.label(egui::RichText::new(t!("path_overlay.pixel_offset", distance = format!("{:.1}", click_info.search_distance)))
                                            .small()
                                            .color(egui::Color32::YELLOW));
                                    }
                                });

                                // Fractal space coordinates
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.fractal")).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("({:.6}, {:.6})",
                                        click_info.fractal_coords.0, click_info.fractal_coords.1))
                                        .color(egui::Color32::LIGHT_GREEN));
                                });

                                // IFS starting point
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.ifs_start")).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("({:.4}, {:.4})",
                                        click_info.path_entry.initial_x, click_info.path_entry.initial_y))
                                        .color(egui::Color32::LIGHT_BLUE));
                                });

                                ui.add_space(6.0);

                                // Path section
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.path_label")).strong().color(egui::Color32::LIGHT_GRAY));
                                    ui.label(egui::RichText::new(t!("path_overlay.path_iterations", count = click_info.path_entry.iteration_count))
                                        .small()
                                        .color(egui::Color32::GRAY));
                                });
                                ui.add_space(2.0);

                                // Wrap path in a scrollable area if it's long
                                if !path_str.is_empty() {
                                    egui::ScrollArea::horizontal().max_width(260.0).show(ui, |ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            for (i, name) in path_str.iter().enumerate() {
                                                if i > 0 {
                                                    ui.label(egui::RichText::new(">").color(egui::Color32::DARK_GRAY));
                                                }
                                                ui.label(egui::RichText::new(name).color(egui::Color32::from_rgb(100, 180, 255)));
                                            }
                                        });
                                    });
                                } else {
                                    ui.label(egui::RichText::new(t!("path_overlay.path_empty")).color(egui::Color32::GRAY));
                                }

                                ui.add_space(6.0);

                                // Hash debug info (shows Prefix Distinct calculation)
                                use crate::renderer::PathEntry;
                                let prefix = click_info.path_entry.get_prefix();
                                let iter_count = click_info.path_entry.iteration_count;
                                // Mix iteration_count into value before hashing (matches GPU)
                                let mixed = prefix ^ (iter_count.wrapping_mul(0x9E3779B9));
                                let hash = PathEntry::scramble_hash(mixed);
                                let hue = click_info.path_entry.compute_prefix_distinct_hue();
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.debug_prefix_distinct")).strong().color(egui::Color32::LIGHT_GRAY));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.debug_path0", value = format!("{:08X}", prefix)))
                                        .small()
                                        .color(egui::Color32::YELLOW));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.debug_mixed", value = format!("{:08X}", mixed)))
                                        .small()
                                        .color(egui::Color32::YELLOW));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.debug_hash", value = format!("{:08X}", hash)))
                                        .small()
                                        .color(egui::Color32::YELLOW));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.debug_hue", value = format!("{:.6}", hue)))
                                        .small()
                                        .color(egui::Color32::YELLOW));
                                });
                            });

                            ui.add_space(12.0);

                            // Right column: 9x9 color preview (clickable)
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(t!("path_overlay.preview_title")).strong().color(egui::Color32::LIGHT_GRAY));
                                ui.add_space(4.0);

                                // Render color grid
                                let (preview_w, preview_h) = click_info.preview_size;
                                let pixel_size = 12.0;
                                let total_size = egui::vec2(
                                    preview_w as f32 * pixel_size,
                                    preview_h as f32 * pixel_size,
                                );

                                // Make clickable to allow selecting other pixels
                                let (rect, response) = ui.allocate_exact_size(total_size, egui::Sense::click());
                                let painter = ui.painter();

                                // Handle click on preview to select a different pixel
                                if response.clicked() {
                                    if let Some(click_pos) = response.interact_pointer_pos() {
                                        // Calculate which cell was clicked
                                        let local_x = click_pos.x - rect.min.x;
                                        let local_y = click_pos.y - rect.min.y;
                                        let cell_x = (local_x / pixel_size) as i32;
                                        let cell_y = (local_y / pixel_size) as i32;

                                        // Calculate offset from center
                                        let center_x = preview_w as i32 / 2;
                                        let center_y = preview_h as i32 / 2;
                                        let offset_x = cell_x - center_x;
                                        let offset_y = cell_y - center_y;

                                        // Calculate new target pixel
                                        let (found_x, found_y) = click_info.found_pixel;
                                        let new_x = (found_x as i32 + offset_x).max(0) as u32;
                                        let new_y = (found_y as i32 + offset_y).max(0) as u32;

                                        // Update hovered_pixel to trigger re-query
                                        *self.context.hovered_pixel = Some((new_x, new_y));
                                    }
                                }

                                // Draw border around preview
                                painter.add(egui::epaint::RectShape::stroke(
                                    rect.expand(1.0),
                                    egui::CornerRadius::same(2),
                                    egui::Stroke::new(1.0, egui::Color32::GRAY),
                                    egui::epaint::StrokeKind::Outside,
                                ));

                                // Draw each pixel
                                for py in 0..preview_h {
                                    for px in 0..preview_w {
                                        let idx = (py * preview_w + px) as usize;
                                        if idx < click_info.color_preview.len() {
                                            let rgba = click_info.color_preview[idx];
                                            let color = egui::Color32::from_rgba_unmultiplied(
                                                rgba[0], rgba[1], rgba[2], rgba[3]
                                            );

                                            let pixel_rect = egui::Rect::from_min_size(
                                                rect.min + egui::vec2(px as f32 * pixel_size, py as f32 * pixel_size),
                                                egui::vec2(pixel_size, pixel_size),
                                            );

                                            painter.rect_filled(pixel_rect, 0.0, color);

                                            // Highlight center pixel with black and white outline (not filled)
                                            let center_x = preview_w / 2;
                                            let center_y = preview_h / 2;
                                            if px == center_x && py == center_y {
                                                // Outer black stroke
                                                painter.add(egui::epaint::RectShape::stroke(
                                                    pixel_rect.shrink(0.5),
                                                    egui::CornerRadius::ZERO,
                                                    egui::Stroke::new(2.0, egui::Color32::BLACK),
                                                    egui::epaint::StrokeKind::Inside,
                                                ));
                                                // Inner white stroke
                                                painter.add(egui::epaint::RectShape::stroke(
                                                    pixel_rect.shrink(2.5),
                                                    egui::CornerRadius::ZERO,
                                                    egui::Stroke::new(1.0, egui::Color32::WHITE),
                                                    egui::epaint::StrokeKind::Inside,
                                                ));
                                            }
                                        }
                                    }
                                }
                            });
                        });

                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(t!("path_overlay.close_hint"))
                            .small()
                            .color(egui::Color32::DARK_GRAY));
                    });
            });
    }

    /// Render Path Editor panel (manage path filters)
    fn render_path_editor_panel(&mut self, ui: &mut egui::Ui) {
        let num_transforms = self.context.flame.transforms.len();
        let response = super::path_editor::render_path_editor_content(
            ui,
            self.context.path_editor_state,
            num_transforms,
        );

        // Handle filter changes
        if let Some(filters) = response.filters_changed {
            *self.context.path_filters_changed = Some(filters);
        }
    }

    /// Render Export panel (PNG export options)
    fn render_export_panel(&mut self, ui: &mut egui::Ui) {
        super::export_panel::render_export_content(
            ui,
            self.context.png_export_with_background,
            self.context.png_export_transparent,
            self.context.export_width,
            self.context.export_height,
            self.context.use_custom_export_size,
            self.context.config_manager,
            *self.context.fractal_viewport_size,
            self.context.png_export_progress,
        );
    }

    /// Render Random Generator panel (generate random flames with settings)
    fn render_random_generator_panel(&mut self, ui: &mut egui::Ui) {
        // Initialize panel if not already created
        if self.context.random_generator_panel.is_none() {
            *self.context.random_generator_panel = Some(super::random_generator::RandomGeneratorPanel::new());
        }

        if let Some(panel) = self.context.random_generator_panel.as_mut() {
            let response = panel.render(ui);

            // Handle generate single request
            if response.generate_single {
                let flame = crate::scene::randomize::generate_random_flame_with_settings(&panel.settings);
                *self.context.generated_flame = Some(flame);
            }

            // Handle generate batch request - create configs with palettes, open File Browser
            if response.generate_batch {
                let flames = crate::scene::randomize::generate_batch(&panel.settings);
                log::info!("Generated batch of {} flames", flames.len());

                // Convert flames to FractalConfigs with palettes
                let use_random_palette = panel.settings.random_palette;
                let palette_count = self.context.palette_library.len();

                let configs: Vec<crate::config::FractalConfig> = flames
                    .into_iter()
                    .enumerate()
                    .map(|(i, flame)| {
                        let mut config = crate::config::FractalConfig::default();
                        config.flame = flame;
                        config.flame.name = format!("Random {}", i + 1);

                        // Assign palette - configs must be self-contained
                        if use_random_palette && palette_count > 0 {
                            // Pick a random palette from the library
                            let idx = rand::random::<usize>() % palette_count;
                            if let Some(palette) = self.context.palette_library.get(idx) {
                                config.palette = palette.clone();
                            }
                        } else {
                            // Use current palette from config manager
                            config.palette = self.context.config_manager.active_config().palette.clone();
                        }

                        config
                    })
                    .collect();

                *self.context.generated_batch = Some(configs);
            }
        }
    }

    /// Render Fractal Browser panel (unified presets/batch/files)
    fn render_fractal_browser_panel(&mut self, ui: &mut egui::Ui) {
        // Initialize panel if not already created
        if self.context.fractal_browser_panel.is_none() {
            *self.context.fractal_browser_panel = Some(super::fractal_browser::FractalBrowserPanel::new());
        }

        if let Some(panel) = self.context.fractal_browser_panel.as_mut() {
            let settings = self.context.config_manager.system_settings();
            let online_mode = settings.online_mode;
            let auth: Option<(&str, &str)> = settings.auth_token.as_ref().map(|token| {
                (settings.api_base_url.as_str(), token.as_str())
            });
            let response = panel.render(ui, online_mode, auth);

            // Handle file open request from Files tab
            if panel.take_open_file_request() {
                *self.context.file_browser_open_requested = true;
            }

            // Handle selection (load the config)
            if let Some(config) = response.selected {
                *self.context.selected_preset_config = Some(config);

                // Pass API flame ID through (for Online tab loads)
                *self.context.loaded_api_flame_id = response.api_flame_id;
            }

            // Pass API notifications through (e.g., delete result)
            if let Some(notification) = response.api_notification {
                *self.context.api_notification = Some(notification);
            }
        }
    }

    /// Render Effects panel (post-processing color and density effects)
    fn render_effects_panel(&mut self, ui: &mut egui::Ui) {
        super::effects_panel::render_effects_panel(
            ui,
            self.context.config_manager,
            self.context.animation_controller,
        );
    }

    /// Render Xaos Editor panel (chaos-weighted transform transitions)
    fn render_xaos_editor_panel(&mut self, ui: &mut egui::Ui) {
        let _ = super::xaos_editor::render_xaos_editor_content(
            ui,
            self.context.config_manager,
            self.context.flame,
            self.context.xaos_editor_state,
        );
    }

    /// Render Save Online Dialog panel (name input)
    fn render_save_online_dialog(&mut self, ui: &mut egui::Ui) {
        let _should_close = super::save_online_dialog::render_save_online_dialog(
            ui,
            self.context.save_online_dialog_state,
        );
        // Panel closes via the dock's X button; action is polled in render_ui
    }

    /// Render Login Dialog panel (email/password form)
    fn render_login_dialog(&mut self, ui: &mut egui::Ui) {
        if let Some(success) = super::login_dialog::render_login_dialog(
            ui,
            self.context.login_dialog_state,
            self.context.config_manager,
        ) {
            // Login succeeded — show notification
            *self.context.api_notification = Some((
                format!("Signed in as {}", success.email),
                false,
            ));
        }
    }

    /// Render Signal panel (signals, audio, generators)
    fn render_signal_panel(&mut self, ui: &mut egui::Ui) {
        let animation_duration = self.context.animation_controller.animation
            .as_ref()
            .map(|a| a.duration)
            .unwrap_or(10.0);
        super::signal_panel::render_signal_panel(
            ui,
            self.context.audio_manager,
            self.context.audio_player,
            self.context.audio_capture,
            self.context.signal_panel_state,
            self.context.signal_manager,
            self.context.current_time,
            animation_duration,
            self.context.load_audio_file,
            self.context.load_signal_file,
            self.context.save_signal_file,
        );
    }
}
