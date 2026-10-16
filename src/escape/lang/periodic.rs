//! Nucleus references for compiled formulas (plan, phase 4, step 9c):
//! what `escape::nucleus` does for the built-in `z^p + c`, for any
//! formula whose state from pass to pass is its iterate alone, through
//! the formula's own derivative (`lang::derive`) run in big floats.
//!
//! A reference whose iterate returns to its start after `p` passes wraps
//! exactly: the perturbed path rebases a pixel onto the orbit's start at
//! the orbit's end, and where `Z_p = Z_0` that rebase changes nothing.
//! So the nucleus is a root of `G(c) = z_p(c) - z_0(c)` -- for `z^2 + c`
//! from zero the built-ins' `f_c^p(0) = 0`, for a formula that starts at
//! its pixel `f^p(c) = c`, the same points.
//!
//! - **The period**: the first `n` whose Newton step from the view's
//!   centre, `|G_n / G'_n|`, falls inside the view's radius -- a ball of
//!   the first order: the view's pixels map to about `G_n + G'_n δ`, a
//!   disc that holds 0 when it does. The built-ins' ball method bounds
//!   the image of `z^p + c` exactly; a formula's map has no bound to
//!   hand, and Newton below is the check.
//! - **Newton**: `c <- c - m G(c) / G'(c)` in big floats, from the view's
//!   centre, until the orbit closes past the view's limit or `|G|`
//!   reaches the precision's floor; `m` the root's multiplicity, read
//!   from the steps (2 for a formula that starts at its pixel).
//!
//! The reference built at the nucleus checks its own closure
//! (`ReferenceOrbit::compute_custom_nucleus_aware`), so nothing here is
//! taken on trust.

use super::bigeval::BigMachine;
use super::interp::{Env, ParamValues};
use super::ir::{Predef, Program, VarId};
use crate::escape::bigfloat::{BigComplex, BigFloat};
use crate::escape::fixedpoint::{limbs_for_view, limbs_for_zoom, FixedPoint};
use std::sync::Arc;

/// A formula's nucleus search: its derivative program, run at points of
/// the plane.
pub struct NucleusSearch {
    prog: Arc<Program>,
    params: ParamValues,
    env: Env,
    dz: VarId,
}

/// log2 |x|, `None` for zero.
fn log2_real(x: &BigFloat) -> Option<f64> {
    let e = x.mag_exp()?;
    let top = *x.limbs.last()? as f64;
    Some(e as f64 + (top / 2f64.powi(63)).log2())
}

/// log2 |z|, `None` for zero.
pub fn log2_abs(z: &BigComplex) -> Option<f64> {
    match (log2_real(&z.re), log2_real(&z.im)) {
        (None, None) => None,
        (Some(a), None) | (None, Some(a)) => Some(a),
        (Some(a), Some(b)) => {
            let m = a.max(b);
            Some(m + 0.5 * ((2.0 * (a - m)).exp2() + (2.0 * (b - m)).exp2()).log2())
        }
    }
}

/// Whether a reference of the formula that returns to its start wraps
/// exactly: its state from pass to pass is z alone -- nothing carried,
/// the pass index unread.
pub fn wraps_exactly(prog: &Program) -> bool {
    prog.carried.is_empty() && super::perturb::carried_vars(&prog.loop_body).is_empty() && prog.cycle.is_none() && !prog.reads(Predef::Pass)
}

/// The period a centre's orbit closes at for a view, as
/// `escape::nucleus::detect_period_for_zoom` finds the built-ins': the
/// first pass whose `|z_n - z_0|` is a new minimum at or below
/// `2^limit_octave` (the view's closure limit), with its octave. `None`
/// when the orbit escapes first, cannot go on, or does not close within
/// `max_period`. The program must wrap exactly ([`wraps_exactly`]).
pub fn closing_period(prog: Arc<Program>, params: ParamValues, env: Env, c: &BigComplex, n: usize, limit_octave: i64, max_period: u32) -> Option<(u32, i64)> {
    let mut m = BigMachine::new(prog, params, env, c.clone(), n).ok()?;
    let z0 = m.z().clone();
    let mut best = i64::MAX;
    for p in 1..=max_period {
        if !m.step().ok()? {
            return None;
        }
        let oct = log2_abs(&m.z().sub(&z0)).map_or(i64::MIN / 2, |l| l.floor() as i64);
        if oct < best {
            best = oct;
            if oct <= limit_octave {
                return Some((p, oct));
            }
        }
    }
    None
}

/// The point that closed best, if its orbit closes at all.
fn closed(best: Option<(f64, BigComplex)>) -> Option<BigComplex> {
    best.filter(|(l, _)| *l <= -24.0).map(|(_, c)| c)
}

