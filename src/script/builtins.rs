//! Heavy math for scripts, implemented in Rust.
//!
//! Rhai is an interpreter: scripts should orchestrate, not crunch. The
//! recipes here — group generators today, L-systems later — run at native
//! speed and hand back plain arrays a script can map onto transforms.
//!
//! # Decomposition
//!
//! Several variations pack an entire group into one variation: they hold
//! N Möbius matrices and pick one per iteration. `schottky_group` is the
//! clearest case. Decomposing means emitting those matrices as N ordinary
//! transforms carrying the `mobius` variation, which turns the group's
//! structure into *flame* structure — each generator gets its own colour,
//! weight, animation tracks, and triangle-editor handle, and the word
//! rules become xaos.

/// Complex arithmetic mirroring `shaders/core/complex.wgsl`, so a
/// decomposition reproduces what the packed variation computes on the
/// GPU. Done in f64 here (the GPU has only f32); results are stored back
/// as f32, so the extra precision only removes error.
type C = [f64; 2];

fn cmul(a: C, b: C) -> C {
    [a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0]]
}

fn csub(a: C, b: C) -> C {
    [a[0] - b[0], a[1] - b[1]]
}

fn cadd(a: C, b: C) -> C {
    [a[0] + b[0], a[1] + b[1]]
}

fn cdiv(a: C, b: C) -> C {
    let denom = b[0] * b[0] + b[1] * b[1];
    let safe = if denom < 1e-30 { 1e-30 } else { denom };
    let conj = [b[0], -b[1]];
    let n = cmul(a, conj);
    [n[0] / safe, n[1] / safe]
}

/// Principal branch, matching `csqrt` in complex.wgsl: non-negative real
/// part, imaginary part taking the sign of the input's.
fn csqrt(a: C) -> C {
    let mag = (a[0] * a[0] + a[1] * a[1]).sqrt();
    let real = (0.5 * (mag + a[0])).max(0.0).sqrt();
    let imag_mag = (0.5 * (mag - a[0])).max(0.0).sqrt();
    [real, if a[1] >= 0.0 { imag_mag } else { -imag_mag }]
}

/// A Möbius transformation `z -> (az + b)/(cz + d)`, normalised to
/// determinant 1 — the same layout the `mobius` variation reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mobius {
    pub a: C,
    pub b: C,
    pub c: C,
    pub d: C,
}

impl Mobius {
    /// `[re_a, im_a, re_b, im_b, re_c, im_c, re_d, im_d]` — the order the
    /// `mobius` variation's parameters are declared in.
    pub fn to_params(self) -> [f64; 8] {
        [
            self.a[0], self.a[1], self.b[0], self.b[1], self.c[0], self.c[1], self.d[0], self.d[1],
        ]
    }

    /// Inverse of a determinant-1 matrix: `[d, -b; -c, a]`.
    pub fn inverse(self) -> Self {
        Self {
            a: self.d,
            b: [-self.b[0], -self.b[1]],
            c: [-self.c[0], -self.c[1]],
            d: self.a,
        }
    }

    pub fn det(self) -> C {
        csub(cmul(self.a, self.d), cmul(self.b, self.c))
    }

    /// Apply to a point, as `su_apply_plain` does.
    pub fn apply(self, z: C) -> C {
        cdiv(cadd(cmul(self.a, z), self.b), cadd(cmul(self.c, z), self.d))
    }

    /// Compose: `self ∘ other` (other applied first).
    pub fn compose(self, other: Self) -> Self {
        Self {
            a: cadd(cmul(self.a, other.a), cmul(self.b, other.c)),
            b: cadd(cmul(self.a, other.b), cmul(self.b, other.d)),
            c: cadd(cmul(self.c, other.a), cmul(self.d, other.c)),
            d: cadd(cmul(self.c, other.b), cmul(self.d, other.d)),
        }
    }
}

/// A circle in the plane: centre and radius.
#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub x: f64,
    pub y: f64,
    pub r: f64,
}

