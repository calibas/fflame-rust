//! Vectors in the Rhai dialect (the KIFS plan, B1): `vec3`, split into
//! three scalars by the lowering.
//!
//! A vector local is three float locals, a vector field of the state
//! three fields (`z__x`, `z__y`, `z__z`), a vector parameter three float
//! parameters (`c_x`, `c_y`, `c_z`), and every operation on vectors is
//! written out on the parts: `dot(a, b)` a sum of products, `normalize`
//! a division by the length. So the IR, the interpreter, the WGSL emitter
//! and the derivation passes see scalar code and need nothing of their
//! own -- a GPU thread runs scalar code anyway.
//!
//! The surface: `vec3(x, y, z)` (or `vec3(s)`, every part `s`); `v.x`,
//! `v.y`, `v.z`, read and set; `+`, `-`, unary `-`; `*` and `/` by a
//! number, or part by part between vectors; `dot`, `cross`, `length`,
//! `normalize`, `abs`, `floor`, `min`, `max`, `clamp`, as functions or as
//! methods. `min`, `max` and `clamp` of numbers come with them: a
//! temporary and an `if`, which the derivation passes take as any `if`.
//! A helper may give a vector, and an `if` may be one, each arm's value
//! three locals set in it; a function parameter of a vector (`p.f(v)`)
//! is taken part by part, each part the real part of the function of it
//! (the KIFS plan, B's leftovers).

use super::*;

/// The three parts of a vector, lowered.
pub(super) type V3 = [ast::Expr; 3];

pub(super) const AXES: [&str; 3] = ["x", "y", "z"];

pub(super) const NUMBER_WANTED: &str = "a vector where a number is wanted: take a part (`v.x`), `length(v)` or `dot(a, b)`";
const VECTOR_WANTED: &str = "a number where a vector is wanted: `vec3(x, y, z)`";

/// A lowered value: a vector's parts, or a number.
pub(super) enum Val {
    V(V3),
    S(ast::Expr),
}

/// What a `.` leads from, named without lowering it.
pub(super) enum Owner {
    State,
    Params,
    Ctx,
}

/// The name of a vector field's or parameter's part `i`.
pub(super) fn part(name: &str, i: usize) -> String {
    format!("{name}__{}", AXES[i])
}

