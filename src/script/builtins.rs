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

// ============================================================================
// L-systems
// ============================================================================

/// One drawn segment of a turtle path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    /// Bracket nesting at the time it was drawn — 0 on the trunk, higher
    /// out along the branches. Useful for colouring by branch depth.
    pub depth: u32,
    /// The symbol that drew it.
    ///
    /// Curves like the Sierpinski arrowhead alternate between two symbols
    /// whose rules are mirror images (`F -> +G-F-G+`, `G -> -F+G+F-`).
    /// The endpoints alone cannot express that: a segment drawn by the
    /// mirrored symbol needs a REFLECTED similarity, not just a rotated
    /// one, or the attractor comes out as the wrong curve entirely.
    pub symbol: char,
}

/// Cap on the expanded string. L-systems grow exponentially: a rule that
/// triples per generation is 3^12 ~ half a million symbols by depth 12,
/// so this fails loudly rather than eating memory.
pub const LSYSTEM_MAX_LEN: usize = 1 << 20;

/// Rewrite `axiom` with `rules` for `depth` generations.
///
/// Every symbol is replaced simultaneously each generation (an L-system
/// is parallel rewriting, unlike a formal grammar); symbols with no rule
/// stand for themselves.
pub fn lsystem_expand(
    axiom: &str,
    rules: &[(char, String)],
    depth: u32,
) -> Result<String, String> {
    let mut current: String = axiom.to_string();
    for generation in 0..depth {
        let mut next = String::with_capacity(current.len() * 2);
        for ch in current.chars() {
            match rules.iter().find(|(k, _)| *k == ch) {
                Some((_, replacement)) => next.push_str(replacement),
                None => next.push(ch),
            }
            if next.len() > LSYSTEM_MAX_LEN {
                return Err(format!(
                    "L-system grew past {LSYSTEM_MAX_LEN} symbols at generation {} — \
                     reduce the depth or shorten the rules",
                    generation + 1
                ));
            }
        }
        current = next;
    }
    Ok(current)
}

/// Walk an expanded L-system string as a turtle, returning drawn segments.
///
/// Commands follow the usual convention (Prusinkiewicz & Lindenmayer,
/// *The Algorithmic Beauty of Plants*):
///
/// * `F`, `G`, `A`, `B` — step forward, drawing
/// * `f`, `g` — step forward without drawing
/// * `+` / `-` — turn left / right by `angle_deg`
/// * `|` — turn 180 degrees
/// * `[` / `]` — push / pop position and heading (branching)
///
/// Any other symbol is ignored, so rules may use extra letters purely as
/// rewriting state.
pub fn turtle(expanded: &str, angle_deg: f64) -> Vec<Segment> {
    let angle = angle_deg * std::f64::consts::PI / 180.0;
    let mut out = Vec::new();
    let (mut x, mut y, mut heading) = (0.0f64, 0.0f64, 0.0f64);
    let mut depth: u32 = 0;
    let mut stack: Vec<(f64, f64, f64, u32)> = Vec::new();

    for ch in expanded.chars() {
        match ch {
            'F' | 'G' | 'A' | 'B' => {
                let (nx, ny) = (x + heading.cos(), y + heading.sin());
                out.push(Segment { x1: x, y1: y, x2: nx, y2: ny, depth, symbol: ch });
                x = nx;
                y = ny;
            }
            'f' | 'g' => {
                x += heading.cos();
                y += heading.sin();
            }
            '+' => heading += angle,
            '-' => heading -= angle,
            '|' => heading += std::f64::consts::PI,
            '[' => {
                stack.push((x, y, heading, depth));
                depth += 1;
            }
            ']' => {
                if let Some((sx, sy, sh, sd)) = stack.pop() {
                    x = sx;
                    y = sy;
                    heading = sh;
                    depth = sd;
                }
            }
            _ => {}
        }
    }
    out
}

/// Find a symbol whose rule is the MIRROR IMAGE of another's.
///
/// Curves like the Sierpinski arrowhead are built from a pair that swap
/// roles with every turn reversed (`F -> +G-F-G+`, `G -> -F+G+F-`). Both
/// symbols draw segments with identical endpoints, so a transform built
/// from endpoints alone loses the chirality and the attractor comes out
/// as the wrong curve. Detecting the pair lets the caller reflect the
/// mirrored pieces without the user having to know any of this.
///
/// Returns the partner of `primary` — the one whose pieces need
/// reflecting — or `None` when no rule mirrors it.
pub fn mirror_partner(rules: &[(char, String)], primary: char) -> Option<char> {
    let primary_rule = rules.iter().find(|(k, _)| *k == primary)?;
    for (other, other_rule) in rules {
        if *other == primary {
            continue;
        }
        // Mirror the primary's rule: reverse every turn and swap the two
        // symbols. If that is exactly the other rule, they are a pair.
        let mirrored: String = primary_rule
            .1
            .chars()
            .map(|c| match c {
                '+' => '-',
                '-' => '+',
                c if c == primary => *other,
                c if c == *other => primary,
                c => c,
            })
            .collect();
        if mirrored == *other_rule {
            return Some(*other);
        }
    }
    None
}

/// Turtle state for the node-rewriting walk.
#[derive(Clone, Copy)]
struct LTurtle {
    x: f64,
    y: f64,
    h: f64,
}

fn lsys_step(ch: char, angle: f64, st: &mut LTurtle, stack: &mut Vec<LTurtle>) {
    match ch {
        // Position-wise, drawing and moving are the same walk.
        'F' | 'G' | 'A' | 'B' | 'f' | 'g' => {
            st.x += st.h.cos();
            st.y += st.h.sin();
        }
        '+' => st.h += angle,
        '-' => st.h -= angle,
        '|' => st.h += std::f64::consts::PI,
        '[' => stack.push(*st),
        ']' => {
            if let Some(s) = stack.pop() {
                *st = s;
            }
        }
        _ => {}
    }
}

/// The IFS of a NODE-rewriting L-system (Hilbert, Peano — space-fillers).
///
/// Edge-rewriting curves decompose by their drawn depth-1 segments. A
/// node-rewriting curve cannot: its drawn pieces are unit steps, nothing
/// shrinks, and the depth-1 construction has nothing to converge to. Its
/// self-similarity lives at the VARIABLE occurrences instead — one
/// generation places a copy of the whole curve at every occurrence of a
/// variable in the rule, joined by edges that become measure-zero in the
/// limit.
///
/// Each occurrence therefore yields one map: the similarity carrying the
/// whole curve's span onto that occurrence's sub-curve span, mirrored
/// when the occurrence is the primary symbol's mirror partner. The spans
/// are measured by walking a DEEP expansion (their positions converge
/// like scaleᵈᵉᵖᵗʰ, so depth 7 puts the error near 1e-2 of a piece),
/// with the depth stepping down until the expansion fits the budget.
///
/// Returned segments are in the same unit-displacement frame
/// `normalize_segments` uses, so `set_segment` consumes them directly;
/// `symbol` is the occurrence's variable, letting the caller apply the
/// mirror exactly as it does for edge systems.
///
/// Requirements, reported rather than guessed around:
/// * the axiom is a single non-drawing variable with a rule;
/// * every variable in that rule is the axiom symbol or its mirror
///   partner — anything else is a graph-directed IFS with two genuinely
///   different sub-curves, which a flat set of transforms cannot express.
pub fn lsystem_node_segments(
    axiom: &str,
    rules: &[(char, String)],
    angle_deg: f64,
) -> Result<Vec<Segment>, String> {
    let angle = angle_deg * std::f64::consts::PI / 180.0;

    let trimmed = axiom.trim();
    let mut it = trimmed.chars();
    let primary = match (it.next(), it.next()) {
        (Some(p), None) => p,
        _ => return Err("the axiom must be a single symbol (like X) for this construction".into()),
    };
    if matches!(primary, 'F' | 'G' | 'A' | 'B' | 'f' | 'g' | '+' | '-' | '|' | '[' | ']') {
        return Err(format!(
            "the axiom `{primary}` draws or moves, so this is an edge-rewriting system"
        ));
    }
    let rule = rules
        .iter()
        .find(|(k, _)| *k == primary)
        .map(|(_, r)| r.clone())
        .ok_or_else(|| format!("the axiom `{primary}` has no rule"))?;
    let partner = mirror_partner(rules, primary);

    let is_var = |c: char| {
        rules.iter().any(|(k, _)| *k == c) && !matches!(c, 'F' | 'G' | 'A' | 'B' | 'f' | 'g')
    };

    let mut occurrences = 0usize;
    for ch in rule.chars() {
        if is_var(ch) {
            occurrences += 1;
            if ch != primary && Some(ch) != partner {
                return Err(format!(
                    "the rule uses `{ch}`, which is neither `{primary}` nor its mirror image — \
                     two genuinely different sub-curves make a graph-directed IFS, which a flat \
                     set of transforms cannot express"
                ));
            }
        }
    }
    if occurrences < 2 {
        return Err("fewer than two variable occurrences — nothing to make copies of".into());
    }

    // A deep expansion pins the sub-curve spans; step the depth down until
    // both expansions fit the budget.
    const CHAR_BUDGET: usize = 400_000;
    let mut depth = 9u32;
    let (exp_primary, exp_partner) = loop {
        let ep = lsystem_expand(&primary.to_string(), rules, depth);
        let pp = match partner {
            Some(p) => lsystem_expand(&p.to_string(), rules, depth),
            None => Ok(String::new()),
        };
        match (ep, pp) {
            (Ok(a), Ok(b)) if a.len() + b.len() <= CHAR_BUDGET => break (a, b),
            _ if depth == 0 => return Err("could not expand this system at any depth".into()),
            _ => depth -= 1,
        }
    };

    // Walk the rule against a pair of expansions, returning chunk
    // endpoints normalized into the unit-displacement frame.
    let walk = |exp_p: &str, exp_q: &str| -> Result<Vec<(f64, f64, f64, f64, char)>, String> {
        let mut st = LTurtle { x: 0.0, y: 0.0, h: 0.0 };
        let mut stack: Vec<LTurtle> = Vec::new();
        let mut chunks: Vec<(f64, f64, f64, f64, char)> = Vec::new();
        for ch in rule.chars() {
            if is_var(ch) {
                let (sx, sy) = (st.x, st.y);
                let body = if ch == primary { exp_p } else { exp_q };
                for bc in body.chars() {
                    lsys_step(bc, angle, &mut st, &mut stack);
                }
                chunks.push((sx, sy, st.x, st.y, ch));
            } else {
                lsys_step(ch, angle, &mut st, &mut stack);
            }
        }
        let (dx, dy) = (st.x, st.y);
        let len2 = dx * dx + dy * dy;
        if len2 < 1e-9 {
            return Err(
                "this curve returns to where it started, so it has no unit-segment frame".into(),
            );
        }
        let map = |px: f64, py: f64| ((px * dx + py * dy) / len2, (py * dx - px * dy) / len2);
        Ok(chunks
            .iter()
            .map(|(x1, y1, x2, y2, sym)| {
                let (ax, ay) = map(*x1, *y1);
                let (bx, by) = map(*x2, *y2);
                (ax, ay, bx, by, *sym)
            })
            .collect())
    };

    let fine = walk(&exp_primary, &exp_partner)?;

    // The measured spans converge geometrically — error ∝ scaleᵈᵉᵖᵗʰ —
    // so one Richardson step against the next-shallower depth removes
    // most of the residual: q∞ ≈ q_d + (q_d − q_{d−1})·σ/(1−σ). Without
    // it the cells miss exact tiling by ~scaleᵈᵉᵖᵗʰ, which shows up as
    // visible seams between the copies.
    let mut refined = fine.clone();
    if depth >= 2 {
        let ep = lsystem_expand(&primary.to_string(), rules, depth - 1);
        let pp = match partner {
            Some(pc) => lsystem_expand(&pc.to_string(), rules, depth - 1),
            None => Ok(String::new()),
        };
        if let (Ok(a), Ok(b)) = (ep, pp) {
            if let Ok(coarse) = walk(&a, &b) {
                if coarse.len() == fine.len() {
                    let mut sigma = 0.0;
                    for f in &fine {
                        sigma += ((f.2 - f.0).powi(2) + (f.3 - f.1).powi(2)).sqrt();
                    }
                    sigma /= fine.len() as f64;
                    if sigma > 0.05 && sigma < 0.95 {
                        let k = sigma / (1.0 - sigma);
                        refined = fine
                            .iter()
                            .zip(coarse.iter())
                            .map(|(f, c)| {
                                (
                                    f.0 + (f.0 - c.0) * k,
                                    f.1 + (f.1 - c.1) * k,
                                    f.2 + (f.2 - c.2) * k,
                                    f.3 + (f.3 - c.3) * k,
                                    f.4,
                                )
                            })
                            .collect();
                    }
                }
            }
        }
    }

    // Grid FASS curves (Hilbert, Peano) have exactly RATIONAL maps: every
    // endpoint is a multiple of 1/m, m the grid subdivision. After the
    // Richardson step we are within ~1e-3 of those values, so snapping is
    // unambiguous — and it is what makes cell exit and next-cell entry
    // agree EXACTLY. Without it, residual error shows up as segments that
    // do not quite meet at cell boundaries. Snap only when the whole
    // configuration is consistent with one grid; otherwise leave it be.
    let mut mean_len = 0.0;
    for f in &refined {
        mean_len += ((f.2 - f.0).powi(2) + (f.3 - f.1).powi(2)).sqrt();
    }
    mean_len /= refined.len() as f64;
    if mean_len > 1e-6 {
        let m = (1.0 / mean_len).round();
        if m >= 2.0 && (1.0 / m - mean_len).abs() < 5e-3 {
            let on_grid = refined.iter().all(|f| {
                [f.0, f.1, f.2, f.3]
                    .iter()
                    .all(|v| (v * m - (v * m).round()).abs() < 0.05)
            });
            if on_grid {
                for f in refined.iter_mut() {
                    f.0 = (f.0 * m).round() / m;
                    f.1 = (f.1 * m).round() / m;
                    f.2 = (f.2 * m).round() / m;
                    f.3 = (f.3 * m).round() / m;
                }
            }
        }
    }

    Ok(refined
        .iter()
        .map(|(ax, ay, bx, by, sym)| Segment {
            x1: *ax,
            y1: *ay,
            x2: *bx,
            y2: *by,
            depth: 0,
            symbol: *sym,
        })
        .collect())
}

