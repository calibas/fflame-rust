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
    if !off[0].is_finite() || !off[1].is_finite() || off[0].abs() > 1.0e7 || off[1].abs() > 1.0e7 {
        return None; // nucleus implausibly far: distrust it
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
    /// Burning Ship: fold |x|, |y| before squaring (power is then 2).
    /// Sign-magnitude makes the fold free (clear the sign flags).
    pub ship: bool,
    /// This orbit sits at a minibrot nucleus of the given period:
    /// Z_period = 0 = Z_0 exactly, so the wrap-rebase is exact and
    /// the orbit never needs extending past the period.
    pub periodic: Option<u32>,
    /// (view − reference) in pixel-spacing units, for the pipeline's
    /// d0. Zero when the reference IS the view center.
    pub ref_offset: [f32; 2],
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
        ship: bool,
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
            periodic: None,
            ref_offset: [0.0, 0.0],
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
        if let Some(p) = self.periodic {
            // A nucleus orbit is complete at its period.
            let _ = p;
            return;
        }
        while (self.orbit.len() as u32) <= max_iter {
            if self.ship {
                // Burning Ship: fold both components (free in
                // sign-magnitude), then square.
                self.z.re.neg = false;
                self.z.im.neg = false;
            }
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
        ship: bool,
    ) -> bool {
        self.center_re == center_re
            && self.center_im == center_im
            && self.n_limbs >= n_limbs
            && self.julia_c == julia_c
            && self.power == power.max(2)
            && self.ship == ship
    }
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
    /// view center, in pixel-spacing units (nucleus references).
    pub ref_offset: [f32; 2],
    /// Which request this data belongs to (bumped on every new
    /// request; stale chunks from an abandoned compute are ignored).
    pub epoch: u64,
    /// Z_n snapshots so far (orbit[0] = the seed).
    pub orbit: Vec<[f32; 2]>,
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
                                && old_req.ship == req.ship;
                            if same { Some(orbit) } else { None }
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
                            p.ref_offset = orbit.ref_offset;
                            p.done = orbit.periodic.is_some()
                                || orbit.escaped_at.is_some()
                                || orbit.len() > req.max_iter;
                        }
                        current = Some((req, orbit, epoch));
                    }

                    // Advance the current job by one chunk.
                    if let Some((req, orbit, epoch)) = current.as_mut() {
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
                                p.orbit.extend_from_slice(&orbit.orbit[have..]);
                                p.done = done;
                            }
                        }
                        if done {
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
    if req.julia_c.is_none() && !req.ship {
        ReferenceOrbit::compute_nucleus_aware(
            &req.center_re,
            &req.center_im,
            req.zoom_log2,
            0,
            req.height_px.max(1.0),
            req.power,
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
                                && old_req.ship == req.ship;
        if same { Some(orbit) } else { None }
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
        p.ref_offset = orbit.ref_offset;
        p.done = orbit.periodic.is_some()
            || orbit.escaped_at.is_some()
            || orbit.len() > req.max_iter;
    }
    *current = Some((req, orbit, epoch));
    Some(())
}

impl ReferenceOrbit {
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
    ) -> Option<Self> {
        if let Some((nre, nim, period, off)) =
            nucleus_for_view(center_re, center_im, zoom_log2, height_px, power)
        {
            if let Some(mut orbit) = Self::compute(
                &nre,
                &nim,
                zoom_log2,
                None,
                period,
                None,
                power,
                false,
            ) {
                if orbit.escaped_at.is_none() && orbit.len() > period {
                    // Store under the VIEW key (the cache is keyed on
                    // what was asked for), remember the relocation.
                    orbit.center_re = center_re.to_string();
                    orbit.center_im = center_im.to_string();
                    orbit.periodic = Some(period);
                    orbit.ref_offset = off;
                    return Some(orbit);
                }
            }
        }
        Self::compute(center_re, center_im, zoom_log2, None, max_iter, None, power, false)
    }
}

