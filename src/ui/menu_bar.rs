/// Render the top menu bar with window visibility toggles
pub fn render_menu_bar(
    ctx: &egui::Context,
    show_performance: &mut bool,
    show_settings: &mut bool,
    show_view: &mut bool,
    show_transforms: &mut bool,
    show_triangle_editor: &mut bool,
    show_tone_mapping: &mut bool,
    show_help: &mut bool,
    show_palette_editor: &mut bool,
    show_config_window: &mut bool,
    show_undo_history: &mut bool,
    workspace: &mut super::workspace::Workspace,
    // File menu actions
    config_load_file: &mut bool,
    config_save_file: &mut bool,
    apophysis_import_file: &mut bool,
    png_export_requested: &mut bool,
    quit_requested: &mut bool,
) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            // File Menu
            ui.menu_button("File", |ui| {
                if ui.button("📂 Open Config...").clicked() {
                    *config_load_file = true;
                    ui.close();
                }

                if ui.button("💾 Save Config As...").clicked() {
                    *config_save_file = true;
                    ui.close();
                }

                ui.separator();

                if ui.button("🔥 Import Apophysis XML...").clicked() {
                    *apophysis_import_file = true;
                    ui.close();
                }

                if ui.button("🖼 Export PNG...").clicked() {
                    *png_export_requested = true;
                    ui.close();
                }

                ui.separator();

                if ui.button("❌ Quit").clicked() {
                    *quit_requested = true;
                    ui.close();
                }
            });

            // Windows Menu
            ui.menu_button("Windows", |ui| {
                ui.checkbox(show_performance, "📊 Performance");
                ui.checkbox(show_settings, "⚙ Settings");
                ui.checkbox(show_view, "🔍 View");
                ui.checkbox(show_transforms, "🔧 Transforms");
                ui.checkbox(show_triangle_editor, "📐 Triangle Editor");
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
