//! The Formula Editor: Rhai formulas, and Ultra Fractal and Fractint
//! formula files, checked as they are typed and used as the escape
//! formula or colouring (`docs/projects/escape-formula-language.md`; the
//! language is in `docs/main/ESCAPE.md`, "The formula language"), and
//! Rhai transformations used as flame variations (the KIFS plan, C1:
//! `variations::custom`; the flame carries the source).
//!
//! A file holds entries, listed above the source (a Rhai file, one).
//! Each is checked as a formula and as a colouring on a pause in typing,
//! and offered as whichever it compiles as. Using one compiles it
//! (`escape::custom`) and puts its source in the config, one undo step,
//! so the file renders anywhere. With Live on, an entry in use is
//! re-applied whenever an edit to it compiles again. A UF or Fractint
//! formula or colouring is edited as Rhai: Edit as Rhai converts it
//! (`custom::convert_to_rhai`) into a new file, the one opened left as
//! it is.

use std::time::Duration;

use crate::config::escape::{CustomDialect, CustomSource};
use crate::config::manager::custom_source_json;
use crate::config::{ConfigManager, ConfigPath, ConfigValue};
use crate::escape::custom;
use crate::lang::{self, EntryKind, Severity};
use crate::storage::formula_store;

/// Where the browser's file picker leaves the opened file's text.
#[cfg(target_arch = "wasm32")]
const PENDING_FORMULA_LOAD: &str = "pending_formula_load_raw";

/// How long typing must pause before the file is checked again.
const RECHECK_AFTER: Duration = Duration::from_millis(400);

/// Messages shown per entry.
const MESSAGES_SHOWN: usize = 6;

/// One entry of the file, as last checked.
struct EntryRow {
    name: String,
    /// The entry's text, name to closing brace: what a config carries.
    source: String,
    /// What it most likely is, for which button leads.
    likely: EntryKind,
    /// `line:col: message` per error, as a formula, as a colouring and as
    /// a transformation (a flame variation).
    as_formula: Vec<String>,
    as_coloring: Vec<String>,
    as_transform: Vec<String>,
    /// As a solid (B3): Rhai only.
    as_solid: Vec<String>,
    /// As an effect (P2): Rhai only.
    as_effect: Vec<String>,
    /// As a simulation's colouring (P4): Rhai only.
    as_sim_coloring: Vec<String>,
    /// As a simulation's model (P4): Rhai only.
    as_sim_model: Vec<String>,
}

impl EntryRow {
    fn errors(&self, kind: EntryKind) -> &[String] {
        match kind {
            EntryKind::Formula => &self.as_formula,
            EntryKind::Coloring => &self.as_coloring,
            EntryKind::Transform => &self.as_transform,
            EntryKind::Solid => &self.as_solid,
            EntryKind::Effect => &self.as_effect,
            EntryKind::SimColoring => &self.as_sim_coloring,
            EntryKind::SimModel => &self.as_sim_model,
        }
    }
}

/// What tells one escape-time entry from another, version to version: a
/// Rhai source's id (`INFO.id`), else its entry name.
fn entry_ident(src: &CustomSource) -> String {
    match src.dialect {
        CustomDialect::Rhai => lang::rhai::info_string(&src.source, "id").filter(|id| crate::scripted::is_uuid(id)).unwrap_or_else(|| src.entry.clone()),
        _ => src.entry.clone(),
    }
}

pub struct FormulaEditorPanel {
    /// The user's files.
    files: Vec<String>,
    /// The stored file being edited; `None` for an unsaved one (a new
    /// file, an example, an opened file).
    file: Option<String>,
    /// The name Save stores it under.
    name: String,
    text: String,
    /// The text as last loaded or saved.
    saved: String,
    dialect: CustomDialect,
    /// The text the entries were last checked against.
    checked: Option<String>,
    changed_at: Option<web_time::Instant>,
    parse_errors: Vec<String>,
    entries: Vec<EntryRow>,
    status: Option<String>,
    live: bool,
    /// What this panel put into the config: the entry Live re-applies,
    /// with the file it came from.
    in_use: Vec<(EntryKind, Option<String>, String)>,
}

impl Default for FormulaEditorPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl FormulaEditorPanel {
    pub fn new() -> Self {
        let mut p = FormulaEditorPanel {
            files: formula_store::list(),
            file: None,
            name: String::new(),
            text: String::new(),
            saved: String::new(),
            dialect: CustomDialect::UltraFractal,
            checked: None,
            changed_at: None,
            parse_errors: Vec::new(),
            entries: Vec::new(),
            status: None,
            live: true,
            in_use: Vec::new(),
        };
        match p.files.first().cloned() {
            Some(f) => p.open_stored(&f),
            None => p.open_example(0),
        }
        p
    }

    fn adopt(&mut self, file: Option<String>, name: String, text: String) {
        self.dialect = if formula_store::is_rhai(&name) {
            CustomDialect::Rhai
        } else if formula_store::is_fractint(&name) {
            CustomDialect::Fractint
        } else {
            CustomDialect::UltraFractal
        };
        self.file = file;
        self.name = name;
        self.saved = text.clone();
        self.text = text;
        self.checked = None;
        self.status = None;
    }

    /// Open a formula the config now uses (the Escape panel's Edit as
    /// Formula): unsaved, and in use, so Live re-applies its edits.
    pub fn open_in_use(&mut self, file: String, src: &CustomSource) {
        self.adopt(None, file, src.source.clone());
        self.in_use.retain(|(k, _, _)| *k != EntryKind::Formula);
        self.in_use.push((EntryKind::Formula, None, src.entry.clone()));
        self.status = Some(t!("formula_editor.editing_builtin", entry = src.entry).to_string());
    }

    fn open_stored(&mut self, file: &str) {
        match formula_store::load(file) {
            Some(text) => self.adopt(Some(file.to_string()), file.to_string(), text),
            None => self.status = Some(t!("formula_editor.cannot_open", file = file).to_string()),
        }
    }

    fn open_example(&mut self, i: usize) {
        if let Some((name, text)) = formula_store::EXAMPLES.get(i) {
            self.adopt(None, (*name).to_string(), (*text).to_string());
        }
    }

    fn is_coloring_file(&self) -> bool {
        formula_store::is_coloring_file(&self.name)
    }

