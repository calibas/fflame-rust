//! A checked program to WGSL.
//!
//! A FORMULA becomes a program the direct template drives:
//! `struct FormulaState` (the iterate and every variable),
//! `formula_init(pixel)`, `formula_loop(s, pixel)`,
//! `formula_bailout(s, pixel)` (true: iterate again) and
//! `formula_same(a, b)` (interior detection's repeat). A COLOURING
//! becomes the built-in colouring interface, so every template that
//! takes a colouring takes it: `ufc_init()` for `accum_init`,
//! `coloring_accum` packing its persistent variables into the `vec4`
//! state, and `coloring_map`.
//!
//! The helpers each program uses are emitted into its own source under
//! its own prefix (`uff_` formula, `ufc_` colouring), so a custom
//! formula and a custom colouring compile side by side, and a colouring
//! carries everything it needs into the perturbed and recolour
//! templates, which have no formula helpers. The cross-platform rules
//! (CLAUDE.md, Metal) hold by construction: `atan2` only through the
//! zero-pair-exact helper, NaN and infinity tested by bits, no
//! self-division, no subnormal literals.
//!
//! A program's `global:` values are computed once per image on the CPU
//! ([`super::interp::globals`]) and read here from the uniform: a
//! formula's from `fdata4`, a colouring's from `cparam` after its
//! parameters. They are not part of any per-pixel state.
//!
//! What the templates provide: `params.max_iter`, `fparam` / `cparam`,
//! `fdata4`, `OrbitSummary`, and the per-pixel environment as private globals
//! `esc_env_px`, `esc_env_size`, `esc_env_pixel`, `esc_env_center`,
//! `esc_env_magn` (see [`ENV_DECLS`]).

use std::collections::BTreeSet;
use std::fmt::Write;

use super::ast::{BinOp, UnOp};
use super::builtins::{Builtin, USER_FUNCTIONS};
use super::check::LOOP_CAP;
use super::ir::*;
use super::Dialect;

/// The per-pixel environment a custom program reads, declared by every
/// template that hosts one and set at the top of its entry point.
pub const ENV_DECLS: &str = "\
// The pixel's environment, for custom formulas and colourings
// (escape::lang): its position and the frame's size in render pixels,
// its point in the plane, and the view.
var<private> esc_env_px: vec2<i32>;
var<private> esc_env_size: vec2<i32>;
var<private> esc_env_pixel: vec2<f32>;
var<private> esc_env_center: vec2<f32>;
var<private> esc_env_magn: f32;
// Set by a custom colouring's coloring_map: this pixel is `#solid`, and
// the template leaves it to the background (lang::wgsl).
var<private> esc_env_solid: bool;
";

/// The private flag a colouring sets for `#solid`. The templates read
/// it straight after their own `coloring_map` call (offset relief calls
/// it again for the points beside the pixel) and leave the pixel to the
/// background; `coloring_map` returns 0 for it, which keeps the relief
/// flat there, as an interior pixel's.
pub const SOLID_FLAG: &str = "esc_env_solid";

/// A custom colouring's three pieces, as `ColoringDef` holds them.
#[derive(Clone, Debug)]
pub struct ColoringWgsl {
    /// `coloring_map`, `ufc_init`, `coloring_accum` and the helpers.
    pub wgsl: String,
    /// Empty: `coloring_accum` is in `wgsl`, which the templates splice
    /// before the accumulator marker.
    pub wgsl_accum: String,
    /// `ufc_init()`.
    pub accum_init: String,
    /// The colouring keeps state or has a loop: it needs the
    /// accumulator compiled in.
    pub needs_accum: bool,
    /// The `vec4`s of state past the first four floats, kept in the
    /// private `ufc_ext` that the templates save and restore beside the
    /// accumulator (`AccumWidth::Extended`).
    pub ext_vec4s: usize,
}

/// The WGSL of a formula program.
pub fn formula(p: &Program) -> String {
    let mut g = Gen::new(p, Ctx::Formula, "uff_");
    let mut body = String::new();
    // The state: z and every variable but the globals.
    body.push_str("struct FormulaState {\n    z: vec2<f32>,\n");
    for (i, v) in p.vars.iter().enumerate() {
        if !v.global {
            let _ = writeln!(body, "    v{i}: {},", wgsl_ty(v.ty));
        }
    }
    // The pass index (`npass`: `pass` is a WGSL keyword), for a program
    // that reads it (a hybrid's groups): zero from `formula_init`, one
    // more after each `formula_loop`.
    let pass = p.reads(Predef::Pass);
    if pass {
        body.push_str("    npass: i32,\n");
    }
    body.push_str("}\n\n");
    body.push_str("fn formula_init(pixel: vec2<f32>) -> FormulaState {\n    var s: FormulaState;\n");
    g.stmts(&p.init, 1, &mut body);
    if pass {
        body.push_str("    s.npass = 0;\n");
    }
    body.push_str("    return s;\n}\n\n");
    body.push_str("fn formula_loop(s_in: FormulaState, pixel: vec2<f32>) -> FormulaState {\n    var s = s_in;\n");
    g.stmts(&p.loop_body, 1, &mut body);
    if pass {
        body.push_str("    s.npass = s.npass + 1;\n");
    }
    body.push_str("    return s;\n}\n\n");
    let test = p.bailout.as_ref().map_or_else(|| "true".to_string(), |b| g.expr(b));
    let _ = write!(
        body,
        "fn formula_bailout(s: FormulaState, pixel: vec2<f32>) -> bool {{\n    return {test};\n}}\n"
    );
    // Two states equal to the bit, for interior detection: the state is
    // everything the loop and the test read that changes, so a repeat
    // proves the orbit periodic. Bits, not `==`: +0 and -0 compare
    // equal and can continue differently, and Metal's fast-math folds
    // float comparisons (CLAUDE.md).
    let mut same = vec!["all(bitcast<vec2<u32>>(a.z) == bitcast<vec2<u32>>(b.z))".to_string()];
    for (i, v) in p.vars.iter().enumerate().filter(|(_, v)| !v.global) {
        same.push(match v.ty {
            Ty::Complex => format!("all(bitcast<vec2<u32>>(a.v{i}) == bitcast<vec2<u32>>(b.v{i}))"),
            Ty::Float => format!("bitcast<u32>(a.v{i}) == bitcast<u32>(b.v{i})"),
            Ty::Int | Ty::Bool => format!("a.v{i} == b.v{i}"),
        });
    }
    // ...at the same place in the cycle: two passes that behave alike.
    if pass {
        let phase = |x: &str| match p.cycle {
            Some(c) => format!(
                "select({x}.npass, {} + ({x}.npass - {}) % {}, {x}.npass >= {})",
                c.prefix,
                c.prefix,
                c.period.max(1),
                c.prefix
            ),
            None => format!("{x}.npass"),
        };
        same.push(format!("{} == {}", phase("a"), phase("b")));
    }
    let _ = write!(
        body,
        "\nfn formula_same(a: FormulaState, b: FormulaState) -> bool {{\n    return {};\n}}\n",
        same.join("\n        && ")
    );
    // UF's perturbation equations, for the perturbed path's scaled
    // rung: the start and the step of the pixel's deviation from the
    // reference, held in units of S (`w = dz / S`, the built-in tiers'
    // form), with the equations' own variables as locals; and the
    // formula's test on the pixel's full z when it reads nothing else.
    if !p.perturb_loop.is_empty() {
        g.ctx = Ctx::Perturb;
        g.small_vars = perturb_var_classes(p);
        let locals: String = p
            .vars
            .iter()
            .enumerate()
            .filter(|(_, v)| v.perturb)
            .map(|(i, v)| format!("    var v{i}: {} = {};\n", wgsl_ty(v.ty), zero_of(v.ty)))
            .collect();
        // The start is pass 0's; the step reads the reference's pass
        // (`pass_index`, a hybrid's phase: step 5b).
        let start_pass = if p.reads(Predef::Pass) { "    let pass_index: i32 = 0;\n" } else { "" };
        let _ = write!(
            body,
            "\nfn formula_perturb_init_scaled(z_ref: vec2<f32>, pixel: vec2<f32>, d0: vec2<f32>, s: f32) -> vec2<f32> {{\n    var w = vec2<f32>(0.0, 0.0);\n{start_pass}{locals}"
        );
        g.sstmts(&p.perturb_init, 1, &mut body);
        body.push_str("    return w;\n}\n");
        let _ = write!(
            body,
            "\nfn formula_perturb_loop_scaled(z_ref: vec2<f32>, w_in: vec2<f32>, pixel: vec2<f32>, d0: vec2<f32>, s: f32, pass_index: i32) -> vec2<f32> {{\n    var w = w_in;\n{locals}"
        );
        g.sstmts(&p.perturb_loop, 1, &mut body);
        body.push_str("    return w;\n}\n");
        if let Some(b) = super::perturb::perturb_test(p) {
            g.ctx = Ctx::PerturbBailout;
            let test = g.expr(&b);
            let _ = write!(body, "\nfn formula_perturb_bailout(z: vec2<f32>, pixel: vec2<f32>) -> bool {{\n    return {test};\n}}\n");
        }
    }
    g.finish(&format!("// custom formula: {}\n", sanitize(&p.name)), body)
}

/// A formula's perturbation equations for the perturbed path's
/// floatexp rung: `formula_perturb_init_fe` and `formula_perturb_loop_fe`,
/// over `CFe2` (the deep template's double-float mantissa with a binary
/// exponent), the reference iterate given as its stored entry (`zm`,
/// `zlo`, `ze`). Every value of the equations is a `CFe2` -- the
/// reference's scale and the deviation's alike, so a coefficient like
/// `2 #z` keeps the reference's double-float precision, as the built-in
/// deep steps do; what has no floatexp form runs on the true values in
/// f32. Spliced only into the deep template, which defines `CFe2` and
/// its helpers; its own helpers are `ufe_`-prefixed. `None` without
/// equations.
pub fn formula_perturb_fe(p: &Program) -> Option<String> {
    if p.perturb_loop.is_empty() {
        return None;
    }
    let mut g = Gen::new(p, Ctx::PerturbFe, "ufe_");
    g.small_vars = p.vars.iter().map(|v| v.perturb && matches!(v.ty, Ty::Float | Ty::Complex)).collect();
    let locals: String = p
        .vars
        .iter()
        .enumerate()
        .filter(|(_, v)| v.perturb)
        .map(|(i, v)| match v.ty {
            Ty::Float | Ty::Complex => format!("    var v{i}: CFe2 = cfe2_zero();\n"),
            t => format!("    var v{i}: {} = {};\n", wgsl_ty(t), zero_of(t)),
        })
        .collect();
    let start_pass = if p.reads(Predef::Pass) { "    let pass_index: i32 = 0;\n" } else { "" };
    let mut body = String::new();
    let _ = write!(
        body,
        "fn formula_perturb_init_fe(zm: vec2<f32>, zlo: vec2<f32>, ze: i32, pixel: vec2<f32>, d0: CFe2) -> CFe2 {{\n    var w = cfe2_zero();\n{start_pass}{locals}"
    );
    g.festmts(&p.perturb_init, 1, &mut body);
    body.push_str("    return w;\n}\n\n");
    let _ = write!(
        body,
        "fn formula_perturb_loop_fe(zm: vec2<f32>, zlo: vec2<f32>, ze: i32, w_in: CFe2, pixel: vec2<f32>, d0: CFe2, pass_index: i32) -> CFe2 {{\n    var w = w_in;\n{locals}"
    );
    g.festmts(&p.perturb_loop, 1, &mut body);
    body.push_str("    return w;\n}\n");
    Some(g.finish(&format!("// custom formula's floatexp equations: {}\n", sanitize(&p.name)), body))
}

/// What [`formula_perturb_fe`] calls beyond the deep template's own
/// `CFe2` helpers: spliced beside it.
pub const PERTURB_FE_HELPERS: &str = "
// The custom formulas' floatexp operations (escape::lang), beside the
// deep template's own CFe2 helpers.
fn cfe2_neg(a: CFe2) -> CFe2 {
    return CFe2(-a.hi, -a.lo, a.e);
}

fn cfe2_sub(a: CFe2, b: CFe2) -> CFe2 {
    return cfe2_add(a, cfe2_neg(b));
}

fn cfe2_conj(a: CFe2) -> CFe2 {
    return CFe2(vec2<f32>(a.hi.x, -a.hi.y), vec2<f32>(a.lo.x, -a.lo.y), a.e);
}

fn cfe2_flip(a: CFe2) -> CFe2 {
    return CFe2(a.hi.yx, a.lo.yx, a.e);
}

fn cfe2_re(a: CFe2) -> CFe2 {
    return cfe2_norm(CFe2(vec2<f32>(a.hi.x, 0.0), vec2<f32>(a.lo.x, 0.0), a.e));
}

fn cfe2_im(a: CFe2) -> CFe2 {
    return cfe2_norm(CFe2(vec2<f32>(a.hi.y, 0.0), vec2<f32>(a.lo.y, 0.0), a.e));
}

// a * i.
fn cfe2_times_i(a: CFe2) -> CFe2 {
    return CFe2(vec2<f32>(-a.hi.y, a.hi.x), vec2<f32>(-a.lo.y, a.lo.x), a.e);
}

// A reference entry: its double-float mantissa and exponent.
fn cfe2_ref(zm: vec2<f32>, zlo: vec2<f32>, ze: i32) -> CFe2 {
    return cfe2_norm(CFe2(zm, zlo, ze));
}

// Each part's absolute value (UF's abs; a real's imaginary part is 0):
// a normalised part's sign is its high part's.
fn cfe2_abs(a: CFe2) -> CFe2 {
    let s = select(vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), a.hi < vec2<f32>(0.0, 0.0));
    return CFe2(a.hi * s, a.lo * s, a.e);
}

// |c + d| - |c| of real CFe2s by sign case (the built-in deep Ship's
// `sfe_diffabs`): a normalised value's sign is its high part's.
fn cfe2_diffabs(c: CFe2, d: CFe2) -> CFe2 {
    let sum = cfe2_add(c, d);
    if (c.hi.x >= 0.0) {
        if (sum.hi.x >= 0.0) {
            return d;
        }
        return cfe2_neg(cfe2_add(cfe2_add(c, c), d));
    }
    if (sum.hi.x > 0.0) {
        return cfe2_add(cfe2_add(c, c), d);
    }
    return cfe2_neg(d);
}

// |a|^2, a real CFe2.
fn cfe2_abs2(a: CFe2) -> CFe2 {
    if (a.e == CFE_ZERO_E) {
        return a;
    }
    let ax = vec2<f32>(a.hi.x, a.lo.x);
    let ay = vec2<f32>(a.hi.y, a.lo.y);
    let r = df_add(df_mul(ax, ax), df_mul(ay, ay));
    return cfe2_norm(CFe2(vec2<f32>(r.x, 0.0), vec2<f32>(r.y, 0.0), a.e * 2));
}

// 1/b: the f32 reciprocal of the mantissa, then one Newton step in
// double float, r(2 - b r), for the double float's precision. A zero
// divisor gives a large value, as the f32 helpers' do.
fn cfe2_recip(b: CFe2) -> CFe2 {
    if (b.e == CFE_ZERO_E) {
        return cfe2_from_f32(vec2<f32>(1.0e30, 0.0));
    }
    let h = b.hi;
    let r0 = cfe2_norm(CFe2(vec2<f32>(h.x, -h.y) / dot(h, h), vec2<f32>(0.0, 0.0), -b.e));
    let two = cfe2_from_f32(vec2<f32>(2.0, 0.0));
    return cfe2_mul(r0, cfe2_sub(two, cfe2_mul(b, r0)));
}

fn cfe2_div(a: CFe2, b: CFe2) -> CFe2 {
    return cfe2_mul(a, cfe2_recip(b));
}

// a^n, n >= 1, by repeated squaring.
fn cfe2_powi(a: CFe2, n: i32) -> CFe2 {
    var acc = cfe2_from_f32(vec2<f32>(1.0, 0.0));
    var base = a;
    var k = n;
    for (var t = 0; t < 32; t = t + 1) {
        if (k <= 0) {
            break;
        }
        if ((k & 1) == 1) {
            acc = cfe2_mul(acc, base);
        }
        base = cfe2_sqr(base);
        k = k >> 1u;
    }
    return acc;
}
";

