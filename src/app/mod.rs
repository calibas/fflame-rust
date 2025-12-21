//! Application state and event loop

mod input;
mod config;
pub mod export;
pub mod render_mode;

pub use render_mode::{RenderModeFSM, RenderModeState, TransitionResult};

#[cfg(not(target_arch = "wasm32"))]
pub use export::export_headless;

#[cfg(target_arch = "wasm32")]
pub use export::export_headless_wasm;

use winit::{event::*, event_loop::{EventLoop, ControlFlow, ActiveEventLoop}, window::Window};
use egui_wgpu::wgpu::SurfaceError;
use std::sync::{Arc, Mutex};

use crate::gpu::device::GpuContext;
use crate::ui::EguiLayer;
use crate::ui::animation_panel::ExportProgress;
use crate::renderer::FlameRenderer;
use crate::scene::transforms::Flame;
use crate::scene::palette::PaletteLibrary;
use crate::scene::presets::PresetLibrary;
use crate::util::PerformanceMetrics;
use crate::config::{FractalConfig, ConfigManager};
use crate::animation::{AnimationController, PlaybackState};

pub struct App {
    // Core state management
    pub(super) config_manager: ConfigManager,  // Single source of truth for ALL config (fractal + system)

    // GPU and rendering resources
    pub(super) gpu: GpuContext,
    pub(super) egui_layer: EguiLayer,
    pub(super) flame_renderer: Option<FlameRenderer>,
    pub(super) flame: Flame,  // Working copy for renderer (synced from config_manager)

    // UI state (not saved in config)
    pub(super) workspace: crate::ui::Workspace,
    pub(super) view_changed_by_keyboard: bool,
    pub(super) paused: bool,
    pub(super) modifiers: winit::keyboard::ModifiersState,
    pub(super) quit_requested: bool,  // Graceful quit requested (check unsaved changes, etc.)

    // Libraries (not saved in config)
    pub(super) palette_library: PaletteLibrary,
    pub(super) preset_library: PresetLibrary,
    pub(super) current_preset_index: usize,  // UI state, not config

    // Animation system
    pub(super) animation_controller: AnimationController,

    // Performance tracking
    pub(super) metrics: PerformanceMetrics,

    // Rendering internals (frame timing and batching)
    pub(super) last_frame_time: Option<web_time::Instant>,
    pub(super) accumulation_batch_size: u32,  // Process every N frames (1 = normal, 4 = batched)
    pub(super) frames_since_accumulation: u32,
    pub(super) use_overwrite_next_frame: bool,  // Persist overwrite mode for brief period after changes
    pub(super) last_param_change_time: Option<web_time::Instant>,  // Track when params last changed
    pub(super) rendering_complete: bool,  // True when rendering has finished (max_iterations reached)
    pub(super) clear_paths_next_frame: bool,  // Clear path buffer on next compute pass (full reset)
    pub(super) ui_needs_repaint: bool,  // Track if UI is requesting repaints (for frame rate boost)
    pub(super) pending_redraws: u32,  // Counter for queued redraws (for UI animations after input)

    // Fractal viewport size (updated from UI each frame)
    pub(super) fractal_viewport_size: (u32, u32),

    // PNG export settings (UI state only, not in config)
    pub(super) export_width: u32,
    pub(super) export_height: u32,
    pub(super) use_custom_export_size: bool,

    // Animation export progress (shared with background export thread)
    pub(super) animation_export_progress: Arc<Mutex<ExportProgress>>,

