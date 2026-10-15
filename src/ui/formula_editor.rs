//! The Formula Editor: Ultra Fractal and Fractint formula files, checked
//! as they are typed and used as the escape formula or colouring
//! (`docs/projects/escape-formula-language.md`; the language is in
//! `docs/main/ESCAPE.md`, "The formula language").
//!
//! A file holds entries, listed above the source. Each is checked as a
//! formula and as a colouring on a pause in typing, and offered as
//! whichever it compiles as. Using
//! one compiles it (`escape::custom`) and puts its source in the config,
//! one undo step, so the file renders anywhere. With Live on, an entry
//! in use is re-applied whenever an edit to it compiles again.

use std::time::Duration;

use crate::config::escape::{CustomDialect, CustomSource};
use crate::config::manager::custom_source_json;
use crate::config::{ConfigManager, ConfigPath, ConfigValue};
use crate::escape::custom;
use crate::escape::lang::{self, EntryKind, Severity};
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
    /// `line:col: message` per error, as a formula and as a colouring.
    as_formula: Vec<String>,
    as_coloring: Vec<String>,
}

impl EntryRow {
    fn errors(&self, kind: EntryKind) -> &[String] {
        match kind {
            EntryKind::Formula => &self.as_formula,
            EntryKind::Coloring => &self.as_coloring,
        }
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
        self.dialect = if formula_store::is_fractint(&name) { CustomDialect::Fractint } else { CustomDialect::UltraFractal };
        self.file = file;
        self.name = name;
        self.saved = text.clone();
        self.text = text;
        self.checked = None;
        self.status = None;
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

    /// Parse the file and check every entry both ways.
    fn recheck(&mut self) {
        let dialect = match self.dialect {
            // The text editor offers UF and Fractint; a hybrid has its
            // own panel.
            CustomDialect::UltraFractal | CustomDialect::Hybrid => lang::Dialect::UltraFractal,
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
                        EntryKind::Coloring => lang::check::check(e, kind, dialect),
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
                let likely = if coloring_file || e.section(lang::ast::SectionKind::Final).is_some() {
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
                }
            })
            .collect();
        self.checked = Some(text);
    }

    /// Compile an entry and put it in the config, one undo step. The
    /// first time an entry is used, its `default:` view (centre and
    /// iterations) comes with it.
    fn use_entry(&mut self, cm: &mut ConfigManager, kind: EntryKind, row: usize) {
        let Some(entry) = self.entries.get(row) else { return };
        let src = CustomSource { dialect: self.dialect, entry: entry.name.clone(), source: entry.source.clone() };
        let name = custom::name_of(kind, &src);
        let compiled = match kind {
            EntryKind::Formula => custom::compile_formula(&src, &name).map(|_| ()),
            EntryKind::Coloring => custom::compile_coloring(&src, &name).map(|_| ()),
        };
        if let Err(d) = compiled {
            let first = d.first().map(|x| x.render(&src.source)).unwrap_or_default();
            self.status = Some(t!("formula_editor.did_not_compile", entry = src.entry, error = first).to_string());
            return;
        }
        let escape = &cm.config().escape;
        let (current, path_src, path_name, description) = match kind {
            EntryKind::Formula => (
                escape.custom_formula.as_ref(),
                ConfigPath::EscapeCustomFormula,
                ConfigPath::EscapeFormula,
                "history.action.use_custom_formula",
            ),
            EntryKind::Coloring => (
                escape.custom_coloring.as_ref(),
                ConfigPath::EscapeCustomColoring,
                ConfigPath::EscapeColoring,
                "history.action.use_custom_coloring",
            ),
        };
        let first_use = current.is_none_or(|c| c.entry != src.entry);
        let mut changes = vec![
            (path_src, ConfigValue::String(custom_source_json(&Some(src.clone())))),
            (path_name, ConfigValue::String(name)),
        ];
        if first_use && kind == EntryKind::Formula {
            // The entry's own view. UF's `magn` is not applied: what span
            // its magnification 1 shows is still to be verified (plan,
            // phase 2).
            let (file, _) = lang::parser::parse_file(&src.source);
            if let Some(e) = file.entries.first() {
                let dialect = if self.dialect == CustomDialect::Fractint { lang::Dialect::Fractint } else { lang::Dialect::UltraFractal };
                if let (Some(p), _) = lang::check::check(e, kind, dialect) {
                    if let Some((re, im)) = p.view.center {
                        changes.push((ConfigPath::EscapeCenterRe, ConfigValue::String(format!("{re}"))));
                        changes.push((ConfigPath::EscapeCenterIm, ConfigValue::String(format!("{im}"))));
                    }
                    if let Some(m) = p.view.maxiter {
                        changes.push((ConfigPath::EscapeMaxIter, ConfigValue::UInt(m)));
                    }
                }
            }
        }
        match cm.update_batch(changes, description.to_string()) {
            Ok(_) => {
                self.in_use.retain(|(k, _, _)| *k != kind);
                self.in_use.push((kind, self.file.clone(), src.entry.clone()));
                self.status = Some(match kind {
                    EntryKind::Formula => t!("formula_editor.using_formula", entry = src.entry),
                    EntryKind::Coloring => t!("formula_editor.using_coloring", entry = src.entry),
                }
                .to_string());
            }
            Err(e) => self.status = Some(format!("{e:?}")),
        }
    }