/// The WGSL of a formula carrying its derivative with respect to the
/// pixel ([`super::derive`]), compiled in place of [`formula`]'s where a
/// colouring reads the derivative: the same functions, a state with the
/// derivative variables, and `formula_dz(s)`. `None` when the iterate
/// has no derivative.
pub fn formula_with_derivative(p: &Program) -> Option<String> {
    let d = super::derive::with_derivative(p)?;
    let dz = d.dz_var?;
    let mut out = formula(&d);
    let _ = write!(out, "\nfn formula_dz(s: FormulaState) -> vec2<f32> {{\n    return s.v{dz};\n}}\n");
    Some(out)
}

/// The WGSL of a colouring.
pub fn coloring(p: &Program) -> ColoringWgsl {
    let needs_accum = !p.state.is_empty() || !p.init.is_empty() || !p.loop_body.is_empty();
    let mut body = String::new();

    // Where each state variable sits: the first four floats in the
    // accumulator's vec4, the rest in `ufc_ext`.
    let mut offsets = Vec::new();
    let mut at = 0usize;
    for &v in &p.state {
        offsets.push((v, at));
        at += if p.vars[v].stored_real { 1 } else { p.vars[v].ty.floats() };
    }
    let ext_vec4s = at.saturating_sub(4).div_ceil(4);
    let lanes = ["x", "y", "z", "w"];
    let slot = |o: usize| {
        if o < 4 {
            format!("state.{}", lanes[o])
        } else {
            format!("uf_ext[{}].{}", (o - 4) / 4, lanes[(o - 4) % 4])
        }
    };
    let unpack = |out: &mut String| {
        for &(v, o) in &offsets {
            let var = &p.vars[v];
            let read = match (var.ty, var.stored_real) {
                (Ty::Complex, true) => format!("vec2<f32>({}, 0.0)", slot(o)),
                (Ty::Complex, false) => format!("vec2<f32>({}, {})", slot(o), slot(o + 1)),
                (Ty::Float, _) => slot(o),
                (Ty::Int, _) => format!("i32(round({}))", slot(o)),
                (Ty::Bool, _) => format!("({} != 0.0)", slot(o)),
            };
            let _ = writeln!(out, "    var v{v}: {} = {read};", wgsl_ty(var.ty));
        }
    };
    let pack = |out: &mut String| {
        let mut parts = vec!["0.0".to_string(); 4 + 4 * ext_vec4s];
        for &(v, o) in &offsets {
            let var = &p.vars[v];
            match (var.ty, var.stored_real) {
                (Ty::Complex, true) => parts[o] = format!("v{v}.x"),
                (Ty::Complex, false) => {
                    parts[o] = format!("v{v}.x");
                    parts[o + 1] = format!("v{v}.y");
                }
                (Ty::Float, _) => parts[o] = format!("v{v}"),
                (Ty::Int, _) => parts[o] = format!("f32(v{v})"),
                (Ty::Bool, _) => parts[o] = format!("select(0.0, 1.0, v{v})"),
            }
        }
        for k in 0..ext_vec4s {
            let _ = writeln!(out, "    uf_ext[{k}] = vec4<f32>({});", parts[4 + 4 * k..8 + 4 * k].join(", "));
        }
        let _ = writeln!(out, "    return vec4<f32>({});", parts[..4].join(", "));
    };
    // The other variables are locals of each function.
    let locals = |out: &mut String| {
        for (i, v) in p.vars.iter().enumerate() {
            if !p.state.contains(&i) && !v.global {
                let _ = writeln!(out, "    var v{i}: {} = {};", wgsl_ty(v.ty), zero_of(v.ty));
            }
        }
    };

    if ext_vec4s > 0 {
        let _ = write!(
            body,
            "// The state past the accumulator's four floats: per pixel, saved\n\
             // and restored beside the accumulator by the templates.\n\
             var<private> uf_ext: array<vec4<f32>, {ext_vec4s}>;\n\n"
        );
    }
    let mut g = Gen::new(p, Ctx::ColoringInit, "ufc_");
    body.push_str("fn ufc_init() -> vec4<f32> {\n");
    for &(v, _) in &offsets {
        let _ = writeln!(body, "    var v{v}: {} = {};", wgsl_ty(p.vars[v].ty), zero_of(p.vars[v].ty));
    }
    locals(&mut body);
    g.stmts(&p.init, 1, &mut body);
    pack(&mut body);
    body.push_str("}\n\n");

    g.ctx = Ctx::ColoringLoop;
    body.push_str(
        "fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {\n",
    );
    unpack(&mut body);
    locals(&mut body);
    g.stmts(&p.loop_body, 1, &mut body);
    pack(&mut body);
    body.push_str("}\n\n");

    g.ctx = Ctx::ColoringFinal;
    body.push_str("fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {\n");
    unpack(&mut body);
    locals(&mut body);
    body.push_str("    var index: f32 = 0.0;\n    var solid: bool = false;\n");
    g.stmts(&p.final_body, 1, &mut body);
    // An INSIDE colouring leaves the outside to the background, and an
    // OUTSIDE one never sees the inside (no ColorsInterior).
    if p.region == Region::Inside {
        body.push_str("    if (sum.escaped) {\n        solid = true;\n    }\n");
    }
    if writes_solid(p) {
        let _ = write!(body, "    {SOLID_FLAG} = solid;\n    if (solid) {{\n        return 0.0;\n    }}\n");
    }
    body.push_str("    return index;\n}\n");

    let wgsl = g.finish(&format!("// custom colouring: {}\n", sanitize(&p.name)), body);
    ColoringWgsl { wgsl, wgsl_accum: String::new(), accum_init: "ufc_init()".to_string(), needs_accum, ext_vec4s }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Formula,
    ColoringInit,
    ColoringLoop,
    ColoringFinal,
    /// The perturbation equations: `#z` is the reference's `z_ref`,
    /// variables are the functions' locals.
    Perturb,
    /// The formula's test on the perturbed path: `#z` is the pixel's
    /// full `z`.
    PerturbBailout,
    /// The perturbation equations on the floatexp rung: values are
    /// `CFe2`; plain f32 code reads them through `cfe2_to_f32`.
    PerturbFe,
}

struct Gen<'p> {
    p: &'p Program,
    ctx: Ctx,
    prefix: &'static str,
    used: BTreeSet<Helper>,
    /// Each parameter's first slot.
    slots: Vec<usize>,
    /// The user functions' slots, after the parameters.
    func_slots: Vec<usize>,
    /// Each packed global's first float in its block.
    global_slots: Vec<Option<usize>>,
    loops: usize,
    /// Inside a constant subexpression: its literals go through
    /// `uf_k`, so WGSL does not fold it ([`constant`]).
    opaque: bool,
    /// The perturbation equations' variables that hold a deviation
    /// (Small, in units of S), by id ([`perturb_var_classes`]).
    small_vars: Vec<bool>,
    /// Emitting a deviation at its true size (`S` times its held value):
    /// inside an operation the scaled form has no rule for.
    true_small: bool,
}

impl<'p> Gen<'p> {
    fn new(p: &'p Program, ctx: Ctx, prefix: &'static str) -> Self {
        let mut slots = Vec::new();
        let mut at = 0;
        for param in &p.params {
            slots.push(at);
            at += param.ty().floats();
        }
        let func_slots = (0..p.funcs.len()).map(|i| at + i).collect();
        let mut global_slots = vec![None; p.vars.len()];
        let mut g = 0;
        for &v in &p.globals {
            global_slots[v] = Some(g);
            g += p.vars[v].ty.floats();
        }
        Gen { p, ctx, prefix, used: BTreeSet::new(), slots, func_slots, global_slots, loops: 0, opaque: false, small_vars: Vec::new(), true_small: false }
    }

    /// Helpers, then the body; `uf_` becomes the program's prefix.
    fn finish(&self, header: &str, body: String) -> String {
        let mut all: BTreeSet<Helper> = BTreeSet::new();
        let mut stack: Vec<Helper> = self.used.iter().copied().collect();
        while let Some(h) = stack.pop() {
            if all.insert(h) {
                stack.extend(h.deps().iter().copied());
            }
        }
        let mut out = String::from(header);
        for h in &all {
            out.push_str(&h.source());
            out.push('\n');
        }
        out.push_str(&body);
        out.replace("uf_", self.prefix)
    }

    /// A formula's function, whose parameters are `fparam`'s and whose
    /// globals are in `fdata`.
    fn formula_side(&self) -> bool {
        matches!(self.ctx, Ctx::Formula | Ctx::Perturb | Ctx::PerturbBailout | Ctx::PerturbFe)
    }

