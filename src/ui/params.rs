//! One renderer for every kind's parameters
//! (docs/projects/one-parameter-system.md): a control per
//! [`ParamKind`] ([`control`]), and a parameter set drawn whole
//! ([`list`]) -- its definitions in order, its layout's conditions and
//! grids -- through a [`ParamStore`], where the values live: the config
//! for a GPU kind ([`ConfigStore`]), a script's run inputs for a script
//! ([`LocalStore`]).
//!
//! What every control keeps, in every kind:
//! - **drawing never edits**: a value finer than a slider's step or
//!   outside its range stays as stored until the user moves it
//!   (`SliderClamping::Edits`, `Never` for a free number; `VkbSlider`
//!   snaps a drag alone), and an edit is written only when it differs;
//! - a **slow** parameter is a dropdown over a small whole-number range,
//!   else commits when the drag ends (`slow_controls`);
//! - a drag's preview is committed when it is released;
//! - a choice reads a stored value rounded and clamped to an entry.

use std::collections::HashMap;

use egui::{Response, RichText, Ui};

use crate::config::{ConfigError, ConfigManager, ConfigPath, UpdateType};
use crate::params::layout::{ParamGrid, ParamLayout};
use crate::params::{ParamDef, ParamKind, Value};

/// Where a parameter set's values live.
pub trait ParamStore {
    /// `def`'s value, its default where unset; `None`: the target has gone
    /// (a transform deleted under the panel), and drawing stops.
    fn get(&self, def: &ParamDef) -> Option<Value>;
    /// An edit.
    fn set(&mut self, def: &ParamDef, value: Value);
    /// A drag ended: what it previewed is committed.
    fn released(&mut self, _def: &ParamDef) {}
    /// Edits as one undo step described by `why`: an edit and the grid
    /// cells it fills.
    fn set_many(&mut self, edits: Vec<(&ParamDef, Value)>, why: &str);
}

/// A GPU kind's values: f32 slots in the config, read by `read` (the
/// default where unset; `None` when the target has gone) and written
/// through the [`ConfigManager`] at `path`.
pub struct ConfigStore<'a> {
    pub manager: &'a mut ConfigManager,
    read: Box<dyn Fn(&ConfigManager, &ParamDef) -> Option<f32> + 'a>,
    path: Box<dyn Fn(&str) -> ConfigPath + 'a>,
    /// The largest update the edits asked for.
    pub update: UpdateType,
}

impl<'a> ConfigStore<'a> {
    pub fn new(
        manager: &'a mut ConfigManager,
        read: impl Fn(&ConfigManager, &ParamDef) -> Option<f32> + 'a,
        path: impl Fn(&str) -> ConfigPath + 'a,
    ) -> Self {
        Self { manager, read: Box::new(read), path: Box::new(path), update: UpdateType::None }
    }

    fn note(&mut self, r: Result<UpdateType, ConfigError>) {
        if let Ok(u) = r {
            self.update = self.update.max(u);
        }
    }
}

impl ParamStore for ConfigStore<'_> {
    fn get(&self, def: &ParamDef) -> Option<Value> {
        (self.read)(self.manager, def).map(|v| def.kind.from_slot(v))
    }

    fn set(&mut self, def: &ParamDef, value: Value) {
        let r = self.manager.update_param((self.path)(&def.name), def.kind.to_slot(&value).into());
        self.note(r);
    }

    fn released(&mut self, def: &ParamDef) {
        let _ = self.manager.force_commit_preview(&(self.path)(&def.name));
    }

    fn set_many(&mut self, edits: Vec<(&ParamDef, Value)>, why: &str) {
        let changes = edits.into_iter().map(|(d, v)| ((self.path)(&d.name), d.kind.to_slot(&v).into())).collect();
        let r = self.manager.update_batch(changes, why.to_string());
        self.note(r);
    }
}

/// A script's run inputs: the panel's own, without undo or animation,
/// set only where the user set one (so a script keeps its defaults). A
/// value of another kind than its parameter's -- the script declared it
/// anew since -- reads as the default.
#[allow(dead_code)] // The Scripts panel's, from step 8 of the plan.
pub struct LocalStore<'a>(pub &'a mut HashMap<String, Value>);

