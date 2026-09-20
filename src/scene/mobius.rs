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
}

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
