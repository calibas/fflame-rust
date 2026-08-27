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

/// Cap on ball-method period detection when hunting a nucleus
/// reference. Beyond this the search costs more than it saves.
const NUCLEUS_MAX_PERIOD: u32 = 100_000;

/// Try to relocate a parameter-plane reference to the minibrot
/// nucleus governing the view. Returns (nucleus_re, nucleus_im,
/// period, ref_offset) where ref_offset = (view − nucleus) in units
/// of the PIXEL SPACING (f32-safe: the nucleus lies within ~a view
/// radius). Mandelbrot (power 2), parameter plane only.
fn nucleus_for_view(
    center_re: &str,
    center_im: &str,
    zoom_log2: f64,
    height_px: f64,
    power: u32,
) -> Option<(String, String, u32, [f32; 2])> {
    let hit = super::nucleus::locate_minibrot(
        center_re,
        center_im,
        zoom_log2,
        NUCLEUS_MAX_PERIOD,
        power,
        20_000,
    )?;
    // ref_offset = (C_view − C_nucleus) / S, computed exactly in
    // fixed-point, exported via floatexp.
    let n = limbs_for_zoom(zoom_log2) + 1;
    let vx = FixedPoint::from_decimal(center_re, n)?;
    let vy = FixedPoint::from_decimal(center_im, n)?;
    let nx = FixedPoint::from_decimal(&hit.re, n)?;
    let ny = FixedPoint::from_decimal(&hit.im, n)?;
    // S = 2^(2 − zoom) / height ⇒ 1/S = height · 2^(zoom − 2).
    let to_px = |d: FixedPoint| -> f64 {
        let fe = d.to_floatexp();
        let e = fe.e as f64 + (zoom_log2 - 2.0) + height_px.log2();
        if fe.m == 0.0 || e < -60.0 {
            0.0
        } else {
            fe.m * 2f64.powf(e.min(40.0))
        }
    };
    let off = [to_px(vx.sub(&nx)) as f32, to_px(vy.sub(&ny)) as f32];
    // 2^15 px: beyond this the f32 sum (pixel_offset + ref_offset)
    // in the shader's d0 quantizes pixel positions past ~2^-8 px —
    // and by ~2^23 px merges pixels entirely (the zoom-700 uniform-
    // collapse bug, ground-truthed against exact orbits). A nucleus
    // that far out buys little; the plain reference is correct.
    if !off[0].is_finite() || !off[1].is_finite() || off[0].abs() > 32768.0 || off[1].abs() > 32768.0
    {
        return None;
    }
    Some((hit.re, hit.im, hit.period, off))
}

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
    /// Burning Ship family: use the ship-variant step (power 2).
    pub ship: bool,
    /// Which fold arrangement (0..=5, the formula's variant enum).
    pub ship_variant: u32,
    /// This orbit sits at a minibrot nucleus of the given period:
    /// Z_period = 0 = Z_0 exactly, so the wrap-rebase is exact and
    /// the orbit never needs extending past the period.
    pub periodic: Option<u32>,
    /// (view − reference) in pixel-spacing units, for the pipeline's
    /// d0. Zero when the reference IS the view center.
    pub ref_offset: [f32; 2],
    /// The view the offset's PIXEL units were measured at. Pixel
    /// units scale with S = 2^(2−zoom)/height, so consumers at any
    /// other view must rescale by 2^(zoom−off_zoom)·(h/off_height)
    /// (see [`Self::offset_for_view`]) — applying the raw numbers at
    /// a different zoom silently pans the render toward the nucleus.
    pub off_zoom_log2: f64,
    pub off_height_px: f64,
    /// Precision this orbit was computed at.
    pub n_limbs: usize,
    /// Zₙ as f32 pairs, orbit[0] = Z₀ = 0. Length = iterations
    /// computed + 1.
    pub orbit: Vec<[f32; 2]>,
    /// The f64 residual of each entry below its f32 hi (DF storage:
    /// Z ≈ hi + lo to ~2^-48 relative). Same length as `orbit`.
    pub orbit_lo: Vec<[f32; 2]>,
    /// Iteration at which the REFERENCE escaped (|Z|² > 4), if it did.
    /// Pixels needing more iterations rebase (wrap to index 0), so a
    /// short orbit is fine — it just stops growing.
    pub escaped_at: Option<u32>,
    /// Live fixed-point state (c and current Z) for append-on-deepen.
    c: FixedComplex,
    z: FixedComplex,
    /// Running |Z| minimum past index 0 as an OCTAVE (extended-range;
    /// f64 magnitudes underflow at 2^-537 and would falsely read as
    /// closures) and its index — the ball-method candidate tracker.
    min_octave: i64,
    min_at: u32,
    /// Closure acceptance limit in octaves: |Z_p| must sit BELOW the
    /// view's pixel scale for the wrap to be exact there. Derived
    /// from the zoom the orbit currently serves (tightens as the
    /// view deepens — shallow closures retire and deeper periods get
    /// discovered progressively).
    closure_limit_octave: i64,
    /// The |Z_period| octave at closure (periodic orbits only;
    /// far-negative for Newton-exact nuclei). Persisted: validity at
    /// a new zoom is closure_octave vs that zoom's limit.
    pub closure_octave: i64,
}

