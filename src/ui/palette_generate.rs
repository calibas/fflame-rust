//! Generating a palette from a script, inside the Palette Editor.
//!
//! Palette generation lives in scripts rather than a Rust generator
//! shared with this panel — scripts are shareable, editable by whoever
//! wants a different scheme, and they get batch-and-choose for free from
//! the Scripts panel. What that would have cost is a generate button
//! *here*, where people look for it; this section buys it back by
//! letting the panel run the scripts instead.
//!
//! Scripts opt in with the `palette` flag:
//! `script("Random Palette", "modifier", ["palette"])`.

use std::collections::HashMap;

use egui;

use crate::config::{ConfigManager, ConfigPath};
use crate::scene::palette::Palette;
use crate::script::library::{self, ScriptEntry};
use crate::script::{ParamValue, ScriptError, ScriptHost, ScriptMeta};

/// Panel state for the generate section.
#[derive(Default)]
pub struct PaletteGenerator {
    /// Palette-flagged scripts, refreshed on demand rather than every
    /// frame — discovery reads and collects every script on disk.
    entries: Vec<ScriptEntry>,
    selected: usize,
    meta: Option<ScriptMeta>,
    values: HashMap<String, ParamValue>,
    seed: u64,
    error: Option<ScriptError>,
    messages: Vec<String>,
    loaded: bool,
}

impl PaletteGenerator {
    pub fn new() -> Self {
        Self { seed: 1, ..Default::default() }
    }

    /// Re-scan, keeping the current selection by id.
    fn reload(&mut self, config_manager: &ConfigManager) {
        let previous = self.entries.get(self.selected).map(|e| e.id.clone());
        let base = config_manager.active_config().clone();
        self.entries = library::discover(&base)
            .into_iter()
            .filter(|e| {
                // The declared flag is the whole opt-in. Reading it needs
                // the collect pass, which discover has already paid for
                // — but it does not keep the flags, so ask again.
                ScriptHost::new()
                    .collect(&e.source, &base)
                    .map(|m| m.flags.palette)
                    .unwrap_or(false)
            })
            .collect();
        self.selected = previous
            .and_then(|id| self.entries.iter().position(|e| e.id == id))
            .unwrap_or(0);
        self.refresh_meta(config_manager);
        self.loaded = true;
    }

    fn refresh_meta(&mut self, config_manager: &ConfigManager) {
        self.values.clear();
        self.error = None;
        self.messages.clear();
        let Some(entry) = self.entries.get(self.selected) else {
            self.meta = None;
            return;
        };
        let base = config_manager.active_config().clone();
        match ScriptHost::new().collect(&entry.source, &base) {
            Ok(meta) => {
                self.meta = Some(meta);
            }
            Err(e) => {
                self.meta = None;
                self.error = Some(e);
            }
        }
    }

    /// Run the selected script and return just its palette.
    ///
    /// Only the palette: a script could touch anything in the config,
    /// and this panel promises a palette. Taking the whole result would
    /// let a mis-declared script rewrite the flame from the Palette
    /// Editor, which nobody would expect from a button called Generate.
    fn run(&mut self, config_manager: &ConfigManager, palettes: &[Palette]) -> Option<Palette> {
        let entry = self.entries.get(self.selected)?;
        let base = config_manager.active_config().clone();
        let host = ScriptHost::with_palettes(palettes.to_vec()).with_scripts(
            library::discover(&base)
                .into_iter()
                .map(|e| (e.id, e.source))
                .collect(),
        );
        match host.run(&entry.source, &base, self.seed, self.values.clone()) {
            Ok(outcome) => {
                self.messages = outcome.messages;
                self.error = None;
                Some(outcome.config.palette)
            }
            Err(e) => {
                self.error = Some(e);
                self.messages.clear();
                None
            }
        }
    }
}

/// Draw the section. Applies through `ConfigPath::Palette`, the same
/// route every other edit in this panel takes, so undo and the GPU
/// update come for free.
pub fn render(
    ui: &mut egui::Ui,
    gen: &mut PaletteGenerator,
    config_manager: &mut ConfigManager,
    palettes: &[Palette],
) {
    if !gen.loaded {
        gen.reload(config_manager);
    }

    egui::CollapsingHeader::new(t!("palette_editor.generate"))
        .default_open(false)
        .show(ui, |ui| {
            if gen.entries.is_empty() {
                ui.label(
                    egui::RichText::new(t!("palette_editor.generate_none"))
                        .weak(),
                );
                if ui.button("⟳").on_hover_text(t!("palette_editor.generate_rescan")).clicked() {
                    gen.reload(config_manager);
                }
                return;
            }

            let mut changed_script = None;
            ui.horizontal(|ui| {
                let label = gen
                    .entries
                    .get(gen.selected)
                    .map(|e| e.display_name.clone())
                    .unwrap_or_default();
                egui::ComboBox::from_id_salt("palette_generator_pick")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for (i, e) in gen.entries.iter().enumerate() {
                            if ui
                                .selectable_label(i == gen.selected, e.display_name.clone())
                                .clicked()
                            {
                                changed_script = Some(i);
                            }
                        }
                    });
                if ui.button("⟳").on_hover_text(t!("palette_editor.generate_rescan")).clicked() {
                    gen.reload(config_manager);
                }
            });
            if let Some(i) = changed_script {
                gen.selected = i;
                gen.refresh_meta(config_manager);
            }

            if let Some(meta) = gen.meta.clone() {
                super::script_params::render(ui, &meta, &mut gen.values);
            }

            let mut run_now = false;
            ui.horizontal(|ui| {
                if ui.button(t!("palette_editor.generate_run")).clicked() {
                    run_now = true;
                }
                // Same seed, same palette — so a result worth keeping can
                // be got back, and Reroll is just the next one along.
                ui.label(t!("palette_editor.generate_seed"));
                ui.add(super::VkbU64::new(&mut gen.seed, "palette_gen_seed").desired_width(150.0));
                if ui
                    .button(t!("palette_editor.generate_reroll"))
                    .clicked()
                {
                    gen.seed = gen.seed.wrapping_add(1);
                    run_now = true;
                }
            });

            if run_now {
                if let Some(palette) = gen.run(config_manager, palettes) {
                    let _ = config_manager.update_param(ConfigPath::Palette, palette.into());
                }
            }

            for m in &gen.messages {
                ui.label(egui::RichText::new(m).weak());
            }
            if let Some(err) = &gen.error {
                let text = match err.line {
                    Some(line) => format!("Line {line}: {}", err.message),
                    None => err.message.clone(),
                };
                ui.colored_label(egui::Color32::from_rgb(240, 120, 120), text);
            }
        });
}