impl NucleusSearch {
    /// The search for a formula whose reference wraps exactly
    /// ([`wraps_exactly`]) and which has a derivative. `None` otherwise.
    pub fn new(prog: &Program, params: ParamValues, env: Env) -> Option<Self> {
        if !wraps_exactly(prog) {
            return None;
        }
        let d = super::derive::with_derivative(prog)?;
        let dz = d.dz_var?;
        Some(NucleusSearch { prog: Arc::new(d), params, env, dz })
    }

    /// The program at `c`, `n` limbs wide: the machine after its init,
    /// with the start's z and derivative.
    fn start(&self, c: &BigComplex, n: usize) -> Option<(BigMachine, BigComplex, BigComplex)> {
        let m = BigMachine::new(self.prog.clone(), self.params.clone(), self.env, c.clone(), n).ok()?;
        let (z0, dz0) = (m.z().clone(), m.complex_var(self.dz));
        Some((m, z0, dz0))
    }

    /// The period of the nucleus governing a view (centre `c`, radius
    /// `2^radius_log2`): the first pass whose Newton step from the
    /// centre lands inside the view. `None` when the centre escapes
    /// first, its orbit cannot go on, or no pass within `max_period`
    /// does.
    pub fn period(&self, c: &BigComplex, n: usize, radius_log2: f64, max_period: u32) -> Option<u32> {
        let (mut m, z0, dz0) = self.start(c, n)?;
        for p in 1..=max_period {
            if !m.step().ok()? {
                return None;
            }
            let g = m.z().sub(&z0);
            let dg = m.complex_var(self.dz).sub(&dz0);
            let Some(lg) = log2_abs(&g) else {
                return Some(p);
            };
            if let Some(ld) = log2_abs(&dg) {
                if lg - ld <= radius_log2 {
                    return Some(p);
                }
            }
        }
        None
    }

    /// The nucleus governing a view (centre, zoom), as
    /// `escape::nucleus::locate_minibrot` finds the built-ins': the
    /// period at the view's precision, no further than `max_period`,
    /// then Newton -- for periods up to `newton_periods` -- to a closure
    /// 64 octaves past the view's limit (a closure just under it serves
    /// no deeper view, and a double root crosses it halfway there).
    ///
    /// The closure is absolute, `|Z_p - Z_0|`; where the start is fixed
    /// (`z = 0`) that is `h' d` for the root's error `d`, and `h'` reaches
    /// 2^zoom for an atom the view's size, so the root is taken to twice
    /// the zoom's bits, and its digits given to that precision.
    pub fn locate(&self, center_re: &str, center_im: &str, zoom_log2: f64, max_period: u32, newton_periods: u32) -> Option<crate::escape::nucleus::Nucleus> {
        let at = |n: usize| -> Option<BigComplex> {
            Some(BigComplex {
                re: BigFloat::from_fixed(&FixedPoint::from_decimal(center_re, n)?),
                im: BigFloat::from_fixed(&FixedPoint::from_decimal(center_im, n)?),
            })
        };
        let n = limbs_for_view(center_re, center_im, zoom_log2);
        let period = self.period(&at(n)?, n, 1.0 - zoom_log2, max_period)?;
        if period > newton_periods {
            return None;
        }
        let n = limbs_for_zoom(2.0 * zoom_log2 + 16.0) + 1;
        let limit = crate::escape::reference::closure_limit_for_zoom(zoom_log2) as f64;
        let c = self.nucleus(&at(n)?, period, n, limit - 64.0)?;
        let digits = ((64 * n as i64 - 8) as f64 * 0.301).ceil() as usize;
        Some(crate::escape::nucleus::Nucleus { re: c.re.to_fixed(n)?.to_decimal(digits), im: c.im.to_fixed(n)?.to_decimal(digits), period })
    }