/// Bounding box of an L-system curve, normalised onto the unit segment.
///
/// Framing needs the extent of the CURVE, not of the depth-1 pieces — the
/// dragon folds well outside its two starting edges. But a deep expansion
/// is large (Hilbert quadruples its non-terminals every generation), and
/// handing that to a script as an array of arrays blows the interpreter's
/// array limit. So walk it here and return four numbers.
///
/// `max_segments` bounds the work: the depth steps down until the walk
/// fits, since a rough box from a shallower expansion beats an error.
/// Returns `(min_x, min_y, max_x, max_y, depth_used)`.
pub fn lsystem_bounds(
    axiom: &str,
    rules: &[(char, String)],
    depth: u32,
    angle_deg: f64,
    max_segments: usize,
) -> Option<(f64, f64, f64, f64, u32)> {
    for d in (0..=depth).rev() {
        let Ok(expanded) = lsystem_expand(axiom, rules, d) else {
            continue;
        };
        let segs = turtle(&expanded, angle_deg);
        if segs.len() > max_segments {
            continue;
        }
        let Some(norm) = normalize_segments(&segs) else {
            continue;
        };
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for s in &norm {
            b.0 = b.0.min(s.x1).min(s.x2);
            b.1 = b.1.min(s.y1).min(s.y2);
            b.2 = b.2.max(s.x1).max(s.x2);
            b.3 = b.3.max(s.y1).max(s.y2);
        }
        if b.0 <= b.2 {
            return Some((b.0, b.1, b.2, b.3, d));
        }
    }
    None
}

/// The pieces of a bracketed (plant) L-system.
#[derive(Debug)]
pub struct PlantPieces {
    /// Recursion sites: each carries a copy of the whole plant.
    pub branches: Vec<Segment>,
    /// Drawn stem segments: rendered by squashed (thin) maps, the
    /// Barnsley fern's rachis trick.
    pub stems: Vec<Segment>,
}

/// The Barnsley-fern construction for bracketed L-systems.
///
/// A plant rule is self-similar at its RECURSION SITES: the plant is its
/// stem plus transformed copies of itself at every occurrence of the
/// recursing symbol — which is exactly an IFS, the way Barnsley's fern
/// is four maps. Each occurrence yields a branch map (the similarity
/// carrying the whole plant's span onto the occurrence's sub-plant
/// span); each drawn stem segment yields a squashed map that lays a
/// flattened copy of the whole plant along the stem, which is how the
/// fern draws its rachis.
///
/// Spans are measured on a deep expansion with one Richardson step,
/// exactly like the node-rewriting construction — a plant is only
/// ASYMPTOTICALLY self-similar (`F=FF` stems lengthen every
/// generation), so finite-depth measurement plus extrapolation is the
/// honest way to the limit maps. No rational snapping: plants are not
/// grid curves.
///
/// `Segment.depth` carries the bracket nesting at the piece's site, so
/// callers can colour by branch level. `Segment.symbol` is the
/// occurrence's symbol, for mirror pairs.
///
/// Two recursion styles are handled:
/// * variable recursion (`X=F-[[X]+X]+F[+FX]-X`, with `F=FF` or not):
///   non-drawing ruled symbols are the recursion sites, drawn symbols
///   are stems;
/// * drawing recursion (`F=FF-[-F+F+F]+[+F-F-F]`): the primary itself
///   draws, every occurrence is a branch map, and there are no separate
///   stems — the copies cover everything.
pub fn lsystem_plant_segments(
    axiom: &str,
    rules: &[(char, String)],
    angle_deg: f64,
) -> Result<PlantPieces, String> {
    let angle = angle_deg * std::f64::consts::PI / 180.0;

    let trimmed = axiom.trim();
    let mut it = trimmed.chars();
    let primary = match (it.next(), it.next()) {
        (Some(p), None) => p,
        _ => return Err("the axiom must be a single symbol for the plant construction".into()),
    };
    let rule = rules
        .iter()
        .find(|(k, _)| *k == primary)
        .map(|(_, r)| r.clone())
        .ok_or_else(|| format!("the axiom `{primary}` has no rule"))?;
    let partner = mirror_partner(rules, primary);

    let is_drawing = |c: char| matches!(c, 'F' | 'G' | 'A' | 'B');
    let has_rule = |c: char| rules.iter().any(|(k, _)| *k == c);
    let primary_draws = is_drawing(primary);

    // Validate: every non-drawing ruled symbol in the rule must be the
    // primary or its mirror partner.
    for ch in rule.chars() {
        if has_rule(ch) && !is_drawing(ch) && !matches!(ch, 'f' | 'g') {
            if ch != primary && Some(ch) != partner {
                return Err(format!(
                    "the rule uses `{ch}`, which is neither `{primary}` nor its mirror image — \
                     two genuinely different sub-plants make a graph-directed IFS, which a flat \
                     set of transforms cannot express"
                ));
            }
        }
    }

    // Expansions for every ruled symbol that appears (primary, partner,
    // and ruled drawing symbols like `F=FF`), at a depth that fits.
    let mut needed: Vec<char> = vec![primary];
    if let Some(p) = partner {
        needed.push(p);
    }
    for ch in rule.chars() {
        if has_rule(ch) && !needed.contains(&ch) {
            needed.push(ch);
        }
    }

    const CHAR_BUDGET: usize = 400_000;
    let mut depth = 8u32;
    let expansions_at = |d: u32| -> Result<Vec<(char, String)>, ()> {
        let mut out = Vec::new();
        let mut total = 0usize;
        for &sym in &needed {
            match lsystem_expand(&sym.to_string(), rules, d) {
                Ok(e) => {
                    total += e.len();
                    out.push((sym, e));
                }
                Err(_) => return Err(()),
            }
        }
        if total > CHAR_BUDGET {
            return Err(());
        }
        Ok(out)
    };
    let exp_fine = loop {
        match expansions_at(depth) {
            Ok(e) => break e,
            Err(()) if depth == 0 => {
                return Err("could not expand this system at any depth".into())
            }
            Err(()) => depth -= 1,
        }
    };

    // One walk of the rule against a set of expansions: literal steps and
    // brackets move the turtle; ruled symbols walk their expansion and
    // record the span. Classification per occurrence:
    //   non-drawing ruled (X)          -> branch
    //   drawing, primary-recursive (F) -> branch when the primary draws
    //   drawing otherwise              -> stem (expanded span if ruled,
    //                                    a single step if not)
    let walk = |exps: &Vec<(char, String)>| -> Result<(Vec<Segment>, Vec<Segment>), String> {
        let body_of = |c: char| exps.iter().find(|(k, _)| *k == c).map(|(_, e)| e.as_str());
        let mut st = LTurtle { x: 0.0, y: 0.0, h: 0.0 };
        let mut stack: Vec<LTurtle> = Vec::new();
        let mut branches: Vec<Segment> = Vec::new();
        let mut stems: Vec<Segment> = Vec::new();
        for ch in rule.chars() {
            let nest = stack.len() as u32;
            let ruled = has_rule(ch);
            if ruled && !is_drawing(ch) && !matches!(ch, 'f' | 'g') {
                // Variable recursion site.
                let (sx, sy) = (st.x, st.y);
                if let Some(body) = body_of(ch) {
                    for bc in body.chars() {
                        lsys_step(bc, angle, &mut st, &mut stack);
                    }
                }
                branches.push(Segment { x1: sx, y1: sy, x2: st.x, y2: st.y, depth: nest, symbol: ch });
            } else if is_drawing(ch) {
                let (sx, sy) = (st.x, st.y);
                if let Some(body) = body_of(ch) {
                    for bc in body.chars() {
                        lsys_step(bc, angle, &mut st, &mut stack);
                    }
                } else {
                    lsys_step(ch, angle, &mut st, &mut stack);
                }
                let seg = Segment { x1: sx, y1: sy, x2: st.x, y2: st.y, depth: nest, symbol: ch };
                if primary_draws && (ch == primary || Some(ch) == partner) {
                    branches.push(seg);
                } else {
                    stems.push(seg);
                }
            } else {
                lsys_step(ch, angle, &mut st, &mut stack);
            }
        }
        let (dx, dy) = (st.x, st.y);
        let len2 = dx * dx + dy * dy;
        if len2 < 1e-9 {
            return Err(
                "this plant returns to where it started, so it has no unit-displacement frame"
                    .into(),
            );
        }
        let map = |px: f64, py: f64| ((px * dx + py * dy) / len2, (py * dx - px * dy) / len2);
        let norm = |v: &mut Vec<Segment>| {
            for s in v.iter_mut() {
                let (ax, ay) = map(s.x1, s.y1);
                let (bx, by) = map(s.x2, s.y2);
                s.x1 = ax;
                s.y1 = ay;
                s.x2 = bx;
                s.y2 = by;
            }
        };
        norm(&mut branches);
        norm(&mut stems);
        Ok((branches, stems))
    };

    let (mut branches, mut stems) = walk(&exp_fine)?;
    if branches.is_empty() {
        return Err("no recursion sites found — nothing carries a copy of the plant".into());
    }

    // Richardson step against the next-shallower depth, as in the node
    // construction: spans converge geometrically toward the limit maps.
    if depth >= 2 {
        if let Ok(exp_coarse) = expansions_at(depth - 1) {
            if let Ok((cb, cs)) = walk(&exp_coarse) {
                if cb.len() == branches.len() && cs.len() == stems.len() {
                    let mut sigma = 0.0;
                    for f in &branches {
                        sigma += ((f.x2 - f.x1).powi(2) + (f.y2 - f.y1).powi(2)).sqrt();
                    }
                    sigma /= branches.len() as f64;
                    if sigma > 0.05 && sigma < 0.95 {
                        let k = sigma / (1.0 - sigma);
                        let refine = |fine: &mut Vec<Segment>, coarse: &Vec<Segment>| {
                            for (f, c) in fine.iter_mut().zip(coarse.iter()) {
                                f.x1 += (f.x1 - c.x1) * k;
                                f.y1 += (f.y1 - c.y1) * k;
                                f.x2 += (f.x2 - c.x2) * k;
                                f.y2 += (f.y2 - c.y2) * k;
                            }
                        };
                        refine(&mut branches, &cb);
                        refine(&mut stems, &cs);
                    }
                }
            }
        }
    }

    Ok(PlantPieces { branches, stems })
}

