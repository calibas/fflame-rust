//! Bivariate linear approximation (BLA) tables — iteration skipping
//! for perturbed rendering.
//!
//! Where the delta iteration is locally linear (|δ| far below the
//! reference's magnitude), a run of 2^ℓ steps collapses to one affine
//! application δ' = A·δ + B·δc. The table is Zhuoran's improved
//! construction as described by Claude Heiland-Allen
//! (https://mathr.co.uk/web/deep-zoom.html and
//! https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html):
//! single-step entries linearize one z → z^p + c step, and level
//! ℓ+1 merges adjacent level-ℓ pairs
//!
//!   A ← A_y·A_x,   B ← A_y·B_x + B_y,
//!   r ← min(r_x, max(0, (r_y − |B_x|·max|δc|) / |A_x|))
//!
//! so an entry is valid exactly when BOTH halves would have been.
//! O(2M) entries for an M-step orbit, built in O(M) merges.
//!
//! Extended range throughout: A is a product of up to 2^ℓ per-step
//! derivatives and leaves f64's exponent range long before it stops
//! being useful, so entries carry f64 mantissas with a shared i64
//! exponent (the CPU mirror of the shader's CFe).
//!
//! Holomorphic formulas only (z^p + c): the Ship family's abs-folds
//! are not linear in δ across a fold-sign change, so it keeps the
//! per-step path.

/// Extended-range complex: f64 mantissa pair sharing one exponent.
#[derive(Clone, Copy, Debug)]
pub struct Cfe64 {
    pub re: f64,
    pub im: f64,
    pub e: i64,
}

impl Cfe64 {
    pub fn zero() -> Self {
        Self { re: 0.0, im: 0.0, e: 0 }
    }

    pub fn from_f64(re: f64, im: f64) -> Self {
        Self { re, im, e: 0 }.norm()
    }

    /// Renormalize the larger mantissa component into [0.5, 1).
    fn norm(self) -> Self {
        let m = self.re.abs().max(self.im.abs());
        if m == 0.0 || !m.is_finite() {
            return Self::zero();
        }
        // f64 exponent extraction without libm: log2 is exact enough
        // here because we only need the octave, and the mantissas are
        // rescaled by an exact power of two.
        let k = m.log2().floor() as i64 + 1;
        let s = pow2(-k);
        Self {
            re: self.re * s,
            im: self.im * s,
            e: self.e.saturating_add(k),
        }
    }

    pub fn mul(self, o: Self) -> Self {
        if self.is_zero() || o.is_zero() {
            return Self::zero();
        }
        Self {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
            e: self.e.saturating_add(o.e),
        }
        .norm()
    }

    pub fn add(self, o: Self) -> Self {
        if self.is_zero() {
            return o;
        }
        if o.is_zero() {
            return self;
        }
        let d = self.e.saturating_sub(o.e);
        if d > 64 {
            return self;
        }
        if d < -64 {
            return o;
        }
        if d >= 0 {
            let s = pow2(-d);
            Self { re: self.re + o.re * s, im: self.im + o.im * s, e: self.e }.norm()
        } else {
            let s = pow2(d);
            Self { re: self.re * s + o.re, im: self.im * s + o.im, e: o.e }.norm()
        }
    }

    pub fn is_zero(self) -> bool {
        self.re == 0.0 && self.im == 0.0
    }

    /// |self| as (mantissa, exponent).
    pub fn mag(self) -> (f64, i64) {
        if self.is_zero() {
            return (0.0, i64::MIN / 2);
        }
        ((self.re * self.re + self.im * self.im).sqrt(), self.e)
    }

    /// Collapse to f64 (may over/underflow to inf/0 — callers on the
    /// CPU test path only).
    pub fn to_f64(self) -> (f64, f64) {
        let s = pow2(self.e);
        (self.re * s, self.im * s)
    }
}

/// 2^k in f64, exact, saturating to 0 / inf outside the exponent
/// range.
fn pow2(k: i64) -> f64 {
    if k < -1074 {
        0.0
    } else if k > 1023 {
        f64::INFINITY
    } else {
        f64::from_bits(if k >= -1022 {
            (((k + 1023) as u64) << 52) as u64
        } else {
            1u64 << (52 + 1022 + k) as u32
        })
    }
}

