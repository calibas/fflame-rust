use super::menu_context::{MenuActions, MenuState};
use crate::scene::transforms::RenderMode;
use rust_i18n::t;

/// Stop menu rows wrapping.
///
/// egui lays a menu out top-down JUSTIFIED, and the default wrap mode
/// is "follow the layout", so a row wider than the popup's current
/// width is broken across two lines rather than widening the popup.
/// The Window menu has 23 rows; wrapped, it runs off the bottom of the
/// screen. `Extend` makes each row report its true width, so the popup
/// sizes to its longest row and flips to the left of its button when
/// there is no room to the right -- which is what started happening to
/// Window once a Mode menu pushed it further along the bar.
pub(super) fn rows_do_not_wrap(ui: &mut egui::Ui) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
}

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
                rows_do_not_wrap(ui);
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

                ui.separator();

                // The one switch for everything that talks to the online
                // service (ONLINE.md, Online mode). It had no control:
                // the setting changed only by editing the settings file.
                let mut online = menu_state.online_mode;
                if ui
                    .checkbox(&mut online, t!("menu.online_features").as_ref())
                    .on_hover_text(t!("menu.tooltip_online_features").as_ref())
                    .changed()
                {
                    menu_actions.file.set_online_mode = Some(online);
                }

                // Present whenever online mode is on, signed in or not —
                // hiding it while signed out makes the feature look
                // absent. Signed out it opens the login panel instead.
                if menu_state.online_mode {
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
                rows_do_not_wrap(ui);
                if ui.add_enabled(menu_state.can_undo, egui::Button::new(t!("menu.undo"))).clicked() {
                    menu_actions.edit.undo = true;
                }

                if ui.add_enabled(menu_state.can_redo, egui::Button::new(t!("menu.redo"))).clicked() {
                    menu_actions.edit.redo = true;
                }

                ui.separator();

                ui.add_enabled(false, egui::Button::new(t!("menu.preferences")));
            });

            // View Menu -- all three rows move a view, which
            // Simulation does not have, so the whole menu goes there.
            if super::visibility::control(
                super::visibility::Control::ViewNavigation,
                menu_state.render_mode,
                menu_state.tonemap_mode,
            ) != super::visibility::Vis::Hide
            {
                ui.menu_button(t!("menu.view"), |ui| {
                    rows_do_not_wrap(ui);
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
            }

            // Rendering Menu -- the viewport's render scale, the quality
            // governor (every mode's), then the chaos game's rows (pause,
            // reset accumulation, iterations per thread, workgroups), and
            // "Reset to Defaults" resets exactly those parameters. Always
            // there: a menu that came and went with the mode moved the
            // mode button after it. Outside the flame modes it says why the
            // chaos game's rows are disabled.
            let chaos_game = super::visibility::control(
                super::visibility::Control::ChaosGame,
                menu_state.render_mode,
                menu_state.tonemap_mode,
            ) == super::visibility::Vis::Show;
            ui.menu_button(t!("menu.rendering"), |ui| {
                rows_do_not_wrap(ui);
                // The viewport's render scale and upscaler
                // (`renderer::upscale`): every mode's but a simulation's,
                // so above the chaos game's rows.
                let scaling = super::visibility::control(
                    super::visibility::Control::RenderScale,
                    menu_state.render_mode,
                    menu_state.tonemap_mode,
                );
                ui.add_enabled_ui(scaling.is_show(), |ui| {
                    let percent = |s: f32| format!("{}%", (s * 100.0).round());
                    ui.menu_button(format!("{}: {}", t!("menu.render_scale"), percent(menu_state.render_scale)), |ui| {
                        rows_do_not_wrap(ui);
                        for &s in &crate::renderer::upscale::RENDER_SCALES {
                            if ui.radio((menu_state.render_scale - s).abs() < 1e-3, percent(s)).clicked() {
                                menu_actions.rendering.set_render_scale = Some(s);
                            }
                        }
                    })
                    .response
                    .on_hover_text(t!("menu.tooltip_render_scale"));
                    ui.menu_button(format!("{}: {}", t!("menu.upscaler"), t!(menu_state.upscaler.label_key())), |ui| {
                        rows_do_not_wrap(ui);
                        for f in crate::renderer::upscale::UpscaleFilter::ALL {
                            if ui.radio(menu_state.upscaler == f, t!(f.label_key())).clicked() {
                                menu_actions.rendering.set_upscaler = Some(f);
                            }
                        }
                    })
                    .response
                    .on_hover_text(t!("menu.tooltip_upscaler"));
                    let mut physical = menu_state.render_physical_pixels;
                    if ui
                        .checkbox(&mut physical, t!("menu.render_physical_pixels"))
                        .on_hover_text(t!("menu.tooltip_render_physical_pixels"))
                        .changed()
                    {
                        menu_actions.rendering.set_render_physical_pixels = Some(physical);
                    }
                });
                if let super::visibility::Vis::Grey(why) = scaling {
                    ui.weak(t!(why));
                }
                // The quality governor, on or off in every mode
                // (app/governor.rs and the flame's batch governor), and the
                // frame rate it holds while the picture moves (`TargetRate`).
                let mut governor = menu_state.frame_governor;
                if ui
                    .checkbox(&mut governor, t!("menu.frame_governor"))
                    .on_hover_text(t!("menu.tooltip_frame_governor"))
                    .changed()
                {
                    menu_actions.rendering.set_frame_governor = Some(governor);
                }
                ui.add_enabled_ui(menu_state.frame_governor, |ui| {
                    use crate::storage::settings::TargetRate;
                    let label = |r: TargetRate| match r {
                        TargetRate::Display => t!("menu.target_rate_display").to_string(),
                        TargetRate::Animation => t!("menu.target_rate_animation", fps = menu_state.export_fps).to_string(),
                        TargetRate::Fixed(f) => format!("{f:.0}"),
                    };
                    ui.menu_button(
                        format!("{}: {} ({:.0} fps)", t!("menu.target_rate"), label(menu_state.target_rate), menu_state.target_fps_now),
                        |ui| {
                            rows_do_not_wrap(ui);
                            let rates = [TargetRate::Display, TargetRate::Animation]
                                .into_iter()
                                .chain(TargetRate::FIXED.iter().map(|f| TargetRate::Fixed(*f)));
                            for r in rates {
                                if ui.radio(menu_state.target_rate == r, label(r)).clicked() {
                                    menu_actions.rendering.set_target_rate = Some(r);
                                }
                            }
                        },
                    )
                    .response
                    .on_hover_text(t!("menu.tooltip_target_rate"));
                });
                ui.separator();
                if !chaos_game {
                    ui.weak(t!(super::visibility::CHAOS_GAME));
                    ui.separator();
                }
                ui.add_enabled_ui(chaos_game, |ui| {
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
                        rows_do_not_wrap(ui);
                        for &ipt in &[128, 256, 512, 1024, 2048, 4096] {
                            if ui.button(format!("{}", ipt)).clicked() {
                                menu_actions.rendering.set_iterations_per_thread = Some(ipt);
                            }
                        }
                    });

                    // Reset Orbits Each Dispatch: the flame's; a variation
                    // that needs it holds it on.
                    let forced_by = &menu_state.reset_orbits_forced_by;
                    let mut reset = menu_state.reset_orbits || forced_by.is_some();
                    let tip = super::settings::reset_orbits_tip(forced_by.as_deref(), menu_state.orbit_length_source.as_ref());
                    if ui
                        .add_enabled(forced_by.is_none(), egui::Checkbox::new(&mut reset, t!("settings.reset_orbits").as_ref()))
                        .on_hover_text(tip.as_ref())
                        .on_disabled_hover_text(tip.as_ref())
                        .changed()
                    {
                        menu_actions.rendering.set_reset_orbits = Some(reset);
                    }

                    // The dispatch while the governor is off
                    ui.add_enabled_ui(!menu_state.frame_governor, |ui| {
                        ui.menu_button(format!("{}: {}", t!("menu.workgroups"), menu_state.manual_workgroups), |ui| {
                            rows_do_not_wrap(ui);
                            for &w in &[1u32, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] {
                                if ui.radio(menu_state.manual_workgroups == w, format!("{w}")).clicked() {
                                    menu_actions.rendering.set_manual_workgroups = Some(w);
                                }
                            }
                        })
                        .response
                        .on_hover_text(t!("menu.tooltip_workgroups"));
                    });

                    ui.separator();

                    // Reset Rendering to Defaults
                    if ui.button(t!("menu.reset_rendering")).clicked() {
                        menu_actions.rendering.reset_to_defaults = true;
                    }
                });
            });

            // Windows Menu
            ui.menu_button(t!("menu.window"), |ui| {
                rows_do_not_wrap(ui);
                // Reset Workspace: the layout up, built fresh, the mode's
                // editor in front (`Workspace::show_main_panel`)
                if ui.button(t!("menu.reset_workspace")).clicked() {
                    let layout = match workspace.current_layout {
                        super::workspace::WorkspaceLayout::Compact => super::workspace::WorkspaceLayout::Standard,
                        other => other,
                    };
                    workspace.apply_layout(layout);
                }

                if ui.button(t!("menu.layout_compact")).clicked() {
                    workspace.apply_layout(super::workspace::WorkspaceLayout::Compact);
                }

                ui.separator();

                // Every row comes from the shared table, and opens its
                // panel in every mode (one-workspace plan): one the mode
                // does not use says so on hover, and at its own top.
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
                    let mut r = ui.selectable_label(open, label.as_ref());
                    if let super::visibility::PanelUse::Inactive { reason, .. } = super::visibility::panel(row.panel, mode, menu_state.solid) {
                        r = r.on_hover_text(t!(reason));
                    }
                    if r.clicked() {
                        workspace.open_floating_panel(row.panel, ctx);
                    }
                }

                ui.separator();
                ui.menu_button(t!("menu.workspace_layout"), |ui| {
                    rows_do_not_wrap(ui);
                    let current = workspace.current_layout;

                    // if ui.selectable_label(current == super::workspace::WorkspaceLayout::Beginner, t!("menu.layout_beginner").as_ref()).clicked() {
                    //     workspace.apply_layout(super::workspace::WorkspaceLayout::Beginner);
                    // }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Standard, t!("menu.layout_standard").as_ref()).clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Standard);
                    }
                    // Each engine's own arrangement. Picked here, never by
                    // a mode change: a switch in one brings the new mode's
                    // editor in beside the one there.
                    for (layout, key) in [
                        (super::workspace::WorkspaceLayout::Flames, "menu.layout_flames"),
                        (super::workspace::WorkspaceLayout::EscapeTime, "menu.layout_escape"),
                        (super::workspace::WorkspaceLayout::Simulation, "menu.layout_simulation"),
                    ] {
                        if ui.selectable_label(current == layout, t!(key).as_ref()).clicked() {
                            workspace.apply_layout(layout);
                        }
                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Animation, t!("menu.layout_animation").as_ref()).clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Animation);
                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Scripting, t!("menu.layout_scripting").as_ref()).clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Scripting);
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
                rows_do_not_wrap(ui);
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

            // The render mode, in the bar itself (one-workspace plan,
            // decision 5): a framed button naming the current mode,
            // whose menu lists the four engines as peers, the active one
            // selected, each with what it renders on hover. A switch
            // brings the mode's editor forward and closes nothing.
            ui.separator();
            let current = t!(super::render_mode::mode_label_key(menu_state.render_mode));
            // Outlined as the Fly Mode button is: the bar's own style
            // draws no frame, and without one it reads as a menu title.
            let button = egui::Button::new(format!("{current} ⏷")).stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(110)));
            let (response, _) = egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
                rows_do_not_wrap(ui);
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
            response.on_hover_text(t!("menu.mode_tip"));

            // Push right-side controls (right-to-left: language, then auth status)
            // Hide when window is too narrow to prevent overlap with left-side menus.
            let available_width = ui.ctx().content_rect().width();
            if available_width >= 500.0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // The progress bar for whatever is rendering -- rightmost,
                // and always there, so nothing beside it moves when a
                // render starts or ends (render_progress.rs).
                let row = ui.spacing().interact_size.y;
                super::render_progress::show(ui, &menu_state.progress, egui::vec2(96.0, row));
                ui.separator();

                // Language selector menu (globe icon)
                ui.menu_button("🌐", |ui| {
                    rows_do_not_wrap(ui);
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
                // Fly mode is every 3D camera's — greyed in a 2D view.
                let fly_enabled = menu_state.fly_available;
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
