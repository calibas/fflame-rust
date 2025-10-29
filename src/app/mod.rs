//! Application state and event loop

mod input;
mod config;
mod export;

pub use export::export_headless;

use winit::{event::*, event_loop::{EventLoop, ControlFlow}, window::Window};
use wgpu::SurfaceError;

use crate::gpu::device::GpuContext;
use crate::ui::EguiLayer;
use crate::renderer::FlameRenderer;
use crate::scene::transforms::Flame;
use crate::scene::palette::{PaletteLibrary, ColorMode};
use crate::scene::presets::PresetLibrary;
use crate::util::PerformanceMetrics;
use crate::config::FractalConfig;
use crate::undo::UndoHistory;
use crate::scene::tonemap::{ToneMapMode, ToneCurve};

pub struct App {
    pub(super) gpu: GpuContext,
    pub(super) egui_layer: EguiLayer,
    pub(super) flame_renderer: Option<FlameRenderer>,
    pub(super) flame: Flame,
    pub(super) iterations_per_thread: u32,
    pub(super) zoom: f32,
    pub(super) pan_x: f32,
    pub(super) pan_y: f32,
    pub(super) rotation: f32,
    pub(super) camera_rotation_x: f32, // 3D camera pitch
    pub(super) camera_rotation_y: f32, // 3D camera yaw
    pub(super) density_scale: f32,
    pub(super) view_changed_by_keyboard: bool,
    pub(super) mouse_dragging: bool,
    pub(super) last_mouse_pos: Option<(f32, f32)>,
    pub(super) metrics: PerformanceMetrics,
    pub(super) palette_library: PaletteLibrary,
    pub(super) current_palette_index: usize,
    pub(super) preset_library: PresetLibrary,
    pub(super) current_preset_index: usize,
    pub(super) color_mode: ColorMode,
    pub(super) paused: bool,
    pub(super) max_iterations: Option<u64>,
    pub(super) speed_factor: f32,
    pub(super) undo_history: UndoHistory,
    pub(super) modifiers: winit::keyboard::ModifiersState,
    pub(super) background_color: [f32; 3],
    // Tone mapping
    pub(super) tonemap_mode: ToneMapMode,
    pub(super) tonemap_curve: ToneCurve,
    pub(super) use_curve: bool,
    pub(super) exposure: f32,
    pub(super) gamma: f32,
    // Rendering
    pub(super) deterministic_rng: bool,
    pub(super) speed_multiplier: u32,  // 1x, 2x, 4x, 8x, 16x (target FPS = 60 * multiplier)
    pub(super) last_frame_time: Option<web_time::Instant>,
    // Batched accumulation experiment
    pub(super) accumulation_batch_size: u32,  // Process every N frames (1 = normal, 4 = batched)
    pub(super) frames_since_accumulation: u32,
    pub(super) histogram_color_scale: f32,  // Precision vs overflow (default: 10.0)
    pub(super) low_density_smoothing: f32,  // 0.0 = no smoothing, 1.0 = max smoothing (default: 0.5)
    pub(super) density_compression_strength: f32,  // 0.0 = linear, 5.0 = strong compression (default: 0.0)
    pub(super) blend_factor: f32,  // Accumulation blend rate: 0.01 (slow/smooth) to 1.0 (fast/flickery), default: 0.1
    pub(super) use_dynamic_blend: bool,  // true = exponential convergence (old), false = fixed blend rate (new)
    pub(super) target_iterations_per_pixel: u32,  // Per-pixel convergence: stop updating pixel after N iterations (0 = disabled)
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

