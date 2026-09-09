use super::menu_context::{MenuActions, MenuState};
use crate::scene::transforms::RenderMode;
use rust_i18n::t;

/// Render the top menu bar with window visibility toggles
pub fn render_menu_bar(
    ctx: &egui::Context,
    workspace: &mut super::workspace::Workspace,
    menu_actions: &mut MenuActions,
    menu_state: &MenuState,
    save_online_dialog_state: &mut super::save_online_dialog::SaveOnlineDialogState,
    #[cfg(not(target_arch = "wasm32"))]
    window: &winit::window::Window,
) {
    #[allow(deprecated)]
    egui::Panel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            // File Menu — same row style as the Window menu (full-width
            // selectable rows). Pure actions have no open/closed state,
            // so their `selected` is always false; entries that open a
            // panel show that panel's state like Window entries do.
            ui.menu_button(t!("menu.file"), |ui| {
                if ui.selectable_label(false, t!("menu.new").as_ref()).clicked() {
                    menu_actions.file.new_flame = true;
                }

                if ui.selectable_label(false, t!("menu.open").as_ref()).clicked() {
                    menu_actions.file.load_config = true;
                }

                if ui.selectable_label(false, t!("menu.save_as").as_ref()).clicked() {
                    menu_actions.file.save_config = true;
                }

                if ui.selectable_label(false, t!("menu.export_flame_xml").as_ref()).clicked() {
                    menu_actions.file.export_flame_xml = true;
                }

                ui.separator();

                // Fractal Browser (presets, batch results, files)
                let browser_open = workspace.panel_exists(super::workspace::PanelType::FractalBrowser);
                if ui.selectable_label(browser_open, t!("menu.from_preset_library").as_ref()).clicked() {
                    menu_actions.file.open_preset_library = true;
                }

                if ui.selectable_label(false, t!("menu.random_flame").as_ref()).clicked() {
                    menu_actions.file.random_flame = true;
                }

                let random_open = workspace.panel_exists(super::workspace::PanelType::RandomGenerator);
                if ui.selectable_label(random_open, t!("menu.random_batch").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::RandomGenerator, ctx);
                }

                // Present whenever online mode is on, signed in or not —
                // hiding it while signed out makes the feature look
                // absent. Signed out it opens the login panel instead.
                if menu_state.online_mode {
                    ui.separator();

                    let signed_in = menu_state.auth_email.is_some();
                    let api_available = menu_state.api_connectivity == crate::api::ApiConnectivity::Online;

                    if ui.add_enabled(!signed_in || api_available, egui::SelectableLabel::new(false, t!("menu.save_online").as_ref())).clicked() {
                      if !signed_in {
                        workspace.open_floating_panel(super::workspace::PanelType::LoginDialog, ctx);
                      } else {
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
                        workspace.open_floating_panel(super::workspace::PanelType::SaveOnlineDialog, ctx);
                      }
                    }
                }

                ui.separator();

                let export_open = workspace.panel_exists(super::workspace::PanelType::Export);
                if ui.selectable_label(export_open, t!("menu.export").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Export, ctx);
                }

                ui.separator();

                let config_open = workspace.panel_exists(super::workspace::PanelType::ConfigDialog);
                if ui.selectable_label(config_open, t!("menu.config_import_export").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::ConfigDialog, ctx);
                }


                ui.separator();

                if ui.selectable_label(false, t!("menu.quit").as_ref()).clicked() {
                    menu_actions.file.quit = true;
                }
            });

            // Edit Menu
            ui.menu_button(t!("menu.edit"), |ui| {
                if ui.add_enabled(menu_state.can_undo, egui::Button::new(t!("menu.undo"))).clicked() {
                    menu_actions.edit.undo = true;
                }

                if ui.add_enabled(menu_state.can_redo, egui::Button::new(t!("menu.redo"))).clicked() {
                    menu_actions.edit.redo = true;
                }

                ui.separator();

                ui.add_enabled(false, egui::Button::new(t!("menu.preferences")));
            });

            // View Menu
            ui.menu_button(t!("menu.view"), |ui| {
                if ui.button(t!("menu.reset_view")).clicked() {
                    menu_actions.view.reset_view = true;
                }

                ui.separator();

                if ui.button(t!("menu.zoom_in")).clicked() {
                    menu_actions.view.zoom_in = true;
                }

                if ui.button(t!("menu.zoom_out")).clicked() {
                    menu_actions.view.zoom_out = true;
                }

            });

            // Mode Menu — the four engines as peers. The View menu used
            // to carry a 2D/3D pair, which could not name the other two
            // and claimed "3D" was selected while you were in Escape.
            // The View PANEL keeps its 2D/3D switch: choosing between
            // the flame's two projections is a view-level decision and
            // belongs next to the camera.
            ui.menu_button(t!("menu.mode"), |ui| {
                for m in RenderMode::ALL {
                    let label = t!(super::render_mode::mode_label_key(*m));
                    if ui
                        .selectable_label(menu_state.render_mode == *m, label.as_ref())
                        .on_hover_text(t!(super::render_mode::mode_tip_key(*m)))
                        .clicked()
                    {
                        menu_actions.set_mode = Some(*m);
                    }
                }
            });

            // Rendering Menu
            ui.menu_button(t!("menu.rendering"), |ui| {
                // Pause/Resume
                let pause_text = if menu_state.is_paused {
                    t!("menu.resume")
                } else {
                    t!("menu.pause")
                };
                if ui.button(pause_text).clicked() {
                    menu_actions.rendering.pause_toggle = true;
                }

                // Reset Accumulation
                if ui.button(t!("menu.reset_accumulation")).clicked() {
                    menu_actions.rendering.reset_accumulation = true;
                }

                ui.separator();

                // Iterations per Thread submenu
                ui.menu_button(t!("menu.iterations_per_thread"), |ui| {
                    for &ipt in &[128, 256, 512, 1024, 2048, 4096] {
                        if ui.button(format!("{}", ipt)).clicked() {
                            menu_actions.rendering.set_iterations_per_thread = Some(ipt);
                        }
                    }
                });

                ui.separator();

                // Reset Rendering to Defaults
                if ui.button(t!("menu.reset_rendering")).clicked() {
                    menu_actions.rendering.reset_to_defaults = true;
                }
            });

            // Windows Menu
            ui.menu_button(t!("menu.window"), |ui| {
                // Reset Workspace to Standard layout
                if ui.button(t!("menu.reset_workspace")).clicked() {
                    workspace.apply_layout(super::workspace::WorkspaceLayout::Standard);
                }

                if ui.button(t!("menu.layout_compact")).clicked() {
                    workspace.apply_layout(super::workspace::WorkspaceLayout::Compact);
                }

                ui.separator();

                // Every row comes from the shared table, and asks
                // the visibility policy whether it means anything in
                // this mode. Before this, the menu offered all 22
                // unconditionally -- in Escape, eight of them opened
                // onto a stub with no warning in the menu itself.
                let mode = menu_state.render_mode;
                for row in super::visibility::WINDOW_MENU {
                    if row.online_only {
                        if !menu_state.online_mode {
                            continue;
                        }
                        ui.separator();
                    }
                    let open = workspace.panel_exists(row.panel);
                    let label = t!(row.label_key);
                    match super::visibility::panel(row.panel, mode) {
                        super::visibility::Vis::Show => {
                            if ui.selectable_label(open, label.as_ref()).clicked() {
                                workspace.open_floating_panel(row.panel, ctx);
                            }
                        }
                        super::visibility::Vis::Grey(reason) => {
                            ui.add_enabled(false, egui::Button::new(label.as_ref()))
                                .on_disabled_hover_text(t!(reason));
                        }
                        super::visibility::Vis::Hide => {}
                    }
                }

                ui.separator();
                ui.menu_button(t!("menu.workspace_layout"), |ui| {
                    let current = workspace.current_layout;

                    // if ui.selectable_label(current == super::workspace::WorkspaceLayout::Beginner, t!("menu.layout_beginner").as_ref()).clicked() {
                    //     workspace.apply_layout(super::workspace::WorkspaceLayout::Beginner);
                    // }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Standard, t!("menu.layout_standard").as_ref()).clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Standard);
                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Animation, t!("menu.layout_animation").as_ref()).clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Animation);
                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Scripting, t!("menu.layout_scripting").as_ref()).clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Scripting);
                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::EscapeTime, t!("menu.layout_escape").as_ref()).clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::EscapeTime);
                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Simulation, t!("menu.layout_simulation").as_ref()).clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Simulation);
                    }
                    // if ui.selectable_label(current == super::workspace::WorkspaceLayout::Advanced, t!("menu.layout_advanced").as_ref()).clicked() {
                    //     workspace.apply_layout(super::workspace::WorkspaceLayout::Advanced);
                    // }
                    // if ui.selectable_label(current == super::workspace::WorkspaceLayout::Export, t!("menu.layout_export").as_ref()).clicked() {
                    //     workspace.apply_layout(super::workspace::WorkspaceLayout::Export);
                    // }
                });
            });

            // Help Menu
            ui.menu_button(t!("menu.help"), |ui| {
                // Help panel opens as floating window in docking system
                let help_open = workspace.panel_exists(super::workspace::PanelType::Help);
                if ui.selectable_label(help_open, t!("menu.help_panel").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Help, ctx);
                }

                let shortcuts_open = workspace.panel_exists(super::workspace::PanelType::KeyboardShortcuts);
                if ui.selectable_label(shortcuts_open, t!("menu.keyboard_shortcuts").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::KeyboardShortcuts, ctx);
                }

                ui.separator();

                if ui.button(t!("menu.report_bug")).clicked() {
                    let _ = webbrowser::open("https://github.com/calibas/fflame-rust/issues/new");
                }

                if ui.button(t!("menu.about")).clicked() {
                    let _ = webbrowser::open("https://github.com/calibas/fflame-rust");
                }
            });

            // Push right-side controls (right-to-left: language, then auth status)
            // Hide when window is too narrow to prevent overlap with left-side menus.
            let available_width = ui.ctx().content_rect().width();
            if available_width >= 500.0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Language selector menu (globe icon) — rightmost
                ui.menu_button("🌐", |ui| {
                    let locales = crate::i18n::supported_locales();
                    let current_locale = crate::i18n::current_locale();

                    for locale in &locales {
                        if ui.selectable_label(
                            current_locale == locale.code,
                            locale.display_text()
                        ).clicked() {
                            // Try to load font for this locale
                            let font_loaded = crate::ui::ensure_font_for_locale(ui.ctx(), locale.code);

                            if font_loaded {
                                // Font loaded or default font is sufficient
                                crate::i18n::set_locale(locale.code);
                                log::info!("Language changed to: {} ({})", locale.name, locale.code);

                            } else {
                                // Font required but not available - reset to English
                                log::warn!("Font required for {} not found - resetting to English", locale.code);
                                crate::i18n::set_locale("en");

                                // Show error message to user
                                #[cfg(not(target_arch = "wasm32"))]
                                {
                                    rfd::MessageDialog::new()
                                        .set_parent(window)
                                        .set_title("Font Required")
                                        .set_description(&format!(
                                            "The {} language requires additional fonts that are not installed.\n\n\
                                            Please download the required font file and place it in:\n\
                                            assets/fonts/\n\n\
                                            See docs/main/I18N.md for details.\n\n\
                                            Falling back to English.",
                                            locale.name
                                        ))
                                        .set_level(rfd::MessageLevel::Warning)
                                        .show();
                                }

                                #[cfg(target_arch = "wasm32")]
                                log::error!("CJK fonts not embedded in WASM build - use English");


                            }
                        }
                    }
                });

                // Auth status display (left of language selector)
                if menu_state.online_mode {
                    ui.separator();

                    // Connectivity indicator
                    match menu_state.api_connectivity {
                        crate::api::ApiConnectivity::Unreachable => {
                            ui.colored_label(
                                egui::Color32::from_rgb(220, 160, 60),
                                t!("auth.offline"),
                            );
                        }
                        crate::api::ApiConnectivity::Unknown if menu_state.auth_email.is_some() => {
                            ui.weak(t!("auth.checking"));
                        }
                        _ => {}
                    }

                    if let Some(ref email) = menu_state.auth_email {
                        if ui.small_button(email.as_str()).clicked() {
                            workspace.open_floating_panel(super::workspace::PanelType::LoginDialog, ctx);
                        }
                    } else {
                        if ui.small_button(t!("auth.not_signed_in")).clicked() {
                            workspace.open_floating_panel(super::workspace::PanelType::LoginDialog, ctx);
                        }
                    }
                }

                // Fly Mode toggle — floats right near the account. A grey
                // outline when off; the outline AND text turn green when on
                // (transparent fill either way, so it reads as an outline).
                let (fly_color, fly_text) = if menu_state.fly_mode_active {
                    let green = egui::Color32::from_rgb(60, 180, 75);
                    (green, egui::RichText::new(t!("menu.fly_mode")).color(green))
                } else {
                    (egui::Color32::from_gray(110), egui::RichText::new(t!("menu.fly_mode")))
                };
                let fly_button = egui::Button::new(fly_text)
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(1.0, fly_color))
                    .gap(2.0);
                ui.style_mut().spacing.button_padding = egui::vec2(5.0, 0.0);
                // Fly mode is 3D-only — disabled (greyed) in 2D.
                let fly_enabled = super::render_mode::fly_mode_available(menu_state.render_mode);
                let resp = ui.add_enabled(fly_enabled, fly_button);
                let resp = if fly_enabled {
                    resp.on_hover_text(t!("view.tooltip_fly_mode"))
                } else {
                    resp.on_disabled_hover_text(t!("menu.fly_mode_requires_3d"))
                };
                if resp.clicked() {
                    menu_actions.fly_mode_toggle = true;
                }
            });
            } // available_width >= 500.0
        });
    });
}