/// Single-slot orbit cache: during a continuous zoom the center is
/// unchanged, so one orbit serves every frame; deepening appends. A
/// pan or precision change replaces the slot.
#[derive(Default)]
pub struct OrbitCache {
    slot: Option<ReferenceOrbit>,
    height_px: f64,
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
        ship: bool,
    ) -> Option<&ReferenceOrbit> {
        let n = limbs_for_zoom(zoom_log2);
        let hit = self
            .slot
            .as_ref()
            .is_some_and(|o| o.serves(center_re, center_im, n, julia_c, power, ship));
        if hit {
            let orbit = self.slot.as_mut().unwrap();
            orbit.extend(max_iter);
        } else if julia_c.is_none() && !ship {
            self.slot = Some(ReferenceOrbit::compute_nucleus_aware(
                center_re,
                center_im,
                zoom_log2,
                max_iter,
                self.height_px.max(1.0),
                power,
            )?);
        } else {
            self.slot = Some(ReferenceOrbit::compute(
                center_re, center_im, zoom_log2, Some(n), max_iter, julia_c, power, ship,
            )?);
        }
        self.slot.as_ref()
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
        let orbit = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 100, None, 2, false).unwrap();
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
        let orbit = ReferenceOrbit::compute("1", "1", 5.0, None, 1000, None, 2, false).unwrap();
        let at = orbit.escaped_at.expect("c = 1+i escapes fast");
        assert!(at < 10);
        assert_eq!(orbit.len() - 1, at);
    }

    #[test]
    fn deepen_is_an_append_and_matches_fresh_compute() {
        let mut a = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 50, None, 2, false).unwrap();
        a.extend(120);
        let b = ReferenceOrbit::compute("-0.5", "0.1", 10.0, None, 120, None, 2, false).unwrap();
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
            let o = cache.get("-0.5", "0.1", 10.0, 50, jc, 2, false).unwrap();
            assert_eq!(o.len(), 51);
        }
        // Same view, deeper iterations: extend in place.
        {
            let o = cache.get("-0.5", "0.1", 10.0, 80, jc, 2, false).unwrap();
            assert_eq!(o.len(), 81);
        }
        // Different center: replace.
        {
            let o = cache.get("-0.75", "0.1", 10.0, 10, jc, 2, false).unwrap();
            assert_eq!(o.center_re, "-0.75");
            assert_eq!(o.len(), 11);
        }
        // Parameter-plane Mandelbrot goes nucleus-aware: an interior
        // view relocates to a periodic reference.
        {
            let o = cache.get("-1.0", "0.0", 10.0, 100, None, 2, false).unwrap();
            assert_eq!(o.periodic, Some(2), "the period-2 nucleus governs c = -1");
            assert_eq!(o.len(), 3);
        }
        // Unparseable center: None.
        assert!(cache.get("not a number", "0", 10.0, 10, None, 2, false).is_none());
    }

    #[test]
    fn julia_orbit_seeds_from_the_center() {
        // Julia: z0 = center, c fixed. First entry must be the center,
        // then iterate z^2 + c.
        let orbit = ReferenceOrbit::compute(
            "0.25", "0.5", 10.0, None, 20, Some((-0.8, 0.156)), 2, false,
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
        cache.get("0.25", "0.5", 10.0, 20, Some((-0.8, 0.156)), 2, false).unwrap();
        let replaced = cache.get("0.25", "0.5", 10.0, 20, None, 2, false).unwrap();
        assert_eq!(replaced.julia_c, None);
    }

    #[test]
    fn cubic_orbit_matches_f64_iteration() {
        // power 3: z^3 + c against a plain f64 loop.
        let orbit =
            ReferenceOrbit::compute("-0.2", "0.4", 10.0, None, 60, None, 3, false).unwrap();
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
        let sync = ReferenceOrbit::compute_nucleus_aware("-0.5", "0.1", 5.0, 10_000, 320.0, 2)
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
            ReferenceOrbit::compute("-0.6", "-0.4", 10.0, None, 60, None, 2, true).unwrap();
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
        let plain = ReferenceOrbit::compute_nucleus_aware("0.3", "0.5", 20.0, 100, 320.0, 2)
            .expect("computes");
        assert_eq!(plain.periodic, None);
        assert_eq!(plain.ref_offset, [0.0, 0.0]);
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
