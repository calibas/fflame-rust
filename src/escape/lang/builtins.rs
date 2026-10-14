//! The built-in functions: names, typing, and their f64 evaluation.
//!
//! UF's list (Reference > Built-in functions: arithmetic,
//! trigonometric, conversion, miscellaneous), plus Fractint's `cosxx`.
//! Branch cuts are the principal ones throughout. The WGSL versions
//! live with the WGSL back end and are tested against these.

use super::ir::Ty;

/// A complex number in f64: the interpreter's value and the reference
/// the GPU is compared with.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct C64 {
    pub re: f64,
    pub im: f64,
}

impl C64 {
    pub const ZERO: C64 = C64 { re: 0.0, im: 0.0 };
    pub const ONE: C64 = C64 { re: 1.0, im: 0.0 };
    pub const I: C64 = C64 { re: 0.0, im: 1.0 };

    pub fn new(re: f64, im: f64) -> Self {
        C64 { re, im }
    }

    pub fn real(re: f64) -> Self {
        C64 { re, im: 0.0 }
    }

    pub fn add(self, o: C64) -> C64 {
        C64::new(self.re + o.re, self.im + o.im)
    }

    pub fn sub(self, o: C64) -> C64 {
        C64::new(self.re - o.re, self.im - o.im)
    }

    pub fn mul(self, o: C64) -> C64 {
        C64::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }

    /// Plain IEEE: a zero divisor gives infinities and NaNs, as UF's and
    /// Fractint's do (neither checks).
    pub fn div(self, o: C64) -> C64 {
        let d = o.re * o.re + o.im * o.im;
        C64::new((self.re * o.re + self.im * o.im) / d, (self.im * o.re - self.re * o.im) / d)
    }

    pub fn scale(self, k: f64) -> C64 {
        C64::new(self.re * k, self.im * k)
    }