/// The Möbius map pairing two circles, with a marking twist.
///
/// Sends the *exterior* of `c1` onto the *interior* of `c2`:
/// `z -> c2 + r1·r2/(z - c1)`, i.e. `[[c2, r1r2 - c1c2], [1, -c1]]`.
/// A point far from `c1` lands near the centre of `c2`; a point on `c1`
/// lands on `c2`.
///
/// `twist_deg` rotates about `c1` before pairing (the group's *marking*),
/// which changes how the tiles are glued without moving the circles.
///
/// Ported from `init_schottky_group` in
/// `src/variations/defs/schottky_group.rs` — keep the two in step, since
/// a decomposition that disagrees with the packed variation renders
/// differently.
pub fn pair_circles(c1: Circle, c2: Circle, twist_deg: f64) -> Mobius {
    // The packed variation floors radii at 0.05; match it, or a
    // decomposition of a degenerate flame would diverge from its render.
    let r1 = c1.r.max(0.05);
    let r2 = c2.r.max(0.05);
    let c1v: C = [c1.x, c1.y];
    let c2v: C = [c2.x, c2.y];

    let ma = c2v;
    let mb = csub([r1 * r2, 0.0], cmul(c1v, c2v));
    let mc: C = [1.0, 0.0];
    let md: C = [-c1v[0], -c1v[1]];

    // Marking rotation about c1: R = [[e^{iφ}, c1(1 - e^{iφ})], [0, 1]]
    let phi = twist_deg * std::f64::consts::PI / 180.0;
    let e: C = [phi.cos(), phi.sin()];
    let rb = cmul(c1v, csub([1.0, 0.0], e));

    let ta = cmul(ma, e);
    let tb = cadd(cmul(ma, rb), mb);
    let tc = cmul(mc, e);
    let td = cadd(cmul(mc, rb), md);

    // Normalise to determinant 1 so the inverse is just [d, -b; -c, a].
    let sd = csqrt(csub(cmul(ta, td), cmul(tb, tc)));
    Mobius {
        a: cdiv(ta, sd),
        b: cdiv(tb, sd),
        c: cdiv(tc, sd),
        d: cdiv(td, sd),
    }
}

/// The four generators of a Schottky group, in the packed variation's own
/// order: `[a, b, a⁻¹, b⁻¹]`.
///
/// That order matters for more than tidiness — the packed variation's
/// "avoid" rule is `k != (prev + 2) % 4`, which in this ordering means
/// "never immediately undo the last generator". A decomposition
/// reproduces it with xaos; see [`avoid_xaos_row`].
pub fn schottky_generators(circles: [Circle; 4], twist_a: f64, twist_b: f64) -> [Mobius; 4] {
    let a = pair_circles(circles[0], circles[1], twist_a);
    let b = pair_circles(circles[2], circles[3], twist_b);
    [a, b, a.inverse(), b.inverse()]
}

/// General inverse, not assuming determinant 1 (`su_matinv`).
fn inv_general(m: Mobius) -> Mobius {
    let det = m.det();
    Mobius {
        a: cdiv(m.d, det),
        b: cdiv([-m.b[0], -m.b[1]], det),
        c: cdiv([-m.c[0], -m.c[1]], det),
        d: cdiv(m.a, det),
    }
}