    /// A Rhai file: one formula, colouring or transformation, the whole
    /// text; which, its functions say (`finish` a colouring's, `escaped`
    /// a formula's, `variation` a transformation's).
    fn recheck_rhai(&mut self) {
        let text = self.text.clone();
        let kind = lang::rhai::kind_of(&text).unwrap_or(EntryKind::Formula);
        let checked = match kind {
            EntryKind::Formula => lang::rhai::program(&text),
            EntryKind::Coloring => lang::rhai::coloring(&text),
            EntryKind::Transform => lang::rhai::transformation_any(&text),
            EntryKind::Solid => lang::rhai::solid(&text),
            EntryKind::Effect => lang::rhai::effect(&text),
            EntryKind::SimColoring => lang::rhai::sim_coloring(&text),
            EntryKind::SimModel => lang::rhai::sim_model(&text, lang::rhai::SimFn::Step(0)),
        };
        let mut errors: Vec<String> = match &checked {
            Ok(_) => Vec::new(),
            Err(d) => d.iter().map(|x| x.render(&text)).collect(),
        };
        // A colouring that sets no index draws nothing: not offered.
        if kind == EntryKind::Coloring && checked.as_ref().is_ok_and(|p| !lang::wgsl::colours_anything(p)) {
            errors.push(t!("formula_editor.colours_nothing").to_string());
        }
        // A transformation becomes a variation: its WGSL too, under the
        // id Use gives it when it needs one.
        if kind == EntryKind::Transform && checked.is_ok() {
            let with_id = if crate::variations::custom::needs_id(&text) { lang::rhai::with_own_id(&text, &crate::scripted::mint_id()) } else { text.clone() };
            if let Err(e) = crate::variations::custom::compile(&with_id) {
                errors.push(e);
            }
        }
        // An effect becomes one: its module too, under the id Use gives it
        // when it has none of its own (none, or not a UUID: a copy of a
        // built-in's, one typed by hand).
        if kind == EntryKind::Effect && checked.is_ok() {
            let with_id = lang::rhai::with_own_id(&text, &crate::scripted::mint_id());
            if let Err(e) = crate::effects::custom::compile(&with_id, crate::provenance::Provenance::Custom) {
                errors.push(e);
            }
        }
        // A simulation colouring becomes one: its module too (P4).
        if kind == EntryKind::SimColoring && checked.is_ok() {
            if let Err(e) = crate::sim::custom::compile(&text, "custom-check") {
                errors.push(e);
            }
        }
        // A model: every pass and its seed (P4).
        if kind == EntryKind::SimModel && checked.is_ok() {
            if let Err(e) = crate::sim::custom::compile_model(&text, "custom-check") {
                errors.push(e);
            }
        }
        let title = match &checked {
            Ok(p) => p.name.clone(),
            Err(_) => lang::rhai::title(&text).unwrap_or_else(|| self.name.clone()),
        };
        let is = |key: &str| vec![t!(key).to_string()];
        // A solid's walk too (B3).
        if kind == EntryKind::Solid {
            if let Some(e) = checked.as_ref().ok().and_then(custom::solid_error) {
                errors.push(e);
            }
        }
        // Its own kind's row has the errors; every other row says what
        // the file is.
        let key = match kind {
            EntryKind::Formula => "formula_editor.rhai_is_formula",
            EntryKind::Coloring => "formula_editor.rhai_is_coloring",
            EntryKind::Transform => "formula_editor.rhai_is_transform",
            EntryKind::Solid => "formula_editor.rhai_is_solid",
            EntryKind::Effect => "formula_editor.rhai_is_effect",
            EntryKind::SimColoring => "formula_editor.rhai_is_sim_coloring",
            EntryKind::SimModel => "formula_editor.rhai_is_sim_model",
        };
        let mut errors = Some(errors);
        let mut row = |k: EntryKind| if k == kind { errors.take().unwrap_or_default() } else { is(key) };
        let (as_formula, as_coloring, as_transform, as_solid, as_effect, as_sim_coloring, as_sim_model) = (
            row(EntryKind::Formula),
            row(EntryKind::Coloring),
            row(EntryKind::Transform),
            row(EntryKind::Solid),
            row(EntryKind::Effect),
            row(EntryKind::SimColoring),
            row(EntryKind::SimModel),
        );
        self.parse_errors = Vec::new();
        self.entries = vec![EntryRow { name: title, source: text.clone(), likely: kind, as_formula, as_coloring, as_transform, as_solid, as_effect, as_sim_coloring, as_sim_model }];
        self.checked = Some(text);
    }

    /// Parse the file and check every entry both ways.
    fn recheck(&mut self) {
        if self.dialect == CustomDialect::Rhai {
            return self.recheck_rhai();
        }
        let dialect = match self.dialect {
            // The text editor offers UF and Fractint; a hybrid has its
            // own panel.
            CustomDialect::UltraFractal | CustomDialect::Hybrid | CustomDialect::Rhai => lang::Dialect::UltraFractal,
            CustomDialect::Fractint => lang::Dialect::Fractint,
        };
        let text = self.text.clone();
        let (file, diags) = lang::parser::parse_file(&text);
        let render = |d: &lang::Diag| match d.severity {
            Severity::Error => d.render(&text),
            Severity::Warning => format!("⚠ {}", d.render(&text)),
        };
        // Errors inside an entry are that entry's; the rest are the
        // file's (a stray line, a missing brace).
        let inside = |d: &lang::Diag| file.entries.iter().any(|e| d.span.start >= e.span.start && d.span.start < e.span.end);
        self.parse_errors = diags.iter().filter(|d| !inside(d)).map(render).collect();
        let coloring_file = self.is_coloring_file();
        self.entries = file
            .entries
            .iter()
            .map(|e| {
                let own: Vec<String> = diags
                    .iter()
                    .filter(|d| d.span.start >= e.span.start && d.span.start < e.span.end && d.severity == Severity::Error)
                    .map(render)
                    .collect();
                let check = |kind| {
                    let (p, d) = match kind {
                        EntryKind::Formula => lang::switch::check_formula(&file, e, dialect),
                        EntryKind::Coloring | EntryKind::Transform | EntryKind::Solid | EntryKind::Effect | EntryKind::SimColoring | EntryKind::SimModel => lang::check::check(e, kind, dialect),
                    };
                    let mut msgs = own.clone();
                    msgs.extend(d.iter().filter(|x| x.severity == Severity::Error).map(render));
                    // A colouring that sets neither #index nor #solid
                    // compiles and draws nothing: not offered as one.
                    if kind == EntryKind::Coloring && p.as_ref().is_some_and(|p| !lang::wgsl::colours_anything(p)) {
                        msgs.push(t!("formula_editor.colours_nothing").to_string());
                    }
                    msgs
                };
                let transform = e.section(lang::ast::SectionKind::Transform).is_some();
                let likely = if transform {
                    EntryKind::Transform
                } else if coloring_file || e.section(lang::ast::SectionKind::Final).is_some() {
                    EntryKind::Coloring
                } else {
                    EntryKind::Formula
                };
                EntryRow {
                    name: e.name.clone(),
                    // With its `switch:` target, which the Julia toggle runs.
                    source: custom::carried_source(&text, &file, e),
                    likely,
                    as_formula: check(EntryKind::Formula),
                    as_coloring: check(EntryKind::Coloring),
                    // A UF transformation as a variation is a later step
                    // (UF support is paused): it shows its errors alone.
                    as_transform: if transform { check(EntryKind::Transform) } else { vec![t!("formula_editor.not_a_transform").to_string()] },
                    as_solid: vec![t!("formula_editor.solid_rhai_only").to_string()],
                    as_effect: vec![t!("formula_editor.effect_rhai_only").to_string()],
                    as_sim_coloring: vec![t!("formula_editor.sim_rhai_only").to_string()],
                    as_sim_model: vec![t!("formula_editor.sim_rhai_only").to_string()],
                }
            })
            .collect();
        self.checked = Some(text);
    }

