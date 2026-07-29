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

/// egui temp-data key the browser file picker writes into (WASM).
#[cfg(target_arch = "wasm32")]
const PENDING_SCRIPT_LOAD: &str = "pending_script_load_raw";
use crate::script::{ParamDecl, ParamValue, ScriptError, ScriptHost, ScriptKind, ScriptMeta};

#[derive(Default)]
pub struct ScriptsResponse {
    /// A flame to load, replacing the current one (single undo step).
    pub generated: Option<FractalConfig>,
    /// A set of flames to open in the Fractal Browser.
    pub batch: Option<Vec<FractalConfig>>,
    /// An animation the script defined, if it defined one. Loaded
    /// alongside the flame so running the script leaves the timeline
    /// ready to play.
    pub animation: Option<crate::animation::Animation>,
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
    /// The user script awaiting a delete confirmation, if any. Deleting
    /// is irreversible and there is no undo for the filesystem, so it
    /// takes a second click — the same shape the Palette Editor uses.
    #[cfg(not(target_arch = "wasm32"))]
    pending_delete: Option<(String, std::path::PathBuf)>,
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
            #[cfg(not(target_arch = "wasm32"))]
            pending_delete: None,
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

    /// Take on a script opened from an arbitrary path. It joins the
    /// picker as an entry so the combo keeps naming what is in the
    /// editor — otherwise the picker would still read "Turntable" while
    /// the editor held something else entirely.
    fn adopt_opened(&mut self, display_name: String, source: String, origin: ScriptOrigin) {
        let kind = ScriptHost::new()
            .collect(&source, &FractalConfig::default())
            .ok()
            .and_then(|m| m.kind)
            .unwrap_or(ScriptKind::Generator);
        self.entries.push(ScriptEntry { display_name: display_name.clone(), kind, source: source.clone(), origin });
        self.selected = self.entries.len() - 1;
        self.text = source;
        // A different script means different parameters; keeping the old
        // values would silently feed one script's settings to another.
        self.values.clear();
        self.meta = None;
        self.collected_from = 0;
        self.error = None;
        self.messages.clear();
        self.warnings.clear();
        self.show_editor = true;
        self.status = Some(format!("Opened {display_name}"));
    }

    /// Switches the script declared, e.g. `["norng"]`.
    fn flags(&self) -> crate::script::ScriptFlags {
        self.meta.as_ref().map(|m| m.flags).unwrap_or_default()
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

        // The browser file picker lands its text in egui's temp store;
        // pick it up here rather than routing it through the app.
        #[cfg(target_arch = "wasm32")]
        if let Some(text) = ui
            .ctx()
            .data_mut(|d| d.remove_temp::<String>(egui::Id::new(PENDING_SCRIPT_LOAD)))
        {
            self.adopt_opened("Opened".to_string(), text, ScriptOrigin::Builtin);
        }

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

        #[cfg(not(target_arch = "wasm32"))]
        self.render_delete_confirmation(ui, current);

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

        self.render_doc(ui);
    }

