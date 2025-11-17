use crate::config::ConfigManager;

/// Render undo/redo history content (for docking panels)
pub fn render_undo_history_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    undo_requested: &mut bool,
    redo_requested: &mut bool,
) {
    ui.heading("History");

    // Get unified history and current position
    let history = config_manager.history();
    let position = config_manager.position();

    if history.is_empty() {
        ui.label("(No history)");
    } else {
        // Show unified timeline with current position highlighted
        egui::ScrollArea::vertical()
            .id_salt("history_scroll_area")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                // Show in reverse order (most recent at top)
                for (i, change) in history.iter().enumerate().rev() {
                    ui.horizontal(|ui| {
                        // Current position marker
                        if i == position.saturating_sub(1) {
                            ui.label("▶");
                        } else {
                            ui.label(" ");
                        }

                        // Determine if this is future (grayed out) or past/current
                        let is_future = i >= position;
                        let text_color = if is_future {
                            ui.style().visuals.weak_text_color()
                        } else {
                            ui.style().visuals.text_color()
                        };

                        // Entry number and description
                        let label = format!("{}. {}", i + 1, change.description);

                        if ui.selectable_label(false, egui::RichText::new(label).color(text_color)).clicked() {
                            // For now, just do one undo/redo
                            // TODO: Add multi-level undo/redo to jump to specific position
                            if i < position {
                                *undo_requested = true;
                            } else if i >= position {
                                *redo_requested = true;
                            }
                        }
                    });
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
    ui.label(format!("History: {} entries", history.len()));
    ui.label(format!("Position: {} (Past: {}, Future: {})",
        position, position, history.len() - position));
}
