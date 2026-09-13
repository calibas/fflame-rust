//! Affine-IFS analysis of a flame: composition, inverses, singular
//! values, the qualifying criterion and a bounding ball.
//!
//! Phase 0 of `docs/projects/ifs-distance-rendering.md`, built as
//! common analysis because three plans want the same numbers: that one
//! (its distance estimate is inverse iteration with a σ_min product),
//! the escape plan's Mode D (its pass count is a function of the
//! largest singular value), and `flame-deep-zoom.md` §7 (its window,
//! its bounds and its conditioning). None of it existed: the only
//! contractiveness measure in the tree, `mean_log_scale`, is a
//! whole-flame mean of `0.5·ln|det A|`, and the determinant is the
//! AREA factor — it averages the two axes, so a map that stretches one
//! and squashes the other reads as neutral, which is exactly the map a
//! singular value exists to catch.
//!
//! Everything here is `f64` on the CPU. The shader that consumes it
//! takes `f32`; the analysis should not be the thing that loses bits.
//!
//! # What a flame transform IS, as an affine
//!
//! The chaos game applies, per transform: the affine, then the
//! PRE-phase variations (each replacing the point), then the weighted
//! sum of normal-phase variations, then the POST-phase variations
//! (each replacing the result), then the post-affine. When every
//! variation is affine the whole thing is one affine map, and the
//! composition here mirrors the shader exactly — the flat and full
//! paths of `affine_3d.wgsl`, the translation summed from three
//! sources, `g` dropped under active plane maps (a documented
//! limitation the shader has and this reproduces rather than fixes),
//! and pre/post variations composed in the order the shader emits
//! them, which is the FLAME's first-occurrence order and not the
//! transform's. Which variations count as affine, and what each one
//! contributes in each space, is [`affine_role`], and the list is
//! small on purpose.
//!
//! # Two kinds of "3D flame"
//!
//! `preserve_z` decides which. With it OFF — the default — the chaos
//! game zeroes z at the end of every iteration, so the trajectory is a
//! 2D IFS and z is a per-point height computed at plot time. The
//! attractor is 2D. With it ON, z carries across iterations and the
//! IFS is genuinely three-dimensional, and then contractivity in z
//! is a real condition: an Apophysis-style transform (XY affine plus a
//! z offset) has unit z scale and fails it. The analysis reports both
//! honestly rather than folding them into one answer.

use crate::scene::transforms::{Flame, Transform};
use crate::variations::{VariationPhase, VariationRegistry};

/// The variations the analysis knows: every name [`affine_role`] can
/// answer for in at least one space. Deliberately short: growing it
/// means proving each addition is affine, in each space, by reading
/// its body (conformal invertible maps — Möbius, spherical inversion,
/// the julia family — are the next candidates and are NOT affine; see
/// the plan's §8).
pub const AFFINE_VARIATIONS: &[&str] = &[
    "linear",
    "linear3D",
    "zscale",
    "ztranslate",
    "affine3D",
    "flatten",
    "zcone",
    "zblur",
    "pre_rotate_x",
    "pre_rotate_y",
    "post_rotate_x",
    "post_rotate_y",
];

/// What a variation does to a transform's map, in one space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AffineRole {
    /// Contributes nothing: a z-only variation in the plane, where the
    /// chaos game has no z, or a 2D stub that returns its input.
    Nothing,
    /// Summed into the normal phase: contributes `w · (M, t)`.
    Sum(Affine3),
    /// A pre-phase variation: replaces the affine's output with `R p`.
    Pre(Affine3),
    /// A post-phase variation: replaces the sum with `R p`.
    Post(Affine3),
}

/// What variation `name` at weight `w` contributes in `space`, or
/// `None` when it is not affine there. Read from the bodies:
///
/// - `linear` / `linear3D`: the identity on the point, summed.
/// - `zscale` / `ztranslate`: `w·z` and `w` on z, summed; in the
///   plane their 2D stubs return zero, so nothing.
/// - `affine3D`: JWildfire's general 3D affine — per-axis scale, six
///   shears, a Z·Y·X rotation and a translation, all fifteen
///   parameters, summed. **This is the one that makes a solid affine
///   IFS buildable**: without it every 3D transform is an XY affine
///   with a unit z scale, and the census found none that qualified.
/// - `flatten`: post-phase `z ← 0`. In the plane its stub returns its
///   input, so nothing; as a solid it is affine and SINGULAR, and the
///   criterion says so — a map with no inverse collapses the attractor.
/// - `zcone`, `zblur`: their 2D stubs return zero, so nothing in the
///   plane; as solids one is nonlinear and the other a measure.
/// - `pre_rotate_x/y`, `post_rotate_x/y`: rotations by the weight in
///   radians about the named axis, replacing the point in their
///   phase; isometries, so they change no singular value. 2D stubs
///   return their input, so nothing in the plane.
///
/// The census before this rule counted five shipped flames lost to
/// `flatten` alone, on a transform whose plane map was affine.
pub fn affine_role(
    name: &str,
    w: f64,
    t: &Transform,
    registry: &VariationRegistry,
    space: Space,
) -> Option<AffineRole> {
    let planar = space == Space::Planar;
    let diag = |x: f64, y: f64, z: f64| Affine3 { m: [[x, 0.0, 0.0], [0.0, y, 0.0], [0.0, 0.0, z]], t: [0.0; 3] };
    Some(match name {
        "linear" | "linear3D" => AffineRole::Sum(diag(w, w, w)),
        "zscale" if planar => AffineRole::Nothing,
        "zscale" => AffineRole::Sum(diag(0.0, 0.0, w)),
        "ztranslate" if planar => AffineRole::Nothing,
        "ztranslate" => AffineRole::Sum(Affine3 { m: [[0.0; 3]; 3], t: [0.0, 0.0, w] }),
        "affine3D" => {
            let a = affine3d_map(t, registry);
            AffineRole::Sum(Affine3 { m: a.m.map(|r| r.map(|v| w * v)), t: a.t.map(|v| w * v) })
        }
        "flatten" if planar => AffineRole::Nothing,
        "flatten" => AffineRole::Post(diag(1.0, 1.0, 0.0)),
        "zcone" | "zblur" if planar => AffineRole::Nothing,
        "pre_rotate_x" | "pre_rotate_y" | "post_rotate_x" | "post_rotate_y" if planar => {
            AffineRole::Nothing
        }
        // pre_rotate_x: (x, s·z + c·y, c·z − s·y); _y: (c·x − s·z, y, s·x + c·z).
        "pre_rotate_x" | "post_rotate_x" | "pre_rotate_y" | "post_rotate_y" => {
            let (sn, cs) = w.sin_cos();
            let m = if name.ends_with('x') {
                [[1.0, 0.0, 0.0], [0.0, cs, sn], [0.0, -sn, cs]]
            } else {
                [[cs, 0.0, -sn], [0.0, 1.0, 0.0], [sn, 0.0, cs]]
            };
            let r = Affine3 { m, t: [0.0; 3] };
            if name.starts_with("pre") { AffineRole::Pre(r) } else { AffineRole::Post(r) }
        }
        _ => return None,
    })
}

// ---------------------------------------------------------------- 2D

/// A 2D affine map `p ↦ M p + t`, row-major.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine2 {
    pub m: [[f64; 2]; 2],
    pub t: [f64; 2],
}

impl Affine2 {
    pub const IDENTITY: Affine2 = Affine2 { m: [[1.0, 0.0], [0.0, 1.0]], t: [0.0, 0.0] };

    pub fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        [
            self.m[0][0] * p[0] + self.m[0][1] * p[1] + self.t[0],
            self.m[1][0] * p[0] + self.m[1][1] * p[1] + self.t[1],
        ]
    }

    /// `self ∘ other`: apply `other` first, then `self`.
    pub fn then_after(&self, other: &Affine2) -> Affine2 {
        let a = &self.m;
        let b = &other.m;
        Affine2 {
            m: [
                [a[0][0] * b[0][0] + a[0][1] * b[1][0], a[0][0] * b[0][1] + a[0][1] * b[1][1]],
                [a[1][0] * b[0][0] + a[1][1] * b[1][0], a[1][0] * b[0][1] + a[1][1] * b[1][1]],
            ],
            t: self.apply(other.t),
        }
    }

    pub fn det(&self) -> f64 {
        self.m[0][0] * self.m[1][1] - self.m[0][1] * self.m[1][0]
    }

    /// The inverse, or `None` when the map is singular (nothing to
    /// invert: the attractor collapses onto a line or a point).
    pub fn inverse(&self) -> Option<Affine2> {
        let d = self.det();
        if !d.is_finite() || d.abs() < 1e-300 {
            return None;
        }
        let inv = [
            [self.m[1][1] / d, -self.m[0][1] / d],
            [-self.m[1][0] / d, self.m[0][0] / d],
        ];
        let t = [
            -(inv[0][0] * self.t[0] + inv[0][1] * self.t[1]),
            -(inv[1][0] * self.t[0] + inv[1][1] * self.t[1]),
        ];
        Some(Affine2 { m: inv, t })
    }

    /// The singular values of `M`, `(σ_min, σ_max)`, in closed form.
    ///
    /// `σ_max` is the map's Lipschitz constant — how much it can
    /// stretch any displacement — and `σ_min` how little. Contractive
    /// means `σ_max < 1`. The determinant is their PRODUCT, which is
    /// why it cannot tell a stretch-and-squash from a uniform scale.
    pub fn singular_values(&self) -> (f64, f64) {
        let [[a, b], [c, d]] = self.m;
        // The two singular values of a 2×2 satisfy
        //   σ² = (‖M‖²_F ± sqrt(‖M‖⁴_F − 4·det²)) / 2.
        let fro2 = a * a + b * b + c * c + d * d;
        let det = a * d - b * c;
        let disc = (fro2 * fro2 - 4.0 * det * det).max(0.0).sqrt();
        let s_max = ((fro2 + disc) * 0.5).max(0.0).sqrt();
        let s_min = ((fro2 - disc) * 0.5).max(0.0).sqrt();
        (s_min, s_max)
    }

    /// The map's fixed point `x = M x + t`, or `None` when `I − M` is
    /// singular (a map with an eigenvalue of exactly 1 has none, or
    /// has a line of them).
    pub fn fixed_point(&self) -> Option<[f64; 2]> {
        let i_minus_m = Affine2 {
            m: [[1.0 - self.m[0][0], -self.m[0][1]], [-self.m[1][0], 1.0 - self.m[1][1]]],
            t: [0.0, 0.0],
        };
        i_minus_m.inverse().map(|inv| inv.apply(self.t))
    }
}

// ---------------------------------------------- the nonlinear maps

/// The nonlinear part of a [`NonlinearMap2`], as the walk inverts it:
/// with `v = post⁻¹(q) / w`, the kernel's inverse takes `v` to the
/// point in the pre-frame, and its local factor multiplies the
/// constant part of the forward map's σ_min (plan §8.8 J3, §8.9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kernel {
    /// `julia` / `julian`: forward `|z|^{d/|n|} · e^{i(arg z + 2πk)/n}`,
    /// every branch undone by `|v|^{|n|/d} · e^{i·n·arg v}` (J1).
    Root { n: i32, d: f64 },
    /// `spherical`: inversion in the circle, its own inverse (S2).
    Spherical,
    /// `bubble`: `4p/(|p|² + 4)`, onto the unit disc and 2-to-1; the
    /// branch picks the inner (0) or outer (1) preimage (S4).
    Bubble,
    /// `hemisphere`: `p / √(|p|² + 1)`, onto the open unit disc,
    /// 1-to-1 (plan §8.10 D1).
    Hemisphere,
    /// `disc`: `(θ/π)·(sin πr, cos πr)` with `θ` from the +y axis --
    /// polar coordinates read the other way. Onto the unit disc and
    /// periodic in `r`, so the branch `m` picks the ring
    /// `r = φ/π + m` (D2).
    Disc,
    /// `blob`: `r·s(θ)·(cos θ, sin θ)`, a reflection in the diagonal
    /// times an angular radial scale `s(θ) = low + (high − low)/2 ·
    /// (sin(waves·θ) + 1)`; onto the plane while `s > 0` (D3).
    Blob { high: f64, low: f64, waves: f64 },
}

impl Kernel {
    /// Blob's angular scale at `theta`.
    fn blob_scale(high: f64, low: f64, waves: f64, theta: f64) -> (f64, f64) {
        let s = low + (high - low) / 2.0 * ((waves * theta).sin() + 1.0);
        let ds = (high - low) / 2.0 * waves * (waves * theta).cos();
        (s, ds)
    }

    /// The forward kernel on `z` in the pre-frame, along `k` -- a
    /// root's branch, or the flame's ε-guarded body for the others.
    pub fn forward(&self, z: [f64; 2], k: u32) -> [f64; 2] {
        let r2 = z[0] * z[0] + z[1] * z[1];
        match *self {
            Kernel::Hemisphere => {
                let t = 1.0 / (r2 + 1.0).sqrt();
                [z[0] * t, z[1] * t]
            }
            Kernel::Disc => {
                // Apophysis: theta = atan2(x, y), the angle from +y.
                let theta = z[0].atan2(z[1]);
                let r = r2.sqrt();
                let rho = theta / std::f64::consts::PI;
                let a = std::f64::consts::PI * r;
                [rho * a.sin(), rho * a.cos()]
            }
            Kernel::Blob { high, low, waves } => {
                let theta = z[0].atan2(z[1]);
                let (sc, _) = Kernel::blob_scale(high, low, waves, theta);
                let r = r2.sqrt();
                [r * sc * theta.cos(), r * sc * theta.sin()]
            }
            Kernel::Root { n, d } => {
                let n = n as f64;
                let rr = r2.sqrt().powf(d / n.abs());
                let a = (z[1].atan2(z[0]) + std::f64::consts::TAU * k as f64) / n;
                [rr * a.cos(), rr * a.sin()]
            }
            Kernel::Spherical => {
                let s = 1.0 / (r2 + 1e-6);
                [z[0] * s, z[1] * s]
            }
            Kernel::Bubble => {
                let s = 4.0 / (r2 + 4.0);
                [z[0] * s, z[1] * s]
            }
        }
    }

    /// The inverse kernel on `v`, along `branch`. A `v` with no
    /// preimage on that branch lands at infinity; the walk never asks
    /// for one, because [`NonlinearMap2::image_gap`] answers first
    /// with the piece's distance (S4, amended).
    pub fn inverse(&self, v: [f64; 2], branch: u32) -> [f64; 2] {
        let r2 = v[0] * v[0] + v[1] * v[1];
        match *self {
            Kernel::Hemisphere => {
                if r2 >= 1.0 {
                    return [v[0] * 1e30, v[1] * 1e30];
                }
                let t = 1.0 / (1.0 - r2).sqrt();
                [v[0] * t, v[1] * t]
            }
            Kernel::Disc => {
                // rho is the input's |theta|/pi; phi, the angle of v
                // from +y, is pi·r modulo 2pi; the ring m and the
                // sign of theta come with the branch (D2).
                let rho = r2.sqrt();
                if rho > 1.0 {
                    return [v[0] * 1e30, v[1] * 1e30];
                }
                let phi = v[0].atan2(v[1]);
                let r = phi / std::f64::consts::PI + branch as f64;
                if r < 0.0 {
                    return [1e30, 1e30];
                }
                let theta = if branch % 2 == 0 { std::f64::consts::PI * rho } else { -std::f64::consts::PI * rho };
                [r * theta.sin(), r * theta.cos()]
            }
            Kernel::Blob { high, low, waves } => {
                let theta = v[1].atan2(v[0]);
                let (sc, _) = Kernel::blob_scale(high, low, waves, theta);
                [v[1] / sc, v[0] / sc]
            }
            Kernel::Root { n, d } => {
                let n = n as f64;
                let rr = r2.sqrt().powf(n.abs() / d);
                let a = n * v[1].atan2(v[0]);
                [rr * a.cos(), rr * a.sin()]
            }
            Kernel::Spherical => {
                let s = 1.0 / r2.max(f64::MIN_POSITIVE);
                [v[0] * s, v[1] * s]
            }
            Kernel::Bubble => {
                if r2 > 1.0 || !(r2 > 0.0) {
                    if r2 > 1.0 {
                        return [v[0] * 1e30, v[1] * 1e30];
                    }
                    // The origin: the inner preimage is the origin,
                    // the outer is at infinity.
                    return if branch == 0 { [0.0, 0.0] } else { [1e30, 0.0] };
                }
                let root = (1.0 - r2).sqrt();
                let f = if branch == 0 { 2.0 - 2.0 * root } else { 2.0 + 2.0 * root };
                let s = f / r2;
                [v[0] * s, v[1] * s]
            }
        }
    }

    /// The factor on the constant σ_min at the point whose image is
    /// `v`, along `branch`.
    pub fn local_sigma_factor(&self, v: [f64; 2], branch: u32) -> f64 {
        let r2 = (v[0] * v[0] + v[1] * v[1]).max(f64::MIN_POSITIVE);
        match *self {
            // The radial singular value, (1 − |v|²)^{3/2}, the smaller
            // of the two and exact (D1).
            Kernel::Hemisphere => (1.0 - r2).max(0.0).powf(1.5),
            // pi·|v| along the input's radius, 1/(pi·r) along its
            // angle, orthogonal images, so the smaller is sigma_min
            // (D2).
            Kernel::Disc => {
                let rho = r2.sqrt();
                let phi = v[0].atan2(v[1]);
                let r = phi / std::f64::consts::PI + branch as f64;
                if r <= 0.0 {
                    return 0.0;
                }
                (std::f64::consts::PI * rho).min(1.0 / (std::f64::consts::PI * r))
            }
            // [[s, s'], [0, s]] in the (radial, tangential) frame: the
            // smaller singular value in closed form (D3).
            Kernel::Blob { high, low, waves } => {
                let theta = v[1].atan2(v[0]);
                let (sc, ds) = Kernel::blob_scale(high, low, waves, theta);
                let a = 2.0 * sc * sc + ds * ds;
                let disc = (a * a - 4.0 * sc.powi(4)).max(0.0).sqrt();
                ((a - disc) * 0.5).max(0.0).sqrt()
            }
            Kernel::Root { n, d } => r2.sqrt().powf(1.0 - (n as f64).abs() / d),
            Kernel::Spherical => r2,
            Kernel::Bubble => {
                // The tangential derivative |v|/|p| (S4).
                if r2 >= 1.0 {
                    return 1.0;
                }
                let root = (1.0 - r2).sqrt();
                let f = if branch == 0 { 2.0 - 2.0 * root } else { 2.0 + 2.0 * root };
                r2 / f
            }
        }
    }

    /// The constant parts of the kernel's singular values, before the
    /// local factor: the root's `min(d,1)/|n|` and `max(d,1)/|n|`;
    /// one for the others.
    fn sigma_const(&self) -> (f64, f64) {
        match *self {
            Kernel::Root { n, d } => {
                // A negative distance is a root of the inverted
                // radius; the derivative's magnitude is what a
                // singular value is.
                let (n, d) = ((n as f64).abs(), d.abs());
                (d.min(1.0) / n, d.max(1.0) / n)
            }
            _ => (1.0, 1.0),
        }
    }

    /// Whether the forward kernel sends a neighbourhood of the
    /// pre-origin to infinity, so that no ball is invariant and the
    /// ball has to be measured (S3): the inversion, and a root with a
    /// negative distance, which is a root of the inverted radius.
    pub fn unbounded_at_origin(&self) -> bool {
        match *self {
            Kernel::Spherical => true,
            Kernel::Root { d, .. } => d < 0.0,
            Kernel::Bubble | Kernel::Hemisphere | Kernel::Disc | Kernel::Blob { .. } => false,
        }
    }