    fn param_fn(&self) -> &'static str {
        if self.formula_side() {
            "fparam"
        } else {
            "cparam"
        }
    }

    fn var(&self, v: VarId) -> String {
        if let Some(k) = self.global_slots[v] {
            return self.global(v, k);
        }
        if self.true_small && self.small_vars.get(v).copied().unwrap_or(false) {
            return format!("(s * v{v})");
        }
        // On the floatexp rung a value of the equations is a CFe2; plain
        // code reads its f32 value.
        if self.ctx == Ctx::PerturbFe && self.small_vars.get(v).copied().unwrap_or(false) {
            return if self.p.vars[v].ty == Ty::Float { format!("cfe2_to_f32(v{v}).x") } else { format!("cfe2_to_f32(v{v})") };
        }
        if self.ctx == Ctx::Formula {
            format!("s.v{v}")
        } else {
            format!("v{v}")
        }
    }

    /// A global's value, from the block the CPU filled: a formula's
    /// `fdata`, a colouring's parameter block after its parameters.
    fn global(&self, v: VarId, k: usize) -> String {
        let slot = |k: usize| {
            if self.formula_side() {
                format!("fdata4({}u).{}", k / 4, ["x", "y", "z", "w"][k % 4])
            } else {
                format!("cparam({}u)", self.p.param_slots() + k)
            }
        };
        match self.p.vars[v].ty {
            Ty::Complex => format!("vec2<f32>({}, {})", slot(k), slot(k + 1)),
            Ty::Float => slot(k),
            Ty::Int => format!("i32(round({}))", slot(k)),
            Ty::Bool => format!("({} != 0.0)", slot(k)),
        }
    }

    fn use_(&mut self, h: Helper) -> &'static str {
        self.used.insert(h);
        h.name()
    }

    // ---- statements -----------------------------------------------------

    fn stmts(&mut self, stmts: &[IStmt], depth: usize, out: &mut String) {
        for s in stmts {
            self.stmt(s, depth, out);
        }
    }

    fn stmt(&mut self, s: &IStmt, depth: usize, out: &mut String) {
        let pad = "    ".repeat(depth);
        match s {
            IStmt::Assign(place, e) => {
                let v = self.expr(e);
                let target = match place {
                    Place::Var(id) => self.var(*id),
                    Place::Z => "s.z".to_string(),
                    Place::Index => "index".to_string(),
                    Place::Solid => "solid".to_string(),
                    Place::DZ => "dz".to_string(),
                };
                let _ = writeln!(out, "{pad}{target} = {v};");
            }
            IStmt::If { arms, otherwise } => {
                for (k, (cond, body)) in arms.iter().enumerate() {
                    let c = self.expr(cond);
                    if k == 0 {
                        let _ = writeln!(out, "{pad}if ({c}) {{");
                    } else {
                        let _ = writeln!(out, "{pad}}} else if ({c}) {{");
                    }
                    self.stmts(body, depth + 1, out);
                }
                if !otherwise.is_empty() {
                    let _ = writeln!(out, "{pad}}} else {{");
                    self.stmts(otherwise, depth + 1, out);
                }
                let _ = writeln!(out, "{pad}}}");
            }
            IStmt::While { cond, body } => {
                // Capped: a loop that never ends must not hang the GPU.
                let k = self.loops;
                self.loops += 1;
                let c = self.expr(cond);
                let _ = writeln!(out, "{pad}for (var loop{k} = 0; loop{k} < {LOOP_CAP}; loop{k} = loop{k} + 1) {{");
                let _ = writeln!(out, "{pad}    if (!({c})) {{\n{pad}        break;\n{pad}    }}");
                self.stmts(body, depth + 1, out);
                let _ = writeln!(out, "{pad}}}");
            }
            IStmt::Repeat { body, cond } => {
                let k = self.loops;
                self.loops += 1;
                let _ = writeln!(out, "{pad}for (var loop{k} = 0; loop{k} < {LOOP_CAP}; loop{k} = loop{k} + 1) {{");
                self.stmts(body, depth + 1, out);
                let c = self.expr(cond);
                let _ = writeln!(out, "{pad}    if ({c}) {{\n{pad}        break;\n{pad}    }}");
                let _ = writeln!(out, "{pad}}}");
            }
        }
    }

    // ---- expressions ----------------------------------------------------

    fn expr(&mut self, e: &IExpr) -> String {
        // WGSL folds a constant expression when it validates, and refuses
        // one whose value is not finite (`sqrt(-3)`, `1 / 0`, an
        // overflow), where UF computes NaN or infinity. Through `uf_k` a
        // constant is not WGSL's constant: the driver still folds it.
        if !self.opaque && constant(e) && !literal(e) {
            self.opaque = true;
            let s = self.expr(e);
            self.opaque = false;
            return s;
        }
        match &e.kind {
            IExprKind::Bool(b) => b.to_string(),
            IExprKind::Int(i) if self.opaque => {
                self.use_(Helper::Opaque);
                format!("uf_ki({}i)", *i as i32)
            }
            IExprKind::Int(i) => format!("{}i", *i as i32),
            IExprKind::Float(f) if self.opaque => {
                self.use_(Helper::Opaque);
                format!("uf_k({})", lit_f32(*f))
            }
            IExprKind::Float(f) => lit_f32(*f),
            IExprKind::Complex(re, im) if self.opaque => {
                self.use_(Helper::Opaque);
                format!("uf_kc(vec2<f32>({}, {}))", lit_f32(*re), lit_f32(*im))
            }
            IExprKind::Complex(re, im) => format!("vec2<f32>({}, {})", lit_f32(*re), lit_f32(*im)),
            IExprKind::Var(v) => self.var(*v),
            IExprKind::Param(id) => {
                let k = self.slots[*id];
                let f = self.param_fn();
                match self.p.params[*id].ty() {
                    Ty::Bool => format!("({f}({k}u) != 0.0)"),
                    Ty::Int => format!("i32(round({f}({k}u)))"),
                    Ty::Float => format!("{f}({k}u)"),
                    Ty::Complex => format!("vec2<f32>({f}({k}u), {f}({}u))", k + 1),
                }
            }
            IExprKind::Predef(p) => self.predef(*p),
            IExprKind::Convert(a) => {
                let v = self.expr(a);
                match (a.ty, e.ty) {
                    (Ty::Int, Ty::Float) => format!("f32({v})"),
                    (Ty::Int, Ty::Complex) => format!("vec2<f32>(f32({v}), 0.0)"),
                    (Ty::Float, Ty::Complex) => format!("vec2<f32>({v}, 0.0)"),
                    (Ty::Bool, Ty::Int) => format!("select(0i, 1i, {v})"),
                    (Ty::Bool, Ty::Float) => format!("select(0.0, 1.0, {v})"),
                    (Ty::Bool, Ty::Complex) => format!("vec2<f32>(select(0.0, 1.0, {v}), 0.0)"),
                    (Ty::Int, Ty::Bool) => format!("({v} != 0i)"),
                    (Ty::Float, Ty::Bool) => format!("({v} != 0.0)"),
                    // Fractint tests the real part alone.
                    (Ty::Complex, Ty::Bool) if self.p.dialect == Dialect::Fractint => format!("({v}.x != 0.0)"),
                    (Ty::Complex, Ty::Bool) => format!("any({v} != vec2<f32>(0.0, 0.0))"),
                    (from, to) => unreachable!("the checker never narrows {from:?} to {to:?}"),
                }
            }
            IExprKind::MakeComplex(a, b) => {
                let (a, b) = (self.expr(a), self.expr(b));
                format!("vec2<f32>({a}, {b})")
            }
            IExprKind::Unary(UnOp::Neg, a) => format!("(-{})", self.expr(a)),
            IExprKind::Unary(UnOp::Not, a) => format!("(!{})", self.expr(a)),
            IExprKind::Binary(op, a, b) => self.binary(*op, a, b),
            IExprKind::PowInt(a, n) => {
                let v = self.expr(a);
                match (a.ty, n) {
                    (_, 0) => if a.ty == Ty::Complex { "vec2<f32>(1.0, 0.0)".to_string() } else { "1.0".to_string() },
                    (_, 1) => v,
                    (Ty::Complex, 2) => format!("{}({v})", self.use_(Helper::CSqr)),
                    (Ty::Complex, _) => format!("{}({v}, {n}i)", self.use_(Helper::CPowI)),
                    (_, 2) => format!("{}({v})", self.use_(Helper::FSq)),
                    (_, _) => format!("{}({v}, {n}i)", self.use_(Helper::FPowI)),
                }
            }
            IExprKind::Modulus(a) => {
                let v = self.expr(a);
                if a.ty == Ty::Complex {
                    format!("{}({v})", self.use_(Helper::CNorm))
                } else {
                    format!("{}({v})", self.use_(Helper::FSq))
                }
            }
            IExprKind::Call(f, a) => self.call(*f, a, e.ty),
            IExprKind::UserCall(id, a) => {
                let v = self.expr(a);
                let k = self.func_slots[*id];
                let f = self.param_fn();
                format!("{}(i32(round({f}({k}u))), {v})", self.use_(Helper::User))
            }
            IExprKind::UserDeriv(id, a) => {
                let v = self.expr(a);
                let k = self.func_slots[*id];
                let f = self.param_fn();
                format!("{}(i32(round({f}({k}u))), {v})", self.use_(Helper::UserD))
            }
        }
    }

    fn predef(&mut self, p: Predef) -> String {
        match (p, self.ctx) {
            (Predef::Pixel, Ctx::Formula | Ctx::Perturb | Ctx::PerturbBailout | Ctx::PerturbFe) => "pixel".into(),
            (Predef::Z, Ctx::PerturbFe) => "cfe2_to_f32(cfe2_ref(zm, zlo, ze))".into(),
            (Predef::DZ, Ctx::PerturbFe) => "cfe2_to_f32(w)".into(),
            (Predef::DPixel, Ctx::PerturbFe) => "cfe2_to_f32(d0)".into(),
            (Predef::Z, Ctx::Perturb) => "z_ref".into(),
            (Predef::Z, Ctx::PerturbBailout) => "z".into(),
            // The equations' deviation and offset, held in units of S.
            (Predef::DZ, _) if self.true_small => "(s * w)".into(),
            (Predef::DPixel, _) if self.true_small => "(s * d0)".into(),
            (Predef::DZ, _) => "w".into(),
            (Predef::DPixel, _) => "d0".into(),
            (Predef::Pixel, _) => "esc_env_pixel".into(),
            (Predef::Z, Ctx::Formula) => "s.z".into(),
            (Predef::Z, Ctx::ColoringLoop) => "z".into(),
            (Predef::Z, Ctx::ColoringFinal) => "sum.z".into(),
            // A colouring's init runs before the first iterate.
            (Predef::Z, Ctx::ColoringInit) => "vec2<f32>(0.0, 0.0)".into(),
            (Predef::NumIter, _) => "i32(sum.n)".into(),
            (Predef::Pass, Ctx::Formula) => "s.npass".into(),
            // The perturbation equations read the reference's pass.
            (Predef::Pass, _) => "pass_index".into(),
            (Predef::MaxIter, _) => "i32(params.max_iter)".into(),
            (Predef::Pi, _) => "3.14159265358979".into(),
            (Predef::E, _) => "2.71828182845905".into(),
            (Predef::Random, _) => format!("{}()", self.use_(Helper::Random)),
            (Predef::Center, _) => "esc_env_center".into(),
            (Predef::Magn, _) => "esc_env_magn".into(),
            (Predef::X, _) => "esc_env_px.x".into(),
            (Predef::Y, _) => "esc_env_px.y".into(),
            (Predef::Width, _) => "esc_env_size.x".into(),
            (Predef::Height, _) => "esc_env_size.y".into(),
            (Predef::ScreenPixel, _) => "vec2<f32>(f32(esc_env_px.x), f32(esc_env_px.y))".into(),
            (Predef::Index, _) => "index".into(),
            (Predef::Solid, _) => "solid".into(),
            (Predef::WhiteSq, _) => "vec2<f32>(f32((esc_env_px.x + esc_env_px.y) & 1), 0.0)".into(),
            (Predef::ScrnMax, _) => "vec2<f32>(f32(esc_env_size.x), f32(esc_env_size.y))".into(),
            (Predef::IsMand, _) => "vec2<f32>(select(1.0, 0.0, (params.flags & 1u) != 0u), 0.0)".into(),
            (Predef::MagXMag, _) => "vec2<f32>(esc_env_magn, 1.0)".into(),
            (Predef::JuliaC, _) => "params.julia_c".into(),
            (Predef::RotSkew, _) => {
                format!("vec2<f32>(degrees({}(params.rot_cs.y, params.rot_cs.x)), 0.0)", self.use_(Helper::Atan2))
            }
        }
    }

    fn binary(&mut self, op: BinOp, a: &IExpr, b: &IExpr) -> String {
        use BinOp::*;
        // A real factor scales (IExpr::is_lifted_real).
        if op == Mul && a.ty == Ty::Complex && (a.is_lifted_real() || b.is_lifted_real()) {
            let (k, z) = if a.is_lifted_real() { (a, b) } else { (b, a) };
            let k = self.real_of(k);
            let z = self.expr(z);
            return format!("({k} * {z})");
        }
        let (l, r) = (self.expr(a), self.expr(b));
        let ty = a.ty;
        match op {
            And => format!("({l} && {r})"),
            Or => format!("({l} || {r})"),
            // Fractint compares real parts, equality too.
            Eq | Ne if ty == Ty::Complex && self.p.dialect == Dialect::Fractint => format!("({l}.x {} {r}.x)", op.symbol()),
            Eq | Ne if ty == Ty::Complex => {
                if op == Eq {
                    format!("all({l} == {r})")
                } else {
                    format!("any({l} != {r})")
                }
            }
            // Complex values order by their real parts.
            Lt | Le | Gt | Ge if ty == Ty::Complex => format!("({l}.x {} {r}.x)", op.symbol()),
            Eq | Ne | Lt | Le | Gt | Ge => format!("({l} {} {r})", op.symbol()),
            Add | Sub => format!("({l} {} {r})", op.symbol()),
            Mul if ty == Ty::Complex => format!("{}({l}, {r})", self.use_(Helper::CMul)),
            Div if ty == Ty::Complex => format!("{}({l}, {r})", self.use_(Helper::CDiv)),
            Pow if ty == Ty::Complex => format!("{}({l}, {r})", self.use_(Helper::CPow)),
            Pow => format!("{}({l}, {r})", self.use_(Helper::FPow)),
            Mod if ty == Ty::Int => format!("{}({l}, {r})", self.use_(Helper::IMod)),
            Mul | Div | Mod => format!("({l} {} {r})", op.symbol()),
            DiffAbs => format!("{}({l}, {r})", self.use_(Helper::DiffAbs)),
        }
    }

    // ---- the perturbation equations on the scaled rung -----------------
    //
    // Each value is Full (the reference's scale: `#z`, parameters,
    // constants) or Small (the deviation's: `#dz`, `#dpixel` and what
    // they make), and a Small one is held in units of S. The rules are
    // the built-in tiers' derivations: a Small times a Full is Small,
    // a product of two Smalls carries one factor S (`S w^2`), a Small
    // over a Full is Small and a ratio of Smalls is Full. Anything else
    // takes the true values (`S` times the held one) and is Full: right,
    // merely without the scaled form's protection from rounding.

    fn sstmts(&mut self, stmts: &[IStmt], depth: usize, out: &mut String) {
        for s in stmts {
            self.sstmt(s, depth, out);
        }
    }

    fn sstmt(&mut self, s: &IStmt, depth: usize, out: &mut String) {
        let pad = "    ".repeat(depth);
        match s {
            IStmt::Assign(place, e) => {
                let (code, class) = self.sgen(e);
                let (target, want) = match place {
                    Place::Var(v) => (format!("v{v}"), if self.small_vars[*v] { Class::Small } else { Class::Full }),
                    // The checker lets the equations write nothing else.
                    _ => ("w".to_string(), Class::Small),
                };
                let code = as_class(code, class, want);
                let _ = writeln!(out, "{pad}{target} = {code};");
            }
            IStmt::If { arms, otherwise } => {
                for (k, (cond, body)) in arms.iter().enumerate() {
                    let c = self.full_code(cond);
                    if k == 0 {
                        let _ = writeln!(out, "{pad}if ({c}) {{");
                    } else {
                        let _ = writeln!(out, "{pad}}} else if ({c}) {{");
                    }
                    self.sstmts(body, depth + 1, out);
                }
                if !otherwise.is_empty() {
                    let _ = writeln!(out, "{pad}}} else {{");
                    self.sstmts(otherwise, depth + 1, out);
                }
                let _ = writeln!(out, "{pad}}}");
            }
            IStmt::While { cond, body } => {
                let k = self.loops;
                self.loops += 1;
                let c = self.full_code(cond);
                let _ = writeln!(out, "{pad}for (var loop{k} = 0; loop{k} < {LOOP_CAP}; loop{k} = loop{k} + 1) {{");
                let _ = writeln!(out, "{pad}    if (!({c})) {{\n{pad}        break;\n{pad}    }}");
                self.sstmts(body, depth + 1, out);
                let _ = writeln!(out, "{pad}}}");
            }
            IStmt::Repeat { body, cond } => {
                let k = self.loops;
                self.loops += 1;
                let _ = writeln!(out, "{pad}for (var loop{k} = 0; loop{k} < {LOOP_CAP}; loop{k} = loop{k} + 1) {{");
                self.sstmts(body, depth + 1, out);
                let c = self.full_code(cond);
                let _ = writeln!(out, "{pad}    if ({c}) {{\n{pad}        break;\n{pad}    }}");
                let _ = writeln!(out, "{pad}}}");
            }
        }
    }

    /// Whether an expression reads a deviation.
    fn has_small(&self, e: &IExpr) -> bool {
        match &e.kind {
            IExprKind::Predef(Predef::DZ | Predef::DPixel) => true,
            IExprKind::Var(v) => self.small_vars.get(*v).copied().unwrap_or(false),
            IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => {
                self.has_small(a)
            }
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => self.has_small(a) || self.has_small(b),
            _ => false,
        }
    }

    /// An expression's true value.
    fn full_code(&mut self, e: &IExpr) -> String {
        let (c, k) = self.sgen(e);
        as_class(c, k, Class::Full)
    }

    /// An expression the scaled form has no rule for, on true values.
    fn true_expr(&mut self, e: &IExpr) -> String {
        let saved = self.true_small;
        self.true_small = true;
        let c = self.expr(e);
        self.true_small = saved;
        c
    }

    /// An expression's code and class: a Small one's code is its value
    /// in units of S.
    fn sgen(&mut self, e: &IExpr) -> (String, Class) {
        use Class::{Full, Small};
        if !self.has_small(e) {
            return (self.expr(e), Full);
        }
        match &e.kind {
            IExprKind::Predef(Predef::DZ) => ("w".into(), Small),
            IExprKind::Predef(Predef::DPixel) => ("d0".into(), Small),
            IExprKind::Var(v) => (format!("v{v}"), Small),
            IExprKind::Convert(a) if e.ty != Ty::Bool && a.ty != Ty::Bool => {
                let (c, k) = self.sgen(a);
                (convert_code(&c, a.ty, e.ty), k)
            }
            IExprKind::MakeComplex(a, b) => {
                let (ca, ka) = self.sgen(a);
                let (cb, kb) = self.sgen(b);
                if ka == Small && kb == Small {
                    (format!("vec2<f32>({ca}, {cb})"), Small)
                } else {
                    (format!("vec2<f32>({}, {})", as_class(ca, ka, Full), as_class(cb, kb, Full)), Full)
                }
            }
            IExprKind::Unary(UnOp::Neg, a) => {
                let (c, k) = self.sgen(a);
                (format!("(-{c})"), k)
            }
            IExprKind::Binary(op @ (BinOp::Add | BinOp::Sub), a, b) if e.ty != Ty::Bool => {
                let (ca, ka) = self.sgen(a);
                let (cb, kb) = self.sgen(b);
                let o = op.symbol();
                if ka == Small && kb == Small {
                    (format!("({ca} {o} {cb})"), Small)
                } else {
                    (format!("({} {o} {})", as_class(ca, ka, Full), as_class(cb, kb, Full)), Full)
                }
            }
            IExprKind::Binary(BinOp::Mul, a, b) if e.ty != Ty::Bool && e.ty != Ty::Int => {
                let (ca, ka) = self.sgen(a);
                let (cb, kb) = self.sgen(b);
                let prod = self.mul_code(&ca, a, ka, &cb, b, kb, e.ty);
                match (ka, kb) {
                    (Small, Small) => (format!("(s * {prod})"), Small),
                    (Full, Full) => (prod, Full),
                    _ => (prod, Small),
                }
            }
            IExprKind::Binary(BinOp::Div, a, b) if e.ty != Ty::Int => {
                let (ca, ka) = self.sgen(a);
                let (cb, kb) = self.sgen(b);
                match (ka, kb) {
                    (Small, Full) => (self.div_code(&ca, a.ty, &cb, b.ty), Small),
                    (Small, Small) => (self.div_code(&ca, a.ty, &cb, b.ty), Full),
                    _ => {
                        let (fa, fb) = (as_class(ca, ka, Full), as_class(cb, kb, Full));
                        (self.div_code(&fa, a.ty, &fb, b.ty), Full)
                    }
                }
            }
            IExprKind::Binary(BinOp::DiffAbs, a, b) => {
                // Positively homogeneous: diffabs(C, S w) / S is
                // diffabs(C / S, w), as the built-in Ship tier has it.
                let (ca, ka) = self.sgen(a);
                let (cb, kb) = self.sgen(b);
                let f = self.use_(Helper::DiffAbs);
                if kb == Small {
                    (format!("{f}({}, {cb})", as_class(ca, ka, Small)), Small)
                } else {
                    (format!("{f}({}, {})", as_class(ca, ka, Full), as_class(cb, kb, Full)), Full)
                }
            }
            IExprKind::PowInt(a, n) if *n >= 1 && a.ty != Ty::Int => {
                let (ca, ka) = self.sgen(a);
                if ka == Full {
                    return (self.true_expr(e), Full);
                }
                match *n {
                    1 => (ca, Small),
                    2 => (format!("(s * {})", self.sq_code(&ca, a.ty)), Small),
                    n => (format!("(pow(s, {}.0) * {})", n - 1, self.powi_code(&ca, a.ty, n)), Small),
                }
            }
            IExprKind::Modulus(a) => {
                let (ca, ka) = self.sgen(a);
                let norm = self.norm_code(&ca, a.ty);
                if ka == Small {
                    (format!("(s * {norm})"), Small)
                } else {
                    (norm, Full)
                }
            }
            IExprKind::Call(f, a) if a.ty != Ty::Int => {
                let (ca, ka) = self.sgen(a);
                let complex = a.ty == Ty::Complex;
                match (f, ka) {
                    (Builtin::Sqr, Small) => (format!("(s * {})", self.sq_code(&ca, a.ty)), Small),
                    (Builtin::Ident, Small) => (ca, Small),
                    (Builtin::Zero, Small) => (zero_of(e.ty).to_string(), Small),
                    (Builtin::Conj, Small) if complex => (format!("({ca} * vec2<f32>(1.0, -1.0))"), Small),
                    (Builtin::Flip, Small) if complex => (format!("({ca}).yx"), Small),
                    (Builtin::Real, Small) if complex => (format!("({ca}).x"), Small),
                    (Builtin::Imag, Small) if complex => (format!("({ca}).y"), Small),
                    _ => (self.true_expr(e), Full),
                }
            }
            _ => (self.true_expr(e), Full),
        }
    }

    // ---- the perturbation equations on the floatexp rung ---------------

    fn festmts(&mut self, stmts: &[IStmt], depth: usize, out: &mut String) {
        for s in stmts {
            self.festmt(s, depth, out);
        }
    }

    fn festmt(&mut self, s: &IStmt, depth: usize, out: &mut String) {
        let pad = "    ".repeat(depth);
        match s {
            IStmt::Assign(place, e) => {
                let (target, cfe) = match place {
                    Place::Var(v) => (format!("v{v}"), self.small_vars[*v]),
                    // The checker lets the equations write nothing else.
                    _ => ("w".to_string(), true),
                };
                let code = if cfe { self.fegen(e) } else { self.expr(e) };
                let _ = writeln!(out, "{pad}{target} = {code};");
            }
            IStmt::If { arms, otherwise } => {
                for (k, (cond, body)) in arms.iter().enumerate() {
                    let c = self.expr(cond);
                    if k == 0 {
                        let _ = writeln!(out, "{pad}if ({c}) {{");
                    } else {
                        let _ = writeln!(out, "{pad}}} else if ({c}) {{");
                    }
                    self.festmts(body, depth + 1, out);
                }
                if !otherwise.is_empty() {
                    let _ = writeln!(out, "{pad}}} else {{");
                    self.festmts(otherwise, depth + 1, out);
                }
                let _ = writeln!(out, "{pad}}}");
            }
            IStmt::While { cond, body } => {
                let k = self.loops;
                self.loops += 1;
                let c = self.expr(cond);
                let _ = writeln!(out, "{pad}for (var loop{k} = 0; loop{k} < {LOOP_CAP}; loop{k} = loop{k} + 1) {{");
                let _ = writeln!(out, "{pad}    if (!({c})) {{\n{pad}        break;\n{pad}    }}");
                self.festmts(body, depth + 1, out);
                let _ = writeln!(out, "{pad}}}");
            }
            IStmt::Repeat { body, cond } => {
                let k = self.loops;
                self.loops += 1;
                let _ = writeln!(out, "{pad}for (var loop{k} = 0; loop{k} < {LOOP_CAP}; loop{k} = loop{k} + 1) {{");
                self.festmts(body, depth + 1, out);
                let c = self.expr(cond);
                let _ = writeln!(out, "{pad}    if ({c}) {{\n{pad}        break;\n{pad}    }}");
                let _ = writeln!(out, "{pad}}}");
            }
        }
    }

    /// Whether an expression reads a CFe2 value: a deviation, the
    /// reference iterate, or one of the equations' numeric variables.
    fn reads_cfe(&self, e: &IExpr) -> bool {
        match &e.kind {
            IExprKind::Predef(Predef::DZ | Predef::DPixel | Predef::Z) => true,
            IExprKind::Var(v) => self.small_vars.get(*v).copied().unwrap_or(false),
            IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => {
                self.reads_cfe(a)
            }
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => self.reads_cfe(a) || self.reads_cfe(b),
            _ => false,
        }
    }

    /// A numeric expression as a CFe2 (a float's imaginary part zero).
    fn fegen(&mut self, e: &IExpr) -> String {
        let plain = |g: &mut Self, e: &IExpr| {
            let v = g.expr(e);
            let v = match e.ty {
                Ty::Complex => v,
                Ty::Int => format!("vec2<f32>(f32({v}), 0.0)"),
                Ty::Bool => format!("vec2<f32>(select(0.0, 1.0, {v}), 0.0)"),
                Ty::Float => format!("vec2<f32>({v}, 0.0)"),
            };
            format!("cfe2_from_f32({v})")
        };
        if !self.reads_cfe(e) {
            return plain(self, e);
        }
        let numeric = |t: Ty| matches!(t, Ty::Float | Ty::Complex);
        match &e.kind {
            IExprKind::Predef(Predef::DZ) => "w".into(),
            IExprKind::Predef(Predef::DPixel) => "d0".into(),
            IExprKind::Predef(Predef::Z) => "cfe2_ref(zm, zlo, ze)".into(),
            IExprKind::Var(v) => format!("v{v}"),
            IExprKind::Convert(a) if numeric(a.ty) && numeric(e.ty) => self.fegen(a),
            IExprKind::MakeComplex(a, b) => {
                let (ca, cb) = (self.fegen(a), self.fegen(b));
                format!("cfe2_add({ca}, cfe2_times_i({cb}))")
            }
            IExprKind::Unary(UnOp::Neg, a) if numeric(a.ty) => format!("cfe2_neg({})", self.fegen(a)),
            IExprKind::Binary(op @ (BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div), a, b) if numeric(e.ty) => {
                let (ca, cb) = (self.fegen(a), self.fegen(b));
                let f = match op {
                    BinOp::Add => "cfe2_add",
                    BinOp::Sub => "cfe2_sub",
                    BinOp::Mul => "cfe2_mul",
                    _ => "cfe2_div",
                };
                format!("{f}({ca}, {cb})")
            }
            IExprKind::PowInt(a, n) if *n >= 1 && numeric(a.ty) => {
                let ca = self.fegen(a);
                match *n {
                    1 => ca,
                    2 => format!("cfe2_sqr({ca})"),
                    n => format!("cfe2_powi({ca}, {n}i)"),
                }
            }
            IExprKind::Modulus(a) if numeric(a.ty) => format!("cfe2_abs2({})", self.fegen(a)),
            IExprKind::Binary(BinOp::DiffAbs, a, b) => {
                let (ca, cb) = (self.fegen(a), self.fegen(b));
                format!("cfe2_diffabs({ca}, {cb})")
            }
            IExprKind::Call(f, a) if numeric(a.ty) && numeric(e.ty) => {
                let complex = a.ty == Ty::Complex;
                match f {
                    Builtin::Sqr => format!("cfe2_sqr({})", self.fegen(a)),
                    Builtin::Ident => self.fegen(a),
                    Builtin::Zero => "cfe2_zero()".into(),
                    Builtin::Recip => format!("cfe2_recip({})", self.fegen(a)),
                    Builtin::Abs => format!("cfe2_abs({})", self.fegen(a)),
                    Builtin::Conj if complex => format!("cfe2_conj({})", self.fegen(a)),
                    Builtin::Flip if complex => format!("cfe2_flip({})", self.fegen(a)),
                    Builtin::Real if complex => format!("cfe2_re({})", self.fegen(a)),
                    Builtin::Imag if complex => format!("cfe2_im({})", self.fegen(a)),
                    _ => plain(self, e),
                }
            }
            _ => plain(self, e),
        }
    }

    /// A product's code; a Full lifted real scales.
    #[allow(clippy::too_many_arguments)]
    fn mul_code(&mut self, ca: &str, a: &IExpr, ka: Class, cb: &str, b: &IExpr, kb: Class, ty: Ty) -> String {
        if ty == Ty::Complex && a.ty == Ty::Complex && b.ty == Ty::Complex {
            if ka == Class::Full && a.is_lifted_real() {
                return format!("({} * {cb})", self.real_of(a));
            }
            if kb == Class::Full && b.is_lifted_real() {
                return format!("({ca} * {})", self.real_of(b));
            }
            return format!("{}({ca}, {cb})", self.use_(Helper::CMul));
        }
        format!("({ca} * {cb})")
    }

    fn div_code(&mut self, ca: &str, at: Ty, cb: &str, bt: Ty) -> String {
        if at == Ty::Complex && bt == Ty::Complex {
            format!("{}({ca}, {cb})", self.use_(Helper::CDiv))
        } else {
            format!("({ca} / {cb})")
        }
    }

    fn sq_code(&mut self, c: &str, t: Ty) -> String {
        if t == Ty::Complex {
            format!("{}({c})", self.use_(Helper::CSqr))
        } else {
            format!("{}({c})", self.use_(Helper::FSq))
        }
    }

    fn powi_code(&mut self, c: &str, t: Ty, n: i64) -> String {
        if t == Ty::Complex {
            format!("{}({c}, {n}i)", self.use_(Helper::CPowI))
        } else {
            format!("{}({c}, {n}i)", self.use_(Helper::FPowI))
        }
    }

    fn norm_code(&mut self, c: &str, t: Ty) -> String {
        if t == Ty::Complex {
            format!("{}({c})", self.use_(Helper::CNorm))
        } else {
            format!("{}({c})", self.use_(Helper::FSq))
        }
    }

    /// The real value a lifted real holds.
    fn real_of(&mut self, e: &IExpr) -> String {
        match &e.kind {
            IExprKind::Complex(re, _) => lit_f32(*re),
            IExprKind::Convert(a) if a.ty == Ty::Int => format!("f32({})", self.expr(a)),
            IExprKind::Convert(a) => self.expr(a),
            _ => unreachable!("is_lifted_real said so"),
        }
    }

    fn call(&mut self, f: Builtin, a: &IExpr, ty: Ty) -> String {
        use Builtin::*;
        let v = self.expr(a);
        // Fractint's round is floor(x + 0.5), halves always up.
        if f == Round && self.p.dialect == Dialect::Fractint {
            return match (a.ty, ty) {
                (Ty::Int, _) => v,
                (_, Ty::Int) => format!("i32(floor({v} + 0.5))"),
                (Ty::Complex, _) => format!("floor({v} + vec2<f32>(0.5, 0.5))"),
                _ => format!("floor({v} + 0.5)"),
            };
        }
        if a.ty == Ty::Complex {
            return match f {
                Real => format!("{v}.x"),
                Imag => format!("{v}.y"),
                Ident => v,
                Zero => "vec2<f32>(0.0, 0.0)".into(),
                Conj => format!("{}({v})", self.use_(Helper::CConj)),
                Flip => format!("{v}.yx"),
                Cabs => format!("length({v})"),
                Abs => format!("abs({v})"),
                Atan2 => format!("{}({v})", self.use_(Helper::CArg)),
                IsNan => format!("{}({v})", self.use_(Helper::CIsNan)),
                IsInf => format!("{}({v})", self.use_(Helper::CIsInf)),
                _ => {
                    let h = complex_helper(f);
                    format!("{}({v})", self.use_(h))
                }
            };
        }
        // A real argument with a complex result: lift it first.
        if ty == Ty::Complex {
            let lifted = format!("vec2<f32>({}, 0.0)", if a.ty == Ty::Int { format!("f32({v})") } else { v });
            let h = complex_helper(f);
            return format!("{}({lifted})", self.use_(h));
        }
        if a.ty == Ty::Int && ty == Ty::Int {
            return match f {
                Sqr => format!("({v} * {v})"),
                Abs => format!("abs({v})"),
                Zero => "0i".into(),
                _ => v,
            };
        }
        let real = match f {
            Sqr => format!("{}({v})", self.use_(Helper::FSq)),
            Sqrt => format!("sqrt({v})"),
            Log => format!("log({v})"),
            Exp => format!("exp({v})"),
            Abs | Cabs => format!("abs({v})"),
            Recip => format!("(1.0 / {v})"),
            Ident | Real => v,
            Zero | Imag => "0.0".into(),
            Sin => format!("sin({}({v}))", self.use_(Helper::Reduce)),
            Cos => format!("cos({}({v}))", self.use_(Helper::Reduce)),
            Tan => format!("tan({}({v}))", self.use_(Helper::Reduce)),
            Cotan => format!("(1.0 / tan({}({v})))", self.use_(Helper::Reduce)),
            Sinh => format!("sinh({v})"),
            Cosh => format!("cosh({v})"),
            Tanh => format!("tanh({v})"),
            Cotanh => format!("(1.0 / tanh({v}))"),
            Asin => format!("asin({v})"),
            Acos => format!("acos({v})"),
            Atan => format!("atan({v})"),
            Asinh => format!("asinh({v})"),
            Acosh => format!("acosh({v})"),
            Atanh => format!("atanh({v})"),
            Atan2 => format!("{}(0.0, {v})", self.use_(Helper::Atan2)),
            Round => format!("{}({v})", self.use_(Helper::Round)),
            Trunc => format!("trunc({v})"),
            Ceil => format!("ceil({v})"),
            Floor => format!("floor({v})"),
            IsNan => return format!("{}({v})", self.use_(Helper::IsNan)),
            IsInf => return format!("{}({v})", self.use_(Helper::IsInf)),
            Conj | Flip | Cosxx => unreachable!("complex-valued"),
        };
        if ty == Ty::Int {
            format!("i32({real})")
        } else {
            real
        }
    }
}