    pub fn neg(self) -> C64 {
        C64::new(-self.re, -self.im)
    }

    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }

    pub fn conj(self) -> C64 {
        C64::new(self.re, -self.im)
    }

    pub fn exp(self) -> C64 {
        let e = self.re.exp();
        C64::new(e * self.im.cos(), e * self.im.sin())
    }

    pub fn ln(self) -> C64 {
        C64::new(self.abs().ln(), self.arg())
    }

    /// The principal root, by the half-angle form that keeps both parts
    /// accurate (no cancellation on either side of the cut).
    pub fn sqrt(self) -> C64 {
        let r = self.abs();
        if r == 0.0 {
            return C64::ZERO;
        }
        let t = ((r + self.re.abs()) * 0.5).sqrt();
        if self.re >= 0.0 {
            C64::new(t, self.im / (2.0 * t))
        } else {
            C64::new(self.im.abs() / (2.0 * t), t.copysign(self.im))
        }
    }

    /// `z^n` for an integer `n`, by repeated squaring; negative powers
    /// invert. `z^0` is 1.
    pub fn powi(self, n: i64) -> C64 {
        let mut base = if n < 0 { C64::ONE.div(self) } else { self };
        let mut e = n.unsigned_abs();
        let mut acc = C64::ONE;
        while e > 0 {
            if e & 1 == 1 {
                acc = acc.mul(base);
            }
            base = base.mul(base);
            e >>= 1;
        }
        acc
    }

    /// `z^w`, principal: `exp(w log z)`, and 0 at the origin.
    pub fn powc(self, w: C64) -> C64 {
        if self.re == 0.0 && self.im == 0.0 {
            return C64::ZERO;
        }
        w.mul(self.ln()).exp()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Builtin {
    Sqr,
    Sqrt,
    Log,
    Exp,
    Abs,
    Cabs,
    Recip,
    Ident,
    Zero,
    Sin,
    Asin,
    Sinh,
    Asinh,
    Cos,
    Acos,
    Cosh,
    Acosh,
    Tan,
    Atan,
    Atan2,
    Tanh,
    Atanh,
    Cotan,
    Cotanh,
    Round,
    Trunc,
    Ceil,
    Floor,
    Real,
    Imag,
    Conj,
    Flip,
    IsInf,
    IsNan,
    Cosxx,
}

/// Every built-in by name (lowered).
pub const BUILTINS: &[(&str, Builtin)] = &[
    ("sqr", Builtin::Sqr),
    ("sqrt", Builtin::Sqrt),
    ("log", Builtin::Log),
    ("exp", Builtin::Exp),
    ("abs", Builtin::Abs),
    ("cabs", Builtin::Cabs),
    ("recip", Builtin::Recip),
    ("ident", Builtin::Ident),
    ("zero", Builtin::Zero),
    ("sin", Builtin::Sin),
    ("asin", Builtin::Asin),
    ("sinh", Builtin::Sinh),
    ("asinh", Builtin::Asinh),
    ("cos", Builtin::Cos),
    ("acos", Builtin::Acos),
    ("cosh", Builtin::Cosh),
    ("acosh", Builtin::Acosh),
    ("tan", Builtin::Tan),
    ("atan", Builtin::Atan),
    ("atan2", Builtin::Atan2),
    ("tanh", Builtin::Tanh),
    ("atanh", Builtin::Atanh),
    ("cotan", Builtin::Cotan),
    ("cotanh", Builtin::Cotanh),
    ("round", Builtin::Round),
    ("trunc", Builtin::Trunc),
    ("ceil", Builtin::Ceil),
    ("floor", Builtin::Floor),
    ("real", Builtin::Real),
    ("imag", Builtin::Imag),
    ("conj", Builtin::Conj),
    ("flip", Builtin::Flip),
    ("isinf", Builtin::IsInf),
    ("isnan", Builtin::IsNan),
    ("cosxx", Builtin::Cosxx),
];

/// What a user function (`@fn(z)`, Fractint's `fn1`..`fn4`) may be set
/// to, in the order the panel lists them; a user function's parameter
/// holds an index into this. Order is part of saved configs: append
/// only.
pub const USER_FUNCTIONS: &[Builtin] = &[
    Builtin::Sin,
    Builtin::Cos,
    Builtin::Tan,
    Builtin::Cotan,
    Builtin::Sinh,
    Builtin::Cosh,
    Builtin::Tanh,
    Builtin::Cotanh,
    Builtin::Asin,
    Builtin::Acos,
    Builtin::Atan,
    Builtin::Asinh,
    Builtin::Acosh,
    Builtin::Atanh,
    Builtin::Sqr,
    Builtin::Sqrt,
    Builtin::Log,
    Builtin::Exp,
    Builtin::Abs,
    Builtin::Cabs,
    Builtin::Conj,
    Builtin::Flip,
    Builtin::Ident,
    Builtin::Zero,
    Builtin::Recip,
    Builtin::Real,
    Builtin::Imag,
    Builtin::Cosxx,
    Builtin::Round,
    Builtin::Trunc,
    Builtin::Ceil,
    Builtin::Floor,
];

impl Builtin {
    pub fn from_name(name: &str) -> Option<Builtin> {
        BUILTINS.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
    }

    pub fn name(self) -> &'static str {
        BUILTINS.iter().find(|(_, b)| *b == self).map(|(n, _)| *n).expect("every built-in is named")
    }

    /// The result type for an argument of type `arg`, or why not.
    pub fn result_ty(self, arg: Ty) -> Result<Ty, String> {
        use Builtin::*;
        if arg == Ty::Bool {
            return Err(format!("`{}` takes a number, not a bool", self.name()));
        }
        Ok(match self {
            IsInf | IsNan => Ty::Bool,
            Cabs | Real | Imag | Atan2 => Ty::Float,
            Conj | Flip | Cosxx => Ty::Complex,
            // Keep an integer an integer where the result must be one.
            Sqr | Abs | Ident | Zero => arg,
            Round | Trunc | Ceil | Floor => {
                if arg == Ty::Complex {
                    Ty::Complex
                } else {
                    Ty::Int
                }
            }
            _ => {
                if arg == Ty::Complex {
                    Ty::Complex
                } else {
                    Ty::Float
                }
            }
        })
    }

    /// Evaluate on a complex argument (every function is defined
    /// there; a real result is returned with a zero imaginary part).
    pub fn eval_c(self, z: C64) -> C64 {
        use Builtin::*;
        let i = C64::I;
        match self {
            Sqr => z.mul(z),
            Sqrt => z.sqrt(),
            Log => z.ln(),
            Exp => z.exp(),
            Abs => C64::new(z.re.abs(), z.im.abs()),
            Cabs => C64::real(z.abs()),
            Recip => C64::ONE.div(z),
            Ident => z,
            Zero => C64::ZERO,
            Sin => C64::new(z.re.sin() * z.im.cosh(), z.re.cos() * z.im.sinh()),
            Cos => C64::new(z.re.cos() * z.im.cosh(), -z.re.sin() * z.im.sinh()),
            Sinh => C64::new(z.re.sinh() * z.im.cos(), z.re.cosh() * z.im.sin()),
            Cosh => C64::new(z.re.cosh() * z.im.cos(), z.re.sinh() * z.im.sin()),
            Tan => Sin.eval_c(z).div(Cos.eval_c(z)),
            Cotan => Cos.eval_c(z).div(Sin.eval_c(z)),
            Tanh => Sinh.eval_c(z).div(Cosh.eval_c(z)),
            Cotanh => Cosh.eval_c(z).div(Sinh.eval_c(z)),
            // asin z = -i log(iz + sqrt(1 - z^2))
            Asin => i.neg().mul(i.mul(z).add(C64::ONE.sub(z.mul(z)).sqrt()).ln()),
            // acos z = pi/2 - asin z
            Acos => C64::real(std::f64::consts::FRAC_PI_2).sub(Asin.eval_c(z)),
            // atan z = (i/2) log((i + z) / (i - z))
            Atan => i.scale(0.5).mul(i.add(z).div(i.sub(z)).ln()),
            // asinh z = log(z + sqrt(z^2 + 1))
            Asinh => z.add(z.mul(z).add(C64::ONE).sqrt()).ln(),
            // acosh z = log(z + sqrt(z + 1) sqrt(z - 1)): the product of
            // roots, not the root of the product, for the principal cut.
            Acosh => z.add(z.add(C64::ONE).sqrt().mul(z.sub(C64::ONE).sqrt())).ln(),
            // atanh z = log((1 + z) / (1 - z)) / 2
            Atanh => C64::ONE.add(z).div(C64::ONE.sub(z)).ln().scale(0.5),
            Atan2 => C64::real(z.arg()),
            Round => C64::new(round_half_away(z.re), round_half_away(z.im)),
            Trunc => C64::new(z.re.trunc(), z.im.trunc()),
            Ceil => C64::new(z.re.ceil(), z.im.ceil()),
            Floor => C64::new(z.re.floor(), z.im.floor()),
            Real => C64::real(z.re),
            Imag => C64::real(z.im),
            Conj => z.conj(),
            Flip => C64::new(z.im, z.re),
            IsInf => C64::real(if z.re.is_infinite() || z.im.is_infinite() { 1.0 } else { 0.0 }),
            IsNan => C64::real(if z.re.is_nan() || z.im.is_nan() { 1.0 } else { 0.0 }),
            // Fractint's: cos with the imaginary part negated, the bug
            // in its cos before v16, kept so old pictures reproduce.
            Cosxx => Cos.eval_c(z).conj(),
        }
    }

    /// The complex derivative at `z`, for a user function's choice
    /// (`wgsl::Helper::UserD` is the GPU's): zero for a function with
    /// none (the folds and parts, `abs`, `conj`, `real` ...; the
    /// roundings, whose derivative is zero where it exists).
    pub fn deriv_c(self, z: C64) -> C64 {
        use Builtin::*;
        let one = C64::ONE;
        match self {
            Sqr => z.scale(2.0),
            Sqrt => C64::real(0.5).div(z.sqrt()),
            Log => one.div(z),
            Exp => z.exp(),
            Recip => one.div(z.mul(z)).neg(),
            Ident => one,
            Sin => Cos.eval_c(z),
            Cos => Sin.eval_c(z).neg(),
            Tan => one.div(Cos.eval_c(z).mul(Cos.eval_c(z))),
            Cotan => one.div(Sin.eval_c(z).mul(Sin.eval_c(z))).neg(),
            Sinh => Cosh.eval_c(z),
            Cosh => Sinh.eval_c(z),
            Tanh => one.div(Cosh.eval_c(z).mul(Cosh.eval_c(z))),
            Cotanh => one.div(Sinh.eval_c(z).mul(Sinh.eval_c(z))).neg(),
            Asin => one.div(one.sub(z.mul(z)).sqrt()),
            Acos => one.div(one.sub(z.mul(z)).sqrt()).neg(),
            Atan => one.div(one.add(z.mul(z))),
            Asinh => one.div(z.mul(z).add(one).sqrt()),
            Acosh => one.div(z.sub(one).sqrt().mul(z.add(one).sqrt())),
            Atanh => one.div(one.sub(z.mul(z))),
            Zero | Abs | Cabs | Real | Imag | Conj | Flip | Atan2 | Round | Trunc | Ceil | Floor | Cosxx | IsInf
            | IsNan => C64::ZERO,
        }
    }

    /// Evaluate on a real argument with a real result (`result_ty` gave
    /// Float, Int or Bool): the real function, which differs from the
    /// complex one off its domain (`sqrt(-1)` is NaN here, `i` there).
    pub fn eval_f(self, x: f64) -> f64 {
        use Builtin::*;
        match self {
            Sqr => x * x,
            Sqrt => x.sqrt(),
            Log => x.ln(),
            Exp => x.exp(),
            Abs => x.abs(),
            Cabs => x.abs(),
            Recip => 1.0 / x,
            Ident => x,
            Zero => 0.0,
            Sin => x.sin(),
            Cos => x.cos(),
            Sinh => x.sinh(),
            Cosh => x.cosh(),
            Tan => x.tan(),
            Cotan => 1.0 / x.tan(),
            Tanh => x.tanh(),
            Cotanh => 1.0 / x.tanh(),
            Asin => x.asin(),
            Acos => x.acos(),
            Atan => x.atan(),
            Asinh => x.asinh(),
            Acosh => x.acosh(),
            Atanh => x.atanh(),
            // The angle of (x, 0).
            Atan2 => 0.0f64.atan2(x),
            Round => round_half_away(x),
            Trunc => x.trunc(),
            Ceil => x.ceil(),
            Floor => x.floor(),
            Real => x,
            Imag => 0.0,
            IsInf => f64::from(u8::from(x.is_infinite())),
            IsNan => f64::from(u8::from(x.is_nan())),
            Conj | Flip | Cosxx => unreachable!("complex-valued"),
        }
    }
}

