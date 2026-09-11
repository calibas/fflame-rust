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
//! weighted sum of variations, then the post-affine. When every
//! variation is affine the whole thing is one affine map, and the
//! composition here mirrors the shader exactly — the flat and full
//! paths of `affine_3d.wgsl`, the translation summed from three
//! sources, `g` dropped under active plane maps (a documented
//! limitation the shader has and this reproduces rather than fixes).
//! Which variations count as affine is the allowlist in
//! [`AFFINE_VARIATIONS`], and it is small on purpose.
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

/// The variations whose 3D bodies are affine in the point, so that a
/// transform using only these composes to one affine map. Deliberately
/// short: growing it means proving each addition is affine
/// (conformal invertible maps — Möbius, spherical inversion — are the
/// next candidates and are NOT affine; see the plan's §7).
///
/// - `linear` / `linear3D`: the identity on the point.
/// - `zscale`: contributes `weight·z` to z.
/// - `ztranslate`: contributes `weight` to z.
/// - `affine3D`: JWildfire's general 3D affine — per-axis scale, six
///   shears, a Z·Y·X rotation and a translation, all fifteen
///   parameters. **This is the one that makes a solid affine IFS
///   buildable**: without it every 3D transform is an XY affine with
///   a unit z scale, and the census found none that qualified.
///
/// `flatten` is affine but singular (it projects to z = 0), so it is
/// excluded: a singular map has no inverse and collapses the attractor.
pub const AFFINE_VARIATIONS: &[&str] = &["linear", "linear3D", "zscale", "ztranslate", "affine3D"];

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
    /// A variation outside [`AFFINE_VARIATIONS`].
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

/// The weighted variation sum a transform applies to the affine's
/// output, `Σ wᵢ·vᵢ(q)`, as one affine map `V` — a sum of affine maps
/// is affine. `linear` contributes `w·I`; `zscale` contributes `w`
/// on the z diagonal; `ztranslate` contributes `w` to the z
/// translation; `affine3D` contributes `w` times its whole map.
fn affine_sum(t: &Transform, registry: &VariationRegistry) -> Result<Affine3, NotAffine> {
    let mut sum = Affine3 { m: [[0.0; 3]; 3], t: [0.0; 3] };
    let mut any = false;
    for name in t.ordered_variation_names(registry) {
        let w = t.variations.get(&name).copied().unwrap_or(0.0) as f64;
        if w == 0.0 {
            continue;
        }
        if !AFFINE_VARIATIONS.contains(&name.as_str()) {
            return Err(NotAffine::Variation(name));
        }
        // The same rule `mean_log_scale` mirrors from the shader
        // builder: `Any`-phase variations sum unless a non-zero
        // priority moved them.
        let summed = match registry.get(&name).map(|i| i.phase.clone()) {
            Some(VariationPhase::Normal) => true,
            Some(VariationPhase::Any) => {
                t.variation_priorities.get(&name).copied().unwrap_or(0) == 0
            }
            Some(_) => false,
            None => true,
        };
        if !summed {
            return Err(NotAffine::Priority(name));
        }
        any = true;
        match name.as_str() {
            "linear" | "linear3D" => {
                for k in 0..3 {
                    sum.m[k][k] += w;
                }
            }
            "zscale" => sum.m[2][2] += w,
            "ztranslate" => sum.t[2] += w,
            "affine3D" => {
                let a = affine3d_map(t, registry);
                for i in 0..3 {
                    for j in 0..3 {
                        sum.m[i][j] += w * a.m[i][j];
                    }
                    sum.t[i] += w * a.t[i];
                }
            }
            _ => unreachable!("allowlisted above"),
        }
    }
    if !any {
        return Err(NotAffine::NoVariations);
    }
    Ok(sum)
}