fn complex_helper(f: Builtin) -> Helper {
    use Builtin::*;
    match f {
        Sqr => Helper::CSqr,
        Sqrt => Helper::CSqrt,
        Log => Helper::CLog,
        Exp => Helper::CExp,
        Recip => Helper::CRecip,
        Sin => Helper::CSin,
        Cos => Helper::CCos,
        Tan => Helper::CTan,
        Cotan => Helper::CCotan,
        Sinh => Helper::CSinh,
        Cosh => Helper::CCosh,
        Tanh => Helper::CTanh,
        Cotanh => Helper::CCotanh,
        Asin => Helper::CAsin,
        Acos => Helper::CAcos,
        Atan => Helper::CAtan,
        Asinh => Helper::CAsinh,
        Acosh => Helper::CAcosh,
        Atanh => Helper::CAtanh,
        Round => Helper::CRound,
        Trunc => Helper::CTrunc,
        Ceil => Helper::CCeil,
        Floor => Helper::CFloor,
        Conj => Helper::CConj,
        Flip => Helper::CFlip,
        Cosxx => Helper::CCosxx,
        Abs => Helper::CAbs,
        Cabs => Helper::CCabs,
        Real => Helper::CReal,
        Imag => Helper::CImag,
        Ident => Helper::CIdent,
        Zero => Helper::CZero,
        Atan2 => Helper::CArgC,
        IsNan | IsInf => unreachable!("bool-valued"),
    }
}

/// The scale a perturbation-equation value lives at (see `Gen::sgen`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// The reference's: its true value.
    Full,
    /// The deviation's: held in units of S.
    Small,
}

fn as_class(code: String, from: Class, to: Class) -> String {
    match (from, to) {
        (Class::Small, Class::Full) => format!("(s * {code})"),
        (Class::Full, Class::Small) => format!("({code} / s)"),
        _ => code,
    }
}

fn convert_code(c: &str, from: Ty, to: Ty) -> String {
    match (from, to) {
        (Ty::Int, Ty::Float) => format!("f32({c})"),
        (Ty::Int, Ty::Complex) => format!("vec2<f32>(f32({c}), 0.0)"),
        (Ty::Float, Ty::Complex) => format!("vec2<f32>({c}, 0.0)"),
        _ => c.to_string(),
    }
}