    /// Compile an entry and put it in the config, one undo step. The
    /// first time an entry is used, its `default:` view (centre and
    /// iterations) comes with it.
    fn use_entry(&mut self, cm: &mut ConfigManager, kind: EntryKind, row: usize) {
        if kind == EntryKind::Transform {
            return self.use_variation(cm, row);
        }
        if kind == EntryKind::Effect {
            return self.use_effect(cm, row);
        }
        if kind == EntryKind::SimColoring {
            return self.use_sim_coloring(cm, row);
        }
        if kind == EntryKind::SimModel {
            return self.use_sim_model(cm, row);
        }
        let Some(entry) = self.entries.get(row) else { return };
        // A Rhai entry is the user's: it has an id of its own (rule 7),
        // given here when it has none -- written into the file, which Save
        // then keeps -- and is named by it and its version.
        let mut source = entry.source.clone();
        let minted = self.dialect == CustomDialect::Rhai && !lang::rhai::info_string(&source, "id").is_some_and(|id| crate::scripted::is_uuid(&id));
        if minted {
            source = lang::rhai::with_own_id(&source, &crate::scripted::mint_id());
        }
        // With the variations it calls (`vary`), which the config carries.
        let src = custom::with_called_variations(kind, CustomSource { dialect: self.dialect, entry: entry.name.clone(), source, variations: Vec::new() });
        let name = custom::name_of(kind, &src);
        let compiled = match kind {
            EntryKind::Formula => custom::compile_formula(&src, &name).map(|_| ()),
            EntryKind::Coloring => custom::compile_coloring(&src, &name).map(|_| ()),
            EntryKind::Solid => custom::compile_solid(&src, &name).map(|_| ()),
            EntryKind::Transform | EntryKind::Effect | EntryKind::SimColoring | EntryKind::SimModel => return,
        };
        if let Err(d) = compiled {
            let first = d.first().map(|x| x.render(&src.source)).unwrap_or_default();
            self.status = Some(t!("formula_editor.did_not_compile", entry = src.entry, error = first).to_string());
            return;
        }
        if minted {
            self.text = src.source.clone();
            self.changed_at = Some(web_time::Instant::now());
        }
        let escape = &cm.config().escape;
        let (current, path_src, path_name, description) = match kind {
            // A solid is the formula's choice (B3).
            EntryKind::Formula | EntryKind::Solid => (
                escape.custom_formula.as_ref(),
                ConfigPath::EscapeCustomFormula,
                ConfigPath::EscapeFormula,
                "history.action.use_custom_formula",
            ),
            EntryKind::Coloring | EntryKind::Transform | EntryKind::Effect | EntryKind::SimColoring | EntryKind::SimModel => (
                escape.custom_coloring.as_ref(),
                ConfigPath::EscapeCustomColoring,
                ConfigPath::EscapeColoring,
                "history.action.use_custom_coloring",
            ),
        };
        // The same entry by its id, which outlives a change of title; one
        // without (another dialect's) by its entry name.
        let ident = entry_ident(&src);
        let first_use = current.is_none_or(|c| entry_ident(c) != ident);
        let mut changes = vec![
            (path_src, ConfigValue::String(custom_source_json(&Some(src.clone())))),
            (path_name, ConfigValue::String(name)),
        ];
        if first_use && kind == EntryKind::Formula {
            // The entry's own view. UF's `magn` is not applied: what span
            // its magnification 1 shows is still to be verified (plan,
            // phase 2).
            if let Ok(p) = custom::check_entry(kind, &src) {
                if let Some((re, im)) = p.view.center {
                    changes.push((ConfigPath::EscapeCenterRe, ConfigValue::String(format!("{re}"))));
                    changes.push((ConfigPath::EscapeCenterIm, ConfigValue::String(format!("{im}"))));
                }
                if let Some(m) = p.view.maxiter {
                    changes.push((ConfigPath::EscapeMaxIter, ConfigValue::UInt(m)));
                }
            }
        }
        // Using one enters Escape (one-workspace plan, decision 4): from
        // a flame, a colouring is drawn over the escape formula the config
        // already has (the Mandelbrot unless one was chosen). A solid put
        // to use from a plane brings the 3D look with it.
        match super::render_mode::choose(cm, changes, description, crate::scene::transforms::RenderMode::Escape) {
            Ok(_) => {
                self.in_use.retain(|(k, _, _)| *k != kind);
                self.in_use.push((kind, self.file.clone(), ident));
                self.status = Some(match kind {
                    _ if minted => t!("formula_editor.id_minted", entry = src.entry),
                    EntryKind::Formula => t!("formula_editor.using_formula", entry = src.entry),
                    EntryKind::Solid => t!("formula_editor.using_solid", entry = src.entry),
                    EntryKind::Coloring | EntryKind::Transform | EntryKind::Effect | EntryKind::SimColoring | EntryKind::SimModel => t!("formula_editor.using_coloring", entry = src.entry),
                }
                .to_string());
            }
            Err(e) => self.status = Some(format!("{e:?}")),
        }
    }

    /// Use a transformation as a flame variation: give it an id if it
    /// needs one (`variations::custom::needs_id`; written into the file,
    /// which Save then keeps), register it under its key (its id and
    /// version, rule 7), and put its source in the flame, one undo step:
    /// what named an earlier version -- the flame's transforms, the lens,
    /// the escape transformations -- moves to this one (`custom::rekey`),
    /// and a source nothing names any more goes. It is then in every
    /// transform's variation picker. In Escape mode it is also made the
    /// camera lens (at full amount if the lens was off).
    fn use_variation(&mut self, cm: &mut ConfigManager, row: usize) {
        use crate::variations::custom;
        let Some(entry) = self.entries.get(row) else { return };
        if self.dialect != CustomDialect::Rhai {
            self.status = Some(t!("formula_editor.variation_rhai_only").to_string());
            return;
        }
        let label = entry.name.clone();
        let mut source = entry.source.clone();
        let minted = custom::needs_id(&source);
        if minted {
            source = lang::rhai::with_own_id(&source, &crate::scripted::mint_id());
        }
        let key = match custom::register(&source) {
            Ok(key) => key,
            Err(e) => {
                self.status = Some(t!("formula_editor.did_not_compile", entry = label, error = e).to_string());
                return;
            }
        };
        if minted {
            // The file carries its id from now on.
            self.text = source.clone();
            self.changed_at = Some(web_time::Instant::now());
        }
        let id = crate::scripted::key_id(&key).unwrap_or_default();
        let title = crate::variations::global_registry().get(&key).map(|v| v.display_name.clone()).unwrap_or_else(|| label.clone());
        let mut after = cm.config().clone();
        let moved = custom::rekey(&mut after, &id, &key);
        // The flame carries the sources it uses: this version, and the
        // others its transforms (or the lens) still name.
        let names = |c: &crate::config::FractalConfig, entry: &str| {
            custom::used_by(&c.flame, entry) || c.escape.lens == entry || c.escape.transforms.iter().any(|t| t.variation == entry)
        };
        let mut list: Vec<CustomSource> = after
            .flame
            .custom_variations
            .iter()
            .filter(|s| crate::scripted::key_id(&s.entry).as_deref() != Some(id.as_str()) && names(&after, &s.entry))
            .cloned()
            .collect();
        let mut src = custom::carried(&source).unwrap_or(CustomSource { dialect: CustomDialect::Rhai, entry: key.clone(), source, variations: Vec::new() });
        src.entry = key.clone();
        list.push(src);
        let as_lens = after.render_mode == crate::scene::transforms::RenderMode::Escape;
        let result = if moved {
            // Renaming a variation in the transforms is no single path's
            // edit: the whole config, as one step.
            after.flame.custom_variations = list;
            if as_lens {
                after.escape.lens = key.clone();
                if after.escape.lens_amount == 0.0 {
                    after.escape.lens_amount = 1.0;
                }
            }
            cm.apply_config_edit(after, "history.action.use_custom_variation".to_string())
        } else {
            let json = serde_json::to_string(&list).unwrap_or_default();
            let mut changes = vec![(ConfigPath::FlameCustomVariations, ConfigValue::String(json))];
            if as_lens {
                changes.push((ConfigPath::EscapeLens, ConfigValue::String(key.clone())));
                if after.escape.lens_amount == 0.0 {
                    changes.push((ConfigPath::EscapeLensAmount, ConfigValue::Float(1.0)));
                }
            }
            cm.update_batch(changes, "history.action.use_custom_variation".to_string()).map(|_| ())
        };
        match result {
            Ok(()) => {
                // By its id: Live follows it from version to version.
                self.in_use.retain(|(k, _, n)| !(*k == EntryKind::Transform && *n == id));
                self.in_use.push((EntryKind::Transform, self.file.clone(), id));
                self.status = Some(if minted {
                    t!("formula_editor.id_minted", entry = label)
                } else if as_lens {
                    t!("formula_editor.using_lens", name = title)
                } else {
                    t!("formula_editor.using_variation", name = title)
                }
                .to_string());
            }
            Err(e) => self.status = Some(format!("{e:?}")),
        }
    }

    /// Use a simulation colouring (P4): give it an id if it has none --
    /// written into the file, which Save then keeps -- register it under
    /// its name (`sim::custom::name_of`: its id and version), and put its
    /// source in the config with the colouring named, one undo step;
    /// using one from another mode enters Simulation in the same step.
    fn use_sim_coloring(&mut self, cm: &mut ConfigManager, row: usize) {
        let Some(entry) = self.entries.get(row) else { return };
        let label = entry.name.clone();
        let mut source = entry.source.clone();
        let minted = !lang::rhai::info_string(&source, "id").is_some_and(|id| crate::scripted::is_uuid(&id));
        if minted {
            source = lang::rhai::with_own_id(&source, &crate::scripted::mint_id());
        }
        let name = match crate::sim::custom::register(&source) {
            Ok(name) => name,
            Err(e) => {
                self.status = Some(t!("formula_editor.did_not_compile", entry = label, error = e).to_string());
                return;
            }
        };
        if minted {
            self.text = source.clone();
            self.changed_at = Some(web_time::Instant::now());
        }
        let carried = crate::sim::custom::carried(&source);
        let ident = entry_ident(&carried);
        let changes = vec![
            (ConfigPath::SimCustomColoring, ConfigValue::String(custom_source_json(&Some(carried)))),
            (ConfigPath::SimColoring, ConfigValue::String(name)),
        ];
        match super::render_mode::choose(cm, changes, "history.action.use_sim_coloring", crate::scene::transforms::RenderMode::Simulation) {
            Ok(_) => {
                self.in_use.retain(|(k, _, _)| *k != EntryKind::SimColoring);
                self.in_use.push((EntryKind::SimColoring, self.file.clone(), ident));
                self.status = Some(if minted { t!("formula_editor.id_minted", entry = label) } else { t!("formula_editor.using_sim_coloring", entry = label) }.to_string());
            }
            Err(e) => self.status = Some(format!("{e:?}")),
        }
    }