// ============================================================================
// 3D L-systems
// ============================================================================

/// 3D turtle state: position plus a right-handed orientation frame.
/// Columns of `r` are heading H, left L, up U; the turtle starts facing
/// +x with up = +z, so 2D rules (no pitch/roll) reproduce the 2D turtle
/// exactly in the z = 0 plane.
#[derive(Clone, Copy)]
struct LTurtle3 {
    p: [f64; 3],
    r: [[f64; 3]; 3],
}

impl LTurtle3 {
    fn new() -> Self {
        Self { p: [0.0; 3], r: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] }
    }
}

fn mat3_mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    out
}

/// ABOP's 3D turtle commands. Local-axis rotations are RIGHT
/// multiplications; `+`/`-` yaw about up, `&`/`^` pitch about left,
/// `\`/`/` roll about heading. Sign conventions are one consistent
/// chirality — mirrored conventions elsewhere mirror the plant, nothing
/// worse.
fn lsys_step3(ch: char, angle: f64, st: &mut LTurtle3, stack: &mut Vec<LTurtle3>) {
    let (s, c) = angle.sin_cos();
    match ch {
        'F' | 'G' | 'A' | 'B' | 'f' | 'g' => {
            st.p[0] += st.r[0][0];
            st.p[1] += st.r[1][0];
            st.p[2] += st.r[2][0];
        }
        '+' => st.r = mat3_mul(&st.r, &[[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]),
        '-' => st.r = mat3_mul(&st.r, &[[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]),
        '&' => st.r = mat3_mul(&st.r, &[[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]),
        '^' => st.r = mat3_mul(&st.r, &[[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]]),
        '\\' => st.r = mat3_mul(&st.r, &[[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]),
        '/' => st.r = mat3_mul(&st.r, &[[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]]),
        '|' => st.r = mat3_mul(&st.r, &[[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]]),
        '[' => stack.push(*st),
        ']' => {
            if let Some(prev) = stack.pop() {
                *st = prev;
            }
        }
        _ => {}
    }
}

/// One extracted 3D piece: a full affine (`x' = m·x + t`, row-major
/// `m00 m01 m02 m10 .. m22` then `tx ty tz`), plus bracket depth and the
/// symbol that produced it.
#[derive(Debug, Clone, Copy)]
pub struct Piece3 {
    pub m: [f64; 12],
    pub depth: u32,
    pub symbol: char,
}

/// The 3D pieces of a bracketed or curve L-system.
#[derive(Debug)]
pub struct Pieces3 {
    pub branches: Vec<Piece3>,
    pub stems: Vec<Piece3>,
}

/// Does a rule set use the 3D commands (`&`, `^`, `\`, `/`)?
pub fn lsystem_uses_3d(axiom: &str, rules: &[(char, String)]) -> bool {
    let is3d = |s: &str| s.chars().any(|c| matches!(c, '&' | '^' | '\\' | '/'));
    is3d(axiom) || rules.iter().any(|(_, r)| is3d(r))
}

/// Unified 3D extraction — plants, edge curves and node curves are all
/// the same construction once pieces carry a full frame:
///
/// * recursion sites (variable, or the primary itself when it draws)
///   become BRANCH maps `t + s·R·x`: translation and scale measured on a
///   deep expansion with a Richardson step (self-similarity is only
///   asymptotic), rotation taken from the turtle's exact frame at the
///   site and then nudged so the frame's heading agrees with the
///   measured sub-displacement;
/// * other drawn symbols become STEM pieces (same form, scale = drawn
///   length), which callers squash for the Barnsley rachis look or skip
///   for curves, where connectors are measure-zero;
/// * mirror-partner occurrences get the reflection folded in (local
///   left axis flipped), the 3D analogue of the 2D mirror flag.
///
/// In 2D the map's rotation was recoverable from a segment's endpoints;
/// in 3D it is not — there is a free roll about the segment — which is
/// exactly why pieces here carry the frame instead of endpoints.
pub fn lsystem_pieces3(
    axiom: &str,
    rules: &[(char, String)],
    angle_deg: f64,
) -> Result<Pieces3, String> {
    let angle = angle_deg * std::f64::consts::PI / 180.0;

    let trimmed = axiom.trim();
    let mut it = trimmed.chars();
    let primary = match (it.next(), it.next()) {
        (Some(p), None) => p,
        _ => return Err("the axiom must be a single symbol for this construction".into()),
    };
    let rule = rules
        .iter()
        .find(|(k, _)| *k == primary)
        .map(|(_, r)| r.clone())
        .ok_or_else(|| format!("the axiom `{primary}` has no rule"))?;
    let partner = mirror_partner(rules, primary);

    let is_drawing = |c: char| matches!(c, 'F' | 'G' | 'A' | 'B');
    let has_rule = |c: char| rules.iter().any(|(k, _)| *k == c);
    let primary_draws = is_drawing(primary);

    for ch in rule.chars() {
        if has_rule(ch) && !is_drawing(ch) && !matches!(ch, 'f' | 'g') {
            if ch != primary && Some(ch) != partner {
                return Err(format!(
                    "the rule uses `{ch}`, which is neither `{primary}` nor its mirror image — \
                     a graph-directed IFS, which a flat set of transforms cannot express"
                ));
            }
        }
    }

    let mut needed: Vec<char> = vec![primary];
    if let Some(p) = partner {
        needed.push(p);
    }
    for ch in rule.chars() {
        if has_rule(ch) && !needed.contains(&ch) {
            needed.push(ch);
        }
    }

    const CHAR_BUDGET: usize = 400_000;
    let mut depth = 8u32;
    let expansions_at = |d: u32| -> Result<Vec<(char, String)>, ()> {
        let mut out = Vec::new();
        let mut total = 0usize;
        for &sym in &needed {
            match lsystem_expand(&sym.to_string(), rules, d) {
                Ok(e) => {
                    total += e.len();
                    out.push((sym, e));
                }
                Err(_) => return Err(()),
            }
        }
        if total > CHAR_BUDGET {
            return Err(());
        }
        Ok(out)
    };
    let exp_fine = loop {
        match expansions_at(depth) {
            Ok(e) => break e,
            Err(()) if depth == 0 => {
                return Err("could not expand this system at any depth".into())
            }
            Err(()) => depth -= 1,
        }
    };

    // Raw walk output per site: entry position, exit position, entry
    // frame, nesting, symbol, is_branch.
    struct Site {
        p1: [f64; 3],
        p2: [f64; 3],
        r: [[f64; 3]; 3],
        depth: u32,
        symbol: char,
        branch: bool,
    }

    let walk = |exps: &Vec<(char, String)>| -> Result<(Vec<Site>, f64), String> {
        let body_of = |c: char| exps.iter().find(|(k, _)| *k == c).map(|(_, e)| e.as_str());
        let mut st = LTurtle3::new();
        let mut stack: Vec<LTurtle3> = Vec::new();
        let mut sites: Vec<Site> = Vec::new();
        for ch in rule.chars() {
            let nest = stack.len() as u32;
            let ruled = has_rule(ch);
            let variable = ruled && !is_drawing(ch) && !matches!(ch, 'f' | 'g');
            if variable || is_drawing(ch) {
                let entry = st;
                if let Some(body) = if ruled { body_of(ch) } else { None } {
                    for bc in body.chars() {
                        lsys_step3(bc, angle, &mut st, &mut stack);
                    }
                } else {
                    lsys_step3(ch, angle, &mut st, &mut stack);
                }
                let branch = variable || (primary_draws && (ch == primary || Some(ch) == partner));
                sites.push(Site {
                    p1: entry.p,
                    p2: st.p,
                    r: entry.r,
                    depth: nest,
                    symbol: ch,
                    branch,
                });
            } else {
                lsys_step3(ch, angle, &mut st, &mut stack);
            }
        }
        let d2 = st.p[0] * st.p[0] + st.p[1] * st.p[1] + st.p[2] * st.p[2];
        if d2 < 1e-9 {
            return Err("this system returns to where it started, so it has no unit frame".into());
        }
        Ok((sites, d2.sqrt()))
    };

    let (mut fine, scale_fine) = walk(&exp_fine)?;
    // Normalize positions by the whole displacement LENGTH only — the
    // start frame is the global frame, so no rotation is applied (unlike
    // 2D, where rotating displacement onto x̂ was a free convenience).
    for s in fine.iter_mut() {
        for k in 0..3 {
            s.p1[k] /= scale_fine;
            s.p2[k] /= scale_fine;
        }
    }

    // Richardson step against the next-shallower depth (positions only;
    // frames are exact turtle states).
    if depth >= 2 {
        if let Ok(coarse_exp) = expansions_at(depth - 1) {
            if let Ok((mut coarse, scale_coarse)) = walk(&coarse_exp) {
                if coarse.len() == fine.len() {
                    for s in coarse.iter_mut() {
                        for k in 0..3 {
                            s.p1[k] /= scale_coarse;
                            s.p2[k] /= scale_coarse;
                        }
                    }
                    let mut sigma = 0.0;
                    let mut nb = 0.0;
                    for s in fine.iter().filter(|s| s.branch) {
                        let d = [s.p2[0] - s.p1[0], s.p2[1] - s.p1[1], s.p2[2] - s.p1[2]];
                        sigma += (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                        nb += 1.0;
                    }
                    if nb > 0.0 {
                        sigma /= nb;
                        if sigma > 0.05 && sigma < 0.95 {
                            let k = sigma / (1.0 - sigma);
                            for (f, c) in fine.iter_mut().zip(coarse.iter()) {
                                for a in 0..3 {
                                    f.p1[a] += (f.p1[a] - c.p1[a]) * k;
                                    f.p2[a] += (f.p2[a] - c.p2[a]) * k;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // The whole system's displacement direction in the global frame: the
    // shape every sub-copy repeats. Used to align each site's frame with
    // its measured sub-displacement.
    let whole_dir = {
        // After normalization the whole runs from the origin a unit
        // distance; recompute from the fine walk's final state direction.
        let last = fine
            .iter()
            .map(|s| s.p2)
            .last()
            .unwrap_or([1.0, 0.0, 0.0]);
        // Not exactly the endpoint (trailing turns don't move), but the
        // endpoint of the last site is within the Richardson residual of
        // it; direction is what matters here.
        let n = (last[0] * last[0] + last[1] * last[1] + last[2] * last[2]).sqrt();
        if n > 1e-9 {
            [last[0] / n, last[1] / n, last[2] / n]
        } else {
            [1.0, 0.0, 0.0]
        }
    };

    let mut branches = Vec::new();
    let mut stems = Vec::new();
    for s in &fine {
        let d = [s.p2[0] - s.p1[0], s.p2[1] - s.p1[1], s.p2[2] - s.p1[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len < 1e-9 {
            continue;
        }

        let mut r = s.r;
        if s.branch {
            // Nudge the frame so R·whole_dir lands on the measured
            // sub-displacement: the map must send the whole's run onto
            // the sub-copy's run, and at finite depth the exact turtle
            // frame misses that by the convergence residual.
            let from = [
                r[0][0] * whole_dir[0] + r[0][1] * whole_dir[1] + r[0][2] * whole_dir[2],
                r[1][0] * whole_dir[0] + r[1][1] * whole_dir[1] + r[1][2] * whole_dir[2],
                r[2][0] * whole_dir[0] + r[2][1] * whole_dir[1] + r[2][2] * whole_dir[2],
            ];
            let to = [d[0] / len, d[1] / len, d[2] / len];
            r = mat3_mul(&rotation_between(&from, &to), &r);
        } else {
            // Stems: the map lays the unit x axis along the drawn
            // segment, so align heading with it directly.
            let h = [r[0][0], r[1][0], r[2][0]];
            let to = [d[0] / len, d[1] / len, d[2] / len];
            r = mat3_mul(&rotation_between(&h, &to), &r);
        }

        // Mirror partners get the reflection folded in: flip the local
        // left axis (column 1), the 3D analogue of the 2D mirror flag.
        let mirror = Some(s.symbol) == partner;
        let flip = if mirror { -1.0 } else { 1.0 };

        let m = [
            len * r[0][0], flip * len * r[0][1], len * r[0][2],
            len * r[1][0], flip * len * r[1][1], len * r[1][2],
            len * r[2][0], flip * len * r[2][1], len * r[2][2],
            s.p1[0], s.p1[1], s.p1[2],
        ];
        let piece = Piece3 { m, depth: s.depth, symbol: s.symbol };
        if s.branch {
            branches.push(piece);
        } else {
            stems.push(piece);
        }
    }

    if branches.is_empty() {
        return Err("no recursion sites found — nothing carries a copy of the system".into());
    }

    // Rotate the global frame so the whole system's displacement lies
    // along +x — the same convention the 2D extractor uses, and the one
    // the path variation's anchors assume: the curve must run from the
    // origin to (1, 0, 0), or "exit of cell i" anchored at x̂ points the
    // wrong way and the chain shatters (found as a nearly empty render).
    // Maps conjugate: A' = G·A·Gᵀ, t' = G·t.
    let g = rotation_between(&whole_dir, &[1.0, 0.0, 0.0]);
    let reframe = |piece: &mut Piece3| {
        let a = [
            [piece.m[0], piece.m[1], piece.m[2]],
            [piece.m[3], piece.m[4], piece.m[5]],
            [piece.m[6], piece.m[7], piece.m[8]],
        ];
        let gt = [
            [g[0][0], g[1][0], g[2][0]],
            [g[0][1], g[1][1], g[2][1]],
            [g[0][2], g[1][2], g[2][2]],
        ];
        let ga = mat3_mul(&g, &a);
        let gagt = mat3_mul(&ga, &gt);
        let t = [piece.m[9], piece.m[10], piece.m[11]];
        for i in 0..3 {
            for j in 0..3 {
                piece.m[i * 3 + j] = gagt[i][j];
            }
            piece.m[9 + i] = g[i][0] * t[0] + g[i][1] * t[1] + g[i][2] * t[2];
        }
    };
    for b in branches.iter_mut() {
        reframe(b);
    }
    for st in stems.iter_mut() {
        reframe(st);
    }

    Ok(Pieces3 { branches, stems })
}

/// The rotation carrying unit vector `from` onto unit vector `to`
/// (identity when they already agree; a half-turn about any
/// perpendicular when opposed).
fn rotation_between(from: &[f64; 3], to: &[f64; 3]) -> [[f64; 3]; 3] {
    let cross = [
        from[1] * to[2] - from[2] * to[1],
        from[2] * to[0] - from[0] * to[2],
        from[0] * to[1] - from[1] * to[0],
    ];
    let dot = from[0] * to[0] + from[1] * to[1] + from[2] * to[2];
    let s2 = cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2];
    if s2 < 1e-18 {
        if dot > 0.0 {
            return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        }
        // Opposed: rotate half a turn about any axis perpendicular to `from`.
        let axis = if from[0].abs() < 0.9 {
            let a = [0.0, -from[2], from[1]];
            let n = (a[1] * a[1] + a[2] * a[2]).sqrt();
            [0.0, a[1] / n, a[2] / n]
        } else {
            let a = [-from[2], 0.0, from[0]];
            let n = (a[0] * a[0] + a[2] * a[2]).sqrt();
            [a[0] / n, 0.0, a[2] / n]
        };
        let (x, y, z) = (axis[0], axis[1], axis[2]);
        return [
            [2.0 * x * x - 1.0, 2.0 * x * y, 2.0 * x * z],
            [2.0 * x * y, 2.0 * y * y - 1.0, 2.0 * y * z],
            [2.0 * x * z, 2.0 * y * z, 2.0 * z * z - 1.0],
        ];
    }
    // Rodrigues via the cross-product matrix: R = I + K + K²·(1-c)/s².
    let k = [
        [0.0, -cross[2], cross[1]],
        [cross[2], 0.0, -cross[0]],
        [-cross[1], cross[0], 0.0],
    ];
    let k2 = mat3_mul(&k, &k);
    let f = (1.0 - dot) / s2;
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = if i == j { 1.0 } else { 0.0 } + k[i][j] + k2[i][j] * f;
        }
    }
    out
}

/// 3D bounding box of the drawn system: (min_x, min_y, min_z, max_x,
/// max_y, max_z), normalized by the whole displacement length. Framing
/// uses xy; the camera handles z.
pub fn lsystem_bounds3(
    axiom: &str,
    rules: &[(char, String)],
    depth: u32,
    angle_deg: f64,
    max_steps: usize,
) -> Option<(f64, f64, f64, f64, f64, f64)> {
    let angle = angle_deg * std::f64::consts::PI / 180.0;
    for d in (0..=depth).rev() {
        let Ok(expanded) = lsystem_expand(axiom, rules, d) else {
            continue;
        };
        if expanded.len() > max_steps {
            continue;
        }
        let mut st = LTurtle3::new();
        let mut stack = Vec::new();
        let mut b = (f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN);
        let mut drew = false;
        for ch in expanded.chars() {
            lsys_step3(ch, angle, &mut st, &mut stack);
            if matches!(ch, 'F' | 'G' | 'A' | 'B') {
                drew = true;
                b.0 = b.0.min(st.p[0]);
                b.1 = b.1.min(st.p[1]);
                b.2 = b.2.min(st.p[2]);
                b.3 = b.3.max(st.p[0]);
                b.4 = b.4.max(st.p[1]);
                b.5 = b.5.max(st.p[2]);
            }
        }
        let d2 = st.p[0] * st.p[0] + st.p[1] * st.p[1] + st.p[2] * st.p[2];
        if !drew || d2 < 1e-9 {
            continue;
        }
        // Same frame as lsystem_pieces3: displacement along +x. The box
        // must be axis-aligned in THAT frame or panning frames the wrong
        // spot, so re-walk the points through the rotation.
        let n = d2.sqrt();
        let dirv = [st.p[0] / n, st.p[1] / n, st.p[2] / n];
        let g = rotation_between(&dirv, &[1.0, 0.0, 0.0]);
        let mut st2 = LTurtle3::new();
        let mut stack2 = Vec::new();
        let mut bb = (f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN);
        for ch in expanded.chars() {
            lsys_step3(ch, angle, &mut st2, &mut stack2);
            if matches!(ch, 'F' | 'G' | 'A' | 'B') {
                let q = [
                    (g[0][0] * st2.p[0] + g[0][1] * st2.p[1] + g[0][2] * st2.p[2]) / n,
                    (g[1][0] * st2.p[0] + g[1][1] * st2.p[1] + g[1][2] * st2.p[2]) / n,
                    (g[2][0] * st2.p[0] + g[2][1] * st2.p[1] + g[2][2] * st2.p[2]) / n,
                ];
                bb.0 = bb.0.min(q[0]);
                bb.1 = bb.1.min(q[1]);
                bb.2 = bb.2.min(q[2]);
                bb.3 = bb.3.max(q[0]);
                bb.4 = bb.4.max(q[1]);
                bb.5 = bb.5.max(q[2]);
            }
        }
        return Some(bb);
    }
    None
}

// ============================================================================
// Graph-directed L-systems (multi-variable) and the 3D Hilbert curve
// ============================================================================

/// One piece of a graph-directed system: a full 3D affine plus WHICH
/// curve type it consumes (`occ`) and which it produces (`owner`).
#[derive(Debug, Clone, Copy)]
pub struct GraphPiece {
    pub m: [f64; 12],
    pub depth: u32,
    /// The occurrence's symbol — the curve type this map CONSUMES.
    pub occ: char,
    /// The rule the occurrence sits in — the curve type it PRODUCES.
    pub owner: char,
}

/// Multi-variable (graph-directed) extraction.
///
/// A system like ABOP's 3D Hilbert has several variables whose curves
/// are built from copies of EACH OTHER — a graph-directed IFS. A flat
/// transform set cannot express that (every map would apply to one
/// attractor), but the chaos game for a GIFS is exactly a flame with
/// XAOS: one transform per occurrence, allowed to follow another only
/// when it consumes the type the other produced
/// (`occ(next) == owner(prev)`), with opacity hiding every type except
/// the axiom's so only the wanted curve plots. The scaffold types still
/// drive the dynamics; they are just invisible.
///
/// Each variable's curve is normalized into its own unit frame
/// (displacement along +x); a map for an occurrence of `W` inside
/// `rule(V)` carries W's frame onto the occurrence's span inside V's
/// frame. Spans measured deep with a Richardson step; orientations from
/// the exact turtle frame, nudged onto the measured span.
pub fn lsystem_graph_pieces(
    axiom: &str,
    rules: &[(char, String)],
    angle_deg: f64,
) -> Result<Vec<GraphPiece>, String> {
    let angle = angle_deg * std::f64::consts::PI / 180.0;

    let trimmed = axiom.trim();
    let mut it = trimmed.chars();
    let primary = match (it.next(), it.next()) {
        (Some(p), None) => p,
        _ => return Err("the axiom must be a single symbol".into()),
    };
    let is_drawing = |c: char| matches!(c, 'F' | 'G' | 'A' | 'B');
    let has_rule = |c: char| rules.iter().any(|(k, _)| *k == c);
    let is_var = |c: char| has_rule(c) && !is_drawing(c) && !matches!(c, 'f' | 'g');
    if !is_var(primary) {
        return Err(format!("the axiom `{primary}` must be a variable with a rule"));
    }
    let rule_of = |c: char| {
        rules
            .iter()
            .find(|(k, _)| *k == c)
            .map(|(_, r)| r.clone())
            .unwrap_or_default()
    };

    // Types reachable from the axiom, in discovery order.
    let mut types: Vec<char> = vec![primary];
    let mut i = 0;
    while i < types.len() {
        for ch in rule_of(types[i]).chars() {
            if is_var(ch) && !types.contains(&ch) {
                types.push(ch);
            }
        }
        i += 1;
    }

    // Everything that needs a deep expansion: the types, plus ruled
    // drawing symbols (F=FF stems elongate).
    let mut needed = types.clone();
    for t in &types {
        for ch in rule_of(*t).chars() {
            if has_rule(ch) && !needed.contains(&ch) {
                needed.push(ch);
            }
        }
    }

    const CHAR_BUDGET: usize = 400_000;
    let mut depth = 8u32;
    let expansions_at = |d: u32| -> Result<Vec<(char, String)>, ()> {
        let mut out = Vec::new();
        let mut total = 0usize;
        for &sym in &needed {
            match lsystem_expand(&sym.to_string(), rules, d) {
                Ok(e) => {
                    total += e.len();
                    out.push((sym, e));
                }
                Err(_) => return Err(()),
            }
        }
        if total > CHAR_BUDGET {
            return Err(());
        }
        Ok(out)
    };
    let exp_fine = loop {
        match expansions_at(depth) {
            Ok(e) => break e,
            Err(()) if depth == 0 => {
                return Err("could not expand this system at any depth".into())
            }
            Err(()) => depth -= 1,
        }
    };

    struct Site {
        p1: [f64; 3],
        p2: [f64; 3],
        r: [[f64; 3]; 3],
        depth: u32,
        occ: char,
    }
    struct TypeWalk {
        sites: Vec<Site>,
        disp: [f64; 3],
    }

    // Walk one type's rule against a set of expansions.
    let walk_type = |v: char, exps: &Vec<(char, String)>| -> Result<TypeWalk, String> {
        let body_of = |c: char| exps.iter().find(|(k, _)| *k == c).map(|(_, e)| e.as_str());
        let mut st = LTurtle3::new();
        let mut stack: Vec<LTurtle3> = Vec::new();
        let mut sites: Vec<Site> = Vec::new();
        for ch in rule_of(v).chars() {
            let nest = stack.len() as u32;
            if is_var(ch) {
                let entry = st;
                if let Some(body) = body_of(ch) {
                    for bc in body.chars() {
                        lsys_step3(bc, angle, &mut st, &mut stack);
                    }
                }
                sites.push(Site { p1: entry.p, p2: st.p, r: entry.r, depth: nest, occ: ch });
            } else if is_drawing(ch) && has_rule(ch) {
                if let Some(body) = body_of(ch) {
                    for bc in body.chars() {
                        lsys_step3(bc, angle, &mut st, &mut stack);
                    }
                }
            } else {
                lsys_step3(ch, angle, &mut st, &mut stack);
            }
        }
        let d2 = st.p[0] * st.p[0] + st.p[1] * st.p[1] + st.p[2] * st.p[2];
        if d2 < 1e-9 {
            return Err(format!(
                "type `{v}` returns to where it started, so it has no unit frame"
            ));
        }
        Ok(TypeWalk { sites, disp: st.p })
    };

    let mut fine: Vec<(char, TypeWalk)> = Vec::new();
    for &v in &types {
        fine.push((v, walk_type(v, &exp_fine)?));
    }
    let coarse: Option<Vec<(char, TypeWalk)>> = if depth >= 2 {
        expansions_at(depth - 1).ok().and_then(|e| {
            let mut out = Vec::new();
            for &v in &types {
                match walk_type(v, &e) {
                    Ok(w) => out.push((v, w)),
                    Err(_) => return None,
                }
            }
            Some(out)
        })
    } else {
        None
    };

    // Per-type frames: normalize by displacement length and rotate the
    // displacement onto +x, so every type's curve runs origin -> (1,0,0).
    let frame_of = |disp: &[f64; 3]| -> ([[f64; 3]; 3], f64) {
        let n = (disp[0] * disp[0] + disp[1] * disp[1] + disp[2] * disp[2]).sqrt();
        let dir = [disp[0] / n, disp[1] / n, disp[2] / n];
        (rotation_between(&dir, &[1.0, 0.0, 0.0]), n)
    };

    let mut pieces: Vec<GraphPiece> = Vec::new();
    for (ti, (v, w)) in fine.iter().enumerate() {
        let (g_v, n_v) = frame_of(&w.disp);
        for (si, s) in w.sites.iter().enumerate() {
            // Occurrence positions in V's normalized frame, Richardson-
            // refined against the coarser depth when available.
            let nrm = |p: &[f64; 3]| {
                [
                    (g_v[0][0] * p[0] + g_v[0][1] * p[1] + g_v[0][2] * p[2]) / n_v,
                    (g_v[1][0] * p[0] + g_v[1][1] * p[1] + g_v[1][2] * p[2]) / n_v,
                    (g_v[2][0] * p[0] + g_v[2][1] * p[1] + g_v[2][2] * p[2]) / n_v,
                ]
            };
            let mut p1 = nrm(&s.p1);
            let mut p2 = nrm(&s.p2);
            if let Some(cw) = &coarse {
                let (cv, cwk) = &cw[ti];
                if *cv == *v && cwk.sites.len() == w.sites.len() {
                    let (g_c, n_c) = frame_of(&cwk.disp);
                    let cs = &cwk.sites[si];
                    let cn = |p: &[f64; 3]| {
                        [
                            (g_c[0][0] * p[0] + g_c[0][1] * p[1] + g_c[0][2] * p[2]) / n_c,
                            (g_c[1][0] * p[0] + g_c[1][1] * p[1] + g_c[1][2] * p[2]) / n_c,
                            (g_c[2][0] * p[0] + g_c[2][1] * p[1] + g_c[2][2] * p[2]) / n_c,
                        ]
                    };
                    let c1 = cn(&cs.p1);
                    let c2 = cn(&cs.p2);
                    let d = [p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]];
                    let sig = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                    if sig > 0.05 && sig < 0.95 {
                        let k = sig / (1.0 - sig);
                        for a in 0..3 {
                            p1[a] += (p1[a] - c1[a]) * k;
                            p2[a] += (p2[a] - c2[a]) * k;
                        }
                    }
                }
            }

            let d = [p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]];
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if len < 1e-9 {
                continue;
            }

            // Rotation: V's frame, times the turtle's entry orientation,
            // times W's frame undone; then nudged so x̂ lands on the
            // measured span (the finite-depth residual).
            let occ_disp = fine
                .iter()
                .find(|(k, _)| *k == s.occ)
                .map(|(_, tw)| tw.disp)
                .unwrap_or([1.0, 0.0, 0.0]);
            let (g_w, _) = frame_of(&occ_disp);
            let gw_t = [
                [g_w[0][0], g_w[1][0], g_w[2][0]],
                [g_w[0][1], g_w[1][1], g_w[2][1]],
                [g_w[0][2], g_w[1][2], g_w[2][2]],
            ];
            let mut r = mat3_mul(&mat3_mul(&g_v, &s.r), &gw_t);
            let rx = [r[0][0], r[1][0], r[2][0]];
            let to = [d[0] / len, d[1] / len, d[2] / len];
            r = mat3_mul(&rotation_between(&rx, &to), &r);

            pieces.push(GraphPiece {
                m: [
                    len * r[0][0], len * r[0][1], len * r[0][2],
                    len * r[1][0], len * r[1][1], len * r[1][2],
                    len * r[2][0], len * r[2][1], len * r[2][2],
                    p1[0], p1[1], p1[2],
                ],
                depth: s.depth,
                occ: s.occ,
                owner: *v,
            });
        }
    }

    if pieces.len() < 2 {
        return Err("fewer than two recursion sites across the whole graph".into());
    }
    Ok(pieces)
}

/// A self-similar 3D Hilbert curve: eight maps at scale 1/2, one per
/// octant in face-adjacent (Gray code) visiting order.
///
/// Multi-variable 3D Hilbert L-systems are graph-directed, but
/// SINGLE-type 3D Hilbert curves exist too: all eight octant sub-curves
/// congruent to the whole via symmetries of the cube (Haverkort's
/// inventory of 3D Hilbert curves). Rather than trusting memory for
/// published matrices, the maps are found by a deterministic search:
/// walk the octants in Gray-code order, and for each pick the first cube
/// symmetry (of the 48) that carries the curve's global entry/exit
/// corners onto octant corners that CHAIN — each octant's exit is the
/// next octant's entry, ending at the global exit. Continuity of the
/// limit curve is exactly that chaining, the same condition the 2D
/// construction rests on.
///
/// Frame: the unit cube [0,1]³, entry (0,0,0), exit (1,0,0) — matching
/// the path variation's anchors. Deterministic: same maps every call.
pub fn hilbert3d_maps() -> Vec<[f64; 12]> {
    // Octants in Gray-code order (consecutive octants share a face),
    // starting at the entry corner's octant, ending at the exit's.
    let gray = [0b000u8, 0b001, 0b011, 0b010, 0b110, 0b111, 0b101, 0b100];
    let oct = |g: u8| -> [f64; 3] {
        [
            if g & 0b100 != 0 { 0.5 } else { 0.0 },
            if g & 0b010 != 0 { 0.5 } else { 0.0 },
            if g & 0b001 != 0 { 0.5 } else { 0.0 },
        ]
    };

    // The 48 symmetries of the cube: signed permutation matrices.
    let mut syms: Vec<[[f64; 3]; 3]> = Vec::new();
    let perms = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    for p in perms {
        for sx in [1.0, -1.0] {
            for sy in [1.0, -1.0] {
                for sz in [1.0, -1.0] {
                    let signs = [sx, sy, sz];
                    let mut m = [[0.0; 3]; 3];
                    for row in 0..3 {
                        m[row][p[row]] = signs[row];
                    }
                    syms.push(m);
                }
            }
        }
    }

    // M(u) = octant + 0.5·(P·(u − c) + c), c the cube centre.
    let apply = |p: &[[f64; 3]; 3], o: &[f64; 3], u: &[f64; 3]| -> [f64; 3] {
        let c = [0.5, 0.5, 0.5];
        let v = [u[0] - c[0], u[1] - c[1], u[2] - c[2]];
        [
            o[0] + 0.5 * (p[0][0] * v[0] + p[0][1] * v[1] + p[0][2] * v[2] + c[0]),
            o[1] + 0.5 * (p[1][0] * v[0] + p[1][1] * v[1] + p[1][2] * v[2] + c[1]),
            o[2] + 0.5 * (p[2][0] * v[0] + p[2][1] * v[1] + p[2][2] * v[2] + c[2]),
        ]
    };
    let close = |a: &[f64; 3], b: &[f64; 3]| {
        (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9 && (a[2] - b[2]).abs() < 1e-9
    };
    let in_octant = |o: &[f64; 3], q: &[f64; 3]| {
        (0..3).all(|k| q[k] >= o[k] - 1e-9 && q[k] <= o[k] + 0.5 + 1e-9)
    };

    let entry_g = [0.0, 0.0, 0.0];
    let exit_g = [1.0, 0.0, 0.0];

    // Depth-first search: octant by octant, first symmetry that chains.
    fn dfs(
        i: usize,
        entry: [f64; 3],
        gray: &[u8; 8],
        oct: &dyn Fn(u8) -> [f64; 3],
        syms: &Vec<[[f64; 3]; 3]>,
        apply: &dyn Fn(&[[f64; 3]; 3], &[f64; 3], &[f64; 3]) -> [f64; 3],
        close: &dyn Fn(&[f64; 3], &[f64; 3]) -> bool,
        in_octant: &dyn Fn(&[f64; 3], &[f64; 3]) -> bool,
        entry_g: &[f64; 3],
        exit_g: &[f64; 3],
        picked: &mut Vec<[[f64; 3]; 3]>,
    ) -> bool {
        if i == 8 {
            return true;
        }
        let o = oct(gray[i]);
        for p in syms {
            let e = apply(p, &o, entry_g);
            if !close(&e, &entry) {
                continue;
            }
            let x = apply(p, &o, exit_g);
            let ok = if i == 7 {
                close(&x, exit_g)
            } else {
                // The exit must be a corner shared with the NEXT octant.
                in_octant(&oct(gray[i + 1]), &x)
            };
            if !ok {
                continue;
            }
            picked.push(*p);
            if dfs(i + 1, x, gray, oct, syms, apply, close, in_octant, entry_g, exit_g, picked) {
                return true;
            }
            picked.pop();
        }
        false
    }

    let mut picked: Vec<[[f64; 3]; 3]> = Vec::new();
    let found = dfs(
        0, entry_g, &gray, &oct, &syms, &apply, &close, &in_octant, &entry_g, &exit_g,
        &mut picked,
    );
    debug_assert!(found, "a chaining symmetry assignment exists");
    if !found {
        return Vec::new();
    }

    picked
        .iter()
        .zip(gray.iter())
        .map(|(p, g)| {
            let o = oct(*g);
            // Affine: x' = 0.5·P·x + (o + 0.5·(c − P·c)).
            let c = [0.5, 0.5, 0.5];
            let pc = [
                p[0][0] * c[0] + p[0][1] * c[1] + p[0][2] * c[2],
                p[1][0] * c[0] + p[1][1] * c[1] + p[1][2] * c[2],
                p[2][0] * c[0] + p[2][1] * c[1] + p[2][2] * c[2],
            ];
            [
                0.5 * p[0][0], 0.5 * p[0][1], 0.5 * p[0][2],
                0.5 * p[1][0], 0.5 * p[1][1], 0.5 * p[1][2],
                0.5 * p[2][0], 0.5 * p[2][1], 0.5 * p[2][2],
                o[0] + 0.5 * (c[0] - pc[0]),
                o[1] + 0.5 * (c[1] - pc[1]),
                o[2] + 0.5 * (c[2] - pc[2]),
            ]
        })
        .collect()
}

/// Rescale and rotate a path so its overall displacement is the unit
/// segment from (0, 0) to (1, 0).
///
/// This is what turns a turtle path into an IFS. A depth-1 expansion
/// draws the pieces that one generation replaces a single edge with; once
/// the whole path spans the unit segment, each piece is a CONTRACTION of
/// it, and the attractor of those contractions is the L-system's limit
/// curve. That equivalence is why a Koch rule can become four transforms.
///
/// Returns `None` when the path starts and ends in the same place (a
/// closed figure such as a Sierpinski triangle's outline), where no such
/// normalisation exists.
pub fn normalize_segments(segs: &[Segment]) -> Option<Vec<Segment>> {
    let first = segs.first()?;
    let last = segs.last()?;
    let (dx, dy) = (last.x2 - first.x1, last.y2 - first.y1);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-12 {
        return None;
    }
    // Inverse of "rotate+scale by (dx, dy), then translate by start":
    // divide by the complex number dx + i*dy.
    let map = |px: f64, py: f64| -> (f64, f64) {
        let (ux, uy) = (px - first.x1, py - first.y1);
        ((ux * dx + uy * dy) / len2, (uy * dx - ux * dy) / len2)
    };
    Some(
        segs.iter()
            .map(|s| {
                let (x1, y1) = map(s.x1, s.y1);
                let (x2, y2) = map(s.x2, s.y2);
                Segment { x1, y1, x2, y2, depth: s.depth, symbol: s.symbol }
            })
            .collect(),
    )
}

// ============================================================================
// Kleinian groups (Indra's Pearls)
// ============================================================================

fn cscale(a: C, s: f64) -> C {
    [a[0] * s, a[1] * s]
}

fn csquare(a: C) -> C {
    cmul(a, a)
}

/// Guard a complex value away from zero the way the shader does, so a
/// degenerate parameter divides to the same place on both sides.
fn safe_c(a: C, eps: f64) -> C {
    if a[0].abs() + a[1].abs() < eps {
        [eps, a[1]]
    } else {
        a
    }
}

/// Solve the Markov trace identity for `tr(AB)`.
///
/// For a group generated by `a` and `b` whose commutator is parabolic,
/// the traces satisfy `x² + y² + z² = xyz` with `x = tr(a)`, `y = tr(b)`,
/// `z = tr(ab)`. Rearranged for `z` that is `z² - xy·z + (x² + y²) = 0`;
/// the shader (following the JWildfire source) takes the MINUS branch.
fn trace_ab(tr_a: C, tr_b: C) -> C {
    let bb = cscale(cmul(tr_a, tr_b), -1.0);
    let cc = cadd(csquare(tr_a), csquare(tr_b));
    let disc = csqrt(csub(csquare(bb), cscale(cc, 4.0)));
    safe_c(cscale(csub(cscale(bb, -1.0), disc), 0.5), 1e-15)
}

/// The generator pair `(a, b)` for one of `klein_group`'s recipes.
///
/// Recipes: 0 Grandma (parabolic commutator, *Indra's Pearls* ch. 6),
/// 1 Maskit μ, 2 Jørgensen, 3 Riley, 4 Riley+, 5 Maskit μ+,
/// 6 Maskit/Leys n-fold.
///
/// Ported from `init_klein_group`; keep the two in step.
pub fn klein_pair(recipe: u32, a_re: f64, a_im: f64, b_re: f64, b_im: f64) -> (Mobius, Mobius) {
    let one: C = [1.0, 0.0];
    let two: C = [2.0, 0.0];
    let zero: C = [0.0, 0.0];
    let i1: C = [0.0, 1.0];
    let i2: C = [0.0, 2.0];
    let i4: C = [0.0, 4.0];

    match recipe {
        3 | 4 => {
            // Riley: a is a lower shear by c, b an upper shear.
            let c: C = [a_re, a_im];
            let upper = if recipe == 4 { [b_re, b_im] } else { two };
            (
                Mobius { a: one, b: zero, c, d: one },
                Mobius { a: one, b: upper, c: zero, d: one },
            )
        }
        1 | 5 | 6 => {
            // Maskit: a = [[-mu*i, -i], [-i, 0]], b an upper shear.
            let mu: C = [a_re, a_im];
            let ma = Mobius {
                a: cmul(cscale(mu, -1.0), i1),
                b: cscale(i1, -1.0),
                c: cscale(i1, -1.0),
                d: zero,
            };
            let upper: C = match recipe {
                5 => [b_re, b_im],
                6 => {
                    // Leys' n-fold variant: 2cos(pi/n) opens the cusp to an
                    // n-fold symmetry instead of a parabolic.
                    let safe_br = if b_re.abs() < 1e-30 { 1e-30 } else { b_re };
                    [2.0 * (std::f64::consts::PI / safe_br).cos(), b_im]
                }
                _ => two,
            };
            (ma, Mobius { a: one, b: upper, c: zero, d: one })
        }
        2 => {
            // Jorgensen: entries straight from the trace identities.
            let tr_a: C = [a_re, a_im];
            let tr_b: C = [b_re, b_im];
            let tr_ab = trace_ab(tr_a, tr_b);
            (
                Mobius {
                    a: csub(tr_a, cdiv(tr_b, tr_ab)),
                    b: cdiv(tr_a, csquare(tr_ab)),
                    c: tr_a,
                    d: cdiv(tr_b, tr_ab),
                },
                Mobius {
                    a: csub(tr_b, cdiv(tr_a, tr_ab)),
                    b: cscale(cdiv(tr_b, csquare(tr_ab)), -1.0),
                    c: cscale(tr_b, -1.0),
                    d: cdiv(tr_a, tr_ab),
                },
            )
        }
        _ => {
            // Grandma's recipe: build the pair so their commutator is
            // parabolic, which is what makes the limit set a curve rather
            // than dust.
            let tr_a: C = [a_re, a_im];
            let tr_b: C = [b_re, b_im];
            let tr_ab = trace_ab(tr_a, tr_b);

            // The commutator's fixed point.
            let z0 = cdiv(
                cmul(csub(tr_ab, two), tr_b),
                cadd(
                    csub(cmul(tr_b, tr_ab), cscale(tr_a, 2.0)),
                    cmul(tr_ab, i2),
                ),
            );

            let ma = Mobius {
                a: cscale(tr_a, 0.5),
                b: cdiv(
                    cadd(csub(cmul(tr_a, tr_ab), cscale(tr_b, 2.0)), i4),
                    cmul(cadd(cscale(tr_ab, 2.0), [4.0, 0.0]), z0),
                ),
                c: cdiv(
                    cmul(csub(csub(cmul(tr_a, tr_ab), cscale(tr_b, 2.0)), i4), z0),
                    csub(cscale(tr_ab, 2.0), [4.0, 0.0]),
                ),
                d: cscale(tr_a, 0.5),
            };
            let mb = Mobius {
                a: cscale(csub(tr_b, i2), 0.5),
                b: cscale(tr_b, 0.5),
                c: cscale(tr_b, 0.5),
                d: cscale(cadd(tr_b, i2), 0.5),
            };
            (ma, mb)
        }
    }
}

/// The four Kleinian generators in `klein_group`'s own index order:
/// `[a, a⁻¹, b, b⁻¹]`.
///
/// Note this is NOT the Schottky/Apollonian order (`a, b, a⁻¹, b⁻¹`) —
/// the word rules key off these indices, so mixing them up produces a
/// plausible-looking but wrong walk.
///
/// `weight` reproduces the variation's own scaling quirk: it computes
/// `w · M(p / w)` and lets the framework's multiply supply the outer `w`.
/// That is conjugation by a scaling, folded into the matrix here so a
/// decomposed transform at weight 1 matches whatever weight the packed
/// variation carried.
pub fn klein_generators(
    recipe: u32,
    a_re: f64,
    a_im: f64,
    b_re: f64,
    b_im: f64,
    weight: f64,
) -> [Mobius; 4] {
    let (mut ma, mut mb) = klein_pair(recipe, a_re, a_im, b_re, b_im);
    if (weight - 1.0).abs() > 1e-9 && weight.abs() > 1e-9 {
        // z -> w*z is [[w,0],[0,1]]; z -> z/w is [[1,0],[0,w]].
        let out_scale = Mobius { a: [weight, 0.0], b: [0.0, 0.0], c: [0.0, 0.0], d: [1.0, 0.0] };
        let in_scale = Mobius { a: [1.0, 0.0], b: [0.0, 0.0], c: [0.0, 0.0], d: [weight, 0.0] };
        ma = out_scale.compose(ma).compose(in_scale);
        mb = out_scale.compose(mb).compose(in_scale);
    }
    // The variation inverts with the SL(2,C) shortcut [d, -b; -c, a].
    [ma, ma.inverse(), mb, mb.inverse()]
}

/// Xaos row that simply FORBIDS one index and keeps the rest equal.
///
/// klein_group draws uniformly from the three allowed generators, unlike
/// the packings and Schottky groups, which redraw into the next one and
/// so double its share. Three variations on "avoid backtracking", three
/// different distributions.
pub fn exclude_xaos_row(forbidden: usize, count: usize) -> Vec<f32> {
    let mut row = vec![1.0f32; count];
    if forbidden < count {
        row[forbidden] = 0.0;
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
    fn klein_generators_are_unimodular_and_paired() {
        // Every recipe must produce det-1 matrices — the variation inverts
        // with the SL(2,C) shortcut [d, -b; -c, a], which is only the
        // inverse when det is 1.
        for recipe in 0..7u32 {
            let g = klein_generators(recipe, 2.0, 0.0, 2.0, 0.0, 1.0);
            for (i, m) in g.iter().enumerate() {
                let det = m.det();
                approx(det[0], 1.0, 1e-6, &format!("recipe {recipe} gen {i} det real"));
                approx(det[1], 0.0, 1e-6, &format!("recipe {recipe} gen {i} det imag"));
            }
            // Index order is [a, a-inverse, b, b-inverse]: 1 undoes 0, 3 undoes 2.
            for (m, inv) in [(g[0], g[1]), (g[2], g[3])] {
                let z = [0.23, -0.41];
                let w = inv.apply(m.apply(z));
                approx(w[0], z[0], 1e-6, &format!("recipe {recipe}: inverse undoes (x)"));
                approx(w[1], z[1], 1e-6, &format!("recipe {recipe}: inverse undoes (y)"));
            }
        }
    }

    #[test]
    fn grandma_makes_the_commutator_parabolic() {
        // The whole point of Grandma's recipe: tr(abABsomething) = -2, the
        // signature of a parabolic commutator. That is what makes the limit
        // set a connected curve instead of dust.
        for (ar, ai, br, bi) in [(2.0, 0.0, 2.0, 0.0), (1.91, 0.05, 2.0, 0.0), (2.0, 0.2, 1.87, -0.1)] {
            let (a, b) = klein_pair(0, ar, ai, br, bi);
            let comm = a.compose(b).compose(a.inverse()).compose(b.inverse());
            let trace = cadd(comm.a, comm.d);
            approx(trace[0], -2.0, 1e-4, "commutator trace is -2 (parabolic)");
            approx(trace[1], 0.0, 1e-4, "commutator trace is real");
        }
    }

    #[test]
    fn klein_traces_match_the_requested_ones() {
        // Grandma builds a pair with GIVEN traces; check it actually did.
        for (ar, ai, br, bi) in [(2.0, 0.0, 2.0, 0.0), (1.87, 0.1, 2.2, -0.3)] {
            let (a, b) = klein_pair(0, ar, ai, br, bi);
            let ta = cadd(a.a, a.d);
            let tb = cadd(b.a, b.d);
            approx(ta[0], ar, 1e-6, "tr(a) real");
            approx(ta[1], ai, 1e-6, "tr(a) imag");
            approx(tb[0], br, 1e-6, "tr(b) real");
            approx(tb[1], bi, 1e-6, "tr(b) imag");
        }
    }

    #[test]
    fn klein_weight_folds_in_exactly() {
        // The variation computes w * M(p/w). Folding that into the matrix
        // must land on the same point as doing it the long way.
        let w = 1.7;
        let plain = klein_generators(0, 2.0, 0.0, 2.0, 0.0, 1.0);
        let scaled = klein_generators(0, 2.0, 0.0, 2.0, 0.0, w);
        let z = [0.4, -0.25];
        for k in 0..4 {
            let long_way = plain[k].apply([z[0] / w, z[1] / w]);
            let got = scaled[k].apply(z);
            approx(got[0], long_way[0] * w, 1e-9, "folded weight (x)");
            approx(got[1], long_way[1] * w, 1e-9, "folded weight (y)");
        }
    }

    #[test]
    fn exclude_rows_forbid_only_the_inverse() {
        // klein_group draws uniformly from the three allowed generators —
        // no doubling, unlike the packings and Schottky groups.
        for from in 0..4usize {
            let forbidden = from ^ 1;
            let row = exclude_xaos_row(forbidden, 4);
            assert_eq!(row[forbidden], 0.0, "from {from}: inverse blocked");
            let allowed: Vec<f32> = row.iter().copied().filter(|v| *v > 0.0).collect();
            assert_eq!(allowed.len(), 3, "three generators remain");
            assert!(allowed.iter().all(|v| *v == 1.0), "and they are equally likely");
        }
    }


    #[test]
    fn lsystem_rewrites_in_parallel() {
        // Every symbol is replaced at once; symbols without a rule stand
        // for themselves.
        let rules = vec![('F', "F+F".to_string())];
        assert_eq!(lsystem_expand("F", &rules, 0).unwrap(), "F");
        assert_eq!(lsystem_expand("F", &rules, 1).unwrap(), "F+F");
        assert_eq!(lsystem_expand("F", &rules, 2).unwrap(), "F+F+F+F");
        // The + has no rule, so it survives untouched.
        assert_eq!(lsystem_expand("+F+", &rules, 1).unwrap(), "+F+F+");

        // Runaway growth fails loudly rather than exhausting memory.
        let boom = vec![('F', "FFFF".to_string())];
        let err = lsystem_expand("F", &boom, 40).unwrap_err();
        assert!(err.contains("grew past"), "{err}");
    }

    #[test]
    fn turtle_draws_and_branches() {
        // A straight run of three unit steps.
        let segs = turtle("FFF", 90.0);
        assert_eq!(segs.len(), 3);
        approx(segs[2].x2, 3.0, 1e-9, "three steps east");
        approx(segs[2].y2, 0.0, 1e-9, "no drift");

        // A right angle.
        let segs = turtle("F+F", 90.0);
        approx(segs[1].x2, 1.0, 1e-9, "turned north (x)");
        approx(segs[1].y2, 1.0, 1e-9, "turned north (y)");

        // Lower-case f moves without drawing.
        assert_eq!(turtle("FfF", 90.0).len(), 2, "f draws nothing");

        // Brackets save and restore position, heading AND depth.
        let segs = turtle("F[+F]F", 90.0);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].depth, 0, "trunk");
        assert_eq!(segs[1].depth, 1, "inside the bracket");
        assert_eq!(segs[2].depth, 0, "back on the trunk");
        // The third segment continues from where the first ended, not
        // from the end of the branch.
        approx(segs[2].x1, 1.0, 1e-9, "resumed at the branch point (x)");
        approx(segs[2].y1, 0.0, 1e-9, "resumed at the branch point (y)");
    }

    #[test]
    fn normalized_segments_are_contractions() {
        // The Koch rule: one edge becomes four, and after normalising the
        // whole path onto the unit segment each piece must be a genuine
        // contraction — that is what makes the IFS converge to the curve.
        let rules = vec![('F', "F+F--F+F".to_string())];
        let expanded = lsystem_expand("F", &rules, 1).unwrap();
        let segs = normalize_segments(&turtle(&expanded, 60.0)).expect("open path");
        assert_eq!(segs.len(), 4, "Koch replaces one edge with four");

        for (i, s) in segs.iter().enumerate() {
            let len = ((s.x2 - s.x1).powi(2) + (s.y2 - s.y1).powi(2)).sqrt();
            approx(len, 1.0 / 3.0, 1e-9, &format!("Koch piece {i} is one third"));
            assert!(len < 1.0, "piece {i} must contract");
        }
        // The path spans exactly the unit segment.
        approx(segs[0].x1, 0.0, 1e-9, "starts at the origin");
        approx(segs[0].y1, 0.0, 1e-9, "starts at the origin");
        approx(segs[3].x2, 1.0, 1e-9, "ends at (1, 0)");
        approx(segs[3].y2, 0.0, 1e-9, "ends at (1, 0)");
    }

    #[test]
    fn segments_remember_their_symbol() {
        // Mirror-pair curves need this: the endpoints alone cannot say
        // whether a piece is the reflected variant.
        let segs = turtle("F+G", 60.0);
        assert_eq!(segs[0].symbol, 'F');
        assert_eq!(segs[1].symbol, 'G');
        // And it survives normalisation.
        let n = normalize_segments(&segs).unwrap();
        assert_eq!(n[0].symbol, 'F');
        assert_eq!(n[1].symbol, 'G');
    }

    #[test]
    fn mirror_pairs_are_detected() {
        // Sierpinski arrowhead: each rule is the other with every turn
        // reversed and the symbols swapped.
        let rules = vec![
            ('F', "+G-F-G+".to_string()),
            ('G', "-F+G+F-".to_string()),
        ];
        assert_eq!(mirror_partner(&rules, 'F'), Some('G'));
        assert_eq!(mirror_partner(&rules, 'G'), Some('F'));

        // Koch has one rule and no partner.
        let koch = vec![('F', "F+F--F+F".to_string())];
        assert_eq!(mirror_partner(&koch, 'F'), None);

        // Two rules that are NOT mirrors of each other (the dragon).
        let dragon = vec![('F', "F+G".to_string()), ('G', "F-G".to_string())];
        assert_eq!(mirror_partner(&dragon, 'F'), None, "dragon is not a mirror pair");
    }

    #[test]
    fn bounds_step_down_when_the_expansion_is_huge() {
        // Hilbert quadruples its non-terminals each generation, so a deep
        // expansion is enormous. Rather than failing, the bounds walk
        // steps down to a depth that fits the budget.
        let hilbert = vec![
            ('G', "+DFFF-GFFFG-FFFD+".to_string()),
            ('D', "-GFFF+DFFFD+FFFG-".to_string()),
        ];
        let (_, _, _, _, used) = lsystem_bounds("G", &hilbert, 8, 90.0, 2_000).expect("bounded");
        assert!(used < 8, "should have stepped down from 8, used {used}");

        // A small system uses the depth it was asked for.
        let koch = vec![('F', "F+F--F+F".to_string())];
        let (_, _, _, _, used) = lsystem_bounds("F", &koch, 4, 60.0, 400_000).unwrap();
        assert_eq!(used, 4, "no need to step down");
    }


    #[test]
    fn hilbert_yields_four_half_scale_maps() {
        // The known IFS of the Hilbert curve: four maps at scale 1/2, the
        // first and last mirrored (they are Y occurrences, and Y is X's
        // mirror partner). The spans are measured on a finite expansion,
        // so the tolerance is loose-ish — but 1/2 is unambiguous.
        let rules = vec![
            ('X', "-YF+XFX+FY-".to_string()),
            ('Y', "+XF-YFY-FX+".to_string()),
        ];
        let segs = lsystem_node_segments("X", &rules, 90.0).expect("hilbert extracts");
        assert_eq!(segs.len(), 4, "four variable occurrences");

        let symbols: Vec<char> = segs.iter().map(|s| s.symbol).collect();
        assert_eq!(symbols, vec!['Y', 'X', 'X', 'Y'], "occurrence order preserved");

        for (i, s) in segs.iter().enumerate() {
            let len = ((s.x2 - s.x1).powi(2) + (s.y2 - s.y1).powi(2)).sqrt();
            // Snapped to the rational grid: EXACTLY one half.
            assert!(
                (len - 0.5).abs() < 1e-12,
                "map {i} scale {len} should be exactly one half"
            );
        }

        // Continuity is the visible property: each cell's exit must BE the
        // next cell's entry (the connective F steps vanish in the limit).
        // Without exact maps the finite-depth path shows segments that do
        // not quite meet at cell boundaries.
        for w in segs.windows(2) {
            assert!(
                (w[0].x2 - w[1].x1).abs() < 1e-12 && (w[0].y2 - w[1].y1).abs() < 1e-12,
                "cell exit must equal next cell entry"
            );
        }
        // And the whole chain runs from the curve's start to its end.
        assert!((segs[0].x1).abs() < 1e-12 && (segs[0].y1).abs() < 1e-12);
        assert!((segs[3].x2 - 1.0).abs() < 1e-12 && (segs[3].y2).abs() < 1e-12);
    }

    #[test]
    fn peano_yields_nine_third_scale_maps() {
        let rules = vec![
            ('X', "XFYFX+F+YFXFY-F-XFYFX".to_string()),
            ('Y', "YFXFY-F-XFYFX+F+YFXFY".to_string()),
        ];
        let segs = lsystem_node_segments("X", &rules, 90.0).expect("peano extracts");
        assert_eq!(segs.len(), 9, "nine variable occurrences");
        for (i, s) in segs.iter().enumerate() {
            let len = ((s.x2 - s.x1).powi(2) + (s.y2 - s.y1).powi(2)).sqrt();
            assert!(
                (len - 1.0 / 3.0).abs() < 1e-3,
                "map {i} scale {len} should be one third"
            );
        }
    }

    #[test]
    fn node_extraction_reports_what_it_cannot_do() {
        // A drawing axiom is an edge system, not a node one.
        let koch = vec![('F', "F+F--F+F".to_string())];
        let err = lsystem_node_segments("F", &koch, 60.0).unwrap_err();
        assert!(err.contains("edge-rewriting"), "{err}");

        // A third, non-mirror variable is a graph-directed IFS.
        let tri = vec![
            ('X', "YFZF".to_string()),
            ('Y', "XFX".to_string()),
            ('Z', "XFY".to_string()),
        ];
        let err = lsystem_node_segments("X", &tri, 90.0).unwrap_err();
        assert!(err.contains("graph-directed"), "{err}");

        // No F anywhere: the walk never moves.
        let still = vec![('X', "X+X".to_string())];
        let err = lsystem_node_segments("X", &still, 90.0).unwrap_err();
        assert!(err.contains("returns to where it started"), "{err}");
    }

    #[test]
    fn closed_figures_cannot_be_normalized() {
        // A path returning to its start has no overall displacement to
        // normalise by; say so rather than dividing by zero.
        let segs = turtle("F+F+F+F", 90.0);
        assert!(normalize_segments(&segs).is_none(), "a closed square has no IFS form");
    }


    #[test]
    fn wikipedia_plant_extracts_branches_and_stems() {
        // X=F-[[X]+X]+F[+FX]-X with F=FF: four X recursion sites, three
        // literal F stems, bracket depths recorded for colouring.
        let rules = vec![
            ('X', "F-[[X]+X]+F[+FX]-X".to_string()),
            ('F', "FF".to_string()),
        ];
        let p = lsystem_plant_segments("X", &rules, 22.5).expect("plant extracts");
        assert_eq!(p.branches.len(), 4, "four X occurrences");
        assert_eq!(p.stems.len(), 3, "three literal F stems");

        // Bracket depths: [[X]+X] puts the first X two deep, the second
        // one deep; [+FX] one deep; the last X on the trunk.
        let depths: Vec<u32> = p.branches.iter().map(|b| b.depth).collect();
        assert_eq!(depths, vec![2, 1, 1, 0], "bracket nesting per site");

        // Every branch map must contract, or the plant diverges.
        for (i, b) in p.branches.iter().enumerate() {
            let len = ((b.x2 - b.x1).powi(2) + (b.y2 - b.y1).powi(2)).sqrt();
            assert!(len > 1e-3 && len < 0.999, "branch {i} scale {len}");
        }
    }

    #[test]
    fn drawing_recursive_bush_has_no_separate_stems() {
        // ABOP's bush: F both draws and recurses, so every occurrence is
        // a branch map and the copies cover the stems themselves.
        let rules = vec![('F', "FF-[-F+F+F]+[+F-F-F]".to_string())];
        let p = lsystem_plant_segments("F", &rules, 22.5).expect("bush extracts");
        assert_eq!(p.branches.len(), 8, "eight F occurrences");
        assert_eq!(p.stems.len(), 0, "no separate stems");
        for (i, b) in p.branches.iter().enumerate() {
            let len = ((b.x2 - b.x1).powi(2) + (b.y2 - b.y1).powi(2)).sqrt();
            assert!(len < 0.999, "branch {i} must contract, got {len}");
        }
    }

    #[test]
    fn plant_extraction_reports_what_it_cannot_do() {
        let tri = vec![
            ('X', "F[+Y]FZ".to_string()),
            ('Y', "F[+X]F".to_string()),
            ('Z', "FF".to_string()),
        ];
        // Z is ruled and drawing? No — Z is not a drawing symbol, and is
        // neither X nor its mirror: graph-directed.
        let err = lsystem_plant_segments("X", &tri, 25.0).unwrap_err();
        assert!(err.contains("graph-directed"), "{err}");

        let none = vec![('X', "FF+FF".to_string())];
        let err = lsystem_plant_segments("X", &none, 25.0).unwrap_err();
        assert!(err.contains("no recursion sites"), "{err}");
    }


    fn col(m: &[f64; 12], j: usize) -> [f64; 3] {
        [m[j], m[3 + j], m[6 + j]]
    }

    fn norm3(v: &[f64; 3]) -> f64 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    #[test]
    fn turtle3_matches_the_2d_turtle_in_the_plane() {
        // A 2D rule through the 3D extractor: same piece scales as the 2D
        // plant extraction, everything in the z = 0 plane.
        let rules = vec![
            ('X', "F-[[X]+X]+F[+FX]-X".to_string()),
            ('F', "FF".to_string()),
        ];
        let p2 = lsystem_plant_segments("X", &rules, 22.5).unwrap();
        let p3 = lsystem_pieces3("X", &rules, 22.5).unwrap();
        assert_eq!(p3.branches.len(), p2.branches.len());
        assert_eq!(p3.stems.len(), p2.stems.len());

        let mut s2: Vec<f64> = p2
            .branches
            .iter()
            .map(|b| ((b.x2 - b.x1).powi(2) + (b.y2 - b.y1).powi(2)).sqrt())
            .collect();
        let mut s3: Vec<f64> = p3.branches.iter().map(|b| norm3(&col(&b.m, 0))).collect();
        s2.sort_by(|a, b| a.partial_cmp(b).unwrap());
        s3.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for (a, b) in s2.iter().zip(s3.iter()) {
            // 1e-4, not 1e-6: the 2D turtle accumulates an angle while the
            // 3D one accumulates matrix products, and over a ~200k-step
            // deep walk the f64 rounding drifts by ~1e-5. Same maps.
            assert!((a - b).abs() < 1e-4, "scales agree across extractors: {a} vs {b}");
        }
        for b in &p3.branches {
            assert!(b.m[11].abs() < 1e-9, "2D rules stay in the z = 0 plane");
            // Planar frame: the zz entry is the whole scale.
            assert!((b.m[8] - norm3(&col(&b.m, 0))).abs() < 1e-6);
        }
    }

    #[test]
    fn rolled_branches_leave_the_plane() {
        // Roll then yaw tips subsequent growth out of the xy plane — the
        // whole point of the 3D commands.
        let rules = vec![
            ('X', "F[\\+X][/-X]FX".to_string()),
            ('F', "FF".to_string()),
        ];
        let p = lsystem_pieces3("X", &rules, 25.0).unwrap();
        assert_eq!(p.branches.len(), 3, "three recursion sites");
        assert_eq!(p.stems.len(), 2, "two stems");

        // The two bracketed branches were rolled before yawing, so their
        // frames must have a genuine z component; the trailing branch
        // stays in-plane.
        let out_of_plane = |m: &[f64; 12]| col(m, 0)[2].abs() + col(m, 1)[2].abs() > 1e-3;
        assert!(out_of_plane(&p.branches[0].m), "rolled branch 0 leaves the plane");
        assert!(out_of_plane(&p.branches[1].m), "rolled branch 1 leaves the plane");
        assert!(!out_of_plane(&p.branches[2].m), "unrolled branch stays planar");

        // Every branch map is a genuine similarity: orthogonal columns of
        // equal length, and that length is the contraction.
        for (i, b) in p.branches.iter().enumerate() {
            let c0 = col(&b.m, 0);
            let c1 = col(&b.m, 1);
            let c2 = col(&b.m, 2);
            let (n0, n1, n2) = (norm3(&c0), norm3(&c1), norm3(&c2));
            assert!((n0 - n1).abs() < 1e-6 && (n1 - n2).abs() < 1e-6, "branch {i} isotropic");
            assert!(n0 > 1e-3 && n0 < 0.999, "branch {i} contracts: {n0}");
            let d01 = c0[0] * c1[0] + c0[1] * c1[1] + c0[2] * c1[2];
            assert!(d01.abs() < 1e-6, "branch {i} columns orthogonal");
        }
    }

    #[test]
    fn edge_pieces3_chain_in_the_unit_frame() {
        // The frame convention the path variation relies on: the system
        // runs from the origin to (1,0,0), and for an edge curve each
        // piece's exit — its map applied to x̂ — is the next piece's
        // entry. This was broken before the global reframe: exits
        // anchored at x̂ pointed the wrong way and the path shattered.
        let rules = vec![('F', r"F+F\--F+F".to_string())];
        let p = lsystem_pieces3("F", &rules, 60.0).unwrap();
        assert_eq!(p.branches.len(), 4);

        let apply = |m: &[f64; 12], v: [f64; 3]| {
            [
                m[0] * v[0] + m[1] * v[1] + m[2] * v[2] + m[9],
                m[3] * v[0] + m[4] * v[1] + m[5] * v[2] + m[10],
                m[6] * v[0] + m[7] * v[1] + m[8] * v[2] + m[11],
            ]
        };
        for w in p.branches.windows(2) {
            let exit = apply(&w[0].m, [1.0, 0.0, 0.0]);
            let entry = [w[1].m[9], w[1].m[10], w[1].m[11]];
            for k in 0..3 {
                assert!(
                    (exit[k] - entry[k]).abs() < 1e-3,
                    "pieces must chain: {exit:?} vs {entry:?}"
                );
            }
        }
        // And the whole chain runs from the origin to (1, 0, 0).
        let first = &p.branches[0].m;
        assert!(first[9].abs() < 1e-6 && first[10].abs() < 1e-6 && first[11].abs() < 1e-6);
        let end = apply(&p.branches[3].m, [1.0, 0.0, 0.0]);
        assert!((end[0] - 1.0).abs() < 1e-3 && end[1].abs() < 1e-3 && end[2].abs() < 1e-3,
            "chain ends at (1,0,0): {end:?}");
    }

    #[test]
    fn lsystem_3d_detection_looks_for_the_3d_commands() {
        let flat = vec![('F', "F+F--F+F".to_string())];
        assert!(!lsystem_uses_3d("F", &flat));
        let rolled = vec![('X', "F[\\+X]FX".to_string())];
        assert!(lsystem_uses_3d("X", &rolled));
        let pitched = vec![('X', "F[&X]F".to_string())];
        assert!(lsystem_uses_3d("X", &pitched));
    }


    #[test]
    fn hilbert3d_maps_tile_and_chain() {
        let maps = hilbert3d_maps();
        assert_eq!(maps.len(), 8, "eight octants");

        let apply = |m: &[f64; 12], v: [f64; 3]| {
            [
                m[0] * v[0] + m[1] * v[1] + m[2] * v[2] + m[9],
                m[3] * v[0] + m[4] * v[1] + m[5] * v[2] + m[10],
                m[6] * v[0] + m[7] * v[1] + m[8] * v[2] + m[11],
            ]
        };

        // Each map is half a cube symmetry: columns orthogonal, length 1/2.
        for (i, m) in maps.iter().enumerate() {
            for j in 0..3 {
                let cl = [m[j], m[3 + j], m[6 + j]];
                let n = (cl[0] * cl[0] + cl[1] * cl[1] + cl[2] * cl[2]).sqrt();
                approx(n, 0.5, 1e-12, &format!("map {i} column {j} scale"));
            }
        }

        // The eight images of the cube's centre are the eight octant
        // centres — the maps tile the cube, nothing doubled.
        let mut centres: Vec<[i32; 3]> = maps
            .iter()
            .map(|m| {
                let c = apply(m, [0.5, 0.5, 0.5]);
                [(c[0] * 4.0).round() as i32, (c[1] * 4.0).round() as i32, (c[2] * 4.0).round() as i32]
            })
            .collect();
        centres.sort();
        centres.dedup();
        assert_eq!(centres.len(), 8, "eight distinct octants");

        // Continuity: the curve enters at (0,0,0), each octant's exit is
        // the next octant's entry, and the whole exits at (1,0,0). This
        // chaining IS what makes the limit a connected curve.
        let entry0 = apply(&maps[0], [0.0, 0.0, 0.0]);
        assert!(entry0.iter().all(|v| v.abs() < 1e-12), "starts at the origin");
        for i in 0..7 {
            let exit_i = apply(&maps[i], [1.0, 0.0, 0.0]);
            let entry_next = apply(&maps[i + 1], [0.0, 0.0, 0.0]);
            for k in 0..3 {
                approx(exit_i[k], entry_next[k], 1e-12, &format!("octant {i} chains"));
            }
        }
        let end = apply(&maps[7], [1.0, 0.0, 0.0]);
        approx(end[0], 1.0, 1e-12, "exits at (1,0,0)");
        approx(end[1], 0.0, 1e-12, "exits at (1,0,0)");
        approx(end[2], 0.0, 1e-12, "exits at (1,0,0)");

        // Deterministic: the search must return the same maps every call,
        // or saved flames would silently change.
        assert_eq!(maps, hilbert3d_maps());
    }

    #[test]
    fn graph_pieces_carry_types_and_gate_correctly() {
        // The 2D Hilbert pair, treated as a two-type GRAPH instead of
        // via mirror folding: X's rule holds occurrences [Y, X, X, Y] and
        // Y's holds [X, Y, Y, X]. Every map must know what it consumes
        // and what it produces — that is what xaos gates on.
        let rules = vec![
            ('X', "-YF+XFX+FY-".to_string()),
            ('Y', "+XF-YFY-FX+".to_string()),
        ];
        let pieces = lsystem_graph_pieces("X", &rules, 90.0).expect("graph extracts");
        assert_eq!(pieces.len(), 8, "four occurrences in each of two rules");

        let owners: Vec<char> = pieces.iter().map(|p| p.owner).collect();
        assert_eq!(owners, vec!['X', 'X', 'X', 'X', 'Y', 'Y', 'Y', 'Y']);
        let occs_x: Vec<char> = pieces.iter().filter(|p| p.owner == 'X').map(|p| p.occ).collect();
        assert_eq!(occs_x, vec!['Y', 'X', 'X', 'Y']);

        // Every map contracts by about one half (it is Hilbert).
        for (i, p) in pieces.iter().enumerate() {
            let c0 = [p.m[0], p.m[3], p.m[6]];
            let n = (c0[0] * c0[0] + c0[1] * c0[1] + c0[2] * c0[2]).sqrt();
            assert!((n - 0.5).abs() < 0.01, "piece {i} scale {n}");
            assert!(p.m[11].abs() < 1e-9, "2D system stays in the plane");
        }

        // A three-type system that mirror folding REFUSES must extract
        // here — that is the point of graph support.
        let tri = vec![
            ('X', "F+YFZF".to_string()),
            ('Y', "FX-F".to_string()),
            ('Z', "F-XF".to_string()),
        ];
        let p = lsystem_graph_pieces("X", &tri, 60.0).expect("three types extract");
        let mut kinds: Vec<char> = p.iter().map(|q| q.owner).collect();
        kinds.dedup();
        assert_eq!(kinds, vec!['X', 'Y', 'Z'], "all three types present");
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
