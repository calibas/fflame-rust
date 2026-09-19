//! One number, three ways: the scalar trait the IFS kernels are
//! written over.
//!
//! `docs/projects/ifs-general.md` D1 and D2. Three of this month's
//! bugs were in hand derivations of the same kernel at a different
//! type -- Bubble's σ_min 21× too large, Bubble's inverse 102% wrong
//! from a cancellation, `big_kernel_inverse` conjugating on the wrong
//! sign -- and all three lived in code that exists three times: f64,
//! [`BigFloat`](crate::escape::bigfloat::BigFloat), WGSL. Writing a
//! kernel ONCE over a trait removes two of those three copies, and
//! the derivative stops being a separate derivation at all.
//!
//! - [`Real`] is what the algebraic kernels need: the four
//!   operations and a square root.
//! - [`Transcendental`] adds `exp`, `ln`, `sin`, `cos` and `atan2`,
//!   which the root, the disc and the blob need. `BigFloat` has `ln`
//!   and `atan2` and lacks the other three, which is why it
//!   implements only [`Real`] today -- the delta plan's rung 3 is
//!   what finishes it, and until then its walk keeps its own
//!   transcription for those kernels.
//! - [`Dual`] carries a value and its gradient in two variables, so
//!   a Jacobian is the same function evaluated at `Dual<f64>` and a
//!   Hessian is the same function at `Dual<Dual<f64>>`. No finite
//!   difference, no step size, and exact at the singularities where
//!   a central difference is least accurate.
//!
//! **Why `lit` takes `&self`.** A `BigFloat` carries its own limb
//! count, so there is no such thing as "the constant 2" without a
//! precision to build it at. Every constant in a kernel therefore
//! comes from a value already in hand, which is the only way the
//! generic body can stay precision-agnostic.

use std::cmp::Ordering;

/// The four operations and a square root, over whatever number.
///
/// Implemented by `f64`, by [`Dual`] of anything that implements it,
/// and by `BigFloat` (in `escape::bigfloat`, so the trait stays
/// where the analysis can see it and the implementation stays with
/// the type).
pub trait Real: Clone {
    /// A constant at this value's precision. See the module note.
    fn lit(&self, v: f64) -> Self;
    /// The value, as a machine float. For a [`Dual`] this is the
    /// value alone -- the gradient is not part of it -- which is what
    /// makes every branch decision in a kernel body read the point
    /// and not the derivative.
    fn to_f64(&self) -> f64;

    fn add(&self, o: &Self) -> Self;
    fn sub(&self, o: &Self) -> Self;
    fn mul(&self, o: &Self) -> Self;
    fn div(&self, o: &Self) -> Self;
    fn neg(&self) -> Self;
    fn sqrt(&self) -> Self;

    /// Ordering against a machine float. The default reads
    /// [`Self::to_f64`], which is exact for `f64` and for a `Dual`
    /// over it; `BigFloat` overrides it, since a value far outside
    /// f64's range compares fine and converts to an infinity.
    fn cmp_f64(&self, v: f64) -> Ordering {
        self.to_f64().partial_cmp(&v).unwrap_or(Ordering::Greater)
    }

    fn zero(&self) -> Self {
        self.lit(0.0)
    }
    fn one(&self) -> Self {
        self.lit(1.0)
    }
    fn sqr(&self) -> Self {
        self.mul(self)
    }
    fn recip(&self) -> Self {
        self.one().div(self)
    }
    fn abs(&self) -> Self {
        if self.to_f64() < 0.0 {
            self.neg()
        } else {
            self.clone()
        }
    }
    /// `|(self, o)|²` -- the form every planar kernel wants, and the
    /// one place a fused square-sum could be substituted later.
    fn hypot2(&self, o: &Self) -> Self {
        self.sqr().add(&o.sqr())
    }
    fn hypot(&self, o: &Self) -> Self {
        self.hypot2(o).sqrt()
    }
    fn is_finite(&self) -> bool {
        self.to_f64().is_finite()
    }
}

/// What the root, the disc and the blob need on top of [`Real`].
///
/// `powf` is provided as `exp(e·ln b)` rather than required, because
/// no implementation has a better one and a wrong exponent rule at
/// `b = 0` is exactly the kind of thing that differs between three
/// copies.
pub trait Transcendental: Real {
    fn exp(&self) -> Self;
    fn ln(&self) -> Self;
    fn sin(&self) -> Self;
    fn cos(&self) -> Self;
    /// `atan2(y, x)`, the full-circle angle, with IEEE's answer at
    /// the four signed-zero pairs.
    fn atan2(y: &Self, x: &Self) -> Self;

