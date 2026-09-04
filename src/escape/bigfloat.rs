//! Thin big-float wrapper for Newton nucleus-finding.
//!
//! The plan's design: Newton's step divides by the derivative orbit,
//! whose dynamic range no fixed binary point can hold — so this type
//! is a limb array plus ONE exponent, normalize-on-demand, reusing
//! the fixed-point limb cores. Still no rounding modes, and the cores
//! still never divide: division is built from multiplication via
//! Newton–Raphson reciprocal (each pass doubles the correct bits from
//! an f64 seed).
//!
//! Representation: `± mag · 2^exp` where `mag` is the limb array read
//! as a little-endian integer. Normalized form keeps the top limb's
//! high bit set (zero is limbs of zeros with `exp = 0`). All values
//! in an expression should carry the same limb count (`n_limbs`),
//! chosen by the caller from the target precision.

use super::fixedpoint::{
    add_mag, cmp_mag, frac_bits, scale_pow2, shl_small, shr_small, sub_mag, FixedPoint,
};

/// `± (limbs as integer) · 2^exp`.
#[derive(Clone, Debug)]
pub struct BigFloat {
    pub neg: bool,
    pub exp: i64,
    pub limbs: Vec<u64>,
}

impl BigFloat {
    pub fn zero(n_limbs: usize) -> Self {
        Self { neg: false, exp: 0, limbs: vec![0; n_limbs] }
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.iter().all(|&l| l == 0)
    }

    pub fn n_limbs(&self) -> usize {
        self.limbs.len()
    }

    /// Shift so the top limb's high bit is set (exact — bits shifted
    /// in at the bottom are zeros). No-op on zero.
    pub fn normalize(&mut self) {
        if self.is_zero() {
            self.exp = 0;
            self.neg = false;
            return;
        }
        // Whole-limb rotation first.
        let n = self.n_limbs();
        let top_nonzero = (0..n).rev().find(|&i| self.limbs[i] != 0).unwrap();
        let limb_shift = n - 1 - top_nonzero;
        if limb_shift > 0 {
            for i in (0..n).rev() {
                self.limbs[i] = if i >= limb_shift { self.limbs[i - limb_shift] } else { 0 };
            }
            self.exp = self.exp.saturating_sub(64 * limb_shift as i64);
        }
        let lz = self.limbs[n - 1].leading_zeros();
        if lz > 0 {
            let carry = shl_small(&mut self.limbs, lz);
            debug_assert_eq!(carry, 0);
            self.exp = self.exp.saturating_sub(lz as i64);
        }
    }

    pub fn from_f64(v: f64, n_limbs: usize) -> Self {
        if v == 0.0 || !v.is_finite() {
            return Self::zero(n_limbs);
        }
        let neg = v < 0.0;
        let (mant, exp2) = {
            // v = mant * 2^exp2 with mant an odd-ish 53-bit integer.
            let bits = v.abs().to_bits();
            let raw_exp = ((bits >> 52) & 0x7ff) as i64;
            let frac = bits & ((1u64 << 52) - 1);
            if raw_exp == 0 {
                (frac, -1074i64)
            } else {
                (frac | (1u64 << 52), raw_exp - 1075)
            }
        };
        let mut out = Self::zero(n_limbs);
        out.limbs[0] = mant;
        out.exp = exp2;
        out.neg = neg;
        out.normalize();
        out
    }

    pub fn to_f64(&self) -> f64 {
        if self.is_zero() {
            return 0.0;
        }
        let n = self.n_limbs();
        // Normalized: top limb's high bit set. Take 64 top bits.
        let mut v = self.limbs[n - 1] as f64;
        if n >= 2 {
            v += (self.limbs[n - 2] as f64) / 2f64.powi(64);
        }
        let e = self.exp + 64 * (n as i64 - 1);
        // `scale_pow2` carries its own saturation, and unlike the
        // `2f64.powi` it replaces it does not zero representable
        // values: the old low guard cut off at exponent -1070, but `v`
        // is a 64-bit mantissa, so magnitudes stayed representable
        // roughly 64 octaves further down than that.
        let mag = scale_pow2(v, e);
        if self.neg { -mag } else { mag }
    }

