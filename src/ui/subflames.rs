//! Subflames panel
//!
//! Lists the main flame's subflames and lets the user switch which one
//! the editor is focused on. Selecting "Main" or a subflame swaps the
//! whole UI/render pipeline onto that flame's data via
//! `ConfigManager::set_editing_target` — the Transforms panel, Triangle
//! Editor, and Fractal viewport all start operating on the selected
//! flame transparently.
//!
//! Add/Delete are only available while editing Main; doing them mid-
//! subflame-edit would shift the index space and confuse both the
//! user and the swap mechanism.

use egui;

use crate::config::manager::{ConfigManager, EditingTarget};

/// Render the Subflames panel content.
pub fn render_subflames_content(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    let active = config_manager.editing_target();
    let logical_count = config_manager.logical_subflame_count();
    let is_subflame = config_manager.is_editing_subflame();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.label(
            egui::RichText::new(
                "Switch which flame the editor and viewport operate on.",
            )
            .small()
            .weak(),
        );
        ui.add_space(4.0);

        // Main flame row (always present)
        let main_name = match active {
            EditingTarget::Main => config_manager.config().flame.name.clone(),
            EditingTarget::Subflame { .. } => {
                // While editing a subflame, the real main lives in the
                // stash. logical_config() reconstructs it.
                config_manager.logical_config().flame.name
            }
        };
        let main_label = if main_name.is_empty() {
            "Main".to_string()
        } else {
            format!("Main — {}", main_name)
        };
        let main_selected = matches!(active, EditingTarget::Main);
        if ui
            .selectable_label(main_selected, main_label)
            .clicked()
            && !main_selected
        {
            if let Err(e) = config_manager.set_editing_target(EditingTarget::Main) {
                log::warn!("set_editing_target(Main) failed: {}", e);
            }
        }

        ui.separator();

        // Subflame rows — each row has the selectable label on the left
        // and a trashcan delete button on the right. Deletion auto-swaps
        // back to Main (in ConfigManager::delete_subflame), so it's safe
        // to invoke from any editing context, including on the active
        // subflame.
        let mut delete_request: Option<usize> = None;
        let mut select_request: Option<usize> = None;
        if logical_count == 0 {
            ui.label(egui::RichText::new("No subflames.").weak());
        } else {
            for i in 0..logical_count {
                let row_selected = matches!(active, EditingTarget::Subflame { index } if index == i);
                let name = subflame_display_name(config_manager, i);
                let label = if name.is_empty() {
                    "Subflame".to_string()
                } else {
                    format!("Subflame — {}", name)
                };
                ui.push_id(("subflame_row", i), |ui| {
                    ui.horizontal(|ui| {
                        // Trash on the right; place it first via
                        // right-to-left layout so the selectable_label
                        // expands to fill the remaining width.
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .small_button("🗑")
                                    .on_hover_text("Delete this subflame")
                                    .clicked()
                                {
                                    delete_request = Some(i);
                                }
                                ui.with_layout(
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        let resp = ui.selectable_label(row_selected, label);
                                        if resp.clicked() && !row_selected {
                                            select_request = Some(i);
                                        }
                                    },
                                );
                            },
                        );
                    });
                });
            }
        }

        // Apply requested deletion/selection after the loop so we don't
        // mutate the manager mid-iteration over its visible state.
        if let Some(i) = delete_request {
            if let Err(e) = config_manager.delete_subflame(i) {
                log::warn!("delete_subflame({}) failed: {}", i, e);
            }
        } else if let Some(i) = select_request {
            if let Err(e) =
                config_manager.set_editing_target(EditingTarget::Subflame { index: i })
            {
                log::warn!("set_editing_target(Subflame {}) failed: {}", i, e);
            }
        }

        ui.add_space(8.0);
        ui.separator();

        // Add button — disabled while editing a subflame because adding
        // mid-edit would shift the index space and break the stash
        // invariant. (Delete is fine; it auto-swaps to Main first.)
        let can_add = !is_subflame;
        if ui
            .add_enabled(can_add, egui::Button::new("+ Add subflame"))
            .clicked()
        {
            if let Err(e) = config_manager.add_subflame() {
                log::warn!("add_subflame failed: {}", e);
            }
        }
        if is_subflame {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Switch to Main before adding a subflame.")
                    .small()
                    .weak(),
            );
        }

        ui.add_space(8.0);

        // Rename of the active flame (only). For the non-active rows,
        // renaming would mean another text field per row — out of scope
        // for v1; user can rename a subflame by selecting it then
        // editing the name here.
        ui.separator();
        ui.label(egui::RichText::new("Rename active flame").small().weak());
        match active {
            EditingTarget::Main => {
                let mut name = config_manager.config().flame.name.clone();
                if ui.text_edit_singleline(&mut name).changed() {
                    config_manager.config_mut().flame.name = name;
                }
            }
            EditingTarget::Subflame { index } => {
                let mut name = config_manager.config().flame.name.clone();
                if ui.text_edit_singleline(&mut name).changed() {
                    if let Err(e) = config_manager.rename_subflame(index, name) {
                        log::warn!("rename_subflame failed: {}", e);
                    }
                }
            }
        }
    });
}

/// Display name for the subflame at user-visible index `i`.
///
/// While editing a subflame, the active slot's data lives in
/// `current.flame` (not in the subflames list) — so resolving names
/// has to account for the swap.
fn subflame_display_name(config_manager: &ConfigManager, i: usize) -> String {
    match config_manager.editing_target() {
        EditingTarget::Main => config_manager
            .config()
            .flame
            .subflames
            .get(i)
            .map(|f| f.name.clone())
            .unwrap_or_default(),
        EditingTarget::Subflame { index: active } => {
            if i == active {
                // The active subflame's data is in current.flame
                config_manager.config().flame.name.clone()
            } else {
                config_manager
                    .visible_subflames()
                    .get(if i < active { i } else { i - 1 })
                    .map(|f| f.name.clone())
                    .unwrap_or_default()
            }
        }
    }
}
