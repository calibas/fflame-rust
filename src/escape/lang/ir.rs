//! The checked program: names resolved, every expression typed, every
//! conversion explicit. Both back ends (the interpreter and WGSL) read
//! this and nothing else.

use super::ast::{BinOp, UnOp};
use super::builtins::Builtin;
use super::{Dialect, EntryKind, Span};

/// A value's type. `color` is phase 4 (direct colouring).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Bool,
    Int,
    Float,
    Complex,
}

impl Ty {
    /// Rank for UF's widening rule: int to float to complex, never back.
    pub fn rank(self) -> u8 {
        match self {
            Ty::Bool => 0,
            Ty::Int => 1,
            Ty::Float => 2,
            Ty::Complex => 3,
        }
    }

    pub fn is_numeric(self) -> bool {
        self != Ty::Bool
    }

    /// The wider of two numeric types.
    pub fn join(self, other: Ty) -> Ty {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Ty::Bool => "bool",
            Ty::Int => "int",
            Ty::Float => "float",
            Ty::Complex => "complex",
        }
    }

    /// Floats this type takes in a colouring's packed state.
    pub fn floats(self) -> usize {
        match self {
            Ty::Complex => 2,
            _ => 1,
        }
    }
}

pub type VarId = usize;
pub type ParamId = usize;
pub type FuncId = usize;

#[derive(Clone, Debug)]
pub struct Var {
    pub name: String,
    pub ty: Ty,
    pub span: Span,
    /// A complex variable whose every assignment is real-valued, kept in
    /// one float (a colouring's state packing; every read is still
    /// complex, so the program means the same).
    pub stored_real: bool,
    /// Set in `global:`: one value per image, read-only elsewhere (UF's
    /// rule), computed on the CPU and read from the uniform.
    pub global: bool,
    /// Made in `perturbinit:` or `perturbloop:`: a temporary of the
    /// perturbation equations, which see no other section's variables.
    pub perturb: bool,
    /// An array's dimensions, the first the slowest, at the settings it
    /// was checked at; empty for a scalar. Its elements are `ty`, read
    /// and written by a flattened index ([`IExprKind::Elem`],
    /// [`IStmt::SetElem`]), zero until written.
    pub dims: Vec<u32>,
    /// The dimensions as written: a literal each for a constant size, an
    /// int expression of the parameters and `#maxiter` for one the
    /// settings fix (UF counts those constants; plan, item 3, step 3f),
    /// which `dims` holds folded. Empty for a scalar.
    pub size: Vec<IExpr>,
}

impl Var {
    /// A scalar of no section's (the passes' own temporaries).
    pub fn temp(name: impl Into<String>, ty: Ty) -> Var {
        Var { name: name.into(), ty, span: Span::default(), stored_real: false, global: false, perturb: false, dims: Vec::new(), size: Vec::new() }
    }

    pub fn is_array(&self) -> bool {
        !self.dims.is_empty()
    }

    /// Elements: 1 for a scalar.
    pub fn len(&self) -> usize {
        self.dims.iter().map(|&d| d as usize).product()
    }

    /// Floats the variable takes packed: its elements', two for a complex.
    pub fn floats(&self) -> usize {
        self.len() * self.ty.floats()
    }
}