impl ParamStore for LocalStore<'_> {
    fn get(&self, def: &ParamDef) -> Option<Value> {
        Some(self.0.get(&*def.name).filter(|v| def.kind.holds(v)).cloned().unwrap_or_else(|| def.kind.default_value()))
    }

    fn set(&mut self, def: &ParamDef, value: Value) {
        self.0.insert(def.name.to_string(), value);
    }

    fn set_many(&mut self, edits: Vec<(&ParamDef, Value)>, _why: &str) {
        for (d, v) in edits {
            self.set(d, v);
        }
    }
}

/// What a control did this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edit {
    pub changed: bool,
    /// A drag ended, or a slow control committed: the preview is
    /// committed.
    pub released: bool,
}

/// One parameter's control, editing `value`; `salt` separates sections
/// whose parameters share names (a formula's and a colouring's
/// `variant`). A value of another kind draws as the default.
pub fn control(ui: &mut Ui, def: &ParamDef, value: &mut Value, salt: &str) -> Edit {
    if !def.kind.holds(value) {
        let mut d = def.kind.default_value();
        let edit = control(ui, def, &mut d, salt);
        if edit.changed {
            *value = d;
        }
        return edit;
    }
    let id = (salt, &*def.name);
    let tip = |r: Response| match def.hint() {
        Some(h) => r.on_hover_text(h),
        None => r,
    };
    let label = &*def.label;
    match (&def.kind, value) {
        (&ParamKind::Float { min, max, free, log, angle, .. }, Value::Float(v)) => {
            let add = |ui: &mut Ui, v: &mut f64| {
                let mut s = super::VkbSlider::new(v, min..=max).text(label).clamping(clamping(free));
                // Logarithmic over a range above zero; a linear drag of
                // the value field is fine-grained, at a 500th of the range.
                s = if log && min > 0.0 { s.logarithmic(true) } else { s.drag_value_speed(((max - min) / 500.0).max(1e-6)) };
                if angle {
                    s = s.suffix("°");
                }
                tip(ui.add(s))
            };
            let edit = number(ui, def, v, (min, max), salt, add);
            if edit.changed && free {
                *v = v.clamp(f32::MIN.into(), f32::MAX.into());
            }
            edit
        }
        (&ParamKind::Int { min, max, free, .. }, Value::Int(v)) => {
            let add = |ui: &mut Ui, v: &mut i64| tip(ui.add(super::VkbSlider::new(v, min..=max).text(label).clamping(clamping(free))));
            number(ui, def, v, (min as f64, max as f64), salt, add)
        }
        (ParamKind::Bool { .. }, Value::Bool(b)) => {
            let r = tip(ui.checkbox(b, label));
            Edit { changed: r.changed(), released: false }
        }
        (ParamKind::Choice { options, .. }, Value::Choice(i)) => {
            if options.is_empty() {
                ui.label(format!("{label}: (no choices)"));
                return Edit::default();
            }
            let before = (*i).min(options.len() - 1);
            let mut picked = before;
            ui.horizontal(|ui| {
                ui.label(label);
                let r = egui::ComboBox::from_id_salt(id)
                    .selected_text(options.get(picked).unwrap_or_default())
                    .show_ui(ui, |ui| {
                        for (k, o) in options.iter().enumerate() {
                            ui.selectable_value(&mut picked, k, o);
                        }
                    })
                    .response;
                tip(r);
            });
            if picked == before {
                return Edit::default();
            }
            *i = picked;
            Edit { changed: true, released: false }
        }
        (ParamKind::Text { max_len, .. }, Value::Text(s)) => {
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label(label);
                // Keyed by parameter, not by position: the list is rebuilt
                // every frame, and a positional id lets the caret and the
                // selection bleed between parameters.
                let r = ui.add(egui::TextEdit::singleline(s).id_salt(id).char_limit(*max_len).desired_width(f32::INFINITY));
                crate::ui::vkb_sync(ui, &r, s);
                changed = tip(r).changed();
            });
            Edit { changed, released: false }
        }
        (ParamKind::Color { .. }, Value::Color(c)) => {
            let mut changed = false;
            ui.horizontal(|ui| {
                changed = tip(ui.color_edit_button_rgb(c)).changed();
                ui.label(label);
            });
            Edit { changed, released: false }
        }
        _ => unreachable!("the value is of its kind"),
    }
}

