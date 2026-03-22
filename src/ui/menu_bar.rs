use super::menu_context::{MenuActions, MenuState};
use rust_i18n::t;

/// Render the top menu bar with window visibility toggles
pub fn render_menu_bar(
    ctx: &egui::Context,
    workspace: &mut super::workspace::Workspace,
    menu_actions: &mut MenuActions,
    menu_state: &MenuState,
    save_online_dialog_state: &mut super::save_online_dialog::SaveOnlineDialogState,
) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            // File Menu
            ui.menu_button(t!("menu.file"), |ui| {
                if ui.button(t!("menu.new")).clicked() {
                    menu_actions.file.new_flame = true;
                }

                if ui.button(t!("menu.open")).clicked() {
                    menu_actions.file.load_config = true;
                }

                if ui.button(t!("menu.save_as")).clicked() {
                    menu_actions.file.save_config = true;
                }

                ui.separator();

                // Fractal Browser (presets, batch results, files)
                if ui.button(t!("menu.from_preset_library")).clicked() {
                    menu_actions.file.open_preset_library = true;
                }

                if ui.button(t!("menu.random_flame")).clicked() {
                    menu_actions.file.random_flame = true;
                }

                if ui.button(t!("menu.random_batch")).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::RandomGenerator, ctx);
                }

                if menu_state.online_mode && menu_state.auth_email.is_some() {
                    ui.separator();

                    let api_available = menu_state.api_connectivity == crate::api::ApiConnectivity::Online;

                    if ui.add_enabled(api_available, egui::Button::new(t!("menu.save_online"))).clicked() {
                        let api_flame_id = if menu_state.has_api_flame_id {
                            menu_state.api_flame_id.clone()
                        } else {
                            None
                        };
                        save_online_dialog_state.open(&menu_state.flame_name, api_flame_id, menu_state.api_flame_is_public, menu_state.has_animation_tracks);
                        workspace.open_floating_panel(super::workspace::PanelType::SaveOnlineDialog, ctx);
                    }
                }

                ui.separator();

                if ui.button(t!("menu.export")).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Export, ctx);
                }

                ui.separator();

                if ui.button(t!("menu.import_apophysis")).clicked() {
                    menu_actions.file.import_apophysis = true;
                }
                // ui.add_enabled(false, egui::Button::new(t!("menu.export_apophysis")));

                ui.separator();

                if ui.button(t!("menu.config_import_export")).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::ConfigDialog, ctx);
                }


                ui.separator();

                if ui.button(t!("menu.quit")).clicked() {
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

                ui.separator();

                // Radio buttons for render mode
                let is_2d = menu_state.render_mode_2d;
                if ui.selectable_label(is_2d, t!("menu.mode_2d").as_ref()).clicked() {
                    menu_actions.view.set_mode_2d = true;
                }

                if ui.selectable_label(!is_2d, t!("menu.mode_3d").as_ref()).clicked() {
                    menu_actions.view.set_mode_3d = true;
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

                ui.separator();

                // Performance opens as floating window in docking system (only one instance)
                let performance_open = workspace.panel_exists(super::workspace::PanelType::Performance);
                if ui.selectable_label(performance_open, t!("menu.window_performance").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Performance, ctx);
                }

                // Settings opens Rendering panel as floating window (Settings was renamed to Rendering)
                let rendering_open = workspace.panel_exists(super::workspace::PanelType::Rendering);
                if ui.selectable_label(rendering_open, t!("menu.window_rendering").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Rendering, ctx);
                }

                // View opens as floating window in docking system
                let view_open = workspace.panel_exists(super::workspace::PanelType::View);
                if ui.selectable_label(view_open, t!("menu.window_view").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::View, ctx);
                }

                // Transforms opens as floating window in docking system
                let transforms_open = workspace.panel_exists(super::workspace::PanelType::Transforms);
                if ui.selectable_label(transforms_open, t!("menu.window_transforms").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Transforms, ctx);
                }

                // Triangle Editor opens as floating window in docking system
                let triangle_editor_open = workspace.panel_exists(super::workspace::PanelType::TriangleEditor);
                if ui.selectable_label(triangle_editor_open, t!("menu.window_triangle_editor").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::TriangleEditor, ctx);
                }

                // Tone Mapping & Colors opens Colors panel as floating window
                let colors_open = workspace.panel_exists(super::workspace::PanelType::Colors);
                if ui.selectable_label(colors_open, t!("menu.window_colors").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Colors, ctx);
                }

                ui.separator();

                // Palette Editor opens as floating window in docking system
                let palette_editor_open = workspace.panel_exists(super::workspace::PanelType::PaletteEditor);
                if ui.selectable_label(palette_editor_open, t!("menu.window_palette_editor").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::PaletteEditor, ctx);
                }

                let palette_library_open = workspace.panel_exists(super::workspace::PanelType::PaletteLibrary);
                if ui.selectable_label(palette_library_open, t!("menu.window_palette_library").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::PaletteLibrary, ctx);
                }

                let fractal_browser_open = workspace.panel_exists(super::workspace::PanelType::FractalBrowser);
                if ui.selectable_label(fractal_browser_open, t!("menu.window_fractal_browser").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::FractalBrowser, ctx);
                }

                // Config Import/Export opens as floating window in docking system
                let config_dialog_open = workspace.panel_exists(super::workspace::PanelType::ConfigDialog);
                if ui.selectable_label(config_dialog_open, t!("menu.window_config_dialog").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::ConfigDialog, ctx);
                }

                // Undo/Redo History opens as floating window in docking system
                let history_open = workspace.panel_exists(super::workspace::PanelType::History);
                if ui.selectable_label(history_open, t!("menu.window_history").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::History, ctx);
                }

                // Animation panel
                let animation_open = workspace.panel_exists(super::workspace::PanelType::Animation);
                if ui.selectable_label(animation_open, t!("menu.window_animation").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Animation, ctx);
                }

                // Path Editor panel (experimental feature)
                let path_editor_open = workspace.panel_exists(super::workspace::PanelType::PathEditor);
                if ui.selectable_label(path_editor_open, t!("menu.window_path_editor").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::PathEditor, ctx);
                }

                // Random Generator panel
                let random_generator_open = workspace.panel_exists(super::workspace::PanelType::RandomGenerator);
                if ui.selectable_label(random_generator_open, t!("menu.window_random_generator").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::RandomGenerator, ctx);
                }

                // Effects panel
                let effects_open = workspace.panel_exists(super::workspace::PanelType::Effects);
                if ui.selectable_label(effects_open, t!("menu.window_effects").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Effects, ctx);
                }

                // Xaos Editor panel
                let xaos_editor_open = workspace.panel_exists(super::workspace::PanelType::XaosEditor);
                if ui.selectable_label(xaos_editor_open, t!("menu.window_xaos_editor").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::XaosEditor, ctx);
                }

                // Signal panel
                let signal_open = workspace.panel_exists(super::workspace::PanelType::Signal);
                if ui.selectable_label(signal_open, t!("menu.window_signal").as_ref()).clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Signal, ctx);
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
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Compact, t!("menu.layout_compact").as_ref()).clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Compact);
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
            });
        });
    });
}