/// `round` as UF and C round: halves away from zero (WGSL's `round`
/// rounds halves to even, so the WGSL version spells this out too).
pub fn round_half_away(x: f64) -> f64 {
    x.round()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: C64, b: C64) -> bool {
        (a.re - b.re).abs() < 1e-12 && (a.im - b.im).abs() < 1e-12
    }

    #[test]
    fn inverse_functions_invert() {
        let z = C64::new(0.3, -0.7);
        for (f, g) in [
            (Builtin::Sin, Builtin::Asin),
            (Builtin::Cos, Builtin::Acos),
            (Builtin::Tan, Builtin::Atan),
            (Builtin::Sinh, Builtin::Asinh),
            (Builtin::Cosh, Builtin::Acosh),
            (Builtin::Tanh, Builtin::Atanh),
            (Builtin::Exp, Builtin::Log),
            (Builtin::Sqr, Builtin::Sqrt),
        ] {
            assert!(close(f.eval_c(g.eval_c(z)), z), "{f:?}({g:?}(z))");
        }
    }

    #[test]
    fn principal_branches_and_powers() {
        assert!(close(C64::real(-4.0).sqrt(), C64::new(0.0, 2.0)));
        assert!(close(C64::new(-4.0, -0.0).sqrt(), C64::new(0.0, -2.0)));
        assert!(close(C64::real(-1.0).ln(), C64::new(0.0, std::f64::consts::PI)));
        let z = C64::new(1.2, -0.4);
        assert!(close(z.powi(3), z.mul(z).mul(z)));
        assert!(close(z.powi(-2), C64::ONE.div(z.mul(z))));
        assert!(close(z.powc(C64::real(3.0)), z.powi(3)));
        assert_eq!(C64::ZERO.powc(C64::real(2.0)), C64::ZERO);
        assert!(Builtin::Sqrt.eval_f(-1.0).is_nan(), "the real sqrt stays real");
        assert_eq!(round_half_away(2.5), 3.0);
        assert_eq!(round_half_away(-2.5), -3.0);
    }

    /// Each derivative against a central difference, away from the cuts.
    #[test]
    fn derivatives_match_finite_differences() {
        let z = C64::new(0.37, -0.61);
        let h = 1e-6;
        for f in USER_FUNCTIONS {
            let d = f.deriv_c(z);
            let fd = f.eval_c(z.add(C64::real(h))).sub(f.eval_c(z.sub(C64::real(h)))).scale(0.5 / h);
            use Builtin::*;
            let holomorphic = !matches!(f, Abs | Cabs | Real | Imag | Conj | Flip | Cosxx | Round | Trunc | Ceil | Floor);
            assert_eq!(holomorphic, d != C64::ZERO || *f == Zero, "{f:?}");
            if holomorphic {
                assert!((d.re - fd.re).abs() < 1e-6 && (d.im - fd.im).abs() < 1e-6, "{f:?}: {d:?} vs {fd:?}");
                // Along the imaginary axis too: the Cauchy-Riemann check.
                let fi = f.eval_c(z.add(C64::new(0.0, h))).sub(f.eval_c(z.sub(C64::new(0.0, h)))).scale(0.5 / h);
                let di = d.mul(C64::I);
                assert!((di.re - fi.re).abs() < 1e-6 && (di.im - fi.im).abs() < 1e-6, "{f:?} along i");
            }
        }
    }

    #[test]
    fn every_user_function_is_a_builtin_and_names_round_trip() {
        for (name, b) in BUILTINS {
            assert_eq!(Builtin::from_name(name), Some(*b));
            assert_eq!(b.name(), *name);
        }
        let mut seen = std::collections::HashSet::new();
        for f in USER_FUNCTIONS {
            assert!(seen.insert(*f), "{f:?} listed twice");
        }
    }
}