/// Which of the perturbation equations' variables hold a deviation
/// (Small): every one, unless something it is assigned is Full, to a
/// fixpoint -- the class each assignment's value has by `Gen::sgen`'s
/// rules.
fn perturb_var_classes(p: &Program) -> Vec<bool> {
    let mut small: Vec<bool> = p.vars.iter().map(|v| v.perturb && v.ty != Ty::Bool && v.ty != Ty::Int).collect();
    fn class(e: &IExpr, small: &[bool]) -> Class {
        use Class::{Full, Small};
        fn has(e: &IExpr, small: &[bool]) -> bool {
            match &e.kind {
                IExprKind::Predef(Predef::DZ | Predef::DPixel) => true,
                IExprKind::Var(v) => small[*v],
                IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => has(a, small),
                IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => has(a, small) || has(b, small),
                _ => false,
            }
        }
        if !has(e, small) {
            return Full;
        }
        match &e.kind {
            IExprKind::Predef(_) | IExprKind::Var(_) => Small,
            IExprKind::Convert(a) if e.ty != Ty::Bool && a.ty != Ty::Bool => class(a, small),
            IExprKind::MakeComplex(a, b) => {
                if class(a, small) == Small && class(b, small) == Small {
                    Small
                } else {
                    Full
                }
            }
            IExprKind::Unary(UnOp::Neg, a) => class(a, small),
            IExprKind::Binary(BinOp::Add | BinOp::Sub, a, b) if e.ty != Ty::Bool => {
                if class(a, small) == Small && class(b, small) == Small {
                    Small
                } else {
                    Full
                }
            }
            IExprKind::Binary(BinOp::Mul, a, b) if e.ty != Ty::Bool && e.ty != Ty::Int => {
                if class(a, small) == Full && class(b, small) == Full {
                    Full
                } else {
                    Small
                }
            }
            IExprKind::Binary(BinOp::Div, a, b) if e.ty != Ty::Int => {
                if class(a, small) == Small && class(b, small) == Full {
                    Small
                } else {
                    Full
                }
            }
            IExprKind::PowInt(a, n) if *n >= 1 && a.ty != Ty::Int => class(a, small),
            IExprKind::Binary(BinOp::DiffAbs, _, b) => class(b, small),
            IExprKind::Modulus(a) => class(a, small),
            IExprKind::Call(f, a) if a.ty != Ty::Int => {
                let complex = a.ty == Ty::Complex;
                let kept = matches!(f, Builtin::Sqr | Builtin::Ident | Builtin::Zero)
                    || (complex && matches!(f, Builtin::Conj | Builtin::Flip | Builtin::Real | Builtin::Imag));
                if kept && class(a, small) == Small {
                    Small
                } else {
                    Full
                }
            }
            _ => Full,
        }
    }
    fn assigns(stmts: &[IStmt], out: &mut Vec<(VarId, IExpr)>) {
        for s in stmts {
            match s {
                IStmt::Assign(Place::Var(v), e) => out.push((*v, e.clone())),
                IStmt::Assign(..) => {}
                IStmt::If { arms, otherwise } => {
                    for (_, b) in arms {
                        assigns(b, out);
                    }
                    assigns(otherwise, out);
                }
                IStmt::While { body, .. } | IStmt::Repeat { body, .. } => assigns(body, out),
            }
        }
    }
    let mut all = Vec::new();
    assigns(&p.perturb_init, &mut all);
    assigns(&p.perturb_loop, &mut all);
    // A literal zero is zero in any unit: it leaves a deviation small
    // (a derived equation's arm that leaves one unchanged from zero).
    let zero = |e: &IExpr| match &e.kind {
        IExprKind::Float(x) => *x == 0.0,
        IExprKind::Complex(x, y) => *x == 0.0 && *y == 0.0,
        _ => false,
    };
    loop {
        let mut changed = false;
        for (v, e) in &all {
            if small[*v] && !zero(e) && class(e, &small) == Class::Full {
                small[*v] = false;
                changed = true;
            }
        }
        if !changed {
            return small;
        }
    }
}

/// Whether an expression reads a variable that is not a global: what
/// the perturbed path does not have, which keeps the reference's `z`
/// alone.
pub fn reads_a_variable(e: &IExpr, p: &Program) -> bool {
    match &e.kind {
        IExprKind::Var(v) => !p.vars[*v].global,
        IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => {
            reads_a_variable(a, p)
        }
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => reads_a_variable(a, p) || reads_a_variable(b, p),
        _ => false,
    }
}

/// Whether WGSL would take an expression for a constant: no variable,
/// parameter, symbol or user function in it.
fn constant(e: &IExpr) -> bool {
    match &e.kind {
        IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) | IExprKind::Complex(..) => true,
        IExprKind::Var(_) | IExprKind::Param(_) | IExprKind::Predef(_) | IExprKind::UserCall(..) | IExprKind::UserDeriv(..) => false,
        IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) => {
            constant(a)
        }
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => constant(a) && constant(b),
    }
}

/// A literal, negated or widened: always finite, nothing to fold.
fn literal(e: &IExpr) -> bool {
    match &e.kind {
        IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) | IExprKind::Complex(..) => true,
        IExprKind::Unary(UnOp::Neg, a) | IExprKind::Convert(a) => literal(a),
        _ => false,
    }
}

/// Whether a colouring can mark a pixel solid: it writes `#solid`, or it
/// colours the inside only.
pub fn writes_solid(p: &Program) -> bool {
    p.region == Region::Inside || writes(&p.final_body, Place::Solid)
}

/// Whether a colouring sets anything a pixel shows: `#index` or `#solid`.
/// One that sets neither compiles, and draws one flat colour.
pub fn colours_anything(p: &Program) -> bool {
    writes(&p.final_body, Place::Index) || writes_solid(p)
}

fn writes(stmts: &[IStmt], place: Place) -> bool {
    stmts.iter().any(|s| match s {
        IStmt::Assign(p, _) => *p == place,
        IStmt::If { arms, otherwise } => arms.iter().any(|(_, b)| writes(b, place)) || writes(otherwise, place),
        IStmt::While { body, .. } | IStmt::Repeat { body, .. } => writes(body, place),
    })
}

fn wgsl_ty(t: Ty) -> &'static str {
    match t {
        Ty::Bool => "bool",
        Ty::Int => "i32",
        Ty::Float => "f32",
        Ty::Complex => "vec2<f32>",
    }
}

fn zero_of(t: Ty) -> &'static str {
    match t {
        Ty::Bool => "false",
        Ty::Int => "0i",
        Ty::Float => "0.0",
        Ty::Complex => "vec2<f32>(0.0, 0.0)",
    }
}

/// An f32 literal WGSL accepts: finite, never subnormal (it would flush
/// to zero on the GPU, and the shader lint refuses it), always with a
/// point or an exponent.
pub fn lit_f32(v: f64) -> String {
    let f = v as f32;
    let f = if !f.is_finite() {
        if v.is_nan() {
            0.0
        } else if v > 0.0 {
            f32::MAX
        } else {
            f32::MIN
        }
    } else if f != 0.0 && f.abs() < f32::MIN_POSITIVE {
        0.0
    } else {
        f
    };
    let s = format!("{f:?}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

/// A comment-safe rendering of an entry name.
fn sanitize(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).collect()
}

/// The helper functions, written with the `uf_` prefix that `finish`
/// replaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Helper {
    Reduce,
    Atan2,
    Round,
    IsNan,
    IsInf,
    FSq,
    FPow,
    FPowI,
    IMod,
    Random,
    CMul,
    CDiv,
    CNorm,
    CSqr,
    CPowI,
    CPow,
    CRecip,
    CConj,
    CFlip,
    CAbs,
    CCabs,
    CReal,
    CImag,
    CIdent,
    CZero,
    CArg,
    CArgC,
    CIsNan,
    CIsInf,
    CLog,
    CExp,
    CSqrt,
    CSin,
    CCos,
    CTan,
    CCotan,
    CSinh,
    CCosh,
    CTanh,
    CCotanh,
    CAsin,
    CAcos,
    CAtan,
    CAsinh,
    CAcosh,
    CAtanh,
    CRound,
    CTrunc,
    CCeil,
    CFloor,
    CCosxx,
    DiffAbs,
    User,
    UserD,
    Opaque,
}

impl Helper {
    fn name(self) -> &'static str {
        use Helper::*;
        match self {
            Reduce => "uf_reduce",
            Atan2 => "uf_atan2",
            Round => "uf_round",
            IsNan => "uf_isnan",
            IsInf => "uf_isinf",
            FSq => "uf_fsq",
            DiffAbs => "uf_diffabs",
            FPow => "uf_powf",
            FPowI => "uf_powi",
            IMod => "uf_imod",
            Random => "uf_random",
            CMul => "uf_cmul",
            CDiv => "uf_cdiv",
            CNorm => "uf_cnorm",
            CSqr => "uf_csqr",
            CPowI => "uf_cpowi",
            CPow => "uf_cpow",
            CRecip => "uf_crecip",
            CConj => "uf_cconj",
            CFlip => "uf_cflip",
            CAbs => "uf_cabsc",
            CCabs => "uf_ccabs",
            CReal => "uf_creal",
            CImag => "uf_cimag",
            CIdent => "uf_cident",
            CZero => "uf_czero",
            CArg => "uf_carg",
            CArgC => "uf_cargc",
            CIsNan => "uf_cisnan",
            CIsInf => "uf_cisinf",
            CLog => "uf_clog",
            CExp => "uf_cexp",
            CSqrt => "uf_csqrt",
            CSin => "uf_csin",
            CCos => "uf_ccos",
            CTan => "uf_ctan",
            CCotan => "uf_ccotan",
            CSinh => "uf_csinh",
            CCosh => "uf_ccosh",
            CTanh => "uf_ctanh",
            CCotanh => "uf_ccotanh",
            CAsin => "uf_casin",
            CAcos => "uf_cacos",
            CAtan => "uf_catan",
            CAsinh => "uf_casinh",
            CAcosh => "uf_cacosh",
            CAtanh => "uf_catanh",
            CRound => "uf_cround",
            CTrunc => "uf_ctrunc",
            CCeil => "uf_cceil",
            CFloor => "uf_cfloor",
            CCosxx => "uf_ccosxx",
            User => "uf_user",
            UserD => "uf_user_d",
            Opaque => "uf_k",
        }
    }

    fn deps(self) -> &'static [Helper] {
        use Helper::*;
        match self {
            FPow => &[],
            CSqr | CPowI => &[CMul, CDiv],
            CPow => &[CMul, CLog, CExp],
            CRecip => &[CDiv],
            CArg | CArgC => &[Atan2],
            CLog => &[Atan2],
            CExp | CSin | CCos | CSinh | CCosh => &[Reduce],
            CTan | CCotan => &[CSin, CCos, CDiv],
            CTanh | CCotanh => &[CSinh, CCosh, CDiv],
            CAsin => &[CMul, CSqrt, CLog],
            CAcos => &[CAsin],
            CAtan | CAtanh => &[CDiv, CLog],
            CAsinh | CAcosh => &[CMul, CSqrt, CLog],
            CRound => &[Round],
            CCosxx => &[CCos],
            CIsNan => &[IsNan],
            CIsInf => &[IsInf],
            User => &[
                CSin, CCos, CTan, CCotan, CSinh, CCosh, CTanh, CCotanh, CAsin, CAcos, CAtan, CAsinh, CAcosh, CAtanh,
                CSqr, CSqrt, CLog, CExp, CAbs, CCabs, CConj, CFlip, CIdent, CZero, CRecip, CReal, CImag, CCosxx,
                CRound, CTrunc, CCeil, CFloor,
            ],
            UserD => &[CMul, CDiv, CSqr, CSqrt, CRecip, CExp, CSin, CCos, CSinh, CCosh],
            _ => &[],
        }
    }

    fn source(self) -> String {
        use Helper::*;
        let src: &str = match self {
            // Cody-Waite reduction of a trig argument: the template's
            // `esc_reduce`, which the measured M2 cos error needed.
            Reduce => "fn uf_reduce(t: f32) -> f32 {
    let k = round(t * 0.15915494309189535);
    var r = fma(-k, 6.2831855, t);
    r = fma(-k, -1.7484555e-7, r);
    r = fma(-k, -7.1054274e-15, r);
    return r;
}",
            // IEEE-exact at the zero pairs, where Metal's atan2 is wrong
            // (CLAUDE.md; shaders/core/utilities.wgsl `ff_atan2`).
            Atan2 => "fn uf_atan2(y: f32, x: f32) -> f32 {
    if (y == 0.0 && x == 0.0) {
        let pi = 3.14159265358979;
        let mag = select(0.0, pi, (bitcast<u32>(x) & 0x80000000u) != 0u);
        return select(mag, -mag, (bitcast<u32>(y) & 0x80000000u) != 0u);
    }
    return atan2(y, x);
}",
            // Halves away from zero, as UF's round (WGSL's goes to even).
            Round => "fn uf_round(x: f32) -> f32 {
    let r = floor(abs(x) + 0.5);
    return select(r, -r, x < 0.0);
}",
            // By bits: Metal's fast-math folds every comparison test.
            IsNan => "fn uf_isnan(x: f32) -> bool {
    let b = bitcast<u32>(x);
    return (b & 0x7f800000u) == 0x7f800000u && (b & 0x007fffffu) != 0u;
}",
            IsInf => "fn uf_isinf(x: f32) -> bool {
    return (bitcast<u32>(x) & 0x7fffffffu) == 0x7f800000u;
}",
            FSq => "fn uf_fsq(x: f32) -> f32 {
    return x * x;
}",
            // |c + d| - |c| by sign case: exact where the difference
            // cancels (derived perturbation equations, `lang::delta`).
            DiffAbs => "fn uf_diffabs(c: f32, d: f32) -> f32 {
    if (c >= 0.0) {
        if (c + d >= 0.0) {
            return d;
        }
        return -(2.0 * c + d);
    }
    if (c + d > 0.0) {
        return 2.0 * c + d;
    }
    return -d;
}",
            // WGSL's pow is undefined for a negative base; a whole
            // exponent keeps it real, as the f64 interpreter's powf.
            FPow => "fn uf_powf(a: f32, b: f32) -> f32 {
    if (b == 0.0) {
        return 1.0;
    }
    if (a < 0.0 && b == floor(b) && abs(b) < 16777216.0) {
        let m = pow(-a, b);
        return select(m, -m, (i32(abs(b)) & 1) == 1);
    }
    return pow(a, b);
}",
            FPowI => "fn uf_powi(x: f32, n: i32) -> f32 {
    var base = select(x, 1.0 / x, n < 0);
    var e = abs(n);
    var acc = 1.0;
    for (var k = 0; k < 32; k = k + 1) {
        if (e == 0) {
            break;
        }
        if ((e & 1) == 1) {
            acc = acc * base;
        }
        base = base * base;
        e = e >> 1u;
    }
    return acc;
}",
            // Truncated, the sign of `a`, as WGSL defines `%` -- spelled
            // out, because the hardware operator measured otherwise on a
            // GTX 1660 under Vulkan (-5 % 3 gave 2, not -2). `-1` is
            // its own case: MIN / -1 overflows.
            IMod => "fn uf_imod(a: i32, b: i32) -> i32 {
    if (b == 0 || b == -1) {
        return 0;
    }
    return a - b * (a / b);
}",
            // One value per pixel, from its position: a hash, so a frame
            // reproduces exactly.
            Random => "fn uf_random() -> vec2<f32> {
    var h = u32(esc_env_px.x) * 1664525u + u32(esc_env_px.y) * 22695477u + 1013904223u;
    h = (h ^ (h >> 16u)) * 2246822519u;
    let a = h ^ (h >> 13u);
    let b = (a * 3266489917u) ^ (a >> 16u);
    return vec2<f32>(f32(a >> 8u), f32(b >> 8u)) / 16777216.0;
}",
            CMul => "fn uf_cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}",
            // Plain IEEE, unguarded: UF and Fractint check nothing either.
            CDiv => "fn uf_cdiv(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let d = b.x * b.x + b.y * b.y;
    return vec2<f32>(a.x * b.x + a.y * b.y, a.y * b.x - a.x * b.y) / d;
}",
            CNorm => "fn uf_cnorm(z: vec2<f32>) -> f32 {
    return z.x * z.x + z.y * z.y;
}",
            CSqr => "fn uf_csqr(z: vec2<f32>) -> vec2<f32> {
    return uf_cmul(z, z);
}",
            CPowI => "fn uf_cpowi(z: vec2<f32>, n: i32) -> vec2<f32> {
    var base = z;
    if (n < 0) {
        base = uf_cdiv(vec2<f32>(1.0, 0.0), z);
    }
    var e = abs(n);
    var acc = vec2<f32>(1.0, 0.0);
    for (var k = 0; k < 32; k = k + 1) {
        if (e == 0) {
            break;
        }
        if ((e & 1) == 1) {
            acc = uf_cmul(acc, base);
        }
        base = uf_cmul(base, base);
        e = e >> 1u;
    }
    return acc;
}",
            CPow => "fn uf_cpow(z: vec2<f32>, w: vec2<f32>) -> vec2<f32> {
    if (z.x == 0.0 && z.y == 0.0) {
        return vec2<f32>(0.0, 0.0);
    }
    return uf_cexp(uf_cmul(w, uf_clog(z)));
}",
            CRecip => "fn uf_crecip(z: vec2<f32>) -> vec2<f32> {
    return uf_cdiv(vec2<f32>(1.0, 0.0), z);
}",
            CConj => "fn uf_cconj(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(z.x, -z.y);
}",
            CFlip => "fn uf_cflip(z: vec2<f32>) -> vec2<f32> {
    return z.yx;
}",
            CAbs => "fn uf_cabsc(z: vec2<f32>) -> vec2<f32> {
    return abs(z);
}",
            CCabs => "fn uf_ccabs(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(length(z), 0.0);
}",
            CReal => "fn uf_creal(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(z.x, 0.0);
}",
            CImag => "fn uf_cimag(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(z.y, 0.0);
}",
            CIdent => "fn uf_cident(z: vec2<f32>) -> vec2<f32> {
    return z;
}",
            CZero => "fn uf_czero(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}",
            CArg => "fn uf_carg(z: vec2<f32>) -> f32 {
    return uf_atan2(z.y, z.x);
}",
            CArgC => "fn uf_cargc(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(uf_atan2(z.y, z.x), 0.0);
}",
            CIsNan => "fn uf_cisnan(z: vec2<f32>) -> bool {
    return uf_isnan(z.x) || uf_isnan(z.y);
}",
            CIsInf => "fn uf_cisinf(z: vec2<f32>) -> bool {
    return uf_isinf(z.x) || uf_isinf(z.y);
}",
            CLog => "fn uf_clog(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(log(length(z)), uf_atan2(z.y, z.x));
}",
            CExp => "fn uf_cexp(z: vec2<f32>) -> vec2<f32> {
    let y = uf_reduce(z.y);
    return exp(z.x) * vec2<f32>(cos(y), sin(y));
}",
            // The half-angle form: neither side of the cut cancels.
            CSqrt => "fn uf_csqrt(z: vec2<f32>) -> vec2<f32> {
    let r = length(z);
    if (r == 0.0) {
        return vec2<f32>(0.0, 0.0);
    }
    let t = sqrt((r + abs(z.x)) * 0.5);
    if (z.x >= 0.0) {
        return vec2<f32>(t, z.y / (2.0 * t));
    }
    let neg = (bitcast<u32>(z.y) & 0x80000000u) != 0u;
    return vec2<f32>(abs(z.y) / (2.0 * t), select(t, -t, neg));
}",
            CSin => "fn uf_csin(z: vec2<f32>) -> vec2<f32> {
    let x = uf_reduce(z.x);
    return vec2<f32>(sin(x) * cosh(z.y), cos(x) * sinh(z.y));
}",
            CCos => "fn uf_ccos(z: vec2<f32>) -> vec2<f32> {
    let x = uf_reduce(z.x);
    return vec2<f32>(cos(x) * cosh(z.y), -sin(x) * sinh(z.y));
}",
            CTan => "fn uf_ctan(z: vec2<f32>) -> vec2<f32> {
    return uf_cdiv(uf_csin(z), uf_ccos(z));
}",
            CCotan => "fn uf_ccotan(z: vec2<f32>) -> vec2<f32> {
    return uf_cdiv(uf_ccos(z), uf_csin(z));
}",
            CSinh => "fn uf_csinh(z: vec2<f32>) -> vec2<f32> {
    let y = uf_reduce(z.y);
    return vec2<f32>(sinh(z.x) * cos(y), cosh(z.x) * sin(y));
}",
            CCosh => "fn uf_ccosh(z: vec2<f32>) -> vec2<f32> {
    let y = uf_reduce(z.y);
    return vec2<f32>(cosh(z.x) * cos(y), sinh(z.x) * sin(y));
}",
            CTanh => "fn uf_ctanh(z: vec2<f32>) -> vec2<f32> {
    return uf_cdiv(uf_csinh(z), uf_ccosh(z));
}",
            CCotanh => "fn uf_ccotanh(z: vec2<f32>) -> vec2<f32> {
    return uf_cdiv(uf_ccosh(z), uf_csinh(z));
}",
            // asin z = -i log(iz + sqrt(1 - z^2))
            CAsin => "fn uf_casin(z: vec2<f32>) -> vec2<f32> {
    let w = uf_csqrt(vec2<f32>(1.0, 0.0) - uf_cmul(z, z));
    let l = uf_clog(vec2<f32>(-z.y, z.x) + w);
    return vec2<f32>(l.y, -l.x);
}",
            CAcos => "fn uf_cacos(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(1.5707963267948966, 0.0) - uf_casin(z);
}",
            // atan z = (i/2) log((i + z) / (i - z))
            CAtan => "fn uf_catan(z: vec2<f32>) -> vec2<f32> {
    let l = uf_clog(uf_cdiv(vec2<f32>(z.x, 1.0 + z.y), vec2<f32>(-z.x, 1.0 - z.y)));
    return vec2<f32>(-0.5 * l.y, 0.5 * l.x);
}",
            CAsinh => "fn uf_casinh(z: vec2<f32>) -> vec2<f32> {
    return uf_clog(z + uf_csqrt(uf_cmul(z, z) + vec2<f32>(1.0, 0.0)));
}",
            CAcosh => "fn uf_cacosh(z: vec2<f32>) -> vec2<f32> {
    let one = vec2<f32>(1.0, 0.0);
    return uf_clog(z + uf_cmul(uf_csqrt(z + one), uf_csqrt(z - one)));
}",
            CAtanh => "fn uf_catanh(z: vec2<f32>) -> vec2<f32> {
    let one = vec2<f32>(1.0, 0.0);
    return 0.5 * uf_clog(uf_cdiv(one + z, one - z));
}",
            CRound => "fn uf_cround(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(uf_round(z.x), uf_round(z.y));
}",
            CTrunc => "fn uf_ctrunc(z: vec2<f32>) -> vec2<f32> {
    return trunc(z);
}",
            CCeil => "fn uf_cceil(z: vec2<f32>) -> vec2<f32> {
    return ceil(z);
}",
            CFloor => "fn uf_cfloor(z: vec2<f32>) -> vec2<f32> {
    return floor(z);
}",
            // Fractint's cos with the imaginary part negated: its bug
            // before v16, kept so the old pictures reproduce.
            CCosxx => "fn uf_ccosxx(z: vec2<f32>) -> vec2<f32> {
    let c = uf_ccos(z);
    return vec2<f32>(c.x, -c.y);
}",
            User => return user_source(),
            UserD => return user_d_source(),
            // The identity, which WGSL does not fold: see `Gen::expr`.
            Opaque => "fn uf_k(x: f32) -> f32 {
    return x;
}
fn uf_ki(x: i32) -> i32 {
    return x;
}
fn uf_kc(x: vec2<f32>) -> vec2<f32> {
    return x;
}",
        };
        src.to_string()
    }
}