/// Octave limit for accepting a closure at a zoom: 16 octaves below
/// pixel scale (margin), and never looser than f32 visibility.
pub fn closure_limit_for_zoom(zoom_log2: f64) -> i64 {
    (-(zoom_log2 + 16.0) as i64).min(-24)
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
        ship: bool,
        ship_variant: u32,
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
            ship,
            ship_variant: ship_variant.min(5),
            periodic: None,
            ref_offset: [0.0, 0.0],
            off_zoom_log2: zoom_log2,
            off_height_px: 1.0,
            n_limbs: n,
            orbit: vec![first],
            orbit_lo: vec![[0.0, 0.0]],
            escaped_at: None,
            z: z0,
            c,
            min_octave: i64::MAX,
            min_at: 0,
            closure_limit_octave: closure_limit_for_zoom(zoom_log2),
            closure_octave: i64::MAX,
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
        if let Some(p) = self.periodic {
            // A nucleus orbit is complete at its period.
            let _ = p;
            return;
        }
        while (self.orbit.len() as u32) <= max_iter {
            if self.ship {
                // Ship-family step from the plain square's parts:
                // sqr gives (x^2 - y^2, 2xy); every variant is a
                // sign/abs rearrangement of those, free in
                // sign-magnitude (see the delta codegen's derivation).
                let x_neg = self.z.re.neg;
                let y_neg = self.z.im.neg;
                let mut sq = self.z.sqr();
                match self.ship_variant {
                    0 => {
                        // Burning Ship: (|x|+i|y|)^2: re unchanged,
                        // im = 2|x||y| = |2xy|.
                        sq.im.neg = false;
                    }
                    1 => {
                        // Perpendicular Mandelbrot: im = -2|x|y.
                        sq.im.neg = false;
                        if !y_neg {
                            sq.im.neg = true;
                        }
                    }
                    2 => {
                        // Perpendicular Ship: im = -2x|y|.
                        sq.im.neg = false;
                        if !x_neg {
                            sq.im.neg = true;
                        }
                    }
                    3 => {
                        // Celtic: re = |x^2 - y^2|.
                        sq.re.neg = false;
                    }
                    4 => {
                        // Buffalo: re = |x^2-y^2|, im = -|2xy|.
                        sq.re.neg = false;
                        sq.im.neg = true;
                    }
                    _ => {
                        // Perpendicular Celtic: celtic re + perp-M im.
                        sq.re.neg = false;
                        sq.im.neg = false;
                        if !y_neg {
                            sq.im.neg = true;
                        }
                    }
                }
                self.z = sq.add(&self.c);
            } else {
                // z^p: square-and-multiply from the two-mul square.
                let mut zp = self.z.sqr();
                for _ in 2..self.power {
                    zp = zp.mul(&self.z);
                }
                self.z = zp.add(&self.c);
            }
            let x = self.z.re.to_f64();
            let y = self.z.im.to_f64();
            // Progressive period detection (parameter-plane power
            // tiers): a new |Z| minimum is a ball-method period
            // candidate; below f32 visibility the orbit has PROVEN
            // its period — become the periodic reference on the spot.
            if self.julia_c.is_none() && !self.ship {
                // |Z| octave from the LIVE fixed-point state —
                // extended range, because f64 magnitudes underflow at
                // 2^-537 and intermediate cascade passes go far
                // deeper without being closures for the current view.
                let fx = self.z.re.to_floatexp();
                let fy = self.z.im.to_floatexp();
                let oct = match (fx.m == 0.0, fy.m == 0.0) {
                    (true, true) => i64::MIN / 2,
                    (true, false) => fy.e,
                    (false, true) => fx.e,
                    (false, false) => fx.e.max(fy.e),
                };
                // The value just computed is iterate index len()
                // (it has not been pushed yet).
                let idx = self.orbit.len() as u32;
                if oct < self.min_octave {
                    self.min_octave = oct;
                    self.min_at = idx;
                    if oct <= self.closure_limit_octave && idx > 0 {
                        let hix = x as f32;
                        let hiy = y as f32;
                        self.orbit.push([hix, hiy]);
                        self.orbit_lo
                            .push([(x - hix as f64) as f32, (y - hiy as f64) as f32]);
                        self.periodic = Some(idx);
                        self.closure_octave = oct;
                        log::info!(
                            "auto-detected periodic reference: period {idx} (|Z| ~ 2^{oct})"
                        );
                        return;
                    }
                }
            }
            let hix = x as f32;
            let hiy = y as f32;
            self.orbit.push([hix, hiy]);
            self.orbit_lo.push([(x - hix as f64) as f32, (y - hiy as f64) as f32]);
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
        ship: bool,
        ship_variant: u32,
    ) -> bool {
        self.center_re == center_re
            && self.center_im == center_im
            && self.n_limbs >= n_limbs
            && self.julia_c == julia_c
            && self.power == power.max(2)
            && self.ship == ship
            && self.ship_variant == ship_variant.min(5)
    }

    /// The relocation offset in the PIXEL UNITS of a given view.
    /// Pixel spacing S = 2^(2−zoom)/height, so
    /// off_px(view) = off_px(measured) · 2^(zoom−off_zoom) · h/off_h.
    /// None when the rescaled offset leaves f32's useful range
    /// (zooming far OUT from where the nucleus was found) — the
    /// caller must recompute the reference rather than render with
    /// a garbage offset.
    pub fn offset_for_view(&self, zoom_log2: f64, height_px: f64) -> Option<[f32; 2]> {
        rescale_offset(
            self.ref_offset,
            self.off_zoom_log2,
            self.off_height_px,
            zoom_log2,
            height_px,
        )
    }

    /// Whether a reuse at the given view can still express this
    /// orbit's relocation (always true for offset-free orbits).
    pub fn relocation_serves(&self, zoom_log2: f64, height_px: f64) -> bool {
        self.offset_for_view(zoom_log2, height_px).is_some()
    }

    /// Serialize for the disk store (`orbit_store`): identity, orbit,
    /// AND the live fixed-point state, so a reloaded orbit deepens
    /// with [`extend`](Self::extend) exactly like the original.
    /// Layout: magic, orbit length at byte 8 (the store's cheap
    /// staleness probe), then little-endian fields (including the
    /// offset's provenance view).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn to_bytes(&self) -> Vec<u8> {
        fn put_str(out: &mut Vec<u8>, s: &str) {
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        }
        fn put_fixed(out: &mut Vec<u8>, f: &super::fixedpoint::FixedPoint) {
            out.push(f.neg as u8);
            out.extend_from_slice(&(f.limbs.len() as u32).to_le_bytes());
            for l in &f.limbs {
                out.extend_from_slice(&l.to_le_bytes());
            }
        }
        let mut out = Vec::with_capacity(64 + self.orbit.len() * 8 + self.n_limbs * 32);
        out.extend_from_slice(super::orbit_store::MAGIC);
        out.extend_from_slice(&(self.orbit.len() as u32).to_le_bytes());
        put_str(&mut out, &self.center_re);
        put_str(&mut out, &self.center_im);
        match self.julia_c {
            None => out.push(0),
            Some((re, im)) => {
                out.push(1);
                out.extend_from_slice(&re.to_le_bytes());
                out.extend_from_slice(&im.to_le_bytes());
            }
        }
        out.extend_from_slice(&self.power.to_le_bytes());
        out.push(self.ship as u8);
        out.extend_from_slice(&self.ship_variant.to_le_bytes());
        match self.periodic {
            None => out.push(0),
            Some(p) => {
                out.push(1);
                out.extend_from_slice(&p.to_le_bytes());
            }
        }
        out.extend_from_slice(&self.closure_octave.to_le_bytes());
        out.extend_from_slice(&self.ref_offset[0].to_le_bytes());
        out.extend_from_slice(&self.ref_offset[1].to_le_bytes());
        out.extend_from_slice(&self.off_zoom_log2.to_le_bytes());
        out.extend_from_slice(&self.off_height_px.to_le_bytes());
        out.extend_from_slice(&(self.n_limbs as u32).to_le_bytes());
        match self.escaped_at {
            None => out.push(0),
            Some(e) => {
                out.push(1);
                out.extend_from_slice(&e.to_le_bytes());
            }
        }
        for z in &self.orbit {
            out.extend_from_slice(&z[0].to_le_bytes());
            out.extend_from_slice(&z[1].to_le_bytes());
        }
        for z in &self.orbit_lo {
            out.extend_from_slice(&z[0].to_le_bytes());
            out.extend_from_slice(&z[1].to_le_bytes());
        }
        for f in [&self.z.re, &self.z.im, &self.c.re, &self.c.im] {
            put_fixed(&mut out, f);
        }
        out
    }

    /// Inverse of [`to_bytes`](Self::to_bytes); None on any
    /// truncation, magic, or shape mismatch (a miss, not an error).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        struct R<'a>(&'a [u8]);
        impl<'a> R<'a> {
            fn take(&mut self, n: usize) -> Option<&'a [u8]> {
                if self.0.len() < n {
                    return None;
                }
                let (a, b) = self.0.split_at(n);
                self.0 = b;
                Some(a)
            }
            fn u8(&mut self) -> Option<u8> {
                Some(self.take(1)?[0])
            }
            fn u32(&mut self) -> Option<u32> {
                Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
            }
            fn f32(&mut self) -> Option<f32> {
                Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
            }
            fn f64(&mut self) -> Option<f64> {
                Some(f64::from_le_bytes(self.take(8)?.try_into().ok()?))
            }
            fn string(&mut self) -> Option<String> {
                let n = self.u32()? as usize;
                if n > 4096 {
                    return None;
                }
                String::from_utf8(self.take(n)?.to_vec()).ok()
            }
            fn fixed(&mut self, expect_limbs: usize) -> Option<super::super::escape::fixedpoint::FixedPoint> {
                let neg = self.u8()? != 0;
                let n = self.u32()? as usize;
                if n != expect_limbs || n > 1024 {
                    return None;
                }
                let mut limbs = Vec::with_capacity(n);
                for _ in 0..n {
                    limbs.push(u64::from_le_bytes(self.take(8)?.try_into().ok()?));
                }
                Some(super::super::escape::fixedpoint::FixedPoint { neg, limbs })
            }
        }
        let mut r = R(bytes);
        if r.take(8)? != super::orbit_store::MAGIC {
            return None;
        }
        let orbit_len = r.u32()? as usize;
        if orbit_len == 0 || orbit_len > 64_000_000 {
            return None;
        }
        let center_re = r.string()?;
        let center_im = r.string()?;
        let julia_c = match r.u8()? {
            0 => None,
            _ => Some((r.f32()?, r.f32()?)),
        };
        let power = r.u32()?;
        let ship = r.u8()? != 0;
        let ship_variant = r.u32()?;
        let periodic = match r.u8()? {
            0 => None,
            _ => Some(r.u32()?),
        };
        let closure_octave = i64::from_le_bytes(r.take(8)?.try_into().ok()?);
        let ref_offset = [r.f32()?, r.f32()?];
        let off_zoom = r.f64()?;
        let off_height = r.f64()?;
        let n_limbs = r.u32()? as usize;
        if n_limbs == 0 || n_limbs > 1024 {
            return None;
        }
        let escaped_at = match r.u8()? {
            0 => None,
            _ => Some(r.u32()?),
        };
        let mut orbit = Vec::with_capacity(orbit_len);
        for _ in 0..orbit_len {
            orbit.push([r.f32()?, r.f32()?]);
        }
        let mut orbit_lo = Vec::with_capacity(orbit_len);
        for _ in 0..orbit_len {
            orbit_lo.push([r.f32()?, r.f32()?]);
        }
        let z = FixedComplex { re: r.fixed(n_limbs)?, im: r.fixed(n_limbs)? };
        let c = FixedComplex { re: r.fixed(n_limbs)?, im: r.fixed(n_limbs)? };
        Some(Self {
            center_re,
            center_im,
            julia_c,
            power,
            ship,
            ship_variant,
            periodic,
            ref_offset,
            off_zoom_log2: off_zoom,
            off_height_px: off_height,
            n_limbs,
            orbit,
            orbit_lo,
            escaped_at,
            z,
            c,
            min_octave: i64::MAX,
            min_at: 0,
            closure_limit_octave: closure_limit_for_zoom(off_zoom),
            closure_octave,
        }
        .with_rescanned_min())
    }

    /// Rebuild the minimum tracker from the stored f32 orbit (loads).
    /// f32 floors at ~2^-149; deeper minima re-emerge from the live
    /// fixed-point state as the orbit extends.
    #[cfg(not(target_arch = "wasm32"))]
    fn with_rescanned_min(mut self) -> Self {
        for (i, z) in self.orbit.iter().enumerate().skip(1) {
            let m = (z[0] as f64) * (z[0] as f64) + (z[1] as f64) * (z[1] as f64);
            let oct = if m == 0.0 { -149 } else { (m.log2() / 2.0) as i64 };
            if oct < self.min_octave {
                self.min_octave = oct;
                self.min_at = i as u32;
            }
        }
        self
    }

    /// Whether this orbit's periodic wrap is exact enough for a view:
    /// non-periodic always serves; a closure serves while its |Z_p|
    /// octave stays below the view's limit.
    pub fn periodic_serves(&self, zoom_log2: f64) -> bool {
        self.periodic.is_none() || self.closure_octave <= closure_limit_for_zoom(zoom_log2)
    }

    /// Retighten the closure limit before extending for a (possibly
    /// deeper) view.
    pub fn set_closure_limit(&mut self, zoom_log2: f64) {
        self.closure_limit_octave = closure_limit_for_zoom(zoom_log2);
    }
}