/// The 2D affine a transform composes to, or why it does not.
///
/// The z-only variations contribute nothing in 2D and are accepted;
/// `linear3D` is the identity in 2D as `linear` is.
pub fn transform_affine_2d(t: &Transform, registry: &VariationRegistry) -> Result<Affine2, NotAffine> {
    let sum = affine_sum(t, registry)?;
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

/// The 3D affine a transform composes to, or why it does not.
///
/// This is the map the chaos game applies when `preserve_z` is ON.
/// With it off the trajectory is 2D and this map's z row describes the
/// plotted height, not the dynamics; see [`analyse`].
pub fn transform_affine_3d(t: &Transform, registry: &VariationRegistry) -> Result<Affine3, NotAffine> {
    let sum = affine_sum(t, registry)?;
    let affine = plane_affine(
        [[t.a as f64, t.b as f64], [t.c as f64, t.d as f64]],
        [t.e as f64, t.f as f64],
        t.g as f64,
        plane(t.yz_coefs),
        plane(t.zx_coefs),
    );
    let mut map = sum.then_after(&affine);
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
}

impl std::fmt::Display for Disqualification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "the flame has no transforms"),
            Self::NotAffine { index, why } => match why {
                NotAffine::Variation(v) => write!(f, "transform {index} uses `{v}`, which is not affine"),
                NotAffine::Priority(v) => write!(f, "transform {index} moves `{v}` out of the weighted sum"),
                NotAffine::NoVariations => write!(f, "transform {index} has no variations"),
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
}

pub type Ifs2 = Ifs<Affine2, [f64; 2]>;
pub type Ifs3 = Ifs<Affine3, [f64; 3]>;

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
    let maps = collect(flame, |t| transform_affine_2d(t, registry).map(|a| (a, a.inverse(), a.singular_values())));
    let final_map = collect_final(flame, |t| transform_affine_2d(t, registry).map(|a| (a, a.inverse(), a.singular_values())));
    let (maps, final_map, errs) = merge(maps, final_map, flame);
    if !errs.is_empty() {
        return Err(errs);
    }
    let ball = ball_2d(&maps);
    Ok(Ifs { maps, final_map, ball })
}

/// The 3D criterion, for a flame run with `preserve_z` on. A flame
/// run with it off is a planar IFS and should be analysed as one.
pub fn analyse_3d(flame: &Flame, registry: &VariationRegistry) -> Result<Ifs3, Vec<Disqualification>> {
    let maps = collect(flame, |t| transform_affine_3d(t, registry).map(|a| (a, a.inverse(), a.singular_values())));
    let final_map = collect_final(flame, |t| transform_affine_3d(t, registry).map(|a| (a, a.inverse(), a.singular_values())));
    let (maps, final_map, errs) = merge(maps, final_map, flame);
    if !errs.is_empty() {
        return Err(errs);
    }
    let ball = ball_3d(&maps);
    Ok(Ifs { maps, final_map, ball })
}

type Raw<A> = Result<(A, Option<A>, (f64, f64)), NotAffine>;

fn collect<A: Copy>(flame: &Flame, f: impl Fn(&Transform) -> Raw<A>) -> Vec<(usize, Raw<A>)> {
    flame.transforms.iter().enumerate().map(|(i, t)| (i, f(t))).collect()
}

fn collect_final<A: Copy>(flame: &Flame, f: impl Fn(&Transform) -> Raw<A>) -> Vec<Raw<A>> {
    flame.final_transforms.iter().map(f).collect()
}

fn merge<A: Copy>(
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
                if !(sigma_max < 1.0) {
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
/// Hart's radius is a bound on a ball each map sends into ITSELF, which
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

/// A ball every map sends into itself (Hart's construction), then
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
fn ball_2d(maps: &[IfsMap<Affine2>]) -> Ball<[f64; 2]> {
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

fn ball_3d(maps: &[IfsMap<Affine3>]) -> Ball<[f64; 3]> {
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
        assert!(close(ifs3.maps[0].forward.m[2][2], 0.5), "{:?}", ifs3.maps[0].forward);
        assert!(close(ifs3.maps[0].sigma_max, 0.5));
        // ...and the g offset survives on the flat path -- SCALED, because
        // the affine (which carries g) runs before the variation sum
        // (which scales z by 0.5): 0.5 · (z + 0.3) has offset 0.15.
        assert!(close(ifs3.maps[0].forward.t[2], 0.15), "{:?}", ifs3.maps[0].forward.t);
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
        bad.variations.insert("spherical".to_string(), 0.5);
        bad.variation_order.push("spherical".to_string());
        let big = affine_xform(1.5, 0.0, 0.0, 0.5, 0.0, 0.0);
        let mut fl = flame_of(vec![affine_xform(0.5, 0.0, 0.0, 0.5, 0.0, 0.0), bad, big]);
        fl.xaos = Some(vec![vec![1.0; 3]; 3]);
        let errs = analyse_2d(&fl, r).unwrap_err();
        let text: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
        assert!(text.iter().any(|s| s == "transform 1 uses `spherical`, which is not affine"), "{text:?}");
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
        assert!(!all.is_empty(), "the census found no shipped flames at all");
    }
}
