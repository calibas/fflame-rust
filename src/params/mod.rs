//! One parameter system for every kind that has parameters -- flame
//! variations, effects, escape formulas and colourings, simulations,
//! CPU scripts (docs/projects/one-parameter-system.md): one definition
//! ([`ParamDef`]), one layout ([`layout`]), one renderer (`ui::params`).
//!
//! Each kind keeps its values where they were: a GPU kind's are f32
//! slots in its config place (read and written through
//! [`ParamKind::from_slot`] / [`ParamKind::to_slot`]), a script's are
//! [`Value`]s, its run inputs.
//!
//! Built-ins are statics written in one const call,
//! `ParamDef::new("feed", "Feed rate", "How fast.", Spec::float(0.055, 0.0, 0.1).slow())`:
//! [`Spec`] is a `Copy` builder because a const function cannot drop
//! what it replaces, which rules out chained builders on [`ParamDef`]
//! itself. A script's are built owned ([`ParamDef::owned`]) each time it
//! declares them.

pub mod layout;
#[cfg(all(test, feature = "engine-flame", feature = "engine-escape", feature = "engine-sim"))]
mod snapshot;

use std::borrow::Cow;

/// A parameter: the key its value is stored under, its label and hint,
/// its kind.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamDef {
    pub name: Cow<'static, str>,
    pub label: Cow<'static, str>,
    /// The tooltip; empty for none.
    pub hint: Cow<'static, str>,
    pub kind: ParamKind,
    /// A change makes the next frame slow (it bounds a loop the kind runs
    /// every step), so a live drag would overshoot: its control is a
    /// dropdown over a small whole-number range, else commits on release.
    pub slow: bool,
}

/// What a parameter holds, with its default and range.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    /// `free`: a typed value may leave the range, and the slider keeps it
    /// (an unlimited float). `log`: the slider runs logarithmically (a
    /// range above zero). `angle`: degrees, drawn with the angle control.
    Float { default: f64, min: f64, max: f64, free: bool, log: bool, angle: bool },
    /// `free` as a float's.
    Int { default: i64, min: i64, max: i64, free: bool },
    Bool { default: bool },
    /// An index into `options`.
    Choice { default: usize, options: Options },
    /// A script's alone: a GPU kind's values are f32 slots.
    Text { default: Cow<'static, str>, max_len: usize },
    /// A script's alone, as text.
    Color { default: [f32; 3] },
}

/// A choice's entries: a built-in's static list, or a script's own.
#[derive(Clone, Debug)]
pub enum Options {
    Static(&'static [&'static str]),
    Owned(Vec<String>),
}

