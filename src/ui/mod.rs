use egui_wgpu::Renderer as EguiRenderer;
use egui_winit::State as EguiWinitState;
use wgpu::*;
use winit::{event::WindowEvent, window::Window};

/// Format large iteration counts with appropriate suffixes (K, M, B, T)
fn format_iterations(n: u64) -> String {
    if n >= 1_000_000_000_000 {
        format!("{:.2}T", n as f64 / 1_000_000_000_000.0)
    } else if n >= 1_000_000_000 {
        format!("{:.2}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.2}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub struct UiResponse {
    pub reset_requested: bool,
    pub flame_changed: bool,
    pub iterations_changed: bool,
    pub view_changed: bool,
    pub density_changed: bool,
    pub palette_changed: bool,
    pub color_mode_changed: bool,
    pub pause_changed: bool,
    pub config_export_requested: Option<String>,
    pub config_import_requested: Option<String>,
    pub config_save_file_requested: bool,
    pub config_load_file_requested: bool,
    pub custom_palette: Option<crate::scene::palette::Palette>,
    pub undo_requested: bool,
    pub redo_requested: bool,
    pub background_color_changed: bool,
    pub png_export_with_background: bool,
    pub png_export_transparent: bool,
    pub palette_export_json: Option<crate::scene::palette::Palette>,
    pub palette_save_file: Option<crate::scene::palette::Palette>,
    pub palette_import_json: Option<String>,
    pub palette_load_file: bool,
    pub palette_imported: Option<crate::scene::palette::Palette>,
    pub preset_changed: bool,
    pub add_transform: bool,
    pub delete_transform: Option<usize>, // Index of transform to delete
    pub render_mode_changed: bool,
    pub projection_changed: bool,
    pub camera_rotation_changed: bool,
}

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

struct PaletteEditor {
    current_palette: crate::scene::palette::Palette,
    selected_stop_index: Option<usize>,
    json_buffer: String,
}

impl EguiLayer {
    pub fn new(window: &Window, device: &Device, format: TextureFormat) -> Self {
        let ctx = egui::Context::default();
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
            palette_editor: PaletteEditor {
                current_palette: crate::scene::palette::Palette::fire(),
                selected_stop_index: None,
                json_buffer: String::new(),
            },
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
            // ==================================================
            // PERFORMANCE WINDOW - Stats + Version Info Only
            // ==================================================
            egui::Window::new("Performance")
                .open(&mut self.show_performance)
                .show(ctx, |ui| {
                    // Version info
                    let version_info = crate::version::get_version_info();
                    ui.heading("Fractal Flame Renderer");
                    ui.label(format!("Version: {}", version_info.full_version()));
                    ui.label(format!("Build: {} ({}) {}",
                        version_info.git_hash,
                        version_info.git_branch,
                        version_info.profile
                    ));

                    ui.separator();

                    // Frame statistics
                    ui.label(format!("FPS: {:.1}", metrics.fps()));
                    ui.label(format!("Frame Time: {:.2} ms", metrics.frame_time_ms()));

                    let (min, max) = metrics.frame_time_range();
                    ui.label(format!("Frame Time Range: {:.2} - {:.2} ms", min, max));

                    ui.separator();

                    // Component timings (collapsible)
                    egui::CollapsingHeader::new("Component Timings")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.label(format!("  Compute: {:.2} ms", metrics.compute_time_ms));
                            ui.label(format!("  Accumulate: {:.2} ms", metrics.accumulate_time_ms));
                            ui.label(format!("  Tonemap: {:.2} ms", metrics.tonemap_time_ms));
                            ui.label(format!("  UI: {:.2} ms", metrics.ui_time_ms));
                            ui.label(format!("  Submit: {:.2} ms", metrics.submit_time_ms));
                            ui.label(format!("  Present: {:.2} ms", metrics.present_time_ms));

                            let total_measured = metrics.compute_time_ms + metrics.accumulate_time_ms +
                                metrics.tonemap_time_ms + metrics.ui_time_ms + metrics.submit_time_ms + metrics.present_time_ms;
                            ui.label(format!("  Total Measured: {:.2} ms", total_measured));
                            ui.label(format!("  Render Function: {:.2} ms", metrics.render_time_ms));

                            let overhead = metrics.frame_time_ms() - metrics.render_time_ms;
                            ui.label(format!("  Overhead (event loop): {:.2} ms", overhead));
                        });

                    ui.separator();

                    // Frame and iteration counts
                    ui.label(format!("Total Frames: {}", metrics.frame_count()));
                    ui.label(format!("Resolution: {}x{}", window_size.width, window_size.height));

                    if let Some(renderer) = &flame_renderer {
                        ui.label(format!("Frames Accumulated: {}", renderer.samples_accumulated()));
                        ui.label(format!("Total Iterations: {}", format_iterations(renderer.total_iterations())));
                    }
                });

            // ==================================================
            // SETTINGS WINDOW - All Controls with Collapsible Sections
            // ==================================================
            egui::Window::new("Settings")
                .open(&mut self.show_settings)
                .show(ctx, |ui| {
                    // Section 1: File & Project
                    egui::CollapsingHeader::new("File & Project")
                        .default_open(true)
                        .show(ui, |ui| {
                            // Config import/export button
                            if ui.button("⚙ Import/Export Config").clicked() {
                                self.show_config_window = !self.show_config_window;
                            }

                            // Undo/Redo buttons
                            ui.horizontal(|ui| {
                                ui.add_enabled_ui(can_undo, |ui| {
                                    if ui.button("⮪ Undo (Ctrl+Z)").clicked() {
                                        undo_requested = true;
                                    }
                                });
                                ui.add_enabled_ui(can_redo, |ui| {
                                    if ui.button("⮬ Redo (Ctrl+Y)").clicked() {
                                        redo_requested = true;
                                    }
                                });
                            });

                            ui.separator();

                            // PNG export buttons
                            ui.label("Export Image");
                            ui.horizontal(|ui| {
                                if ui.button("💾 PNG (with BG)").clicked() {
                                    png_export_with_background = true;
                                }
                                if ui.button("💾 PNG (transparent)").clicked() {
                                    png_export_transparent = true;
                                }
                            });
                        });

                    // Section 2: Preset & Rendering
                    egui::CollapsingHeader::new("Preset & Rendering")
                        .default_open(true)
                        .show(ui, |ui| {
                            // Preset selector
                            let presets = preset_library.presets();
                            let current_preset_name = presets.get(*current_preset_index)
                                .map(|p| p.flame.name.as_str())
                                .unwrap_or("Unknown");

                egui::ComboBox::from_label("Preset")
                    .selected_text(current_preset_name)
                    .show_ui(ui, |ui| {
                        for (idx, preset) in presets.iter().enumerate() {
                            if ui.selectable_value(current_preset_index, idx, &preset.flame.name).changed() {
                                println!("UI: Preset changed to {} ({})", preset.flame.name, idx);
                                preset_changed = true;
                            }
                        }
                    });

                ui.separator();

                // 3D Rendering Controls
                ui.label("Render Mode");
                ui.horizontal(|ui| {
                    let was_2d = matches!(flame.render_mode, crate::scene::transforms::RenderMode::TwoD);
                    if ui.selectable_label(was_2d, "2D").clicked() {
                        flame.render_mode = crate::scene::transforms::RenderMode::TwoD;
                        render_mode_changed = true;
                        flame_changed = true;
                    }
                    if ui.selectable_label(!was_2d, "3D").clicked() {
                        flame.render_mode = crate::scene::transforms::RenderMode::ThreeD;
                        render_mode_changed = true;
                        flame_changed = true;
                    }
                });

                // Show projection controls only in 3D mode
                if matches!(flame.render_mode, crate::scene::transforms::RenderMode::ThreeD) {
                    ui.label("Projection");
                    ui.horizontal(|ui| {
                        let is_ortho = matches!(flame.projection, crate::scene::transforms::ProjectionType::Orthographic);
                        if ui.selectable_label(is_ortho, "Orthographic").clicked() {
                            flame.projection = crate::scene::transforms::ProjectionType::Orthographic;
                            projection_changed = true;
                            flame_changed = true;
                        }
                        if ui.selectable_label(!is_ortho, "Perspective").clicked() {
                            flame.projection = crate::scene::transforms::ProjectionType::Perspective { strength: 2.0 };
                            projection_changed = true;
                            flame_changed = true;
                        }
                    });

                    // Perspective strength slider
                    if let crate::scene::transforms::ProjectionType::Perspective { strength } = &mut flame.projection {
                        if ui.add(egui::Slider::new(strength, 0.5..=10.0).text("Perspective Strength")).changed() {
                            projection_changed = true;
                            flame_changed = true;
                        }
                    }
                }

                ui.separator();

                ui.label(format!("FPS: {:.1}", metrics.fps()));
                ui.label(format!("Frame Time: {:.2} ms", metrics.frame_time_ms()));

                let (min, max) = metrics.frame_time_range();
                ui.label(format!("Frame Time Range: {:.2} - {:.2} ms", min, max));

                ui.separator();
                ui.label("Component Timings:");
                ui.label(format!("  Compute: {:.2} ms", metrics.compute_time_ms));
                ui.label(format!("  Accumulate: {:.2} ms", metrics.accumulate_time_ms));
                ui.label(format!("  Tonemap: {:.2} ms", metrics.tonemap_time_ms));
                ui.label(format!("  UI: {:.2} ms", metrics.ui_time_ms));
                ui.label(format!("  Submit: {:.2} ms", metrics.submit_time_ms));
                ui.label(format!("  Present: {:.2} ms", metrics.present_time_ms));

                let total_measured = metrics.compute_time_ms + metrics.accumulate_time_ms +
                metrics.tonemap_time_ms + metrics.ui_time_ms + metrics.submit_time_ms + metrics.present_time_ms;
                ui.label(format!("  Total Measured: {:.2} ms", total_measured));
                ui.label(format!("  Render Function: {:.2} ms", metrics.render_time_ms));

                let overhead = metrics.frame_time_ms() - metrics.render_time_ms;
                ui.label(format!("  Overhead (event loop): {:.2} ms", overhead));

                ui.separator();
                ui.label(format!("Total Frames: {}", metrics.frame_count()));
                ui.label(format!("Resolution: {}x{}", window_size.width, window_size.height));


                // Config import/export button
                ui.separator();
                if ui.button("⚙ Import/Export Config").clicked() {
                    self.show_config_window = !self.show_config_window;
                }

                // Undo/Redo buttons
                ui.separator();
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(can_undo, |ui| {
                        if ui.button("⮪ Undo (Ctrl+Z)").clicked() {
                            undo_requested = true;
                        }
                    });
                    ui.add_enabled_ui(can_redo, |ui| {
                        if ui.button("⮬ Redo (Ctrl+Y)").clicked() {
                            redo_requested = true;
                        }
                    });
                });

                // PNG export buttons
                ui.separator();
                ui.label("Export Image");
                ui.horizontal(|ui| {
                    if ui.button("💾 Save PNG (with BG)").clicked() {
                        png_export_with_background = true;
                    }
                    if ui.button("💾 Save PNG (transparent)").clicked() {
                        png_export_transparent = true;
                    }
                });

                // Accumulation controls
                if let Some(renderer) = &flame_renderer {
                    ui.separator();
                    ui.label(format!("Frames: {}", renderer.samples_accumulated()));
                    ui.label(format!("Total Iterations: {}", format_iterations(renderer.total_iterations())));

                    // Pause/Resume button
                    let button_text = if *paused { "▶ Resume" } else { "⏸ Pause" };
                    if ui.button(button_text).clicked() {
                        *paused = !*paused;
                        pause_changed = true;
                    }

                    if ui.button("Reset Accumulation").clicked() {
                        reset_requested = true;
                    }

                    // Max iterations control
                    ui.separator();
                    ui.label("Max Iterations");

                    let mut max_enabled = max_iterations.is_some();
                    if ui.checkbox(&mut max_enabled, "Enable max iterations").changed() {
                        if max_enabled {
                            *max_iterations = Some(1_000_000_000);
                        } else {
                            *max_iterations = None;
                        }
                    }

                    if let Some(max) = max_iterations {
                        // Use a logarithmic slider for better control across large ranges
                        let mut log_value = (*max as f64).log10();
                        if ui.add(egui::Slider::new(&mut log_value, 3.0..=12.0)
                            .custom_formatter(|n, _| format!("{}", format_iterations(10f64.powf(n) as u64)))
                        ).changed() {
                            *max = 10f64.powf(log_value) as u64;
                        }

                        // Show progress if enabled
                        let current = renderer.total_iterations();
                        if current >= *max {
                            ui.label("* Max iterations reached");
                        } else {
                            let progress = current as f64 / *max as f64;
                            ui.label(format!("Progress: {} / {} ({:.1}%)",
                                format_iterations(current),
                                format_iterations(*max),
                                progress * 100.0
                            ));
                        }
                    }
                }

                // Render settings
                ui.separator();
                ui.label("Render Settings");
                if ui.add(egui::Slider::new(iterations_per_thread, 64..=4096).text("Iterations per Thread")).changed() {
                    iterations_changed = true;
                }
                if ui.add(egui::Slider::new(density_scale, 0.01..=10.0).text("Density Scale")).changed() {
                    density_changed = true;
                }

                // Background color picker
                ui.separator();
                ui.label("Background Color");
                if ui.color_edit_button_rgb(background_color).changed() {
                    background_color_changed = true;
                }

                // Color settings
                ui.separator();
                ui.label("Color Settings");

                use crate::scene::palette::ColorMode;
                let current_mode = *color_mode;
                let selected_text = match current_mode {
                    ColorMode::Transform => "Transform Colors",
                    ColorMode::Palette => "Palette",
                    ColorMode::Speed => "Speed",
                };

                egui::ComboBox::from_label("Color Mode")
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        if ui.selectable_value(color_mode, ColorMode::Transform, "Transform Colors").changed() {
                            color_mode_changed = true;
                        }
                        if ui.selectable_value(color_mode, ColorMode::Palette, "Palette").changed() {
                            color_mode_changed = true;
                        }
                        if ui.selectable_value(color_mode, ColorMode::Speed, "Speed").changed() {
                            color_mode_changed = true;
                        }
                    });

                // Show palette selector for Palette and Speed modes
                if matches!(*color_mode, ColorMode::Palette | ColorMode::Speed) {
                    let palettes = palette_library.palettes();
                    let current_palette_name = palettes.get(*current_palette_index)
                        .map(|p| p.name.as_str())
                        .unwrap_or("Unknown");

                    egui::ComboBox::from_label("Palette")
                        .selected_text(current_palette_name)
                        .show_ui(ui, |ui| {
                            for (idx, palette) in palettes.iter().enumerate() {
                                if ui.selectable_value(current_palette_index, idx, &palette.name).changed() {
                                    palette_changed = true;
                                }
                            }
                        });
                }

                // Show speed factor slider in Speed mode
                if matches!(*color_mode, ColorMode::Speed) {
                    if ui.add(egui::Slider::new(speed_factor, 0.0..=1.0).text("Speed Blend Factor")).changed() {
                        color_mode_changed = true;
                    }
                }

                // Palette editor button
                if matches!(*color_mode, ColorMode::Palette | ColorMode::Speed) {
                    if ui.button("🎨 Edit Palette").clicked() {
                        self.show_palette_editor = !self.show_palette_editor;
                        // Load current palette into editor
                        if let Some(pal) = palette_library.get(*current_palette_index) {
                            self.palette_editor.current_palette = pal.clone();
                        }
                    }
                }

                // View settings
                ui.separator();
                ui.label("View");
                ui.horizontal(|ui| {
                    ui.label("Zoom:");
                    if ui.add(egui::DragValue::new(zoom).speed(0.01).range(0.01..=10000.0)).changed() {
                        view_changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Zoom In").clicked() {
                        *zoom *= 1.5;
                        view_changed = true;
                    }
                    if ui.button("Zoom Out").clicked() {
                        *zoom /= 1.5;
                        view_changed = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Pan X:");
                    if ui.add(egui::DragValue::new(pan_x).speed(0.01)).changed() {
                        view_changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Pan Y:");
                    if ui.add(egui::DragValue::new(pan_y).speed(0.01)).changed() {
                        view_changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Rotation:");
                    // Convert radians to degrees for display
                    let mut degrees = rotation.to_degrees();
                    if ui.add(egui::Slider::new(&mut degrees, -180.0..=180.0).suffix("°")).changed() {
                        *rotation = degrees.to_radians();
                        view_changed = true;
                    }
                });

                // 3D Camera rotation controls (only visible in 3D mode)
                if matches!(flame.render_mode, crate::scene::transforms::RenderMode::ThreeD) {
                    ui.separator();
                    ui.label("3D Camera Rotation");

                    ui.horizontal(|ui| {
                        ui.label("Camera Pitch (X):");
                        let mut degrees_x = camera_rotation_x.to_degrees();
                        if ui.add(egui::Slider::new(&mut degrees_x, -180.0..=180.0).suffix("°")).changed() {
                            *camera_rotation_x = degrees_x.to_radians();
                            camera_rotation_changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Camera Yaw (Y):");
                        let mut degrees_y = camera_rotation_y.to_degrees();
                        if ui.add(egui::Slider::new(&mut degrees_y, -180.0..=180.0).suffix("°")).changed() {
                            *camera_rotation_y = degrees_y.to_radians();
                            camera_rotation_changed = true;
                        }
                    });
                }

                // Pan step size depends on zoom (more zoomed in = smaller steps)
                let pan_step = 0.1 / *zoom;

                // Arrow keys layout
                ui.horizontal(|ui| {
                    ui.add_space(30.0);
                    if ui.button("  ^  ").clicked() {
                        *pan_y -= pan_step;
                        view_changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("  <  ").clicked() {
                        *pan_x -= pan_step;
                        view_changed = true;
                    }
                    if ui.button("  v  ").clicked() {
                        *pan_y += pan_step;
                        view_changed = true;
                    }
                    if ui.button("  >  ").clicked() {
                        *pan_x += pan_step;
                        view_changed = true;
                    }
                });

                if ui.button("Reset View").clicked() {
                    *zoom = 1.0;
                    *pan_x = 0.0;
                    *pan_y = 0.0;
                    *rotation = 0.0;
                    *camera_rotation_x = 0.0;
                    *camera_rotation_y = 0.0;
                    view_changed = true;
                    camera_rotation_changed = true;
                }
            });

            // Transforms window
            egui::Window::new("Transforms").show(ctx, |ui| {
                ui.heading(format!("Transforms ({})", flame.transforms.len()));

                // Add/Delete transform buttons
                ui.horizontal(|ui| {
                    if ui.button("➕ Add Transform").clicked() {
                        add_transform = true;
                        flame_changed = true;
                    }
                    ui.label(format!("({} transforms)", flame.transforms.len()));
                });

                ui.separator();

                let mut delete_index = None;
                let num_transforms = flame.transforms.len();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, transform) in flame.transforms.iter_mut().enumerate() {
                        ui.push_id(i, |ui| {
                            egui::CollapsingHeader::new(format!("Transform {}", i))
                                .default_open(i == 0)
                                .show(ui, |ui| {
                                    ui.label("Affine Matrix");

                                    ui.horizontal(|ui| {
                                        ui.label("a:");
                                        if ui.add(egui::DragValue::new(&mut transform.a).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                        ui.label("b:");
                                        if ui.add(egui::DragValue::new(&mut transform.b).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("c:");
                                        if ui.add(egui::DragValue::new(&mut transform.c).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                        ui.label("d:");
                                        if ui.add(egui::DragValue::new(&mut transform.d).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("e:");
                                        if ui.add(egui::DragValue::new(&mut transform.e).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                        ui.label("f:");
                                        if ui.add(egui::DragValue::new(&mut transform.f).speed(0.01)).changed() {
                                            flame_changed = true;
                                        }
                                    });

                                    // Z offset (only show in 3D mode)
                                    if matches!(flame.render_mode, crate::scene::transforms::RenderMode::ThreeD) {
                                        ui.horizontal(|ui| {
                                            ui.label("g (Z offset):");
                                            if ui.add(egui::DragValue::new(&mut transform.g).speed(0.01)).changed() {
                                                flame_changed = true;
                                            }
                                        });
                                    }

                                    ui.separator();
                                    ui.label("Weight");
                                    if ui.add(egui::Slider::new(&mut transform.weight, 0.0..=2.0)).changed() {
                                        flame_changed = true;
                                    }

                                    ui.separator();
                                    ui.label("Color");
                                    if ui.horizontal(|ui| {
                                        ui.label("R:");
                                        let r_changed = ui.add(egui::Slider::new(&mut transform.color[0], 0.0..=1.0)).changed();
                                        ui.label("G:");
                                        let g_changed = ui.add(egui::Slider::new(&mut transform.color[1], 0.0..=1.0)).changed();
                                        ui.label("B:");
                                        let b_changed = ui.add(egui::Slider::new(&mut transform.color[2], 0.0..=1.0)).changed();
                                        r_changed || g_changed || b_changed
                                    }).inner {
                                        flame_changed = true;
                                    }

                                    if ui.add(egui::Slider::new(&mut transform.color_speed, 0.0..=1.0).text("Color Speed")).changed() {
                                        flame_changed = true;
                                    }

                                    ui.separator();
                                    ui.label("2D Variations");

                                    let variation_names_2d = [
                                        "Linear", "Sinusoidal", "Spherical", "Swirl",
                                        "Horseshoe", "Polar", "Handkerchief", "Heart",
                                        "Disc", "Spiral", "Hyperbolic", "Diamond",
                                        "Ex", "Julia", "Bent", "Waves"
                                    ];

                                    for (idx, name) in variation_names_2d.iter().enumerate() {
                                        if ui.add(egui::Slider::new(&mut transform.variations[idx], 0.0..=2.0).text(*name)).changed() {
                                            flame_changed = true;
                                        }
                                    }

                                    // Show 3D variations only in 3D mode
                                    if matches!(flame.render_mode, crate::scene::transforms::RenderMode::ThreeD) {
                                        ui.separator();
                                        ui.label("3D Variations");

                                        let variation_names_3d = [
                                            "Zcone", "Flatten", "Hemisphere",
                                            "PreRotateX", "PreRotateY", "PostRotateX", "PostRotateY",
                                            "ZScale"
                                        ];

                                        for (i, name) in variation_names_3d.iter().enumerate() {
                                            let idx = 16 + i; // 3D variations start at index 16
                                            if ui.add(egui::Slider::new(&mut transform.variations[idx], 0.0..=2.0).text(*name)).changed() {
                                                flame_changed = true;
                                            }
                                        }
                                    }

                                    ui.separator();

                                    // Delete button (only show if more than 1 transform exists)
                                    if num_transforms > 1 {
                                        if ui.button("🗑 Delete Transform").clicked() {
                                            delete_index = Some(i);
                                            flame_changed = true;
                                        }
                                    }
                                });
                        });
                    }
                });

                // Set delete_transform if a transform was marked for deletion
                if let Some(idx) = delete_index {
                    delete_transform = Some(idx);
                }
            });

            // Palette Editor window
            if self.show_palette_editor {
                egui::Window::new("Palette Editor")
                    .default_width(600.0)
                    .show(ctx, |ui| {
                        ui.heading(&self.palette_editor.current_palette.name);
                        ui.separator();

                        // Gradient preview
                        ui.label("Gradient Preview:");
                        let preview_height = 40.0;
                        let (rect, _response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), preview_height),
                            egui::Sense::click()
                        );

                        if ui.is_rect_visible(rect) {
                            let painter = ui.painter();
                            // Draw gradient preview
                            let steps = rect.width() as usize;
                            for i in 0..steps {
                                let t = i as f32 / steps as f32;
                                let color = self.palette_editor.current_palette.sample_color(t);
                                let x = rect.left() + i as f32;
                                let color_u8 = egui::Color32::from_rgb(
                                    (color[0] * 255.0) as u8,
                                    (color[1] * 255.0) as u8,
                                    (color[2] * 255.0) as u8,
                                );
                                painter.rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(x, rect.top()),
                                        egui::vec2(1.0, preview_height)
                                    ),
                                    0.0,
                                    color_u8
                                );
                            }
                        }

                        ui.separator();

                        // Color stops list
                        ui.label("Color Stops:");

                        let mut stop_to_remove = None;

                        egui::ScrollArea::vertical()
                            .max_height(250.0)
                            .show(ui, |ui| {
                                let stops_len = self.palette_editor.current_palette.stops.len();
                                for i in 0..stops_len {
                                    ui.horizontal(|ui| {
                                        let stop = &mut self.palette_editor.current_palette.stops[i];

                                        ui.label(format!("Stop {}:", i));

                                        // Position slider - use integer 0-255
                                        let mut pos_int = (stop.position * 255.0) as i32;
                                        if ui.add(egui::Slider::new(&mut pos_int, 0..=255)
                                            .text("Position")).changed() {
                                            stop.position = pos_int as f32 / 255.0;
                                        }

                                        // Color picker
                                        let mut color = [stop.color[0], stop.color[1], stop.color[2]];
                                        if ui.color_edit_button_rgb(&mut color).changed() {
                                            stop.color = color;
                                        }

                                        // Remove button (but keep at least 2 stops)
                                        if stops_len > 2 && ui.button("🗑").clicked() {
                                            stop_to_remove = Some(i);
                                        }
                                    });
                                }
                            });

                        // Remove stop if requested
                        if let Some(idx) = stop_to_remove {
                            self.palette_editor.current_palette.stops.remove(idx);
                        }

                        ui.separator();

                        // Add stop button
                        if ui.button("➕ Add Color Stop").clicked() {
                            use crate::scene::palette::ColorStop;
                            let new_position = if self.palette_editor.current_palette.stops.is_empty() {
                                0.5
                            } else {
                                // Find a gap to insert
                                self.palette_editor.current_palette.stops.last().unwrap().position
                            };
                            self.palette_editor.current_palette.stops.push(ColorStop {
                                position: new_position,
                                color: [1.0, 1.0, 1.0],
                            });
                            // Sort by position
                            self.palette_editor.current_palette.stops.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
                        }

                        ui.separator();

                        // Import/Export section
                        ui.collapsing("Import/Export Palette", |ui| {
                            ui.horizontal(|ui| {
                                if ui.button("📋 Export to Clipboard").clicked() {
                                    palette_export_json = Some(self.palette_editor.current_palette.clone());
                                }

                                if ui.button("💾 Save as .palette").clicked() {
                                    palette_save_file = Some(self.palette_editor.current_palette.clone());
                                }
                            });

                            ui.separator();
                            ui.label("Import from JSON:");

                            egui::ScrollArea::vertical()
                                .max_height(150.0)
                                .show(ui, |ui| {
                                    ui.text_edit_multiline(&mut self.palette_editor.json_buffer);
                                });

                            ui.horizontal(|ui| {
                                if ui.button("✅ Import").clicked() && !self.palette_editor.json_buffer.is_empty() {
                                    palette_import_json = Some(self.palette_editor.json_buffer.clone());
                                }

                                if ui.button("🗑 Clear").clicked() {
                                    self.palette_editor.json_buffer.clear();
                                }

                                if ui.button("📂 Load .palette").clicked() {
                                    palette_load_file = true;
                                }
                            });
                        });

                        ui.separator();

                        ui.horizontal(|ui| {
                            if ui.button("✅ Apply").clicked() {
                                custom_palette = Some(self.palette_editor.current_palette.clone());
                                palette_changed = true;
                            }

                            if ui.button("Close").clicked() {
                                self.show_palette_editor = false;
                            }
                        });
                    });
            }

            if self.show_config_window {
                egui::Window::new("Import/Export Configuration")
                    .collapsible(false)
                    .show(ctx, |ui| {
                        ui.heading("Fractal Configuration");
                        ui.label("Export/import all settings except iterations and max iterations.");
                        ui.separator();

                        ui.horizontal(|ui| {
                            if ui.button("📋 Export to Clipboard").clicked() {
                                config_export_json = Some(String::new()); // Will be filled in app.rs
                            }

                            if ui.button("💾 Save as .flame").clicked() {
                                config_save_file = true;
                            }
                        });

                        ui.separator();
                        ui.label("Import from JSON:");

                        egui::ScrollArea::vertical()
                            .max_height(300.0)
                            .show(ui, |ui| {
                                ui.text_edit_multiline(&mut self.config_json_buffer);
                            });

                        ui.horizontal(|ui| {
                            if ui.button("✅ Import").clicked() && !self.config_json_buffer.is_empty() {
                                config_import_json = Some(self.config_json_buffer.clone());
                            }

                            if ui.button("🗑 Clear").clicked() {
                                self.config_json_buffer.clear();
                            }

                            if ui.button("📁 Load .flame").clicked() {
                                config_load_file = true;
                            }
                        });

                        ui.separator();
                        if ui.button("Close").clicked() {
                            self.show_config_window = false;
                        }
                    });
            }
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