    /// `self^e`, for `self ≥ 0`. Zero to a positive power is zero,
    /// which `exp(e·ln 0)` would give as `exp(−∞)` on a machine float
    /// and as garbage on anything without an infinity.
    fn powf(&self, e: &Self) -> Self {
        let b = self.to_f64();
        if b == 0.0 {
            let ev = e.to_f64();
            return if ev > 0.0 {
                self.zero()
            } else if ev == 0.0 {
                self.one()
            } else {
                self.lit(f64::INFINITY)
            };
        }
        e.mul(&self.ln()).exp()
    }

    /// `(sin, cos)` together, since every kernel that wants one wants
    /// both and an implementation may share the range reduction.
    fn sin_cos(&self) -> (Self, Self) {
        (self.sin(), self.cos())
    }
}

impl Real for f64 {
    fn lit(&self, v: f64) -> Self {
        v
    }
    fn to_f64(&self) -> f64 {
        *self
    }
    fn add(&self, o: &Self) -> Self {
        self + o
    }
    fn sub(&self, o: &Self) -> Self {
        self - o
    }
    fn mul(&self, o: &Self) -> Self {
        self * o
    }
    fn div(&self, o: &Self) -> Self {
        self / o
    }
    fn neg(&self) -> Self {
        -self
    }
    fn sqrt(&self) -> Self {
        f64::sqrt(*self)
    }
    fn abs(&self) -> Self {
        f64::abs(*self)
    }
    fn recip(&self) -> Self {
        f64::recip(*self)
    }
    fn hypot(&self, o: &Self) -> Self {
        f64::hypot(*self, *o)
    }
    fn is_finite(&self) -> bool {
        f64::is_finite(*self)
    }
}

/// `f32`, for the arithmetic the SHADER runs.
///
/// `Real` only: the kernels that need a transcendental are the ones
/// the shader writes out by hand in WGSL, and a `Transcendental` impl
/// here would invite a CPU body that has no WGSL twin. What this is
/// for is checking an f32 expression's conditioning against its own
/// f64 value -- `the_difference_forms_survive_f32` -- which needs
/// only the four operations and a square root.
impl Real for f32 {
    fn lit(&self, v: f64) -> Self {
        v as f32
    }
    fn to_f64(&self) -> f64 {
        *self as f64
    }
    fn add(&self, o: &Self) -> Self {
        self + o
    }
    fn sub(&self, o: &Self) -> Self {
        self - o
    }
    fn mul(&self, o: &Self) -> Self {
        self * o
    }
    fn div(&self, o: &Self) -> Self {
        self / o
    }
    fn neg(&self) -> Self {
        -self
    }
    fn sqrt(&self) -> Self {
        f32::sqrt(*self)
    }
    fn abs(&self) -> Self {
        f32::abs(*self)
    }
    fn recip(&self) -> Self {
        f32::recip(*self)
    }
    fn is_finite(&self) -> bool {
        f32::is_finite(*self)
    }
}

impl Transcendental for f64 {
    fn exp(&self) -> Self {
        f64::exp(*self)
    }
    fn ln(&self) -> Self {
        f64::ln(*self)
    }
    fn sin(&self) -> Self {
        f64::sin(*self)
    }
    fn cos(&self) -> Self {
        f64::cos(*self)
    }
    fn atan2(y: &Self, x: &Self) -> Self {
        f64::atan2(*y, *x)
    }
    fn powf(&self, e: &Self) -> Self {
        f64::powf(*self, *e)
    }
    fn sin_cos(&self) -> (Self, Self) {
        f64::sin_cos(*self)
    }
}

/// A value and its gradient in two variables.
///
/// Forward-mode automatic differentiation, two seeds wide because
/// the plane is two-dimensional: seed a point with
/// [`Dual::seed2`] and every function of it comes back carrying its
/// own Jacobian row. `Dual<Dual<T>>` nests, and the second
/// derivative is then `value.d[i].d[j]` -- see [`hessian2`].
///
/// The gradient is carried as `T`, not `f64`, so a `Dual<BigFloat>`
/// differentiates at full precision.
#[derive(Clone, Debug)]
pub struct Dual<T> {
    /// The value.
    pub v: T,
    /// `∂v/∂x` and `∂v/∂y`.
    pub d: [T; 2],
}

