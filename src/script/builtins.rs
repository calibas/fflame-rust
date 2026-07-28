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

    let mut st = LTurtle { x: 0.0, y: 0.0, h: 0.0 };
    let mut stack: Vec<LTurtle> = Vec::new();
    let mut chunks: Vec<(f64, f64, f64, f64, char)> = Vec::new();
    for ch in rule.chars() {
        if is_var(ch) {
            let (sx, sy) = (st.x, st.y);
            let body = if ch == primary { &exp_primary } else { &exp_partner };
            for bc in body.chars() {
                lsys_step(bc, angle, &mut st, &mut stack);
            }
            chunks.push((sx, sy, st.x, st.y, ch));
        } else {
            lsys_step(ch, angle, &mut st, &mut stack);
        }
    }

    // Normalize into the unit-displacement frame (walk started at the origin).
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
            Segment { x1: ax, y1: ay, x2: bx, y2: by, depth: 0, symbol: *sym }
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
            assert!(
                (len - 0.5).abs() < 0.02,
                "map {i} scale {len} should be about one half"
            );
        }
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
                (len - 1.0 / 3.0).abs() < 0.02,
                "map {i} scale {len} should be about one third"
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
