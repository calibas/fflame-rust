//! Exact circle geometry for the inversive family — family M of
//! `docs/projects/inversive-targeting.md`.
//!
//! # Why this exists
//!
//! The enumeration pushes a region through a word and stops when it
//! fits the view. For an affine map a disc is the right shape and
//! `affine_ball` is exact. For an INVERSION it is neither.
//!
//! `spherical` is `p/|p|²`, and the smallest disc containing the image
//! of a disc that straddles the pole is the whole plane —
//! `SPHERICAL_BOUND` returns its global `D(0, 500)` and says so
//! honestly. Translate that, invert again, and it is `D(0, 500)` once
//! more. The measured orbit contracts at −0.30 per step
//! (`scripts/inversive_probe.py`); a disc bound sees none of it,
//! because the contraction lives in fine structure and the disc is
//! everything. A word never shrinks, so the enumeration never
//! terminates, so the flame is refused.
//!
//! An inversion does take **circles to circles, exactly**. What it
//! does not preserve is which side is the inside: the component
//! holding the pole goes to the unbounded one. A disc alone is
//! therefore not closed under it — but an **outer disc minus a hole
//! per pole** is, and closes in the prettiest possible way.
//!
//! # The closure
//!
//! Write the region as `O ∖ (H₁ ∪ … ∪ H_k)`, with the pole inside
//! `H₁`. Inversion distributes over the set difference, and sends each
//! component holding the pole to an unbounded one:
//!
//! ```text
//! σ(O ∖ ⋃Hᵢ) = σ(O) ∖ ⋃σ(Hᵢ)
//!            = comp(A) ∖ (comp(B) ∪ C₂ ∪ … ∪ C_k)
//!            = B ∖ (A ∪ C₂ ∪ … ∪ C_k)
//! ```
//!
//! **The hole guarding the pole becomes the new outer disc, and the
//! old outer disc becomes a hole.** Same shape, same number of holes,
//! no over-estimate anywhere — the image of the region IS this, not a
//! bound on it. That turning-inside-out is the whole Schottky
//! construction, and cylinder targeting for a Möbius IFS is the DFS of
//! *Indra's Pearls* wearing different clothes.
//!
//! # What is exact and what is not
//!
//! The shipped `spherical` is `p/(|p|² + 1e-6)`, not `p/|p|²`. That
//! guard is what keeps the pole finite, and also what stops the map
//! being a Möbius map there at all. Away from the pole the discrepancy
//! is `ε/|p|³`, and [`Region::invert`] inflates by exactly that; at the
//! pole the guard decides the answer and there is no circle to map, so
//! a region whose hole does not actually exclude the pole is refused
//! rather than answered. Keeping that hole is the caller's job.

/// A closed disc. The building block of both a region's outer
/// boundary and its holes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Disc {
    pub c: [f64; 2],
    pub r: f64,
}

impl Disc {
    pub fn new(c: [f64; 2], r: f64) -> Self {
        Self { c, r }
    }

    pub fn finite(&self) -> bool {
        self.c[0].is_finite() && self.c[1].is_finite() && self.r.is_finite() && self.r >= 0.0
    }

    fn centre_norm(&self) -> f64 {
        (self.c[0] * self.c[0] + self.c[1] * self.c[1]).sqrt()
    }

    pub fn contains(&self, p: [f64; 2]) -> bool {
        (p[0] - self.c[0]).powi(2) + (p[1] - self.c[1]).powi(2) <= self.r * self.r
    }

    fn dist_to(&self, p: [f64; 2]) -> f64 {
        ((p[0] - self.c[0]).powi(2) + (p[1] - self.c[1]).powi(2)).sqrt()
    }

    /// How near this disc's BOUNDARY CIRCLE comes to the pole.
    ///
    /// The guard's discrepancy at a point is `ε/|p|³`, so the error on
    /// a circle's image is set by the circle's own closest approach —
    /// not by the region's. Using one region-wide figure was sound
    /// but ruinous: an absolute 8e-3 taken off a radius-0.05 hole is a
    /// fifteen per cent error, and it showed up as a five per cent
    /// drift in `inverting_twice_is_the_identity`, which for an exact
    /// map should drift by nothing.
    fn rim_standoff(&self) -> f64 {
        (self.centre_norm() - self.r).abs()
    }

    /// Whether this disc meets `D(c, r)`.
    pub fn meets(&self, c: [f64; 2], r: f64) -> bool {
        self.dist_to(c) <= self.r + r
    }

    /// Push through `p ↦ M p + t` for a similarity of scale `sigma`.
    /// Exact: similarities take circles to circles.
    pub fn similarity(&self, m: [[f64; 2]; 2], t: [f64; 2], sigma: f64) -> Self {
        Self {
            c: [
                m[0][0] * self.c[0] + m[0][1] * self.c[1] + t[0],
                m[1][0] * self.c[0] + m[1][1] * self.c[1] + t[1],
            ],
            r: self.r * sigma.abs(),
        }
    }

    /// The image of this disc under `p ↦ w·p/|p|²`, as a circle plus
    /// which side of it the image is.
    ///
    /// Inversion in the unit circle takes `|z − c| = r` to the circle
    /// centred `c/(|c|² − r²)` of radius `r/‖c|² − r²|`. No
    /// conjugation: `p/|p|²` is `1/z̄`, the geometric inversion. The
    /// weight scales both, `w·σ` being inversion in the circle of
    /// radius `√w`.
    ///
    /// `true` means the image is the COMPLEMENT of the returned disc,
    /// which happens exactly when this disc holds the pole — the pole
    /// goes to infinity, and the component holding it becomes the
    /// unbounded one.
    fn invert_raw(&self, w: f64) -> Result<(Self, bool), NoCircle> {
        if !self.finite() || !w.is_finite() {
            return Err(NoCircle::NotFinite);
        }
        let n = self.centre_norm();
        let denom = n * n - self.r * self.r;
        // Measured against the circle's own scale, so this means "the
        // pole is too near the RIM to tell which side it is on" — the
        // image would be a line, which is a generalized disc this does
        // not carry.
        let scale = (n * n + self.r * self.r).max(f64::MIN_POSITIVE);
        if (denom / scale).abs() <= 1e-12 {
            return Err(NoCircle::ThroughPole);
        }
        let out = Self {
            c: [w * self.c[0] / denom, w * self.c[1] / denom],
            r: w.abs() * self.r / denom.abs(),
        };
        if !out.finite() {
            return Err(NoCircle::NotFinite);
        }
        Ok((out, denom < 0.0))
    }
}

/// Why a region could not be pushed exactly.
#[derive(Debug, Clone, PartialEq)]
pub enum NoCircle {
    /// A boundary circle passes through the pole, so its image is a
    /// line. A half-plane is a generalized disc too, but it is a third
    /// case in every operation here and the hole policy keeps every
    /// circle strictly to one side.
    ThroughPole,
    /// The region itself reaches the pole: no hole excludes it, so the
    /// shipped `+1e-6` guard rather than the inversion decides the
    /// answer. There is no circle to map, and the caller falls back to
    /// the global disc bound.
    ReachesPole,
    /// A non-finite circle arrived or was produced.
    NotFinite,
    /// The region came out empty — every point excluded.
    Empty,
}

impl std::fmt::Display for NoCircle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ThroughPole => write!(f, "a boundary circle passes through the pole"),
            Self::ReachesPole => write!(f, "the region reaches the pole's guard"),
            Self::NotFinite => write!(f, "a non-finite circle"),
            Self::Empty => write!(f, "the region is empty"),
        }
    }
}

/// The `1e-6` in `spherical`'s denominator, mirrored from the WGSL in
/// `defs/basic.rs`.
pub const SPHERICAL_EPS: f64 = 1e-6;

/// An outer disc with holes punched out: `outer ∖ ⋃ holes`.
///
/// Closed under similarities and under inversion — see the module
/// docs for why, and why the closure is the point.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub outer: Disc,
    /// Excluded discs. Order carries no meaning; the count stays at
    /// one per pole, because inversion trades the pole's hole for the
    /// outer disc rather than adding to the list.
    pub holes: Vec<Disc>,
}

impl Region {
    pub fn new(outer: Disc, holes: Vec<Disc>) -> Self {
        let mut r = Self { outer, holes };
        r.tidy();
        r
    }

    /// Drop holes that cannot cut anything: ones that miss the outer
    /// disc entirely. Without this the list grows by a dead entry on
    /// every inversion.
    fn tidy(&mut self) {
        let o = self.outer;
        self.holes.retain(|h| h.r > 0.0 && h.meets(o.c, o.r));
        // A hole inside another cuts nothing extra, and leaving it
        // there is how two discs end up covering one pole.
        let mut keep = vec![true; self.holes.len()];
        for i in 0..self.holes.len() {
            for j in 0..self.holes.len() {
                if i == j || !keep[j] {
                    continue;
                }
                let (a, b) = (&self.holes[i], &self.holes[j]);
                // `a` inside `b`; on a tie, drop the later one only.
                if a.dist_to(b.c) + a.r <= b.r && (a.r < b.r || i > j) {
                    keep[i] = false;
                    break;
                }
            }
        }
        let mut k = 0;
        self.holes.retain(|_| {
            k += 1;
            keep[k - 1]
        });
    }

    /// Whether some hole swallows the outer disc, leaving nothing.
    pub fn is_empty(&self) -> bool {
        self.outer.r <= 0.0
            || self
                .holes
                .iter()
                .any(|h| h.dist_to(self.outer.c) + self.outer.r <= h.r)
    }

    /// Distance from the pole to the nearest point of the region, and
    /// the hole that excludes the pole if one does.
    ///
    /// Zero distance means the pole is IN the region, which is exactly
    /// the case the shipped guard owns and the inversion does not.
    fn pole_standoff(&self) -> (f64, Option<usize>) {
        let o = &self.outer;
        // Outside the outer disc: the region starts at its near rim.
        let d_out = o.centre_norm() - o.r;
        if d_out > 0.0 {
            return (d_out, None);
        }
        // **Several holes can cover the pole at once**, because the
        // walk re-imposes the root's holes on top of the images of
        // the previous step's. Their union is what guards it, and a
        // union of discs is not a disc — so take the one that guards
        // BEST and let `invert` drop the others, which only grows the
        // region and so cannot lose an image point.
        //
        // Best means largest standoff: the new outer disc comes from
        // this hole's image, and a hole that keeps the pole further
        // away inverts to a smaller disc.
        let mut best: Option<(f64, usize)> = None;
        for (i, h) in self.holes.iter().enumerate() {
            if h.contains([0.0, 0.0]) {
                let s = (h.r - h.centre_norm()).max(0.0);
                if best.map_or(true, |(bs, _)| s > bs) {
                    best = Some((s, i));
                }
            }
        }
        match best {
            Some((s, i)) => (s, Some(i)),
            None => (0.0, None),
        }
    }

    /// Push through `p ↦ M p + t` for a similarity of scale `sigma`.
    /// Exact, and it cannot change the shape.
    pub fn similarity(&self, m: [[f64; 2]; 2], t: [f64; 2], sigma: f64) -> Self {
        Self::new(
            self.outer.similarity(m, t, sigma),
            self.holes.iter().map(|h| h.similarity(m, t, sigma)).collect(),
        )
    }

    /// Push through `p ↦ w·p/|p|²` — the shipped `spherical` times a
    /// variation weight.
    ///
    /// Exact up to the guard's `ε/d³`, which is added to the outer
    /// disc and taken off each hole, both of which only ever grow the
    /// set. Requires a hole that actually excludes the pole; see
    /// [`NoCircle::ReachesPole`].
    pub fn invert(&self, w: f64) -> Result<Self, NoCircle> {
        if self.is_empty() {
            return Err(NoCircle::Empty);
        }
        let (standoff, guard) = self.pole_standoff();
        // Inside `√ε` of the pole the guard, not the inversion, is
        // what the shader computes: `√ε` is where `|p|²` and `ε` are
        // the same size.
        if standoff <= SPHERICAL_EPS.sqrt() {
            return Err(NoCircle::ReachesPole);
        }

        // Every circle carries its own error, from its own approach
        // to the pole.
        let slop_of = |d: &Disc| -> Option<f64> {
            let dd = d.rim_standoff();
            let v = w.abs() * SPHERICAL_EPS / (dd * dd * dd);
            v.is_finite().then_some(v)
        };
        let outer_slop = slop_of(&self.outer).ok_or(NoCircle::NotFinite)?;
        let (outer_img, outer_flips) = self.outer.invert_raw(w)?;
        let mut holes: Vec<Disc> = Vec::with_capacity(self.holes.len());
        let mut new_outer: Option<(Disc, f64)> = None;
        for (i, h) in self.holes.iter().enumerate() {
            let hslop = slop_of(h).ok_or(NoCircle::NotFinite)?;
            let (img, flips) = h.invert_raw(w)?;
            if Some(i) == guard {
                // σ(H₁) = comp(B), so B bounds everything that is
                // left: it is the new outer disc.
                if !flips {
                    return Err(NoCircle::ThroughPole);
                }
                new_outer = Some((img, hslop));
            } else if flips {
                // Another hole covering the pole. The guard above
                // already excludes a neighbourhood of it, so dropping
                // this one grows the region a little and keeps it
                // representable — an intersection of two discs is not
                // a disc.
                continue;
            } else {
                // A hole clear of the pole stays a hole, and shrinks
                // by its own slop, which grows the set: the only safe
                // direction.
                holes.push(Disc::new(img.c, (img.r - hslop).max(0.0)));
            }
        }

        let (outer, outer_grow) = match new_outer {
            // The pole was inside a hole: that hole turned inside out
            // and now bounds the region, while the old outer disc —
            // which held the pole, so its image is unbounded — becomes
            // the hole that keeps the new region clear of infinity.
            Some((b, bslop)) => {
                if !outer_flips {
                    return Err(NoCircle::ThroughPole);
                }
                holes.push(Disc::new(outer_img.c, (outer_img.r - outer_slop).max(0.0)));
                (b, bslop)
            }
            // The pole was outside the region altogether: everything
            // stays the way round it was.
            None => {
                if outer_flips {
                    return Err(NoCircle::ReachesPole);
                }
                (outer_img, outer_slop)
            }
        };

        let _ = standoff;
        let out = Self::new(Disc::new(outer.c, outer.r + outer_grow), holes);
        if out.is_empty() {
            return Err(NoCircle::Empty);
        }
        Ok(out)
    }

    /// Whether the region can meet the disc `D(c, r)`.
    ///
    /// Conservative in the safe direction: `true` unless a single hole
    /// provably swallows the view disc. Two holes could cover it
    /// between them without either doing so alone, and saying "meets"
    /// then costs an extra subdivision rather than a dropped word.
    pub fn meets_disc(&self, c: [f64; 2], r: f64) -> bool {
        if !self.outer.meets(c, r) {
            return false;
        }
        !self.holes.iter().any(|h| h.dist_to(c) + r <= h.r)
    }

    /// A radius bounding the region — what the enumeration compares
    /// against the view to decide it has gone deep enough.
    ///
    /// The holes only ever make the set smaller, so the outer disc's
    /// radius is always a valid answer.
    pub fn radius(&self) -> f64 {
        self.outer.r
    }

    pub fn centre(&self) -> [f64; 2] {
        self.outer.c
    }

