//! Compact mode floating menu button — replaces the top menu bar on small screens.
//!
//! Renders a small hamburger button anchored to the top-right corner.
//! Fades out after 5 seconds of inactivity, reappears on any input.
//! Tapping the button opens a popup with the same menu items as the
//! desktop menu bar, but panels open via `open_compact_panel()` (docked
//! into the main surface) instead of floating windows.

use super::menu_context::{MenuActions, MenuState};
use super::workspace::{PanelType, Workspace};
use rust_i18n::t;

/// Render the compact floating menu button and its popup.
pub fn render_compact_menu(
    ctx: &egui::Context,
    workspace: &mut Workspace,
    menu_actions: &mut MenuActions,
    menu_state: &MenuState,
    save_online_dialog_state: &mut super::save_online_dialog::SaveOnlineDialogState,
) {

    // Semi-transparent background frame
    let bg_alpha = 180;
    let text_alpha = 255;

    let frame = egui::Frame::NONE
        .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, bg_alpha))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(4));

    egui::Area::new(egui::Id::new("compact_menu_button"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 16.0))
        .order(egui::Order::Tooltip)
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                let button_text = egui::RichText::new("\u{2630}") // hamburger icon
                    .size(20.0)
                    .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, text_alpha));

                let response = ui.add(egui::Button::new(button_text).frame(false));
                let popup_id = ui.id().with("compact_menu_popup");

                if response.clicked() {
                    ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                }

                egui::popup_below_widget(ui, popup_id, &response, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                    ui.set_min_width(200.0);
                    ui.style_mut().override_font_id = Some(egui::FontId::proportional(16.0));
                    render_compact_menu_items(ui, ctx, workspace, menu_actions, menu_state, save_online_dialog_state);
                });
            });
        });
}

/// Render the menu items inside the compact popup.
/// Uses `open_compact_panel()` to dock panels into the main surface.
fn render_compact_menu_items(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    workspace: &mut Workspace,
    menu_actions: &mut MenuActions,
    menu_state: &MenuState,
    save_online_dialog_state: &mut super::save_online_dialog::SaveOnlineDialogState,
) {
    // --- Window submenu ---
    ui.menu_button(t!("menu.window"), |ui| {
        let panel_items: &[(PanelType, &str)] = &[
            (PanelType::Transforms, "menu.window_transforms"),
            (PanelType::TriangleEditor, "menu.window_triangle_editor"),
            (PanelType::Colors, "menu.window_colors"),
            (PanelType::View, "menu.window_view"),
            (PanelType::Rendering, "menu.window_rendering"),
            (PanelType::PaletteEditor, "menu.window_palette_editor"),
            (PanelType::PaletteLibrary, "menu.window_palette_library"),
            (PanelType::FractalBrowser, "menu.window_fractal_browser"),
            (PanelType::History, "menu.window_history"),
            (PanelType::Effects, "menu.window_effects"),
            (PanelType::XaosEditor, "menu.window_xaos_editor"),
            (PanelType::Animation, "menu.window_animation"),
            (PanelType::Signal, "menu.window_signal"),
            (PanelType::Export, "menu.export"),
        ];

        for &(panel_type, key) in panel_items {
            let is_open = workspace.panel_exists(panel_type);
            if ui.selectable_label(is_open, t!(key).as_ref()).clicked() {
                workspace.open_compact_panel(panel_type, ctx);
                ui.close();
            }
        }
    });
    // --- File submenu ---
    ui.menu_button(t!("menu.file"), |ui| {
        if ui.button(t!("menu.new")).clicked() {
            menu_actions.file.new_flame = true;
            ui.close();
        }

        if ui.button(t!("menu.from_preset_library")).clicked() {
            menu_actions.file.open_preset_library = true;
            ui.close();
        }

        if ui.button(t!("menu.random_flame")).clicked() {
            menu_actions.file.random_flame = true;
            ui.close();
        }

        if ui.button(t!("menu.random_batch")).clicked() {
            workspace.open_compact_panel(PanelType::RandomGenerator, ctx);
            ui.close();
        }

        if menu_state.online_mode && menu_state.auth_email.is_some() {
            let api_available = menu_state.api_connectivity == crate::api::ApiConnectivity::Online;

            if ui.add_enabled(api_available, egui::Button::new(t!("menu.save_online"))).clicked() {
                let api_flame_id = if menu_state.has_api_flame_id {
                    menu_state.api_flame_id.clone()
                } else {
                    None
                };
                save_online_dialog_state.open(&menu_state.flame_name, api_flame_id, menu_state.api_flame_is_public, menu_state.has_animation_tracks);
                workspace.open_compact_panel(PanelType::SaveOnlineDialog, ctx);
                ui.close();
            }
        }

        if ui.button(t!("menu.config_import_export")).clicked() {
            workspace.open_compact_panel(PanelType::ConfigDialog, ctx);
            ui.close();
        }
    });

    // --- View submenu ---
    ui.menu_button(t!("menu.view"), |ui| {
        if ui.button(t!("menu.reset_view")).clicked() {
            menu_actions.view.reset_view = true;
            ui.close();
        }

        let is_2d = menu_state.render_mode_2d;
        if ui.selectable_label(is_2d, t!("menu.mode_2d").as_ref()).clicked() {
            menu_actions.view.set_mode_2d = true;
            ui.close();
        }
        if ui.selectable_label(!is_2d, t!("menu.mode_3d").as_ref()).clicked() {
            menu_actions.view.set_mode_3d = true;
            ui.close();
        }
    });

    ui.separator();

    // --- Rendering ---
    let pause_text = if menu_state.is_paused { t!("menu.resume") } else { t!("menu.pause") };
    if ui.button(pause_text).clicked() {
        menu_actions.rendering.pause_toggle = true;
        ui.close();
    }

    if ui.button(t!("menu.reset_accumulation")).clicked() {
        menu_actions.rendering.reset_accumulation = true;
        ui.close();
    }

    ui.separator();

    // --- Edit ---
    if ui.add_enabled(menu_state.can_undo, egui::Button::new(t!("menu.undo"))).clicked() {
        menu_actions.edit.undo = true;
        ui.close();
    }
    if ui.add_enabled(menu_state.can_redo, egui::Button::new(t!("menu.redo"))).clicked() {
        menu_actions.edit.redo = true;
        ui.close();
    }

    ui.separator();

    // --- Account ---
    if menu_state.online_mode {
        if let Some(ref email) = menu_state.auth_email {
            if ui.small_button(email.as_str()).clicked() {
                workspace.open_compact_panel(PanelType::LoginDialog, ctx);
                ui.close();
            }
        } else {
            if ui.small_button(t!("auth.not_signed_in")).clicked() {
                workspace.open_compact_panel(PanelType::LoginDialog, ctx);
                ui.close();
            }
        }
    }

    // --- Help ---
    if ui.button(t!("menu.help_panel")).clicked() {
        workspace.open_compact_panel(PanelType::Help, ctx);
        ui.close();
    }

    // --- Layout switch ---
    ui.separator();
    if ui.button(t!("menu.desktop_view")).clicked() {
        workspace.apply_layout(super::workspace::WorkspaceLayout::Standard);
        ui.close();
    }
}