    /// Newton for the period-`period` nucleus near `guess`, in big
    /// floats `n` limbs wide, until the orbit closes: `|G| = |z_p - z_0|`
    /// at most `2^closure_log2`, the step below the working precision,
    /// or `|G|` no longer falling (the precision's floor) -- the point
    /// that closed best, as after 64 steps, if it closes at all (2^-24, as
    /// a hinted period must). `None` when a step leaves the region (a
    /// step past 1, |c| past 4, an orbit past any sense); the caller
    /// verifies the closure against its view.
    ///
    /// The root can be multiple: a formula that starts at its pixel has
    /// `G = h^2` for the built-ins' `h = f_c^p(0)` (`z_p` is `h^2 + c`,
    /// `z_0` is `c`), where plain Newton only halves the distance each
    /// step, and `G`'s rounding bounds the root to half the precision.
    /// So the multiplicity `m` is read from the steps' ratio (`1 - 1/m`
    /// when linear), the step taken `m G / G'`; and what is asked is
    /// the closure, which is what the periodic wrap needs.
    pub fn nucleus(&self, guess: &BigComplex, period: u32, n: usize, closure_log2: f64) -> Option<BigComplex> {
        let bits = 64 * n as i64 - 8;
        let mut c = guess.clone();
        let mut last: Option<f64> = None;
        let mut mult = 1.0;
        let mut best: Option<(f64, BigComplex)> = None;
        let mut stalled = 0;
        for _ in 0..64 {
            let (mut m, z0, dz0) = self.start(&c, n)?;
            for _ in 0..period {
                // The test is not asked: an iterate past the bailout far
                // from the nucleus can still come back under Newton. One
                // past any sense ends the search.
                m.step().ok()?;
                if log2_abs(m.z()).is_some_and(|l| l > 4096.0) {
                    return None;
                }
            }
            let g = m.z().sub(&z0);
            let Some(lg) = log2_abs(&g).filter(|&l| l > closure_log2) else {
                return Some(c);
            };
            match &best {
                Some((b, _)) if lg > b - 1.0 => {
                    stalled += 1;
                    if stalled >= 3 {
                        return closed(best);
                    }
                }
                _ => stalled = 0,
            }
            if best.as_ref().is_none_or(|(b, _)| lg < *b) {
                best = Some((lg, c.clone()));
            }
            let dg = m.complex_var(self.dz).sub(&dz0);
            if dg.is_zero() {
                return None;
            }
            let step = g.div(&dg);
            let size = log2_abs(&step)?;
            if !size.is_finite() || size > 0.0 {
                return None;
            }
            // Converging linearly: a multiple root.
            if let Some(prev) = last {
                let ratio = (size - prev).exp2();
                if ratio > 0.3 && ratio < 0.95 {
                    mult = (1.0 / (1.0 - ratio)).round().max(1.0);
                }
            }
            last = Some(size);
            c = c.sub(&if mult == 1.0 { step } else { step.mul_f64(mult) });
            if c.norm_sqr_f64() > 16.0 {
                return None;
            }
            if size < -(bits - 4) as f64 {
                return Some(c);
            }
        }
        closed(best)
    }
}

#[cfg(test)]
mod tests {
    use super::super::builtins::C64;
    use super::super::check::check;
    use super::super::parser::parse_file;
    use super::super::{Dialect, EntryKind};
    use super::*;

    fn search(src: &str) -> Option<NucleusSearch> {
        let (file, d) = parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = check(&file.entries[0], EntryKind::Formula, Dialect::UltraFractal);
        assert!(!super::super::has_errors(&d), "{d:?}");
        let p = p.unwrap();
        let params = ParamValues::defaults(&p);
        NucleusSearch::new(&p, params, Env::at(C64::ZERO, 1000))
    }

    fn re_im(c: &BigComplex) -> (f64, f64) {
        (c.re.to_f64(), c.im.to_f64())
    }

    /// The built-ins' nuclei, found through the formula's derivative:
    /// the Mandelbrot's period-3 antenna nucleus from zero and from the
    /// pixel (the same point, one pass along), the cubic's period-2
    /// nucleus at i; and the periods of views around them, as the
    /// built-ins' ball method gives them.
    #[test]
    fn the_nuclei_of_the_builtins_through_the_derivative() {
        let n = 3;
        let antenna = -1.754_877_666_246_692_7;
        for src in [
            "M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}",
            "P {\ninit:\n z = #pixel\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}",
        ] {
            let s = search(src).expect("the Mandelbrot searches");
            let c = s.nucleus(&BigComplex::from_f64(-1.76, 0.001, n), 3, n, -170.0).expect("converges");
            let (re, im) = re_im(&c);
            assert!((re - antenna).abs() < 1e-15 && im.abs() < 1e-15, "{src}: ({re}, {im})");
            let builtin = crate::escape::nucleus::find_nucleus("-1.76", "0.001", 3, 100.0, 2).unwrap();
            assert!(builtin.re.starts_with(&format!("{:.15}", re)[..16]), "{} against {re}", builtin.re);
            for (cx, cy, radius_log2) in [(-1.754, 0.0005, -8.0), (-1.0, 0.0, -8.0)] {
                let want = crate::escape::nucleus::find_period(&format!("{cx}"), &format!("{cy}"), radius_log2, 100, 2);
                let got = s.period(&BigComplex::from_f64(cx, cy, n), n, radius_log2, 100);
                assert_eq!(got, want, "{src} at ({cx}, {cy})");
            }
        }
        let cubic = search("C {\ninit:\n z = 0\nloop:\n z = z^3 + #pixel\nbailout:\n |z| < 4\n}").unwrap();
        let c = cubic.nucleus(&BigComplex::from_f64(0.05, 0.9, n), 2, n, -170.0).expect("converges");
        let (re, im) = re_im(&c);
        assert!(re.abs() < 1e-15 && (im - 1.0).abs() < 1e-15, "({re}, {im})");
        assert_eq!(cubic.period(&BigComplex::from_f64(0.0, 1.0, n), n, -8.0, 100), Some(2));
    }