    /// Use a simulation model (P4), as a colouring is used: an id if it has
    /// none, registered under its name, its source in the config with the
    /// model named, one undo step entering Simulation. A new version of
    /// the model in use keeps its parameters, dt and steps (the config's
    /// `SimModel` rule).
    fn use_sim_model(&mut self, cm: &mut ConfigManager, row: usize) {
        let Some(entry) = self.entries.get(row) else { return };
        let label = entry.name.clone();
        let mut source = entry.source.clone();
        let minted = !lang::rhai::info_string(&source, "id").is_some_and(|id| crate::scripted::is_uuid(&id));
        if minted {
            source = lang::rhai::with_own_id(&source, &crate::scripted::mint_id());
        }
        let name = match crate::sim::custom::register_model(&source) {
            Ok(name) => name,
            Err(e) => {
                self.status = Some(t!("formula_editor.did_not_compile", entry = label, error = e).to_string());
                return;
            }
        };
        if minted {
            self.text = source.clone();
            self.changed_at = Some(web_time::Instant::now());
        }
        let carried = crate::sim::custom::carried(&source);
        let ident = entry_ident(&carried);
        let changes = vec![
            (ConfigPath::SimCustomModel, ConfigValue::String(custom_source_json(&Some(carried)))),
            (ConfigPath::SimModel, ConfigValue::String(name)),
        ];
        match super::render_mode::choose(cm, changes, "history.action.use_sim_model", crate::scene::transforms::RenderMode::Simulation) {
            Ok(_) => {
                self.in_use.retain(|(k, _, _)| *k != EntryKind::SimModel);
                self.in_use.push((EntryKind::SimModel, self.file.clone(), ident));
                self.status = Some(if minted { t!("formula_editor.id_minted", entry = label) } else { t!("formula_editor.using_sim_model", entry = label) }.to_string());
            }
            Err(e) => self.status = Some(format!("{e:?}")),
        }
    }

    /// The Effects panel's Edit as Rhai: the built-in effect at `index` of
    /// its chain (`density`, which) becomes one of the user's -- the source
    /// the built-in runs, with an id of its own, registered, and the
    /// instance moved to it with its values, one undo step -- and opens
    /// here unsaved and in use, so Live re-applies an edit. Save keeps it
    /// in the library; the built-in is untouched.
    pub fn edit_effect_as_rhai(&mut self, cm: &mut ConfigManager, density: bool, index: usize, builtin: &str) {
        let Some(text) = crate::effects::custom::builtin_twin_source(builtin) else { return };
        let source = lang::rhai::with_own_id(&text, &crate::scripted::mint_id());
        let key = match crate::effects::custom::register(&source, crate::provenance::Provenance::Custom) {
            Ok(key) => key,
            Err(e) => {
                self.status = Some(t!("formula_editor.did_not_compile", entry = builtin, error = e).to_string());
                return;
            }
        };
        let Some(src) = crate::effects::custom::carried(&source) else { return };
        let mut list = cm.config().custom_effects.clone();
        list.push(src);
        let path = if density { ConfigPath::DensityEffectType { index } } else { ConfigPath::ColorEffectType { index } };
        let changes = vec![
            (path, ConfigValue::String(key.clone())),
            (ConfigPath::CustomEffects, ConfigValue::String(serde_json::to_string(&list).unwrap_or_default())),
        ];
        if let Err(e) = cm.update_batch(changes, "history.action.edit_effect_as_rhai".to_string()) {
            self.status = Some(format!("{e:?}"));
            return;
        }
        let title = crate::effects::global_effect_registry().get(builtin).map(|i| i.translated_name()).unwrap_or_else(|| builtin.to_string());
        let file = formula_store::file_name_for(&format!("{builtin}.rhai")).unwrap_or_else(|| "effect.rhai".into());
        self.adopt(None, file, source);
        let id = crate::effects::custom::id_of(&key);
        self.in_use.retain(|(k, _, n)| !(*k == EntryKind::Effect && *n == id));
        self.in_use.push((EntryKind::Effect, None, id));
        self.status = Some(t!("formula_editor.editing_builtin_effect", name = title).to_string());
    }

    /// Use an effect (P2): give it an id if it has none -- written into the
    /// file, which Save then keeps -- register it under its key (its id
    /// and version, rule 7), and put its source in the config, one undo
    /// step: the instances of an earlier version move to this one
    /// (`ColorEffectType`, their parameters kept), and the source the
    /// config no longer runs goes. The first time, an instance is added at
    /// the end of its chain (a second step: adding one is structural).
    fn use_effect(&mut self, cm: &mut ConfigManager, row: usize) {
        use crate::effects::{global_effect_registry, EffectCategory, EffectInstance};
        let Some(entry) = self.entries.get(row) else { return };
        if self.dialect != CustomDialect::Rhai {
            self.status = Some(t!("formula_editor.effect_rhai_only").to_string());
            return;
        }
        let name = entry.name.clone();
        let mut source = entry.source.clone();
        let mut minted = false;
        if !lang::rhai::info_string(&source, "id").is_some_and(|id| crate::scripted::is_uuid(&id)) {
            source = lang::rhai::with_own_id(&source, &crate::scripted::mint_id());
            minted = true;
        }
        let key = match crate::effects::custom::register(&source, crate::provenance::Provenance::Custom) {
            Ok(key) => key,
            Err(e) => {
                self.status = Some(t!("formula_editor.did_not_compile", entry = name, error = e).to_string());
                return;
            }
        };
        if minted {
            // The file carries its id from now on.
            self.text = source.clone();
            self.changed_at = Some(web_time::Instant::now());
        }
        use crate::effects::custom::id_of;
        let id = id_of(&key);
        let config = cm.config();
        // The instances of any version of this effect move to this one.
        let mut changes = Vec::new();
        let mut in_chain = false;
        for (list, path) in [
            (&config.color_effects, (|index| ConfigPath::ColorEffectType { index }) as fn(usize) -> ConfigPath),
            (&config.density_effects, |index| ConfigPath::DensityEffectType { index }),
        ] {
            for (index, e) in list.iter().enumerate() {
                if id_of(&e.effect_type) == id {
                    in_chain = true;
                    if e.effect_type != key {
                        changes.push((path(index), ConfigValue::String(key.clone())));
                    }
                }
            }
        }
        // The config carries the sources its chains run: this one, and
        // the others an instance still names.
        let runs = |s: &CustomSource| config.color_effects.iter().chain(&config.density_effects).any(|e| e.effect_type == s.entry);
        let mut list: Vec<CustomSource> = config.custom_effects.iter().filter(|s| id_of(&s.entry) != id && runs(s)).cloned().collect();
        let Some(src) = crate::effects::custom::carried(&source) else { return };
        list.push(src);
        changes.push((ConfigPath::CustomEffects, ConfigValue::String(serde_json::to_string(&list).unwrap_or_default())));
        if let Err(e) = cm.update_batch(changes, "history.action.use_custom_effect".to_string()) {
            self.status = Some(format!("{e:?}"));
            return;
        }
        let (title, category) = {
            let registry = global_effect_registry();
            let info = registry.get(&key);
            (info.map(|i| i.translated_name()).unwrap_or_else(|| id.clone()), info.map(|i| i.category))
        };
        if !in_chain {
            let effect = EffectInstance::new(&key);
            let description = format!("Add Effect: {title}");
            let change = match category {
                Some(EffectCategory::Density) => {
                    let index = cm.active_config().density_effects.len();
                    crate::config::ConfigChange::add_density_effect_snapshot(index, effect, description)
                }
                _ => {
                    let index = cm.active_config().color_effects.len();
                    crate::config::ConfigChange::add_color_effect_snapshot(index, effect, description)
                }
            };
            if let Err(e) = cm.apply_structural_change(change) {
                self.status = Some(format!("{e:?}"));
                return;
            }
        }
        // By its id: Live follows it from version to version.
        self.in_use.retain(|(k, _, n)| !(*k == EntryKind::Effect && *n == id));
        self.in_use.push((EntryKind::Effect, self.file.clone(), id));
        self.status = Some(if minted {
            t!("formula_editor.id_minted", entry = name)
        } else {
            t!("formula_editor.using_effect", name = title)
        }
        .to_string());
    }

