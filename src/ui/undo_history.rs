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

    // Calculate max height: available space minus room for buttons/stats (min 200px)
    let available_height = ui.available_height();
    let max_scroll_height = (available_height - 150.0).max(200.0);

    // Show unified timeline with #1 (initial state) at top, growing downward
    egui::ScrollArea::vertical()
        .id_salt("history_scroll_area")
        .max_height(max_scroll_height)  // Responsive to panel size
        .auto_shrink([false, true])
        .stick_to_bottom(true)  // Auto-scroll to bottom (most recent)
        .show(ui, |ui| {
            // Show initial state as entry #1 (pinned at top)
            ui.horizontal(|ui| {
                // Mark if we're at initial state
                if position == 0 {
                    ui.label("▶");
                } else {
                    ui.label(" ");
                }

                // Initial state is always "past" unless we're at position 0
                let text_color = ui.style().visuals.text_color();

                if ui.selectable_label(false, egui::RichText::new("1. Initial state").color(text_color)).clicked() {
                    // Undo back to initial state
                    if position > 0 {
                        *undo_requested = true;
                    }
                }
            });

            // Show history entries in forward order (oldest to newest, #2, #3, #4...)
            for (i, change) in history.iter().enumerate() {
                ui.horizontal(|ui| {
                    // Current position marker (position points to next change to undo)
                    // So if position == i+1, this change is the current state
                    if position == i + 1 {
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

                    // Entry number (1-indexed, including initial state as #1)
                    let label = format!("{}. {}", i + 2, change.description);

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

    ui.separator();

    // Quick action buttons
    ui.horizontal(|ui| {
        ui.add_enabled_ui(config_manager.can_undo(), |ui| {
            if ui.button("⮪ Undo").clicked() {
                *undo_requested = true;
            }
        });
        ui.add_enabled_ui(config_manager.can_redo(), |ui| {
            if ui.button("⮫ Redo").clicked() {
                *redo_requested = true;
            }
        });
    });

    // Show stats (total includes initial state as entry #1)
    ui.separator();
    ui.label(format!("Total entries: {} (including initial state)", history.len() + 1));
    ui.label(format!("Current position: #{}", position + 1));
}