    /// Past f64: at 4 limbs Newton closes the orbit to the working
    /// precision, ~2^-240 -- for the formula that starts at its pixel
    /// too, whose nucleus is a double root.
    #[test]
    fn newton_closes_the_orbit_to_the_working_precision() {
        let n = 4;
        for src in [
            "M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}",
            "P {\ninit:\n z = #pixel\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}",
        ] {
            let s = search(src).unwrap();
            let c = s.nucleus(&BigComplex::from_f64(-1.76, 0.001, n), 3, n, -230.0).expect("closes");
            let (mut m, z0, _) = s.start(&c, n).unwrap();
            for _ in 0..3 {
                m.step().unwrap();
            }
            let left = log2_abs(&m.z().sub(&z0)).unwrap_or(f64::NEG_INFINITY);
            assert!(left < -230.0, "{src}: |z_3 - z_0| ~ 2^{left}");
        }
    }

    /// Where a periodic reference would not wrap exactly, there is no
    /// search: a value carried besides z, the pass index read, no
    /// derivative (a fold).
    #[test]
    fn no_search_where_the_wrap_would_not_be_exact() {
        assert!(search("Ph {\ninit:\n z = 0\n y = 0\nloop:\n t = z\n z = sqr(z) + #pixel - 0.5 * y\n y = t\nbailout:\n |z| < 4\n}").is_none(), "carried");
        assert!(search("S {\ninit:\n z = 0\nloop:\n z = sqr(abs(z)) + #pixel\nbailout:\n |z| < 4\n}").is_none(), "a fold");
    }

    /// Escaping centres and bad guesses fail cleanly.
    #[test]
    fn the_search_fails_cleanly() {
        let n = 3;
        let s = search("M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}").unwrap();
        assert_eq!(s.period(&BigComplex::from_f64(2.0, 1.5, n), n, -8.0, 100), None);
        assert!(s.nucleus(&BigComplex::from_f64(3.0, 2.0, n), 7, n, -170.0).is_none());
        assert!(s.nucleus(&BigComplex::from_f64(0.5, 0.5, n), 5000, n, -170.0).is_none());
    }

    /// What a pass costs: the plain program (the reference orbit) and
    /// its derivative (the search), at a few widths, against the
    /// built-ins' fixed-point step.
    #[test]
    #[ignore = "measurement"]
    fn measure_the_search() {
        let src = "M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}";
        let (file, _) = parse_file(src);
        let (p, _) = check(&file.entries[0], EntryKind::Formula, Dialect::UltraFractal);
        let p = std::sync::Arc::new(p.unwrap());
        let params = ParamValues::defaults(&p);
        let s = NucleusSearch::new(&p, params.clone(), Env::at(C64::ZERO, 1000)).unwrap();
        let steps = 20_000;
        for n in [2usize, 4, 8, 16] {
            let c = BigComplex::from_f64(-0.1, 0.1, n);
            let t = std::time::Instant::now();
            let mut m = BigMachine::new(p.clone(), params.clone(), Env::at(C64::ZERO, 1000), c.clone(), n).unwrap();
            for _ in 0..steps {
                m.step().unwrap();
            }
            let plain = t.elapsed().as_secs_f64();
            let t = std::time::Instant::now();
            let (mut m, _, _) = s.start(&c, n).unwrap();
            for _ in 0..steps {
                m.step().unwrap();
            }
            let derived = t.elapsed().as_secs_f64();
            let t = std::time::Instant::now();
            let fixed = crate::escape::reference::ReferenceOrbit::compute("-0.1", "0.1", 0.0, Some(n), steps, None, 2, false, 0, [0.0, 0.0]).unwrap();
            let builtin = t.elapsed().as_secs_f64();
            println!(
                "{n} limbs, {steps} passes: plain {:.0} ns a pass, with the derivative {:.0}, the built-in's fixed point {:.0} ({} entries)",
                plain * 1e9 / steps as f64,
                derived * 1e9 / steps as f64,
                builtin * 1e9 / steps as f64,
                fixed.len()
            );
        }
    }
}