    /// Live: re-apply the entries in use whose text changed and still
    /// compiles, while the config still uses them.
    fn reapply_live(&mut self, cm: &mut ConfigManager) {
        let in_use = self.in_use.clone();
        for (kind, file, entry) in in_use {
            if file != self.file {
                continue;
            }
            // A variation: by its id, while the flame carries a version of
            // it.
            if kind == EntryKind::Transform {
                let carried = cm
                    .config()
                    .flame
                    .custom_variations
                    .iter()
                    .find(|s| crate::scripted::key_id(&s.entry).as_deref() == Some(entry.as_str()))
                    .map(|s| s.source.clone());
                let row = self.entries.iter().position(|r| crate::variations::custom::id_of_source(&r.source).as_deref() == Some(entry.as_str()));
                if let (Some(carried), Some(row)) = (carried, row) {
                    let r = &self.entries[row];
                    if r.errors(kind).is_empty() && r.source != carried {
                        self.use_variation(cm, row);
                    }
                }
                continue;
            }
            // An effect: by its id, while the config carries a version of
            // it.
            if kind == EntryKind::Effect {
                let carried = cm
                    .config()
                    .custom_effects
                    .iter()
                    .find(|s| crate::effects::custom::id_of(&s.entry) == *entry)
                    .map(|s| s.source.clone());
                let row = self.entries.iter().position(|r| lang::rhai::info_string(&r.source, "id").as_deref() == Some(entry.as_str()));
                if let (Some(carried), Some(row)) = (carried, row) {
                    let r = &self.entries[row];
                    if r.errors(kind).is_empty() && r.source != carried {
                        self.use_effect(cm, row);
                    }
                }
                continue;
            }
            // A simulation colouring or model: by its id, while the config
            // carries a version of it.
            if matches!(kind, EntryKind::SimColoring | EntryKind::SimModel) {
                let sim = &cm.config().sim;
                let (carried, name) = if kind == EntryKind::SimModel { (&sim.custom_model, &sim.model) } else { (&sim.custom_coloring, &sim.coloring) };
                let carried = carried.as_ref().filter(|c| entry_ident(c) == entry && name.starts_with(crate::sim::custom::PREFIX)).map(|c| c.source.clone());
                let row = self.entries.iter().position(|r| entry_ident(&CustomSource { dialect: CustomDialect::Rhai, entry: r.name.clone(), source: r.source.clone(), variations: Vec::new() }) == entry);
                if let (Some(carried), Some(row)) = (carried, row) {
                    let r = &self.entries[row];
                    if r.errors(kind).is_empty() && r.source != carried {
                        if kind == EntryKind::SimModel {
                            self.use_sim_model(cm, row);
                        } else {
                            self.use_sim_coloring(cm, row);
                        }
                    }
                }
                continue;
            }
            let escape = &cm.config().escape;
            let (carried, name) = match kind {
                EntryKind::Formula | EntryKind::Solid => (escape.custom_formula.as_ref(), &escape.formula),
                EntryKind::Coloring | EntryKind::Transform | EntryKind::Effect | EntryKind::SimColoring | EntryKind::SimModel => (escape.custom_coloring.as_ref(), &escape.coloring),
            };
            let Some(carried) = carried else { continue };
            if entry_ident(carried) != entry || !name.starts_with(custom::PREFIX) {
                continue;
            }
            let dialect = self.dialect;
            let Some(row) = self
                .entries
                .iter()
                .position(|r| entry_ident(&CustomSource { dialect, entry: r.name.clone(), source: r.source.clone(), variations: Vec::new() }) == entry)
            else {
                continue;
            };
            let r = &self.entries[row];
            if r.errors(kind).is_empty() && (r.source != carried.source || carried.dialect != self.dialect) {
                self.use_entry(cm, kind, row);
            }
        }
    }

    /// Open an entry of this UF or Fractint file as a new Rhai file,
    /// converted.
    fn edit_as_rhai(&mut self, row: usize) {
        let Some((entry, kind)) = self.entries.get(row).map(|r| (r.name.clone(), r.likely)) else { return };
        match custom::convert_to_rhai(&self.text, &entry, kind, self.dialect, &self.name) {
            Ok(text) => {
                let name = formula_store::file_name_for(&format!("{entry}.rhai")).unwrap_or_else(|| "converted.rhai".into());
                self.adopt(None, name, text);
                self.status = Some(t!("formula_editor.converted", entry = entry).to_string());
            }
            Err(e) => self.status = Some(t!("formula_editor.cannot_convert", entry = entry, error = e).to_string()),
        }
    }

    fn save(&mut self) {
        // A Rhai file keeps its extension under any name it is given.
        if self.dialect == CustomDialect::Rhai && !formula_store::is_rhai(&self.name) {
            let stem = self.name.rsplit_once('.').map_or(self.name.as_str(), |(s, _)| s).to_string();
            self.name = format!("{stem}.rhai");
        }
        match formula_store::save(&self.name, &self.text) {
            Ok(file) => {
                self.saved = self.text.clone();
                self.status = Some(t!("formula_editor.saved", file = file).to_string());
                // A plug-in's lists offer what was saved: its new version.
                if formula_store::link_of(&file).is_some_and(|l| l.plugin) {
                    if let Err(e) = crate::storage::library::register_plugin(&file, &self.text) {
                        self.status = Some(t!("formula_editor.plugin_failed", file = file, error = e).to_string());
                    }
                }
                self.name = file.clone();
                self.file = Some(file);
                self.files = formula_store::list();
            }
            Err(e) => self.status = Some(e),
        }
    }

    /// Whether the open text could be a plug-in: a Rhai variation,
    /// effect, formula, colouring or solid (P3).
    fn could_be_plugin(&self) -> bool {
        self.dialect == CustomDialect::Rhai && lang::rhai::kind_of(&self.text).is_some()
    }