/// A free number takes a typed value past its slider's range; a bounded
/// one refuses it, and neither rewrites what it is handed.
fn clamping(free: bool) -> egui::SliderClamping {
    if free {
        egui::SliderClamping::Never
    } else {
        egui::SliderClamping::Edits
    }
}

/// A number a slider edits: a float's f64, an int's i64.
trait Number: Clone + PartialEq + Send + Sync + 'static {
    fn to_f32(&self) -> f32;
    fn from_f32(v: f32) -> Self;
}

impl Number for f64 {
    fn to_f32(&self) -> f32 {
        *self as f32
    }
    fn from_f32(v: f32) -> Self {
        f64::from(v)
    }
}

impl Number for i64 {
    fn to_f32(&self) -> f32 {
        *self as f32
    }
    fn from_f32(v: f32) -> Self {
        v as i64
    }
}

/// A number's control: `add`'s slider, live. A slow one's next frame is
/// slow, so a live drag would overshoot: over a small whole-number range
/// it is a dropdown, else the slider commits when the drag ends -- either
/// way at once, with no preview.
fn number<T: Number>(ui: &mut Ui, def: &ParamDef, v: &mut T, (min, max): (f64, f64), salt: &str, add: impl FnOnce(&mut Ui, &mut T) -> Response) -> Edit {
    if !def.slow {
        let r = add(ui, v);
        return Edit { changed: r.changed(), released: r.drag_stopped() };
    }
    let changed = if super::slow_controls::fits_a_dropdown(min as f32, max as f32) {
        let mut f = v.to_f32();
        let changed = super::slow_controls::whole_numbers(ui, (salt, &*def.name), &def.label, def.hint(), &mut f, min as i32, max as i32);
        if changed {
            *v = T::from_f32(f);
        }
        changed
    } else {
        let id = ui.id().with((salt, &*def.name, "on_release"));
        super::slow_controls::on_release(ui, id, v, add).1
    };
    Edit { changed, released: changed }
}

/// A parameter set to draw: its definitions, in order, and how.
pub struct List<'a> {
    pub defs: &'a [ParamDef],
    pub layout: Option<&'a ParamLayout>,
    /// As [`control`]'s.
    pub salt: &'a str,
    /// Parameters another control draws (a solid's lighting, McCabe's
    /// per-scale warps).
    pub hide: &'a dyn Fn(&ParamDef) -> bool,
    /// The simulation grid's size, which a grid's warnings read.
    pub grid_size: (u32, u32),
    /// The undo step's description when an edit fills a grid.
    pub fill_label: &'a str,
}

fn shown(_: &ParamDef) -> bool {
    false
}

impl<'a> List<'a> {
    pub fn new(defs: &'a [ParamDef], salt: &'a str) -> Self {
        Self { defs, layout: None, salt, hide: &shown, grid_size: (0, 0), fill_label: "history.action.param_table" }
    }

    pub fn layout(self, layout: Option<&'a ParamLayout>) -> Self {
        Self { layout, ..self }
    }

    pub fn hide(self, hide: &'a dyn Fn(&ParamDef) -> bool) -> Self {
        Self { hide, ..self }
    }

    pub fn grid_size(self, grid_size: (u32, u32)) -> Self {
        Self { grid_size, ..self }
    }

    pub fn fill_label(self, fill_label: &'a str) -> Self {
        Self { fill_label, ..self }
    }

    /// The f32 slot of parameter `name`, as a layout reads it (0 for a
    /// name it does not define).
    fn slot<S: ParamStore + ?Sized>(&self, store: &S, name: &str) -> f32 {
        self.defs.iter().find(|d| d.name == name).and_then(|d| Some(d.kind.to_slot(&store.get(d)?))).unwrap_or(0.0)
    }
}

/// Draw a parameter set: each parameter its layout lists, then its grids.
pub fn list<S: ParamStore>(ui: &mut Ui, store: &mut S, l: &List) {
    list_with(ui, store, l, |_, _, _| {});
}