impl<T: Real> Dual<T> {
    /// A constant: no dependence on either variable.
    pub fn constant(v: T) -> Self {
        let z = v.zero();
        Dual { d: [z.clone(), z], v }
    }

    /// The `i`th variable, at `v`.
    pub fn variable(v: T, i: usize) -> Self {
        let z = v.zero();
        let o = v.one();
        let mut d = [z.clone(), z];
        d[i] = o;
        Dual { v, d }
    }

    /// A planar point seeded so that `d[0]` is `∂/∂x` and `d[1]` is
    /// `∂/∂y`.
    pub fn seed2(p: [T; 2]) -> [Self; 2] {
        let [x, y] = p;
        [Dual::variable(x, 0), Dual::variable(y, 1)]
    }
}

impl<T: Real> Real for Dual<T> {
    fn lit(&self, v: f64) -> Self {
        Dual::constant(self.v.lit(v))
    }
    fn to_f64(&self) -> f64 {
        self.v.to_f64()
    }
    fn add(&self, o: &Self) -> Self {
        Dual {
            v: self.v.add(&o.v),
            d: [self.d[0].add(&o.d[0]), self.d[1].add(&o.d[1])],
        }
    }
    fn sub(&self, o: &Self) -> Self {
        Dual {
            v: self.v.sub(&o.v),
            d: [self.d[0].sub(&o.d[0]), self.d[1].sub(&o.d[1])],
        }
    }
    fn mul(&self, o: &Self) -> Self {
        // (ab)' = a'b + ab'
        Dual {
            v: self.v.mul(&o.v),
            d: [
                self.d[0].mul(&o.v).add(&self.v.mul(&o.d[0])),
                self.d[1].mul(&o.v).add(&self.v.mul(&o.d[1])),
            ],
        }
    }
    fn div(&self, o: &Self) -> Self {
        // (a/b)' = (a'b − ab')/b², written as (a' − (a/b)·b')/b so
        // the quotient is formed once and no square overflows.
        let q = self.v.div(&o.v);
        Dual {
            d: [
                self.d[0].sub(&q.mul(&o.d[0])).div(&o.v),
                self.d[1].sub(&q.mul(&o.d[1])).div(&o.v),
            ],
            v: q,
        }
    }
    fn neg(&self) -> Self {
        Dual { v: self.v.neg(), d: [self.d[0].neg(), self.d[1].neg()] }
    }
    fn sqrt(&self) -> Self {
        // (√a)' = a'/(2√a)
        let r = self.v.sqrt();
        let two_r = r.add(&r);
        Dual { d: [self.d[0].div(&two_r), self.d[1].div(&two_r)], v: r }
    }
    fn cmp_f64(&self, v: f64) -> Ordering {
        self.v.cmp_f64(v)
    }
    fn is_finite(&self) -> bool {
        self.v.is_finite() && self.d[0].is_finite() && self.d[1].is_finite()
    }
}

impl<T: Transcendental> Transcendental for Dual<T> {
    fn exp(&self) -> Self {
        let e = self.v.exp();
        Dual { d: [self.d[0].mul(&e), self.d[1].mul(&e)], v: e }
    }
    fn ln(&self) -> Self {
        Dual {
            v: self.v.ln(),
            d: [self.d[0].div(&self.v), self.d[1].div(&self.v)],
        }
    }
    fn sin(&self) -> Self {
        let (s, c) = self.v.sin_cos();
        Dual { v: s, d: [self.d[0].mul(&c), self.d[1].mul(&c)] }
    }
    fn cos(&self) -> Self {
        let (s, c) = self.v.sin_cos();
        let ns = s.neg();
        Dual { v: c, d: [self.d[0].mul(&ns), self.d[1].mul(&ns)] }
    }
    fn atan2(y: &Self, x: &Self) -> Self {
        // d atan2(y, x) = (x·dy − y·dx)/(x² + y²)
        let den = x.v.hypot2(&y.v);
        let g = |i: usize| {
            x.v.mul(&y.d[i]).sub(&y.v.mul(&x.d[i])).div(&den)
        };
        Dual { v: T::atan2(&y.v, &x.v), d: [g(0), g(1)] }
    }
    fn sin_cos(&self) -> (Self, Self) {
        let (s, c) = self.v.sin_cos();
        let ns = s.neg();
        (
            Dual { d: [self.d[0].mul(&c), self.d[1].mul(&c)], v: s },
            Dual { d: [self.d[0].mul(&ns), self.d[1].mul(&ns)], v: c.clone() },
        )
    }
}

