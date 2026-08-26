//! Newton nucleus-finding: locate the minibrot governing a view.
//!
//! Two stages, both classic (mathr's "navigating by period" lineage):
//!
//! 1. **Period detection, ball method**: iterate a disc under the map
//!    — center orbit `z` in full-precision fixed-point, radius `R` in
//!    extended-range floats: `R' = 2|z|R + R² + r` (`r` = the view
//!    radius, which covers every `c` in the view). The first `n`
//!    where the disc contains 0 (`|zₙ| ≤ Rₙ`) is the lowest period
//!    whose atom dominates the view.
//! 2. **Newton for the nucleus**: solve `F(c) = f_c^p(0) = 0` with
//!    `c ← c − F(c)/F'(c)`, where `F' = dc-derivative orbit
//!    (`dz ← 2·z·dz + 1`). Runs in [`BigComplex`] — the derivative's
//!    dynamic range is the whole reason the big-float wrapper exists.
//!    Quadratic convergence from anywhere in the atom domain.
//!
//! What it buys the renderer: the reference orbit *at the nucleus* is
//! periodic with `Z_p = 0 = Z₀`, so the wrap-rebase is exact, the
//! orbit array is `p` entries instead of `max_iter`, and the
//! reference is maximally glitch-resistant. And it is deep-zoom
//! *navigation*: "center on the minibrot here" with arbitrary-
//! precision output digits.

use super::bigfloat::{BigComplex, BigFloat};
use super::fixedpoint::{limbs_for_zoom, FixedComplex, FixedPoint, FloatExp};

/// Ball-method period detection for the view (center, radius 2^radius_log2).
///
/// Returns the lowest candidate period, or None if the center orbit
/// escapes first (exterior-dominated view) or nothing is found within
/// `max_period` iterations.
pub fn find_period(
    center_re: &str,
    center_im: &str,
    radius_log2: f64,
    max_period: u32,
    power: u32,
) -> Option<u32> {
    let n = limbs_for_zoom(-radius_log2);
    let c = FixedComplex {
        re: FixedPoint::from_decimal(center_re, n)?,
        im: FixedPoint::from_decimal(center_im, n)?,
    };
    let r = FloatExp {
        m: 1.0,
        e: radius_log2.floor() as i64,
    };
    let power = power.clamp(2, 12);
    let mut z = FixedComplex::zero(n);
    let mut radius = r;
    for p in 1..=max_period {
        // z <- z^power + c (square-and-multiply).
        let mut zp = z.sqr();
        for _ in 2..power {
            zp = zp.mul(&z);
        }
        z = zp.add(&c);
        let zx = z.re.to_f64();
        let zy = z.im.to_f64();
        let z_abs = FloatExp::from_f64((zx * zx + zy * zy).sqrt());
        if zx * zx + zy * zy > 16.0 {
            return None; // center orbit escaped: no interior period here
        }
        // Ball image bound: |(z+dz)^p - z^p| <= (|z|+R)^p - |z|^p for
        // |dz| <= R (binomial, all terms positive), so
        // R' = (|z|+R)^p - |z|^p + r. For p = 2 this is exactly the
        // classic 2|z|R + R^2 + r.
        let zr = z_abs.add(radius);
        let mut zr_p = zr;
        let mut za_p = z_abs;
        for _ in 1..power {
            zr_p = zr_p.mul(zr);
            za_p = za_p.mul(z_abs);
        }
        radius = zr_p.add(FloatExp { m: -za_p.m, e: za_p.e }).add(r);
        if z_abs.abs_less_than(radius) {
            return Some(p);
        }
    }
    None
}

/// Result of a successful nucleus search.
pub struct Nucleus {
    pub re: String,
    pub im: String,
    pub period: u32,
}

