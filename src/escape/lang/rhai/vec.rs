//! Vectors and quaternions in the Rhai dialect (the KIFS plan, B1; the
//! quaternions plan, docs/projects/quaternions.md): `vec3` and `quat`,
//! split into their parts by the lowering.
//!
//! A vector local is three float locals and a quaternion local four; a
//! vector field of the state three fields (`z__x`, `z__y`, `z__z`) and a
//! quaternion field four (`q__w`, `q__x`, `q__y`, `q__z`, `w` the scalar
//! part); a vector parameter three float parameters (`c_x`, `c_y`, `c_z`)
//! and a quaternion parameter four (`c_w` ...). Every operation is
//! written out on the parts: `dot(a, b)` a sum of products, a product of
//! quaternions Hamilton's sums, a quaternion's `sin` the complex `sin` of
//! `t + i r` carried back along its vector part. So the IR, the
//! interpreter, the WGSL emitter and the derivation passes see scalar
//! code and need nothing of their own -- a GPU thread runs scalar code
//! anyway.
//!
//! The vector surface: `vec3(x, y, z)` (or `vec3(s)`, every part `s`);
//! `v.x`, `v.y`, `v.z`, read and set; `+`, `-`, unary `-`; `*` and `/` by
//! a number, or part by part between vectors; `dot`, `cross`, `length`,
//! `normalize`, `abs`, `floor`, `min`, `max`, `clamp`, as functions or as
//! methods. `min`, `max` and `clamp` of numbers come with them: a
//! temporary and an `if`, which the derivation passes take as any `if`.
//! A helper may give a vector, and an `if` may be one, each arm's value
//! its parts set in it; a function parameter of a vector (`p.f(v)`) is
//! taken part by part, each part the real part of the function of it
//! (the KIFS plan, B's leftovers).
//!
//! The quaternion surface: `quat(w, x, y, z)`, `quat(w, v)` (`v` a
//! vector) or `quat(w)`; `q.w`, `q.x`, `q.y`, `q.z` read and set, `q.v`
//! its vector part; `+` and `-` between quaternions, or a number to or
//! from the scalar part; unary `-`; `*` Hamilton's product, or a scaling
//! by a number; `/` by a number (`q / r` is refused: `q * inv(r)` and
//! `inv(r) * q` differ); `dot`, `length`, `normalize`, `conj`, `inv`;
//! `sin`, `cos`, `tan`, `sinh`, `cosh`, `tanh`, `exp`, `ln` (and `log`,
//! base 10, as Rhai's), `sqrt` and `pow(q, n)` (or `q ** n`) for a number
//! `n`; `q.complex()`, the complex number `t + i r` those act in.

use super::*;

/// What a vector is: a 3D vector, or a quaternion (its scalar part
/// first).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Vec3,
    Quat,
}

impl Kind {
    /// The parts' names, in order.
    pub(super) fn axes(self) -> &'static [&'static str] {
        match self {
            Kind::Vec3 => &["x", "y", "z"],
            Kind::Quat => &["w", "x", "y", "z"],
        }
    }

    fn what(self) -> &'static str {
        match self {
            Kind::Vec3 => "a vector",
            Kind::Quat => "a quaternion",
        }
    }

    fn parts_are(self) -> &'static str {
        match self {
            Kind::Vec3 => "a vector's parts are `.x`, `.y` and `.z`",
            Kind::Quat => "a quaternion's parts are `.w`, `.x`, `.y` and `.z`, and `.v` its vector part",
        }
    }
}

/// A lowered vector: its kind, and its parts in the kind's order.
#[derive(Clone)]
pub(super) struct Vector {
    pub(super) kind: Kind,
    pub(super) parts: Vec<ast::Expr>,
}

impl Vector {
    pub(super) fn new(kind: Kind, parts: Vec<ast::Expr>) -> Self {
        debug_assert_eq!(parts.len(), kind.axes().len());
        Vector { kind, parts }
    }
}

pub(super) const NUMBER_WANTED: &str = "a vector where a number is wanted: take a part (`v.x`), `length(v)` or `dot(a, b)`";
const VECTOR_WANTED: &str = "a number where a vector is wanted: `vec3(x, y, z)`";

/// The functions a quaternion takes through its complex plane.
const PLANE_FUNCTIONS: &[&str] = &["sin", "cos", "tan", "sinh", "cosh", "tanh", "exp", "ln", "log", "sqrt"];

/// A lowered value: a vector's parts, or a number.
pub(super) enum Val {
    V(Vector),
    S(ast::Expr),
}

/// What a `.` leads from, named without lowering it.
pub(super) enum Owner {
    State,
    Params,
    Ctx,
}

/// The name of a vector field's or parameter's part `i`.
pub(super) fn part(name: &str, kind: Kind, i: usize) -> String {
    format!("{name}__{}", kind.axes()[i])
}

/// What the vector probe binds a name it has not lowered to: a vector of
/// a kind, or a number. Only their kinds are read.
fn vec_marker(kind: Kind) -> Binding {
    let z = || mk(ExprKind::Float(0.0), Span::default());
    Binding::VecValue(Box::new(Vector::new(kind, kind.axes().iter().map(|_| z()).collect())))
}

fn num_marker() -> Binding {
    Binding::Value(mk(ExprKind::Float(0.0), Span::default()))
}

fn bin(op: BinOp, a: ast::Expr, b: ast::Expr, span: Span) -> ast::Expr {
    mk(ExprKind::Binary(op, Box::new(a), Box::new(b)), span)
}

fn call1(name: &str, a: ast::Expr, span: Span) -> ast::Expr {
    mk(ExprKind::Call { name: name.to_string(), args: vec![a], user: false }, span)
}

fn neg(a: ast::Expr, span: Span) -> ast::Expr {
    mk(ExprKind::Unary(UnOp::Neg, Box::new(a)), span)
}

/// The sum of the products of two lists of parts.
fn dot_of(a: &[ast::Expr], b: &[ast::Expr], span: Span) -> ast::Expr {
    let mut terms = a.iter().zip(b).map(|(x, y)| bin(BinOp::Mul, x.clone(), y.clone(), span));
    let first = terms.next().expect("a part");
    terms.fold(first, |acc, t| bin(BinOp::Add, acc, t, span))
}

/// The kind `.rhs` of a vector of `kind` gives: a vector's `.abs()`, a
/// quaternion's `.sin()` or `.v`; `None` for a number (`.x`,
/// `.length()`, `.complex()`; anything after a number is a number's).
fn member_kind(kind: Kind, rhs: &Expr) -> Option<Kind> {
    match (kind, rhs) {
        (Kind::Vec3, Expr::MethodCall(f, _)) => {
            matches!(f.name.as_str(), "abs" | "floor" | "normalize" | "cross" | "min" | "max" | "clamp").then_some(Kind::Vec3)
        }
        (Kind::Quat, Expr::MethodCall(f, _)) => {
            let n = f.name.as_str();
            (matches!(n, "normalize" | "conj" | "inv" | "pow") || PLANE_FUNCTIONS.contains(&n)).then_some(Kind::Quat)
        }
        (Kind::Quat, Expr::Property(prop, _)) => (prop.2.as_str() == "v").then_some(Kind::Vec3),
        (_, Expr::Dot(be, _, _)) => member_kind(kind, &be.lhs).and_then(|k| member_kind(k, &be.rhs)),
        _ => None,
    }
}

