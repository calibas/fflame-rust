//! Reference orbits for perturbation rendering.
//!
//! One full-precision Mandelbrot orbit per (center, precision), stored
//! as f32 pairs for the GPU — only `2Zₙ` enters the delta iteration's
//! linear term, so *relative* precision is what matters and f32
//! mantissas suffice (the standard Kalles-Fraktaler practice); with
//! Zhuoran rebasing the near-zero passes are handled by construction
//! rather than by wider storage.
//!
//! The fixed-point iteration state is kept alive so deepening
//! (`max_iter` grows) is an **append**, not a recompute — the plan's
//! "append-on-deepen" cache behavior. The cache key is
//! (center strings, limb count); a max_iter increase extends the hit
//! in place.

use super::fixedpoint::{limbs_for_zoom, FixedComplex, FixedPoint};

/// A computed reference orbit plus the live state to extend it.
pub struct ReferenceOrbit {
    /// Exact center, as the config's decimal strings.
    pub center_re: String,
    pub center_im: String,
    /// Julia mode: the fixed c this orbit iterates under (the seed is
    /// then the CENTER). None = parameter plane (seed 0, c = center).
    pub julia_c: Option<(f32, f32)>,
    /// The map's power (z^p + c). 2 = Mandelbrot (two-mul squaring
    /// fast path); higher powers square-and-multiply.
    pub power: u32,
    /// Precision this orbit was computed at.
    pub n_limbs: usize,
    /// Zₙ as f32 pairs, orbit[0] = Z₀ = 0. Length = iterations
    /// computed + 1.
    pub orbit: Vec<[f32; 2]>,
    /// Iteration at which the REFERENCE escaped (|Z|² > 4), if it did.
    /// Pixels needing more iterations rebase (wrap to index 0), so a
    /// short orbit is fine — it just stops growing.
    pub escaped_at: Option<u32>,
    /// Live fixed-point state (c and current Z) for append-on-deepen.
    c: FixedComplex,
    z: FixedComplex,
}

impl ReferenceOrbit {
    /// Compute an orbit at the given precision. `zoom_log2` only picks
    /// the default limb count via [`limbs_for_zoom`] when `n_limbs`
    /// is None.
    pub fn compute(
        center_re: &str,
        center_im: &str,
        zoom_log2: f64,
        n_limbs: Option<usize>,
        max_iter: u32,
        julia_c: Option<(f32, f32)>,
        power: u32,
    ) -> Option<Self> {
        let n = n_limbs.unwrap_or_else(|| limbs_for_zoom(zoom_log2));
        let center = FixedComplex {
            re: FixedPoint::from_decimal(center_re, n)?,
            im: FixedPoint::from_decimal(center_im, n)?,
        };
        // Parameter plane: z0 = 0, c = center. Julia (dynamical)
        // plane: z0 = center, c = the fixed Julia constant (f32
        // config values — exact in fixed-point).
        let (z0, c, first) = match julia_c {
            None => {
                (FixedComplex::zero(n), center, [0.0f32, 0.0f32])
            }
            Some((jre, jim)) => {
                let first = [center.re.to_f64() as f32, center.im.to_f64() as f32];
                let c = FixedComplex {
                    re: FixedPoint::from_f64(jre as f64, n),
                    im: FixedPoint::from_f64(jim as f64, n),
                };
                (center, c, first)
            }
        };
        let mut orbit = Self {
            center_re: center_re.to_string(),
            center_im: center_im.to_string(),
            julia_c,
            power: power.max(2),
            n_limbs: n,
            orbit: vec![first],
            escaped_at: None,
            z: z0,
            c,
        };
        orbit.extend(max_iter);
        Some(orbit)
    }

    /// Number of usable orbit entries (indices 0..len).
    pub fn len(&self) -> u32 {
        self.orbit.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.orbit.len() <= 1
    }