    /// From a fixed-point value (exact).
    pub fn from_fixed(v: &FixedPoint) -> Self {
        let mut out = Self {
            neg: v.neg,
            exp: -(frac_bits(v.n_limbs()) as i64),
            limbs: v.limbs.clone(),
        };
        out.normalize();
        out
    }

    /// Into a fixed-point value at `n_limbs` precision. Returns None
    /// when the magnitude exceeds the fixed-point headroom.
    pub fn to_fixed(&self, n_limbs: usize) -> Option<FixedPoint> {
        if self.is_zero() {
            return Some(FixedPoint::zero(n_limbs));
        }
        let target_exp = -(frac_bits(n_limbs) as i64);
        let mut out = FixedPoint::zero(n_limbs);
        // Place each source bit b (at absolute exponent self.exp + i)
        // into the target: position i + (self.exp - target_exp).
        let shift = self.exp - target_exp;
        let src = &self.limbs;
        let src_bits = 64 * src.len() as i64;
        for limb_i in 0..src.len() {
            if src[limb_i] == 0 {
                continue;
            }
            let base = 64 * limb_i as i64 + shift;
            for bit in 0..64u32 {
                if (src[limb_i] >> bit) & 1 == 0 {
                    continue;
                }
                let pos = base + bit as i64;
                if pos < 0 {
                    continue; // below the fixed-point resolution
                }
                if pos >= 64 * n_limbs as i64 {
                    return None; // out of headroom
                }
                out.limbs[(pos / 64) as usize] |= 1u64 << (pos % 64);
            }
        }
        let _ = src_bits;
        out.neg = self.neg;
        Some(out)
    }

    fn with_same_shape(&self) -> Self {
        Self::zero(self.n_limbs())
    }

    /// Signed addition. Both operands must share a limb count.
    pub fn add(&self, other: &Self) -> Self {
        let n = self.n_limbs();
        debug_assert_eq!(other.n_limbs(), n);
        if self.is_zero() {
            return other.clone();
        }
        if other.is_zero() {
            return self.clone();
        }
        // Align: bring the larger-exponent operand down is impossible
        // (bits fall off the top); instead shift the SMALLER one right.
        let (hi, lo) = if self.exp >= other.exp { (self, other) } else { (other, self) };
        let d = hi.exp - lo.exp;
        let total_bits = 64 * n as i64;
        let mut lo_limbs = lo.limbs.clone();
        if d >= total_bits {
            return hi.clone(); // the small addend is entirely below resolution
        }
        // Shift lo right by d (losing low bits — below the result's
        // resolution).
        let limb_shift = (d / 64) as usize;
        if limb_shift > 0 {
            for i in 0..n {
                lo_limbs[i] = if i + limb_shift < n { lo_limbs[i + limb_shift] } else { 0 };
            }
        }
        let bit_shift = (d % 64) as u32;
        if bit_shift > 0 {
            shr_small(&mut lo_limbs, bit_shift);
        }

        let mut out = self.with_same_shape();
        out.exp = hi.exp;
        if hi.neg == lo.neg {
            let carry = add_mag(&hi.limbs, &lo_limbs, &mut out.limbs);
            out.neg = hi.neg;
            if carry {
                // Overflow past the top: shift right one bit, absorb
                // the carry.
                shr_small(&mut out.limbs, 1);
                let top = out.limbs.len() - 1;
                out.limbs[top] |= 1u64 << 63;
                out.exp += 1;
            }
        } else {
            match cmp_mag(&hi.limbs, &lo_limbs) {
                std::cmp::Ordering::Less => {
                    sub_mag(&lo_limbs, &hi.limbs, &mut out.limbs);
                    out.neg = lo.neg;
                }
                _ => {
                    sub_mag(&hi.limbs, &lo_limbs, &mut out.limbs);
                    out.neg = hi.neg;
                }
            }
        }
        out.normalize();
        out
    }

    pub fn sub(&self, other: &Self) -> Self {
        let flipped = Self { neg: !other.neg, exp: other.exp, limbs: other.limbs.clone() };
        self.add(&flipped)
    }