    // Rendering mode state machine (Normal, Animating, Overwrite)
    pub(super) render_mode: RenderModeFSM,
}
impl App {
    pub async fn run(event_loop: EventLoop<()>, window: Window) -> Result<(), Box<dyn std::error::Error>> {
        let gpu = GpuContext::new(&window).await.expect("GPU init failed");
        let egui_layer = EguiLayer::new(&window, &gpu.device, gpu.config.format);

        // Load preset library and use first preset (with ALL its settings)
        let preset_library = PresetLibrary::new();
        let palette_library = PaletteLibrary::new();

        // Use entire FractalConfig from first preset (not just the flame!)
        let initial_config = preset_library.get(0)
            .cloned()
            .unwrap_or_default();

        // Note: Device-specific settings (iterations_per_thread, vsync_enabled, target_fps)
        // are loaded by ConfigManager from SystemSettings

        let flame = initial_config.flame.clone();

        let flame_renderer = FlameRenderer::new(
            &gpu.device,
            &gpu.queue,
            gpu.config.format,
            gpu.size.width,
            gpu.size.height,
            &flame,
        );

        // ConfigManager loads SystemSettings automatically
        let config_manager = ConfigManager::new(initial_config.clone());

        // Get initial size before moving gpu
        let initial_viewport_size = (gpu.size.width, gpu.size.height);

        // Get export dimensions before moving config_manager
        let export_width = config_manager.system_settings().default_export_width;
        let export_height = config_manager.system_settings().default_export_height;

        let mut app = Self {
            config_manager,
            gpu,
            egui_layer,
            flame_renderer: Some(flame_renderer),
            flame,
            workspace: crate::ui::Workspace::new(),
            view_changed_by_keyboard: false,
            paused: false,
            modifiers: winit::keyboard::ModifiersState::default(),
            quit_requested: false,
            palette_library,
            preset_library,
            current_preset_index: 0,
            animation_controller: AnimationController::new(),
            metrics: PerformanceMetrics::new(),
            last_frame_time: None,
            accumulation_batch_size: 4, // EXPERIMENT: Test batching
            frames_since_accumulation: 0,
            use_overwrite_next_frame: false,
            last_param_change_time: None,
            rendering_complete: false,
            clear_paths_next_frame: true,  // Clear paths on first frame
            ui_needs_repaint: false,
            pending_redraws: 0,
            fractal_viewport_size: initial_viewport_size, // Initialize to window size
            export_width,
            export_height,
            use_custom_export_size: false,  // Default to viewport size
            animation_export_progress: Arc::new(Mutex::new(ExportProgress::default())),
            render_mode: RenderModeFSM::new(),
        };

        #[allow(deprecated)]
        event_loop.run(move |event, elwt| {
            match event {
                Event::WindowEvent { event, window_id } if window_id == window.id() => {
                    // Let egui handle events first
                    let consumed = app.egui_layer.handle_event(&event, &window);

                    // Request redraw for events that need visual updates
                    // This wakes from ControlFlow::Wait when user interacts
                    match &event {
                        WindowEvent::CursorMoved { .. } |
                        WindowEvent::MouseInput { .. } |
                        WindowEvent::MouseWheel { .. } |
                        WindowEvent::KeyboardInput { .. } => {
                            // Queue 15 frames for UI animations (hover effects, transitions, etc.)
                            app.pending_redraws = 15;
                            window.request_redraw();
                        }
                        WindowEvent::Resized(_) |
                        WindowEvent::ScaleFactorChanged { .. } => {
                            // Window events need just 1 frame
                            app.pending_redraws = 1;
                            window.request_redraw();
                        }
                        _ => {}
                    }

                    match event {
                        WindowEvent::CloseRequested => {
                            app.shutdown(elwt);
                        },
                        WindowEvent::Resized(size) => {
                            // Skip resize if dimensions are zero (happens when minimizing on Windows)
                            if size.width > 0 && size.height > 0 {
                                log::debug!("Window resized to {}x{}", size.width, size.height);
                                app.gpu.resize(size);
                                // NOTE: Don't resize renderer here - it will be resized by fractal viewport resize
                                // The fractal panel is smaller than the window (due to UI panels)
                                // Resizing renderer to window size causes aspect ratio mismatch
                            }
                        },
                        WindowEvent::ScaleFactorChanged { .. } => {
                            // Handle DPI/zoom changes - resize surface to match new scale
                            // Note: Renderer resize will happen via fractal viewport resize in render()
                            let new_size = window.inner_size();
                            if new_size.width > 0 && new_size.height > 0 {
                                app.gpu.resize(new_size);
                                window.request_redraw();
                            }
                        },
                        WindowEvent::KeyboardInput { event: key_event, .. } if !consumed => {
                            // Handle keyboard input only if egui didn't consume it
                            app.handle_keyboard(&key_event);
                        }
                        WindowEvent::ModifiersChanged(new_modifiers) => {
                            app.modifiers = new_modifiers.state();
                        }
                        WindowEvent::RedrawRequested => {
                            app.update();
                            match app.render(&window) {
                                Ok(_) => {},
                                Err(SurfaceError::Lost | SurfaceError::Outdated) => app.gpu.resize(app.gpu.size),
                                Err(SurfaceError::OutOfMemory) => elwt.exit(),
                                Err(e) => eprintln!("Render error: {:?}", e),
                            }

                            // Handle graceful quit (triggered by File → Quit menu)
                            if app.quit_requested {
                                app.shutdown(elwt);
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
                    use std::time::Duration;
                    use web_time::Instant;

                    // Check if actively rendering (not paused and under max_iterations)
                    let config = app.config_manager.active_config();
                    let max_iterations = Some(config.max_iterations);
                    let is_rendering = !app.paused && app.flame_renderer.as_ref().map_or(false, |r| {
                        max_iterations.map_or(true, |max| r.total_iterations() < max)
                    });

                    // Check if animation is playing (needs continuous redraws)
                    let animation_playing = app.animation_controller.is_playing();

                    // Update present mode based on system settings
                    app.gpu.set_present_mode(app.config_manager.system_settings().vsync_enabled);

                    // EVENT-DRIVEN RENDERING:
                    // Only render when something actually changes
                    if is_rendering || animation_playing || app.pending_redraws > 0 {
                        // Actively rendering fractals OR UI animations pending
                        if app.config_manager.system_settings().vsync_enabled {
                            // VSync enabled: render continuously, let VSync cap frame rate
                            window.request_redraw();
                        } else {
                            // VSync disabled: manually limit to target FPS
                            let target_frame_time = Duration::from_secs_f32(1.0 / app.config_manager.system_settings().target_fps);
                            let now = Instant::now();
                            if let Some(last_frame) = app.last_frame_time {
                                let elapsed = now.duration_since(last_frame);
                                if elapsed >= target_frame_time {
                                    window.request_redraw();
                                } else {
                                    let wait_until = last_frame + target_frame_time;
                                    elwt.set_control_flow(ControlFlow::WaitUntil(wait_until));
                                }
                            } else {
                                window.request_redraw();
                            }
                        }

                        // Decrement pending redraws counter after requesting next frame
                        // AboutToWait fires AFTER RedrawRequested, so we're counting frames that just drew
                        // While actively rendering, keep counter at minimum 1 for UI responsiveness
                        if app.pending_redraws > 0 {
                            if is_rendering {
                                // Keep at least 1 while rendering for smooth UI
                                app.pending_redraws = app.pending_redraws.saturating_sub(1).max(1);
                            } else {
                                // Not rendering: count down to 0 normally
                                app.pending_redraws -= 1;
                            }
                        }
                    } else {
                        // Truly idle: sleep until event wakes us
                        elwt.set_control_flow(ControlFlow::Wait);
                    }
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
    fn render(&mut self, window: &Window) -> Result<(), SurfaceError> {
        use web_time::Instant;

        // Skip rendering if window is minimized (size is 0)
        // This prevents surface errors and wasted GPU work
        if self.gpu.size.width == 0 || self.gpu.size.height == 0 {
            return Ok(());
        }

        let render_start = Instant::now();

        // Calculate delta time BEFORE updating last_frame_time (for animation)
        let delta_time = self.last_frame_time
            .map(|last| render_start.duration_since(last).as_secs_f64())
            .unwrap_or(1.0 / 60.0);

        self.last_frame_time = Some(render_start);

        // ============================================================================
        // NEW FRAME ORDER (Fixed race conditions):
        // 1. Render UI (reads current state, shows previous frame's fractal)
        // 2. Process all UI responses and config updates
        // 3. Get FINAL config after all updates
        // 4. Compute/accumulate/tonemap (generates new fractal with updated config)
        // 5. Submit and present
        // ============================================================================

        let frame = self.gpu.surface.get_current_texture()?;
        let surface_view = frame.texture.create_view(&egui_wgpu::wgpu::TextureViewDescriptor::default());
        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // ============================================================================
        // PHASE 1: Render UI First
        // ============================================================================
        // UI displays PREVIOUS frame's fractal while we prepare CURRENT frame
        let t_ui_start = Instant::now();
        let can_undo = self.can_undo();
        let can_redo = self.can_redo();

        // Register renderer's fractal texture with egui for display
        if let Some(ref renderer) = self.flame_renderer {
            self.egui_layer.register_fractal_texture(
                &self.gpu.device,
                renderer.get_fractal_texture_view(),
                self.fractal_viewport_size.0,
                self.fractal_viewport_size.1,
            );
        }

        // Get a snapshot of export progress for UI display
        let export_progress = self.animation_export_progress.lock().unwrap().clone();

        let ui_response = self.egui_layer.render_ui(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &surface_view,
            window,
            self.gpu.size,
            &self.metrics,
            &mut self.config_manager,
            self.flame_renderer.as_mut(),
            &mut self.flame,
            &mut self.palette_library,
            &self.preset_library,
            &mut self.animation_controller,
            &mut self.current_preset_index,
            &mut self.paused,
            &mut self.quit_requested,
            can_undo,
            can_redo,
            &mut self.workspace,
            &mut self.export_width,
            &mut self.export_height,
            &mut self.use_custom_export_size,
            &export_progress,
        );

        self.metrics.record_ui_time(t_ui_start.elapsed().as_secs_f64() * 1000.0);

        // Track UI repaint requests for frame rate optimization
        self.ui_needs_repaint = ui_response.needs_repaint;

        // Handle viewport resize immediately (before rendering)
        if let Some(viewport_size) = ui_response.fractal_viewport_size {
            if viewport_size != self.fractal_viewport_size {
                log::info!("Fractal viewport resize: {:?} → {:?}", self.fractal_viewport_size, viewport_size);
                self.fractal_viewport_size = viewport_size;

                // Resize renderer to match new viewport dimensions
                if let Some(ref mut renderer) = self.flame_renderer {
                    // Get config for resize
                    let resize_config = self.config_manager.active_config();
                    let mut resize_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                        label: Some("Viewport Resize Encoder"),
                    });
                    renderer.resize(&self.gpu.device, &mut resize_encoder, &self.gpu.queue, viewport_size.0, viewport_size.1,
                        &self.flame, self.config_manager.system_settings().iterations_per_thread, resize_config.zoom, resize_config.pan_x, resize_config.pan_y, resize_config.rotation,
                        resize_config.camera_rotation_x, resize_config.camera_rotation_y, resize_config.camera_z, resize_config.speed_factor);
                    self.gpu.queue.submit(std::iter::once(resize_encoder.finish()));

                    // Restore palette and color mode after buffer recreation
                    let palette = resize_config.palette.as_ref()
                        .or_else(|| self.palette_library.get(resize_config.palette_index));
                    if let Some(palette) = palette {
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, palette, resize_config.palette_rotation);
                    }
                    renderer.set_color_mode(&self.gpu.queue, resize_config.color_mode, self.config_manager.system_settings().iterations_per_thread, self.config_manager.system_settings().burn_in,
                        resize_config.zoom, resize_config.pan_x, resize_config.pan_y, resize_config.rotation,
                        resize_config.camera_rotation_x, resize_config.camera_rotation_y, resize_config.camera_z, resize_config.speed_factor);
                    renderer.set_path_map_style(resize_config.path_map_style);

                    // Restore tonemap parameters after buffer recreation (not in live preview mode)
                    renderer.update_tonemap(&self.gpu.queue, resize_config.tonemap_mode, resize_config.use_curve, resize_config.exposure, resize_config.gamma,
                        resize_config.gamma_threshold, resize_config.brightness, resize_config.vibrancy, resize_config.saturation, resize_config.hue_shift, resize_config.value_scale,
                        resize_config.alpha_blend_low, resize_config.alpha_blend_high,
                        viewport_size.0, viewport_size.1, renderer.total_iterations(), resize_config.max_iterations, resize_config.zoom, self.config_manager.system_settings().iterations_per_thread, 1, false);
                    renderer.update_curve_lut(&self.gpu.queue, &resize_config.tonemap_curve);

                    // Re-register texture with egui after resize (new texture view created)
                    self.egui_layer.register_fractal_texture(
                        &self.gpu.device,
                        renderer.get_fractal_texture_view(),
                        viewport_size.0,
                        viewport_size.1,
                    );
                }
            }
        }

        // Submit UI rendering (must happen before we start processing responses)
        let t_submit = Instant::now();

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        self.metrics.record_submit_time(t_submit.elapsed().as_secs_f64() * 1000.0);

        // ============================================================================
        // PHASE 2: Process ALL UI Responses and Config Updates
        // ============================================================================

        // Handle config export
        if ui_response.config_export_requested.is_some() {
            let config = self.export_config();
            if let Ok(json) = config.to_json() {
                // Copy to clipboard using egui's built-in clipboard
                self.egui_layer.ctx.copy_text(json);
            }
        }

        // Handle config save to file
        if ui_response.config_save_file_requested {
            let config = self.export_config();

            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Fractal Flame Config", &["fflame"])
                    .set_file_name("fractal.fflame")
                    .save_file()
                {
                    if let Err(e) = config.save_to_file(&path) {
                        eprintln!("Failed to save config: {}", e);
                    } else {
                        println!("Config saved to: {}", path.display());
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: async file dialog
                if let Ok(json) = config.to_json() {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Some(file_handle) = rfd::AsyncFileDialog::new()
                            .add_filter("Fractal Flame Config", &["fflame"])
                            .set_file_name("fractal.fflame")
                            .save_file()
                            .await
                        {
                            let _ = file_handle.write(json.as_bytes()).await;
                        }
                    });
                }
            }
        }

        // Handle config import
        if let Some(json) = ui_response.config_import_requested {
            match FractalConfig::from_json(&json) {
                Ok(config) => {
                    if let Err(e) = self.load_config_with_undo(config, "Import Config".to_string()) {
                        eprintln!("Failed to import config: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to import config: {}", e);
                }
            }
        }

        // Handle add transform
        if ui_response.add_transform {
            let insert_index = self.config_manager.active_config().flame.transforms.len();

            // Create a new default transform with identity affine and linear variation
            let mut new_transform = crate::scene::transforms::Transform::default();
            // Linear variation with weight 1.0
            new_transform.set_variation("linear", 1.0);
            new_transform.color = 0.5;  // Mid-palette position
            new_transform.color_speed = 0.5;

            // Create specialized snapshot for efficient undo/redo
            let change = crate::config::ConfigChange::add_transform_snapshot(
                insert_index,
                new_transform,
                "Add Transform".to_string(),
            );

            if let Err(e) = self.config_manager.apply_structural_change(change) {
                eprintln!("Failed to add transform: {}", e);
            } else {
                // Update app state from config
                self.flame = self.config_manager.active_config().flame.clone();
            }
        }

        // Handle delete transform
        if let Some(idx) = ui_response.delete_transform {
            let config = self.config_manager.active_config();

            if config.flame.transforms.len() > 1 && idx < config.flame.transforms.len() {
                // Get the transform before deleting
                let deleted_transform = config.flame.transforms[idx].clone();

                // Create specialized snapshot for efficient undo/redo
                let change = crate::config::ConfigChange::delete_transform_snapshot(
                    idx,
                    deleted_transform,
                    format!("Delete Transform {}", idx + 1),
                );

                if let Err(e) = self.config_manager.apply_structural_change(change) {
                    eprintln!("Failed to delete transform: {}", e);
                } else {
                    // Update app state from config
                    self.flame = self.config_manager.active_config().flame.clone();
                }
            }
        }

        // Handle custom palette from editor or library
        if let Some(custom_pal) = ui_response.custom_palette {
            // Add or update palette in library (prevents duplicates)
            let _palette_index = self.palette_library.add_or_update(custom_pal.clone());

            // Apply the palette to the config via ConfigManager
            if let Ok(update) = self.config_manager.update_param(
                crate::config::ConfigPath::Palette,
                crate::config::ConfigValue::Palette(custom_pal.clone()),
            ) {
                // Update renderer if needed (ColorOnly or IterationReset)
                if matches!(update, crate::config::UpdateType::ColorOnly | crate::config::UpdateType::IterationReset) {
                    let config = self.config_manager.active_config();
                    if let Some(ref mut renderer) = self.flame_renderer {
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &custom_pal, config.palette_rotation);
                    }
                }
            }
        }

        // Handle config load from file
        if ui_response.config_load_file_requested {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Fractal Flame Config", &["fflame"])
                    .pick_file()
                {
                    match FractalConfig::load_multi_from_file(&path) {
                        Ok(configs) => {
                            if configs.is_empty() {
                                eprintln!("No configurations found in file");
                            } else if configs.len() == 1 {
                                // Single config: load directly
                                let config = configs.into_iter().next().unwrap();
                                if let Err(e) = self.load_config_with_undo(config, "Load Config".to_string()) {
                                    eprintln!("Failed to load config: {}", e);
                                } else {
                                    println!("Config loaded from: {}", path.display());
                                }
                            } else {
                                // Multiple configs: load first one and open File Browser
                                let filename = path.file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("file")
                                    .to_string();
                                println!("Found {} configs in {}, loading first and opening File Browser", configs.len(), filename);

                                // Load all configs into File Browser
                                self.egui_layer.load_configs_into_browser(configs.clone(), &filename);

                                // Load the first config
                                let first_config = configs.into_iter().next().unwrap();
                                if let Err(e) = self.load_config_with_undo(first_config, "Load Config".to_string()) {
                                    eprintln!("Failed to load config: {}", e);
                                }

                                // Open the File Browser panel
                                use crate::ui::workspace::PanelType;
                                self.workspace.open_floating_panel(PanelType::FileBrowser);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to load config: {}", e);
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: async file dialog - we'll spawn the dialog and handle the result
                // Note: We can't directly import_config from async, so we'll just load to buffer
                let ctx = self.egui_layer.ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(file_handle) = rfd::AsyncFileDialog::new()
                        .add_filter("Fractal Flame", &["flame"])
                        .pick_file()
                        .await
                    {
                        let contents = file_handle.read().await;
                        let json = String::from_utf8_lossy(&contents).to_string();
                        // Copy to clipboard so user can paste it
                        ctx.copy_text(json);
                        log::info!("Config loaded to clipboard - paste to import");
                    }
                });
            }
        }

        // Handle Apophysis .flame import
        if ui_response.apophysis_import_file_requested {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Apophysis Flame", &["flame"])
                    .pick_file()
                {
                    match std::fs::read_to_string(&path) {
                        Ok(xml) => {
                            match crate::apophysis_xml::parse_flame_xml(&xml) {
                                Ok(configs) => {
                                    if configs.is_empty() {
                                        eprintln!("No flames found in file");
                                    } else if configs.len() == 1 {
                                        // Single flame: import directly
                                        let config = configs.into_iter().next().unwrap();
                                        if let Err(e) = self.load_config_with_undo(config, "Import Apophysis Flame".to_string()) {
                                            eprintln!("Failed to import flame: {}", e);
                                        } else {
                                            println!("Imported Apophysis flame from: {}", path.display());
                                        }
                                    } else {
                                        // Multiple flames: load first one and open File Browser
                                        let filename = path.file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("file")
                                            .to_string();
                                        println!("Found {} flames in {}, loading first and opening File Browser", configs.len(), filename);

                                        // Load all configs into File Browser
                                        self.egui_layer.load_configs_into_browser(configs.clone(), &filename);

                                        // Load the first config
                                        let first_config = configs.into_iter().next().unwrap();
                                        if let Err(e) = self.load_config_with_undo(first_config, "Import Apophysis Flame".to_string()) {
                                            eprintln!("Failed to import flame: {}", e);
                                        }

                                        // Open the File Browser panel
                                        use crate::ui::workspace::PanelType;
                                        self.workspace.open_floating_panel(PanelType::FileBrowser);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse Apophysis XML: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read file: {}", e);
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: async file dialog
                let ctx = self.egui_layer.ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(file_handle) = rfd::AsyncFileDialog::new()
                        .add_filter("Apophysis Flame", &["flame"])
                        .pick_file()
                        .await
                    {
                        let contents = file_handle.read().await;
                        let xml = String::from_utf8_lossy(&contents).to_string();
                        match crate::apophysis_xml::parse_flame_xml(&xml) {
                            Ok(configs) => {
                                if !configs.is_empty() {
                                    // Store the config in egui memory for pickup on next frame
                                    ctx.data_mut(|data| {
                                        data.insert_temp(
                                            egui::Id::new("pending_apophysis_import"),
                                            configs[0].clone()
                                        );
                                    });
                                    log::info!("Apophysis flame imported successfully");
                                    ctx.request_repaint();
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to parse Apophysis XML: {}", e);
                            }
                        }
                    }
                });
            }
        }

        // Handle file browser open request
        if ui_response.file_browser_open_requested {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Fractal Flame Config", &["fflame"])
                    .pick_file()
                {
                    self.egui_layer.load_file_into_browser(path);
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: async file dialog - load into egui memory
                let ctx = self.egui_layer.ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(file_handle) = rfd::AsyncFileDialog::new()
                        .add_filter("Fractal Flame", &["fflame"])
                        .pick_file()
                        .await
                    {
                        let contents = file_handle.read().await;
                        let json = String::from_utf8_lossy(&contents).to_string();

                        // Store the JSON in egui memory for pickup on next frame
                        ctx.data_mut(|data| {
                            data.insert_temp(
                                egui::Id::new("pending_file_browser_json"),
                                json
                            );
                        });
                        ctx.request_repaint();
                    }
                });
            }
        }

        // Handle palette export to clipboard
        if let Some(palette) = ui_response.palette_export_json {
            if let Ok(json) = serde_json::to_string_pretty(&palette) {
                self.egui_layer.ctx.copy_text(json);
            }
        }

        // Handle palette save to file
        if let Some(palette) = ui_response.palette_save_file {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Palette", &["palette"])
                    .set_file_name("palette.palette")
                    .save_file()
                {
                    if let Ok(json) = serde_json::to_string_pretty(&palette) {
                        if let Err(e) = std::fs::write(&path, json) {
                            eprintln!("Failed to save palette: {}", e);
                        } else {
                            println!("Palette saved to: {}", path.display());
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: async file dialog
                if let Ok(json) = serde_json::to_string_pretty(&palette) {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Some(file_handle) = rfd::AsyncFileDialog::new()
                            .add_filter("Palette", &["palette"])
                            .set_file_name("palette.palette")
                            .save_file()
                            .await
                        {
                            let _ = file_handle.write(json.as_bytes()).await;
                        }
                    });
                }
            }
        }

        // Handle palette import from JSON
        if let Some(json) = ui_response.palette_import_json {
            match serde_json::from_str::<crate::scene::palette::Palette>(&json) {
                Ok(mut palette) => {
                    // Check if palette with same name exists (case-insensitive)
                    let existing_idx = self.palette_library.iter()
                        .position(|p| p.name.to_lowercase() == palette.name.to_lowercase());

                    if existing_idx.is_some() {
                        // Generate unique name with (Copy) or (Copy N) suffix
                        let base_name = palette.name.clone();
                        let mut counter = 1;
                        let mut new_name = format!("{} (Copy)", base_name);

                        while self.palette_library.iter().any(|p| p.name.to_lowercase() == new_name.to_lowercase()) {
                            counter += 1;
                            new_name = format!("{} (Copy {})", base_name, counter);
                        }

                        palette.name = new_name;
                        palette.built_in = false; // Mark as custom
                    } else {
                        palette.built_in = false; // Mark as custom
                    }

                    // Add to library (now guaranteed to have unique name)
                    let _palette_idx = self.palette_library.add_or_update(palette.clone());

                    // Update palette editor with the new palette
                    self.egui_layer.update_palette_editor(palette.clone());

                    // Set as active palette in config (this is what the UI checks)
                    let _ = self.config_manager.update_param(
                        crate::config::ConfigPath::Palette,
                        crate::config::ConfigValue::Palette(palette.clone())
                    );

                    // Update renderer
                    let config = self.config_manager.active_config();
                    if let Some(ref mut renderer) = self.flame_renderer {
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &palette, config.palette_rotation);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to import palette: {}", e);
                }
            }
        }

        // Handle palette load from file
        if ui_response.palette_load_file {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Palette", &["palette"])
                    .pick_file()
                {
                    match std::fs::read_to_string(&path) {
                        Ok(json) => {
                            match serde_json::from_str::<crate::scene::palette::Palette>(&json) {
                                Ok(palette) => {
                                    // Update palette editor
                                    self.egui_layer.update_palette_editor(palette.clone());

                                    // Add or update in library (prevents duplicates)
                                    let palette_idx = self.palette_library.add_or_update(palette.clone());
                                    // Set to the palette
                                    let _ = self.config_manager.update_param(
                                        crate::config::ConfigPath::PaletteIndex,
                                        (palette_idx as u32).into()
                                    );

                                    // Update renderer
                                    let config = self.config_manager.active_config();
                                    if let Some(ref mut renderer) = self.flame_renderer {
                                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &palette, config.palette_rotation);
                                    }

                                    println!("Palette loaded from: {}", path.display());
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse palette file: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read palette file: {}", e);
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: async file dialog
                let ctx = self.egui_layer.ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(file_handle) = rfd::AsyncFileDialog::new()
                        .add_filter("Palette", &["palette"])
                        .pick_file()
                        .await
                    {
                        let contents = file_handle.read().await;
                        let json = String::from_utf8_lossy(&contents).to_string();
                        // Copy to clipboard so user can paste it
                        ctx.copy_text(json);
                        log::info!("Palette loaded to clipboard - paste to import");
                    }
                });
            }
        }

        // Handle undo/redo from UI buttons
        if ui_response.undo_requested {
            self.undo();
        }
        if ui_response.redo_requested {
            self.redo();
        }

        // Handle panel open requests
        if ui_response.open_palette_editor {
            use crate::ui::workspace::PanelType;
            self.workspace.open_floating_panel(PanelType::PaletteEditor);
        }
        if ui_response.open_config_dialog {
            use crate::ui::workspace::PanelType;
            self.workspace.open_floating_panel(PanelType::ConfigDialog);
        }

        // Check for pending Apophysis import from WASM async file dialog
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(config) = self.egui_layer.ctx.data_mut(|data| {
                data.remove_temp::<crate::config::FractalConfig>(egui::Id::new("pending_apophysis_import"))
            }) {
                if let Err(e) = self.load_config_with_undo(config, "Import Apophysis Flame".to_string()) {
                    log::error!("Failed to import Apophysis flame: {}", e);
                } else {
                    log::info!("Apophysis flame imported successfully");
                }
            }
        }

        // Handle preset selection from Preset Library panel
        if let Some(config) = ui_response.selected_preset_config {
            if let Err(e) = self.load_config_with_undo(config, "Load Preset".to_string()) {
                log::error!("Failed to load preset: {}", e);
            } else {
                log::info!("Preset loaded successfully");
            }
        }

        // Handle PNG export
        if ui_response.png_export_with_background || ui_response.png_export_transparent {
            let transparent = ui_response.png_export_transparent;

            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: use blocking task for both capture and save
                // Build metadata before borrowing renderer
                let export_config = self.export_config();
                let render_time_ms = self.metrics.render_time_ms;

                // Check if we need custom-size export
                if self.use_custom_export_size {
                    // Custom-size export: create temporary renderer at export dimensions
                    self.export_custom_size(transparent, export_config, render_time_ms);
                } else if let Some(ref mut renderer) = self.flame_renderer {
                    // Viewport-size export: use current renderer
                    let total_iterations = renderer.total_iterations();

                    // For transparent export, we need to re-run tonemap with transparent_mode=1
                    if transparent {
                        let iterations_per_thread = self.config_manager.system_settings().iterations_per_thread;
                        renderer.set_transparent_mode(&self.gpu.queue, true, &export_config, iterations_per_thread);

                        // Run tonemap pass with transparent mode
                        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                            label: Some("Transparent Export Tonemap"),
                        });
                        renderer.tonemap_pass(&mut encoder);
                        self.gpu.queue.submit(std::iter::once(encoder.finish()));
                    }

                    let pixels_future = renderer.read_fractal_pixels(&self.gpu.device, &self.gpu.queue, transparent, export_config.background_color);

                    match pollster::block_on(pixels_future) {
                        Ok((width, height, rgba_data)) => {
                            // Build metadata with captured values
                            let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                width,
                                height,
                                total_iterations,
                                render_time_ms,
                                self.config_manager.system_settings().iterations_per_thread,
                                export_config.speed_factor,
                                &export_config,
                            );

                            // Encode PNG with metadata
                            match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata)) {
                                Ok(png_data) => {
                                    // Open file dialog
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter("PNG Image", &["png"])
                                        .set_file_name("fractal.png")
                                        .save_file()
                                    {
                                        if let Err(e) = std::fs::write(&path, png_data) {
                                            eprintln!("Failed to save PNG: {}", e);
                                        } else {
                                            println!("PNG saved to: {}", path.display());
                                        }
                                    }
                                }
                                Err(e) => eprintln!("Failed to encode PNG: {}", e),
                            }
                        }
                        Err(e) => eprintln!("Failed to capture pixels: {}", e),
                    }

                    // Reset transparent mode back to normal for display
                    if transparent {
                        let iterations_per_thread = self.config_manager.system_settings().iterations_per_thread;
                        renderer.set_transparent_mode(&self.gpu.queue, false, &export_config, iterations_per_thread);

                        // Run tonemap pass to restore normal display
                        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                            label: Some("Restore Normal Tonemap"),
                        });
                        renderer.tonemap_pass(&mut encoder);
                        self.gpu.queue.submit(std::iter::once(encoder.finish()));
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: Use async task for pixel reading and file save
                use wasm_bindgen_futures::spawn_local;
                use crate::renderer::compute_kernel::FlameRenderer;

                let export_config = self.export_config();
                let iterations_per_thread = self.config_manager.system_settings().iterations_per_thread;
                let background_color = export_config.background_color;

                if self.use_custom_export_size {
                    // Custom size export: create temporary renderer and render
                    let export_width = self.export_width;
                    let export_height = self.export_height;
                    let max_iterations = export_config.max_iterations;

                    log::info!("WASM: Exporting at custom size {}×{}", export_width, export_height);

                    // Create temporary renderer at export dimensions
                    let surface_format = egui_wgpu::wgpu::TextureFormat::Rgba8Unorm;
                    let mut temp_renderer = FlameRenderer::new(
                        &self.gpu.device,
                        &self.gpu.queue,
                        surface_format,
                        export_width,
                        export_height,
                        &export_config.flame,
                    );

                    // Load config into temp renderer
                    let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                        label: Some("WASM Custom Export Encoder"),
                    });

                    let palette = export_config.palette.as_ref()
                        .or_else(|| self.palette_library.get(export_config.palette_index))
                        .cloned()
                        .unwrap_or_default();

                    temp_renderer.load_config(&self.gpu.device, &mut encoder, &self.gpu.queue, &export_config, &palette, iterations_per_thread, 20); // burn_in - use default for WASM export
                    self.gpu.queue.submit(std::iter::once(encoder.finish()));

                    // Render frames until we reach max_iterations
                    let render_start = web_time::Instant::now();
                    let mut total_rendered = 0u64;

                    const NUM_WORKGROUPS: u32 = 128;
                    const THREADS_PER_WORKGROUP: u64 = 64;
                    const BATCH_SIZE: u32 = 4;

                    let mut batch_frame_count = 0;

                    while total_rendered < max_iterations {
                        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                            label: Some("WASM Export Render Frame"),
                        });

                        let clear_histogram = batch_frame_count == 0;
                        // Clear paths only on very first batch of the entire export
                        let clear_paths = total_rendered == 0 && clear_histogram;

                        temp_renderer.compute_pass(
                            &mut encoder,
                            &self.gpu.queue,
                            NUM_WORKGROUPS,
                            iterations_per_thread,
                            20, // burn_in - use default for WASM export
                            export_config.zoom,
                            export_config.pan_x,
                            export_config.pan_y,
                            export_config.rotation,
                            export_config.camera_rotation_x,
                            export_config.camera_rotation_y,
                            export_config.camera_z,
                            export_config.speed_factor,
                            clear_histogram,
                            clear_paths,
                        );

                        let samples_this_frame = NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * iterations_per_thread as u64;
                        total_rendered += samples_this_frame;
                        batch_frame_count += 1;

                        if batch_frame_count >= BATCH_SIZE {
                            let total_samples_in_batch = samples_this_frame * BATCH_SIZE as u64;
                            temp_renderer.accumulate_pass(&mut encoder, &self.gpu.queue, &self.gpu.device, total_samples_in_batch);
                            batch_frame_count = 0;
                        }

                        self.gpu.queue.submit(std::iter::once(encoder.finish()));

                        if total_rendered >= max_iterations {
                            if batch_frame_count > 0 {
                                let mut final_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                                    label: Some("WASM Export Final Accumulation"),
                                });
                                let total_samples_in_batch = samples_this_frame * batch_frame_count as u64;
                                temp_renderer.accumulate_pass(&mut final_encoder, &self.gpu.queue, &self.gpu.device, total_samples_in_batch);
                                self.gpu.queue.submit(std::iter::once(final_encoder.finish()));
                            }
                            break;
                        }
                    }

                    let render_time_ms = render_start.elapsed().as_secs_f64() * 1000.0;

                    // Set transparent mode if requested
                    if transparent {
                        temp_renderer.set_transparent_mode(&self.gpu.queue, true, &export_config, iterations_per_thread);
                    }

                    // Final tonemap pass
                    let mut final_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                        label: Some("WASM Export Final Tonemap"),
                    });
                    temp_renderer.tonemap_pass(&mut final_encoder);
                    self.gpu.queue.submit(std::iter::once(final_encoder.finish()));

                    // Move renderer to heap for async task
                    let temp_renderer = Box::new(temp_renderer);
                    let speed_factor = export_config.speed_factor;

                    // SAFETY: Device and Queue live for program lifetime.
                    // temp_renderer is moved into the async task and will be dropped there.
                    let device: &'static egui_wgpu::wgpu::Device = unsafe { std::mem::transmute(&self.gpu.device) };
                    let queue: &'static egui_wgpu::wgpu::Queue = unsafe { std::mem::transmute(&self.gpu.queue) };

                    spawn_local(async move {
                        match temp_renderer.read_fractal_pixels(device, queue, transparent, background_color).await {
                            Ok((width, height, rgba_data)) => {
                                let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                    width,
                                    height,
                                    total_rendered,
                                    render_time_ms,
                                    iterations_per_thread,
                                    speed_factor,
                                    &export_config,
                                );

                                match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata)) {
                                    Ok(png_data) => {
                                        if let Some(file_handle) = rfd::AsyncFileDialog::new()
                                            .add_filter("PNG Image", &["png"])
                                            .set_file_name("fractal.png")
                                            .save_file()
                                            .await
                                        {
                                            if let Err(e) = file_handle.write(&png_data).await {
                                                log::error!("Failed to save PNG: {:?}", e);
                                            } else {
                                                log::info!("PNG saved: {}×{} in {:.2}s", width, height, render_time_ms / 1000.0);
                                            }
                                        }
                                    }
                                    Err(e) => log::error!("Failed to encode PNG: {}", e),
                                }
                            }
                            Err(e) => log::error!("Failed to capture pixels: {}", e),
                        }
                        // temp_renderer is dropped here
                    });
                } else if let Some(ref mut renderer) = self.flame_renderer {
                    // Viewport size export: use current renderer
                    //
                    // IMPORTANT: We must read pixels BEFORE spawning the async task, because
                    // the next frame will overwrite fractal_texture with a new render.
                    // The async task is only used for the file dialog and write.

                    let total_iterations = renderer.total_iterations();
                    let render_time_ms = self.metrics.render_time_ms;
                    let speed_factor = export_config.speed_factor;

                    // For transparent export, set transparent mode and run tonemap before reading
                    if transparent {
                        renderer.set_transparent_mode(&self.gpu.queue, true, &export_config, iterations_per_thread);

                        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                            label: Some("Transparent Export Tonemap"),
                        });
                        renderer.tonemap_pass(&mut encoder);
                        self.gpu.queue.submit(std::iter::once(encoder.finish()));
                    }

                    // Read pixels NOW, before the next frame overwrites the texture
                    // We create a staging buffer and initiate the copy immediately
                    let width = renderer.width;
                    let height = renderer.height;
                    let (staging_buffer, padded_bytes_per_row) = renderer.create_pixel_staging_buffer(&self.gpu.device);

                    // Copy texture to staging buffer
                    let mut copy_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                        label: Some("Viewport Export Copy"),
                    });
                    renderer.copy_fractal_to_buffer(&mut copy_encoder, &staging_buffer, padded_bytes_per_row);
                    self.gpu.queue.submit(std::iter::once(copy_encoder.finish()));

                    // Now spawn async task to wait for buffer map and save file
                    let device: &'static egui_wgpu::wgpu::Device = unsafe { std::mem::transmute(&self.gpu.device) };

                    spawn_local(async move {
                        // Map the staging buffer (this is the async part)
                        let buffer_slice = staging_buffer.slice(..);
                        let (tx, rx) = futures::channel::oneshot::channel();
                        buffer_slice.map_async(egui_wgpu::wgpu::MapMode::Read, move |result| {
                            let _ = tx.send(result);
                        });
                        let _ = device.poll(egui_wgpu::wgpu::PollType::Wait { submission_index: None, timeout: None });

                        match rx.await {
                            Ok(Ok(())) => {
                                let data = buffer_slice.get_mapped_range();

                                // Extract RGBA data from staging buffer (handle row padding)
                                let bytes_per_pixel = 4u32;
                                let unpadded_bytes_per_row = (width * bytes_per_pixel) as usize;
                                let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);

                                for y in 0..height {
                                    let row_start = (y * padded_bytes_per_row) as usize;
                                    // Only copy the actual pixel data, not the padding
                                    let row_data = &data[row_start..row_start + unpadded_bytes_per_row];
                                    // For opaque export, the shader already blended with background (transparent_mode=0)
                                    // For transparent export, we ran tonemap with transparent_mode=1
                                    rgba_data.extend_from_slice(row_data);
                                }
                                drop(data);
                                staging_buffer.unmap();

                                // Build metadata and encode PNG
                                let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                    width,
                                    height,
                                    total_iterations,
                                    render_time_ms,
                                    iterations_per_thread,
                                    speed_factor,
                                    &export_config,
                                );

                                match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata)) {
                                    Ok(png_data) => {
                                        if let Some(file_handle) = rfd::AsyncFileDialog::new()
                                            .add_filter("PNG Image", &["png"])
                                            .set_file_name("fractal.png")
                                            .save_file()
                                            .await
                                        {
                                            if let Err(e) = file_handle.write(&png_data).await {
                                                log::error!("Failed to save PNG: {:?}", e);
                                            } else {
                                                log::info!("PNG saved successfully!");
                                            }
                                        }
                                    }
                                    Err(e) => log::error!("Failed to encode PNG: {}", e),
                                }
                            }
                            Ok(Err(e)) => log::error!("Buffer map error: {:?}", e),
                            Err(_) => log::error!("Failed to receive buffer map result"),
                        }
                    });
                }
            }
        }

        // Handle animation export request
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(export_settings) = ui_response.animation_export_requested {
            // Check if already exporting
            let already_exporting = self.animation_export_progress.lock().unwrap().is_exporting;
            if already_exporting {
                log::warn!("Animation export already in progress");
            } else if let Some(ref animation) = self.animation_controller.animation {
                use crate::animation::export::{AnimationExportConfig, UiProgressCallback, export_animation_fast, VideoEncodingSettings};

                let export_config = AnimationExportConfig {
                    config: self.config_manager.active_config().clone(),
                    animation: animation.clone(),
                    output_path: export_settings.output_path.clone(),
                    width: export_settings.width,
                    height: export_settings.height,
                    fps: export_settings.fps,
                    iterations_per_thread: export_settings.iterations_per_thread,
                    video_settings: VideoEncodingSettings {
                        codec: export_settings.video_codec,
                        hardware_accel: export_settings.hardware_accel,
                        quality: export_settings.video_quality,
                    },
                };

                println!("Starting animation export (background thread)...");
                println!("  Output: {}", export_config.output_path.display());
                println!("  Resolution: {}x{} @ {} FPS", export_config.width, export_config.height, export_config.fps);
                println!("  Total frames: {}", export_config.total_frames());
                println!("  Codec: {} (CRF {})", export_config.video_settings.codec.display_name(), export_config.video_settings.quality);

                // Set initial export progress
                {
                    let mut p = self.animation_export_progress.lock().unwrap();
                    p.is_exporting = true;
                    p.current_frame = 0;
                    p.total_frames = export_config.total_frames();
                    p.seconds_per_frame = 0.0;
                    p.status = "Starting export...".to_string();
                }

                // Clone progress Arc for the background thread
                let progress_arc = Arc::clone(&self.animation_export_progress);

                // Spawn background thread for export
                std::thread::spawn(move || {
                    let mut progress = UiProgressCallback::new(Arc::clone(&progress_arc));

                    match pollster::block_on(export_animation_fast(export_config, &mut progress)) {
                        Ok(result) => {
                            println!("\nAnimation export complete!");
                            println!("  {} frames in {:.1}s", result.total_frames, result.total_time_ms / 1000.0);
                            println!("  Output: {}", result.output_path.display());

                            // Mark export complete
                            if let Ok(mut p) = progress_arc.lock() {
                                p.is_exporting = false;
                                p.status = format!("Complete: {}", result.output_path.display());
                            }
                        }
                        Err(e) => {
                            eprintln!("Animation export failed: {}", e);
                            if let Ok(mut p) = progress_arc.lock() {
                                p.is_exporting = false;
                                p.status = format!("Failed: {}", e);
                            }
                        }
                    }
                });
            } else {
                eprintln!("No animation loaded for export");
            }
        }

        // Handle animation timeline scrubbing (slider drag or frame step)
        if ui_response.animation_seek_changed {
            // Evaluate animation at current scrubbed time and apply to config
            let frame_values = self.animation_controller.evaluate_frame();

            for (path_str, json_value) in frame_values {
                if let Some(path) = crate::config::ConfigPath::from_string_key(&path_str) {
                    if let Some(config_value) = crate::config::json_to_config_value(&json_value, &path) {
                        // Apply silently (no undo point) - just preview the position
                        if let Err(e) = self.config_manager.update_param_silent(path, config_value) {
                            log::warn!("Animation scrub: failed to update {}: {}", path_str, e);
                        }
                    }
                }
            }

            // Sync flame from config
            self.flame = self.config_manager.active_config().flame.clone();

            // Force a GPU update to show the scrubbed frame
            self.use_overwrite_next_frame = true;
        }

        // Handle UI responses and keyboard input (needs to be after submit since we need a new encoder)
        // Get pending actions from ConfigManager (replaces individual boolean flags)
        let actions = self.config_manager.get_pending_actions();

        // View changes can also come from keyboard input
        let view_changed_by_keyboard = self.view_changed_by_keyboard;


        // Determine if any GPU updates are needed
        let needs_update = actions.reset_accumulation || actions.update_flame || actions.update_palette
            || actions.update_tone_curve || actions.update_view || actions.rebuild_shader
            || view_changed_by_keyboard;

        if needs_update {
            if let Some(ref mut renderer) = self.flame_renderer {
                // Get current config for updates
                let update_config = self.config_manager.active_config();

                let mut update_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                    label: Some("Update Encoder"),
                });

                // Update flame if UpdateAction indicates (includes preview mode live updates)
                if actions.update_flame {
                    renderer.update_flame(&self.gpu.device, &self.gpu.queue, &self.flame,
                        self.config_manager.system_settings().iterations_per_thread, self.config_manager.system_settings().burn_in,
                        update_config.zoom, update_config.pan_x, update_config.pan_y,
                        update_config.rotation, update_config.camera_rotation_x, update_config.camera_rotation_y, update_config.camera_z, update_config.speed_factor);
                }

                // Update view parameters (includes view changes and iteration changes)
                if actions.update_view || view_changed_by_keyboard {
                    renderer.set_deterministic_rng(update_config.deterministic_rng);
                    renderer.update_iterations(&self.gpu.queue, self.config_manager.system_settings().iterations_per_thread, self.config_manager.system_settings().burn_in,
                        update_config.zoom, update_config.pan_x, update_config.pan_y, update_config.rotation,
                        update_config.camera_rotation_x, update_config.camera_rotation_y, update_config.camera_z, update_config.speed_factor);
                }

                // Update palette if needed (also handles color mode changes)
                if actions.update_palette {
                    // Get palette from ConfigManager (includes preview mode changes from palette editor)
                    let palette = update_config.palette.as_ref()
                        .or_else(|| self.palette_library.get(update_config.palette_index));

                    if let Some(palette) = palette {
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, palette, update_config.palette_rotation);
                    }

                    // Update color mode in GPU params (ColorMode changes trigger update_palette)
                    renderer.set_color_mode(&self.gpu.queue, update_config.color_mode,
                        self.config_manager.system_settings().iterations_per_thread, self.config_manager.system_settings().burn_in,
                        update_config.zoom, update_config.pan_x,
                        update_config.pan_y, update_config.rotation, update_config.camera_rotation_x,
                        update_config.camera_rotation_y, update_config.camera_z, update_config.speed_factor);
                }

                // Update tone curve LUT if changed
                if actions.update_tone_curve {
                    renderer.update_curve_lut(&self.gpu.queue, &update_config.tonemap_curve);
                }

                // Rebuild shader if variation set changed
                if actions.rebuild_shader {
                    // TODO: Implement shader rebuild logic when variation system supports it
                    // For now, this would require recreating the compute pipeline
                }

                // Handle accumulation reset based on change type
                let should_full_reset = actions.reset_accumulation || view_changed_by_keyboard;
                let has_view_or_color_change = actions.update_view || actions.update_palette;

                if should_full_reset {
                    // Structural changes: Clear buffer and reset counters (blank frame expected)
                    renderer.reset(&mut update_encoder, &self.gpu.queue, self.config_manager.system_settings().iterations_per_thread,
                        update_config.zoom, update_config.pan_x, update_config.pan_y, update_config.rotation,
                        update_config.camera_rotation_x, update_config.camera_rotation_y, update_config.camera_z, update_config.speed_factor);
                    self.frames_since_accumulation = 0;
                    self.rendering_complete = false;  // Reset completion flag
                    self.clear_paths_next_frame = true;  // Clear path buffer on full reset
                } else if has_view_or_color_change && renderer.total_iterations() >= update_config.max_iterations {
                    // View/color changes when fractal has stopped iterating:
                    // Reset counter to restart iteration (smooth transition via overwrite mode)
                    renderer.reset_iteration_counter();
                    self.frames_since_accumulation = 0;
                    self.rendering_complete = false;  // Reset completion flag
                    self.clear_paths_next_frame = true;  // Clear path buffer when restarting
                }

                self.gpu.queue.submit(std::iter::once(update_encoder.finish()));
            }
        }

        // Set overwrite flag based on whether we had changes recently
        // Keep it ON for brief period (100ms ~6 frames) after last change for smooth transitions
        // This handles continuous drag, discrete scroll, and transform changes
        // Note: Excludes tone_curve (post-processing only, doesn't affect accumulation buffer)
        let had_changes = actions.update_view || actions.update_palette || actions.update_flame;
        let now = web_time::Instant::now();

        // Track previous overwrite state to detect transitions
        let was_overwrite = self.use_overwrite_next_frame;

        if had_changes && !actions.reset_accumulation {
            // Changes happened → enable overwrite mode and update timestamp
            self.use_overwrite_next_frame = true;
            self.last_param_change_time = Some(now);
        } else if !had_changes {
            // No changes this frame → check if we're still within the smooth transition window
            if let Some(last_change) = self.last_param_change_time {
                let time_since_change = now.duration_since(last_change);
                // Keep overwrite ON for 100ms after last change (~6 frames at 60fps)
                self.use_overwrite_next_frame = time_since_change.as_millis() < 100;

                // When overwrite window expires, reset iteration counter for clean rebuild
                if was_overwrite && !self.use_overwrite_next_frame {
                    if let Some(ref mut renderer) = self.flame_renderer {
                        renderer.reset_iteration_counter();
                        self.rendering_complete = false;  // Reset completion flag
                        self.clear_paths_next_frame = true;  // Clear path buffer for clean rebuild
                        log::debug!("Overwrite window expired → reset iteration counter for clean rebuild");
                    }
                }
            } else {
                self.use_overwrite_next_frame = false;
            }
        }
        // If reset_accumulation=true, disable overwrite (let normal accumulation work after reset)

        // Clear pending actions after executing them
        self.config_manager.clear_pending_actions();

        // Clear keyboard flag for next frame
        self.view_changed_by_keyboard = false;

        // ============================================================================
        // ANIMATION UPDATE (between UI processing and rendering)
        // ============================================================================
        // Detect animation state transitions and update FSM accordingly
        let was_fsm_animating = self.render_mode.is_animating();
        let is_controller_playing = self.animation_controller.state == PlaybackState::Playing;

        // Detect play start: controller started playing but FSM not yet in animation mode
        if is_controller_playing && !was_fsm_animating {
            self.render_mode.enter_animation(self.config_manager.active_config());
            // Enable animation mode in ConfigManager - UI changes become silent (no undo)
            self.config_manager.set_animation_mode(true);
        }

        // Detect user stop/pause: FSM was animating but controller is no longer playing
        // (This catches manual stop/pause clicks from UI - auto-stop is handled below after update())
        if was_fsm_animating && !is_controller_playing {
            // Disable animation mode before exit so undo entry creation works
            self.config_manager.set_animation_mode(false);
            self.handle_animation_exit();

            // Only restore base config when animation is STOPPED (not paused)
            // When paused, the fractal should stay at the current timeline position
            if self.animation_controller.state == PlaybackState::Stopped {
                if let Some(ref animation) = self.animation_controller.animation {
                    if let Some(ref base_config) = animation.base_config {
                        // Load the base config silently (the undo entry was already created by handle_animation_exit)
                        if let Err(e) = self.config_manager.load_config_silent(base_config.clone()) {
                            log::error!("Failed to restore base config: {}", e);
                        }
                        self.flame = base_config.flame.clone();
                        self.use_overwrite_next_frame = true;
                    }
                }
            }
        }

        if is_controller_playing {
            // Update animation time (delta_time calculated at frame start, before last_frame_time update)
            self.animation_controller.update(delta_time);

            // Check if animation auto-stopped (LoopMode::Once reached end)
            let auto_stopped = self.animation_controller.state != PlaybackState::Playing;
            if auto_stopped {
                // Disable animation mode before exit so undo entry creation works
                self.config_manager.set_animation_mode(false);
                // Animation finished naturally - exit animation mode and create undo snapshot
                self.handle_animation_exit();

                // Restore base config when animation stops (returns to original state)
                if let Some(ref animation) = self.animation_controller.animation {
                    if let Some(ref base_config) = animation.base_config {
                        // Load the base config silently (the undo entry was already created by handle_animation_exit)
                        if let Err(e) = self.config_manager.load_config_silent(base_config.clone()) {
                            log::error!("Failed to restore base config: {}", e);
                        }
                        self.flame = base_config.flame.clone();
                        self.use_overwrite_next_frame = true;
                    }
                }
            }

            // Evaluate all tracks and apply values to ConfigManager (silently, no undo)
            let frame_values = self.animation_controller.evaluate_frame();

            for (path_str, json_value) in frame_values {
                // Parse the string key back to ConfigPath
                if let Some(path) = crate::config::ConfigPath::from_string_key(&path_str) {
                    // Convert JSON value to ConfigValue
                    if let Some(config_value) = crate::config::json_to_config_value(&json_value, &path) {
                        // Apply silently (no undo point)
                        if let Err(e) = self.config_manager.update_param_silent(path, config_value) {
                            log::warn!("Animation: failed to update {}: {}", path_str, e);
                        }
                    }
                } else {
                    log::warn!("Animation: unknown path key: {}", path_str);
                }
            }

            // Sync flame from config (animation may have changed transform parameters)
            self.flame = self.config_manager.active_config().flame.clone();
        }

        // ============================================================================
        // PHASE 3: Get FINAL Config and Render Fractal
        // ============================================================================
        // Single config read after all updates are complete
        let final_config = self.config_manager.active_config();

        // Create new encoder for rendering phase
        let mut render_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Fractal Render Encoder"),
        });

        // Run flame compute shader with progressive refinement
        if let Some(ref mut renderer) = self.flame_renderer {
            // Overwrite mode logic:
            // - Use flag set in previous frame (changes were detected then, applied now)
            // - When fractal stopped: Always allow overwrite to enable live parameter updates
            // - During animation playback: Depends on quality mode setting
            //   - Responsive mode: Use overwrite for smooth real-time preview
            //   - HighQuality mode: Use batched accumulation for better quality
            let has_stopped = renderer.total_iterations() >= final_config.max_iterations;
            let animation_uses_overwrite = is_controller_playing && self.animation_controller.use_overwrite_mode();
            let use_overwrite = self.use_overwrite_next_frame || has_stopped || animation_uses_overwrite;
            renderer.set_overwrite_mode(use_overwrite);

            // Check if we should continue iterating
            // During animation playback, always iterate (ignore max_iterations limit)
            // Skip GPU work during video export to avoid GPU contention (separate device in background thread)
            let is_video_exporting = self.animation_export_progress.lock()
                .map(|p| p.is_exporting)
                .unwrap_or(false);
            let max_iterations = Some(final_config.max_iterations);
            let should_iterate = !self.paused && !is_video_exporting && (
                is_controller_playing ||
                max_iterations.map_or(true, |max| renderer.total_iterations() < max)
            );

            // Mark rendering as complete the frame after max_iterations is reached
            if !should_iterate && !self.rendering_complete {
                self.rendering_complete = true;
                log::debug!("Rendering complete: max_iterations reached");
            }

            if should_iterate {
                const NUM_WORKGROUPS: u32 = 128;

                self.frames_since_accumulation += 1;

                // Determine if we should accumulate this frame
                // During overwrite mode, accumulate every frame for smooth transitions
                // During normal accumulation, batch to reduce GPU overhead
                let batch_size = if use_overwrite { 1 } else { self.accumulation_batch_size };
                let should_accumulate = self.frames_since_accumulation >= batch_size;

                let t_compute = Instant::now();
                // 1. Compute new samples with fresh random seed
                // Clear histogram only when starting a new batch (frame 1 of batch)
                let clear_histogram = self.frames_since_accumulation == 1;
                // Clear paths only on full reset (not every batch)
                let clear_paths = self.clear_paths_next_frame;
                if clear_paths {
                    self.clear_paths_next_frame = false;  // Reset flag after use
                }

                let samples_this_frame = renderer.compute_pass(&mut render_encoder, &self.gpu.queue, NUM_WORKGROUPS,
                    self.config_manager.system_settings().iterations_per_thread, self.config_manager.system_settings().burn_in,
                    final_config.zoom, final_config.pan_x, final_config.pan_y, final_config.rotation,
                    final_config.camera_rotation_x, final_config.camera_rotation_y, final_config.camera_z, final_config.speed_factor, clear_histogram, clear_paths);

                self.metrics.record_compute_time(t_compute.elapsed().as_secs_f64() * 1000.0);

                let t_accumulate = Instant::now();
                // 2. Accumulate samples - but only every N frames if batching enabled
                if should_accumulate {
                    // samples_this_frame is only THIS frame's samples, but histogram contains
                    // accumulated samples from all frames in the batch
                    // Pass total samples for proper blend_factor calculation
                    let total_samples_in_batch = samples_this_frame * batch_size as u64;

                    renderer.accumulate_pass(&mut render_encoder, &self.gpu.queue, &self.gpu.device, total_samples_in_batch);

                    self.frames_since_accumulation = 0;
                    self.metrics.record_accumulate_time(t_accumulate.elapsed().as_secs_f64() * 1000.0);
                } else {
                    self.metrics.record_accumulate_time(0.0);
                }
            } else {
                self.metrics.record_compute_time(0.0);
                self.metrics.record_accumulate_time(0.0);
            }
            
            let t_tonemap = Instant::now();
            // 3. Update accumulation parameters from config
            renderer.set_low_density_smoothing(final_config.low_density_smoothing);
            renderer.set_density_compression_strength(final_config.density_compression_strength);
            renderer.set_blend_factor(final_config.blend_factor);
            renderer.set_use_dynamic_blend(final_config.use_dynamic_blend);
            renderer.set_target_iterations_per_pixel(final_config.target_iterations_per_pixel);

            // 4. Update tonemap parameters and render to fractal texture
            renderer.update_density_scale(&self.gpu.queue, final_config.density_scale);
            renderer.update_background_color(&self.gpu.queue, final_config.background_color);
            renderer.set_path_map_style(final_config.path_map_style);
            // Calculate batch_size for tonemap (same logic as accumulation)
            let batch_size_for_tonemap = if use_overwrite { 1 } else { self.accumulation_batch_size };
            // is_live_preview: Only during active editing, not when rendering stops
            let is_live_preview = self.use_overwrite_next_frame;
            renderer.update_tonemap(&self.gpu.queue, final_config.tonemap_mode, final_config.use_curve,
                final_config.exposure, final_config.gamma, final_config.gamma_threshold, final_config.brightness,
                final_config.vibrancy, final_config.saturation, final_config.hue_shift, final_config.value_scale,
                final_config.alpha_blend_low, final_config.alpha_blend_high,
                renderer.width, renderer.height, renderer.total_iterations(), final_config.max_iterations, final_config.zoom,
                self.config_manager.system_settings().iterations_per_thread, batch_size_for_tonemap, is_live_preview);

            // Render to internal fractal texture
            renderer.tonemap_pass(&mut render_encoder);

            self.metrics.record_tonemap_time(t_tonemap.elapsed().as_secs_f64() * 1000.0);
        }

        // Submit rendering commands
        let t_submit = Instant::now();

        self.gpu.queue.submit(std::iter::once(render_encoder.finish()));
        self.metrics.record_submit_time(t_submit.elapsed().as_secs_f64() * 1000.0);

        // Generate one thumbnail for preset library if needed (one per frame)
        // This is blocking but only ~1-2 seconds per thumbnail
        // Note: Thumbnail generation uses pollster which isn't available on WASM
        #[cfg(not(target_arch = "wasm32"))]
        if self.egui_layer.preset_library_needs_thumbnails() {
            self.egui_layer.generate_preset_thumbnail(
                &self.gpu.device,
                &self.gpu.queue,
                &self.palette_library,
            );
            // Request immediate repaint to continue generation next frame
            window.request_redraw();
        }

        // Generate one thumbnail for file browser if needed (one per frame)
        #[cfg(not(target_arch = "wasm32"))]
        if self.egui_layer.file_browser_needs_thumbnails() {
            self.egui_layer.generate_file_browser_thumbnail(
                &self.gpu.device,
                &self.gpu.queue,
                &self.palette_library,
            );
            // Request immediate repaint to continue generation next frame
            window.request_redraw();
        }

        // Handle PathMap mode: query path at clicked pixel or close overlay
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Check if close was requested
            if self.egui_layer.take_close_path_overlay() {
                self.egui_layer.set_path_click_info(None);
            }

            // Handle new path query
            if let Some((click_x, click_y)) = self.egui_layer.take_clicked_pixel() {
                if let Some(ref renderer) = self.flame_renderer {
                    let config = self.config_manager.active_config();
                    let width = renderer.width;
                    let height = renderer.height;

                    // Clamp to valid pixel coordinates
                    let pixel_x = click_x.min(width - 1);
                    let pixel_y = click_y.min(height - 1);

                    // Read path entry for this specific pixel
                    let path_entry = match pollster::block_on(renderer.read_path_buffer(&self.gpu.device, &self.gpu.queue)) {
                        Ok(path_buffer) => path_buffer[pixel_y as usize][pixel_x as usize],
                        Err(e) => {
                            log::error!("Failed to read path buffer: {}", e);
                            crate::renderer::PathEntry::default()
                        }
                    };

                    // Calculate fractal space coordinates
                    let fractal_coords = Self::pixel_to_fractal(pixel_x, pixel_y, width, height, config);

                    // Read 9x9 color preview from fractal texture
                    let color_preview = match pollster::block_on(
                        renderer.read_pixel_region(&self.gpu.device, &self.gpu.queue, pixel_x, pixel_y, 9, 9)
                    ) {
                        Ok(pixels) => pixels,
                        Err(_) => vec![[0, 0, 0, 255]; 81], // Fallback to black
                    };

                    let click_info = crate::ui::PathClickInfo {
                        click_pixel: (click_x, click_y),
                        found_pixel: (pixel_x, pixel_y),
                        fractal_coords,
                        search_distance: 0.0, // No search, exact pixel
                        path_entry,
                        color_preview,
                        preview_size: (9, 9),
                    };

                    if path_entry.iteration_count > 0 {
                        log::info!("Path at ({}, {}): {:?}", pixel_x, pixel_y, path_entry.to_vec());
                    } else {
                        log::debug!("No path data at ({}, {})", pixel_x, pixel_y);
                    }
                    self.egui_layer.set_path_click_info(Some(click_info));
                }
            }
        }

        let t5 = Instant::now();

        frame.present();

        self.metrics.record_present_time(t5.elapsed().as_secs_f64() * 1000.0);

        self.metrics.record_render_time(render_start.elapsed().as_secs_f64() * 1000.0);


        Ok(())
    }

    /// Graceful shutdown - performs cleanup and exits
    /// Called from: File → Quit, window close button (X), Alt+F4
    fn shutdown(&mut self, event_loop: &ActiveEventLoop) {
        // TODO: Check for unsaved changes
        // TODO: Show confirmation dialog if needed
        // TODO: Perform cleanup tasks

        log::info!("Graceful shutdown initiated");
        event_loop.exit();
    }

    /// Convert pixel coordinates to fractal space coordinates
    /// Takes into account zoom, pan, and rotation
    fn pixel_to_fractal(
        pixel_x: u32,
        pixel_y: u32,
        width: u32,
        height: u32,
        config: &crate::config::FractalConfig,
    ) -> (f32, f32) {
        // Convert pixel to normalized device coordinates (-1 to 1)
        let ndc_x = (pixel_x as f32 / width as f32) * 2.0 - 1.0;
        let ndc_y = (pixel_y as f32 / height as f32) * 2.0 - 1.0;

        // Account for aspect ratio
        let aspect = width as f32 / height as f32;
        let scaled_x = ndc_x * aspect;
        let scaled_y = ndc_y;

        // Apply inverse rotation
        let cos_r = (-config.rotation).cos();
        let sin_r = (-config.rotation).sin();
        let rotated_x = scaled_x * cos_r - scaled_y * sin_r;
        let rotated_y = scaled_x * sin_r + scaled_y * cos_r;

        // Apply inverse zoom and add pan
        let fractal_x = rotated_x / config.zoom + config.pan_x;
        let fractal_y = rotated_y / config.zoom + config.pan_y;

        (fractal_x, fractal_y)
    }
}