// ------------------------------------------------ complex arithmetic

// The planar kernels are complex maps, and their DIFFERENCE forms
// (`ifs-perturbation-delta.md` §3) are written in complex algebra.
// These are the operations those forms need, over any `Real`, so one
// body serves f64, `Dual` and `BigFloat`.

/// `a + b`.
pub fn cadd<T: Real>(a: &[T; 2], b: &[T; 2]) -> [T; 2] {
    [a[0].add(&b[0]), a[1].add(&b[1])]
}

/// `a − b`.
pub fn csub<T: Real>(a: &[T; 2], b: &[T; 2]) -> [T; 2] {
    [a[0].sub(&b[0]), a[1].sub(&b[1])]
}

/// `a · b`.
pub fn cmul<T: Real>(a: &[T; 2], b: &[T; 2]) -> [T; 2] {
    [
        a[0].mul(&b[0]).sub(&a[1].mul(&b[1])),
        a[0].mul(&b[1]).add(&a[1].mul(&b[0])),
    ]
}

/// `a · s`, a real scale.
pub fn cscale<T: Real>(a: &[T; 2], s: &T) -> [T; 2] {
    [a[0].mul(s), a[1].mul(s)]
}

/// `a / s`, a real divisor.
pub fn cdiv_real<T: Real>(a: &[T; 2], s: &T) -> [T; 2] {
    [a[0].div(s), a[1].div(s)]
}

/// `conj(a)`.
pub fn cconj<T: Real>(a: &[T; 2]) -> [T; 2] {
    [a[0].clone(), a[1].neg()]
}

/// `|a|²`.
pub fn cnorm2<T: Real>(a: &[T; 2]) -> T {
    a[0].hypot2(&a[1])
}

/// Zero, at `like`'s precision.
pub fn czero<T: Real>(like: &T) -> [T; 2] {
    [like.zero(), like.zero()]
}

/// `a^n`, by repeated multiplication. `n = 0` is one.
pub fn cpow<T: Real>(a: &[T; 2], n: u32) -> [T; 2] {
    let mut out = [a[0].one(), a[0].zero()];
    for _ in 0..n {
        out = cmul(&out, a);
    }
    out
}

/// `(Z + δ)^n − Z^n`, **without forming the difference**.
///
/// `w^n − z^n = (w − z)·Σ_{j<n} w^j z^{n−1−j}`, and `w − z` is `δ`
/// exactly. Every term of the sum is `O(1)` and the whole product is
/// `O(δ)`, so nothing cancels -- which is the point: the direct form
/// loses every digit of `δ` below `|Z^n|`'s last one, and at a deep
/// zoom that is all of them.
///
/// `w` is passed rather than recomputed so the caller's `Z + δ` is
/// the one used: at `BigFloat` it is exact, and forming it twice
/// would be two roundings instead of one.
pub fn cpow_delta<T: Real>(z: &[T; 2], w: &[T; 2], d: &[T; 2], n: u32) -> [T; 2] {
    if n == 0 {
        return czero(&z[0]);
    }
    let mut sum = czero(&z[0]);
    for j in 0..n {
        sum = cadd(&sum, &cmul(&cpow(w, j), &cpow(z, n - 1 - j)));
    }
    cmul(d, &sum)
}

/// The Jacobian of a planar map, as `[[∂u/∂x, ∂u/∂y], [∂v/∂x, ∂v/∂y]]`.
///
/// `f` is the map written once over [`Real`]; this calls it at
/// `Dual<T>` and reads the gradients off the result. `None` when any
/// component is not finite, which is how a kernel's pole and the
/// edge of its image report themselves without a separate rule.
/// A [`Dual`] with THREE derivative slots, for a map of three
/// variables.
///
/// [`Dual`] carries two because the plane needs two. The solid needs
/// three, and the alternative to a second type was making the slot
/// count a const generic -- which every caller would then have to
/// name, in a codebase where all but one of them is planar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dual3<T> {
    pub v: T,
    pub d: [T; 3],
}