impl Lowerer<'_> {
    /// What `e` names as the left of a `.`: the state (or a variation's
    /// `this`), the parameters, the context.
    pub(super) fn owner(&self, e: &Expr, scope: &Scope) -> Option<Owner> {
        match e {
            Expr::ThisPtr(_) if scope.which == Fn3::Variation => Some(Owner::State),
            Expr::Variable(v, _, _) => match scope.get(v.1.as_str()) {
                Some(Binding::State) => Some(Owner::State),
                Some(Binding::Params) => Some(Owner::Params),
                Some(Binding::Ctx) => Some(Owner::Ctx),
                _ => None,
            },
            _ => None,
        }
    }

    /// The vector a field of the state, a parameter or a context entry
    /// is, when it is one. A quaternion's parts include `x`, so it is
    /// looked for first, by its `w`.
    pub(super) fn named_vector(&self, owner: &Owner, name: &str, scope: &Scope, span: Span) -> Option<Vector> {
        let kinds = [Kind::Quat, Kind::Vec3];
        match owner {
            Owner::State => kinds.into_iter().find(|k| scope.fields.iter().any(|f| *f == part(name, *k, 0))).map(|kind| {
                let parts = (0..kind.axes().len()).map(|i| self.field_target(scope.which, &part(name, kind, i), span)).collect();
                Vector::new(kind, parts)
            }),
            Owner::Params => {
                if let Some(kind) = self.param_vecs.get(name).copied() {
                    Some(Vector::new(kind, kind.axes().iter().map(|a| mk(ExprKind::Param(format!("{name}_{a}")), span)).collect()))
                } else {
                    // A vector `prepare` gave (B's leftovers).
                    kinds.into_iter().find(|k| self.prepared.contains_key(&part(name, *k, 0))).map(|kind| {
                        Vector::new(kind, (0..kind.axes().len()).map(|i| mk(ExprKind::Name(prepared_name(&part(name, kind, i))), span)).collect())
                    })
                }
            }
            // A 3D variation's accumulator (B2).
            Owner::Ctx => (self.three && name == "accum").then(|| {
                let accum = || mk(ExprKind::Predef("accum".into()), span);
                Vector::new(Kind::Vec3, vec![call1("real", accum(), span), call1("imag", accum(), span), mk(ExprKind::Predef("accumz".into()), span)])
            }),
        }
    }

    /// Whether `e` is a vector, by its form and its names.
    pub(super) fn is_vec(&self, e: &Expr, scope: &Scope) -> bool {
        self.vec_kind(e, scope).is_some()
    }

    /// The kind of vector `e` is, if it is one.
    pub(super) fn vec_kind(&self, e: &Expr, scope: &Scope) -> Option<Kind> {
        match e {
            Expr::Variable(v, _, _) => match scope.get(v.1.as_str()) {
                Some(Binding::VecLocal(kind, _)) => Some(*kind),
                Some(Binding::VecValue(x)) => Some(x.kind),
                _ => None,
            },
            Expr::Dot(be, _, _) => {
                if let Some(kind) = self.vec_kind(&be.lhs, scope) {
                    return member_kind(kind, &be.rhs);
                }
                let owner = self.owner(&be.lhs, scope)?;
                let (first, rest) = match &be.rhs {
                    Expr::Dot(b2, _, _) => (&b2.lhs, Some(&b2.rhs)),
                    r => (r, None),
                };
                match first {
                    Expr::Property(prop, _) => {
                        let kind = self.named_vector(&owner, prop.2.as_str(), scope, Span::default())?.kind;
                        match rest {
                            None => Some(kind),
                            Some(r) => member_kind(kind, r),
                        }
                    }
                    // `p.f(v)`: a function parameter of a vector.
                    Expr::MethodCall(f, _) if matches!(owner, Owner::Params) && rest.is_none() && f.args.len() == 1 => self.vec_kind(&f.args[0], scope),
                    _ => None,
                }
            }
            Expr::FnCall(f, _) => {
                let n = f.name.as_str();
                if f.op_token.is_some() || is_operator(n) {
                    if !matches!(n, "+" | "-" | "*" | "/" | "**") {
                        return None;
                    }
                    return f.args.iter().find_map(|a| self.vec_kind(a, scope));
                }
                if self.fns.contains_key(n) {
                    return self.helper_gives_vec(n, &f.args, scope);
                }
                self.call_kind(n, &f.args, scope)
            }
            // An `if` as a value, by its first arm's.
            Expr::Stmt(b) => match b.statements() {
                [Stmt::If(fc, _)] => self.block_gives_vec(&fc.body, scope),
                _ => None,
            },
            _ => None,
        }
    }

    /// The kind a built-in function of `args` gives, if a vector.
    fn call_kind(&self, n: &str, args: &[Expr], scope: &Scope) -> Option<Kind> {
        let first = || args.first().and_then(|a| self.vec_kind(a, scope));
        match n {
            "vec3" | "cross" => Some(Kind::Vec3),
            "quat" => Some(Kind::Quat),
            "normalize" | "abs" | "floor" | "min" | "max" | "clamp" => first(),
            "conj" | "inv" | "pow" => first().filter(|k| *k == Kind::Quat),
            _ if PLANE_FUNCTIONS.contains(&n) => first().filter(|k| *k == Kind::Quat),
            // A solid's call of a built-in variation (P5).
            "vary" => (self.solid && args.get(1).is_some_and(|a| self.is_vec(a, scope))).then_some(Kind::Vec3),
            _ => None,
        }
    }

    /// A value, a vector or a number as it is.
    pub(super) fn arg(&mut self, e: &Expr, scope: &mut Scope) -> R<Val> {
        if self.is_vec(e, scope) {
            Ok(Val::V(self.vexpr(e, scope)?))
        } else {
            Ok(Val::S(self.expr(e, scope)?))
        }
    }

    /// The vector `e`, as its parts.
    pub(super) fn vexpr(&mut self, e: &Expr, scope: &mut Scope) -> R<Vector> {
        let span = self.pos.span(e.position());
        match e {
            Expr::Variable(v, _, p) => match scope.get(v.1.as_str()).cloned() {
                Some(Binding::VecLocal(kind, n)) => Ok(Vector::new(kind, n.into_iter().map(|n| mk(ExprKind::Name(n), span)).collect())),
                Some(Binding::VecValue(x)) => Ok(*x),
                _ => self.err(*p, VECTOR_WANTED),
            },
            Expr::Dot(be, _, _) => {
                let got = if self.is_vec(&be.lhs, scope) {
                    let v = self.vexpr(&be.lhs, scope)?;
                    self.member(v, &be.rhs, scope)?
                } else {
                    let Some(owner) = self.owner(&be.lhs, scope) else { return self.err(e.position(), VECTOR_WANTED) };
                    let (first, rest) = match &be.rhs {
                        Expr::Dot(b2, _, _) => (&b2.lhs, Some(&b2.rhs)),
                        r => (r, None),
                    };
                    // `p.f(v)`: the function of each part, its real part.
                    if let (Owner::Params, Expr::MethodCall(f, fp), None) = (&owner, first, rest) {
                        if f.args.len() == 1 && self.is_vec(&f.args[0], scope) {
                            let v = self.vexpr(&f.args[0], scope)?;
                            let span = self.pos.span(*fp);
                            let name = f.name.to_string();
                            let v = self.hold_all(v, "part");
                            let parts = v.parts.into_iter().map(|c| call1("real", mk(ExprKind::Call { name: name.clone(), args: vec![c], user: true }, span), span)).collect();
                            return Ok(Vector::new(v.kind, parts));
                        }
                    }
                    let Expr::Property(prop, pp) = first else { return self.err(e.position(), VECTOR_WANTED) };
                    let Some(v) = self.named_vector(&owner, prop.2.as_str(), scope, self.pos.span(*pp)) else {
                        return self.err(e.position(), VECTOR_WANTED);
                    };
                    match rest {
                        Some(r) => self.member(v, r, scope)?,
                        None => Val::V(v),
                    }
                };
                match got {
                    Val::V(v) => Ok(v),
                    Val::S(_) => self.err(e.position(), VECTOR_WANTED),
                }
            }
            Expr::FnCall(f, p) => self.vcall(f, *p, scope),
            Expr::Stmt(b) => match b.statements() {
                [Stmt::If(fc, p)] => self.vec_if_value(fc, *p, scope),
                _ => self.err(e.position(), VECTOR_WANTED),
            },
            _ => self.err(e.position(), VECTOR_WANTED),
        }
    }

    /// A call giving a vector: a helper of the script's, inlined, or a
    /// function of vectors.
    fn vcall(&mut self, f: &FnCallExpr, p: Position, scope: &mut Scope) -> R<Vector> {
        let n = f.name.as_str();
        if n == "vary" && self.solid {
            return self.vary_map(f, p, scope);
        }
        if let Some(def) = self.fns.get(n).cloned() {
            if matches!(n, "start" | "step" | "escaped" | "distance" | "finish" | "prepare" | "init" | "variation" | "variation3") {
                return self.err(p, format!("`{n}` is the entry's own, not a helper to call"));
            }
            let mut vals = Vec::new();
            for a in f.args.iter() {
                vals.push(self.arg(a, scope)?);
            }
            if def.params.len() != vals.len() {
                return self.err(p, format!("`{n}` takes {}", def.params.len()));
            }
            if self.depth > 32 {
                return self.err(p, "helpers call each other too deep (recursion?)");
            }
            let mut inner = Scope::new(Fn3::Helper, Vec::new());
            for (param, a) in def.params.iter().zip(vals) {
                match a {
                    Val::S(a) => inner.bind(param, Binding::Value(a)),
                    Val::V(v) => inner.bind(param, Binding::VecValue(Box::new(v))),
                }
            }
            self.depth += 1;
            let r = if self.pure { self.pure_vec_body(&def.body, &mut inner) } else { self.vec_value_block(&def.body, &mut inner).map(|(body, v)| {
                self.pending.extend(body);
                v
            }) };
            self.depth -= 1;
            return r;
        }
        let mut args = Vec::new();
        for a in f.args.iter() {
            args.push(self.arg(a, scope)?);
        }
        match self.vfunc(n, args, p)? {
            Val::V(v) => Ok(v),
            Val::S(_) => self.err(p, VECTOR_WANTED),
        }
    }

    /// The kind of vector the helper `name`, called with `args`, gives:
    /// its parameters bound by their arguments' kinds, its body read as
    /// [`Self::block_gives_vec`] reads one.
    fn helper_gives_vec(&self, name: &str, args: &[Expr], scope: &Scope) -> Option<Kind> {
        let def = self.fns.get(name)?;
        if def.params.len() != args.len() || self.probe.get() > 32 {
            return None;
        }
        let mut inner = Scope::new(Fn3::Helper, Vec::new());
        for (param, a) in def.params.iter().zip(args.iter()) {
            inner.bind(param, self.vec_kind(a, scope).map_or_else(num_marker, vec_marker));
        }
        self.probe.set(self.probe.get() + 1);
        let r = self.block_gives_vec(&def.body, &inner);
        self.probe.set(self.probe.get() - 1);
        r
    }

    /// The kind of vector a block ends in, if one: its `let`s bound by
    /// their values' kinds, in a scope of its own, then its last value
    /// read.
    pub(super) fn block_gives_vec(&self, b: &StmtBlock, scope: &Scope) -> Option<Kind> {
        let stmts = b.statements();
        let (last, rest) = stmts.split_last()?;
        let mut inner = scope.clone();
        inner.blocks.push(HashMap::new());
        for st in rest {
            if let Stmt::Var(x, _, _) = st {
                let (ident, value, _) = &**x;
                let marker = self.vec_kind(value, &inner).map_or_else(num_marker, vec_marker);
                inner.bind(&ident.name, marker);
            }
        }
        match final_value(last)? {
            Final::Expr(x) => self.vec_kind(x, &inner),
            Final::Call(f, _) => {
                let n = f.name.as_str();
                if self.fns.contains_key(n) {
                    self.helper_gives_vec(n, &f.args, &inner)
                } else if matches!(n, "+" | "-" | "*" | "/" | "**") {
                    f.args.iter().find_map(|a| self.vec_kind(a, &inner))
                } else {
                    self.call_kind(n, &f.args, &inner)
                }
            }
            Final::If(fc, _) => self.block_gives_vec(&fc.body, &inner),
        }
    }

    /// A block that ends in a vector: its statements (with what they left
    /// pending) and the vector, as [`Lowerer::value_block`] for numbers.
    fn vec_value_block(&mut self, b: &StmtBlock, scope: &mut Scope) -> R<(Vec<ast::Stmt>, Vector)> {
        let outer = std::mem::take(&mut self.pending);
        scope.blocks.push(HashMap::new());
        let r = (|| {
            let stmts = b.statements();
            let Some((last, rest)) = stmts.split_last() else { return self.err(b.position(), "each arm ends with its value") };
            let mut out = Vec::new();
            for st in rest {
                self.stmt(st, scope, &mut out)?;
            }
            let Some(f) = final_value(last) else { return self.err(last.position(), "each arm ends with its value") };
            let v = self.final_vec(f, scope)?;
            // Held, so a part read twice is not computed twice, and the
            // arm's statements are all before its value is taken.
            let v = self.hold_all(v, "part");
            self.flush(&mut out);
            Ok((out, v))
        })();
        scope.blocks.pop();
        self.pending = outer;
        r
    }

    /// A helper inlined into `escaped`: its `let`s substituted, then the
    /// vector.
    fn pure_vec_body(&mut self, b: &StmtBlock, scope: &mut Scope) -> R<Vector> {
        let stmts = b.statements();
        let Some((last, lets)) = stmts.split_last() else { return self.err(b.position(), "the function ends with its value") };
        scope.blocks.push(HashMap::new());
        let r = (|| {
            for st in lets {
                match st {
                    Stmt::Var(x, _, _) => {
                        let (ident, value, _) = &**x;
                        let v = self.arg(value, scope)?;
                        match v {
                            Val::V(v) => scope.bind(&ident.name, Binding::VecValue(Box::new(v))),
                            Val::S(s) => scope.bind(&ident.name, Binding::Value(s)),
                        }
                    }
                    Stmt::Noop(_) => {}
                    other => return self.err(other.position(), "`escaped` is one test: here only `let`s, then the value"),
                }
            }
            let Some(f) = final_value(last) else { return self.err(last.position(), "the function ends with its value") };
            self.final_vec(f, scope)
        })();
        scope.blocks.pop();
        r
    }

    /// A block's last value, as a vector.
    fn final_vec(&mut self, f: Final, scope: &mut Scope) -> R<Vector> {
        match f {
            Final::Expr(e) => self.vexpr(e, scope),
            Final::Call(c, p) => self.vcall(c, p, scope),
            Final::If(fc, p) => self.vec_if_value(fc, p, scope),
        }
    }

    /// An `if` as a vector value: a local per part set in each arm, its
    /// statements left pending -- [`Lowerer::if_value`] for vectors.
    fn vec_if_value(&mut self, fc: &FlowControl, p: Position, scope: &mut Scope) -> R<Vector> {
        if self.pure {
            return self.err(p, "`escaped` is one test: write the `if` as a comparison, or move it to `step`");
        }
        let span = self.pos.span(p);
        let mut arms: Vec<(Option<ast::Expr>, Vec<ast::Stmt>, Vector)> = Vec::new();
        let mut cur = fc;
        let mut conds_pending = Vec::new();
        loop {
            let cond = self.expr(&cur.expr, scope)?;
            if !arms.is_empty() && !self.pending.is_empty() {
                return self.err(cur.expr.position(), "an `else if` condition holds no `if` value or helper with statements");
            }
            conds_pending.append(&mut self.pending);
            let (body, value) = self.vec_value_block(&cur.body, scope)?;
            arms.push((Some(cond), body, value));
            match cur.branch.statements() {
                [Stmt::If(inner, _)] => cur = &**inner,
                [] => return self.err(p, "an `if` used as a value needs its `else`"),
                _ => {
                    let (body, value) = self.vec_value_block(&cur.branch, scope)?;
                    arms.push((None, body, value));
                    break;
                }
            }
        }
        let kind = arms[0].2.kind;
        if arms.iter().any(|a| a.2.kind != kind) {
            return self.err(p, "each arm of the `if` gives the same: a vector, or a quaternion");
        }
        let names = self.vec_local(kind, "if");
        self.pending.extend(conds_pending);
        for n in &names {
            self.pending.push(ast::Stmt::Decl { ty: TypeName::Float, name: n.clone(), init: None, dims: None, span });
        }
        let mut lowered = Vec::new();
        let mut otherwise = None;
        for (cond, mut body, value) in arms {
            for (n, c) in names.iter().zip(value.parts) {
                body.push(assign(mk(ExprKind::Name(n.clone()), span), c, span));
            }
            match cond {
                Some(c) => lowered.push((c, body)),
                None => otherwise = Some(body),
            }
        }
        self.pending.push(ast::Stmt::If { arms: lowered, otherwise, span });
        Ok(Vector::new(kind, names.into_iter().map(|n| mk(ExprKind::Name(n), span)).collect()))
    }

    /// `.rhs` of the vector `v`: a part, a method, or a chain of them.
    pub(super) fn member(&mut self, v: Vector, rhs: &Expr, scope: &mut Scope) -> R<Val> {
        match rhs {
            Expr::Property(prop, p) => {
                let name = prop.2.as_str();
                if v.kind == Kind::Quat && name == "v" {
                    return Ok(Val::V(Vector::new(Kind::Vec3, v.parts[1..].to_vec())));
                }
                match v.kind.axes().iter().position(|a| *a == name) {
                    Some(i) => Ok(Val::S(v.parts[i].clone())),
                    None => self.err(*p, format!("{}, not `.{name}`", v.kind.parts_are())),
                }
            }
            Expr::MethodCall(f, p) => {
                let mut args = vec![Val::V(v)];
                for a in f.args.iter() {
                    args.push(self.arg(a, scope)?);
                }
                self.vfunc(f.name.as_str(), args, *p)
            }
            Expr::Dot(be, _, _) => match self.member(v, &be.lhs, scope)? {
                Val::V(v) => self.member(v, &be.rhs, scope),
                Val::S(s) => Ok(Val::S(self.chain(Base::Value(s), &be.rhs, scope)?)),
            },
            other => self.err(other.position(), v.kind.parts_are()),
        }
    }

    /// A number used more than once, held in a temporary so it is
    /// computed once (a draw drawn once); as it is when it is a name or
    /// a constant, or in `escaped`, which has no statements.
    pub(super) fn hold(&mut self, e: ast::Expr, what: &str) -> ast::Expr {
        let trivial = match &e.kind {
            ExprKind::Name(_) | ExprKind::Param(_) | ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Bool(_) => true,
            ExprKind::Predef(n) => n != "draw",
            _ => false,
        };
        if trivial || self.pure {
            return e;
        }
        let ty = self.ty_of(&e);
        let name = self.local(what, ty);
        let span = e.span;
        self.pending.push(ast::Stmt::Decl { ty, name: name.clone(), init: Some(e), dims: None, span });
        mk(ExprKind::Name(name), span)
    }

    /// Every part of `v` held ([`Self::hold`]).
    fn hold_all(&mut self, v: Vector, what: &str) -> Vector {
        let kind = v.kind;
        let parts = v.parts.into_iter().map(|c| self.hold(c, what)).collect();
        Vector::new(kind, parts)
    }

    /// `min(a, b)` or `max(a, b)` of numbers: a temporary and an `if`.
    pub(super) fn min_max(&mut self, a: ast::Expr, b: ast::Expr, max: bool, p: Position) -> R<ast::Expr> {
        let name = if max { "max" } else { "min" };
        if self.pure {
            return self.err(p, format!("`{name}` sets a value: use it in `step`, not `escaped`"));
        }
        let span = self.pos.span(p);
        let ty = join(self.ty_of(&a), self.ty_of(&b));
        let b = self.hold(b, "bound");
        let t = self.local(name, ty);
        self.pending.push(ast::Stmt::Decl { ty, name: t.clone(), init: Some(a), dims: None, span });
        let cond = bin(if max { BinOp::Gt } else { BinOp::Lt }, b.clone(), mk(ExprKind::Name(t.clone()), span), span);
        self.pending.push(ast::Stmt::If { arms: vec![(cond, vec![assign(mk(ExprKind::Name(t.clone()), span), b, span)])], otherwise: None, span });
        Ok(mk(ExprKind::Name(t), span))
    }

    /// Hamilton's product of two quaternions, scalar part first.
    fn hamilton(&mut self, a: Vector, b: Vector, span: Span) -> Vector {
        let a = self.hold_all(a, "part").parts;
        let b = self.hold_all(b, "part").parts;
        let m = |i: usize, j: usize| bin(BinOp::Mul, a[i].clone(), b[j].clone(), span);
        let add = |x: ast::Expr, y: ast::Expr| bin(BinOp::Add, x, y, span);
        let sub = |x: ast::Expr, y: ast::Expr| bin(BinOp::Sub, x, y, span);
        Vector::new(
            Kind::Quat,
            vec![
                sub(sub(sub(m(0, 0), m(1, 1)), m(2, 2)), m(3, 3)),
                sub(add(add(m(0, 1), m(1, 0)), m(2, 3)), m(3, 2)),
                add(add(sub(m(0, 2), m(1, 3)), m(2, 0)), m(3, 1)),
                add(sub(add(m(0, 3), m(1, 2)), m(2, 1)), m(3, 0)),
            ],
        )
    }

    /// The complex number `t + i r` a quaternion's functions act in, with
    /// its parts held: `(t + i r, [x, y, z], r)`.
    fn plane_of(&mut self, q: Vector, span: Span) -> (ast::Expr, Vec<ast::Expr>, ast::Expr) {
        let parts = self.hold_all(q, "part").parts;
        let r = call1("sqrt", dot_of(&parts[1..], &parts[1..], span), span);
        let r = self.hold(r, "across");
        (mk(ExprKind::Complex(Box::new(parts[0].clone()), Box::new(r.clone())), span), parts[1..].to_vec(), r)
    }

    /// A quaternion's function `g`, of the complex `t + i r` carried back
    /// along the vector part: `g = a + i b` gives `a + b v / r`. For a
    /// function with real coefficients `b` is 0 where `r` is, so the
    /// division's floor never shows.
    fn through_plane(&mut self, q: Vector, g: impl FnOnce(ast::Expr) -> ast::Expr, span: Span) -> Vector {
        let (zc, v, r) = self.plane_of(q, span);
        let gz = self.hold(g(zc), "plane");
        let across = bin(BinOp::Div, call1("imag", gz.clone(), span), bin(BinOp::Add, r, mk(ExprKind::Float(1e-30), span), span), span);
        let across = self.hold(across, "across");
        let mut parts = vec![call1("real", gz, span)];
        parts.extend(v.into_iter().map(|c| bin(BinOp::Mul, c, across.clone(), span)));
        Vector::new(Kind::Quat, parts)
    }

    /// A function or operator with vectors among its arguments, or one of
    /// the functions that come with them (`min`, `max`, `clamp` of
    /// numbers).
    pub(super) fn vfunc(&mut self, name: &str, args: Vec<Val>, p: Position) -> R<Val> {
        use Val::{S, V};
        let span = self.pos.span(p);
        let each = |v: Vector, f: &dyn Fn(ast::Expr) -> ast::Expr| -> Vector { Vector::new(v.kind, v.parts.into_iter().map(f).collect()) };
        let zip = |a: Vector, b: Vector, op: BinOp| -> Vector {
            let kind = a.kind;
            Vector::new(kind, a.parts.into_iter().zip(b.parts).map(|(x, y)| bin(op, x, y, span)).collect())
        };
        let mut args = args;
        let first_kind = match args.first() {
            Some(V(v)) => Some(v.kind),
            _ => None,
        };
        // A quaternion's functions of its complex plane.
        if first_kind == Some(Kind::Quat) && args.len() == 1 && PLANE_FUNCTIONS.contains(&name) {
            let Some(V(q)) = args.pop() else { unreachable!("a quaternion") };
            // `ln` the natural logarithm, `log` Rhai's base 10, as for numbers.
            let f = name.to_string();
            let g = move |z: ast::Expr| match f.as_str() {
                "ln" => call1("log", z, span),
                "log" => bin(BinOp::Div, call1("log", z, span), mk(ExprKind::Float(std::f64::consts::LN_10), span), span),
                other => call1(other, z, span),
            };
            return Ok(V(self.through_plane(q, g, span)));
        }
        Ok(match (name, args.len()) {
            ("vec3", 3) => {
                let mut parts = Vec::new();
                for a in args {
                    match a {
                        S(s) => parts.push(s),
                        V(_) => return self.err(p, "`vec3(x, y, z)` takes three numbers"),
                    }
                }
                V(Vector::new(Kind::Vec3, parts))
            }
            ("vec3", 1) => match args.pop() {
                Some(S(s)) => {
                    let s = self.hold(s, "part");
                    V(Vector::new(Kind::Vec3, vec![s.clone(), s.clone(), s]))
                }
                _ => return self.err(p, "`vec3(s)` takes a number: every part `s`"),
            },
            ("quat", 4) => {
                let mut parts = Vec::new();
                for a in args {
                    match a {
                        S(s) => parts.push(s),
                        V(_) => return self.err(p, "`quat(w, x, y, z)` takes four numbers, or `quat(w, v)` a number and a vector"),
                    }
                }
                V(Vector::new(Kind::Quat, parts))
            }
            ("quat", 2) => match (args.remove(0), args.remove(0)) {
                (S(w), V(v)) if v.kind == Kind::Vec3 => {
                    let mut parts = vec![w];
                    parts.extend(v.parts);
                    V(Vector::new(Kind::Quat, parts))
                }
                _ => return self.err(p, "`quat(w, v)`: the scalar part, a number, and the vector part, a vector"),
            },
            ("quat", 1) => match args.pop() {
                Some(S(w)) => {
                    let zero = || mk(ExprKind::Float(0.0), span);
                    V(Vector::new(Kind::Quat, vec![w, zero(), zero(), zero()]))
                }
                _ => return self.err(p, "`quat(w)` takes a number: a quaternion with no vector part"),
            },
            ("+" | "-", 2) => {
                let op = if name == "+" { BinOp::Add } else { BinOp::Sub };
                match (args.remove(0), args.remove(0)) {
                    (V(a), V(b)) if a.kind == b.kind => V(zip(a, b, op)),
                    // A number to or from a quaternion's scalar part.
                    (V(mut q), S(s)) if q.kind == Kind::Quat => {
                        q.parts[0] = bin(op, q.parts[0].clone(), s, span);
                        V(q)
                    }
                    (S(s), V(q)) if q.kind == Kind::Quat => {
                        let mut q = if op == BinOp::Sub { each(q, &|x| neg(x, span)) } else { q };
                        q.parts[0] = bin(BinOp::Add, s, q.parts[0].clone(), span);
                        V(q)
                    }
                    (V(a), V(b)) => return self.err(p, format!("`{name}` between {} and {}", a.kind.what(), b.kind.what())),
                    _ => return self.err(p, format!("`{name}` adds or takes away vectors from vectors: a number to each part is `v + vec3(s)`")),
                }
            }
            ("-", 1) => match args.remove(0) {
                V(a) => V(each(a, &|x| neg(x, span))),
                S(_) => return self.err(p, VECTOR_WANTED),
            },
            ("*", 2) => match (args.remove(0), args.remove(0)) {
                (V(a), V(b)) if a.kind == Kind::Quat && b.kind == Kind::Quat => V(self.hamilton(a, b, span)),
                (V(a), V(b)) if a.kind == Kind::Vec3 && b.kind == Kind::Vec3 => V(zip(a, b, BinOp::Mul)),
                (V(a), V(b)) => return self.err(p, format!("`*` between {} and {}", a.kind.what(), b.kind.what())),
                (V(a), S(s)) => {
                    let s = self.hold(s, "factor");
                    V(each(a, &|x| bin(BinOp::Mul, x, s.clone(), span)))
                }
                (S(s), V(b)) => {
                    let s = self.hold(s, "factor");
                    V(each(b, &|x| bin(BinOp::Mul, s.clone(), x, span)))
                }
                (S(_), S(_)) => return self.err(p, VECTOR_WANTED),
            },
            ("/", 2) => match (args.remove(0), args.remove(0)) {
                (V(a), V(b)) if a.kind == Kind::Vec3 && b.kind == Kind::Vec3 => V(zip(a, b, BinOp::Div)),
                (V(a), V(b)) if a.kind == Kind::Quat && b.kind == Kind::Quat => {
                    return self.err(p, "`q / r` of quaternions is two things: `q * inv(r)` or `inv(r) * q`")
                }
                (V(a), V(b)) => return self.err(p, format!("`/` between {} and {}", a.kind.what(), b.kind.what())),
                (V(a), S(s)) => {
                    let s = self.hold(s, "factor");
                    V(each(a, &|x| bin(BinOp::Div, x, s.clone(), span)))
                }
                // A number over a quaternion: the number times its inverse.
                (S(s), V(q)) if q.kind == Kind::Quat => {
                    let inv = self.inverse(q, span);
                    let s = self.hold(s, "factor");
                    V(each(inv, &|x| bin(BinOp::Mul, s.clone(), x, span)))
                }
                (S(s), V(b)) => {
                    let s = self.hold(s, "factor");
                    V(each(b, &|x| bin(BinOp::Div, s.clone(), x, span)))
                }
                (S(_), S(_)) => return self.err(p, VECTOR_WANTED),
            },
            ("dot", 2) => match (args.remove(0), args.remove(0)) {
                (V(a), V(b)) if a.kind == b.kind => S(dot_of(&a.parts, &b.parts, span)),
                _ => return self.err(p, "`dot(a, b)` takes two vectors, or two quaternions"),
            },
            ("length", 1) => match args.remove(0) {
                V(a) => {
                    let a = self.hold_all(a, "part");
                    S(call1("sqrt", dot_of(&a.parts, &a.parts, span), span))
                }
                S(_) => return self.err(p, "`length(v)` of a vector; of a complex number, `z.abs()`"),
            },
            ("normalize", 1) => match args.remove(0) {
                V(a) => {
                    let a = self.hold_all(a, "part");
                    let len = call1("sqrt", dot_of(&a.parts, &a.parts, span), span);
                    let len = self.hold(len, "length");
                    V(each(a, &|c| bin(BinOp::Div, c, len.clone(), span)))
                }
                S(_) => return self.err(p, "`normalize(v)` takes a vector"),
            },
            ("cross", 2) => match (args.remove(0), args.remove(0)) {
                (V(a), V(b)) if a.kind == Kind::Vec3 && b.kind == Kind::Vec3 => {
                    let a = self.hold_all(a, "part").parts;
                    let b = self.hold_all(b, "part").parts;
                    let m = |i: usize, j: usize| bin(BinOp::Mul, a[i].clone(), b[j].clone(), span);
                    V(Vector::new(
                        Kind::Vec3,
                        vec![bin(BinOp::Sub, m(1, 2), m(2, 1), span), bin(BinOp::Sub, m(2, 0), m(0, 2), span), bin(BinOp::Sub, m(0, 1), m(1, 0), span)],
                    ))
                }
                _ => return self.err(p, "`cross(a, b)` takes two vectors"),
            },
            ("conj", 1) => match args.remove(0) {
                V(q) if q.kind == Kind::Quat => {
                    let mut q = q;
                    for c in q.parts.iter_mut().skip(1) {
                        *c = neg(c.clone(), span);
                    }
                    V(q)
                }
                _ => return self.err(p, "`conj(q)` of a quaternion (or of a complex number)"),
            },
            ("inv", 1) => match args.remove(0) {
                V(q) if q.kind == Kind::Quat => V(self.inverse(q, span)),
                _ => return self.err(p, "`inv(q)` of a quaternion: its conjugate over its squared length"),
            },
            ("pow" | "**", 2) => match (args.remove(0), args.remove(0)) {
                (V(q), S(n)) if q.kind == Kind::Quat => V(self.through_plane(q, |z| bin(BinOp::Pow, z, n, span), span)),
                _ => return self.err(p, "`pow(q, n)`: a quaternion to a number's power"),
            },
            ("complex", 1) => match args.remove(0) {
                V(q) if q.kind == Kind::Quat => S(self.plane_of(q, span).0),
                _ => return self.err(p, "`q.complex()` of a quaternion: `complex(t, r)`, its scalar part and its vector part's length"),
            },
            ("abs" | "floor", 1) => match args.remove(0) {
                V(a) if a.kind == Kind::Vec3 => V(each(a, &|x| call1(name, x, span))),
                V(_) => return self.err(p, format!("`{name}` of a quaternion: take its parts, or `length(q)`")),
                S(_) => return self.err(p, VECTOR_WANTED),
            },
            ("min" | "max", 2) => {
                let max = name == "max";
                match (args.remove(0), args.remove(0)) {
                    (S(a), S(b)) => S(self.min_max(a, b, max, p)?),
                    (V(a), V(b)) if a.kind == Kind::Vec3 && b.kind == Kind::Vec3 => {
                        let mut out = Vec::new();
                        for (x, y) in a.parts.into_iter().zip(b.parts) {
                            out.push(self.min_max(x, y, max, p)?);
                        }
                        V(Vector::new(Kind::Vec3, out))
                    }
                    (V(a), S(b)) if a.kind == Kind::Vec3 => {
                        let b = self.hold(b, "bound");
                        let mut out = Vec::new();
                        for x in a.parts {
                            out.push(self.min_max(x, b.clone(), max, p)?);
                        }
                        V(Vector::new(Kind::Vec3, out))
                    }
                    (S(_), V(_)) => return self.err(p, format!("`{name}(v, s)`: the vector first")),
                    _ => return self.err(p, format!("`{name}` of vectors; of a quaternion, take its parts")),
                }
            }
            ("clamp", 3) => {
                let (v, lo, hi) = (args.remove(0), args.remove(0), args.remove(0));
                let (S(lo), S(hi)) = (lo, hi) else { return self.err(p, "`clamp(x, lo, hi)`: the bounds are numbers") };
                let lo = self.hold(lo, "bound");
                let hi = self.hold(hi, "bound");
                match v {
                    S(x) => {
                        let m = self.min_max(x, lo, true, p)?;
                        S(self.min_max(m, hi, false, p)?)
                    }
                    V(v) if v.kind == Kind::Vec3 => {
                        let mut out = Vec::new();
                        for x in v.parts {
                            let m = self.min_max(x, lo.clone(), true, p)?;
                            out.push(self.min_max(m, hi.clone(), false, p)?);
                        }
                        V(Vector::new(Kind::Vec3, out))
                    }
                    V(_) => return self.err(p, "`clamp` of a vector; of a quaternion, take its parts"),
                }
            }
            (n, k) => {
                if matches!(n, "vec3" | "quat" | "dot" | "cross" | "length" | "normalize" | "min" | "max" | "clamp" | "abs" | "floor" | "conj" | "inv" | "pow" | "complex") {
                    return self.err(p, format!("`{n}` does not take {k} argument{}", if k == 1 { "" } else { "s" }));
                }
                if first_kind == Some(Kind::Quat) {
                    return self.err(p, format!("`{n}` takes numbers; a quaternion's functions are {}, `pow`, `conj` and `inv`", PLANE_FUNCTIONS.join(", ")));
                }
                return self.err(p, format!("`{n}` takes numbers; of a vector, take its parts (`v.x`) or `length(v)`"));
            }
        })
    }

    /// A quaternion's inverse: its conjugate over its squared length.
    fn inverse(&mut self, q: Vector, span: Span) -> Vector {
        let q = self.hold_all(q, "part");
        let norm = self.hold(dot_of(&q.parts, &q.parts, span), "norm");
        let parts = q
            .parts
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let c = if i == 0 { c } else { neg(c, span) };
                bin(BinOp::Div, c, norm.clone(), span)
            })
            .collect();
        Vector::new(Kind::Quat, parts)
    }

    /// `vary("name", v[, #{ param: value }])` in a solid: a built-in flame
    /// variation's 3D body applied to the vector `v` (3d-look-and-tools
    /// P5, C3). One `Stmt::Map`, the solid's next call, into three new
    /// float locals. A parameter's value is a number or one of the
    /// solid's float parameters (`p.name`), which reach the variation per
    /// picture; the rest are the variation's defaults. GPU only: not in
    /// `escaped` (one test, nothing before it), nor in `prepare`.
    pub(super) fn vary_map(&mut self, f: &FnCallExpr, p: Position, scope: &mut Scope) -> R<Vector> {
        use super::super::ir::{MapCall, MapValue};
        const FORM: &str = "`vary(\"name\", v)` or `vary(\"name\", v, #{ param: value })`: a built-in variation's 3D body applied to a vector";
        if !(2..=3).contains(&f.args.len()) {
            return self.err(p, FORM);
        }
        let Expr::StringConstant(vname, _) = &f.args[0] else { return self.err(p, FORM) };
        let vname = vname.to_string();
        if self.pure {
            return self.err(p, "`escaped` is one test: call the variation in `step` and keep what it gives in the state");
        }
        // A built-in with a 3D body.
        let known: Option<Vec<String>> = {
            let registry = crate::variations::global_registry();
            registry
                .get(&vname)
                .filter(|v| v.provenance != crate::provenance::Provenance::Custom && v.wgsl_source_3d.is_some())
                .map(|v| v.parameters.iter().map(|x| x.name.clone()).collect())
        };
        let Some(known) = known else {
            return self.err(
                p,
                format!("`{vname}` is not a built-in variation with a 3D body: a solid calls those on a vector (a variation of yours takes a point of the plane: `vary(\"{vname}\", complex(v.x, v.y))`)"),
            );
        };
        let mut params: Vec<(String, MapValue)> = Vec::new();
        if let Some(arg) = f.args.get(2) {
            let Expr::Map(m, _) = arg else { return self.err(arg.position(), "the parameters are a map, `#{ name: value }`") };
            for (k, v) in &m.0 {
                if !known.iter().any(|x| x == k.name.as_str()) {
                    let all: Vec<String> = known.iter().map(|x| format!("`{x}`")).collect();
                    let has = if all.is_empty() { "none".to_string() } else { all.join(", ") };
                    return self.err(k.pos, format!("`{vname}` has no parameter `{}`; it has {has}", k.name));
                }
                let number = |e: &Expr| match e {
                    Expr::FloatConstant(x, _) => Some(**x),
                    Expr::IntegerConstant(i, _) => Some(*i as f64),
                    _ => None,
                };
                let value = match v {
                    _ if number(v).is_some() => MapValue::Const(number(v).expect("a number")),
                    Expr::FnCall(fc, _) if fc.name.as_str() == "-" && fc.args.len() == 1 && number(&fc.args[0]).is_some() => {
                        MapValue::Const(-number(&fc.args[0]).expect("a number"))
                    }
                    Expr::Dot(be, _, _) if matches!(self.owner(&be.lhs, scope), Some(Owner::Params)) => {
                        let Expr::Property(prop, pp) = &be.rhs else {
                            return self.err(v.position(), "a solid's parameter, `p.name`");
                        };
                        let name = prop.2.to_string();
                        if !matches!(self.param_tys.get(name.as_str()), Some(TypeName::Float | TypeName::Int)) {
                            return self.err(*pp, format!("`{name}` is not one of the solid's number parameters (in `PARAMS`)"));
                        }
                        MapValue::Param(name)
                    }
                    _ => {
                        return self.err(
                            v.position(),
                            "a built-in variation's parameter is a number or one of the solid's parameters (`p.name`): it reaches the variation per picture, not per pixel",
                        )
                    }
                };
                params.push((k.name.to_string(), value));
            }
        }
        let input = self.vexpr(&f.args[1], scope)?;
        if input.kind != Kind::Vec3 {
            return self.err(f.args[1].position(), "a variation takes a vector: a quaternion's vector part is `q.v`");
        }
        let span = self.pos.span(p);
        let call = self.maps.len() as u32;
        self.maps.push(MapCall { variation: vname, params });
        let out = self.vec_local(Kind::Vec3, "vary");
        for n in &out {
            self.pending.push(ast::Stmt::Decl { ty: TypeName::Float, name: n.clone(), init: None, dims: None, span });
        }
        let input: [ast::Expr; 3] = input.parts.try_into().map_err(|_| Diag::error(span, "three parts"))?;
        let out3: [String; 3] = out.clone().try_into().map_err(|_| Diag::error(span, "three parts"))?;
        self.pending.push(ast::Stmt::Map { call, input: Box::new(input), out: out3, span });
        Ok(Vector::new(Kind::Vec3, out.into_iter().map(|n| mk(ExprKind::Name(n), span)).collect()))
    }

    /// A float local per part for the vector `written`.
    pub(super) fn vec_local(&mut self, kind: Kind, written: &str) -> Vec<String> {
        kind.axes()
            .iter()
            .map(|a| {
                let n = self.local(&format!("{written}_{a}"), TypeName::Float);
                if let Some(last) = self.names.last_mut() {
                    last.1 = format!("{written}.{a}");
                }
                n
            })
            .collect()
    }

    /// What a vector target names: the part of `whole` that `prop`
    /// names -- one part, or a quaternion's vector part, three.
    fn part_target(&self, whole: Vector, prop: &str, at: Position) -> R<Target> {
        if whole.kind == Kind::Quat && prop == "v" {
            return Ok(Target::Whole(Vector::new(Kind::Vec3, whole.parts[1..].to_vec())));
        }
        match whole.kind.axes().iter().position(|a| *a == prop) {
            Some(i) => Ok(Target::Part(whole.parts[i].clone())),
            None => self.err(at, whole.kind.parts_are()),
        }
    }

    /// An assignment to a vector or one of its parts -- a vector local, a
    /// vector field of the state (`s.z`, `this.p`), a part of either --
    /// or a new vector field in `start`. False when it is none of them.
    pub(super) fn vec_assign(&mut self, lhs: &Expr, rhs: &Expr, op: Option<&str>, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<bool> {
        let span = self.pos.span(lhs.position());
        // A vector local, whole or one part (`v = ...`, `v.x = ...`).
        let local = |e: &Expr| match e {
            Expr::Variable(v, _, _) => match scope.get(v.1.as_str()) {
                Some(Binding::VecLocal(kind, n)) => Some(Vector::new(*kind, n.iter().map(|n| mk(ExprKind::Name(n.clone()), span)).collect())),
                _ => None,
            },
            _ => None,
        };
        let target = match lhs {
            Expr::Variable(..) => match local(lhs) {
                Some(v) => Target::Whole(v),
                None => return Ok(false),
            },
            Expr::Dot(be, _, _) => match local(&be.lhs) {
                Some(v) => {
                    let Expr::Property(prop, pp) = &be.rhs else { return self.err(lhs.position(), "set a vector or one of its parts, `v.x = ...`") };
                    self.part_target(v, prop.2.as_str(), *pp)?
                }
                None => return self.vec_field_assign(lhs, rhs, op, scope, out),
            },
            _ => return Ok(false),
        };
        self.assign_to(target, rhs, op, span, scope, out)?;
        Ok(true)
    }

    /// A vector field of the state, or a part of one; or a new vector
    /// field in `start`.
    fn vec_field_assign(&mut self, lhs: &Expr, rhs: &Expr, op: Option<&str>, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<bool> {
        let Expr::Dot(be, _, _) = lhs else { return Ok(false) };
        if let Expr::ThisPtr(tp) = &be.lhs {
            self.this_state(*tp, scope)?;
        }
        let Some(Owner::State) = self.owner(&be.lhs, scope) else { return Ok(false) };
        let span = self.pos.span(lhs.position());
        let (first, rest) = match &be.rhs {
            Expr::Dot(b2, _, _) => (&b2.lhs, Some(&b2.rhs)),
            r => (r, None),
        };
        let Expr::Property(prop, pp) = first else { return Ok(false) };
        let f = prop.2.to_string();
        let Some(v) = self.named_vector(&Owner::State, &f, scope, self.pos.span(*pp)) else {
            // A new vector field, in `start`.
            if rest.is_none() && scope.which.builds() && op.is_none() && !scope.fields.contains(&f) && self.is_vec(rhs, scope) {
                let value = self.vexpr(rhs, scope)?;
                self.flush(out);
                let kind = value.kind;
                for (i, c) in value.parts.into_iter().enumerate() {
                    self.declare_field(&part(&f, kind, i), TypeName::Float, c, *pp, scope, out)?;
                }
                return Ok(true);
            }
            return Ok(false);
        };
        let target = match rest {
            None => Target::Whole(v),
            Some(Expr::Property(p2, pp2)) => self.part_target(v, p2.2.as_str(), *pp2)?,
            Some(other) => return self.err(other.position(), "set a vector field or one of its parts, `s.z.x = ...`"),
        };
        self.assign_to(target, rhs, op, span, scope, out)?;
        Ok(true)
    }

    /// `target = rhs` (or `target op= rhs`): a whole vector through
    /// temporaries, its parts read before any is set; or one part.
    fn assign_to(&mut self, target: Target, rhs: &Expr, op: Option<&str>, span: Span, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        let at = rhs.position();
        match target {
            Target::Whole(whole) => {
                let value = match op {
                    Some(syntax) => {
                        let r = self.arg(rhs, scope)?;
                        match self.vfunc(syntax, vec![Val::V(whole.clone()), r], at)? {
                            Val::V(v) => v,
                            Val::S(_) => return self.err(at, VECTOR_WANTED),
                        }
                    }
                    None => self.vexpr(rhs, scope)?,
                };
                if value.kind != whole.kind {
                    return self.err(at, format!("{} where {} is wanted", value.kind.what(), whole.kind.what()));
                }
                self.flush(out);
                let mut temps = Vec::new();
                for (i, c) in value.parts.into_iter().enumerate() {
                    let t = self.local(&format!("new_{}", whole.kind.axes()[i]), TypeName::Float);
                    out.push(ast::Stmt::Decl { ty: TypeName::Float, name: t.clone(), init: Some(c), dims: None, span });
                    temps.push(t);
                }
                for (target, t) in whole.parts.into_iter().zip(temps) {
                    out.push(assign(target, mk(ExprKind::Name(t), span), span));
                }
            }
            Target::Part(part) => {
                let mut v = self.expr(rhs, scope)?;
                if let Some(syntax) = op {
                    let bop = binop(syntax).ok_or_else(|| Diag::error(span, format!("`{syntax}=` is not a formula's")))?;
                    v = self.binary(bop, part.clone(), v, at)?;
                }
                self.flush(out);
                out.push(assign(part, v, span));
            }
        }
        Ok(())
    }

    /// A `PARAMS` entry whose value is `vec3(x, y, z)` or `quat(w, x, y,
    /// z)`: a float parameter per part, `name_x` ..., each with the
    /// entry's settings and its part in the caption.
    pub(super) fn vec_param(&mut self, name: &str, value: &FnCallExpr, settings: Vec<Setting>, span: Span, out: &mut Settings) -> R<()> {
        let kind = if value.name.as_str() == "quat" { Kind::Quat } else { Kind::Vec3 };
        if value.args.len() != kind.axes().len() {
            return Err(Diag::error(
                span,
                match kind {
                    Kind::Vec3 => "a vector parameter's value is `vec3(x, y, z)` of numbers",
                    Kind::Quat => "a quaternion parameter's value is `quat(w, x, y, z)` of numbers",
                },
            ));
        }
        for (i, a) in value.args.iter().enumerate() {
            let lit = self.literal(a)?;
            let lit = match lit.kind {
                ExprKind::Int(v) => mk(ExprKind::Float(v as f64), lit.span),
                ExprKind::Float(_) => lit,
                _ => return Err(Diag::error(span, "a vector parameter's parts are numbers")),
            };
            let mut s: Vec<Setting> = settings.iter().filter(|x| x.name != "caption").cloned().collect();
            let caption = settings
                .iter()
                .find(|x| x.name == "caption")
                .and_then(|x| match &x.value {
                    SettingValue::Strings(v) => v.first().cloned(),
                    _ => None,
                })
                .unwrap_or_else(|| name.to_string());
            let axis = kind.axes()[i];
            s.push(Setting { name: "caption".into(), value: SettingValue::Strings(vec![format!("{caption} ({axis})")]), span });
            s.push(Setting { name: "default".into(), value: SettingValue::Expr(lit), span });
            let pname = format!("{name}_{axis}");
            self.param_tys.insert(pname.clone(), TypeName::Float);
            out.params.push(ParamBlock { ty: Some(TypeName::Float), name: pname, settings: s, span });
        }
        self.param_vecs.insert(name.to_string(), kind);
        Ok(())
    }
}

/// What an assignment to a vector sets: the whole of one (or a
/// quaternion's vector part), or one part.
enum Target {
    Whole(Vector),
    Part(ast::Expr),
}

#[cfg(test)]
mod tests {
    use super::super::{program, with_variations};
    use crate::escape::lang::builtins::C64;
    use crate::escape::lang::interp::{run, Env, ParamValues};

    /// Every vector operation, in a formula that carries a vector in its
    /// state and takes a vector parameter.
    const VECTORS: &str = r#"
const PARAMS = #{ bailout: 16.0, axis: #{ value: vec3(0.3, -0.5, 0.8), caption: "Axis" }, k: 0.7 };
fn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel, w: vec3(pixel.re, pixel.im, 0.25) } }
fn step(s, p) {
    let a = normalize(p.axis);
    let v = cross(s.w, a) * p.k + abs(s.w) - vec3(0.5);
    v.z = v.z * 0.5;
    let m = clamp(v, -2.0, 2.0);
    s.w = m + a * dot(m, a) * 0.1;
    s.w.y += 0.01;
    let r = s.w.length();
    s.z = s.z * s.z + s.c + complex(s.w.x, s.w.y) * 0.1 / (1.0 + r) + max(r, 0.5) * 0.001;
    s
}
fn escaped(s, p) { s.z.norm_sqr() > p.bailout }
"#;

    /// The same arithmetic, written in numbers.
    const PARTS: &str = r#"
