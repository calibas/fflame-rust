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
    let bg_alpha = 200;
    let text_alpha = 255;

    let frame = egui::Frame::NONE
        .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, bg_alpha))
        .corner_radius(6.0)
        .inner_margin(egui::Margin { left: 8, right: 4, top: 4, bottom: 4 });

    egui::Area::new(egui::Id::new("compact_menu_button"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 16.0))
        .order(egui::Order::Tooltip)
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                let button_text = egui::RichText::new("\u{2630}") // hamburger icon
                    .size(23.0)
                    .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, text_alpha));

                let response = ui.add(
                    egui::Button::new(button_text)
                        .frame(false)
                        .min_size(egui::vec2(25.0, 25.0))
                );
                // Expand clickable area by 5px beyond the visible button
                let expanded = ui.interact(
                    response.rect.expand(18.0),
                    response.id.with("touch_padding"),
                    egui::Sense::click(),
                );
                let popup_id = ui.id().with("compact_menu_popup");

                if response.clicked() || expanded.clicked() {
                    egui::Popup::toggle_id(ui.ctx(), popup_id);
                }

                // `from_response` defaults to "always open"; switch to
                // memory-backed open state so the `toggle_id` call above
                // (and CloseOnClickOutside) actually control visibility.
                egui::Popup::from_response(&response)
                    .id(popup_id)
                    .open_memory(None)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
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
            (PanelType::SolidLighting, "menu.window_solid_lighting"),
            // Palette Editor / Palette Library deliberately absent: both
            // are reachable from the Colors panel, and on a phone this
            // menu must fit on screen — it was scrolling off the bottom.
            (PanelType::FractalBrowser, "menu.window_fractal_browser"),
            (PanelType::Variations, "menu.window_variations"),
            (PanelType::Subflames, "menu.window_subflames"),
            (PanelType::Scripts, "menu.window_scripts"),
            (PanelType::Escape, "menu.window_escape"),
            (PanelType::History, "menu.window_history"),
            (PanelType::Effects, "menu.window_effects"),
            (PanelType::XaosEditor, "menu.window_xaos_editor"),
            (PanelType::Animation, "menu.window_animation"),
            (PanelType::Signal, "menu.window_signal"),
            (PanelType::Performance, "menu.window_performance"),
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
    // Same row style as the Window submenu (full-width selectable rows,
    // not size-to-content buttons). Actions carry no open/closed state,
    // so their `selected` is always false; panel rows show theirs.
    ui.menu_button(t!("menu.file"), |ui| {
        if ui.selectable_label(false, t!("menu.new").as_ref()).clicked() {
            menu_actions.file.new_flame = true;
            ui.close();
        }

        // Opens the Fractal Browser docked (bottom in portrait), same
        // as every Window-menu panel — it IS a panel, the File entry is
        // just a shortcut that lands on the Presets tab.
        let browser_open = workspace.panel_exists(PanelType::FractalBrowser);
        if ui.selectable_label(browser_open, t!("menu.from_preset_library").as_ref()).clicked() {
            menu_actions.file.open_preset_library = true;
            workspace.open_compact_panel(PanelType::FractalBrowser, ctx);
            ui.close();
        }

        if ui.selectable_label(false, t!("menu.random_flame").as_ref()).clicked() {
            menu_actions.file.random_flame = true;
            ui.close();
        }

        let random_open = workspace.panel_exists(PanelType::RandomGenerator);
        if ui.selectable_label(random_open, t!("menu.random_batch").as_ref()).clicked() {
            workspace.open_compact_panel(PanelType::RandomGenerator, ctx);
            ui.close();
        }

        let export_open = workspace.panel_exists(PanelType::Export);
        if ui.selectable_label(export_open, t!("menu.export").as_ref()).clicked() {
            workspace.open_compact_panel(PanelType::Export, ctx);
            ui.close();
        }

        // Shown whenever online mode is on, signed in or not: hiding it
        // when signed out (the old gate) made the feature look absent —
        // the reported "Save Online missing from the mobile File menu".
        // Signed out, it routes to the login panel instead.
        if menu_state.online_mode {
            let signed_in = menu_state.auth_email.is_some();
            let api_available = menu_state.api_connectivity == crate::api::ApiConnectivity::Online;

            if ui
                .add_enabled(
                    !signed_in || api_available,
                    egui::SelectableLabel::new(false, t!("menu.save_online").as_ref()),
                )
                .clicked()
            {
                if !signed_in {
                    workspace.open_compact_panel(PanelType::LoginDialog, ctx);
                    ui.close();
                    return;
                }
                let api_flame_id = if menu_state.has_api_flame_id {
                    menu_state.api_flame_id.clone()
                } else {
                    None
                };
                save_online_dialog_state.open(
                    &menu_state.flame_name,
                    api_flame_id,
                    menu_state.api_animation_id.clone(),
                    menu_state.api_flame_is_public,
                    menu_state.has_animation_tracks,
                    menu_state.animation_count,
                    menu_state.flame_owned,
                    menu_state.animation_owned,
                );
                workspace.open_compact_panel(PanelType::SaveOnlineDialog, ctx);
                ui.close();
            }
        }

        let config_open = workspace.panel_exists(PanelType::ConfigDialog);
        if ui.selectable_label(config_open, t!("menu.config_import_export").as_ref()).clicked() {
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

    // --- Animation transport ---
    // Play/pause/stop drive the ANIMATION, not the renderer. These used
    // to toggle the render pause, which on a phone reads as "the app
    // froze"; a transport row means the animation everywhere else in
    // the app (animation panel, Space key), so it does here too.
    let play_text = if menu_state.animation_playing {
        t!("menu.pause_animation")
    } else {
        t!("menu.play_animation")
    };
    let can_play = menu_state.animation_playing || menu_state.has_animation_tracks;
    if ui.add_enabled(can_play, egui::Button::new(play_text.as_ref())).clicked() {
        menu_actions.animation.play_pause = true;
        ui.close();
    }
    if ui
        .add_enabled(menu_state.animation_playing, egui::Button::new(t!("menu.stop_animation").as_ref()))
        .clicked()
    {
        menu_actions.animation.stop = true;
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
