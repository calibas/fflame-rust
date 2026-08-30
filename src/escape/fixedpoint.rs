//! Fixed-point big-number arithmetic for deep-zoom reference orbits.
//!
//! The plan's own design (escape-time plan, "Arbitrary precision: our
//! own fixed-point, no new dependencies"): the reference orbit lives
//! in a bounded box (|z| ≤ 2 until escape), so the CPU side needs
//! **fixed-point**, not arbitrary-precision floating point — `[u64]`
//! limbs, a few integer bits of headroom, implied binary point. That
//! deletes MPFR's hard parts: no exponents, no normalization, no
//! rounding modes, and the core never divides (only small-scalar
//! division for decimal I/O).
//!
//! Representation: sign-magnitude. A value with `n` limbs (little-
//! endian, `limbs[0]` least significant) represents
//! `± mag / 2^(64·n − INT_BITS)` — i.e. [`INT_BITS`] integer bits at
//! the top of the most-significant limb, the rest fractional. Callers
//! size `n` for their target precision **plus one guard limb** (the
//! plan: truncating multiplies drift over 10⁶–10⁸-iteration orbits;
//! the guard absorbs both that and the truncated-multiplication
//! error).
//!
//! The limb routines are **exponent-agnostic** (slices in, shift
//! amounts as parameters) so the future Newton big-float wrapper can
//! reuse them unchanged.
//!
//! The [`FloatExp`] export is a first-class, tested operation: near-
//! zero orbit values are exactly where rebasing and glitch behavior
//! live, so `2Zₙ` at tiny |Zₙ| must convert exactly (leading-zero
//! count + shift — no rounding surprises).

/// Integer bits of headroom at the top of the representation.
/// Intermediates like x² + y² reach 8 before subtraction (plan);
/// 8 bits (range ±128) is comfortable and costs nothing.
pub const INT_BITS: u32 = 8;

/// Fractional bits for an `n`-limb value.
#[inline]
pub fn frac_bits(n_limbs: usize) -> u32 {
    (n_limbs as u32) * 64 - INT_BITS
}

/// How many limbs (guard included) a zoom depth needs: the pixel
/// spacing at `zoom_log2` is ~2^(2 − zoom), and the delta pipeline
/// wants ~64 bits below that; plus the guard limb.
/// Reference precision for a view: the zoom's needs OR the center's
/// intrinsic digit depth, whichever is deeper (capped - a megabyte
/// center string must not conjure a gigabyte orbit pipeline). See
/// the deep-collapse note in reference.rs.
pub fn limbs_for_view(center_re: &str, center_im: &str, zoom_log2: f64) -> usize {
    let frac_digits = |s: &str| {
        s.trim()
            .split_once('.')
            .map(|(_, f)| f.trim_end_matches(|c: char| !c.is_ascii_digit()).len())
            .unwrap_or(0)
    };
    let digits = frac_digits(center_re).max(frac_digits(center_im));
    // digits · log2(10)/64 limbs, ceil-ish, +1 headroom.
    let digit_limbs = (digits * 10) / 192 + 2;
    limbs_for_zoom(zoom_log2).max(digit_limbs.min(2048))
}

pub fn limbs_for_zoom(zoom_log2: f64) -> usize {
    let bits = (zoom_log2.max(0.0) as u32).saturating_add(64 + INT_BITS);
    (bits as usize).div_ceil(64) + 1
}

// ============================================================
// Exponent-agnostic magnitude cores (little-endian limb slices)
// ============================================================

/// c = a + b (equal lengths). Returns the carry out.
pub(crate) fn add_mag(a: &[u64], b: &[u64], c: &mut [u64]) -> bool {
    let mut carry = false;
    for i in 0..a.len() {
        let (s1, c1) = a[i].overflowing_add(b[i]);
        let (s2, c2) = s1.overflowing_add(carry as u64);
        c[i] = s2;
        carry = c1 || c2;
    }
    carry
}

/// c = a − b, requiring a ≥ b (caller compares first).
pub(crate) fn sub_mag(a: &[u64], b: &[u64], c: &mut [u64]) {
    let mut borrow = false;
    for i in 0..a.len() {
        let (d1, b1) = a[i].overflowing_sub(b[i]);
        let (d2, b2) = d1.overflowing_sub(borrow as u64);
        c[i] = d2;
        borrow = b1 || b2;
    }
    debug_assert!(!borrow, "sub_mag requires a >= b");
}

/// Magnitude comparison.
pub(crate) fn cmp_mag(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    for i in (0..a.len()).rev() {
        match a[i].cmp(&b[i]) {
            std::cmp::Ordering::Equal => continue,
            ord => return ord,
        }
    }
    std::cmp::Ordering::Equal
}

/// In-place left shift by `bits` < 64. Returns the bits shifted out.
pub(crate) fn shl_small(a: &mut [u64], bits: u32) -> u64 {
    if bits == 0 {
        return 0;
    }
    let mut carry = 0u64;
    for limb in a.iter_mut() {
        let new_carry = *limb >> (64 - bits);
        *limb = (*limb << bits) | carry;
        carry = new_carry;
    }
    carry
}

/// In-place right shift by `bits` < 64.
pub(crate) fn shr_small(a: &mut [u64], bits: u32) {
    if bits == 0 {
        return;
    }
    let mut carry = 0u64;
    for limb in a.iter_mut().rev() {
        let new_carry = *limb << (64 - bits);
        *limb = (*limb >> bits) | carry;
        carry = new_carry;
    }
}

/// In-place multiply by a small scalar. Returns the carry out.
fn mul_small(a: &mut [u64], m: u64) -> u64 {
    let mut carry = 0u64;
    for limb in a.iter_mut() {
        let p = (*limb as u128) * (m as u128) + (carry as u128);
        *limb = p as u64;
        carry = (p >> 64) as u64;
    }
    carry
}

/// In-place divide by a small scalar. Returns the remainder.
fn div_small(a: &mut [u64], d: u64) -> u64 {
    let mut rem = 0u64;
    for limb in a.iter_mut().rev() {
        let cur = ((rem as u128) << 64) | (*limb as u128);
        *limb = (cur / (d as u128)) as u64;
        rem = (cur % (d as u128)) as u64;
    }
    rem
}

