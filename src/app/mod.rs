//! Application state and event loop

mod input;
mod config;
pub mod export;

#[cfg(not(target_arch = "wasm32"))]
pub use export::export_headless;

#[cfg(target_arch = "wasm32")]
pub use export::export_headless_wasm;

use winit::{event::*, event_loop::{EventLoop, ControlFlow, ActiveEventLoop}, window::Window};
use egui_wgpu::wgpu::SurfaceError;

use crate::gpu::device::GpuContext;
use crate::ui::EguiLayer;
use crate::renderer::FlameRenderer;
use crate::scene::transforms::Flame;
use crate::scene::palette::{PaletteLibrary, Palette, ColorMode};
use crate::scene::presets::PresetLibrary;
use crate::util::PerformanceMetrics;
use crate::config::{FractalConfig, ConfigManager};
use crate::scene::tonemap::{ToneMapMode, ToneCurve};

pub struct App {
    // Core state management
    pub(super) config_manager: ConfigManager,  // Single source of truth for all config

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

    // Performance tracking
    pub(super) metrics: PerformanceMetrics,

    // Rendering internals (frame timing and batching)
    pub(super) last_frame_time: Option<web_time::Instant>,
    pub(super) accumulation_batch_size: u32,  // Process every N frames (1 = normal, 4 = batched)
    pub(super) frames_since_accumulation: u32,
    pub(super) use_overwrite_next_frame: bool,  // Persist overwrite mode for brief period after changes
    pub(super) last_param_change_time: Option<web_time::Instant>,  // Track when params last changed

    // Fractal viewport size (updated from UI each frame)
    pub(super) fractal_viewport_size: (u32, u32),

    // PNG export settings (UI state only, not in config)
    pub(super) export_width: u32,
    pub(super) export_height: u32,
    pub(super) use_custom_export_size: bool,
}
impl App {
    pub async fn run(event_loop: EventLoop<()>, window: Window) -> Result<(), Box<dyn std::error::Error>> {
        let gpu = GpuContext::new(&window).await.expect("GPU init failed");
        let egui_layer = EguiLayer::new(&window, &gpu.device, gpu.config.format);

        // Load preset library and use first preset
        let preset_library = PresetLibrary::new();
        let initial_preset = preset_library.get(0).map(|p| p.flame.clone()).unwrap_or_default();
        let flame = initial_preset;

        let flame_renderer = FlameRenderer::new(
            &gpu.device,
            &gpu.queue,
            gpu.config.format,
            gpu.size.width,
            gpu.size.height,
            &flame,
        );

        let palette_library = PaletteLibrary::new();

        // Use palette from library (no need to duplicate)
        let initial_palette = palette_library.get(1)
            .cloned()
            .unwrap_or_else(|| Palette::grayscale());

        // Create initial config for undo history
        let initial_config = FractalConfig {
            flame: flame.clone(),
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            camera_rotation_x: 0.0,
            camera_rotation_y: 0.0,
            camera_z: 0.0,
            density_scale: 1.0,
            speed_factor: crate::config::DEFAULT_SPEED_FACTOR,
            max_iterations: crate::config::DEFAULT_MAX_ITERATIONS,
            color_mode: ColorMode::Palette,
            palette_index: 1,
            palette: Some(initial_palette),
            palette_rotation: 0.0,
            background_color: [0.0, 0.0, 0.0],
            tonemap_mode: ToneMapMode::Logarithmic,
            tonemap_curve: ToneCurve::linear(),
            use_curve: true,
            exposure: crate::config::DEFAULT_EXPOSURE,
            gamma: crate::config::DEFAULT_GAMMA,
            brightness: 1.0,
            vibrancy: 1.0,
            saturation: crate::config::defaults::DEFAULT_SATURATION,
            hue_shift: crate::config::defaults::DEFAULT_HUE_SHIFT,
            value_scale: crate::config::defaults::DEFAULT_VALUE_SCALE,
            gamma_threshold: crate::config::defaults::DEFAULT_GAMMA_THRESHOLD,
            deterministic_rng: false,
            histogram_color_scale: crate::config::DEFAULT_HISTOGRAM_COLOR_SCALE,
            low_density_smoothing: crate::config::DEFAULT_LOW_DENSITY_SMOOTHING,
            density_compression_strength: crate::config::DEFAULT_DENSITY_COMPRESSION,
            blend_factor: crate::config::DEFAULT_BLEND_FACTOR,
            use_dynamic_blend: crate::config::DEFAULT_USE_DYNAMIC_BLEND,
            target_iterations_per_pixel: crate::config::DEFAULT_TARGET_ITERATIONS_PER_PIXEL as u32,
            iterations_per_thread: crate::config::DEFAULT_ITERATIONS_PER_THREAD,
            speed_multiplier: crate::config::DEFAULT_SPEED_MULTIPLIER,
        };

        let config_manager = ConfigManager::new(initial_config.clone());

        // Get initial size before moving gpu
        let initial_viewport_size = (gpu.size.width, gpu.size.height);

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
            metrics: PerformanceMetrics::new(),
            last_frame_time: None,
            accumulation_batch_size: 4, // EXPERIMENT: Test batching
            frames_since_accumulation: 0,
            use_overwrite_next_frame: false,
            last_param_change_time: None,
            fractal_viewport_size: initial_viewport_size, // Initialize to window size
            export_width: 1920,  // Default export resolution
            export_height: 1080,
            use_custom_export_size: false,  // Default to viewport size
        };