        // Create initial config for undo history
        let initial_config = FractalConfig {
            flame: flame.clone(),
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            camera_rotation_x: 0.0,
            camera_rotation_y: 0.0,
            density_scale: 1.0,
            speed_factor: 0.5,
            max_iterations: 1_000_000_000,  // Effectively unlimited
            color_mode: ColorMode::Transform,
            palette_index: 1,
            palette: Some(palette_library.get(1).unwrap().clone()),
            background_color: [0.0, 0.0, 0.0],
            tonemap_mode: ToneMapMode::Logarithmic,
            tonemap_curve: ToneCurve::linear(),
            use_curve: true,
            exposure: 1.0,
            gamma: 2.2,
            deterministic_rng: false,
            histogram_color_scale: 10.0,  // Balanced default
            low_density_smoothing: 0.5,  // Moderate smoothing default
            density_compression_strength: 0.0,  // Linear accumulation default (no compression)
            blend_factor: 0.1,  // 10% blend rate - good balance between speed and smoothness
            target_iterations_per_pixel: 0,  // Disabled by default
        };

        let mut app = Self {
            gpu,
            egui_layer,
            flame_renderer: Some(flame_renderer),
            flame,
            iterations_per_thread: 256,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            camera_rotation_x: 0.0,
            camera_rotation_y: 0.0,
            density_scale: 1.0,
            view_changed_by_keyboard: false,
            mouse_dragging: false,
            last_mouse_pos: None,
            metrics: PerformanceMetrics::new(),
            palette_library,
            current_palette_index: 1, // Start with Fire palette
            preset_library,
            current_preset_index: 0, // Start with first preset
            color_mode: ColorMode::Transform,
            paused: false,
            max_iterations: Some(1_000_000_000),
            speed_factor: 0.5,
            undo_history: UndoHistory::new(initial_config),
            modifiers: winit::keyboard::ModifiersState::default(),
            background_color: [0.0, 0.0, 0.0], // Default to black
            tonemap_mode: ToneMapMode::Logarithmic,
            tonemap_curve: ToneCurve::linear(),
            use_curve: false,  // Curves disabled by default
            exposure: 1.0,
            gamma: 2.2,
            deterministic_rng: true, // Enabled by default for reproducible rendering
            speed_multiplier: 1, // Default 1x (60 FPS)
            last_frame_time: None,
            // Batched accumulation: 1 = normal (every frame), 4 = experimental batching
            accumulation_batch_size: 4, // EXPERIMENT: Test batching
            frames_since_accumulation: 0,
            histogram_color_scale: 10.0, // Balanced default
            low_density_smoothing: 0.5, // Moderate smoothing default
            density_compression_strength: 0.0, // Linear accumulation default (no compression)
            blend_factor: 0.1, // 10% blend rate - good balance between speed and smoothness
            use_dynamic_blend: true, // Default to exponential convergence (old behavior)
            target_iterations_per_pixel: 0, // Default: disabled (no per-pixel convergence)
        };