/// [`list`], with `after` drawn after each parameter's control (a
/// complex parameter's polar form, after its parts).
pub fn list_with<S: ParamStore>(ui: &mut Ui, store: &mut S, l: &List, mut after: impl FnMut(&mut Ui, &mut S, &ParamDef)) {
    for def in l.defs {
        if (l.hide)(def) || l.layout.is_some_and(|layout| !layout.lists(&def.name, &|n| l.slot(&*store, n))) {
            continue;
        }
        let Some(stored) = store.get(def) else { return };
        let mut value = stored.clone();
        let edit = control(ui, def, &mut value, l.salt);
        if edit.changed && value != stored {
            commit(store, l, def, value);
        }
        if edit.released {
            store.released(def);
        }
        after(ui, store, def);
    }
    for g in l.layout.iter().flat_map(|layout| layout.grids.iter()).filter(|g| g.drawn) {
        grid(ui, store, l, g);
    }
}

/// An edit of `def`, with the grid cells it fills: an edit that turns a
/// grid on fills it in the same undo step, so the switch changes nothing
/// on screen.
fn commit<S: ParamStore + ?Sized>(store: &mut S, l: &List, def: &ParamDef, value: Value) {
    let fills = l
        .layout
        .map(|layout| {
            let before = |n: &str| l.slot(&*store, n);
            let slot = def.kind.to_slot(&value);
            layout.fills(&before, &|n| if n == def.name { slot } else { before(n) })
        })
        .unwrap_or_default();
    if fills.is_empty() {
        store.set(def, value);
        return;
    }
    let mut edits = vec![(def, value)];
    edits.extend(fills.into_iter().filter_map(|(name, v)| {
        let d = l.defs.iter().find(|d| d.name == name)?;
        Some((d, d.kind.from_slot(v)))
    }));
    store.set_many(edits, l.fill_label);
}