    /// Whether the kernel's image is the unit disc of `v`, so that a
    /// `v` outside it is an image gap away from the piece (S4).
    pub fn image_is_unit_disc(&self) -> bool {
        matches!(self, Kernel::Bubble | Kernel::Hemisphere | Kernel::Disc)
    }

    pub fn variation(&self) -> &'static str {
        match self {
            Kernel::Root { n: 2, d } if *d == 1.0 => "julia",
            Kernel::Root { .. } => "julian",
            Kernel::Spherical => "spherical",
            Kernel::Bubble => "bubble",
            Kernel::Hemisphere => "hemisphere",
            Kernel::Disc => "disc",
            Kernel::Blob { .. } => "blob",
        }
    }
}

/// A transform whose one nonlinear variation the walk can invert
/// (plan §8.8, §8.9): forward `p ↦ post(w · K(pre(p)))`, inverse
/// `q ↦ pre⁻¹(K⁻¹(post⁻¹(q) / w))` along this map's `branch`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonlinearMap2 {
    pub kernel: Kernel,
    /// Which preimage this map follows, of `kernel.branches()`.
    pub branch: u32,
    /// The affine applied before the kernel: the transform's affine
    /// composed with its pre-phase variations.
    pub pre: Affine2,
    /// The affine applied after: the post-phase variations composed
    /// with the post-affine.
    pub post: Affine2,
    pub pre_inv: Affine2,
    pub post_inv: Affine2,
    /// The variation's weight; the kernel's output is scaled by it.
    pub w: f64,
}

impl NonlinearMap2 {
    /// The forward map along `k` (a root's branch; the others ignore
    /// it).
    pub fn apply_branch(&self, p: [f64; 2], k: u32) -> [f64; 2] {
        let z = self.kernel.forward(self.pre.apply(p), k);
        self.post.apply([self.w * z[0], self.w * z[1]])
    }

    /// The point before the kernel's inverse, `post⁻¹(q) / w`.
    fn before_kernel(&self, q: [f64; 2]) -> [f64; 2] {
        let v = self.post_inv.apply(q);
        [v[0] / self.w, v[1] / self.w]
    }

    /// The inverse along this map's branch.
    pub fn apply_inverse(&self, q: [f64; 2]) -> [f64; 2] {
        let u = self.kernel.inverse(self.before_kernel(q), self.branch);
        if !(u[0].is_finite() && u[1].is_finite()) || u[0].abs() > 1e29 || u[1].abs() > 1e29 {
            return [f64::INFINITY, f64::INFINITY];
        }
        self.pre_inv.apply(u)
    }

    /// The local factor on the forward map's σ_min at the point whose
    /// image is `q`.
    pub fn local_sigma_factor(&self, q: [f64; 2]) -> f64 {
        self.kernel.local_sigma_factor(self.before_kernel(q), self.branch)
    }

    /// When `q` is outside this map's IMAGE, a lower bound on its
    /// distance to the image -- and so to this map's piece of the set
    /// -- in `q`'s own frame; `None` when `q` is inside it (S4).
    ///
    /// Only `bubble` has one: its image is the disc `|v| ≤ 1`, so a
    /// `q` with `|v| > 1` has no preimage on either branch, and its
    /// distance to the piece is at least `(|v| − 1)` scaled back
    /// through `w` and the post-affine's smallest stretch. Reporting
    /// "no preimage" as infinitely far was wrong by exactly this: the
    /// piece is not far, it is just not reachable by inversion.
    pub fn image_gap(&self, q: [f64; 2]) -> Option<f64> {
        if !self.kernel.image_is_unit_disc() {
            return None;
        }
        let v = self.before_kernel(q);
        let r = v[0].hypot(v[1]);
        if r > 1.0 {
            let (post_lo, _) = self.post.singular_values();
            Some((r - 1.0) * self.w.abs() * post_lo)
        } else {
            None
        }
    }

    /// How many branches this map has, given the ball: bubble's two;
    /// disc's rings up to the ball's reach in the pre-frame,
    /// `⌊r_max⌋ + 2` with `r_max = |pre(c)| + σ_max(pre)·R`, capped
    /// at twelve (D2); one for the rest.
    pub fn branch_count(&self, ball: &Ball<[f64; 2]>) -> u32 {
        match self.kernel {
            Kernel::Bubble => 2,
            Kernel::Disc => {
                let c = self.pre.apply(ball.centre);
                let (_, pre_hi) = self.pre.singular_values();
                let r_max = c[0].hypot(c[1]) + pre_hi * ball.radius;
                (r_max.floor() as u32 + 2).min(12)
            }
            _ => 1,
        }
    }

    /// The constant parts of the forward map's singular values:
    /// `σ(post) · |w| · σ(pre)` times the kernel's constants, to be
    /// multiplied by the local factor.
    pub fn singular_values(&self) -> (f64, f64) {
        let (pre_lo, pre_hi) = self.pre.singular_values();
        let (post_lo, post_hi) = self.post.singular_values();
        let (k_lo, k_hi) = self.kernel.sigma_const();
        let w = self.w.abs();
        (post_lo * w * pre_lo * k_lo, post_hi * w * pre_hi * k_hi)
    }
}

/// What a 2D map is: the affine case, or a nonlinear map (J8), in
/// either direction. `Nonlinear` applies the forward map's principal
/// branch; `NonlinearInverse` the inverse along the map's branch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Map2 {
    Affine(Affine2),
    Nonlinear(NonlinearMap2),
    NonlinearInverse(NonlinearMap2),
}

impl Map2 {
    pub fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        match self {
            Map2::Affine(a) => a.apply(p),
            Map2::Nonlinear(r) => r.apply_branch(p, 0),
            Map2::NonlinearInverse(r) => r.apply_inverse(p),
        }
    }

    pub fn inverse(&self) -> Option<Map2> {
        match self {
            Map2::Affine(a) => a.inverse().map(Map2::Affine),
            Map2::Nonlinear(r) => Some(Map2::NonlinearInverse(*r)),
            Map2::NonlinearInverse(r) => Some(Map2::Nonlinear(*r)),
        }
    }

    /// The singular values, or for a nonlinear map the constant parts
    /// of them.
    pub fn singular_values(&self) -> (f64, f64) {
        match self {
            Map2::Affine(a) => a.singular_values(),
            Map2::Nonlinear(r) | Map2::NonlinearInverse(r) => r.singular_values(),
        }
    }

    /// An affine map's fixed point; a nonlinear map has no closed form.
    pub fn fixed_point(&self) -> Option<[f64; 2]> {
        match self {
            Map2::Affine(a) => a.fixed_point(),
            _ => None,
        }
    }

    pub fn as_affine(&self) -> Option<Affine2> {
        match self {
            Map2::Affine(a) => Some(*a),
            _ => None,
        }
    }

    pub fn is_affine(&self) -> bool {
        matches!(self, Map2::Affine(_))
    }

    /// The nonlinear map behind a map, either direction.
    pub fn nonlinear(&self) -> Option<&NonlinearMap2> {
        match self {
            Map2::Affine(_) => None,
            Map2::Nonlinear(r) | Map2::NonlinearInverse(r) => Some(r),
        }
    }
}

/// Whether the criterion checks a map's σ_max against 1. An affine
/// map contracts or does not; a root map expands near its critical
/// point and contracts far from it, and whether the IFS is bounded is
/// the ball's question (J5), not this one.
pub trait MapKind {
    fn contraction_is_checked(&self) -> bool;
}

impl MapKind for Affine2 {
    fn contraction_is_checked(&self) -> bool {
        true
    }
}

impl MapKind for Affine3 {
    fn contraction_is_checked(&self) -> bool {
        true
    }
}

impl MapKind for Map2 {
    fn contraction_is_checked(&self) -> bool {
        self.is_affine()
    }
}

impl MapKind for Map3 {
    fn contraction_is_checked(&self) -> bool {
        self.is_affine()
    }
}

// ------------------------------------------------ the 3D nonlinear maps

/// The nonlinear part of a [`NonlinearMap3`] (plan §8.11 step 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kernel3 {
    /// `julia3D`: a root whose radius is the 3D radius with `z` scaled
    /// by `1/|n|`, whose elevation is kept and whose azimuth is
    /// divided by `n`.
    Root3 { n: i32 },
    /// `julia3Dz`: the plane's root on `xy`, with `z` scaled by
    /// `r^{1/n − 1}/|n|`.
    RootZ3 { n: i32 },
    /// `quaternion_julia` in inverse mode (plan §8.11 step 3): the
    /// forward map is the root `(q − c)^{1/n}` with radius exponent
    /// `d/n`, on the quaternion `(xyz, w)` whose scalar `w` is the
    /// walk's `aux`; the walk's inverse is `qⁿ + c`.
    Quaternion { n: i32, d: f64, c: [f64; 4] },
}

/// The Hamilton product of `(x, y, z, w)` quaternions, scalar `w`.
fn qmul(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    let (av, aw) = ([a[0], a[1], a[2]], a[3]);
    let (bv, bw) = ([b[0], b[1], b[2]], b[3]);
    let cross = [av[1] * bv[2] - av[2] * bv[1], av[2] * bv[0] - av[0] * bv[2], av[0] * bv[1] - av[1] * bv[0]];
    [
        aw * bv[0] + bw * av[0] + cross[0],
        aw * bv[1] + bw * av[1] + cross[1],
        aw * bv[2] + bw * av[2] + cross[2],
        aw * bw - (av[0] * bv[0] + av[1] * bv[1] + av[2] * bv[2]),
    ]
}

/// A quaternion `(x, y, z, w)`, scalar part `w`, to an INTEGER power
/// by Hamilton products -- exact, where the polar form's `acos(w/|q|)`
/// loses half its digits near the real axis, which is exactly where
/// the walk's first inverse step lands every slice point (`(xyz, 0)²`
/// is real). A negative power is the conjugate's, over `|q|^{2|n|}`.
fn qpow(q: [f64; 4], n: f64) -> [f64; 4] {
    let k = n.round() as i32;
    let mut base = if k < 0 {
        let m2 = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3];
        if m2 < 1e-300 {
            return [0.0; 4];
        }
        [-q[0] / m2, -q[1] / m2, -q[2] / m2, q[3] / m2]
    } else {
        q
    };
    let mut e = k.unsigned_abs();
    let mut out = [0.0, 0.0, 0.0, 1.0];
    while e > 0 {
        if e & 1 == 1 {
            out = qmul(out, base);
        }
        base = qmul(base, base);
        e >>= 1;
    }
    out
}

/// One of the `|n|` quaternion roots of `q`, branch `k`, with radius
/// exponent `d/n` -- the variation's `qjulia_qroot`.
fn qroot(q: [f64; 4], n: f64, k: u32, d: f64) -> [f64; 4] {
    let mag = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if mag < 1e-300 {
        return [0.0; 4];
    }
    let rad = mag.powf(d / n);
    let ang = ((q[3] / mag).clamp(-1.0, 1.0).acos() + std::f64::consts::TAU * k as f64) / n;
    let vlen = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
    let nhat = if vlen > 1e-300 { [q[0] / vlen, q[1] / vlen, q[2] / vlen] } else { [1.0, 0.0, 0.0] };
    let sn = rad * ang.sin();
    [sn * nhat[0], sn * nhat[1], sn * nhat[2], rad * ang.cos()]
}

impl Kernel3 {
    pub fn power(&self) -> i32 {
        match *self {
            Kernel3::Root3 { n } | Kernel3::RootZ3 { n } | Kernel3::Quaternion { n, .. } => n,
        }
    }

    /// The forward kernel on `z` in the pre-frame with the scalar
    /// `aux`, along branch `k`; the 3D roots pass `aux` through.
    pub fn forward(&self, z: [f64; 3], aux: f64, k: u32) -> ([f64; 3], f64) {
        let n = self.power() as f64;
        let r2d = z[0] * z[0] + z[1] * z[1];
        let a = (z[1].atan2(z[0]) + std::f64::consts::TAU * k as f64) / n;
        match *self {
            Kernel3::Quaternion { d, c, .. } => {
                let q = [z[0] - c[0], z[1] - c[1], z[2] - c[2], aux - c[3]];
                let r = qroot(q, n, k, d);
                ([r[0], r[1], r[2]], r[3])
            }
            Kernel3::Root3 { .. } => {
                let zz = z[2] / n.abs();
                let rho = (r2d + zz * zz).sqrt();
                if rho == 0.0 {
                    return ([0.0; 3], aux);
                }
                let f = rho.powf(1.0 / n - 1.0);
                let rxy = r2d.sqrt();
                ([f * rxy * a.cos(), f * rxy * a.sin(), f * zz], aux)
            }
            Kernel3::RootZ3 { .. } => {
                if r2d == 0.0 {
                    return ([0.0; 3], aux);
                }
                let r = r2d.powf(1.0 / (2.0 * n));
                let z_out = r * z[2] / (r2d.sqrt() * n.abs());
                ([r * a.cos(), r * a.sin(), z_out], aux)
            }
        }
    }

    /// The inverse kernel on `v` with the scalar `aux`, single-valued.
    pub fn inverse(&self, v: [f64; 3], aux: f64) -> ([f64; 3], f64) {
        let n = self.power() as f64;
        let rxy = v[0].hypot(v[1]);
        let theta = n * v[1].atan2(v[0]);
        match *self {
            Kernel3::Quaternion { d, c, .. } => {
                // The polynomial: the root's radius exponent d/n undone
                // by n/d on the magnitude, the angle by n.
                let q = [v[0], v[1], v[2], aux];
                let mag = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
                let p = qpow(q, n);
                let scale = if mag > 1e-300 { mag.powf(n / d) / mag.powf(n) } else { 0.0 };
                ([p[0] * scale + c[0], p[1] * scale + c[1], p[2] * scale + c[2]], p[3] * scale + c[3])
            }
            Kernel3::Root3 { .. } => {
                let rho_out = (rxy * rxy + v[2] * v[2]).sqrt();
                if rho_out == 0.0 {
                    return ([0.0; 3], aux);
                }
                let rho = rho_out.powf(n);
                let (cos_e, sin_e) = (rxy / rho_out, v[2] / rho_out);
                let sqrt_r2d = rho * cos_e;
                let zz = rho * sin_e;
                ([sqrt_r2d * theta.cos(), sqrt_r2d * theta.sin(), zz * n.abs()], aux)
            }
            Kernel3::RootZ3 { .. } => {
                if rxy == 0.0 {
                    return ([0.0; 3], aux);
                }
                let sqrt_r2d = rxy.powf(n);
                ([sqrt_r2d * theta.cos(), sqrt_r2d * theta.sin(), v[2] * n.abs() * rxy.powf(n - 1.0)], aux)
            }
        }
    }

    /// The factor on the constant σ_min at the point whose image is
    /// `v` with scalar `aux`.
    pub fn local_sigma_factor(&self, v: [f64; 3], aux: f64) -> f64 {
        let n = self.power() as f64;
        match *self {
            // The plane's root formula on the 4D magnitude (step 3):
            // |v|^(1 - n/d), the transverse directions being no
            // smaller a stretch than the root's own plane.
            Kernel3::Quaternion { d, .. } => {
                let mag = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + aux * aux).sqrt().max(f64::MIN_POSITIVE);
                mag.powf(1.0 - n / d)
            }
            Kernel3::Root3 { .. } => {
                let rho_out = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(f64::MIN_POSITIVE);
                rho_out.powf(1.0 - n)
            }
            Kernel3::RootZ3 { .. } => {
                // In the (r, theta, z) frame: the 2D root's a on xy, c on
                // z, and a shear b from z into r; the smaller singular
                // value of [[a, 0], [b, c]] against a.
                let rxy = v[0].hypot(v[1]).max(f64::MIN_POSITIVE);
                let r = rxy.powf(n);
                let z = v[2] * n.abs() * rxy.powf(n - 1.0);
                let a = r.powf(1.0 / n - 1.0) / n.abs();
                let c = a;
                let b = z * (1.0 / n - 1.0) * r.powf(1.0 / n - 2.0) / n.abs();
                let s = a * a + b * b + c * c;
                let disc = (s * s - 4.0 * a * a * c * c).max(0.0).sqrt();
                ((s - disc) * 0.5).max(0.0).sqrt().min(a)
            }
        }
    }

    /// The constant parts of the kernel's singular values.
    fn sigma_const(&self) -> (f64, f64) {
        match *self {
            Kernel3::Root3 { n } => {
                let n = (n as f64).abs();
                (1.0 / (n * n), 1.0)
            }
            Kernel3::RootZ3 { .. } => (1.0, 1.0),
            Kernel3::Quaternion { n, d, .. } => {
                let (n, d) = ((n as f64).abs(), d.abs());
                (d.min(1.0) / n, d.max(1.0) / n)
            }
        }
    }

    pub fn unbounded_at_origin(&self) -> bool {
        match *self {
            Kernel3::Quaternion { d, .. } => d < 0.0,
            _ => self.power() < 0,
        }
    }

    /// Whether the kernel reads and writes the walk's scalar `aux`.
    pub fn uses_aux(&self) -> bool {
        matches!(self, Kernel3::Quaternion { .. })
    }

    pub fn variation(&self) -> &'static str {
        match self {
            Kernel3::Root3 { .. } => "julia3D",
            Kernel3::RootZ3 { .. } => "julia3Dz",
            Kernel3::Quaternion { .. } => "quaternion_julia",
        }
    }
}

/// A 3D transform whose one nonlinear variation the walk can invert:
/// forward `p ↦ post(w · K(pre(p)))`, inverse
/// `q ↦ pre⁻¹(K⁻¹(post⁻¹(q) / w))`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonlinearMap3 {
    pub kernel: Kernel3,
    pub pre: Affine3,
    pub post: Affine3,
    pub pre_inv: Affine3,
    pub post_inv: Affine3,
    pub w: f64,
}

impl NonlinearMap3 {
    /// The forward map along `k`, carrying the scalar `aux`. The
    /// affines act on xyz; the weight scales the kernel's whole
    /// quaternion, as the variation's weighted sum does.
    pub fn apply_branch_aux(&self, p: [f64; 3], aux: f64, k: u32) -> ([f64; 3], f64) {
        let (z, aux) = self.kernel.forward(self.pre.apply(p), aux, k);
        (self.post.apply([self.w * z[0], self.w * z[1], self.w * z[2]]), self.w * aux)
    }

    pub fn apply_branch(&self, p: [f64; 3], k: u32) -> [f64; 3] {
        self.apply_branch_aux(p, 0.0, k).0
    }

    fn before_kernel(&self, q: [f64; 3], aux: f64) -> ([f64; 3], f64) {
        let v = self.post_inv.apply(q);
        ([v[0] / self.w, v[1] / self.w, v[2] / self.w], aux / self.w)
    }

    pub fn apply_inverse_aux(&self, q: [f64; 3], aux: f64) -> ([f64; 3], f64) {
        let (v, a) = self.before_kernel(q, aux);
        let (u, a) = self.kernel.inverse(v, a);
        if !(u[0].is_finite() && u[1].is_finite() && u[2].is_finite() && a.is_finite()) {
            return ([f64::INFINITY; 3], a);
        }
        (self.pre_inv.apply(u), a)
    }

    pub fn apply_inverse(&self, q: [f64; 3]) -> [f64; 3] {
        self.apply_inverse_aux(q, 0.0).0
    }

    pub fn local_sigma_factor(&self, q: [f64; 3], aux: f64) -> f64 {
        let (v, a) = self.before_kernel(q, aux);
        self.kernel.local_sigma_factor(v, a)
    }

