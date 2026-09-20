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
        for (i, h) in self.holes.iter().enumerate() {
            if h.contains([0.0, 0.0]) {
                // Inside a hole: the region starts at that hole's rim.
                return ((h.r - h.centre_norm()).max(0.0), Some(i));
            }
        }
        (0.0, None)
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
            } else {
                // A hole clear of the pole stays a hole, and a hole
                // shrinks by its slop — which grows the set, the only
                // safe direction.
                if flips {
                    return Err(NoCircle::ThroughPole);
                }
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