/// Rescale a pixel-unit relocation offset from the view it was
/// measured at to another view. Pixel spacing S = 2^(2−zoom)/height,
/// so off_px scales by 2^(Δzoom)·(h/off_h). None when the result
/// leaves f32's useful range — render with a recomputed reference,
/// never with a garbage offset.
pub fn rescale_offset(
    off: [f32; 2],
    off_zoom: f64,
    off_height: f64,
    zoom_log2: f64,
    height_px: f64,
) -> Option<[f32; 2]> {
    if off == [0.0, 0.0] {
        return Some([0.0, 0.0]);
    }
    let factor = 2f64.powf((zoom_log2 - off_zoom).clamp(-2000.0, 60.0))
        * (height_px.max(1.0) / off_height.max(1.0));
    let x = off[0] as f64 * factor;
    let y = off[1] as f64 * factor;
    // Same 2^15 px ceiling as nucleus_for_view — a rescaled offset
    // rides the identical f32 d0 sum.
    if !x.is_finite() || !y.is_finite() || x.abs() > 32768.0 || y.abs() > 32768.0 {
        return None;
    }
    Some([x as f32, y as f32])
}

/// A reference-orbit request, as the worker sees it.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, PartialEq)]
pub struct OrbitRequest {
    pub center_re: String,
    pub center_im: String,
    pub n_limbs: usize,
    pub max_iter: u32,
    pub julia_c: Option<(f32, f32)>,
    pub power: u32,
    pub ship: bool,
    pub ship_variant: u32,
    /// Verified-before-use period hint (parameter-plane power tiers).
    pub reference_period: Option<u32>,
    /// Zoom (for nucleus search precision) and viewport height (for
    /// the relocation offset's pixel units).
    pub zoom_log2: f64,
    pub height_px: f64,
}