    /// Make a stored file a plug-in, registered at every start (P3), or
    /// stop. It is saved first: what loads at start is the file. One
    /// that needs an id of its own (a variation without a name, an
    /// effect) gets one, written into the file.
    fn set_plugin(&mut self, file: &str, on: bool) {
        let mut link = formula_store::link_of(file).unwrap_or_default();
        if !on {
            link.plugin = false;
            crate::storage::library::unlist_plugin(file);
            self.status = Some(match formula_store::set_link(file, link) {
                Ok(()) => t!("formula_editor.plugin_off", file = file).to_string(),
                Err(e) => e,
            });
            return;
        }
        if self.text != self.saved {
            self.status = Some(t!("formula_editor.plugin_save_first").to_string());
            return;
        }
        let needs_id = match lang::rhai::kind_of(&self.text) {
            Some(EntryKind::Transform) => crate::variations::custom::needs_id(&self.text),
            Some(EntryKind::Effect) => !lang::rhai::info_string(&self.text, "id").is_some_and(|id| crate::scripted::is_uuid(&id)),
            _ => false,
        };
        if needs_id {
            self.text = lang::rhai::with_own_id(&self.text, &crate::scripted::mint_id());
            self.save();
        }
        match crate::storage::library::register_plugin(file, &self.text) {
            Ok(p) => {
                link.plugin = true;
                self.status = Some(match formula_store::set_link(file, link) {
                    Ok(()) => t!("formula_editor.plugin_on", title = p.title).to_string(),
                    Err(e) => e,
                });
            }
            Err(e) => self.status = Some(t!("formula_editor.plugin_failed", file = file, error = e).to_string()),
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, cm: &mut ConfigManager, #[allow(unused_variables)] window: &winit::window::Window) {
        // The browser's picker leaves the opened text in egui's temp store.
        #[cfg(target_arch = "wasm32")]
        if let Some(text) = ui.ctx().data_mut(|d| d.remove_temp::<String>(egui::Id::new(PENDING_FORMULA_LOAD))) {
            // The picker gives the text alone: a Rhai formula declares
            // its functions.
            let name = if text.contains("fn step") { "opened.rhai" } else { "opened.ufm" };
            self.adopt(None, name.to_string(), text);
        }

        // Every Use in every mode (one-workspace plan): a formula, a
        // colouring or a solid is the escape engine's, and using one
        // enters Escape; a transformation is the flame's variation, or in
        // Escape the camera lens.
        let escape_mode = cm.config().render_mode == crate::scene::transforms::RenderMode::Escape;

        // ---- Files ----
        ui.horizontal_wrapped(|ui| {
            let dirty = self.text != self.saved;
            let current = format!("{}{}", self.name, if dirty { " *" } else { "" });
            let mut open: Option<String> = None;
            let mut example: Option<usize> = None;
            let mut shipped_effect: Option<(String, String)> = None;
            egui::ComboBox::from_id_salt("formula_editor_file")
                .selected_text(current)
                .width(180.0)
                .show_ui(ui, |ui| {
                    // The library's marks: whose it is, and a plug-in.
                    let links = formula_store::links();
                    for f in &self.files {
                        let link = links.get(f).cloned().unwrap_or_default();
                        let label = format!("{f}{}", crate::storage::library::marks(crate::storage::library::origin_of_link(&link), link.plugin));
                        if ui.selectable_label(self.file.as_deref() == Some(f.as_str()), label).clicked() {
                            open = Some(f.clone());
                        }
                    }
                    if !self.files.is_empty() {
                        ui.separator();
                    }
                    for (i, (name, _)) in formula_store::EXAMPLES.iter().enumerate() {
                        if ui.selectable_label(false, t!("formula_editor.example", file = name)).clicked() {
                            example = Some(i);
                        }
                    }
                    // The built-in effects' sources, which the app runs:
                    // opened unsaved, a copy to Save and Use as one's own.
                    ui.separator();
                    for (name, source) in crate::effects::custom::BUILTIN_TWINS {
                        let title = crate::effects::global_effect_registry().get(name).map(|i| i.translated_name()).unwrap_or_else(|| name.to_string());
                        if ui.selectable_label(false, t!("formula_editor.example_effect", name = title)).clicked() {
                            shipped_effect = Some((format!("{name}.rhai"), source.to_string()));
                        }
                    }
                    // The simulation colourings and models that ship as Rhai
                    // (P4).
                    for (name, source) in crate::sim::custom::SHIPPED.iter().chain(crate::sim::custom::SHIPPED_MODELS) {
                        let title = lang::rhai::title(source).unwrap_or_else(|| name.to_string());
                        if ui.selectable_label(false, t!("formula_editor.example_sim", name = title)).clicked() {
                            shipped_effect = Some((format!("sim-{name}.rhai"), source.to_string()));
                        }
                    }
                });
            if let Some((file, text)) = shipped_effect {
                self.adopt(None, file, text);
            }
            if let Some(f) = open {
                self.open_stored(&f);
            }
            if let Some(i) = example {
                self.open_example(i);
            }
            if ui.button(t!("formula_editor.new")).on_hover_text(t!("formula_editor.new_tip")).clicked() {
                // Outside Escape a new file is a variation.
                let text = if escape_mode { NEW_FILE } else { NEW_VARIATION };
                self.adopt(None, "untitled.rhai".to_string(), text.to_string());
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(t!("formula_editor.name"));
            ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(140.0));
            if ui.button(t!("formula_editor.save")).on_hover_text(t!("formula_editor.save_tip")).clicked() {
                self.save();
            }
            if let Some(file) = self.file.clone() {
                if ui.button(t!("formula_editor.delete")).clicked() {
                    crate::storage::library::unlist_plugin(&file);
                    self.status = Some(match formula_store::delete(&file) {
                        Ok(()) => t!("formula_editor.deleted", file = file).to_string(),
                        Err(e) => e,
                    });
                    self.files = formula_store::list();
                    self.file = None;
                }
                // A plug-in: registered at every start, so it is in the
                // lists without a config carrying it (P3).
                if self.could_be_plugin() {
                    let mut on = formula_store::link_of(&file).is_some_and(|l| l.plugin);
                    if ui.checkbox(&mut on, t!("formula_editor.plugin")).on_hover_text(t!("formula_editor.plugin_tip")).changed() {
                        self.set_plugin(&file, on);
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                if ui.button(t!("formula_editor.open")).on_hover_text(t!("formula_editor.open_tip")).clicked() {
                    let picked = rfd::FileDialog::new()
                        .set_parent(window)
                        .add_filter("Formula file", formula_store::EXTENSIONS)
                        .pick_file();
                    if let Some(path) = picked {
                        match std::fs::read(&path) {
                            // Old formula files are often Latin-1, not UTF-8.
                            Ok(bytes) => {
                                let text = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
                                let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                                self.adopt(None, name, text);
                            }
                            Err(e) => self.status = Some(format!("{e}")),
                        }
                    }
                }
                if ui.button(t!("formula_editor.export")).on_hover_text(t!("formula_editor.export_tip")).clicked() {
                    let picked = rfd::FileDialog::new()
                        .set_parent(window)
                        .add_filter("Formula file", formula_store::EXTENSIONS)
                        .set_file_name(&self.name)
                        .save_file();
                    if let Some(path) = picked {
                        self.status = Some(match std::fs::write(&path, &self.text) {
                            Ok(()) => t!("formula_editor.exported", file = path.display().to_string()).to_string(),
                            Err(e) => format!("{e}"),
                        });
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                if ui.button(t!("formula_editor.open")).on_hover_text(t!("formula_editor.open_tip")).clicked() {
                    crate::app::trigger_browser_file_picker(".rhai,.ufm,.ucl,.frm", ui.ctx().clone(), PENDING_FORMULA_LOAD);
                }
                if ui.button(t!("formula_editor.export")).on_hover_text(t!("formula_editor.export_tip")).clicked() {
                    self.status = Some(match crate::app::trigger_browser_download(self.text.as_bytes(), &self.name, "text/plain") {
                        Ok(()) => t!("formula_editor.exported", file = self.name.clone()).to_string(),
                        Err(e) => e,
                    });
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(t!("formula_editor.language"));
            let before = self.dialect;
            ui.selectable_value(&mut self.dialect, CustomDialect::Rhai, t!("formula_editor.rhai"));
            ui.selectable_value(&mut self.dialect, CustomDialect::UltraFractal, t!("formula_editor.uf"));
            ui.selectable_value(&mut self.dialect, CustomDialect::Fractint, t!("formula_editor.fractint"));
            if self.dialect != before {
                self.checked = None;
            }
            ui.checkbox(&mut self.live, t!("formula_editor.live")).on_hover_text(t!("formula_editor.live_tip"));
        });

        // Check on a pause in typing, not on every keystroke.
        if self.checked.as_deref() != Some(self.text.as_str()) {
            let waited = self.changed_at.map_or(RECHECK_AFTER, |t| t.elapsed());
            if waited >= RECHECK_AFTER {
                self.recheck();
                if self.live {
                    self.reapply_live(cm);
                }
            } else {
                ui.ctx().request_repaint_after(RECHECK_AFTER - waited);
            }
        }

        // ---- Entries ----
        if self.entries.is_empty() {
            ui.weak(t!("formula_editor.no_entries"));
        }
        let mut clicked: Option<(EntryKind, usize)> = None;
        let mut convert: Option<usize> = None;
        let foreign = self.dialect != CustomDialect::Rhai;
        for (i, row) in self.entries.iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                ui.strong(&row.name);
                // What it most likely is leads, the rest in their order.
                use EntryKind::*;
                const ALL: [EntryKind; 7] = [Formula, Coloring, Transform, Solid, Effect, SimColoring, SimModel];
                let mut order = ALL;
                order.sort_by_key(|k| *k != row.likely);
                for kind in order {
                    let ok = row.errors(kind).is_empty();
                    let label = match kind {
                        EntryKind::Formula => t!("formula_editor.use_formula"),
                        EntryKind::Coloring => t!("formula_editor.use_coloring"),
                        EntryKind::Transform if escape_mode => t!("formula_editor.use_lens"),
                        EntryKind::Transform => t!("formula_editor.use_variation"),
                        EntryKind::Solid => t!("formula_editor.use_solid"),
                        EntryKind::Effect => t!("formula_editor.use_effect"),
                        EntryKind::SimColoring => t!("formula_editor.use_sim_coloring"),
                        EntryKind::SimModel => t!("formula_editor.use_sim_model"),
                    };
                    // The kind it is not is offered only when it compiles
                    // as that too; the kind it is shows its errors.
                    if !ok && kind != row.likely {
                        continue;
                    }
                    let b = ui.add_enabled(ok, egui::Button::new(label));
                    if b.clicked() {
                        clicked = Some((kind, i));
                    }
                }
                // A UF or Fractint entry is edited as Rhai.
                if foreign && row.errors(row.likely).is_empty() && ui.button(t!("formula_editor.edit_as_rhai")).on_hover_text(t!("formula_editor.edit_as_rhai_tip")).clicked() {
                    convert = Some(i);
                }
            });
            let errors = row.errors(row.likely);
            for e in errors.iter().take(MESSAGES_SHOWN) {
                ui.colored_label(egui::Color32::from_rgb(240, 120, 120), e);
            }
            if errors.len() > MESSAGES_SHOWN {
                ui.weak(t!("formula_editor.more_errors", count = errors.len() - MESSAGES_SHOWN));
            }
        }
        for e in &self.parse_errors {
            ui.colored_label(egui::Color32::from_rgb(240, 120, 120), e);
        }
        if let Some((kind, i)) = clicked {
            self.use_entry(cm, kind, i);
        }
        if let Some(i) = convert {
            self.edit_as_rhai(i);
        }
        ui.separator();
        // ---- The source ----
        //
        // In a scroll area of its own: a long file would otherwise make
        // the panel scroll to the editor's cursor, and carry the entries
        // and the file controls out of view.
        let mut changed = false;
        // Highlighted in its language, as the Scripts panel highlights Rhai.
        let lang = match self.dialect {
            CustomDialect::Rhai => super::code_highlight::Lang::Rhai,
            _ => super::code_highlight::Lang::Formula,
        };
        let font_id = egui::TextStyle::Monospace.resolve(ui.style());
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
            let mut job = super::code_highlight::highlight_lang(buf.as_str(), font_id.clone(), lang);
            job.wrap.max_width = wrap_width;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        egui::ScrollArea::vertical().id_salt("formula_editor_scroll").max_height(360.0).show(ui, |ui| {
            let editor = ui.add(
                egui::TextEdit::multiline(&mut self.text)
                    .id_salt("formula_editor_source")
                    .code_editor()
                    .desired_rows(16)
                    .desired_width(f32::INFINITY)
                    .layouter(&mut layouter),
            );
            super::vkb_sync_opts(ui, &editor, &self.text, "multiline");
            changed = editor.changed();
        });
        if changed {
            self.changed_at = Some(web_time::Instant::now());
        }

        if let Some(s) = &self.status {
            ui.weak(s);
        }
    }
}

/// What New starts from in a flame mode: a variation, a point to a point
/// (the KIFS plan, C1).
const NEW_VARIATION: &str = "\
// A new variation. `variation` takes a point `z` and the parameters `p`
// and gives the point it maps to; the transform's weight scales it.
// `INFO.name` is its name in the variation list.
//
// Also at hand: `rand()`, a draw in [0, 1); a third argument `ctx` with
// `ctx.accum` (the transform's earlier variations, summed) and
// `ctx.weight`; `fn init(p)` returning a state each thread keeps, read
// and set as `this.name`; and ending with `#{ z: point, color: index }`
// to colour the point.

const PARAMS = #{
    twist: #{ value: 1.0, min: -4.0, max: 4.0, caption: \"Twist\" },
};

const INFO = #{ title: \"My Variation\", name: \"my_variation\" };

fn variation(z, p) {
    let r2 = z.norm_sqr();
    let a = r2 * p.twist;
    complex(z.re * cos(a) - z.im * sin(a), z.re * sin(a) + z.im * cos(a))
}
";

/// What New starts from: a formula in Rhai, the language's own.
const NEW_FILE: &str = "\
// A new formula. `start` gives each pixel its state, `step` moves it
// one pass on, `escaped` says when it has left.

const PARAMS = #{
    bailout: #{ value: 10000.0, min: 1.0, caption: \"Bailout\" },
};

const INFO = #{ title: \"My Formula\" };

fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), c: pixel }
}