    /// Live: re-apply the entries in use whose text changed and still
    /// compiles, while the config still uses them.
    fn reapply_live(&mut self, cm: &mut ConfigManager) {
        let in_use = self.in_use.clone();
        for (kind, file, entry) in in_use {
            if file != self.file {
                continue;
            }
            let escape = &cm.config().escape;
            let (carried, name) = match kind {
                EntryKind::Formula => (escape.custom_formula.as_ref(), &escape.formula),
                EntryKind::Coloring => (escape.custom_coloring.as_ref(), &escape.coloring),
            };
            let Some(carried) = carried else { continue };
            if carried.entry != entry || !name.starts_with(custom::PREFIX) {
                continue;
            }
            let Some(row) = self.entries.iter().position(|r| r.name == entry) else { continue };
            let r = &self.entries[row];
            if r.errors(kind).is_empty() && (r.source != carried.source || carried.dialect != self.dialect) {
                self.use_entry(cm, kind, row);
            }
        }
    }

    fn save(&mut self) {
        match formula_store::save(&self.name, &self.text) {
            Ok(file) => {
                self.saved = self.text.clone();
                self.status = Some(t!("formula_editor.saved", file = file).to_string());
                self.name = file.clone();
                self.file = Some(file);
                self.files = formula_store::list();
            }
            Err(e) => self.status = Some(e),
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, cm: &mut ConfigManager, #[allow(unused_variables)] window: &winit::window::Window) {
        // The browser's picker leaves the opened text in egui's temp store.
        #[cfg(target_arch = "wasm32")]
        if let Some(text) = ui.ctx().data_mut(|d| d.remove_temp::<String>(egui::Id::new(PENDING_FORMULA_LOAD))) {
            self.adopt(None, "opened.ufm".to_string(), text);
        }

        // ---- Files ----
        ui.horizontal_wrapped(|ui| {
            let dirty = self.text != self.saved;
            let current = format!("{}{}", self.name, if dirty { " *" } else { "" });
            let mut open: Option<String> = None;
            let mut example: Option<usize> = None;
            egui::ComboBox::from_id_salt("formula_editor_file")
                .selected_text(current)
                .width(180.0)
                .show_ui(ui, |ui| {
                    for f in &self.files {
                        if ui.selectable_label(self.file.as_deref() == Some(f.as_str()), f).clicked() {
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
                });
            if let Some(f) = open {
                self.open_stored(&f);
            }
            if let Some(i) = example {
                self.open_example(i);
            }
            if ui.button(t!("formula_editor.new")).on_hover_text(t!("formula_editor.new_tip")).clicked() {
                self.adopt(None, "untitled.ufm".to_string(), NEW_FILE.to_string());
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
                    self.status = Some(match formula_store::delete(&file) {
                        Ok(()) => t!("formula_editor.deleted", file = file).to_string(),
                        Err(e) => e,
                    });
                    self.files = formula_store::list();
                    self.file = None;
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
                    crate::app::trigger_browser_file_picker(".ufm,.ucl,.frm", ui.ctx().clone(), PENDING_FORMULA_LOAD);
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
        for (i, row) in self.entries.iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                ui.strong(&row.name);
                let order = match row.likely {
                    EntryKind::Formula => [EntryKind::Formula, EntryKind::Coloring],
                    EntryKind::Coloring => [EntryKind::Coloring, EntryKind::Formula],
                };
                for kind in order {
                    let ok = row.errors(kind).is_empty();
                    let label = match kind {
                        EntryKind::Formula => t!("formula_editor.use_formula"),
                        EntryKind::Coloring => t!("formula_editor.use_coloring"),
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
        ui.separator();
        // ---- The source ----
        //
        // In a scroll area of its own: a long file would otherwise make
        // the panel scroll to the editor's cursor, and carry the entries
        // and the file controls out of view.
        let mut changed = false;
        egui::ScrollArea::vertical().id_salt("formula_editor_scroll").max_height(360.0).show(ui, |ui| {
            let editor = ui.add(
                egui::TextEdit::multiline(&mut self.text)
                    .id_salt("formula_editor_source")
                    .code_editor()
                    .desired_rows(16)
                    .desired_width(f32::INFINITY),
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

/// What New starts from.
const NEW_FILE: &str = "\
; A new formula file, in Ultra Fractal's language.

MyFormula {
init:
  z = 0
loop:
  z = sqr(z) + #pixel
bailout:
  |z| < @bailout
default:
  title = \"My Formula\"
  float param bailout
    default = 10000
  endparam
}
";