impl<T: Real> Dual3<T> {
    pub fn constant(v: T) -> Self {
        let z = v.zero();
        Self { d: [z.clone(), z.clone(), z], v }
    }

    /// `v`, differentiated with respect to slot `k`.
    pub fn variable(v: T, k: usize) -> Self {
        let mut out = Self::constant(v);
        out.d[k] = out.v.one();
        out
    }
}

impl<T: Real> Real for Dual3<T> {
    fn lit(&self, v: f64) -> Self {
        Self::constant(self.v.lit(v))
    }
    fn to_f64(&self) -> f64 {
        self.v.to_f64()
    }
    fn add(&self, o: &Self) -> Self {
        Self {
            v: self.v.add(&o.v),
            d: [self.d[0].add(&o.d[0]), self.d[1].add(&o.d[1]), self.d[2].add(&o.d[2])],
        }
    }
    fn sub(&self, o: &Self) -> Self {
        Self {
            v: self.v.sub(&o.v),
            d: [self.d[0].sub(&o.d[0]), self.d[1].sub(&o.d[1]), self.d[2].sub(&o.d[2])],
        }
    }
    fn mul(&self, o: &Self) -> Self {
        Self {
            v: self.v.mul(&o.v),
            d: [
                self.d[0].mul(&o.v).add(&self.v.mul(&o.d[0])),
                self.d[1].mul(&o.v).add(&self.v.mul(&o.d[1])),
                self.d[2].mul(&o.v).add(&self.v.mul(&o.d[2])),
            ],
        }
    }
    fn div(&self, o: &Self) -> Self {
        let inv = o.v.recip();
        let inv2 = inv.mul(&inv);
        Self {
            v: self.v.mul(&inv),
            d: [
                self.d[0].mul(&o.v).sub(&self.v.mul(&o.d[0])).mul(&inv2),
                self.d[1].mul(&o.v).sub(&self.v.mul(&o.d[1])).mul(&inv2),
                self.d[2].mul(&o.v).sub(&self.v.mul(&o.d[2])).mul(&inv2),
            ],
        }
    }
    fn neg(&self) -> Self {
        Self {
            v: self.v.neg(),
            d: [self.d[0].neg(), self.d[1].neg(), self.d[2].neg()],
        }
    }
    fn sqrt(&self) -> Self {
        let r = self.v.sqrt();
        // `1/(2√v)`, and at v = 0 the derivative is infinite -- which
        // the caller reads as "no Jacobian here", the same answer the
        // plane's `Dual` gives.
        let k = r.add(&r).recip();
        Self {
            v: r,
            d: [self.d[0].mul(&k), self.d[1].mul(&k), self.d[2].mul(&k)],
        }
    }
    fn cmp_f64(&self, v: f64) -> std::cmp::Ordering {
        self.v.cmp_f64(v)
    }
    fn is_finite(&self) -> bool {
        self.v.is_finite() && self.d.iter().all(|x| x.is_finite())
    }
}

impl<T: Transcendental> Transcendental for Dual3<T> {
    fn exp(&self) -> Self {
        let e = self.v.exp();
        Self {
            v: e.clone(),
            d: [self.d[0].mul(&e), self.d[1].mul(&e), self.d[2].mul(&e)],
        }
    }
    fn ln(&self) -> Self {
        let inv = self.v.recip();
        Self {
            v: self.v.ln(),
            d: [self.d[0].mul(&inv), self.d[1].mul(&inv), self.d[2].mul(&inv)],
        }
    }
    fn sin(&self) -> Self {
        let (s, c) = self.v.sin_cos();
        Self {
            v: s,
            d: [self.d[0].mul(&c), self.d[1].mul(&c), self.d[2].mul(&c)],
        }
    }
    fn cos(&self) -> Self {
        let (s, c) = self.v.sin_cos();
        let m = s.neg();
        Self {
            v: c,
            d: [self.d[0].mul(&m), self.d[1].mul(&m), self.d[2].mul(&m)],
        }
    }
    fn atan2(y: &Self, x: &Self) -> Self {
        // d atan2(y, x) = (x dy − y dx) / (x² + y²).
        let den = x.v.mul(&x.v).add(&y.v.mul(&y.v)).recip();
        Self {
            v: T::atan2(&y.v, &x.v),
            d: [
                x.v.mul(&y.d[0]).sub(&y.v.mul(&x.d[0])).mul(&den),
                x.v.mul(&y.d[1]).sub(&y.v.mul(&x.d[1])).mul(&den),
                x.v.mul(&y.d[2]).sub(&y.v.mul(&x.d[2])).mul(&den),
            ],
        }
    }
}