/// Truncated fixed-point multiply: `c = (a · b) >> (64·n − INT_BITS)`,
/// all three `n` limbs.
///
/// Only the partial products landing in the top window are computed —
/// `i + j ≥ n − 2` — which is the plan's truncated multiplication
/// (roughly the n(n+1)/2 high half instead of n²). The lowest kept
/// limb absorbs the carries of the dropped region; the dropped
/// products contribute strictly less than `n` ulps of that limb, which
/// the guard limb is sized to absorb.
fn mul_trunc(a: &[u64], b: &[u64], c: &mut [u64]) {
    let n = a.len();
    debug_assert_eq!(b.len(), n);
    debug_assert_eq!(c.len(), n);

    // Accumulate product limbs n−2 .. 2n−1 (window + one guard below).
    // acc[k] holds product limb (n − 2 + k), k in 0..n+2.
    let mut acc = vec![0u64; n + 2];
    for i in 0..n {
        let lo_j = (n as isize - 2 - i as isize).max(0) as usize;
        let mut carry = 0u64;
        for j in lo_j..n {
            let k = i + j - (n - 2);
            let p = (a[i] as u128) * (b[j] as u128)
                + (acc[k] as u128)
                + (carry as u128);
            acc[k] = p as u64;
            carry = (p >> 64) as u64;
        }
        // Propagate the final carry upward.
        let mut k = i + n - (n - 2);
        while carry != 0 && k < acc.len() {
            let (s, o) = acc[k].overflowing_add(carry);
            acc[k] = s;
            carry = o as u64;
            k += 1;
        }
        debug_assert_eq!(carry, 0, "product overflowed INT_BITS headroom");
    }

    // The full product's limb (n−1) starts at acc[1]; we want
    // (product >> (64·n − INT_BITS)) = (product >> 64·(n−1)) >> (64 − INT_BITS).
    shr_small(&mut acc[1..], 64 - INT_BITS);
    c.copy_from_slice(&acc[1..n + 1]);
}

// ============================================================
// FixedPoint value type
// ============================================================

/// A signed fixed-point number: `± mag / 2^frac_bits(limbs.len())`.
/// Zero is canonicalized to non-negative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedPoint {
    pub neg: bool,
    pub limbs: Vec<u64>,
}

impl FixedPoint {
    pub fn zero(n_limbs: usize) -> Self {
        Self { neg: false, limbs: vec![0; n_limbs] }
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.iter().all(|&l| l == 0)
    }

    fn canonicalize(&mut self) {
        if self.neg && self.is_zero() {
            self.neg = false;
        }
    }

    pub fn n_limbs(&self) -> usize {
        self.limbs.len()
    }

    /// Signed addition.
    pub fn add(&self, other: &Self) -> Self {
        let n = self.n_limbs();
        debug_assert_eq!(other.n_limbs(), n);
        let mut out = Self::zero(n);
        if self.neg == other.neg {
            let carry = add_mag(&self.limbs, &other.limbs, &mut out.limbs);
            debug_assert!(!carry, "addition overflowed INT_BITS headroom");
            out.neg = self.neg;
        } else {
            match cmp_mag(&self.limbs, &other.limbs) {
                std::cmp::Ordering::Less => {
                    sub_mag(&other.limbs, &self.limbs, &mut out.limbs);
                    out.neg = other.neg;
                }
                _ => {
                    sub_mag(&self.limbs, &other.limbs, &mut out.limbs);
                    out.neg = self.neg;
                }
            }
        }
        out.canonicalize();
        out
    }

    /// Signed subtraction.
    pub fn sub(&self, other: &Self) -> Self {
        let flipped = Self { neg: !other.neg, limbs: other.limbs.clone() };
        self.add(&flipped)
    }

    /// Truncated fixed-point multiply.
    pub fn mul(&self, other: &Self) -> Self {
        let n = self.n_limbs();
        debug_assert_eq!(other.n_limbs(), n);
        let mut out = Self::zero(n);
        mul_trunc(&self.limbs, &other.limbs, &mut out.limbs);
        out.neg = self.neg != other.neg;
        out.canonicalize();
        out
    }

    /// Multiply by 2 (one-bit left shift) — complex squaring's `2xy`.
    pub fn double(&self) -> Self {
        let mut out = self.clone();
        let carry = shl_small(&mut out.limbs, 1);
        debug_assert_eq!(carry, 0, "doubling overflowed INT_BITS headroom");
        out
    }

    /// self², cheaper composition point for callers (same cost as mul
    /// here; the two-big-mul complex squaring trick lives in
    /// [`FixedComplex::sqr`]).
    pub fn sqr(&self) -> Self {
        let mut out = self.mul(self);
        out.neg = false;
        out
    }

    /// Reciprocal by Newton iteration, for `|self| >= 1`.
    ///
    /// The module header says the core never divides, and that stayed
    /// true while every reference map was a polynomial. The RATIONAL
    /// families need it: Feather iterates `z^p / (1 + x^2 - i*y^2)`,
    /// and its reference orbit cannot be built without dividing.
    ///
    /// **Restricted to `|self| >= 1` on purpose.** This is fixed point
    /// with [`INT_BITS`] = 8 integer bits, so the representable range
    /// is about +-128; a reciprocal of anything smaller than 1/128
    /// simply does not fit, and near a pole it would not fit by a wide
    /// margin. Rather than saturate — which would put a quietly wrong
    /// reference orbit into the cache — this returns `None` and lets
    /// the caller decline. Feather is safe by construction (its
    /// denominator's real part is `1 + x^2`, so `|D| >= 1` always),
    /// which is exactly why it is the rational family that could ship
    /// first.
    ///
    /// Newton doubles the correct bits each step: `x <- x*(2 - a*x)`,
    /// seeded from f64 (53 bits) and run to cover the full limb width.
    /// Cost is two full-width multiplies per step, so a reference
    /// iteration that divides is several times a polynomial one. The
    /// standard fix — run the early steps at reduced precision, since
    /// they only need to be right to their own width — is left for
    /// when a profile says it matters; reference orbits are cached to
    /// disk, so this is a one-off per location.
    pub fn recip(&self) -> Option<Self> {
        let n = self.n_limbs();
        if self.is_zero() {
            return None;
        }
        let a = self.to_f64();
        // 1/|a| must fit the +-2^INT_BITS range with room to spare.
        if !(a.abs() >= 1.0) || !a.is_finite() {
            return None;
        }
        let mut x = Self::from_f64(1.0 / a, n);
        let two = Self::from_f64(2.0, n);
        // 53 correct bits doubling each step, to the full width.
        let target_bits = frac_bits(n) + INT_BITS;
        let mut good = 50u32;
        while good < target_bits {
            // x <- x * (2 - a*x)
            let ax = self.mul(&x);
            let corr = two.sub(&ax);
            x = x.mul(&corr);
            good = good.saturating_mul(2);
        }
        Some(x)
    }
}

