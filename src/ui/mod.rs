pub mod animation_panel;
mod config_dialog;
mod effects_panel;
mod export_panel;
mod font_loader;
mod formatting;
pub mod fractal_browser;
pub mod fractal_gallery;
mod help;
pub mod histogram;
mod menu_bar;
mod menu_context;
mod palette_editor;
mod palette_library;
mod panel_viewer;
mod path_editor;
mod performance;
mod random_generator;
mod response;
mod settings;
mod tone_mapping;
pub mod track_editor;
mod transforms;
mod triangle_editor;
mod undo_history;
mod variation_controls;
mod variation_params;
mod view;
pub mod workspace;

pub use animation_panel::{ExportProgress, TimelineLayout};
pub use export_panel::PngExportProgress;
pub use histogram::LevelsState;
pub use track_editor::TrackEditorState;
pub use font_loader::{ensure_font_for_locale, initialize_default_fonts};
pub use menu_context::{MenuActions, MenuState};
pub use palette_editor::PaletteEditor;
pub use response::UiResponse;
pub use workspace::Workspace;

/// Information about a clicked pixel in PathMap mode
/// Includes pixel coordinates, fractal space coordinates, path data, and a 5x5 color preview
#[derive(Clone, Debug)]
pub struct PathClickInfo {
    /// View space pixel coordinates (where user clicked)
    pub click_pixel: (u32, u32),
    /// Actual pixel with valid path data (may differ if click was empty)
    pub found_pixel: (u32, u32),
    /// Fractal space coordinates of the found pixel
    pub fractal_coords: (f32, f32),
    /// Distance from click to found pixel (0 if exact match)
    pub search_distance: f32,
    /// Path data at the found pixel
    pub path_entry: crate::renderer::PathEntry,
    /// 5x5 color preview centered on found pixel (RGBA, row-major)
    /// May be smaller if near edges
    pub color_preview: Vec<[u8; 4]>,
    /// Dimensions of the color preview (width, height) - usually 5x5
    pub preview_size: (u32, u32),
}

use egui_wgpu::wgpu::*;
use egui_wgpu::{Renderer as EguiRenderer, RendererOptions};
use egui_winit::State as EguiWinitState;
use winit::{event::WindowEvent, window::Window};

pub struct EguiLayer {
    state: EguiWinitState,
    pub ctx: egui_dock::egui::Context,
    renderer: EguiRenderer,
    config_json_buffer: String,
    palette_editor: PaletteEditor,

    // Fractal texture ID (registered from renderer's texture)
    fractal_texture_id: Option<egui_dock::egui::TextureId>,
    // Track registered texture dimensions to detect resize
    fractal_texture_width: u32,
    fractal_texture_height: u32,

    // Selected preset config (from FractalBrowser or other sources)
    selected_preset_config: Option<crate::config::FractalConfig>,

    // Animation export settings
    animation_export_settings: animation_panel::AnimationExportSettings,

    // Track editor state
    track_editor_state: track_editor::TrackEditorState,

    // PathMap mode: clicked pixel info (includes path, coordinates, color preview)
    clicked_pixel: Option<(u32, u32)>,
    path_click_info: Option<PathClickInfo>,
    close_path_overlay: bool,

    // Path editor state
    path_editor_state: path_editor::PathEditorState,

    // Random generator panel state
    random_generator_panel: Option<random_generator::RandomGeneratorPanel>,
    generated_flame: Option<crate::scene::transforms::Flame>,
    generated_batch: Option<Vec<crate::config::FractalConfig>>,

    // Fractal browser panel state
    fractal_browser_panel: Option<fractal_browser::FractalBrowserPanel>,

    // Histogram for density visualization (levels now in ConfigManager)
    density_histogram: crate::renderer::DensityHistogram,
}

