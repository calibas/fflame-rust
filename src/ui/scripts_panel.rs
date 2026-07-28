//! Scripts panel — run generator and modifier scripts on the flame.
//!
//! Holds both kinds: a **generator** builds a flame from scratch, a
//! **modifier** reworks the flame you have open. Declared parameters
//! appear as ordinary sliders above the Run button, and the seed makes
//! any result reproducible — same script, same seed, same flame.
//!
//! The panel only *produces* a config; applying it (as a single undo
//! step) happens in the app's response handling, the same route presets
//! and the random generator take.

use std::collections::HashMap;

use egui;

use crate::config::FractalConfig;
use crate::script::library::{self, ScriptEntry, ScriptOrigin};
use crate::script::{ParamDecl, ParamValue, ScriptError, ScriptHost, ScriptKind, ScriptMeta};

#[derive(Default)]
pub struct ScriptsResponse {
    /// A flame to load, replacing the current one (single undo step).
    pub generated: Option<FractalConfig>,
    /// A set of flames to open in the Fractal Browser.
    pub batch: Option<Vec<FractalConfig>>,
}

pub struct ScriptsPanel {
    entries: Vec<ScriptEntry>,
    selected: usize,
    /// The editable copy of the selected script.
    text: String,
    /// Text the cached metadata was collected from; a mismatch re-collects.
    collected_from: u64,
    meta: Option<ScriptMeta>,
    /// Values for declared parameters, keyed by parameter name.
    values: HashMap<String, ParamValue>,
    seed: u64,
    batch_count: u32,
    show_editor: bool,
    /// Whether the editor body was laid out last frame, so opening it can be
    /// detected as a transition (the body doesn't exist yet when the header
    /// is clicked, so the focus request has to wait a frame).
    editor_was_open: bool,
    /// One-shot: focus the editor on the next frame that draws it.
    focus_editor: bool,
    error: Option<ScriptError>,
    messages: Vec<String>,
    warnings: Vec<String>,
    status: Option<String>,
    initialized: bool,
    /// Snapshot of the loaded palette library, refreshed each frame from
    /// the app so newly loaded packs are selectable.
    palettes: Vec<crate::scene::palette::Palette>,
}

impl Default for ScriptsPanel {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            selected: 0,
            text: String::new(),
            collected_from: 0,
            meta: None,
            values: HashMap::new(),
            seed: 1,
            batch_count: 9,
            show_editor: false,
            editor_was_open: false,
            focus_editor: false,
            error: None,
            messages: Vec::new(),
            warnings: Vec::new(),
            status: None,
            initialized: false,
            palettes: Vec::new(),
        }
    }
}

