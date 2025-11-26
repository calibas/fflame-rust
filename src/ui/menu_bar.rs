use super::menu_context::{MenuActions, MenuState};
use rust_i18n::t;

/// Render the top menu bar with window visibility toggles
pub fn render_menu_bar(
    ctx: &egui::Context,
    workspace: &mut super::workspace::Workspace,
    menu_actions: &mut MenuActions,
    menu_state: &MenuState,
) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            // File Menu
            ui.menu_button(t!("menu.file"), |ui| {
                if ui.button("📂 Open...").clicked() {
                    menu_actions.file.load_config = true;
                }

                if ui.button("💾 Save As...").clicked() {
                    menu_actions.file.save_config = true;
                }

                ui.separator();

                // Preset Library
                if ui.button("📚 From Preset Library...").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::PresetLibrary);
                }

                ui.separator();

                if ui.button("Import Apophysis XML...").clicked() {
                    menu_actions.file.import_apophysis = true;

                }
                ui.add_enabled(false, egui::Button::new("Export Apophysis XML..."));

                ui.separator();

                if ui.button("📄 Config Import/Export...").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::ConfigDialog);

                }

                ui.separator();

                if ui.button("🖼 Export PNG...").clicked() {
                    menu_actions.file.export_png = true;

                }

                if ui.button("🖼 Export Transparent PNG...").clicked() {
                    menu_actions.file.export_png_transparent = true;

                }

                ui.separator();

                if ui.button("❌ Quit").clicked() {
                    menu_actions.file.quit = true;

                }
            });

            // Edit Menu
            ui.menu_button(t!("menu.edit"), |ui| {
                if ui.add_enabled(menu_state.can_undo, egui::Button::new("⮪ Undo")).clicked() {
                    menu_actions.edit.undo = true;
                }

                if ui.add_enabled(menu_state.can_redo, egui::Button::new("⮬ Redo")).clicked() {
                    menu_actions.edit.redo = true;
                }

                ui.separator();

                ui.separator();

                ui.add_enabled(false, egui::Button::new("⚙ Preferences..."));
            });

            // View Menu
            ui.menu_button(t!("menu.view"), |ui| {
                if ui.button("Reset View").clicked() {
                    menu_actions.view.reset_view = true;

                }

                ui.separator();

                if ui.button("Zoom In").clicked() {
                    menu_actions.view.zoom_in = true;

                }

                if ui.button("Zoom Out").clicked() {
                    menu_actions.view.zoom_out = true;

                }

                ui.separator();

                // Radio buttons for render mode
                let is_2d = menu_state.render_mode_2d;
                if ui.selectable_label(is_2d, "2D Mode").clicked() {
                    menu_actions.view.set_mode_2d = true;

                }

                if ui.selectable_label(!is_2d, "3D Mode").clicked() {
                    menu_actions.view.set_mode_3d = true;

                }
            });

            // Transforms Menu
            ui.menu_button("Transforms", |ui| {
                // Panel visibility toggles
                let transforms_open = workspace.panel_exists(super::workspace::PanelType::Transforms);
                if ui.selectable_label(transforms_open, "Show Transform Editor").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Transforms);

                }
                let triangle_editor_open = workspace.panel_exists(super::workspace::PanelType::TriangleEditor);
                if ui.selectable_label(triangle_editor_open, "Show Triangle Editor").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::TriangleEditor);

                }
                let palette_editor_open = workspace.panel_exists(super::workspace::PanelType::PaletteEditor);
                if ui.selectable_label(palette_editor_open, "Show Palette Editor").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::PaletteEditor);

                }

                ui.separator();

                // Add Transform (functional)
                if ui.button("Add Transform").clicked() {
                    menu_actions.transform.add_transform = true;

                }
            });

            // Rendering Menu
            ui.menu_button(t!("menu.rendering"), |ui| {
                // Pause/Resume
                let pause_text = if menu_state.is_paused { "▶ Resume" } else { "⏸ Pause" };
                if ui.button(pause_text).clicked() {
                    menu_actions.rendering.pause_toggle = true;

                }

                // Reset Accumulation
                if ui.button("🔄 Reset Accumulation").clicked() {
                    menu_actions.rendering.reset_accumulation = true;

                }

                ui.separator();

                // Speed submenu
                ui.menu_button("Speed ▶", |ui| {
                    for &speed in &[1u32, 2, 4, 8, 16] {
                        if ui.button(format!("{}x", speed)).clicked() {
                            menu_actions.rendering.set_speed = Some(speed);

                        }
                    }
                });

                ui.separator();

                // Iterations per Thread submenu
                ui.menu_button("Iterations per Thread ▶", |ui| {
                    for &ipt in &[64u32, 128, 256, 512, 1024] {
                        if ui.button(format!("{}", ipt)).clicked() {
                            menu_actions.rendering.set_iterations_per_thread = Some(ipt);

                        }
                    }
                });
            });

            // Windows Menu
            ui.menu_button(t!("menu.window"), |ui| {
                // Performance opens as floating window in docking system (only one instance)
                let performance_open = workspace.panel_exists(super::workspace::PanelType::Performance);
                if ui.selectable_label(performance_open, "📊 Performance").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Performance);

                }

                // Settings opens Rendering panel as floating window (Settings was renamed to Rendering)
                let rendering_open = workspace.panel_exists(super::workspace::PanelType::Rendering);
                if ui.selectable_label(rendering_open, "⚙ Rendering").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Rendering);

                }

                // View opens as floating window in docking system
                let view_open = workspace.panel_exists(super::workspace::PanelType::View);
                if ui.selectable_label(view_open, "🔍 View").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::View);

                }

                // Transforms opens as floating window in docking system
                let transforms_open = workspace.panel_exists(super::workspace::PanelType::Transforms);
                if ui.selectable_label(transforms_open, "🔧 Transforms").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Transforms);

                }

                // Triangle Editor opens as floating window in docking system
                let triangle_editor_open = workspace.panel_exists(super::workspace::PanelType::TriangleEditor);
                if ui.selectable_label(triangle_editor_open, "📐 Triangle Editor").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::TriangleEditor);

                }

                // Tone Mapping & Colors opens Colors panel as floating window
                let colors_open = workspace.panel_exists(super::workspace::PanelType::Colors);
                if ui.selectable_label(colors_open, "🎨 Tone Mapping & Colors").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Colors);

                }

                ui.separator();

                // Palette Editor opens as floating window in docking system
                let palette_editor_open = workspace.panel_exists(super::workspace::PanelType::PaletteEditor);
                if ui.selectable_label(palette_editor_open, "🎨 Palette Editor").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::PaletteEditor);

                }

                let palette_library_open = workspace.panel_exists(super::workspace::PanelType::PaletteLibrary);
                if ui.selectable_label(palette_library_open, "📚 Palette Library").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::PaletteLibrary);
                }

                let preset_library_open = workspace.panel_exists(super::workspace::PanelType::PresetLibrary);
                if ui.selectable_label(preset_library_open, "📚 Preset Library").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::PresetLibrary);
                }

                let file_browser_open = workspace.panel_exists(super::workspace::PanelType::FileBrowser);
                if ui.selectable_label(file_browser_open, "📂 File Browser").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::FileBrowser);
                }

                // Config Import/Export opens as floating window in docking system
                let config_dialog_open = workspace.panel_exists(super::workspace::PanelType::ConfigDialog);
                if ui.selectable_label(config_dialog_open, "📄 Config Import/Export").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::ConfigDialog);

                }

                // Undo/Redo History opens as floating window in docking system
                let history_open = workspace.panel_exists(super::workspace::PanelType::History);
                if ui.selectable_label(history_open, "⮪ Undo/Redo History").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::History);

                }

                ui.separator();
                ui.menu_button("📐 Workspace Layout", |ui| {
                    let current = workspace.current_layout;

                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Beginner, "Beginner").clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Beginner);

                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Standard, "Standard").clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Standard);

                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Advanced, "Advanced").clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Advanced);

                    }
                    if ui.selectable_label(current == super::workspace::WorkspaceLayout::Export, "Export").clicked() {
                        workspace.apply_layout(super::workspace::WorkspaceLayout::Export);

                    }
                });
            });

            // Help Menu
            ui.menu_button(t!("menu.help"), |ui| {
                // Help panel opens as floating window in docking system
                let help_open = workspace.panel_exists(super::workspace::PanelType::Help);
                if ui.selectable_label(help_open, "❓ Help").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Help);

                }

                if ui.selectable_label(help_open, "⌨ Keyboard Shortcuts").clicked() {
                    workspace.open_floating_panel(super::workspace::PanelType::Help);

                }

                ui.separator();

                if ui.button("🐛 Report Bug...").clicked() {
                    let _ = webbrowser::open("https://github.com/calibas/fflame-rust/issues/new");
                }

                if ui.button("ℹ About...").clicked() {
                    let _ = webbrowser::open("https://github.com/calibas/fflame-rust");
                }
            });

            // Push language selector to the right side
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Language selector menu (globe icon)
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
            });
        });
    });
}
