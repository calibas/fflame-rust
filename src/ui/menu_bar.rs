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
) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
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
            });
        });
    });
}