/// The four generators of the classical Apollonian gasket group, in the
/// packed variation's `[a, b, a⁻¹, b⁻¹]` order.
///
/// The base pair is the standard one from *Indra's Pearls*:
/// `a = [[1, 0], [-2i, 1]]`, `b = [[1-i, 1], [1, 1+i]]`. Both have
/// determinant 1, and the stored inverses are exactly their inverses.
///
/// `deform` applies Bagula's triquasiconformal conjugation `g -> C g C⁻¹`
/// with `C = dk(δ)·s0·qf(θ + iη)`, matching `su_conjugator` — conjugation
/// preserves the group, so the inverse pairing survives it.
///
/// Ported from `variation_apollonian_gasket`; keep the two in step.
pub fn apollonian_generators(
    deform: bool,
    theta_deg: f64,
    eta_deg: f64,
    delta: f64,
) -> [Mobius; 4] {
    let base = [
        Mobius { a: [1.0, 0.0], b: [0.0, 0.0], c: [0.0, -2.0], d: [1.0, 0.0] },
        Mobius { a: [1.0, -1.0], b: [1.0, 0.0], c: [1.0, 0.0], d: [1.0, 1.0] },
        Mobius { a: [1.0, 0.0], b: [0.0, 0.0], c: [0.0, 2.0], d: [1.0, 0.0] },
        Mobius { a: [1.0, 1.0], b: [-1.0, 0.0], c: [-1.0, 0.0], d: [1.0, -1.0] },
    ];
    if !deform {
        return base;
    }
    let cj = qc_conjugator(theta_deg, eta_deg, delta);
    let cji = inv_general(cj);
    [
        cj.compose(base[0]).compose(cji),
        cj.compose(base[1]).compose(cji),
        cj.compose(base[2]).compose(cji),
        cj.compose(base[3]).compose(cji),
    ]
}

/// `C = dk(δ)·s0·qf(θ + iη)` — the quasiconformal conjugator, mirroring
/// `su_conjugator` in su_mobius.wgsl (which takes radians; this takes the
/// degrees the parameters are stored in).
pub fn qc_conjugator(theta_deg: f64, eta_deg: f64, delta: f64) -> Mobius {
    const S0: f64 = 0.7071068;
    let theta = theta_deg * std::f64::consts::PI / 180.0;
    let eta = eta_deg * std::f64::consts::PI / 180.0;
    let (ch, sh) = (eta.cosh(), eta.sinh());
    let (ct, st) = (theta.cos(), theta.sin());
    let ca: C = [ct * ch, -st * sh];
    let sa: C = [st * ch, ct * sh];
    let qf = Mobius { a: ca, b: [-sa[0], -sa[1]], c: sa, d: ca };
    let dk = Mobius {
        a: [1.0, delta],
        b: [1.0, 0.0],
        c: [1.0, 0.0],
        d: [1.0, -delta],
    };
    let s0 = Mobius { a: [S0, 0.0], b: [0.0, -S0], c: [0.0, -S0], d: [S0, 0.0] };
    dk.compose(s0).compose(qf)
}

// ============================================================================
// Sphere / circle packings
// ============================================================================

/// A mirror of a packing: a sphere (or circle, with `z` = 0) to invert in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub r: f64,
}

impl Sphere {
    /// Invert a point in this sphere: `x -> c + r^2 (x - c)/|x - c|^2`.
    /// An inversion is its own inverse, which is why a packing forbids
    /// REPEATING a mirror rather than forbidding some other index.
    pub fn invert(self, p: [f64; 3]) -> [f64; 3] {
        let v = [p[0] - self.x, p[1] - self.y, p[2] - self.z];
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).max(1e-12);
        let k = self.r * self.r / n;
        [self.x + k * v[0], self.y + k * v[1], self.z + k * v[2]]
    }
}

/// `sp_hash01` from the shader - the classic GLSL sine hash.
///
/// Computed in f32 deliberately: the shader's is f32, and `fract` after a
/// multiply by 43758 amplifies any difference in `sin` into a completely
/// different value. Even so, GPU and CPU `sin` need not agree to the last
/// bit, so a packing with **Size Jitter above 0** may decompose to
/// slightly different radii. At the default jitter of 0 the hash is
/// unused and the decomposition is exact.
fn sp_hash01(k: u32) -> f64 {
    let v: f32 = ((k as f32) * 127.1 + 311.7).sin() * 43758.5453;
    (v - v.floor()) as f64
}

const TAU: f64 = std::f64::consts::TAU;