impl EguiLayer {
    pub fn new(window: &Window, device: &Device, format: TextureFormat) -> Self {
        let ctx = egui_dock::egui::Context::default();

        // Initialize fonts with Noto Sans (better Unicode coverage than Ubuntu-Light)
        font_loader::initialize_default_fonts(&ctx);

        // Configure style to disable window shadows
        ctx.set_visuals(egui_dock::egui::Visuals {
            window_shadow: egui_dock::egui::epaint::Shadow::NONE,
            ..egui_dock::egui::Visuals::dark()
        });

        let viewport_id = ctx.viewport_id();
        let state = EguiWinitState::new(ctx.clone(), viewport_id, window, None, None, None);
        let renderer = EguiRenderer::new(
            device,
            format,
            RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );
        Self {
            state,
            ctx,
            renderer,
            config_json_buffer: String::new(),
            palette_editor: PaletteEditor::new(),
            fractal_texture_id: None,
            fractal_texture_width: 0,
            fractal_texture_height: 0,
            selected_preset_config: None,
            animation_export_settings: animation_panel::AnimationExportSettings::default(),
            track_editor_state: track_editor::TrackEditorState::default(),
            clicked_pixel: None,
            path_click_info: None,
            close_path_overlay: false,
            path_editor_state: path_editor::PathEditorState::new(),
            random_generator_panel: None,
            generated_flame: None,
            generated_batch: None,
            fractal_browser_panel: None,
            density_histogram: crate::renderer::DensityHistogram::default(),
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent, window: &Window) -> bool {
        let response = self.state.on_window_event(window, event);

        // For mouse events, we need to be more aggressive about detecting UI interaction
        // The issue: is_using_pointer() can return false even when over panels
        // Better approach: Check if pointer is over ANY layer (not just interacting with widgets)
        match event {
            WindowEvent::MouseInput { .. } | WindowEvent::CursorMoved { .. } | WindowEvent::MouseWheel { .. } => {
                // Check multiple egui states to detect UI interaction
                let is_using = self.ctx.is_using_pointer();
                let wants_pointer = self.ctx.wants_pointer_input();
                let is_pointer_over_area = self.ctx.is_pointer_over_area();

                // Consume if egui wants the pointer OR pointer is over any UI area
                let consumed = response.consumed && (is_using || wants_pointer || is_pointer_over_area);

                // DEBUG: Log pointer state for cursor moves (only when dragging might be active)
                // if matches!(event, WindowEvent::CursorMoved { .. }) && consumed {
                //     log::debug!("CursorMoved over UI: consumed={}, is_using={}, wants_pointer={}, is_over_area={}",
                //         response.consumed, is_using, wants_pointer, is_pointer_over_area);
                // }

                consumed
            }
            _ => response.consumed
        }
    }

    pub fn update_palette_editor(&mut self, palette: crate::scene::palette::Palette) {
        self.palette_editor.current_palette = palette;
    }

    /// Get the clicked pixel coordinates (for PathMap mode)
    /// Returns Some((x, y)) if user clicked on the fractal viewport
    pub fn take_clicked_pixel(&mut self) -> Option<(u32, u32)> {
        self.clicked_pixel.take()
    }

    /// Update the cached path click info for display
    pub fn set_path_click_info(&mut self, info: Option<PathClickInfo>) {
        self.path_click_info = info;
    }

    /// Get reference to current path click info
    pub fn path_click_info(&self) -> Option<&PathClickInfo> {
        self.path_click_info.as_ref()
    }

    /// Check if the path overlay should be closed and reset the flag
    pub fn take_close_path_overlay(&mut self) -> bool {
        let close = self.close_path_overlay;
        self.close_path_overlay = false;
        close
    }

    /// Register the renderer's fractal texture with egui for display
    /// Call this when the texture size changes or on first frame
    pub fn register_fractal_texture(&mut self, device: &Device, texture_view: &TextureView, width: u32, height: u32) {
        // Check if we need to re-register (size changed or first time)
        let needs_reregister = self.fractal_texture_id.is_none()
            || self.fractal_texture_width != width
            || self.fractal_texture_height != height;

        if needs_reregister {
            log::info!("Re-registering fractal texture: {}x{} → {}x{}",
                self.fractal_texture_width, self.fractal_texture_height, width, height);

            // Unregister old texture if it exists
            if let Some(old_id) = self.fractal_texture_id.take() {
                self.renderer.free_texture(&old_id);
            }

            self.fractal_texture_width = width;
            self.fractal_texture_height = height;
        }

        // ALWAYS update the texture view, even if size didn't change
        // This is critical for minimize/restore - the texture view can become stale
        // even if the size is the same
        if let Some(old_id) = self.fractal_texture_id.take() {
            if !needs_reregister {
                self.renderer.free_texture(&old_id);
            }
        }

        let texture_id = self.renderer.register_native_texture(
            device,
            texture_view,
            FilterMode::Linear,
        );
        self.fractal_texture_id = Some(texture_id);
    }

    /// Get the egui TextureId for the fractal texture (for displaying in UI)
    pub fn fractal_texture_id(&self) -> Option<egui_dock::egui::TextureId> {
        self.fractal_texture_id
    }

    pub fn render_ui(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
        encoder: &mut egui_wgpu::wgpu::CommandEncoder,
        target_view: &egui_wgpu::wgpu::TextureView,
        window: &Window,
        window_size: winit::dpi::PhysicalSize<u32>,
        metrics: &crate::util::PerformanceMetrics,
        config_manager: &mut crate::config::ConfigManager,
        flame_renderer: Option<&mut crate::renderer::compute_kernel::FlameRenderer>,
        flame: &mut crate::scene::transforms::Flame,
        palette_library: &mut crate::scene::palette::PaletteLibrary,
        preset_library: &crate::scene::presets::PresetLibrary,
        animation_controller: &mut crate::animation::AnimationController,
        paused: &mut bool,
        quit_requested: &mut bool,
        can_undo: bool,
        can_redo: bool,
        workspace: &mut workspace::Workspace,
        export_width: &mut u32,
        export_height: &mut u32,
        use_custom_export_size: &mut bool,
        animation_export_progress: &animation_panel::ExportProgress,
        png_export_progress: &export_panel::PngExportProgress,
        fullscreen_mode: bool,
    ) -> UiResponse {
        let raw_input = self.state.take_egui_input(window);

        // Note: Config change tracking now handled by ConfigManager.get_pending_actions()
        // Only non-config actions tracked here (file I/O, palette library, transforms, etc.)

        let mut add_transform = false;
        let mut delete_transform = None;
        let mut clone_transform = None;

        // Config import/export
        let mut config_export_json = None;
        let mut config_import_json = None;
        let mut config_save_file = false;
        let mut config_load_file = false;
        let mut apophysis_import_file = false;
        let mut new_flame_requested = false;
        let mut random_flame_requested = false;

        // Palette library management
        let mut palette_export_json = None;
        let mut palette_save_file = None;
        let mut palette_save_to_library = None;
        let mut palette_delete_from_library = None;
        let mut palette_import_json = None;
        let mut palette_load_file = false;

        // Undo/redo
        let mut undo_requested = false;
        let mut redo_requested = false;

        // Export
        let mut png_export_with_background = false;
        let mut png_export_transparent = false;
        // let mut png_export_requested = false;

        // Panel open requests
        let mut open_palette_editor = false;
        let mut open_palette_library = false;
        let mut open_config_dialog = false;
        let mut open_triangle_editor = false;
        let mut open_preset_library = false;

        // Fractal viewport size tracking
        let mut fractal_viewport_size = None;

        // File browser
        let mut file_browser_open_requested = false;

        // Animation export
        let mut animation_export_requested: Option<animation_panel::AnimationExportSettings> = None;
        let mut animation_seek_changed = false;

        // Path filters
        let mut path_filters_changed: Option<Vec<crate::gpu::buffers::GpuPathFilter>> = None;

        // Menu actions and state
        let mut menu_actions = MenuActions::default();
        let menu_state = MenuState {
            can_undo,
            can_redo,
            is_paused: *paused,
            render_mode_2d: config_manager.active_config().flame.render_mode == crate::scene::transforms::RenderMode::TwoD,
        };

        // Log ConfigManager state at start of UI render
        // log::debug!("render_ui start: ConfigManager has exposure={:.3}, gamma={:.3}",
        //     config_manager.config().exposure, config_manager.config().gamma);

        // Get fractal texture ID before the closure (avoid borrow conflict)
        let fractal_texture_id = self.fractal_texture_id();

        let full_output = self.ctx.run(raw_input, |ctx| {
            // Debug: Print font info once after fonts are available
            static FONT_DEBUG_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !FONT_DEBUG_DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
                font_loader::debug_font_info(ctx);
            }

            // Fullscreen mode: render only the fractal, no UI panels
            if fullscreen_mode {
                // Render fractal texture fullscreen
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        if let Some(texture_id) = fractal_texture_id {
                            let available = ui.available_size();
                            ui.add(egui::Image::new(egui::load::SizedTexture::new(
                                texture_id,
                                available,
                            )));
                            // Track viewport size for renderer resize
                            fractal_viewport_size = Some((available.x as u32, available.y as u32));
                        }
                    });

                // Show exit hint at bottom
                egui::Area::new(egui::Id::new("fullscreen_hint"))
                    .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -20.0))
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.add(egui::Label::new(
                            egui::RichText::new("Press F or Esc to exit fullscreen")
                                .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180))
                                .size(14.0)
                        ).selectable(false));
                    });

                return;
            }

            // Normal mode: render menu bar and dock panels
            menu_bar::render_menu_bar(
                ctx,
                workspace,
                &mut menu_actions,
                &menu_state,
            );

            // All windows are now dockable panels (see Windows menu)
            // Fullscreen docking system with Fractal Viewport as a panel
            // Fractal renders as a panel in the dock, can be arranged with other panels

            // Render fullscreen DockArea - manages all panels including FractalViewport
            // egui automatically handles input routing for panels
            egui_dock::DockArea::new(&mut workspace.dock_state)
                .id(egui::Id::new("main_dock_area"))
                .show(ctx, &mut panel_viewer::PanelViewer {
                    context: panel_viewer::PanelContext {
                        // Core state
                        config_manager,
                        flame,

                        // Libraries
                        preset_library,
                        palette_library,

                        // Renderer
                        flame_renderer: flame_renderer.as_ref().map(|v| &**v),

                        // Animation controller
                        animation_controller,

                        // Action flags
                        add_transform: &mut add_transform,
                        delete_transform: &mut delete_transform,
                        clone_transform: &mut clone_transform,
                        undo_requested: &mut undo_requested,
                        redo_requested: &mut redo_requested,
                        open_palette_editor: &mut open_palette_editor,
                        open_palette_library: &mut open_palette_library,
                        open_triangle_editor: &mut open_triangle_editor,
                        open_preset_library: &mut open_preset_library,

                        // UI state
                        paused,
                        png_export_with_background: &mut png_export_with_background,
                        png_export_transparent: &mut png_export_transparent,
                        export_width,
                        export_height,
                        use_custom_export_size,
                        palette_editor: &mut self.palette_editor,
                        palette_export_json: &mut palette_export_json,
                        palette_save_file: &mut palette_save_file,
                        palette_save_to_library: &mut palette_save_to_library,
                        palette_delete_from_library: &mut palette_delete_from_library,
                        palette_import_json: &mut palette_import_json,
                        palette_load_file: &mut palette_load_file,

                        // Performance metrics
                        metrics,
                        window_size,

                        // Fractal texture for display
                        fractal_texture_id,
                        fractal_viewport_size: &mut fractal_viewport_size,

                        // Config dialog state
                        config_json_buffer: &mut self.config_json_buffer,
                        config_export_json: &mut config_export_json,
                        config_import_json: &mut config_import_json,
                        config_save_file: &mut config_save_file,
                        config_load_file: &mut config_load_file,
                        apophysis_import_file: &mut apophysis_import_file,
                        open_config_dialog: &mut open_config_dialog,

                        // Selected preset config (from FractalBrowser or other sources)
                        selected_preset_config: &mut self.selected_preset_config,

                        // File browser open request (shared by FractalBrowser)
                        file_browser_open_requested: &mut file_browser_open_requested,

                        // Animation export settings
                        animation_export_settings: &mut self.animation_export_settings,
                        animation_export_requested: &mut animation_export_requested,
                        animation_export_progress,

                        // Track editor state
                        track_editor_state: &mut self.track_editor_state,

                        // Animation seek changed flag
                        animation_seek_changed: &mut animation_seek_changed,

                        // PathMap mode: clicked pixel and path info
                        hovered_pixel: &mut self.clicked_pixel,
                        path_click_info: &self.path_click_info,
                        close_path_overlay: &mut self.close_path_overlay,

                        // Path editor state
                        path_editor_state: &mut self.path_editor_state,
                        path_filters_changed: &mut path_filters_changed,

                        // PNG export progress
                        png_export_progress,

                        // Random generator panel state
                        random_generator_panel: &mut self.random_generator_panel,
                        generated_flame: &mut self.generated_flame,
                        generated_batch: &mut self.generated_batch,

                        // Fractal browser panel state
                        fractal_browser_panel: &mut self.fractal_browser_panel,

                        // Histogram for density visualization (levels now in ConfigManager)
                        density_histogram: &self.density_histogram,
                    },
                });

            // Show palette editor dialogs (fixed mode warning, overwrite/delete confirmations)
            palette_editor::render_palette_dialogs(
                ctx,
                &mut self.palette_editor,
                config_manager,
                &mut palette_save_to_library,
                &mut palette_delete_from_library,
            );

            // Note: quit_requested is now handled in app.rs event loop for graceful shutdown
        });

        // Log egui repaint requests for performance investigation
        let needs_repaint = self.ctx.has_requested_repaint();

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [window_size.width, window_size.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        self.renderer
            .update_buffers(device, queue, encoder, &tris, &screen_descriptor);

        {
            let mut rpass = encoder.begin_render_pass(&egui_wgpu::wgpu::RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(egui_wgpu::wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: egui_wgpu::wgpu::Operations {
                        load: egui_wgpu::wgpu::LoadOp::Load, // Load existing content (flame rendering)
                        store: egui_wgpu::wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // forget_lifetime() converts RenderPass<'a> to RenderPass<'static>
            // This is the official wgpu API for satisfying egui-wgpu's 'static requirement
            let mut rpass = rpass.forget_lifetime();

            self.renderer
                .render(&mut rpass, &tris, &screen_descriptor);
        } // Render pass is dropped here

        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }

        // Handle View menu actions BEFORE syncing flame (so changes take effect this frame)
        use crate::config::ConfigPath;
        if menu_actions.view.reset_view {
            let _ = config_manager.update_param(ConfigPath::Zoom, 1.0.into());
            let _ = config_manager.update_param(ConfigPath::Pan, (0.0, 0.0).into());
            let _ = config_manager.update_param(ConfigPath::Rotation, 0.0.into());
        }

        if menu_actions.view.zoom_in {
            let current_zoom = config_manager.active_config().zoom;
            let _ = config_manager.update_param(ConfigPath::Zoom, (current_zoom * 1.2).into());
        }

        if menu_actions.view.zoom_out {
            let current_zoom = config_manager.active_config().zoom;
            let _ = config_manager.update_param(ConfigPath::Zoom, (current_zoom / 1.2).into());
        }

        if menu_actions.view.set_mode_2d {
            let _ = config_manager.update_param(
                ConfigPath::RenderMode,
                crate::scene::transforms::RenderMode::TwoD.into()
            );
        }

        if menu_actions.view.set_mode_3d {
            let _ = config_manager.update_param(
                ConfigPath::RenderMode,
                crate::scene::transforms::RenderMode::ThreeD.into()
            );
        }

        // Sync flame from ConfigManager AFTER UI updates
        *flame = config_manager.active_config().flame.clone();

        // Extract menu actions into individual flags for backward compatibility
        config_load_file |= menu_actions.file.load_config;
        config_save_file |= menu_actions.file.save_config;
        apophysis_import_file |= menu_actions.file.import_apophysis;
        new_flame_requested |= menu_actions.file.new_flame;
        random_flame_requested |= menu_actions.file.random_flame;
        if menu_actions.file.export_png {
            png_export_with_background = true;
        }
        if menu_actions.file.export_png_transparent {
            png_export_transparent = true;
        }
        *quit_requested |= menu_actions.file.quit;

        // Combine undo/redo from both menu and panels (OR to not override panel buttons)
        undo_requested |= menu_actions.edit.undo;
        redo_requested |= menu_actions.edit.redo;

        // Handle Rendering menu actions
        if menu_actions.rendering.pause_toggle {
            *paused = !*paused;
        }

        if menu_actions.rendering.reset_accumulation {
            let _ = config_manager.request_reset();
        }

        if let Some(ipt) = menu_actions.rendering.set_iterations_per_thread {
            let _ = config_manager.update_system_setting(
                crate::config::ConfigPath::SystemIterationsPerThread,
                ipt.into()
            );
        }

        // Take selected preset config (reset to None after returning)
        let selected_preset_config = self.selected_preset_config.take();

        // Take generated flame from random generator panel
        let generated_flame = self.generated_flame.take();
        let generated_batch = self.generated_batch.take();

        UiResponse {
            config_export_requested: config_export_json,
            config_import_requested: config_import_json,
            config_save_file_requested: config_save_file,
            config_load_file_requested: config_load_file,
            apophysis_import_file_requested: apophysis_import_file,
            new_flame_requested,
            random_flame_requested,
            palette_export_json,
            palette_save_file,
            palette_save_to_library,
            palette_delete_from_library,
            palette_import_json,
            palette_load_file,
            undo_requested,
            redo_requested,
            png_export_with_background,
            png_export_transparent,
            add_transform,
            delete_transform,
            clone_transform,
            open_palette_editor,
            open_palette_library,
            open_config_dialog,
            open_triangle_editor,
            open_preset_library,
            fractal_viewport_size,
            needs_repaint,
            selected_preset_config,
            file_browser_open_requested,
            animation_export_requested,
            animation_seek_changed,
            path_filters_changed,
            generated_flame,
            generated_batch,
        }
    }

    /// Check if fractal browser panel needs thumbnail generation
    #[cfg(not(target_arch = "wasm32"))]
    pub fn fractal_browser_needs_thumbnails(&self) -> bool {
        if let Some(ref panel) = self.fractal_browser_panel {
            panel.is_generating()
        } else {
            false
        }
    }

    /// Generate one thumbnail for the fractal browser (call once per frame)
    /// Returns true if generation is complete
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate_fractal_browser_thumbnail(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
        palette_library: &crate::scene::palette::PaletteLibrary,
    ) -> bool {
        if let Some(ref mut panel) = self.fractal_browser_panel {
            // Get which tab/config needs generation
            if let Some((tab, _config)) = panel.next_pending_config() {
                panel.generate_one_thumbnail(tab, &self.ctx, |config| {
                    crate::renderer::render_thumbnail(device, queue, config, palette_library)
                })
            } else {
                true // Nothing to generate
            }
        } else {
            true // No panel, nothing to generate
        }
    }

    /// WASM: Start async thumbnail generation for fractal browser
    #[cfg(target_arch = "wasm32")]
    pub fn start_fractal_browser_thumbnails(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
    ) {
        if let Some(ref mut panel) = self.fractal_browser_panel {
            panel.start_async_thumbnails(device, queue);
        }
    }

    /// Load batch results into the unified Fractal Browser panel
    pub fn load_batch_into_fractal_browser(&mut self, configs: Vec<crate::config::FractalConfig>) {
        // Initialize panel if not already created
        if self.fractal_browser_panel.is_none() {
            self.fractal_browser_panel = Some(fractal_browser::FractalBrowserPanel::new());
        }

        if let Some(ref mut panel) = self.fractal_browser_panel {
            panel.load_batch(configs);
        }
    }

    /// Load file contents into the unified Fractal Browser panel
    pub fn load_file_into_fractal_browser(&mut self, path: std::path::PathBuf) {
        // Initialize panel if not already created
        if self.fractal_browser_panel.is_none() {
            self.fractal_browser_panel = Some(fractal_browser::FractalBrowserPanel::new());
        }

        if let Some(ref mut panel) = self.fractal_browser_panel {
            panel.load_file(path);
        }
    }

    /// Load JSON configs into the unified Fractal Browser panel
    pub fn load_json_into_fractal_browser(&mut self, json: &str, source_name: &str) {
        // Initialize panel if not already created
        if self.fractal_browser_panel.is_none() {
            self.fractal_browser_panel = Some(fractal_browser::FractalBrowserPanel::new());
        }

        if let Some(ref mut panel) = self.fractal_browser_panel {
            panel.load_json(json, source_name);
        }
    }

    /// Update the density histogram from computed data
    pub fn update_histogram(&mut self, histogram: crate::renderer::DensityHistogram) {
        self.density_histogram = histogram;
        // Note: Auto-levels is a one-shot button in render_levels_controls_managed
    }

    /// Get a reference to the density histogram
    pub fn density_histogram(&self) -> &crate::renderer::DensityHistogram {
        &self.density_histogram
    }
}