impl FixedPoint {
    /// Reciprocal with an explicit power-of-two scale: returns `(r, k)`
    /// with `1/self == r * 2^k` and `|r|` in `(0.5, 1]`.
    ///
    /// This is what lets the POLE-BEARING families divide at all.
    /// [`Self::recip`] can only invert `|a| >= 1`, because fixed point
    /// with [`INT_BITS`] = 8 stops at ±128 — but a map like McMullen's
    /// `z^n + c/z^m` legitimately visits `|z| < 1`, where the plain
    /// reciprocal is out of range and the QUOTIENT is still perfectly
    /// representable. Normalizing first separates the two questions:
    /// the reciprocal is always taken of a value in [1,2), and whether
    /// the answer fits is decided on the answer.
    ///
    /// The normalizing shift is exact — `to_floatexp` finds the MSB by
    /// leading-zero count, with no rounding — and left-shifting cannot
    /// lose bits here, because the MSB lands at exponent 0. What a
    /// tiny input does cost is SIGNIFICANCE: a value whose MSB sits k
    /// bits below the binary point carries only `frac_bits - k`
    /// meaningful bits, and the quotient inherits that. Near a genuine
    /// pole that is severe — and there the orbit is about to escape,
    /// which is the honest answer anyway.
    pub fn recip_scaled(&self) -> Option<(Self, i64)> {
        if self.is_zero() {
            return None;
        }
        let fe = self.to_floatexp();
        // self == m * 2^e with m in [1,2), so self * 2^-e is in [1,2)
        // and 1/self == recip(self * 2^-e) * 2^-e.
        let k = -fe.e;
        let mut scaled = self.clone();
        scaled.shift_pow2(k);
        let r = scaled.recip()?;
        Some((r, k))
    }
}

impl FixedComplex {
    /// Complex division `self / other`, via `conj` over the squared
    /// magnitude — so the only reciprocal taken is of a REAL value,
    /// which is where [`FixedPoint::recip`]'s range guarantee lives.
    ///
    /// `None` exactly when the QUOTIENT does not fit the fixed-point
    /// range (±2^[`INT_BITS`]), which for a pole-bearing map is the
    /// same event as "this orbit has escaped" — `z^n + c/z^m` at tiny
    /// `z` produces a huge iterate, and the shader's own pole sentinel
    /// says the same thing by feeding a large value into the bailout.
    ///
    /// Note what this does NOT refuse: a small denominator. Dividing
    /// by 0.1 is fine as long as the answer fits, which
    /// [`FixedPoint::recip_scaled`] arranges by normalizing before
    /// inverting. An earlier version refused `|other|^2 < 1` outright
    /// — a statement about the implementation rather than the
    /// requirement, and one that would have kept McMullen and Magnet
    /// out for no reason.
    pub fn div(&self, other: &Self) -> Option<Self> {
        let d2 = other.re.sqr().add(&other.im.sqr());
        let (inv, k) = d2.recip_scaled()?;
        let mut conj_im = other.im.clone();
        conj_im.neg = !conj_im.neg && !conj_im.is_zero();
        let num = self.mul(&FixedComplex { re: other.re.clone(), im: conj_im });
        let mut re = num.re.mul(&inv);
        let mut im = num.im.mul(&inv);
        // The scale is applied last, so overflow is decided on the
        // answer. Checked BEFORE shifting: shift_pow2 drops bits off
        // the top silently, which would turn an escaped orbit into a
        // plausible small one.
        if k > 0 {
            for v in [&re, &im] {
                if !v.is_zero() && v.to_floatexp().e + k >= INT_BITS as i64 {
                    return None;
                }
            }
        }
        re.shift_pow2(k);
        im.shift_pow2(k);
        Some(FixedComplex { re, im })
    }
}

impl FixedPoint {
    // --------------------------------------------------------
    // Conversions
    // --------------------------------------------------------