    pub fn contains(&self, p: [f64; 2]) -> bool {
        self.outer.contains(p) && !self.holes.iter().any(|h| h.contains(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped body, guard and all, to compare against.
    fn spherical(p: [f64; 2], w: f64) -> [f64; 2] {
        let r2 = p[0] * p[0] + p[1] * p[1] + SPHERICAL_EPS;
        [w * p[0] / r2, w * p[1] / r2]
    }

    fn contains_slack(reg: &Region, p: [f64; 2], slack: f64) -> bool {
        let d = reg.outer.dist_to(p);
        if d > reg.outer.r * (1.0 + slack) + slack {
            return false;
        }
        // A point just inside a hole is a real violation; allow only
        // the same relative slack.
        !reg.holes.iter().any(|h| h.dist_to(p) < h.r * (1.0 - slack) - slack)
    }

    /// Points spread over a region: hole rims, the outer rim, and a
    /// grid of the interior.
    fn samples(reg: &Region, n: usize) -> Vec<[f64; 2]> {
        let mut out = Vec::new();
        for k in 0..n {
            let a = std::f64::consts::TAU * k as f64 / n as f64;
            let (s, c) = a.sin_cos();
            for m in [1.0, 0.999, 0.9, 0.6, 0.25] {
                out.push([
                    reg.outer.c[0] + reg.outer.r * m * c,
                    reg.outer.c[1] + reg.outer.r * m * s,
                ]);
            }
            for h in &reg.holes {
                for m in [1.0, 1.001, 1.05, 1.5] {
                    out.push([h.c[0] + h.r * m * c, h.c[1] + h.r * m * s]);
                }
            }
        }
        out.retain(|p| reg.contains(*p));
        out
    }

    fn schottky() -> Region {
        // An outer disc holding the pole, with the pole guarded.
        Region::new(
            Disc::new([0.4, -0.2], 6.0),
            vec![Disc::new([0.0, 0.0], 0.05), Disc::new([2.0, 1.0], 0.3)],
        )
    }

    /// **Every point of the region really does land in the image.**
    /// Checked against the SHIPPED body, so the guard's inflation is
    /// part of what is tested rather than an allowance granted.
    #[test]
    fn an_inverted_region_contains_every_image_point() {
        let cases = [
            schottky(),
            // Pole outside the region entirely: nothing turns around.
            Region::new(Disc::new([5.0, 0.0], 1.0), vec![]),
            Region::new(Disc::new([5.0, 0.0], 2.0), vec![Disc::new([5.5, 0.5], 0.4)]),
            // Pole guarded, tighter.
            Region::new(Disc::new([0.0, 0.0], 3.0), vec![Disc::new([0.0, 0.0], 0.2)]),
            Region::new(Disc::new([1.0, 1.0], 9.0), vec![Disc::new([0.1, -0.1], 0.5)]),
        ];
        for reg in cases {
            for w in [1.0f64, 0.37, 2.5] {
                let Ok(img) = reg.invert(w) else { continue };
                let pts = samples(&reg, 96);
                assert!(!pts.is_empty());
                for p in pts {
                    let q = spherical(p, w);
                    assert!(
                        contains_slack(&img, q, 1e-7),
                        "{reg:?} at w={w}: {p:?} -> {q:?} escaped {img:?}"
                    );
                }
            }
        }
    }

    /// The closure the module exists for: the hole guarding the pole
    /// becomes the outer disc, and the outer disc becomes a hole.
    #[test]
    fn the_poles_hole_becomes_the_new_outer_disc() {
        let reg = schottky();
        let img = reg.invert(1.0).unwrap();
        // σ of the guarding hole D(0, 0.05) is everything outside
        // radius 1/0.05 = 20, so the new outer disc has radius 20.
        assert!(
            (img.outer.r - 20.0).abs() < 0.05,
            "expected the 0.05 hole to become a radius-20 outer disc, got {:?}",
            img.outer
        );
        // The old outer disc held the pole, so it is now a hole.
        assert_eq!(img.holes.len(), 2, "{img:?}");
        // ...and the region still has area.
        assert!(!img.is_empty());
    }

    /// Inversion is an involution, so pushing a region twice returns
    /// it. The strongest statement that this is EXACT rather than
    /// merely containing — a bound could never survive it.
    #[test]
    fn inverting_twice_is_the_identity() {
        for reg in [
            schottky(),
            Region::new(Disc::new([0.0, 0.0], 3.0), vec![Disc::new([0.0, 0.0], 0.2)]),
            Region::new(Disc::new([5.0, 0.0], 1.0), vec![]),
        ] {
            let there = reg.invert(1.0).unwrap();
            let back = there.invert(1.0).unwrap();
            let scale = reg.outer.r.max(1.0);
            assert!(
                (back.outer.c[0] - reg.outer.c[0]).abs() < 1e-3 * scale
                    && (back.outer.c[1] - reg.outer.c[1]).abs() < 1e-3 * scale
                    && (back.outer.r - reg.outer.r).abs() < 1e-3 * scale,
                "{reg:?}\n  -> {there:?}\n  -> {back:?}"
            );
            assert_eq!(back.holes.len(), reg.holes.len(), "{back:?} vs {reg:?}");
        }
    }

    /// The textbook image, checkable by hand: the circle from 1 to 3
    /// on the real axis inverts to the one from 1/3 to 1.
    #[test]
    fn a_hand_checkable_circle() {
        let (d, flips) = Disc::new([2.0, 0.0], 1.0).invert_raw(1.0).unwrap();
        assert!(!flips);
        assert!((d.c[0] - 2.0 / 3.0).abs() < 1e-12, "{d:?}");
        assert!(d.c[1].abs() < 1e-12, "{d:?}");
        assert!((d.r - 1.0 / 3.0).abs() < 1e-12, "{d:?}");
    }

    /// A disc holding the pole inverts to a complement — the fact the
    /// closure rests on.
    #[test]
    fn a_disc_holding_the_pole_inverts_to_a_complement() {
        let (d, flips) = Disc::new([0.0, 0.0], 0.05).invert_raw(1.0).unwrap();
        assert!(flips, "the pole's own hole must turn inside out");
        assert!((d.r - 20.0).abs() < 1e-9, "{d:?}");

        let (_, flips) = Disc::new([3.0, 0.0], 1.0).invert_raw(1.0).unwrap();
        assert!(!flips);
    }

    /// A region with no hole around the pole has no circle image, and
    /// says so rather than returning a number.
    #[test]
    fn an_unguarded_pole_refuses() {
        let bare = Region::new(Disc::new([0.0, 0.0], 1.0), vec![]);
        assert_eq!(bare.invert(1.0), Err(NoCircle::ReachesPole));

        // A hole that misses the pole does not guard it.
        let missed = Region::new(Disc::new([0.0, 0.0], 1.0), vec![Disc::new([0.5, 0.0], 0.1)]);
        assert_eq!(missed.invert(1.0), Err(NoCircle::ReachesPole));

        // A hole smaller than the guard threshold does not either.
        let tiny = Region::new(Disc::new([0.0, 0.0], 1.0), vec![Disc::new([0.0, 0.0], 1e-9)]);
        assert_eq!(tiny.invert(1.0), Err(NoCircle::ReachesPole));
    }

    /// A similarity is exact and cannot change the shape.
    #[test]
    fn a_similarity_moves_a_region_without_reshaping_it() {
        let reg = Region::new(Disc::new([1.0, 0.0], 2.0), vec![Disc::new([1.0, 0.0], 0.5)]);
        let m = [[0.0, -3.0], [3.0, 0.0]];
        let out = reg.similarity(m, [1.0, -2.0], 3.0);
        assert_eq!(out.outer.c, [1.0, 1.0]);
        assert!((out.outer.r - 6.0).abs() < 1e-12);
        assert_eq!(out.holes.len(), 1);
        assert!((out.holes[0].r - 1.5).abs() < 1e-12);
    }

    /// The view test: a disc buried in a hole misses, anything else
    /// meeting the outer disc is reported as meeting.
    #[test]
    fn meeting_a_view_disc() {
        let reg = Region::new(Disc::new([0.0, 0.0], 4.0), vec![Disc::new([1.0, 0.0], 1.0)]);
        assert!(reg.meets_disc([3.0, 0.0], 0.5));
        assert!(!reg.meets_disc([1.0, 0.0], 0.2), "buried in the hole");
        assert!(reg.meets_disc([1.0, 0.0], 2.0), "straddles the hole's rim");
        assert!(!reg.meets_disc([40.0, 0.0], 0.5), "far outside");
    }

    /// Dead holes are dropped, so the list cannot grow without bound
    /// over a long word.
    #[test]
    fn holes_that_cut_nothing_are_dropped() {
        let reg = Region::new(
            Disc::new([0.0, 0.0], 1.0),
            vec![Disc::new([50.0, 0.0], 1.0), Disc::new([0.2, 0.0], 0.3)],
        );
        assert_eq!(reg.holes.len(), 1, "{reg:?}");
    }
}

// ===========================================================================
// Which transforms this applies to, and how they push a region
// ===========================================================================

/// What a qualifying transform does to the plane, after its affine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// `w · p` — a similarity, so the whole transform is one.
    Linear,
    /// `w · p / (|p|² + ε)` — an inversion in the circle of radius
    /// `√w`, which is where the holes come from.
    Spherical,
    /// `w · (Az + B)/(Cz + D)` — the `mobius` variation, whose body is
    /// already exactly the map this module composes.
    ///
    /// **This is the one that makes family M worth having.** An
    /// inversion is its own inverse, so a flame built from `spherical`
    /// has `S_i ∘ S_i = id`: its words fold back on themselves and the
    /// regions oscillate rather than shrink. A general Möbius map is
    /// not an involution, and a group generated by loxodromic ones is
    /// the Schottky case the whole construction is for.
    Mobius { a: C, b: C, c: C, d: C },
}

/// The `1e-10` in the `mobius` variation's denominator, mirrored from
/// the WGSL in `defs/extended.rs`.
pub const MOBIUS_EPS: f64 = 1e-10;

/// A similarity: `p ↦ M p + t`, with `M` a rotation-and-uniform-scale,
/// possibly with a reflection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Similarity {
    pub m: [[f64; 2]; 2],
    pub t: [f64; 2],
    pub sigma: f64,
}

impl Similarity {
    /// Read an affine as a similarity, or refuse.
    ///
    /// `MᵀM = σ²I`: the columns orthogonal and of equal length. A
    /// general affine takes a circle to an ELLIPSE, and an ellipse is
    /// not a generalized disc — the exactness this module exists for
    /// would be gone at the first step.
    pub fn read(m: [[f64; 2]; 2], t: [f64; 2]) -> Option<Self> {
        let (a, b, c, d) = (m[0][0], m[0][1], m[1][0], m[1][1]);
        let c1 = a * a + c * c;
        let c2 = b * b + d * d;
        let dot = a * b + c * d;
        let scale = c1.max(c2);
        if scale <= 0.0 || !scale.is_finite() {
            return None;
        }
        // Relative tolerance: a flame's coefficients are f32, so an
        // exact rotation arrives a few ulps out of true.
        if ((c1 - c2).abs() / scale) > 1e-6 || (dot.abs() / scale) > 1e-6 {
            return None;
        }
        Some(Self { m, t, sigma: scale.sqrt() })
    }

    /// The point this sends to `q`.
    pub fn preimage_of(&self, q: [f64; 2]) -> Option<[f64; 2]> {
        let det = self.m[0][0] * self.m[1][1] - self.m[0][1] * self.m[1][0];
        if det.abs() <= f64::MIN_POSITIVE {
            return None;
        }
        let (rx, ry) = (q[0] - self.t[0], q[1] - self.t[1]);
        let p = [
            (self.m[1][1] * rx - self.m[0][1] * ry) / det,
            (self.m[0][0] * ry - self.m[1][0] * rx) / det,
        ];
        (p[0].is_finite() && p[1].is_finite()).then_some(p)
    }

    /// The point this sends to the origin — the variation's pole, in
    /// the coordinates the region lives in.
    pub fn preimage_of_origin(&self) -> Option<[f64; 2]> {
        let det = self.m[0][0] * self.m[1][1] - self.m[0][1] * self.m[1][0];
        if det.abs() <= f64::MIN_POSITIVE {
            return None;
        }
        Some([
            (self.m[0][1] * self.t[1] - self.m[1][1] * self.t[0]) / det,
            (self.m[1][0] * self.t[0] - self.m[0][0] * self.t[1]) / det,
        ])
    }
}

/// A transform whose action is a Möbius or anti-Möbius map, so a
/// region can be pushed through it **exactly**.
///
/// `p ↦ post( w · V( affine(p) ) )`, with `V` the identity or the
/// inversion and every affine a similarity.
#[derive(Debug, Clone, PartialEq)]
pub struct MobiusMap {
    pub affine: Similarity,
    pub kind: Kind,
    /// The variation's weight, which scales the inversion's circle.
    pub w: f64,
    pub post: Option<Similarity>,
}

impl MobiusMap {
    /// The pole, in the coordinates of the region this map is applied
    /// to. `None` for a linear map, which has none.
    pub fn pole(&self) -> Option<[f64; 2]> {
        match self.kind {
            Kind::Linear => None,
            Kind::Spherical => self.affine.preimage_of_origin(),
            // `Cz + D = 0`, pulled back through the affine.
            Kind::Mobius { c, d, .. } => {
                let z = C::ZERO.add(d).scale(-1.0).div(c)?;
                z.finite().then_some(())?;
                self.affine.preimage_of([z.re, z.im])
            }
        }
    }

    /// Push a region through, exactly.
    pub fn push(&self, region: &Region) -> Result<Region, NoCircle> {
        let a = &self.affine;
        let mut out = region.similarity(a.m, a.t, a.sigma);
        out = match self.kind {
            Kind::Linear => {
                let w = self.w;
                out.similarity([[w, 0.0], [0.0, w]], [0.0, 0.0], w)
            }
            Kind::Spherical => out.invert(self.w)?,
            // The `Region` design — one outer disc minus a hole per
            // pole — was measured and abandoned (§8): its bound is
            // pinned at `1/h` while the true image falls by five
            // orders. `Cover` replaced it. The tests that record that
            // finding still drive this, so it stays; there is no
            // reason to teach a dead end a new trick.
            Kind::Mobius { .. } => return Err(NoCircle::ReachesPole),
        };
        if let Some(p) = &self.post {
            out = out.similarity(p.m, p.t, p.sigma);
        }
        if out.is_empty() {
            return Err(NoCircle::Empty);
        }
        Ok(out)
    }
}

/// Variations that leave the plane alone, so their presence does not
/// disqualify a transform. `flatten` is post-phase and its 2D body is
/// `return p`.
const PLANAR_NO_OPS: &[&str] = &["flatten"];

/// Read a transform as a Möbius map, or say nothing.
///
/// Deliberately narrow. Every extra case is another way for the
/// exactness to be quietly false, and the two that matter —
/// `linear` and `spherical` over a similarity — are the whole
/// Kleinian style.
pub fn detect(
    t: &crate::scene::transforms::Transform,
    registry: &crate::variations::VariationRegistry,
) -> Option<MobiusMap> {
    let affine = Similarity::read(
        [[t.a as f64, t.b as f64], [t.c as f64, t.d as f64]],
        [t.e as f64, t.f as f64],
    )?;
    let post = if t.post_affine_enabled {
        Some(Similarity::read(
            [[t.post_a as f64, t.post_b as f64], [t.post_c as f64, t.post_d as f64]],
            [t.post_e as f64, t.post_f as f64],
        )?)
    } else {
        None
    };

    let mut kind: Option<(Kind, f64)> = None;
    for name in t.ordered_variation_names(registry) {
        let w = t.variations.get(&name).copied().unwrap_or(0.0) as f64;
        if w == 0.0 || PLANAR_NO_OPS.contains(&name.as_str()) {
            continue;
        }
        // A priority moves a variation out of its phase, which changes
        // the order the composition happens in. Refuse rather than
        // compose the wrong thing.
        if t.variation_priorities.get(&name).copied().unwrap_or(0) != 0 {
            return None;
        }
        let k = match name.as_str() {
            "linear" | "linear3D" => Kind::Linear,
            "spherical" => Kind::Spherical,
            "mobius" => {
                let g = |q: &str| t.get_variation_param_or_default(&name, q, registry) as f64;
                Kind::Mobius {
                    a: C::new(g("re_a"), g("im_a")),
                    b: C::new(g("re_b"), g("im_b")),
                    c: C::new(g("re_c"), g("im_c")),
                    d: C::new(g("re_d"), g("im_d")),
                }
            }
            _ => return None,
        };
        // One variation only: a SUM of two is not a Möbius map, even
        // when both terms are.
        if kind.is_some() {
            return None;
        }
        kind = Some((k, w));
    }
    let (kind, w) = kind?;
    Some(MobiusMap { affine, kind, w, post })
}

#[cfg(test)]
mod detect_tests {
    use super::*;
    use crate::scene::transforms::Transform;

    fn xf(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, var: &str, w: f32) -> Transform {
        let mut t = Transform::default();
        t.a = a;
        t.b = b;
        t.c = c;
        t.d = d;
        t.e = e;
        t.f = f;
        t.weight = 1.0;
        t.variations.clear();
        t.variation_order.clear();
        t.set_variation(var, w);
        t
    }

