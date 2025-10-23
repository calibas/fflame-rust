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

pub struct App {
    gpu: GpuContext,
    egui_layer: EguiLayer,
    flame_renderer: Option<FlameRenderer>,
    flame: Flame,
    iterations_per_thread: u32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    rotation: f32,
    camera_rotation_x: f32, // 3D camera pitch
    camera_rotation_y: f32, // 3D camera yaw
    density_scale: f32,
    view_changed_by_keyboard: bool,
    mouse_dragging: bool,
    last_mouse_pos: Option<(f32, f32)>,
    metrics: PerformanceMetrics,
    palette_library: PaletteLibrary,
    current_palette_index: usize,
    preset_library: PresetLibrary,
    current_preset_index: usize,
    color_mode: ColorMode,
    paused: bool,
    max_iterations: Option<u64>,
    speed_factor: f32,
    undo_history: UndoHistory,
    modifiers: winit::keyboard::ModifiersState,
    background_color: [f32; 3],
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
            color_mode: ColorMode::Transform,
            palette_index: 1,
            background_color: [0.0, 0.0, 0.0],
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
                    // Use Wait instead of Poll to let the browser control frame timing
                    elwt.set_control_flow(ControlFlow::Wait);
                    window.request_redraw();
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

    fn handle_keyboard(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{KeyCode, PhysicalKey};

        // Only handle key press (not release)
        if !event.state.is_pressed() {
            return;
        }

        // Check for Ctrl/Cmd modifier
        let ctrl_or_cmd = {
            #[cfg(target_os = "macos")]
            { self.modifiers.super_key() }
            #[cfg(not(target_os = "macos"))]
            { self.modifiers.control_key() }
        };

        // Handle undo/redo with logical key
        use winit::keyboard::Key;
        if ctrl_or_cmd {
            if let Key::Character(ref c) = event.logical_key {
                let c_lower = c.to_lowercase().to_string();
                if c_lower == "z" {
                    self.undo();
                    return;
                } else if c_lower == "y" {
                    self.redo();
                    return;
                }
            }
        }

        let pan_step = 0.1 / self.zoom;

        match event.physical_key {
            PhysicalKey::Code(KeyCode::ArrowUp) => {
                self.pan_y -= pan_step;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowDown) => {
                self.pan_y += pan_step;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                self.pan_x -= pan_step;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowRight) => {
                self.pan_x += pan_step;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::Equal) | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                self.zoom *= 1.5;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::Minus) | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                self.zoom /= 1.5;
                self.view_changed_by_keyboard = true;
            }
            _ => {}
        }
    }

    fn handle_mouse_button(&mut self, state: winit::event::ElementState, button: winit::event::MouseButton, consumed: bool) {
        use winit::event::{ElementState, MouseButton};

        if button == MouseButton::Left {
            match state {
                ElementState::Pressed => {
                    // Only start dragging if egui didn't consume the event
                    if !consumed {
                        self.mouse_dragging = true;
                    }
                }
                ElementState::Released => {
                    // Always clear dragging state on release, even if egui consumed it
                    self.mouse_dragging = false;
                    self.last_mouse_pos = None;
                }
            }
        }
    }

    fn handle_mouse_move(&mut self, x: f32, y: f32) {
        if self.mouse_dragging {
            if let Some((last_x, last_y)) = self.last_mouse_pos {
                // Calculate delta in screen pixels
                let dx = x - last_x;
                let dy = y - last_y;

                // Convert to fractal space - scale inversely with zoom and window size
                let scale = f32::min(self.gpu.size.width as f32, self.gpu.size.height as f32) * 0.25;
                let pan_dx = -dx / (scale * self.zoom);
                let pan_dy = -dy / (scale * self.zoom); // Negative to match drag direction

                self.pan_x += pan_dx;
                self.pan_y += pan_dy;
                self.view_changed_by_keyboard = true; // Reuse same flag for mouse
            }
        }
        // Always update mouse position for zoom-to-cursor
        self.last_mouse_pos = Some((x, y));
    }

    fn handle_mouse_wheel(&mut self, delta: winit::event::MouseScrollDelta) {
        use winit::event::MouseScrollDelta;

        let zoom_factor = match delta {
            MouseScrollDelta::LineDelta(_x, y) => {
                // Each line is typically one "click" of the wheel
                // Positive y = scroll up = zoom in, negative y = scroll down = zoom out
                if y > 0.0 {
                    1.1f32.powf(y)
                } else if y < 0.0 {
                    1.1f32.powf(y)
                } else {
                    1.0
                }
            }
            MouseScrollDelta::PixelDelta(pos) => {
                // Pixel delta for touchpad scrolling
                // Positive y = scroll up = zoom in
                let y = pos.y as f32;
                if y.abs() > 0.1 {
                    1.1f32.powf(y * 0.01)
                } else {
                    1.0
                }
            }
        };

        if zoom_factor != 1.0 {
            // Zoom in toward cursor, zoom out from center
            if zoom_factor > 1.0 {
                // Zooming in - zoom toward mouse cursor position
                if let Some((mouse_x, mouse_y)) = self.last_mouse_pos {
                    // Convert mouse position from screen space to fractal space
                    // Screen center
                    let center_x = self.gpu.size.width as f32 / 2.0;
                    let center_y = self.gpu.size.height as f32 / 2.0;

                    // Mouse offset from center in screen pixels
                    let mouse_offset_x = mouse_x - center_x;
                    let mouse_offset_y = mouse_y - center_y;

                    // Convert to fractal space (account for current zoom and scale)
                    let scale = f32::min(self.gpu.size.width as f32, self.gpu.size.height as f32) * 0.25;
                    let fractal_offset_x = mouse_offset_x / (scale * self.zoom);
                    let fractal_offset_y = mouse_offset_y / (scale * self.zoom);

                    // Calculate the point in fractal space that the mouse is pointing at
                    let point_x = self.pan_x + fractal_offset_x;
                    let point_y = self.pan_y + fractal_offset_y;

                    // Apply zoom
                    self.zoom *= zoom_factor;

                    // Adjust pan so the point under the mouse stays in the same place
                    // After zoom, the fractal coordinates change, so we need to compensate
                    let new_fractal_offset_x = mouse_offset_x / (scale * self.zoom);
                    let new_fractal_offset_y = mouse_offset_y / (scale * self.zoom);

                    self.pan_x = point_x - new_fractal_offset_x;
                    self.pan_y = point_y - new_fractal_offset_y;
                } else {
                    // No mouse position, zoom to center
                    self.zoom *= zoom_factor;
                }
            } else {
                // Zooming out - always zoom from center
                self.zoom *= zoom_factor;
            }

            self.view_changed_by_keyboard = true; // Reuse same flag
        }
    }

    fn render(&mut self, window: &Window) -> Result<(), SurfaceError> {
        use web_time::Instant;

        let render_start = Instant::now();

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
                let t0 = Instant::now();
                // 1. Compute new samples with fresh random seed
                let samples_this_frame = renderer.compute_pass(&mut encoder, &self.gpu.queue, 128, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.rotation, self.camera_rotation_x, self.camera_rotation_y, self.speed_factor);
                self.metrics.record_compute_time(t0.elapsed().as_secs_f64() * 1000.0);

                let t1 = Instant::now();
                // 2. Accumulate samples (blend with previous frames)
                renderer.accumulate_pass(&mut encoder, &self.gpu.queue, &self.gpu.device, samples_this_frame);
                self.metrics.record_accumulate_time(t1.elapsed().as_secs_f64() * 1000.0);
            } else {
                self.metrics.record_compute_time(0.0);
                self.metrics.record_accumulate_time(0.0);
            }

            let t2 = Instant::now();
            // 3. Update tonemap parameters and render to screen
            renderer.update_density_scale(&self.gpu.queue, self.density_scale);
            renderer.update_background_color(&self.gpu.queue, self.background_color);
            renderer.tonemap_pass(&mut encoder, &view);
            self.metrics.record_tonemap_time(t2.elapsed().as_secs_f64() * 1000.0);
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
                    .add_filter("Fractal Flame", &["flame"])
                    .set_file_name("fractal.flame")
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
                            .add_filter("Fractal Flame", &["flame"])
                            .set_file_name("fractal.flame")
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
            // Add to library if not already there
            let palette_lib = &mut self.palette_library;
            palette_lib.add(custom_pal);
            // Set to the newly added palette (last in list)
            self.current_palette_index = palette_lib.palettes().len() - 1;

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
                    .add_filter("Fractal Flame", &["flame"])
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
                if let Some(ref renderer) = self.flame_renderer {
                    let pixels_future = renderer.capture_pixels(&self.gpu.device, &self.gpu.queue, transparent, self.gpu.config.format);

                    match pollster::block_on(pixels_future) {
                        Ok((width, height, rgba_data)) => {
                            // Encode PNG
                            match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data) {
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
                use wasm_bindgen_futures::spawn_local;

                if let Some(ref renderer) = self.flame_renderer {
                    let device: &'static wgpu::Device = unsafe { std::mem::transmute(&self.gpu.device) };
                    let queue: &'static wgpu::Queue = unsafe { std::mem::transmute(&self.gpu.queue) };
                    let renderer: &'static crate::renderer::compute_kernel::FlameRenderer =
                        unsafe { std::mem::transmute(renderer) };
                    let format = self.gpu.config.format;

                    spawn_local(async move {
                        // Await pixel capture
                        match renderer.capture_pixels(device, queue, transparent, format).await {
                            Ok((width, height, rgba_data)) => {
                                // Encode PNG (owned data, no borrowing)
                                match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data) {
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
            || ui_response.triangle_drag_ended;

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
            let should_capture = ui_response.triangle_drag_started || view_changed || ui_response.palette_changed
                || ui_response.color_mode_changed || ui_response.density_changed || ui_response.background_color_changed
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
                    renderer.update_iterations(&self.gpu.queue, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.rotation, self.camera_rotation_x, self.camera_rotation_y, self.speed_factor);
                }

                // Note: density_scale and background_color are updated every frame before tonemap pass
                // so we don't need to update them here

                if ui_response.color_mode_changed {
                    renderer.set_color_mode(&self.gpu.queue, self.color_mode, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.rotation, self.camera_rotation_x, self.camera_rotation_y, self.speed_factor);
                }

                if ui_response.palette_changed {
                    if let Some(palette) = self.palette_library.get(self.current_palette_index) {
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, palette);
                    }
                }

                // Reset accumulation when view changes, palette changes, color mode changes, background color changes, flame changes, or user requests it
                // Note: preset_changed is handled separately above with import_config() which also resets
                // For triangle dragging: reset on first frame (triangle_drag_started) and when drag ends (triangle_drag_ended), but not during continuous drag
                let should_reset = ui_response.reset_requested || view_changed || ui_response.palette_changed || ui_response.color_mode_changed
                    || ui_response.background_color_changed || ui_response.triangle_drag_started || ui_response.triangle_drag_ended
                    || (ui_response.flame_changed && !ui_response.triangle_dragging);
                if should_reset {
                    renderer.reset(&mut update_encoder, &self.gpu.queue, self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y, self.rotation, self.camera_rotation_x, self.camera_rotation_y, self.speed_factor);
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

    /// Export current configuration to FractalConfig
    pub fn export_config(&self) -> FractalConfig {
        FractalConfig {
            flame: self.flame.clone(),
            zoom: self.zoom,
            pan_x: self.pan_x,
            pan_y: self.pan_y,
            rotation: self.rotation,
            camera_rotation_x: self.camera_rotation_x,
            camera_rotation_y: self.camera_rotation_y,
            density_scale: self.density_scale,
            speed_factor: self.speed_factor,
            color_mode: self.color_mode,
            palette_index: self.current_palette_index,
            background_color: self.background_color,
        }
    }

    /// Import configuration from FractalConfig
    pub fn import_config(&mut self, config: FractalConfig) {
        // Update app-level state
        self.flame = config.flame.clone();
        self.zoom = config.zoom;
        self.pan_x = config.pan_x;
        self.pan_y = config.pan_y;
        self.rotation = config.rotation;
        self.camera_rotation_x = config.camera_rotation_x;
        self.camera_rotation_y = config.camera_rotation_y;
        self.density_scale = config.density_scale;
        self.speed_factor = config.speed_factor;
        self.color_mode = config.color_mode;
        self.current_palette_index = config.palette_index;
        self.background_color = config.background_color;

        // Use the comprehensive load_config function to ensure all GPU state is synchronized
        if let Some(ref mut renderer) = self.flame_renderer {
            let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Config Import Encoder"),
            });

            if let Some(palette) = self.palette_library.get(self.current_palette_index) {
                renderer.load_config(&self.gpu.device, &mut encoder, &self.gpu.queue, &config, palette, self.iterations_per_thread);
            }

            self.gpu.queue.submit(std::iter::once(encoder.finish()));
        }
    }

    /// Capture current state to undo history before making a change
    fn capture_state(&mut self) {
        let config = self.export_config();
        self.undo_history.push(config);
    }

    /// Undo to previous state
    pub fn undo(&mut self) {
        let config = self.undo_history.undo().cloned();
        if let Some(config) = config {
            self.import_config(config);
        }
    }

    /// Redo to next state
    pub fn redo(&mut self) {
        let config = self.undo_history.redo().cloned();
        if let Some(config) = config {
            self.import_config(config);
        }
    }

    pub fn can_undo(&self) -> bool {
        self.undo_history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo_history.can_redo()
    }
}