/// Extended-range magnitude: mantissa in [0.5, 2), exponent i64.
/// Ordering compare that never constructs an over/underflowing f64.
#[derive(Clone, Copy, Debug)]
pub struct MagFe {
    pub m: f64,
    pub e: i64,
}

impl MagFe {
    pub fn zero() -> Self {
        Self { m: 0.0, e: i64::MIN / 2 }
    }

    pub fn from_f64(v: f64) -> Self {
        let v = v.abs();
        if v == 0.0 || !v.is_finite() {
            return Self::zero();
        }
        let k = v.log2().floor() as i64;
        Self { m: v * pow2(-k), e: k }
    }

    fn norm(self) -> Self {
        if self.m == 0.0 {
            return Self::zero();
        }
        let k = self.m.log2().floor() as i64;
        Self { m: self.m * pow2(-k), e: self.e.saturating_add(k) }
    }

    pub fn mul(self, o: Self) -> Self {
        if self.m == 0.0 || o.m == 0.0 {
            return Self::zero();
        }
        Self { m: self.m * o.m, e: self.e.saturating_add(o.e) }.norm()
    }

    pub fn div(self, o: Self) -> Self {
        if self.m == 0.0 || o.m == 0.0 {
            return Self::zero();
        }
        Self { m: self.m / o.m, e: self.e.saturating_sub(o.e) }.norm()
    }

    /// max(0, self − o).
    pub fn sub_clamped(self, o: Self) -> Self {
        if o.m == 0.0 {
            return self;
        }
        if self.m == 0.0 {
            return Self::zero();
        }
        let d = self.e.saturating_sub(o.e);
        if d > 64 {
            return self;
        }
        if d < -64 {
            return Self::zero();
        }
        let v = if d >= 0 { self.m - o.m * pow2(-d) } else { self.m * pow2(d) - o.m };
        if v <= 0.0 {
            return Self::zero();
        }
        let base = if d >= 0 { self.e } else { o.e };
        Self { m: v, e: base }.norm()
    }

    pub fn less_than(self, o: Self) -> bool {
        if self.m == 0.0 {
            return o.m != 0.0;
        }
        if o.m == 0.0 {
            return false;
        }
        if self.e != o.e {
            self.e < o.e
        } else {
            self.m < o.m
        }
    }

    pub fn min(self, o: Self) -> Self {
        if self.less_than(o) {
            self
        } else {
            o
        }
    }
}

/// One BLA entry: δ' = A·δ + B·δc, valid while |δ| < r.
#[derive(Clone, Copy, Debug)]
pub struct BlaEntry {
    pub a: Cfe64,
    pub b: Cfe64,
    pub r: MagFe,
}

/// The full table. `levels[l]` holds entries each skipping 2^(l+1)
/// iterations; entry k of level l covers orbit steps
/// [k·2^(l+1), (k+1)·2^(l+1)). Level -1 (a single step) is the
/// ordinary delta iteration and is not stored.
pub struct BlaTable {
    pub levels: Vec<Vec<BlaEntry>>,
    /// The |δc| bound the radii were built for.
    pub dc_max: f64,
}

/// Linearization tolerance: the dropped nonlinear term is kept below
/// eps · the linear term. 2^-24 targets f32 delta precision.
pub const BLA_EPS: f64 = 5.960_464_477_539_063e-8;

impl BlaTable {
    /// Build from a reference orbit (Zₙ as f32 pairs, orbit[0] = Z₀)
    /// for z^p + c. `dc_max` bounds |δc| over the viewport (0 for
    /// Julia — every skip's B term is then exact).
    ///
    /// Single-step entry at n (one z_n → z_{n+1} step):
    ///   A = p·Zₙ^(p−1), B = 1,
    ///   r = eps·|Zₙ|·2/(p−1)  — |δ| below this keeps the largest
    ///   dropped term, C(p,2)·Z^(p−2)·δ², under eps of the linear one.
    pub fn build(orbit: &[[f32; 2]], power: u32, dc_max: f64) -> Self {
        Self::build_with_dc(orbit, power, MagFe::from_f64(dc_max), dc_max)
    }