/// Newton's method for the period-`p` nucleus near the guess.
///
/// `precision_log2` sizes the working precision (use the zoom depth
/// plus margin); the returned decimal strings carry enough digits to
/// recenter at that depth. Fails (None) when Newton leaves the region
/// (|c| > 4 or a step exceeding 1) or does not settle within 64
/// passes.
pub fn find_nucleus(
    guess_re: &str,
    guess_im: &str,
    period: u32,
    precision_log2: f64,
    power: u32,
) -> Option<Nucleus> {
    let power = power.clamp(2, 12);
    let n = limbs_for_zoom(precision_log2) + 1;
    let bits = 64 * n as i64 - 8;
    let cx = FixedPoint::from_decimal(guess_re, n)?;
    let cy = FixedPoint::from_decimal(guess_im, n)?;
    let mut c = BigComplex {
        re: BigFloat::from_fixed(&cx),
        im: BigFloat::from_fixed(&cy),
    };
    let one = BigFloat::from_f64(1.0, n);

    for _ in 0..64 {
        // F(c) = f_c^p(0), F'(c) = d/dc of the same.
        let mut z = BigComplex::zero(n);
        let mut dz = BigComplex::zero(n);
        let mut escaped_mid_orbit = false;
        for _ in 0..period {
            // dz <- p z^(p-1) dz + 1 (dc-derivative chain rule).
            let mut zp1 = z.clone(); // z^1
            for _ in 2..power {
                zp1 = zp1.mul(&z); // z^(p-1)
            }
            let mut d = zp1.mul(&dz);
            // multiply by the integer power (small): repeated add via
            // exponent bump for powers of two, generic sum otherwise.
            let mut acc = BigComplex::zero(n);
            for _ in 0..power {
                acc = acc.add(&d);
            }
            d = acc;
            dz = BigComplex {
                re: d.re.add(&one),
                im: d.im,
            };
            // z <- z^p + c
            let mut zp = z.mul(&z);
            for _ in 2..power {
                zp = zp.mul(&z);
            }
            z = zp.add(&c);
            // An escaped orbit squares its EXPONENT every step — bail
            // before it saturates: this c is far from any period-p
            // nucleus, so the Newton pass is garbage anyway.
            let huge = z.re.mag_exp().unwrap_or(i64::MIN) > 1 << 20
                || z.im.mag_exp().unwrap_or(i64::MIN) > 1 << 20;
            if huge {
                escaped_mid_orbit = true;
                break;
            }
        }
        if escaped_mid_orbit {
            return None;
        }
        if dz.re.is_zero() && dz.im.is_zero() {
            return None; // super-degenerate: cannot step
        }
        let step = z.div(&dz);
        let step_mag2 = step.norm_sqr_f64();
        if !step_mag2.is_finite() || step_mag2 > 1.0 {
            return None; // left the basin
        }
        c = c.sub(&step);
        if c.norm_sqr_f64() > 16.0 {
            return None;
        }
        // Converged when the step is below the working precision.
        let settled = step.re.is_zero() && step.im.is_zero()
            || step_settled(&step, bits);
        if settled {
            let fx = c.re.to_fixed(n)?;
            let fy = c.im.to_fixed(n)?;
            let digits = ((bits as f64) * 0.301).ceil() as usize;
            return Some(Nucleus {
                re: fx.to_decimal(digits),
                im: fy.to_decimal(digits),
                period,
            });
        }
    }
    None
}

/// Is the Newton step below ~2^-(bits-4) in magnitude?
fn step_settled(step: &BigComplex, bits: i64) -> bool {
    let mag = |f: &BigFloat| -> Option<i64> {
        if f.is_zero() {
            None
        } else {
            Some(f.exp + 64 * f.n_limbs() as i64 - 1)
        }
    };
    let threshold = -(bits - 4);
    mag(&step.re).is_none_or(|e| e < threshold)
        && mag(&step.im).is_none_or(|e| e < threshold)
}

impl BigComplex {
    /// Multiply both components by 2^k (exponent bump).
    fn mul_scalar_pow2(&self, k: i64) -> Self {
        Self {
            re: self.re.mul_pow2(k),
            im: self.im.mul_pow2(k),
        }
    }
}