    /// Multiply, keeping the top `n` limbs of the 2n-limb product
    /// (the discarded half is below the result's resolution).
    pub fn mul(&self, other: &Self) -> Self {
        let n = self.n_limbs();
        debug_assert_eq!(other.n_limbs(), n);
        if self.is_zero() || other.is_zero() {
            return Self::zero(n);
        }
        let mut prod = vec![0u64; 2 * n];
        for i in 0..n {
            let mut carry = 0u64;
            for j in 0..n {
                let p = (self.limbs[i] as u128) * (other.limbs[j] as u128)
                    + (prod[i + j] as u128)
                    + (carry as u128);
                prod[i + j] = p as u64;
                carry = (p >> 64) as u64;
            }
            prod[i + n] = prod[i + n].wrapping_add(carry);
        }
        let mut out = Self {
            neg: self.neg != other.neg,
            // Keep limbs n..2n: their unit is 2^(64 n) relative to the
            // integer product. Saturating: an escaped orbit squared
            // repeatedly doubles the exponent past i64 in ~62 steps —
            // saturation keeps the value an honest "astronomically
            // huge" instead of a panic (callers bail on magnitude).
            exp: self
                .exp
                .saturating_add(other.exp)
                .saturating_add(64 * n as i64),
            limbs: prod[n..].to_vec(),
        };
        out.normalize();
        out
    }

    pub fn mul_pow2(&self, k: i64) -> Self {
        let mut out = self.clone();
        if !out.is_zero() {
            out.exp = out.exp.saturating_add(k);
        }
        out
    }

    /// Magnitude exponent (~log2 |self|), None for zero.
    pub fn mag_exp(&self) -> Option<i64> {
        if self.is_zero() {
            None
        } else {
            Some(self.exp.saturating_add(64 * self.n_limbs() as i64 - 1))
        }
    }

    pub fn neg(&self) -> Self {
        let mut out = self.clone();
        if !out.is_zero() {
            out.neg = !out.neg;
        }
        out
    }

    /// Reciprocal via Newton–Raphson (multiplication only, per the
    /// plan): x ← x(2 − d·x), doubling correct bits from an f64 seed.
    /// Panics on zero input (callers guard).
    pub fn recip(&self) -> Self {
        assert!(!self.is_zero(), "reciprocal of zero");
        let n = self.n_limbs();
        // Seed from the value's f64 with the exponent factored out to
        // dodge overflow/underflow: self = m · 2^E with m in [1, 2).
        let e_norm = self.exp + 64 * n as i64 - 1;
        let m = Self {
            neg: false,
            exp: -(64 * n as i64 - 1),
            limbs: self.limbs.clone(),
        };
        let seed = 1.0 / m.to_f64();
        let mut x = Self::from_f64(seed, n);
        let two = Self::from_f64(2.0, n);
        // 53 seed bits double per pass.
        let target_bits = 64 * n as i64;
        let mut have = 50i64;
        while have < target_bits + 8 {
            let dx = m.mul(&x);
            let corr = two.sub(&dx);
            x = x.mul(&corr);
            have *= 2;
        }
        let mut out = x.mul_pow2(-e_norm);
        out.neg = self.neg;
        out
    }

    /// |self|.
    pub fn abs(&self) -> Self {
        let mut out = self.clone();
        out.neg = false;
        out
    }