    /// Extended-range form: `dc` as mantissa·2^exponent, so tables
    /// build at any depth (an f64 |δc| underflows past ~zoom 1000 —
    /// exactly where multi-million-iteration renders need the skips
    /// most). `dc_max_hint` is only recorded on the table.
    pub fn build_with_dc(orbit: &[[f32; 2]], power: u32, dc: MagFe, dc_max_hint: f64) -> Self {
        let p = power.max(2);
        let steps = orbit.len().saturating_sub(1);
        // Level 0 of the recurrence: single steps (used only to merge
        // — the shader's plain iteration handles unskipped steps).
        let mut prev: Vec<BlaEntry> = (0..steps)
            .map(|n| {
                let z = Cfe64::from_f64(orbit[n][0] as f64, orbit[n][1] as f64);
                // p·Z^(p−1)
                let mut a = Cfe64::from_f64(p as f64, 0.0);
                for _ in 0..(p - 1) {
                    a = a.mul(z);
                }
                let (zm, ze) = z.mag();
                let r = MagFe { m: zm, e: ze }
                    .norm()
                    .mul(MagFe::from_f64(BLA_EPS * 2.0 / (p as f64 - 1.0)));
                BlaEntry { a, b: Cfe64::from_f64(1.0, 0.0), r }
            })
            .collect();
        let mut levels = Vec::new();
        while prev.len() >= 2 {
            let merged: Vec<BlaEntry> = prev
                .chunks_exact(2)
                .map(|pair| Self::merge(&pair[0], &pair[1], dc))
                .collect();
            levels.push(merged.clone());
            prev = merged;
        }
        Self { levels, dc_max: dc_max_hint }
    }

    /// y ∘ x: apply x's span first, then y's.
    fn merge(x: &BlaEntry, y: &BlaEntry, dc: MagFe) -> BlaEntry {
        let a = y.a.mul(x.a);
        let b = y.a.mul(x.b).add(y.b);
        let (axm, axe) = x.a.mag();
        let (bxm, bxe) = x.b.mag();
        let ax = MagFe { m: axm, e: axe }.norm();
        let bx = MagFe { m: bxm, e: bxe }.norm();
        let r = x.r.min(y.r.sub_clamped(bx.mul(dc)).div(ax));
        BlaEntry { a, b, r }
    }

