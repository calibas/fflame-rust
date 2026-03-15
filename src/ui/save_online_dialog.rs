//! Save Online dialog — docked panel for naming a flame before saving to the API

use egui;
use rust_i18n::t;
use super::response::ApiSaveAction;

/// State for the Save Online dialog panel
pub struct SaveOnlineDialogState {
    /// Flame name to save
    pub name: String,
    /// API flame ID if this flame already exists online (enables Update mode)
    pub api_flame_id: Option<String>,
    /// Upload a thumbnail after saving
    pub upload_thumbnail: bool,
    /// Make the flame publicly visible (None = keep existing for updates)
    pub make_public: Option<bool>,
    /// Also save the current animation alongside the flame
    pub save_animation: bool,
    /// Whether animation tracks exist (controls default for save_animation)
    pub has_animation_tracks: bool,
    /// Action produced by the dialog (polled after render)
    pub action: Option<ApiSaveAction>,
    /// Whether the dialog should be closed (Cancel or after Save)
    pub close_requested: bool,
}

impl Default for SaveOnlineDialogState {
    fn default() -> Self {
        Self {
            name: String::new(),
            api_flame_id: None,
            upload_thumbnail: true,
            make_public: Some(false),
            save_animation: false,
            has_animation_tracks: false,
            action: None,
            close_requested: false,
        }
    }
}

impl SaveOnlineDialogState {
    /// Pre-fill the dialog before opening
    pub fn open(&mut self, name: &str, api_flame_id: Option<String>, is_public: Option<bool>, has_animation_tracks: bool) {
        self.name = name.to_string();
        self.api_flame_id = api_flame_id;
        self.upload_thumbnail = true;
        // For new flames: default private. For updates: use known visibility, or None = keep existing.
        self.make_public = if self.api_flame_id.is_some() {
            is_public // Some(true/false) if known, None if unknown
        } else {
            Some(false)
        };
        self.save_animation = has_animation_tracks;
        self.has_animation_tracks = has_animation_tracks;
        self.action = None;
        self.close_requested = false;
    }

    /// Take the pending action (if any)
    pub fn take_action(&mut self) -> Option<ApiSaveAction> {
        self.action.take()
    }

    /// Whether this is an update to an existing flame
    pub fn is_update(&self) -> bool {
        self.api_flame_id.is_some()
    }
}

/// Render the Save Online dialog contents.
pub fn render_save_online_dialog(
    ui: &mut egui::Ui,
    state: &mut SaveOnlineDialogState,
) {
    let is_update = state.is_update();

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(t!("api.save_dialog_name_label"));
        let response = ui.text_edit_singleline(&mut state.name);

        // Auto-focus the text field
        if !response.has_focus() {
            response.request_focus();
        }

        // Enter key submits (primary action: update or save new)
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let name = state.name.trim().to_string();
            if !name.is_empty() {
                state.action = Some(make_primary_action(state, &name));
                state.close_requested = true;
            }
        }
    });

    ui.add_space(4.0);

    // Options checkboxes
    ui.checkbox(&mut state.upload_thumbnail, t!("api.save_dialog_thumbnail"));

    // Visibility toggle
    if is_update {
        // Three-state for updates: None (keep existing), Some(false), Some(true)
        let mut checked = state.make_public.unwrap_or(false);
        let response = ui.checkbox(&mut checked, t!("api.save_dialog_public"));
        if response.changed() {
            state.make_public = Some(checked);
        }
        if state.make_public.is_none() {
            ui.label(egui::RichText::new(t!("api.save_dialog_visibility_unchanged")).weak().small());
        }
    } else {
        let mut checked = state.make_public.unwrap_or(false);
        if ui.checkbox(&mut checked, t!("api.save_dialog_public")).changed() {
            state.make_public = Some(checked);
        }
    }

    if state.has_animation_tracks {
        ui.checkbox(&mut state.save_animation, t!("api.save_dialog_animation"));
    }

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        if is_update {
            // Update button (primary)
            if ui.button(t!("api.save_dialog_update")).clicked() {
                let name = state.name.trim().to_string();
                if !name.is_empty() {
                    state.action = Some(ApiSaveAction::Update {
                        name,
                        upload_thumbnail: state.upload_thumbnail,
                        make_public: state.make_public,
                        save_animation: state.save_animation,
                    });
                    state.close_requested = true;
                }
            }
            // Save As Copy button
            if ui.button(t!("api.save_dialog_save_as_copy")).clicked() {
                let name = state.name.trim().to_string();
                if !name.is_empty() {
                    state.action = Some(ApiSaveAction::SaveNew {
                        name,
                        upload_thumbnail: state.upload_thumbnail,
                        make_public: state.make_public.unwrap_or(false),
                        save_animation: state.save_animation,
                    });
                    state.close_requested = true;
                }
            }
        } else {
            // Save button (new flame)
            if ui.button(t!("api.save_dialog_save")).clicked() {
                let name = state.name.trim().to_string();
                if !name.is_empty() {
                    state.action = Some(ApiSaveAction::SaveNew {
                        name,
                        upload_thumbnail: state.upload_thumbnail,
                        make_public: state.make_public.unwrap_or(false),
                        save_animation: state.save_animation,
                    });
                    state.close_requested = true;
                }
            }
        }
        if ui.button(t!("api.save_dialog_cancel")).clicked() {
            state.close_requested = true;
        }
    });
}

/// Build the primary action (used for Enter key submission)
fn make_primary_action(state: &SaveOnlineDialogState, name: &str) -> ApiSaveAction {
    if state.is_update() {
        ApiSaveAction::Update {
            name: name.to_string(),
            upload_thumbnail: state.upload_thumbnail,
            make_public: state.make_public,
            save_animation: state.save_animation,
        }
    } else {
        ApiSaveAction::SaveNew {
            name: name.to_string(),
            upload_thumbnail: state.upload_thumbnail,
            make_public: state.make_public.unwrap_or(false),
            save_animation: state.save_animation,
        }
    }
}