    /// Magnitude comparison. Both operands must be normalized (every
    /// constructor and operation here normalizes), so the exponent
    /// orders them and the limbs break the tie.
    pub fn cmp_abs(&self, other: &Self) -> std::cmp::Ordering {
        match (self.is_zero(), other.is_zero()) {
            (true, true) => return std::cmp::Ordering::Equal,
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        match self.exp.cmp(&other.exp) {
            std::cmp::Ordering::Equal => cmp_mag(&self.limbs, &other.limbs),
            ord => ord,
        }
    }

    /// Division by a small positive integer, truncating: the
    /// `1/(2j+1)` of the transcendental series. Long division over
    /// the limbs -- the one division the format does on its own
    /// limbs; everything wider is the Newton reciprocal.
    pub fn div_small(&self, d: u32) -> Self {
        assert!(d > 0, "division by zero");
        if self.is_zero() || d == 1 {
            return self.clone();
        }
        let mut out = self.clone();
        let mut rem: u128 = 0;
        for i in (0..out.limbs.len()).rev() {
            let cur = (rem << 64) | out.limbs[i] as u128;
            out.limbs[i] = (cur / d as u128) as u64;
            rem = cur % d as u128;
        }
        out.normalize();
        out
    }

    /// Square root of a non-negative value, to the format's width:
    /// Newton on the RECIPROCAL root (multiplication only, like
    /// [`Self::recip`]), then one multiply.
    pub fn sqrt(&self) -> Self {
        assert!(!self.neg, "sqrt of a negative value");
        if self.is_zero() {
            return self.clone();
        }
        let n = self.n_limbs();
        // self = m * 2^(2k) with m in [1, 4): an even exponent split
        // so the root's scale is exact.
        let e = self.mag_exp().unwrap();
        let k = e.div_euclid(2);
        let m = self.mul_pow2(-2 * k);
        let mut r = Self::from_f64(1.0 / m.to_f64().sqrt(), n);
        let three = Self::from_f64(3.0, n);
        let target = 64 * n as i64 + 8;
        let mut have = 50i64;
        while have < target {
            // r <- r (3 - m r^2) / 2
            let mr2 = m.mul(&r).mul(&r);
            r = r.mul(&three.sub(&mr2)).mul_pow2(-1);
            have *= 2;
        }
        m.mul(&r).mul_pow2(k)
    }

    /// Natural logarithm of a positive value, to the format's width.
    ///
    /// `ln(m 2^k) = k ln 2 + 2 atanh((m-1)/(m+1))` with m in [1, 2):
    /// the series argument is at most 1/3, so each term gains three
    /// bits. This is what the Ducks reference needs per iteration.
    pub fn ln(&self) -> Self {
        assert!(!self.neg && !self.is_zero(), "ln of a non-positive value");
        let n = self.n_limbs();
        let k = self.mag_exp().unwrap();
        let m = self.mul_pow2(-k);
        let one = Self::from_f64(1.0, n);
        let s = m.sub(&one).mul(&m.add(&one).recip());
        let ln_m = odd_series(&s, false).mul_pow2(1);
        ln2(n).mul(&Self::from_f64(k as f64, n)).add(&ln_m)
    }

    /// Principal-value `atan2(y, x)` in (-pi, pi], to the format's
    /// width: reduce to |t| <= tan(pi/16) by two half-angle steps
    /// (each one square root), then the alternating odd series.
    pub fn atan2(y: &Self, x: &Self) -> Self {
        let n = y.n_limbs();
        let pi = pi(n);
        let half_pi = pi.mul_pow2(-1);
        if y.is_zero() {
            return if x.neg && !x.is_zero() { pi } else { Self::zero(n) };
        }
        if x.is_zero() {
            return if y.neg { half_pi.neg() } else { half_pi };
        }
        let ax = x.abs();
        let ay = y.abs();
        let swap = ay.cmp_abs(&ax) == std::cmp::Ordering::Greater;
        let (num, den) = if swap { (&ax, &ay) } else { (&ay, &ax) };
        let one = Self::from_f64(1.0, n);
        // t in (0, 1]; atan(t) = 2 atan(t / (1 + sqrt(1 + t^2))).
        let mut t = num.mul(&den.recip());
        for _ in 0..2 {
            let root = one.add(&t.mul(&t)).sqrt();
            t = t.mul(&one.add(&root).recip());
        }
        let mut a = odd_series(&t, true).mul_pow2(2);
        if swap {
            a = half_pi.sub(&a);
        }
        if x.neg {
            a = pi.sub(&a);
        }
        if y.neg {
            a = a.neg();
        }
        a
    }
}

/// `sum_j s^(2j+1)/(2j+1)`, plain (atanh) or alternating (atan), for
/// |s| well inside 1. Stops when a term drops below the sum's
/// resolution at the format's width.
fn odd_series(s: &BigFloat, alternating: bool) -> BigFloat {
    let n = s.n_limbs();
    if s.is_zero() {
        return BigFloat::zero(n);
    }
    let cutoff = s.mag_exp().unwrap() - 64 * n as i64 - 4;
    let s2 = s.mul(s);
    let mut term = s.clone();
    let mut sum = BigFloat::zero(n);
    let mut j = 0u32;
    loop {
        let t = term.div_small(2 * j + 1);
        sum = if alternating && j % 2 == 1 { sum.sub(&t) } else { sum.add(&t) };
        term = term.mul(&s2);
        j += 1;
        if term.is_zero() || term.mag_exp().unwrap() < cutoff || j > 200_000 {
            break;
        }
    }
    sum
}

type ConstCache = std::sync::Mutex<std::collections::HashMap<usize, BigFloat>>;

fn cached(cache: &std::sync::OnceLock<ConstCache>, n: usize, build: impl FnOnce() -> BigFloat) -> BigFloat {
    let cache = cache.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(v) = cache.lock().unwrap().get(&n) {
        return v.clone();
    }
    let v = build();
    cache.lock().unwrap().insert(n, v.clone());
    v
}

/// ln 2 at `n` limbs: 2 atanh(1/3). Cached per width -- `ln` is
/// called once per reference iteration.
fn ln2(n: usize) -> BigFloat {
    static CACHE: std::sync::OnceLock<ConstCache> = std::sync::OnceLock::new();
    cached(&CACHE, n, || {
        let third = BigFloat::from_f64(1.0, n).div_small(3);
        odd_series(&third, false).mul_pow2(1)
    })
}

/// pi at `n` limbs by Machin: 16 atan(1/5) - 4 atan(1/239). Cached.
fn pi(n: usize) -> BigFloat {
    static CACHE: std::sync::OnceLock<ConstCache> = std::sync::OnceLock::new();
    cached(&CACHE, n, || {
        let a = odd_series(&BigFloat::from_f64(1.0, n).div_small(5), true);
        let b = odd_series(&BigFloat::from_f64(1.0, n).div_small(239), true);
        a.mul_pow2(4).sub(&b.mul_pow2(2))
    })
}

/// Complex big-float, for the Newton nucleus iteration.
#[derive(Clone, Debug)]
pub struct BigComplex {
    pub re: BigFloat,
    pub im: BigFloat,
}

impl BigComplex {
    pub fn zero(n_limbs: usize) -> Self {
        Self { re: BigFloat::zero(n_limbs), im: BigFloat::zero(n_limbs) }
    }

