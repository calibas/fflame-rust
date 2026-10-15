//! Kalles Fraktaler-style hybrid formulas (plan, phase 3, step 5),
//! after KF 2.15's hybrid designer as its manual describes it
//! (`mathr.co.uk/kf/manual.html`, "Hybrid Formula dialog"; nothing is
//! taken from its source).
//!
//! A hybrid is a list of groups, each run `repeats` times -- one loop
//! pass a repeat -- in order, looping back to the group `loop_start`
//! (the groups before it a prefix, run once). A group's lines run in
//! sequence, each combining two operators of the current z (`+`, `-`,
//! `*`, `/`); an operator is `(±|x| ± |y| i)^p · a`: each part folded
//! and negated or not, a positive integer power, a complex multiplier.
//! The pixel is added once a pass, after the lines.
//!
//! A hybrid compiles to a program of the formula language through
//! generated UF text, checked with the two names only generated text
//! may read: `#pass` (which group runs is a branch on it) and
//! `#juliac`. The program records its cycle ([`Cycle`]), which the
//! perturbed path's references follow. The multipliers are the
//! program's parameters, so changing one recompiles nothing; this
//! value keeps their starting values.

use super::ir::{Cycle, Program};
use super::{Diag, Dialect, EntryKind, Span};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// KF's limits: four groups of two lines.
pub const MAX_GROUPS: usize = 4;
pub const MAX_LINES: usize = 2;
pub const MAX_REPEATS: u32 = 64;
/// The derivation's binomial powers stop here (`lang::delta`).
pub const MAX_POWER: u32 = 12;

/// A hybrid formula.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hybrid {
    pub groups: Vec<Group>,
    /// The group the loop returns to; those before it run once.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub loop_start: usize,
}

/// A group: its lines, run `repeats` passes in a row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub repeats: u32,
    pub lines: Vec<Line>,
}

/// A line: `z = one <combine> two`, or `z = one` alone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub one: Operator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub two: Option<Operator>,
    #[serde(default, skip_serializing_if = "is_add")]
    pub combine: Combine,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Combine {
    #[default]
    Add,
    Sub,
    Mul,
    Div,
}

impl Combine {
    pub const ALL: [Combine; 4] = [Combine::Add, Combine::Sub, Combine::Mul, Combine::Div];

    pub fn symbol(self) -> &'static str {
        match self {
            Combine::Add => "+",
            Combine::Sub => "-",
            Combine::Mul => "*",
            Combine::Div => "/",
        }
    }
}

/// `(±|x| ± |y| i)^power · mul`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Operator {
    #[serde(default, skip_serializing_if = "is_false")]
    pub abs_x: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub abs_y: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub neg_x: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub neg_y: bool,
    pub power: u32,
    /// The multiplier's starting value (the parameter holds the live
    /// one).
    #[serde(default = "unit", skip_serializing_if = "is_unit")]
    pub mul: [f64; 2],
}

