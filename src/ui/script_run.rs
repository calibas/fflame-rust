//! Running a CPU script (docs/projects/catalog.md, *Phase 4*, decision 2
//! of the survey): a generator's or modifier's parameters, seed,
//! Generate / Apply and Batch / Mutate, and a library's import line -- what
//! the Scripts panel did, as one piece the Catalog's info pane draws for
//! the script selected and the Rhai Editor's Run draws for the text being
//! written.
//!
//! A script runs on the host every script shares: the whole library
//! (`script::library::discover`, read again when the files change), so one
//! script calls or imports another by id as it does headlessly; somebody
//! else's runs restricted (`ScriptHost::with_untrusted`, and
//! `with_untrusted_entry` for the one being run).

use crate::config::FractalConfig;
use crate::script::library::ScriptEntry;
use crate::script::{ParamValue, ScriptError, ScriptHost, ScriptKind, ScriptMeta};
use std::collections::HashMap;

/// What a run produced, for the app to load.
#[derive(Default)]
pub struct RunResult {
    /// A flame to load, replacing the current one (one undo step).
    pub generated: Option<FractalConfig>,
    /// Flames to open in the Fractal Browser.
    pub batch: Option<Vec<FractalConfig>>,
    /// An animation the script defined, loaded beside the flame.
    pub animation: Option<crate::animation::Animation>,
}

pub struct ScriptRun {
    /// The script the controls are for (its id, or a label for text being
    /// edited): another script starts its values afresh.
    for_id: String,
    text_hash: u64,
    meta: Option<ScriptMeta>,
    values: HashMap<String, ParamValue>,
    seed: u64,
    batch_count: u32,
    error: Option<ScriptError>,
    messages: Vec<String>,
    warnings: Vec<String>,
    status: Option<String>,
    /// The library as last discovered, and the file generation it was
    /// discovered at.
    library: Vec<ScriptEntry>,
    library_generation: Option<u64>,
}

impl Default for ScriptRun {
    fn default() -> Self {
        ScriptRun {
            for_id: String::new(),
            text_hash: 0,
            meta: None,
            values: HashMap::new(),
            seed: 1,
            batch_count: 9,
            error: None,
            messages: Vec::new(),
            warnings: Vec::new(),
            status: None,
            library: Vec::new(),
            library_generation: None,
        }
    }
}