    /// Save / Revert / Open / Save As / Delete — everything that acts on
    /// the script file rather than on its text.
    fn render_file_actions(&mut self, ui: &mut egui::Ui) {
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

        // Open / Save As take the same file-dialog route the
        // Animation panel uses for .anim: rfd on desktop, the
        // browser picker and a download on the web.
        #[cfg(not(target_arch = "wasm32"))]
        {
            if ui
                .button("Open…")
                .on_hover_text("Open a .rhai script from anywhere on disk")
                .clicked()
            {
                let picked = rfd::FileDialog::new()
                    .add_filter("Flame script", &["rhai"])
                    .pick_file();
                if let Some(path) = picked {
                    match std::fs::read_to_string(&path) {
                        Ok(text) => {
                            let name = path
                                .file_stem()
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "Opened".to_string());
                            self.adopt_opened(name, text, ScriptOrigin::File(path));
                        }
                        Err(e) => self.status = Some(format!("Open failed: {e}")),
                    }
                }
            }

            if ui
                .button("Save As…")
                .on_hover_text("Write this script to a .rhai file anywhere on disk")
                .clicked()
            {
                let picked = rfd::FileDialog::new()
                    .add_filter("Flame script", &["rhai"])
                    .set_file_name(format!("{}.rhai", self.script_name()))
                    .save_file();
                if let Some(path) = picked {
                    self.status = Some(match std::fs::write(&path, &self.text) {
                        Ok(()) => format!("Saved to {}", path.display()),
                        Err(e) => format!("Save failed: {e}"),
                    });
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            if ui.button("Open…").on_hover_text("Open a .rhai script").clicked() {
                crate::app::trigger_browser_file_picker(
                    ".rhai",
                    ui.ctx().clone(),
                    PENDING_SCRIPT_LOAD,
                );
            }
            if ui.button("Save As…").on_hover_text("Download this script").clicked() {
                let name = format!(
                    "{}.rhai",
                    self.script_name().to_lowercase().replace(' ', "_")
                );
                self.status = Some(
                    match crate::app::trigger_browser_download(
                        self.text.as_bytes(),
                        &name,
                        "text/plain",
                    ) {
                        Ok(()) => format!("Downloaded {name}"),
                        Err(e) => format!("Download failed: {e}"),
                    },
                );
            }
        }

        // Delete sits with the other file actions: it acts on
        // the same script the rest of this row does. Enabled only
        // for a copy in the user folder — editing a shipped
        // starter saves a user copy that shadows it, and deleting
        // THAT copy is how the original comes back.
        #[cfg(not(target_arch = "wasm32"))]
        {
            use egui::Color32;

            let target = self.entries.get(self.selected).and_then(|e| match &e.origin {
                ScriptOrigin::File(path) if library::is_user_script(path) => {
                    Some((e.display_name.clone(), path.clone()))
                }
                _ => None,
            });
            let hint = match &target {
                Some((name, path)) => format!("Delete “{name}” from {}", path.display()),
                None => "Only your own scripts can be deleted — the shipped ones are read-only".to_string(),
            };
            // egui dims a disabled widget's own colours, but an explicit
            // RichText colour is taken as deliberate and left alone — so
            // a red icon stays red, just darker, and reads as enabled.
            // Pick the colour from the state instead.
            let tint = if target.is_some() {
                Color32::LIGHT_RED
            } else {
                ui.visuals().widgets.noninteractive.fg_stroke.color
            };
            if ui
                .add_enabled(target.is_some(), egui::Button::new(egui::RichText::new("🗑").color(tint)))
                .on_hover_text(hint)
                .clicked()
            {
                self.pending_delete = target;
            }
        }

        if matches!(
            self.entries.get(self.selected).map(|e| &e.origin),
            Some(ScriptOrigin::Builtin)
        ) {
            ui.label(egui::RichText::new("built-in").weak());
        }
    }

    /// Ask before deleting: there is no undo for a removed file.
    #[cfg(not(target_arch = "wasm32"))]
    fn render_delete_confirmation(&mut self, ui: &mut egui::Ui, current: &FractalConfig) {
        let Some((name, path)) = self.pending_delete.clone() else {
            return;
        };
        let mut close = false;
        egui::Window::new("Delete script")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label(format!("Delete “{name}”?"));
                // Name the file: two scripts can share a display name,
                // and this is the thing that actually disappears.
                ui.label(egui::RichText::new(path.display().to_string()).weak().monospace());
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        self.status = Some(match library::delete_user_script(&path) {
                            Ok(()) => format!("Deleted {}", path.display()),
                            Err(e) => e,
                        });
                        // Re-scan: a deleted user copy may reveal the
                        // shipped script it was shadowing.
                        self.reload(current);
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        if close {
            self.pending_delete = None;
        }
    }

    /// The script's own header comment, as its description.
    ///
    /// The summary is always visible — for this audience a script whose
    /// purpose you can only learn by opening the editor may as well not
    /// be there. The rest goes behind a disclosure because it is often
    /// long: `lsystem.rhai` carries a symbol table and a list of rules
    /// to try, which is exactly the reference someone wants while using
    /// it and exactly the wall of text nobody wants above the controls.
    fn render_doc(&self, ui: &mut egui::Ui) {
        let doc = crate::script::parse_doc(&self.text);
        if doc.is_empty() {
            return;
        }
        if !doc.summary.is_empty() {
            ui.label(&doc.summary);
        }
        if doc.body.is_empty() {
            return;
        }
        // Keyed by script, so opening the details for one doesn't leave
        // the next one expanded.
        egui::CollapsingHeader::new("About this script")
            .id_salt(("script_doc", self.selected))
            .default_open(false)
            .show(ui, |ui| {
                // Prose is hard-wrapped in the source at about 72
                // columns. Rendering each source line as its own label
                // would freeze that width, so consecutive prose lines
                // are joined into a paragraph and left for egui to wrap
                // at whatever width the panel actually has.
                let mut para: Vec<&str> = Vec::new();
                let mut flush = |ui: &mut egui::Ui, para: &mut Vec<&str>| {
                    if !para.is_empty() {
                        ui.label(egui::RichText::new(para.join(" ")).weak());
                        para.clear();
                    }
                };
                for line in doc.body.lines() {
                    if line.is_empty() {
                        flush(ui, &mut para);
                        ui.add_space(4.0);
                    } else if crate::script::doc_line_is_heading(line) {
                        flush(ui, &mut para);
                        ui.add_space(4.0);
                        let text = line.strip_prefix("# ").unwrap_or(line);
                        ui.label(egui::RichText::new(text).strong());
                    } else if line.starts_with(char::is_whitespace) {
                        // Indented lines are tables — the L-system symbol
                        // list, the rules to try. Joining or proportional
                        // text would throw their columns out of alignment.
                        flush(ui, &mut para);
                        ui.label(egui::RichText::new(line).weak().monospace());
                    } else {
                        para.push(line);
                    }
                }
                flush(ui, &mut para);
            });
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
                ParamDecl::Color { key: _, label, default } => {
                    let mut v = match self.values.get(&key) {
                        Some(ParamValue::Color(v)) => *v,
                        _ => *default,
                    };
                    ui.horizontal(|ui| {
                        // The same picker the Palette Editor, Solid panel
                        // and background colour already use.
                        if ui.color_edit_button_rgb(&mut v).changed() {
                            self.values.insert(key.clone(), ParamValue::Color(v));
                        }
                        ui.label(label.clone());
                    });
                }
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
        // A script that declared `norng` ignores the seed entirely, so
        // the seed field, Reroll and Batch would all be controls that
        // change nothing — worse than absent, because they imply the
        // result varies.
        let uses_rng = !self.flags().no_rng;

        if uses_rng {
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
        }

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

            if !uses_rng {
                return;
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
                if let Some(animation) = &outcome.animation {
                    self.messages.push(format!(
                        "animation: {:.3}s, {} track(s) — loaded into the timeline",
                        animation.duration,
                        animation.tracks.len()
                    ));
                }
                response.animation = outcome.animation;
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

                // File actions sit above the editor: they act on the
                // script as a whole, and a row buried under sixteen rows
                // of code is a row you have to go looking for.
                ui.horizontal(|ui| {
                    self.render_file_actions(ui);
                });

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