/// One-call navigation helper: detect the period for the view, then
/// Newton to its nucleus. `zoom_log2` is the escape config's zoom
/// (view radius ≈ 2^(1 − zoom)).
pub fn locate_minibrot(
    center_re: &str,
    center_im: &str,
    zoom_log2: f64,
    max_period: u32,
    power: u32,
    newton_period_budget: u32,
) -> Option<Nucleus> {
    let radius_log2 = 1.0 - zoom_log2;
    let period = find_period(center_re, center_im, radius_log2, max_period, power)?;
    // Newton cost is passes x period x big-muls. Blocking callers
    // (CLI, the worker's per-request auto-relocation) pass a modest
    // budget; the panel's background search can afford the full
    // detection range.
    if period > newton_period_budget {
        return None;
    }
    find_nucleus(center_re, center_im, period, zoom_log2 + 16.0, power)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_detection_finds_known_atoms() {
        // A view around the period-2 nucleus at c = -1.
        assert_eq!(find_period("-1.0", "0.0", -8.0, 100, 2), Some(2));
        // The famous real period-3 nucleus on the antenna.
        assert_eq!(find_period("-1.7548776662", "0.0", -10.0, 100, 2), Some(3));
        // Deep exterior: the center orbit escapes, no period.
        assert_eq!(find_period("2.0", "1.5", -8.0, 100, 2), None);
    }

    #[test]
    fn newton_lands_on_the_period_2_nucleus() {
        // F(c) = c^2 + c: the period-2 root is exactly -1.
        let n = find_nucleus("-0.9", "0.05", 2, 40.0, 2).expect("converges");
        let re: f64 = n.re.parse().unwrap();
        let im: f64 = n.im.parse().unwrap();
        assert!((re - -1.0).abs() < 1e-10, "re = {}", n.re);
        assert!(im.abs() < 1e-10, "im = {}", n.im);
    }

    #[test]
    fn newton_lands_on_the_period_3_antenna_nucleus() {
        // Known constant: c = -1.7548776662466927600...
        let n = find_nucleus("-1.76", "0.001", 3, 60.0, 2).expect("converges");
        assert!(
            n.re.starts_with("-1.75487766624669276"),
            "re = {}",
            n.re
        );
        let im: f64 = n.im.parse().unwrap();
        assert!(im.abs() < 1e-15, "im = {}", n.im);
    }

    #[test]
    fn locate_minibrot_end_to_end() {
        // A shallow view over the period-3 atom: detect + refine.
        let hit = locate_minibrot("-1.754", "0.0005", 9.0, 200, 2, 20_000).expect("found");
        assert_eq!(hit.period, 3);
        assert!(hit.re.starts_with("-1.754877666"), "re = {}", hit.re);
    }

    #[test]
    fn escaped_inner_orbit_bails_instead_of_overflowing() {
        // A large period at a c whose orbit escapes within a few
        // iterations: the inner loop must bail (this used to double
        // the exponent per step and overflow i64 in BigFloat::mul —
        // found by a deep render). The first attempt at this test
        // used a cusp-adjacent point and Newton legitimately CONVERGED
        // to a period-5000 nucleus — hence the unambiguous exterior c.
        assert!(find_nucleus("0.5", "0.5", 5000, 60.0, 2).is_none());
    }

    #[test]
    fn cubic_nucleus_lands_on_a_known_root() {
        // z^3 + c, period 2: F(c) = (c^3 + c) -> c(c^2 + 1) = 0, so
        // the nonzero period-2 nuclei are c = +-i exactly.
        let hit = find_nucleus("0.05", "0.9", 2, 40.0, 3).expect("converges");
        let re: f64 = hit.re.parse().unwrap();
        let im: f64 = hit.im.parse().unwrap();
        assert!(re.abs() < 1e-10, "re = {}", hit.re);
        assert!((im - 1.0).abs() < 1e-10, "im = {}", hit.im);
        // And period detection on the cubic around it.
        assert_eq!(find_period("0.0", "1.0", -8.0, 100, 3), Some(2));
    }

    #[test]
    fn newton_reports_failure_instead_of_nonsense() {
        // A guess nowhere near any period-7 atom, in the far exterior:
        // Newton must fail cleanly, not return garbage.
        assert!(find_nucleus("3.0", "2.0", 7, 40.0, 2).is_none());
    }
}