    pub fn from_f64(re: f64, im: f64, n_limbs: usize) -> Self {
        Self { re: BigFloat::from_f64(re, n_limbs), im: BigFloat::from_f64(im, n_limbs) }
    }

    pub fn add(&self, o: &Self) -> Self {
        Self { re: self.re.add(&o.re), im: self.im.add(&o.im) }
    }

    pub fn sub(&self, o: &Self) -> Self {
        Self { re: self.re.sub(&o.re), im: self.im.sub(&o.im) }
    }

    pub fn mul(&self, o: &Self) -> Self {
        Self {
            re: self.re.mul(&o.re).sub(&self.im.mul(&o.im)),
            im: self.re.mul(&o.im).add(&self.im.mul(&o.re)),
        }
    }

    pub fn norm_sqr(&self) -> BigFloat {
        self.re.mul(&self.re).add(&self.im.mul(&self.im))
    }

    /// self / other, via the real reciprocal of |other|²
    /// (multiplication-only division).
    pub fn div(&self, other: &Self) -> Self {
        let inv_n2 = other.norm_sqr().recip();
        let conj = Self { re: other.re.clone(), im: other.im.neg() };
        let num = self.mul(&conj);
        Self { re: num.re.mul(&inv_n2), im: num.im.mul(&inv_n2) }
    }

    pub fn norm_sqr_f64(&self) -> f64 {
        let r = self.re.to_f64();
        let i = self.im.to_f64();
        r * r + i * i
    }

    /// From a fixed-point value (exact).
    pub fn from_fixed(v: &super::fixedpoint::FixedComplex) -> Self {
        Self { re: BigFloat::from_fixed(&v.re), im: BigFloat::from_fixed(&v.im) }
    }

    pub fn is_zero(&self) -> bool {
        self.re.is_zero() && self.im.is_zero()
    }

    pub fn mul_pow2(&self, k: i64) -> Self {
        Self { re: self.re.mul_pow2(k), im: self.im.mul_pow2(k) }
    }