    /// Extend the orbit so it covers `max_iter` iterations (no-op if
    /// it already does, or if the reference escaped earlier).
    pub fn extend(&mut self, max_iter: u32) {
        if self.escaped_at.is_some() {
            return;
        }
        while (self.orbit.len() as u32) <= max_iter {
            // z^p: square-and-multiply from the two-mul square.
            let mut zp = self.z.sqr();
            for _ in 2..self.power {
                zp = zp.mul(&self.z);
            }
            self.z = zp.add(&self.c);
            let x = self.z.re.to_f64();
            let y = self.z.im.to_f64();
            self.orbit.push([x as f32, y as f32]);
            if x * x + y * y > 4.0 {
                self.escaped_at = Some(self.orbit.len() as u32 - 1);
                break;
            }
        }
    }

    /// Test-only view of the live fixed-point iterate.
    #[cfg(test)]
    pub(crate) fn z_state(&self) -> (&super::fixedpoint::FixedPoint, &super::fixedpoint::FixedPoint) {
        (&self.z.re, &self.z.im)
    }

    /// Whether this orbit serves the given request (same center and
    /// at-least precision; iterations are extendable, so they don't
    /// invalidate).
    pub fn serves(
        &self,
        center_re: &str,
        center_im: &str,
        n_limbs: usize,
        julia_c: Option<(f32, f32)>,
        power: u32,
    ) -> bool {
        self.center_re == center_re
            && self.center_im == center_im
            && self.n_limbs >= n_limbs
            && self.julia_c == julia_c
            && self.power == power.max(2)
    }
}

/// Single-slot orbit cache: during a continuous zoom the center is
/// unchanged, so one orbit serves every frame; deepening appends. A
/// pan or precision change replaces the slot.
#[derive(Default)]
pub struct OrbitCache {
    slot: Option<ReferenceOrbit>,
}

impl OrbitCache {
    /// Get (computing or extending as needed) the orbit for a view.
    /// Returns None only when the center strings fail to parse.
    pub fn get(
        &mut self,
        center_re: &str,
        center_im: &str,
        zoom_log2: f64,
        max_iter: u32,
        julia_c: Option<(f32, f32)>,
        power: u32,
    ) -> Option<&ReferenceOrbit> {
        let n = limbs_for_zoom(zoom_log2);
        let hit = self
            .slot
            .as_ref()
            .is_some_and(|o| o.serves(center_re, center_im, n, julia_c, power));
        if hit {
            let orbit = self.slot.as_mut().unwrap();
            orbit.extend(max_iter);
        } else {
            self.slot = Some(ReferenceOrbit::compute(
                center_re, center_im, zoom_log2, Some(n), max_iter, julia_c, power,
            )?);
        }
        self.slot.as_ref()
    }

    pub fn clear(&mut self) {
        self.slot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shallow_orbit_matches_f64_iteration() {
        // c = -0.5 + 0.1i is inside the main cardioid: never escapes,
        // so the orbit length is exactly what we asked for.
        let orbit = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 100, None, 2).unwrap();
        let (mut zx, mut zy) = (0.0f64, 0.0f64);
        for i in 1..=100usize {
            let t = zx * zx - zy * zy + -0.5;
            zy = 2.0 * zx * zy + 0.1;
            zx = t;
            let [ox, oy] = orbit.orbit[i];
            assert!(
                (ox as f64 - zx).abs() < 1e-5 && (oy as f64 - zy).abs() < 1e-5,
                "iteration {i}: orbit ({ox}, {oy}) vs f64 ({zx}, {zy})"
            );
        }
        assert_eq!(orbit.escaped_at, None);
    }

    #[test]
    fn escaping_reference_stops_early() {
        let orbit = ReferenceOrbit::compute("1", "1", 5.0, None, 1000, None, 2).unwrap();
        let at = orbit.escaped_at.expect("c = 1+i escapes fast");
        assert!(at < 10);
        assert_eq!(orbit.len() - 1, at);
    }