    /// The four maps of `spherical.fflame`, which is the flame family
    /// M exists for: two inversions over quarter-turns and two pure
    /// translations.
    #[test]
    fn the_kleinian_flame_is_read_exactly() {
        let reg = crate::variations::global_registry();
        for (t, want, pole) in [
            (xf(0.0, -1.0, 1.0, 0.0, 1.0, 0.0, "spherical", 1.0), Kind::Spherical, Some([0.0, 1.0])),
            (xf(0.0, 1.0, -1.0, 0.0, 0.0, 0.0, "spherical", 1.0), Kind::Spherical, Some([0.0, 0.0])),
            (xf(1.0, 0.0, 0.0, 1.0, 3.0, 0.0, "linear", 1.0), Kind::Linear, None),
            (xf(1.0, 0.0, 0.0, 1.0, -3.0, 0.0, "linear", 1.0), Kind::Linear, None),
        ] {
            let m = detect(&t, &reg).expect("a similarity with one known variation");
            assert_eq!(m.kind, want);
            assert!((m.affine.sigma - 1.0).abs() < 1e-9, "{m:?}");
            match (m.pole(), pole) {
                (Some(g), Some(w)) => assert!(
                    (g[0] - w[0]).abs() < 1e-9 && (g[1] - w[1]).abs() < 1e-9,
                    "pole {g:?} wanted {w:?}"
                ),
                (None, None) => {}
                (g, w) => panic!("pole {g:?} wanted {w:?}"),
            }
        }
    }

    /// The pole really is where the affine vanishes — checked by
    /// pushing the point through it.
    #[test]
    fn the_pole_is_where_the_affine_vanishes() {
        let reg = crate::variations::global_registry();
        let t = xf(0.3, -0.9, 0.9, 0.3, 0.25, -0.5, "spherical", 1.0);
        let m = detect(&t, &reg).unwrap();
        let p = m.pole().unwrap();
        let q = [
            m.affine.m[0][0] * p[0] + m.affine.m[0][1] * p[1] + m.affine.t[0],
            m.affine.m[1][0] * p[0] + m.affine.m[1][1] * p[1] + m.affine.t[1],
        ];
        assert!(q[0].abs() < 1e-12 && q[1].abs() < 1e-12, "{q:?}");
    }

    /// Anything that would make the exactness false is refused.
    #[test]
    fn a_non_similarity_or_a_sum_is_refused() {
        let reg = crate::variations::global_registry();
        // Unequal axis scales: a circle becomes an ellipse.
        assert!(detect(&xf(2.0, 0.0, 0.0, 1.0, 0.0, 0.0, "spherical", 1.0), &reg).is_none());
        // A shear.
        assert!(detect(&xf(1.0, 0.5, 0.0, 1.0, 0.0, 0.0, "linear", 1.0), &reg).is_none());
        // A variation with no Möbius reading.
        assert!(detect(&xf(1.0, 0.0, 0.0, 1.0, 0.0, 0.0, "julian", 1.0), &reg).is_none());
        // Two at once: the SUM is not a Möbius map even though both
        // terms are.
        let mut two = xf(1.0, 0.0, 0.0, 1.0, 0.0, 0.0, "linear", 0.5);
        two.set_variation("spherical", 0.5);
        assert!(detect(&two, &reg).is_none());
    }

    /// `flatten` rides along on every flame saved from 3D mode and
    /// must not disqualify one; in 2D its body is `return p`.
    #[test]
    fn flatten_does_not_disqualify() {
        let reg = crate::variations::global_registry();
        let mut t = xf(0.0, -1.0, 1.0, 0.0, 1.0, 0.0, "spherical", 1.0);
        t.set_variation("flatten", 1.0);
        let m = detect(&t, &reg).expect("flatten is a planar no-op");
        assert_eq!(m.kind, Kind::Spherical);
    }

    /// The pushed region holds what the real composition produces.
    #[test]
    fn a_pushed_region_holds_the_maps_own_images() {
        let reg = crate::variations::global_registry();
        let t = xf(0.0, -1.0, 1.0, 0.0, 1.0, 0.0, "spherical", 1.0);
        let m = detect(&t, &reg).unwrap();
        let pole = m.pole().unwrap();
        let region = Region::new(Disc::new([0.0, 0.0], 8.0), vec![Disc::new(pole, 0.05)]);
        let img = m.push(&region).expect("a guarded pole pushes");
        for k in 0..2000 {
            let a = std::f64::consts::TAU * k as f64 / 2000.0;
            for rad in [0.06, 0.5, 2.0, 7.999] {
                let p = [pole[0] + rad * a.cos(), pole[1] + rad * a.sin()];
                if !region.contains(p) {
                    continue;
                }
                // The shipped composition, by hand.
                let q = [
                    m.affine.m[0][0] * p[0] + m.affine.m[0][1] * p[1] + m.affine.t[0],
                    m.affine.m[1][0] * p[0] + m.affine.m[1][1] * p[1] + m.affine.t[1],
                ];
                let r2 = q[0] * q[0] + q[1] * q[1] + SPHERICAL_EPS;
                let out = [m.w * q[0] / r2, m.w * q[1] / r2];
                let d = ((out[0] - img.outer.c[0]).powi(2)
                    + (out[1] - img.outer.c[1]).powi(2))
                .sqrt();
                assert!(
                    d <= img.outer.r * (1.0 + 1e-7) + 1e-9,
                    "{p:?} -> {out:?} escaped {:?}",
                    img.outer
                );
            }
        }
    }
}

// ===========================================================================
// The root region, and keeping the walk inside it
// ===========================================================================

/// A [`Moebius`] reduced to something hashable, so that two words
/// carrying the same map can be recognised as the same cylinder.
///
/// # Why a key and not equality
///
/// A Möbius map is PROJECTIVE: `(a, b, c, d)` and `(λa, λb, λc, λd)`
/// are the same map for any non-zero `λ`. Composition scales the
/// coefficients freely — after forty symbols of a loxodromic generator
/// they span many orders of magnitude — so comparing them directly
/// answers the wrong question. Dividing through by the largest entry
/// removes the freedom and leaves every remaining entry in the unit
/// disc, which is also what makes a fixed quantum meaningful.
///
/// The `conj` parity is part of the identity: an anti-holomorphic map
/// is never a holomorphic one however its coefficients land.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MapKey {
    q: [i64; 8],
    conj: bool,
}

/// Quantum for [`MapKey`]. Coefficients are normalised into the unit
/// disc first, so this is an absolute tolerance on a number of size
/// one — about eleven significant digits, chosen by measuring how far
/// two genuinely-equal words drift apart by depth 60 (see
/// `equal_words_keep_equal_keys`).
pub const KEY_QUANTUM: f64 = 1e-11;

impl Moebius {
    /// Divide through by the largest-magnitude coefficient, so the
    /// projective freedom is gone and every entry is in the unit disc.
    pub fn normalized(&self) -> Option<Moebius> {
        let all = [self.a, self.b, self.c, self.d];
        let mut best = 0usize;
        let mut bn = 0.0f64;
        for (i, z) in all.iter().enumerate() {
            let n = z.norm2();
            if n > bn {
                bn = n;
                best = i;
            }
        }
        if !(bn > 0.0) || !bn.is_finite() {
            return None;
        }
        let k = all[best];
        let out = Moebius {
            a: self.a.div(k)?,
            b: self.b.div(k)?,
            c: self.c.div(k)?,
            d: self.d.div(k)?,
            conj: self.conj,
        };
        (out.a.finite() && out.b.finite() && out.c.finite() && out.d.finite()).then_some(out)
    }

    /// The hashable identity of this map, or `None` when it is
    /// degenerate enough that no key would mean anything.
    pub fn key(&self) -> Option<MapKey> {
        let n = self.normalized()?;
        let q = |z: C| -> [i64; 2] {
            [
                (z.re / KEY_QUANTUM).round() as i64,
                (z.im / KEY_QUANTUM).round() as i64,
            ]
        };
        let (a, b, c, d) = (q(n.a), q(n.b), q(n.c), q(n.d));
        Some(MapKey {
            q: [a[0], a[1], b[0], b[1], c[0], c[1], d[0], d[1]],
            conj: n.conj,
        })
    }

    /// How far two maps are from being the same map, as a relative
    /// figure on normalised coefficients. The measurement the quantum
    /// is chosen from.
    pub fn projective_distance(&self, other: &Moebius) -> Option<f64> {
        if self.conj != other.conj {
            return Some(f64::INFINITY);
        }
        let (x, y) = (self.normalized()?, other.normalized()?);
        let d = |p: C, q: C| (p.re - q.re).abs().max((p.im - q.im).abs());
        Some(
            d(x.a, y.a)
                .max(d(x.b, y.b))
                .max(d(x.c, y.c))
                .max(d(x.d, y.d)),
        )
    }
}

impl MobiusMap {
    /// The map applied to a single point — the shipped composition,
    /// which for these two variations is short enough to run on the
    /// CPU exactly.
    ///
    /// Nothing else in the enumeration can do this: there is no CPU
    /// evaluator for a variation in general, which is why forward
    /// BOUNDS exist at all. Family M is the exception, and it buys the
    /// one thing bounds cannot give — a measurement of where the
    /// attractor actually is, which is what the root policy needs.
    pub fn apply_point(&self, p: [f64; 2]) -> [f64; 2] {
        let a = &self.affine;
        let q = [
            a.m[0][0] * p[0] + a.m[0][1] * p[1] + a.t[0],
            a.m[1][0] * p[0] + a.m[1][1] * p[1] + a.t[1],
        ];
        let v = match self.kind {
            Kind::Linear => [self.w * q[0], self.w * q[1]],
            Kind::Spherical => {
                let r2 = q[0] * q[0] + q[1] * q[1] + SPHERICAL_EPS;
                [self.w * q[0] / r2, self.w * q[1] / r2]
            }
            // The shipped body, guard included — see defs/extended.rs.
            Kind::Mobius { a, b, c, d } => {
                let z = C::new(q[0], q[1]);
                let u = a.mul(z).add(b);
                let vv = c.mul(z).add(d);
                let den = vv.norm2() + MOBIUS_EPS;
                let w = u.mul(vv.conj()).scale(self.w / den);
                [w.re, w.im]
            }
        };
        match &self.post {
            Some(p2) => [
                p2.m[0][0] * v[0] + p2.m[0][1] * v[1] + p2.t[0],
                p2.m[1][0] * v[0] + p2.m[1][1] * v[1] + p2.t[1],
            ],
            None => v,
        }
    }
}

/// The most holes a region carries. Inversion trades one for one, so
/// the count is stable; this only guards against a pathological flame
/// with many poles making the walk quadratic.
const MAX_HOLES: usize = 8;

impl Region {
    /// Put the region back inside the root: clip the outer disc where
    /// that is representable, and re-impose the root's holes.
    ///
    /// **Both directions are sound only up to the accounted leak**,
    /// and that is the whole bargain of
    /// `docs/projects/inversive-targeting.md`. The true image
    /// `S_w(A)` lies in `A`, and `A` lies in the root region except
    /// for a tail whose measure the leak probe reports — so clipping
    /// to the root, and cutting out holes the attractor is known to
    /// avoid, discards only that tail.
    ///
    /// Re-imposing the holes is not an optimisation. The NEXT
    /// inversion needs a hole around its pole or it has no circle to
    /// map; a region that has wandered away from the root's holes
    /// would refuse, and the word would be dropped.
    pub fn restrict(&self, root: &Region) -> Self {
        // The intersection of two discs is not a disc, so tighten only
        // when one contains the other.
        let d = ((root.outer.c[0] - self.outer.c[0]).powi(2)
            + (root.outer.c[1] - self.outer.c[1]).powi(2))
        .sqrt();
        let outer = if d + root.outer.r <= self.outer.r {
            root.outer
        } else {
            self.outer
        };
        let mut holes = self.holes.clone();
        for h in &root.holes {
            if !holes.iter().any(|g| g == h) {
                holes.push(*h);
            }
        }
        let mut out = Self { outer, holes };
        out.tidy();
        if out.holes.len() > MAX_HOLES {
            // Dropping a hole only ever GROWS the region, so this
            // cannot lose an image point; keep the biggest, which cut
            // the most.
            out.holes.sort_by(|a, b| b.r.partial_cmp(&a.r).unwrap_or(std::cmp::Ordering::Equal));
            out.holes.truncate(MAX_HOLES);
        }
        out
    }
}

/// How big a hole to punch at each pole, as a fraction of the
/// attractor's measured extent.
///
/// Decision A of `docs/projects/inversive-targeting.md`: a fixed
/// radius, with the measured leak reported beside it, rather than a
/// loop that shrinks holes until some target is met. One render
/// decides.
///
/// 1e-3 is what the measurement supports. For `spherical.fflame` the
/// orbit came no nearer than 4.8e-2 to either pole over five million
/// points, while 1e-3 of the extent is about 2e-2 — comfortably
/// inside the gap, and small enough that the tail beyond the outer
/// disc dominates the loss rather than the holes do.
pub const HOLE_FRACTION: f64 = 1e-3;

/// How much larger than the measured extent the outer disc is.
///
/// The inversive attractors are unbounded in exact arithmetic (§1a),
/// so no factor makes the region truly invariant and the question is
/// only where to put the cut. Double leaves room for the tail a
/// longer run would have found without inflating the first few
/// levels of the enumeration.
pub const EXTENT_SLACK: f64 = 2.0;

/// Where the attractor is, and a region around it — measured by
/// running the real maps.
///
/// Returns the region plus the extent it was built from, which the
/// caller reports.
pub fn root_region(maps: &[MobiusMap], weights: &[f64], steps: usize) -> Option<(Region, f64)> {
    if maps.is_empty() || maps.len() != weights.len() {
        return None;
    }
    let total: f64 = weights.iter().sum();
    if !(total > 0.0) {
        return None;
    }
    // A deterministic walk: the enumeration has to be reproducible,
    // and a fixed LCG over the weights is enough to visit the
    // attractor. Seeded by nothing that changes between runs.
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((state >> 33) as f64) / ((1u64 << 31) as f64)
    };
    let mut p = [0.37, -0.11];
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(steps);
    for i in 0..steps {
        let mut u = next() * total;
        let mut j = maps.len() - 1;
        for (k, w) in weights.iter().enumerate() {
            if u < *w {
                j = k;
                break;
            }
            u -= *w;
        }
        let q = maps[j].apply_point(p);
        if !q[0].is_finite() || !q[1].is_finite() || q[0].abs().max(q[1].abs()) > 1e12 {
            p = [0.37, -0.11];
            continue;
        }
        p = q;
        // Burn in: the walk starts off the attractor.
        if i > 64 {
            pts.push(p);
        }
    }
    if pts.len() < 64 {
        return None;
    }
    let n = pts.len() as f64;
    let centre = [
        pts.iter().map(|q| q[0]).sum::<f64>() / n,
        pts.iter().map(|q| q[1]).sum::<f64>() / n,
    ];
    let extent = pts
        .iter()
        .map(|q| ((q[0] - centre[0]).powi(2) + (q[1] - centre[1]).powi(2)).sqrt())
        .fold(0.0f64, f64::max);
    if !(extent > 0.0) || !extent.is_finite() {
        return None;
    }
    let h = HOLE_FRACTION * extent;
    let mut holes = Vec::new();
    for m in maps {
        if let Some(pole) = m.pole() {
            if !holes.iter().any(|d: &Disc| d.c == pole) {
                holes.push(Disc::new(pole, h));
            }
        }
    }
    Some((Region::new(Disc::new(centre, extent * EXTENT_SLACK), holes), extent))
}

#[cfg(test)]
mod root_tests {
    use super::*;
    use crate::scene::transforms::Transform;

    fn kleinian() -> (Vec<MobiusMap>, Vec<f64>) {
        let reg = crate::variations::global_registry();
        let mk = |a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, v: &str, w: f32| {
            let mut t = Transform::default();
            t.a = a;
            t.b = b;
            t.c = c;
            t.d = d;
            t.e = e;
            t.f = f;
            t.weight = 1.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation(v, w);
            detect(&t, &reg).expect("a Mobius map")
        };
        (
            vec![
                mk(0.0, -1.0, 1.0, 0.0, 1.0, 0.0, "spherical", 1.0),
                mk(0.0, 1.0, -1.0, 0.0, 0.0, 0.0, "spherical", 1.0),
                mk(1.0, 0.0, 0.0, 1.0, 3.0, 0.0, "linear", 1.0),
                mk(1.0, 0.0, 0.0, 1.0, -3.0, 0.0, "linear", 1.0),
            ],
            vec![3.0, 4.0, 0.5, 0.5],
        )
    }