fn step(s, p) {
    s.z = s.z.sqr() + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::transforms::RenderMode;

    /// Every Use in every mode (one-workspace plan, decision 4): from a
    /// flame, using an escape-time entry enters Escape in one undo step --
    /// a colouring over the formula the config already has.
    #[test]
    fn using_an_escape_entry_from_a_flame_enters_escape() {
        let mut cm = ConfigManager::new(crate::config::FractalConfig::default());
        assert_eq!(cm.config().render_mode, RenderMode::TwoD, "fixture assumption");
        let formula = cm.config().escape.formula.clone();
        let mut panel = FormulaEditorPanel::new();
        panel.adopt(None, "frothy-basin-coloring.rhai".into(), include_str!("../../assets/formulas/frothy-basin-coloring.rhai").into());
        panel.recheck();
        let before = cm.history_len();
        panel.use_entry(&mut cm, EntryKind::Coloring, 0);
        let c = cm.config();
        assert_eq!(c.render_mode, RenderMode::Escape, "{:?}", panel.status);
        assert!(c.escape.coloring.starts_with(custom::PREFIX), "{}", c.escape.coloring);
        // Given an id, written into the file, and named by it.
        let id = lang::rhai::info_string(&panel.text, "id").expect("an id in the file");
        assert_eq!(crate::scripted::key_id(&c.escape.coloring[custom::PREFIX.len()..]).as_deref(), Some(id.as_str()), "{}", c.escape.coloring);
        assert_eq!(c.escape.formula, formula, "over the formula the config had");
        assert_eq!(cm.history_len() - before, 1, "one undo step");
        cm.undo().unwrap();
        assert_eq!(cm.config().render_mode, RenderMode::TwoD);

        panel.adopt(None, "volterra-lotka.rhai".into(), include_str!("../../assets/formulas/volterra-lotka.rhai").into());
        panel.recheck();
        panel.use_entry(&mut cm, EntryKind::Formula, 0);
        let c = cm.config();
        assert_eq!(c.render_mode, RenderMode::Escape, "{:?}", panel.status);
        assert!(c.escape.formula.starts_with(custom::PREFIX), "{}", c.escape.formula);
    }

    /// An effect opened with an id that is not one of the user's -- a copy
    /// of a built-in's twin, `vignette`, or one typed by hand -- is offered
    /// for Use, and Use gives it a UUID, written into the file, before it
    /// goes into the config and the chain. (It was refused: "not an
    /// effect's id", with Use disabled and nothing to do about it.)
    #[test]
    fn an_effect_with_a_hand_written_id_gets_its_own_on_use() {
        let mut cm = ConfigManager::new(crate::config::FractalConfig::default());
        let mut panel = FormulaEditorPanel::new();
        let copied = include_str!("../../assets/effects/vignette.rhai").replace("id: \"vignette\"", "id: \"my_vignette\"");
        panel.adopt(None, "my-vignette.rhai".into(), copied);
        panel.recheck();
        assert!(panel.entries[0].errors(EntryKind::Effect).is_empty(), "{:?}", panel.entries[0].errors(EntryKind::Effect));
        panel.use_entry(&mut cm, EntryKind::Effect, 0);
        let id = lang::rhai::info_string(&panel.text, "id").expect("an id in the file");
        assert!(crate::scripted::is_uuid(&id), "{id}: {:?}", panel.status);
        let c = cm.config();
        assert_eq!(c.custom_effects.len(), 1);
        assert_eq!(crate::effects::custom::id_of(&c.custom_effects[0].entry), id);
        let key = c.custom_effects[0].entry.clone();
        assert_eq!(crate::effects::custom::id_of(&key), id, "keyed by its id and version");
        assert_eq!(c.color_effects.last().map(|e| e.effect_type.as_str()), Some(key.as_str()));
        let instances = c.color_effects.len();

        // Edited and used again: the instance moves to the new version, its
        // source replaces the old, one undo step; both versions stay
        // registered, so the undo renders what it did.
        panel.text = panel.text.replace("value: 0.5, min: 0.0, max: 1.0, caption: \"Radius\"", "value: 0.7, min: 0.0, max: 1.0, caption: \"Radius\"");
        panel.recheck();
        let before = cm.history_len();
        panel.use_entry(&mut cm, EntryKind::Effect, 0);
        let c = cm.config();
        let new_key = c.custom_effects[0].entry.clone();
        assert_ne!(new_key, key, "a new version, a new key");
        assert_eq!(crate::effects::custom::id_of(&new_key), id);
        assert_eq!(c.custom_effects.len(), 1, "the old source goes");
        assert_eq!(c.color_effects.len(), instances, "no second instance");
        assert_eq!(c.color_effects.last().map(|e| e.effect_type.as_str()), Some(new_key.as_str()));
        assert_eq!(cm.history_len() - before, 1, "one undo step");
        cm.undo().unwrap();
        assert_eq!(cm.config().color_effects.last().map(|e| e.effect_type.as_str()), Some(key.as_str()));
        assert!(crate::effects::global_effect_registry().get(&key).is_some(), "the old version still renders");
        let mut registry = crate::effects::global_effect_registry_mut();
        registry.remove_by_name(&key);
        registry.remove_by_name(&new_key);
    }

    /// Edit as Rhai on a built-in effect: the instance runs the user's
    /// copy of the source the built-in runs, under a UUID of its own, its
    /// settings kept, in one undo step; the editor holds that copy in use;
    /// the built-in is untouched, and undo returns the instance to it.
    #[test]
    fn edit_as_rhai_makes_a_builtin_effect_the_users() {
        let mut config = crate::config::FractalConfig::default();
        let mut vignette = crate::effects::EffectInstance::new("vignette");
        vignette.set_param("intensity", 0.8);
        config.color_effects = vec![vignette];
        let mut cm = ConfigManager::new(config);
        let mut panel = FormulaEditorPanel::new();
        let before = cm.history_len();
        panel.edit_effect_as_rhai(&mut cm, false, 0, "vignette");
        let c = cm.config();
        let key = c.color_effects[0].effect_type.clone();
        let id = crate::effects::custom::id_of(&key);
        assert!(crate::scripted::is_uuid(&id), "{key}: {:?}", panel.status);
        assert_eq!(c.color_effects[0].params.get("intensity"), Some(&0.8), "its settings kept");
        assert_eq!(c.custom_effects.len(), 1);
        assert_eq!(c.custom_effects[0].entry, key);
        assert_eq!(cm.history_len() - before, 1, "one undo step");
        assert_eq!(lang::rhai::info_string(&panel.text, "id").as_deref(), Some(id.as_str()), "the editor holds the copy");
        assert!(panel.in_use.iter().any(|(k, _, n)| *k == EntryKind::Effect && *n == id), "in use, for Live");
        assert!(crate::effects::global_effect_registry().get("vignette").is_some_and(|i| i.provenance.is_builtin()), "the built-in stays");
        cm.undo().unwrap();
        assert_eq!(cm.config().color_effects[0].effect_type, "vignette");
        crate::effects::global_effect_registry_mut().remove_by_name(&key);
    }

    /// A variation from before ids (its `INFO.name` alone) is Used under
    /// its name's derived id, the file untouched; edited and Used again,
    /// the transforms naming the old version move to the new one, its
    /// parameters with it, in one undo step; the old source goes, and undo
    /// brings the old version back, still registered. One without a name
    /// or an id gets an id written into the file.
    #[test]
    #[ignore = "writes the global variation registry (VARIATIONS.md, Testing): run alone"]
    fn a_new_version_of_a_variation_moves_its_transforms() {
        let mut cm = ConfigManager::new(crate::config::FractalConfig::default());
        let mut panel = FormulaEditorPanel::new();
        let text = "const INFO = #{ title: \"Rekey\", name: \"rhai_use_rekey_test\" };\nconst PARAMS = #{ k: 0.5 };\nfn variation(z, p) {\n    z * p.k\n}\n";
        panel.adopt(None, "rekey.rhai".into(), text.into());
        panel.recheck();
        assert!(panel.entries[0].errors(EntryKind::Transform).is_empty(), "{:?}", panel.entries[0].errors(EntryKind::Transform));
        panel.use_entry(&mut cm, EntryKind::Transform, 0);
        assert_eq!(panel.text, text, "a name derives its id: nothing written");
        let key = cm.config().flame.custom_variations[0].entry.clone();
        assert_eq!(crate::variations::custom::key_of(text).as_deref(), Some(key.as_str()), "{:?}", panel.status);
        cm.config_mut().flame.transforms.push(crate::scene::transforms::Transform::default());
        cm.config_mut().flame.transforms[0].variations.insert(key.clone(), 0.5);
        cm.config_mut().flame.transforms[0].variation_params.insert(format!("{key}.k"), 0.75);

        panel.text = text.replace("z * p.k", "z * p.k * 0.5");
        panel.recheck();
        let before = cm.history_len();
        panel.use_entry(&mut cm, EntryKind::Transform, 0);
        let c = cm.config();
        let new_key = c.flame.custom_variations[0].entry.clone();
        assert_ne!(new_key, key, "a new version, a new key");
        assert_eq!(crate::scripted::key_id(&new_key), crate::scripted::key_id(&key), "of the same id");
        assert_eq!(c.flame.custom_variations.len(), 1, "the old source goes");
        let t = &c.flame.transforms[0];
        assert_eq!(t.variations.get(&new_key), Some(&0.5));
        assert!(!t.variations.contains_key(&key));
        assert_eq!(t.variation_params.get(&format!("{new_key}.k")), Some(&0.75));
        assert_eq!(cm.history_len() - before, 1, "one undo step");
        cm.undo().unwrap();
        assert_eq!(cm.config().flame.transforms[0].variations.get(&key), Some(&0.5));
        assert_eq!(cm.config().flame.custom_variations[0].entry, key);
        assert!(crate::variations::global_registry().has(&key), "the old version still renders");

        let bare = "const INFO = #{ title: \"Bare\" };\nfn variation(z, p) {\n    z\n}\n";
        panel.adopt(None, "bare.rhai".into(), bare.into());
        panel.recheck();
        assert!(panel.entries[0].errors(EntryKind::Transform).is_empty(), "{:?}", panel.entries[0].errors(EntryKind::Transform));
        panel.use_entry(&mut cm, EntryKind::Transform, 0);
        let id = lang::rhai::info_string(&panel.text, "id").expect("an id in the file");
        assert!(crate::scripted::is_uuid(&id), "{id}: {:?}", panel.status);
        let bare_key = cm.config().flame.custom_variations.iter().map(|s| s.entry.clone()).find(|e| crate::scripted::key_id(e).as_deref() == Some(id.as_str()));
        let mut registry = crate::variations::global_registry_mut();
        for k in [Some(key), Some(new_key), bare_key].into_iter().flatten() {
            registry.remove_by_name(&k);
        }
    }
}