impl Default for Operator {
    fn default() -> Self {
        Operator { abs_x: false, abs_y: false, neg_x: false, neg_y: false, power: 2, mul: unit() }
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

fn is_add(c: &Combine) -> bool {
    *c == Combine::Add
}

fn unit() -> [f64; 2] {
    [1.0, 0.0]
}

fn is_unit(m: &[f64; 2]) -> bool {
    *m == unit()
}

impl Hybrid {
    /// One group of one line: `z = op + c`.
    pub fn single(op: Operator) -> Self {
        Hybrid { groups: vec![Group { repeats: 1, lines: vec![Line { one: op, two: None, combine: Combine::Add }] }], loop_start: 0 }
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("not a hybrid: {e}"))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("a hybrid serialises")
    }

    /// What makes it no hybrid: empty, past KF's limits, a power or a
    /// repeat count out of range, a loop start past the groups.
    pub fn validate(&self) -> Result<(), String> {
        if self.groups.is_empty() {
            return Err("a hybrid needs a group".into());
        }
        if self.groups.len() > MAX_GROUPS {
            return Err(format!("a hybrid has at most {MAX_GROUPS} groups"));
        }
        if self.loop_start >= self.groups.len() {
            return Err("the loop starts past the last group".into());
        }
        for (g, group) in self.groups.iter().enumerate() {
            if group.repeats == 0 || group.repeats > MAX_REPEATS {
                return Err(format!("group {}: repeats run from 1 to {MAX_REPEATS}", g + 1));
            }
            if group.lines.is_empty() || group.lines.len() > MAX_LINES {
                return Err(format!("group {}: a group has 1 to {MAX_LINES} lines", g + 1));
            }
            for (l, line) in group.lines.iter().enumerate() {
                for op in std::iter::once(&line.one).chain(line.two.as_ref()) {
                    if op.power == 0 || op.power > MAX_POWER {
                        return Err(format!("group {}, line {}: powers run from 1 to {MAX_POWER}", g + 1, l + 1));
                    }
                    if !op.mul.iter().all(|v| v.is_finite()) {
                        return Err(format!("group {}, line {}: the multiplier is not a number", g + 1, l + 1));
                    }
                }
            }
        }
        Ok(())
    }

    /// The loop's shape: the passes the prefix takes, and one round.
    pub fn cycle(&self) -> Cycle {
        let passes = |gs: &[Group]| gs.iter().map(|g| g.repeats).sum::<u32>();
        Cycle { prefix: passes(&self.groups[..self.loop_start]), period: passes(&self.groups[self.loop_start..]) }
    }

    /// Whether the groups differ from pass to pass: more than one group
    /// in all, or a prefix. One group repeated is the same every pass.
    fn branches(&self) -> bool {
        self.groups.len() > 1
    }

    /// The parameter holding an operator's multiplier, by 1-based place.
    pub fn param_name(group: usize, line: usize, second: bool) -> String {
        format!("a{}_{}_{}", group + 1, line + 1, if second { 2 } else { 1 })
    }

    /// The program's text, in UF's language with the internal names.
    pub fn source(&self) -> Result<String, String> {
        self.validate()?;
        let mut t = String::new();
        t.push_str("Hybrid {\n");
        // The Julia toggle sets `ismand` (1 on the parameter plane):
        // arithmetic, not a branch, so the init stays one value a pixel.
        t.push_str("init:\n  z = #pixel * (1 - ismand)\n  c = #pixel * ismand + #juliac * (1 - ismand)\n");
        t.push_str("loop:\n");
        let lines = |t: &mut String, g: usize, pad: &str| {
            for (l, line) in self.groups[g].lines.iter().enumerate() {
                let one = operator_text(&line.one, &Hybrid::param_name(g, l, false));
                match &line.two {
                    None => {
                        let _ = writeln!(t, "{pad}z = {one}");
                    }
                    Some(two) => {
                        let two = operator_text(two, &Hybrid::param_name(g, l, true));
                        let _ = writeln!(t, "{pad}z = ({one}) {} ({two})", line.combine.symbol());
                    }
                }
            }
        };
        if self.branches() {
            let Cycle { prefix, period } = self.cycle();
            // The pass this one behaves as: past the prefix, its place in
            // the first round.
            t.push_str("  int k = #pass\n");
            if prefix > 0 {
                let _ = writeln!(t, "  if k >= {prefix}\n    k = {prefix} + (k - {prefix}) % {period}\n  endif");
            } else {
                let _ = writeln!(t, "  k = k % {period}");
            }
            let mut end = 0;
            let last = self.groups.len() - 1;
            for g in 0..self.groups.len() {
                end += self.groups[g].repeats;
                if g == 0 {
                    let _ = writeln!(t, "  if k < {end}");
                } else if g < last {
                    let _ = writeln!(t, "  elseif k < {end}");
                } else {
                    t.push_str("  else\n");
                }
                lines(&mut t, g, "    ");
            }
            t.push_str("  endif\n");
        } else {
            lines(&mut t, 0, "  ");
        }
        t.push_str("  z = z + c\n");
        t.push_str("bailout:\n  |z| < @bailout\n");
        t.push_str("default:\n  title = \"Hybrid\"\n");
        t.push_str("  float param bailout\n    caption = \"Bailout value\"\n    default = 4\n    min = 1\n  endparam\n");
        for (g, group) in self.groups.iter().enumerate() {
            for (l, line) in group.lines.iter().enumerate() {
                for (second, op) in std::iter::once((false, &line.one)).chain(line.two.as_ref().map(|o| (true, o))) {
                    let _ = writeln!(
                        t,
                        "  complex param {}\n    caption = \"Group {}, line {}, operator {}\"\n    default = ({}, {})\n  endparam",
                        Hybrid::param_name(g, l, second),
                        g + 1,
                        l + 1,
                        if second { 2 } else { 1 },
                        number(op.mul[0]),
                        number(op.mul[1]),
                    );
                }
            }
        }
        t.push_str("}\n");
        Ok(t)
    }

    /// The program: the source checked with the internal names, its
    /// cycle recorded when its groups branch on the pass.
    pub fn program(&self) -> Result<Program, Vec<Diag>> {
        let text = self.source().map_err(|e| vec![Diag::error(Span::default(), e)])?;
        let (file, mut diags) = super::parser::parse_file(&text);
        let Some(entry) = file.entries.first() else {
            return Err(diags);
        };
        let (program, d) = super::check::check_internal(entry, EntryKind::Formula, Dialect::UltraFractal);
        diags.extend(d);
        match program {
            Some(mut p) if !super::has_errors(&diags) => {
                if p.reads(super::ir::Predef::Pass) {
                    p.cycle = Some(self.cycle());
                }
                Ok(p)
            }
            _ => Err(diags),
        }
    }
}

/// A number as UF reads it: never in exponent form.
fn number(v: f64) -> String {
    let s = format!("{v}");
    if s.contains('e') || s.contains("inf") || s.contains("NaN") {
        format!("{v:.17}")
    } else {
        s
    }
}

/// `(±|x| ± |y| i)^p * @a`, of the current z.
fn operator_text(op: &Operator, param: &str) -> String {
    let part = |f: &str, abs: bool, neg: bool| {
        let v = if abs { format!("abs({f}(z))") } else { format!("{f}(z)") };
        if neg {
            format!("-{v}")
        } else {
            v
        }
    };
    let base = if op.abs_x || op.abs_y || op.neg_x || op.neg_y {
        format!("({}, {})", part("real", op.abs_x, op.neg_x), part("imag", op.abs_y, op.neg_y))
    } else {
        "z".to_string()
    };
    let powered = if op.power == 1 { base } else { format!("{base}^{}", op.power) };
    format!("{powered} * @{param}")
}

/// Starting points: the built-in formulas KF's "To Hybrid" maps, and
/// some of the classic mixtures.
pub fn presets() -> Vec<(&'static str, Hybrid)> {
    let op = |abs_x: bool, abs_y: bool, neg_x: bool, neg_y: bool, power: u32| Operator { abs_x, abs_y, neg_x, neg_y, power, mul: unit() };
    let line = |o: Operator| Line { one: o, two: None, combine: Combine::Add };
    let group = |repeats: u32, o: Operator| Group { repeats, lines: vec![line(o)] };
    let mandelbrot = op(false, false, false, false, 2);
    let ship = op(true, true, false, false, 2);
    vec![
        ("Mandelbrot", Hybrid::single(mandelbrot.clone())),
        ("Cubic Mandelbrot", Hybrid::single(op(false, false, false, false, 3))),
        ("Burning Ship", Hybrid::single(ship.clone())),
        ("Tricorn", Hybrid::single(op(false, false, false, true, 2))),
        ("Perpendicular Mandelbrot", Hybrid::single(op(true, false, false, true, 2))),
        ("Perpendicular Burning Ship", Hybrid::single(op(false, true, false, true, 2))),
        (
            "Mandelbrot / Burning Ship",
            Hybrid { groups: vec![group(1, mandelbrot.clone()), group(1, ship.clone())], loop_start: 0 },
        ),
        (
            "Burning Ship, then Mandelbrot twice",
            Hybrid { groups: vec![group(1, ship), group(2, mandelbrot.clone())], loop_start: 0 },
        ),
        (
            "Cubic start, then Mandelbrot",
            Hybrid { groups: vec![group(1, op(false, false, false, false, 3)), group(1, mandelbrot)], loop_start: 1 },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::builtins::C64;
    use super::super::interp::{run, Env, ParamValues};
    use super::*;

    #[test]
    fn a_hybrid_round_trips_through_json() {
        for (name, h) in presets() {
            let back = Hybrid::from_json(&h.to_json()).unwrap();
            assert_eq!(back, h, "{name}");
            h.validate().unwrap();
        }
        // Defaults stay out of the text.
        assert_eq!(Hybrid::single(Operator::default()).to_json(), r#"{"groups":[{"repeats":1,"lines":[{"one":{"power":2}}]}]}"#);
    }

    #[test]
    fn what_is_no_hybrid() {
        let ok = Hybrid::single(Operator::default());
        let with = |f: &dyn Fn(&mut Hybrid)| {
            let mut h = ok.clone();
            f(&mut h);
            h.validate()
        };
        assert!(with(&|h| h.groups.clear()).is_err());
        assert!(with(&|h| h.loop_start = 1).is_err());
        assert!(with(&|h| h.groups[0].repeats = 0).is_err());
        assert!(with(&|h| h.groups[0].lines[0].one.power = 0).is_err());
        assert!(with(&|h| h.groups[0].lines[0].one.power = MAX_POWER + 1).is_err());
        assert!(with(&|h| h.groups[0].lines.clear()).is_err());
        assert!(with(&|h| h.groups = vec![h.groups[0].clone(); MAX_GROUPS + 1]).is_err());
        assert!(with(&|h| h.groups[0].lines[0].one.mul = [f64::NAN, 0.0]).is_err());
    }

    /// Each preset iterates as its definition by hand: the groups in
    /// order, the prefix once, the operators' folds and powers.
    #[test]
    fn hybrids_iterate_as_written() {
        fn op_value(o: &Operator, z: C64, a: C64) -> C64 {
            let mut x = if o.abs_x { z.re.abs() } else { z.re };
            let mut y = if o.abs_y { z.im.abs() } else { z.im };
            if o.neg_x {
                x = -x;
            }
            if o.neg_y {
                y = -y;
            }
            let b = C64::new(x, y);
            let mut p = b;
            for _ in 1..o.power {
                p = p.mul(b);
            }
            p.mul(a)
        }
        for (name, h) in presets() {
            let p = h.program().unwrap_or_else(|d| panic!("{name}: {d:?}"));
            assert_eq!(p.cycle.is_some(), h.groups.len() > 1, "{name}");
            let params = ParamValues::defaults(&p);
            for c in [C64::new(-0.3, 0.6), C64::new(0.25, -0.1), C64::new(-1.6, 0.02)] {
                let n_max = 60;
                let got = run(&p, &params, None, &Env::at(c, n_max));
                // By hand.
                let cyc = h.cycle();
                let mut z = C64::ZERO;
                let mut n = 0;
                let mut escaped = false;
                for pass in 0..n_max {
                    let k = cyc.phase(pass);
                    let mut start = 0;
                    let g = h.groups.iter().position(|g| {
                        start += g.repeats;
                        k < start
                    });
                    for line in &h.groups[g.unwrap()].lines {
                        let one = op_value(&line.one, z, C64::new(line.one.mul[0], line.one.mul[1]));
                        z = match &line.two {
                            None => one,
                            Some(t) => {
                                let two = op_value(t, z, C64::new(t.mul[0], t.mul[1]));
                                match line.combine {
                                    Combine::Add => one.add(two),
                                    Combine::Sub => one.sub(two),
                                    Combine::Mul => one.mul(two),
                                    Combine::Div => one.div(two),
                                }
                            }
                        };
                    }
                    z = z.add(c);
                    n = pass + 1;
                    if z.norm_sqr() >= 4.0 {
                        escaped = true;
                        break;
                    }
                }
                assert_eq!((got.n, got.escaped), (n, escaped), "{name} at {c:?}");
                assert!(got.z.sub(z).abs() <= 1e-9 * (1.0 + z.abs()), "{name} at {c:?}: {:?} against {z:?}", got.z);
            }
        }
    }
}