    /// The largest skip applicable at orbit index m with |δ| = d_mag:
    /// (level, entry) — level l skips 2^(l+1) iterations. None = take
    /// a plain step.
    pub fn best(&self, m: usize, d_mag: MagFe) -> Option<(usize, &BlaEntry)> {
        let mut found = None;
        for (l, level) in self.levels.iter().enumerate() {
            let span = 1usize << (l + 1);
            if m % span != 0 {
                break;
            }
            let k = m / span;
            match level.get(k) {
                Some(e) if d_mag.less_than(e.r) => found = Some((l, e)),
                _ => break,
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f64_orbit(cr: f64, ci: f64, p: u32, n: usize) -> Vec<[f32; 2]> {
        let mut out = vec![[0.0f32; 2]];
        let (mut x, mut y) = (0.0f64, 0.0f64);
        for _ in 0..n {
            let (mut zr, mut zi) = (x, y);
            for _ in 1..p {
                let t = zr * x - zi * y;
                zi = zr * y + zi * x;
                zr = t;
            }
            x = zr + cr;
            y = zi + ci;
            out.push([x as f32, y as f32]);
        }
        out
    }

    fn delta_steps(
        orbit: &[[f32; 2]],
        p: u32,
        start: usize,
        count: usize,
        d: (f64, f64),
        dc: (f64, f64),
    ) -> (f64, f64) {
        // Exact full-precision delta recurrence:
        // δ' = (Z+δ)^p − Z^p + δc, expanded via full complex ops.
        let (mut dr, mut di) = d;
        for n in start..start + count {
            let (zr, zi) = (orbit[n][0] as f64, orbit[n][1] as f64);
            let (fr, fi) = (zr + dr, zi + di);
            let powc = |mut ar: f64, mut ai: f64, p: u32| -> (f64, f64) {
                let (br, bi) = (ar, ai);
                for _ in 1..p {
                    let t = ar * br - ai * bi;
                    ai = ar * bi + ai * br;
                    ar = t;
                }
                (ar, ai)
            };
            let (f_pr, f_pi) = powc(fr, fi, p);
            let (z_pr, z_pi) = powc(zr, zi, p);
            dr = f_pr - z_pr + dc.0;
            di = f_pi - z_pi + dc.1;
        }
        (dr, di)
    }

    #[test]
    fn skip_matches_explicit_steps() {
        // An interior-adjacent parameter: long non-escaping orbit.
        for (p, cr, ci) in [(2u32, -0.7436, 0.1318), (3, -0.2209, 0.7873)] {
            let orbit = f64_orbit(cr, ci, p, 512);
            let dc_max = 1e-14;
            let table = BlaTable::build(&orbit, p, dc_max);
            assert!(table.levels.len() >= 8, "power {p}");
            let dc = (0.4e-14, -0.3e-14);
            for l in [0usize, 3, 6] {
                let span = 1usize << (l + 1);
                for k in [0usize, 1] {
                    let m = k * span;
                    let e = &table.levels[l][k];
                    if e.r.m == 0.0 {
                        continue; // entry never valid (reference near 0)
                    }
                    // A delta safely inside the validity radius.
                    let d_mag = e.r.m * pow2(e.r.e) * 0.1;
                    if d_mag == 0.0 || !d_mag.is_finite() {
                        continue;
                    }
                    let d = (d_mag * 0.6, -d_mag * 0.8);
                    let (er, ei) = delta_steps(&orbit, p, m, span, d, dc);
                    let dd = Cfe64::from_f64(d.0, d.1);
                    let dcc = Cfe64::from_f64(dc.0, dc.1);
                    let (sr, si) = e.a.mul(dd).add(e.b.mul(dcc)).to_f64();
                    let scale = (er * er + ei * ei).sqrt().max(1e-300);
                    let err = ((sr - er).powi(2) + (si - ei).powi(2)).sqrt() / scale;
                    assert!(
                        err < 1e-4,
                        "p={p} level {l} entry {k}: skip ({sr:e},{si:e}) vs explicit ({er:e},{ei:e}), rel {err:e}"
                    );
                }
            }
        }
    }

    #[test]
    fn radius_shrinks_up_levels() {
        let orbit = f64_orbit(-0.7436, 0.1318, 2, 256);
        let table = BlaTable::build(&orbit, 2, 1e-14);
        // Entry 0 of each level covers a prefix of the same orbit —
        // its radius can only tighten (or stay) as the span doubles.
        for l in 1..table.levels.len() {
            let hi = table.levels[l][0].r;
            let lo = table.levels[l - 1][0].r;
            assert!(
                !lo.less_than(hi),
                "level {l}: radius grew ({} 2^{} > {} 2^{})",
                hi.m, hi.e, lo.m, lo.e
            );
        }
    }

    #[test]
    fn best_respects_alignment_and_radius() {
        let orbit = f64_orbit(-0.7436, 0.1318, 2, 256);
        let table = BlaTable::build(&orbit, 2, 1e-14);
        // Misaligned index: no skip from an odd m.
        assert!(table.best(1, MagFe::from_f64(1e-30)).is_none());
        // m = 0 contains the Z₀ = 0 step, whose linearization has no
        // linear part: radius 0, never skippable — by construction.
        assert!(table.best(0, MagFe::from_f64(1e-30)).is_none());
        // Tiny delta at an aligned interior index: skips apply, and
        // the level walk reaches past the minimum span.
        let (l, _) = table.best(8, MagFe::from_f64(1e-30)).unwrap();
        assert!(l >= 1, "deep skip expected for a tiny delta, got level {l}");
        // Huge delta: nothing is valid.
        assert!(table.best(8, MagFe::from_f64(10.0)).is_none());
    }

    #[test]
    fn zero_reference_entry_is_never_valid() {
        // Z₀ = 0 ⇒ the first single-step is pure δ² — no linear part,
        // radius 0, and every merged entry containing it inherits a
        // conservative radius. best() at m=0 must respect r=0 when the
        // orbit is all-zero (degenerate c=0 case).
        let orbit = vec![[0.0f32; 2]; 64];
        let table = BlaTable::build(&orbit, 2, 0.0);
        assert!(table.best(0, MagFe::from_f64(1e-30)).is_none());
    }
}
