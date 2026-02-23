//! Save Online dialog — docked panel for naming a flame before saving to the API

use egui;
use rust_i18n::t;
use super::response::ApiSaveAction;

/// State for the Save Online dialog panel
pub struct SaveOnlineDialogState {
    /// Flame name to save
    pub name: String,
    /// Whether this is a "new copy" save (true) or first save (false)
    pub is_copy: bool,
    /// Action produced by the dialog (polled after render)
    pub action: Option<ApiSaveAction>,
    /// Whether the dialog should be closed (Cancel or after Save)
    pub close_requested: bool,
}

impl Default for SaveOnlineDialogState {
    fn default() -> Self {
        Self {
            name: String::new(),
            is_copy: false,
            action: None,
            close_requested: false,
        }
    }
}

impl SaveOnlineDialogState {
    /// Pre-fill the dialog before opening
    pub fn open(&mut self, name: &str, is_copy: bool) {
        self.name = name.to_string();
        self.is_copy = is_copy;
        self.action = None;
        self.close_requested = false;
    }

    /// Take the pending action (if any)
    pub fn take_action(&mut self) -> Option<ApiSaveAction> {
        self.action.take()
    }
}

/// Render the Save Online dialog contents.
pub fn render_save_online_dialog(
    ui: &mut egui::Ui,
    state: &mut SaveOnlineDialogState,
) {
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(t!("api.save_dialog_name_label"));
        let response = ui.text_edit_singleline(&mut state.name);

        // Auto-focus the text field
        if !response.has_focus() {
            response.request_focus();
        }

        // Enter key submits
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let name = state.name.trim().to_string();
            if !name.is_empty() {
                state.action = Some(ApiSaveAction::SaveNew { name });
                state.close_requested = true;
            }
        }
    });

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        if ui.button(t!("api.save_dialog_save")).clicked() {
            let name = state.name.trim().to_string();
            if !name.is_empty() {
                state.action = Some(ApiSaveAction::SaveNew { name });
                state.close_requested = true;
            }
        }
        if ui.button(t!("api.save_dialog_cancel")).clicked() {
            state.close_requested = true;
        }
    });
}
