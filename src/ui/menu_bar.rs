use super::menu_context::{MenuActions, MenuState};

/// Render the top menu bar with window visibility toggles
pub fn render_menu_bar(
    ctx: &egui::Context,
    show_tone_mapping: &mut bool,
    show_help: &mut bool,
    show_palette_editor: &mut bool,
    show_config_window: &mut bool,
    show_undo_history: &mut bool,
    workspace: &mut super::workspace::Workspace,
    menu_actions: &mut MenuActions,
    menu_state: &MenuState,
) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            // File Menu
            ui.menu_button("File", |ui| {
                if ui.button("📂 Open Config...").clicked() {
                    menu_actions.file.load_config = true;
                    ui.close();
                }

                if ui.button("💾 Save Config As...").clicked() {
                    menu_actions.file.save_config = true;
                    ui.close();
                }

                ui.separator();

                if ui.button("Import Apophysis XML...").clicked() {
                    menu_actions.file.import_apophysis = true;
                    ui.close();
                }
                ui.add_enabled(false, egui::Button::new("Export Apophysis XML..."));

                if ui.button("🖼 Export PNG...").clicked() {
                    menu_actions.file.export_png = true;
                    ui.close();
                }

                ui.separator();

                if ui.button("❌ Quit").clicked() {
                    menu_actions.file.quit = true;
                    ui.close();
                }
            });

            // Edit Menu
            ui.menu_button("Edit", |ui| {
                if ui.add_enabled(menu_state.can_undo, egui::Button::new("⮪ Undo")).clicked() {
                    menu_actions.edit.undo = true;
                }

                if ui.add_enabled(menu_state.can_redo, egui::Button::new("⮬ Redo")).clicked() {
                    menu_actions.edit.redo = true;
                }

                ui.separator();

                // Future features (not implemented yet)
                ui.add_enabled(false, egui::Button::new("📋 Copy Transform"));
                ui.add_enabled(false, egui::Button::new("📄 Paste Transform"));
                ui.add_enabled(false, egui::Button::new("📑 Duplicate Transform"));

                ui.separator();

                ui.add_enabled(false, egui::Button::new("⚙ Preferences..."));
            });

            // View Menu
            ui.menu_button("View", |ui| {
                if ui.button("Reset View").clicked() {
                    menu_actions.view.reset_view = true;
                    ui.close();
                }

                ui.separator();

                if ui.button("Zoom In").clicked() {
                    menu_actions.view.zoom_in = true;
                    ui.close();
                }

                if ui.button("Zoom Out").clicked() {
                    menu_actions.view.zoom_out = true;
                    ui.close();
                }

                ui.separator();

                // Radio buttons for render mode
                let is_2d = menu_state.render_mode_2d;
                if ui.selectable_label(is_2d, "2D Mode").clicked() {
                    menu_actions.view.set_mode_2d = true;
                    ui.close();
                }

                if ui.selectable_label(!is_2d, "3D Mode").clicked() {
                    menu_actions.view.set_mode_3d = true;
                    ui.close();
                }
            });

            // Transforms Menu
            ui.menu_button("Transforms", |ui| {
                // Panel visibility toggles
                let transforms_open = workspace.panel_exists(super::workspace::PanelType::Transforms);
                if ui.selectable_label(transforms_open, "Show Transform Editor").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Transforms);
                    ui.close();
                }
                let triangle_editor_open = workspace.panel_exists(super::workspace::PanelType::TriangleEditor);
                if ui.selectable_label(triangle_editor_open, "Show Triangle Editor").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::TriangleEditor);
                    ui.close();
                }
                ui.checkbox(show_palette_editor, "Show Palette Editor");

                ui.separator();

                // Add Transform (functional)
                if ui.button("Add Transform").clicked() {
                    menu_actions.transform.add_transform = true;
                    ui.close();
                }

                // Not implemented yet
                ui.add_enabled(false, egui::Button::new("Copy Transform"));
                ui.add_enabled(false, egui::Button::new("Paste Transform"));
                ui.add_enabled(false, egui::Button::new("Duplicate Transform"));
                ui.add_enabled(false, egui::Button::new("Delete Transform"));
                ui.add_enabled(false, egui::Button::new("Randomize Transform"));
            });

            // Rendering Menu
            ui.menu_button("Rendering", |ui| {
                // Pause/Resume
                let pause_text = if menu_state.is_paused { "▶ Resume" } else { "⏸ Pause" };
                if ui.button(pause_text).clicked() {
                    menu_actions.rendering.pause_toggle = true;
                    ui.close();
                }

                // Reset Accumulation
                if ui.button("🔄 Reset Accumulation").clicked() {
                    menu_actions.rendering.reset_accumulation = true;
                    ui.close();
                }

                ui.separator();

                // Speed submenu
                ui.menu_button("Speed ▶", |ui| {
                    for &speed in &[1u32, 2, 4, 8, 16] {
                        if ui.button(format!("{}x", speed)).clicked() {
                            menu_actions.rendering.set_speed = Some(speed);
                            ui.close();
                        }
                    }
                });

                ui.separator();

                // Iterations per Thread submenu
                ui.menu_button("Iterations per Thread ▶", |ui| {
                    for &ipt in &[64u32, 128, 256, 512, 1024] {
                        if ui.button(format!("{}", ipt)).clicked() {
                            menu_actions.rendering.set_iterations_per_thread = Some(ipt);
                            ui.close();
                        }
                    }
                });

                ui.separator();

                // Benchmark (not implemented)
                ui.add_enabled(false, egui::Button::new("Benchmark..."));
            });

            // Windows Menu
            ui.menu_button("Windows", |ui| {
                // Performance opens as floating window in docking system (only one instance)
                let performance_open = workspace.panel_exists(super::workspace::PanelType::Performance);
                if ui.selectable_label(performance_open, "📊 Performance").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Performance);
                    ui.close();
                }

                // Settings opens Rendering panel as floating window (Settings was renamed to Rendering)
                let rendering_open = workspace.panel_exists(super::workspace::PanelType::Rendering);
                if ui.selectable_label(rendering_open, "⚙ Rendering").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Rendering);
                    ui.close();
                }

                // View opens as floating window in docking system
                let view_open = workspace.panel_exists(super::workspace::PanelType::View);
                if ui.selectable_label(view_open, "🔍 View").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::View);
                    ui.close();
                }

                // Transforms opens as floating window in docking system
                let transforms_open = workspace.panel_exists(super::workspace::PanelType::Transforms);
                if ui.selectable_label(transforms_open, "🔧 Transforms").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Transforms);
                    ui.close();
                }

                // Triangle Editor opens as floating window in docking system
                let triangle_editor_open = workspace.panel_exists(super::workspace::PanelType::TriangleEditor);
                if ui.selectable_label(triangle_editor_open, "📐 Triangle Editor").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::TriangleEditor);
                    ui.close();
                }
                ui.checkbox(show_tone_mapping, "🎨 Tone Mapping & Colors");
                ui.checkbox(show_help, "❓ Help");
                ui.separator();
                ui.checkbox(show_palette_editor, "🎨 Palette Editor");
                ui.checkbox(show_config_window, "📄 Config Import/Export");
                ui.checkbox(show_undo_history, "⮪ Undo/Redo History");

                ui.separator();
                ui.menu_button("📐 Workspace Layout", |ui| {
                    let current = workspace.current_layout;

                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Beginner, "Beginner").clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Beginner);
                        ui.close();
                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Standard, "Standard").clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Standard);
                        ui.close();
                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Advanced, "Advanced").clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Advanced);
                        ui.close();
                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Export, "Export").clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Export);
                        ui.close();
                    }
                });
            });
        });
    });
}