    #[test]
    fn deepen_is_an_append_and_matches_fresh_compute() {
        let mut a = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 50, None, 2).unwrap();
        a.extend(120);
        let b = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 120, None, 2).unwrap();
        assert_eq!(a.orbit.len(), b.orbit.len());
        for (i, (x, y)) in a.orbit.iter().zip(b.orbit.iter()).enumerate() {
            assert_eq!(x, y, "diverged at iteration {i}");
        }
    }

    #[test]
    fn cache_hits_extends_and_replaces() {
        let mut cache = OrbitCache::default();
        {
            let o = cache.get("-0.5", "0.1", 10.0, 50, None, 2).unwrap();
            assert_eq!(o.len(), 51);
        }
        // Same view, deeper iterations: extend in place.
        {
            let o = cache.get("-0.5", "0.1", 10.0, 80, None, 2).unwrap();
            assert_eq!(o.len(), 81);
        }
        // Different center: replace.
        {
            let o = cache.get("-0.75", "0.1", 10.0, 10, None, 2).unwrap();
            assert_eq!(o.center_re, "-0.75");
            assert_eq!(o.len(), 11);
        }
        // Unparseable center: None.
        assert!(cache.get("not a number", "0", 10.0, 10, None, 2).is_none());
    }

    #[test]
    fn julia_orbit_seeds_from_the_center() {
        // Julia: z0 = center, c fixed. First entry must be the center,
        // then iterate z^2 + c.
        let orbit = ReferenceOrbit::compute(
            "0.25", "0.5", 10.0, None, 20, Some((-0.8, 0.156)), 2,
        )
        .unwrap();
        assert_eq!(orbit.orbit[0], [0.25, 0.5]);
        let (mut zx, mut zy) = (0.25f64, 0.5f64);
        for i in 1..=20usize {
            let t = zx * zx - zy * zy + -0.8f32 as f64;
            zy = 2.0 * zx * zy + 0.156f32 as f64;
            zx = t;
            if i < orbit.orbit.len() {
                let [ox, oy] = orbit.orbit[i];
                assert!(
                    (ox as f64 - zx).abs() < 1e-5 && (oy as f64 - zy).abs() < 1e-5,
                    "iteration {i}"
                );
            }
        }
        // And the cache distinguishes julia from param-plane orbits.
        let mut cache = OrbitCache::default();
        cache.get("0.25", "0.5", 10.0, 20, Some((-0.8, 0.156)), 2).unwrap();
        let replaced = cache.get("0.25", "0.5", 10.0, 20, None, 2).unwrap();
        assert_eq!(replaced.julia_c, None);
    }

    #[test]
    fn cubic_orbit_matches_f64_iteration() {
        // power 3: z^3 + c against a plain f64 loop.
        let orbit =
            ReferenceOrbit::compute("-0.2", "0.4", 10.0, None, 60, None, 3).unwrap();
        let (mut zx, mut zy) = (0.0f64, 0.0f64);
        for i in 1..=60usize {
            let (x2, y2) = (zx * zx - zy * zy, 2.0 * zx * zy);
            let t = x2 * zx - y2 * zy + -0.2;
            zy = x2 * zy + y2 * zx + 0.4;
            zx = t;
            if i >= orbit.orbit.len() {
                break;
            }
            let [ox, oy] = orbit.orbit[i];
            assert!(
                (ox as f64 - zx).abs() < 1e-5 && (oy as f64 - zy).abs() < 1e-5,
                "iteration {i}: orbit ({ox}, {oy}) vs f64 ({zx}, {zy})"
            );
        }
    }

    #[test]
    fn deep_center_precision_is_carried() {
        // Two centers differing at the 60th decimal digit: the
        // fixed-point ITERATES must differ (f32 snapshots cannot show
        // 1e-60, and chaos amplification is unreliable — interior
        // orbits contract, boundary orbits escape — so compare the
        // full-precision state directly).
        let a = ReferenceOrbit::compute(
            "-0.500000000000000000000000000000000000000000000000000000000001",
            "0.1",
            220.0,
            None,
            50,
            None,
            2,
        )
        .unwrap();
        let b = ReferenceOrbit::compute(
            "-0.500000000000000000000000000000000000000000000000000000000002",
            "0.1",
            220.0,
            None,
            50,
            None,
            2,
        )
        .unwrap();
        let (are, _) = a.z_state();
        let (bre, _) = b.z_state();
        let diff = are.sub(bre);
        assert!(
            !diff.is_zero(),
            "1e-60 center difference vanished from the fixed-point orbit"
        );
        // And it is far below anything f64 could have carried.
        let fe = diff.to_floatexp();
        assert!(fe.e < -120, "difference 2^{} is too large to prove deep precision", fe.e);
    }
}