/// Progressive snapshot the render side reads each frame.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct OrbitProgress {
    /// Relocation of this orbit's reference relative to the request's
    /// view center, in pixel-spacing units (nucleus references), plus
    /// the view those units were measured at — the consumer rescales
    /// to ITS view (`rescale_offset`), because the worker may serve
    /// one orbit across many zoom levels.
    pub ref_offset: [f32; 2],
    pub off_zoom_log2: f64,
    pub off_height_px: f64,
    /// The reference's period when periodic (hinted or auto-detected)
    /// — the panel surfaces it.
    pub detected_period: Option<u32>,
    /// Which request this data belongs to (bumped on every new
    /// request; stale chunks from an abandoned compute are ignored).
    pub epoch: u64,
    /// Z_n snapshots so far (orbit[0] = the seed).
    pub orbit: Vec<[f32; 2]>,
    /// DF residuals, parallel to `orbit`.
    pub orbit_lo: Vec<[f32; 2]>,
    /// The orbit covers its request's max_iter (or escaped early).
    pub done: bool,
}

/// Reference orbits on a worker thread with progressive upload (the
/// plan's phase-4 item): the render side posts the latest request and
/// reads whatever prefix exists each frame — rebasing treats a
/// too-short orbit as an early wrap, so partial-orbit frames are
/// merely noisier, never wrong, and refine as chunks land. Deepening
/// reuses the live fixed-point state (append, not recompute) as long
/// as the rest of the request matches.
#[cfg(not(target_arch = "wasm32"))]
pub struct OrbitWorker {
    tx: std::sync::mpsc::Sender<(u64, OrbitRequest)>,
    pub progress: std::sync::Arc<std::sync::Mutex<OrbitProgress>>,
    latest: Option<OrbitRequest>,
    epoch: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl OrbitWorker {
    /// Iterations per chunk between snapshot publications.
    const CHUNK: u32 = 4096;

    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<(u64, OrbitRequest)>();
        let progress = std::sync::Arc::new(std::sync::Mutex::new(OrbitProgress::default()));
        let shared = progress.clone();
        std::thread::Builder::new()
            .name("escape-orbit".into())
            .spawn(move || {
                let mut current: Option<(OrbitRequest, ReferenceOrbit, u64)> = None;
                loop {
                    // Take the newest pending request (drain the queue).
                    let mut next = match current {
                        // Idle: block for work.
                        None => match rx.recv() {
                            Ok(r) => Some(r),
                            Err(_) => return,
                        },
                        // Working: only preempt if something arrived.
                        Some(_) => match rx.try_recv() {
                            Ok(r) => Some(r),
                            Err(std::sync::mpsc::TryRecvError::Empty) => None,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                        },
                    };
                    while let Ok(r) = rx.try_recv() {
                        next = Some(r);
                    }

                    if let Some((epoch, req)) = next {
                        // Same orbit, deeper? Keep the live state and
                        // append. Otherwise start fresh.
                        let reuse = current.take().and_then(|(old_req, orbit, _)| {
                            let same = old_req.center_re == req.center_re
                                && old_req.center_im == req.center_im
                                && old_req.n_limbs == req.n_limbs
                                && old_req.julia_c == req.julia_c
                                && old_req.power == req.power
                                && old_req.ship == req.ship
                                && old_req.ship_variant == req.ship_variant
                                && old_req.reference_period == req.reference_period
                                && orbit.periodic_serves(req.zoom_log2);
                            if same
                                && orbit
                                    .relocation_serves(req.zoom_log2, req.height_px.max(1.0))
                            {
                                Some(orbit)
                            } else {
                                None
                            }
                        });
                        let orbit = match reuse {
                            Some(o) => o,
                            None => {
                                match worker_compute_orbit(&req) {
                                    Some(o) => o,
                                    None => {
                                        // Unparseable center: publish an
                                        // empty done state.
                                        let mut p = shared.lock().unwrap();
                                        p.epoch = epoch;
                                        p.orbit.clear();
                                        p.orbit_lo.clear();
                                        p.done = true;
                                        continue;
                                    }
                                }
                            }
                        };
                        // Publish the (possibly reused) prefix.
                        {
                            let mut p = shared.lock().unwrap();
                            p.epoch = epoch;
                            p.orbit.clear();
                            p.orbit.extend_from_slice(&orbit.orbit);
                            p.orbit_lo.clear();
                            p.orbit_lo.extend_from_slice(&orbit.orbit_lo);
                            p.ref_offset = orbit.ref_offset;
                            p.off_zoom_log2 = orbit.off_zoom_log2;
                            p.off_height_px = orbit.off_height_px;
                            p.detected_period = orbit.periodic;
                            p.done = orbit.periodic.is_some()
                                || orbit.escaped_at.is_some()
                                || orbit.len() > req.max_iter;
                        }
                        current = Some((req, orbit, epoch));
                    }

                    // Advance the current job by one chunk.
                    if let Some((req, orbit, epoch)) = current.as_mut() {
                        orbit.set_closure_limit(req.zoom_log2);
                        let target = (orbit.len().saturating_sub(1) + Self::CHUNK)
                            .min(req.max_iter);
                        orbit.extend(target);
                        let done = orbit.periodic.is_some()
                            || orbit.escaped_at.is_some()
                            || orbit.len() > req.max_iter;
                        {
                            let mut p = shared.lock().unwrap();
                            if p.epoch == *epoch {
                                let have = p.orbit.len();
                                if orbit.orbit.len() >= have {
                                    p.orbit.extend_from_slice(&orbit.orbit[have..]);
                                    p.orbit_lo.extend_from_slice(&orbit.orbit_lo[have..]);
                                } else {
                                    // Auto-closure truncated the orbit
                                    // to one period: republish whole.
                                    p.orbit.clear();
                                    p.orbit.extend_from_slice(&orbit.orbit);
                                    p.orbit_lo.clear();
                                    p.orbit_lo.extend_from_slice(&orbit.orbit_lo);
                                }
                                p.detected_period = orbit.periodic;
                                p.done = done;
                            }
                        }
                        if done {
                            // Persist the finished orbit (cost-gated).
                            super::orbit_store::maybe_save(orbit);
                            // Keep state for future deepening but stop
                            // spinning: park until the next request.
                            let parked = current.take().unwrap();
                            match rx.recv() {
                                Ok(r) => {
                                    current = Some((parked.0, parked.1, parked.2));
                                    // Re-queue the received request into
                                    // the normal path next loop.
                                    let _ = tx_loopback_send(&shared, r, &mut current);
                                }
                                Err(_) => return,
                            }
                        }
                    }
                }
            })
            .expect("spawn escape-orbit worker");
        Self { tx, progress, latest: None, epoch: 0 }
    }

