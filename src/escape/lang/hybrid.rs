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
//!
//! **Any formula as a group** (phase 4, step 7): a group may run a
//! formula's step instead of KF's lines ([`Member`], its Rhai source).
//! Such a hybrid is composed from its members' programs ([`compose`]):
//! z shared, each member's other state and parameters under its group's
//! name, the loop a branch on the pass; a group of KF lines becomes a
//! member of its own. The result is one program, which prints as one
//! Rhai formula with its `cycle` (`lang::to_rhai`).

use super::ast::BinOp;
use super::ir::{Cycle, IExpr, IExprKind, IStmt, Place, Predef, Program, Ty, Var, VarId};
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

/// A group: its lines, or a formula's step, run `repeats` passes in a
/// row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub repeats: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<Line>,
    /// A formula whose step the group runs instead of lines (step 7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<Member>,
    /// A second formula the group blends with the first (step 8): both
    /// steps from the same z, the new z `mix(a, b, t)` by the group's
    /// own parameter `g<group>_mix`, a continuous morph between them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend: Option<Member>,
}

/// A formula as a hybrid's group: its Rhai source (a UF or Fractint
/// entry is converted when added) and the name it was added as.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub title: String,
    pub rhai: String,
}

impl Group {
    /// A group of KF lines.
    pub fn of_lines(repeats: u32, lines: Vec<Line>) -> Self {
        Group { repeats, lines, formula: None, blend: None }
    }

