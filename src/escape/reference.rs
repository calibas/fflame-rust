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

/// Which map a reference orbit iterates -- everything that changes
/// the ORBIT, and therefore everything its cache key must cover.
///
/// Collapses what used to be three loose arguments threaded through
/// every orbit signature, and adds the piece Phoenix needs: a
/// continuous formula parameter. That cannot ride a variant enum the
/// way the Tricorn fold selector does, and a reference cached without
/// it would be silently reused after the user changed it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MapId {
    /// Exponent of the power map (2 for the Ship and Phoenix families).
    pub power: u32,
    /// Burning Ship family: the fold step, with `variant` selecting
    /// the arrangement.
    pub ship: bool,
    /// Ship fold variant when `ship`; otherwise which NON-fold family
    /// this is (`MAP_PLAIN`, `MAP_CONJ`, `MAP_PHOENIX`).
    pub variant: u32,
    /// Continuous parameter that changes the map. Phoenix's `p`
    /// (re, im); zero for every other family.
    pub params: [f32; 2],
}

impl MapId {
    /// The plain power map `z^p + c`.
    pub fn power(p: u32) -> Self {
        Self { power: p.max(2), ship: false, variant: MAP_PLAIN, params: [0.0, 0.0] }
    }
    /// A Burning Ship fold variant.
    pub fn ship(variant: u32) -> Self {
        Self { power: 2, ship: true, variant: variant.min(5), params: [0.0, 0.0] }
    }
    /// `conj(z)^p + c`.
    pub fn conj(p: u32) -> Self {
        Self { power: p.max(2), ship: false, variant: MAP_CONJ, params: [0.0, 0.0] }
    }
    /// `z^2 + c + p*z_prev`.
    pub fn phoenix(p: [f32; 2]) -> Self {
        Self { power: 2, ship: false, variant: MAP_PHOENIX, params: p }
    }
    /// Bytes for the on-disk key and the serialized identity.
    pub fn key_bytes(&self) -> [u8; 17] {
        let mut b = [0u8; 17];
        b[0..4].copy_from_slice(&self.power.to_le_bytes());
        b[4] = self.ship as u8;
        b[5..9].copy_from_slice(&self.variant.to_le_bytes());
        b[9..13].copy_from_slice(&self.params[0].to_le_bytes());
        b[13..17].copy_from_slice(&self.params[1].to_le_bytes());
        b
    }
}

/// `ship_variant` when `ship` is FALSE: which non-fold map the
/// reference iterates. 0 is the plain power `z^p`; 1 is `conj(z)^p`,
/// the Tricorn/Multicorn family.
///
/// Encoded in the existing variant rather than as a parallel `conj`
/// flag because that field already threads through every orbit
/// signature, the on-disk key and `serves()` -- so a new flag would
/// mean the same fact in two places, and a cache key that could
/// disagree with itself. When a third non-fold family lands, the
/// honest refactor is a small `MapId` struct carrying all of
/// (power, ship, variant); this is deliberately the cheaper step.
pub const MAP_PLAIN: u32 = 0;
pub const MAP_CONJ: u32 = 1;
/// `z^2 + c + p*z_prev` -- carries a second live state and a
/// continuous parameter, so it is the family that forced [`MapId`].
pub const MAP_PHOENIX: u32 = 2;

/// The period of the reference the renderer is CURRENTLY using, for
/// the panel to display: 0 = aperiodic (or none yet). Progressive
/// detection finds deeper periods as a dive continues and retires
/// shallow ones, so this changes under the user mid-zoom - which is
/// exactly what makes it worth showing rather than leaving implicit.
static LIVE_PERIOD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Seconds a reference of `iters` iterations at `n_limbs` will take
/// to compute, from the measured cost of the step.
///
/// A reference iteration is two truncated big multiplies and nothing
/// else that matters (measured: 48.9 us at 197 limbs, 91% of it in
/// those two calls), so the cost is iterations x limbs^2 x a
/// constant. The constant is calibrated against the f3 reference:
/// 10,100,100 iterations at 197 limbs took 495 s.
pub fn predicted_orbit_seconds(iters: u32, n_limbs: usize) -> f64 {
    const PER_LIMB2_ITER: f64 = 1.263e-9;
    iters as f64 * (n_limbs as f64) * (n_limbs as f64) * PER_LIMB2_ITER
}

/// How far along the reference the renderer is waiting for is:
/// (iterations computed, iterations wanted). Both zero when nothing
/// is pending.
static ORBIT_HAVE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static ORBIT_WANT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Publish reference-build progress for the viewport overlay. `want`
/// of zero clears it.
pub fn set_orbit_progress(have: u32, want: u32) {
    ORBIT_HAVE.store(have, std::sync::atomic::Ordering::Relaxed);
    ORBIT_WANT.store(want, std::sync::atomic::Ordering::Relaxed);
}

/// (computed, wanted) while a reference the render is WAITING on is
/// still building, else None. A reference that renders progressively
/// never reports here — there is nothing for the user to wait for.
pub fn orbit_progress() -> Option<(u32, u32)> {
    let want = ORBIT_WANT.load(std::sync::atomic::Ordering::Relaxed);
    if want == 0 {
        return None;
    }
    Some((ORBIT_HAVE.load(std::sync::atomic::Ordering::Relaxed), want))
}