    /// Parse a decimal string (`-1.7499`, `0.001`, `2`) at the given
    /// limb count. This is the exact-center entry point: the escape
    /// config's decimal-string center parses straight to full
    /// precision, no f64 in between.
    pub fn from_decimal(s: &str, n_limbs: usize) -> Option<Self> {
        let s = s.trim();
        let (neg, rest) = match s.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, s.strip_prefix('+').unwrap_or(s)),
        };
        if rest.is_empty() {
            return None;
        }
        let (int_part, frac_part) = match rest.split_once('.') {
            Some((i, f)) => (i, f),
            None => (rest, ""),
        };
        if int_part.is_empty() && frac_part.is_empty() {
            return None;
        }
        if !int_part.bytes().all(|b| b.is_ascii_digit())
            || !frac_part.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }

        // Fraction, most-significant digit last: r = (r + d) / 10.
        let mut out = Self::zero(n_limbs);
        for b in frac_part.bytes().rev() {
            let d = (b - b'0') as u64;
            // r += d  (as an integer at the top of the representation)
            add_int_u64(&mut out.limbs, d);
            let rem = div_small(&mut out.limbs, 10);
            let _ = rem; // truncation below the lowest limb — guard absorbs
        }

        // Integer part: value += int (must fit INT_BITS).
        let mut int_val = 0u64;
        for b in int_part.bytes() {
            int_val = int_val.checked_mul(10)?.checked_add((b - b'0') as u64)?;
            if int_val >= (1u64 << (INT_BITS - 1)) {
                return None; // out of headroom
            }
        }
        add_int_u64(&mut out.limbs, int_val);

        out.neg = neg;
        out.canonicalize();
        Some(out)
    }

    /// Format as a decimal string with `digits` fractional digits.
    pub fn to_decimal(&self, digits: usize) -> String {
        let mut s = String::new();
        if self.neg {
            s.push('-');
        }
        let int = self.int_part();
        s.push_str(&int.to_string());
        if digits > 0 {
            s.push('.');
            let mut frac = self.clone();
            clear_int_part(&mut frac.limbs);
            for _ in 0..digits {
                let carry = mul_small(&mut frac.limbs, 10);
                // The integer part that just appeared is carry·2^INT_BITS
                // plus the top INT_BITS of the top limb.
                let d = (carry << INT_BITS) | take_int_part(&mut frac.limbs);
                debug_assert!(d < 10);
                s.push((b'0' + d as u8) as char);
            }
        }
        s
    }

    fn int_part(&self) -> u64 {
        let top = *self.limbs.last().unwrap();
        top >> (64 - INT_BITS)
    }

    /// `dec + delta` exactly: parse the decimal at the precision the
    /// zoom needs, add the (small) f64 delta in fixed-point, format
    /// back with enough digits that nothing is lost.
    ///
    /// This is what pan and zoom-to-cursor must use past ~zoom 45:
    /// f64 round-tripping the CENTER caps the absolute step at the
    /// center's own ulp (~2.2e-16 near |re| = 1.4) while the pixel
    /// spacing keeps shrinking — horizontal pans "skip" while the
    /// small-imaginary axis still works, exactly the reported
    /// symptom. The DELTA itself is fine in f64 (it only needs
    /// relative precision); the accumulation is what needs exactness.
    pub fn decimal_add_f64(dec: &str, delta: f64, zoom_log2: f64) -> Option<String> {
        if !delta.is_finite() {
            return None;
        }
        Self::decimal_add_floatexp(dec, delta, 0, zoom_log2)
    }

    /// `dec + m·2^e2` exactly — the any-depth form of
    /// [`decimal_add_f64`](Self::decimal_add_f64): the mantissa
    /// carries the shape of the delta, the exponent its scale, so
    /// pixel-sized steps survive past f64's exponent range.
    pub fn decimal_add_floatexp(dec: &str, m: f64, e2: i64, zoom_log2: f64) -> Option<String> {
        if !m.is_finite() {
            return None;
        }
        // Precision follows the DEEPER of (what the zoom needs, what
        // the input already carries): a curated location can hold
        // thousands of digits — valid far past the current view — and
        // reformatting it to zoom-proportional digits on a pan would
        // silently truncate the location's depth.
        let dec = dec.trim();
        let frac_digits = dec
            .split_once('.')
            .map(|(_, f)| f.trim_end_matches(|c: char| !c.is_ascii_digit()).len())
            .unwrap_or(0);
        let auto_digits = (zoom_log2.max(0.0) * 0.30103) as usize + 24;
        let digits = auto_digits.max(frac_digits);
        let n = (limbs_for_zoom(zoom_log2) + 1)
            .max((digits * 10) / 192 + 2); // digits·log2(10)/64 limbs, ceil-ish
        let base = Self::from_decimal(dec, n)?;
        let d = Self::from_floatexp(m, e2, n);
        Some(base.add(&d).to_decimal(digits))
    }

    /// Construct `m · 2^e2` exactly. `m` needs only f64 RELATIVE
    /// precision (53 mantissa bits); the exponent rides separately,
    /// so pixel-sized deltas stay constructible at ANY zoom — plain
    /// f64 values underflow to zero past ~zoom 1060.
    pub fn from_floatexp(m: f64, e2: i64, n_limbs: usize) -> Self {
        if m == 0.0 || !m.is_finite() {
            return Self::zero(n_limbs);
        }
        // Normalize |m| into [1, 2) and fold the remainder into the
        // exponent so the f64 seed always fits the headroom window.
        let lg = m.abs().log2().floor();
        let mant = m / 2f64.powi(lg as i32);
        let mut v = Self::from_f64(mant, n_limbs);
        v.shift_pow2(e2.saturating_add(lg as i64));
        v
    }

    /// Multiply by 2^e in place: whole-limb moves plus a sub-limb
    /// shift. Bits leaving the representable range drop (right shifts
    /// underflow toward zero; left shifts past the integer headroom
    /// would be a caller bug — deltas here are always sub-unit).
    pub fn shift_pow2(&mut self, e: i64) {
        if e == 0 || self.is_zero() {
            return;
        }
        let n = self.limbs.len();
        let mag = e.unsigned_abs();
        let limb_shift = (mag / 64) as usize;
        let bit_shift = (mag % 64) as u32;
        if limb_shift >= n {
            self.limbs.fill(0);
            self.canonicalize();
            return;
        }
        if e > 0 {
            // Left: toward the top limb.
            for i in (0..n).rev() {
                self.limbs[i] = if i >= limb_shift { self.limbs[i - limb_shift] } else { 0 };
            }
            if bit_shift > 0 {
                shl_small(&mut self.limbs, bit_shift);
            }
        } else {
            // Right: toward zero.
            for i in 0..n {
                self.limbs[i] = if i + limb_shift < n { self.limbs[i + limb_shift] } else { 0 };
            }
            if bit_shift > 0 {
                shr_small(&mut self.limbs, bit_shift);
            }
        }
        self.canonicalize();
    }

    pub fn from_f64(v: f64, n_limbs: usize) -> Self {
        let neg = v < 0.0;
        assert!(
            v.abs() < (1u64 << (INT_BITS - 1)) as f64,
            "from_f64 out of INT_BITS headroom"
        );
        let mut result = Self::zero(n_limbs);
        let mut acc = v.abs();
        // Integer part into the headroom window, then fractional bits
        // limb by limb (an f64 carries 53 significant bits, so this
        // terminates after at most two limbs of work).
        let int = acc.floor();
        add_int_u64(&mut result.limbs, int as u64);
        acc -= int;
        for i in (0..n_limbs).rev() {
            if acc == 0.0 {
                break;
            }
            let take = if i == n_limbs - 1 { 64 - INT_BITS } else { 64 };
            acc *= 2f64.powi(take as i32);
            let part = acc.floor();
            acc -= part;
            if i == n_limbs - 1 {
                result.limbs[i] |= part as u64;
            } else {
                result.limbs[i] = part as u64;
            }
        }
        result.neg = neg;
        result.canonicalize();
        result
    }

    pub fn to_f64(&self) -> f64 {
        let fe = self.to_floatexp();
        fe.to_f64()
    }

    /// Exact export to mantissa + exponent form via leading-zero count
    /// — the operation the plan calls out as first-class: near-zero
    /// orbit values must convert exactly (2Zₙ at tiny |Zₙ| is where
    /// rebasing lives).
    pub fn to_floatexp(&self) -> FloatExp {
        // Find the highest set bit.
        let mut top = None;
        for i in (0..self.limbs.len()).rev() {
            if self.limbs[i] != 0 {
                top = Some((i, 63 - self.limbs[i].leading_zeros()));
                break;
            }
        }
        let (limb_i, bit_i) = match top {
            None => return FloatExp { m: 0.0, e: 0 },
            Some(t) => t,
        };
        // Absolute bit position of the MSB, counting from the binary
        // point: bit b of limb i sits at exponent
        // (64·i + b) − frac_bits.
        let msb_exp = (64 * limb_i as i64 + bit_i as i64) - frac_bits(self.n_limbs()) as i64;

        // Collect the top 53 bits starting at the MSB.
        let mut mantissa = 0u64;
        for k in 0..53u64 {
            let pos = 64 * limb_i as i64 + bit_i as i64 - k as i64;
            if pos < 0 {
                break;
            }
            let li = (pos / 64) as usize;
            let bi = (pos % 64) as u32;
            if (self.limbs[li] >> bi) & 1 == 1 {
                mantissa |= 1u64 << (52 - k);
            }
        }
        let m = (mantissa as f64) / (1u64 << 52) as f64; // in [1, 2)
        FloatExp {
            m: if self.neg { -m } else { m },
            e: msb_exp,
        }
    }
}

