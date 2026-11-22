//! The Settings panel (`PanelType::Preferences`): the device's preferences
//! that belong to no other panel -- the online account, the pickers'
//! downloads, the language. `SystemSettings` only, saved at once and
//! outside the undo history; never anything of the loaded config.
//!
//! Not the old "Settings" panel, which became the Rendering panel
//! (`ui::settings`, `PanelType::Rendering`): the chaos game's and the
//! display's settings stay there, as the camera's stay in View and the
//! export's in Export. A preference that has a panel of its own belongs
//! in it; the menus keep their actions, not their switches.

use crate::config::ConfigManager;

/// Draw the panel.
pub fn render_preferences_panel(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        // ---- Online ----
        ui.heading(t!("preferences.online"));
        // The online account (ONLINE.md, Online account): off, the app is
        // a guest of the online service -- downloads still work, nothing
        // signs in or saves online.
        let mut online = config_manager.system_settings().online_account;
        if ui
            .checkbox(&mut online, t!("menu.online_account").as_ref())
            .on_hover_text(t!("menu.tooltip_online_account").as_ref())
            .changed()
        {
            let settings = config_manager.system_settings_mut();
            settings.online_account = online;
            let _ = settings.save();
        }
        ui.add_space(8.0);

        // ---- Catalog ----
        ui.heading(t!("preferences.catalog"));
        // The pickers' downloads (L3).
        let mut downloads = config_manager.system_settings().picker_downloads != Some(false);
        if ui
            .checkbox(&mut downloads, t!("menu.picker_downloads").as_ref())
            .on_hover_text(t!("menu.picker_downloads_tip").as_ref())
            .changed()
        {
            let settings = config_manager.system_settings_mut();
            settings.picker_downloads = (!downloads).then_some(false);
            let _ = settings.save();
        }
        ui.add_space(8.0);

        // ---- Language ----
        // The desktop's globe menu does the same; on a phone this is the
        // only place. The app saves the choice (`App`, after the frame).
        ui.heading(t!("preferences.language"));
        let current = crate::i18n::current_locale();
        let locales = crate::i18n::supported_locales();
        let shown = locales.iter().find(|l| current == l.code).map_or_else(|| current.clone(), |l| l.display_text());
        let missing_font = egui::Id::new("preferences_missing_font");
        egui::ComboBox::from_id_salt("preferences_language").selected_text(shown).show_ui(ui, |ui| {
            for locale in &locales {
                if ui.selectable_label(current == locale.code, locale.display_text()).clicked() {
                    let ok = super::menu_bar::choose_locale(ui.ctx(), locale.code);
                    ui.ctx().data_mut(|d| {
                        if ok {
                            d.remove_temp::<String>(missing_font);
                        } else {
                            d.insert_temp(missing_font, locale.name.to_string());
                        }
                    });
                }
            }
        });
        if let Some(name) = ui.ctx().data(|d| d.get_temp::<String>(missing_font)) {
            ui.colored_label(ui.visuals().warn_fg_color, t!("preferences.font_missing", language = name));
        }
    });
}

#[cfg(test)]
mod tests {
    /// The panel shows its three sections and the switches as the settings
    /// stand. (Not clicked: a change saves the settings file.)
    #[test]
    fn the_settings_panel_shows_the_preferences() {
        let mut cm = crate::config::ConfigManager::new(crate::config::FractalConfig::default());
        cm.system_settings_mut().online_account = true;
        cm.system_settings_mut().picker_downloads = Some(false);
        let ctx = egui::Context::default();
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show_inside(ui, |ui| super::render_preferences_panel(ui, &mut cm));
            });
            texts.clear();
            fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
                match shape {
                    egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    _ => {}
                }
            }
            for s in &out.shapes {
                walk(&s.shape, &mut texts);
            }
        }
        for key in ["preferences.online", "preferences.catalog", "preferences.language", "menu.online_account", "menu.picker_downloads"] {
            let text = t!(key).to_string();
            assert!(texts.iter().any(|t| *t == text), "{key} is drawn: {texts:?}");
        }
        // Drawing changes nothing.
        assert!(cm.system_settings().online_account);
        assert_eq!(cm.system_settings().picker_downloads, Some(false));
    }
}
