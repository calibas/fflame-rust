use crate::config::ConfigManager;
use rust_i18n::t;

/// Translate a history description
///
/// Descriptions can be:
/// - Simple i18n keys: `history.action.reset_view`
/// - i18n keys with parameters: `history.action.translate_up|name=Transform 1`
/// - Fallback: returned as-is if not a valid i18n key
fn translate_description(description: &str) -> String {
    // Check if this looks like an i18n key (starts with "history.")
    if description.starts_with("history.") {
        // Check for parameters (pipe-separated: key|param1=value1|param2=value2)
        if let Some(pipe_pos) = description.find('|') {
            let key = &description[..pipe_pos];
            let params_str = &description[pipe_pos + 1..];

            // Parse the "name" parameter (most common case)
            // Format: key|name=value
            if let Some(name_value) = params_str.strip_prefix("name=") {
                t!(key, name = name_value).to_string()
            } else {
                // Unknown parameter format - just translate the key
                t!(key).to_string()
            }
        } else {
            // Simple key without parameters
            t!(description).to_string()
        }
    } else {
        // Not an i18n key - return as-is (backward compatibility with old descriptions)
        description.to_string()
    }
}

/// Render undo/redo history content (for docking panels)
pub fn render_undo_history_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    undo_requested: &mut bool,
    redo_requested: &mut bool,
) {
    ui.heading(t!("history.heading"));

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

                let initial_label = format!("1. {}", t!("history.initial_state"));
                if ui.selectable_label(false, egui::RichText::new(initial_label).color(text_color)).clicked() {
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
                    let translated = translate_description(&change.description);
                    let label = format!("{}. {}", i + 2, translated);

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
            if ui.button(format!("⮪ {}", t!("history.undo"))).clicked() {
                *undo_requested = true;
            }
        });
        ui.add_enabled_ui(config_manager.can_redo(), |ui| {
            if ui.button(format!("⮫ {}", t!("history.redo"))).clicked() {
                *redo_requested = true;
            }
        });
    });

    // Show stats (total includes initial state as entry #1)
    ui.separator();
    ui.label(t!("history.total_entries", count = history.len() + 1));
    ui.label(t!("history.current_position", position = position + 1));
}