    /// Post a request (deduplicated). Returns the epoch that data for
    /// it will carry.
    pub fn request(&mut self, req: OrbitRequest) -> u64 {
        if self.latest.as_ref() == Some(&req) {
            return self.epoch;
        }
        self.epoch += 1;
        self.latest = Some(req.clone());
        let _ = self.tx.send((self.epoch, req));
        self.epoch
    }
}

/// Worker-side orbit construction: nucleus-aware for the eligible
/// case (parameter-plane Mandelbrot), plain otherwise. Starts at zero
/// iterations — the chunk loop grows it (a periodic nucleus orbit
/// arrives complete).
#[cfg(not(target_arch = "wasm32"))]
fn worker_compute_orbit(req: &OrbitRequest) -> Option<ReferenceOrbit> {
    if let Some(o) = super::orbit_store::load(
        &req.center_re,
        &req.center_im,
        req.n_limbs,
        req.julia_c,
        req.power,
        req.ship,
        req.ship_variant,
        req.zoom_log2,
        req.height_px.max(1.0),
    ) {
        if req.reference_period.is_none() || o.periodic == req.reference_period {
            return Some(o);
        }
    }
    if req.julia_c.is_none() && !req.ship {
        ReferenceOrbit::compute_nucleus_aware(
            &req.center_re,
            &req.center_im,
            req.zoom_log2,
            0,
            req.height_px.max(1.0),
            req.power,
            req.reference_period,
        )
    } else {
        ReferenceOrbit::compute(
            &req.center_re,
            &req.center_im,
            0.0,
            Some(req.n_limbs),
            0,
            req.julia_c,
            req.power,
            req.ship,
            req.ship_variant,
        )
    }
}

/// Helper for the parked-worker wakeup: fold a received request into
/// `current` exactly the way the main loop would.
#[cfg(not(target_arch = "wasm32"))]
fn tx_loopback_send(
    shared: &std::sync::Arc<std::sync::Mutex<OrbitProgress>>,
    (epoch, req): (u64, OrbitRequest),
    current: &mut Option<(OrbitRequest, ReferenceOrbit, u64)>,
) -> Option<()> {
    let reuse = current.take().and_then(|(old_req, orbit, _)| {
        let same = old_req.center_re == req.center_re
            && old_req.center_im == req.center_im
            && old_req.n_limbs == req.n_limbs
            && old_req.julia_c == req.julia_c
            && old_req.power == req.power
                                && old_req.ship == req.ship
                                && old_req.ship_variant == req.ship_variant
                                && old_req.reference_period == req.reference_period
                                && orbit.periodic_serves(req.zoom_log2);
        if same && orbit.relocation_serves(req.zoom_log2, req.height_px.max(1.0)) {
            Some(orbit)
        } else {
            None
        }
    });
    let orbit = match reuse {
        Some(o) => o,
        None => worker_compute_orbit(&req)?,
    };
    {
        let mut p = shared.lock().unwrap();
        p.epoch = epoch;
        p.orbit.clear();
        p.orbit.extend_from_slice(&orbit.orbit);
        p.orbit_lo.clear();
        p.orbit_lo.extend_from_slice(&orbit.orbit_lo);
        p.ref_offset = orbit.ref_offset;
        p.off_zoom_log2 = orbit.off_zoom_log2;
        p.off_height_px = orbit.off_height_px;
        p.detected_period = orbit.periodic;
        p.done = orbit.periodic.is_some()
            || orbit.escaped_at.is_some()
            || orbit.len() > req.max_iter;
    }
    *current = Some((req, orbit, epoch));
    Some(())
}

impl ReferenceOrbit {
    /// Build a PERIODIC reference from a period hint (fraktaler-3's
    /// `reference.period`): the center is taken as the nucleus, its
    /// orbit computed for exactly one period at the view's full
    /// precision, and VERIFIED to close (|Z_period| below f32
    /// visibility — the exact-wrap requirement). A wrong hint returns
    /// None (the caller warns and falls back). This is the deep-dive
    /// reference: one period long, never extends, and its
    /// delta-crushes align with the true cascade dynamics.
    pub fn try_periodic_from_hint(
        center_re: &str,
        center_im: &str,
        zoom_log2: f64,
        period: u32,
        power: u32,
    ) -> Option<Self> {
        if period == 0 {
            return None;
        }
        let n = super::fixedpoint::limbs_for_view(center_re, center_im, zoom_log2);
        // Compute WITHOUT auto-closure: a shallower cascade closure
        // would truncate the orbit mid-verification and preempt the
        // hinted DEEP reference (observed: period 142,232 stealing a
        // 1,137,764 hint). The hint asks for exactly this period; the
        // closure check below is the arbiter.
        let mut orbit = Self::compute(
            center_re, center_im, zoom_log2, Some(n), 0, None, power, false, 0,
        )?;
        orbit.closure_limit_octave = i64::MIN / 4;
        orbit.extend(period);
        if orbit.escaped_at.is_some() || orbit.len() <= period {
            log::warn!("reference period {period}: center orbit escapes before closing");
            return None;
        }
        // Closure check on the LIVE fixed-point state (the stored f32
        // value would round a near-miss to zero).
        let zx = orbit.z.re.to_f64();
        let zy = orbit.z.im.to_f64();
        if zx * zx + zy * zy > 2f64.powi(-48) {
            log::warn!(
                "reference period {period} rejected: |Z_period| ~ {:.3e} (not a nucleus closure)",
                (zx * zx + zy * zy).sqrt()
            );
            return None;
        }
        orbit.periodic = Some(period);
        orbit.orbit.truncate(period as usize + 1);
        orbit.orbit_lo.truncate(period as usize + 1);
        {
            let fx = orbit.z.re.to_floatexp();
            let fy = orbit.z.im.to_floatexp();
            orbit.closure_octave = match (fx.m == 0.0, fy.m == 0.0) {
                (true, true) => i64::MIN / 2,
                (true, false) => fy.e,
                (false, true) => fx.e,
                (false, false) => fx.e.max(fy.e),
            };
        }
        log::info!("periodic reference from hint: period {period} at {n} limbs");
        Some(orbit)
    }