impl Options {
    pub fn len(&self) -> usize {
        match self {
            Options::Static(o) => o.len(),
            Options::Owned(o) => o.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, i: usize) -> Option<&str> {
        match self {
            Options::Static(o) => o.get(i).copied(),
            Options::Owned(o) => o.get(i).map(String::as_str),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        (0..self.len()).filter_map(|i| self.get(i))
    }
}

/// The same entries, however held.
impl PartialEq for Options {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

/// A parameter's value: a script's run input, or a GPU kind's f32 slot
/// read as its kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Float(f64),
    Int(i64),
    Bool(bool),
    Choice(usize),
    Text(String),
    Color([f32; 3]),
}

impl ParamKind {
    /// The default, as a value.
    pub fn default_value(&self) -> Value {
        match self {
            ParamKind::Float { default, .. } => Value::Float(*default),
            ParamKind::Int { default, .. } => Value::Int(*default),
            ParamKind::Bool { default } => Value::Bool(*default),
            ParamKind::Choice { default, .. } => Value::Choice(*default),
            ParamKind::Text { default, .. } => Value::Text(default.to_string()),
            ParamKind::Color { default } => Value::Color(*default),
        }
    }

    /// Whether `v` is a value of this kind (a script edited since a
    /// value was set can declare its parameter anew).
    pub fn holds(&self, v: &Value) -> bool {
        matches!(
            (self, v),
            (ParamKind::Float { .. }, Value::Float(_))
                | (ParamKind::Int { .. }, Value::Int(_))
                | (ParamKind::Bool { .. }, Value::Bool(_))
                | (ParamKind::Choice { .. }, Value::Choice(_))
                | (ParamKind::Text { .. }, Value::Text(_))
                | (ParamKind::Color { .. }, Value::Color(_))
        )
    }

    /// The default, as a GPU kind's slot holds it.
    pub fn slot_default(&self) -> f32 {
        self.to_slot(&self.default_value())
    }

    /// The range a GPU kind's slot is published and clamped with: a
    /// number's bounds, a choice's indices; none for a switch (whose two
    /// values need none), text or a colour.
    pub fn bounds(&self) -> Option<(f64, f64)> {
        match self {
            ParamKind::Float { min, max, .. } => Some((*min, *max)),
            ParamKind::Int { min, max, .. } => Some((*min as f64, *max as f64)),
            ParamKind::Choice { options, .. } => Some((0.0, options.len().saturating_sub(1) as f64)),
            ParamKind::Bool { .. } | ParamKind::Text { .. } | ParamKind::Color { .. } => None,
        }
    }

    /// The range a GPU kind's slot holds: [`Self::bounds`], a switch's
    /// 0..1.
    pub fn slot_range(&self) -> (f32, f32) {
        let (lo, hi) = self.bounds().unwrap_or((0.0, 1.0));
        (lo as f32, hi as f32)
    }

    /// A choice's entries.
    pub fn options(&self) -> Option<&Options> {
        match self {
            ParamKind::Choice { options, .. } => Some(options),
            _ => None,
        }
    }

    /// An f32 slot read as this kind: an int truncated (as its slider
    /// reads it), a switch on above one half, a choice rounded and
    /// clamped to an entry (a value saved before the list existed, or set
    /// by a script or a key mid-way, lands on a real one). Text and a
    /// colour hold no slot, so read as their defaults.
    pub fn from_slot(&self, v: f32) -> Value {
        match self {
            ParamKind::Float { .. } => Value::Float(f64::from(v)),
            ParamKind::Int { .. } => Value::Int(v as i64),
            ParamKind::Bool { .. } => Value::Bool(v > 0.5),
            ParamKind::Choice { options, .. } => {
                let last = options.len().saturating_sub(1);
                Value::Choice(if v.is_finite() { (v.round().max(0.0) as usize).min(last) } else { 0 })
            }
            ParamKind::Text { .. } | ParamKind::Color { .. } => self.default_value(),
        }
    }

    /// A value as an f32 slot holds it (text and a colour hold none: 0).
    pub fn to_slot(&self, v: &Value) -> f32 {
        match v {
            Value::Float(f) => *f as f32,
            Value::Int(i) => *i as f32,
            Value::Bool(b) => f32::from(u8::from(*b)),
            Value::Choice(i) => *i as f32,
            Value::Text(_) | Value::Color(_) => 0.0,
        }
    }
}

impl ParamDef {
    /// A built-in, in one const call.
    pub const fn new(name: &'static str, label: &'static str, hint: &'static str, spec: Spec) -> Self {
        let kind = match spec.kind {
            SpecKind::Float { default, min, max, free, log, angle } => {
                ParamKind::Float { default: default as f64, min: min as f64, max: max as f64, free, log, angle }
            }
            SpecKind::Int { default, min, max, free } => ParamKind::Int { default, min, max, free },
            SpecKind::Bool { default } => ParamKind::Bool { default },
            SpecKind::Choice { default, options } => ParamKind::Choice { default, options: Options::Static(options) },
        };
        Self { name: Cow::Borrowed(name), label: Cow::Borrowed(label), hint: Cow::Borrowed(hint), kind, slow: spec.slow }
    }

    /// One built at run time: a script's, or one downloaded.
    pub fn owned(name: impl Into<Cow<'static, str>>, label: impl Into<Cow<'static, str>>, hint: impl Into<Cow<'static, str>>, kind: ParamKind) -> Self {
        Self { name: name.into(), label: label.into(), hint: hint.into(), kind, slow: false }
    }

    /// The tooltip, if it has one.
    pub fn hint(&self) -> Option<&str> {
        (!self.hint.is_empty()).then_some(&*self.hint)
    }

    /// Its default as a GPU kind's slot holds it.
    pub fn slot_default(&self) -> f32 {
        self.kind.slot_default()
    }

    /// Its slot's range ([`ParamKind::slot_range`]).
    pub fn slot_range(&self) -> (f32, f32) {
        self.kind.slot_range()
    }

    /// A choice's entries; none for another kind.
    pub fn options(&self) -> Option<&Options> {
        self.kind.options()
    }
}

/// A parameter as the API spells it -- the variations' and the effects'
/// wire format (VARIATIONS_WIRE_FORMAT.md): its type's name
/// ([`wire_type`]), its numbers as the f32s its slot holds, a switch's
/// bounds and an empty hint null.
pub fn api_json(d: &ParamDef) -> serde_json::Value {
    let bounds = d.kind.bounds();
    serde_json::json!({
        "name": d.name,
        "display_name": d.label,
        "param_type": wire_type(&d.kind),
        "default_value": d.slot_default(),
        "min_value": bounds.map(|b| b.0 as f32),
        "max_value": bounds.map(|b| b.1 as f32),
        "description": d.hint(),
    })
}

/// A kind's name on the wire: `float`, `unlimited_float` (a free
/// float), `angle`, `integer`, `unlimited_integer`, `boolean`, `enum`
/// with its choices. Text and a colour are a script's, never a
/// variation's or an effect's.
pub fn wire_type(k: &ParamKind) -> serde_json::Value {
    match k {
        ParamKind::Float { angle: true, .. } => "angle".into(),
        ParamKind::Float { free: true, .. } => "unlimited_float".into(),
        ParamKind::Float { .. } => "float".into(),
        ParamKind::Int { free: true, .. } => "unlimited_integer".into(),
        ParamKind::Int { .. } => "integer".into(),
        ParamKind::Bool { .. } => "boolean".into(),
        ParamKind::Choice { options, .. } => serde_json::json!({ "enum": { "choices": options.iter().collect::<Vec<_>>() } }),
        ParamKind::Text { .. } => "text".into(),
        ParamKind::Color { .. } => "color".into(),
    }
}

/// A parameter downloaded (a variation's or an effect's), a missing
/// bound filled as its control draws one: `float` the range an unranged
/// float is drawn over (a variation's -10..10, an effect's 0..1); an
/// unlimited float -10..10, an integer 1..10, an unlimited one
/// -100..100, an angle 0..360.
pub fn from_api(p: &crate::api::types::ApiVariationParameter, float: (f32, f32)) -> ParamDef {
    use crate::api::types::ApiParamType as T;
    let range = |(lo, hi): (f32, f32)| (f64::from(p.min_value.unwrap_or(lo)), f64::from(p.max_value.unwrap_or(hi)));
    let number = |fallback, free, angle| {
        let (min, max) = range(fallback);
        ParamKind::Float { default: p.default_value.into(), min, max, free, log: false, angle }
    };
    let int = |fallback, free| {
        let (min, max) = range(fallback);
        ParamKind::Int { default: p.default_value as i64, min: min as i64, max: max as i64, free }
    };
    let kind = match &p.param_type {
        T::Float => number(float, false, false),
        T::UnlimitedFloat => number((-10.0, 10.0), true, false),
        T::Angle => number((0.0, 360.0), false, true),
        T::Integer => int((1.0, 10.0), false),
        T::UnlimitedInteger => int((-100.0, 100.0), true),
        T::Boolean => ParamKind::Bool { default: p.default_value > 0.5 },
        T::Enum { choices } => ParamKind::Choice { default: p.default_value.round().max(0.0) as usize, options: Options::Owned(choices.clone()) },
    };
    ParamDef::owned(p.name.clone(), p.display_name.clone(), p.description.clone().unwrap_or_default(), kind)
}

/// A built-in's kind, for [`ParamDef::new`]. Its numbers are f32, the
/// GPU kinds' slots, so a default is the f32 its literal names, and the
/// f64 the definition stores holds it exactly.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    kind: SpecKind,
    slow: bool,
}

#[derive(Clone, Copy, Debug)]
enum SpecKind {
    Float { default: f32, min: f32, max: f32, free: bool, log: bool, angle: bool },
    Int { default: i64, min: i64, max: i64, free: bool },
    Bool { default: bool },
    Choice { default: usize, options: &'static [&'static str] },
}

