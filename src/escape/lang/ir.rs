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
}

impl Predef {
    pub fn ty(self) -> Ty {
        match self {
            Predef::Pixel | Predef::Z | Predef::Random | Predef::Center | Predef::ScreenPixel => Ty::Complex,
            Predef::NumIter | Predef::MaxIter | Predef::X | Predef::Y | Predef::Width | Predef::Height => Ty::Int,
            Predef::Pi | Predef::E | Predef::Magn | Predef::Index => Ty::Float,
            Predef::Solid => Ty::Bool,
            Predef::WhiteSq | Predef::ScrnMax | Predef::IsMand | Predef::MagXMag | Predef::RotSkew => Ty::Complex,
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    Var(VarId),
    Z,
    Index,
    Solid,
}

#[derive(Clone, Debug)]
pub enum IStmt {
    Assign(Place, IExpr),
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
    /// The formula's continue test (true: iterate again).
    pub bailout: Option<IExpr>,
    /// A colouring's `final:`.
    pub final_body: Vec<IStmt>,
    pub region: Region,
    pub view: ViewSettings,
    /// A colouring's variables that live from one section, or one
    /// iteration, to the next: what its packed state holds, in order.
    pub state: Vec<VarId>,
}

impl Program {
    /// Floats the colouring's state takes.
    pub fn state_floats(&self) -> usize {
        self.state
            .iter()
            .map(|&v| if self.vars[v].stored_real { 1 } else { self.vars[v].ty.floats() })
            .sum()
    }

    /// Whether any section reads `p`.
    pub fn reads(&self, p: Predef) -> bool {
        fn e(x: &IExpr, p: Predef) -> bool {
            match &x.kind {
                IExprKind::Predef(q) => *q == p,
                IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) => e(a, p),
                IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => e(a, p) || e(b, p),
                _ => false,
            }
        }
        fn s(st: &[IStmt], p: Predef) -> bool {
            st.iter().any(|x| match x {
                IStmt::Assign(_, v) => e(v, p),
                IStmt::If { arms, otherwise } => arms.iter().any(|(c, b)| e(c, p) || s(b, p)) || s(otherwise, p),
                IStmt::While { cond, body } | IStmt::Repeat { body, cond } => e(cond, p) || s(body, p),
            })
        }
        s(&self.init, p) || s(&self.loop_body, p) || s(&self.final_body, p) || self.bailout.as_ref().is_some_and(|b| e(b, p))
    }

    /// Slots the parameters take in a 16-float block: one each, two for
    /// a complex; one per user function.
    pub fn param_slots(&self) -> usize {
        self.params.iter().map(|p| p.ty().floats()).sum::<usize>() + self.funcs.len()
    }
}