    pub fn singular_values(&self) -> (f64, f64) {
        let (pre_lo, pre_hi) = self.pre.singular_values();
        let (post_lo, post_hi) = self.post.singular_values();
        let (k_lo, k_hi) = self.kernel.sigma_const();
        let w = self.w.abs();
        (post_lo * w * pre_lo * k_lo, post_hi * w * pre_hi * k_hi)
    }
}

/// What a 3D map is: the affine case, or a nonlinear map in either
/// direction (plan §8.11 step 2), as the plane's [`Map2`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Map3 {
    Affine(Affine3),
    Nonlinear(NonlinearMap3),
    NonlinearInverse(NonlinearMap3),
}

impl Map3 {
    pub fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        match self {
            Map3::Affine(a) => a.apply(p),
            Map3::Nonlinear(r) => r.apply_branch(p, 0),
            Map3::NonlinearInverse(r) => r.apply_inverse(p),
        }
    }

    pub fn inverse(&self) -> Option<Map3> {
        match self {
            Map3::Affine(a) => a.inverse().map(Map3::Affine),
            Map3::Nonlinear(r) => Some(Map3::NonlinearInverse(*r)),
            Map3::NonlinearInverse(r) => Some(Map3::Nonlinear(*r)),
        }
    }

    pub fn singular_values(&self) -> (f64, f64) {
        match self {
            Map3::Affine(a) => a.singular_values(),
            Map3::Nonlinear(r) | Map3::NonlinearInverse(r) => r.singular_values(),
        }
    }

    pub fn fixed_point(&self) -> Option<[f64; 3]> {
        match self {
            Map3::Affine(a) => a.fixed_point(),
            _ => None,
        }
    }

    pub fn as_affine(&self) -> Option<Affine3> {
        match self {
            Map3::Affine(a) => Some(*a),
            _ => None,
        }
    }

    pub fn is_affine(&self) -> bool {
        matches!(self, Map3::Affine(_))
    }

    pub fn nonlinear(&self) -> Option<&NonlinearMap3> {
        match self {
            Map3::Affine(_) => None,
            Map3::Nonlinear(r) | Map3::NonlinearInverse(r) => Some(r),
        }
    }
}

// ---------------------------------------------------------------- 3D

/// A 3D affine map `p ↦ M p + t`, row-major.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine3 {
    pub m: [[f64; 3]; 3],
    pub t: [f64; 3],
}

impl Affine3 {
    pub const IDENTITY: Affine3 = Affine3 {
        m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: [0.0, 0.0, 0.0],
    };

    pub fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        let m = &self.m;
        [
            m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + self.t[0],
            m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + self.t[1],
            m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + self.t[2],
        ]
    }

    /// `self ∘ other`: apply `other` first, then `self`.
    pub fn then_after(&self, other: &Affine3) -> Affine3 {
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..3).map(|k| self.m[i][k] * other.m[k][j]).sum();
            }
        }
        Affine3 { m, t: self.apply(other.t) }
    }

    pub fn det(&self) -> f64 {
        let m = &self.m;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    pub fn inverse(&self) -> Option<Affine3> {
        let d = self.det();
        if !d.is_finite() || d.abs() < 1e-300 {
            return None;
        }
        let m = &self.m;
        // Adjugate over the determinant.
        let inv = [
            [
                (m[1][1] * m[2][2] - m[1][2] * m[2][1]) / d,
                (m[0][2] * m[2][1] - m[0][1] * m[2][2]) / d,
                (m[0][1] * m[1][2] - m[0][2] * m[1][1]) / d,
            ],
            [
                (m[1][2] * m[2][0] - m[1][0] * m[2][2]) / d,
                (m[0][0] * m[2][2] - m[0][2] * m[2][0]) / d,
                (m[0][2] * m[1][0] - m[0][0] * m[1][2]) / d,
            ],
            [
                (m[1][0] * m[2][1] - m[1][1] * m[2][0]) / d,
                (m[0][1] * m[2][0] - m[0][0] * m[2][1]) / d,
                (m[0][0] * m[1][1] - m[0][1] * m[1][0]) / d,
            ],
        ];
        let linear = Affine3 { m: inv, t: [0.0; 3] };
        let mt = linear.apply(self.t);
        Some(Affine3 { m: inv, t: [-mt[0], -mt[1], -mt[2]] })
    }

    /// The singular values of `M`, `(σ_min, σ_max)`.
    ///
    /// The squares of the singular values are the eigenvalues of the
    /// symmetric matrix `MᵀM`, found in closed form (the trigonometric
    /// solution of the characteristic cubic for a real symmetric 3×3,
    /// which is exact and has no iteration to converge).
    pub fn singular_values(&self) -> (f64, f64) {
        let m = &self.m;
        // A = MᵀM, symmetric.
        let mut a = [[0.0f64; 3]; 3];
        for (i, row) in a.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..3).map(|k| m[k][i] * m[k][j]).sum();
            }
        }
        let eig = symmetric3_eigenvalues(&a);
        let lo = eig.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
        let hi = eig.iter().cloned().fold(0.0f64, f64::max).max(0.0);
        (lo.sqrt(), hi.sqrt())
    }

    pub fn fixed_point(&self) -> Option<[f64; 3]> {
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = if i == j { 1.0 - self.m[i][j] } else { -self.m[i][j] };
            }
        }
        Affine3 { m, t: [0.0; 3] }.inverse().map(|inv| inv.apply(self.t))
    }
}

/// Eigenvalues of a real symmetric 3×3, in closed form.
///
/// Smith's trigonometric method: shift by the mean, scale by the
/// deviation, and the three roots are `2 cos((φ + 2πk)/3)` for the
/// angle the reduced cubic gives. Exact for a symmetric matrix, which
/// always has three real eigenvalues; the clamp on the cosine argument
/// absorbs the rounding that can push it a hair past ±1.
fn symmetric3_eigenvalues(a: &[[f64; 3]; 3]) -> [f64; 3] {
    let p1 = a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2];
    if p1 == 0.0 {
        // Already diagonal.
        return [a[0][0], a[1][1], a[2][2]];
    }
    let q = (a[0][0] + a[1][1] + a[2][2]) / 3.0;
    let p2 = (a[0][0] - q).powi(2) + (a[1][1] - q).powi(2) + (a[2][2] - q).powi(2) + 2.0 * p1;
    let p = (p2 / 6.0).sqrt();
    // B = (A − qI) / p
    let mut b = [[0.0f64; 3]; 3];
    for (i, row) in b.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (a[i][j] - if i == j { q } else { 0.0 }) / p;
        }
    }
    let det_b = b[0][0] * (b[1][1] * b[2][2] - b[1][2] * b[2][1])
        - b[0][1] * (b[1][0] * b[2][2] - b[1][2] * b[2][0])
        + b[0][2] * (b[1][0] * b[2][1] - b[1][1] * b[2][0]);
    let r = (det_b / 2.0).clamp(-1.0, 1.0);
    let phi = r.acos() / 3.0;
    let two_pi_3 = 2.0 * std::f64::consts::PI / 3.0;
    [
        q + 2.0 * p * phi.cos(),
        q + 2.0 * p * (phi + 2.0 * two_pi_3).cos(),
        q + 2.0 * p * (phi + two_pi_3).cos(),
    ]
}

// ---------------------------------------------------------- composition

/// Why a transform is not one affine map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotAffine {
    /// A variation [`affine_role`] cannot answer for in this space.
    Variation(String),
    /// An affine variation whose `fx_priority` moved it out of the
    /// weighted sum, so it composes instead of summing. Rare, and the
    /// composition would still be affine — deliberately not derived in
    /// this version rather than derived wrongly.
    Priority(String),
    /// No variation at all: the chaos game's normal result is the
    /// origin, so the map is constant. Affine, but degenerate; the
    /// attractor of a constant map is a point.
    NoVariations,
    /// A root variation summed with something else in the normal
    /// phase: a weighted sum of a root and an affine has no
    /// closed-form inverse (plan §8.4, J4).
    MixedSum(String),
    /// A root variation with a power or distance of zero, which is
    /// not a map with an inverse.
    Degenerate(String),
    /// A variation in a mode the walk does not invert: a
    /// `quaternion_julia` in forward mode or with a projection other
    /// than the vector one (plan §8.11 step 3).
    Mode(String),
}

/// `affine3D`'s map from its fifteen parameters, mirroring the
/// shader's body exactly: scale, then shear (the shear terms multiply
/// the SCALED coordinates), then the rotation the body writes out
/// longhand — which is `Rz · Ry · Rx` — then the translation. The
/// variation's weight multiplies every output term, translation
/// included ("cpp uses VVAR consistently on every output term"), so
/// the caller scales the whole thing.
fn affine3d_map(t: &Transform, registry: &VariationRegistry) -> Affine3 {
    let p = |name: &str| t.get_variation_param_or_default("affine3D", name, registry) as f64;
    let (tx, ty, tz) = (p("translateX"), p("translateY"), p("translateZ"));
    let (sx, sy, sz) = (p("scaleX"), p("scaleY"), p("scaleZ"));
    let d2r = std::f64::consts::PI / 180.0;
    let (rx, ry, rz) = (p("rotateX") * d2r, p("rotateY") * d2r, p("rotateZ") * d2r);
    let (shxy, shxz, shyx, shyz, shzx, shzy) =
        (p("shearXY"), p("shearXZ"), p("shearYX"), p("shearYZ"), p("shearZX"), p("shearZY"));
    // mx = sx·x + shxy·sy·y + shxz·sz·z, and so on: shear of the
    // scaled point. With every shear zero this is diag(sx, sy, sz),
    // which is the body's no-shear branch.
    let shs = Affine3 {
        m: [
            [sx, shxy * sy, shxz * sz],
            [shyx * sx, sy, shyz * sz],
            [shzx * sx, shzy * sy, sz],
        ],
        t: [0.0; 3],
    };
    let (sinx, cosx, siny, cosy, sinz, cosz) = (rx.sin(), rx.cos(), ry.sin(), ry.cos(), rz.sin(), rz.cos());
    // nx = cosz·(cosy·mx + siny·(sinx·my + cosx·mz)) − sinz·(cosx·my − sinx·mz)
    // ny = sinz·(cosy·mx + siny·(sinx·my + cosx·mz)) + cosz·(cosx·my − sinx·mz)
    // nz = −siny·mx + cosy·(sinx·my + cosx·mz)
    let rot = Affine3 {
        m: [
            [cosz * cosy, cosz * siny * sinx - sinz * cosx, cosz * siny * cosx + sinz * sinx],
            [sinz * cosy, sinz * siny * sinx + cosz * cosx, sinz * siny * cosx - cosz * sinx],
            [-siny, cosy * sinx, cosy * cosx],
        ],
        t: [tx, ty, tz],
    };
    rot.then_after(&shs)
}

/// A transform's variations as three affine maps: the pre-phase
/// composition `P`, the weighted normal-phase sum `V`, and the
/// post-phase composition `Q`, so that the variation stage is
/// `q ↦ Q(V(P(q)))`.
struct VariationStage {
    pre: Affine3,
    sum: Affine3,
    post: Affine3,
    /// Whether anything was summed.
    any: bool,
    /// The nonlinear variations met, with their weights (planar only).
    roots: Vec<(&'static str, f64)>,
}

/// The variation stage, or why it is not affine in `space`.
///
/// `order` is the order the shader emits variations in — the flame's
/// first-occurrence order (`Flame::active_variation_names_ordered`),
/// which is what decides how two pre-rotations about different axes
/// compose. A sum does not care; a composition does.
fn variation_stage(
    t: &Transform,
    registry: &VariationRegistry,
    space: Space,
    order: &[String],
) -> Result<VariationStage, NotAffine> {
    let mut stage = VariationStage {
        pre: Affine3::IDENTITY,
        sum: Affine3 { m: [[0.0; 3]; 3], t: [0.0; 3] },
        post: Affine3::IDENTITY,
        any: false,
        roots: Vec::new(),
    };
    let mut any = false;
    // Every active variation, in the shader's order; the transform's
    // own order is the fallback for names the flame order lacks.
    let own = t.ordered_variation_names(registry);
    let names = order.iter().filter(|n| t.variations.contains_key(*n)).chain(own.iter().filter(|n| !order.contains(n)));
    for name in names {
        let w = t.variations.get(name).copied().unwrap_or(0.0) as f64;
        if w == 0.0 {
            continue;
        }
        let Some(role) = affine_role(name, w, t, registry, space) else {
            // A kernel in the plane is a nonlinear map the analysis
            // knows (plan §8.8, §8.9); the transform's kind is decided
            // once the whole stage is known.
            let root = match (name.as_str(), space) {
                ("julia", Space::Planar) => Some("julia"),
                ("julian", Space::Planar) => Some("julian"),
                ("spherical", Space::Planar) => Some("spherical"),
                ("bubble", Space::Planar) => Some("bubble"),
                ("hemisphere", Space::Planar) => Some("hemisphere"),
                ("disc", Space::Planar) => Some("disc"),
                ("blob", Space::Planar) => Some("blob"),
                ("julia3D", Space::Solid) => Some("julia3D"),
                ("julia3Dz", Space::Solid) => Some("julia3Dz"),
                ("quaternion_julia", Space::Solid) => Some("quaternion_julia"),
                _ => None,
            };
            let Some(root) = root else {
                return Err(NotAffine::Variation(name.clone()));
            };
            let summed = match registry.get(name).map(|i| i.phase.clone()) {
                Some(VariationPhase::Any) => {
                    t.variation_priorities.get(name).copied().unwrap_or(0) == 0
                }
                Some(VariationPhase::Normal) | None => true,
                Some(_) => false,
            };
            if !summed {
                return Err(NotAffine::Priority(name.clone()));
            }
            stage.roots.push((root, w));
            continue;
        };
        match role {
            AffineRole::Nothing => {}
            AffineRole::Sum(a) => {
                // The same rule `mean_log_scale` mirrors from the shader
                // builder: `Any`-phase variations sum unless a non-zero
                // priority moved them.
                let summed = match registry.get(name).map(|i| i.phase.clone()) {
                    Some(VariationPhase::Normal) => true,
                    Some(VariationPhase::Any) => {
                        t.variation_priorities.get(name).copied().unwrap_or(0) == 0
                    }
                    Some(_) => false,
                    None => true,
                };
                if !summed {
                    return Err(NotAffine::Priority(name.clone()));
                }
                any = true;
                for i in 0..3 {
                    for j in 0..3 {
                        stage.sum.m[i][j] += a.m[i][j];
                    }
                    stage.sum.t[i] += a.t[i];
                }
            }
            // Later variations apply after earlier ones.
            AffineRole::Pre(r) => stage.pre = r.then_after(&stage.pre),
            AffineRole::Post(r) => stage.post = r.then_after(&stage.post),
        }
    }
    stage.any = any;
    if !any && stage.roots.is_empty() {
        return Err(NotAffine::NoVariations);
    }
    Ok(stage)
}

/// The 2D map a transform composes to -- affine, or a nonlinear map
/// with its kernel -- or why it is neither. A `bubble` returns its
/// inner branch; `analyse_2d` adds the outer (S1).
///
/// A kernel must be ALONE in the normal phase (J4): summed with an
/// affine it has no closed-form inverse. The pre-affine and pre-phase
/// affines compose before it, the post-phase affines and the
/// post-affine after.
pub fn transform_map_2d_ordered(
    t: &Transform,
    registry: &VariationRegistry,
    order: &[String],
) -> Result<Map2, NotAffine> {
    let stage = variation_stage(t, registry, Space::Planar, order)?;
    if stage.roots.is_empty() {
        return transform_affine_2d_ordered(t, registry, order).map(Map2::Affine);
    }
    let (kind, w) = stage.roots[0];
    if stage.roots.len() > 1 || stage.any {
        return Err(NotAffine::MixedSum(kind.to_string()));
    }
    let kernel = match kind {
        "julia" => Kernel::Root { n: 2, d: 1.0 },
        "julian" => {
            let p = |name: &str| t.get_variation_param_or_default("julian", name, registry) as f64;
            let (n, d) = (p("power").round() as i32, p("dist"));
            if n == 0 || !(d != 0.0) || !d.is_finite() {
                return Err(NotAffine::Degenerate(kind.to_string()));
            }
            Kernel::Root { n, d }
        }
        "spherical" => Kernel::Spherical,
        "bubble" => Kernel::Bubble,
        "hemisphere" => Kernel::Hemisphere,
        "disc" => Kernel::Disc,
        "blob" => {
            let p = |name: &str| t.get_variation_param_or_default("blob", name, registry) as f64;
            let (high, low, waves) = (p("high"), p("low"), p("waves"));
            // The scale must stay positive for the preimage to be one
            // point (D3).
            if !(high > 0.0) || !(low > 0.0) || !waves.is_finite() {
                return Err(NotAffine::Degenerate(kind.to_string()));
            }
            Kernel::Blob { high, low, waves }
        }
        _ => unreachable!("collected above"),
    };
    if !(w != 0.0) || !w.is_finite() {
        return Err(NotAffine::Degenerate(kind.to_string()));
    }
    let xy = |a: &Affine3| Affine2 { m: [[a.m[0][0], a.m[0][1]], [a.m[1][0], a.m[1][1]]], t: [a.t[0], a.t[1]] };
    let affine = Affine2 {
        m: [[t.a as f64, t.b as f64], [t.c as f64, t.d as f64]],
        t: [t.e as f64, t.f as f64],
    };
    let pre = xy(&stage.pre).then_after(&affine);
    let mut post = xy(&stage.post);
    if t.post_affine_enabled {
        let post_affine = Affine2 {
            m: [[t.post_a as f64, t.post_b as f64], [t.post_c as f64, t.post_d as f64]],
            t: [t.post_e as f64, t.post_f as f64],
        };
        post = post_affine.then_after(&post);
    }
    // A singular pre or post affine is a singular map; the caller's
    // `inverse()` reports it, so hand back a map whose inverse is None.
    let (Some(pre_inv), Some(post_inv)) = (pre.inverse(), post.inverse()) else {
        return Ok(Map2::Affine(Affine2 { m: [[0.0; 2]; 2], t: [0.0; 2] }));
    };
    Ok(Map2::Nonlinear(NonlinearMap2 { kernel, branch: 0, pre, post, pre_inv, post_inv, w }))
}

/// The 2D affine a transform composes to, or why it does not, with
/// the variations in the transform's own order — which is the
/// shader's whenever the transform is alone in its flame.
pub fn transform_affine_2d(t: &Transform, registry: &VariationRegistry) -> Result<Affine2, NotAffine> {
    transform_affine_2d_ordered(t, registry, &t.ordered_variation_names(registry))
}

/// The 2D affine a transform composes to, or why it does not.
///
/// The z-only variations, the 2D stubs and `flatten` contribute
/// nothing in the plane and are accepted; `linear3D` is the identity
/// in 2D as `linear` is.
pub fn transform_affine_2d_ordered(
    t: &Transform,
    registry: &VariationRegistry,
    order: &[String],
) -> Result<Affine2, NotAffine> {
    let stage = variation_stage(t, registry, Space::Planar, order)?;
    if let Some((kind, _)) = stage.roots.first() {
        return Err(NotAffine::Variation(kind.to_string()));
    }
    // Nothing composes in the plane: every pre/post variation the
    // analysis knows is a stub there.
    debug_assert_eq!(stage.pre, Affine3::IDENTITY);
    debug_assert_eq!(stage.post, Affine3::IDENTITY);
    let sum = stage.sum;
    let affine = Affine2 {
        m: [[t.a as f64, t.b as f64], [t.c as f64, t.d as f64]],
        t: [t.e as f64, t.f as f64],
    };
    // In 2D the chaos game has no z, so the sum's z column multiplies
    // zero and its xy block with the xy translation is the whole map.
    let vsum = Affine2 {
        m: [[sum.m[0][0], sum.m[0][1]], [sum.m[1][0], sum.m[1][1]]],
        t: [sum.t[0], sum.t[1]],
    };
    let mut map = vsum.then_after(&affine);
    if t.post_affine_enabled {
        let post = Affine2 {
            m: [[t.post_a as f64, t.post_b as f64], [t.post_c as f64, t.post_d as f64]],
            t: [t.post_e as f64, t.post_f as f64],
        };
        map = post.then_after(&map);
    }
    Ok(map)
}

/// The flat-or-full 3D affine of `affine_3d.wgsl`, from an XY affine,
/// a z offset, and the two plane maps with their identity flags.
fn plane_affine(
    xy: [[f64; 2]; 2],
    xy_t: [f64; 2],
    g: f64,
    yz: Option<[f64; 6]>,
    zx: Option<[f64; 6]>,
) -> Affine3 {
    match (yz, zx) {
        // The flat path: XY affine, z passes through plus the offset.
        (None, None) => Affine3 {
            m: [[xy[0][0], xy[0][1], 0.0], [xy[1][0], xy[1][1], 0.0], [0.0, 0.0, 1.0]],
            t: [xy_t[0], xy_t[1], g],
        },
        // The full path, exactly as the shader composes it: XY linear,
        // then YZ on (y, z), then ZX on (x, z), then a translation
        // summed from three sources -- and `g` is NOT among them,
        // which the shader documents as a limitation and this mirrors.
        _ => {
            let yz = yz.unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            let zx = zx.unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            let xy3 = Affine3 {
                m: [[xy[0][0], xy[0][1], 0.0], [xy[1][0], xy[1][1], 0.0], [0.0, 0.0, 1.0]],
                t: [0.0; 3],
            };
            // ny = yz[0]·y + yz[2]·z ; nz = yz[1]·y + yz[3]·z
            let yz3 = Affine3 {
                m: [[1.0, 0.0, 0.0], [0.0, yz[0], yz[2]], [0.0, yz[1], yz[3]]],
                t: [0.0; 3],
            };
            // nx = zx[0]·x + zx[2]·z ; nz = zx[1]·x + zx[3]·z
            let zx3 = Affine3 {
                m: [[zx[0], 0.0, zx[2]], [0.0, 1.0, 0.0], [zx[1], 0.0, zx[3]]],
                t: [0.0; 3],
            };
            let mut map = zx3.then_after(&yz3.then_after(&xy3));
            map.t = [xy_t[0] + zx[4], xy_t[1] + yz[4], yz[5] + zx[5]];
            map
        }
    }
}

const IDENTITY_PLANE: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn plane(coefs: [f32; 6]) -> Option<[f64; 6]> {
    if coefs == IDENTITY_PLANE {
        None
    } else {
        Some(coefs.map(|c| c as f64))
    }
}

/// The 3D affine a transform composes to, or why it does not, with
/// the variations in the transform's own order — the shader's whenever
/// the transform is alone in its flame.
pub fn transform_affine_3d(t: &Transform, registry: &VariationRegistry) -> Result<Affine3, NotAffine> {
    transform_affine_3d_ordered(t, registry, &t.ordered_variation_names(registry))
}

/// The 3D affine a transform composes to, or why it does not.
///
/// This is the map the chaos game applies when `preserve_z` is ON:
/// the affine, the pre-phase composition, the sum, the post-phase
/// composition, the post-affine. With `preserve_z` off the trajectory
/// is 2D and this map's z row describes the plotted height, not the
/// dynamics; see [`analyse_2d`].
pub fn transform_affine_3d_ordered(
    t: &Transform,
    registry: &VariationRegistry,
    order: &[String],
) -> Result<Affine3, NotAffine> {
    let stage = variation_stage(t, registry, Space::Solid, order)?;
    if let Some((kind, _)) = stage.roots.first() {
        return Err(NotAffine::Variation(kind.to_string()));
    }
    let affine = plane_affine(
        [[t.a as f64, t.b as f64], [t.c as f64, t.d as f64]],
        [t.e as f64, t.f as f64],
        t.g as f64,
        plane(t.yz_coefs),
        plane(t.zx_coefs),
    );
    let mut map = stage.post.then_after(&stage.sum.then_after(&stage.pre.then_after(&affine)));
    if t.post_affine_enabled {
        let post = plane_affine(
            [[t.post_a as f64, t.post_b as f64], [t.post_c as f64, t.post_d as f64]],
            [t.post_e as f64, t.post_f as f64],
            t.post_g as f64,
            plane(t.yz_post_coefs),
            plane(t.zx_post_coefs),
        );
        map = post.then_after(&map);
    }
    Ok(map)
}

// -------------------------------------------------------------- criterion

/// Why a flame does not qualify. Every variant names the transform
/// (or the flame-level feature) so the panel can say which, and the
/// order here is the order they are worth fixing in.
#[derive(Debug, Clone, PartialEq)]
pub enum Disqualification {
    /// The flame has no transforms.
    Empty,
    /// Transform `index` is not one affine map.
    NotAffine { index: usize, why: NotAffine },
    /// Transform `index`'s map is singular: no inverse.
    Singular { index: usize },
    /// Transform `index` is not contractive: its largest singular value
    /// is `sigma_max ≥ 1`. In 3D under `preserve_z`, an Apophysis-style
    /// transform fails here with `sigma_max == 1.0` exactly, because
    /// its z scale is one.
    NotContractive { index: usize, sigma_max: f64 },
    /// Xaos is set: a graph-directed IFS, whose distance estimate is
    /// not this one (plan §7).
    Xaos,
    /// A final transform is not affine, or is singular. `why` is `None`
    /// for singular.
    FinalNotAffine { why: Option<NotAffine> },
    /// More than one final transform. The composition would be affine
    /// too, but the chaos game picks among them, which is a choice this
    /// analysis does not model.
    MultipleFinals { count: usize },
    /// No ball every map sends into itself was found (plan §8.8 J5):
    /// the root maps do not keep the set bounded.
    NoBall,
}

impl std::fmt::Display for Disqualification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "the flame has no transforms"),
            Self::NotAffine { index, why } => match why {
                NotAffine::Variation(v) => write!(f, "transform {index} uses `{v}`, which is not affine"),
                NotAffine::Priority(v) => write!(f, "transform {index} moves `{v}` out of the weighted sum"),
                NotAffine::NoVariations => write!(f, "transform {index} has no variations"),
                NotAffine::MixedSum(v) => write!(
                    f,
                    "transform {index} sums `{v}` with another variation; a root must be alone in its sum"
                ),
                NotAffine::Degenerate(v) => write!(f, "transform {index}'s `{v}` has a parameter that leaves it no single inverse (a zero power or distance, a scale that reaches zero)"),
                NotAffine::Mode(v) => write!(f, "transform {index}'s `{v}` must be in inverse mode with the vector projection"),
            },
            Self::Singular { index } => write!(f, "transform {index} is singular (no inverse)"),
            Self::NotContractive { index, sigma_max } => {
                write!(f, "transform {index} is not contractive (σ_max = {sigma_max:.3})")
            }
            Self::Xaos => write!(f, "xaos is not supported"),
            Self::FinalNotAffine { why: Some(NotAffine::Variation(v)) } => {
                write!(f, "the final transform uses `{v}`, which is not affine")
            }
            Self::FinalNotAffine { why: Some(w) } => write!(f, "the final transform is not affine ({w:?})"),
            Self::FinalNotAffine { why: None } => write!(f, "the final transform is singular"),
            Self::MultipleFinals { count } => write!(f, "{count} final transforms; at most one is supported"),
            Self::NoBall => write!(f, "no bounding ball: the root maps do not keep the set bounded"),
        }
    }
}