/// `uf_user(k, z)`: the user function at index `k` of
/// [`USER_FUNCTIONS`]; out of range is the identity.
fn user_source() -> String {
    let mut s = String::from("fn uf_user(k: i32, z: vec2<f32>) -> vec2<f32> {\n    switch k {\n");
    for (i, f) in USER_FUNCTIONS.iter().enumerate() {
        let _ = writeln!(s, "        case {i}: {{\n            return {}(z);\n        }}", complex_helper(*f).name());
    }
    s.push_str("        default: {\n            return z;\n        }\n    }\n}");
    s
}

/// `uf_user_d(k, z)`: the derivative of the user function at index `k`
/// ([`Builtin::deriv_c`] on the CPU): zero for a choice with none, and
/// out of range the identity's, 1.
fn user_d_source() -> String {
    use Builtin::*;
    let mut s = String::from(
        "fn uf_user_d(k: i32, z: vec2<f32>) -> vec2<f32> {\n    let one = vec2<f32>(1.0, 0.0);\n    switch k {\n",
    );
    for (i, f) in USER_FUNCTIONS.iter().enumerate() {
        let d = match f {
            Sqr => "2.0 * z",
            Sqrt => "uf_cdiv(vec2<f32>(0.5, 0.0), uf_csqrt(z))",
            Log => "uf_crecip(z)",
            Exp => "uf_cexp(z)",
            Recip => "-uf_crecip(uf_csqr(z))",
            Ident => "one",
            Sin => "uf_ccos(z)",
            Cos => "-uf_csin(z)",
            Tan => "uf_crecip(uf_csqr(uf_ccos(z)))",
            Cotan => "-uf_crecip(uf_csqr(uf_csin(z)))",
            Sinh => "uf_ccosh(z)",
            Cosh => "uf_csinh(z)",
            Tanh => "uf_crecip(uf_csqr(uf_ccosh(z)))",
            Cotanh => "-uf_crecip(uf_csqr(uf_csinh(z)))",
            Asin => "uf_crecip(uf_csqrt(one - uf_csqr(z)))",
            Acos => "-uf_crecip(uf_csqrt(one - uf_csqr(z)))",
            Atan => "uf_crecip(one + uf_csqr(z))",
            Asinh => "uf_crecip(uf_csqrt(uf_csqr(z) + one))",
            Acosh => "uf_crecip(uf_cmul(uf_csqrt(z - one), uf_csqrt(z + one)))",
            Atanh => "uf_crecip(one - uf_csqr(z))",
            _ => "vec2<f32>(0.0, 0.0)",
        };
        let _ = writeln!(s, "        case {i}: {{\n            return {d};\n        }}");
    }
    s.push_str("        default: {\n            return one;\n        }\n    }\n}");
    s
}

/// What the templates provide, enough to validate a program alone.
const HARNESS: &str = "
struct OrbitSummary {
    z: vec2<f32>,
    n: u32,
    escaped: bool,
    converged: bool,
    period: u32,
    dz: vec2<f32>,
}
struct HarnessParams {
    max_iter: u32,
    flags: u32,
    rot_cs: vec2<f32>,
    julia_c: vec2<f32>,
}
@group(0) @binding(0) var<uniform> params: HarnessParams;
fn fparam(i: u32) -> f32 {
    return f32(i);
}
fn cparam(i: u32) -> f32 {
    return f32(i);
}
fn fdata4(i: u32) -> vec4<f32> {
    return vec4<f32>(f32(i));
}
";


/// naga's verdict on a whole module.
#[cfg(not(target_arch = "wasm32"))]
fn validate_module(src: &str) -> Result<(), String> {
    use egui_wgpu::wgpu::naga;
    let module = naga::front::wgsl::parse_str(src).map_err(|e| e.emit_to_string(src))?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module)
        .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

/// Validate a compiled formula against the template's interface. A
/// failure is a compiler bug, reported as a message rather than as a
/// GPU validation error. Desktop only: the web build has no naga, and
/// the browser validates.
#[cfg(not(target_arch = "wasm32"))]
pub fn validate_formula(wgsl: &str) -> Result<(), String> {
    validate_module(&format!(
        "{HARNESS}{ENV_DECLS}{wgsl}\n@compute @workgroup_size(1)\nfn main() {{\n    \
         var s = formula_init(vec2<f32>(0.0, 0.0));\n    s = formula_loop(s, vec2<f32>(0.0, 0.0));\n    \
         let go = formula_bailout(s, vec2<f32>(0.0, 0.0));\n}}\n"
    ))
}