    /// Scale by a real f64 (a small integer coefficient, typically).
    pub fn mul_f64(&self, k: f64) -> Self {
        let r = BigFloat::from_f64(k, self.re.n_limbs());
        Self { re: self.re.mul(&r), im: self.im.mul(&r) }
    }

    /// Principal complex log. The origin (a log singularity) returns
    /// the shader's own sentinel, `esc_clog`'s (-34.5, 0) for
    /// |z|^2 < 1e-30, so a reference iterate agrees with the pixel
    /// formula there instead of evaluating atan2 at a zero pair.
    pub fn ln(&self) -> Self {
        let n = self.re.n_limbs();
        let r2 = self.norm_sqr();
        if r2.is_zero() || r2.to_f64() < 1e-30 {
            return Self::from_f64(-34.5, 0.0, n);
        }
        Self { re: r2.ln().mul_pow2(-1), im: BigFloat::atan2(&self.im, &self.re) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 4;

    fn bf(v: f64) -> BigFloat {
        BigFloat::from_f64(v, N)
    }

    #[test]
    fn f64_round_trip_and_arithmetic() {
        for v in [1.0, -0.7453, 3.5e10, -2.2e-18, 0.0, 1e300, -1e-300] {
            let x = bf(v);
            assert!(
                (x.to_f64() - v).abs() <= v.abs() * 1e-15,
                "{v} -> {}",
                x.to_f64()
            );
        }
        assert!((bf(1.375).add(&bf(-0.7453)).to_f64() - (1.375 - 0.7453)).abs() < 1e-14);
        assert!((bf(1.375).mul(&bf(-0.7453)).to_f64() - (1.375 * -0.7453)).abs() < 1e-14);
        // Exponent-far addition: the small addend vanishes cleanly.
        assert_eq!(bf(1e300).add(&bf(1e-300)).to_f64(), 1e300);
    }

    #[test]
    fn subtraction_cancellation_normalizes() {
        // 1 - (1 - 2^-200): the result is 2^-200, far below f64's view
        // of the operands — the whole point of the wide format.
        let one = bf(1.0);
        let tiny_exp = -200i64;
        let tiny = BigFloat { neg: false, exp: tiny_exp, limbs: {
            let mut l = vec![0u64; N];
            l[0] = 1;
            l
        }};
        let almost = one.sub(&tiny);
        let back = one.sub(&almost);
        assert!(!back.is_zero());
        let log2 = back.to_f64().log2();
        assert!((log2 - tiny_exp as f64).abs() < 0.5, "got 2^{log2}");
    }

    #[test]
    fn reciprocal_is_full_precision() {
        // x * recip(x) must equal 1 to the format's own precision
        // (~256 bits): the error term, viewed at full width, is tiny.
        for v in [3.0, -0.7453, 1.9e-30, 7.7e25] {
            let x = bf(v);
            let r = x.recip();
            let prod = x.mul(&r);
            let err = prod.sub(&bf(1.0));
            if !err.is_zero() {
                // magnitude exponent of the error must be far below 2^0
                let e = err.exp + 64 * N as i64 - 1;
                assert!(e < -(64 * (N as i64) - 16), "recip({v}) error 2^{e}");
            }
        }
    }

    #[test]
    fn complex_division_identity() {
        let a = BigComplex::from_f64(1.7, -0.4, N);
        let b = BigComplex::from_f64(-0.3, 0.9, N);
        let q = a.div(&b);
        let back = q.mul(&b);
        assert!((back.re.to_f64() - 1.7).abs() < 1e-13);
        assert!((back.im.to_f64() - -0.4).abs() < 1e-13);
    }

    #[test]
    fn fixed_point_round_trips() {
        let fx = FixedPoint::from_decimal("-0.7453", 4).unwrap();
        let big = BigFloat::from_fixed(&fx);
        assert!((big.to_f64() - -0.7453).abs() < 1e-15);
        let back = big.to_fixed(4).unwrap();
        assert_eq!(back, fx, "fixed -> big -> fixed must be exact");
        // Out of headroom: refused, not wrapped.
        assert!(bf(1e9).to_fixed(4).is_none());
    }

    /// The error's magnitude exponent must sit far below the format's
    /// width (256 bits at N = 4): "full precision" as `recip` tests it.
    fn assert_tiny(err: &BigFloat, what: &str) {
        if let Some(e) = err.mag_exp() {
            assert!(e < -(64 * N as i64 - 24), "{what}: error 2^{e}");
        }
    }

    #[test]
    fn small_division_and_sqrt_are_full_precision() {
        // 1/7 * 7 = 1 to the width.
        let seventh = bf(1.0).div_small(7);
        assert_tiny(&seventh.mul(&bf(7.0)).sub(&bf(1.0)), "1/7");
        for v in [2.0, 0.75, 1.9e-30, 7.7e25, 3.0e100] {
            let x = bf(v);
            let r = x.sqrt();
            assert!((r.to_f64() - v.sqrt()).abs() <= v.sqrt() * 1e-15, "sqrt({v})");
            // sqrt(x)^2 - x at the width, relative to x.
            let err = r.mul(&r).sub(&x);
            if let (Some(ee), Some(xe)) = (err.mag_exp(), x.mag_exp()) {
                assert!(ee - xe < -(64 * N as i64 - 24), "sqrt({v}) error 2^{}", ee - xe);
            }
        }
    }

    #[test]
    fn ln_matches_f64_and_its_identities_hold_at_width() {
        for v in [2.0, 10.0, 0.5, 1.0 + 1e-9, 3.7e20, 2.2e-18] {
            let l = bf(v).ln();
            assert!((l.to_f64() - v.ln()).abs() <= 2e-15 * v.ln().abs().max(1.0), "ln({v}) = {}", l.to_f64());
        }
        assert!(bf(1.0).ln().is_zero(), "ln 1 = 0 exactly");
        // ln(a b) = ln a + ln b, and ln(x^2) = 2 ln x, to the width.
        let (a, b) = (bf(3.25), bf(0.71));
        assert_tiny(&a.mul(&b).ln().sub(&a.ln().add(&b.ln())), "ln(ab)");
        let x = bf(1.7e9);
        assert_tiny(&x.mul(&x).ln().sub(&x.ln().mul_pow2(1)), "ln(x^2)");
        // ln 2 itself against the f64 constant.
        assert!((ln2(N).to_f64() - std::f64::consts::LN_2).abs() < 1e-16);
    }

    #[test]
    fn atan2_matches_f64_in_every_quadrant_and_at_width() {
        for (y, x) in [
            (1.0, 1.0),
            (1.0, -1.0),
            (-1.0, -1.0),
            (-1.0, 1.0),
            (0.3, 2.0),
            (2.0, 0.3),
            (-2.0, 0.3),
            (0.0, 1.0),
            (0.0, -1.0),
            (1.0, 0.0),
            (-1.0, 0.0),
            (1e-12, 1.0),
            (1.0, 1e-12),
            (-5.0, 1e-300),
        ] {
            let a = BigFloat::atan2(&bf(y), &bf(x));
            assert!((a.to_f64() - y.atan2(x)).abs() < 2e-15, "atan2({y}, {x}) = {}", a.to_f64());
        }
        assert!((pi(N).to_f64() - std::f64::consts::PI).abs() < 1e-15);
        // The double-angle identity at the width: for a first-quadrant
        // (y, x), atan2(2xy, x^2 - y^2) = 2 atan2(y, x) as long as the
        // doubled angle stays below pi/2.
        let (y, x) = (bf(0.3), bf(1.1));
        let lhs = BigFloat::atan2(&x.mul(&y).mul_pow2(1), &x.mul(&x).sub(&y.mul(&y)));
        let rhs = BigFloat::atan2(&y, &x).mul_pow2(1);
        assert_tiny(&lhs.sub(&rhs), "double angle");
    }

    #[test]
    fn complex_ln_agrees_with_f64_and_guards_the_origin() {
        let z = BigComplex::from_f64(-0.4, 0.9, N);
        let l = z.ln();
        let r = (0.4f64 * 0.4 + 0.9 * 0.9).sqrt().ln();
        assert!((l.re.to_f64() - r).abs() < 2e-15);
        assert!((l.im.to_f64() - 0.9f64.atan2(-0.4)).abs() < 2e-15);
        let o = BigComplex::zero(N).ln();
        assert_eq!(o.re.to_f64(), -34.5);
        assert!(o.im.is_zero());
    }
}
