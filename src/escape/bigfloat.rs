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
    add_mag, cmp_mag, frac_bits, shl_small, shr_small, sub_mag, FixedPoint,
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
            self.exp -= 64 * limb_shift as i64;
        }
        let lz = self.limbs[n - 1].leading_zeros();
        if lz > 0 {
            let carry = shl_small(&mut self.limbs, lz);
            debug_assert_eq!(carry, 0);
            self.exp -= lz as i64;
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
        let mag = if e > 1023 {
            f64::INFINITY
        } else if e < -1070 {
            0.0
        } else {
            v * 2f64.powi(e as i32)
        };
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
            // integer product.
            exp: self.exp + other.exp + 64 * n as i64,
            limbs: prod[n..].to_vec(),
        };
        out.normalize();
        out
    }

    pub fn mul_pow2(&self, k: i64) -> Self {
        let mut out = self.clone();
        if !out.is_zero() {
            out.exp += k;
        }
        out
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
}