const PARAMS = #{ bailout: 16.0, ax: 0.3, ay: -0.5, az: 0.8, k: 0.7 };
fn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel, wx: pixel.re, wy: pixel.im, wz: 0.25 } }
fn step(s, p) {
    let l = sqrt(p.ax * p.ax + p.ay * p.ay + p.az * p.az);
    let ax = p.ax / l;
    let ay = p.ay / l;
    let az = p.az / l;
    let vx = (s.wy * az - s.wz * ay) * p.k + abs(s.wx) - 0.5;
    let vy = (s.wz * ax - s.wx * az) * p.k + abs(s.wy) - 0.5;
    let vz = (s.wx * ay - s.wy * ax) * p.k + abs(s.wz) - 0.5;
    vz = vz * 0.5;
    let mx = if vx < -2.0 { -2.0 } else if vx > 2.0 { 2.0 } else { vx };
    let my = if vy < -2.0 { -2.0 } else if vy > 2.0 { 2.0 } else { vy };
    let mz = if vz < -2.0 { -2.0 } else if vz > 2.0 { 2.0 } else { vz };
    let d = mx * ax + my * ay + mz * az;
    s.wx = mx + ax * d * 0.1;
    s.wy = my + ay * d * 0.1;
    s.wz = mz + az * d * 0.1;
    s.wy = s.wy + 0.01;
    let r = sqrt(s.wx * s.wx + s.wy * s.wy + s.wz * s.wz);
    let big = if r > 0.5 { r } else { 0.5 };
    s.z = s.z * s.z + s.c + complex(s.wx, s.wy) * 0.1 / (1.0 + r) + big * 0.001;
    s
}
fn escaped(s, p) { s.z.norm_sqr() > p.bailout }
"#;

    fn compiled(src: &str) -> crate::escape::lang::ir::Program {
        program(src).unwrap_or_else(|d| panic!("{:?}", d.iter().map(|x| x.render(src)).collect::<Vec<_>>()))
    }

    /// A formula written with vectors runs as the same arithmetic written
    /// in numbers, pixel by pixel; its vector parameter is three float
    /// parameters; and it compiles to WGSL that validates.
    #[test]
    fn vectors_run_as_their_parts_written_out() {
        let (v, n) = (compiled(VECTORS), compiled(PARTS));
        let names: Vec<&str> = v.params.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["bailout", "axis_x", "axis_y", "axis_z", "k"]);
        assert_eq!(v.params[1].caption, "Axis (x)");
        let (pv, pn) = (ParamValues::defaults(&v), ParamValues::defaults(&n));
        let mut escaped = 0;
        for i in 0..20 {
            for j in 0..20 {
                let px = C64::new(-2.2 + 3.0 * i as f64 / 19.0, -1.5 + 3.0 * j as f64 / 19.0);
                let (a, b) = (run(&v, &pv, None, &Env::at(px, 150)), run(&n, &pn, None, &Env::at(px, 150)));
                assert_eq!((a.n, a.escaped), (b.n, b.escaped), "at {px:?}");
                assert!((a.z.re - b.z.re).abs() < 1e-12 && (a.z.im - b.z.im).abs() < 1e-12, "at {px:?}: {:?} against {:?}", a.z, b.z);
                escaped += usize::from(a.escaped);
            }
        }
        assert!(escaped > 20 && escaped < 400, "the view holds both: {escaped}");
        #[cfg(not(target_arch = "wasm32"))]
        crate::escape::lang::wgsl::validate_formula(&crate::escape::lang::wgsl::formula(&v)).expect("valid WGSL");
    }

    /// What vectors refuse, with the reason.
    #[test]
    fn vectors_refuse_what_they_cannot_be() {
        let msg = |step: &str, escaped: &str| {
            let src = format!(
                "const PARAMS = #{{ bailout: 4.0 }};\nfn half(v) {{ v * 0.5 }}\nfn start(pixel, p) {{ #{{ z: complex(0.0, 0.0), c: pixel, w: vec3(1.0, 2.0, 3.0) }} }}\n\
                 fn step(s, p) {{ {step} s }}\nfn escaped(s, p) {{ {escaped} }}\n"
            );
            match with_variations(&[], || program(&src)) {
                Ok(_) => panic!("compiled: {step} / {escaped}"),
                Err(d) => d.iter().map(|x| x.message.clone()).collect::<Vec<_>>().join("; "),
            }
        };
        let ok_test = "s.z.norm_sqr() > p.bailout";
        assert!(msg("s.z = s.w;", ok_test).contains("a vector where a number is wanted"));
        assert!(msg("s.z = s.z + half(s.w);", ok_test).contains("a vector where a number is wanted"));
        // `escaped` is one test: a helper ending in an `if` is not one.
        assert!(msg("s.z = s.z * s.z + s.c;", "length(if s.z.re > 0.0 { s.w } else { -s.w }) > p.bailout").contains("`escaped` is one test"));
        assert!(msg("let u = if s.z.re > 0.0 { s.w };", ok_test).contains("needs its `else`"));
        assert!(msg("let u = vec3(1.0, 2.0);", ok_test).contains("does not take 2"));
        assert!(msg("let r = length(2.0);", ok_test).contains("of a complex number"));
        assert!(msg("s.w.q = 1.0;", ok_test).contains("`.x`, `.y` and `.z`"));
        assert!(msg("s.z = s.z + s.c;", "max(s.z.re, 1.0) > p.bailout").contains("not `escaped`"));
    }

    /// Helpers that give vectors -- with `let`s, ending in an `if`, calling
    /// one another, in `escaped` too -- `if`s and `else if`s as vector
    /// values, a function parameter of a vector, and a vector `prepare`
    /// gives, run as the same arithmetic written in numbers (B's leftovers).
    const VECTOR_VALUES: &str = r#"