/// One map of a qualifying IFS, with what the distance estimate needs
/// of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IfsMap<A> {
    pub forward: A,
    pub inverse: A,
    pub sigma_min: f64,
    pub sigma_max: f64,
    /// The transform's index in the flame, so a colouring can look up
    /// its colour.
    pub transform_index: usize,
}

/// A ball `B(centre, radius)` that every map of the IFS sends into
/// itself, so the attractor lies inside it and a point outside it is
/// outside the attractor by at least the excess.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ball<P> {
    pub centre: P,
    pub radius: f64,
}

/// A flame that qualifies, as an IFS: its maps, its optional final map
/// (applied after, to the whole attractor), and a bounding ball.
#[derive(Debug, Clone, PartialEq)]
pub struct Ifs<A, P> {
    pub maps: Vec<IfsMap<A>>,
    pub final_map: Option<IfsMap<A>>,
    pub ball: Ball<P>,
    /// The ball centre's scalar coordinate, for a solid whose kernels
    /// carry a fourth one (plan §8.11 step 3); zero otherwise.
    pub aux_centre: f64,
}

pub type Ifs2 = Ifs<Map2, [f64; 2]>;
pub type Ifs3 = Ifs<Map3, [f64; 3]>;

/// Which dynamics to analyse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    /// The 2D IFS — what the chaos game runs in 2D mode, and ALSO what
    /// it runs in 3D mode with `preserve_z` off, where z is zeroed
    /// every iteration and only the plot has depth.
    Planar,
    /// The 3D IFS the chaos game runs with `preserve_z` on.
    Solid,
}

/// The criterion, answering *why not* rather than *whether*.
///
/// Returns every disqualification found rather than the first, so the
/// panel can list them: a flame with two non-affine transforms should
/// say so once, not make the user fix one to discover the next.
pub fn analyse_2d(flame: &Flame, registry: &VariationRegistry) -> Result<Ifs2, Vec<Disqualification>> {
    let order = flame.active_variation_names_ordered(registry);
    let maps = collect(flame, |t| transform_map_2d_ordered(t, registry, &order).map(|a| (a, a.inverse(), a.singular_values())));
    // The final transform stays affine (J4).
    let final_map = collect_final(flame, |t| {
        transform_affine_2d_ordered(t, registry, &order)
            .map(|a| (Map2::Affine(a), a.inverse().map(Map2::Affine), a.singular_values()))
    });
    let (maps, final_map, mut errs) = merge(maps, final_map, flame);
    if !errs.is_empty() {
        return Err(errs);
    }
    // The ball first, on the unexpanded maps -- a forward map does
    // not depend on the branch -- because a disc's branch count is
    // read off it (D2).
    let Some(ball) = ball_2d(&maps) else {
        errs.push(Disqualification::NoBall);
        return Err(errs);
    };
    let aux_centre = 0.0;
    // One map per (transform, branch): a kernel with several
    // preimages is several maps that share a transform and differ in
    // the branch (S1), so the walk's loops and the address keep their
    // shape.
    let maps: Vec<IfsMap<Map2>> = maps
        .into_iter()
        .flat_map(|m| {
            let branches = m.forward.nonlinear().map_or(1, |n| n.branch_count(&ball));
            (0..branches).map(move |b| {
                let mut mb = m;
                if let (Map2::Nonlinear(f), Map2::NonlinearInverse(i)) = (&mut mb.forward, &mut mb.inverse) {
                    f.branch = b;
                    i.branch = b;
                }
                mb
            })
        })
        .collect();
    Ok(Ifs { maps, final_map, ball, aux_centre })
}

/// The 3D criterion, for a flame run with `preserve_z` on. A flame
/// run with it off is a planar IFS and should be analysed as one.
pub fn analyse_3d(flame: &Flame, registry: &VariationRegistry) -> Result<Ifs3, Vec<Disqualification>> {
    let order = flame.active_variation_names_ordered(registry);
    let maps = collect(flame, |t| transform_map_3d_ordered(t, registry, &order).map(|a| (a, a.inverse(), a.singular_values())));
    // The final transform stays affine (J4).
    let final_map = collect_final(flame, |t| {
        transform_affine_3d_ordered(t, registry, &order)
            .map(|a| (Map3::Affine(a), a.inverse().map(Map3::Affine), a.singular_values()))
    });
    let (maps, final_map, mut errs) = merge(maps, final_map, flame);
    if !errs.is_empty() {
        return Err(errs);
    }
    let Some((ball, aux_centre)) = ball_3d(&maps) else {
        errs.push(Disqualification::NoBall);
        return Err(errs);
    };
    Ok(Ifs { maps, final_map, ball, aux_centre })
}

/// The 3D map a transform composes to -- affine, or a nonlinear map
/// with its kernel -- or why it is neither (plan §8.11 step 2). The
/// same rules as the plane's [`transform_map_2d_ordered`].
pub fn transform_map_3d_ordered(
    t: &Transform,
    registry: &VariationRegistry,
    order: &[String],
) -> Result<Map3, NotAffine> {
    let stage = variation_stage(t, registry, Space::Solid, order)?;
    if stage.roots.is_empty() {
        return transform_affine_3d_ordered(t, registry, order).map(Map3::Affine);
    }
    let (kind, w) = stage.roots[0];
    if stage.roots.len() > 1 || stage.any {
        return Err(NotAffine::MixedSum(kind.to_string()));
    }
    let n = t.get_variation_param_or_default(kind, "power", registry).round() as i32;
    if n == 0 || !(w != 0.0) || !w.is_finite() {
        return Err(NotAffine::Degenerate(kind.to_string()));
    }
    let kernel = match kind {
        "julia3D" => Kernel3::Root3 { n },
        "julia3Dz" => Kernel3::RootZ3 { n },
        "quaternion_julia" => {
            let p = |name: &str| t.get_variation_param_or_default(kind, name, registry) as f64;
            if p("inverse") < 0.5 || p("projection").round() != 0.0 {
                return Err(NotAffine::Mode(kind.to_string()));
            }
            let d = p("dist");
            if !(d != 0.0) || !d.is_finite() {
                return Err(NotAffine::Degenerate(kind.to_string()));
            }
            Kernel3::Quaternion { n, d, c: [p("cx"), p("cy"), p("cz"), p("cw")] }
        }
        _ => unreachable!("collected above"),
    };
    let affine = plane_affine(
        [[t.a as f64, t.b as f64], [t.c as f64, t.d as f64]],
        [t.e as f64, t.f as f64],
        t.g as f64,
        plane(t.yz_coefs),
        plane(t.zx_coefs),
    );
    let pre = stage.pre.then_after(&affine);
    let mut post = stage.post;
    if t.post_affine_enabled {
        let post_affine = plane_affine(
            [[t.post_a as f64, t.post_b as f64], [t.post_c as f64, t.post_d as f64]],
            [t.post_e as f64, t.post_f as f64],
            t.post_g as f64,
            plane(t.yz_post_coefs),
            plane(t.zx_post_coefs),
        );
        post = post_affine.then_after(&post);
    }
    let (Some(pre_inv), Some(post_inv)) = (pre.inverse(), post.inverse()) else {
        return Ok(Map3::Affine(Affine3 { m: [[0.0; 3]; 3], t: [0.0; 3] }));
    };
    Ok(Map3::Nonlinear(NonlinearMap3 { kernel, pre, post, pre_inv, post_inv, w }))
}

type Raw<A> = Result<(A, Option<A>, (f64, f64)), NotAffine>;

fn collect<A: Copy>(flame: &Flame, f: impl Fn(&Transform) -> Raw<A>) -> Vec<(usize, Raw<A>)> {
    flame.transforms.iter().enumerate().map(|(i, t)| (i, f(t))).collect()
}

fn collect_final<A: Copy>(flame: &Flame, f: impl Fn(&Transform) -> Raw<A>) -> Vec<Raw<A>> {
    flame.final_transforms.iter().map(f).collect()
}

fn merge<A: Copy + MapKind>(
    maps: Vec<(usize, Raw<A>)>,
    finals: Vec<Raw<A>>,
    flame: &Flame,
) -> (Vec<IfsMap<A>>, Option<IfsMap<A>>, Vec<Disqualification>) {
    let mut errs = Vec::new();
    let mut out = Vec::new();
    if maps.is_empty() {
        errs.push(Disqualification::Empty);
    }
    for (index, raw) in maps {
        match raw {
            Err(why) => errs.push(Disqualification::NotAffine { index, why }),
            Ok((forward, None, _)) => {
                let _ = forward;
                errs.push(Disqualification::Singular { index });
            }
            Ok((forward, Some(inverse), (sigma_min, sigma_max))) => {
                if forward.contraction_is_checked() && !(sigma_max < 1.0) {
                    errs.push(Disqualification::NotContractive { index, sigma_max });
                }
                out.push(IfsMap { forward, inverse, sigma_min, sigma_max, transform_index: index });
            }
        }
    }
    if flame.xaos.is_some() {
        errs.push(Disqualification::Xaos);
    }
    let final_map = match finals.len() {
        0 => None,
        1 => match finals.into_iter().next().expect("len 1") {
            Err(why) => {
                errs.push(Disqualification::FinalNotAffine { why: Some(why) });
                None
            }
            Ok((_, None, _)) => {
                errs.push(Disqualification::FinalNotAffine { why: None });
                None
            }
            Ok((forward, Some(inverse), (sigma_min, sigma_max))) => Some(IfsMap {
                forward,
                inverse,
                sigma_min,
                sigma_max,
                transform_index: usize::MAX,
            }),
        },
        count => {
            errs.push(Disqualification::MultipleFinals { count });
            None
        }
    };
    (out, final_map, errs)
}

// ----------------------------------------------------------------- ball

/// Relative slack on the bounding ball's radius, so points ON the
/// attractor are strictly inside it rather than a rounding error
/// outside. See [`ball_2d`].
pub const BALL_MARGIN: f64 = 1e-9;

/// How many times [`ball_2d`] / [`ball_3d`] refine their first ball.
///
/// The fixed ball's radius bounds a ball each map sends into ITSELF, which
/// is a much stronger property than the estimate needs — the walk only
/// needs the attractor to be inside. So the first ball can be far
/// larger than the set: the Heighway dragon's is 1.707 around a set of
/// circumradius ~0.75, and a ball that loose leaves the greedy branch
/// choice guessing over a region where every branch keeps the point
/// inside.
///
/// Refining is one line of set theory: if `A ⊆ B` then
/// `A = ∪ Sᵢ(A) ⊆ ∪ Sᵢ(B)`, so a ball containing the images is another
/// valid ball, and iterating contracts it toward the attractor's own
/// bounding ball. Every iterate is valid, so the smallest one wins.
const BALL_REFINEMENTS: usize = 64;

/// A ball every map sends into itself, then
/// refined [`BALL_REFINEMENTS`] times toward the attractor.
///
/// For centre `c`, map `S` with Lipschitz `L < 1` sends `B(c, R)` into
/// `B(S(c), L·R)`, which lies inside `B(c, R)` whenever
/// `|S(c) − c| + L·R ≤ R`, i.e. `R ≥ |S(c) − c| / (1 − L)`. The radius is
/// the largest such bound over the maps. The centre is the mean of the
/// maps' fixed points, which lie on the attractor and so sit where the
/// bound is tight; a map without a fixed point (none here, since every
/// map is contractive) would fall back to the origin.
///
/// The radius carries [`BALL_MARGIN`]. The bound above is a supremum
/// the attractor ATTAINS -- the Sierpinski apex sits exactly on the
/// sphere -- so in f64 an attractor point lands a few ulps outside and
/// the distance walk reads it as having escaped at level 0. Growing
/// the ball is always safe (any radius above the bound still satisfies
/// `S(B) subset B`) and costs 1e-9 of relative tightness.
fn ball_2d(maps: &[IfsMap<Map2>]) -> Option<Ball<[f64; 2]>> {
    if maps.iter().all(|m| m.forward.is_affine()) {
        let affine: Vec<IfsMap<Affine2>> = maps
            .iter()
            .map(|m| IfsMap {
                forward: m.forward.as_affine().expect("affine"),
                inverse: m.inverse.as_affine().expect("affine"),
                sigma_min: m.sigma_min,
                sigma_max: m.sigma_max,
                transform_index: m.transform_index,
            })
            .collect();
        return Some(ball_2d_affine(&affine));
    }
    ball_2d_numeric(maps)
}

