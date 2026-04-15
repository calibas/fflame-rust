//! Save Online dialog — unified panel for saving flame and animation to the API

use egui;
use rust_i18n::t;
use super::response::{ApiSaveAction, ApiAnimationSaveAction};

/// State for the Save Online dialog panel
pub struct SaveOnlineDialogState {
    /// Flame name to save
    pub name: String,
    /// API flame ID if this flame already exists online
    pub api_flame_id: Option<String>,
    /// API animation ID if this animation already exists online
    pub api_animation_id: Option<String>,
    /// Upload a thumbnail after saving
    pub upload_thumbnail: bool,
    /// Make the flame publicly visible
    pub make_public: bool,
    /// Whether animation tracks exist (controls animation section)
    pub has_animation_tracks: bool,
    /// Number of animations linked to this flame (from API)
    pub animation_count: u32,
    /// Whether the current user owns the flame (disables Update if false)
    pub flame_owned: bool,
    /// Whether the current user owns the animation (disables Update if false)
    pub animation_owned: bool,
    /// Flame action produced by the dialog
    pub flame_action: Option<ApiSaveAction>,
    /// Animation action produced by the dialog
    pub animation_action: Option<ApiAnimationSaveAction>,
    /// Whether the dialog should be closed
    pub close_requested: bool,
}

impl Default for SaveOnlineDialogState {
    fn default() -> Self {
        Self {
            name: String::new(),
            api_flame_id: None,
            api_animation_id: None,
            upload_thumbnail: true,
            make_public: false,
            has_animation_tracks: false,
            animation_count: 0,
            flame_owned: true,
            animation_owned: true,
            flame_action: None,
            animation_action: None,
            close_requested: false,
        }
    }
}

impl SaveOnlineDialogState {
    /// Pre-fill the dialog before opening
    pub fn open(
        &mut self,
        name: &str,
        api_flame_id: Option<String>,
        api_animation_id: Option<String>,
        is_public: Option<bool>,
        has_animation_tracks: bool,
        animation_count: u32,
        flame_owned: bool,
        animation_owned: bool,
    ) {
        self.name = name.to_string();
        self.api_flame_id = api_flame_id;
        self.api_animation_id = api_animation_id;
        self.upload_thumbnail = true;
        self.make_public = is_public.unwrap_or(false);
        self.has_animation_tracks = has_animation_tracks;
        self.animation_count = animation_count;
        self.flame_owned = flame_owned;
        self.animation_owned = animation_owned;
        self.flame_action = None;
        self.animation_action = None;
        self.close_requested = false;
    }

    /// Take the pending flame action (if any)
    pub fn take_flame_action(&mut self) -> Option<ApiSaveAction> {
        self.flame_action.take()
    }

    /// Take the pending animation action (if any)
    pub fn take_animation_action(&mut self) -> Option<ApiAnimationSaveAction> {
        self.animation_action.take()
    }

    /// Whether the flame exists online
    pub fn has_flame(&self) -> bool {
        self.api_flame_id.is_some()
    }

    /// Whether the animation exists online
    pub fn has_animation(&self) -> bool {
        self.api_animation_id.is_some()
    }
}

/// Render the Save Online dialog contents.
pub fn render_save_online_dialog(
    ui: &mut egui::Ui,
    state: &mut SaveOnlineDialogState,
) {
    let has_flame = state.has_flame();
    let has_animation = state.has_animation();

    ui.add_space(8.0);

    // Name field
    ui.horizontal(|ui| {
        ui.label(t!("api.save_dialog_name_label"));
        let response = ui.text_edit_singleline(&mut state.name);
        super::vkb_sync(ui, &response, &state.name);
    });

    ui.add_space(4.0);

    // Options
    ui.checkbox(&mut state.upload_thumbnail, t!("api.save_dialog_thumbnail"));
    ui.checkbox(&mut state.make_public, t!("api.save_dialog_public"));

    ui.add_space(8.0);

    // ── Flame section ──
    ui.separator();
    ui.label(egui::RichText::new(t!("api.save_dialog_flame_section")).strong());
    ui.add_space(4.0);

    // Show flame ID if set
    if let Some(ref id) = state.api_flame_id {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(t!("api.save_dialog_id_label")).size(10.0).color(egui::Color32::GRAY));
            ui.label(egui::RichText::new(id).size(10.0).monospace());
        });
    }

    // Warning if animations depend on this flame
    if has_flame && state.animation_count > 0 {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(
                    t!("api.save_dialog_animation_warning", count = state.animation_count)
                )
                .color(egui::Color32::from_rgb(255, 200, 80))
                .small()
            );
        });
        ui.add_space(2.0);
    }

    ui.horizontal(|ui| {
        let name = state.name.trim().to_string();
        let name_valid = !name.is_empty();

        if has_flame {
            // Update (only if owned)
            if ui.add_enabled(name_valid && state.flame_owned, egui::Button::new(t!("api.save_dialog_update"))).clicked() {
                state.flame_action = Some(ApiSaveAction::Update {
                    name: name.clone(),
                    upload_thumbnail: state.upload_thumbnail,
                    make_public: state.make_public,
                });
            }
            // Save as Copy
            if ui.add_enabled(name_valid, egui::Button::new(t!("api.save_dialog_save_as_copy"))).clicked() {
                state.flame_action = Some(ApiSaveAction::SaveNew {
                    name,
                    upload_thumbnail: state.upload_thumbnail,
                    make_public: state.make_public,
                });
            }
        } else {
            // Save (new flame)
            if ui.add_enabled(name_valid, egui::Button::new(t!("api.save_dialog_save"))).clicked() {
                state.flame_action = Some(ApiSaveAction::SaveNew {
                    name,
                    upload_thumbnail: state.upload_thumbnail,
                    make_public: state.make_public,
                });
            }
        }
    });

    ui.add_space(8.0);

    // ── Animation section ──
    ui.separator();
    ui.label(egui::RichText::new(t!("api.save_dialog_animation_section")).strong());
    ui.add_space(4.0);

    // Show animation ID if set
    if let Some(ref id) = state.api_animation_id {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(t!("api.save_dialog_id_label")).size(10.0).color(egui::Color32::GRAY));
            ui.label(egui::RichText::new(id).size(10.0).monospace());
        });
    }

    // Animation buttons: only active if flame is saved AND has tracks
    let can_save_animation = has_flame && state.has_animation_tracks;

    ui.horizontal(|ui| {
        let make_public = state.make_public;
        if has_animation {
            // Update (only if owned)
            if ui.add_enabled(state.animation_owned, egui::Button::new(t!("api.save_dialog_update"))).clicked() {
                state.animation_action = Some(ApiAnimationSaveAction::Update { make_public });
            }
            // Save as Copy
            if ui.add_enabled(can_save_animation, egui::Button::new(t!("api.save_dialog_save_as_copy"))).clicked() {
                state.animation_action = Some(ApiAnimationSaveAction::SaveNew { make_public });
            }
        } else {
            // Save (new animation)
            if ui.add_enabled(can_save_animation, egui::Button::new(t!("api.save_dialog_save"))).clicked() {
                state.animation_action = Some(ApiAnimationSaveAction::SaveNew { make_public });
            }
        }
    });

    if !has_flame && state.has_animation_tracks {
        ui.label(
            egui::RichText::new(t!("api.save_dialog_save_flame_first"))
                .small()
                .color(egui::Color32::GRAY)
        );
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // Cancel (right-aligned)
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.button(t!("api.save_dialog_cancel")).clicked() {
            state.close_requested = true;
        }
    });
}