/// Configuration circle `k` (2D) - `sp_conf2`.
fn sp_conf2(mode: u32, k: u32, n: u32, rs: f64, jit: f64) -> Sphere {
    if mode >= 2 {
        if k == 0 {
            return Sphere { x: 0.0, y: 0.0, z: 0.0, r: 1.0 };
        }
        let i = k - 1;
        let s = (std::f64::consts::PI / n as f64).sin();
        let rt = s / (1.0 + s);
        let r = rt * rs * (1.0 - jit * sp_hash01(i));
        let d = 1.0 - r;
        let a = TAU * i as f64 / n as f64;
        return Sphere { x: d * a.cos(), y: d * a.sin(), z: 0.0, r };
    }
    // Soddy tangent circles.
    match k {
        0 => Sphere { x: 0.0, y: 0.0, z: 0.0, r: 1.0 },
        1 => Sphere { x: 0.0, y: 0.5358984, z: 0.0, r: 0.4641016 },
        2 => Sphere { x: -0.4641016, y: -0.2679492, z: 0.0, r: 0.4641016 },
        _ => Sphere { x: 0.4641016, y: -0.2679492, z: 0.0, r: 0.4641016 },
    }
}

/// Mirror circle `k` (2D) - `sp_mirror2`: Soddy duals for mode 0.
fn sp_mirror2(mode: u32, k: u32, n: u32, rs: f64, jit: f64) -> Sphere {
    if mode != 0 {
        return sp_conf2(mode, k, n, rs, jit);
    }
    match k {
        0 => Sphere { x: 0.0, y: 0.0, z: 0.0, r: 0.2679492 },
        1 => Sphere { x: 0.0, y: -2.0, z: 0.0, r: 1.7320508 },
        2 => Sphere { x: 1.7320508, y: 1.0, z: 0.0, r: 1.7320508 },
        _ => Sphere { x: -1.7320508, y: 1.0, z: 0.0, r: 1.7320508 },
    }
}

/// Configuration sphere `k` (3D) - `sp_conf3`.
#[allow(clippy::too_many_arguments)]
fn sp_conf3(mode: u32, k: u32, n: u32, rs: f64, cs: f64, jit: f64, tilt: f64) -> Sphere {
    if mode >= 2 {
        if k == 0 {
            return Sphere { x: 0.0, y: 0.0, z: 0.0, r: 1.0 };
        }
        let (ct, st) = (tilt.cos(), tilt.sin());
        let cth = ct * ct * (TAU / n as f64).cos() - st * st;
        let sh2 = (0.5 * (1.0 - cth)).max(1e-6).sqrt();
        let mut rt = sh2 / (1.0 + sh2);
        if n >= 3 {
            // Same-parity neighbours crowd toward the pole as tilt grows;
            // cap the radius at their tangency too, or the mirrors overlap
            // and the reflection group stops being discrete.
            let cth2 = ct * ct * (2.0 * TAU / n as f64).cos() + st * st;
            let shp = (0.5 * (1.0 - cth2)).max(1e-6).sqrt();
            rt = rt.min(shp / (1.0 + shp));
        }
        if mode == 3 && k > n {
            // Polar caps, sized to kiss the outer sphere and the ring.
            let rn = rt * rs;
            let dn = 1.0 - rn;
            let rho = (dn * dn + 1.0 - rn * rn - 2.0 * dn * st)
                / (2.0 * (1.0 + rn - dn * st).max(1e-4));
            let cr = (rho * cs).max(1e-4);
            let h = 1.0 - cr;
            let sgn = if k == n + 2 { -1.0 } else { 1.0 };
            return Sphere { x: 0.0, y: 0.0, z: sgn * h, r: cr };
        }
        let i = k - 1;
        let r = rt * rs * (1.0 - jit * sp_hash01(i));
        let d = 1.0 - r;
        let a = TAU * i as f64 / n as f64;
        let ph = if i % 2 == 1 { -tilt } else { tilt };
        return Sphere {
            x: d * a.cos() * ph.cos(),
            y: d * a.sin() * ph.cos(),
            z: d * ph.sin(),
            r,
        };
    }
    // Soddy tangent spheres: outer + tetrahedral inner.
    match k {
        0 => Sphere { x: 0.0, y: 0.0, z: 0.0, r: 1.0 },
        1 => Sphere { x: 0.3178372, y: 0.3178372, z: 0.3178372, r: 0.4494897 },
        2 => Sphere { x: 0.3178372, y: -0.3178372, z: -0.3178372, r: 0.4494897 },
        3 => Sphere { x: -0.3178372, y: 0.3178372, z: -0.3178372, r: 0.4494897 },
        _ => Sphere { x: -0.3178372, y: -0.3178372, z: 0.3178372, r: 0.4494897 },
    }
}