/// A ball every map sends into itself, found numerically (J5), for
/// an IFS with root maps -- which have no fixed-point formula and no
/// global σ_max.
///
/// The centre is the mean of a short chaos game over the forward maps
/// (a root's branch drawn at random). The radius starts at that
/// sample's extent and grows until every map sends the sampled disc
/// -- its boundary circle and interior rings -- into the disc; a
/// margin of 5% then covers the sampling. A radius that has not
/// settled in sixty rounds is no ball: the maps do not keep the set
/// bounded.
fn ball_2d_numeric(maps: &[IfsMap<Map2>]) -> Option<Ball<[f64; 2]>> {
    // A fixed-seed LCG: the ball must be the same ball every time.
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let branch = |m: &Map2, p: [f64; 2], u: f64| -> [f64; 2] {
        match m {
            Map2::Nonlinear(r) => match r.kernel {
                Kernel::Root { n, .. } => r.apply_branch(p, (u * (n.unsigned_abs() as f64)).floor() as u32),
                _ => r.apply_branch(p, 0),
            },
            other => other.apply(p),
        }
    };

    let mut p = [0.0, 0.0];
    let mut sample = Vec::with_capacity(4000);
    let mut lost = 0usize;
    for i in 0..4200 {
        let m = &maps[(next() * maps.len() as f64).floor() as usize % maps.len()];
        p = branch(&m.forward, p, next());
        if !(p[0].is_finite() && p[1].is_finite()) {
            // A kernel unbounded at its pre-origin sends the odd point
            // to infinity, as the flame's chaos game respawns it; the
            // sample restarts and the point is not kept. A game that
            // keeps leaving has no bulk to measure.
            lost += 1;
            if lost > 400 {
                return None;
            }
            // Not the origin: `disc` sends the origin to itself and a
            // root of a negative distance sends it to infinity, so a
            // game restarted there never leaves (Julian Disc, which
            // is exactly those two maps).
            p = [0.1234, 0.0567];
            continue;
        }
        if i >= 200 {
            sample.push(p);
        }
    }
    if sample.len() < 1000 {
        return None;
    }
    let n = sample.len() as f64;
    let centre = [sample.iter().map(|p| p[0]).sum::<f64>() / n, sample.iter().map(|p| p[1]).sum::<f64>() / n];
    let dist = |p: [f64; 2]| ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2)).sqrt();

    // A set built from a kernel that sends the pre-origin to infinity
    // -- an inversion, a root of a negative distance -- is unbounded
    // through it, and no ball is invariant. Its ball is the BULK of
    // the sample -- the 99.5th percentile radius with a 30% margin --
    // and not a proof (S3): the sparse tail beyond it is drawn as
    // exterior.
    if maps.iter().any(|m| m.forward.nonlinear().is_some_and(|n| n.kernel.unbounded_at_origin())) {
        let mut radii: Vec<f64> = sample.iter().map(|&p| dist(p)).collect();
        radii.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let bulk = radii[(radii.len() as f64 * 0.995) as usize].max(1e-9);
        return Some(Ball { centre, radius: bulk * 1.3 });
    }

    let mut radius = sample.iter().map(|&p| dist(p)).fold(0.0f64, f64::max).max(1e-9);

    // Grow until the sampled disc maps into the disc: sixty rounds of
    // the plain fixed-point iteration, which is what the julia and
    // bubble balls were found by, then twenty more overshooting.
    for round in 0..80 {
        let mut reach = 0.0f64;
        for ring in [1.0f64, 0.75, 0.5, 0.25] {
            let count = if ring == 1.0 { 256 } else { 64 };
            for j in 0..count {
                let a = std::f64::consts::TAU * j as f64 / count as f64;
                let q = [centre[0] + radius * ring * a.cos(), centre[1] + radius * ring * a.sin()];
                for m in maps {
                    let branches = match m.forward.nonlinear().map(|r| r.kernel) {
                        Some(Kernel::Root { n, .. }) => n.unsigned_abs(),
                        _ => 1,
                    };
                    for k in 0..branches {
                        let img = match &m.forward {
                            Map2::Nonlinear(r) => r.apply_branch(q, k),
                            other => other.apply(q),
                        };
                        let d = dist(img);
                        if !d.is_finite() {
                            return None;
                        }
                        reach = reach.max(d);
                    }
                }
            }
        }
        if reach <= radius {
            return Some(Ball { centre, radius: radius * (1.0 + BALL_MARGIN) });
        }
        // The plain iteration `radius = reach` climbs toward its limit
        // from below by a geometric step and can fail to satisfy
        // `reach <= radius` exactly -- a blob IFS was still creeping
        // at 0.51 after sixty rounds. Past sixty, five percent over
        // the reach lands above the limit the moment the maps contract
        // in the large. Not from the start, because the balls the
        // shipped julia presets were framed on came from the plain
        // iteration and an overshoot moves them.
        radius = if round < 60 { reach } else { reach * 1.05 };
    }
    None
}

fn ball_2d_affine(maps: &[IfsMap<Affine2>]) -> Ball<[f64; 2]> {
    let fixed: Vec<[f64; 2]> = maps.iter().filter_map(|m| m.forward.fixed_point()).collect();
    let centre = if fixed.is_empty() {
        [0.0, 0.0]
    } else {
        let n = fixed.len() as f64;
        [fixed.iter().map(|p| p[0]).sum::<f64>() / n, fixed.iter().map(|p| p[1]).sum::<f64>() / n]
    };
    let radius = maps
        .iter()
        .map(|m| {
            let sc = m.forward.apply(centre);
            let d = ((sc[0] - centre[0]).powi(2) + (sc[1] - centre[1]).powi(2)).sqrt();
            d / (1.0 - m.sigma_max)
        })
        .fold(0.0f64, f64::max);

    // Refine: the images' own bounding ball is another valid one.
    let (mut c, mut r) = (centre, radius);
    let (mut best_c, mut best_r) = (centre, radius);
    for _ in 0..BALL_REFINEMENTS {
        let n = maps.len() as f64;
        let images: Vec<[f64; 2]> = maps.iter().map(|m| m.forward.apply(c)).collect();
        let nc = [
            images.iter().map(|p| p[0]).sum::<f64>() / n,
            images.iter().map(|p| p[1]).sum::<f64>() / n,
        ];
        let nr = maps
            .iter()
            .zip(&images)
            .map(|(m, im)| {
                let d = ((im[0] - nc[0]).powi(2) + (im[1] - nc[1]).powi(2)).sqrt();
                d + m.sigma_max * r
            })
            .fold(0.0f64, f64::max);
        c = nc;
        r = nr;
        if r < best_r {
            best_c = c;
            best_r = r;
        }
    }
    Ball { centre: best_c, radius: best_r * (1.0 + BALL_MARGIN) }
}

fn ball_3d(maps: &[IfsMap<Map3>]) -> Option<(Ball<[f64; 3]>, f64)> {
    if maps.iter().all(|m| m.forward.is_affine()) {
        let affine: Vec<IfsMap<Affine3>> = maps
            .iter()
            .map(|m| IfsMap {
                forward: m.forward.as_affine().expect("affine"),
                inverse: m.inverse.as_affine().expect("affine"),
                sigma_min: m.sigma_min,
                sigma_max: m.sigma_max,
                transform_index: m.transform_index,
            })
            .collect();
        return Some((ball_3d_affine(&affine), 0.0));
    }
    ball_3d_numeric(maps)
}

/// The 3D ball for an IFS with nonlinear maps, found as the plane's
/// (J5, S3): a chaos-game sample for the centre, then a radius grown
/// until every map sends the sampled ball -- a Fibonacci sphere and
/// interior shells -- into it, or the sample's bulk when a kernel is
/// unbounded at its pre-origin.
/// Returns the ball and the centre's scalar coordinate. The sample and
/// the search run in four dimensions when a kernel carries the scalar
/// (step 3): the walk's escape radius is the 4D distance, and a 3D
/// slice point's distance to the slice-set is at least its 4D distance
/// to the 4D set.
fn ball_3d_numeric(maps: &[IfsMap<Map3>]) -> Option<(Ball<[f64; 3]>, f64)> {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let four_d = maps.iter().any(|m| m.forward.nonlinear().is_some_and(|n| n.kernel.uses_aux()));
    let branches_of = |m: &Map3| m.nonlinear().map_or(1, |n| n.kernel.power().unsigned_abs());
    let step = |m: &Map3, p: [f64; 3], aux: f64, k: u32| -> ([f64; 3], f64) {
        match m {
            Map3::Nonlinear(r) => r.apply_branch_aux(p, aux, k),
            other => (other.apply(p), aux),
        }
    };

    let mut p = [0.0; 3];
    let mut aux = 0.0;
    let mut sample: Vec<[f64; 4]> = Vec::with_capacity(4000);
    let mut lost = 0usize;
    for i in 0..4200 {
        let m = &maps[(next() * maps.len() as f64).floor() as usize % maps.len()];
        let k = (next() * branches_of(&m.forward) as f64).floor() as u32;
        let (np, na) = step(&m.forward, p, aux, k);
        p = np;
        aux = na;
        if !(p[0].is_finite() && p[1].is_finite() && p[2].is_finite() && aux.is_finite()) {
            lost += 1;
            if lost > 400 {
                return None;
            }
            p = [0.1234, 0.0567, 0.0891];
            aux = 0.0;
            continue;
        }
        if i >= 200 {
            sample.push([p[0], p[1], p[2], aux]);
        }
    }
    if sample.len() < 1000 {
        return None;
    }
    let n = sample.len() as f64;
    let centre4 = [
        sample.iter().map(|p| p[0]).sum::<f64>() / n,
        sample.iter().map(|p| p[1]).sum::<f64>() / n,
        sample.iter().map(|p| p[2]).sum::<f64>() / n,
        if four_d { sample.iter().map(|p| p[3]).sum::<f64>() / n } else { 0.0 },
    ];
    let centre = [centre4[0], centre4[1], centre4[2]];
    let dist = |p: [f64; 4]| {
        ((p[0] - centre4[0]).powi(2) + (p[1] - centre4[1]).powi(2) + (p[2] - centre4[2]).powi(2) + (p[3] - centre4[3]).powi(2)).sqrt()
    };

    if maps.iter().any(|m| m.forward.nonlinear().is_some_and(|n| n.kernel.unbounded_at_origin())) {
        let mut radii: Vec<f64> = sample.iter().map(|&p| dist(p)).collect();
        radii.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let bulk = radii[(radii.len() as f64 * 0.995) as usize].max(1e-9);
        return Some((Ball { centre, radius: bulk * 1.3 }, centre4[3]));
    }

    let mut radius = sample.iter().map(|&p| dist(p)).fold(0.0f64, f64::max).max(1e-9);
    let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
    // Directions: a Fibonacci sphere in 3D, and in 4D a fixed set of
    // pseudo-random unit vectors, which is even enough for a maximum.
    let mut dirs: Vec<[f64; 4]> = Vec::new();
    if four_d {
        let mut st: u64 = 0x1234_5678_9ABC_DEF1;
        let mut rnd = || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (st >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        };
        while dirs.len() < 512 {
            let v = [rnd(), rnd(), rnd(), rnd()];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt();
            if (0.2..=1.0).contains(&l) {
                dirs.push([v[0] / l, v[1] / l, v[2] / l, v[3] / l]);
            }
        }
    } else {
        for j in 0..256 {
            let y = 1.0 - 2.0 * (j as f64 + 0.5) / 256.0;
            let rr = (1.0 - y * y).sqrt();
            let phi = golden * j as f64;
            dirs.push([rr * phi.cos(), y, rr * phi.sin(), 0.0]);
        }
    }
    for round in 0..80 {
        let mut reach = 0.0f64;
        for shell in [1.0f64, 0.7, 0.4] {
            let stride = if shell == 1.0 { 1 } else { 3 };
            for (j, d) in dirs.iter().enumerate() {
                if j % stride != 0 {
                    continue;
                }
                let q = [
                    centre4[0] + radius * shell * d[0],
                    centre4[1] + radius * shell * d[1],
                    centre4[2] + radius * shell * d[2],
                ];
                let qa = centre4[3] + radius * shell * d[3];
                for m in maps {
                    for k in 0..branches_of(&m.forward) {
                        let (img, ia) = step(&m.forward, q, qa, k);
                        let dd = dist([img[0], img[1], img[2], ia]);
                        if !dd.is_finite() {
                            return None;
                        }
                        reach = reach.max(dd);
                    }
                }
            }
        }
        if reach <= radius {
            return Some((Ball { centre, radius: radius * (1.0 + BALL_MARGIN) }, centre4[3]));
        }
        radius = if round < 60 { reach } else { reach * 1.05 };
    }
    None
}