    /// Parameter-plane Mandelbrot: try a nucleus-relocated periodic
    /// reference first (exact wrap, period-length orbit, maximal
    /// glitch resistance); fall back to the view-center reference.
    /// `height_px` converts the relocation offset into pixel units.
    pub fn compute_nucleus_aware(
        center_re: &str,
        center_im: &str,
        zoom_log2: f64,
        max_iter: u32,
        height_px: f64,
        power: u32,
        period_hint: Option<u32>,
    ) -> Option<Self> {
        // Diagnostic escape hatch: render with plain references to
        // isolate relocation-dependent differences.
        let skip_nucleus = std::env::var("ESCAPE_DISABLE_NUCLEUS").is_ok();
        if !skip_nucleus {
            if let Some(p) = period_hint {
                if let Some(orbit) =
                    Self::try_periodic_from_hint(center_re, center_im, zoom_log2, p, power)
                {
                    return Some(orbit);
                }
            }
        }
        if let Some((nre, nim, period, off)) = (!skip_nucleus)
            .then(|| nucleus_for_view(center_re, center_im, zoom_log2, height_px, power))
            .flatten()
        {
            log::info!(
                "nucleus relocation: period {period}, offset ({:.3}, {:.3}) px, zoom {zoom_log2:.2}",
                off[0],
                off[1]
            );
            if let Some(mut orbit) = Self::compute(
                &nre,
                &nim,
                zoom_log2,
                None,
                period,
                None,
                power,
                false,
                0,
            ) {
                if orbit.escaped_at.is_none() && orbit.len() > period {
                    // Store under the VIEW key (the cache is keyed on
                    // what was asked for), remember the relocation.
                    orbit.center_re = center_re.to_string();
                    orbit.center_im = center_im.to_string();
                    orbit.periodic = Some(period);
                    orbit.closure_octave = i64::MIN / 2;
                    orbit.ref_offset = off;
                    orbit.off_zoom_log2 = zoom_log2;
                    orbit.off_height_px = height_px.max(1.0);
                    return Some(orbit);
                }
            }
        }
        let n = super::fixedpoint::limbs_for_view(center_re, center_im, zoom_log2);
        Self::compute(
            center_re, center_im, zoom_log2, Some(n), max_iter, None, power, false, 0,
        )
    }
}

/// Single-slot orbit cache: during a continuous zoom the center is
/// unchanged, so one orbit serves every frame; deepening appends. A
/// pan or precision change replaces the slot.
#[derive(Default)]
pub struct OrbitCache {
    slot: Option<ReferenceOrbit>,
    height_px: f64,
    /// Verified-before-use period hint for parameter-plane power
    /// tiers (see [`ReferenceOrbit::try_periodic_from_hint`]).
    reference_period: Option<u32>,
}