/// The Jacobian of a map of three variables, by [`Dual3`].
///
/// `J[i][j] = ∂out_i/∂in_j`. `None` where any entry is not finite --
/// a pole, a branch cut, or a point the map does not reach.
pub fn jacobian3<F>(at: [f64; 3], f: F) -> Option<[[f64; 3]; 3]>
where
    F: Fn([Dual3<f64>; 3]) -> [Dual3<f64>; 3],
{
    let arg = [
        Dual3::variable(at[0], 0),
        Dual3::variable(at[1], 1),
        Dual3::variable(at[2], 2),
    ];
    let out = f(arg);
    let mut j = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for k in 0..3 {
            j[i][k] = out[i].d[k];
            if !j[i][k].is_finite() {
                return None;
            }
        }
    }
    Some(j)
}

pub fn jacobian2<T, F>(p: [T; 2], f: F) -> Option<[[T; 2]; 2]>
where
    T: Real,
    F: FnOnce([Dual<T>; 2]) -> [Dual<T>; 2],
{
    let out = f(Dual::seed2(p));
    for c in &out {
        if !c.is_finite() {
            return None;
        }
    }
    let [a, b] = out;
    Some([[a.d[0].clone(), a.d[1].clone()], [b.d[0].clone(), b.d[1].clone()]])
}