fn ball_3d_affine(maps: &[IfsMap<Affine3>]) -> Ball<[f64; 3]> {
    let fixed: Vec<[f64; 3]> = maps.iter().filter_map(|m| m.forward.fixed_point()).collect();
    let centre = if fixed.is_empty() {
        [0.0; 3]
    } else {
        let n = fixed.len() as f64;
        [
            fixed.iter().map(|p| p[0]).sum::<f64>() / n,
            fixed.iter().map(|p| p[1]).sum::<f64>() / n,
            fixed.iter().map(|p| p[2]).sum::<f64>() / n,
        ]
    };
    let radius = maps
        .iter()
        .map(|m| {
            let sc = m.forward.apply(centre);
            let d = ((sc[0] - centre[0]).powi(2) + (sc[1] - centre[1]).powi(2) + (sc[2] - centre[2]).powi(2)).sqrt();
            d / (1.0 - m.sigma_max)
        })
        .fold(0.0f64, f64::max);

    let (mut c, mut r) = (centre, radius);
    let (mut best_c, mut best_r) = (centre, radius);
    for _ in 0..BALL_REFINEMENTS {
        let n = maps.len() as f64;
        let images: Vec<[f64; 3]> = maps.iter().map(|m| m.forward.apply(c)).collect();
        let nc = [
            images.iter().map(|p| p[0]).sum::<f64>() / n,
            images.iter().map(|p| p[1]).sum::<f64>() / n,
            images.iter().map(|p| p[2]).sum::<f64>() / n,
        ];
        let nr = maps
            .iter()
            .zip(&images)
            .map(|(m, im)| {
                let d = ((im[0] - nc[0]).powi(2)
                    + (im[1] - nc[1]).powi(2)
                    + (im[2] - nc[2]).powi(2))
                .sqrt();
                d + m.sigma_max * r
            })
            .fold(0.0f64, f64::max);
        c = nc;
        r = nr;
        if r < best_r {
            best_c = c;
            best_r = r;
        }
    }
    Ball { centre: best_c, radius: best_r * (1.0 + BALL_MARGIN) }
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variations::global_registry;
    use std::collections::HashMap;

    fn affine_xform(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Transform {
        let mut t = Transform::default();
        t.a = a;
        t.b = b;
        t.c = c;
        t.d = d;
        t.e = e;
        t.f = f;
        t.variations = HashMap::from([("linear".to_string(), 1.0)]);
        t.variation_order = vec!["linear".to_string()];
        t
    }

    fn flame_of(transforms: Vec<Transform>) -> Flame {
        let mut fl = Flame::default();
        fl.transforms = transforms;
        fl.final_transforms.clear();
        fl.xaos = None;
        fl
    }

    /// Inputs are `f32` (a transform's coefficients), so a tolerance
    /// tighter than f32's own rounding would fail on 0.3 itself.
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    // ---- what each variation contributes, per space -----------------

    fn with(mut t: Transform, name: &str, w: f32) -> Transform {
        t.variations.insert(name.to_string(), w);
        t.variation_order.push(name.to_string());
        t
    }

    /// In the plane the chaos game has no z, and a variation that
    /// only writes z -- or whose 2D body returns its input -- does
    /// nothing there. The analysis used to reject them by name; the
    /// census counted five shipped flames lost to `flatten` alone.
    /// With them present the gasket is still the gasket, map for map.
    #[test]
    fn flatten_and_the_z_only_variations_are_nothing_in_the_plane() {
        let guard = global_registry();
        let r = &*guard;
        let plain = flame_of(vec![
            affine_xform(0.5, 0.0, 0.0, 0.5, 0.0, 0.0),
            affine_xform(0.5, 0.0, 0.0, 0.5, 0.5, 0.0),
            affine_xform(0.5, 0.0, 0.0, 0.5, 0.0, 0.5),
        ]);
        let mut dressed = plain.clone();
        dressed.transforms[0] = with(dressed.transforms[0].clone(), "flatten", 1.0);
        dressed.transforms[1] = with(dressed.transforms[1].clone(), "zcone", 0.7);
        dressed.transforms[1] = with(dressed.transforms[1].clone(), "zblur", 0.3);
        dressed.transforms[2] = with(dressed.transforms[2].clone(), "pre_rotate_x", 1.2);
        dressed.transforms[2] = with(dressed.transforms[2].clone(), "post_rotate_y", -0.4);
        dressed.transforms[2] = with(dressed.transforms[2].clone(), "ztranslate", 2.0);
        let a = analyse_2d(&plain, r).expect("the gasket qualifies");
        let b = analyse_2d(&dressed, r).expect("dressed in z-only variations it still does");
        for (x, y) in a.maps.iter().zip(&b.maps) {
            assert_eq!(x.forward, y.forward);
        }
        assert_eq!(a.ball, b.ball);
    }

    /// As a solid `flatten` is affine and singular: it projects to
    /// z = 0 and has no inverse. The criterion says which, rather than
    /// calling it non-affine.
    #[test]
    fn flatten_is_singular_as_a_solid() {
        let guard = global_registry();
        let r = &*guard;
        let mut t = affine_xform(0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
        t.variations.clear();
        t.variation_order.clear();
        let t = with(with(t, "linear3D", 1.0), "flatten", 1.0);
        let m = transform_affine_3d(&t, r).expect("affine");
        assert_eq!(m.apply([1.0, 2.0, 3.0]), [0.5, 1.0, 0.0]);
        let errs = analyse_3d(&flame_of(vec![t]), r).unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, Disqualification::Singular { index: 0 })), "{errs:?}");
        assert!(!errs.iter().any(|e| matches!(e, Disqualification::NotAffine { .. })), "{errs:?}");
    }

    /// A pre-rotation turns the affine's output before the sum and a
    /// post-rotation turns the sum, each by its weight in radians
    /// about its axis, exactly as the bodies write them. Worked by
    /// hand: (1, 2, 3) through a unit affine, pre_rotate_x by π/2
    /// ((x, z, −y) → (1, 3, −2)), a half-scale sum ((0.5, 1.5, −1)),
    /// post_rotate_y by π/2 ((−z, y, x) → (1, 1.5, 0.5)).
    #[test]
    fn a_pre_rotation_turns_the_affines_output_and_a_post_rotation_the_sum() {
        let guard = global_registry();
        let r = &*guard;
        let mut t = affine_xform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        t.variations.clear();
        t.variation_order.clear();
        let half_pi = std::f32::consts::FRAC_PI_2;
        let t = with(with(with(t, "linear3D", 0.5), "pre_rotate_x", half_pi), "post_rotate_y", half_pi);
        let m = transform_affine_3d(&t, r).expect("affine");
        let p = m.apply([1.0, 2.0, 3.0]);
        assert!(close(p[0], 1.0) && close(p[1], 1.5) && close(p[2], 0.5), "{p:?}");
        // Isometries: the singular values are the sum's.
        let (lo, hi) = m.singular_values();
        assert!(close(lo, 0.5) && close(hi, 0.5), "({lo}, {hi})");
        // And it qualifies as a solid, contractive with an inverse.
        let ifs = analyse_3d(&flame_of(vec![t]), r).expect("qualifies");
        let q = [0.3, -0.2, 0.9];
        let back = ifs.maps[0].inverse.apply(ifs.maps[0].forward.apply(q));
        for k in 0..3 {
            assert!(close(back[k], q[k]));
        }
    }

    /// Two pre-rotations about different axes do not commute, and the
    /// shader applies them in the FLAME's first-occurrence order, not
    /// the transform's: `resolve_phase_buckets` walks
    /// `active_variation_names_ordered`. So a transform listing x then
    /// y, in a flame whose first transform listed y then x, rotates
    /// about y first. The analysis takes the order it is given, and
    /// `analyse_3d` gives it the flame's.
    #[test]
    fn two_pre_rotations_compose_in_the_flames_order() {
        let guard = global_registry();
        let r = &*guard;
        let half_pi = std::f32::consts::FRAC_PI_2;
        let base = |names: [&str; 2]| {
            let mut t = affine_xform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
            t.variations.clear();
            t.variation_order.clear();
            let t = with(t, "linear3D", 0.5);
            with(with(t, names[0], half_pi), names[1], half_pi)
        };
        let xy = base(["pre_rotate_x", "pre_rotate_y"]);
        let yx = base(["pre_rotate_y", "pre_rotate_x"]);
        // Alone, each transform's own order is the shader's.
        // x then y: (1,2,3) → (1, 3, −2) → (c·x − s·z, y, s·x + c·z) = (2, 3, 1); half → (1, 1.5, 0.5).
        let a = transform_affine_3d(&xy, r).unwrap().apply([1.0, 2.0, 3.0]);
        assert!(close(a[0], 1.0) && close(a[1], 1.5) && close(a[2], 0.5), "{a:?}");
        // y then x: (1,2,3) → (−3, 2, 1) → (x, z, −y) = (−3, 1, −2); half → (−1.5, 0.5, −1).
        let b = transform_affine_3d(&yx, r).unwrap().apply([1.0, 2.0, 3.0]);
        assert!(close(b[0], -1.5) && close(b[1], 0.5) && close(b[2], -1.0), "{b:?}");

        // In a flame led by the y-then-x transform, the x-then-y one
        // is emitted y first too, and analyses to the same map.
        let fl = flame_of(vec![yx.clone(), xy.clone()]);
        let order = fl.active_variation_names_ordered(r);
        assert_eq!(
            order.iter().position(|n| n == "pre_rotate_y").unwrap() < order.iter().position(|n| n == "pre_rotate_x").unwrap(),
            true
        );
        let ifs = analyse_3d(&fl, r).expect("qualifies");
        let c = ifs.maps[1].forward.apply([1.0, 2.0, 3.0]);
        assert!(close(c[0], b[0]) && close(c[1], b[1]) && close(c[2], b[2]), "{c:?} vs {b:?}");
    }

    // ---- the root maps (plan 8.8) -----------------------------------

    /// One `julia` transform with pre-translation `−c`: forward
    /// `±sqrt(p − c)`, inverse `q² + c`.
    fn julia_xform(c: [f32; 2]) -> Transform {
        let mut t = affine_xform(1.0, 0.0, 0.0, 1.0, -c[0], -c[1]);
        t.variations.clear();
        t.variation_order.clear();
        with(t, "julia", 1.0)
    }

    /// A julia transform is a root map whose single-valued inverse
    /// undoes EVERY branch of the forward map (J1), and whose local
    /// scale factor is `|v|^(1 − |n|/d)`.
    #[test]
    fn a_julia_transform_is_a_root_map_with_one_inverse() {
        let guard = global_registry();
        let r = &*guard;
        let t = julia_xform([0.3, -0.4]);
        let m = transform_map_2d_ordered(&t, r, &t.ordered_variation_names(r)).expect("a root map");
        let Map2::Nonlinear(root) = m else { panic!("expected a root map, got {m:?}") };
        assert_eq!((root.kernel, root.w), (Kernel::Root { n: 2, d: 1.0 }, 1.0));
        // ±sqrt(p − c): both branches square back to p − c, and the
        // inverse returns p.
        let p = [0.7, 0.2];
        for k in 0..2 {
            let q = root.apply_branch(p, k);
            let back = root.apply_inverse(q);
            assert!(close(back[0], p[0]) && close(back[1], p[1]), "branch {k}: {back:?} vs {p:?}");
            // The inverse is q² + c.
            let want = [q[0] * q[0] - q[1] * q[1] + 0.3, 2.0 * q[0] * q[1] - 0.4];
            assert!(close(back[0], want[0]) && close(back[1], want[1]));
        }
        // The two branches differ by a sign.
        let (q0, q1) = (root.apply_branch(p, 0), root.apply_branch(p, 1));
        assert!(close(q0[0], -q1[0]) && close(q0[1], -q1[1]));
        // Local factor at q: |q|^(1 − 2) = 1/|q|, so the forward σ_min
        // there is (1/2)/|q| -- the chain rule of z² + c.
        let q = [0.5, 0.5];
        let (lo, _) = root.singular_values();
        assert!(close(lo, 0.5));
        assert!(close(root.local_sigma_factor(q) * lo, 0.5 / q[0].hypot(q[1])));

        // julian carries its own power and distance.
        let mut j = affine_xform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        j.variations.clear();
        j.variation_order.clear();
        let mut j = with(j, "julian", 0.8);
        j.set_variation_param("julian", "power", 3.0);
        j.set_variation_param("julian", "dist", 1.5);
        let m = transform_map_2d_ordered(&j, r, &j.ordered_variation_names(r)).expect("a root map");
        let root = m.nonlinear().copied().expect("root");
        assert_eq!(root.kernel, Kernel::Root { n: 3, d: 1.5 });
        assert!(close(root.w, 0.8));
        for k in 0..3 {
            let back = root.apply_inverse(root.apply_branch(p, k));
            assert!(close(back[0], p[0]) && close(back[1], p[1]), "branch {k}: {back:?}");
        }
    }

    /// A root summed with an affine has no closed-form inverse (J4);
    /// a root with a power of zero is not a map. Both are said, not
    /// silently treated as affine or as unknown.
    #[test]
    fn a_root_must_be_alone_in_its_sum_and_have_a_power() {
        let guard = global_registry();
        let r = &*guard;
        let mixed = with(julia_xform([0.0, 0.0]), "linear", 0.5);
        let errs = analyse_2d(&flame_of(vec![mixed]), r).unwrap_err();
        assert!(
            errs.iter().any(|e| matches!(e, Disqualification::NotAffine { why: NotAffine::MixedSum(v), .. } if v == "julia")),
            "{errs:?}"
        );
        let mut j = affine_xform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        j.variations.clear();
        j.variation_order.clear();
        let mut j = with(j, "julian", 1.0);
        j.set_variation_param("julian", "power", 0.0);
        let errs = analyse_2d(&flame_of(vec![j]), r).unwrap_err();
        assert!(
            errs.iter().any(|e| matches!(e, Disqualification::NotAffine { why: NotAffine::Degenerate(v), .. } if v == "julian")),
            "{errs:?}"
        );
        // And the affine-only reading still says what it always did.
        let why = transform_affine_2d(&julia_xform([0.0, 0.0]), r).unwrap_err();
        assert_eq!(why, NotAffine::Variation("julia".to_string()));
    }

    /// The ball is found numerically (J5) and holds the set: for
    /// `c = −1` the filled Julia set reaches the fixed point
    /// `(1 + √5)/2 ≈ 1.618` on the real axis, so the ball must reach
    /// past it, and every branch of the map must send the ball into
    /// itself.
    #[test]
    fn a_julia_ifs_gets_a_ball_every_branch_keeps() {
        let guard = global_registry();
        let r = &*guard;
        let ifs = analyse_2d(&flame_of(vec![julia_xform([-1.0, 0.0])]), r).expect("qualifies");
        assert_eq!(ifs.maps.len(), 1);
        let b = ifs.ball;
        let reach = |p: [f64; 2]| (p[0] - b.centre[0]).hypot(p[1] - b.centre[1]);
        let phi = (1.0 + 5f64.sqrt()) / 2.0;
        assert!(reach([phi, 0.0]) <= b.radius, "the ball {b:?} misses the fixed point");
        assert!(b.radius < 6.0, "the ball {b:?} is looser than it should be");
        let root = ifs.maps[0].forward.nonlinear().unwrap();
        for j in 0..360 {
            let a = (j as f64).to_radians();
            let q = [b.centre[0] + b.radius * a.cos(), b.centre[1] + b.radius * a.sin()];
            for k in 0..2 {
                assert!(reach(root.apply_branch(q, k)) <= b.radius * (1.0 + 1e-9), "branch {k} leaves the ball at {j} degrees");
            }
        }
        // A root that does not keep the set bounded has no ball: a
        // julian with dist = 4 on power 2 is |z|² -- the map doubles
        // the exponent and nothing contains it.
        let mut j = affine_xform(1.0, 0.0, 0.0, 1.0, 1.0, 0.0);
        j.variations.clear();
        j.variation_order.clear();
        let mut j = with(j, "julian", 1.0);
        j.set_variation_param("julian", "power", 2.0);
        j.set_variation_param("julian", "dist", 4.0);
        let errs = analyse_2d(&flame_of(vec![j]), r).unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, Disqualification::NoBall)), "{errs:?}");
    }

    /// Gate 2 of plan 8.9: every kernel's inverse undoes each of its
    /// branches, and a bubble transform is two maps that share its
    /// colour and differ in the branch.
    #[test]
    fn every_kernel_inverse_undoes_each_of_its_branches() {
        let guard = global_registry();
        let r = &*guard;
        let mut sph = affine_xform(0.6, 0.2, -0.1, 0.5, 0.3, -0.2);
        sph.variations.clear();
        sph.variation_order.clear();
        let sph = with(sph, "spherical", 0.7);
        let mut bub = affine_xform(0.9, -0.3, 0.3, 0.9, -0.4, 0.1);
        bub.variations.clear();
        bub.variation_order.clear();
        let bub = with(bub, "bubble", 1.3);
        let mut neg = affine_xform(1.0, 0.0, 0.0, 1.0, 0.2, 0.1);
        neg.variations.clear();
        neg.variation_order.clear();
        let mut neg = with(neg, "julian", 0.5);
        neg.set_variation_param("julian", "power", 3.0);
        neg.set_variation_param("julian", "dist", -1.0);

        for (t, kernel) in [(&sph, Kernel::Spherical), (&bub, Kernel::Bubble), (&neg, Kernel::Root { n: 3, d: -1.0 })] {
            let m = transform_map_2d_ordered(t, r, &t.ordered_variation_names(r)).expect("a nonlinear map");
            let base = m.nonlinear().copied().expect("nonlinear");
            assert_eq!(base.kernel, kernel);
            let (lo, hi) = base.singular_values();
            assert!(lo > 0.0 && hi >= lo, "({lo}, {hi})");
            // Points on both sides of bubble's fold circle |pre(p)| = 2,
            // and off the origin for the others.
            for p in [[0.3, 0.4], [-1.7, 2.6], [4.0, -3.5], [0.05, -0.02]] {
                let q = base.apply_branch(p, 0);
                // Which branch of a bubble holds p is decided by |pre(p)|.
                let z = base.pre.apply(p);
                let inner = z[0].hypot(z[1]) <= 2.0;
                let branch = if kernel == Kernel::Bubble && !inner { 1 } else { 0 };
                let mut mb = base;
                mb.branch = branch;
                let back = mb.apply_inverse(q);
                // The inversion's forward keeps the flame's 1e-6 and
                // its inverse drops it (S2): the round trip is off by
                // about ε/|z|², which at |z| ~ 0.5 is 4e-6.
                let tol = if kernel == Kernel::Spherical { 1e-4 } else { 1e-6 };
                assert!((back[0] - p[0]).abs() < tol && (back[1] - p[1]).abs() < tol, "{kernel:?} branch {branch}: {p:?} -> {q:?} -> {back:?}");
                // The local factor is the forward derivative's scale,
                // checked by finite differences along the tangent for
                // bubble and along both axes for the conformal ones.
                let (c_lo, _) = base.singular_values();
                let s = c_lo * mb.local_sigma_factor(q);
                let h = 1e-6;
                let d1 = base.apply_branch([p[0] + h, p[1]], 0);
                let d2 = base.apply_branch([p[0], p[1] + h], 0);
                let g1 = ((d1[0] - q[0]).hypot(d1[1] - q[1])) / h;
                let g2 = ((d2[0] - q[0]).hypot(d2[1] - q[1])) / h;
                if kernel != Kernel::Bubble {
                    // Conformal times affine: the product of parts is a
                    // lower bound on the stretch in any direction.
                    assert!(s <= g1.min(g2) * (1.0 + 1e-4) + 1e-9, "{kernel:?} at {p:?}: sigma {s} exceeds stretch {g1}/{g2}");
                }
            }
        }

        // A bubble transform is two maps of the IFS.
        let ifs = analyse_2d(&flame_of(vec![bub.clone()]), r).expect("qualifies");
        assert_eq!(ifs.maps.len(), 2);
        assert_eq!(ifs.maps[0].transform_index, ifs.maps[1].transform_index);
        assert_eq!(ifs.maps[0].inverse.nonlinear().unwrap().branch, 0);
        assert_eq!(ifs.maps[1].inverse.nonlinear().unwrap().branch, 1);
        // Its ball is invariant: the image is the post-affine of the
        // unit disc scaled by w.
        for j in 0..90 {
            let a = (j as f64 * 4.0).to_radians();
            let q = [ifs.ball.centre[0] + ifs.ball.radius * a.cos(), ifs.ball.centre[1] + ifs.ball.radius * a.sin()];
            let img = ifs.maps[0].forward.apply(q);
            let d = (img[0] - ifs.ball.centre[0]).hypot(img[1] - ifs.ball.centre[1]);
            assert!(d <= ifs.ball.radius * (1.0 + 1e-9));
        }
        // A point outside the unit disc in the pre-frame of the inverse
        // has no preimage on either branch: it lands at infinity.
        let far = ifs.maps[0].inverse.apply(ifs.maps[0].forward.apply([100.0, 0.0]).map(|x| x * 1.5 + 5.0));
        let _ = far;
        let mut mb = ifs.maps[0].inverse.nonlinear().copied().unwrap();
        let q_out = mb.post.apply([2.0 * mb.w, 0.0]);
        for b in 0..2 {
            mb.branch = b;
            let u = mb.apply_inverse(q_out);
            assert!(!u[0].is_finite(), "branch {b} of a point with no preimage should be at infinity, got {u:?}");
        }

        // A spherical IFS gets the measured ball (S3), and a
        // negative-distance root does too.
        let ifs = analyse_2d(&flame_of(vec![sph.clone(), affine_xform(0.5, 0.0, 0.0, 0.5, 1.0, 0.0)]), r).expect("qualifies");
        assert!(ifs.ball.radius > 0.0 && ifs.ball.radius.is_finite());
        let ifs = analyse_2d(&flame_of(vec![neg.clone(), affine_xform(0.5, 0.0, 0.0, 0.5, 1.0, 0.0)]), r).expect("qualifies");
        assert!(ifs.ball.radius > 0.0 && ifs.ball.radius.is_finite());
    }

    /// Plan 8.10 gate 1: hemisphere, disc and blob round-trip, disc
    /// on every ring the ball allows, and each local factor is the
    /// forward map's smallest stretch to a finite difference.
    #[test]
    fn the_fold_kernels_undo_each_of_their_branches() {
        let guard = global_registry();
        let r = &*guard;
        let mut hemi = affine_xform(0.8, 0.1, -0.2, 0.7, 0.2, -0.1);
        hemi.variations.clear();
        hemi.variation_order.clear();
        let hemi = with(hemi, "hemisphere", 1.4);
        let mut disc = affine_xform(0.9, 0.3, -0.3, 0.9, 0.5, 0.2);
        disc.variations.clear();
        disc.variation_order.clear();
        let disc = with(disc, "disc", 1.1);
        let mut blob = affine_xform(0.6, 0.0, 0.0, 0.6, 0.1, 0.3);
        blob.variations.clear();
        blob.variation_order.clear();
        let mut blob = with(blob, "blob", 0.9);
        blob.set_variation_param("blob", "high", 1.4);
        blob.set_variation_param("blob", "low", 0.6);
        blob.set_variation_param("blob", "waves", 5.0);

        for (t, kernel) in [
            (&hemi, Kernel::Hemisphere),
            (&disc, Kernel::Disc),
            (&blob, Kernel::Blob { high: 1.4, low: 0.6, waves: 5.0 }),
        ] {
            let m = transform_map_2d_ordered(t, r, &t.ordered_variation_names(r)).expect("a nonlinear map");
            let base = m.nonlinear().copied().expect("nonlinear");
            let got = base.kernel;
            let same = match (got, kernel) {
                (Kernel::Blob { high: a, low: b, waves: c }, Kernel::Blob { high: x, low: y, waves: z }) => {
                    close(a, x) && close(b, y) && close(c, z)
                }
                (a, b) => a == b,
            };
            assert!(same, "{got:?} vs {kernel:?}");
            for p in [[0.3, 0.4], [-1.7, 2.6], [4.0, -3.5], [0.05, -0.02], [2.9, 0.1]] {
                let q = base.apply_branch(p, 0);
                // Which ring of a disc holds p: r = |pre(p)| on the ring
                // floor(r - phi/pi + 1/2)... simpler, try every ring and
                // require exactly the one that lands back on p.
                let branches = if kernel == Kernel::Disc { 12 } else { 1 };
                let mut hits = 0;
                for b in 0..branches {
                    let mut mb = base;
                    mb.branch = b;
                    let back = mb.apply_inverse(q);
                    if (back[0] - p[0]).abs() < 1e-6 && (back[1] - p[1]).abs() < 1e-6 {
                        hits += 1;
                        // The local factor is a lower bound on the
                        // forward stretch in any direction.
                        let (c_lo, _) = base.singular_values();
                        let sg = c_lo * mb.local_sigma_factor(q);
                        let h = 1e-6;
                        let d1 = base.apply_branch([p[0] + h, p[1]], 0);
                        let d2 = base.apply_branch([p[0], p[1] + h], 0);
                        let g1 = (d1[0] - q[0]).hypot(d1[1] - q[1]) / h;
                        let g2 = (d2[0] - q[0]).hypot(d2[1] - q[1]) / h;
                        assert!(sg <= g1.min(g2) * (1.0 + 1e-3) + 1e-9, "{kernel:?} at {p:?}: sigma {sg} exceeds stretch {g1}/{g2}");
                    }
                }
                assert_eq!(hits, 1, "{kernel:?}: {p:?} came back on {hits} branches");
            }
        }

        // A disc IFS gets as many rings as its ball reaches, and no
        // more than twelve; a hemisphere's ball is invariant.
        let ifs = analyse_2d(&flame_of(vec![disc.clone(), affine_xform(0.5, 0.0, 0.0, 0.5, 0.5, 0.0)]), r).expect("qualifies");
        let rings = ifs.maps.iter().filter(|m| m.transform_index == 0).count();
        assert!((2..=12).contains(&rings), "{rings} rings");
        let ifs = analyse_2d(&flame_of(vec![hemi.clone(), affine_xform(0.5, 0.0, 0.0, 0.5, 0.5, 0.0)]), r).expect("qualifies");
        for j in 0..90 {
            let a = (j as f64 * 4.0).to_radians();
            let q = [ifs.ball.centre[0] + ifs.ball.radius * a.cos(), ifs.ball.centre[1] + ifs.ball.radius * a.sin()];
            for m in &ifs.maps {
                let img = m.forward.apply(q);
                let d = (img[0] - ifs.ball.centre[0]).hypot(img[1] - ifs.ball.centre[1]);
                assert!(d <= ifs.ball.radius * (1.0 + 1e-9));
            }
        }
        // A blob whose scale reaches zero has no single inverse.
        let mut flat = blob.clone();
        flat.set_variation_param("blob", "low", 0.0);
        let errs = analyse_2d(&flame_of(vec![flat]), r).unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, Disqualification::NotAffine { why: NotAffine::Degenerate(v), .. } if v == "blob")), "{errs:?}");
    }

    /// A 3D transform carrying one julia-family 3D root at `power`,
    /// with the given XY affine and z offset; `linear3D` is dropped.
    fn root3_xform(variation: &str, power: f32, a: [f32; 6], g: f32, w: f32) -> Transform {
        let mut t = affine_xform(a[0], a[1], a[2], a[3], a[4], a[5]);
        t.g = g;
        t.variations.clear();
        t.variation_order.clear();
        let mut t = with(t, variation, w);
        t.set_variation_param(variation, "power", power);
        t
    }

    /// Plan 8.11 step 2, gate 1: julia3D and julia3Dz round-trip on
    /// every branch, and each local factor is the forward map's
    /// smallest stretch to a finite difference in three directions.
    #[test]
    fn the_3d_root_kernels_undo_each_of_their_branches() {
        let guard = global_registry();
        let r = &*guard;
        let cases = [
            (root3_xform("julia3D", 3.0, [1.0, 0.0, 0.0, 1.0, 0.3, -0.2], 0.1, 0.9), Kernel3::Root3 { n: 3 }),
            (root3_xform("julia3D", 2.0, [0.9, 0.2, -0.2, 0.9, 0.0, 0.4], -0.3, 1.1), Kernel3::Root3 { n: 2 }),
            (root3_xform("julia3Dz", 2.0, [1.0, 0.0, 0.0, 1.0, -0.4, 0.1], 0.2, 0.8), Kernel3::RootZ3 { n: 2 }),
            (root3_xform("julia3Dz", 4.0, [0.8, 0.0, 0.0, 0.8, 0.1, 0.1], 0.0, 1.0), Kernel3::RootZ3 { n: 4 }),
        ];
        for (t, kernel) in &cases {
            let m = transform_map_3d_ordered(t, r, &t.ordered_variation_names(r)).expect("a nonlinear map");
            let base = m.nonlinear().copied().expect("nonlinear");
            assert_eq!(base.kernel, *kernel);
            let n = kernel.power().unsigned_abs();
            for p in [[0.3, 0.4, 0.2], [-1.2, 0.7, -0.5], [2.0, -1.5, 1.1], [0.05, -0.02, 0.4]] {
                for k in 0..n {
                    let q = base.apply_branch(p, k);
                    let back = base.apply_inverse(q);
                    for i in 0..3 {
                        assert!((back[i] - p[i]).abs() < 1e-6, "{kernel:?} branch {k}: {p:?} -> {q:?} -> {back:?}");
                    }
                }
                // The local factor is below the forward stretch in
                // every axis direction.
                let q = base.apply_branch(p, 0);
                let (c_lo, _) = base.singular_values();
                let sg = c_lo * base.local_sigma_factor(q, 0.0);
                let h = 1e-6;
                for axis in 0..3 {
                    let mut pp = p;
                    pp[axis] += h;
                    let d = base.apply_branch(pp, 0);
                    let g = ((d[0] - q[0]).powi(2) + (d[1] - q[1]).powi(2) + (d[2] - q[2]).powi(2)).sqrt() / h;
                    assert!(sg <= g * (1.0 + 1e-3) + 1e-9, "{kernel:?} at {p:?} axis {axis}: sigma {sg} exceeds stretch {g}");
                }
            }
        }

        // A pair of 3D roots with a contracting affine qualifies as a
        // solid and gets a ball every branch keeps.
        // A unit affine with linear3D at a half: the sum is 0.5·I in
        // all three axes. (linear3D at one on a half-scale XY affine
        // leaves z at unit scale and is not a contraction.)
        let mut aff = affine_xform(1.0, 0.0, 0.0, 1.0, 0.6, 0.0);
        aff.g = 0.2;
        aff.variations.clear();
        aff.variation_order.clear();
        let aff = with(aff, "linear3D", 0.5);
        let fl = flame_of(vec![cases[0].0.clone(), cases[1].0.clone(), aff]);
        let ifs3 = analyse_3d(&fl, r).expect("qualifies");
        assert_eq!(ifs3.maps.len(), 3);
        assert!(ifs3.ball.radius.is_finite() && ifs3.ball.radius > 0.0);
        // Sampled more finely than the search samples, and on a
        // different lattice: the search's sampling plus its margin
        // must cover what it did not visit.
        let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
        let mut worst = 0.0f64;
        for j in 0..2000 {
            let y = 1.0 - 2.0 * (j as f64 + 0.37) / 2000.0;
            let rr = (1.0 - y * y).sqrt();
            let phi = golden * j as f64 + 0.5;
            let q = [
                ifs3.ball.centre[0] + ifs3.ball.radius * rr * phi.cos(),
                ifs3.ball.centre[1] + ifs3.ball.radius * y,
                ifs3.ball.centre[2] + ifs3.ball.radius * rr * phi.sin(),
            ];
            for m in &ifs3.maps {
                let branches = m.forward.nonlinear().map_or(1, |nl| nl.kernel.power().unsigned_abs());
                for k in 0..branches {
                    let img = match &m.forward {
                        Map3::Nonlinear(nl) => nl.apply_branch(q, k),
                        other => other.apply(q),
                    };
                    let d = ((img[0] - ifs3.ball.centre[0]).powi(2) + (img[1] - ifs3.ball.centre[1]).powi(2) + (img[2] - ifs3.ball.centre[2]).powi(2)).sqrt();
                    worst = worst.max(d / ifs3.ball.radius);
                }
            }
        }
        println!("  the ball's images reach {worst:.4} of its radius on a finer sphere than the search's");
        // Measured 1.0014: the search samples 256 directions and adds
        // 5%, and a finer sphere finds images a seventh of a percent
        // past the reported radius. Invariant to sampling, then, and
        // a set point can sit at most that far outside the ball.
        assert!(worst <= 1.005, "an image reaches {worst:.4} of the ball's radius");
        // And a root summed with an affine is refused, as in the plane.
        let mixed = with(cases[0].0.clone(), "linear3D", 0.5);
        let errs = analyse_3d(&flame_of(vec![mixed]), r).unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, Disqualification::NotAffine { why: NotAffine::MixedSum(v), .. } if v == "julia3D")), "{errs:?}");
    }

    /// An inverse-mode `quaternion_julia` transform at `c`, identity
    /// affine, weight `w`.
    fn qjulia_xform(c: [f32; 4], power: f32, dist: f32, w: f32) -> Transform {
        let mut t = affine_xform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        t.variations.clear();
        t.variation_order.clear();
        let mut t = with(t, "quaternion_julia", w);
        t.set_variation_param("quaternion_julia", "cx", c[0]);
        t.set_variation_param("quaternion_julia", "cy", c[1]);
        t.set_variation_param("quaternion_julia", "cz", c[2]);
        t.set_variation_param("quaternion_julia", "cw", c[3]);
        t.set_variation_param("quaternion_julia", "power", power);
        t.set_variation_param("quaternion_julia", "dist", dist);
        t.set_variation_param("quaternion_julia", "inverse", 1.0);
        t
    }

    /// Plan 8.11 step 3, gate 1: the quaternion kernel round-trips in
    /// four dimensions on every branch, with the scalar carried, and
    /// its local factor is below the forward stretch in every axis of
    /// the 4D point. Forward mode and the other projections are
    /// refused by name.
    #[test]
    fn the_quaternion_kernel_undoes_each_of_its_branches_in_4d() {
        let guard = global_registry();
        let r = &*guard;
        for (c, power, dist, w) in [
            ([-1.0f32, 0.2, 0.0, 0.0], 2.0f32, 1.0f32, 1.0f32),
            ([-0.3, 0.5, 0.4, 0.1], 3.0, 1.0, 0.9),
            ([0.2, -0.4, 0.1, -0.3], 2.0, 1.4, 1.1),
        ] {
            let t = qjulia_xform(c, power, dist, w);
            let m = transform_map_3d_ordered(&t, r, &t.ordered_variation_names(r)).expect("a nonlinear map");
            let base = m.nonlinear().copied().expect("nonlinear");
            let Kernel3::Quaternion { n, d, c: kc } = base.kernel else { panic!("{:?}", base.kernel) };
            assert_eq!(n, power as i32);
            assert!(close(d, dist as f64));
            for i in 0..4 {
                assert!(close(kc[i], c[i] as f64));
            }
            for (p, aux) in [([0.3, 0.4, 0.2], 0.1), ([-1.2, 0.7, -0.5], -0.6), ([2.0, -1.5, 1.1], 0.8), ([0.05, -0.02, 0.4], 0.0)] {
                for k in 0..(power as u32) {
                    let (q, qa) = base.apply_branch_aux(p, aux, k);
                    let (back, ba) = base.apply_inverse_aux(q, qa);
                    for i in 0..3 {
                        assert!((back[i] - p[i]).abs() < 1e-6, "c {c:?} branch {k}: {p:?}/{aux} -> {q:?}/{qa} -> {back:?}/{ba}");
                    }
                    assert!((ba - aux).abs() < 1e-6, "c {c:?} branch {k}: aux {aux} came back {ba}");
                }
                let (q, qa) = base.apply_branch_aux(p, aux, 0);
                let (c_lo, _) = base.singular_values();
                let sg = c_lo * base.local_sigma_factor(q, qa);
                let h = 1e-6;
                for axis in 0..4 {
                    let (mut pp, mut aa) = (p, aux);
                    if axis < 3 { pp[axis] += h } else { aa += h }
                    let (dq, da) = base.apply_branch_aux(pp, aa, 0);
                    let g = ((dq[0] - q[0]).powi(2) + (dq[1] - q[1]).powi(2) + (dq[2] - q[2]).powi(2) + (da - qa).powi(2)).sqrt() / h;
                    assert!(sg <= g * (1.0 + 1e-3) + 1e-9, "c {c:?} at {p:?}/{aux} axis {axis}: sigma {sg} exceeds stretch {g}");
                }
            }
        }
        // Forward mode is refused, and so is a non-vector projection.
        let mut fwd = qjulia_xform([-1.0, 0.2, 0.0, 0.0], 2.0, 1.0, 1.0);
        fwd.set_variation_param("quaternion_julia", "inverse", 0.0);
        let errs = analyse_3d(&flame_of(vec![fwd]), r).unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, Disqualification::NotAffine { why: NotAffine::Mode(v), .. } if v == "quaternion_julia")), "{errs:?}");
        let mut depth = qjulia_xform([-1.0, 0.2, 0.0, 0.0], 2.0, 1.0, 1.0);
        depth.set_variation_param("quaternion_julia", "projection", 1.0);
        let errs = analyse_3d(&flame_of(vec![depth]), r).unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, Disqualification::NotAffine { why: NotAffine::Mode(_), .. })), "{errs:?}");
        // And a single transform qualifies, with a 4D ball.
        let ifs3 = analyse_3d(&flame_of(vec![qjulia_xform([-1.0, 0.2, 0.0, 0.0], 2.0, 1.0, 1.0)]), r).expect("qualifies");
        assert_eq!(ifs3.maps.len(), 1);
        assert!(ifs3.ball.radius > 1.0 && ifs3.ball.radius < 4.0, "{:?}", ifs3.ball);
    }

    /// A single inverse-mode quaternion transform: the walk's
    /// membership (never leaves the ball within L levels) against the
    /// direct 4D iteration q -> q^2 + c from the same slice point,
    /// which is what the set IS for one transform. Prints a
    /// measurement of disagreement on a grid through the ball.
    #[test]
    fn a_single_quaternion_transforms_walk_is_the_direct_iteration() {
        let guard = global_registry();
        let r = &*guard;
        // Which constants have an interior on the pure-vector slice
        // (scalar w = 0)? Printed, so the fixtures below can be chosen
        // from what the set is rather than from a guess: a c in the
        // variation's (i, j, k, scalar) layout.
        for cand in [
            [-1.0f32, 0.2, 0.0, 0.0],
            [0.2, 0.0, 0.0, -1.0],
            [0.0, 0.0, 0.0, -1.0],
            [0.0, 0.0, 0.0, -0.5],
            [0.3, 0.0, 0.0, -0.6],
            [0.3, 0.5, 0.4, 0.1],
            [-0.2, 0.8, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.25],
        ] {
            let cf = [cand[0] as f64, cand[1] as f64, cand[2] as f64, cand[3] as f64];
            let mut inside = 0usize;
            let mut total = 0usize;
            for iz in 0..16 {
                for iy in 0..16 {
                    for ix in 0..16 {
                        let f = |i: usize| 3.0 * (i as f64 + 0.5) / 16.0 - 1.5;
                        let mut q = [f(ix), f(iy), f(iz), 0.0];
                        let mut escaped = false;
                        for _ in 0..40 {
                            if q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3] > 16.0 {
                                escaped = true;
                                break;
                            }
                            let sq = qmul(q, q);
                            q = [sq[0] + cf[0], sq[1] + cf[1], sq[2] + cf[2], sq[3] + cf[3]];
                        }
                        total += 1;
                        inside += (!escaped) as usize;
                    }
                }
            }
            println!("  c {cand:?}: {inside} of {total} grid points bounded on the w = 0 slice");
        }
        // -0.6 + 0.3i as a quaternion, scalar -0.6, i 0.3: 360 of 4096
        // bounded above, an interior to agree on.
        let c = [0.3f32, 0.0, 0.0, -0.6];
        let ifs3 = analyse_3d(&flame_of(vec![qjulia_xform(c, 2.0, 1.0, 1.0)]), r).expect("qualifies");
        let cf = [c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64];
        let radius = ifs3.ball.radius;
        let centre = ifs3.ball.centre;
        let (mut n, mut bad, mut walk_in, mut direct_in) = (0usize, 0usize, 0usize, 0usize);
        const G: usize = 24;
        for iz in 0..G {
            for iy in 0..G {
                for ix in 0..G {
                    let f = |i: usize| 2.0 * (i as f64 + 0.5) / G as f64 - 1.0;
                    let (u, v, w) = (f(ix), f(iy), f(iz));
                    if (u * u + v * v + w * w).sqrt() > 0.8 {
                        continue;
                    }
                    let p = [centre[0] + radius * u, centre[1] + radius * v, centre[2] + radius * w];
                    let e = crate::scene::ifs_estimate::estimate_aux(&ifs3, p, 0.0, 24, 1);
                    // Direct: q0 = (p, 0), iterate q^2 + c, escaped when the
                    // 4D distance to the ball's centre passes its radius.
                    let mut q = [p[0], p[1], p[2], 0.0];
                    let mut escaped = false;
                    for _ in 0..24 {
                        let d4 = ((q[0] - centre[0]).powi(2) + (q[1] - centre[1]).powi(2) + (q[2] - centre[2]).powi(2) + (q[3] - ifs3.aux_centre).powi(2)).sqrt();
                        if d4 > radius {
                            escaped = true;
                            break;
                        }
                        let sq = qmul(q, q);
                        q = [sq[0] + cf[0], sq[1] + cf[1], sq[2] + cf[2], sq[3] + cf[3]];
                    }
                    n += 1;
                    walk_in += (!e.escaped) as usize;
                    direct_in += (!escaped) as usize;
                    if e.escaped == !escaped {
                        // (both escaped, or both not) is agreement
                    }
                    if e.escaped != escaped {
                        bad += 1;
                    }
                }
            }
        }
        println!("  single quaternion: {n} points, walk interior {walk_in}, direct interior {direct_in}, membership disagreements {bad}");
        assert_eq!(bad, 0, "the walk and the direct iteration disagree on {bad} of {n} points");
    }

    /// Sierpiński: three half-scale maps. Every singular value is 0.5,
    /// every map inverts to a doubling, and the fixed points are the
    /// triangle's corners.
    #[test]
    fn sierpinski_is_three_half_scale_maps() {
        let guard = global_registry();
        let r = &*guard;
        let fl = flame_of(vec![
            affine_xform(0.5, 0.0, 0.0, 0.5, 0.0, 0.0),
            affine_xform(0.5, 0.0, 0.0, 0.5, 0.5, 0.0),
            affine_xform(0.5, 0.0, 0.0, 0.5, 0.0, 0.5),
        ]);
        let ifs = analyse_2d(&fl, r).expect("qualifies");
        assert_eq!(ifs.maps.len(), 3);
        for m in &ifs.maps {
            assert!(close(m.sigma_min, 0.5) && close(m.sigma_max, 0.5), "{m:?}");
            // The inverse of a half-scale-and-shift is a double-and-unshift.
            let p = [0.3, 0.7];
            let back = m.inverse.apply(m.forward.apply(p));
            assert!(close(back[0], p[0]) && close(back[1], p[1]));
        }
        // Fixed points: (0,0), (1,0), (0,1).
        let fp: Vec<[f64; 2]> = ifs.maps.iter().map(|m| m.forward.fixed_point().unwrap()).collect();
        assert!(close(fp[0][0], 0.0) && close(fp[0][1], 0.0));
        assert!(close(fp[1][0], 1.0) && close(fp[1][1], 0.0));
        assert!(close(fp[2][0], 0.0) && close(fp[2][1], 1.0));
        // The ball contains the triangle: every corner is inside.
        for p in &fp {
            let d = ((p[0] - ifs.ball.centre[0]).powi(2) + (p[1] - ifs.ball.centre[1]).powi(2)).sqrt();
            assert!(d <= ifs.ball.radius + 1e-9, "corner {p:?} outside ball {:?}", ifs.ball);
        }
        // And every map sends the ball into itself.
        for m in &ifs.maps {
            let sc = m.forward.apply(ifs.ball.centre);
            let shift = ((sc[0] - ifs.ball.centre[0]).powi(2) + (sc[1] - ifs.ball.centre[1]).powi(2)).sqrt();
            assert!(shift + m.sigma_max * ifs.ball.radius <= ifs.ball.radius + 1e-9);
        }
    }

    /// The case the determinant cannot see: a map that stretches x by
    /// 2 and squashes y by 0.25 has |det| = 0.5 -- "contractive" by
    /// area -- and σ_max = 2. It is not contractive, and the old
    /// measure would have said it was.
    #[test]
    fn a_stretch_and_squash_is_caught_by_the_singular_value_not_the_determinant() {
        let guard = global_registry();
        let r = &*guard;
        let t = affine_xform(2.0, 0.0, 0.0, 0.25, 0.0, 0.0);
        let a = transform_affine_2d(&t, r).unwrap();
        assert!(close(a.det().abs(), 0.5), "area factor reads contractive");
        let (lo, hi) = a.singular_values();
        assert!(close(lo, 0.25) && close(hi, 2.0));
        let fl = flame_of(vec![t]);
        let errs = analyse_2d(&fl, r).unwrap_err();
        assert!(
            matches!(errs[0], Disqualification::NotContractive { index: 0, sigma_max } if close(sigma_max, 2.0)),
            "{errs:?}"
        );
        // A rotation composed with it does not change the singular values.
        let rot = Affine2 { m: [[0.6, -0.8], [0.8, 0.6]], t: [0.0, 0.0] };
        let (lo2, hi2) = rot.then_after(&a).singular_values();
        assert!(close(lo2, 0.25) && close(hi2, 2.0));
    }

    /// An Apophysis-style 3D flame -- XY affine plus a z offset, no
    /// plane maps -- has unit z scale. As a SOLID it is not contractive
    /// and must fail with σ_max exactly 1; as a PLANAR IFS (the
    /// preserve_z = false default, where z is zeroed each iteration) it
    /// is a perfectly good half-scale IFS.
    #[test]
    fn an_apophysis_3d_flame_is_planar_not_solid() {
        let guard = global_registry();
        let r = &*guard;
        let mut t = affine_xform(0.5, 0.0, 0.0, 0.5, 0.1, 0.0);
        t.g = 0.3;
        let fl = flame_of(vec![t]);
        // Planar: qualifies.
        let ifs2 = analyse_2d(&fl, r).expect("planar qualifies");
        assert!(close(ifs2.maps[0].sigma_max, 0.5));
        // Solid: the z row is (0, 0, 1), σ_max = 1 exactly.
        let errs = analyse_3d(&fl, r).unwrap_err();
        assert!(
            matches!(errs[0], Disqualification::NotContractive { index: 0, sigma_max } if close(sigma_max, 1.0)),
            "{errs:?}"
        );
        // Give it a zscale below one and it becomes a solid IFS.
        let mut t3 = fl.transforms[0].clone();
        t3.variations.insert("zscale".to_string(), -0.5);
        t3.variation_order.push("zscale".to_string());
        let fl3 = flame_of(vec![t3]);
        let ifs3 = analyse_3d(&fl3, r).expect("with zscale it is contractive in z");
        // z scale = w_linear + w_zscale = 1 - 0.5 = 0.5.
        assert!(close(ifs3.maps[0].forward.as_affine().unwrap().m[2][2], 0.5), "{:?}", ifs3.maps[0].forward);
        assert!(close(ifs3.maps[0].sigma_max, 0.5));
        // ...and the g offset survives on the flat path -- SCALED, because
        // the affine (which carries g) runs before the variation sum
        // (which scales z by 0.5): 0.5 · (z + 0.3) has offset 0.15.
        assert!(close(ifs3.maps[0].forward.as_affine().unwrap().t[2], 0.15), "{:?}", ifs3.maps[0].forward.as_affine().unwrap().t);
    }

    /// The 3D composition follows the shader's full path exactly, and
    /// the test carries a finding: a plane ROTATION does not make an
    /// Apophysis flame contractive. A 90° YZ rotation on an XY
    /// half-scale map sends the unit z scale into y, and σ_max is
    /// exactly 1. The plane map has to scale as well as rotate.
    #[test]
    fn plane_maps_compose_as_the_shader_does() {
        let guard = global_registry();
        let r = &*guard;
        // (yz[0]·y + yz[2]·z, yz[1]·y + yz[3]·z): a pure 90° rotation is
        // ny = -z, nz = y -> [0, 1, -1, 0, 0, 0].
        let mut rot = affine_xform(0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
        rot.yz_coefs = [0.0, 1.0, -1.0, 0.0, 0.0, 0.0];
        rot.variations.insert("zscale".to_string(), -0.5);
        rot.variation_order.push("zscale".to_string());
        let a = transform_affine_3d(&rot, r).unwrap();
        // Affine: (x, y, z) -> (0.5x, -z, 0.5y); the sum then scales z
        // by (1 - 0.5). M = [[0.5,0,0],[0,0,-1],[0,0.25,0]]: the -1 is z
        // feeding y unscaled.
        let (lo, hi) = a.singular_values();
        assert!(close(hi, 1.0) && close(lo, 0.25), "({lo}, {hi})");

        // Scale the rotation by 0.5 and it is contractive, with singular
        // values that can be written down: M = [[0.5,0,0],[0,0,-0.5],[0,0.125,0]].
        let mut scaled = rot.clone();
        scaled.yz_coefs = [0.0, 0.5, -0.5, 0.0, 0.0, 0.0];
        let a = transform_affine_3d(&scaled, r).unwrap();
        let p = a.apply([0.0, 1.0, 0.0]);
        // (0,1,0) -> xy (0,0.5,0) -> yz (0, 0, 0.25) -> z scaled 0.5 -> (0, 0, 0.125)
        assert!(close(p[0], 0.0) && close(p[1], 0.0) && close(p[2], 0.125), "{p:?}");
        let (lo, hi) = a.singular_values();
        assert!(close(hi, 0.5) && close(lo, 0.125), "({lo}, {hi})");
        let q = [0.2, -0.4, 0.9];
        let back = a.inverse().unwrap().apply(a.apply(q));
        for k in 0..3 {
            assert!(close(back[k], q[k]), "{back:?} vs {q:?}");
        }
    }

    /// `affine3D` composes to the map its shader body computes, checked
    /// by evaluating the body's own formula at a point with every
    /// parameter non-trivial. Rotation alone has σ = 1 and is refused;
    /// with the scales below one it is a solid IFS map.
    #[test]
    fn affine3d_matches_its_shader_body_and_can_make_a_solid_map() {
        let guard = global_registry();
        let r = &*guard;
        let mut t = Transform::default();
        t.a = 1.0; t.d = 1.0; // identity XY affine
        t.variations = HashMap::from([("affine3D".to_string(), 0.5)]);
        t.variation_order = vec!["affine3D".to_string()];
        let set = |t: &mut Transform, k: &str, v: f32| { t.variation_params.insert(format!("affine3D.{k}"), v); };
        set(&mut t, "scaleX", 0.8); set(&mut t, "scaleY", 0.6); set(&mut t, "scaleZ", 0.7);
        set(&mut t, "rotateX", 20.0); set(&mut t, "rotateY", -35.0); set(&mut t, "rotateZ", 50.0);
        set(&mut t, "shearXY", 0.1); set(&mut t, "shearXZ", -0.2); set(&mut t, "shearYX", 0.05);
        set(&mut t, "shearYZ", 0.15); set(&mut t, "shearZX", -0.1); set(&mut t, "shearZY", 0.25);
        set(&mut t, "translateX", 0.3); set(&mut t, "translateY", -0.2); set(&mut t, "translateZ", 0.4);

        // The shader body, transcribed, at weight 0.5.
        let w = 0.5f64;
        let (sx, sy, sz) = (0.8f64, 0.6, 0.7);
        let (shxy, shxz, shyx, shyz, shzx, shzy) = (0.1f64, -0.2, 0.05, 0.15, -0.1, 0.25);
        let d2r = std::f64::consts::PI / 180.0;
        let (sinx, cosx) = ((20.0 * d2r).sin(), (20.0 * d2r).cos());
        let (siny, cosy) = ((-35.0 * d2r).sin(), (-35.0 * d2r).cos());
        let (sinz, cosz) = ((50.0 * d2r).sin(), (50.0 * d2r).cos());
        let body = |x: f64, y: f64, z: f64| -> [f64; 3] {
            let mx = shxy * sy * y + shxz * sz * z + sx * x;
            let my = shyx * sx * x + shyz * sz * z + sy * y;
            let mz = shzx * sx * x + shzy * sy * y + sz * z;
            let nx = cosz * (cosy * mx + siny * (sinx * my + cosx * mz)) - sinz * (cosx * my - sinx * mz) + 0.3;
            let ny = sinz * (cosy * mx + siny * (sinx * my + cosx * mz)) + cosz * (cosx * my - sinx * mz) - 0.2;
            let nz = -siny * mx + cosy * (sinx * my + cosx * mz) + 0.4;
            [w * nx, w * ny, w * nz]
        };
        let a = transform_affine_3d(&t, r).unwrap();
        for q in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.3, -0.7, 0.9], [-1.2, 0.4, -0.5]] {
            let got = a.apply(q);
            let want = body(q[0], q[1], q[2]);
            for k in 0..3 {
                assert!((got[k] - want[k]).abs() < 1e-6, "at {q:?}: {got:?} vs {want:?}");
            }
        }
        // At weight 0.5 with scales ≤ 0.8 and small shears it is contractive.
        let (_, hi) = a.singular_values();
        assert!(hi < 1.0, "σ_max = {hi}");
        let fl = flame_of(vec![t.clone()]);
        assert!(analyse_3d(&fl, r).is_ok(), "a scaled affine3D is a solid IFS map");

        // The 2D reading is the xy block: the same body with z = 0,
        // first two outputs.
        let a2 = transform_affine_2d(&t, r).unwrap();
        let got = a2.apply([0.3, -0.7]);
        let want = body(0.3, -0.7, 0.0);
        assert!((got[0] - want[0]).abs() < 1e-6 && (got[1] - want[1]).abs() < 1e-6, "{got:?} vs {want:?}");
    }

    /// The symmetric-3×3 eigenvalue closed form, against a matrix with
    /// known eigenvalues (a rotation of diag(1, 4, 9)).
    #[test]
    fn symmetric_eigenvalues_are_exact() {
        // Q diag(1,4,9) Qᵀ for a rotation about z by 30°.
        let c = 30f64.to_radians().cos();
        let s = 30f64.to_radians().sin();
        let q = [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]];
        let d = [1.0, 4.0, 9.0];
        let mut a = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] = (0..3).map(|k| q[i][k] * d[k] * q[j][k]).sum();
            }
        }
        let mut e = symmetric3_eigenvalues(&a);
        e.sort_by(|x, y| x.partial_cmp(y).unwrap());
        assert!(close(e[0], 1.0) && close(e[1], 4.0) && close(e[2], 9.0), "{e:?}");
        // And a diagonal input takes the early path.
        let e2 = symmetric3_eigenvalues(&[[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 5.0]]);
        assert_eq!(e2, [2.0, 3.0, 5.0]);
    }

    /// Every disqualification is reported, with the transform named,
    /// and a non-affine variation is the first thing said.
    #[test]
    fn every_reason_is_reported_not_just_the_first() {
        let guard = global_registry();
        let r = &*guard;
        let mut bad = affine_xform(0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
        bad.variations.insert("sinusoidal".to_string(), 0.5);
        bad.variation_order.push("sinusoidal".to_string());
        let big = affine_xform(1.5, 0.0, 0.0, 0.5, 0.0, 0.0);
        let mut fl = flame_of(vec![affine_xform(0.5, 0.0, 0.0, 0.5, 0.0, 0.0), bad, big]);
        fl.xaos = Some(vec![vec![1.0; 3]; 3]);
        let errs = analyse_2d(&fl, r).unwrap_err();
        let text: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
        assert!(text.iter().any(|s| s == "transform 1 uses `sinusoidal`, which is not affine"), "{text:?}");
        assert!(text.iter().any(|s| s.starts_with("transform 2 is not contractive")), "{text:?}");
        assert!(text.iter().any(|s| s == "xaos is not supported"), "{text:?}");
        assert_eq!(errs.len(), 3, "{text:?}");
    }

    /// A final transform is applied to the whole attractor and rides
    /// along inverted; a second one is refused.
    #[test]
    fn a_single_affine_final_is_carried_and_two_are_refused() {
        let guard = global_registry();
        let r = &*guard;
        let mut fl = flame_of(vec![affine_xform(0.5, 0.0, 0.0, 0.5, 0.0, 0.0)]);
        fl.final_transforms = vec![affine_xform(0.0, -1.0, 1.0, 0.0, 0.2, 0.0)];
        let ifs = analyse_2d(&fl, r).expect("one affine final is fine");
        let f = ifs.final_map.expect("carried");
        assert!(close(f.sigma_max, 1.0), "a rotation has σ = 1, and a final need not contract");
        fl.final_transforms.push(affine_xform(0.5, 0.0, 0.0, 0.5, 0.0, 0.0));
        let errs = analyse_2d(&fl, r).unwrap_err();
        assert!(matches!(errs[0], Disqualification::MultipleFinals { count: 2 }));
    }
}