        #[allow(deprecated)]
        event_loop.run(move |event, elwt| {
            match event {
                Event::WindowEvent { event, window_id } if window_id == window.id() => {
                    // Let egui handle events first
                    let consumed = app.egui_layer.handle_event(&event, &window);

                    match event {
                        WindowEvent::CloseRequested => elwt.exit(),
                        WindowEvent::Resized(size) => {
                            // Skip resize if dimensions are zero (happens when minimizing on Windows)
                            if size.width > 0 && size.height > 0 {
                                app.gpu.resize(size);
                                // Also resize renderer buffers to match
                                if let Some(ref mut renderer) = app.flame_renderer {
                                    let mut encoder = app.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                        label: Some("Resize Encoder"),
                                    });
                                    renderer.resize(&app.gpu.device, &mut encoder, &app.gpu.queue, size.width, size.height,
                                        &app.flame, app.iterations_per_thread, app.zoom, app.pan_x, app.pan_y, app.rotation,
                                        app.camera_rotation_x, app.camera_rotation_y, app.speed_factor);
                                    app.gpu.queue.submit(std::iter::once(encoder.finish()));
                                }
                            }
                        },
                        WindowEvent::ScaleFactorChanged { .. } => {
                            // Handle DPI/zoom changes - resize canvas to match new scale
                            let new_size = window.inner_size();
                            if new_size.width > 0 && new_size.height > 0 {
                                app.gpu.resize(new_size);
                                if let Some(ref mut renderer) = app.flame_renderer {
                                    let mut encoder = app.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                        label: Some("Scale Factor Resize Encoder"),
                                    });
                                    renderer.resize(&app.gpu.device, &mut encoder, &app.gpu.queue, new_size.width, new_size.height,
                                        &app.flame, app.iterations_per_thread, app.zoom, app.pan_x, app.pan_y, app.rotation,
                                        app.camera_rotation_x, app.camera_rotation_y, app.speed_factor);
                                    app.gpu.queue.submit(std::iter::once(encoder.finish()));
                                }
                                window.request_redraw();
                            }
                        },
                        WindowEvent::KeyboardInput { event: key_event, .. } if !consumed => {
                            // Handle keyboard input only if egui didn't consume it
                            app.handle_keyboard(&key_event);
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            // Always handle mouse releases to clear dragging state,
                            // but only handle presses if egui didn't consume them
                            app.handle_mouse_button(state, button, consumed);
                        }
                        WindowEvent::CursorMoved { position, .. } if !consumed => {
                            app.handle_mouse_move(position.x as f32, position.y as f32);
                        }
                        WindowEvent::MouseWheel { delta, .. } if !consumed => {
                            app.handle_mouse_wheel(delta);
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
                    let is_rendering = !app.paused && app.flame_renderer.as_ref().map_or(false, |r| {
                        app.max_iterations.map_or(true, |max| r.total_iterations() < max)
                    });

                    // Use speed multiplier when actively rendering, otherwise default to 60 FPS
                    let multiplier = if is_rendering { app.speed_multiplier } else { 1 };
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

        let render_start = Instant::now();
        self.last_frame_time = Some(render_start);

        let frame = self.gpu.surface.get_current_texture()?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Run flame compute shader with progressive refinement
        if let Some(ref mut renderer) = self.flame_renderer {
            // Note: tonemap parameters are updated when they actually change (via ui_response handlers)
            // No need to update every frame

            // Check if we should continue iterating
            let should_iterate = !self.paused &&
                self.max_iterations.map_or(true, |max| renderer.total_iterations() < max);

            if should_iterate {
                const NUM_WORKGROUPS: u32 = 128;

                self.frames_since_accumulation += 1;

                // Determine if we should accumulate this frame
                let should_accumulate = self.frames_since_accumulation >= self.accumulation_batch_size;

                let t0 = Instant::now();
                // 1. Compute new samples with fresh random seed
                // Clear histogram only when starting a new batch (frame 1 of batch)
                let clear_histogram = self.frames_since_accumulation == 1;
                let samples_this_frame = renderer.compute_pass(&mut encoder, &self.gpu.queue, NUM_WORKGROUPS, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.rotation, self.camera_rotation_x, self.camera_rotation_y, self.speed_factor, clear_histogram);
                self.metrics.record_compute_time(t0.elapsed().as_secs_f64() * 1000.0);

                let t1 = Instant::now();
                // 2. Accumulate samples - but only every N frames if batching enabled
                if should_accumulate {
                    // samples_this_frame is only THIS frame's samples, but histogram contains
                    // accumulated samples from all frames in the batch
                    // Pass total samples for proper blend_factor calculation
                    let total_samples_in_batch = samples_this_frame * self.accumulation_batch_size as u64;
                    renderer.accumulate_pass(&mut encoder, &self.gpu.queue, &self.gpu.device, total_samples_in_batch);
                    self.frames_since_accumulation = 0;
                    self.metrics.record_accumulate_time(t1.elapsed().as_secs_f64() * 1000.0);
                } else {
                    self.metrics.record_accumulate_time(0.0);
                }

                // 3. Adjust per-pixel scales every frame (prevents temporal aliasing / vertical lines)
                // TEST: Disable adaptive scaling to test fixed global scale
                // renderer.adjust_scale_pass(&mut encoder);
            } else {
                self.metrics.record_compute_time(0.0);
                self.metrics.record_accumulate_time(0.0);
            }

            let t2 = Instant::now();
            // 3. Update tonemap parameters and render to screen
            renderer.update_density_scale(&self.gpu.queue, self.density_scale);
            renderer.update_background_color(&self.gpu.queue, self.background_color);
            renderer.update_tonemap(&self.gpu.queue, self.tonemap_mode, self.use_curve, self.exposure, self.gamma);
            renderer.tonemap_pass(&mut encoder, &view);
            self.metrics.record_tonemap_time(t2.elapsed().as_secs_f64() * 1000.0);

            // DEBUG: Log scale statistics every 60 frames
            static mut DEBUG_FRAME_COUNT: u32 = 0;
            // Note: debug_scale_stats() removed - scale is now a uniform constant
        }

        // Render UI on top and handle updates
        let t3 = Instant::now();
        let can_undo = self.can_undo();
        let can_redo = self.can_redo();

        let ui_response = self.egui_layer.render_ui(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &view,
            window,
            self.gpu.size,
            &self.metrics,
            self.flame_renderer.as_mut(),
            &mut self.flame,
            &mut self.iterations_per_thread,
            &mut self.zoom,
            &mut self.pan_x,
            &mut self.pan_y,
            &mut self.rotation,
            &mut self.camera_rotation_x,
            &mut self.camera_rotation_y,
            &mut self.density_scale,
            &self.palette_library,
            &mut self.current_palette_index,
            &self.preset_library,
            &mut self.current_preset_index,
            &mut self.color_mode,
            &mut self.paused,
            &mut self.max_iterations,
            &mut self.speed_factor,
            can_undo,
            can_redo,
            &mut self.background_color,
            &mut self.tonemap_mode,
            &mut self.tonemap_curve,
            &mut self.use_curve,
            &mut self.exposure,
            &mut self.gamma,
            &mut self.deterministic_rng,
            &mut self.speed_multiplier,
            &mut self.histogram_color_scale,
            &mut self.low_density_smoothing,
            &mut self.density_compression_strength,
            &mut self.blend_factor,
            &mut self.use_dynamic_blend,
            &mut self.target_iterations_per_pixel,
        );
        self.metrics.record_ui_time(t3.elapsed().as_secs_f64() * 1000.0);

        let t4 = Instant::now();
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        self.metrics.record_submit_time(t4.elapsed().as_secs_f64() * 1000.0);

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
                    self.capture_state();
                    self.import_config(config);
                }
                Err(e) => {
                    eprintln!("Failed to import config: {}", e);
                }
            }
        }

        // Handle add transform
        if ui_response.add_transform {
            self.capture_state();

            // Create a new default transform
            let new_transform = crate::scene::transforms::Transform {
                a: 0.5,
                b: 0.0,
                c: 0.0,
                d: 0.5,
                e: 0.0,
                f: 0.0,
                g: 0.0, // Z offset
                weight: 1.0,
                variations: {
                    let mut v = std::collections::HashMap::new();
                    v.insert("linear".to_string(), 0.5);
                    v
                },
                variation_params: std::collections::HashMap::new(),
                color: [0.5, 0.5, 0.5],
                color_speed: 0.5,
            };

            self.flame.transforms.push(new_transform);
        }

        // Handle delete transform
        if let Some(idx) = ui_response.delete_transform {
            if self.flame.transforms.len() > 1 && idx < self.flame.transforms.len() {
                self.capture_state();
                self.flame.transforms.remove(idx);
            }
        }

        // Handle custom palette from editor
        if let Some(custom_pal) = ui_response.custom_palette {
            // Capture state before applying palette change (for undo)
            self.capture_state();

            // Check if this palette already exists in library by name
            let palette_lib = &mut self.palette_library;
            let mut found_index = None;
            for (i, lib_palette) in palette_lib.iter().enumerate() {
                if lib_palette.name == custom_pal.name {
                    found_index = Some(i);
                    break;
                }
            }

            if let Some(idx) = found_index {
                // Palette exists, update it in place
                palette_lib.update(idx, custom_pal);
                self.current_palette_index = idx;
            } else {
                // New palette, add to library
                palette_lib.add(custom_pal);
                self.current_palette_index = palette_lib.palettes().len() - 1;
            }

            // Update renderer
            if let Some(ref mut renderer) = self.flame_renderer {
                if let Some(palette) = palette_lib.get(self.current_palette_index) {
                    renderer.update_palette(&self.gpu.device, &self.gpu.queue, palette);
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
                            self.capture_state();
                            self.import_config(config);
                            println!("Config loaded from: {}", path.display());
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
                                        self.capture_state();
                                        self.import_config(configs.into_iter().next().unwrap());
                                        println!("Imported Apophysis flame from: {}", path.display());
                                    } else {
                                        // Multiple flames: let user choose
                                        // TODO: Add multi-flame selection dialog
                                        println!("Found {} flames, importing first one", configs.len());
                                        self.capture_state();
                                        self.import_config(configs.into_iter().next().unwrap());
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
                                    if let Ok(json) = serde_json::to_string_pretty(&configs[0]) {
                                        ctx.copy_text(json);
                                        log::info!("Apophysis flame converted to JSON - paste to import");
                                    }
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
                Ok(palette) => {
                    // Update palette editor
                    self.egui_layer.update_palette_editor(palette.clone());

                    // Add to library
                    self.palette_library.add(palette.clone());
                    // Set to the newly added palette (last in list)
                    self.current_palette_index = self.palette_library.palettes().len() - 1;

                    // Update renderer
                    if let Some(ref mut renderer) = self.flame_renderer {
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &palette);
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

                                    // Add to library
                                    self.palette_library.add(palette.clone());
                                    // Set to the newly added palette (last in list)
                                    self.current_palette_index = self.palette_library.palettes().len() - 1;

                                    // Update renderer
                                    if let Some(ref mut renderer) = self.flame_renderer {
                                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &palette);
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

        // Handle PNG export
        if ui_response.png_export_with_background || ui_response.png_export_transparent {
            let transparent = ui_response.png_export_transparent;

            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: use blocking task for both capture and save
                // Build metadata before borrowing renderer
                let config = self.export_config();
                let render_time_ms = self.metrics.render_time_ms;

                if let Some(ref mut renderer) = self.flame_renderer {
                    let total_iterations = renderer.total_iterations();
                    let pixels_future = renderer.capture_pixels(&self.gpu.device, &self.gpu.queue, transparent, self.gpu.config.format);

                    match pollster::block_on(pixels_future) {
                        Ok((width, height, rgba_data)) => {
                            // Build metadata with captured values
                            let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                width,
                                height,
                                total_iterations,
                                render_time_ms,
                                self.iterations_per_thread,
                                self.speed_factor,
                                &config,
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
                // We need mutable access for capture_pixels which may temporarily modify
                // background_color, but this is safe since the async task completes before
                // any other code runs (single-threaded WASM environment).
                use wasm_bindgen_futures::spawn_local;

                if let Some(ref mut renderer) = self.flame_renderer {
                    // Build metadata before moving into async closure
                    let config = self.export_config();
                    let total_iterations = renderer.total_iterations();
                    let render_time_ms = self.metrics.render_time_ms;
                    let iterations_per_thread = self.iterations_per_thread;
                    let speed_factor = self.speed_factor;

                    let device: &'static wgpu::Device = unsafe { std::mem::transmute(&self.gpu.device) };
                    let queue: &'static wgpu::Queue = unsafe { std::mem::transmute(&self.gpu.queue) };
                    let renderer: &'static mut crate::renderer::compute_kernel::FlameRenderer =
                        unsafe { std::mem::transmute(renderer) };
                    let format = self.gpu.config.format;

                    spawn_local(async move {
                        // Await pixel capture
                        match renderer.capture_pixels(device, queue, transparent, format).await {
                            Ok((width, height, rgba_data)) => {
                                // Build metadata with captured dimensions
                                let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                    width,
                                    height,
                                    total_iterations,
                                    render_time_ms,
                                    iterations_per_thread,
                                    speed_factor,
                                    &config,
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
        let view_changed = ui_response.view_changed || self.view_changed_by_keyboard || ui_response.camera_rotation_changed;
        let needs_update = ui_response.reset_requested || ui_response.flame_changed || ui_response.iterations_changed
            || view_changed || ui_response.palette_changed || ui_response.color_mode_changed || ui_response.pause_changed
            || ui_response.triangle_drag_ended || ui_response.tonemap_curve_changed || ui_response.histogram_color_scale_changed
            || ui_response.low_density_smoothing_changed || ui_response.density_compression_changed || ui_response.blend_factor_changed
            || ui_response.use_dynamic_blend_changed || ui_response.target_iterations_changed;

        // Note: density_changed and background_color_changed don't need encoder updates,
        // they're handled every frame before tonemap pass

        // Handle preset change BEFORE other updates (requires mutable self)
        let preset_loaded = if ui_response.preset_changed {
            if let Some(preset) = self.preset_library.get(self.current_preset_index).cloned() {
                println!("Loading preset: {} (index {})", preset.flame.name, self.current_preset_index);
                println!("  Transforms: {}", preset.flame.transforms.len());
                self.import_config(preset);
                // import_config calls capture_state internally and resets accumulation
                // Skip normal update logic since import_config handled everything
                true
            } else {
                false
            }
        } else {
            false
        };

        // Only do normal updates if we didn't load a preset
        if !preset_loaded {
            // Capture state before applying meaningful changes
            // Only capture on drag START, not during continuous dragging
            // Note: palette_changed removed - undo is captured when Apply is clicked (see custom_palette handler)
            // Note: exposure_changed uses lazy undo to throttle captures during drag
            let should_capture = ui_response.triangle_drag_started || view_changed
                || ui_response.color_mode_changed || ui_response.density_changed || ui_response.background_color_changed
                || ui_response.tonemap_mode_changed || ui_response.tonemap_curve_changed
                || ui_response.exposure_changed || ui_response.gamma_changed
                || (ui_response.flame_changed && !ui_response.triangle_drag_started); // Other flame changes (not dragging)
            if should_capture {
                self.capture_state();
            }
        }

        if needs_update && !preset_loaded {
            if let Some(ref mut renderer) = self.flame_renderer {
                let mut update_encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Update Encoder"),
                });

                if ui_response.flame_changed {
                    renderer.update_flame(&self.gpu.device, &self.gpu.queue, &self.flame, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.rotation, self.camera_rotation_x, self.camera_rotation_y, self.speed_factor);
                }

                if ui_response.iterations_changed || view_changed {
                    renderer.set_deterministic_rng(self.deterministic_rng);
                    renderer.update_iterations(&self.gpu.queue, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.rotation, self.camera_rotation_x, self.camera_rotation_y, self.speed_factor);
                }

                if ui_response.histogram_color_scale_changed {
                    // Update the renderer's histogram color scale and GPU params
                    renderer.set_histogram_color_scale(&self.gpu.queue, self.histogram_color_scale, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.rotation, self.camera_rotation_x, self.camera_rotation_y, self.speed_factor);
                    // Reset will be triggered below in should_reset
                }

                if ui_response.low_density_smoothing_changed {
                    // Update the renderer's low-density smoothing parameter
                    renderer.set_low_density_smoothing(self.low_density_smoothing);
                    // No reset needed - smoothing is applied during accumulation
                }

                if ui_response.density_compression_changed {
                    // Update the renderer's density compression strength
                    renderer.set_density_compression_strength(self.density_compression_strength);
                    // No reset needed - compression is applied during accumulation
                }

                if ui_response.blend_factor_changed {
                    // Update the renderer's blend factor
                    renderer.set_blend_factor(self.blend_factor);
                    // No reset needed - blend factor is applied during accumulation
                }

                if ui_response.use_dynamic_blend_changed {
                    // Update the renderer's dynamic blend setting
                    renderer.set_use_dynamic_blend(self.use_dynamic_blend);
                    // Reset needed to see effect immediately
                }

                if ui_response.target_iterations_changed {
                    // Update the renderer's per-pixel iteration limit
                    renderer.set_target_iterations_per_pixel(self.target_iterations_per_pixel);
                }

                // Note: density_scale and background_color are updated every frame before tonemap pass
                // so we don't need to update them here

                if ui_response.color_mode_changed {
                    renderer.set_color_mode(&self.gpu.queue, self.color_mode, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.rotation, self.camera_rotation_x, self.camera_rotation_y, self.speed_factor);

                    // When switching to Palette or Speed mode, ensure the correct palette is loaded
                    if matches!(self.color_mode, ColorMode::Palette | ColorMode::Speed) {
                        if let Some(palette) = self.palette_library.get(self.current_palette_index) {
                            renderer.update_palette(&self.gpu.device, &self.gpu.queue, palette);
                        }
                    }
                }

                if ui_response.palette_changed {
                    if let Some(palette) = self.palette_library.get(self.current_palette_index) {
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, palette);
                    }
                }

                // Update tone curve LUT if curve changed
                if ui_response.tonemap_curve_changed {
                    renderer.update_curve_lut(&self.gpu.queue, &self.tonemap_curve);
                }

                // Reset accumulation when view changes, palette changes, color mode changes, background color changes, flame changes, or user requests it
                // Note: preset_changed is handled separately above with import_config() which also resets
                // For triangle dragging: reset on first frame (triangle_drag_started) and when drag ends (triangle_drag_ended), but not during continuous drag
                // Tone mapping: reset on mode change (log vs linear affects accumulation), but not curve/exposure/gamma (post-processing only)
                let should_reset = ui_response.reset_requested || view_changed || ui_response.palette_changed || ui_response.color_mode_changed
                    || ui_response.background_color_changed || ui_response.tonemap_mode_changed || ui_response.triangle_drag_started || ui_response.triangle_drag_ended
                    || ui_response.histogram_color_scale_changed  // New scale incompatible with old samples
                    || ui_response.low_density_smoothing_changed  // New smoothing needs fresh samples to see effect
                    || ui_response.density_compression_changed  // New compression needs fresh samples to see effect
                    || ui_response.blend_factor_changed  // New blend rate needs fresh start to see effect
                    || ui_response.use_dynamic_blend_changed  // Switching blend modes needs fresh start
                    || ui_response.target_iterations_changed  // New iteration limit needs fresh iteration counts
                    || (ui_response.flame_changed && !ui_response.triangle_dragging);
                if should_reset {
                    renderer.reset(&mut update_encoder, &self.gpu.queue, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.rotation, self.camera_rotation_x, self.camera_rotation_y, self.speed_factor);
                    if ui_response.histogram_color_scale_changed {
                        self.frames_since_accumulation = 0;  // Reset batch counter
                    }
                }

                self.gpu.queue.submit(std::iter::once(update_encoder.finish()));
            }
        }

        // Clear keyboard flag for next frame
        self.view_changed_by_keyboard = false;

        let t5 = Instant::now();
        frame.present();
        self.metrics.record_present_time(t5.elapsed().as_secs_f64() * 1000.0);

        self.metrics.record_render_time(render_start.elapsed().as_secs_f64() * 1000.0);

        Ok(())
    }
}