/// [`validate_formula`] for a colouring.
#[cfg(not(target_arch = "wasm32"))]
pub fn validate_coloring(c: &ColoringWgsl) -> Result<(), String> {
    validate_module(&format!(
        "{HARNESS}{ENV_DECLS}{}\n@compute @workgroup_size(1)\nfn main() {{\n    var st = {};\n    \
         st = coloring_accum(vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 0.0), st);\n    \
         let v = coloring_map(OrbitSummary(vec2<f32>(1.0, 1.0), 3u, true, false, 0u, vec2<f32>(1.0, 1.0)), st);\n}}\n",
        c.wgsl, c.accum_init
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::check::check;
    use super::super::parser::parse_file;
    use super::super::{Dialect, EntryKind};
    use super::*;

    pub(crate) fn validate(src: &str) -> Result<(), String> {
        super::validate_module(src)?;
        let lint = crate::variations::shader_lint::self_operations(src);
        if !lint.is_empty() {
            return Err(format!("fast-math self-operations: {lint:?}"));
        }
        let sub = crate::variations::shader_lint::subnormal_literals(src);
        if !sub.is_empty() {
            return Err(format!("subnormal literals: {sub:?}"));
        }
        Ok(())
    }

    fn program(src: &str, kind: EntryKind) -> Program {
        let (file, d) = parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = check(&file.entries[0], kind, Dialect::UltraFractal);
        assert!(!super::super::has_errors(&d), "{d:?}");
        p.unwrap()
    }

    fn formula_module(p: &Program) -> String {
        format!(
            "{HARNESS}{ENV_DECLS}{}\n@compute @workgroup_size(1)\nfn main() {{\n    var s = formula_init(vec2<f32>(0.1, 0.2));\n    \
             s = formula_loop(s, vec2<f32>(0.1, 0.2));\n    let go = formula_bailout(s, vec2<f32>(0.1, 0.2));\n}}\n",
            formula(p)
        )
    }

    fn coloring_module(p: &Program) -> String {
        let c = coloring(p);
        format!(
            "{HARNESS}{ENV_DECLS}{}\n@compute @workgroup_size(1)\nfn main() {{\n    var st = {};\n    \
             st = coloring_accum(vec2<f32>(0.1, 0.2), vec2<f32>(0.0), vec2<f32>(0.1, 0.2), st);\n    \
             let v = coloring_map(OrbitSummary(vec2<f32>(1.0), 3u, true, false, 0u, vec2<f32>(1.0)), st);\n}}\n",
            c.wgsl, c.accum_init
        )
    }

    /// Every built-in, every operator and type, every statement: the
    /// generated WGSL parses, validates and passes the fast-math lint.
    #[test]
    fn everything_the_language_has_compiles_to_valid_wgsl() {
        let mut calls = String::new();
        for (name, _) in super::super::builtins::BUILTINS {
            let _ = writeln!(calls, "  c = c + {name}(z)");
            if !matches!(*name, "conj" | "flip" | "cosxx" | "isnan" | "isinf") {
                let _ = writeln!(calls, "  f = f + real({name}(f))");
            }
        }
        let src = format!(
            "All {{\ninit:\n  complex c = (0.5, -0.25)\n  float f = 1.5\n  int i = 3\n  bool b = true\n  z = #pixel\nloop:\n{calls}\
             \n  i = i % 2 + round(f) - 1\n  f = f ^ 2.5 + 2^0.5 + (-f)^3 + |c| + |f| + #pi * #e\n  c = c ^ (1, 1) + c ^ 3 + c^-2 + c / z + @p\n\
             \n  b = (c == z) || (c < z) && !b || (f >= 1) || isnan(c) || isinf(f)\n  if b\n   z = z * 2\n  elseif i > 2\n   z = @fn(z)\n  else\n   z = fn1(z)\n  endif\n\
             \n  while i < 5\n   i = i + 1\n  endwhile\n  repeat\n   i = i - 1\n  until i <= 0\n  z = z + c + f + #random + #center * #magn + #screenpixel\
             \nbailout:\n  |z| < 1e10 && #x >= 0 && #width > 0 && #maxiter > 0\n}}\n"
        );
        let p = program(&src, EntryKind::Formula);
        let module = formula_module(&p);
        if let Err(e) = validate(&module) {
            panic!("{e}\n---\n{module}");
        }
    }

    #[test]
    fn a_colouring_compiles_with_its_state_packed() {
        let p = program(
            "Avg {\ninit:\n sum = 0\n int count = 0\n w = (0, 0)\nloop:\n sum = sum + cabs(#z)\n count = count + 1\n w = #z\n\
             final:\n #index = real(sum) / count + cabs(w) + #numiter / #maxiter\n if count > 3\n  #solid = true\n endif\n}",
            EntryKind::Coloring,
        );
        // sum (real: 1), count (1), w (2).
        assert_eq!(p.state_floats(), 4, "{:?}", p.state.iter().map(|&v| &p.vars[v].name).collect::<Vec<_>>());
        let module = coloring_module(&p);
        if let Err(e) = validate(&module) {
            panic!("{e}\n---\n{module}");
        }
        let c = coloring(&p);
        assert!(c.needs_accum);
        assert!(c.wgsl.contains("ufc_"), "helpers carry the colouring's prefix");
        assert!(!c.wgsl.contains("uf_"), "no unprefixed helper is left");
    }

    /// Run a formula's init and one loop on the GPU for each input point,
    /// `sel` as every parameter slot's value; returns the iterates.
    fn gpu_one_step(p: &Program, inputs: &[(f32, f32, f32)]) -> Option<Vec<(f32, f32)>> {
        let out = gpu_run(&formula(p), "vec4<f32>(s.z, 0.0, 0.0)", inputs)?;
        Some(out.iter().map(|v| (v[0], v[1])).collect())
    }

    /// [`gpu_one_step`] for a formula carrying its derivative: the
    /// iterates and their derivatives.
    fn gpu_one_step_dz(p: &Program, inputs: &[(f32, f32, f32)]) -> Option<Vec<((f32, f32), (f32, f32))>> {
        let wgsl = formula_with_derivative(p).expect("differentiable");
        let out = gpu_run(&wgsl, "vec4<f32>(s.z, formula_dz(s))", inputs)?;
        Some(out.iter().map(|v| ((v[0], v[1]), (v[2], v[3]))).collect())
    }

    /// The probe: `wgsl`'s init and one loop per input, `out` (of `s`)
    /// written back.
    fn gpu_run(wgsl: &str, out: &str, inputs: &[(f32, f32, f32)]) -> Option<Vec<[f32; 4]>> {
        use egui_wgpu::wgpu;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        let src = format!(
            "@group(0) @binding(0) var<storage, read_write> io: array<vec4<f32>>;\n\
             var<private> sel: f32;\nfn fparam(i: u32) -> f32 {{\n    return sel;\n}}\n{ENV_DECLS}{}\n\
             @compute @workgroup_size(64)\nfn main(@builtin(global_invocation_id) g: vec3<u32>) {{\n    \
             if (g.x >= arrayLength(&io)) {{\n        return;\n    }}\n    let v = io[g.x];\n    sel = v.z;\n    \
             var s = formula_init(v.xy);\n    s = formula_loop(s, v.xy);\n    io[g.x] = {out};\n}}\n",
            wgsl
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lang probe"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("lang probe"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let host: Vec<[f32; 4]> = inputs.iter().map(|&(x, y, s)| [x, y, s, 0.0]).collect();
        let bytes: &[u8] = bytemuck::cast_slice(&host);
        let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
        let storage = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: bytes.len() as u64, usage, mapped_at_creation: false });
        queue.write_buffer(&storage, 0, bytes);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: storage.as_entire_binding() }],
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups((inputs.len() as u32).div_ceil(64), 1, 1);
        }
        enc.copy_buffer_to_buffer(&storage, 0, &readback, 0, bytes.len() as u64);
        queue.submit(std::iter::once(enc.finish()));
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        let view = slice.get_mapped_range();
        let got: &[[f32; 4]] = bytemuck::cast_slice(&view);
        Some(got.to_vec())
    }

    /// Off every branch cut and every .5, so f32 and f64 agree on which
    /// side they are.
    const POINTS: &[(f64, f64)] = &[(0.3, -0.7), (1.4, 0.2), (-1.7, 0.6), (0.1, 1.3), (-0.45, -0.35), (2.2, -1.9), (0.8, 0.05)];

    fn close(gpu: (f32, f32), cpu: super::super::builtins::C64) -> bool {
        let scale = cpu.abs().max(1.0);
        ((gpu.0 as f64 - cpu.re).abs() / scale) < 2e-5 && ((gpu.1 as f64 - cpu.im).abs() / scale) < 2e-5
    }

    /// Every user-selectable function's WGSL against its f64 definition.
    #[test]
    #[ignore = "needs a GPU"]
    fn every_complex_function_matches_f64_on_the_gpu() {
        let p = program("U {\ninit:\n z = #pixel\nloop:\n z = @f(z)\nbailout:\n false\n}", EntryKind::Formula);
        let mut inputs = Vec::new();
        for (k, _) in USER_FUNCTIONS.iter().enumerate() {
            for &(x, y) in POINTS {
                inputs.push((x as f32, y as f32, k as f32));
            }
        }
        let Some(out) = gpu_one_step(&p, &inputs) else {
            eprintln!("no GPU");
            return;
        };
        let mut bad = Vec::new();
        for ((x, y, k), got) in inputs.iter().zip(&out) {
            let f = USER_FUNCTIONS[*k as usize];
            let want = f.eval_c(super::super::builtins::C64::new(*x as f64, *y as f64));
            if !close(*got, want) {
                bad.push(format!("{f:?}({x}, {y}): gpu {got:?}, f64 ({}, {})", want.re, want.im));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// The operators: integer and complex powers, division, real powers
    /// of a negative base, mixed types, comparisons.
    #[test]
    #[ignore = "needs a GPU"]
    fn operators_match_the_interpreter_on_the_gpu() {
        let p = program(
            "O {\ninit:\n z = #pixel\nloop:\n float x = real(z) * 3\n int i = round(x)\n\
             z = z^3 - z^(0.5, 0.25) + z^-2 + z / (1.5, -0.5) + (-x)^3 + |x|^2.5 + (-2.0)^i + i % 3 + |z| * (z < 1)\n\
             bailout:\n false\n}",
            EntryKind::Formula,
        );
        let inputs: Vec<(f32, f32, f32)> = POINTS.iter().map(|&(x, y)| (x as f32, y as f32, 0.0)).collect();
        let Some(out) = gpu_one_step(&p, &inputs) else {
            eprintln!("no GPU");
            return;
        };
        let params = super::super::interp::ParamValues::defaults(&p);
        for (&(x, y), got) in POINTS.iter().zip(&out) {
            let pixel = super::super::builtins::C64::new(x as f32 as f64, y as f32 as f64);
            let want = super::super::interp::run(&p, &params, None, &super::super::interp::Env::at(pixel, 1)).z;
            assert!(close(*got, want), "({x}, {y}): gpu {got:?}, f64 ({}, {})", want.re, want.im);
        }
    }

    /// Every user function's derivative (`uf_user_d`) against its f64
    /// one: `z = @f(#pixel)` carries `dz = f'(pixel)`.
    #[test]
    #[ignore = "needs a GPU"]
    fn every_user_derivative_matches_f64_on_the_gpu() {
        let p = program("U {\ninit:\n z = #pixel\nloop:\n z = @f(z)\nbailout:\n false\n}", EntryKind::Formula);
        let mut inputs = Vec::new();
        for (k, _) in USER_FUNCTIONS.iter().enumerate() {
            for &(x, y) in POINTS {
                inputs.push((x as f32, y as f32, k as f32));
            }
        }
        let Some(out) = gpu_one_step_dz(&p, &inputs) else {
            eprintln!("no GPU");
            return;
        };
        let mut bad = Vec::new();
        for ((x, y, k), (_, got)) in inputs.iter().zip(&out) {
            let f = USER_FUNCTIONS[*k as usize];
            let want = f.deriv_c(super::super::builtins::C64::new(*x as f64, *y as f64));
            if !close(*got, want) {
                bad.push(format!("{f:?}'({x}, {y}): gpu {got:?}, f64 ({}, {})", want.re, want.im));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// A derived program's derivative on the GPU against the
    /// interpreter's: the rules, emitted.
    #[test]
    #[ignore = "needs a GPU"]
    fn derivatives_match_the_interpreter_on_the_gpu() {
        let p = program(
            "D {\ninit:\n z = #pixel\n w = 0.5 * z\nloop:\n w = sin(w) / (z + 3) + exp(z) * 0.1\n\
             z = z^3 * 0.2 + w - sqrt(z + 2) + log(z + 4) + (z + 3)^(0.5, 0.1) + 2^z * 0.05 + recip(z + 5) - z / (z^2 + 7)\n\
             bailout:\n false\n}",
            EntryKind::Formula,
        );
        let inputs: Vec<(f32, f32, f32)> = POINTS.iter().map(|&(x, y)| (x as f32, y as f32, 0.0)).collect();
        let Some(out) = gpu_one_step_dz(&p, &inputs) else {
            eprintln!("no GPU");
            return;
        };
        let d = super::super::derive::with_derivative(&p).unwrap();
        let params = super::super::interp::ParamValues::defaults(&d);
        for (&(x, y), (z, dz)) in POINTS.iter().zip(&out) {
            let pixel = super::super::builtins::C64::new(x as f32 as f64, y as f32 as f64);
            let want = super::super::interp::run(&d, &params, None, &super::super::interp::Env::at(pixel, 1));
            assert!(close(*z, want.z), "({x}, {y}): gpu z {z:?}, f64 {:?}", want.z);
            assert!(close(*dz, want.dz), "({x}, {y}): gpu dz {dz:?}, f64 {:?}", want.dz);
        }
    }

    /// A Fractint program using every name of its own compiles to valid
    /// WGSL, in both kinds of context.
    #[test]
    fn fractint_names_compile_to_valid_wgsl() {
        let (file, d) = parse_file(
            "F { z = pixel + whitesq + scrnpix + scrnmax + magxmag + rotskew + center, c = p1, p1 = p1 * 2 : \
             if (ismand) \n z = sqr(z) + pixel \n else \n z = sqr(z) + c \n endif, z = z + round(z) * 0, LastSqr <= 4 && |z| >= 0 }",
        );
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = check(&file.entries[0], EntryKind::Formula, Dialect::Fractint);
        assert!(!super::super::has_errors(&d), "{d:?}");
        let module = formula_module(&p.unwrap());
        if let Err(e) = validate(&module) {
            panic!("{e}\n---\n{module}");
        }
    }

    /// Globals of every type compile in both kinds of entry: read from
    /// the uniform, kept out of the per-pixel state.
    #[test]
    fn globals_compile_to_valid_wgsl() {
        let f = program(
            "G {\nglobal:\n complex c = @a * 2\n float k = #magn\n int n = #maxiter - 2\n bool b = k > 1\n\
             init:\n z = 0\nloop:\n z = sqr(z) + #pixel * k + c\nbailout:\n |z| < n && b\n}",
            EntryKind::Formula,
        );
        let module = formula_module(&f);
        if let Err(e) = validate(&module) {
            panic!("{e}\n---\n{module}");
        }
        let state = &module[module.find("struct FormulaState").unwrap()..];
        let state = &state[..state.find('}').unwrap()];
        assert_eq!(state.matches(':').count(), 1, "only z is per pixel: {state}");
        assert!(module.contains("fdata4(0u).x") && module.contains("fdata4(0u).w"), "{module}");

        let c = program(
            "C {\nglobal:\n float k = @scale * 2\n complex w = (1, 2)\ninit:\n float s = 0\nloop:\n s = s + k\n\
             final:\n #index = s + real(w)\ndefault:\n float param scale\n  default = 1\n endparam\n}",
            EntryKind::Coloring,
        );
        let module = coloring_module(&c);
        if let Err(e) = validate(&module) {
            panic!("{e}\n---\n{module}");
        }
        // After the one parameter: slots 1, 2 and 3.
        assert!(module.contains("cparam(1u)") && module.contains("cparam(3u)"), "{module}");
    }

    /// Each built-in called alone brings every helper it needs (the
    /// user-function switch brings them all, which hid `acos` needing
    /// `asin`, found by compiling UF's public formulas).
    #[test]
    fn every_builtin_alone_brings_its_helpers() {
        for (name, b) in super::super::builtins::BUILTINS {
            // The tests are bools: used as one.
            let body = if matches!(b, Builtin::IsInf | Builtin::IsNan) {
                format!("if {name}(z)\n  z = 0\n endif")
            } else {
                format!("z = {name}(z) + #pixel")
            };
            let src = format!("B {{\ninit:\n z = #pixel\nloop:\n {body}\nbailout:\n |z| < 4\n}}");
            let module = formula_module(&program(&src, EntryKind::Formula));
            if let Err(e) = validate(&module) {
                panic!("{name}: {e}");
            }
        }
    }

    /// WGSL refuses a constant expression whose value is not finite,
    /// where UF computes NaN or infinity: a constant argument out of a
    /// function's domain, a division by zero, an overflow. Found
    /// compiling UF's public formulas (`sqrt(-3)` in a colouring).
    #[test]
    fn constants_out_of_range_still_compile() {
        for e in [
            "sqrt(-3)", "log(0)", "log(-1)", "asin(2)", "acos(2)", "acosh(0.5)", "atanh(1)", "atanh(2)",
            "1 / 0", "1.0 / 0.0", "0.0 / 0.0", "1e30 * 1e30", "-1e30 * 1e30", "exp(1000)", "sinh(1000)", "cosh(1000)",
            "tan(1.5707963267948966)", "1e38 + 1e38", "(-8)^(1/3)", "0^-1", "recip(0)", "cotan(0)",
        ] {
            let src = format!("C {{\ninit:\n float x = {e}\nloop:\n z = z + x + #pixel\nbailout:\n |z| < 4\n}}");
            let module = formula_module(&program(&src, EntryKind::Formula));
            if let Err(err) = validate(&module) {
                panic!("{e}: {err}");
            }
        }
    }

    /// The test files (`tests/formulas/`): name, text, kind, dialect.
    const TEST_FILES: &[(&str, &str, EntryKind, Dialect)] = &[
        ("language.ufm", include_str!("../../../tests/formulas/language.ufm"), EntryKind::Formula, Dialect::UltraFractal),
        ("language.ucl", include_str!("../../../tests/formulas/language.ucl"), EntryKind::Coloring, Dialect::UltraFractal),
        ("fractint.frm", include_str!("../../../tests/formulas/fractint.frm"), EntryKind::Formula, Dialect::Fractint),
    ];

    /// The Rhai test formulas, one to a file (`lang::rhai`).
    const RHAI_TEST_FILES: &[(&str, &str)] = &[
        ("rhai/mandelbrot.rhai", include_str!("../../../tests/formulas/rhai/mandelbrot.rhai")),
        ("rhai/constructs.rhai", include_str!("../../../tests/formulas/rhai/constructs.rhai")),
        ("rhai/plane.rhai", include_str!("../../../tests/formulas/rhai/plane.rhai")),
        ("rhai/cycle.rhai", include_str!("../../../tests/formulas/rhai/cycle.rhai")),
        ("rhai/functions.rhai", include_str!("../../../tests/formulas/rhai/functions.rhai")),
    ];

    /// Every entry of the test files, checked as the registry checks it
    /// (a formula with its `switch:` target): (file, entry, program).
    fn test_file_entries() -> Vec<(&'static str, String, Program)> {
        let mut out = Vec::new();
        let mut failures = Vec::new();
        for &(file, text, kind, dialect) in TEST_FILES {
            let (parsed, d) = parse_file(text);
            assert!(!super::super::has_errors(&d), "{file}: {:?}", d.iter().map(|x| x.render(text)).collect::<Vec<_>>());
            for e in &parsed.entries {
                let (p, d) = match kind {
                    EntryKind::Formula => super::super::switch::check_formula(&parsed, e, dialect),
                    EntryKind::Coloring => check(e, kind, dialect),
                };
                let errors: Vec<String> = d.iter().filter(|x| x.severity == super::super::Severity::Error).map(|x| x.render(text)).collect();
                match p {
                    Some(p) if errors.is_empty() => out.push((file, e.name.clone(), p)),
                    _ => failures.push(format!("{file} / {}: {errors:?}", e.name)),
                }
            }
        }
        for &(file, text) in RHAI_TEST_FILES {
            match super::super::rhai::program(text) {
                Ok(p) => out.push((file, p.name.clone(), p)),
                Err(d) => failures.push(format!("{file}: {:?}", d.iter().map(|x| x.render(text)).collect::<Vec<_>>())),
            }
        }
        // Every formula again, converted to Rhai and compiled back
        // (`lang::to_rhai`): what the editor's Edit as Rhai gives.
        let converted: Vec<_> = out
            .iter()
            .filter(|(file, _, p)| p.kind == EntryKind::Formula && !file.ends_with(".rhai"))
            .map(|(file, name, p)| {
                let text = super::super::to_rhai::formula(p, &[]).unwrap_or_else(|e| panic!("{file} / {name}: {e}"));
                let back = super::super::rhai::program(&text).unwrap_or_else(|d| panic!("{file} / {name} as Rhai: {:?}", d[0].render(&text)));
                (*file, format!("{name} as Rhai"), back)
            })
            .collect();
        out.extend(converted);
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        out
    }

    /// Every entry of the test files compiles, to WGSL that validates:
    /// a formula's derivative too where it has one.
    #[test]
    fn every_test_file_entry_compiles_to_valid_wgsl() {
        let entries = test_file_entries();
        assert!(entries.len() >= 24, "{} entries", entries.len());
        for (file, name, p) in &entries {
            let checked = match p.kind {
                EntryKind::Formula => validate_formula(&formula(p)).and_then(|_| match formula_with_derivative(p) {
                    Some(d) => validate_formula(&d),
                    None => Ok(()),
                }),
                EntryKind::Coloring => validate_coloring(&coloring(p)),
            };
            if let Err(e) = checked {
                panic!("{file} / {name}: {e}");
            }
        }
        // What the files are for: the switch pair, the Julia toggle from
        // `ismand`, globals, directives and a derivative all present.
        let find = |n: &str| &entries.iter().find(|(_, m, _)| m == n).unwrap_or_else(|| panic!("{n}")).2;
        assert!(find("SwitchM").reads(Predef::IsMand) && find("FrScreen").reads(Predef::IsMand));
        assert!(!find("Globals").globals.is_empty() && !find("GlobalScale").globals.is_empty());
        assert!(formula_with_derivative(find("Control")).is_some());
        assert!(coloring(find("Extended")).ext_vec4s > 0, "state past the accumulator's four floats");
        assert!(formula_with_derivative(find("Functions")).is_none(), "it reads abs and real");
    }

    /// A parameter block as the GPU holds it: the defaults, in slot
    /// order, then the user functions' choices.
    fn default_slots(p: &Program) -> Vec<f32> {
        let mut out = Vec::new();
        for param in &p.params {
            out.push(param.default.0 as f32);
            if param.ty() == Ty::Complex {
                out.push(param.default.1 as f32);
            }
        }
        out.extend(p.funcs.iter().map(|f| f.default as f32));
        out
    }

    /// The probe for whole programs: a formula, with a colouring when
    /// given, over `io` (two vec4 per point, the point in the first) and
    /// `blocks` (fparams, cparams, then fdata, as the uniform holds them).
    fn gpu_program_probe(body: &str, wgsl: &[&str], points: &[(f64, f64)], blocks: &[f32], max_iter: u32) -> Option<Vec<[f32; 8]>> {
        use egui_wgpu::wgpu;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        let src = format!(
            "@group(0) @binding(0) var<storage, read_write> io: array<vec4<f32>>;\n\
             @group(0) @binding(1) var<storage, read> blocks: array<f32>;\n\
             struct OrbitSummary {{\n    z: vec2<f32>,\n    n: u32,\n    escaped: bool,\n    converged: bool,\n    period: u32,\n    dz: vec2<f32>,\n}}\n\
             struct ProbeParams {{\n    max_iter: u32,\n    flags: u32,\n    rot_cs: vec2<f32>,\n    julia_c: vec2<f32>,\n}}\n\
             var<private> params: ProbeParams;\n\
             fn fparam(i: u32) -> f32 {{\n    return blocks[i];\n}}\n\
             fn cparam(i: u32) -> f32 {{\n    return blocks[64u + i];\n}}\n\
             fn fdata4(i: u32) -> vec4<f32> {{\n    let k = 128u + 4u * i;\n    return vec4<f32>(blocks[k], blocks[k + 1u], blocks[k + 2u], blocks[k + 3u]);\n}}\n\
             {ENV_DECLS}{}\n\
             @compute @workgroup_size(64)\nfn main(@builtin(global_invocation_id) g: vec3<u32>) {{\n\
             \x20   if (2u * g.x >= arrayLength(&io)) {{\n        return;\n    }}\n\
             \x20   let px = io[2u * g.x].xy;\n\
             \x20   _ = blocks[0];\n\
             \x20   params = ProbeParams({max_iter}u, 0u, vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 0.0));\n\
             \x20   esc_env_px = vec2<i32>(0, 0);\n    esc_env_size = vec2<i32>(1, 1);\n    esc_env_pixel = px;\n\
             \x20   esc_env_center = vec2<f32>(0.0, 0.0);\n    esc_env_magn = 1.0;\n    esc_env_solid = false;\n\
             {body}\n}}\n",
            wgsl.join("\n")
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("program probe"), source: wgpu::ShaderSource::Wgsl(src.into()) });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("program probe"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let host: Vec<[f32; 4]> = points.iter().flat_map(|&(x, y)| [[x as f32, y as f32, 0.0, 0.0], [0.0; 4]]).collect();
        let bytes: &[u8] = bytemuck::cast_slice(&host);
        let mut block = vec![0.0f32; 128 + 256];
        block[..blocks.len()].copy_from_slice(blocks);
        let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
        let storage = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: bytes.len() as u64, usage, mapped_at_creation: false });
        queue.write_buffer(&storage, 0, bytes);
        let block_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (block.len() * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&block_buf, 0, bytemuck::cast_slice(&block));
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: storage.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: block_buf.as_entire_binding() },
            ],
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups((points.len() as u32).div_ceil(64), 1, 1);
        }
        enc.copy_buffer_to_buffer(&storage, 0, &readback, 0, bytes.len() as u64);
        queue.submit(std::iter::once(enc.finish()));
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        let view = slice.get_mapped_range();
        let got: &[[f32; 8]] = bytemuck::cast_slice(&view);
        Some(got.to_vec())
    }

    /// Close enough for f32 against f64 over a few iterations.
    fn near(gpu: f32, cpu: f64, tol: f64) -> bool {
        (gpu as f64 - cpu).abs() <= tol * cpu.abs().max(1.0)
    }

    /// Every hybrid preset compiles to valid WGSL, its pass index and
    /// its phase comparison included.
    #[test]
    fn hybrids_compile_to_valid_wgsl() {
        for (name, h) in super::super::hybrid::presets() {
            let p = h.program().unwrap_or_else(|d| panic!("{name}: {d:?}"));
            let w = formula(&p);
            validate_formula(&w).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(w.contains("npass: i32"), h.groups.len() > 1, "{name}");
        }
    }

    /// The hybrid presets on the GPU iterate as the interpreter does:
    /// past the prefix and through several rounds of their groups.
    #[test]
    #[ignore = "needs a GPU"]
    fn hybrids_run_on_the_gpu_as_the_interpreter_does() {
        use super::super::builtins::C64;
        use super::super::interp::{run, Env, ParamValues};
        const STEPS: u32 = 7;
        let points = [(0.3, -0.2), (-0.4, 0.1), (0.1, 0.45), (-0.25, -0.3), (0.05, 0.02), (-0.6, 0.3)];
        let mut compared = 0;
        for (name, h) in super::super::hybrid::presets() {
            let p = h.program().unwrap();
            let blocks = default_slots(&p);
            let body = format!(
                "    var s = formula_init(px);\n    for (var i = 0u; i < {STEPS}u; i = i + 1u) {{\n        s = formula_loop(s, px);\n    }}\n\
                 \x20   io[2u * g.x] = vec4<f32>(s.z, 0.0, 0.0);"
            );
            let Some(out) = gpu_program_probe(&body, &[&formula(&p)], &points, &blocks, STEPS) else {
                eprintln!("no GPU");
                return;
            };
            for (&(x, y), got) in points.iter().zip(&out) {
                let px = C64::new(x as f32 as f64, y as f32 as f64);
                let want = run(&p, &ParamValues::defaults(&p), None, &Env::at(px, STEPS));
                if want.escaped && want.n < STEPS {
                    continue;
                }
                compared += 1;
                assert!(near(got[0], want.z.re, 1e-4) && near(got[1], want.z.im, 1e-4), "{name} at ({x}, {y}): gpu ({}, {}), f64 {:?}", got[0], got[1], want.z);
            }
        }
        assert!(compared > 30, "{compared}");
    }

    /// Every test file entry runs on the GPU as the f64 interpreter runs
    /// it: a formula's iterate, derivative and test after a few
    /// iterations; a colouring's `#index`, `#solid` and count beside a
    /// Mandelbrot, in UF's order (formula loop, colouring loop, test).
    #[test]
    #[ignore = "needs a GPU"]
    fn test_files_run_on_the_gpu_as_the_interpreter_does() {
        use super::super::builtins::C64;
        use super::super::interp::{globals, run, Env, ParamValues};
        let entries = test_file_entries();
        let mut bad = Vec::new();
        let (mut compared, mut skipped) = (0usize, 0usize);
        // Formulas: two iterations at small points.
        const STEPS: u32 = 2;
        let points = [(0.3, -0.2), (-0.4, 0.1), (0.1, 0.45), (-0.25, -0.3), (0.05, 0.02)];
        for (file, name, p) in entries.iter().filter(|(_, _, p)| p.kind == EntryKind::Formula) {
            let params = ParamValues::defaults(p);
            let mut blocks = default_slots(p);
            blocks.resize(128, 0.0);
            blocks.extend(globals(p, &params, &Env::at(C64::ZERO, STEPS)));
            let derived = formula_with_derivative(p);
            let wgsl = derived.clone().unwrap_or_else(|| formula(p));
            let dz = if derived.is_some() { "formula_dz(s)" } else { "vec2<f32>(0.0, 0.0)" };
            let body = format!(
                "    var s = formula_init(px);\n    for (var i = 0u; i < {STEPS}u; i = i + 1u) {{\n        s = formula_loop(s, px);\n    }}\n\
                 \x20   let go = formula_bailout(s, px);\n    io[2u * g.x] = vec4<f32>(s.z, select(0.0, 1.0, go), 0.0);\n\
                 \x20   io[2u * g.x + 1u] = vec4<f32>({dz}, 0.0, 0.0);"
            );
            let Some(out) = gpu_program_probe(&body, &[&wgsl], &points, &blocks, STEPS) else {
                eprintln!("no GPU");
                return;
            };
            let program = super::super::derive::with_derivative(p).unwrap_or_else(|| p.clone());
            for (&(x, y), got) in points.iter().zip(&out) {
                let px = C64::new(x as f32 as f64, y as f32 as f64);
                let want = run(&program, &ParamValues::defaults(&program), None, &Env::at(px, STEPS));
                // A point that left before the last step is not compared.
                if want.escaped && want.n < STEPS {
                    skipped += 1;
                    continue;
                }
                compared += 1;
                let go = !want.escaped;
                let ok = near(got[0], want.z.re, 1e-4)
                    && near(got[1], want.z.im, 1e-4)
                    && (got[2] == 1.0) == go
                    && near(got[4], want.dz.re, 1e-3)
                    && near(got[5], want.dz.im, 1e-3);
                if !ok {
                    bad.push(format!("{file} / {name} at ({x}, {y}): gpu z ({}, {}) go {} dz ({}, {}); f64 z {:?} go {go} dz {:?}", got[0], got[1], got[2], got[4], got[5], want.z, want.dz));
                }
            }
        }
        // Colourings: beside the Mandelbrot, inside and outside points.
        let (mfile, _) = parse_file("M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}");
        let (m, _) = check(&mfile.entries[0], EntryKind::Formula, Dialect::UltraFractal);
        let m = m.unwrap();
        const MAX: u32 = 40;
        let points = [(-0.2, 0.1), (0.1, 0.1), (-1.0, 0.05), (1.0, 1.0), (-2.1, 0.1), (-0.5, 1.2), (0.6, 0.6)];
        for (file, name, c) in entries.iter().filter(|(_, _, p)| p.kind == EntryKind::Coloring) {
            let cparams = ParamValues::defaults(c);
            let mut blocks = vec![0.0f32; 64];
            let mut cblock = default_slots(c);
            cblock.extend(globals(c, &cparams, &Env::at(C64::ZERO, MAX)));
            cblock.resize(64, 0.0);
            blocks.extend(cblock);
            let cw = coloring(c);
            let body = "    var s = formula_init(px);\n    var z = s.z;\n    var st = ufc_init();\n    var n = 0u;\n    var escaped = false;\n\
                        \x20   for (var i = 0u; i < params.max_iter; i = i + 1u) {\n        let zp = z;\n        s = formula_loop(s, px);\n        z = s.z;\n\
                        \x20       st = coloring_accum(z, zp, px, st);\n        if (!formula_bailout(s, px)) {\n            escaped = true;\n            n = i + 1u;\n            break;\n        }\n    }\n\
                        \x20   if (!escaped) {\n        n = params.max_iter;\n    }\n\
                        \x20   let index = coloring_map(OrbitSummary(z, n, escaped, false, 0u, vec2<f32>(0.0, 0.0)), st);\n\
                        \x20   io[2u * g.x] = vec4<f32>(index, select(0.0, 1.0, esc_env_solid), f32(n), select(0.0, 1.0, escaped));";
            let Some(out) = gpu_program_probe(body, &[&formula(&m), &cw.wgsl], &points, &blocks, MAX) else {
                eprintln!("no GPU");
                return;
            };
            for (&(x, y), got) in points.iter().zip(&out) {
                let px = C64::new(x as f32 as f64, y as f32 as f64);
                let want = run(&m, &ParamValues::defaults(&m), Some((c, &cparams)), &Env::at(px, MAX));
                let solid = got[1] == 1.0;
                let ok = solid == want.solid
                    && got[2] as u32 == want.n
                    && (got[3] == 1.0) == want.escaped
                    && (solid || near(got[0], want.index, 1e-3));
                if !ok {
                    bad.push(format!("{file} / {name} at ({x}, {y}): gpu index {} solid {solid} n {}; f64 index {} solid {} n {}", got[0], got[2], want.index, want.solid, want.n));
                }
                compared += 1;
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
        println!("compared {compared} (entry, point) pairs; skipped {skipped} that left early");
        assert!(skipped * 10 < compared, "too few points compared: {compared}, {skipped} skipped");
    }

    /// A formula's perturbation equations compile beside it: the start,
    /// the step and the test on the full z, valid WGSL; a test that
    /// reads another variable leaves the last out.
    #[test]
    fn perturbation_equations_compile_to_valid_wgsl() {
        let p = program(
            "P {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < @bailout\nperturbinit:\n #dz = 0\n\
             perturbloop:\n t = 2 * #z\n #dz = t * #dz + sqr(#dz) + #dpixel\n}",
            EntryKind::Formula,
        );
        let module = formula_module(&p);
        if let Err(e) = validate(&module) {
            panic!("{e}\n---\n{module}");
        }
        for f in ["fn formula_perturb_init_scaled(", "fn formula_perturb_loop_scaled(", "fn formula_perturb_bailout("] {
            assert!(module.contains(f), "{f}");
        }
        let q = program(
            "Q {\ninit:\n z = 0\nloop:\n w = z\n z = sqr(z) + #pixel\nbailout:\n |z - w| < 4\nperturbloop:\n #dz = #dpixel\n}",
            EntryKind::Formula,
        );
        let module = formula_module(&q);
        assert!(module.contains("fn formula_perturb_loop_scaled(") && !module.contains("fn formula_perturb_bailout("));
    }

    #[test]
    fn literals_are_valid_wgsl_floats() {
        assert_eq!(lit_f32(1.0), "1.0");
        assert_eq!(lit_f32(-2.5), "-2.5");
        assert_eq!(lit_f32(1e-45), "0.0", "subnormal flushes here, not on the GPU");
        assert_eq!(lit_f32(1e40), format!("{:?}", f32::MAX));
        assert!(lit_f32(1e-7).contains('e'));
    }
}