/// Add a small integer to the value (i.e. `v += k` where k is an
/// integer, landing in the INT_BITS window at the top).
fn add_int_u64(limbs: &mut [u64], k: u64) {
    let n = limbs.len();
    debug_assert!(k < (1 << INT_BITS));
    let top = &mut limbs[n - 1];
    let (s, o) = top.overflowing_add(k << (64 - INT_BITS));
    debug_assert!(!o, "integer add overflowed headroom");
    *top = s;
}

/// Zero the integer part in place.
fn clear_int_part(limbs: &mut [u64]) {
    let n = limbs.len();
    limbs[n - 1] &= u64::MAX >> INT_BITS;
}

/// Read and clear the integer part.
fn take_int_part(limbs: &mut [u64]) -> u64 {
    let n = limbs.len();
    let v = limbs[n - 1] >> (64 - INT_BITS);
    clear_int_part(limbs);
    v
}

// ============================================================
// FloatExp — mantissa + wide exponent (CPU side)
// ============================================================

/// Extended-range float: `m · 2^e` with |m| in [1, 2) (or 0). The CPU
/// twin of the WGSL floatexp the delta pipeline will use; f64
/// mantissa here, truncated to f32 at upload.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloatExp {
    pub m: f64,
    pub e: i64,
}

impl FloatExp {
    pub fn zero() -> Self {
        Self { m: 0.0, e: 0 }
    }

    pub fn from_f64(v: f64) -> Self {
        if v == 0.0 || !v.is_finite() {
            return Self { m: 0.0, e: 0 };
        }
        let e = v.abs().log2().floor() as i64;
        let m = v / 2f64.powi(e as i32);
        Self { m, e }
    }

    pub fn to_f64(self) -> f64 {
        if self.m == 0.0 {
            return 0.0;
        }
        if self.e > 1023 {
            return if self.m > 0.0 { f64::INFINITY } else { f64::NEG_INFINITY };
        }
        if self.e < -1070 {
            return 0.0;
        }
        self.m * 2f64.powi(self.e as i32)
    }

    fn renorm(self) -> Self {
        if self.m == 0.0 {
            return Self::zero();
        }
        let shift = self.m.abs().log2().floor() as i64;
        Self { m: self.m / 2f64.powi(shift as i32), e: self.e + shift }
    }

    /// Product (exact exponent bookkeeping, f64 mantissa rounding).
    pub fn mul(self, other: Self) -> Self {
        if self.m == 0.0 || other.m == 0.0 {
            return Self::zero();
        }
        Self { m: self.m * other.m, e: self.e + other.e }.renorm()
    }

    /// Sum; addends more than ~60 octaves apart collapse to the larger.
    pub fn add(self, other: Self) -> Self {
        if self.m == 0.0 {
            return other;
        }
        if other.m == 0.0 {
            return self;
        }
        let (hi, lo) = if self.e >= other.e { (self, other) } else { (other, self) };
        let d = hi.e - lo.e;
        if d > 60 {
            return hi;
        }
        Self { m: hi.m + lo.m / 2f64.powi(d as i32), e: hi.e }.renorm()
    }

    /// |self| < |other| (magnitude comparison across the full range).
    pub fn abs_less_than(self, other: Self) -> bool {
        if self.m == 0.0 {
            return other.m != 0.0;
        }
        if other.m == 0.0 {
            return false;
        }
        // Renormalized invariants: |m| in [1, 2).
        let a = self.renorm();
        let b = other.renorm();
        if a.e != b.e {
            return a.e < b.e;
        }
        a.m.abs() < b.m.abs()
    }
}

// ============================================================
// Complex wrapper
// ============================================================