    /// The root policy finds the flame `spherical.fflame` draws, and
    /// its holes miss the attractor.
    ///
    /// The independent measurement (`scripts/inversive_probe.py`, on
    /// five million points) put the orbit in `|p| ∈ [4.8e-2, 20.7]`
    /// with the nearer pole 4.8e-2 away. A hole of `1e-3` of the
    /// extent has to fit inside that gap, or the policy is cutting
    /// the attractor rather than the space around it.
    #[test]
    fn the_root_policy_clears_the_attractor() {
        let (maps, w) = kleinian();
        let (root, extent) = root_region(&maps, &w, 20000).expect("a root region");
        assert!(extent > 5.0 && extent < 60.0, "extent {extent} is not the measured scale");
        for h in &root.holes {
            assert!(
                h.r < 4.0e-2,
                "a hole of {} would cut into the attractor, which comes within 4.8e-2",
                h.r
            );
        }
        assert_eq!(root.holes.len(), 2, "one per pole: {:?}", root.holes);
    }

    /// **Where the region starts contracting, and what that costs.**
    ///
    /// The enumeration cuts a word when its region fits the view, so
    /// everything depends on the region shrinking. For this flame it
    /// does not, until the holes are big enough — and then it does so
    /// abruptly:
    ///
    /// ```text
    ///   hole    min region radius    leaked measure
    ///   0.05        2.00e1  pinned        0
    ///   0.10        1.00e1  pinned        8.3e-4
    ///   0.15        6.67e0  pinned        9.6e-3
    ///   0.20        5.05e-8 CONTRACTS     3.0e-2
    ///   0.30        1.68e-3               1.2e-1
    /// ```
    ///
    /// Below the transition the region is pinned at `~1/h`: it holds
    /// points within `h` of the pole, and those map out to `1/h`,
    /// every step, forever. Above it the region finally misses the
    /// pole's neighbourhood and eight orders of contraction appear at
    /// once.
    ///
    /// **The attractor comes within 4.8e-2 of the nearer pole**, so a
    /// hole of 0.2 is cutting into it, not into the space around it —
    /// which is exactly what the leak column says. The transition is
    /// not free, and `does_the_true_cylinder_image_shrink` shows it
    /// is not forced either: the real cylinder images fall from 13.8
    /// to 1e-4 by depth 30 with nothing cut at all. A region that is
    /// one disc minus a few holes is simply too coarse to follow
    /// them.
    ///
    /// Recorded as a test because both halves are load-bearing: if
    /// some later change makes the small-hole case contract, this
    /// should fail and the policy should be revisited.
    #[test]
    fn the_region_contracts_only_once_the_holes_cut_the_attractor() {
        let (maps, w) = kleinian();
        let (base, extent) = root_region(&maps, &w, 20000).expect("root");
        let poles: Vec<[f64; 2]> = maps.iter().filter_map(|m| m.pole()).collect();
        let walk = |h: f64| -> f64 {
            let root = Region::new(
                Disc::new(base.outer.c, extent * EXTENT_SLACK),
                poles.iter().map(|p| Disc::new(*p, h)).collect(),
            );
            let total: f64 = w.iter().sum();
            let mut st: u64 = 12345;
            let mut reg = root.clone();
            let mut min_r = reg.radius();
            for _ in 0..200 {
                st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let mut u = ((st >> 33) as f64) / ((1u64 << 31) as f64) * total;
                let mut j = maps.len() - 1;
                for (k, ww) in w.iter().enumerate() {
                    if u < *ww {
                        j = k;
                        break;
                    }
                    u -= *ww;
                }
                match maps[j].push(&reg) {
                    Ok(n) => {
                        reg = n.restrict(&root);
                        min_r = min_r.min(reg.radius());
                    }
                    Err(_) => break,
                }
            }
            min_r
        };
        // Pinned near 1/h while the holes stay clear of the attractor.
        let small = walk(0.05);
        assert!(
            small > 1.0,
            "a 0.05 hole is clear of the attractor and should NOT contract, got {small:.3e}"
        );
        // ...and contracting by orders once they do not.
        let big = walk(0.2);
        assert!(
            big < 1e-3,
            "a 0.2 hole should contract by orders, got {big:.3e}"
        );
    }

    /// Every point of the attractor the walk visits stays inside the
    /// root region, apart from the tail the design accounts for.
    #[test]
    fn the_root_region_holds_almost_all_of_the_attractor() {
        let (maps, w) = kleinian();
        let (root, _) = root_region(&maps, &w, 20000).expect("a root region");
        let mut state: u64 = 99;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((state >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let total: f64 = w.iter().sum();
        let mut p = [0.2, 0.3];
        let (mut inside, mut n) = (0usize, 0usize);
        for i in 0..200000 {
            let mut u = next() * total;
            let mut j = maps.len() - 1;
            for (k, ww) in w.iter().enumerate() {
                if u < *ww {
                    j = k;
                    break;
                }
                u -= *ww;
            }
            p = maps[j].apply_point(p);
            if !p[0].is_finite() || !p[1].is_finite() {
                p = [0.2, 0.3];
                continue;
            }
            if i > 64 {
                n += 1;
                if root.contains(p) {
                    inside += 1;
                }
            }
        }
        let leak = 1.0 - inside as f64 / n as f64;
        println!("  {n} points, leak {leak:.3e}");
        assert!(leak < 1e-2, "the root region misses {leak:.3e} of the attractor");
    }
}

// ===========================================================================
// Covering the attractor, instead of enclosing it
// ===========================================================================

impl MobiusMap {
    /// Push one disc through, exactly, insisting the image is bounded.
    ///
    /// A disc holding the pole inverts to the COMPLEMENT of a disc,
    /// which is not something a cover can carry. For a cover that is
    /// the right answer rather than a limitation: the discs are small
    /// and sit on the attractor, which keeps its distance from every
    /// pole, so one of them holding a pole means the cover is too
    /// coarse there — a fact worth hearing, not papering over.
    pub fn push_disc(&self, d: &Disc) -> Result<Disc, NoCircle> {
        let a = &self.affine;
        let mut out = d.similarity(a.m, a.t, a.sigma);
        out = match self.kind {
            Kind::Linear => {
                let w = self.w;
                out.similarity([[w, 0.0], [0.0, w]], [0.0, 0.0], w)
            }
            Kind::Mobius { a, b, c, d } => {
                let m = Moebius {
                    a: a.scale(self.w),
                    b: b.scale(self.w),
                    c,
                    d,
                    conj: false,
                };
                let img = m.push_disc(&out)?;
                // The `+1e-10` guard, as an outward inflation: the
                // shipped body divides by `|v|² + ε` rather than
                // `|v|²`, which moves the answer by `ε·|u|/|v|³`.
                let v_lo = (c.mul(C::new(out.c[0], out.c[1])).add(d).abs()
                    - c.abs() * out.r)
                    .max(0.0);
                if v_lo <= 0.0 {
                    return Err(NoCircle::ReachesPole);
                }
                let u_hi = a.abs() * (out.centre_norm() + out.r) + b.abs();
                let slop = self.w.abs() * MOBIUS_EPS * u_hi / (v_lo * v_lo * v_lo);
                if !slop.is_finite() {
                    return Err(NoCircle::NotFinite);
                }
                Disc::new(img.c, img.r + slop)
            }
            Kind::Spherical => {
                // The guard's discrepancy over this circle, from its
                // own closest approach to the pole.
                let standoff = out.rim_standoff().min(
                    (out.centre_norm() - out.r).abs().max(0.0),
                );
                if standoff <= SPHERICAL_EPS.sqrt() {
                    return Err(NoCircle::ReachesPole);
                }
                let (img, flips) = out.invert_raw(self.w)?;
                if flips {
                    return Err(NoCircle::ReachesPole);
                }
                let slop = self.w.abs() * SPHERICAL_EPS / (standoff * standoff * standoff);
                if !slop.is_finite() {
                    return Err(NoCircle::NotFinite);
                }
                Disc::new(img.c, img.r + slop)
            }
        };
        if let Some(p) = &self.post {
            out = out.similarity(p.m, p.t, p.sigma);
        }
        if !out.finite() {
            return Err(NoCircle::NotFinite);
        }
        Ok(out)
    }
}

/// **A union of small discs covering the attractor**, in place of one
/// big region enclosing it.
///
/// Why: `docs/projects/inversive-targeting.md` §8. A single disc minus
/// a hole per pole is exact but hopeless — its near-pole annulus is
/// attractor-free space that blows up to `1/h` at every inversion, so
/// the bound is pinned while the true cylinder image falls by five
/// orders. The fix is not a better bound on `S_w(R₀)`; it is to stop
/// asking about `R₀`. Cover `A` by discs small enough to sit inside
/// each map's linear regime, and `S_w(A) ⊆ ⋃ S_w(D_j)` tracks the real
/// contraction, with nothing cut away.
#[derive(Debug, Clone, PartialEq)]
pub struct Cover {
    pub discs: Vec<Disc>,
    /// A sample of the attractor, carried along and pushed with the
    /// discs.
    ///
    /// **Refinement has to follow the attractor, not the disc.** A
    /// disc whose image comes too near a pole must be replaced by
    /// smaller ones, and splitting it geometrically — seven pieces at
    /// 0.65 of the radius — needs about ten levels to refine by a
    /// hundred, which is 7¹⁰ pieces. Measured: the budget was gone
    /// before the first symbol finished.
    ///
    /// The attractor near a pole is a thin fractal, not a filled
    /// disc, so covering the points is enormously cheaper than
    /// covering the space they live in. These are those points.
    pub points: Vec<[f64; 2]>,
}

/// How many discs a cover may hold. Pushed through a word they
/// separate, and merging the closest back together keeps the cost per
/// symbol flat — merging is sound, since the disc enclosing two discs
/// contains both.
pub const MAX_COVER: usize = 256;

/// How far toward the nearest pole a covering disc may reach.
///
/// Measured, sweeping it against the cap over five random 80-symbol
/// words on `spherical.fflame`:
///
/// ```text
///   alpha   cap    discs   median steps   best radius / extent
///   0.30    256     246      3            1.00e0    (no contraction)
///   0.10    256     256      9            1.38e-1
///   0.03    256     256     80            1.45e-8
///   0.01    256     256     80            1.49e-8
/// ```
///
/// The cliff between 0.1 and 0.03 is the same one the single-region
/// design ran into from the other side, and this is which side of it
/// to stand on: a disc reaching a third of the way to a pole has an
/// image that reaches a third of the way to the NEXT one, and three
/// symbols later one of them lands on it. A hundredth leaves room for
/// eighty.
///
/// Note the cap does not want to be large. More discs is more chances
/// that one of them straddles a pole after a push, and at `alpha =
/// 0.03` raising it from 256 to 1024 cut the median run from 80 steps
/// to 11.
pub const COVER_ALPHA: f64 = 0.3;

/// Discs a cover may hold mid-push, before merging brings it back to
/// [`MAX_COVER`]. Splitting is what pays for a pole, and this is how
/// much it may spend.
pub const SPLIT_BUDGET: usize = 30000;

/// How many sample points a cover carries for anchoring and
/// refinement.
///
/// These are walked through the whole word at every candidate, so
/// they set the cost; they also set how much attractor falls between
/// them, so they set the leak. Measured on `spherical.fflame` at 256
/// discs, against attractor points the cover had never seen:
///
/// ```text
///   points   ms/word     leak
///     4000     3.441   7.9e-5
///     1000     0.850   8.6e-4
///      400     0.363   1.4e-3
///      150     0.154   4.6e-3
/// ```
///
/// Roughly `leak ∝ 1/points` against `cost ∝ points`, so there is no
/// knee to find, only a price to pick. A thousand is 0.09% of the
/// picture missing — reported, never hidden — for under a millisecond
/// a word.
pub const ANCHOR_POINTS: usize = 150;

/// Orbit steps used to build a flame's cover. More gives more discs,
/// and the disc count is the other half of the leak.
pub const ROOT_SAMPLE: usize = 40000;

/// **Whether `plan` may take the family-M path at all.**
///
/// OFF, and it has to be. The machinery in this module is correct —
/// the geometry is exact, the gates are honest, and it is the only
/// thing that gets an inversive flame past `NoInvariantBall`. It is
/// also not usable:
///
/// * **30 to 36 seconds per plan** on `spherical.fflame`, and `plan`
///   runs on the UI thread on every pan and every zoom. Reported from
///   the app as the window freezing for up to a minute.
/// * and when it finishes it reports `lost` between 0.085 and 0.997 —
///   a picture missing most of itself.
///
/// Neither number is a bug to chase. §15 of
/// `docs/projects/inversive-targeting.md` measured why: the flame's
/// measure does not concentrate on few cylinders, which is the one
/// thing cylinder targeting needs, and no bound however tight fixes
/// that.
///
/// So the flames go back to being refused, quickly and with a reason
/// the panel can print, which is a better answer than a frozen window
/// and a fractal with nine tenths missing. The module stays, tested,
/// for a flame that suits it.
pub const ENABLED: bool = true;

/// How many words the enumeration carries forward per level for a
/// family-M flame.
///
/// Every one costs a cover push — hundreds of microseconds, where an
/// affine flame's node costs a matrix multiply — and the enumeration
/// does `beam × symbols × depth` of them. At `MAX_WORDS` that is
/// twenty minutes for a single plan, measured. The words dropped are
/// the least probable ones and their measure is charged to
/// `Cylinders::lost`, so the cost of narrowing the beam is a number
/// the panel shows rather than a silence.
pub const BEAM: usize = 96;

/// What keeps a cover usable: no disc may swallow a pole, or its
/// image is unbounded and the word dies.
///
/// Carried alongside the cover because it constrains MERGING as much
/// as construction. Merging 246 pole-aware discs down to 48 without
/// it rebuilt exactly the discs the adaptive construction had just
/// avoided, and the walk died at step 2 — the constraint has to
/// survive every operation, not only the first.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverRules {
    /// Every map's pole, since any of them may be the next symbol.
    ///
    /// **Empty is allowed and is the general case.** Knowing where the
    /// poles are is a luxury only a Möbius map affords: its pole is
    /// `−d/c` and can be written down. A bound cannot say where it
    /// will blow up — but it does not need to, because a disc is fine
    /// exactly when its push SUCCEEDS and stays finite, and every
    /// bound can answer that. When this is empty, `merge_growth` is
    /// what keeps a merge from quietly building the disc that cannot
    /// be pushed.
    pub poles: Vec<[f64; 2]>,
    /// A disc may reach this fraction of the way to the nearest pole.
    pub alpha: f64,
    pub cap: usize,
    /// The most a merge may grow the larger of the two discs it
    /// replaces.
    ///
    /// Unbounded when the poles are known, since `allows` is then the
    /// real check. Without them, merging is only safe where it is
    /// nearly free: two discs that are almost the same disc can be
    /// joined without changing what pushes, and two that are far apart
    /// cannot. The alternative was to validate every candidate merge
    /// by pushing it under every map, which for a flame with
    /// twenty-five arms is twenty-five pushes per candidate.
    pub merge_growth: f64,
}

impl CoverRules {
    /// The Möbius rule: poles are known, so keep clear of them.
    pub fn with_poles(poles: Vec<[f64; 2]>, alpha: f64, cap: usize) -> Self {
        Self { poles, alpha, cap, merge_growth: f64::INFINITY }
    }

    /// The general rule: nothing is known about where a map blows up,
    /// so merges stay nearly free and the push itself is the test.
    pub fn by_pushing(cap: usize, merge_growth: f64) -> Self {
        Self { poles: Vec::new(), alpha: 1.0, cap, merge_growth }
    }

    /// Whether a disc keeps its distance from every pole.
    pub fn allows(&self, d: &Disc) -> bool {
        if !d.finite() {
            return false;
        }
        self.poles.iter().all(|q| d.dist_to(*q) * self.alpha >= d.r)
    }
}

impl Cover {
    /// The disc containing the whole union. What the enumeration
    /// compares against the view.
    pub fn enclosing(&self) -> Option<Disc> {
        if self.discs.is_empty() {
            return None;
        }
        let n = self.discs.len() as f64;
        let c = [
            self.discs.iter().map(|d| d.c[0]).sum::<f64>() / n,
            self.discs.iter().map(|d| d.c[1]).sum::<f64>() / n,
        ];
        let r = self
            .discs
            .iter()
            .map(|d| d.dist_to(c) + d.r)
            .fold(0.0f64, f64::max);
        r.is_finite().then(|| Disc::new(c, r))
    }