/// The Hessian of a planar map: `h[c][i][j] = ∂²out_c/∂x_i∂x_j`.
///
/// The same function at `Dual<Dual<T>>`. The outer dual's gradient
/// components are themselves duals, so the second derivative is
/// already there and no step size is chosen anywhere.
pub fn hessian2<T, F>(p: [T; 2], f: F) -> Option<[[[T; 2]; 2]; 2]>
where
    T: Real,
    F: FnOnce([Dual<Dual<T>>; 2]) -> [Dual<Dual<T>>; 2],
{
    let [x, y] = p;
    let seed = [
        Dual::variable(Dual::variable(x, 0), 0),
        Dual::variable(Dual::variable(y, 1), 1),
    ];
    let out = f(seed);
    for c in &out {
        if !c.is_finite() {
            return None;
        }
    }
    let read = |c: &Dual<Dual<T>>| {
        [
            [c.d[0].d[0].clone(), c.d[0].d[1].clone()],
            [c.d[1].d[0].clone(), c.d[1].d[1].clone()],
        ]
    };
    Some([read(&out[0]), read(&out[1])])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trait's arithmetic is the arithmetic it stands for.
    ///
    /// Not a tautology: every default method ([`Real::recip`],
    /// [`Real::hypot2`], [`Transcendental::powf`]) is a separate
    /// expression from the one `f64` overrides it with, and the
    /// overrides are what the kernels will actually call.
    #[test]
    fn the_f64_impl_is_f64() {
        for &a in &[1.0f64, -3.5, 0.125, 1e-8, 7.0] {
            for &b in &[2.0f64, -0.75, 1e6, 0.5] {
                assert_eq!(a.add(&b), a + b);
                assert_eq!(a.sub(&b), a - b);
                assert_eq!(a.mul(&b), a * b);
                assert_eq!(a.div(&b), a / b);
                assert_eq!(a.hypot2(&b), a * a + b * b);
                assert_eq!(Real::hypot(&a, &b), a.hypot(b));
                assert_eq!(<f64 as Transcendental>::atan2(&a, &b), a.atan2(b));
                assert_eq!(a.abs(), a.abs());
                assert!((a.recip() - 1.0 / a).abs() < 1e-15 * (1.0 / a).abs());
            }
            if a > 0.0 {
                assert_eq!(Real::sqrt(&a), a.sqrt());
                assert!((Real::to_f64(&Transcendental::exp(&Transcendental::ln(&a))) - a).abs() < 1e-12 * a);
                let pw = Transcendental::powf(&a, &2.5);
                assert!((pw - a.powf(2.5)).abs() < 1e-12 * a.powf(2.5));
            }
        }
        // powf's zero cases, which is where three copies disagree.
        assert_eq!(Transcendental::powf(&0.0f64, &2.0), 0.0);
        assert_eq!(Transcendental::powf(&0.0f64, &0.0), 1.0);
        assert!(Transcendental::powf(&0.0f64, &-1.0).is_infinite());
    }

    /// The dual's gradient is the derivative.
    ///
    /// Against central differences on functions chosen for having
    /// every rule in them -- a quotient, a square root, a
    /// transcendental pair and an `atan2` -- away from the points
    /// where a central difference is bad, which is the only place it
    /// can be the judge.
    #[test]
    fn the_dual_derivative_is_the_derivative() {
        type F = fn([f64; 2]) -> [f64; 2];
        type D = fn([Dual<f64>; 2]) -> [Dual<f64>; 2];
        let cases: [(&str, F, D); 4] = [
            (
                "quotient",
                |p| [p[0] / (1.0 + p[1] * p[1]), p[1] / (5.0 + p[0])],
                |p| {
                    [
                        p[0].div(&p[1].sqr().add(&p[1].lit(1.0))),
                        p[1].div(&p[0].add(&p[0].lit(5.0))),
                    ]
                },
            ),
            (
                "root",
                |p| {
                    let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
                    [r, p[0] * r]
                },
                |p| {
                    let r = p[0].hypot(&p[1]);
                    [r.clone(), p[0].mul(&r)]
                },
            ),
            (
                "trig",
                |p| [p[0].sin() * p[1].cos(), (p[0] * p[1]).exp()],
                |p| [p[0].sin().mul(&p[1].cos()), p[0].mul(&p[1]).exp()],
            ),
            (
                "angle",
                |p| {
                    let a = p[1].atan2(p[0]);
                    [a, a.abs().ln()]
                },
                |p| {
                    let a = Transcendental::atan2(&p[1], &p[0]);
                    [a.clone(), a.abs().ln()]
                },
            ),
        ];
        for (name, f, fd) in cases {
            for p in [[1.3f64, 0.7], [-2.0, 1.1], [0.4, 2.9], [3.0, -1.7]] {
                let j = jacobian2(p, fd).unwrap_or_else(|| panic!("{name} at {p:?} is not finite"));
                let h = 1e-5;
                for i in 0..2 {
                    let mut lo = p;
                    let mut hi = p;
                    lo[i] -= h;
                    hi[i] += h;
                    let (a, b) = (f(lo), f(hi));
                    for c in 0..2 {
                        let fd_val = (b[c] - a[c]) / (2.0 * h);
                        let tol = 1e-5 * (1.0 + fd_val.abs());
                        assert!(
                            (j[c][i] - fd_val).abs() < tol,
                            "{name} at {p:?}: d out[{c}]/d p[{i}] dual {} vs difference {fd_val}",
                            j[c][i]
                        );
                    }
                }
            }
        }
    }

    /// The nested dual's second derivative is the second derivative,
    /// and it is SYMMETRIC -- which a central difference of a
    /// Jacobian is not, and which is the cheapest evidence the
    /// nesting is wired up right.
    #[test]
    fn the_nested_dual_is_the_second_derivative() {
        // f = (x²y + sin x, √(x² + y²)): every second derivative
        // non-zero and known by hand for the first component.
        let fd = |p: [Dual<Dual<f64>>; 2]| {
            [
                p[0].sqr().mul(&p[1]).add(&p[0].sin()),
                p[0].hypot(&p[1]),
            ]
        };
        for p in [[1.3f64, 0.7], [-2.0, 1.1], [0.4, 2.9]] {
            let h = hessian2(p, fd).expect("finite");
            // ∂²/∂x² = 2y − sin x, ∂²/∂x∂y = 2x, ∂²/∂y² = 0.
            let want = [
                [2.0 * p[1] - p[0].sin(), 2.0 * p[0]],
                [2.0 * p[0], 0.0],
            ];
            for i in 0..2 {
                for j in 0..2 {
                    assert!(
                        (h[0][i][j] - want[i][j]).abs() < 1e-12,
                        "at {p:?}: h[0][{i}][{j}] = {} want {}",
                        h[0][i][j],
                        want[i][j]
                    );
                }
            }
            // The radial component's Hessian is symmetric and its
            // trace is 1/|p| -- the Laplacian of the distance.
            let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
            assert!((h[1][0][1] - h[1][1][0]).abs() < 1e-14);
            assert!((h[1][0][0] + h[1][1][1] - 1.0 / r).abs() < 1e-12);
        }
    }
}