const PARAMS = #{ bailout: 16.0, dir: #{ value: vec3(0.6, -0.3, 0.9), caption: "Direction" }, f: #{ function: "sin" } };
fn prepare(p) { #{ axis: normalize(p.dir), gain: length(p.dir) } }
fn reflect(v, n) {
    let d = dot(v, n);
    v - n * (2.0 * d)
}
fn folded(v, n) {
    let r = reflect(v, n);
    if dot(v, n) < 0.0 { r } else { v }
}
fn grow(v) { folded(v, vec3(1.0, 0.0, 0.0)) * 1.1 }
fn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel, w: vec3(pixel.re, pixel.im, 0.3) } }
fn step(s, p) {
    let a = folded(s.w, p.axis);
    let b = if a.x > 0.5 { grow(a) } else if a.y > 0.0 { a * 0.5 } else { -a };
    s.w = p.f(b) + vec3(0.0, 0.0, 0.1 * p.gain);
    s.z = s.z * s.z + s.c + complex(s.w.x, s.w.y) * 0.05;
    s
}
fn escaped(s, p) { s.z.norm_sqr() + length(reflect(s.w, vec3(1.0, 0.0, 0.0))) * 0.0 > p.bailout }
"#;

    /// The same arithmetic, written in numbers.
    const VECTOR_VALUES_PARTS: &str = r#"
