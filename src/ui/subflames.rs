//! Subflames panel
//!
//! Lists the main flame's subflames and lets the user pick which one
//! the editor (Transforms panel + Triangle Editor) targets. Since
//! the un-swap refactor, this is purely a routing change — no data
//! is moved. The renderer keeps rendering the parent's flame by
//! default, with the subflame's live edits inlined via `subflame_wf`,
//! so the user sees the effect of their edits on the parent's
//! output as they tweak.
//!
//! A separate "View subflame in isolation" checkbox flips the
//! renderer's source to the active subflame's IFS standalone — for
//! previewing the inner pattern on its own.

use egui;

use crate::config::manager::{ConfigManager, EditingTarget};

/// Render the Subflames panel content.
///
/// `view_subflame_in_isolation` is a renderer-side preference (kept
/// on `App`): when true and a subflame is being edited, the viewport
/// shows that subflame's IFS in isolation. False means the parent
/// flame is rendered with the live-edited subflame embedded.
pub fn render_subflames_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    view_subflame_in_isolation: &mut bool,
) {
    let active = config_manager.editing_target();
    let logical_count = config_manager.logical_subflame_count();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.label(
            egui::RichText::new(
                "Pick which flame the editor targets. The viewport renders the parent flame by default — toggle below to preview a subflame standalone.",
            )
            .small()
            .weak(),
        );
        ui.add_space(4.0);

        // Main flame row — always present.
        let main_name = config_manager.config().flame.name.clone();
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

        // Subflame rows — selectable label on the left, trashcan on
        // the right. Delete works in any editing context (the
        // ConfigManager updates editing_target to track the new index
        // space).
        let mut delete_request: Option<usize> = None;
        let mut select_request: Option<usize> = None;
        if logical_count == 0 {
            ui.label(egui::RichText::new("No subflames.").weak());
        } else {
            for i in 0..logical_count {
                let row_selected = matches!(active, EditingTarget::Subflame { index } if index == i);
                let name = subflame_display_name(config_manager, i);
                // Row label always leads with the index so the user
                // can tell rows apart even when names collide / are
                // blank. Format: "{index} - {name}", with "Subflame"
                // as a placeholder name when unnamed.
                let display = if name.is_empty() { "Subflame" } else { name.as_str() };
                let label = format!("{} - {}", i, display);
                ui.push_id(("subflame_row", i), |ui| {
                    ui.horizontal(|ui| {
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

        // Apply requested mutation outside the loop to avoid borrow
        // collisions with the ConfigManager helpers.
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

        // Add button. With un-swap there's no index-stability concern,
        // so this works in any editing context — the user can be on
        // a subflame and still add a new one at the end of the list.
        if ui.button("+ Add subflame").clicked() {
            if let Err(e) = config_manager.add_subflame() {
                log::warn!("add_subflame failed: {}", e);
            }
        }

        ui.add_space(8.0);
        ui.separator();

        // View-mode toggle — only meaningful while editing a subflame.
        let is_subflame = matches!(active, EditingTarget::Subflame { .. });
        let resp = ui.add_enabled(
            is_subflame,
            egui::Checkbox::new(view_subflame_in_isolation, "View subflame in isolation"),
        );
        if resp.changed() {
            // Nudge the renderer to re-upload + reset accumulation
            // so the new render source takes effect this frame.
            config_manager.request_flame_refresh();
        }
        if !is_subflame {
            ui.label(
                egui::RichText::new("Only meaningful while editing a subflame.")
                    .small()
                    .weak(),
            );
        }

        ui.add_space(8.0);
        ui.separator();

        // Rename — operates on the active flame (Main or the selected
        // subflame). With un-swap this is a direct index into
        // current.flame.{name | subflames[i].name}.
        ui.label(egui::RichText::new("Rename active flame").small().weak());
        match active {
            EditingTarget::Main => {
                let mut name = config_manager.config().flame.name.clone();
                if ui.text_edit_singleline(&mut name).changed() {
                    config_manager.config_mut().flame.name = name;
                }
            }
            EditingTarget::Subflame { index } => {
                let mut name = config_manager
                    .config()
                    .flame
                    .subflames
                    .get(index)
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
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
/// Since the un-swap refactor, this is just a direct lookup.
fn subflame_display_name(config_manager: &ConfigManager, i: usize) -> String {
    config_manager
        .config()
        .flame
        .subflames
        .get(i)
        .map(|f| f.name.clone())
        .unwrap_or_default()
}
