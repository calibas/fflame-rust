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

    /// The same value at a different width: zero limbs appended
    /// below when widening, the low limbs dropped when narrowing.
    ///
    /// Widening is EXACT -- the value is `mag · 2^exp` and prepending
    /// `k` zero limbs while subtracting `64k` from the exponent is
    /// the same number, still normalized. Narrowing truncates, which
    /// is the rounding this type does everywhere else too.
    ///
    /// This exists for the transcendentals' GUARD LIMB. `exp` halves
    /// its argument until it is under `2^-32` and squares the series
    /// back, and `sin_cos` does the same with the double-angle
    /// formulas; each of those thirty-odd steps doubles the relative
    /// error, so a series summed at the caller's width comes back
    /// thirty bits short of it. Measured: 222 bits of 256 without a
    /// guard limb, 248 with one.
    pub fn with_limbs(&self, n: usize) -> Self {
        let have = self.n_limbs();
        if n == have {
            return self.clone();
        }
        if self.is_zero() {
            return Self::zero(n);
        }
        let mut limbs;
        let exp;
        if n > have {
            let k = n - have;
            limbs = vec![0u64; k];
            limbs.extend_from_slice(&self.limbs);
            exp = self.exp.saturating_sub(64 * k as i64);
        } else {
            let k = have - n;
            limbs = self.limbs[k..].to_vec();
            exp = self.exp.saturating_add(64 * k as i64);
        }
        let mut out = Self { neg: self.neg, exp, limbs };
        out.normalize();
        out
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

    /// `e^x` to the format's width.
    ///
    /// `x = k ln2 + r` with `|r| <= ln2/2`, then `r` is halved `m`
    /// times until it is under `2^-32`, the Taylor series is summed,
    /// and the result is squared back `m` times. Halving costs one
    /// squaring each and buys a factor of two in the series argument,
    /// so the term count falls like `1/m` while the squarings grow
    /// like `m` -- the balance is shallow and thirty-two bits is well
    /// inside it at every width this type is used at.
    ///
    /// Squaring AMPLIFIES the error: `m` squarings multiply it by
    /// `2^m`, so the series is summed to the width plus `m` bits of
    /// headroom rather than to the width. The gate that catches
    /// getting that wrong is `the_transcendentals_hold_their_width`,
    /// which asks for 200 bits and would read 168 without it.
    pub fn exp(&self) -> Self {
        let out = self.n_limbs();
        // One guard limb: the squarings below double the relative
        // error once each, and sixty-four spare bits cover every `m`
        // this reduction can reach.
        let n = out + 1;
        let x = self.with_limbs(n);
        let one = Self::from_f64(1.0, n);
        if x.is_zero() {
            return one.with_limbs(out);
        }
        let l2 = ln2(n);
        // k = round(x / ln2). The quotient is O(1) whenever the
        // result is representable at all -- `exp` past 2^63 is not a
        // number any caller here wants -- so f64 is the right type
        // for it and the remainder is formed at full width.
        let k = (x.mul(&l2.recip()).to_f64()).round();
        if !k.is_finite() {
            return Self::zero(out);
        }
        let r = x.sub(&l2.mul(&Self::from_f64(k, n)));
        // Halve until |r| < 2^-32.
        let mut m = 0u32;
        let mut t = r;
        while t.mag_exp().is_some_and(|e| e > -32) && m < 64 {
            t = t.mul_pow2(-1);
            m += 1;
        }
        // The series, to the width plus the headroom the squarings
        // will eat.
        let target = -(64 * n as i64) - m as i64 - 8;
        let mut sum = one.clone();
        let mut term = one;
        let mut j = 1u32;
        loop {
            term = term.mul(&t).div_small(j);
            if term.is_zero() {
                break;
            }
            sum = sum.add(&term);
            if term.mag_exp().is_some_and(|e| e < target) || j > 100_000 {
                break;
            }
            j += 1;
        }
        for _ in 0..m {
            sum = sum.mul(&sum);
        }
        sum.mul_pow2(k as i64).with_limbs(out)
    }

    /// `x^y` for a positive base: `exp(y ln x)`.
    ///
    /// A zero base gives zero, which is right for the positive
    /// exponents the kernels raise a radius to and is the same answer
    /// `f64::powf` gives there.
    pub fn powf(&self, y: &Self) -> Self {
        if self.is_zero() {
            return Self::zero(self.n_limbs());
        }
        self.abs().ln().mul(y).exp()
    }

    /// `(sin x, cos x)` to the format's width, both at once because
    /// the reduction and the series are shared.
    ///
    /// `x` is reduced modulo `2 pi` -- at FULL width, which is the
    /// whole reason this exists: a reference orbit's angle is a
    /// number whose leading digits cancel, and reducing it in f64
    /// would keep none of the ones that matter. Then the argument is
    /// halved to under `2^-32` and the double-angle formulas rebuild
    /// it, exactly as `exp` squares its way back.
    ///
    /// **The reduction is where a big float earns its keep, and it is
    /// also where it can still lose.** `x mod 2 pi` is a
    /// cancellation: the quotient is computed here in f64, so an `x`
    /// past `2^53 · 2pi` reduces against a quotient that is itself
    /// rounded and the answer is noise. Callers at that magnitude are
    /// out of scope -- and out of scope everywhere, since
    /// `CLAUDE.md`'s own note records three mutually incompatible
    /// answers for `sin(1e20 pi)` across f32 platforms.
    pub fn sin_cos(&self) -> (Self, Self) {
        let out = self.n_limbs();
        // One guard limb, for the doubling steps -- see
        // [`Self::with_limbs`].
        let n = out + 1;
        let one = Self::from_f64(1.0, n);
        if self.is_zero() {
            return (Self::zero(out), one.with_limbs(out));
        }
        let two_pi = pi(n).mul_pow2(1);
        // Reduce modulo 2pi when the argument is large enough for it
        // to matter; below that the halving handles the range.
        let mut x = self.with_limbs(n);
        if x.cmp_abs(&two_pi) != std::cmp::Ordering::Less {
            let q = x.mul(&two_pi.recip()).to_f64().trunc();
            if !q.is_finite() {
                return (Self::zero(out), one.with_limbs(out));
            }
            x = x.sub(&two_pi.mul(&Self::from_f64(q, n)));
        }
        let mut m = 0u32;
        while x.mag_exp().is_some_and(|e| e > -32) && m < 64 {
            x = x.mul_pow2(-1);
            m += 1;
        }
        let target = -(64 * n as i64) - m as i64 - 8;
        // sin and cos from one alternating series each, sharing x^2.
        let x2 = x.mul(&x);
        let mut sin = x.clone();
        let mut cos = one.clone();
        let mut ts = x;
        let mut tc = one.clone();
        let mut j = 1u32;
        loop {
            // sin's next term: -t x^2 / ((2j)(2j+1)); cos's:
            // -t x^2 / ((2j-1)(2j)).
            tc = tc.mul(&x2).div_small(2 * j - 1).div_small(2 * j).neg();
            cos = cos.add(&tc);
            ts = ts.mul(&x2).div_small(2 * j).div_small(2 * j + 1).neg();
            sin = sin.add(&ts);
            let done = |t: &Self| t.is_zero() || t.mag_exp().is_some_and(|e| e < target);
            if (done(&ts) && done(&tc)) || j > 100_000 {
                break;
            }
            j += 1;
        }
        for _ in 0..m {
            // sin(2a) = 2 sin a cos a; cos(2a) = 1 - 2 sin^2 a, whose
            // form avoids the cancellation `cos^2 - sin^2` has when
            // the angle is near pi/4.
            let s2 = sin.mul(&cos).mul_pow2(1);
            let c2 = one.sub(&sin.mul(&sin).mul_pow2(1));
            sin = s2;
            cos = c2;
        }
        (sin.with_limbs(out), cos.with_limbs(out))
    }

    /// `sin x`, when the cosine is not wanted.
    pub fn sin(&self) -> Self {
        self.sin_cos().0
    }

    /// `cos x`.
    pub fn cos(&self) -> Self {
        self.sin_cos().1
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

/// `BigFloat` as the scalar the IFS kernels are written over
/// (`ifs-general.md` D2).
///
/// **[`Real`] only, not [`Transcendental`].** This type has `sqrt`,
/// `ln` and `atan2` and lacks `exp`, `sin` and `cos`, which is item 8
/// of the delta plan's order of work. So the algebraic kernels --
/// spherical, bubble, hemisphere, and a root whose exponent is a
/// whole power -- can run their one generic body at this precision
/// today, and the disc, the blob and a fractional root cannot.
///
/// A constant comes from [`Real::lit`], which takes `&self` because
/// there is no "the number two" without a limb count to build it at.
impl crate::scene::ifs_real::Real for BigFloat {
    fn lit(&self, v: f64) -> Self {
        BigFloat::from_f64(v, self.n_limbs())
    }
    fn to_f64(&self) -> f64 {
        BigFloat::to_f64(self)
    }
    fn add(&self, o: &Self) -> Self {
        BigFloat::add(self, o)
    }
    fn sub(&self, o: &Self) -> Self {
        BigFloat::sub(self, o)
    }
    fn mul(&self, o: &Self) -> Self {
        BigFloat::mul(self, o)
    }
    /// Division is multiplication by the reciprocal, and a zero
    /// divisor PANICS rather than returning an infinity this type
    /// does not have. Every kernel body guards its own denominators
    /// by value before dividing.
    fn div(&self, o: &Self) -> Self {
        BigFloat::mul(self, &o.recip())
    }
    fn neg(&self) -> Self {
        BigFloat::neg(self)
    }
    /// A negative argument gives zero rather than panicking: the
    /// kernel bodies clamp by value first (`(1 − x).max(0)`), and a
    /// value that rounds to a hair below zero there is a zero.
    fn sqrt(&self) -> Self {
        if self.neg {
            return Self::zero(self.n_limbs());
        }
        BigFloat::sqrt(self)
    }
    fn abs(&self) -> Self {
        BigFloat::abs(self)
    }
    /// The comparison a kernel's branch test makes, without going
    /// through `to_f64` -- which saturates to an infinity outside
    /// f64's range, where this type is still perfectly ordered.
    fn cmp_f64(&self, v: f64) -> std::cmp::Ordering {
        let other = BigFloat::from_f64(v, self.n_limbs());
        match (self.neg, other.neg) {
            (false, true) => std::cmp::Ordering::Greater,
            (true, false) => std::cmp::Ordering::Less,
            (false, false) => self.cmp_abs(&other),
            (true, true) => other.cmp_abs(self),
        }
    }
    fn is_finite(&self) -> bool {
        true
    }
}

/// The kernels' own bodies, at arbitrary precision
/// (`ifs-general.md` D2, and item 8 of the delta plan's order of
/// work).
///
/// With this the reference walk stops having a hand-written
/// transcription per kernel: `kernel_inverse_gen::<BigFloat>` IS the
/// f64 body, at whatever width the zoom asks for, and a `disc`, a
/// `blob` or a fractional root gets the same deep handover a
/// `spherical` has had since the beginning.
///
/// `powf` is the trait's default, `exp(e ln x)`, because that is what
/// this type's `exp` and `ln` are for.
impl crate::scene::ifs_real::Transcendental for BigFloat {
    fn exp(&self) -> Self {
        BigFloat::exp(self)
    }
    /// A non-positive argument gives zero rather than panicking, for
    /// the same reason [`Real::sqrt`](crate::scene::ifs_real::Real::sqrt)
    /// does: a kernel body clamps by value first, and a value that
    /// rounds to a hair below zero there is a zero.
    fn ln(&self) -> Self {
        if self.neg || self.is_zero() {
            return Self::zero(self.n_limbs());
        }
        BigFloat::ln(self)
    }
    fn sin(&self) -> Self {
        BigFloat::sin(self)
    }
    fn cos(&self) -> Self {
        BigFloat::cos(self)
    }
    fn atan2(y: &Self, x: &Self) -> Self {
        BigFloat::atan2(y, x)
    }
    fn sin_cos(&self) -> (Self, Self) {
        BigFloat::sin_cos(self)
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

    /// `exp`, `sin` and `cos` hold the width they are asked for, and
    /// they hold it where an f64 cannot reach.
    ///
    /// Two checks, because agreeing with `f64` only says the first
    /// seventeen digits are right and the whole point of this type is
    /// the ones after them.
    ///
    /// *Against f64*, at arguments f64 handles well: within a few
    /// ulps, which is all f64 itself is worth.
    ///
    /// *Against IDENTITIES*, at the full width: `exp(a)·exp(b) =
    /// exp(a+b)`, `sin² + cos² = 1`, `sin(2a) = 2 sin a cos a`. An
    /// identity is a reference that costs nothing to state and is
    /// exact at every width, and it catches the two mistakes this
    /// code can make -- too few series terms, and too little headroom
    /// before the squaring or doubling steps amplify the error. The
    /// second is real: at 200 bits with no headroom the identities
    /// read 168.
    #[test]
    fn the_transcendentals_hold_their_width() {
        // Four limbs: 256 bits, and the identities should hold to
        // nearly all of them.
        const N: usize = 4;
        const BITS: i64 = 64 * N as i64;
        let big = |v: f64| BigFloat::from_f64(v, N);
        let one = big(1.0);

        // Against f64.
        for &x in &[0.0f64, 1e-9, 0.5, 1.0, -1.0, 2.5, -7.25, 13.0, 30.0, -30.0] {
            let got = big(x).exp().to_f64();
            let want = x.exp();
            let rel = if want == 0.0 { got.abs() } else { (got / want - 1.0).abs() };
            assert!(rel < 1e-14, "exp({x}) = {got:e}, not {want:e} (rel {rel:.2e})");
            let (s, c) = big(x).sin_cos();
            assert!(
                (s.to_f64() - x.sin()).abs() < 1e-14 && (c.to_f64() - x.cos()).abs() < 1e-14,
                "sin_cos({x}) = ({}, {}), not ({}, {})",
                s.to_f64(),
                c.to_f64(),
                x.sin(),
                x.cos()
            );
        }
        // And past where f64's own reduction is trustworthy, which is
        // the case this type exists for: the answer is checked by
        // identity below, not against f64 here.
        for &x in &[1e9f64, 1e15] {
            let (s, c) = big(x).sin_cos();
            let sum = s.mul(&s).add(&c.mul(&c)).sub(&one);
            let bits = sum.mag_exp().map_or(i64::MIN, |e| -e);
            assert!(
                bits > BITS - 40,
                "sin^2+cos^2 at x = {x:e} holds only {bits} bits of {BITS}"
            );
        }

        // Identities, at width.
        //
        // `mag_exp` of the residue is its exponent, so `-mag_exp` is
        // how many bits below one it sits -- the number of correct
        // bits, directly.
        let held = |r: &BigFloat| r.mag_exp().map_or(i64::MAX, |e| -e);
        let mut worst = i64::MAX;
        for &(a, b) in &[(0.3f64, 0.7f64), (1.0, -2.5), (-4.0, 9.0), (11.5, 0.125)] {
            let lhs = big(a).exp().mul(&big(b).exp());
            // The sum is formed HERE, not in f64: `0.3 + 0.7` is
            // 0.9999999999999999, and asking the identity against
            // that reads 55 bits of a perfectly good 227 -- the
            // test's own arithmetic, not the function's.
            let rhs = big(a).add(&big(b)).exp();
            // Relative: divide the difference by the value, or a
            // large `exp` looks accurate for being large.
            let rel = lhs.sub(&rhs).mul(&rhs.recip());
            worst = worst.min(held(&rel));
        }
        for &x in &[0.1f64, 1.0, 2.0, 3.0, -5.5, 100.0] {
            let (s, c) = big(x).sin_cos();
            worst = worst.min(held(&s.mul(&s).add(&c.mul(&c)).sub(&one)));
            let (s2, _) = big(2.0 * x).sin_cos();
            worst = worst.min(held(&s2.sub(&s.mul(&c).mul_pow2(1))));
        }
        for &x in &[0.5f64, 2.0, 7.0, 1234.5] {
            // exp and ln are each other's, which is the pair the
            // kernels' `powf` goes through.
            let r = big(x).ln().exp();
            worst = worst.min(held(&r.sub(&big(x)).mul(&big(x).recip())));
        }
        println!("  identities hold {worst} bits of {BITS} at {N} limbs");
        assert!(
            worst > BITS - 24,
            "the identities hold only {worst} bits of {BITS} -- the series is short or the \
             squaring has no headroom"
        );
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