/// Complex fixed-point value. Squaring uses the plan's two-big-mul
/// identity — Re = (x+y)(x−y), Im = 2xy (the 2 is a shift) — a free
/// 33% since the big multiply is the entire runtime. The two muls are
/// independent, which is the only parallelism orbits have.
#[derive(Clone, Debug)]
pub struct FixedComplex {
    pub re: FixedPoint,
    pub im: FixedPoint,
}

impl FixedComplex {
    pub fn zero(n_limbs: usize) -> Self {
        Self { re: FixedPoint::zero(n_limbs), im: FixedPoint::zero(n_limbs) }
    }

    /// z² via two multiplies.
    pub fn sqr(&self) -> Self {
        let re = self.re.add(&self.im).mul(&self.re.sub(&self.im));
        let im = self.re.mul(&self.im).double();
        Self { re, im }
    }

    /// General complex multiply (four big muls — used by the
    /// Multibrot reference's power chain; squaring stays on the
    /// two-mul fast path).
    pub fn mul(&self, other: &Self) -> Self {
        let re = self.re.mul(&other.re).sub(&self.im.mul(&other.im));
        let im = self.re.mul(&other.im).add(&self.im.mul(&other.re));
        Self { re, im }
    }

    pub fn add(&self, other: &Self) -> Self {
        Self { re: self.re.add(&other.re), im: self.im.add(&other.im) }
    }

    /// |z|² as f64 — for the escape check (the reference escapes at
    /// |Z| > 2; f64 is plenty for a threshold comparison).
    pub fn norm_sqr_f64(&self) -> f64 {
        let x = self.re.to_f64();
        let y = self.im.to_f64();
        x * x + y * y
    }
}

#[cfg(test)]
mod recip_tests {
    use super::*;

    /// The Newton reciprocal must be exact to the FULL limb width,
    /// not merely to f64 — that is the whole reason it exists.
    ///
    /// Checked without a bignum library: `a * (1/a)` must come back as
    /// 1 to within a few ulps of the fixed-point representation. A
    /// reciprocal that merely converted through f64 would be right to
    /// 2^-53 and fail this by hundreds of digits.
    #[test]
    fn reciprocal_is_exact_to_the_full_width() {
        for n in [4usize, 12, 40] {
            let one = FixedPoint::from_f64(1.0, n);
            for v in [1.0f64, 1.5, 2.0, 3.14159265358979, 7.0, 99.5, -1.25, -64.0] {
                let a = FixedPoint::from_f64(v, n);
                let inv = a.recip().expect("in range");
                let prod = a.mul(&inv);
                let err = prod.sub(&one);
                // Allowed: a handful of ulps at the bottom limb, from
                // the truncating multiplies inside Newton.
                // limbs[0] is LEAST significant, so a full-width
                // reciprocal leaves error only in the bottom limb or
                // two (the truncating multiplies inside Newton).
                let top = err.limbs.iter().rposition(|&l| l != 0);
                assert!(
                    top.map_or(true, |i| i <= 1),
                    "1/{v} at {n} limbs: a*(1/a) - 1 is nonzero up to limb {top:?} of \
                     {n}, so the reciprocal is not full-width"
                );
            }
        }
    }

    /// Out of range must REFUSE, not saturate.
    ///
    /// With 8 integer bits the representable range is about +-128, so
    /// 1/a for |a| < 1 can overflow it. Saturating there would write a
    /// quietly wrong reference orbit into the on-disk cache, which is
    /// the failure this project least wants; `None` makes the caller
    /// decide.
    #[test]
    fn reciprocal_refuses_what_it_cannot_represent() {
        let n = 8;
        for v in [0.5f64, 0.01, 1e-9, -0.25] {
            let a = FixedPoint::from_f64(v, n);
            assert!(
                a.recip().is_none(),
                "recip({v}) should refuse: 1/{v} does not fit {} integer bits",
                INT_BITS
            );
        }
        assert!(FixedPoint::zero(n).recip().is_none(), "1/0 must refuse");
    }

    /// Complex division must match an f64 oracle, and must refuse
    /// exactly when the magnitude is below the representable range.
    #[test]
    fn complex_division_matches_f64_and_refuses_poles() {
        let n = 16;
        let cases = [
            ((1.0f64, 2.0f64), (3.0f64, -1.0f64)),
            ((-0.75, 0.5), (1.0, 0.0)),
            ((2.5, -3.25), (-2.0, 1.5)),
        ];
        for ((ar, ai), (br, bi)) in cases {
            let a = FixedComplex {
                re: FixedPoint::from_f64(ar, n),
                im: FixedPoint::from_f64(ai, n),
            };
            let b = FixedComplex {
                re: FixedPoint::from_f64(br, n),
                im: FixedPoint::from_f64(bi, n),
            };
            let q = a.div(&b).expect("in range");
            let d = br * br + bi * bi;
            let (wr, wi) = ((ar * br + ai * bi) / d, (ai * br - ar * bi) / d);
            assert!(
                (q.re.to_f64() - wr).abs() < 1e-12 && (q.im.to_f64() - wi).abs() < 1e-12,
                "({ar}+{ai}i)/({br}+{bi}i) = {}+{}i, want {wr}+{wi}i",
                q.re.to_f64(),
                q.im.to_f64()
            );
        }
        // A SMALL denominator is fine as long as the answer fits —
        // this is what recip_scaled buys, and what the pole-bearing
        // families need. 1/(0.1+0.2i) = 2 - 4i, comfortably in range.
        let small = FixedComplex {
            re: FixedPoint::from_f64(0.1, n),
            im: FixedPoint::from_f64(0.2, n),
        };
        let one = FixedComplex {
            re: FixedPoint::from_f64(1.0, n),
            im: FixedPoint::zero(n),
        };
        let q = one.div(&small).expect("small divisor, in-range quotient");
        assert!(
            (q.re.to_f64() - 2.0).abs() < 1e-12 && (q.im.to_f64() + 4.0).abs() < 1e-12,
            "1/(0.1+0.2i) = {}+{}i, want 2-4i",
            q.re.to_f64(),
            q.im.to_f64()
        );

        // What it MUST refuse is a quotient that does not fit: with 8
        // integer bits the range stops at ±128, and for a map with a
        // pole that refusal IS the escape signal. Saturating instead
        // would turn an escaped orbit into a plausible small one and
        // write it to the on-disk cache.
        let tiny = FixedComplex {
            re: FixedPoint::from_f64(1.0 / 4096.0, n),
            im: FixedPoint::zero(n),
        };
        assert!(
            one.div(&tiny).is_none(),
            "1/(1/4096) = 4096 is out of range and must refuse, not wrap"
        );
    }