fn hash_of(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

impl ScriptsPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-scan the script folders, keeping the current selection by name.
    fn reload(&mut self, base: &FractalConfig) {
        let previous = self.entries.get(self.selected).map(|e| e.display_name.clone());
        self.entries = library::discover(base);
        self.selected = previous
            .and_then(|name| self.entries.iter().position(|e| e.display_name == name))
            .unwrap_or(0);
        self.load_selected();
    }

    fn load_selected(&mut self) {
        self.text = self
            .entries
            .get(self.selected)
            .map(|e| e.source.clone())
            .unwrap_or_default();
        self.values.clear();
        self.meta = None;
        self.collected_from = 0;
        self.error = None;
        self.messages.clear();
        self.warnings.clear();
        self.status = None;
    }

    /// Refresh cached metadata when the script text has changed.
    fn refresh_meta(&mut self, base: &FractalConfig) {
        let hash = hash_of(&self.text);
        if hash == self.collected_from {
            return;
        }
        self.collected_from = hash;
        match ScriptHost::new().collect(&self.text, base) {
            Ok(meta) => {
                // Drop values whose parameter no longer exists, keep the rest
                // so editing a script doesn't reset the sliders.
                let keys: Vec<String> = meta.params.iter().map(|p| p.key().to_string()).collect();
                self.values.retain(|k, _| keys.contains(k));
                self.meta = Some(meta);
                self.error = None;
            }
            Err(e) => {
                self.meta = None;
                self.error = Some(e);
            }
        }
    }

    fn declared_kind(&self) -> ScriptKind {
        self.meta
            .as_ref()
            .and_then(|m| m.kind)
            .or_else(|| self.entries.get(self.selected).map(|e| e.kind))
            .unwrap_or(ScriptKind::Generator)
    }

    /// Run once. Generators start from a fresh config, modifiers from the
    /// flame on screen; both inherit the current palette unless the script
    /// chooses one.
    fn run_once(
        &self,
        current: &FractalConfig,
        seed: u64,
    ) -> Result<crate::script::ScriptOutcome, ScriptError> {
        // Both kinds start from the CURRENT palette, so a script that says
        // nothing about palettes keeps yours. A script that calls
        // set_palette / random_palette overrides it from the loaded
        // library.
        let base = match self.declared_kind() {
            ScriptKind::Modifier => current.clone(),
            ScriptKind::Generator => {
                let mut fresh = FractalConfig::default();
                fresh.palette = current.palette.clone();
                fresh
            }
        };
        ScriptHost::with_palettes(self.palettes.clone()).run(
            &self.text,
            &base,
            seed,
            self.values.clone(),
        )
    }

    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        current: &FractalConfig,
        palettes: Vec<crate::scene::palette::Palette>,
    ) -> ScriptsResponse {
        let mut response = ScriptsResponse::default();
        self.palettes = palettes;

        if !self.initialized {
            self.initialized = true;
            self.reload(current);
        }
        self.refresh_meta(current);

        egui::ScrollArea::vertical().show(ui, |ui| {
            self.render_picker(ui, current);
            ui.separator();
            self.render_params(ui);
            ui.separator();
            self.render_run_controls(ui, current, &mut response);
            self.render_editor(ui);
            self.render_output(ui);
        });

        response
    }

    fn render_picker(&mut self, ui: &mut egui::Ui, current: &FractalConfig) {
        ui.horizontal(|ui| {
            ui.label("Script:");
            let label = self
                .entries
                .get(self.selected)
                .map(|e| e.label())
                .unwrap_or_else(|| "(none found)".to_string());
            let mut changed = None;
            egui::ComboBox::from_id_salt("script_picker")
                .selected_text(label)
                .width(240.0)
                .show_ui(ui, |ui| {
                    for (i, entry) in self.entries.iter().enumerate() {
                        if ui
                            .selectable_label(i == self.selected, entry.label())
                            .clicked()
                        {
                            changed = Some(i);
                        }
                    }
                });
            if let Some(i) = changed {
                self.selected = i;
                self.load_selected();
            }
            if ui.button("⟳").on_hover_text("Re-scan the script folders").clicked() {
                self.reload(current);
            }
        });

        match self.declared_kind() {
            ScriptKind::Generator => {
                ui.label(egui::RichText::new("Builds a new flame from scratch.").weak());
            }
            ScriptKind::Modifier => {
                ui.label(
                    egui::RichText::new("Changes the flame you have open.").weak(),
                );
            }
        }
    }

    fn render_params(&mut self, ui: &mut egui::Ui) {
        let Some(meta) = self.meta.clone() else {
            return;
        };
        if meta.params.is_empty() {
            ui.label(egui::RichText::new("This script has no settings.").weak());
            return;
        }

        for decl in &meta.params {
            let key = decl.key().to_string();
            match decl {
                ParamDecl::Float { label, default, min, max, .. } => {
                    let mut v = match self.values.get(&key) {
                        Some(ParamValue::Float(v)) => *v,
                        _ => *default,
                    };
                    if ui
                        .add(super::VkbSlider::new(&mut v, *min..=*max).text(label.clone()))
                        .changed()
                    {
                        self.values.insert(key, ParamValue::Float(v));
                    }
                }
                ParamDecl::Int { label, default, min, max, .. } => {
                    let mut v = match self.values.get(&key) {
                        Some(ParamValue::Int(v)) => *v,
                        _ => *default,
                    };
                    if ui
                        .add(super::VkbSlider::new(&mut v, *min..=*max).text(label.clone()))
                        .changed()
                    {
                        self.values.insert(key, ParamValue::Int(v));
                    }
                }
                ParamDecl::Bool { label, default, .. } => {
                    let mut v = match self.values.get(&key) {
                        Some(ParamValue::Bool(v)) => *v,
                        _ => *default,
                    };
                    if ui.checkbox(&mut v, label.clone()).changed() {
                        self.values.insert(key, ParamValue::Bool(v));
                    }
                }
                ParamDecl::Text { label, default, max_len, .. } => {
                    let mut v = match self.values.get(&key) {
                        Some(ParamValue::Text(v)) => v.clone(),
                        _ => default.clone(),
                    };
                    ui.horizontal(|ui| {
                        ui.label(label.clone());
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut v)
                                // Keyed by parameter, not by position: the
                                // param list is rebuilt from the script every
                                // frame, and positional ids let the caret and
                                // selection bleed between params (and between
                                // scripts that happen to lay out alike).
                                .id_salt(&key)
                                .char_limit(*max_len)
                                .desired_width(f32::INFINITY),
                        );
                        if r.changed() {
                            self.values.insert(key.clone(), ParamValue::Text(v.clone()));
                        }
                    });
                }
                ParamDecl::Choice { label, options, default, .. } => {
                    let mut idx = match self.values.get(&key) {
                        Some(ParamValue::Choice(i)) => *i,
                        _ => *default,
                    };
                    let shown = options.get(idx).cloned().unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label(label.clone());
                        egui::ComboBox::from_id_salt(format!("script_choice_{key}"))
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for (i, opt) in options.iter().enumerate() {
                                    if ui.selectable_label(i == idx, opt).clicked() {
                                        idx = i;
                                        self.values.insert(key.clone(), ParamValue::Choice(i));
                                    }
                                }
                            });
                    });
                }
            }
        }
    }

    fn render_run_controls(
        &mut self,
        ui: &mut egui::Ui,
        current: &FractalConfig,
        response: &mut ScriptsResponse,
    ) {
        ui.horizontal(|ui| {
            ui.label("Seed:");
            ui.add(super::VkbDragValue::new(&mut self.seed).speed(1.0));
            if ui
                .button("Reroll")
                .on_hover_text("Next seed — a different flame from the same script")
                .clicked()
            {
                self.seed = self.seed.wrapping_add(1);
                self.execute(current, response);
            }
        });

        ui.horizontal(|ui| {
            let verb = match self.declared_kind() {
                ScriptKind::Generator => "Generate",
                ScriptKind::Modifier => "Apply",
            };
            if ui
                .add_enabled(self.error.is_none(), egui::Button::new(verb))
                .clicked()
            {
                self.execute(current, response);
            }

            ui.separator();
            ui.add(
                super::VkbDragValue::new(&mut self.batch_count)
                    .speed(1.0)
                    .range(2..=64),
            );
            // A modifier run across many seeds IS mutation: same starting
            // flame, a different random walk each time. Name the button
            // after what it does rather than how it's implemented.
            let (batch_label, batch_hint) = match self.declared_kind() {
                ScriptKind::Generator => (
                    "Batch",
                    "Generate this many flames from consecutive seeds, and open them in the Fractal Browser",
                ),
                ScriptKind::Modifier => (
                    "Mutate",
                    "Make this many variations of the current flame and open them in the Fractal Browser, with the original first for comparison",
                ),
            };
            if ui
                .add_enabled(self.error.is_none(), egui::Button::new(batch_label))
                .on_hover_text(batch_hint)
                .clicked()
            {
                self.execute_batch(current, response);
            }
        });
    }

    fn execute(&mut self, current: &FractalConfig, response: &mut ScriptsResponse) {
        match self.run_once(current, self.seed) {
            Ok(outcome) => {
                self.messages = outcome.messages;
                self.warnings = outcome.warnings;
                self.error = None;
                self.status = Some(format!(
                    "Seed {} — {} transform(s)",
                    self.seed,
                    outcome.config.flame.transforms.len()
                ));
                response.generated = Some(outcome.config);
            }
            Err(e) => {
                self.status = None;
                self.error = Some(e);
            }
        }
    }

    fn execute_batch(&mut self, current: &FractalConfig, response: &mut ScriptsResponse) {
        let mut configs = Vec::new();
        // Mutating: lead with the untouched flame so the batch can be
        // compared against it, and so picking "none of these" is a click
        // rather than an undo.
        if self.declared_kind() == ScriptKind::Modifier {
            let mut original = current.clone();
            original.flame.name = "Original".to_string();
            configs.push(original);
        }
        let start = self.seed;
        for i in 0..self.batch_count as u64 {
            let seed = start.wrapping_add(i);
            match self.run_once(current, seed) {
                Ok(outcome) => {
                    let mut cfg = outcome.config;
                    // Name each result by its seed so a good one can be
                    // reproduced from the script alone.
                    cfg.flame.name = format!("{} #{seed}", self.script_name());
                    configs.push(cfg);
                }
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            }
        }
        let mutated = self.declared_kind() == ScriptKind::Modifier;
        self.status = Some(format!(
            "{} {} from seeds {}–{}{}",
            self.batch_count,
            if mutated { "mutations" } else { "flames" },
            start,
            start.wrapping_add(self.batch_count as u64 - 1),
            if mutated { " (original first)" } else { "" }
        ));
        response.batch = Some(configs);
    }

    fn script_name(&self) -> String {
        self.meta
            .as_ref()
            .map(|m| m.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Script".to_string())
    }

    fn render_editor(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        let section = egui::CollapsingHeader::new("Edit script")
            .default_open(self.show_editor)
            .show(ui, |ui| {
                self.show_editor = true;
                let editor = ui.add(
                    egui::TextEdit::multiline(&mut self.text)
                        // A stable id: without one the editor's id comes from
                        // a positional counter, so the focus request below
                        // can't name it and typing state is lost whenever the
                        // widgets above it change (a status or error line
                        // appearing is enough).
                        .id_salt("script_editor")
                        .code_editor()
                        .desired_rows(16)
                        .desired_width(f32::INFINITY),
                );
                // Take keyboard focus when the section opens. Without it the
                // editor holds no focus, egui reports it doesn't want
                // keyboard input, and keystrokes fall through to the app's
                // global shortcuts (arrows pan the fractal) instead of
                // reaching the script. Same one-shot pattern as the Add
                // Variations search box.
                if std::mem::take(&mut self.focus_editor) {
                    editor.request_focus();
                }

                ui.horizontal(|ui| {
                    #[cfg(not(target_arch = "wasm32"))]
                    if ui
                        .button("Save")
                        .on_hover_text("Save to your scripts folder (shipped scripts are never overwritten)")
                        .clicked()
                    {
                        match library::save_user_script(&self.script_name(), &self.text) {
                            Ok(path) => {
                                self.status = Some(format!("Saved to {}", path.display()));
                            }
                            Err(e) => self.status = Some(format!("Save failed: {e}")),
                        }
                    }

                    if ui
                        .button("Revert")
                        .on_hover_text("Discard edits and reload the script")
                        .clicked()
                    {
                        self.load_selected();
                    }

                    if matches!(
                        self.entries.get(self.selected).map(|e| &e.origin),
                        Some(ScriptOrigin::Builtin)
                    ) {
                        ui.label(egui::RichText::new("built-in").weak());
                    }
                });
            });

        // The header is clicked one frame before its body first lays out, so
        // the focus request is armed on the transition and consumed above on
        // the next frame.
        let open_now = section.body_response.is_some();
        if open_now && !self.editor_was_open {
            self.focus_editor = true;
        }
        self.editor_was_open = open_now;
    }

    fn render_output(&mut self, ui: &mut egui::Ui) {
        if let Some(err) = &self.error {
            ui.separator();
            // Position first: for this audience the line number is the
            // difference between a fixable typo and a dead end.
            let text = match err.line {
                Some(line) => format!("Line {line}: {}", err.message),
                None => err.message.clone(),
            };
            ui.colored_label(egui::Color32::from_rgb(240, 120, 120), text);
        }

        for w in &self.warnings {
            ui.colored_label(egui::Color32::from_rgb(230, 190, 120), format!("⚠ {w}"));
        }

        if !self.messages.is_empty() {
            ui.separator();
            for m in &self.messages {
                ui.label(egui::RichText::new(m).weak());
            }
        }

        if let Some(status) = &self.status {
            ui.separator();
            ui.label(egui::RichText::new(status).weak());
        }
    }
}