impl Spec {
    pub const fn float(default: f32, min: f32, max: f32) -> Self {
        Self { kind: SpecKind::Float { default, min, max, free: false, log: false, angle: false }, slow: false }
    }

    /// Degrees, drawn with the angle control.
    pub const fn angle(default: f32, min: f32, max: f32) -> Self {
        Self { kind: SpecKind::Float { default, min, max, free: false, log: false, angle: true }, slow: false }
    }

    pub const fn int(default: i64, min: i64, max: i64) -> Self {
        Self { kind: SpecKind::Int { default, min, max, free: false }, slow: false }
    }

    pub const fn bool(default: bool) -> Self {
        Self { kind: SpecKind::Bool { default }, slow: false }
    }

    pub const fn choice(default: usize, options: &'static [&'static str]) -> Self {
        Self { kind: SpecKind::Choice { default, options }, slow: false }
    }

    /// A typed value may leave the range (a number's alone).
    pub const fn free(self) -> Self {
        let kind = match self.kind {
            SpecKind::Float { default, min, max, log, angle, .. } => SpecKind::Float { default, min, max, free: true, log, angle },
            SpecKind::Int { default, min, max, .. } => SpecKind::Int { default, min, max, free: true },
            _ => panic!("only a number is free"),
        };
        Self { kind, ..self }
    }

    /// A logarithmic slider (a float's alone).
    pub const fn log(self) -> Self {
        let kind = match self.kind {
            SpecKind::Float { default, min, max, free, angle, .. } => SpecKind::Float { default, min, max, free, log: true, angle },
            _ => panic!("only a float is logarithmic"),
        };
        Self { kind, ..self }
    }

    /// A change makes the next frame slow.
    pub const fn slow(self) -> Self {
        Self { slow: true, ..self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static BUILT_INS: &[ParamDef] = &[
        ParamDef::new("feed", "Feed rate", "How fast.", Spec::float(0.055, 0.0, 0.1).log().slow()),
        ParamDef::new("turn", "Turn", "", Spec::angle(-0.1, -360.0, 360.0).free()),
        ParamDef::new("n", "N", "", Spec::int(3, 1, 10).free()),
        ParamDef::new("on", "On", "", Spec::bool(true)),
        ParamDef::new("mode", "Mode", "", Spec::choice(1, &["A", "B", "C"])),
    ];

    /// A built-in is written in one const call, its flags land, and its
    /// f32 default survives the f64 exactly.
    #[test]
    fn a_built_in_holds_what_its_spec_says() {
        let [feed, turn, n, on, mode] = BUILT_INS else { unreachable!() };
        assert_eq!(feed.kind, ParamKind::Float { default: f64::from(0.055f32), min: 0.0, max: f64::from(0.1f32), free: false, log: true, angle: false });
        assert!(feed.slow && !turn.slow);
        assert_eq!(feed.kind.slot_default().to_bits(), 0.055f32.to_bits());
        assert_eq!(turn.kind.slot_default().to_bits(), (-0.1f32).to_bits());
        assert!(matches!(turn.kind, ParamKind::Float { free: true, angle: true, log: false, .. }));
        assert_eq!(n.kind, ParamKind::Int { default: 3, min: 1, max: 10, free: true });
        assert_eq!((on.kind.slot_default(), mode.kind.slot_default()), (1.0, 1.0));
        assert_eq!((feed.hint(), turn.hint()), (Some("How fast."), None));
    }

    /// A slot reads as its kind's value and writes back the same slot.
    #[test]
    fn a_slot_reads_as_its_kind() {
        let mode = &BUILT_INS[4].kind;
        assert_eq!(mode.from_slot(1.4), Value::Choice(1));
        assert_eq!(mode.from_slot(9.0), Value::Choice(2), "clamped to an entry");
        assert_eq!(mode.from_slot(-3.0), Value::Choice(0));
        assert_eq!(mode.from_slot(f32::NAN), Value::Choice(0));
        assert_eq!(BUILT_INS[3].kind.from_slot(0.5), Value::Bool(false));
        assert_eq!(BUILT_INS[3].kind.from_slot(0.6), Value::Bool(true));
        assert_eq!(BUILT_INS[2].kind.from_slot(-2.7), Value::Int(-2), "truncated, as the slider reads it");
        for (def, slot) in BUILT_INS.iter().zip([0.003f32, 400.0, 7.0, 1.0, 2.0]) {
            assert_eq!(def.kind.to_slot(&def.kind.from_slot(slot)), slot, "{}", def.name);
        }
    }

    /// The published range: a number's bounds, a choice's indices, none
    /// for a switch.
    #[test]
    fn bounds_are_a_numbers_or_a_choices() {
        assert_eq!(BUILT_INS[2].kind.bounds(), Some((1.0, 10.0)));
        assert_eq!(BUILT_INS[4].kind.bounds(), Some((0.0, 2.0)));
        assert_eq!(BUILT_INS[3].kind.bounds(), None);
    }

    /// A script's owned entries equal a built-in's static ones.
    #[test]
    fn options_compare_by_their_entries() {
        let owned = Options::Owned(vec!["A".into(), "B".into(), "C".into()]);
        assert_eq!(owned, Options::Static(&["A", "B", "C"]));
        assert_ne!(owned, Options::Static(&["A", "B"]));
        assert_eq!(owned.iter().collect::<Vec<_>>(), ["A", "B", "C"]);
    }
}