    /// The scaled reciprocal must be right BELOW 1, where the plain
    /// one cannot reach — that is its whole reason for existing.
    #[test]
    fn scaled_reciprocal_reaches_below_one() {
        let n = 16;
        for v in [0.5f64, 0.1, 0.01, 1.0 / 1024.0, -0.125, 3.0, 100.0] {
            let a = FixedPoint::from_f64(v, n);
            let (r, k) = a.recip_scaled().expect("nonzero");
            // Contract: 1/self == r * 2^k.
            let got = r.to_f64() * (k as f64).exp2();
            let want = 1.0 / v;
            assert!(
                (got - want).abs() <= want.abs() * 1e-12,
                "recip_scaled({v}) = {} * 2^{k} = {got}, want {want}",
                r.to_f64()
            );
            // And the mantissa it inverted was normalized, so |r| is
            // in (0.5, 1] whatever the input's magnitude.
            let m = r.to_f64().abs();
            assert!(m > 0.5 - 1e-12 && m <= 1.0 + 1e-12, "recip_scaled({v}) mantissa {m}");
        }
        assert!(FixedPoint::zero(8).recip_scaled().is_none(), "1/0 must refuse");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 4; // 256 bits: 8 int + 248 frac

    fn fp(s: &str) -> FixedPoint {
        FixedPoint::from_decimal(s, N).unwrap()
    }

    #[test]
    fn decimal_round_trips() {
        for s in ["0.5", "-0.7453", "0.1127", "1.25", "-2", "0.001953125", "3.75"] {
            let v = fp(s);
            let out = v.to_decimal(30);
            let back: f64 = out.parse().unwrap();
            let orig: f64 = s.parse().unwrap();
            assert!(
                (back - orig).abs() < 1e-25,
                "{s} -> {out} (drifted)"
            );
        }
    }

    #[test]
    fn f64_conversions_agree() {
        for v in [0.5, -0.7453, 0.1127, 1.9999, -3.5, 0.0, 1e-15, -1e-15] {
            let x = FixedPoint::from_f64(v, N);
            assert!(
                (x.to_f64() - v).abs() <= v.abs() * 1e-15,
                "{v} -> {} (drifted)",
                x.to_f64()
            );
        }
    }

    #[test]
    fn arithmetic_matches_f64_at_shallow_precision() {
        let a = fp("1.375");
        let b = fp("-0.7453");
        assert!((a.add(&b).to_f64() - (1.375 + -0.7453)).abs() < 1e-14);
        assert!((a.sub(&b).to_f64() - (1.375 - -0.7453)).abs() < 1e-14);
        assert!((a.mul(&b).to_f64() - (1.375 * -0.7453)).abs() < 1e-14);
        assert!((b.sqr().to_f64() - 0.7453f64 * 0.7453).abs() < 1e-14);
        assert!((b.double().to_f64() - -1.4906).abs() < 1e-14);
    }

    #[test]
    fn mandelbrot_orbit_matches_f64() {
        // 60 iterations at a non-escaping-ish c: fixed-point and f64
        // must agree to f64's own roundoff class.
        let c = FixedComplex {
            re: fp("-0.7453"),
            im: fp("0.1127"),
        };
        let mut z = FixedComplex::zero(N);
        let (mut zx, mut zy) = (0.0f64, 0.0f64);
        for i in 0..60 {
            z = z.sqr().add(&c);
            let t = zx * zx - zy * zy + -0.7453;
            zy = 2.0 * zx * zy + 0.1127;
            zx = t;
            let dx = (z.re.to_f64() - zx).abs();
            let dy = (z.im.to_f64() - zy).abs();
            // f64 itself accumulates error; allow its drift, not ours.
            assert!(
                dx < 1e-9 && dy < 1e-9,
                "iteration {i}: fixed ({}, {}) vs f64 ({zx}, {zy})",
                z.re.to_f64(),
                z.im.to_f64()
            );
        }
    }

    #[test]
    fn deep_precision_survives_where_f64_dies() {
        // A center perturbation far below f64 resolution must be
        // representable and round-trip through decimal exactly enough
        // to distinguish from the unperturbed value.
        let n = limbs_for_zoom(200.0); // ~2^-200 pixel scale
        let base = "-0.74530000000000000000000000000000000000000000000000000000000000001";
        let a = FixedPoint::from_decimal(base, n).unwrap();
        let b = FixedPoint::from_decimal("-0.7453", n).unwrap();
        let diff = a.sub(&b);
        assert!(!diff.is_zero(), "deep digits were lost");
        let fe = diff.to_floatexp();
        // 1e-65 ~ 2^-216
        assert!(fe.e < -210 && fe.e > -222, "unexpected magnitude 2^{}", fe.e);
    }

    #[test]
    fn floatexp_export_is_exact_near_zero() {
        // A value with a single bit set far down must export with the
        // exact exponent and mantissa 1.0.
        let mut v = FixedPoint::zero(N);
        v.limbs[0] = 1; // the lowest representable bit
        let fe = v.to_floatexp();
        assert_eq!(fe.m, 1.0);
        assert_eq!(fe.e, -(frac_bits(N) as i64));
        // And zero exports as zero.
        assert_eq!(FixedPoint::zero(N).to_floatexp().m, 0.0);
    }

    #[test]
    fn sign_cases() {
        let a = fp("0.5");
        let b = fp("-0.75");
        assert_eq!(a.add(&b).to_decimal(4), "-0.2500");
        assert_eq!(b.add(&a).to_decimal(4), "-0.2500");
        assert_eq!(a.sub(&b).to_decimal(4), "1.2500");
        assert_eq!(b.sub(&a).to_decimal(4), "-1.2500");
        assert_eq!(a.mul(&b).to_decimal(4), "-0.3750");
        assert_eq!(b.mul(&b).to_decimal(4), "0.5625");
        // Zero is canonicalized non-negative.
        let z = a.sub(&a);
        assert!(z.is_zero());
        assert!(!z.neg);
    }

    #[test]
    fn decimal_add_preserves_deep_precision() {
        // The reported failure: pixel-sized steps at zoom 50 near
        // re = -1.414... are ~4e-18 — under f64's ulp there (2.2e-16),
        // so an f64 round-trip drops them entirely. The fixed-point
        // path must accumulate 100 such steps to their exact sum.
        let step = 4.0e-18;
        let mut re = "-1.4143355295031044".to_string();
        for _ in 0..100 {
            re = FixedPoint::decimal_add_f64(&re, step, 50.0).unwrap();
        }
        let n = limbs_for_zoom(50.0) + 1;
        let moved = FixedPoint::from_decimal(&re, n)
            .unwrap()
            .sub(&FixedPoint::from_decimal("-1.4143355295031044", n).unwrap());
        let total = moved.to_f64();
        assert!(
            (total - 100.0 * step).abs() < 1e-24,
            "accumulated {total:e}, expected {:e}",
            100.0 * step
        );
        // Control: the f64 path loses every step.
        let f64_way = -1.4143355295031044f64 + step;
        assert_eq!(f64_way, -1.4143355295031044, "f64 keeps the step?!");
    }

    #[test]
    fn from_floatexp_matches_from_f64_and_shifts_exactly() {
        let n = 4;
        for v in [0.375f64, -1.5, 0.001953125] {
            assert_eq!(FixedPoint::from_floatexp(v, 0, n), FixedPoint::from_f64(v, n));
            // A ±small shift equals scaling the f64 (still exact).
            assert_eq!(
                FixedPoint::from_floatexp(v, -3, n),
                FixedPoint::from_f64(v / 8.0, n)
            );
            assert_eq!(
                FixedPoint::from_floatexp(v, 2, n),
                FixedPoint::from_f64(v * 4.0, n)
            );
        }
        assert!(FixedPoint::from_floatexp(f64::NAN, 0, n).is_zero());
    }

    #[test]
    fn decimal_add_floatexp_survives_past_f64_range() {
        // A pixel-sized step at zoom 2000: 2^-2005 underflows f64
        // outright — the symbolic form must land it. The decimal
        // format is zoom-proportional (~626 digits here), so a dyadic
        // delta is stored to ~2^-2079 granularity, not exactly — 69
        // bits BELOW pixel size, which is the design contract. Assert
        // the step lands with far-sub-pixel accuracy, both ways.
        let z = 2000.0;
        let n = limbs_for_zoom(z) + 1;
        let stepped = FixedPoint::decimal_add_f64("0.5", 0.0, z).unwrap();
        let moved = FixedPoint::decimal_add_floatexp(&stepped, 1.5, -2005, z).unwrap();
        let delta = FixedPoint::from_decimal(&moved, n)
            .unwrap()
            .sub(&FixedPoint::from_decimal(&stepped, n).unwrap())
            .to_floatexp();
        // The step itself: 1.5·2^-2005, to ~1e-9 relative.
        assert_eq!(delta.e, -2005, "step magnitude octave"); // 1.5·2^-2005: m=1.5 ∈ [1,2), e=-2005
        let rel = (delta.m * 2f64.powi((delta.e + 2005) as i32) - 1.5).abs() / 1.5;
        assert!(rel < 1e-9, "step landed with relative error {rel:e}");
        // Round trip: residual at least 40 bits below pixel size.
        let back = FixedPoint::decimal_add_floatexp(&moved, -1.5, -2005, z).unwrap();
        let resid = FixedPoint::from_decimal(&back, n)
            .unwrap()
            .sub(&FixedPoint::from_decimal(&stepped, n).unwrap())
            .to_floatexp();
        assert!(
            resid.m == 0.0 || (resid.e as f64) < -(z + 40.0),
            "round-trip residual 2^{} not sub-pixel",
            resid.e
        );
    }

    #[test]
    fn decimal_add_preserves_deep_centers_at_shallow_zoom() {
        // A 3757-digit curated location panned at zoom 20 must keep
        // its depth: the fractional digits in equals the digits out.
        let deep = format!("-1.{}", "0918273645".repeat(40)); // 400 digits
        let out = FixedPoint::decimal_add_f64(&deep, 0.0, 20.0).unwrap();
        let frac_out = out.split_once('.').unwrap().1.len();
        assert!(frac_out >= 400, "pan truncated a deep center to {frac_out} digits");
        // Exact string round-trip is impossible (a decimal fraction
        // has an infinite binary expansion; the parse truncates at
        // the limb width) — the contract is that the reformat error
        // stays BELOW the input's last digit: value agreement to
        // ~10^-398 for a 400-digit center.
        let n = limbs_for_zoom(1400.0);
        let diff = FixedPoint::from_decimal(&deep, n)
            .unwrap()
            .sub(&FixedPoint::from_decimal(&out, n).unwrap())
            .to_floatexp();
        assert!(
            diff.m == 0.0 || (diff.e as f64) < -(398.0 * 3.3219),
            "reformat error 2^{} reaches into the kept digits",
            diff.e
        );
    }

    #[test]
    fn decimal_add_shallow_matches_f64() {
        // At shallow zoom the helper must agree with plain arithmetic.
        let out = FixedPoint::decimal_add_f64("-0.5", 0.125, 4.0).unwrap();
        let v: f64 = out.parse().unwrap();
        assert!((v - (-0.375)).abs() < 1e-12, "{out}");
    }

    #[test]
    fn limbs_for_zoom_scales() {
        assert!(limbs_for_zoom(0.0) >= 2);
        assert!(limbs_for_zoom(100.0) >= 3);
        assert!(limbs_for_zoom(1000.0) >= 17);
    }
}