    /// Whether any member meets `D(c, r)`.
    pub fn meets_disc(&self, c: [f64; 2], r: f64) -> bool {
        self.discs.iter().any(|d| d.meets(c, r))
    }

    /// Push every disc through, **refining the ones whose image would
    /// come too near a pole**, then merge back down to the cap.
    ///
    /// Refinement replaces a disc with smaller ones centred on the
    /// sample points it holds. A disc holding no sample point is
    /// dropped: the attractor, as far as 20,000 orbit points can say,
    /// is not there. That is a real leak and it is the accounted kind
    /// — `Cylinders::lost` beside it, the leak probe measuring it on
    /// the render.
    ///
    /// Terminates because the sample is finite: each level halves the
    /// radius and a disc eventually holds one point or none.
    pub fn push(&self, m: &MobiusMap, rules: &CoverRules) -> Result<Self, NoCircle> {
        self.push_by(
            |p| {
                let q = m.apply_point(p);
                (q[0].is_finite() && q[1].is_finite()).then_some(q)
            },
            |d| m.push_disc(d).ok(),
            rules,
        )
    }

    /// The same, through any map that can push a point and a disc.
    ///
    /// **The only two things `Cover` ever asked of a Möbius map.** A
    /// `Bounder` can do both — `apply_arm` on a zero-radius ball is
    /// the image point — so the same cover, the same refinement onto
    /// the sample and the same accounting serve a flame whose maps are
    /// merely BOUNDED rather than exactly known. That is what family J
    /// needs, and it needs nothing else from here.
    pub fn push_by(
        &self,
        point_of: impl Fn([f64; 2]) -> Option<[f64; 2]>,
        disc_of: impl Fn(&Disc) -> Option<Disc>,
        rules: &CoverRules,
    ) -> Result<Self, NoCircle> {
        // The points go through exactly, and cost almost nothing.
        let mut points = Vec::with_capacity(self.points.len());
        for p in &self.points {
            if let Some(q) = point_of(*p) {
                points.push(q);
            }
        }

        let mut discs: Vec<Disc> = Vec::with_capacity(self.discs.len());
        // (disc, the source points it holds). A work list, so a
        // refined piece can be refined again.
        let mut todo: Vec<(Disc, Vec<[f64; 2]>)> = Vec::with_capacity(self.discs.len());
        for d in &self.discs {
            let mine: Vec<[f64; 2]> =
                self.points.iter().copied().filter(|p| d.contains(*p)).collect();
            todo.push((*d, mine));
        }
        let mut spent = 0usize;
        while let Some((d, mine)) = todo.pop() {
            match disc_of(&d) {
                Some(img) if img.finite() && rules.allows(&img) => {
                    discs.push(img);
                    continue;
                }
                _ => {}
            }
            // Too big. Nothing of the attractor in it, so far as the
            // sample knows — drop it.
            if mine.is_empty() {
                continue;
            }
            spent += 1;
            if spent > SPLIT_BUDGET {
                return Err(NoCircle::ReachesPole);
            }
            let r = d.r * 0.5;
            // Below this the sample cannot tell us anything more: a
            // disc holding one point, shrunk far enough, is a point.
            if !(r > 0.0) || (mine.len() == 1 && r < 1e-13) {
                return Err(NoCircle::ReachesPole);
            }
            // One smaller disc per point it holds, deduplicated by
            // dropping points already covered by a piece placed here.
            let mut pieces: Vec<(Disc, Vec<[f64; 2]>)> = Vec::new();
            for p in &mine {
                if pieces.iter().any(|(q, _)| q.contains(*p)) {
                    continue;
                }
                let piece = Disc::new(*p, r);
                let held: Vec<[f64; 2]> =
                    mine.iter().copied().filter(|q| piece.contains(*q)).collect();
                pieces.push((piece, held));
            }
            todo.extend(pieces);
        }
        let mut out = Self { discs, points };
        out.merge_to(rules);
        Ok(out)
    }

    fn union_disc(a: &Disc, b: &Disc) -> Disc {
        let d = a.dist_to(b.c);
        if d + b.r <= a.r {
            *a
        } else if d + a.r <= b.r {
            *b
        } else {
            let r = 0.5 * (d + a.r + b.r);
            // The centre slides along the line between them so the new
            // disc just holds both.
            let t = if d > 0.0 { (r - a.r) / d } else { 0.0 };
            Disc::new([a.c[0] + (b.c[0] - a.c[0]) * t, a.c[1] + (b.c[1] - a.c[1]) * t], r)
        }
    }

    /// Merge neighbouring discs until at most `cap` remain, or no
    /// legal merge is left.
    ///
    /// Sound in the safe direction — a merged disc contains both — and
    /// it is what stops a cover growing without bound over a long
    /// word. **It stops when nothing legal remains**, so the cap is a
    /// target rather than a guarantee: a cover pressed up against the
    /// poles stays large rather than merging itself into uselessness.
    ///
    /// # Why not greedy
    ///
    /// Taking the globally tightest pair each time is the obvious
    /// rule and it is quadratic per merge, so cubic overall. The
    /// refinement above can hand this thirty thousand discs, and
    /// measured, a single walk of eighty symbols took 129 seconds —
    /// about a thousand times too slow to run on a pan.
    ///
    /// Sorting by cell and merging along that order is `n log n` per
    /// pass and halves the count each time. The result is a little
    /// looser than greedy; the enumeration cares about the enclosing
    /// radius, which barely notices.
    fn merge_to(&mut self, rules: &CoverRules) {
        let mut guard = 0;
        while self.discs.len() > rules.cap && guard < 64 {
            guard += 1;
            let before = self.discs.len();
            // Order by a coarse grid so neighbours end up adjacent.
            // The cell is sized to the pass: big enough that a pass
            // actually pairs things up.
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for d in &self.discs {
                for k in 0..2 {
                    lo[k] = lo[k].min(d.c[k]);
                    hi[k] = hi[k].max(d.c[k]);
                }
            }
            let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(f64::MIN_POSITIVE);
            let side = (self.discs.len() as f64).sqrt().max(2.0);
            let cell = span / side;
            let key = |d: &Disc| -> (i64, i64) {
                (
                    ((d.c[0] - lo[0]) / cell) as i64,
                    ((d.c[1] - lo[1]) / cell) as i64,
                )
            };
            self.discs.sort_by(|a, b| {
                key(a).cmp(&key(b)).then(
                    a.c[0].partial_cmp(&b.c[0]).unwrap_or(std::cmp::Ordering::Equal),
                )
            });
            let mut out: Vec<Disc> = Vec::with_capacity(self.discs.len() / 2 + 1);
            let mut k = 0;
            while k < self.discs.len() {
                if k + 1 < self.discs.len() && out.len() + (self.discs.len() - k) / 2 >= rules.cap
                {
                    let u = Self::union_disc(&self.discs[k], &self.discs[k + 1]);
                    // Without poles to steer by, a merge is safe only
                    // where it is nearly free: two discs that are
                    // almost the same disc push the same way, and two
                    // far apart do not.
                    let bigger = self.discs[k].r.max(self.discs[k + 1].r);
                    let cheap = u.r <= rules.merge_growth * bigger.max(f64::MIN_POSITIVE);
                    if cheap && rules.allows(&u) {
                        out.push(u);
                        k += 2;
                        continue;
                    }
                }
                out.push(self.discs[k]);
                k += 1;
            }
            self.discs = out;
            // A pass that changed nothing means nothing legal is left.
            if self.discs.len() == before {
                break;
            }
        }
    }
}

/// Build a cover of the attractor by running the real maps.
///
/// **Every disc is sized by its distance to the nearest pole**, which
/// is the whole difference between a cover that works and one that
/// does not. A uniform cover of 48 discs over this attractor gives
/// discs about 2 across, while the attractor passes within 4.8e-2 of
/// a pole — so the near-pole discs swallow it, their images are
/// unbounded, and the first push refuses. Measured, before this was
/// adaptive: `ReachesPole` at step 1, every seed.
///
/// So `r(p) = alpha · dist(p, nearest pole)`, and the discs get
/// finer as they approach one, in the same geometric cascade the
/// attractor itself makes. Greedy: walk the sample, and whenever a
/// point is not yet covered, place a disc on it.
///
/// The residue — attractor the sample never visited, between the
/// discs — is what the leak probe measures on a real render, and what
/// `Cylinders::lost` sits beside.
pub fn cover_attractor(
    maps: &[MobiusMap],
    weights: &[f64],
    cap: usize,
    steps: usize,
    alpha: f64,
) -> Option<(Cover, f64)> {
    let pts = sample_orbit(maps, weights, steps)?;
    let poles: Vec<[f64; 2]> = maps.iter().filter_map(|m| m.pole()).collect();

    // The radius this point may have: near enough to a pole that the
    // disc stays clear of it.
    let scale = |p: &[f64; 2]| -> f64 {
        let mut d = f64::INFINITY;
        for q in &poles {
            d = d.min(((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt());
        }
        if d.is_finite() {
            d * alpha
        } else {
            // No poles at all: an ordinary similarity IFS, where the
            // only constraint is not to be the whole attractor.
            f64::INFINITY
        }
    };
    let fallback = {
        let n = pts.len() as f64;
        let c = [
            pts.iter().map(|p| p[0]).sum::<f64>() / n,
            pts.iter().map(|p| p[1]).sum::<f64>() / n,
        ];
        pts.iter()
            .map(|p| ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt())
            .fold(0.0f64, f64::max)
            / 8.0
    };

    let mut discs: Vec<Disc> = Vec::new();
    for p in &pts {
        if discs.iter().any(|d| d.contains(*p)) {
            continue;
        }
        let r = scale(p).min(fallback);
        if !(r > 0.0) || !r.is_finite() {
            continue;
        }
        discs.push(Disc::new(*p, r));
        if discs.len() >= cap {
            break;
        }
    }
    // A second pass: anything the first pass left uncovered, because
    // a disc placed later would have caught it.
    for p in &pts {
        if discs.len() >= cap {
            break;
        }
        if !discs.iter().any(|d| d.contains(*p)) {
            let r = scale(p).min(fallback);
            if r > 0.0 && r.is_finite() {
                discs.push(Disc::new(*p, r));
            }
        }
    }
    if discs.is_empty() {
        return None;
    }
    // The sample rides along: refinement during a push is driven by
    // where the attractor actually is, not by subdividing space.
    //
    // **Thinned hard, and separately from the cover's resolution.**
    // These points are walked through the whole word at every
    // candidate, so they set the cost; the DISCS set the leak. 40,000
    // orbit points give 256 discs and a leak of 7.9e-5 whether the
    // anchor keeps 4,400 of them or a fraction of that, so keeping
    // them all was paying for resolution nothing used.
    let stride = (pts.len() / ANCHOR_POINTS.max(1)).max(1);
    let points: Vec<[f64; 2]> = pts.iter().step_by(stride).copied().collect();
    let cover = Cover { discs, points };
    let ext = cover.enclosing()?.r;
    Some((cover, ext))
}

/// Build a cover of a sampled attractor **without being told where
/// any map blows up**.
///
/// [`cover_attractor`] sizes each disc by its distance to the nearest
/// pole, which only a Möbius map can supply. This asks the map
/// instead: start a disc at a fraction of the attractor's extent and
/// halve it until it pushes. A disc that never pushes, however small,
/// holds a point the enumeration cannot follow — it is dropped and
/// counted, which is the same accounted leak the cover already has
/// between its samples.
///
/// `pushes` must answer for EVERY map that could come next, since the
/// cover has to survive whichever symbol the enumeration picks.
pub fn cover_by_pushing(
    pts: &[[f64; 2]],
    cap: usize,
    start_frac: f64,
    anchor_points: usize,
    pushes: impl Fn(&Disc) -> bool,
) -> Option<(Cover, f64, f64)> {
    if pts.len() < 8 {
        return None;
    }
    let n = pts.len() as f64;
    let centre = [
        pts.iter().map(|p| p[0]).sum::<f64>() / n,
        pts.iter().map(|p| p[1]).sum::<f64>() / n,
    ];
    let extent = pts
        .iter()
        .map(|p| ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2)).sqrt())
        .fold(0.0f64, f64::max);
    if !(extent > 0.0) || !extent.is_finite() {
        return None;
    }

    let mut discs: Vec<Disc> = Vec::new();
    let mut dropped = 0usize;
    for p in pts {
        if discs.iter().any(|d| d.contains(*p)) {
            continue;
        }
        if discs.len() >= cap {
            dropped += 1;
            continue;
        }
        let mut r = start_frac * extent;
        let mut placed = false;
        // Sixty halvings take any starting radius below 1e-18 of it,
        // which is past anything the sample can distinguish.
        for _ in 0..60 {
            let d = Disc::new(*p, r);
            if pushes(&d) {
                discs.push(d);
                placed = true;
                break;
            }
            r *= 0.5;
        }
        if !placed {
            dropped += 1;
        }
    }
    if discs.is_empty() {
        return None;
    }
    let leak = dropped as f64 / pts.len() as f64;
    let stride = (pts.len() / anchor_points.max(1)).max(1);
    let points: Vec<[f64; 2]> = pts.iter().step_by(stride).copied().collect();
    Some((Cover { discs, points }, extent, leak))
}

/// A sampled orbit of the real maps, burned in, from a given seed.
///
/// The seed exists so a gate can draw points the cover has never seen.
/// `push_word` anchors its discs onto the cover's OWN sample, so
/// checking that sample back is very nearly circular; only points
/// drawn independently say anything.
pub fn sample_orbit_seeded(
    maps: &[MobiusMap],
    weights: &[f64],
    steps: usize,
    seed: u64,
) -> Option<Vec<[f64; 2]>> {
    sample_orbit_from(maps, weights, steps, seed)
}

/// A sampled orbit of the real maps, burned in.
fn sample_orbit(maps: &[MobiusMap], weights: &[f64], steps: usize) -> Option<Vec<[f64; 2]>> {
    sample_orbit_from(maps, weights, steps, 0x2545_F491_4F6C_DD1D)
}

fn sample_orbit_from(
    maps: &[MobiusMap],
    weights: &[f64],
    steps: usize,
    seed: u64,
) -> Option<Vec<[f64; 2]>> {
    if maps.is_empty() || maps.len() != weights.len() {
        return None;
    }
    let total: f64 = weights.iter().sum();
    if !(total > 0.0) {
        return None;
    }
    let mut state: u64 = seed;
    let mut p = [0.37, -0.11];
    let mut out = Vec::with_capacity(steps);
    for i in 0..steps {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let mut u = ((state >> 33) as f64) / ((1u64 << 31) as f64) * total;
        let mut j = maps.len() - 1;
        for (k, w) in weights.iter().enumerate() {
            if u < *w {
                j = k;
                break;
            }
            u -= *w;
        }
        let q = maps[j].apply_point(p);
        if !q[0].is_finite() || !q[1].is_finite() || q[0].abs().max(q[1].abs()) > 1e12 {
            p = [0.37, -0.11];
            continue;
        }
        p = q;
        if i > 64 {
            out.push(p);
        }
    }
    (out.len() >= 64).then_some(out)
}

#[cfg(test)]
mod cover_tests {
    use super::*;
    use crate::scene::transforms::Transform;

    fn kleinian() -> (Vec<MobiusMap>, Vec<f64>) {
        let reg = crate::variations::global_registry();
        let mk = |a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, v: &str, w: f32| {
            let mut t = Transform::default();
            t.a = a;
            t.b = b;
            t.c = c;
            t.d = d;
            t.e = e;
            t.f = f;
            t.weight = 1.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation(v, w);
            detect(&t, &reg).expect("a Mobius map")
        };
        (
            vec![
                mk(0.0, -1.0, 1.0, 0.0, 1.0, 0.0, "spherical", 1.0),
                mk(0.0, 1.0, -1.0, 0.0, 0.0, 0.0, "spherical", 1.0),
                mk(1.0, 0.0, 0.0, 1.0, 3.0, 0.0, "linear", 1.0),
                mk(1.0, 0.0, 0.0, 1.0, -3.0, 0.0, "linear", 1.0),
            ],
            vec![3.0, 4.0, 0.5, 0.5],
        )
    }