impl OrbitCache {
    /// The currently cached orbit, if any (read-only — BLA table
    /// construction reads the CPU copy the GPU mirror was built from).
    pub fn peek(&self) -> Option<&ReferenceOrbit> {
        self.slot.as_ref()
    }

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
        ship: bool,
        ship_variant: u32,
    ) -> Option<&ReferenceOrbit> {
        // The center's own digits set a precision FLOOR: a truncated
        // deep center is a different (shallow, early-escaping) point,
        // and pixels that can't outgrow d0 before that escape collapse
        // onto it (the zoom-685 uniform-frame bug).
        let n = super::fixedpoint::limbs_for_view(center_re, center_im, zoom_log2);
        let hit = self.slot.as_ref().is_some_and(|o| {
            o.serves(center_re, center_im, n, julia_c, power, ship, ship_variant)
                && o.periodic_serves(zoom_log2)
        });
        if hit {
            let orbit = self.slot.as_mut().unwrap();
            orbit.set_closure_limit(zoom_log2);
            orbit.extend(max_iter);
        } else {
            // Disk store first (desktop): an exact-identity hit skips
            // the fixed-point recompute entirely and still deepens.
            #[cfg(not(target_arch = "wasm32"))]
            let loaded = super::orbit_store::load(
                center_re,
                center_im,
                n,
                julia_c,
                power,
                ship,
                ship_variant.min(5),
                zoom_log2,
                self.height_px.max(1.0),
            );
            #[cfg(target_arch = "wasm32")]
            let loaded: Option<ReferenceOrbit> = None;
            // A stored orbit only serves a hint-set request if it IS
            // the hinted periodic form.
            let loaded = loaded.filter(|o| {
                (self.reference_period.is_none() || o.periodic == self.reference_period)
                    && o.periodic_serves(zoom_log2)
            });
            let orbit = if let Some(mut o) = loaded {
                o.extend(max_iter);
                o
            } else if julia_c.is_none() && !ship {
                ReferenceOrbit::compute_nucleus_aware(
                    center_re,
                    center_im,
                    zoom_log2,
                    max_iter,
                    self.height_px.max(1.0),
                    power,
                    self.reference_period,
                )?
            } else {
                ReferenceOrbit::compute(
                    center_re, center_im, zoom_log2, Some(n), max_iter, julia_c, power, ship,
                    ship_variant,
                )?
            };
            self.slot = Some(orbit);
        }
        // Persist anything worth keeping (cost-gated; rewrites only
        // when deeper than what the store already holds).
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(o) = self.slot.as_ref() {
            super::orbit_store::maybe_save(o);
        }
        self.slot.as_ref()
    }

    /// Budgeted variant of [`get`](Self::get): extend the orbit by at
    /// most `budget` iterations this call, returning the (possibly
    /// partial) orbit and whether it now covers the request. The
    /// single-threaded WASM path calls this once per frame so the tab
    /// stays responsive while a deep reference computes; rebasing
    /// renders partial-orbit frames correctly (early wrap), so each
    /// slice refines the image. Skips the nucleus search (a blocking
    /// Newton run has no place on a UI thread) — plain references,
    /// which rebasing serves fine.
    pub fn get_budgeted(
        &mut self,
        center_re: &str,
        center_im: &str,
        zoom_log2: f64,
        max_iter: u32,
        julia_c: Option<(f32, f32)>,
        power: u32,
        ship: bool,
        ship_variant: u32,
        budget: u32,
    ) -> Option<(&ReferenceOrbit, bool)> {
        let n = super::fixedpoint::limbs_for_view(center_re, center_im, zoom_log2);
        let hit = self.slot.as_ref().is_some_and(|o| {
            o.serves(center_re, center_im, n, julia_c, power, ship, ship_variant)
                && o.periodic_serves(zoom_log2)
        });
        let budget = budget.max(64);
        if hit {
            let orbit = self.slot.as_mut().unwrap();
            orbit.set_closure_limit(zoom_log2);
            let target = orbit.len().saturating_sub(1).saturating_add(budget).min(max_iter);
            orbit.extend(target);
        } else {
            self.slot = Some(ReferenceOrbit::compute(
                center_re,
                center_im,
                zoom_log2,
                Some(n),
                budget.min(max_iter),
                julia_c,
                power,
                ship,
                ship_variant,
            )?);
        }
        let orbit = self.slot.as_ref().unwrap();
        let done = orbit.periodic.is_some()
            || orbit.escaped_at.is_some()
            || orbit.len() > max_iter;
        Some((orbit, done))
    }

    /// The reference-period hint; a change invalidates the slot so
    /// the next get() rebuilds with (or without) the periodic form.
    pub fn set_reference_period(&mut self, period: Option<u32>) {
        if self.reference_period != period {
            self.reference_period = period;
            self.slot = None;
        }
    }

    /// The viewport height the relocation offset is measured against.
    pub fn set_height(&mut self, h: f64) {
        if (self.height_px - h).abs() > 0.5 {
            self.height_px = h;
            // The offset unit changed: recompute on next get.
            self.slot = None;
        }
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
        let orbit = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 100, None, 2, false, 0).unwrap();
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
        let orbit = ReferenceOrbit::compute("1", "1", 5.0, None, 1000, None, 2, false, 0).unwrap();
        let at = orbit.escaped_at.expect("c = 1+i escapes fast");
        assert!(at < 10);
        assert_eq!(orbit.len() - 1, at);
    }

    #[test]
    fn deepen_is_an_append_and_matches_fresh_compute() {
        let mut a = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 50, None, 2, false, 0).unwrap();
        a.extend(120);
        let b = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 120, None, 2, false, 0).unwrap();
        assert_eq!(a.orbit.len(), b.orbit.len());
        for (i, (x, y)) in a.orbit.iter().zip(b.orbit.iter()).enumerate() {
            assert_eq!(x, y, "diverged at iteration {i}");
        }
    }

    #[test]
    fn cache_hits_extends_and_replaces() {
        // Julia c = 0 (z -> z^2 from |z0| < 1: bounded, never escapes,
        // and the Julia key skips nucleus relocation) keeps orbit
        // lengths exactly deterministic for the cache mechanics.
        let jc = Some((0.0f32, 0.0f32));
        let mut cache = OrbitCache::default();
        {
            let o = cache.get("-0.5", "0.1", 10.0, 50, jc, 2, false, 0).unwrap();
            assert_eq!(o.len(), 51);
        }
        // Same view, deeper iterations: extend in place.
        {
            let o = cache.get("-0.5", "0.1", 10.0, 80, jc, 2, false, 0).unwrap();
            assert_eq!(o.len(), 81);
        }
        // Different center: replace.
        {
            let o = cache.get("-0.75", "0.1", 10.0, 10, jc, 2, false, 0).unwrap();
            assert_eq!(o.center_re, "-0.75");
            assert_eq!(o.len(), 11);
        }
        // Parameter-plane Mandelbrot goes nucleus-aware: an interior
        // view relocates to a periodic reference.
        {
            let o = cache.get("-1.0", "0.0", 10.0, 100, None, 2, false, 0).unwrap();
            assert_eq!(o.periodic, Some(2), "the period-2 nucleus governs c = -1");
            assert_eq!(o.len(), 3);
        }
        // Unparseable center: None.
        assert!(cache.get("not a number", "0", 10.0, 10, None, 2, false, 0).is_none());
    }

    #[test]
    fn julia_orbit_seeds_from_the_center() {
        // Julia: z0 = center, c fixed. First entry must be the center,
        // then iterate z^2 + c.
        let orbit = ReferenceOrbit::compute(
            "0.25", "0.5", 10.0, None, 20, Some((-0.8, 0.156)), 2, false, 0,
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
        cache.get("0.25", "0.5", 10.0, 20, Some((-0.8, 0.156)), 2, false, 0).unwrap();
        let replaced = cache.get("0.25", "0.5", 10.0, 20, None, 2, false, 0).unwrap();
        assert_eq!(replaced.julia_c, None);
    }

    #[test]
    fn cubic_orbit_matches_f64_iteration() {
        // power 3: z^3 + c against a plain f64 loop.
        let orbit =
            ReferenceOrbit::compute("-0.2", "0.4", 10.0, None, 60, None, 3, false, 0).unwrap();
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn worker_converges_to_the_synchronous_orbit() {
        let mut worker = OrbitWorker::new();
        let req = OrbitRequest {
            center_re: "-0.5".into(),
            center_im: "0.1".into(),
            n_limbs: 3,
            max_iter: 10_000,
            julia_c: None,
            power: 2,
            ship: false,
            ship_variant: 0,
            reference_period: None,
            zoom_log2: 5.0,
            height_px: 320.0,
        };
        let epoch = worker.request(req);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            {
                let p = worker.progress.lock().unwrap();
                if p.epoch == epoch && p.done {
                    break;
                }
            }
            assert!(std::time::Instant::now() < deadline, "worker never finished");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // The worker goes nucleus-aware for this request; compare
        // against the same nucleus-aware synchronous compute.
        let sync = ReferenceOrbit::compute_nucleus_aware("-0.5", "0.1", 5.0, 10_000, 320.0, 2, None)
            .unwrap();
        let p = worker.progress.lock().unwrap();
        assert_eq!(p.orbit.len(), sync.orbit.len());
        assert_eq!(p.orbit, sync.orbit, "worker orbit differs from synchronous");
        assert_eq!(p.ref_offset, sync.ref_offset);

        // Preemption: a new request replaces the old epoch.
        drop(p);
        let e2 = worker.request(OrbitRequest {
            center_re: "-0.75".into(),
            center_im: "0.05".into(),
            n_limbs: 3,
            max_iter: 2_000,
            julia_c: None,
            power: 2,
            ship: false,
            ship_variant: 0,
            reference_period: None,
            zoom_log2: 5.0,
            height_px: 320.0,
        });
        assert!(e2 > epoch);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            {
                let p = worker.progress.lock().unwrap();
                if p.epoch == e2 && p.done {
                    break;
                }
            }
            assert!(std::time::Instant::now() < deadline, "second request never finished");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn ship_orbit_matches_f64_iteration() {
        let orbit =
            ReferenceOrbit::compute("-0.6", "-0.4", 10.0, None, 60, None, 2, true, 0).unwrap();
        let (mut zx, mut zy) = (0.0f64, 0.0f64);
        for i in 1..=60usize {
            let (ax, ay) = (zx.abs(), zy.abs());
            let t = ax * ax - ay * ay + -0.6;
            zy = 2.0 * ax * ay + -0.4;
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
    fn nucleus_reference_relocates_and_is_periodic() {
        // A view near (not on) the period-3 antenna nucleus: the
        // reference must relocate to the nucleus (Z_3 = 0), carry the
        // relocation offset, and arrive complete at period length.
        let orbit = ReferenceOrbit::compute_nucleus_aware(
            "-1.7548776",
            "0.0000001",
            20.0,
            5000,
            320.0,
            2,
            None,
        )
        .expect("computes");
        assert_eq!(orbit.periodic, Some(3));
        assert_eq!(orbit.len(), 4); // Z_0..Z_3
        let [zx, zy] = orbit.orbit[3];
        assert!(
            (zx * zx + zy * zy) < 1e-10,
            "Z_period must be ~0, got ({zx}, {zy})"
        );
        assert!(
            orbit.ref_offset[0].abs() > 0.0 || orbit.ref_offset[1].abs() > 0.0,
            "off-nucleus view must carry a relocation offset"
        );
        // Far from any small-period atom: falls back to the plain
        // view-center reference.
        let plain = ReferenceOrbit::compute_nucleus_aware("0.3", "0.5", 20.0, 100, 320.0, 2, None)
            .expect("computes");
        assert_eq!(plain.periodic, None);
        assert_eq!(plain.ref_offset, [0.0, 0.0]);
    }





    #[test]
    fn ship_variant_orbits_match_f64() {
        // Each variant's fixed-point step against a plain f64 loop.
        let f64_step = |v: u32, zx: f64, zy: f64, cr: f64, ci: f64| -> (f64, f64) {
            let (x, y) = (zx, zy);
            let (re, im) = match v {
                0 => {
                    let (a, b) = (x.abs(), y.abs());
                    (a * a - b * b, 2.0 * a * b)
                }
                1 => (x * x - y * y, -2.0 * x.abs() * y),
                2 => (x * x - y * y, -2.0 * x * y.abs()),
                3 => ((x * x - y * y).abs(), 2.0 * x * y),
                4 => ((x * x - y * y).abs(), -2.0 * (x * y).abs()),
                _ => ((x * x - y * y).abs(), -2.0 * x.abs() * y),
            };
            (re + cr, im + ci)
        };
        for v in 0..=5u32 {
            let orbit = ReferenceOrbit::compute(
                "-0.55", "-0.4", 10.0, None, 40, None, 2, true, v,
            )
            .unwrap();
            let (mut zx, mut zy) = (0.0f64, 0.0f64);
            for i in 1..=40usize {
                let (nx, ny) = f64_step(v, zx, zy, -0.55, -0.4);
                zx = nx;
                zy = ny;
                if i >= orbit.orbit.len() {
                    break;
                }
                let [ox, oy] = orbit.orbit[i];
                assert!(
                    (ox as f64 - zx).abs() < 1e-4 && (oy as f64 - zy).abs() < 1e-4,
                    "variant {v} iteration {i}: orbit ({ox}, {oy}) vs f64 ({zx}, {zy})"
                );
            }
        }
    }

    #[test]
    fn budgeted_get_converges_to_the_blocking_orbit() {
        // Slices must end at the exact same orbit the blocking call
        // produces (Julia key: skips nucleus relocation on both
        // paths, so the comparison is byte-exact).
        let jc = Some((-0.8f32, 0.156f32));
        let mut blocking = OrbitCache::default();
        let full = blocking
            .get("0.1", "0.2", 12.0, 500, jc, 2, false, 0)
            .unwrap()
            .orbit
            .clone();
        let mut sliced = OrbitCache::default();
        let mut steps = 0;
        loop {
            let (orbit, done) = sliced
                .get_budgeted("0.1", "0.2", 12.0, 500, jc, 2, false, 0, 64)
                .unwrap();
            steps += 1;
            assert!(steps < 100, "failed to converge");
            assert!(
                orbit.orbit.as_slice() == &full[..orbit.orbit.len().min(full.len())],
                "slice {steps} diverged from the blocking orbit"
            );
            if done {
                assert_eq!(orbit.orbit, full, "converged orbit differs");
                break;
            }
        }
        assert!(steps > 3, "budget was not actually slicing ({steps} steps)");
    }



    #[test]
    fn extend_auto_detects_a_periodic_center() {
        // The period-3 antenna nucleus (16-digit truncation): the
        // center orbit closes at index 3 far below f32 visibility —
        // extend() must become the periodic reference on its own,
        // truncate to one period, and stop.
        let orbit = ReferenceOrbit::compute(
            "-1.7548776662466927",
            "0",
            10.0,
            None,
            1000,
            None,
            2,
            false,
            0,
        )
        .unwrap();
        assert_eq!(orbit.periodic, Some(3), "auto-detection missed the closure");
        assert_eq!(orbit.len(), 4, "orbit must truncate to one period");
        // The SAME center at a deep view must NOT accept the shallow
        // closure (|Z_3| ~ 2^-56 is far above a zoom-300 pixel): the
        // wrap would inject a visible discontinuity there.
        let deep_view = ReferenceOrbit::compute(
            "-1.7548776662466927",
            "0",
            300.0,
            None,
            1000,
            None,
            2,
            false,
            0,
        )
        .unwrap();
        assert_eq!(
            deep_view.periodic, None,
            "shallow closure must be rejected at a deep view"
        );
        assert!(
            !orbit.periodic_serves(300.0),
            "periodic_serves must retire the closure as the view deepens"
        );
        assert!(orbit.periodic_serves(12.0));

        // A generic exterior center must NEVER auto-close.
        let plain =
            ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 500, None, 2, false, 0).unwrap();
        assert_eq!(plain.periodic, None);
        // Julia references are exempt (their wrap returns to Z_0, not 0).
        let julia = ReferenceOrbit::compute(
            "0.0",
            "0.0",
            10.0,
            None,
            100,
            Some((-0.1, 0.05)),
            2,
            false,
            0,
        )
        .unwrap();
        assert_eq!(julia.periodic, None, "Julia must not auto-close");
    }

    #[test]
    fn relocation_offset_rescales_with_the_view() {
        let mut orbit =
            ReferenceOrbit::compute("-0.5", "0.1", 30.0, None, 50, None, 2, false, 0).unwrap();
        orbit.ref_offset = [12.0, -3.0];
        orbit.off_zoom_log2 = 30.0;
        orbit.off_height_px = 480.0;
        // Same view: identity.
        assert_eq!(orbit.offset_for_view(30.0, 480.0), Some([12.0, -3.0]));
        // +2 zoom doubles pixel density twice: offsets scale by 4.
        assert_eq!(orbit.offset_for_view(32.0, 480.0), Some([48.0, -12.0]));
        // Doubled height (e.g. 2x supersampling): offsets double —
        // THE "pan changes with AA" bug when this was missing.
        assert_eq!(orbit.offset_for_view(30.0, 960.0), Some([24.0, -6.0]));
        // Zooming far OUT overflows the useful range: a miss, so the
        // caller recomputes instead of rendering a garbage offset.
        assert_eq!(orbit.offset_for_view(30.0 - 40.0, 480.0), Some([12.0 / 1.0995116e12, -3.0 / 1.0995116e12].map(|v: f32| v)));
        assert!(orbit.offset_for_view(70.0, 480.0).is_none(), "overflow must miss");
        assert!(!orbit.relocation_serves(70.0, 480.0));
        // Offset-free orbits serve any view.
        orbit.ref_offset = [0.0, 0.0];
        assert!(orbit.relocation_serves(300.0, 480.0));
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
            false,
            0,
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
            false,
            0,
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