fn hash_of(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// The library's file generation: an adoption, or a file of the Catalog's.
pub fn files_generation(cloud: &crate::app::script_cloud::ScriptCloudState) -> u64 {
    cloud.library_generation.wrapping_add(crate::storage::manage::generation())
}

impl ScriptRun {
    /// The library entry of `id`, discovering the library again when the
    /// files changed.
    fn entry(&mut self, id: &str, current: &FractalConfig, generation: u64) -> Option<ScriptEntry> {
        if self.library_generation != Some(generation) {
            self.library = crate::script::library::discover(current);
            self.library_generation = Some(generation);
        }
        self.library.iter().find(|e| e.id == id).cloned()
    }

    fn host(&self, palettes: &[crate::scene::palette::Palette], untrusted: bool) -> ScriptHost {
        let mut host = ScriptHost::with_palettes(palettes.to_vec()).with_scripts(self.library.iter().map(|e| (e.id.clone(), e.source.clone())).collect());
        // Downloaded scripts may only call shipped ones: the library set
        // covers one reached through `run_script` or `import`, the entry
        // flag the one being run.
        host = host.with_untrusted(self.library.iter().filter(|e| e.untrusted).map(|e| e.id.clone()));
        if untrusted {
            host = host.with_untrusted_entry();
        }
        host
    }

    fn kind(&self) -> ScriptKind {
        self.meta.as_ref().and_then(|m| m.kind).unwrap_or(ScriptKind::Generator)
    }

    fn name(&self) -> String {
        self.meta.as_ref().map(|m| m.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| "Script".to_string())
    }

    /// The controls for the library's script `id` (the Catalog's).
    pub fn render_stored(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        current: &FractalConfig,
        palettes: &[crate::scene::palette::Palette],
        generation: u64,
    ) -> RunResult {
        let Some(entry) = self.entry(id, current, generation) else {
            ui.weak(t!("script_run.not_in_library"));
            return RunResult::default();
        };
        self.render_text(ui, id, &entry.source, entry.untrusted, current, palettes, generation)
    }

    /// The controls for `text`, labelled `id` (the editor's, or a stored
    /// script's); `untrusted` runs it restricted.
    #[allow(clippy::too_many_arguments)]
    pub fn render_text(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        text: &str,
        untrusted: bool,
        current: &FractalConfig,
        palettes: &[crate::scene::palette::Palette],
        generation: u64,
    ) -> RunResult {
        if self.library_generation != Some(generation) {
            self.library = crate::script::library::discover(current);
            self.library_generation = Some(generation);
        }
        if self.for_id != id {
            // A different script means different parameters: one script's
            // settings must not feed another.
            *self = ScriptRun { library: std::mem::take(&mut self.library), library_generation: self.library_generation, seed: self.seed, batch_count: self.batch_count, ..Default::default() };
            self.for_id = id.to_string();
        }
        let hash = hash_of(text);
        if hash != self.text_hash {
            self.text_hash = hash;
            match self.host(palettes, untrusted).collect(text, current) {
                Ok(meta) => {
                    // Keep values whose parameter still exists, so editing a
                    // script does not reset its sliders.
                    let keys: Vec<String> = meta.params.iter().map(|p| p.name.to_string()).collect();
                    self.values.retain(|k, _| keys.contains(k));
                    self.warnings = meta.warnings.clone();
                    self.meta = Some(meta);
                    self.error = None;
                }
                Err(e) => {
                    self.meta = None;
                    self.error = Some(e);
                }
            }
        }

        let mut out = RunResult::default();
        if let Some(meta) = self.meta.clone() {
            super::script_params::render(ui, &meta, &mut self.values);
        }
        if self.kind() == ScriptKind::Library {
            // A library does nothing on its own; its use is the import line.
            ui.label(egui::RichText::new(format!("import \"{id}\" as lib;")).monospace()).on_hover_text(t!("script_run.import_tip"));
            self.render_output(ui);
            return out;
        }
        // A script that declared `norng` ignores the seed: a seed, Reroll
        // and Batch would change nothing.
        let uses_rng = !self.meta.as_ref().is_some_and(|m| m.flags.no_rng);
        if uses_rng {
            ui.horizontal(|ui| {
                ui.label(t!("script_run.seed"));
                ui.add(super::VkbU64::new(&mut self.seed, "script_run_seed").desired_width(150.0));
                if ui.button(t!("script_run.reroll")).on_hover_text(t!("script_run.reroll_tip")).clicked() {
                    self.seed = self.seed.wrapping_add(1);
                    self.execute(text, untrusted, current, palettes, &mut out);
                }
            });
        }
        ui.horizontal(|ui| {
            let verb = match self.kind() {
                ScriptKind::Modifier => t!("script_run.apply"),
                _ => t!("script_run.generate"),
            };
            if ui.add_enabled(self.error.is_none(), egui::Button::new(verb)).clicked() {
                self.execute(text, untrusted, current, palettes, &mut out);
            }
            if !uses_rng {
                return;
            }
            ui.separator();
            ui.add(super::VkbDragValue::new(&mut self.batch_count).speed(1.0).range(2..=64));
            // A modifier run across many seeds is mutation.
            let (label, tip) = match self.kind() {
                ScriptKind::Modifier => (t!("script_run.mutate"), t!("script_run.mutate_tip")),
                _ => (t!("script_run.batch"), t!("script_run.batch_tip")),
            };
            if ui.add_enabled(self.error.is_none(), egui::Button::new(label)).on_hover_text(tip).clicked() {
                self.execute_batch(text, untrusted, current, palettes, &mut out);
            }
        });
        self.render_output(ui);
        out
    }

    /// Run once: a generator from a fresh config, a modifier from the
    /// flame on screen; both keep the current palette unless the script
    /// chooses one.
    fn run_once(&self, text: &str, untrusted: bool, current: &FractalConfig, palettes: &[crate::scene::palette::Palette], seed: u64) -> Result<crate::script::ScriptOutcome, ScriptError> {
        let base = match self.kind() {
            ScriptKind::Modifier => current.clone(),
            ScriptKind::Generator | ScriptKind::Library => {
                let mut fresh = FractalConfig::default();
                fresh.palette = current.palette.clone();
                fresh
            }
        };
        self.host(palettes, untrusted).run(text, &base, seed, self.values.clone())
    }

    fn execute(&mut self, text: &str, untrusted: bool, current: &FractalConfig, palettes: &[crate::scene::palette::Palette], out: &mut RunResult) {
        match self.run_once(text, untrusted, current, palettes, self.seed) {
            Ok(outcome) => {
                self.messages = outcome.messages;
                self.warnings = outcome.warnings;
                self.error = None;
                self.status = Some(t!("script_run.ran", seed = self.seed, count = outcome.config.flame.transforms.len()).to_string());
                if let Some(animation) = &outcome.animation {
                    self.messages.push(t!("script_run.animation", seconds = format!("{:.3}", animation.duration), tracks = animation.tracks.len()).to_string());
                }
                out.animation = outcome.animation;
                out.generated = Some(outcome.config);
            }
            Err(e) => {
                self.status = None;
                self.error = Some(e);
            }
        }
    }

    fn execute_batch(&mut self, text: &str, untrusted: bool, current: &FractalConfig, palettes: &[crate::scene::palette::Palette], out: &mut RunResult) {
        let mutating = self.kind() == ScriptKind::Modifier;
        let mut configs = Vec::new();
        // Mutating: the untouched flame first, to compare against, and so
        // "none of these" is a click rather than an undo.
        if mutating {
            let mut original = current.clone();
            original.flame.name = t!("script_run.original").to_string();
            configs.push(original);
        }
        let start = self.seed;
        for i in 0..self.batch_count as u64 {
            let seed = start.wrapping_add(i);
            match self.run_once(text, untrusted, current, palettes, seed) {
                Ok(outcome) => {
                    let mut cfg = outcome.config;
                    // Named by its seed, so a good one can be made again.
                    cfg.flame.name = format!("{} #{seed}", self.name());
                    configs.push(cfg);
                }
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            }
        }
        let last = start.wrapping_add(self.batch_count as u64 - 1);
        self.status = Some(match mutating {
            true => t!("script_run.mutated", count = self.batch_count, first = start, last = last),
            false => t!("script_run.batched", count = self.batch_count, first = start, last = last),
        }
        .to_string());
        out.batch = Some(configs);
    }

    fn render_output(&self, ui: &mut egui::Ui) {
        if let Some(err) = &self.error {
            // The line first: for this audience it is the difference
            // between a fixable typo and a dead end.
            let text = match err.line {
                Some(line) => t!("script_run.error_line", line = line, message = err.message.clone()).to_string(),
                None => err.message.clone(),
            };
            ui.colored_label(ui.visuals().error_fg_color, text);
        }
        for w in &self.warnings {
            ui.colored_label(ui.visuals().warn_fg_color, format!("\u{26a0} {w}"));
        }
        for m in &self.messages {
            ui.label(egui::RichText::new(m).weak());
        }
        if let Some(status) = &self.status {
            ui.label(egui::RichText::new(status).weak());
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// A shipped generator, run from the Catalog's controls: its
    /// parameters are read, Generate gives a flame, Batch as many flames
    /// as asked, each named by its seed.
    #[test]
    fn a_stored_generator_runs_and_batches() {
        let id = crate::script::library::EMBEDDED[0].0.trim_end_matches(".rhai");
        let current = FractalConfig::default();
        let mut run = ScriptRun::default();
        let ctx = egui::Context::default();
        let mut text = String::new();
        let _ = ctx.run_ui(Default::default(), |ui| {
            let _ = run.render_stored(ui, id, &current, &[], 0);
            text = run.library.iter().find(|e| e.id == id).map(|e| e.source.clone()).unwrap_or_default();
        });
        assert!(run.meta.is_some() && run.error.is_none(), "read: {:?}", run.error);
        assert_eq!(run.kind(), ScriptKind::Generator);

        let mut out = RunResult::default();
        run.execute(&text, false, &current, &[], &mut out);
        assert!(out.generated.is_some(), "Generate gives a flame: {:?}", run.error);

        run.batch_count = 3;
        let mut out = RunResult::default();
        run.execute_batch(&text, false, &current, &[], &mut out);
        let batch = out.batch.expect("a batch");
        assert_eq!(batch.len(), 3);
        assert!(batch[0].flame.name.ends_with(" #1"), "{}", batch[0].flame.name);
    }
}