/// The predefined symbols a program reads or writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Predef {
    Pixel,
    Z,
    NumIter,
    MaxIter,
    Pi,
    E,
    Random,
    Center,
    Magn,
    X,
    Y,
    Width,
    Height,
    ScreenPixel,
    Index,
    Solid,
    /// Fractint's: 1 on the pixels where (row + col) is odd.
    WhiteSq,
    /// Fractint's: the frame's size in pixels, (width, height).
    ScrnMax,
    /// Fractint's: 1 on the parameter plane, 0 on the Julia plane (the
    /// Julia toggle).
    IsMand,
    /// Fractint's: (magnification, x-magnification factor 1).
    MagXMag,
    /// Fractint's: (rotation in degrees, skew 0).
    RotSkew,
    /// The Julia constant: what a `switch:` target's `#pixel` copy
    /// reads ([`super::switch`]); no name in either language.
    JuliaC,
    /// `#dz`: the pixel's deviation from the reference orbit, in the
    /// perturbation sections (where `#z` is the reference's value).
    DZ,
    /// `#dpixel`: the pixel's offset from the reference point.
    DPixel,
    /// The loop pass being run, 0 on the first (in the test, the passes
    /// run): what a hybrid's groups branch on ([`super::hybrid`]). No
    /// name in either language: UF reads `#numiter` only in a
    /// colouring's `final:`.
    Pass,
    /// The panel's crease settings, in a formula's creases section (a
    /// Rhai `fn creases`; `FormulaFeature::Creases`): how much finer each
    /// later crease is, the pattern (0 valley, 1 alternate, 2 random),
    /// the flip as a sign, and how far a slope runs in paper sizes (0:
    /// unbounded, the 2D relief's; a terrain's levels off).
    CreaseDecay,
    CreasePattern,
    CreaseSign,
    CreaseReach,
    /// What the creases section gives: the folded paper's height at the
    /// starting point, and its gradient there.
    CreaseHeight,
    CreaseSlope,
    /// A fresh uniform draw in [0, 1) each time it is read: Rhai's
    /// `rand()` (the KIFS plan, C2a). In a flame variation the flame's
    /// own stream (`NeedsRng`); elsewhere a stream per pixel, seeded by
    /// its place. No name in UF, whose `#random` is one value a pixel.
    Draw,
    /// A flame variation's context (C2b): the weighted sum of the
    /// transform's earlier variations (JWildfire's `pVarTP`;
    /// `NeedsAccum`), and the variation's own weight.
    Accum,
    Weight,
    /// What a flame variation writes as its colour (C2d): the palette
    /// index the transform's `direct_color` blends in (`WritesColor`).
    VarColor,
}