/// Mirror sphere `k` (3D) - `sp_mirror3`: Soddy duals for mode 0.
#[allow(clippy::too_many_arguments)]
fn sp_mirror3(mode: u32, k: u32, n: u32, rs: f64, cs: f64, jit: f64, tilt: f64) -> Sphere {
    if mode != 0 {
        return sp_conf3(mode, k, n, rs, cs, jit, tilt);
    }
    match k {
        0 => Sphere { x: 0.0, y: 0.0, z: 0.0, r: 0.3178372 },
        1 => Sphere { x: -1.7320508, y: -1.7320508, z: -1.7320508, r: 2.8284271 },
        2 => Sphere { x: -1.7320508, y: 1.7320508, z: 1.7320508, r: 2.8284271 },
        3 => Sphere { x: 1.7320508, y: -1.7320508, z: 1.7320508, r: 2.8284271 },
        _ => Sphere { x: 1.7320508, y: 1.7320508, z: -1.7320508, r: 2.8284271 },
    }
}

/// Every mirror of a `sphere_packing`, in WORLD coordinates.
///
/// The variation works in `p / size` and scales back on the way out, so a
/// mirror `(c, r)` there is `(size*c, size*r)` here - the form a
/// decomposed transform needs.
///
/// Modes: 0 Apollonian (dual spheres), 1 Tangent Spheres, 2 Ring,
/// 3 Ring + Caps (3D only; renders as Ring in 2D).
#[allow(clippy::too_many_arguments)]
pub fn sphere_packing_mirrors(
    mode: u32,
    size: f64,
    ring_n: u32,
    ring_scale: f64,
    cap_scale: f64,
    jitter: f64,
    tilt_deg: f64,
    three_d: bool,
) -> Vec<Sphere> {
    let n = ring_n.clamp(2, 16);
    let tilt = tilt_deg * std::f64::consts::PI / 180.0;
    let count = if three_d {
        match mode {
            2 => 1 + n,
            3 => 3 + n,
            _ => 5,
        }
    } else if mode >= 2 {
        1 + n
    } else {
        4
    };

    (0..count)
        .map(|k| {
            let m = if three_d {
                sp_mirror3(mode, k, n, ring_scale, cap_scale, jitter, tilt)
            } else {
                sp_mirror2(mode, k, n, ring_scale, jitter)
            };
            Sphere { x: m.x * size, y: m.y * size, z: m.z * size, r: m.r * size }
        })
        .collect()
}

/// Xaos row for "don't pick the same one twice in a row".
///
/// Like [`avoid_xaos_row`] but for a self-inverse generator: an inversion
/// undoes itself, so the blocked index is the transform's own. The packed
/// variation redraws into the NEXT mirror, so that one is twice as likely.
pub fn repeat_xaos_row(from: usize, count: usize) -> Vec<f32> {
    let mut row = vec![1.0f32; count];
    if count > 1 {
        row[from] = 0.0;
        row[(from + 1) % count] += 1.0;
    }
    row
}