/// The census: which of the shipped flames qualify, and why the rest
/// do not.
///
/// Not a pass/fail test. It is the plan's phase-0 gate — *"a script or
/// test that runs the criterion over every shipped flame preset and
/// reports the count and the reasons; that number goes in this
/// document"* — and it prints rather than asserts, because the number
/// is the deliverable and a threshold would only rot. The one thing it
/// does assert is that the analysis ran on every flame without a panic.
#[cfg(test)]
mod census {
    use super::*;
    use crate::variations::global_registry;
    use std::collections::BTreeMap;

    /// Every shipped flame: the embedded preset library, then the
    /// visual-regression configs on disk when present.
    fn shipped() -> Vec<(String, crate::config::FractalConfig)> {
        let mut out: Vec<(String, crate::config::FractalConfig)> = Vec::new();
        for (i, c) in crate::resources::presets::load_presets_with_fallback().into_iter().enumerate() {
            out.push((format!("preset[{i}] {}", c.flame.name), c));
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/visual/configs");
        for sub in ["2d", "3d", "solid", "variations", "tonemap"] {
            let dir = root.join(sub);
            if dir.is_dir() {
                for c in crate::scene::assets::load_configs_from_dir(&dir) {
                    out.push((format!("{sub}/{}", c.flame.name), c));
                }
            }
        }
        out
    }

    #[test]
    fn how_many_shipped_flames_are_affine_ifss() {
        let guard = global_registry();
        let r = &*guard;
        let all = shipped();
        let mut planar_ok = Vec::new();
        let mut solid_ok = Vec::new();
        let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
        let mut first_reason: BTreeMap<String, usize> = BTreeMap::new();
        for (name, c) in &all {
            match analyse_2d(&c.flame, r) {
                Ok(_) => planar_ok.push(name.clone()),
                Err(errs) => {
                    // Count every reason, and separately the FIRST one,
                    // which is the one a user would be told.
                    let mut seen = std::collections::BTreeSet::new();
                    for e in &errs {
                        let key = match e {
                            Disqualification::NotAffine { why: NotAffine::Variation(v), .. } => {
                                format!("non-affine variation `{v}`")
                            }
                            Disqualification::NotAffine { why, .. } => format!("{why:?}"),
                            Disqualification::NotContractive { .. } => "not contractive".to_string(),
                            Disqualification::Singular { .. } => "singular".to_string(),
                            Disqualification::Xaos => "xaos".to_string(),
                            Disqualification::FinalNotAffine { .. } => "non-affine final".to_string(),
                            Disqualification::MultipleFinals { .. } => "multiple finals".to_string(),
                            Disqualification::Empty => "empty".to_string(),
                            Disqualification::NoBall => "no invariant ball".to_string(),
                        };
                        seen.insert(key);
                    }
                    for k in &seen {
                        *reasons.entry(k.clone()).or_default() += 1;
                    }
                    if let Some(k) = seen.iter().next() {
                        *first_reason.entry(k.clone()).or_default() += 1;
                    }
                }
            }
            // A 3D flame with preserve_z on is a solid IFS candidate.
            if c.render_mode == crate::scene::transforms::RenderMode::ThreeD && c.preserve_z {
                if analyse_3d(&c.flame, r).is_ok() {
                    solid_ok.push(name.clone());
                }
            }
        }
        let preserve_z_on = all
            .iter()
            .filter(|(_, c)| c.render_mode == crate::scene::transforms::RenderMode::ThreeD && c.preserve_z)
            .count();
        // The second column, plan §8.4: past affine the practical rule
        // is ONE nonlinear normal-phase variation per transform, with
        // affine variations summed beside it and the pre/post affines
        // composing around it. Which shipped flames have that shape,
        // and which variation each would need supported? Structural
        // only -- it says nothing about whether the variation has a
        // closed-form inverse or whether the map contracts.
        let mut one_nonlinear: BTreeMap<String, usize> = BTreeMap::new();
        let mut needs: BTreeMap<String, usize> = BTreeMap::new();
        // What the plane can already invert: an affine role, or a
        // root (plan 8.8).
        let known = |name: &str, w: f64, t: &Transform| {
            affine_role(name, w, t, r, Space::Planar).is_some()
                || matches!(name, "julia" | "julian" | "spherical" | "bubble" | "hemisphere" | "disc" | "blob")
        };
        for (_, c) in &all {
            if c.flame.xaos.is_some() || c.flame.final_transforms.len() > 1 {
                continue;
            }
            let mut set = std::collections::BTreeSet::new();
            let mut shape_ok = true;
            for t in c.flame.transforms.iter().chain(&c.flame.final_transforms) {
                let mut nonlinear = Vec::new();
                for name in t.ordered_variation_names(r) {
                    let w = t.variations.get(&name).copied().unwrap_or(0.0) as f64;
                    if w == 0.0 {
                        continue;
                    }
                    if !known(&name, w, t) {
                        nonlinear.push(name);
                    }
                }
                if nonlinear.len() > 1 {
                    shape_ok = false;
                    break;
                }
                set.extend(nonlinear);
            }
            if shape_ok && !set.is_empty() {
                for n in &set {
                    *needs.entry(n.clone()).or_default() += 1;
                }
                let key: Vec<&str> = set.iter().map(|s| s.as_str()).collect();
                *one_nonlinear.entry(key.join(" + ")).or_default() += 1;
            }
        }
        let presets = all.iter().filter(|(n, _)| n.starts_with("preset[")).count();
        println!("\n=== affine-IFS census over {} shipped flames ===", all.len());
        println!(
            "  ({presets} from the preset library, the rest visual-regression configs; \
             {preserve_z_on} are 3D with preserve_z on, which is what a SOLID candidate needs)"
        );
        println!("qualify as a PLANAR affine IFS: {}", planar_ok.len());
        for n in &planar_ok {
            println!("    {n}");
        }
        println!("qualify as a SOLID affine IFS (3D + preserve_z): {}", solid_ok.len());
        for n in &solid_ok {
            println!("    {n}");
        }
        println!("why the other {} do not (a flame may count under several):", all.len() - planar_ok.len());
        let mut v: Vec<_> = reasons.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1));
        for (k, n) in v.iter().take(25) {
            println!("    {n:4}  {k}");
        }
        // The preset library alone is the catalogue a user meets; the
        // visual-regression configs are mostly one-variation smoke
        // tests and inflate every count above.
        println!("the preset library alone: {} of {presets} qualify; the others need", planar_ok.iter().filter(|n| n.starts_with("preset[")).count());
        for (name, c) in all.iter().filter(|(n, _)| n.starts_with("preset[")) {
            if planar_ok.contains(name) {
                continue;
            }
            let mut set = std::collections::BTreeSet::new();
            for t in c.flame.transforms.iter().chain(&c.flame.final_transforms) {
                for v in t.ordered_variation_names(r) {
                    let w = t.variations.get(&v).copied().unwrap_or(0.0) as f64;
                    if w != 0.0 && !known(&v, w, t) {
                        set.insert(v);
                    }
                }
            }
            let set: Vec<String> = set.into_iter().collect();
            if set.is_empty() {
                let why: Vec<String> = analyse_2d(&c.flame, r).err().unwrap_or_default().iter().map(|d| d.to_string()).collect();
                println!("    {name}: every variation is known, but: {}", why.join("; "));
            } else {
                println!("    {name}: {}", set.join(", "));
            }
        }
        let shaped: usize = one_nonlinear.values().sum();
        println!(
            "of the rest, {shaped} have ONE nonlinear variation per transform (plan §8.4's shape), \
             and would need these supported:"
        );
        let mut v: Vec<_> = needs.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (k, n) in v.iter().take(20) {
            println!("    {n:4}  {k}");
        }
        println!("  by the set a flame needs:");
        let mut v: Vec<_> = one_nonlinear.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (k, n) in v.iter().take(12) {
            println!("    {n:4}  {k}");
        }
        assert!(!all.is_empty(), "the census found no shipped flames at all");
    }
}