const PARAMS = #{ bailout: 16.0, dx: 0.6, dy: -0.3, dz: 0.9, f: #{ function: "sin" } };
fn prepare(p) {
    let l = sqrt(p.dx * p.dx + p.dy * p.dy + p.dz * p.dz);
    #{ ax: p.dx / l, ay: p.dy / l, az: p.dz / l, gain: l }
}
fn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel, wx: pixel.re, wy: pixel.im, wz: 0.3 } }
fn step(s, p) {
    let d = s.wx * p.ax + s.wy * p.ay + s.wz * p.az;
    let ax = if d < 0.0 { s.wx - p.ax * (2.0 * d) } else { s.wx };
    let ay = if d < 0.0 { s.wy - p.ay * (2.0 * d) } else { s.wy };
    let az = if d < 0.0 { s.wz - p.az * (2.0 * d) } else { s.wz };
    let gx = if ax < 0.0 { ax - 2.0 * ax } else { ax };
    let bx = if ax > 0.5 { gx * 1.1 } else if ay > 0.0 { ax * 0.5 } else { -ax };
    let by = if ax > 0.5 { ay * 1.1 } else if ay > 0.0 { ay * 0.5 } else { -ay };
    let bz = if ax > 0.5 { az * 1.1 } else if ay > 0.0 { az * 0.5 } else { -az };
    s.wx = p.f(complex(bx, 0.0)).re;
    s.wy = p.f(complex(by, 0.0)).re;
    s.wz = p.f(complex(bz, 0.0)).re + 0.1 * p.gain;
    s.z = s.z * s.z + s.c + complex(s.wx, s.wy) * 0.05;
    s
}
fn escaped(s, p) { s.z.norm_sqr() > p.bailout }
"#;

    #[test]
    fn vector_values_run_as_their_parts_written_out() {
        let (v, n) = (compiled(VECTOR_VALUES), compiled(VECTOR_VALUES_PARTS));
        let (pv, pn) = (ParamValues::defaults(&v), ParamValues::defaults(&n));
        let mut escaped = 0;
        for i in 0..20 {
            for j in 0..20 {
                let px = C64::new(-2.2 + 3.0 * i as f64 / 19.0, -1.5 + 3.0 * j as f64 / 19.0);
                let (a, b) = (run(&v, &pv, None, &Env::at(px, 150)), run(&n, &pn, None, &Env::at(px, 150)));
                assert_eq!((a.n, a.escaped), (b.n, b.escaped), "at {px:?}");
                assert!((a.z.re - b.z.re).abs() < 1e-12 && (a.z.im - b.z.im).abs() < 1e-12, "at {px:?}: {:?} against {:?}", a.z, b.z);
                escaped += usize::from(a.escaped);
            }
        }
        assert!(escaped > 20 && escaped < 400, "the view holds both: {escaped}");
        #[cfg(not(target_arch = "wasm32"))]
        crate::escape::lang::wgsl::validate_formula(&crate::escape::lang::wgsl::formula(&v)).expect("valid WGSL");
    }

    /// A helper takes a vector; `escaped` reads a vector's length; a
    /// vector field is set whole, then by a part.
    #[test]
    fn vectors_go_where_numbers_go() {
        let src = r#"
const PARAMS = #{ bailout: 4.0 };
fn spin(v, t) { complex(v.x * cos(t) - v.y * sin(t), v.x * sin(t) + v.y * cos(t)) }
fn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel, w: vec3(pixel.re, pixel.im, 0.0) } }
fn step(s, p) {
    s.w = s.w * 0.9;
    s.w.z = s.w.z + 0.1;
    s.z = s.z * s.z + s.c + spin(s.w, 0.3) * 0.01;
    s
}
fn escaped(s, p) { s.z.norm_sqr() + length(s.w) * 0.0 > p.bailout }
"#;
        let p = compiled(src);
        let r = run(&p, &ParamValues::defaults(&p), None, &Env::at(C64::new(-0.5, 0.2), 100));
        assert!(!r.escaped, "inside the main cardioid, nearly: {r:?}");
        let r = run(&p, &ParamValues::defaults(&p), None, &Env::at(C64::new(1.5, 1.5), 100));
        assert!(r.escaped);
    }

    // ---- quaternions (docs/projects/quaternions.md) ----

    type Q = [f64; 4];

    /// Hamilton's product in its vector form: `(t t' - v.v', t v' + t' v +
    /// v x v')`, scalar part first.
    fn hamilton(a: Q, b: Q) -> Q {
        let (t, v, u, w) = (a[0], [a[1], a[2], a[3]], b[0], [b[1], b[2], b[3]]);
        let dot = v[0] * w[0] + v[1] * w[1] + v[2] * w[2];
        let cross = [v[1] * w[2] - v[2] * w[1], v[2] * w[0] - v[0] * w[2], v[0] * w[1] - v[1] * w[0]];
        [t * u - dot, t * w[0] + u * v[0] + cross[0], t * w[1] + u * v[1] + cross[1], t * w[2] + u * v[2] + cross[2]]
    }

    /// A function of a quaternion as Halayka (2009) writes them, eq. (17)
    /// to (21): a scalar part and a multiple of the vector part's
    /// direction, each of `t` and `r = |v|`.
    fn halayka(q: Q, f: impl Fn(f64, f64) -> (f64, f64)) -> Q {
        let r = (q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        let (a, b) = f(q[0], r);
        // On the scalar axis the vector part stays zero (b is zero there).
        let k = if r > 0.0 { b / r } else { 0.0 };
        [a, k * q[1], k * q[2], k * q[3]]
    }

    /// The same through the complex plane `t + i r`.
    fn plane(q: Q, g: impl Fn(C64) -> C64) -> Q {
        halayka(q, |t, r| {
            let z = g(C64::new(t, r));
            (z.re, z.im)
        })
    }

    fn inverse(q: Q) -> Q {
        let n = q.iter().map(|x| x * x).sum::<f64>();
        [q[0] / n, -q[1] / n, -q[2] / n, -q[3] / n]
    }

    /// The quaternion `op` gives at the pixel, run by the interpreter: two
    /// runs of a formula whose `z` is two of its parts. `q` is the state's
    /// quaternion, `r` a quaternion parameter.
    fn quat_at(op: &str, px: C64) -> Q {
        let parts = |a: &str, b: &str| {
            let src = format!(
                "const PARAMS = #{{ r: #{{ value: quat(0.3, 0.5, 0.4, 0.2), caption: \"R\" }} }};\n\
                 fn start(pixel, p) {{ #{{ z: complex(0.0, 0.0), q: quat(pixel.re, pixel.im, 0.35, -0.25) }} }}\n\
                 fn step(s, p) {{ let q = s.q; let r = p.r; let o = {op}; s.z = complex(o.{a}, o.{b}); s }}\n\
                 fn escaped(s, p) {{ s.z.norm_sqr() > 1.0e30 }}\n"
            );
            let p = compiled(&src);
            run(&p, &ParamValues::defaults(&p), None, &Env::at(px, 3)).z
        };
        let (wx, yz) = (parts("w", "x"), parts("y", "z"));
        [wx.re, wx.im, yz.re, yz.im]
    }

    /// Each quaternion operation, run by the interpreter, against the same
    /// algebra written independently in f64: Hamilton's product in its
    /// vector form, the functions as Halayka's equations give them, the
    /// rest through the complex plane they act in; and Halayka's own
    /// steps, `(1 + C) sin Z` and `inv(sinh Z) cosh Z`.
    #[test]
    fn quaternions_run_as_the_algebra_says() {
        use crate::escape::lang::builtins::Builtin;
        let r: Q = [0.3, 0.5, 0.4, 0.2];
        let sin = |q: Q| halayka(q, |t, r| (t.sin() * r.cosh(), t.cos() * r.sinh()));
        let cos = |q: Q| halayka(q, |t, r| (t.cos() * r.cosh(), -t.sin() * r.sinh()));
        let exp = |q: Q| halayka(q, |t, r| (t.exp() * r.cos(), t.exp() * r.sin()));
        let sinh = |q: Q| halayka(q, |t, r| (t.sinh() * r.cos(), t.cosh() * r.sin()));
        let cosh = |q: Q| halayka(q, |t, r| (t.cosh() * r.cos(), t.sinh() * r.sin()));
        let cases: Vec<(&str, Box<dyn Fn(Q) -> Q>)> = vec![
            ("q + r", Box::new(move |q: Q| [q[0] + r[0], q[1] + r[1], q[2] + r[2], q[3] + r[3]])),
            ("q - r * 2.0", Box::new(move |q: Q| [q[0] - 2.0 * r[0], q[1] - 2.0 * r[1], q[2] - 2.0 * r[2], q[3] - 2.0 * r[3]])),
            ("q * r", Box::new(move |q: Q| hamilton(q, r))),
            ("r * q", Box::new(move |q: Q| hamilton(r, q))),
            ("q + 2.5", Box::new(|q: Q| [q[0] + 2.5, q[1], q[2], q[3]])),
            ("1.0 - q", Box::new(|q: Q| [1.0 - q[0], -q[1], -q[2], -q[3]])),
            ("-q / 4.0", Box::new(|q: Q| q.map(|x| -x / 4.0))),
            ("2.0 / q", Box::new(|q: Q| inverse(q).map(|x| 2.0 * x))),
            ("conj(q)", Box::new(|q: Q| [q[0], -q[1], -q[2], -q[3]])),
            ("inv(q)", Box::new(inverse)),
            ("q.inv() * q", Box::new(|_| [1.0, 0.0, 0.0, 0.0])),
            ("normalize(q)", Box::new(|q: Q| {
                let n = q.iter().map(|x| x * x).sum::<f64>().sqrt();
                q.map(|x| x / n)
            })),
            ("sin(q)", Box::new(sin)),
            ("cos(q)", Box::new(cos)),
            ("exp(q)", Box::new(exp)),
            ("sinh(q)", Box::new(sinh)),
            ("q.cosh()", Box::new(cosh)),
            ("tan(q)", Box::new(|q: Q| plane(q, |z| Builtin::Tan.eval_c(z)))),
            ("tanh(q)", Box::new(|q: Q| plane(q, |z| Builtin::Tanh.eval_c(z)))),
            ("ln(q)", Box::new(|q: Q| plane(q, |z| z.ln()))),
            ("log(q)", Box::new(|q: Q| plane(q, |z| z.ln().scale(1.0 / std::f64::consts::LN_10)))),
            ("sqrt(q)", Box::new(|q: Q| plane(q, |z| z.sqrt()))),
            ("pow(q, 2.5)", Box::new(|q: Q| plane(q, |z| z.ln().scale(2.5).exp()))),
            // A power through the plane is the product's power.
            ("q ** 2.0", Box::new(|q: Q| hamilton(q, q))),
            ("pow(q, 3.0)", Box::new(|q: Q| hamilton(hamilton(q, q), q))),
            ("quat(q.w, q.v * 2.0)", Box::new(|q: Q| [q[0], 2.0 * q[1], 2.0 * q[2], 2.0 * q[3]])),
            ("quat(1.5)", Box::new(|_| [1.5, 0.0, 0.0, 0.0])),
            ("quat(0.0, 1.0, 0.0, 0.0) * quat(0.0, 0.0, 1.0, 0.0)", Box::new(|_| [0.0, 0.0, 0.0, 1.0])),
            ("quat(0.0, 0.0, 1.0, 0.0) * quat(0.0, 1.0, 0.0, 0.0)", Box::new(|_| [0.0, 0.0, 0.0, -1.0])),
            // Halayka's (2009) figures 3 and 8.
            ("sin(q) + r * sin(q)", Box::new(move |q: Q| {
                let s = sin(q);
                let rs = hamilton(r, s);
                [s[0] + rs[0], s[1] + rs[1], s[2] + rs[2], s[3] + rs[3]]
            })),
            ("r * (inv(sinh(q)) * cosh(q))", Box::new(move |q: Q| hamilton(r, hamilton(inverse(sinh(q)), cosh(q))))),
        ];
        for (op, want) in &cases {
            for px in [C64::new(0.4, -0.3), C64::new(-1.1, 0.6), C64::new(0.05, 1.3)] {
                let q: Q = [px.re, px.im, 0.35, -0.25];
                let (got, want) = (quat_at(op, px), want(q));
                let close = got.iter().zip(&want).all(|(a, b)| (a - b).abs() <= 1e-10 * (1.0 + b.abs()));
                assert!(close, "{op} at {q:?}: {got:?} against {want:?}");
            }
        }
    }

    /// A quaternion's numbers, its parameter, and its parts set: `dot`,
    /// `length` and `q.complex()`; a `PARAMS` quaternion is four float
    /// parameters; a state field's part and vector part set in place.
    #[test]
    fn quaternions_go_where_vectors_go() {
        let px = C64::new(0.4, -0.3);
        let q: Q = [0.4, -0.3, 0.35, -0.25];
        let r: Q = [0.3, 0.5, 0.4, 0.2];
        let scalar = |expr: &str| {
            let src = format!(
                "const PARAMS = #{{ r: #{{ value: quat(0.3, 0.5, 0.4, 0.2), caption: \"R\" }} }};\n\
                 fn start(pixel, p) {{ #{{ z: complex(0.0, 0.0), q: quat(pixel.re, pixel.im, 0.35, -0.25) }} }}\n\
                 fn step(s, p) {{ s.z = {expr}; s }}\nfn escaped(s, p) {{ s.z.norm_sqr() > 1.0e30 }}\n"
            );
            let p = compiled(&src);
            run(&p, &ParamValues::defaults(&p), None, &Env::at(px, 3)).z
        };
        let len = q.iter().map(|x| x * x).sum::<f64>().sqrt();
        let z = scalar("complex(dot(s.q, p.r), length(s.q))");
        assert!((z.re - q.iter().zip(&r).map(|(a, b)| a * b).sum::<f64>()).abs() < 1e-12 && (z.im - len).abs() < 1e-12, "{z:?}");
        let z = scalar("s.q.complex()");
        let vlen = (q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        assert!((z.re - q[0]).abs() < 1e-12 && (z.im - vlen).abs() < 1e-12, "{z:?}");

        let p = compiled(
            "const PARAMS = #{ c: #{ value: quat(0.3, 0.5, 0.4, 0.2), caption: \"C\" } };\n\
             fn start(pixel, p) { #{ z: complex(0.0, 0.0) } }\nfn step(s, p) { s.z = complex(p.c.w, p.c.z); s }\n\
             fn escaped(s, p) { s.z.norm_sqr() > 4.0 }\n",
        );
        let names: Vec<&str> = p.params.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["c_w", "c_x", "c_y", "c_z"]);
        assert_eq!(p.params[0].caption, "C (w)");

        // A part, then the vector part, set in the state.
        let src = "const PARAMS = #{ k: 2.0 };\n\
                   fn start(pixel, p) { #{ z: complex(0.0, 0.0), q: quat(pixel.re, pixel.im, 0.35, -0.25) } }\n\
                   fn step(s, p) { s.q.x = p.k; s.q.v = s.q.v * 0.5; s.q.w += 1.0; s.z = complex(s.q.w, s.q.x) + complex(s.q.y, s.q.z) * complex(0.0, 1.0) * 0.0; s }\n\
                   fn escaped(s, p) { s.z.norm_sqr() > 1.0e30 }\n";
        let p = compiled(src);
        let z = run(&p, &ParamValues::defaults(&p), None, &Env::at(px, 1)).z;
        assert!((z.re - (q[0] + 1.0)).abs() < 1e-12 && (z.im - 1.0).abs() < 1e-12, "{z:?}");
    }

    /// What quaternions refuse, with the reason.
    #[test]
    fn quaternions_refuse_what_they_cannot_be() {
        let msg = |step: &str| {
            let src = format!(
                "const PARAMS = #{{ bailout: 4.0 }};\nfn start(pixel, p) {{ #{{ z: complex(0.0, 0.0), c: pixel, q: quat(1.0, 2.0, 3.0, 4.0) }} }}\n\
                 fn step(s, p) {{ let q = s.q; {step} s }}\nfn escaped(s, p) {{ s.z.norm_sqr() > p.bailout }}\n"
            );
            match with_variations(&[], || program(&src)) {
                Ok(_) => panic!("compiled: {step}"),
                Err(d) => d.iter().map(|x| x.message.clone()).collect::<Vec<_>>().join("; "),
            }
        };
        assert!(msg("let o = q / q;").contains("two things"));
        assert!(msg("let o = q + vec3(1.0);").contains("between a quaternion and a vector"));
        assert!(msg("let o = abs(q);").contains("of a quaternion"));
        assert!(msg("s.z = complex(q.u, 0.0);").contains("a quaternion's parts"));
        assert!(msg("let o = q; o = vec3(1.0);").contains("a vector where a quaternion is wanted"));
        assert!(msg("let o = cross(q, q);").contains("two vectors"));
    }

    /// The quaternion Julia set `q^2 + c` as a Rhai solid, its estimate
    /// Hart's (the built-in's), run by the interpreter against the same
    /// in f64; and its WGSL validates.
    const QUAT_JULIA: &str = r#"
const PARAMS = #{
    c: #{ value: quat(-1.0, 0.2, 0.0, 0.0), caption: "C" },
    bailout: 2.0,
};
const INFO = #{ title: "Quaternion Julia (test)", radius: 2.0 };
fn start(pos, p) { #{ z: quat(pos.x, pos.y, pos.z, 0.0), dr: 1.0, out: false } }
fn step(s, p) {
    s.dr = 2.0 * length(s.z) * s.dr;
    s.z = s.z * s.z + p.c;
    if length(s.z) > p.bailout { s.out = true; }
    s
}
fn escaped(s, p) { length(s.z) > 1.0e4 }
fn distance(s, p) {
    if s.out {
        let m = length(s.z);
        0.5 * m * ln(m) / s.dr
    } else {
        0.0
    }
}
"#;

    #[test]
    fn a_quaternion_solid_runs_as_its_algebra() {
        use crate::escape::lang::interp;
        let p = crate::escape::lang::rhai::solid(QUAT_JULIA).unwrap_or_else(|d| panic!("{:?}", d.iter().map(|x| x.render(QUAT_JULIA)).collect::<Vec<_>>()));
        let base = crate::escape::ifs::template_parameters().len();
        let w = crate::escape::lang::wgsl::solid(&p, base, base + p.param_slots()).expect("WGSL");
        assert!(w.contains("length(vec4<f32>("), "the walk's escape reads the whole quaternion:\n{w}");
        #[cfg(not(target_arch = "wasm32"))]
        crate::escape::lang::wgsl::validate_solid(&w).expect("validates");
        let c: Q = [-1.0, 0.2, 0.0, 0.0];
        let params = ParamValues::defaults(&p);
        let mut outside = 0;
        for pos in [[0.3, 0.1, -0.2], [1.2, -0.4, 0.5], [0.0, 0.0, 0.0], [0.02, -0.03, 0.01], [-1.4, 0.2, -0.3], [0.9, 0.9, 0.9]] {
            let (mut q, mut dr, mut out) = ([pos[0], pos[1], pos[2], 0.0], 1.0, false);
            for _ in 0..32 {
                dr *= 2.0 * q.iter().map(|x| x * x).sum::<f64>().sqrt();
                let sq = hamilton(q, q);
                q = [sq[0] + c[0], sq[1] + c[1], sq[2] + c[2], sq[3] + c[3]];
                let m = q.iter().map(|x| x * x).sum::<f64>().sqrt();
                out |= m > 2.0;
                if m > 1.0e4 {
                    break;
                }
            }
            let m = q.iter().map(|x| x * x).sum::<f64>().sqrt();
            let want = if out { 0.5 * m * m.ln() / dr } else { 0.0 };
            let got = interp::solid(&p, &params, pos, 32);
            outside += usize::from(out);
            assert!((got - want).abs() <= 1e-9 * (1.0 + want.abs()), "at {pos:?}: {got} against {want}");
        }
        assert!(outside >= 2 && outside < 6, "the points hold both: {outside}");
    }

    /// A 3D variation computes with quaternions (Halayka's step, sliced
    /// to the point's space), runs as the algebra says, and its WGSL
    /// validates.
    #[test]
    fn a_3d_variation_computes_with_quaternions() {
        use crate::escape::lang::interp::VariationRun;
        let src = "const PARAMS = #{ c: #{ value: quat(0.3, 0.5, 0.4, 0.2), caption: \"C\" } };\n\
                   const INFO = #{ name: \"rhai_quat_step_test\" };\n\
                   fn variation3(v, p) {\n    let q = quat(v.x, v.y, v.z, 0.0);\n    let o = sin(q) + p.c * sin(q);\n    vec3(o.w, o.x, o.y)\n}\n";
        let dl = crate::variations::custom::compile(src).unwrap_or_else(|e| panic!("{e}"));
        assert!(dl.shader_3d.is_some());
        let p = crate::escape::lang::rhai::transformation3(src).unwrap();
        let params = ParamValues::defaults(&p);
        let c: Q = [0.3, 0.5, 0.4, 0.2];
        for v in [[0.3f64, -0.7, 0.2], [-1.2, 0.4, 0.9]] {
            let q: Q = [v[0], v[1], v[2], 0.0];
            let s = halayka(q, |t, r| (t.sin() * r.cosh(), t.cos() * r.sinh()));
            let cs = hamilton(c, s);
            let want = [s[0] + cs[0], s[1] + cs[1], s[2] + cs[2]];
            let got = VariationRun::new(&p, &params, 1).call3(v, [0.0; 3], 1.0).0;
            assert!(got.iter().zip(&want).all(|(a, b)| (a - b).abs() < 1e-10), "{got:?} against {want:?}");
        }
    }

    /// Halayka's (2009) six steps written in f64, each with its power and
    /// constant as the paper's figures have them: `(step, power, C, f)`.
    fn halayka_steps() -> Vec<(f64, f64, Q, Box<dyn Fn(Q) -> Q>)> {
        let c1: Q = [0.3, 0.5, 0.4, 0.2];
        let c2: Q = [1.0, 1.0, 1.0, 1.0];
        let add = |a: Q, b: Q| [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]];
        let sin = |q: Q| halayka(q, |t, r| (t.sin() * r.cosh(), t.cos() * r.sinh()));
        let cos = |q: Q| halayka(q, |t, r| (t.cos() * r.cosh(), -t.sin() * r.sinh()));
        let exp = |q: Q| halayka(q, |t, r| (t.exp() * r.cos(), t.exp() * r.sin()));
        let sinh = |q: Q| halayka(q, |t, r| (t.sinh() * r.cos(), t.cosh() * r.sin()));
        let cosh = |q: Q| halayka(q, |t, r| (t.cosh() * r.cos(), t.sinh() * r.sin()));
        vec![
            (0.0, 4.0, c1, Box::new(move |z: Q| {
                let z2 = hamilton(z, z);
                add(hamilton(z2, z2), c1)
            })),
            (1.0, 2.0, c1, Box::new(move |z: Q| add(sin(z), hamilton(c1, sin(z))))),
            (2.0, 2.0, c1, Box::new(move |z: Q| add(cos(z), hamilton(c1, cos(z))))),
            (3.0, 2.0, c1, Box::new(move |z: Q| add(exp(hamilton(z, z)), c1))),
            (4.0, 2.0, c2, Box::new(move |z: Q| add(inverse(sinh(z)), hamilton(c2, inverse(sinh(z)))))),
            (5.0, 2.0, c2, Box::new(move |z: Q| hamilton(c2, hamilton(inverse(sinh(z)), cosh(z))))),
        ]
    }

    /// Whether `pos` is in Halayka's set: 8 steps keep |Z| within 4, a
    /// step with no number (a singular point) leaving it, as the paper's
    /// lattice has it.
    fn in_halaykas_set(f: &dyn Fn(Q) -> Q, pos: [f64; 3]) -> bool {
        let mut z: Q = [pos[0], pos[1], pos[2], 0.0];
        (0..8).all(|_| {
            z = f(z);
            z.iter().map(|x| x * x).sum::<f64>().sqrt() <= 4.0
        })
    }

    /// The shipped solid, and its parameters at a step of `halayka_steps`
    /// and a `draw`.
    fn halayka_solid() -> crate::escape::lang::ir::Program {
        let src = include_str!("../../../../assets/formulas/solid-halayka-quaternions.rhai").replace("\r\n", "\n");
        crate::escape::lang::rhai::solid(&src).unwrap_or_else(|d| panic!("{:?}", d.iter().map(|x| x.render(&src)).collect::<Vec<_>>()))
    }

    fn halayka_params(p: &crate::escape::lang::ir::Program, step: f64, power: f64, c: Q, draw: usize) -> ParamValues {
        let mut params = ParamValues::defaults(p);
        let mut set = |name: &str, v: f64| {
            let i = p.params.iter().position(|x| x.name == name).unwrap_or_else(|| panic!("no parameter {name}"));
            params.values[i] = (v, 0.0);
        };
        set("step", step);
        set("power", power);
        for (k, v) in ["c_w", "c_x", "c_y", "c_z"].iter().zip(c) {
            set(k, v);
        }
        set("draw", draw as f64);
        params
    }

    /// The shipped solid of Halayka's sets (`solid-halayka-quaternions.rhai`)
    /// holds each lattice point the paper's definition does -- in the set
    /// while 8 steps keep |Z| within 4, Z = (x, y, z, 0) -- for each of its
    /// six steps, against the steps written in f64 here; its estimate is
    /// positive outside the set and none or less inside (the complement's
    /// the other way about, the surface's positive in the set and on the
    /// escaped side past its shell), and never more than the distance to a
    /// lattice point of the other kind.
    #[test]
    fn halaykas_sets_hold_what_the_paper_says() {
        use crate::escape::lang::interp;
        let p = halayka_solid();
        const N: usize = 9;
        let lattice: Vec<[f64; 3]> = (0..N * N * N).map(|i| [i % N, (i / N) % N, i / (N * N)].map(|k| -1.4 + 2.8 * k as f64 / (N - 1) as f64)).collect();
        for (step, power, c, f) in &halayka_steps() {
            let inside: Vec<bool> = lattice.iter().map(|pos| in_halaykas_set(f, *pos)).collect();
            let held = inside.iter().filter(|x| **x).count();
            assert!(held > 0 && held < lattice.len(), "step {step}: the lattice holds both ({held})");
            for draw in 0..3 {
                let params = halayka_params(&p, *step, *power, *c, draw);
                for (i, pos) in lattice.iter().enumerate() {
                    let d = interp::solid(&p, &params, *pos, 8);
                    assert!(d.is_finite(), "step {step}, draw {draw} at {pos:?}: {d}");
                    // The set, the complement, or the surface between them
                    // (signed: none or less in the solid); the surface a
                    // shell on its escaped side, so a point of the set is
                    // never in it, and one that escapes may be.
                    match draw {
                        0 => assert_eq!(d <= 0.0, inside[i], "step {step}, the set at {pos:?}: {d}"),
                        1 => assert_eq!(d <= 0.0, !inside[i], "step {step}, the complement at {pos:?}: {d}"),
                        _ => assert!(d > 0.0 || !inside[i], "step {step}, the surface at {pos:?}, in the set: {d}"),
                    }
                    if d > 0.0 {
                        let nearest = lattice
                            .iter()
                            .zip(&inside)
                            .filter(|(_, ins)| **ins != inside[i])
                            .map(|(q, _)| ((q[0] - pos[0]).powi(2) + (q[1] - pos[1]).powi(2) + (q[2] - pos[2]).powi(2)).sqrt())
                            .fold(f64::INFINITY, f64::min);
                        assert!(d <= nearest + 1e-9, "step {step}, draw {draw} at {pos:?}: {d} oversteps a point {nearest} away");
                    }
                }
            }
        }
    }

    /// The solid's march never passes the surface it is after: rays from
    /// points of the box, sphere-traced by its estimate, stop no later than
    /// the first change of membership sampling finds along them (a two
    /// hundredth of a unit apart), for each step and each way of drawing --
    /// within a step where they march through the set (a fortieth of the
    /// box: there is no bound inside) -- and stop only in the solid. The
    /// estimate's caps are what this holds to.
    #[test]
    fn halaykas_sets_are_never_stepped_through() {
        use crate::escape::lang::interp;
        let p = halayka_solid();
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        const SAMPLE: f64 = 0.005;
        for (step, power, c, f) in &halayka_steps() {
            for draw in 0..3 {
                let params = halayka_params(&p, *step, *power, *c, draw);
                // What the solid holds at a point of the box.
                let solid = |pos: [f64; 3]| {
                    let ins = in_halaykas_set(f.as_ref(), pos);
                    match draw {
                        0 => ins,
                        1 => !ins,
                        _ => false,
                    }
                };
                let mut traced = 0;
                for k in 0..48 {
                    // From a point of the box, in any direction; half of
                    // them near its middle, where the smaller sets are.
                    let reach = if k % 2 == 0 { 1.4 } else { 0.6 };
                    let o = [(next() * 2.0 - 1.0) * reach, (next() * 2.0 - 1.0) * reach, (next() * 2.0 - 1.0) * reach];
                    let mut d = [next() - 0.5, next() - 0.5, next() - 0.5];
                    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                    d = d.map(|x| x / len);
                    let at = |t: f64| [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
                    let start = in_halaykas_set(f.as_ref(), o);
                    if solid(o) {
                        continue;
                    }
                    // The first change: into the solid, or across the
                    // surface (draw 2), by sampling.
                    let inside_box = |q: [f64; 3]| q.iter().all(|x| x.abs() <= 1.5);
                    let mut first = f64::INFINITY;
                    let mut t = 0.0;
                    while inside_box(at(t)) {
                        let ins = in_halaykas_set(f.as_ref(), at(t));
                        if (draw == 2 && ins != start) || (draw != 2 && solid(at(t))) {
                            first = t;
                            break;
                        }
                        t += SAMPLE;
                    }
                    // The march, by the estimate, as the solid's
                    // `march: "inside"` has it: a hit only where the
                    // estimate is none, each step at least the pixel.
                    const PIXEL: f64 = 1e-4;
                    let mut t = 0.0;
                    let mut hit = f64::INFINITY;
                    for _ in 0..4000 {
                        let e = interp::solid(&p, &params, at(t), 8);
                        if e <= 0.0 {
                            hit = t;
                            break;
                        }
                        t += e.max(PIXEL);
                        if !inside_box(at(t)) {
                            break;
                        }
                    }
                    // Through the set (the complement, the surface), steps
                    // of a fortieth of the box; outside it, the reading.
                    let slack = if draw == 0 || !start { SAMPLE } else { SAMPLE + 0.025 * 1.5 };
                    assert!(hit <= first + slack, "step {step}, draw {draw}: from {o:?} along {d:?} the march stopped at {hit}, past the surface at {first}");
                    // And where it stops is the solid: the set, or the
                    // escaped side (the complement, the surface's shell).
                    if hit.is_finite() {
                        assert_eq!(in_halaykas_set(f.as_ref(), at(hit)), draw == 0, "step {step}, draw {draw}: from {o:?} along {d:?} the march stopped at {hit}, out of the solid");
                    }
                    traced += usize::from(first.is_finite());
                }
                assert!(traced > 2, "step {step}, draw {draw}: rays that meet the surface: {traced}");
            }
        }
    }

    /// The escape-path variation (`variation-escape-paths.rhai`) gives
    /// points on the Bezier paths of orbits that stay: on a coarse lattice
    /// and a short path, each point's start is the lattice point it lies
    /// by, that start is in the set, and the point is the path's at the t
    /// its colour gives (t = colour x Length), the orbit Z0 .. Z8 the
    /// control points -- against the same written in f64.
    #[test]
    fn the_escape_paths_lie_on_their_orbits_paths() {
        use crate::escape::lang::interp::VariationRun;
        let src = include_str!("../../../../assets/formulas/variation-escape-paths.rhai").replace("\r\n", "\n");
        let dl = crate::variations::custom::compile(&src).unwrap_or_else(|e| panic!("{e}"));
        assert!(dl.shader_3d.is_some() && dl.shader_2d.is_none());
        let p = crate::escape::lang::rhai::transformation3(&src).unwrap();
        let mut params = ParamValues::defaults(&p);
        let mut set = |name: &str, v: f64| {
            let i = p.params.iter().position(|x| x.name == name).unwrap_or_else(|| panic!("no parameter {name}"));
            params.values[i] = (v, 0.0);
        };
        let (spacing, length) = (0.5, 0.01);
        set("spacing", spacing);
        set("length", length);
        let c: Q = [0.3, 0.5, 0.4, 0.2];
        let step = |z: Q| {
            let sq = hamilton(z, z);
            [sq[0] + c[0], sq[1] + c[1], sq[2] + c[2], sq[3] + c[3]]
        };
        let mut run = VariationRun::new(&p, &params, 7);
        let mut on_paths = 0;
        for _ in 0..200 {
            let (got, color) = run.call3([0.0; 3], [0.0; 3], 1.0);
            let color = color.expect("a colour");
            if got[2] > 1.0e3 {
                continue;
            }
            assert!((0.0..=1.0).contains(&color), "{color}");
            let t = color * length;
            // The lattice point it started from.
            let a = got.map(|x| ((x / spacing).floor() + 0.5) * spacing);
            let mut z: Q = [a[0], a[1], a[2], 0.0];
            assert!(in_halaykas_set(&step, a), "a path from {a:?}, whose orbit escapes");
            let (n, u) = (8usize, 1.0 - t);
            let mut w = u.powi(n as i32);
            let mut sum = z.map(|x| x * w);
            for k in 0..n {
                z = step(z);
                w *= (n - k) as f64 / (k + 1) as f64 * t / u;
                for (s, x) in sum.iter_mut().zip(z) {
                    *s += x * w;
                }
            }
            let want = [sum[0], sum[1], sum[2]];
            assert!(got.iter().zip(&want).all(|(g, w)| (g - w).abs() < 1e-9), "{got:?} against {want:?} from {a:?} at t {t}");
            on_paths += 1;
        }
        assert!(on_paths > 100, "most draws find a start: {on_paths}");
    }
}