/// One row of the xaos matrix reproducing the packed "avoid" rule.
///
/// The packed variation does not merely *forbid* the inverse — it draws
/// uniformly from four generators and, on hitting the forbidden one,
/// uses the next instead. So the forbidden generator gets probability 0
/// and its successor gets double. Relative weights `[1,1,1,1]` with those
/// two entries adjusted reproduce that distribution exactly.
pub fn avoid_xaos_row(from: usize) -> [f32; 4] {
    let mut row = [1.0f32; 4];
    let forbidden = (from + 2) % 4;
    row[forbidden] = 0.0;
    row[(forbidden + 1) % 4] = 2.0;
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64, what: &str) {
        assert!((a - b).abs() < tol, "{what}: {a} vs {b}");
    }

    /// The defining property: the pairing map must send the exterior of
    /// one circle onto the interior of the other.
    #[test]
    fn pairing_maps_circle_onto_circle() {
        let c1 = Circle { x: -1.2, y: 0.0, r: 0.8 };
        let c2 = Circle { x: 1.2, y: 0.0, r: 0.8 };
        let m = pair_circles(c1, c2, 0.0);

        // Points ON c1 land ON c2.
        for k in 0..8 {
            let theta = k as f64 * std::f64::consts::TAU / 8.0;
            let z = [c1.x + c1.r * theta.cos(), c1.y + c1.r * theta.sin()];
            let w = m.apply(z);
            let dist = ((w[0] - c2.x).powi(2) + (w[1] - c2.y).powi(2)).sqrt();
            approx(dist, c2.r, 1e-9, "image of a point on C1 lies on C2");
        }

        // A point far outside c1 lands near the CENTRE of c2 (interior).
        let far = m.apply([500.0, 500.0]);
        let d = ((far[0] - c2.x).powi(2) + (far[1] - c2.y).powi(2)).sqrt();
        assert!(d < 0.02, "far exterior point should land near C2's centre, got {d}");
    }

    #[test]
    fn generators_are_normalised_and_invertible() {
        let circles = [
            Circle { x: -1.2, y: 0.0, r: 0.8 },
            Circle { x: 1.2, y: 0.0, r: 0.8 },
            Circle { x: 0.0, y: -1.2, r: 0.8 },
            Circle { x: 0.0, y: 1.2, r: 0.8 },
        ];
        let gens = schottky_generators(circles, 17.0, -33.0);

        for (i, g) in gens.iter().enumerate() {
            let det = g.det();
            approx(det[0], 1.0, 1e-9, &format!("gen {i} det real"));
            approx(det[1], 0.0, 1e-9, &format!("gen {i} det imag"));
        }

        // a ∘ a⁻¹ is the identity, and the stored inverses really are the
        // inverses of the stored generators.
        for (g, inv) in [(gens[0], gens[2]), (gens[1], gens[3])] {
            let id = g.compose(inv);
            approx(id.a[0], id.d[0], 1e-9, "identity has a == d");
            approx(id.b[0], 0.0, 1e-9, "identity has b == 0");
            approx(id.b[1], 0.0, 1e-9, "identity has b == 0");
            approx(id.c[0], 0.0, 1e-9, "identity has c == 0");
            approx(id.c[1], 0.0, 1e-9, "identity has c == 0");
            // And it moves points nowhere.
            let z = [0.31, -0.47];
            let w = id.apply(z);
            approx(w[0], z[0], 1e-9, "identity fixes x");
            approx(w[1], z[1], 1e-9, "identity fixes y");
        }
    }

    #[test]
    fn twist_rotates_without_moving_the_circles() {
        // The marking changes the gluing, not the circle pairing: images
        // of C1 must still land on C2 whatever the twist.
        let c1 = Circle { x: -1.0, y: 0.3, r: 0.6 };
        let c2 = Circle { x: 1.1, y: -0.2, r: 0.9 };
        for twist in [0.0, 30.0, 90.0, -145.0, 360.0] {
            let m = pair_circles(c1, c2, twist);
            let z = [c1.x + c1.r, c1.y];
            let w = m.apply(z);
            let dist = ((w[0] - c2.x).powi(2) + (w[1] - c2.y).powi(2)).sqrt();
            approx(dist, c2.r, 1e-9, &format!("twist {twist} keeps C1 -> C2"));
        }

        // A nonzero twist is a genuinely different map.
        let plain = pair_circles(c1, c2, 0.0);
        let twisted = pair_circles(c1, c2, 45.0);
        let z = [0.2, 0.1];
        let (p, t) = (plain.apply(z), twisted.apply(z));
        assert!(
            (p[0] - t[0]).abs() + (p[1] - t[1]).abs() > 1e-3,
            "twist should change where points go"
        );
    }

    #[test]
    fn apollonian_generators_form_a_group() {
        let g = apollonian_generators(false, 0.0, 0.0, 1.0);
        for (i, m) in g.iter().enumerate() {
            let det = m.det();
            approx(det[0], 1.0, 1e-12, &format!("gen {i} det real"));
            approx(det[1], 0.0, 1e-12, &format!("gen {i} det imag"));
        }
        // Slots 2 and 3 really are the inverses of 0 and 1.
        for (m, inv) in [(g[0], g[2]), (g[1], g[3])] {
            let z = [0.37, -0.21];
            let w = inv.apply(m.apply(z));
            approx(w[0], z[0], 1e-9, "inverse undoes the generator (x)");
            approx(w[1], z[1], 1e-9, "inverse undoes the generator (y)");
        }
    }

    #[test]
    fn quasiconformal_deform_preserves_the_group() {
        // Conjugation g -> C g C⁻¹ is a group isomorphism: determinants
        // and the inverse pairing must survive it.
        let g = apollonian_generators(true, 45.0, 12.0, 1.3);
        for (i, m) in g.iter().enumerate() {
            let det = m.det();
            approx(det[0], 1.0, 1e-6, &format!("deformed gen {i} det real"));
            approx(det[1], 0.0, 1e-6, &format!("deformed gen {i} det imag"));
        }
        for (m, inv) in [(g[0], g[2]), (g[1], g[3])] {
            let z = [0.19, 0.43];
            let w = inv.apply(m.apply(z));
            approx(w[0], z[0], 1e-6, "deformed inverse still undoes it (x)");
            approx(w[1], z[1], 1e-6, "deformed inverse still undoes it (y)");
        }
        // And it is a genuinely different group from the undeformed one.
        let plain = apollonian_generators(false, 45.0, 12.0, 1.3);
        let z = [0.2, 0.1];
        let (a, b) = (plain[0].apply(z), g[0].apply(z));
        assert!(
            (a[0] - b[0]).abs() + (a[1] - b[1]).abs() > 1e-3,
            "deform should move points"
        );
    }


    #[test]
    fn inversion_fixes_its_own_sphere() {
        // Defining property: points ON the mirror stay put, and inverting
        // twice returns you where you started.
        let s = Sphere { x: 0.3, y: -0.2, z: 0.5, r: 0.8 };
        let third = 1.0 / 3.0f64.sqrt();
        for (dx, dy, dz) in [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), (third, third, third)] {
            // The direction must be exactly unit length, or the point is
            // not actually on the sphere and inversion legitimately moves it.
            let p = [s.x + s.r * dx, s.y + s.r * dy, s.z + s.r * dz];
            let q = s.invert(p);
            for i in 0..3 {
                approx(q[i], p[i], 1e-9, "a point on the mirror is fixed");
            }
        }
        let p = [1.7, -0.4, 2.2];
        let back = s.invert(s.invert(p));
        for i in 0..3 {
            approx(back[i], p[i], 1e-9, "inversion is an involution");
        }
        // Inside maps outside.
        let inside = s.invert([s.x + 0.1, s.y, s.z]);
        let d = ((inside[0] - s.x).powi(2) + (inside[1] - s.y).powi(2) + (inside[2] - s.z).powi(2)).sqrt();
        assert!(d > s.r, "a point inside must land outside, got {d}");
    }

    #[test]
    fn packing_mirrors_match_the_configuration() {
        // Soddy 2D: outer circle plus three inner, and size scales all.
        let m = sphere_packing_mirrors(1, 1.0, 6, 1.0, 1.0, 0.0, 0.0, false);
        assert_eq!(m.len(), 4, "2D Soddy has 4 mirrors");
        approx(m[0].r, 1.0, 1e-9, "outer circle is the unit circle");
        for inner in &m[1..] {
            approx(inner.r, 0.4641016, 1e-6, "inner Soddy radius");
            // Inner circles kiss the outer one: |c| + r == 1.
            let d = (inner.x * inner.x + inner.y * inner.y).sqrt();
            approx(d + inner.r, 1.0, 1e-6, "inner circle kisses the outer");
        }

        // Size is a straight scale factor.
        let big = sphere_packing_mirrors(1, 2.5, 6, 1.0, 1.0, 0.0, 0.0, false);
        for (a, b) in m.iter().zip(big.iter()) {
            approx(b.r, a.r * 2.5, 1e-9, "size scales radii");
            approx(b.x, a.x * 2.5, 1e-9, "size scales centres");
        }

        // Ring mode: outer plus N, and neighbours are tangent at scale 1.
        for n in [3u32, 5, 8] {
            let ring = sphere_packing_mirrors(2, 1.0, n, 1.0, 1.0, 0.0, 0.0, false);
            assert_eq!(ring.len() as u32, 1 + n, "ring has 1 + N mirrors");
            let a = ring[1];
            let b = ring[2];
            let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
            approx(d, a.r + b.r, 1e-6, "adjacent ring circles are tangent");
        }

        // 3D Soddy: outer plus a tetrahedron of four.
        let s3 = sphere_packing_mirrors(1, 1.0, 6, 1.0, 1.0, 0.0, 0.0, true);
        assert_eq!(s3.len(), 5, "3D Soddy has 5 mirrors");
        for inner in &s3[1..] {
            let d = (inner.x * inner.x + inner.y * inner.y + inner.z * inner.z).sqrt();
            approx(d + inner.r, 1.0, 1e-6, "inner sphere kisses the outer");
        }

        // Ring + Caps adds two polar spheres.
        let caps = sphere_packing_mirrors(3, 1.0, 6, 1.0, 1.0, 0.0, 0.0, true);
        assert_eq!(caps.len(), 9, "ring + caps = 1 + 6 + 2");
        assert!(caps[7].z > 0.0 && caps[8].z < 0.0, "caps sit at opposite poles");
    }

    #[test]
    fn repeat_rows_block_the_mirror_itself() {
        // An inversion undoes itself, so the packing blocks REPEATS - a
        // different rule from the Mobius groups, where the blocked index
        // is the inverse two slots away.
        for count in [4usize, 5, 9] {
            for from in 0..count {
                let row = repeat_xaos_row(from, count);
                assert_eq!(row[from], 0.0, "count {count}, from {from}: no repeat");
                assert_eq!(row[(from + 1) % count], 2.0, "successor doubled");
                let total: f32 = row.iter().sum();
                assert_eq!(total, count as f32, "weights still sum to the count");
            }
        }
    }

    #[test]
    fn avoid_rows_reproduce_the_packed_distribution() {
        // Packed rule: draw k uniform in 0..4; if k == (prev+2)%4, use
        // (k+1)%4 instead. So the inverse is unreachable and its successor
        // is twice as likely.
        for from in 0..4 {
            let row = avoid_xaos_row(from);
            let forbidden = (from + 2) % 4;
            assert_eq!(row[forbidden], 0.0, "from {from}: inverse must be unreachable");
            assert_eq!(row[(forbidden + 1) % 4], 2.0, "from {from}: successor doubled");
            let total: f32 = row.iter().sum();
            assert_eq!(total, 4.0, "from {from}: weights still sum to 4");

            // Simulate the packed rule and compare the distribution.
            let mut counts = [0u32; 4];
            for k in 0..4usize {
                let used = if k == forbidden { (k + 1) % 4 } else { k };
                counts[used] += 1;
            }
            for m in 0..4 {
                assert_eq!(
                    row[m], counts[m] as f32,
                    "from {from} -> {m}: xaos weight must match the packed rule"
                );
            }
        }
    }
}