/// What the vector probe binds a name it has not lowered to: a vector,
/// or a number. Only their kinds are read.
fn vec_marker() -> Binding {
    let z = || mk(ExprKind::Float(0.0), Span::default());
    Binding::VecValue(Box::new([z(), z(), z()]))
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
    /// is, when it is one.
    pub(super) fn named_vector(&self, owner: &Owner, name: &str, scope: &Scope, span: Span) -> Option<V3> {
        match owner {
            Owner::State => scope
                .fields
                .iter()
                .any(|f| *f == part(name, 0))
                .then(|| [0, 1, 2].map(|i| self.field_target(scope.which, &part(name, i), span))),
            Owner::Params => {
                if self.param_vecs.contains(name) {
                    Some(AXES.map(|a| mk(ExprKind::Param(format!("{name}_{a}")), span)))
                } else if self.prepared.contains_key(&part(name, 0)) {
                    // A vector `prepare` gave (B's leftovers).
                    Some([0, 1, 2].map(|i| mk(ExprKind::Name(prepared_name(&part(name, i))), span)))
                } else {
                    None
                }
            }
            // A 3D variation's accumulator (B2).
            Owner::Ctx => (self.three && name == "accum").then(|| {
                let accum = || mk(ExprKind::Predef("accum".into()), span);
                [call1("real", accum(), span), call1("imag", accum(), span), mk(ExprKind::Predef("accumz".into()), span)]
            }),
        }
    }

    /// Whether `e` is a vector, by its form and its names.
    pub(super) fn is_vec(&self, e: &Expr, scope: &Scope) -> bool {
        match e {
            Expr::Variable(v, _, _) => matches!(scope.get(v.1.as_str()), Some(Binding::VecLocal(_) | Binding::VecValue(_))),
            Expr::Dot(be, _, _) => {
                if self.is_vec(&be.lhs, scope) {
                    return member_is_vec(&be.rhs);
                }
                let Some(owner) = self.owner(&be.lhs, scope) else { return false };
                let (first, rest) = match &be.rhs {
                    Expr::Dot(b2, _, _) => (&b2.lhs, Some(&b2.rhs)),
                    r => (r, None),
                };
                match first {
                    Expr::Property(prop, _) if self.named_vector(&owner, prop.2.as_str(), scope, Span::default()).is_some() => {
                        rest.is_none_or(member_is_vec)
                    }
                    // `p.f(v)`: a function parameter of a vector.
                    Expr::MethodCall(f, _) if matches!(owner, Owner::Params) && rest.is_none() => {
                        f.args.len() == 1 && self.is_vec(&f.args[0], scope)
                    }
                    _ => false,
                }
            }
            Expr::FnCall(f, _) => {
                let n = f.name.as_str();
                if f.op_token.is_some() || is_operator(n) {
                    return matches!(n, "+" | "-" | "*" | "/") && f.args.iter().any(|a| self.is_vec(a, scope));
                }
                if self.fns.contains_key(n) {
                    return self.helper_gives_vec(n, &f.args, scope);
                }
                match n {
                    "vec3" | "cross" | "normalize" => true,
                    "abs" | "floor" | "min" | "max" | "clamp" => f.args.first().is_some_and(|a| self.is_vec(a, scope)),
                    _ => false,
                }
            }
            // An `if` as a value, by its first arm's.
            Expr::Stmt(b) => match b.statements() {
                [Stmt::If(fc, _)] => self.block_gives_vec(&fc.body, scope),
                _ => false,
            },
            _ => false,
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

    /// The vector `e`, as its three parts.
    pub(super) fn vexpr(&mut self, e: &Expr, scope: &mut Scope) -> R<V3> {
        let span = self.pos.span(e.position());
        match e {
            Expr::Variable(v, _, p) => match scope.get(v.1.as_str()).cloned() {
                Some(Binding::VecLocal(n)) => Ok(n.map(|n| mk(ExprKind::Name(n), span))),
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
                            let v = self.hold3(v, "part");
                            return Ok(v.map(|c| call1("real", mk(ExprKind::Call { name: name.clone(), args: vec![c], user: true }, span), span)));
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
    fn vcall(&mut self, f: &FnCallExpr, p: Position, scope: &mut Scope) -> R<V3> {
        let n = f.name.as_str();
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

    /// Whether the helper `name`, called with `args`, gives a vector: its
    /// parameters bound by their arguments' kinds, its body read as
    /// [`Self::block_gives_vec`] reads one.
    fn helper_gives_vec(&self, name: &str, args: &[Expr], scope: &Scope) -> bool {
        let Some(def) = self.fns.get(name) else { return false };
        if def.params.len() != args.len() || self.probe.get() > 32 {
            return false;
        }
        let mut inner = Scope::new(Fn3::Helper, Vec::new());
        for (param, a) in def.params.iter().zip(args.iter()) {
            inner.bind(param, if self.is_vec(a, scope) { vec_marker() } else { num_marker() });
        }
        self.probe.set(self.probe.get() + 1);
        let r = self.block_gives_vec(&def.body, &inner);
        self.probe.set(self.probe.get() - 1);
        r
    }

    /// Whether a block ends in a vector: its `let`s bound by their values'
    /// kinds, in a scope of its own, then its last value read.
    pub(super) fn block_gives_vec(&self, b: &StmtBlock, scope: &Scope) -> bool {
        let stmts = b.statements();
        let Some((last, rest)) = stmts.split_last() else { return false };
        let mut inner = scope.clone();
        inner.blocks.push(HashMap::new());
        for st in rest {
            if let Stmt::Var(x, _, _) = st {
                let (ident, value, _) = &**x;
                let marker = if self.is_vec(value, &inner) { vec_marker() } else { num_marker() };
                inner.bind(&ident.name, marker);
            }
        }
        match final_value(last) {
            Some(Final::Expr(x)) => self.is_vec(x, &inner),
            Some(Final::Call(f, _)) => {
                let n = f.name.as_str();
                if self.fns.contains_key(n) {
                    self.helper_gives_vec(n, &f.args, &inner)
                } else {
                    match n {
                        "vec3" | "cross" | "normalize" => true,
                        "abs" | "floor" | "min" | "max" | "clamp" | "+" | "-" | "*" | "/" => f.args.iter().any(|a| self.is_vec(a, &inner)),
                        _ => false,
                    }
                }
            }
            Some(Final::If(fc, _)) => self.block_gives_vec(&fc.body, &inner),
            None => false,
        }
    }

    /// A block that ends in a vector: its statements (with what they left
    /// pending) and the vector, as [`Lowerer::value_block`] for numbers.
    fn vec_value_block(&mut self, b: &StmtBlock, scope: &mut Scope) -> R<(Vec<ast::Stmt>, V3)> {
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
            let v = self.hold3(v, "part");
            self.flush(&mut out);
            Ok((out, v))
        })();
        scope.blocks.pop();
        self.pending = outer;
        r
    }

    /// A helper inlined into `escaped`: its `let`s substituted, then the
    /// vector.
    fn pure_vec_body(&mut self, b: &StmtBlock, scope: &mut Scope) -> R<V3> {
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
    fn final_vec(&mut self, f: Final, scope: &mut Scope) -> R<V3> {
        match f {
            Final::Expr(e) => self.vexpr(e, scope),
            Final::Call(c, p) => self.vcall(c, p, scope),
            Final::If(fc, p) => self.vec_if_value(fc, p, scope),
        }
    }

    /// An `if` as a vector value: three locals set in each arm, its
    /// statements left pending -- [`Lowerer::if_value`] for vectors.
    fn vec_if_value(&mut self, fc: &FlowControl, p: Position, scope: &mut Scope) -> R<V3> {
        if self.pure {
            return self.err(p, "`escaped` is one test: write the `if` as a comparison, or move it to `step`");
        }
        let span = self.pos.span(p);
        let mut arms: Vec<(Option<ast::Expr>, Vec<ast::Stmt>, V3)> = Vec::new();
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
        let names = self.vec_local("if");
        self.pending.extend(conds_pending);
        for n in &names {
            self.pending.push(ast::Stmt::Decl { ty: TypeName::Float, name: n.clone(), init: None, dims: None, span });
        }
        let mut lowered = Vec::new();
        let mut otherwise = None;
        for (cond, mut body, value) in arms {
            for (n, c) in names.iter().zip(value) {
                body.push(assign(mk(ExprKind::Name(n.clone()), span), c, span));
            }
            match cond {
                Some(c) => lowered.push((c, body)),
                None => otherwise = Some(body),
            }
        }
        self.pending.push(ast::Stmt::If { arms: lowered, otherwise, span });
        Ok(names.map(|n| mk(ExprKind::Name(n), span)))
    }

    /// `.rhs` of the vector `v`: a part, a method, or a chain of them.
    pub(super) fn member(&mut self, v: V3, rhs: &Expr, scope: &mut Scope) -> R<Val> {
        match rhs {
            Expr::Property(prop, p) => {
                let [x, y, z] = v;
                match prop.2.as_str() {
                    "x" => Ok(Val::S(x)),
                    "y" => Ok(Val::S(y)),
                    "z" => Ok(Val::S(z)),
                    o => self.err(*p, format!("a vector's parts are `.x`, `.y` and `.z`, not `.{o}`")),
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
            other => self.err(other.position(), "a vector's parts are `.x`, `.y` and `.z`"),
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

    fn hold3(&mut self, v: V3, what: &str) -> V3 {
        let [x, y, z] = v;
        [self.hold(x, what), self.hold(y, what), self.hold(z, what)]
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

    /// A function or operator with vectors among its arguments, or one of
    /// the functions that come with them (`min`, `max`, `clamp` of
    /// numbers).
    pub(super) fn vfunc(&mut self, name: &str, args: Vec<Val>, p: Position) -> R<Val> {
        use Val::{S, V};
        let span = self.pos.span(p);
        let each = |v: V3, f: &dyn Fn(ast::Expr) -> ast::Expr| -> V3 {
            let [x, y, z] = v;
            [f(x), f(y), f(z)]
        };
        let zip = |a: V3, b: V3, op: BinOp| -> V3 {
            let [ax, ay, az] = a;
            let [bx, by, bz] = b;
            [bin(op, ax, bx, span), bin(op, ay, by, span), bin(op, az, bz, span)]
        };
        let mut args = args;
        Ok(match (name, args.len()) {
            ("vec3", 3) => {
                let mut parts = Vec::new();
                for a in args {
                    match a {
                        S(s) => parts.push(s),
                        V(_) => return self.err(p, "`vec3(x, y, z)` takes three numbers"),
                    }
                }
                let [x, y, z]: [ast::Expr; 3] = parts.try_into().map_err(|_| Diag::error(span, "three parts"))?;
                V([x, y, z])
            }
            ("vec3", 1) => match args.pop() {
                Some(S(s)) => {
                    let s = self.hold(s, "part");
                    V([s.clone(), s.clone(), s])
                }
                _ => return self.err(p, "`vec3(s)` takes a number: every part `s`"),
            },
            ("+" | "-", 2) => match (args.remove(0), args.remove(0)) {
                (V(a), V(b)) => V(zip(a, b, if name == "+" { BinOp::Add } else { BinOp::Sub })),
                _ => return self.err(p, format!("`{name}` adds or takes away vectors from vectors: a number to each part is `v + vec3(s)`")),
            },
            ("-", 1) => match args.remove(0) {
                V(a) => V(each(a, &|x| mk(ExprKind::Unary(UnOp::Neg, Box::new(x)), span))),
                S(_) => return self.err(p, VECTOR_WANTED),
            },
            ("*" | "/", 2) => {
                let op = if name == "*" { BinOp::Mul } else { BinOp::Div };
                match (args.remove(0), args.remove(0)) {
                    (V(a), V(b)) => V(zip(a, b, op)),
                    (V(a), S(s)) => {
                        let s = self.hold(s, "factor");
                        V(each(a, &|x| bin(op, x, s.clone(), span)))
                    }
                    (S(s), V(b)) => {
                        let s = self.hold(s, "factor");
                        V(each(b, &|x| bin(op, s.clone(), x, span)))
                    }
                    (S(_), S(_)) => return self.err(p, VECTOR_WANTED),
                }
            }
            ("dot", 2) => match (args.remove(0), args.remove(0)) {
                (V([ax, ay, az]), V([bx, by, bz])) => {
                    S(bin(BinOp::Add, bin(BinOp::Add, bin(BinOp::Mul, ax, bx, span), bin(BinOp::Mul, ay, by, span), span), bin(BinOp::Mul, az, bz, span), span))
                }
                _ => return self.err(p, "`dot(a, b)` takes two vectors"),
            },
            ("length", 1) => match args.remove(0) {
                V(a) => {
                    let [x, y, z] = self.hold3(a, "part");
                    let sq = |c: ast::Expr| bin(BinOp::Mul, c.clone(), c, span);
                    S(call1("sqrt", bin(BinOp::Add, bin(BinOp::Add, sq(x), sq(y), span), sq(z), span), span))
                }
                S(_) => return self.err(p, "`length(v)` of a vector; of a complex number, `z.abs()`"),
            },
            ("normalize", 1) => match args.remove(0) {
                V(a) => {
                    let [x, y, z] = self.hold3(a, "part");
                    let sq = |c: &ast::Expr| bin(BinOp::Mul, c.clone(), c.clone(), span);
                    let len = call1("sqrt", bin(BinOp::Add, bin(BinOp::Add, sq(&x), sq(&y), span), sq(&z), span), span);
                    let len = self.hold(len, "length");
                    V([x, y, z].map(|c| bin(BinOp::Div, c, len.clone(), span)))
                }
                S(_) => return self.err(p, "`normalize(v)` takes a vector"),
            },
            ("cross", 2) => match (args.remove(0), args.remove(0)) {
                (V(a), V(b)) => {
                    let [ax, ay, az] = self.hold3(a, "part");
                    let [bx, by, bz] = self.hold3(b, "part");
                    let m = |a: &ast::Expr, b: &ast::Expr| bin(BinOp::Mul, a.clone(), b.clone(), span);
                    V([
                        bin(BinOp::Sub, m(&ay, &bz), m(&az, &by), span),
                        bin(BinOp::Sub, m(&az, &bx), m(&ax, &bz), span),
                        bin(BinOp::Sub, m(&ax, &by), m(&ay, &bx), span),
                    ])
                }
                _ => return self.err(p, "`cross(a, b)` takes two vectors"),
            },
            ("abs" | "floor", 1) => match args.remove(0) {
                V(a) => V(each(a, &|x| call1(name, x, span))),
                S(_) => return self.err(p, VECTOR_WANTED),
            },
            ("min" | "max", 2) => {
                let max = name == "max";
                match (args.remove(0), args.remove(0)) {
                    (S(a), S(b)) => S(self.min_max(a, b, max, p)?),
                    (V([ax, ay, az]), V([bx, by, bz])) => V([self.min_max(ax, bx, max, p)?, self.min_max(ay, by, max, p)?, self.min_max(az, bz, max, p)?]),
                    (V([ax, ay, az]), S(b)) => {
                        let b = self.hold(b, "bound");
                        V([self.min_max(ax, b.clone(), max, p)?, self.min_max(ay, b.clone(), max, p)?, self.min_max(az, b, max, p)?])
                    }
                    (S(_), V(_)) => return self.err(p, format!("`{name}(v, s)`: the vector first")),
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
                    V(parts) => {
                        let mut out = Vec::new();
                        for x in parts {
                            let m = self.min_max(x, lo.clone(), true, p)?;
                            out.push(self.min_max(m, hi.clone(), false, p)?);
                        }
                        let [x, y, z]: [ast::Expr; 3] = out.try_into().map_err(|_| Diag::error(span, "three parts"))?;
                        V([x, y, z])
                    }
                }
            }
            (n, k) => {
                if matches!(n, "vec3" | "dot" | "cross" | "length" | "normalize" | "min" | "max" | "clamp" | "abs" | "floor") {
                    return self.err(p, format!("`{n}` does not take {k} argument{}", if k == 1 { "" } else { "s" }));
                }
                return self.err(p, format!("`{n}` takes numbers; of a vector, take its parts (`v.x`) or `length(v)`"));
            }
        })
    }

    /// Three float locals for the vector `written`.
    pub(super) fn vec_local(&mut self, written: &str) -> [String; 3] {
        AXES.map(|a| {
            let n = self.local(&format!("{written}_{a}"), TypeName::Float);
            if let Some(last) = self.names.last_mut() {
                last.1 = format!("{written}.{a}");
            }
            n
        })
    }

    /// An assignment to a vector or one of its parts -- a vector local, a
    /// vector field of the state (`s.z`, `this.p`), a part of either --
    /// or a new vector field in `start`. False when it is none of them.
    pub(super) fn vec_assign(&mut self, lhs: &Expr, rhs: &Expr, op: Option<&str>, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<bool> {
        let span = self.pos.span(lhs.position());
        // A vector local, whole or one part (`v = ...`, `v.x = ...`).
        let local = |e: &Expr| match e {
            Expr::Variable(v, _, _) => match scope.get(v.1.as_str()) {
                Some(Binding::VecLocal(n)) => Some(n.clone()),
                _ => None,
            },
            _ => None,
        };
        let target: Result<[ast::Expr; 3], ast::Expr> = match lhs {
            Expr::Variable(..) => match local(lhs) {
                Some(n) => Ok(n.map(|n| mk(ExprKind::Name(n), span))),
                None => return Ok(false),
            },
            Expr::Dot(be, _, _) => match local(&be.lhs) {
                Some(n) => {
                    let Expr::Property(prop, pp) = &be.rhs else { return self.err(lhs.position(), "set a vector or one of its parts, `v.x = ...`") };
                    let Some(i) = AXES.iter().position(|a| *a == prop.2.as_str()) else {
                        return self.err(*pp, "a vector's parts are `.x`, `.y` and `.z`");
                    };
                    Err(mk(ExprKind::Name(n[i].clone()), span))
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
                let parts = self.vexpr(rhs, scope)?;
                self.flush(out);
                for (i, c) in parts.into_iter().enumerate() {
                    self.declare_field(&part(&f, i), TypeName::Float, c, *pp, scope, out)?;
                }
                return Ok(true);
            }
            return Ok(false);
        };
        let target = match rest {
            None => Ok(v),
            Some(Expr::Property(p2, pp2)) => match AXES.iter().position(|a| *a == p2.2.as_str()) {
                Some(i) => {
                    let [x, y, z] = v;
                    Err([x, y, z].into_iter().nth(i).expect("an axis"))
                }
                None => return self.err(*pp2, "a vector's parts are `.x`, `.y` and `.z`"),
            },
            Some(other) => return self.err(other.position(), "set a vector field or one of its parts, `s.z.x = ...`"),
        };
        self.assign_to(target, rhs, op, span, scope, out)?;
        Ok(true)
    }

    /// `target = rhs` (or `target op= rhs`): a whole vector through
    /// temporaries, its parts read before any is set; or one part.
    fn assign_to(&mut self, target: Result<[ast::Expr; 3], ast::Expr>, rhs: &Expr, op: Option<&str>, span: Span, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        let at = rhs.position();
        match target {
            Ok(whole) => {
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
                self.flush(out);
                let mut temps = Vec::new();
                for (i, c) in value.into_iter().enumerate() {
                    let t = self.local(&format!("new_{}", AXES[i]), TypeName::Float);
                    out.push(ast::Stmt::Decl { ty: TypeName::Float, name: t.clone(), init: Some(c), dims: None, span });
                    temps.push(t);
                }
                for (target, t) in whole.into_iter().zip(temps) {
                    out.push(assign(target, mk(ExprKind::Name(t), span), span));
                }
            }
            Err(part) => {
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

    /// A `PARAMS` entry whose value is `vec3(x, y, z)`: three float
    /// parameters, `name_x`, `name_y` and `name_z`, each with the entry's
    /// settings and its axis in the caption.
    pub(super) fn vec_param(&mut self, name: &str, value: &FnCallExpr, settings: Vec<Setting>, span: Span, out: &mut Settings) -> R<()> {
        if value.args.len() != 3 {
            return Err(Diag::error(span, "a vector parameter's value is `vec3(x, y, z)` of numbers"));
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
            s.push(Setting { name: "caption".into(), value: SettingValue::Strings(vec![format!("{caption} ({})", AXES[i])]), span });
            s.push(Setting { name: "default".into(), value: SettingValue::Expr(lit), span });
            let pname = format!("{name}_{}", AXES[i]);
            self.param_tys.insert(pname.clone(), TypeName::Float);
            out.params.push(ParamBlock { ty: Some(TypeName::Float), name: pname, settings: s, span });
        }
        self.param_vecs.insert(name.to_string());
        Ok(())
    }
}

/// Whether `.rhs` of a vector gives a vector (`.abs()`) or a number (`.x`,
/// `.length()`; anything after a number is a number's).
fn member_is_vec(rhs: &Expr) -> bool {
    match rhs {
        Expr::MethodCall(f, _) => matches!(f.name.as_str(), "abs" | "floor" | "normalize" | "cross" | "min" | "max" | "clamp"),
        Expr::Dot(be, _, _) => member_is_vec(&be.lhs) && member_is_vec(&be.rhs),
        _ => false,
    }
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
}