        #[allow(deprecated)]
        event_loop.run(move |event, elwt| {
            match event {
                Event::WindowEvent { event, window_id } if window_id == window.id() => {
                    // Let egui handle events first
                    let consumed = app.egui_layer.handle_event(&event, &window);

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

                    // Use speed multiplier when actively rendering, otherwise default to 60 FPS
                    let multiplier = if is_rendering { config.speed_multiplier } else { 1 };
                    let target_fps = 60.0 * multiplier as f64;
                    let target_frame_time = Duration::from_secs_f64(1.0 / target_fps);

                    let now = Instant::now();
                    if let Some(last_frame) = app.last_frame_time {
                        let elapsed = now.duration_since(last_frame);
                        if elapsed >= target_frame_time {
                            // Time for next frame, request redraw
                            window.request_redraw();
                        } else {
                            // Wait until next frame is due
                            let wait_until = last_frame + target_frame_time;
                            elwt.set_control_flow(ControlFlow::WaitUntil(wait_until));
                        }
                    } else {
                        // First frame, render immediately
                        window.request_redraw();
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
            &mut self.current_preset_index,
            &mut self.paused,
            &mut self.quit_requested,
            can_undo,
            can_redo,
            &mut self.workspace,
            &mut self.export_width,
            &mut self.export_height,
            &mut self.use_custom_export_size,
        );
        self.metrics.record_ui_time(t_ui_start.elapsed().as_secs_f64() * 1000.0);

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
                        &self.flame, resize_config.iterations_per_thread, resize_config.zoom, resize_config.pan_x, resize_config.pan_y, resize_config.rotation,
                        resize_config.camera_rotation_x, resize_config.camera_rotation_y, resize_config.camera_z, resize_config.speed_factor);
                    self.gpu.queue.submit(std::iter::once(resize_encoder.finish()));

                    // Restore palette and color mode after buffer recreation
                    let palette = resize_config.palette.as_ref()
                        .or_else(|| self.palette_library.get(resize_config.palette_index));
                    if let Some(palette) = palette {
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, palette, resize_config.palette_rotation);
                    }
                    renderer.set_color_mode(&self.gpu.queue, resize_config.color_mode, resize_config.iterations_per_thread,
                        resize_config.zoom, resize_config.pan_x, resize_config.pan_y, resize_config.rotation,
                        resize_config.camera_rotation_x, resize_config.camera_rotation_y, resize_config.camera_z, resize_config.speed_factor);

                    // Restore tonemap parameters after buffer recreation (not in live preview mode)
                    renderer.update_tonemap(&self.gpu.queue, resize_config.tonemap_mode, resize_config.use_curve, resize_config.exposure, resize_config.gamma,
                        resize_config.gamma_threshold, resize_config.brightness, resize_config.vibrancy, resize_config.saturation, resize_config.hue_shift, resize_config.value_scale,
                        viewport_size.0, viewport_size.1, renderer.total_iterations(), resize_config.max_iterations, resize_config.zoom, resize_config.iterations_per_thread, 1, false);
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
                    match FractalConfig::load_from_file(&path) {
                        Ok(config) => {
                            if let Err(e) = self.load_config_with_undo(config, "Load Config".to_string()) {
                                eprintln!("Failed to load config: {}", e);
                            } else {
                                println!("Config loaded from: {}", path.display());
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
                                        // Multiple flames: let user choose
                                        // TODO: Add multi-flame selection dialog
                                        println!("Found {} flames, importing first one", configs.len());
                                        let config = configs.into_iter().next().unwrap();
                                        if let Err(e) = self.load_config_with_undo(config, "Import Apophysis Flame".to_string()) {
                                            eprintln!("Failed to import flame: {}", e);
                                        }
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
                } else if let Some(ref renderer) = self.flame_renderer {
                    // Viewport-size export: use current renderer
                    let total_iterations = renderer.total_iterations();
                    let pixels_future = renderer.read_fractal_pixels(&self.gpu.device, &self.gpu.queue, transparent, export_config.background_color);

                    match pollster::block_on(pixels_future) {
                        Ok((width, height, rgba_data)) => {
                            // Build metadata with captured values
                            let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                width,
                                height,
                                total_iterations,
                                render_time_ms,
                                export_config.iterations_per_thread,
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
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: Use unsafe lifetime extension
                // SAFETY: The Device, Queue, and FlameRenderer live in App which persists
                // for the entire program lifetime. The GPU resources won't be dropped
                // until the app exits, so extending their lifetime to 'static is safe.
                // read_fractal_pixels only needs immutable access since it reads from
                // the renderer's internal fractal_texture without modification.
                use wasm_bindgen_futures::spawn_local;

                // Build metadata before borrowing renderer
                let export_config = self.export_config();

                if let Some(ref renderer) = self.flame_renderer {
                    let total_iterations = renderer.total_iterations();
                    let render_time_ms = self.metrics.render_time_ms;
                    let iterations_per_thread = export_config.iterations_per_thread;
                    let speed_factor = export_config.speed_factor;
                    let background_color = export_config.background_color;

                    let device: &'static egui_wgpu::wgpu::Device = unsafe { std::mem::transmute(&self.gpu.device) };
                    let queue: &'static egui_wgpu::wgpu::Queue = unsafe { std::mem::transmute(&self.gpu.queue) };
                    let renderer: &'static crate::renderer::compute_kernel::FlameRenderer =
                        unsafe { std::mem::transmute(renderer) };

                    spawn_local(async move {
                        // Await pixel capture
                        match renderer.read_fractal_pixels(device, queue, transparent, background_color).await {
                            Ok((width, height, rgba_data)) => {
                                // Build metadata with captured dimensions
                                let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                    width,
                                    height,
                                    total_iterations,
                                    render_time_ms,
                                    iterations_per_thread,
                                    speed_factor,
                                    &export_config,
                                );

                                // Encode PNG with metadata (owned data, no borrowing)
                                match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata)) {
                                    Ok(png_data) => {
                                        // Open file dialog and save
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
                            Err(e) => log::error!("Failed to capture pixels: {}", e),
                        }
                    });
                }
            }
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
                        update_config.iterations_per_thread, update_config.zoom, update_config.pan_x, update_config.pan_y,
                        update_config.rotation, update_config.camera_rotation_x, update_config.camera_rotation_y, update_config.camera_z, update_config.speed_factor);
                }

                // Update view parameters (includes view changes and iteration changes)
                if actions.update_view || view_changed_by_keyboard {
                    renderer.set_deterministic_rng(update_config.deterministic_rng);
                    renderer.update_iterations(&self.gpu.queue, update_config.iterations_per_thread,
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
                        update_config.iterations_per_thread, update_config.zoom, update_config.pan_x,
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
                    renderer.reset(&mut update_encoder, &self.gpu.queue, update_config.iterations_per_thread,
                        update_config.zoom, update_config.pan_x, update_config.pan_y, update_config.rotation,
                        update_config.camera_rotation_x, update_config.camera_rotation_y, update_config.camera_z, update_config.speed_factor);
                    self.frames_since_accumulation = 0;
                } else if has_view_or_color_change && renderer.total_iterations() >= update_config.max_iterations {
                    // View/color changes when fractal has stopped iterating:
                    // Reset counter to restart iteration (smooth transition via overwrite mode)
                    renderer.reset_iteration_counter();
                    self.frames_since_accumulation = 0;
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
            let has_stopped = renderer.total_iterations() >= final_config.max_iterations;
            let use_overwrite = self.use_overwrite_next_frame || has_stopped;
            renderer.set_overwrite_mode(use_overwrite);

            // Check if we should continue iterating
            let max_iterations = Some(final_config.max_iterations);
            let should_iterate = !self.paused &&
                max_iterations.map_or(true, |max| renderer.total_iterations() < max);

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
                let samples_this_frame = renderer.compute_pass(&mut render_encoder, &self.gpu.queue, NUM_WORKGROUPS,
                    final_config.iterations_per_thread, final_config.zoom, final_config.pan_x, final_config.pan_y, final_config.rotation,
                    final_config.camera_rotation_x, final_config.camera_rotation_y, final_config.camera_z, final_config.speed_factor, clear_histogram);
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
            // Calculate batch_size for tonemap (same logic as accumulation)
            let batch_size_for_tonemap = if use_overwrite { 1 } else { self.accumulation_batch_size };
            // is_live_preview: Only during active editing, not when rendering stops
            let is_live_preview = self.use_overwrite_next_frame;
            renderer.update_tonemap(&self.gpu.queue, final_config.tonemap_mode, final_config.use_curve,
                final_config.exposure, final_config.gamma, final_config.gamma_threshold, final_config.brightness,
                final_config.vibrancy, final_config.saturation, final_config.hue_shift, final_config.value_scale,
                renderer.width, renderer.height, renderer.total_iterations(), final_config.max_iterations, final_config.zoom,
                final_config.iterations_per_thread, batch_size_for_tonemap, is_live_preview);

            // Render to internal fractal texture
            renderer.tonemap_pass(&mut render_encoder);
            self.metrics.record_tonemap_time(t_tonemap.elapsed().as_secs_f64() * 1000.0);
        }

        // Submit rendering commands
        let t_submit = Instant::now();
        self.gpu.queue.submit(std::iter::once(render_encoder.finish()));
        self.metrics.record_submit_time(t_submit.elapsed().as_secs_f64() * 1000.0);

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
}