impl Predef {
    pub fn ty(self) -> Ty {
        match self {
            Predef::Pixel | Predef::Z | Predef::Random | Predef::Center | Predef::ScreenPixel | Predef::Accum => Ty::Complex,
            Predef::Draw | Predef::Weight | Predef::VarColor => Ty::Float,
            Predef::NumIter | Predef::MaxIter | Predef::X | Predef::Y | Predef::Width | Predef::Height | Predef::Pass | Predef::CreasePattern => Ty::Int,
            Predef::Pi | Predef::E | Predef::Magn | Predef::Index | Predef::CreaseDecay | Predef::CreaseSign | Predef::CreaseReach | Predef::CreaseHeight => Ty::Float,
            Predef::CreaseSlope => Ty::Complex,
            Predef::Solid => Ty::Bool,
            Predef::WhiteSq
            | Predef::ScrnMax
            | Predef::IsMand
            | Predef::MagXMag
            | Predef::RotSkew
            | Predef::JuliaC
            | Predef::DZ
            | Predef::DPixel => Ty::Complex,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamKind {
    Bool,
    Int,
    Float,
    Complex,
    /// An int chosen from named items.
    Enum,
}

#[derive(Clone, Debug)]
pub struct Param {
    /// Lowered, as `@name` reads it and the config's map keys it.
    pub name: String,
    pub caption: String,
    pub kind: ParamKind,
    /// The default; a complex default's imaginary part in `.1`.
    pub default: (f64, f64),
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub hint: String,
    pub choices: Vec<String>,
    pub exponential: bool,
    pub span: Span,
}

impl Param {
    pub fn ty(&self) -> Ty {
        match self.kind {
            ParamKind::Bool => Ty::Bool,
            ParamKind::Int | ParamKind::Enum => Ty::Int,
            ParamKind::Float => Ty::Float,
            ParamKind::Complex => Ty::Complex,
        }
    }
}

/// A user function: chosen by the user from [`super::builtins::USER_FUNCTIONS`].
#[derive(Clone, Debug)]
pub struct UserFunc {
    pub name: String,
    pub caption: String,
    /// Index into `USER_FUNCTIONS`.
    pub default: usize,
    pub hint: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct IExpr {
    pub kind: IExprKind,
    pub ty: Ty,
}

impl IExpr {
    /// A real value lifted to complex: a literal with a zero imaginary
    /// part, or a converted int or float. A complex product with one
    /// scales the other operand, as both back ends compute it: cheaper,
    /// and as the built-ins write `2.0 * z` (the full product's
    /// `0 * inf` would make an overflowed operand NaN).
    pub fn is_lifted_real(&self) -> bool {
        match &self.kind {
            IExprKind::Complex(_, im) => *im == 0.0,
            IExprKind::Convert(a) => self.ty == Ty::Complex && matches!(a.ty, Ty::Int | Ty::Float),
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum IExprKind {
    Bool(bool),
    Int(i64),
    Float(f64),
    Complex(f64, f64),
    Var(VarId),
    Param(ParamId),
    Predef(Predef),
    /// Convert the operand to `ty` (widening, or bool to and from a
    /// number).
    Convert(Box<IExpr>),
    /// `(re, im)` from two float expressions.
    MakeComplex(Box<IExpr>, Box<IExpr>),
    Unary(UnOp, Box<IExpr>),
    /// Both operands already of the operation's type (comparisons: of
    /// their common type; the result is bool).
    Binary(BinOp, Box<IExpr>, Box<IExpr>),
    /// `base ^ n` for a constant integer `n`: repeated multiplication,
    /// as exact as the arithmetic.
    PowInt(Box<IExpr>, i64),
    /// `|x|`, the squared modulus, of a float or complex operand.
    Modulus(Box<IExpr>),
    /// A built-in on an operand of the type `result_ty` was asked about
    /// (an int argument is converted first unless the result is int).
    Call(Builtin, Box<IExpr>),
    UserCall(FuncId, Box<IExpr>),
    /// The derivative of the user's chosen function at the operand
    /// (zero for a choice with no complex derivative): the derivative
    /// pass's, never the checker's.
    UserDeriv(FuncId, Box<IExpr>),
    /// The real-linear derivative of the user's chosen function at the
    /// operand, `f'(a) d = A d + B conj(d)`: `A` (false) or `B` (true),
    /// each choice by its rule (`derive::user_jac_forms`), so a user call
    /// in a Jacobian is two nodes, not an arm per choice. The derivative
    /// pass's, never the checker's.
    UserJac(FuncId, Box<IExpr>, bool),
    /// Which function a user function's parameter picks, an int, its
    /// index into `USER_FUNCTIONS`: the derived perturbation equations',
    /// which branch on it, never the checker's.
    FuncChoice(FuncId),
    /// `f(a + d) - f(a)` for the function a user function's parameter
    /// picks, by its cancellation-free form (`delta::user_diff_forms`):
    /// the derived perturbation equations', never the checker's.
    UserDiff(FuncId, Box<IExpr>, Box<IExpr>),
    /// An array variable's element at a flattened index (an int, clamped
    /// to the array: UF stops the render past it, the GPU cannot).
    Elem(VarId, Box<IExpr>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    Var(VarId),
    Z,
    Index,
    Solid,
    /// `#dz`, in the perturbation sections.
    DZ,
    /// The creases section's height and gradient ([`Predef::CreaseHeight`]).
    CreaseHeight,
    CreaseSlope,
    /// `#pixel`, written in a transformation's `transform:` section: the
    /// point it gives.
    Pixel,
    /// A flame variation's colour ([`Predef::VarColor`]).
    Color,
}

#[derive(Clone, Debug)]
pub enum IStmt {
    Assign(Place, IExpr),
    /// `var[index] = value`: an array's element, at a flattened index
    /// clamped as [`IExprKind::Elem`] reads it.
    SetElem { var: VarId, index: IExpr, value: IExpr },
    If { arms: Vec<(IExpr, Vec<IStmt>)>, otherwise: Vec<IStmt> },
    While { cond: IExpr, body: Vec<IStmt> },
    Repeat { body: Vec<IStmt>, cond: IExpr },
}

/// A colouring's region tag (`Name(INSIDE) { ... }`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Both,
    Inside,
    Outside,
}

/// View settings from `default:` that become the entry's preset.
#[derive(Clone, Debug, Default)]
pub struct ViewSettings {
    pub title: Option<String>,
    pub center: Option<(f64, f64)>,
    pub magn: Option<f64>,
    pub angle: Option<f64>,
    pub maxiter: Option<u32>,
}

/// A checked entry.
#[derive(Clone, Debug)]
pub struct Program {
    pub kind: EntryKind,
    /// Fractint's tests read the real part alone, and its `round` is
    /// `floor(x + 0.5)`; the back ends follow this.
    pub dialect: Dialect,
    pub name: String,
    pub vars: Vec<Var>,
    pub params: Vec<Param>,
    pub funcs: Vec<UserFunc>,
    pub init: Vec<IStmt>,
    pub loop_body: Vec<IStmt>,
    /// `global:`, run once per image (on the CPU, by the interpreter).
    pub global_body: Vec<IStmt>,
    /// The global variables the other sections read, in the order they
    /// are packed: a formula's into the uniform's `fdata`, a colouring's
    /// into its parameter block after its parameters.
    pub globals: Vec<VarId>,
    /// The formula's continue test (true: iterate again).
    pub bailout: Option<IExpr>,
    /// A colouring's `final:`.
    pub final_body: Vec<IStmt>,
    /// UF's perturbation equations (`perturbinit:`, `perturbloop:`):
    /// the pixel's deviation `#dz` from the reference orbit, its start
    /// and its step, with `#z` the reference's value and `#dpixel` the
    /// pixel's offset. Empty: the formula has none.
    pub perturb_init: Vec<IStmt>,
    pub perturb_loop: Vec<IStmt>,
    /// The `perturb` setting: when the equations hold (parameters and
    /// constants only; absent, always).
    pub perturb_when: Option<IExpr>,
    /// The `degree` setting: the formula's degree at infinity (the `p` of
    /// `|z_{n+1}| ~ |z_n|^p`), a float of parameters and constants, which
    /// the smooth colourings divide by, as a built-in's `escape_degree`.
    /// Absent, 2. Our own setting (a Rhai formula's `fn degree(p)`); UF
    /// has none.
    pub degree: Option<IExpr>,
    pub region: Region,
    pub view: ViewSettings,
    /// A colouring's variables that live from one section, or one
    /// iteration, to the next: what its packed state holds, in order.
    pub state: Vec<VarId>,
    /// The variable holding the iterate's derivative with respect to
    /// the pixel, in a program the derivative pass extended.
    pub dz_var: Option<VarId>,
    /// Beside it, where the derivative is a Jacobian (`derive::with_jacobian`):
    /// the iterate's `Q`, `dz_var` its `P` (plan, phase 4, step 9d).
    pub dzq_var: Option<VarId>,
    /// And the pixel's own derivative, 1 unless the perturbed floatexp
    /// rung rescales it (`derive::with_derivative`).
    pub dpixel_var: Option<VarId>,
    /// How a program that reads the pass index ([`Predef::Pass`]) uses
    /// it: periodically after a prefix (a hybrid's groups).
    pub cycle: Option<Cycle>,
    /// What the derived perturbation equations carry besides z, in the
    /// order the reference records them ([`Carried`]). Empty: z alone.
    pub carried: Vec<Carried>,
    /// A folding formula's paper (a Rhai `fn creases`): from the starting
    /// point (`#pixel` there) and the crease settings, the folded paper's
    /// height and gradient (`Place::CreaseHeight`, `Place::CreaseSlope`),
    /// reading its own variables, the parameters and the globals. Empty:
    /// none.
    pub creases: Vec<IStmt>,
    /// A transformation's `transform:` section (`EntryKind::Transform`):
    /// `#pixel` the point in, and the point out once written. Empty for
    /// a formula or a colouring.
    pub transform: Vec<IStmt>,
}

/// A variable the loop carries from one pass to the next besides z, as
/// derived perturbation equations follow it (plan, phase 4 step 5): the
/// reference orbit records `var` at the start of each pass, `reference`
/// reads that record in the equations, and `deviation` is the pixel's
/// deviation from it, kept from pass to pass as `#dz` is. A rebase onto
/// the orbit's start moves it as it moves `#dz`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carried {
    pub var: VarId,
    pub reference: VarId,
    pub deviation: VarId,
}

/// A loop whose passes repeat: the first `prefix` passes run once,
/// then every pass `n` behaves as pass `prefix + (n - prefix) % period`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cycle {
    pub prefix: u32,
    pub period: u32,
}

impl Cycle {
    /// The pass that pass `n` behaves as: itself in the prefix, else
    /// its place in the first round of the loop.
    pub fn phase(self, n: u32) -> u32 {
        if n < self.prefix {
            n
        } else {
            self.prefix + (n - self.prefix) % self.period.max(1)
        }
    }
}

/// An array size's value: literals, parameters (`param` gives each its
/// value, an int's rounded), `#maxiter`, the constants, arithmetic and the
/// real built-ins (`round`, `trunc`, `sqr`, ...). `None` for anything
/// else, which the checker refuses in a size.
pub fn size_value(e: &IExpr, param: &dyn Fn(ParamId) -> f64, maxiter: u32) -> Option<f64> {
    use IExprKind as K;
    let v = |a: &IExpr| size_value(a, param, maxiter);
    Some(match &e.kind {
        K::Int(i) => *i as f64,
        K::Float(x) => *x,
        K::Bool(b) => f64::from(u8::from(*b)),
        K::Param(id) => param(*id),
        K::Predef(Predef::MaxIter) => f64::from(maxiter),
        K::Predef(Predef::Pi) => std::f64::consts::PI,
        K::Predef(Predef::E) => std::f64::consts::E,
        K::Convert(a) => v(a)?,
        K::Unary(UnOp::Neg, a) => -v(a)?,
        K::Binary(op, a, b) => {
            let (x, y) = (v(a)?, v(b)?);
            match op {
                BinOp::Add => x + y,
                BinOp::Sub => x - y,
                BinOp::Mul => x * y,
                BinOp::Div => x / y,
                BinOp::Pow => x.powf(y),
                _ => return None,
            }
        }
        K::PowInt(a, n) => v(a)?.powi(i32::try_from(*n).ok()?),
        K::Modulus(a) => v(a)?.powi(2),
        K::Call(b, a) => b.eval_f(v(a)?),
        _ => return None,
    })
}

impl Program {
    /// Whether an array's size reads the settings (a parameter,
    /// `#maxiter`): the program is checked again at each settings' sizes
    /// (`custom::resolve`).
    pub fn sized_by_settings(&self) -> bool {
        self.vars.iter().any(|v| v.size.iter().any(|e| !matches!(e.kind, IExprKind::Int(_))))
    }

    /// Every array's dimensions at the settings: the parameters' values by
    /// index (as `interp::ParamValues` holds them) and the iteration cap.
    /// `None` where a size has no value.
    pub fn sizes_at(&self, values: &[(f64, f64)], maxiter: u32) -> Option<Vec<i64>> {
        let param = |id: ParamId| {
            let x = values.get(id).map_or(self.params[id].default.0, |v| v.0);
            if self.params[id].ty() == Ty::Int { x.round() } else { x }
        };
        let mut out = Vec::new();
        for v in &self.vars {
            for e in &v.size {
                out.push(size_value(e, &param, maxiter)?.round() as i64);
            }
        }
        Some(out)
    }

    /// Floats the colouring's state takes.
    pub fn state_floats(&self) -> usize {
        self.state
            .iter()
            .map(|&v| if self.vars[v].stored_real { 1 } else { self.vars[v].floats() })
            .sum()
    }

    /// Floats the packed globals take: two for a complex, one otherwise,
    /// an array's for each of its elements.
    pub fn global_floats(&self) -> usize {
        self.globals.iter().map(|&v| self.vars[v].floats()).sum()
    }

    /// Whether any section reads `p`.
    pub fn reads(&self, p: Predef) -> bool {
        fn e(x: &IExpr, p: Predef) -> bool {
            match &x.kind {
                IExprKind::Predef(q) => *q == p,
                IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) | IExprKind::UserJac(_, a, _) | IExprKind::Elem(_, a) => e(a, p),
                IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => e(a, p) || e(b, p),
                _ => false,
            }
        }
        fn s(st: &[IStmt], p: Predef) -> bool {
            st.iter().any(|x| match x {
                IStmt::Assign(_, v) => e(v, p),
                IStmt::SetElem { index, value, .. } => e(index, p) || e(value, p),
                IStmt::If { arms, otherwise } => arms.iter().any(|(c, b)| e(c, p) || s(b, p)) || s(otherwise, p),
                IStmt::While { cond, body } | IStmt::Repeat { body, cond } => e(cond, p) || s(body, p),
            })
        }
        s(&self.global_body, p)
            || s(&self.init, p)
            || s(&self.loop_body, p)
            || s(&self.final_body, p)
            || self.bailout.as_ref().is_some_and(|b| e(b, p))
            || s(&self.perturb_init, p)
            || s(&self.perturb_loop, p)
            || s(&self.creases, p)
            || s(&self.transform, p)
    }

    /// Slots the parameters take in a 16-float block: one each, two for
    /// a complex; one per user function.
    pub fn param_slots(&self) -> usize {
        self.params.iter().map(|p| p.ty().floats()).sum::<usize>() + self.funcs.len()
    }
}