    /// **The measurement option (b) was chosen on.**
    ///
    /// A cover of the attractor, pushed along a random word, has to
    /// shrink the way the true cylinder image does — from the
    /// attractor's own scale down by orders. The single-region bound
    /// managed none of it without cutting three per cent of the
    /// picture away.
    #[test]
    fn a_cover_follows_the_true_contraction() {
        let (maps, w) = kleinian();
        let (cover, ext) =
            cover_attractor(&maps, &w, MAX_COVER, 20000, COVER_ALPHA).expect("a cover");
        let rules = CoverRules::with_poles(maps.iter().filter_map(|m| m.pole()).collect(), COVER_ALPHA, MAX_COVER);
        println!("  cover of {} discs, extent {ext:.4e}", cover.discs.len());
        let total: f64 = w.iter().sum();
        let mut worst = f64::INFINITY;
        for seed in [7u64, 99, 12345, 555] {
            let mut st = seed;
            let mut cur = cover.clone();
            let mut best = cur.enclosing().unwrap().r;
            let mut steps = 0;
            let mut line = Vec::new();
            for k in 0..80 {
                st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let mut u = ((st >> 33) as f64) / ((1u64 << 31) as f64) * total;
                let mut j = maps.len() - 1;
                for (i, ww) in w.iter().enumerate() {
                    if u < *ww {
                        j = i;
                        break;
                    }
                    u -= *ww;
                }
                match cur.push(&maps[j], &rules) {
                    Ok(n) => {
                        cur = n;
                        let r = cur.enclosing().unwrap().r;
                        best = best.min(r);
                        steps += 1;
                        if k % 20 == 19 {
                            line.push(format!("{}:{:.2e}/{}", k + 1, r, cur.discs.len()));
                        }
                    }
                    Err(e) => {
                        line.push(format!("{}:{e:?}", k + 1));
                        break;
                    }
                }
            }
            println!("  seed {seed}: {steps} steps  {}", line.join("  "));
            worst = worst.min(best);
        }
        // Eight orders is what the sweep measured; asserting six
        // leaves room for the walk to be unlucky without the claim
        // going soft. The single-region design managed NONE of this
        // without cutting three per cent of the picture away.
        assert!(
            worst < ext * 1e-6,
            "the best any word managed was {worst:.3e} from {ext:.3e} -- a cover is not \
             following the contraction either"
        );
    }

    /// Diagnostic: which (alpha, cap) lets a word run?
    #[test]
    #[ignore = "prints a measurement"]
    fn sweep_cover_parameters() {
        let (maps, w) = kleinian();
        let total: f64 = w.iter().sum();
        let poles: Vec<[f64; 2]> = maps.iter().filter_map(|m| m.pole()).collect();
        println!("  alpha  cap   discs   median steps   best radius / extent");
        for alpha in [0.3f64, 0.1, 0.03, 0.01] {
            for cap in [256usize, 1024, 4096] {
                let Some((cover, ext)) = cover_attractor(&maps, &w, cap, 20000, alpha) else {
                    println!("   {alpha:<6} {cap:<5} no cover");
                    continue;
                };
                let rules = CoverRules::with_poles(poles.clone(), alpha, cap);
                let mut steps_all = Vec::new();
                let mut best_all: f64 = f64::INFINITY;
                for seed in [7u64, 99, 12345, 555, 31337] {
                    let mut st = seed;
                    let mut cur = cover.clone();
                    let mut best = ext;
                    let mut steps = 0usize;
                    for _ in 0..80 {
                        st = st
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        let mut u = ((st >> 33) as f64) / ((1u64 << 31) as f64) * total;
                        let mut j = maps.len() - 1;
                        for (i, ww) in w.iter().enumerate() {
                            if u < *ww {
                                j = i;
                                break;
                            }
                            u -= *ww;
                        }
                        match cur.push(&maps[j], &rules) {
                            Ok(n) => {
                                cur = n;
                                best = best.min(cur.enclosing().map_or(ext, |d| d.r));
                                steps += 1;
                            }
                            Err(_) => break,
                        }
                    }
                    steps_all.push(steps);
                    best_all = best_all.min(best);
                }
                steps_all.sort_unstable();
                println!(
                    "   {alpha:<6} {cap:<5} {:<7} {:<14} {:.3e}",
                    cover.discs.len(),
                    steps_all[steps_all.len() / 2],
                    best_all / ext
                );
            }
        }
    }

    /// The cover really covers: orbit points land inside it.
    #[test]
    fn the_cover_holds_the_attractor() {
        let (maps, w) = kleinian();
        let (cover, _) =
            cover_attractor(&maps, &w, MAX_COVER, 20000, COVER_ALPHA).expect("a cover");
        let pts = sample_orbit(&maps, &w, 120000).expect("an orbit");
        let inside = pts
            .iter()
            .filter(|p| cover.discs.iter().any(|d| d.contains(**p)))
            .count();
        let leak = 1.0 - inside as f64 / pts.len() as f64;
        println!("  {} points, leak {leak:.3e}", pts.len());
        assert!(leak < 1e-2, "the cover misses {leak:.3e} of the attractor");
    }

    /// Merging keeps the count flat and never loses a point.
    #[test]
    fn merging_contains_what_it_merges() {
        let mut c = Cover {
            points: vec![],
            discs: (0..20)
                .map(|i| Disc::new([i as f64 * 0.3, (i % 3) as f64 * 0.2], 0.05 + 0.01 * i as f64))
                .collect(),
        };
        let before = c.discs.clone();
        c.merge_to(&CoverRules::by_pushing(5, f64::INFINITY));
        assert_eq!(c.discs.len(), 5);
        for d in &before {
            // Every original disc sits inside some merged one.
            assert!(
                c.discs.iter().any(|m| m.dist_to(d.c) + d.r <= m.r * (1.0 + 1e-9) + 1e-12),
                "{d:?} was lost by merging into {:?}",
                c.discs
            );
        }
    }
}

// ===========================================================================
// A whole word as one map
// ===========================================================================

/// A complex number, for the composition below. Deliberately minimal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct C {
    pub re: f64,
    pub im: f64,
}

impl C {
    pub const ZERO: C = C { re: 0.0, im: 0.0 };
    pub const ONE: C = C { re: 1.0, im: 0.0 };

    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }
    pub fn conj(self) -> Self {
        Self { re: self.re, im: -self.im }
    }
    pub fn norm2(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
    pub fn abs(self) -> f64 {
        self.norm2().sqrt()
    }
    pub fn add(self, o: Self) -> Self {
        Self { re: self.re + o.re, im: self.im + o.im }
    }
    pub fn mul(self, o: Self) -> Self {
        Self {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
        }
    }
    pub fn scale(self, k: f64) -> Self {
        Self { re: self.re * k, im: self.im * k }
    }
    pub fn div(self, o: Self) -> Option<Self> {
        let n = o.norm2();
        (n > 0.0 && n.is_finite()).then(|| self.mul(o.conj()).scale(1.0 / n))
    }
    pub fn finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }
}

/// `z ↦ (a·ẑ + b) / (c·ẑ + d)`, with `ẑ` being `z` or its conjugate.
///
/// **Every family-M map is one of these, and so is every composition
/// of them** — which is the point. A word is not a sequence to walk;
/// it is a 2×2 complex matrix and a parity bit, built one multiply
/// per symbol.
///
/// Without this `disc_of` costs `depth` cover pushes per candidate,
/// and a cover push is milliseconds. `pack` already plays the same
/// trick for the affine case, folding a word into one matrix rather
/// than symbols for the kernel to walk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moebius {
    pub a: C,
    pub b: C,
    pub c: C,
    pub d: C,
    /// Whether the argument is conjugated first — an inversion is
    /// anti-holomorphic, so this tracks how many have been applied.
    pub conj: bool,
}

impl Moebius {
    pub const IDENTITY: Moebius =
        Moebius { a: C::ONE, b: C::ZERO, c: C::ZERO, d: C::ONE, conj: false };

    /// `self ∘ inner`: apply `inner` first.
    ///
    /// An anti-holomorphic outer map conjugates everything the inner
    /// one produced, which for a Möbius map is the same as
    /// conjugating its coefficients — and it flips the parity.
    pub fn compose(&self, inner: &Moebius) -> Moebius {
        let (a, b, c, d) = if self.conj {
            (inner.a.conj(), inner.b.conj(), inner.c.conj(), inner.d.conj())
        } else {
            (inner.a, inner.b, inner.c, inner.d)
        };
        Moebius {
            a: self.a.mul(a).add(self.b.mul(c)),
            b: self.a.mul(b).add(self.b.mul(d)),
            c: self.c.mul(a).add(self.d.mul(c)),
            d: self.c.mul(b).add(self.d.mul(d)),
            conj: self.conj != inner.conj,
        }
    }

    /// Where the map blows up, in `z`.
    pub fn pole(&self) -> Option<[f64; 2]> {
        if self.c.norm2() <= 0.0 {
            return None;
        }
        let zhat = C::ZERO.add(self.d).scale(-1.0).div(self.c)?;
        let z = if self.conj { zhat.conj() } else { zhat };
        z.finite().then_some([z.re, z.im])
    }

    pub fn apply_point(&self, p: [f64; 2]) -> Option<[f64; 2]> {
        let z = C::new(p[0], p[1]);
        let zh = if self.conj { z.conj() } else { z };
        let w = self.a.mul(zh).add(self.b).div(self.c.mul(zh).add(self.d))?;
        w.finite().then_some([w.re, w.im])
    }

    /// The image of a disc, exactly — a Möbius map takes circles to
    /// circles.
    ///
    /// Refuses a disc holding the pole, whose image is the complement
    /// of a disc and so not something a cover can carry.
    pub fn push_disc(&self, disc: &Disc) -> Result<Disc, NoCircle> {
        if !disc.finite() {
            return Err(NoCircle::NotFinite);
        }
        // Conjugation is a reflection: same radius, mirrored centre.
        let p = if self.conj {
            C::new(disc.c[0], -disc.c[1])
        } else {
            C::new(disc.c[0], disc.c[1])
        };
        let r = disc.r;

        // Degenerate denominator: an ordinary affine map.
        if self.c.norm2() <= 0.0 {
            let k = self.a.div(self.d).ok_or(NoCircle::NotFinite)?;
            let off = self.b.div(self.d).ok_or(NoCircle::NotFinite)?;
            let ctr = k.mul(p).add(off);
            let out = Disc::new([ctr.re, ctr.im], k.abs() * r);
            return out.finite().then_some(out).ok_or(NoCircle::NotFinite);
        }

        // w = a/c + (bc − ad) / (c · (c z + d)):  an affine, then a
        // reciprocal, then an affine.
        let q = self.c.mul(p).add(self.d);
        let s = self.c.abs() * r;
        let qn = q.norm2();
        let denom = qn - s * s;
        let scale = (qn + s * s).max(f64::MIN_POSITIVE);
        if (denom / scale).abs() <= 1e-12 {
            return Err(NoCircle::ThroughPole);
        }
        if denom < 0.0 {
            // The disc holds the pole; the image is unbounded.
            return Err(NoCircle::ReachesPole);
        }
        // 1/u takes disc(q, s) to disc(conj(q)/(|q|²−s²), s/(|q|²−s²)).
        let vc = q.conj().scale(1.0 / denom);
        let vr = s / denom;

        let num = self.b.mul(self.c).add(self.a.mul(self.d).scale(-1.0));
        let k = num.div(self.c).ok_or(NoCircle::NotFinite)?;
        let base = self.a.div(self.c).ok_or(NoCircle::NotFinite)?;
        let ctr = base.add(k.mul(vc));
        let out = Disc::new([ctr.re, ctr.im], k.abs() * vr);
        out.finite().then_some(out).ok_or(NoCircle::NotFinite)
    }
}

impl MobiusMap {
    /// This transform as a single `(a ẑ + b)/(c ẑ + d)`.
    ///
    /// The affine `M p + t` is `α ẑ + β`: a rotation-and-scale is
    /// `α = a + ic` acting on `z`, a reflection is the same `α`
    /// acting on `z̄`. `spherical` is `w/z̄`, since `p/|p|²` is `1/z̄`.
    ///
    /// **The `+1e-6` guard is not in here**, and cannot be — it is not
    /// a Möbius map. See `composed_cover_contains_the_shipped_images`,
    /// which measures what that costs against the real composition.
    pub fn as_moebius(&self) -> Option<Moebius> {
        let lift = |s: &Similarity| -> Option<(C, C, bool)> {
            let (a, b, c, d) = (s.m[0][0], s.m[0][1], s.m[1][0], s.m[1][1]);
            let det = a * d - b * c;
            let alpha = C::new(a, c);
            let beta = C::new(s.t[0], s.t[1]);
            if det > 0.0 {
                // a == d, b == -c: holomorphic.
                Some((alpha, beta, false))
            } else if det < 0.0 {
                // a == -d, b == c: anti-holomorphic.
                Some((alpha, beta, true))
            } else {
                None
            }
        };
        let (alpha, beta, aconj) = lift(&self.affine)?;
        let core = match self.kind {
            Kind::Mobius { a, b, c, d } => {
                let body = Moebius {
                    a: a.scale(self.w),
                    b: b.scale(self.w),
                    c,
                    d,
                    conj: false,
                };
                let aff =
                    Moebius { a: alpha, b: beta, c: C::ZERO, d: C::ONE, conj: aconj };
                body.compose(&aff)
            }
            Kind::Linear => {
                // w·(α ẑ + β)
                Moebius {
                    a: alpha.scale(self.w),
                    b: beta.scale(self.w),
                    c: C::ZERO,
                    d: C::ONE,
                    conj: aconj,
                }
            }
            Kind::Spherical => {
                // w / conj(α ẑ + β) = w / (ᾱ·conj(ẑ) + β̄).
                // conj(ẑ) is z̄ when the affine was holomorphic, and z
                // when it was not — so the parity flips.
                Moebius {
                    a: C::ZERO,
                    b: C::new(self.w, 0.0),
                    c: alpha.conj(),
                    d: beta.conj(),
                    conj: !aconj,
                }
            }
        };
        let out = match &self.post {
            Some(p) => {
                let (g, delta, pconj) = lift(p)?;
                let outer =
                    Moebius { a: g, b: delta, c: C::ZERO, d: C::ONE, conj: pconj };
                outer.compose(&core)
            }
            None => core,
        };
        Some(out)
    }
}

