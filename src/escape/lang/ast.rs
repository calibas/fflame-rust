//! The parse tree of a formula file, before names and types are
//! resolved.

use super::Span;

/// A file: its entries in order.
#[derive(Clone, Debug, Default)]
pub struct File {
    pub entries: Vec<Entry>,
}

/// One `Name(setting) { ... }` entry.
#[derive(Clone, Debug)]
pub struct Entry {
    /// As written (identifiers compare lowered, but the user's name is
    /// what the panel shows).
    pub name: String,
    /// The parenthesised value after the name: symmetry for a formula,
    /// `INSIDE` / `OUTSIDE` for a colouring.
    pub setting: Option<String>,
    /// The whole entry, name to closing brace.
    pub span: Span,
    /// The body between the braces.
    pub body: Span,
    pub sections: Vec<Section>,
    pub default: Settings,
    pub switch: Settings,
}

impl Entry {
    pub fn section(&self, kind: SectionKind) -> Option<&Section> {
        self.sections.iter().find(|s| s.kind == kind)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionKind {
    Global,
    Builtin,
    Init,
    Loop,
    Bailout,
    PerturbInit,
    PerturbLoop,
    Final,
    Transform,
}

impl SectionKind {
    /// The label as written, lowered; `default` and `switch` hold
    /// settings, not statements, and are kept apart.
    pub fn from_label(label: &str) -> Option<Self> {
        Some(match label {
            "global" => SectionKind::Global,
            "builtin" => SectionKind::Builtin,
            "init" => SectionKind::Init,
            "loop" => SectionKind::Loop,
            "bailout" => SectionKind::Bailout,
            "perturbinit" => SectionKind::PerturbInit,
            "perturbloop" => SectionKind::PerturbLoop,
            "final" => SectionKind::Final,
            "transform" => SectionKind::Transform,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            SectionKind::Global => "global",
            SectionKind::Builtin => "builtin",
            SectionKind::Init => "init",
            SectionKind::Loop => "loop",
            SectionKind::Bailout => "bailout",
            SectionKind::PerturbInit => "perturbinit",
            SectionKind::PerturbLoop => "perturbloop",
            SectionKind::Final => "final",
            SectionKind::Transform => "transform",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Section {
    pub kind: SectionKind,
    pub span: Span,
    pub body: Vec<Stmt>,
    /// Fractint's shape: the loop began at an empty `:` label, so its
    /// last statement is the bailout test.
    pub last_is_bailout: bool,
    /// False for the statements before any label, which are `init:` in
    /// a formula and `final:` in a colouring; the checker decides.
    pub labelled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeName {
    Bool,
    Int,
    Float,
    Complex,
    Color,
}

impl TypeName {
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "bool" => TypeName::Bool,
            "int" => TypeName::Int,
            "float" => TypeName::Float,
            "complex" => TypeName::Complex,
            "color" => TypeName::Color,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub enum Stmt {
    /// `float x = 1`, or an array, `float a[10, 4]` (`dims`, each size as
    /// written; empty for UF's dynamic `float a[]`).
    Decl { ty: TypeName, name: String, init: Option<Expr>, dims: Option<Vec<Expr>>, span: Span },
    Expr(Expr),
    If { arms: Vec<(Expr, Vec<Stmt>)>, otherwise: Option<Vec<Stmt>>, span: Span },
    While { cond: Expr, body: Vec<Stmt>, span: Span },
    Repeat { body: Vec<Stmt>, cond: Expr, span: Span },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    /// `|a + b| - |a|` of reals, by sign case (laser blaster's
    /// `diffabs`): the deviation of `abs` in derived perturbation
    /// equations (`lang::delta`). Never parsed.
    DiffAbs,
    /// Rhai's bit operations, on ints as 32-bit patterns (UF has none);
    /// on bools `&` and `|` are and and or, `^` xor. `>>` is logical, as
    /// a hash of unsigned values wants it.
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Pow => "^",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::DiffAbs => "diffabs",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Imag(f64),
    Bool(bool),
    /// `(re, im)`.
    Complex(Box<Expr>, Box<Expr>),
    /// A bare name: a variable, or a predefined symbol or parameter
    /// written without its prefix (Fractint's `pixel`, `p1`).
    Name(String),
    /// `#name`.
    Predef(String),
    /// `@name`.
    Param(String),
    /// `name(args)`, or `@name(arg)` when `user` (a user function).
    Call { name: String, args: Vec<Expr>, user: bool },
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `|x|`: the squared modulus.
    Modulus(Box<Expr>),
    /// `name[i, j]`: an array's element.
    Index(String, Vec<Expr>),
    /// `target = value`; an expression, so `a = b = 1` chains.
    Assign(Box<Expr>, Box<Expr>),
    /// A string: a value only where UF compares an enum parameter with
    /// an item's name, `@type == "Sine"`.
    Str(String),
}

/// The `default:` (or `switch:`) section: `name = value` settings and
/// `param` / `func` / `heading` blocks.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub items: Vec<Setting>,
    pub params: Vec<ParamBlock>,
    pub funcs: Vec<FuncBlock>,
}

impl Settings {
    pub fn get(&self, name: &str) -> Option<&Setting> {
        self.items.iter().rev().find(|s| s.name == name)
    }
}

#[derive(Clone, Debug)]
pub struct Setting {
    pub name: String,
    pub value: SettingValue,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum SettingValue {
    Expr(Expr),
    /// One or more strings (`enum = "a" "b"` lists several).
    Strings(Vec<String>),
    /// A function reference, `default = sin()`.
    Func(String),
}

#[derive(Clone, Debug)]
pub struct ParamBlock {
    pub ty: Option<TypeName>,
    pub name: String,
    pub settings: Vec<Setting>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct FuncBlock {
    pub ty: Option<TypeName>,
    pub name: String,
    pub settings: Vec<Setting>,
    pub span: Span,
}

impl ParamBlock {
    pub fn get(&self, name: &str) -> Option<&Setting> {
        self.settings.iter().rev().find(|s| s.name == name)
    }
}

impl FuncBlock {
    pub fn get(&self, name: &str) -> Option<&Setting> {
        self.settings.iter().rev().find(|s| s.name == name)
    }
}