    /// A group running a formula's step.
    pub fn of_formula(repeats: u32, title: &str, rhai: &str) -> Self {
        Group { repeats, lines: Vec::new(), formula: Some(Member { title: title.into(), rhai: rhai.into() }), blend: None }
    }
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
        Hybrid { groups: vec![Group::of_lines(1, vec![Line { one: op, two: None, combine: Combine::Add }])], loop_start: 0 }
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
            if group.blend.is_some() && group.formula.is_none() {
                return Err(format!("group {}: a blend needs a formula to blend with", g + 1));
            }
            if group.formula.is_some() {
                if !group.lines.is_empty() {
                    return Err(format!("group {}: a formula or lines, not both", g + 1));
                }
                continue;
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

    /// Whether a group runs a formula's step (step 7): the hybrid is
    /// then composed ([`compose`]) rather than written as UF text.
    pub fn has_formulas(&self) -> bool {
        self.groups.iter().any(|g| g.formula.is_some())
    }

    /// The program's text, in UF's language with the internal names: a
    /// hybrid of KF lines alone.
    pub fn source(&self) -> Result<String, String> {
        self.validate()?;
        if self.has_formulas() {
            return Err("a hybrid with formulas is composed, not written".into());
        }
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
        if self.has_formulas() {
            return self.composed();
        }
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

impl Hybrid {
    /// A hybrid with a formula among its groups: each group's program
    /// (a formula's, or its KF lines' alone), composed.
    fn composed(&self) -> Result<Program, Vec<Diag>> {
        let error = |e: String| vec![Diag::error(Span::default(), e)];
        self.validate().map_err(error)?;
        let mut members = Vec::new();
        for (g, group) in self.groups.iter().enumerate() {
            let member = |m: &Member| {
                super::rhai::program(&m.rhai).map_err(|d| error(format!("group {} ({}): {}", g + 1, m.title, d.first().map(|x| x.message.clone()).unwrap_or_default())))
            };
            let p = match &group.formula {
                Some(m) => member(m)?,
                None => Hybrid { groups: vec![Group::of_lines(1, group.lines.clone())], loop_start: 0 }.program()?,
            };
            let b = group.blend.as_ref().map(member).transpose()?;
            members.push((p, b));
        }
        // KF lines keep their multipliers' names (`a<group>_<line>_<n>`)
        // as a hybrid of lines alone has them; everything else is under
        // its group's name. The first group's `bailout` stays the
        // panel's bailout.
        // A blend's second formula is under `g<group>b_`.
        let rename = |g: usize, second: bool, name: &str| -> String {
            if second {
                return format!("g{}b_{name}", g + 1);
            }
            if self.groups[g].formula.is_none() {
                if let Some(rest) = name.strip_prefix("a1_") {
                    return format!("a{}_{rest}", g + 1);
                }
            }
            if g == 0 && name == "bailout" {
                return name.into();
            }
            format!("g{}_{name}", g + 1)
        };
        let repeats: Vec<u32> = self.groups.iter().map(|g| g.repeats).collect();
        let mut p = compose(&members, &rename, &repeats, self.cycle()).map_err(error)?;
        p.view.title = Some("Hybrid".into());
        if self.branches() {
            p.cycle = Some(self.cycle());
        }
        Ok(p)
    }
}

/// One program from a hybrid's members (phase 4, step 7): z shared, each
/// member's variables, parameters, user functions and globals under its
/// group's name (`rename`), the first member's start and test. Every
/// member's start runs, in order, for its own state; only the first
/// sets z (the others' writes to z go to a variable of their own). The
/// loop runs, each pass, the step of the group whose turn it is
/// (`repeats` passes each, the cycle's prefix once). A member's own
/// perturbation equations, perturb setting and degree are dropped: the
/// hybrid's equations are derived whole (`lang::delta`). A member that
/// reads the pass itself has no place in a schedule.
pub fn compose(members: &[(Program, Option<Program>)], rename: &dyn Fn(usize, bool, &str) -> String, repeats: &[u32], cycle: Cycle) -> Result<Program, String> {
    let (first, _) = members.first().ok_or("a hybrid needs a group")?;
    let mut out = first.clone();
    out.vars.clear();
    out.params.clear();
    out.funcs.clear();
    out.init.clear();
    out.loop_body.clear();
    out.global_body.clear();
    out.globals.clear();
    out.final_body.clear();
    out.perturb_init.clear();
    out.perturb_loop.clear();
    out.perturb_when = None;
    out.degree = None;
    out.bailout = None;
    out.state.clear();
    out.dz_var = None;
    out.dzq_var = None;
    out.dpixel_var = None;
    out.cycle = None;
    out.carried.clear();
    out.name = "Hybrid".into();
    let var = |out: &mut Program, name: String, ty: Ty| {
        out.vars.push(Var { name, ty, span: Span::default(), stored_real: false, global: false, perturb: false });
        out.vars.len() - 1
    };
    let z = || IExpr { kind: IExprKind::Predef(Predef::Z), ty: Ty::Complex };
    let cv = |v: VarId| IExpr { kind: IExprKind::Var(v), ty: Ty::Complex };
    let several = members.len() > 1 || members.iter().any(|(_, b)| b.is_some());
    let mut loops = Vec::new();
    for (g, (a, b)) in members.iter().enumerate() {
        // Each formula of the group: its declarations under its name,
        // its start (z its own unless the hybrid's first), its step.
        let mut steps = Vec::new();
        for (second, m) in std::iter::once((false, a)).chain(b.iter().map(|b| (true, b))) {
            if m.dialect != first.dialect {
                return Err(format!("group {}: another dialect", g + 1));
            }
            if m.reads(Predef::Pass) {
                return Err(format!("group {}: a formula that reads the pass cannot take turns", g + 1));
            }
            let (vo, po, fo) = (out.vars.len(), out.params.len(), out.funcs.len());
            for v in &m.vars {
                out.vars.push(Var { name: rename(g, second, &v.name), ..v.clone() });
            }
            let label = if second { format!("Group {} (blended)", g + 1) } else { format!("Group {}", g + 1) };
            for p in &m.params {
                let mut p = p.clone();
                p.name = rename(g, second, &p.name);
                if several {
                    p.caption = format!("{label}: {}", p.caption);
                }
                out.params.push(p);
            }
            for f in &m.funcs {
                let mut f = f.clone();
                f.name = rename(g, second, &f.name);
                if several {
                    f.caption = format!("{label}: {}", f.caption);
                }
                out.funcs.push(f);
            }
            // The formulas after the hybrid's first keep their start's z
            // to themselves.
            let own_z = (g > 0 || second).then(|| var(&mut out, format!("g{}{}_z", g + 1, if second { "b" } else { "" }), Ty::Complex));
            let map = Remap { vo, po, fo, z: None };
            out.globals.extend(m.globals.iter().map(|v| v + vo));
            out.global_body.extend(map.stmts(&m.global_body));
            out.init.extend(Remap { z: own_z, ..map }.stmts(&m.init));
            steps.push(map.stmts(&m.loop_body));
            if g == 0 && !second {
                out.bailout = m.bailout.as_ref().map(|b| map.expr(b));
            }
        }
        let mut step = steps.remove(0);
        if let Some(other) = steps.pop() {
            // Both from the same z; z = mix(a's, b's, t).
            let (zin, za) = (var(&mut out, format!("g{}_zin", g + 1), Ty::Complex), var(&mut out, format!("g{}_za", g + 1), Ty::Complex));
            out.params.push(super::ir::Param {
                name: format!("g{}_mix", g + 1),
                caption: format!("Group {}: Blend", g + 1),
                kind: super::ir::ParamKind::Float,
                default: (0.5, 0.0),
                min: Some(0.0),
                max: Some(1.0),
                hint: "0 runs the group's first formula, 1 its second, between them a mix of the two steps".into(),
                choices: Vec::new(),
                exponential: false,
                span: Span::default(),
            });
            let t = IExpr { kind: IExprKind::Convert(Box::new(IExpr { kind: IExprKind::Param(out.params.len() - 1), ty: Ty::Float })), ty: Ty::Complex };
            let bin = |op: BinOp, a: IExpr, b: IExpr| IExpr { kind: IExprKind::Binary(op, Box::new(a), Box::new(b)), ty: Ty::Complex };
            let mut blended = vec![IStmt::Assign(Place::Var(zin), z())];
            blended.extend(step);
            blended.push(IStmt::Assign(Place::Var(za), z()));
            blended.push(IStmt::Assign(Place::Z, cv(zin)));
            blended.extend(other);
            blended.push(IStmt::Assign(Place::Z, bin(BinOp::Add, cv(za), bin(BinOp::Mul, bin(BinOp::Sub, z(), cv(za)), t))));
            step = blended;
        }
        loops.push(step);
    }
    if members.len() == 1 {
        out.loop_body = loops.pop().unwrap_or_default();
        return Ok(out);
    }
    // The pass this one behaves as -- past the prefix, its place in the
    // first round -- and the group whose turn that is.
    let k = out.vars.len();
    out.vars.push(Var { name: "turn".into(), ty: Ty::Int, span: Span::default(), stored_real: false, global: false, perturb: false });
    let int = |v: i64| IExpr { kind: IExprKind::Int(v), ty: Ty::Int };
    let kv = || IExpr { kind: IExprKind::Var(k), ty: Ty::Int };
    let bin = |op: BinOp, a: IExpr, b: IExpr, ty: Ty| IExpr { kind: IExprKind::Binary(op, Box::new(a), Box::new(b)), ty };
    out.loop_body.push(IStmt::Assign(Place::Var(k), IExpr { kind: IExprKind::Predef(Predef::Pass), ty: Ty::Int }));
    let (prefix, period) = (i64::from(cycle.prefix), i64::from(cycle.period.max(1)));
    let wrapped = bin(BinOp::Add, int(prefix), bin(BinOp::Mod, bin(BinOp::Sub, kv(), int(prefix), Ty::Int), int(period), Ty::Int), Ty::Int);
    if prefix > 0 {
        out.loop_body.push(IStmt::If { arms: vec![(bin(BinOp::Ge, kv(), int(prefix), Ty::Bool), vec![IStmt::Assign(Place::Var(k), wrapped)])], otherwise: Vec::new() });
    } else {
        out.loop_body.push(IStmt::Assign(Place::Var(k), wrapped));
    }
    let mut arms = Vec::new();
    let mut end = 0i64;
    let last = loops.pop().unwrap_or_default();
    for (g, body) in loops.into_iter().enumerate() {
        end += i64::from(repeats[g]);
        arms.push((bin(BinOp::Lt, kv(), int(end), Ty::Bool), body));
    }
    out.loop_body.push(IStmt::If { arms, otherwise: last });
    Ok(out)
}

/// A member's code in the composed program: its variables, parameters
/// and user functions at their offsets, and z, where `z` is set, as that
/// variable.
#[derive(Clone, Copy)]
struct Remap {
    vo: VarId,
    po: usize,
    fo: usize,
    z: Option<VarId>,
}

impl Remap {
    fn stmts(&self, stmts: &[IStmt]) -> Vec<IStmt> {
        stmts.iter().map(|s| self.stmt(s)).collect()
    }

    fn stmt(&self, s: &IStmt) -> IStmt {
        match s {
            IStmt::Assign(place, e) => {
                let place = match place {
                    Place::Var(v) => Place::Var(v + self.vo),
                    Place::Z => match self.z {
                        Some(z) => Place::Var(z),
                        None => Place::Z,
                    },
                    other => other.clone(),
                };
                IStmt::Assign(place, self.expr(e))
            }
            IStmt::If { arms, otherwise } => IStmt::If { arms: arms.iter().map(|(c, b)| (self.expr(c), self.stmts(b))).collect(), otherwise: self.stmts(otherwise) },
            IStmt::While { cond, body } => IStmt::While { cond: self.expr(cond), body: self.stmts(body) },
            IStmt::Repeat { body, cond } => IStmt::Repeat { body: self.stmts(body), cond: self.expr(cond) },
        }
    }

    fn expr(&self, e: &IExpr) -> IExpr {
        let b = |a: &IExpr| Box::new(self.expr(a));
        let kind = match &e.kind {
            IExprKind::Var(v) => IExprKind::Var(v + self.vo),
            IExprKind::Param(p) => IExprKind::Param(p + self.po),
            IExprKind::Predef(Predef::Z) if self.z.is_some() => IExprKind::Var(self.z.unwrap_or_default()),
            IExprKind::UserCall(f, a) => IExprKind::UserCall(f + self.fo, b(a)),
            IExprKind::UserDeriv(f, a) => IExprKind::UserDeriv(f + self.fo, b(a)),
            IExprKind::UserJac(f, a, w) => IExprKind::UserJac(f + self.fo, b(a), *w),
            IExprKind::FuncChoice(f) => IExprKind::FuncChoice(f + self.fo),
            IExprKind::UserDiff(f, a, d) => IExprKind::UserDiff(f + self.fo, b(a), b(d)),
            IExprKind::Convert(a) => IExprKind::Convert(b(a)),
            IExprKind::Unary(op, a) => IExprKind::Unary(*op, b(a)),
            IExprKind::PowInt(a, n) => IExprKind::PowInt(b(a), *n),
            IExprKind::Modulus(a) => IExprKind::Modulus(b(a)),
            IExprKind::Call(f, a) => IExprKind::Call(*f, b(a)),
            IExprKind::MakeComplex(x, y) => IExprKind::MakeComplex(b(x), b(y)),
            IExprKind::Binary(op, x, y) => IExprKind::Binary(*op, b(x), b(y)),
            other => other.clone(),
        };
        IExpr { kind, ty: e.ty }
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
    let group = |repeats: u32, o: Operator| Group::of_lines(repeats, vec![line(o)]);
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

/// Formulas as groups (phase 4, step 7).
#[cfg(test)]
mod formula_members {
    use super::super::builtins::C64;
    use super::super::interp::{perturb_holds, run, run_perturbed, Env, ParamValues};
    use super::*;

    fn twin(name: &str) -> (String, String) {
        let src = crate::escape::twins::source(name).unwrap();
        (super::super::rhai::title(src).unwrap(), src.to_string())
    }

    /// Points on both planes, outside and in, with the Julia constant.
    fn envs() -> Vec<Env> {
        let mut out = Vec::new();
        for julia in [false, true] {
            for c in [C64::new(-0.3, 0.6), C64::new(0.25, -0.1), C64::new(-1.6, 0.02), C64::new(-0.75, 0.12), C64::new(0.4, 0.4)] {
                let mut env = Env::at(c, 80);
                env.julia = julia;
                env.julia_c = C64::new(-0.8, 0.156);
                out.push(env);
            }
        }
        out
    }

    /// The Mandelbrot's and the Burning Ship's twins taking turns run as
    /// KF's lines taking the same turns, on both planes.
    #[test]
    fn formulas_take_turns_as_kf_lines_do() {
        let ((mt, ms), (bt, bs)) = (twin("mandelbrot"), twin("burning_ship"));
        let mandelbrot = Operator::default();
        let ship = Operator { abs_x: true, abs_y: true, ..Operator::default() };
        let line = |o: Operator| vec![Line { one: o, two: None, combine: Combine::Add }];
        for (repeats, loop_start) in [([1, 1], 0), ([1, 2], 0), ([2, 1], 1)] {
            let formulas = Hybrid { groups: vec![Group::of_formula(repeats[0], &mt, &ms), Group::of_formula(repeats[1], &bt, &bs)], loop_start };
            let lines = Hybrid { groups: vec![Group::of_lines(repeats[0], line(mandelbrot.clone())), Group::of_lines(repeats[1], line(ship.clone()))], loop_start };
            let (a, b) = (formulas.program().unwrap(), lines.program().unwrap());
            assert_eq!(a.cycle, b.cycle);
            for env in envs() {
                let (ra, rb) = (run(&a, &ParamValues::defaults(&a), None, &env), run(&b, &ParamValues::defaults(&b), None, &env));
                assert_eq!((ra.n, ra.escaped), (rb.n, rb.escaped), "{repeats:?} from {loop_start} at {:?} julia {}", env.pixel, env.julia);
                assert!(ra.z.sub(rb.z).abs() <= 1e-9 * rb.z.abs().max(1.0), "{repeats:?}: {:?} against {:?}", ra.z, rb.z);
            }
        }
        // A group of lines among formulas keeps its multipliers' names.
        let mixed = Hybrid { groups: vec![Group::of_formula(1, &mt, &ms), Group::of_lines(1, line(ship))], loop_start: 0 };
        let p = mixed.program().unwrap();
        let names: Vec<&str> = p.params.iter().map(|x| x.name.as_str()).collect();
        assert!(names.contains(&"bailout") && names.contains(&"a2_1_1") && names.contains(&"g2_bailout"), "{names:?}");
    }

    /// A member keeps its own state across the others' turns: the
    /// Phoenix's previous z is the z it saw on its own last turn.
    #[test]
    fn a_member_keeps_its_state_across_turns() {
        let ((pt, ps), (mt, ms)) = (twin("phoenix"), twin("mandelbrot"));
        let h = Hybrid { groups: vec![Group::of_formula(1, &pt, &ps), Group::of_formula(1, &mt, &ms)], loop_start: 0 };
        let p = h.program().unwrap();
        let params = ParamValues::defaults(&p);
        let k = C64::new(-0.5, 0.0);
        for c in [C64::new(-0.3, 0.6), C64::new(0.25, -0.1), C64::new(-0.6, 0.4)] {
            let got = run(&p, &params, None, &Env::at(c, 60));
            let (mut z, mut prev) = (C64::ZERO, C64::ZERO);
            let mut n = 0;
            while n < 60 && z.re * z.re + z.im * z.im <= 4.0 {
                if n % 2 == 0 {
                    let next = z.mul(z).add(c).add(k.mul(prev));
                    prev = z;
                    z = next;
                } else {
                    z = z.mul(z).add(c);
                }
                n += 1;
            }
            assert_eq!(got.n, n, "at {c:?}");
            assert!(got.z.sub(z).abs() <= 1e-9 * z.abs().max(1.0), "at {c:?}: {:?} against {z:?}", got.z);
        }
    }

    /// The composed program prints as one Rhai formula, its cycle with
    /// it, which runs as the hybrid; and its deep zoom derives, running
    /// perturbed as it runs.
    #[test]
    fn a_composed_hybrid_is_one_rhai_formula_and_deep_zooms() {
        let ((mt, ms), (bt, bs)) = (twin("mandelbrot"), twin("burning_ship"));
        let h = Hybrid { groups: vec![Group::of_formula(1, &mt, &ms), Group::of_formula(2, &bt, &bs)], loop_start: 0 };
        let p = h.program().unwrap();
        let text = super::super::to_rhai::formula(&p, &[]).unwrap();
        assert!(text.contains("cycle: #{ prefix: 0, period: 3 }"), "{text}");
        let back = super::super::rhai::program(&text).unwrap();
        assert_eq!(super::super::to_rhai::parting(&p, &back, 100), None);
        let d = super::super::delta::with_derived_equations(&p).expect("equations");
        assert!(super::super::perturb::perturbation(&d).is_ok());
        let params = ParamValues::defaults(&d);
        let reference = C64::new(-0.7436, 0.1318);
        assert!(perturb_holds(&d, &params, &Env::at(reference, 200)));
        for j in 0..8 {
            let t = j as f64 * 0.785;
            let px = reference.add(C64::new(t.cos(), t.sin()).scale(1e-5 * (1.0 + j as f64)));
            let env = Env::at(px, 200);
            let direct = run(&p, &ParamValues::defaults(&p), None, &env);
            let pert = run_perturbed(&d, &params, &env, reference, true).unwrap();
            assert_eq!((pert.n, pert.escaped), (direct.n, direct.escaped), "at {px:?}");
        }
    }

    /// A blend of two formulas (phase 4, step 8): at 0 the group's first
    /// formula, at 1 its second, between them the mix of the two steps by
    /// hand; deep zoom derives through it.
    #[test]
    fn a_blend_morphs_between_two_formulas() {
        let ((mt, ms), (bt, bs)) = (twin("mandelbrot"), twin("burning_ship"));
        let h = Hybrid { groups: vec![Group { blend: Some(Member { title: bt, rhai: bs.clone() }), ..Group::of_formula(1, &mt, &ms) }], loop_start: 0 };
        assert_eq!(Hybrid::from_json(&h.to_json()).unwrap(), h);
        let p = h.program().unwrap();
        let mix = p.params.iter().position(|x| x.name == "g1_mix").expect("the blend's parameter");
        let with = |t: f64| {
            let mut params = ParamValues::defaults(&p);
            params.values[mix].0 = t;
            params
        };
        for (t, alone) in [(0.0, &ms), (1.0, &bs)] {
            let alone = super::super::rhai::program(alone).unwrap();
            for env in envs() {
                let (ra, rb) = (run(&p, &with(t), None, &env), run(&alone, &ParamValues::defaults(&alone), None, &env));
                assert_eq!((ra.n, ra.escaped), (rb.n, rb.escaped), "t = {t} at {:?} julia {}", env.pixel, env.julia);
            }
        }
        for c in [C64::new(-0.3, 0.6), C64::new(0.25, -0.1), C64::new(-1.6, 0.02)] {
            let got = run(&p, &with(0.3), None, &Env::at(c, 60));
            let mut z = C64::ZERO;
            let mut n = 0;
            while n < 60 && z.re * z.re + z.im * z.im <= 4.0 {
                let a = z.mul(z).add(c);
                let (ax, ay) = (z.re.abs(), z.im.abs());
                let b = C64::new(ax * ax - ay * ay, 2.0 * ax * ay).add(c);
                z = a.add(b.sub(a).scale(0.3));
                n += 1;
            }
            assert_eq!(got.n, n, "at {c:?}");
        }
        let d = super::super::delta::with_derived_equations(&p).expect("equations");
        assert!(super::super::perturb::perturbation(&d).is_ok());
        let reference = C64::new(-0.3, 0.6);
        for j in 0..8 {
            let t = j as f64 * 0.785;
            let px = reference.add(C64::new(t.cos(), t.sin()).scale(1e-5 * (1.0 + j as f64)));
            let env = Env::at(px, 200);
            let direct = run(&p, &with(0.3), None, &env);
            let pert = run_perturbed(&d, &with(0.3), &env, reference, true).unwrap();
            assert_eq!((pert.n, pert.escaped), (direct.n, direct.escaped), "at {px:?}");
        }
        // A blend needs a formula to blend with.
        let mut bad = h.clone();
        bad.groups[0].formula = None;
        assert!(bad.validate().is_err());
    }

    /// What a hybrid of formulas refuses, and its JSON.
    #[test]
    fn what_takes_no_turns() {
        let (mt, ms) = twin("mandelbrot");
        let h = Hybrid { groups: vec![Group::of_formula(1, &mt, &ms), Group::of_lines(1, vec![Line { one: Operator::default(), two: None, combine: Combine::Add }])], loop_start: 1 };
        assert_eq!(Hybrid::from_json(&h.to_json()).unwrap(), h);
        // A formula and lines in one group.
        let mut both = h.clone();
        both.groups[0].lines = both.groups[1].lines.clone();
        assert!(both.validate().is_err());
        // A member that reads the pass.
        let pass = "const PARAMS = #{};\nfn start(pixel, p, ctx) { #{ z: pixel } }\nfn step(s, p, ctx) { if ctx.pass % 2 == 0 { s.z = s.z * s.z + ctx.pixel; } s }\nfn escaped(s, p) { s.z.norm_sqr() > 4.0 }\n";
        let bad = Hybrid { groups: vec![Group::of_formula(1, "P", pass), Group::of_formula(1, &mt, &ms)], loop_start: 0 };
        assert!(bad.program().is_err());
        // A member that does not compile.
        let broken = Hybrid { groups: vec![Group::of_formula(1, "B", "fn start(") ], loop_start: 0 };
        assert!(broken.program().is_err());
    }
}