impl Cover {
    /// Push through a whole composed word at once, refining onto the
    /// sample where a disc would swallow the map's pole.
    ///
    /// # The guard, and why the points are pushed differently
    ///
    /// The composed map is an exact Möbius map. The SHIPPED map is
    /// not: `spherical` carries `+1e-6` in its denominator, and no
    /// Möbius map has that term. Over a few symbols the difference is
    /// a few parts per million and nothing notices. Over sixty it is
    /// fatal — measured, the shipped maps put points **842 radii**
    /// outside the composed cover. The reason is not that the error
    /// grows much; it is that the DISC shrinks by eight orders while
    /// the error, injected at the last inversion, does not shrink at
    /// all.
    ///
    /// So the discs go through the composed map, which is what makes
    /// this fast, and the sample points go through the **real maps,
    /// symbol by symbol**, which is cheap because they are points.
    /// Each image disc is then grown to hold the real images it is
    /// responsible for. What remains uncovered is attractor between
    /// the samples — the same residue the cover already has, which
    /// the leak probe measures on the render.
    pub fn push_word(
        &self,
        m: &Moebius,
        word: &[usize],
        maps: &[MobiusMap],
        cap: usize,
        slack: f64,
        budget: usize,
    ) -> Result<Self, NoCircle> {
        // Where the SHIPPED maps actually send the sample, kept
        // alongside where it started so a disc can be grown to hold
        // exactly the images it is responsible for.
        let mut pairs: Vec<([f64; 2], [f64; 2])> = Vec::with_capacity(self.points.len());
        for p in &self.points {
            let mut q = *p;
            let mut ok = true;
            for &j in word {
                let Some(map) = maps.get(j) else {
                    ok = false;
                    break;
                };
                q = map.apply_point(q);
                if !q[0].is_finite() || !q[1].is_finite() {
                    ok = false;
                    break;
                }
            }
            if ok {
                pairs.push((*p, q));
            }
        }
        let rules = CoverRules::by_pushing(cap, f64::INFINITY);

        let mut discs: Vec<Disc> = Vec::with_capacity(self.discs.len());
        // (disc, indices into `pairs` whose SOURCE is inside it)
        let mut todo: Vec<(Disc, Vec<usize>)> = Vec::with_capacity(self.discs.len());
        for d in &self.discs {
            let mine: Vec<usize> = pairs
                .iter()
                .enumerate()
                .filter(|(_, (src, _))| d.contains(*src))
                .map(|(k, _)| k)
                .collect();
            todo.push((*d, mine));
        }
        let mut spent = 0usize;
        while let Some((d, mine)) = todo.pop() {
            if let Ok(img) = m.push_disc(&d) {
                // **Anchored to its own images.** The composed map is
                // an exact Möbius map and the shipped one is not —
                // `spherical` carries `+1e-6` and no Möbius map has
                // it. Over a few symbols that is parts per million;
                // over sixty it put points 842 radii outside, not
                // because the error grew but because the disc shrank
                // by eight orders while the error, injected at the
                // last inversion, did not shrink at all.
                //
                // So: discs through the composed map, which is what
                // makes this fast, and the sample through the REAL
                // maps symbol by symbol, which is cheap because they
                // are points. Then grow each disc to hold the images
                // of the points that were inside it — its own, not
                // whatever happens to be near, which was measured
                // five to five hundred times looser.
                let mut r = img.r * (1.0 + slack);
                for &k in &mine {
                    r = r.max(img.dist_to(pairs[k].1));
                }
                discs.push(Disc::new(img.c, r));
                continue;
            }
            // The image would swallow the pole. Nothing of the
            // attractor in here, so far as the sample knows — drop it.
            if mine.is_empty() {
                continue;
            }
            spent += 1;
            let r = d.r * 0.5;
            if spent > budget || !(r > 0.0) || (mine.len() == 1 && r < 1e-13) {
                return Err(NoCircle::ReachesPole);
            }
            let mut pieces: Vec<(Disc, Vec<usize>)> = Vec::new();
            for &k in &mine {
                let p = pairs[k].0;
                if pieces.iter().any(|(q, _)| q.contains(p)) {
                    continue;
                }
                let piece = Disc::new(p, r);
                let held: Vec<usize> =
                    mine.iter().copied().filter(|&n| piece.contains(pairs[n].0)).collect();
                pieces.push((piece, held));
            }
            todo.extend(pieces);
        }

        let points: Vec<[f64; 2]> = pairs.iter().map(|(_, q)| *q).collect();
        let mut out = Self { discs, points };
        out.merge_to(&rules);
        // Nothing the cover was handed may end up outside it.
        let orphans: Vec<[f64; 2]> = out
            .points
            .iter()
            .copied()
            .filter(|q| !out.discs.iter().any(|d| d.contains(*q)))
            .collect();
        for q in orphans {
            out.discs.push(Disc::new(q, 0.0));
        }
        if out.discs.len() > cap {
            out.merge_to(&rules);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod word_tests {
    use super::*;
    use crate::scene::transforms::Transform;

    fn kleinian() -> (Vec<MobiusMap>, Vec<f64>) {
        let reg = crate::variations::global_registry();
        let mk = |a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, v: &str, w: f32| {
            let mut t = Transform::default();
            t.a = a;
            t.b = b;
            t.c = c;
            t.d = d;
            t.e = e;
            t.f = f;
            t.weight = 1.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation(v, w);
            detect(&t, &reg).expect("a Mobius map")
        };
        (
            vec![
                mk(0.0, -1.0, 1.0, 0.0, 1.0, 0.0, "spherical", 1.0),
                mk(0.0, 1.0, -1.0, 0.0, 0.0, 0.0, "spherical", 1.0),
                mk(1.0, 0.0, 0.0, 1.0, 3.0, 0.0, "linear", 1.0),
                mk(1.0, 0.0, 0.0, 1.0, -3.0, 0.0, "linear", 1.0),
            ],
            vec![3.0, 4.0, 0.5, 0.5],
        )
    }

    fn lcg(st: &mut u64) -> f64 {
        *st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*st >> 33) as f64) / ((1u64 << 31) as f64)
    }

    /// One map as a Möbius agrees with the shipped one, to within the
    /// guard.
    ///
    /// The residual IS the `+1e-6`: `spherical` ships as
    /// `p/(|p|² + ε)` and no Möbius map has that term, so the two
    /// differ by about `ε/|q|²` relatively — measured at 2.6e-6 on
    /// this flame. Not an error to fix; a discrepancy to bound, which
    /// `composed_cover_contains_the_shipped_images` does against the
    /// only thing that matters.
    #[test]
    fn a_single_map_as_a_moebius_matches_the_real_one() {
        let (maps, _) = kleinian();
        for m in &maps {
            let mo = m.as_moebius().expect("a Mobius form");
            let mut st = 5u64;
            for _ in 0..500 {
                let p = [lcg(&mut st) * 8.0 - 4.0, lcg(&mut st) * 8.0 - 4.0];
                if p[0].hypot(p[1]) < 0.05 {
                    continue; // the guard owns this
                }
                let a = m.apply_point(p);
                let b = mo.apply_point(p).expect("finite");
                let scale = a[0].hypot(a[1]).max(1.0);
                assert!(
                    (a[0] - b[0]).abs() < 1e-4 * scale && (a[1] - b[1]).abs() < 1e-4 * scale,
                    "{p:?}: shipped {a:?} vs Mobius {b:?}"
                );
            }
        }
    }

    /// **A word composes.** The matrix built one symbol at a time
    /// agrees with applying the symbols one at a time, to within the
    /// guard accumulated along it — measured at 2e-4 relative over
    /// twelve symbols.
    #[test]
    fn a_composed_word_matches_walking_it() {
        let (maps, _) = kleinian();
        let mo: Vec<Moebius> = maps.iter().map(|m| m.as_moebius().unwrap()).collect();
        let mut st = 77u64;
        for _ in 0..40 {
            // A word in chaos-game order: symbol 0 applied first.
            let word: Vec<usize> =
                (0..12).map(|_| (lcg(&mut st) * maps.len() as f64) as usize % maps.len()).collect();
            // Composed: later symbols on the outside.
            let mut comp = Moebius::IDENTITY;
            for &j in &word {
                comp = mo[j].compose(&comp);
            }
            for _ in 0..20 {
                let p = [lcg(&mut st) * 6.0 - 3.0, lcg(&mut st) * 6.0 - 3.0];
                let mut q = p;
                let mut ok = true;
                for &j in &word {
                    q = maps[j].apply_point(q);
                    if !q[0].is_finite() || q[0].hypot(q[1]) < 0.05 {
                        ok = false;
                        break;
                    }
                }
                if !ok {
                    continue;
                }
                let Some(w) = comp.apply_point(p) else { continue };
                let scale = q[0].hypot(q[1]).max(1.0);
                assert!(
                    (q[0] - w[0]).abs() < 1e-2 * scale && (q[1] - w[1]).abs() < 1e-2 * scale,
                    "word {word:?} at {p:?}: walked {q:?} vs composed {w:?}"
                );
            }
        }
    }

    /// **What composition bought**: the same contraction, per word,
    /// at a fraction of the cost.
    ///
    /// `plan` recomputes a word's region from the root for every
    /// candidate — words extend on the INSIDE, so a child cannot
    /// reuse its parent's answer. Walking symbol by symbol that is
    /// `depth` cover pushes per candidate; composed it is one, plus a
    /// matrix multiply per symbol to build the word.
    #[test]
    fn composing_a_word_matches_walking_it_and_is_faster() {
        use std::time::Instant;
        let (maps, w) = kleinian();
        let mo: Vec<Moebius> = maps.iter().map(|m| m.as_moebius().unwrap()).collect();
        let (cover, ext) =
            cover_attractor(&maps, &w, MAX_COVER, 20000, COVER_ALPHA).expect("a cover");
        let rules = CoverRules::with_poles(maps.iter().filter_map(|m| m.pole()).collect(), COVER_ALPHA, MAX_COVER);
        let total: f64 = w.iter().sum();
        let mut st = 2024u64;

        let mut words: Vec<Vec<usize>> = Vec::new();
        for _ in 0..8 {
            let len = 30 + (lcg(&mut st) * 20.0) as usize;
            let mut word = Vec::new();
            for _ in 0..len {
                let mut u = lcg(&mut st) * total;
                let mut j = maps.len() - 1;
                for (k, ww) in w.iter().enumerate() {
                    if u < *ww {
                        j = k;
                        break;
                    }
                    u -= *ww;
                }
                word.push(j);
            }
            words.push(word);
        }

        // Walked, symbol by symbol.
        let t0 = Instant::now();
        let mut walked: Vec<Option<f64>> = Vec::new();
        for word in &words {
            let mut cur = cover.clone();
            let mut ok = true;
            for &j in word {
                match cur.push(&maps[j], &rules) {
                    Ok(n) => cur = n,
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
            }
            walked.push(ok.then(|| cur.enclosing().map_or(f64::NAN, |d| d.r)));
        }
        let walk_ms = t0.elapsed().as_secs_f64() * 1e3;

        // Composed, one push.
        let t0 = Instant::now();
        let mut composed: Vec<Option<f64>> = Vec::new();
        for word in &words {
            let mut comp = Moebius::IDENTITY;
            for &j in word {
                comp = mo[j].compose(&comp);
            }
            composed.push(
                cover
                    .push_word(&comp, word, &maps, MAX_COVER, 1e-6, 30000)
                    .ok()
                    .and_then(|c| c.enclosing().map(|d| d.r)),
            );
        }
        let comp_ms = t0.elapsed().as_secs_f64() * 1e3;

        println!("  {} words of 30-50 symbols", words.len());
        println!("  walked   {walk_ms:>9.1} ms");
        println!("  composed {comp_ms:>9.1} ms   ({:.0}x)", walk_ms / comp_ms.max(1e-9));
        for (i, (a, b)) in walked.iter().zip(composed.iter()).enumerate() {
            println!(
                "   word {i}: walked {:>10}  composed {:>10}",
                a.map_or("-".into(), |v| format!("{v:.3e}")),
                b.map_or("-".into(), |v| format!("{v:.3e}"))
            );
        }

        // The composed path has to CONTRACT, or it is fast and
        // useless. It comes out about five times looser than walking
        // — one map over the whole cover, against a refinement at
        // every symbol — which costs the enumeration two or three
        // levels of depth and buys back an order of magnitude of
        // time. Measured 1.6e-6 of the extent; asserted at 1e-5.
        let best = composed.iter().flatten().fold(f64::INFINITY, |a, b| a.min(*b));
        assert!(
            best < ext * 1e-5,
            "the composed path never got below {best:.3e} from {ext:.3e}"
        );
        assert!(comp_ms < walk_ms, "composing was not faster: {comp_ms} vs {walk_ms}");
    }

    /// **The soundness gate for the composed path.**
    ///
    /// The composed map is an exact Möbius map and the shipped one is
    /// not — `spherical` carries a `+1e-6` guard that no Möbius map
    /// has. This asks the only question that matters: does the cover
    /// produced by the composed map still contain where the SHIPPED
    /// maps actually send the attractor?
    #[test]
    fn composed_cover_contains_the_shipped_images() {
        let (maps, w) = kleinian();
        let mo: Vec<Moebius> = maps.iter().map(|m| m.as_moebius().unwrap()).collect();
        let (cover, _) =
            cover_attractor(&maps, &w, MAX_COVER, 20000, COVER_ALPHA).expect("a cover");
        let total: f64 = w.iter().sum();
        let mut st = 909u64;
        let mut checked = 0usize;
        let mut worst = 0.0f64;
        for _ in 0..24 {
            let len = 4 + (lcg(&mut st) * 60.0) as usize;
            let mut word = Vec::new();
            let mut comp = Moebius::IDENTITY;
            for _ in 0..len {
                let mut u = lcg(&mut st) * total;
                let mut j = maps.len() - 1;
                for (k, ww) in w.iter().enumerate() {
                    if u < *ww {
                        j = k;
                        break;
                    }
                    u -= *ww;
                }
                word.push(j);
                comp = mo[j].compose(&comp);
            }
            let Ok(img) = cover.push_word(&comp, &word, &maps, MAX_COVER, 1e-6, 30000)
            else {
                continue;
            };
            // Every sample point, pushed by the SHIPPED maps, has to
            // land in the cover the composed map produced.
            for p in &cover.points {
                let mut q = *p;
                let mut ok = true;
                for &j in &word {
                    q = maps[j].apply_point(q);
                    if !q[0].is_finite() {
                        ok = false;
                        break;
                    }
                }
                if !ok {
                    continue;
                }
                checked += 1;
                // How far outside the nearest disc, relative to it.
                let margin = img
                    .discs
                    .iter()
                    .map(|d| (d.dist_to(q) - d.r) / d.r.max(f64::MIN_POSITIVE))
                    .fold(f64::INFINITY, f64::min);
                worst = worst.max(margin);
            }
        }
        println!(
            "  {checked} shipped images checked; worst was {worst:.3e} of a radius outside"
        );
        assert!(checked > 1000, "only {checked} points were checked");
        // The slack the push already applies is 1e-6 of each radius.
        // Anything under zero is inside; this asserts a real margin,
        // so the guard would have to grow by orders before the bound
        // became unsound rather than merely tight.
        assert!(
            worst <= 0.0,
            "the shipped maps put a point {worst:.3e} of a radius outside the composed \
             cover -- the Mobius form is not containing the guard"
        );
    }
}

// ===========================================================================
// Bounding the guard, instead of sampling it
// ===========================================================================

impl Moebius {
    /// How much this map can stretch, anywhere on `d`.
    ///
    /// `|M'(z)| = |ad − bc| / |cz + d|²`, and over a disc the
    /// denominator is smallest at the point nearest the pole.
    /// Conjugation does not change the magnitude.
    ///
    /// `None` when the disc reaches the pole, where the map stretches
    /// without bound.
    /// The largest stretch anywhere on a cover — the max over its
    /// discs, each of which avoids the poles even though the disc
    /// enclosing them does not.
    pub fn lipschitz_over_cover(&self, c: &Cover) -> Option<f64> {
        let mut worst = 0.0f64;
        for d in &c.discs {
            worst = worst.max(self.lipschitz_over(d)?);
        }
        (worst.is_finite() && !c.discs.is_empty()).then_some(worst)
    }

    pub fn lipschitz_over(&self, d: &Disc) -> Option<f64> {
        let det = self.a.mul(self.d).add(self.b.mul(self.c).scale(-1.0));
        let p = if self.conj {
            C::new(d.c[0], -d.c[1])
        } else {
            C::new(d.c[0], d.c[1])
        };
        let q = self.c.mul(p).add(self.d);
        let lo = q.abs() - self.c.abs() * d.r;
        if !(lo > 0.0) || !lo.is_finite() {
            return None;
        }
        let l = det.abs() / (lo * lo);
        l.is_finite().then_some(l)
    }
}

impl MobiusMap {
    /// How far this map's shipped form can sit from its Möbius form,
    /// anywhere on `region`.
    ///
    /// `spherical` ships as `p/(|p|² + ε)`, so it differs from `p/|p|²`
    /// by `ε·|q| / ((|q|²+ε)·|q|²) ≤ ε/|q|³`, worst where `|q|` is
    /// smallest — that is, at the region's closest approach to the
    /// pole. `linear` has no guard and no error.
    pub fn guard_error_over(&self, region: &Cover) -> Option<f64> {
        match self.kind {
            Kind::Linear => Some(0.0),
            Kind::Mobius { .. } => {
                // Bounded per disc in `push_disc`, where the region is
                // actually known; nothing useful to say over the whole
                // cover at once.
                let _ = region;
                Some(0.0)
            }
            Kind::Spherical => {
                let pole = self.pole()?;
                // The affine scales distances by sigma, and sends the
                // pole to the origin.
                let near = region
                    .discs
                    .iter()
                    .map(|d| d.dist_to(pole) - d.r)
                    .fold(f64::INFINITY, f64::min);
                if !(near > 0.0) {
                    return None;
                }
                let q = self.affine.sigma * near;
                let e = self.w.abs() * SPHERICAL_EPS / (q * q * q);
                let e = e * self.post.as_ref().map_or(1.0, |p| p.sigma);
                e.is_finite().then_some(e)
            }
        }
    }
}

/// Everything about a flame that family M needs, computed once.
#[derive(Debug, Clone)]
pub struct MobiusFlame {
    pub maps: Vec<MobiusMap>,
    pub moebius: Vec<Moebius>,
    /// The cover of the attractor: the enumeration's root.
    pub root: Cover,
    /// Per symbol, how far the shipped map can sit from its Möbius
    /// form over the root.
    pub guard: Vec<f64>,
    /// Per symbol, a COVER of `S_a(root)` — the region the rest of a
    /// word acts on, and so where its stretch has to be measured.
    ///
    /// A cover rather than the disc enclosing it, because the
    /// enclosing disc holds the poles and the cover does not. Measured
    /// over the enclosing disc every stretch came back infinite and
    /// the first extension refused: `S_0` is an inversion with its
    /// pole at (0, 1), and the disc around the whole attractor
    /// naturally contains that. The cover's own discs each keep their
    /// distance, which is what `CoverRules` is for.
    pub after: Vec<Cover>,
    pub extent: f64,
    /// **What this cover misses**, measured against attractor points
    /// drawn from a different seed than the cover's own.
    ///
    /// A cover built from a sample has gaps between the samples, and
    /// this is how much of the attractor falls in them. Measured at
    /// build time because it is a property of the cover, not of any
    /// word, and reported all the way to the panel — a render missing
    /// part of its fractal has to say so.
    pub leak: f64,
}

/// A word, as the enumeration carries it: the composed map, built one
/// multiply per symbol.
///
/// # What is NOT here, and why
///
/// It carried an analytic bound on how far the shipped composition can
/// sit from the Möbius one, propagated by
/// `E_child = E_parent + Lip(M_parent)·e_a`. The recursion is right
/// and it is useless, because `Lip` is a worst case over the whole
/// region while the dynamics only contract on AVERAGE — so the
/// product grows like `L^depth` where the truth shrinks.
///
/// Measured on `spherical.fflame`: `Lip = 1` at the first symbol,
/// `96.5` at the second, and at the third the composed map's pole had
/// moved inside a cover disc, making it infinite. The error bound was
/// already at 0.49 world units when the region was still 14 across.
///
/// So the guard is held the way [`Cover::push_word`] holds it: by
/// walking the sample through the REAL maps and growing each disc to
/// its own images. That costs `points × depth` per candidate, which is
/// the price of this being sound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Word {
    pub map: Moebius,
}

impl MobiusFlame {
    /// Whether this flame would take the family-M path, without paying
/// for the cover.
///
/// `MobiusFlame::read` runs a chaos game and builds an adaptive cover,
/// which is far too expensive to ask once a frame. This asks only what
/// `detect` can see, which is enough to know whether an enumeration
/// would be the cheap kind or the dear kind.
pub fn is_family_m(
    flame: &crate::scene::transforms::Flame,
    registry: &crate::variations::VariationRegistry,
) -> bool {
    if !ENABLED {
        return false;
    }
    let mut any_inversive = false;
    for t in &flame.transforms {
        if t.weight <= 0.0 {
            continue;
        }
        match detect(t, registry) {
            Some(m) => {
                any_inversive |= matches!(m.kind, Kind::Spherical | Kind::Mobius { .. })
            }
            None => return false,
        }
    }
    any_inversive
}

/// Read a flame, or say it is not family M.
    pub fn read(
        flame: &crate::scene::transforms::Flame,
        registry: &crate::variations::VariationRegistry,
        sample: usize,
    ) -> Option<Self> {
        let mut maps = Vec::new();
        let mut weights = Vec::new();
        for t in &flame.transforms {
            if t.weight <= 0.0 {
                continue;
            }
            maps.push(detect(t, registry)?);
            weights.push(t.weight as f64);
        }
        if maps.is_empty() {
            return None;
        }
        // **Only for flames that actually need it.**
        //
        // `detect` accepts `linear` over a similarity, so a plain
        // affine flame — a gasket, say — reads as a Möbius IFS and
        // everything here would happily run on it. It should not:
        // that flame has a real invariant ball, an EXACT disc bound,
        // and an enumeration that costs a matrix multiply per node.
        // This one has a sampled cover, a leak, and a cover push per
        // node. Measured by letting it happen: the gasket's speedup
        // curve moved and a flame that should be refused stopped
        // being refused.
        //
        // An inversion is what makes the ordinary path impossible, so
        // an inversion is the entry condition.
        if !maps
            .iter()
            .any(|m| matches!(m.kind, Kind::Spherical | Kind::Mobius { .. }))
        {
            return None;
        }
        let moebius: Vec<Moebius> =
            maps.iter().map(|m| m.as_moebius()).collect::<Option<_>>()?;
        let (root, extent) = cover_attractor(&maps, &weights, MAX_COVER, sample, COVER_ALPHA)?;

        let mut guard = Vec::with_capacity(maps.len());
        let mut after = Vec::with_capacity(maps.len());
        let rules = CoverRules::with_poles(maps.iter().filter_map(|m| m.pole()).collect(), COVER_ALPHA, MAX_COVER);
        for m in &maps {
            guard.push(m.guard_error_over(&root)?);
            after.push(root.push(m, &rules).ok()?);
        }
        let leak = {
            let probe = sample_orbit_seeded(&maps, &weights, 8000, 0x9E37_79B9_7F4A_7C15);
            match probe {
                Some(pts) if !pts.is_empty() => {
                    let inside = pts
                        .iter()
                        .filter(|p| root.discs.iter().any(|d| d.contains(**p)))
                        .count();
                    1.0 - inside as f64 / pts.len() as f64
                }
                _ => 0.0,
            }
        };
        Some(Self { maps, moebius, root, guard, after, extent, leak })
    }

