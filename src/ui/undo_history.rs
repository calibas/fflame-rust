use crate::config::ConfigManager;

/// Render the Undo/Redo History window
pub fn render_undo_history_window(
    ctx: &egui::Context,
    show_undo_history: &mut bool,
    config_manager: &mut ConfigManager,
    undo_requested: &mut bool,
    redo_requested: &mut bool,
) {
    egui::Window::new("Undo/Redo History")
        .open(show_undo_history)
        .default_width(300.0)
        .show(ctx, |ui| {
            ui.heading("Undo History");

            // Undo stack (show in reverse order - most recent first)
            let undo_history = config_manager.undo_history();
            if undo_history.is_empty() {
                ui.label("(No undo history)");
            } else {
                egui::ScrollArea::vertical()
                    .id_source("undo_scroll_area")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for (i, change) in undo_history.iter().enumerate().rev() {
                            // Show each change with its description
                            let label = format!("{}. {}", undo_history.len() - i, change.description);

                            if ui.selectable_label(false, label).clicked() {
                                // Calculate how many undos to perform to get to this point
                                let undos_needed = i + 1;
                                // For now, just do one undo and let user click multiple times
                                // TODO: Add multi-level undo support
                                *undo_requested = true;
                            }
                        }
                    });
            }

            ui.separator();
            ui.heading("Redo History");

            // Redo stack (show in order)
            let redo_history = config_manager.redo_history();
            if redo_history.is_empty() {
                ui.label("(No redo history)");
            } else {
                egui::ScrollArea::vertical()
                    .id_source("redo_scroll_area")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for (i, change) in redo_history.iter().enumerate() {
                            // Show each change with its description
                            let label = format!("{}. {}", i + 1, change.description);

                            if ui.selectable_label(false, label).clicked() {
                                // For now, just do one redo and let user click multiple times
                                // TODO: Add multi-level redo support
                                *redo_requested = true;
                            }
                        }
                    });
            }

            ui.separator();

            // Quick action buttons
            ui.horizontal(|ui| {
                ui.add_enabled_ui(config_manager.can_undo(), |ui| {
                    if ui.button("⮪ Undo").clicked() {
                        *undo_requested = true;
                    }
                });
                ui.add_enabled_ui(config_manager.can_redo(), |ui| {
                    if ui.button("⮬ Redo").clicked() {
                        *redo_requested = true;
                    }
                });
            });

            // Show stats
            ui.separator();
            ui.label(format!("Undo stack: {} entries", undo_history.len()));
            ui.label(format!("Redo stack: {} entries", redo_history.len()));
        });
}