/// Record the period of the reference now in use (None = aperiodic).
pub fn set_live_reference_period(period: Option<u32>) {
    LIVE_PERIOD.store(
        period.unwrap_or(0),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// The period of the reference now in use, if it is periodic.
pub fn live_reference_period() -> Option<u32> {
    match LIVE_PERIOD.load(std::sync::atomic::Ordering::Relaxed) {
        0 => None,
        p => Some(p),
    }
}

/// The iterate's plain f32 value: `hi * 2^e`, flushing to zero below
/// f32's normal range - which is exactly what the pre-exponent
/// storage handed every consumer, so any reader that only needs a
/// coarse value (BLA radii, the rebase test, `z_full`) keeps its old
/// semantics.
#[inline]
pub fn entry_value(hi: [f32; 2], e: i32) -> [f32; 2] {
    if e == 0 {
        return hi;
    }
    if e < -126 {
        return [0.0, 0.0];
    }
    let s = 2f32.powi(e);
    [hi[0] * s, hi[1] * s]
}

/// Split an iterate into the (hi, lo, exponent) storage form.
///
/// Above 2^-90 the exponent is 0 and the pair is the plain DF value -
/// byte-identical to the old format, and `lo` stays a normal f32.
/// Below it the magnitude is taken from the LIVE fixed-point state,
/// not from the f64 pair: deep references reach 2^-1379, far past
/// f64's 2^-1074, so an f64 round-trip would lose the very iterates
/// this exists to keep.
fn split_entry(z: &FixedComplex, x: f64, y: f64) -> ([f32; 2], [f32; 2], i32) {
    const DEEP: f64 = 8.077_935_669_463_16e-28; // 2^-90
    if x.abs().max(y.abs()) >= DEEP {
        let hix = x as f32;
        let hiy = y as f32;
        return (
            [hix, hiy],
            [(x - hix as f64) as f32, (y - hiy as f64) as f32],
            0,
        );
    }
    let fx = z.re.to_floatexp();
    let fy = z.im.to_floatexp();
    let e = match (fx.m == 0.0, fy.m == 0.0) {
        (true, true) => return ([0.0, 0.0], [0.0, 0.0], 0),
        (true, false) => fy.e,
        (false, true) => fx.e,
        (false, false) => fx.e.max(fy.e),
    };
    let comp = |f: super::fixedpoint::FloatExp| -> f64 {
        if f.m == 0.0 {
            return 0.0;
        }
        let shift = f.e - e;
        if shift < -1070 {
            return 0.0;
        }
        f.m * (shift as f64).exp2()
    };
    let mx = comp(fx);
    let my = comp(fy);
    let hix = mx as f32;
    let hiy = my as f32;
    (
        [hix, hiy],
        [(mx - hix as f64) as f32, (my - hiy as f64) as f32],
        e.clamp(-2_000_000_000, 2_000_000_000) as i32,
    )
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
    /// Per-entry binary exponent: the stored iterate is
    /// `(hi + lo) * 2^orbit_e[i]`. It is **zero for every iterate at
    /// or above 2^-90**, so those entries hold the plain f32 value
    /// and read exactly as they did before this field existed.
    ///
    /// Only a reference passing very close to a nucleus produces a
    /// nonzero exponent - and those iterates are precisely the ones
    /// f32 storage used to flush to zero, which deleted the 2*Z*delta
    /// term from the delta recurrence for that step. Measured on the
    /// z700 field location: a 2^-183 dip at i=8897 cost the delta 154
    /// octaves it never recovered (growth ran at half the true rate
    /// from there on), pushing a corner pixel's escape from its true
    /// 23,649 out to 41,163 - the "deep zoom renders interior mush"
    /// wall.
    pub orbit_e: Vec<i32>,
    /// Compression corrections (see [`Correction`]): the places the
    /// DD shadow was reset to full precision. Everything BETWEEN
    /// corrections is regenerated on load by replaying the shadow --
    /// this is the entire content of a stored orbit's array section,
    /// which is what turns a 202 MB f3 file into a couple of MB.
    corrections: Vec<Correction>,
    /// Live DD shadow (re_hi, re_lo, im_hi, im_lo), tracking the
    /// fixed-point iterate. NaN = poisoned (a truncation or an
    /// out-of-range dip): every subsequent entry then records a
    /// correction, which is always safe.
    shadow: [f64; 4],
    /// DD of c, derived from the fixed-point c (recomputed on load).
    c_dd: [f64; 4],
    /// Iteration at which the REFERENCE escaped (|Z|² > 4), if it did.
    /// Pixels needing more iterations rebase (wrap to index 0), so a
    /// short orbit is fine — it just stops growing.
    pub escaped_at: Option<u32>,
    /// Live fixed-point state (c and current Z) for append-on-deepen.
    c: FixedComplex,
    z: FixedComplex,
    /// The PREVIOUS iterate, for two-term recurrences (Phoenix). Zero
    /// and unread for every other family; carried in the live state
    /// so a reloaded orbit deepens identically.
    z_prev: FixedComplex,
    /// Phoenix's `p`. Part of the orbit's IDENTITY:
    /// a different p is a different orbit, so it is hashed into the
    /// cache key and compared by `serves`.
    pub map_params: [f32; 2],
    p_fixed: FixedComplex,
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
    /// A period hint that was TRIED on this center and turned out too
    /// shallow to wrap at the view it was built for, with the |Z_p|
    /// octave it actually closed at.
    ///
    /// Without this, a request carrying that hint cannot recognise
    /// THIS orbit as its answer: the cache key hashes the center, not
    /// the period, so a stored plain reference gets rejected and a
    /// perfectly good multi-minute computation is repeated. Keeping
    /// the octave rather than a bare "rejected" flag makes the
    /// decision zoom-aware - the same hint may be too shallow here
    /// and exactly right two hundred octaves out.
    pub hint_period: Option<u32>,
    pub hint_octave: i64,
}

/// Octave limit for accepting a closure at a zoom: 16 octaves below
/// pixel scale (margin), and never looser than f32 visibility.
pub fn closure_limit_for_zoom(zoom_log2: f64) -> i64 {
    (-(zoom_log2 + 16.0) as i64).min(-24)
}

/// Double-double (DD) arithmetic for the compression shadow: ~106-bit
/// working precision from f64 pairs, IEEE-exact building blocks
/// (TwoSum / TwoProd-via-FMA), so the shadow replays IDENTICALLY on
/// every platform -- `f64::mul_add` is a single correctly-rounded
/// operation everywhere (hardware FMA or libm; wasm lowers to libm).
/// That determinism is what lets a saved file store only the shadow's
/// CORRECTIONS and regenerate the orbit arrays bit-for-bit on load.
mod dd {
    #[inline]
    pub fn two_sum(a: f64, b: f64) -> (f64, f64) {
        let s = a + b;
        let bb = s - a;
        (s, (a - (s - bb)) + (b - bb))
    }

    #[inline]
    pub fn add(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
        let (sh, sl) = two_sum(a.0, b.0);
        two_sum(sh, sl + a.1 + b.1)
    }

    #[inline]
    pub fn mul(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
        let p = a.0 * b.0;
        let e = f64::mul_add(a.0, b.0, -p);
        two_sum(p, e + a.0 * b.1 + a.1 * b.0)
    }

    #[inline]
    pub fn neg(a: (f64, f64)) -> (f64, f64) {
        (-a.0, -a.1)
    }

    /// The DD value exported the way `FixedPoint::to_f64` exports:
    /// TRUNCATED toward zero at 53 bits (to_floatexp collects the top
    /// 53 bits without rounding). Round-to-nearest first, then step
    /// one ulp toward zero when the discarded tail says nearest
    /// rounded away from zero.
    #[inline]
    pub fn trunc_f64(a: (f64, f64)) -> f64 {
        let (r, t) = two_sum(a.0, a.1);
        if r == 0.0 || !r.is_finite() || t == 0.0 {
            return r;
        }
        if (r > 0.0 && t < 0.0) || (r < 0.0 && t > 0.0) {
            // Exact magnitude sits below |r|: truncation is the next
            // representable toward zero (bit trick is sign-magnitude
            // safe: subtracting one from the bits of a nonzero f64
            // shrinks its magnitude for either sign).
            return f64::from_bits(r.to_bits() - 1);
        }
        r
    }

    #[inline]
    pub fn is_neg(a: (f64, f64)) -> bool {
        a.0 < 0.0 || (a.0 == 0.0 && a.1 < 0.0)
    }

    #[inline]
    pub fn abs(a: (f64, f64)) -> (f64, f64) {
        if is_neg(a) { neg(a) } else { a }
    }
}

/// One compression correction: at `idx` the shadow was reset to the
/// full-precision orbit value (`dd`, natural scale, degenerate when
/// the value is below f64 range -- the following entries then force
/// their own corrections, which is exactly right through deep dips),
/// and the entry's stored triple is carried verbatim (the replay
/// cannot derive it from a value the shadow could not represent).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Correction {
    pub idx: u32,
    pub hi: [f32; 2],
    pub lo: [f32; 2],
    pub e: i32,
    /// Shadow reset value: (re_hi, re_lo, im_hi, im_lo).
    pub dd: [f64; 4],
}

/// MSB exponent of an f64 (floor(log2 |v|)), from the bits -- exact,
/// matching `FixedPoint::to_floatexp`'s exponent convention.
fn f64_msb_exp(v: f64) -> Option<i64> {
    if v == 0.0 || !v.is_finite() {
        return None;
    }
    let bits = v.abs().to_bits();
    let be = (bits >> 52) & 0x7ff;
    if be != 0 {
        Some(be as i64 - 1023)
    } else {
        let m = bits & ((1u64 << 52) - 1);
        Some(63 - m.leading_zeros() as i64 - 1074)
    }
}

/// The shadow's version of [`split_entry`]: decompose a DD iterate
/// into the (hi, lo, exponent) storage form using the SAME branch
/// structure. Compute and replay both call exactly this; whenever it
/// would disagree with the true fixed-point decomposition, compute
/// emits a correction instead -- so the replayed arrays are
/// byte-identical BY CONSTRUCTION, not by tolerance.
fn shadow_split(sx: (f64, f64), sy: (f64, f64)) -> ([f32; 2], [f32; 2], i32) {
    let x = dd::trunc_f64(sx);
    let y = dd::trunc_f64(sy);
    const DEEP: f64 = 8.077_935_669_463_16e-28; // 2^-90, as split_entry
    if x.abs().max(y.abs()) >= DEEP {
        let hix = x as f32;
        let hiy = y as f32;
        return (
            [hix, hiy],
            [(x - hix as f64) as f32, (y - hiy as f64) as f32],
            0,
        );
    }
    let e = match (f64_msb_exp(x), f64_msb_exp(y)) {
        (None, None) => return ([0.0, 0.0], [0.0, 0.0], 0),
        (None, Some(e)) => e,
        (Some(e), None) => e,
        (Some(a), Some(b)) => a.max(b),
    };
    let scale = (-(e as f64)).exp2();
    let mx = if x == 0.0 { 0.0 } else { x * scale };
    let my = if y == 0.0 { 0.0 } else { y * scale };
    let hix = mx as f32;
    let hiy = my as f32;
    (
        [hix, hiy],
        [(mx - hix as f64) as f32, (my - hiy as f64) as f32],
        e.clamp(-2_000_000_000, 2_000_000_000) as i32,
    )
}

/// One shadow iteration step, mirroring the fixed-point step: z^p + c
/// with square-and-multiply, or the Ship-family fold. Compute and
/// replay share it (the determinism contract).
fn shadow_step(
    sx: (f64, f64),
    sy: (f64, f64),
    c: &[f64; 4],
    power: u32,
    ship: bool,
    ship_variant: u32,
) -> ((f64, f64), (f64, f64)) {
    let cx = (c[0], c[1]);
    let cy = (c[2], c[3]);
    if ship {
        let x_neg = dd::is_neg(sx);
        let y_neg = dd::is_neg(sy);
        // (x^2 - y^2, 2xy), then the variant's sign/abs rearrangement
        // (mirrors the sign-magnitude folds in extend()).
        let re = dd::add(dd::mul(sx, sx), dd::neg(dd::mul(sy, sy)));
        let im = dd::mul(sx, sy); // xy; doubled below
        let im = (2.0 * im.0, 2.0 * im.1);
        let (re, im) = match ship_variant {
            0 => (re, dd::abs(im)),
            1 => (re, if y_neg { dd::abs(im) } else { dd::neg(dd::abs(im)) }),
            2 => (re, if x_neg { dd::abs(im) } else { dd::neg(dd::abs(im)) }),
            3 => (dd::abs(re), im),
            4 => (dd::abs(re), dd::neg(dd::abs(im))),
            _ => (dd::abs(re), if y_neg { dd::abs(im) } else { dd::neg(dd::abs(im)) }),
        };
        (dd::add(re, cx), dd::add(im, cy))
    } else {
        // z^2, then square-and-multiply for higher powers.
        let mut zx = dd::add(dd::mul(sx, sx), dd::neg(dd::mul(sy, sy)));
        let xy = dd::mul(sx, sy);
        let mut zy = (2.0 * xy.0, 2.0 * xy.1);
        for _ in 2..power {
            let nx = dd::add(dd::mul(zx, sx), dd::neg(dd::mul(zy, sy)));
            let ny = dd::add(dd::mul(zx, sy), dd::mul(zy, sx));
            zx = nx;
            zy = ny;
        }
        (dd::add(zx, cx), dd::add(zy, cy))
    }
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
        map_params: [f32; 2],
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
            z_prev: FixedComplex::zero(n),
            map_params,
            p_fixed: FixedComplex {
                re: FixedPoint::from_f64(map_params[0] as f64, n),
                im: FixedPoint::from_f64(map_params[1] as f64, n),
            },
            orbit: vec![first],
            orbit_lo: vec![[0.0, 0.0]],
            orbit_e: vec![0],
            escaped_at: None,
            z: z0,
            c,
            min_octave: i64::MAX,
            min_at: 0,
            closure_limit_octave: closure_limit_for_zoom(zoom_log2),
            closure_octave: i64::MAX,
            hint_period: None,
            hint_octave: i64::MAX,
            corrections: Vec::new(),
            shadow: [f64::NAN; 4],
            c_dd: [0.0; 4],
        };
        orbit.init_shadow();
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

    /// DD extraction of a fixed-point value: hi = round-to-f64, lo =
    /// round-to-f64 of the exact remainder. ~2^-105 total accuracy in
    /// range; degenerate below f64 range (callers treat that as a
    /// poisoned shadow).
    fn fixed_dd(v: &super::fixedpoint::FixedPoint, n: usize) -> (f64, f64) {
        let hi = v.to_f64();
        if hi == 0.0 || !hi.is_finite() {
            return (hi, 0.0);
        }
        let lo = v.sub(&super::fixedpoint::FixedPoint::from_f64(hi, n)).to_f64();
        (hi, lo)
    }

    /// Seed the shadow + corrections from the CURRENT state (entry 0
    /// is always a correction; c_dd derives from the fixed c).
    fn init_shadow(&mut self) {
        let n = self.n_limbs;
        let (cx, cxl) = Self::fixed_dd(&self.c.re, n);
        let (cy, cyl) = Self::fixed_dd(&self.c.im, n);
        self.c_dd = [cx, cxl, cy, cyl];
        let (zx, zxl) = Self::fixed_dd(&self.z.re, n);
        let (zy, zyl) = Self::fixed_dd(&self.z.im, n);
        self.shadow = [zx, zxl, zy, zyl];
        self.corrections = vec![Correction {
            idx: 0,
            hi: self.orbit[0],
            lo: self.orbit_lo[0],
            e: self.orbit_e[0],
            dd: self.shadow,
        }];
    }

    /// Push the just-computed iterate's storage triple, advancing the
    /// compression shadow: the triple always comes from the TRUE
    /// fixed-point value (renders are untouched by compression); a
    /// correction is recorded whenever the shadow's decomposition
    /// would differ -- so replay-on-load reproduces the arrays
    /// byte-for-byte, with no tolerance in the loop.
    fn push_entry(&mut self, x: f64, y: f64) {
        let (hi, lo, ee) = split_entry(&self.z, x, y);
        let idx = self.orbit.len() as u32;
        let (sx, sy) = shadow_step(
            (self.shadow[0], self.shadow[1]),
            (self.shadow[2], self.shadow[3]),
            &self.c_dd,
            self.power,
            self.ship,
            self.ship_variant,
        );
        let ok = sx.0.is_finite() && sy.0.is_finite() && {
            let (shi, slo, se) = shadow_split(sx, sy);
            shi == hi && slo == lo && se == ee
        };
        if ok {
            self.shadow = [sx.0, sx.1, sy.0, sy.1];
        } else {
            let n = self.n_limbs;
            let (zx, zxl) = Self::fixed_dd(&self.z.re, n);
            let (zy, zyl) = Self::fixed_dd(&self.z.im, n);
            self.shadow = [zx, zxl, zy, zyl];
            self.corrections.push(Correction { idx, hi, lo, e: ee, dd: self.shadow });
        }
        self.orbit.push(hi);
        self.orbit_lo.push(lo);
        self.orbit_e.push(ee);
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
                // z^p, or conj(z)^p for the Tricorn family. See
                // MAP_CONJ: with `ship` false, `ship_variant` selects
                // which non-fold map this is.
                if self.ship_variant == MAP_PHOENIX {
                    // z' = z^2 + c + p*z_prev, and z_prev advances to
                    // the iterate we just left.
                    let sq = self.z.sqr();
                    let term = self.p_fixed.mul(&self.z_prev);
                    let next = sq.add(&self.c).add(&term);
                    self.z_prev = std::mem::replace(&mut self.z, next);
                    let x = self.z.re.to_f64();
                    let y = self.z.im.to_f64();
                    self.push_entry(x, y);
                    if x * x + y * y > 4.0 {
                        self.escaped_at = Some(self.orbit.len() as u32 - 1);
                        break;
                    }
                    continue;
                }
                let mut base = FixedComplex {
                    re: self.z.re.clone(),
                    im: self.z.im.clone(),
                };
                if self.ship_variant == MAP_CONJ {
                    // conj: flip the sign bit. Zero stays canonical
                    // (a zero magnitude is non-negative by contract).
                    base.im.neg = !base.im.neg && !base.im.limbs.iter().all(|l| *l == 0);
                }
                let mut zp = base.sqr();
                for _ in 2..self.power {
                    zp = zp.mul(&base);
                }
                self.z = zp.add(&self.c);
            }
            let x = self.z.re.to_f64();
            let y = self.z.im.to_f64();
            // Progressive period detection (parameter-plane power
            // tiers): a new |Z| minimum is a ball-method period
            // candidate; below f32 visibility the orbit has PROVEN
            // its period — become the periodic reference on the spot.
            if self.julia_c.is_none() && !self.ship && self.ship_variant == MAP_PLAIN {
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
                        self.push_entry(x, y);
                        self.periodic = Some(idx);
                        self.closure_octave = oct;
                        log::info!(
                            "auto-detected periodic reference: period {idx} (|Z| ~ 2^{oct})"
                        );
                        return;
                    }
                }
            }
            self.push_entry(x, y);
            if x * x + y * y > 4.0 {
                self.escaped_at = Some(self.orbit.len() as u32 - 1);
                break;
            }
        }
    }

    /// The iterate's plain f32 value (deep entries read as zero,
    /// exactly as pre-exponent f32 storage did).
    pub fn z_f32(&self, i: usize) -> [f32; 2] {
        entry_value(self.orbit[i], self.orbit_e.get(i).copied().unwrap_or(0))
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
        map_params: [f32; 2],
    ) -> bool {
        self.center_re == center_re
            && self.center_im == center_im
            && self.n_limbs >= n_limbs
            && self.julia_c == julia_c
            && self.power == power.max(2)
            && self.ship == ship
            && self.ship_variant == ship_variant.min(5)
            && self.map_params == map_params
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
        // Fixed prefix: the store's staleness probe reads exactly this
        // much (see orbit_store::saved_meta) to decide whether a
        // rewrite would ADD anything, without parsing the whole file.
        out.extend_from_slice(&self.hint_period.unwrap_or(0).to_le_bytes());
        out.extend_from_slice(&self.hint_octave.to_le_bytes());
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
        out.extend_from_slice(&self.map_params[0].to_le_bytes());
        out.extend_from_slice(&self.map_params[1].to_le_bytes());
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
        // FFORBIT6 array section, tagged by a mode byte:
        //   1 = COMPRESSED -- shadow corrections + an RLE exponent
        //       stream; the hi/lo arrays are NOT stored, load replays
        //       the DD shadow between corrections and regenerates
        //       them byte-for-byte (measured: a chaotic 20k orbit is
        //       50x smaller; the f3 orbit ~2.5 MB from 202 MB).
        //   0 = RAW -- the FFORBIT5-style arrays. Chosen when the
        //       corrections would be LARGER than the arrays: an orbit
        //       dipping near zero every few iterations (a periodic
        //       nucleus at a cascade -- exactly the orbits the store
        //       marks precious) records a 52 B correction per entry,
        //       2.6x worse than 20 B/iteration raw. Whichever is
        //       smaller wins; both are byte-exact.
        let mut runs: Vec<(u32, i32)> = Vec::new();
        for &e in &self.orbit_e {
            match runs.last_mut() {
                Some((n, v)) if *v == e => *n += 1,
                _ => runs.push((1, e)),
            }
        }
        let compressed_bytes = 4 + self.corrections.len() * 52 + 4 + runs.len() * 8;
        let raw_bytes = self.orbit.len() * 20;
        if compressed_bytes < raw_bytes {
            out.push(1);
            out.extend_from_slice(&(self.corrections.len() as u32).to_le_bytes());
            for c in &self.corrections {
                out.extend_from_slice(&c.idx.to_le_bytes());
                out.extend_from_slice(&c.hi[0].to_le_bytes());
                out.extend_from_slice(&c.hi[1].to_le_bytes());
                out.extend_from_slice(&c.lo[0].to_le_bytes());
                out.extend_from_slice(&c.lo[1].to_le_bytes());
                out.extend_from_slice(&c.e.to_le_bytes());
                for d in c.dd {
                    out.extend_from_slice(&d.to_le_bytes());
                }
            }
            // e-stream RLE: (count, value) runs -- e is zero away
            // from deep dips, so this is a handful of runs.
            out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
            for (n, v) in runs {
                out.extend_from_slice(&n.to_le_bytes());
                out.extend_from_slice(&v.to_le_bytes());
            }
        } else {
            out.push(0);
            for z in &self.orbit {
                out.extend_from_slice(&z[0].to_le_bytes());
                out.extend_from_slice(&z[1].to_le_bytes());
            }
            for z in &self.orbit_lo {
                out.extend_from_slice(&z[0].to_le_bytes());
                out.extend_from_slice(&z[1].to_le_bytes());
            }
            for e in &self.orbit_e {
                out.extend_from_slice(&e.to_le_bytes());
            }
        }
        for f in [
            &self.z.re,
            &self.z.im,
            &self.c.re,
            &self.c.im,
            &self.z_prev.re,
            &self.z_prev.im,
        ] {
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
        let hint_period = match r.u32()? {
            0 => None,
            p => Some(p),
        };
        let hint_octave = i64::from_le_bytes(r.take(8)?.try_into().ok()?);
        let center_re = r.string()?;
        let center_im = r.string()?;
        let julia_c = match r.u8()? {
            0 => None,
            _ => Some((r.f32()?, r.f32()?)),
        };
        let power = r.u32()?;
        let ship = r.u8()? != 0;
        let ship_variant = r.u32()?;
        let map_params = [r.f32()?, r.f32()?];
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
        let mode = r.u8()?;
        if mode == 0 {
            // RAW mode: FFORBIT5-style arrays (the dip-dense case).
            let mut orbit = Vec::with_capacity(orbit_len);
            for _ in 0..orbit_len {
                orbit.push([r.f32()?, r.f32()?]);
            }
            let mut orbit_lo = Vec::with_capacity(orbit_len);
            for _ in 0..orbit_len {
                orbit_lo.push([r.f32()?, r.f32()?]);
            }
            let mut orbit_e = Vec::with_capacity(orbit_len);
            for _ in 0..orbit_len {
                orbit_e.push(r.u32()? as i32);
            }
            let z = FixedComplex { re: r.fixed(n_limbs)?, im: r.fixed(n_limbs)? };
            let c = FixedComplex { re: r.fixed(n_limbs)?, im: r.fixed(n_limbs)? };
            let c_dd = {
                let (cx, cxl) = Self::fixed_dd(&c.re, n_limbs);
                let (cy, cyl) = Self::fixed_dd(&c.im, n_limbs);
                [cx, cxl, cy, cyl]
            };
            // A poisoned shadow makes any future extend record plain
            // corrections -- always safe, and this orbit's shape is
            // correction-dense anyway (that is why it is raw).
            let seed = Correction {
                idx: 0,
                hi: orbit[0],
                lo: orbit_lo[0],
                e: orbit_e[0],
                dd: [f64::NAN; 4],
            };
            return Some(
                Self {
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
                    orbit_e,
                    escaped_at,
                    z,
                    c,
                    min_octave: i64::MAX,
                    min_at: 0,
                    closure_limit_octave: closure_limit_for_zoom(off_zoom),
                    closure_octave,
                    hint_period,
                    hint_octave,
                    z_prev: FixedComplex::zero(n_limbs),
                    map_params,
                    p_fixed: FixedComplex {
                        re: FixedPoint::from_f64(map_params[0] as f64, n_limbs),
                        im: FixedPoint::from_f64(map_params[1] as f64, n_limbs),
                    },
                    corrections: vec![seed],
                    shadow: [f64::NAN; 4],
                    c_dd,
                }
                .with_rescanned_min(),
            );
        }
        if mode != 1 {
            return None;
        }
        // COMPRESSED mode: corrections, then the RLE exponent stream,
        // then REPLAY the DD shadow to regenerate the hi/lo arrays.
        // `shadow_step` and `shadow_split` are the same functions
        // compute() ran, and compute() emitted a correction at every
        // index where they would disagree with the true decomposition
        // -- so this reproduces the arrays byte-for-byte.
        let n_corr = r.u32()? as usize;
        if n_corr == 0 || n_corr > orbit_len {
            return None;
        }
        let mut corrections = Vec::with_capacity(n_corr);
        for _ in 0..n_corr {
            corrections.push(Correction {
                idx: r.u32()?,
                hi: [r.f32()?, r.f32()?],
                lo: [r.f32()?, r.f32()?],
                e: r.u32()? as i32,
                dd: [r.f64()?, r.f64()?, r.f64()?, r.f64()?],
            });
        }
        if corrections[0].idx != 0 {
            return None;
        }
        let n_runs = r.u32()? as usize;
        if n_runs > orbit_len {
            return None;
        }
        let mut orbit_e = Vec::with_capacity(orbit_len);
        for _ in 0..n_runs {
            let count = r.u32()? as usize;
            let v = r.u32()? as i32;
            if orbit_e.len() + count > orbit_len {
                return None;
            }
            orbit_e.extend(std::iter::repeat(v).take(count));
        }
        if orbit_e.len() != orbit_len {
            return None;
        }
        let z = FixedComplex { re: r.fixed(n_limbs)?, im: r.fixed(n_limbs)? };
        let c = FixedComplex { re: r.fixed(n_limbs)?, im: r.fixed(n_limbs)? };
        let z_prev = FixedComplex { re: r.fixed(n_limbs)?, im: r.fixed(n_limbs)? };
        let c_dd = {
            let (cx, cxl) = Self::fixed_dd(&c.re, n_limbs);
            let (cy, cyl) = Self::fixed_dd(&c.im, n_limbs);
            [cx, cxl, cy, cyl]
        };
        let mut orbit = Vec::with_capacity(orbit_len);
        let mut orbit_lo = Vec::with_capacity(orbit_len);
        let mut shadow = [f64::NAN; 4];
        let mut next_corr = 0usize;
        for i in 0..orbit_len {
            if next_corr < corrections.len() && corrections[next_corr].idx as usize == i {
                let cc = &corrections[next_corr];
                if cc.e != orbit_e[i] {
                    return None;
                }
                orbit.push(cc.hi);
                orbit_lo.push(cc.lo);
                shadow = cc.dd;
                next_corr += 1;
            } else {
                if i == 0 {
                    return None; // entry 0 must be a correction
                }
                let (sx, sy) = shadow_step(
                    (shadow[0], shadow[1]),
                    (shadow[2], shadow[3]),
                    &c_dd,
                    power,
                    ship,
                    ship_variant,
                );
                shadow = [sx.0, sx.1, sy.0, sy.1];
                let (hi, lo, ee) = shadow_split(sx, sy);
                if ee != orbit_e[i] {
                    return None;
                }
                orbit.push(hi);
                orbit_lo.push(lo);
            }
        }
        if next_corr != corrections.len() {
            return None;
        }
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
            orbit_e,
            escaped_at,
            z,
            c,
            min_octave: i64::MAX,
            min_at: 0,
            closure_limit_octave: closure_limit_for_zoom(off_zoom),
            closure_octave,
            hint_period,
            hint_octave,
            z_prev,
            map_params,
            p_fixed: FixedComplex {
                re: FixedPoint::from_f64(map_params[0] as f64, n_limbs),
                im: FixedPoint::from_f64(map_params[1] as f64, n_limbs),
            },
            corrections,
            shadow,
            c_dd,
        }
        .with_rescanned_min())
    }

    /// Rebuild the minimum tracker from the stored f32 orbit (loads).
    /// f32 floors at ~2^-149; deeper minima re-emerge from the live
    /// fixed-point state as the orbit extends.
    #[cfg(not(target_arch = "wasm32"))]
    fn with_rescanned_min(mut self) -> Self {
        for i in 1..self.orbit.len() {
            let z = self.z_f32(i);
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

    /// Whether this orbit answers a request carrying `hint`.
    ///
    /// A periodic orbit of that period obviously does. So does a
    /// PLAIN orbit that already tried the hint and measured it too
    /// shallow for this zoom — that is the right reference for the
    /// request, and rebuilding it would only rediscover the same
    /// fact at the same cost.
    pub fn answers_hint(&self, hint: Option<u32>, zoom_log2: f64) -> bool {
        match hint {
            None => true,
            Some(p) => {
                self.periodic == Some(p)
                    || (self.hint_period == Some(p)
                        && self.hint_octave > closure_limit_for_zoom(zoom_log2))
            }
        }
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
    /// Continuous map parameter (Phoenix's p); zero elsewhere. Part
    /// of the orbit identity, so it takes part in request dedup.
    pub map_params: [f32; 2],
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
    /// Which ORBIT this data is (bumped only when the orbit content
    /// is replaced -- a fresh compute, a truncation republish --
    /// never when a request merely reuses the same orbit). The
    /// render side keys its GPU mirror on this, so a zoom tick that
    /// reuses the orbit costs no re-upload; appends under an
    /// unchanged generation extend the mirror in place.
    pub generation: u64,
    /// Z_n snapshots so far (orbit[0] = the seed).
    pub orbit: Vec<[f32; 2]>,
    /// DF residuals, parallel to `orbit`.
    pub orbit_lo: Vec<[f32; 2]>,
    /// Per-entry exponents, parallel to `orbit` (see
    /// [`ReferenceOrbit::orbit_e`]).
    pub orbit_e: Vec<i32>,
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
                            // n_limbs: >= not == -- an orbit at higher
                            // precision serves a shallower request (the
                            // blocking cache's `serves` agrees), and
                            // equality rebuilt a full-depth orbit at
                            // every limb crossing of a zoom-out.
                            let same = old_req.center_re == req.center_re
                                && old_req.center_im == req.center_im
                                && old_req.n_limbs >= req.n_limbs
                                && old_req.julia_c == req.julia_c
                                && old_req.power == req.power
                                && old_req.ship == req.ship
                                && old_req.ship_variant == req.ship_variant
                                && old_req.reference_period == req.reference_period
                                && old_req.map_params == req.map_params
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
                        let (orbit, reused) = match reuse {
                            Some(o) => (o, true),
                            None => {
                                match worker_compute_orbit(&req) {
                                    Some(o) => (o, false),
                                    None => {
                                        // Unparseable center: publish an
                                        // empty done state (new -- empty
                                        // -- content: bump generation).
                                        let mut p = shared.lock().unwrap();
                                        p.epoch = epoch;
                                        p.generation = p.generation.wrapping_add(1);
                                        p.orbit.clear();
                                        p.orbit_lo.clear();
                                        p.orbit_e.clear();
                                        p.done = true;
                                        continue;
                                    }
                                }
                            }
                        };
                        // Publish. A REUSED orbit already mirrors the
                        // shared progress exactly (every mutation to it
                        // is followed by a publish under the lock), so
                        // only the request-scoped metadata changes --
                        // cloning a 10M-entry orbit here on every zoom
                        // tick was hundreds of MB of memcpy per wheel
                        // notch, for content that had not changed. A
                        // fresh orbit is new content: bump the
                        // generation and clone.
                        {
                            let mut p = shared.lock().unwrap();
                            p.epoch = epoch;
                            if !reused {
                                p.generation = p.generation.wrapping_add(1);
                                p.orbit.clear();
                                p.orbit.extend_from_slice(&orbit.orbit);
                                p.orbit_lo.clear();
                                p.orbit_lo.extend_from_slice(&orbit.orbit_lo);
                                p.orbit_e.clear();
                                p.orbit_e.extend_from_slice(&orbit.orbit_e);
                            }
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
                                    p.orbit_e.extend_from_slice(&orbit.orbit_e[have..]);
                                } else {
                                    // Auto-closure truncated the orbit
                                    // to one period: republish whole,
                                    // as NEW content (the mirror must
                                    // not append onto the longer old
                                    // prefix).
                                    p.generation = p.generation.wrapping_add(1);
                                    p.orbit.clear();
                                    p.orbit.extend_from_slice(&orbit.orbit);
                                    p.orbit_lo.clear();
                                    p.orbit_lo.extend_from_slice(&orbit.orbit_lo);
                                    p.orbit_e.clear();
                                    p.orbit_e.extend_from_slice(&orbit.orbit_e);
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
        // Same acceptance filter as the blocking cache
        // (`OrbitCache::get`): the orbit must answer the request's
        // hint AND its periodicity must serve this zoom. Accepting
        // any stored orbit when no hint is set let a periodic orbit
        // whose closure cannot serve the zoom through -- its wrap is
        // inexact at that depth and renders a displaced structure.
        if o.answers_hint(req.reference_period, req.zoom_log2)
            && o.periodic_serves(req.zoom_log2)
        {
            return Some(o);
        }
    }
    // Nucleus relocation is derived for the plain power map (Newton on
    // f_c^p(0), ball-method periods, the closure test): the
    // anti-holomorphic family needs its own derivation, so it takes
    // the plain reference path. It ALSO has to, mechanically -- a
    // nucleus orbit is built with ship_variant 0, which a MAP_CONJ
    // request can never be served by, so routing it here made the
    // cache rebuild every frame and the render never settle.
    if req.julia_c.is_none() && !req.ship && req.ship_variant == MAP_PLAIN {
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
            req.map_params,
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
            && old_req.n_limbs >= req.n_limbs
            && old_req.julia_c == req.julia_c
            && old_req.power == req.power
                                && old_req.ship == req.ship
                                && old_req.ship_variant == req.ship_variant
                                && old_req.reference_period == req.reference_period
                                && old_req.map_params == req.map_params
                                && orbit.periodic_serves(req.zoom_log2);
        if same && orbit.relocation_serves(req.zoom_log2, req.height_px.max(1.0)) {
            Some(orbit)
        } else {
            None
        }
    });
    let (orbit, reused) = match reuse {
        Some(o) => (o, true),
        None => (worker_compute_orbit(&req)?, false),
    };
    {
        let mut p = shared.lock().unwrap();
        p.epoch = epoch;
        if !reused {
            // See the main loop: a reused orbit already mirrors the
            // shared progress; only fresh content is cloned.
            p.generation = p.generation.wrapping_add(1);
            p.orbit.clear();
            p.orbit.extend_from_slice(&orbit.orbit);
            p.orbit_lo.clear();
            p.orbit_lo.extend_from_slice(&orbit.orbit_lo);
            p.orbit_e.clear();
            p.orbit_e.extend_from_slice(&orbit.orbit_e);
        }
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
            center_re, center_im, zoom_log2, Some(n), 0, None, power, false, 0, [0.0, 0.0],
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
        orbit.orbit_e.truncate(period as usize + 1);
        // Compression state past the truncation is void; a periodic
        // orbit never extends, and a poisoned shadow would record
        // corrections if it somehow did (always safe).
        orbit.corrections.retain(|c| (c.idx as usize) < period as usize + 1);
        orbit.shadow = [f64::NAN; 4];
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
                if let Some(mut orbit) =
                    Self::try_periodic_from_hint(center_re, center_im, zoom_log2, p, power)
                {
                    if orbit.periodic_serves(zoom_log2) {
                        return Some(orbit);
                    }
                    // The hinted period CLOSES, but not below this
                    // view's pixel scale — the center is not the
                    // nucleus to enough digits for the wrap to be
                    // exact here. Returning it anyway is what made
                    // extreme zooms spin: the cache rejects a
                    // non-serving orbit on the very next frame, so
                    // every frame paid a full reference computation
                    // and nothing ever rendered (measured at
                    // zoom 9316: one rebuild every 54 s, forever).
                    //
                    // Keep the work instead. The orbit already IS the
                    // plain prefix 0..=p and the live fixed-point
                    // state continues it, so dropping the periodicity
                    // and extending costs nothing extra — and
                    // ordinary auto-detection can still close it at a
                    // depth that does serve.
                    log::warn!(
                        "reference period {p} does not serve zoom {zoom_log2:.0}: \
                         |Z_p| ~ 2^{} vs pixel-scale limit 2^{} — continuing as a plain \
                         reference (refine the center toward the nucleus for an exact wrap)",
                        orbit.closure_octave,
                        closure_limit_for_zoom(zoom_log2),
                    );
                    orbit.hint_period = Some(p);
                    orbit.hint_octave = orbit.closure_octave;
                    orbit.periodic = None;
                    orbit.closure_octave = i64::MAX;
                    orbit.set_closure_limit(zoom_log2);
                    orbit.extend(max_iter);
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
                [0.0, 0.0],
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
            [0.0, 0.0],
        )
    }
}

/// Single-slot orbit cache: during a continuous zoom the center is
/// unchanged, so one orbit serves every frame; deepening appends. A
/// pan or precision change replaces the slot.
#[derive(Default)]
pub struct OrbitCache {
    slot: Option<ReferenceOrbit>,
    /// Bumped whenever the SLOT CONTENT is replaced (never on a pure
    /// extend). The renderer keys its GPU mirror and BLA table on
    /// this: two different orbits can have the same length (pan at a
    /// fixed max_iter -- both non-escaping orbits are max_iter+1
    /// long), so a length compare alone would leave a stale
    /// reference bound.
    generation: u64,
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

    /// Identity of the current slot content (see the field doc).
    pub fn generation(&self) -> u64 {
        self.generation
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
        map_params: [f32; 2],
    ) -> Option<&ReferenceOrbit> {
        // The center's own digits set a precision FLOOR: a truncated
        // deep center is a different (shallow, early-escaping) point,
        // and pixels that can't outgrow d0 before that escape collapse
        // onto it (the zoom-685 uniform-frame bug).
        let n = super::fixedpoint::limbs_for_view(center_re, center_im, zoom_log2);
        let hit = self.slot.as_ref().is_some_and(|o| {
            o.serves(center_re, center_im, n, julia_c, power, ship, ship_variant, map_params)
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
                o.answers_hint(self.reference_period, zoom_log2) && o.periodic_serves(zoom_log2)
            });
            let orbit = if let Some(mut o) = loaded {
                o.extend(max_iter);
                o
            } else if julia_c.is_none() && !ship && ship_variant == MAP_PLAIN {
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
                    ship_variant, map_params,
                )?
            };
            self.generation = self.generation.wrapping_add(1);
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
        map_params: [f32; 2],
        budget: u32,
    ) -> Option<(&ReferenceOrbit, bool)> {
        let n = super::fixedpoint::limbs_for_view(center_re, center_im, zoom_log2);
        let hit = self.slot.as_ref().is_some_and(|o| {
            o.serves(center_re, center_im, n, julia_c, power, ship, ship_variant, map_params)
                && o.periodic_serves(zoom_log2)
        });
        let budget = budget.max(64);
        if hit {
            let orbit = self.slot.as_mut().unwrap();
            orbit.set_closure_limit(zoom_log2);
            let target = orbit.len().saturating_sub(1).saturating_add(budget).min(max_iter);
            orbit.extend(target);
        } else {
            self.generation = self.generation.wrapping_add(1);
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
                map_params,
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
        let orbit = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 100, None, 2, false, 0, [0.0, 0.0]).unwrap();
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
        let orbit = ReferenceOrbit::compute("1", "1", 5.0, None, 1000, None, 2, false, 0, [0.0, 0.0]).unwrap();
        let at = orbit.escaped_at.expect("c = 1+i escapes fast");
        assert!(at < 10);
        assert_eq!(orbit.len() - 1, at);
    }

    /// Phoenix's reference is a TWO-TERM recurrence, and its
    /// parameter is part of the orbit's identity.
    ///
    /// Both halves matter: an orbit that ignored z_prev would be the
    /// plain quadratic, and one cached without `p` would be silently
    /// reused after the user changed it -- the second is the failure
    /// that cannot be seen in a single render, only in the next one.
    #[test]
    fn phoenix_reference_carries_history_and_its_parameter() {
        let p = [-0.5f32, 0.1];
        let orbit = ReferenceOrbit::compute(
            "-0.2", "0.35", 20.0, None, 150, None, 2, false, MAP_PHOENIX, p,
        )
        .expect("orbit");
        // f64 shadow of z' = z^2 + c + p*z_prev.
        let (cx, cy) = (-0.2f64, 0.35f64);
        let (mut zx, mut zy) = (0.0f64, 0.0f64);
        let (mut px, mut py) = (0.0f64, 0.0f64);
        let mut worst = 0.0f64;
        for i in 1..(orbit.len() as usize).min(150) {
            let (nx, ny) = (
                zx * zx - zy * zy + cx + (p[0] as f64) * px - (p[1] as f64) * py,
                2.0 * zx * zy + cy + (p[0] as f64) * py + (p[1] as f64) * px,
            );
            px = zx;
            py = zy;
            zx = nx;
            zy = ny;
            let got = orbit.z_f32(i);
            worst = worst.max(
                ((got[0] as f64 - zx).powi(2) + (got[1] as f64 - zy).powi(2)).sqrt(),
            );
        }
        assert!(worst < 1e-4, "phoenix reference diverges from z^2 + c + p*z_prev: {worst:e}");

        // A different p is a DIFFERENT orbit, and `serves` must say so
        // -- otherwise the cache hands back the wrong reference.
        let other = ReferenceOrbit::compute(
            "-0.2", "0.35", 20.0, None, 150, None, 2, false, MAP_PHOENIX, [0.25, 0.1],
        )
        .expect("orbit");
        let n = orbit.n_limbs;
        assert!(
            orbit.serves("-0.2", "0.35", n, None, 2, false, MAP_PHOENIX, p),
            "an orbit must serve its own identity"
        );
        assert!(
            !orbit.serves("-0.2", "0.35", n, None, 2, false, MAP_PHOENIX, [0.25, 0.1]),
            "a different p must MISS, or the cache returns a stale reference"
        );
        let d = (1..(other.len() as usize).min(150))
            .map(|i| {
                let (a, b) = (orbit.z_f32(i), other.z_f32(i));
                ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
            })
            .fold(0.0f32, f32::max);
        assert!(d > 1e-3, "different p produced the same orbit: {d:e}");
    }

    /// The conjugate family's REFERENCE must actually conjugate.
    ///
    /// The delta step assumes the reference iterates conj(Z)^p + C. If
    /// the orbit were the plain power instead, shallow views would
    /// still look right -- deltas dominate there and rebasing hides it
    /// -- while deep views, where the reference carries the signal,
    /// would be wrong. So this checks the orbit against the map.
    #[test]
    fn conjugate_reference_iterates_the_conjugate_map() {
        let orbit = ReferenceOrbit::compute(
            "-0.90755797705302632", "0.10050898208800299",
            30.0, None, 200, None, 2, false, MAP_CONJ, [0.0, 0.0]
        )
        .expect("orbit");
        let (cx, cy) = (-0.90755797705302632f64, 0.10050898208800299f64);
        let (mut zx, mut zy) = (0.0f64, 0.0f64);
        let mut worst = 0.0f64;
        for i in 1..(orbit.len() as usize).min(200) {
            let (px, py) = (zx, -zy); // conjugate
            let (nx, ny) = (px * px - py * py + cx, 2.0 * px * py + cy);
            zx = nx;
            zy = ny;
            let got = orbit.z_f32(i);
            worst = worst.max(
                ((got[0] as f64 - zx).powi(2) + (got[1] as f64 - zy).powi(2)).sqrt(),
            );
        }
        assert!(worst < 1e-4, "conjugate reference diverges from conj(z)^2 + c: {worst:e}");

        // And it must NOT be the plain power.
        let plain = ReferenceOrbit::compute(
            "-0.90755797705302632", "0.10050898208800299",
            30.0, None, 200, None, 2, false, MAP_PLAIN, [0.0, 0.0]
        )
        .expect("orbit");
        let d = (0..(plain.len() as usize).min(200))
            .map(|i| {
                let (a, b) = (orbit.z_f32(i), plain.z_f32(i));
                ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
            })
            .fold(0.0f32, f32::max);
        assert!(d > 1e-3, "conjugate and plain references are identical: {d:e}");
    }

    #[test]
    fn deepen_is_an_append_and_matches_fresh_compute() {
        let mut a = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 50, None, 2, false, 0, [0.0, 0.0]).unwrap();
        a.extend(120);
        let b = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 120, None, 2, false, 0, [0.0, 0.0]).unwrap();
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
            let o = cache.get("-0.5", "0.1", 10.0, 50, jc, 2, false, 0, [0.0, 0.0]).unwrap();
            assert_eq!(o.len(), 51);
        }
        // Same view, deeper iterations: extend in place.
        {
            let o = cache.get("-0.5", "0.1", 10.0, 80, jc, 2, false, 0, [0.0, 0.0]).unwrap();
            assert_eq!(o.len(), 81);
        }
        // Different center: replace.
        {
            let o = cache.get("-0.75", "0.1", 10.0, 10, jc, 2, false, 0, [0.0, 0.0]).unwrap();
            assert_eq!(o.center_re, "-0.75");
            assert_eq!(o.len(), 11);
        }
        // Parameter-plane Mandelbrot goes nucleus-aware: an interior
        // view relocates to a periodic reference.
        {
            let o = cache.get("-1.0", "0.0", 10.0, 100, None, 2, false, 0, [0.0, 0.0]).unwrap();
            assert_eq!(o.periodic, Some(2), "the period-2 nucleus governs c = -1");
            assert_eq!(o.len(), 3);
        }
        // Unparseable center: None.
        assert!(cache.get("not a number", "0", 10.0, 10, None, 2, false, 0, [0.0, 0.0]).is_none());
    }

    #[test]
    fn julia_orbit_seeds_from_the_center() {
        // Julia: z0 = center, c fixed. First entry must be the center,
        // then iterate z^2 + c.
        let orbit = ReferenceOrbit::compute(
            "0.25", "0.5", 10.0, None, 20, Some((-0.8, 0.156)), 2, false, 0, [0.0, 0.0]
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
        cache.get("0.25", "0.5", 10.0, 20, Some((-0.8, 0.156)), 2, false, 0, [0.0, 0.0]).unwrap();
        let replaced = cache.get("0.25", "0.5", 10.0, 20, None, 2, false, 0, [0.0, 0.0]).unwrap();
        assert_eq!(replaced.julia_c, None);
    }

    #[test]
    fn cubic_orbit_matches_f64_iteration() {
        // power 3: z^3 + c against a plain f64 loop.
        let orbit =
            ReferenceOrbit::compute("-0.2", "0.4", 10.0, None, 60, None, 3, false, 0, [0.0, 0.0]).unwrap();
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
            map_params: [0.0, 0.0],
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
            map_params: [0.0, 0.0],
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
            ReferenceOrbit::compute("-0.6", "-0.4", 10.0, None, 60, None, 2, true, 0, [0.0, 0.0]).unwrap();
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
    fn the_orbit_cost_model_matches_what_we_measured() {
        // The f3 reference: 10,100,100 iterations at 197 limbs took
        // 495 s of single-threaded fixed-point arithmetic. The model
        // exists to decide whether to WAIT for a reference or render
        // against its growing prefix, so being within a factor of two
        // is what matters, not precision.
        let f3 = predicted_orbit_seconds(10_100_100, 197);
        assert!(
            (400.0..600.0).contains(&f3),
            "f3 reference predicted at {f3:.0} s, measured 495 s"
        );
        // A shallow view's reference is not worth waiting for.
        assert!(predicted_orbit_seconds(10_000, 13) < 0.01);
        // Cost is quadratic in limbs: twice the precision is 4x.
        let a = predicted_orbit_seconds(100_000, 50);
        let b = predicted_orbit_seconds(100_000, 100);
        assert!((b / a - 4.0).abs() < 1e-6);
    }

    #[test]
    fn a_too_shallow_hint_makes_the_plain_orbit_the_answer() {
        // The cache key hashes the CENTER, not the period, so a
        // request carrying a hint has to be able to recognise a
        // stored plain reference as its answer — otherwise setting
        // the period field silently discards a multi-minute orbit
        // and recomputes it (measured: 8 minutes, observed by a user
        // at zoom 9316 with period 71,100).
        let mut orbit =
            ReferenceOrbit::compute("-0.5", "0.1", 60.0, None, 200, None, 2, false, 0, [0.0, 0.0]).unwrap();
        orbit.hint_period = Some(4242);
        orbit.hint_octave = -100; // the hint closes only at 2^-100

        // Deep view: 2^-100 cannot wrap below its pixel scale, so the
        // plain orbit IS what that request should get.
        assert!(orbit.answers_hint(Some(4242), 200.0));
        // Shallow view: there the same hint WOULD serve, so the
        // periodic form must be built rather than this one reused.
        assert!(!orbit.answers_hint(Some(4242), 50.0));
        // A different hint is a different question.
        assert!(!orbit.answers_hint(Some(99), 200.0));
        // No hint asks nothing of the orbit.
        assert!(orbit.answers_hint(None, 200.0));

        // A genuinely periodic orbit answers its own period.
        let mut periodic =
            ReferenceOrbit::compute("-0.5", "0.1", 60.0, None, 200, None, 2, false, 0, [0.0, 0.0]).unwrap();
        periodic.periodic = Some(4242);
        assert!(periodic.answers_hint(Some(4242), 200.0));

        // The fact survives serialization (it lives in the fixed
        // prefix so the store can read it without parsing the file).
        let back = ReferenceOrbit::from_bytes(&orbit.to_bytes()).expect("round trip");
        assert_eq!(back.hint_period, Some(4242));
        assert_eq!(back.hint_octave, -100);
    }

    #[test]
    fn near_nucleus_iterates_survive_f32_storage() {
        // The z700 escape-lag bug: a reference iterate that passes
        // very close to a nucleus is far below f32's smallest normal
        // (2^-126), so plain f32 storage read it as EXACTLY ZERO -
        // which deletes 2*Z*delta from that step of the delta
        // recurrence. Measured cost on the field location: a 2^-183
        // dip at i=8897 dropped the delta 154 octaves it never
        // recovered, and a corner pixel whose true escape is 23,649
        // rendered as interior past 41,163.
        //
        // The exponent must therefore be carried, and the mantissa
        // must stay normalized.
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
        assert_eq!(orbit.orbit_e.len(), orbit.orbit.len());
        // Z_3 is the nucleus closure: deep enough that f32 alone
        // cannot hold it.
        let e = orbit.orbit_e[3];
        assert!(
            e < -126,
            "a nucleus closure must carry its own exponent, got {e}"
        );
        let m = orbit.orbit[3];
        let mag = m[0].abs().max(m[1].abs());
        assert!(
            (1.0..4.0).contains(&mag),
            "stored mantissa must be normalized, got {mag}"
        );
        // The value view is unchanged from the pre-exponent format.
        assert_eq!(entry_value(m, e), [0.0, 0.0]);
        assert_eq!(orbit.z_f32(3), [0.0, 0.0]);
        // ...and the magnitude the recurrence needs is recoverable:
        // mantissa * 2^e agrees with the exact fixed-point iterate.
        let (zre, _zim) = orbit.z_state();
        let fe = zre.to_floatexp();
        assert!(
            (fe.e - e as i64).abs() <= 1,
            "octave mismatch: stored 2^{e} vs exact 2^{}",
            fe.e
        );
        // Every ordinary iterate keeps exponent 0 (the old format),
        // so nothing else in the pipeline changes meaning.
        assert_eq!(orbit.orbit_e[0], 0);
        assert_eq!(orbit.orbit_e[1], 0);
        assert_eq!(orbit.z_f32(1), orbit.orbit[1]);
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
        // Through the accessor: a closure this deep is stored as a
        // normalized mantissa plus an exponent (the raw array holds
        // the mantissa, which is O(1) by construction).
        let [zx, zy] = orbit.z_f32(3);
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
                "-0.55", "-0.4", 10.0, None, 40, None, 2, true, v, [0.0, 0.0]
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
            .get("0.1", "0.2", 12.0, 500, jc, 2, false, 0, [0.0, 0.0])
            .unwrap()
            .orbit
            .clone();
        let mut sliced = OrbitCache::default();
        let mut steps = 0;
        loop {
            let (orbit, done) = sliced
                .get_budgeted("0.1", "0.2", 12.0, 500, jc, 2, false, 0, [0.0, 0.0], 64)
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
            0, [0.0, 0.0]
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
            0, [0.0, 0.0]
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
            ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 500, None, 2, false, 0, [0.0, 0.0]).unwrap();
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
            0, [0.0, 0.0]
        )
        .unwrap();
        assert_eq!(julia.periodic, None, "Julia must not auto-close");
    }

    #[test]
    fn relocation_offset_rescales_with_the_view() {
        let mut orbit =
            ReferenceOrbit::compute("-0.5", "0.1", 30.0, None, 50, None, 2, false, 0, [0.0, 0.0]).unwrap();
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
            0, [0.0, 0.0]
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
            0, [0.0, 0.0]
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