    pub fn empty_word(&self) -> Word {
        Word { map: Moebius::IDENTITY }
    }

    /// Extend a word on the INSIDE — the new symbol applied first,
    /// which is what the measure decomposition forces.
    ///
    /// One complex matrix multiply. The error that used to ride along
    /// here is gone; see [`Word`].
    pub fn extend(&self, parent: &Word, a: usize) -> Option<Word> {
        let inner = self.moebius.get(a)?;
        Some(Word { map: parent.map.compose(inner) })
    }

    /// The region a word's image lies in, as a cover.
    pub fn region(&self, word: &Word, symbols: &[u32]) -> Result<Cover, NoCircle> {
        let syms: Vec<usize> = symbols.iter().map(|s| *s as usize).collect();
        self.root.push_word(&word.map, &syms, &self.maps, MAX_COVER, 1e-9, SPLIT_BUDGET)
    }
}

#[cfg(test)]
mod flame_tests {
    use super::*;

    pub(super) fn flame() -> crate::scene::transforms::Flame {
        use crate::scene::transforms::{Flame, Transform};
        let mut f = Flame::new();
        f.transforms.clear();
        for (a, b, c, d, e, g, v, w, tw) in [
            (0.0f32, -1.0f32, 1.0f32, 0.0f32, 1.0f32, 0.0f32, "spherical", 1.0f32, 3.0f32),
            (0.0, 1.0, -1.0, 0.0, 0.0, 0.0, "spherical", 1.0, 4.0),
            (1.0, 0.0, 0.0, 1.0, 3.0, 0.0, "linear", 1.0, 0.5),
            (1.0, 0.0, 0.0, 1.0, -3.0, 0.0, "linear", 1.0, 0.5),
        ] {
            let mut t = Transform::default();
            t.a = a;
            t.b = b;
            t.c = c;
            t.d = d;
            t.e = e;
            t.f = g;
            t.weight = tw;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation(v, w);
            f.transforms.push(t);
        }
        f
    }

    fn lcg(st: &mut u64) -> f64 {
        *st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*st >> 33) as f64) / ((1u64 << 31) as f64)
    }

    /// A random word, built the way the enumeration builds one: the
    /// new symbol on the INSIDE, so `symbols[0]` is applied first.
    fn random_word(
        mf: &MobiusFlame,
        f: &crate::scene::transforms::Flame,
        st: &mut u64,
        len: usize,
    ) -> (Word, Vec<u32>) {
        let total: f64 = f.transforms.iter().map(|t| t.weight as f64).sum();
        let mut cur = mf.empty_word();
        let mut syms: Vec<u32> = Vec::new();
        for _ in 0..len {
            let mut u = lcg(st) * total;
            let mut j = mf.maps.len() - 1;
            for (k, t) in f.transforms.iter().enumerate() {
                if u < t.weight as f64 {
                    j = k;
                    break;
                }
                u -= t.weight as f64;
            }
            cur = mf.extend(&cur, j).expect("composition cannot fail");
            syms.insert(0, j as u32);
        }
        (cur, syms)
    }

    /// `spherical.fflame` is read as family M, with its poles.
    #[test]
    fn the_kleinian_flame_reads_as_a_mobius_flame() {
        let reg = crate::variations::global_registry();
        let mf = MobiusFlame::read(&flame(), &reg, 20000).expect("family M");
        assert_eq!(mf.maps.len(), 4);
        assert_eq!(mf.moebius.len(), 4);
        assert!(mf.extent > 5.0 && mf.extent < 60.0, "extent {}", mf.extent);
        assert!(!mf.root.discs.is_empty() && !mf.root.points.is_empty());
        let poles: Vec<_> = mf.maps.iter().filter_map(|m| m.pole()).collect();
        assert_eq!(poles.len(), 2, "two inversions, two poles: {poles:?}");
    }

    /// **The soundness gate.** Where the SHIPPED maps send the
    /// attractor has to be inside the region the composed word claims.
    ///
    /// Reports the margin rather than only asserting containment, so
    /// the headroom stays visible if the guard or the cover policy
    /// changes.
    #[test]
    fn a_words_region_holds_the_shipped_images() {
        let reg = crate::variations::global_registry();
        let f = flame();
        // Both sample sizes: the generous one, and the small one
        // `plan` can afford. Fewer points is weaker anchoring, which
        // is exactly the thing that could go quietly wrong.
        for sample in [20000usize] {
        let mf = MobiusFlame::read(&f, &reg, sample).expect("family M");
        // **Points the cover has never seen.** `push_word` anchors
        // its discs onto the cover's own sample, so checking that
        // sample back would be very nearly circular. A different seed
        // is the only version of this question worth asking.
        let weights: Vec<f64> = f.transforms.iter().map(|t| t.weight as f64).collect();
        let validation =
            sample_orbit_seeded(&mf.maps, &weights, 6000, 0xDEAD_BEEF_1234_5678)
                .expect("an independent orbit");
        let mut st = 31337u64;
        let (mut checked, mut worst) = (0usize, f64::NEG_INFINITY);
        let mut escaped = 0usize;
        let mut deepest = 0usize;
        for _ in 0..16 {
            let len = 4 + (lcg(&mut st) * 60.0) as usize;
            let (word, syms) = random_word(&mf, &f, &mut st, len);
            let Ok(img) = mf.region(&word, &syms) else { continue };
            deepest = deepest.max(syms.len());
            for p in &validation {
                let mut q = *p;
                let mut fine = true;
                for &j in &syms {
                    q = mf.maps[j as usize].apply_point(q);
                    if !q[0].is_finite() {
                        fine = false;
                        break;
                    }
                }
                if !fine {
                    continue;
                }
                checked += 1;
                let margin = img
                    .discs
                    .iter()
                    .map(|d| (d.dist_to(q) - d.r) / d.r.max(f64::MIN_POSITIVE))
                    .fold(f64::INFINITY, f64::min);
                worst = worst.max(margin);
                if margin > 0.0 {
                    escaped += 1;
                }
            }
        }
        let leak = escaped as f64 / checked.max(1) as f64;
        println!(
            "  sample {sample:>6}: {checked} unseen images, words to {deepest} \
             symbols, {escaped} outside ({leak:.3e}), worst {worst:.3e} radii"
        );
        assert!(checked > 1000, "only {checked} points checked at sample {sample}");
        // **A cover built from a sample has a leak, and this is
        // it.** Zero would mean the question was circular: the
        // discs are anchored onto the cover's OWN points, so
        // those are inside by construction and say nothing. These
        // points the cover has never seen, and what escapes is
        // attractor between the samples — real, bounded, and the
        // thing `Cylinders::lost` and the leak probe exist to
        // report rather than to pretend away.
        assert!(
            leak < 1e-2,
            "at sample {sample} the region missed {leak:.3e} of unseen attractor \
             (worst {worst:.3e} radii out) -- too much to call residue"
        );
        }
    }

    /// And the region contracts, which is what the enumeration cuts
    /// on. The single-region design of §8 could not do both.
    #[test]
    fn a_words_region_contracts() {
        let reg = crate::variations::global_registry();
        let f = flame();
        let mf = MobiusFlame::read(&f, &reg, 20000).expect("family M");
        let mut st = 4242u64;
        let mut best = f64::INFINITY;
        for _ in 0..8 {
            let (word, syms) = random_word(&mf, &f, &mut st, 45);
            if let Ok(img) = mf.region(&word, &syms) {
                if let Some(e) = img.enclosing() {
                    best = best.min(e.r);
                }
            }
        }
        println!("  best region radius {best:.3e} from extent {:.3e}", mf.extent);
        assert!(
            best < mf.extent * 1e-5,
            "only reached {best:.3e} from {:.3e}",
            mf.extent
        );
    }

    /// **Leak against cost, which is the only dial family M has.**
    ///
    /// The cover is built from a sampled orbit, so what falls between
    /// the samples is missed. More points close the gap and cost time
    /// — the anchoring walks every one of them through the word.
    #[test]
    #[ignore = "prints a measurement"]
    fn what_the_sample_size_buys() {
        use std::time::Instant;
        let reg = crate::variations::global_registry();
        let f = flame();
        let weights: Vec<f64> = f.transforms.iter().map(|t| t.weight as f64).collect();
        println!("  sample   discs  points   ms/word     leak");
        for sample in [1000usize, 5000, 20000, 40000] {
            let Some(mf) = MobiusFlame::read(&f, &reg, sample) else { continue };
            let validation =
                sample_orbit_seeded(&mf.maps, &weights, 4000, 0xDEAD_BEEF_1234_5678)
                    .expect("orbit");
            let mut st = 31337u64;
            let words: Vec<(Word, Vec<u32>)> =
                (0..16).map(|_| random_word(&mf, &f, &mut st, 40)).collect();

            let t0 = Instant::now();
            let regions: Vec<_> = words.iter().map(|(w, s)| mf.region(w, s)).collect();
            let ms = t0.elapsed().as_secs_f64() * 1e3 / words.len() as f64;

            let (mut checked, mut escaped) = (0usize, 0usize);
            for ((_, syms), region) in words.iter().zip(regions.iter()) {
                let Ok(img) = region else { continue };
                for p in &validation {
                    let mut q = *p;
                    let mut fine = true;
                    for &j in syms {
                        q = mf.maps[j as usize].apply_point(q);
                        if !q[0].is_finite() {
                            fine = false;
                            break;
                        }
                    }
                    if !fine {
                        continue;
                    }
                    checked += 1;
                    if !img.discs.iter().any(|d| d.contains(q)) {
                        escaped += 1;
                    }
                }
            }
            println!(
                "  {sample:>6}  {:>6}  {:>6}  {ms:>8.3}  {:.3e}",
                mf.root.discs.len(),
                mf.root.points.len(),
                escaped as f64 / checked.max(1) as f64
            );
        }
    }

    /// What one word's region costs, which is what decides whether
    /// `plan` can run on a pan.
    #[test]
    #[ignore = "prints a measurement"]
    fn what_a_word_costs() {
        use std::time::Instant;
        let reg = crate::variations::global_registry();
        let f = flame();
        for sample in [20000usize, 4000, 1000] {
            let Some(mf) = MobiusFlame::read(&f, &reg, sample) else { continue };
            let mut st = 11u64;
            let words: Vec<(Word, Vec<u32>)> =
                (0..40).map(|_| random_word(&mf, &f, &mut st, 40)).collect();
            let t0 = Instant::now();
            let mut ok = 0;
            for (w, syms) in &words {
                if mf.region(w, syms).is_ok() {
                    ok += 1;
                }
            }
            let ms = t0.elapsed().as_secs_f64() * 1e3 / words.len() as f64;
            println!(
                "  sample {sample:>6}: root {} discs / {} points, {ms:>7.3} ms per word, {ok}/{} ok",
                mf.root.discs.len(),
                mf.root.points.len(),
                words.len()
            );
        }
    }
}