/// A grid (rhai-plugins P7; McCabe's per-scale table): its live rows by
/// its shown columns, each cell its parameter's value, a row its grid
/// warns of marked. Hidden, a grid with a fill draws the warning its
/// fill earns -- the coarsest row's -- in a line.
fn grid<S: ParamStore>(ui: &mut Ui, store: &mut S, l: &List, g: &ParamGrid) {
    let value = |n: &str| l.slot(&*store, n);
    let rows = g.live(&value);
    let warn = ui.visuals().warn_fg_color;
    if !g.show_if.holds(&value) {
        if let (Some(f), Some(w)) = (&g.fill, &g.warn) {
            if let Some(msg) = (0..rows).rev().find_map(|r| w(r, &(f.cells)(&value, r), &value, l.grid_size, true)) {
                ui.label(RichText::new(msg).small().color(warn));
            }
        }
        return;
    }
    let columns: Vec<_> = g.columns.iter().filter(|c| c.show_if.holds(&value)).collect();
    let warnings: Vec<Option<String>> = (0..rows).map(|r| g.warn.as_ref().and_then(|w| w(r, &g.row(r, &value), &value, l.grid_size, false))).collect();
    let cells: Vec<Vec<Option<(&ParamDef, f32)>>> = (0..rows)
        .map(|r| {
            columns
                .iter()
                .map(|c| {
                    let name = g.cell(r, &c.key);
                    Some((l.defs.iter().find(|d| d.name == name)?, value(&name)))
                })
                .collect()
        })
        .collect();
    let mut edits = Vec::new();
    egui::Grid::new(format!("{}_{}_grid", l.salt, g.name)).striped(true).spacing([6.0, 2.0]).show(ui, |ui| {
        ui.label("");
        for c in &columns {
            ui.label(RichText::new(c.header.resolve()).small());
        }
        ui.end_row();
        for (r, row) in cells.iter().enumerate() {
            ui.label((r + 1).to_string());
            for (c, cell) in columns.iter().zip(row) {
                let Some((def, mut v)) = *cell else {
                    ui.label("");
                    continue;
                };
                let (min, max) = def.kind.bounds().unwrap_or((0.0, 1.0));
                let speed = if c.whole { 0.05 } else { ((max - min) / 1000.0).max(1e-4) };
                let dv = egui::DragValue::new(&mut v).range(min..=max).clamp_existing_to_range(false).speed(speed);
                let dv = if c.whole { dv.fixed_decimals(0) } else { dv.max_decimals(3) };
                let r = ui.add(dv);
                let r = match def.hint() {
                    Some(h) => r.on_hover_text(h),
                    None => r,
                };
                if r.changed() {
                    edits.push((def, Some(if c.whole { v.round() } else { v })));
                }
                if r.drag_stopped() {
                    edits.push((def, None));
                }
            }
            if let Some(msg) = &warnings[r] {
                ui.label(RichText::new("⚠").color(warn)).on_hover_text(msg);
            }
            ui.end_row();
        }
    });
    for (def, v) in edits {
        match v {
            Some(v) => store.set(def, def.kind.from_slot(v)),
            None => store.released(def),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::layout::{GridColumn, GridFill, ShowIf, Text};
    use crate::params::Spec;
    use crate::scripted::Label;
    use std::sync::Arc;

    /// A store that records what reaches it.
    #[derive(Default)]
    struct Recorder {
        values: HashMap<String, Value>,
        log: Vec<String>,
    }

    impl ParamStore for Recorder {
        fn get(&self, def: &ParamDef) -> Option<Value> {
            LocalStore(&mut self.values.clone()).get(def)
        }
        fn set(&mut self, def: &ParamDef, value: Value) {
            self.log.push(format!("set {} {value:?}", def.name));
            self.values.insert(def.name.to_string(), value);
        }
        fn released(&mut self, def: &ParamDef) {
            self.log.push(format!("released {}", def.name));
        }
        fn set_many(&mut self, edits: Vec<(&ParamDef, Value)>, why: &str) {
            let names: Vec<_> = edits.iter().map(|(d, _)| d.name.to_string()).collect();
            self.log.push(format!("set_many {why} {}", names.join(",")));
            for (d, v) in edits {
                self.values.insert(d.name.to_string(), v);
            }
        }
    }

    fn every_kind() -> Vec<ParamDef> {
        let owned = |name: &'static str, kind| ParamDef::owned(name, name, "", kind);
        vec![
            ParamDef::new("f", "F", "A float.", Spec::float(0.5, 0.0, 0.2)),
            ParamDef::new("free", "Free", "", Spec::float(0.0, -1.0, 1.0).free()),
            ParamDef::new("turn", "Turn", "", Spec::angle(0.0, -360.0, 360.0)),
            ParamDef::new("k", "K", "", Spec::float(1e-3, 1e-6, 1.0).log()),
            ParamDef::new("n", "N", "", Spec::int(3, 1, 10)),
            ParamDef::new("big", "Big", "", Spec::int(0, -100, 100).free()),
            ParamDef::new("on", "On", "", Spec::bool(true)),
            ParamDef::new("mode", "Mode", "", Spec::choice(0, &["A", "B", "C"])),
            ParamDef::new("depth", "Depth", "", Spec::float(4.0, 1.0, 12.0).slow()),
            ParamDef::new("steps", "Steps", "", Spec::int(40, 1, 400).slow()),
            owned("rule", ParamKind::Text { default: "F+F".into(), max_len: 32 }),
            owned("tint", ParamKind::Color { default: [0.1, 0.2, 0.3] }),
        ]
    }

    /// Draw `defs` through `store` for a few frames with no input,
    /// reporting the parameters drawn.
    fn draw(defs: &[ParamDef], layout: Option<&ParamLayout>, store: &mut Recorder) -> Vec<String> {
        let ctx = egui::Context::default();
        let mut drawn = Vec::new();
        for _ in 0..3 {
            drawn.clear();
            let _ = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    list_with(ui, store, &List::new(defs, "test").layout(layout), |_, _, d| drawn.push(d.name.to_string()));
                });
            });
        }
        drawn
    }

    /// Drawing never edits, in any kind of control: a value finer than a
    /// slider's step, outside its range, off a choice's entries, or of
    /// another kind stays as stored.
    #[test]
    fn drawing_edits_nothing_in_any_control() {
        let defs = every_kind();
        let mut store = Recorder::default();
        for (name, v) in [
            ("f", Value::Float(0.006)),
            ("free", Value::Float(7.5)),
            ("turn", Value::Float(500.0)),
            ("k", Value::Float(3e-7)),
            ("n", Value::Int(40)),
            ("big", Value::Int(-1000)),
            ("mode", Value::Choice(9)),
            ("depth", Value::Float(2.5)),
            ("steps", Value::Int(1000)),
            ("on", Value::Float(1.0)),
        ] {
            store.values.insert(name.into(), v);
        }
        let before = store.values.clone();
        let drawn = draw(&defs, None, &mut store);
        assert_eq!(drawn.len(), defs.len(), "every kind drawn: {drawn:?}");
        assert!(store.log.is_empty(), "drawing wrote: {:?}", store.log);
        assert_eq!(store.values, before);
    }

    /// A target gone stops the list rather than inventing a value.
    #[test]
    fn a_gone_target_stops_drawing() {
        struct Gone;
        impl ParamStore for Gone {
            fn get(&self, _: &ParamDef) -> Option<Value> {
                None
            }
            fn set(&mut self, _: &ParamDef, _: Value) {
                panic!("no target")
            }
            fn set_many(&mut self, _: Vec<(&ParamDef, Value)>, _: &str) {
                panic!("no target")
            }
        }
        let ctx = egui::Context::default();
        let mut after = 0;
        let _ = ctx.run_ui(Default::default(), |ui| list_with(ui, &mut Gone, &List::new(&every_kind(), "gone"), |_, _, _| after += 1));
        assert_eq!(after, 0);
    }

    /// A layout's conditions and grid: the parameters listed at these
    /// values, and an edit that turns the grid on fills its cells in one
    /// step, while one that does not is a single edit.
    #[test]
    fn a_layout_lists_by_its_conditions_and_fills_its_grid_on_the_switch() {
        let defs = vec![
            ParamDef::new("table", "Table", "", Spec::bool(false)),
            ParamDef::new("base", "Base", "", Spec::float(2.0, 0.0, 10.0)),
            ParamDef::new("s0_a", "s0 a", "", Spec::float(0.0, 0.0, 10.0)),
            ParamDef::new("s1_a", "s1 a", "", Spec::float(0.0, 0.0, 10.0)),
        ];
        let on = ShowIf(vec![("table".into(), vec![1.0])]);
        let layout = ParamLayout {
            show_if: vec![("base".into(), ShowIf(vec![("table".into(), vec![0.0])]))],
            grids: vec![ParamGrid {
                name: "g".into(),
                cell: "s{row}_{column}".into(),
                rows: 2,
                live_rows: None,
                columns: vec![GridColumn { key: "a".into(), header: Text::Label(Label::text("a")), whole: false, show_if: ShowIf::default() }],
                show_if: on.clone(),
                fill: Some(GridFill { when: on, cells: Arc::new(|v, r| [("a".to_string(), v("base") * (r + 1) as f32)].into()) }),
                warn: None,
                drawn: true,
            }],
        };
        let mut store = Recorder::default();
        assert_eq!(draw(&defs, Some(&layout), &mut store), ["table", "base"], "a grid's cells are its own, not the list's");
        let l = List::new(&defs, "test").layout(Some(&layout));
        commit(&mut store, &l, &defs[1], Value::Float(3.0));
        commit(&mut store, &l, &defs[0], Value::Bool(true));
        assert_eq!(store.log, ["set base Float(3.0)", "set_many history.action.param_table table,s0_a,s1_a"]);
        assert_eq!((store.values["s0_a"].clone(), store.values["s1_a"].clone()), (Value::Float(3.0), Value::Float(6.0)));
        assert_eq!(draw(&defs, Some(&layout), &mut store), ["table"], "base is listed while the table is off");
    }

    /// A script's store: a value of another kind than its parameter's
    /// reads as the default, and only a set value is kept.
    #[test]
    fn a_local_store_keeps_only_what_was_set() {
        let defs = every_kind();
        let mut values = HashMap::from([("n".to_string(), Value::Float(2.0))]);
        let mut store = LocalStore(&mut values);
        assert_eq!(store.get(&defs[4]), Some(Value::Int(3)));
        store.set(&defs[6], Value::Bool(false));
        assert_eq!(values.len(), 2);
        assert_eq!(values["on"], Value::Bool(false));
    }
}
