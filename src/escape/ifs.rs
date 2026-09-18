//! Mode D — the flame as a distance field
//! ([docs/projects/ifs-distance-rendering.md](../../docs/projects/ifs-distance-rendering.md),
//! phase 1).
//!
//! A fourth registry pair on the escape engine's one pattern (D1).
//! Where a mode-A formula iterates a function of the pixel and a
//! mode-B field sums a series, a mode-D **distance function** answers
//! "how far is this pixel from the set" — and the set, for the one
//! entry that ships in phase 1, is the attractor of the loaded flame,
//! analysed into affine maps by
//! [`crate::scene::ifs_analysis`] and walked by the inverse iteration
//! [`crate::scene::ifs_estimate`] gates on the CPU.
//!
//! Later entries need no flame at all: the Mandelbulb, Mandelbox and
//! KIFS distance estimators are `IfsDef`s whose WGSL never touches the
//! map buffer (plan §5, phase 4). That is why this is a registry and
//! not a special case.
//!
//! # WGSL contract (template `IFS_TEMPLATE` in assembler.rs)
//!
//! An `IfsDef` defines
//! `fn ifs_evaluate(uv: vec2<f32>) -> IfsResult`, where `uv` is the
//! pixel's normalised offset — the screen spanning [-½, ½] on each
//! axis.
//!
//! Not a position, because at a deep zoom there is no position an f32
//! could hold. The walk starts from the beam state the CPU handed over
//! (`scene::ifs_estimate::seed_beam`), and each seed's `basis` carries
//! `uv` straight to that candidate's delta. §2.5's reference/delta
//! split is real and exact — `S⁻¹(C + δ) = S⁻¹(C) + M⁻¹δ`, no cross
//! term — and it lives entirely on the CPU, which is the point of
//! doing it there: the handover happens where the delta has grown to a
//! quarter of the ball's radius, and at that size the shader can carry
//! one combined point with nothing lost.
//!
//! An `IfsColoringDef` defines
//! `fn ifs_color(res: IfsResult) -> IfsShade`. `IfsShade { t, lum }` is
//! the field templates' convention: `t` is the palette position
//! (wrapped), `lum` multiplies the sampled colour.
//!
//! `res.distance` is **in pixels**, not plane units. In plane units it
//! underflows f32 at a zoom the walk could otherwise still resolve,
//! and every colouring wanted it divided by the pixel width anyway —
//! so the edge width and the halo reach are both in pixels, and the
//! contour bands come out zoom-invariant.
//!
//! Params reach both through `fparam(i)` / `cparam(i)`, exactly as in
//! modes A and B.

use super::EscapeParamDef;
use crate::scene::ifs_analysis::{Affine2, Affine3, Ifs2, Ifs3, Map2, Map3};

/// A mode-D distance function.
pub struct IfsDef {
    /// Registry name — what `EscapeConfig::formula` stores.
    pub name: &'static str,
    pub display_name: &'static str,
    /// Whether this entry marches a RAY through three dimensions
    /// rather than evaluating a plane.
    ///
    /// It selects the template, because the two differ in everything
    /// around the walk — a camera, a march, a normal and a shade —
    /// while sharing the walk's shape and, notably, all four
    /// colourings: a colouring maps one of the four quantities to a
    /// palette position, and that is the same question in either
    /// dimension.
    pub solid: bool,
    /// Whether this entry reads the flame's map buffer. `true` means
    /// the render is gated on the loaded flame passing the criterion
    /// (§2.4) and the panel says so; `false` is a self-contained
    /// distance estimator that renders whatever flame is loaded.
    pub needs_flame: bool,
    /// Coloring used when `EscapeConfig::coloring` names another
    /// registry's entry (the usual state right after a switch).
    pub default_coloring: &'static str,
    pub parameters: &'static [EscapeParamDef],
    pub presets: &'static [super::EscapePreset],
    pub wgsl: &'static str,
}

/// A mode-D coloring: the four quantities of §2.3 → palette position
/// and luminance.
pub struct IfsColoringDef {
    pub name: &'static str,
    pub display_name: &'static str,
    pub parameters: &'static [EscapeParamDef],
    pub wgsl: &'static str,
}

// ====================================================================
// The distance function
// ====================================================================

/// The loaded flame's attractor, by inverse iteration.
///
/// Transcribed from [`crate::scene::ifs_estimate::estimate`], which is
/// the reference and carries the gates. The one structural difference
/// is the reference/delta split described in the module docs.
pub static IFS_FLAME: IfsDef = IfsDef {
    name: "ifs_flame",
    display_name: "Flame Attractor (Distance)",
    solid: false,
    needs_flame: true,
    default_coloring: "ifs_distance",
    presets: &[],
    parameters: &[
        EscapeParamDef {
            name: "levels",
            display_name: "Inverse Depth",
            default: 24.0,
            min: 1.0,
            max: 256.0,
            tooltip: "How many inverse maps to apply before giving up and calling the \
                      pixel part of the set. Deeper resolves finer structure and is \
                      never less sound; the depth a zoom needs grows like \
                      log(1/zoom)/log(1/sigma).",
            choices: &[],
        },
        EscapeParamDef {
            name: "beam",
            display_name: "Beam Width",
            default: 4.0,
            min: 1.0,
            max: 8.0,
            tooltip: "How many branch addresses the walk follows at once. Every \
                      address bounds the distance to ITS piece and the truth is the \
                      smallest, so following one can only read too far -- which \
                      erodes an attractor whose pieces share a boundary. 1 is the \
                      greedy walk. Measured at 384 on five sets: 4 matches 8 on \
                      three of them exactly and differs by 16 pixels on the \
                      Heighway dragon, whose two pieces share a boundary, while 8 \
                      costs half again as much -- nearly all of the beam's price \
                      is the last doubling. 2 loses thousands of pixels on the \
                      dragon and 1 loses hundreds even on a Sierpinski, where the \
                      pieces touch at points. Raise it to 8 for a still of an \
                      overlapping set; lower it to 1 when the picture does not \
                      change.",
            choices: &[],
        },
        EscapeParamDef {
            name: "extent",
            display_name: "Extent",
            default: 0.0,
            min: 0.0,
            max: 100.0,
            tooltip: "The radius of the ball the set is cut at, about its measured \
                      centre; 0 measures it from the flame. For an affine set the \
                      measurement is exact and this changes nothing worth having. \
                      For a set with inversions (a julia of negative distance, \
                      spherical) the set is unbounded, the ball is a cut through \
                      it at the bulk of a sample, and every far-field reading -- \
                      the smooth gradient beside a piece, the edge of every hole \
                      -- is scaled by the radius. The sample drifts as the flame \
                      animates, and the exterior slides with it: 16 pixels for a \
                      six-degree turn of one transform, measured, while the set \
                      there moved 94. The criterion above shows the measured \
                      value; set it here to hold it through an animation.",
            choices: &[],
        },
    ],
    wgsl: r#"
// The kernel's inverse on v, along the row's branch (plan 8.8 J1,
// 8.9 S2/S4): a root is |v|^(|n|/d) at angle n*arg(v); spherical is
// v/|v|^2; bubble's inner (branch 0) or outer preimage is v scaled by
// ifs_bubble_scale, and a v outside the unit disc has none and lands
// at infinity, which the walk reads as that piece having escaped.
// Bubble's radial scale, u = v * s, along the row's branch -- written
// so nothing cancels. The inner branch's f = 2 - 2*sqrt(1 - x) is a
// difference of two numbers either side of 2 and keeps only the digits
// x is below 1: three of f32's seven at |v| = 1e-4. With
// x = (1 - root)(1 + root) the root divides out (see
// Kernel::bubble_scale).
fn ifs_bubble_scale(r2: f32, branch: f32) -> f32 {
    let root = sqrt(max(1.0 - r2, 0.0));
    let up = 1.0 + root;
    return select(2.0 * up / max(r2, 1e-30), 2.0 / up, branch == 0.0);
}

fn ifs_kernel_inverse(i: u32, v: vec2<f32>) -> vec2<f32> {
    let kind = ifs_maps[i].kind;
    let r2 = dot(v, v);
    if (kind == 1.0) {
        let n = ifs_maps[i].params.x;
        let a = n * ff_atan2(v.y, v.x);
        let rr = pow(sqrt(r2), abs(n) / ifs_maps[i].params.y);
        return vec2<f32>(rr * cos(a), rr * sin(a));
    }
    if (kind == 2.0) {
        return v / max(r2, 1e-30);
    }
    if (kind == 4.0) {
        // hemisphere: v / sqrt(1 - |v|^2) (plan 8.10 D1)
        if (r2 >= 1.0) {
            return v * 1e30;
        }
        return v / sqrt(1.0 - r2);
    }
    if (kind == 5.0) {
        // disc: rho = |v| is |theta|/pi; phi, the angle of v from +y,
        // is pi*r modulo 2pi; the branch m is the ring, and its parity
        // the sign of theta (D2).
        let rho = sqrt(r2);
        if (rho > 1.0) {
            return v * 1e30;
        }
        let pi = 3.14159265358979;
        let m = ifs_maps[i].branch;
        let r = ff_atan2(v.x, v.y) / pi + m;
        if (r < 0.0) {
            return vec2<f32>(1e30, 1e30);
        }
        let even = fract(m * 0.5) == 0.0;
        let theta = select(-pi * rho, pi * rho, even);
        return vec2<f32>(r * sin(theta), r * cos(theta));
    }
    if (kind == 6.0) {
        // blob: swap(v) / s(theta), theta the angle of v from +x (D3).
        let theta = ff_atan2(v.y, v.x);
        let high = ifs_maps[i].params.x;
        let low = ifs_maps[i].params.y;
        let waves = ifs_maps[i].params.z;
        let s = low + (high - low) * 0.5 * (sin(waves * theta) + 1.0);
        return vec2<f32>(v.y, v.x) / s;
    }
    // bubble
    if (r2 > 1.0) {
        return v * 1e30;
    }
    return v * ifs_bubble_scale(r2, ifs_maps[i].branch);
}

// The factor on the row's constant sigma_min at v (J3, S2, S4): the
// chain rule at the orbit point for a root, |f'| = |v|^2 for the
// inversion, and for bubble the smaller of the tangential derivative
// |v|/|p| and the radial one, which is the tangential times
// sqrt(1 - |v|^2) -- the forward folds at |p| = 2 and its radial
// derivative passes through zero there (see Kernel::local_sigma_factor).
fn ifs_kernel_sigma(i: u32, v: vec2<f32>) -> f32 {
    let kind = ifs_maps[i].kind;
    let r2 = max(dot(v, v), 1e-30);
    if (kind == 1.0) {
        return pow(sqrt(r2), 1.0 - abs(ifs_maps[i].params.x) / ifs_maps[i].params.y);
    }
    if (kind == 2.0) {
        return r2;
    }
    if (kind == 4.0) {
        return pow(max(1.0 - r2, 0.0), 1.5);
    }
    if (kind == 5.0) {
        let pi = 3.14159265358979;
        let rho = sqrt(r2);
        let r = ff_atan2(v.x, v.y) / pi + ifs_maps[i].branch;
        if (r <= 0.0) {
            return 0.0;
        }
        return min(pi * rho, 1.0 / (pi * r));
    }
    if (kind == 6.0) {
        let theta = ff_atan2(v.y, v.x);
        let high = ifs_maps[i].params.x;
        let low = ifs_maps[i].params.y;
        let waves = ifs_maps[i].params.z;
        let s = low + (high - low) * 0.5 * (sin(waves * theta) + 1.0);
        let ds = (high - low) * 0.5 * waves * cos(waves * theta);
        let a = 2.0 * s * s + ds * ds;
        let d = sqrt(max(a * a - 4.0 * s * s * s * s, 0.0));
        return sqrt(max((a - d) * 0.5, 0.0));
    }
    if (r2 >= 1.0) {
        return 1.0;
    }
    return sqrt(1.0 - r2) / ifs_bubble_scale(r2, ifs_maps[i].branch);
}

// When p is outside map i's IMAGE, a lower bound on its distance to
// the map's piece, in p's frame; negative when it is inside (S4).
// The kernels whose image is the unit disc of v have one: bubble,
// hemisphere and disc (kinds 3, 4, 5).
fn ifs_image_gap(i: u32, p: vec2<f32>) -> f32 {
    let kind = ifs_maps[i].kind;
    let hole = ifs_maps[i].params.w;
    // An inversion's image has a hole about the pole; a point inside
    // it is a known distance from the piece, and is exactly the point
    // whose inverse would overflow f32. Scored, not expanded.
    let inversion = (kind == 1.0 || kind == 2.0) && hole > 0.0;
    if (!inversion && kind != 3.0 && kind != 4.0 && kind != 5.0) {
        return -1.0;
    }
    let m = ifs_maps[i].inv_m;
    let t = ifs_maps[i].inv_t;
    let v = vec2<f32>(
        m.x * p.x + m.y * p.y + t.x,
        m.z * p.x + m.w * p.y + t.y,
    );
    let r = length(v);
    if (inversion) {
        if (r < hole) {
            return (hole - r) * ifs_maps[i].params.z;
        }
        return -1.0;
    }
    if (r > 1.0) {
        return (r - 1.0) * ifs_maps[i].params.z;
    }
    return -1.0;
}

// Inverse of map i.
//
// An affine row (kind == 0) is one affine. A nonlinear row is the
// post-inverse with 1/w folded in, then the kernel's inverse along
// the row's branch, then the pre-inverse.
fn ifs_inv_point(i: u32, p: vec2<f32>) -> vec2<f32> {
    let m = ifs_maps[i].inv_m;
    let t = ifs_maps[i].inv_t;
    let q = vec2<f32>(
        m.x * p.x + m.y * p.y + t.x,
        m.z * p.x + m.w * p.y + t.y,
    );
    if (ifs_maps[i].kind == 0.0) {
        return q;
    }
    let u = ifs_kernel_inverse(i, q);
    let pm = ifs_maps[i].pre_m;
    let pt = ifs_maps[i].pre_t;
    return vec2<f32>(
        pm.x * u.x + pm.y * u.y + pt.x,
        pm.z * u.x + pm.w * u.y + pt.y,
    );
}

// The forward map's sigma_min at the point whose image is p: the
// row's constant, times the kernel's local factor.
fn ifs_inv_sigma(i: u32, p: vec2<f32>) -> f32 {
    let s = ifs_maps[i].sigma_min;
    if (ifs_maps[i].kind == 0.0) {
        return s;
    }
    let m = ifs_maps[i].inv_m;
    let t = ifs_maps[i].inv_t;
    let v = vec2<f32>(
        m.x * p.x + m.y * p.y + t.x,
        m.z * p.x + m.w * p.y + t.y,
    );
    return s * ifs_kernel_sigma(i, v);
}

// The plane's radius, without `length()`'s overflow: a julia orbit
// squares its radius per level and passes f32's square by its ninth.
// See ifs_radius4 in the solid walk.
fn ifs_radius2(q: vec2<f32>, c: vec2<f32>) -> f32 {
    let d = q - c;
    let m = max(abs(d.x), abs(d.y));
    if (m > 1e15) {
        return m * length(d / m);
    }
    return length(d);
}

// Where in the annulus [R, R/sigma] an escaped point sits, counted
// DOWN so the level rises toward the set and joins continuously onto
// the next band.
fn ifs_residual(r: f32, radius: f32, sigma: f32) -> f32 {
    if (!(radius > 0.0) || !(sigma > 0.0) || !(sigma < 1.0)) {
        return 0.0;
    }
    let across = log(r / radius) / log(1.0 / sigma);
    return 1.0 - clamp(across, 0.0, 1.0);
}

// One partial address the beam is still following.
//
// A single POINT, not a reference and a delta: the CPU hands over at
// the level where the delta has grown to a quarter of the ball's
// radius, and at that size f32 holds the sum with nothing to spare
// for. The split is real and it is exact -- it is just entirely the
// CPU's, which is the whole point of doing it there.
struct IfsCand {
    q: vec2<f32>,
    // The point at first escape -- the orbit-trap coordinate.
    point: vec2<f32>,
    // Product of the sigma_min applied so far, PER PIXEL WIDTH. In
    // world units this underflows f32 at a zoom the delta could still
    // have been carried through.
    sigma: f32,
    // Running maximum of sigma * (r - radius), in the same pixel
    // units, unclamped so it is negative while inside the ball.
    bound: f32,
    // Distance from the ball's centre now -- THE ranking key, and the
    // whole of it. Ranking by `bound` instead is degenerate: it is a
    // running maximum, so once a path grazes the ball's edge every
    // descendant inherits the same value and the siblings cannot be
    // told apart (measured on the gasket: loose at 568 of 576 grid
    // points that way against 10 this way).
    r: f32,
    addr: f32,
    level: f32,
    color: f32,
    // sigma_min of the last map applied, for the annulus residual.
    last_sigma: f32,
    // bit 0 = escaped, bit 1 = done (past FAR, no longer expanded).
    flags: u32,
    // The map this path took last, or `IFS_NO_LAST` at level zero.
    // Only xaos reads it: appending a child puts it immediately
    // BEFORE this one in the chaos game's order, and that is the
    // transition the graph has to admit (`ifs-general.md` D4).
    last: u32,
}

fn ifs_evaluate(uv: vec2<f32>) -> IfsResult {
    let c = ifs_centre();
    let radius = ifs_radius();
    let n = ifs_count();

    var res: IfsResult;
    res.distance = 0.0;
    res.level = 0.0;
    res.address = 0.0;
    res.color = 0.0;
    res.point = vec2<f32>(0.0, 0.0);
    res.escaped = 0u;
    res.depth = 0u;
    if (n == 0u) {
        // No qualifying flame. Report the pixel as infinitely far from
        // the set rather than on it: distance 0 is what a point ON the
        // attractor returns, and would fill the frame with the
        // interior colour instead of drawing nothing. The panel is
        // where the reason is explained.
        res.distance = 1e30;
        res.escaped = 1u;
        return res;
    }

    let max_levels = u32(clamp(fparam(0u), 1.0, 256.0));
    let beam = u32(clamp(fparam(1u), 1.0, f32(IFS_MAX_BEAM)));
    let far = max(radius, 1.0) * 1e12;
    let handover = ifs_handover_level();
    var addr_scale = ifs_addr_scale();
    // The smallest bound among pieces the walk could not enter (S4):
    // the answer is the minimum over ALL pieces, reachable or not.
    // The prefix met some of them and carried what it found.
    var dead_min = ifs_seed_dead_min();

    // Seed from the reference orbit the CPU walked. Every one of these
    // candidates is where the view centre's own beam had got to, and
    // `basis` carries this pixel's offset the same distance.
    // The seeds are every pixel's top `beam` -- the handover stops at
    // the first level the view does not agree on that -- ranked here
    // by THIS pixel's position, as the walk ranks every level, by
    // insertion so no array wider than the beam is declared.
    var live: array<IfsCand, IFS_MAX_BEAM>;
    var next: array<IfsCand, IFS_MAX_BEAM>;
    var live_count = 0u;
    let seed_count = max(ifs_seed_count(), 1u);
    for (var j = 0u; j < seed_count; j = j + 1u) {
        let a = ifs_seed(j, 0u);
        let b = ifs_seed(j, 1u);
        let d = ifs_seed(j, 2u);
        let e = ifs_seed(j, 3u);
        var cand: IfsCand;
        // position + basis * uv + Q(uv). The basis is already composed
        // with the view, and Q is the second-order part a nonlinear
        // inverse would otherwise drop -- zero on an affine walk, so
        // an affine delta stays the exact thing it has always been.
        let f = ifs_seed(j, 4u);
        let g = ifs_seed(j, 5u);
        let uu = uv.x * uv.x;
        let uvv = uv.x * uv.y;
        let vv = uv.y * uv.y;
        cand.q = vec2<f32>(
            a.x + a.z * uv.x + a.w * uv.y + f.x * uu + f.z * uvv + g.x * vv,
            a.y + b.x * uv.x + b.y * uv.y + f.y * uu + f.w * uvv + g.y * vv,
        );
        cand.sigma = b.z;
        cand.bound = b.w;
        cand.r = ifs_radius2(cand.q, c);
        cand.addr = d.x;
        cand.last_sigma = d.y;
        cand.level = d.z;
        // Bits 0 and 1 are the flags; bits 8 and up are the last map
        // plus one, packed by `pack_seeds` because every seed vec4 is
        // already full. Zero there is a handover at level 0, which has
        // no last map and admits every child.
        let fw = bitcast<u32>(d.w);
        cand.flags = fw & 3u;
        let lastp = fw >> 8u;
        cand.last = select(IFS_NO_LAST, lastp - 1u, lastp != 0u);
        cand.point = vec2<f32>(e.x, e.y);
        cand.color = e.z;
        var pos = live_count;
        for (var i = 0u; i < live_count; i = i + 1u) {
            var ck = cand.r;
            var lk = live[i].r;
            if (ifs_weighted_key()) {
                ck = ck * cand.sigma;
                lk = lk * live[i].sigma;
            }
            if (ck < lk) {
                pos = i;
                break;
            }
        }
        if (pos < beam) {
            var i = min(live_count, beam - 1u);
            loop {
                if (i <= pos) {
                    break;
                }
                live[i] = live[i - 1u];
                i = i - 1u;
            }
            live[pos] = cand;
            live_count = min(live_count + 1u, beam);
        }
    }

    // The best path that has finished: its bound is final and it need
    // not hold a beam slot.
    var best_done: IfsCand;
    var has_done = false;
    // The deepest level any finished path reached (see the CPU walk).
    var deepest_done = -1.0;
    for (var k = 0u; k < max_levels; k = k + 1u) {
        var all_done = true;
        for (var ci = 0u; ci < live_count; ci = ci + 1u) {
            if ((live[ci].flags & 2u) != 0u) {
                continue;
            }
            let r = ifs_radius2(live[ci].q, c);
            live[ci].r = r;
            // A level whose term is not a number says nothing about
            // the piece; the bound the path had stands. Folding an
            // overflowed term in is what painted a grand julian's
            // cut-outs -- see `ifs_estimate::fold_level`. `<=` is the
            // Metal-safe test: false for inf and for NaN alike.
            let term = live[ci].sigma * (r - radius);
            if (abs(term) <= 1e37) {
                live[ci].bound = max(live[ci].bound, term);
            }

            if (r > radius && (live[ci].flags & 1u) == 0u) {
                live[ci].flags = live[ci].flags | 1u;
                live[ci].level =
                    f32(handover + k) + ifs_residual(r, radius, live[ci].last_sigma);
                live[ci].point = live[ci].q;
            }

            if (!(r < far)) {
                live[ci].flags = live[ci].flags | 2u;
                // Frozen inside the ball: no information (CPU rule).
                if (!(abs(r) <= 1e37) && !(live[ci].bound > 0.0)) {
                    live[ci].bound = 1e38;
                }
            } else {
                all_done = false;
            }
        }
        if (all_done) {
            break;
        }
        // Positions evaluated must be exactly `max_levels`: expanding
        // on the last pass would score a level deeper than asked for.
        if (k + 1u >= max_levels) {
            break;
        }

        // Expand, keeping only the `beam` best children.
        //
        // Two passes, and the split is what makes this affordable. The
        // obvious form -- build each child in full and insertion-sort
        // it into the keep-list -- shifts a whole candidate through the
        // list for each of `beam * n` children, which was the shader's
        // entire cost. Pass one ranks by KEY alone; pass two rebuilds
        // only the `beam` survivors.
        var key: array<f32, IFS_MAX_BEAM>;
        var src: array<u32, IFS_MAX_BEAM>;
        var next_count = 0u;
        let stride = n + 1u;

        for (var ci = 0u; ci < live_count; ci = ci + 1u) {
            var first = 0u;
            var last = n;
            if ((live[ci].flags & 2u) != 0u) {
                // Its level, if it escaped. A path that finished without
                // escaping froze inside the ball and says nothing about
                // depth either -- counting it as "unescaped", the
                // deepest possible, flattened the level colouring.
                if ((live[ci].flags & 1u) != 0u) {
                    deepest_done = max(deepest_done, live[ci].level);
                }
                // A done path never expands, so its bound is final. It
                // leaves the beam and is remembered by that bound --
                // kept, it would compete for a slot by a key that is
                // no longer a number, rank last, and be pruned in
                // favour of live paths that lead nowhere. See the CPU
                // walk for the measurement.
                let b = live[ci].bound;
                if (abs(b) <= 1e37 && (!has_done || b < best_done.bound)) {
                    best_done = live[ci];
                    has_done = true;
                }
                continue;
            }
            for (var bi = first; bi < last; bi = bi + 1u) {
                // A transition the graph forbids is not a gap: a gap
                // is a proof that no PREIMAGE exists and belongs in
                // the answer's minimum, while this branch simply is
                // not part of the dynamics and has nothing to say.
                if (bi < n && !ifs_admits(bi, live[ci].last)) {
                    continue;
                }
                var cand_key = live[ci].r;
                if (bi < n) {
                    var gap = ifs_image_gap(bi, live[ci].q);
                    if (gap >= 0.0) {
                        // The larger of the geometric gap and what one
                        // step of the expansion would bound, when that
                        // step is representable -- so the field is
                        // continuous at the gap's edge (see the CPU
                        // walk's `gap_bound`). Below f32's reach the
                        // step is not a number and the geometric gap,
                        // which dominates there anyway, stands alone.
                        let gq = ifs_inv_point(bi, live[ci].q);
                        let gt = ifs_inv_sigma(bi, live[ci].q) * (ifs_radius2(gq, c) - radius);
                        if (abs(gt) <= 1e37) {
                            gap = max(gap, gt);
                        }
                        dead_min = min(dead_min, max(live[ci].bound, live[ci].sigma * gap));
                        continue;
                    }
                    cand_key = ifs_radius2(ifs_inv_point(bi, live[ci].q), c);
                    if (ifs_weighted_key()) {
                        cand_key = cand_key * live[ci].sigma * ifs_inv_sigma(bi, live[ci].q);
                    }
                    // Not a number ranks LAST: the path has already
                    // left the representable plane.
                    if (!(cand_key <= 1e37)) {
                        cand_key = 1e38;
                    }
                }
                var pos = next_count;
                for (var j = 0u; j < next_count; j = j + 1u) {
                    if (cand_key < key[j]) {
                        pos = j;
                        break;
                    }
                }
                if (pos < beam) {
                    var j = min(next_count, beam - 1u);
                    loop {
                        if (j <= pos) {
                            break;
                        }
                        key[j] = key[j - 1u];
                        src[j] = src[j - 1u];
                        j = j - 1u;
                    }
                    key[pos] = cand_key;
                    src[pos] = ci * stride + bi;
                    next_count = min(next_count + 1u, beam);
                }
            }
        }

        if (next_count == 0u) {
            // Every live path was fully gapped: its own bound is not
            // an answer (see the CPU walk). A done path would have
            // re-entered `next`, so none of these is done.
            for (var ci = 0u; ci < live_count; ci = ci + 1u) {
                live[ci].bound = 1e38;
            }
            break;
        }
        for (var k2 = 0u; k2 < next_count; k2 = k2 + 1u) {
            let parent = src[k2] / stride;
            let bi = src[k2] % stride;
            var child = live[parent];
            if (bi < n) {
                child.q = ifs_inv_point(bi, live[parent].q);
                child.sigma = live[parent].sigma * ifs_inv_sigma(bi, live[parent].q);
                child.last_sigma = ifs_maps[bi].sigma_min;
                child.last = bi;
                child.r = ifs_radius2(child.q, c);
                // Score the child as it is made: one that inherited
                // only its parent's bound would rank identically to
                // all its siblings.
                let cterm = child.sigma * (child.r - radius);
                if (abs(cterm) <= 1e37) {
                    child.bound = max(live[parent].bound, cterm);
                } else {
                    child.bound = live[parent].bound;
                }
                if ((live[parent].flags & 1u) == 0u) {
                    child.addr = live[parent].addr + f32(bi) * addr_scale;
                    if (handover + k == 0u) {
                        // The coarsest branch is the piece the point is
                        // in, so its transform colour is the direct
                        // analogue of a flame's.
                        child.color = ifs_maps[bi].color;
                    }
                }
            }
            next[k2] = child;
        }

        for (var ci = 0u; ci < next_count; ci = ci + 1u) {
            live[ci] = next[ci];
        }
        live_count = next_count;
        addr_scale = addr_scale / f32(n);
    }

    // The beam is RANKED by position but ANSWERED by bound: pruning
    // asks "which piece is this point in", and the DISTANCE asks
    // "which surviving address gives the smallest".
    // Among FINITE bounds: a candidate whose bound overflowed has
    // nothing to say, and "infinitely far" is exactly the cut-out.
    var win = 0u;
    for (var ci = 1u; ci < live_count; ci = ci + 1u) {
        let b = live[ci].bound;
        let wb = live[win].bound;
        if (abs(b) <= 1e37 && (!(abs(wb) <= 1e37) || b < wb)) {
            win = ci;
        }
    }
    // A finished path that beats every live one is the answer.
    if (has_done && (!(abs(live[win].bound) <= 1e37) || best_done.bound < live[win].bound)) {
        live[win] = best_done;
    }
    let best = live[win];

    // The LEVEL is a different question and takes a different answer:
    // the deepest any surviving address reached. That is what an
    // escape-time colouring means by a level, and it is what the
    // Hepting-Hart escape buffer computes -- so it is the quantity
    // D9 is decided on. Measured against exhaustive search over three
    // overlapping IFSs, taking the winner's level instead costs up to
    // a percentage point and is not even monotone in the beam width.
    let unescaped = f32(handover + max_levels);
    var deepest = -1.0;
    for (var ci = 0u; ci < live_count; ci = ci + 1u) {
        var lvl = unescaped;
        if ((live[ci].flags & 1u) != 0u) {
            lvl = live[ci].level;
        }
        deepest = max(deepest, lvl);
    }
    deepest = max(deepest, deepest_done);

    // No finite winner: the gaps are the answer; no gaps either:
    // nothing is known and zero is the sound reading.
    if (abs(best.bound) <= 1e37) {
        res.distance = max(min(best.bound, dead_min), 0.0);
    } else if (dead_min < 1e37) {
        res.distance = max(dead_min, 0.0);
    } else {
        res.distance = 0.0;
    }
    res.address = best.addr;
    res.color = best.color;
    res.level = deepest;
    res.depth = u32(max(floor(deepest), 0.0));
    if ((best.flags & 1u) != 0u) {
        res.point = best.point;
        res.escaped = 1u;
    } else {
        res.point = best.q;
    }
    return res;
}
"#,
};

/// The loaded flame's attractor in three dimensions, sphere-traced.
///
/// The walk is the planar one's, in 3D arithmetic; what it answers is
/// different in one way that matters, and the marcher is the reason.
/// A plane render wants the distance in PIXELS, because that is what
/// an edge and a halo are measured in and because world units
/// underflow f32 under a deep zoom. A marcher steps BY the distance,
/// so it has to be a length in the space the ray is crossing.
pub static IFS_FLAME_3D: IfsDef = IfsDef {
    name: "ifs_flame_3d",
    display_name: "Flame Attractor (Solid)",
    solid: true,
    needs_flame: true,
    default_coloring: "ifs_address",
    presets: &[],
    parameters: &[
        EscapeParamDef {
            name: "levels",
            display_name: "Inverse Depth",
            default: 24.0,
            min: 1.0,
            max: 256.0,
            tooltip: "How many inverse maps to apply before calling the point part of \
                      the set. Every march step pays this, so it costs more here than \
                      in the plane.",
            choices: &[],
        },
        EscapeParamDef {
            name: "beam",
            display_name: "Beam Width",
            default: 1.0,
            min: 1.0,
            max: 8.0,
            tooltip: "How many branch addresses the walk follows at once. Every address \
                      bounds the distance to ITS piece and the truth is the smallest, so \
                      following one can only read too far -- which a marcher turns into \
                      a surface that is not there. Raise it for a solid whose pieces \n                      OVERLAP; one whose pieces TILE needs nothing, and a march \n                      pays the beam on every step rather than once per pixel -- \n                      measured at 2.8x the time for two bytes of difference.",
            choices: &[],
        },
        EscapeParamDef {
            name: "steps",
            display_name: "March Steps",
            default: 96.0,
            min: 4.0,
            max: 512.0,
            tooltip: "How many times a ray may step before giving up. A ray that runs \
                      out is left as a miss, so too few steps eat holes in a surface \
                      seen at a glancing angle, where a ray travels furthest for the \
                      distance it closes.",
            choices: &[],
        },
        EscapeParamDef {
            name: "shadow",
            display_name: "Shadows",
            default: 0.7,
            min: 0.0,
            max: 1.0,
            tooltip: "How dark a shadowed surface goes. 0 skips the shadow march \
                      entirely, and that march is what shadows cost -- one more walk \
                      of the distance function per LIGHT per lit pixel, so the price \
                      is set by how many lights you switch on.",
            choices: &[],
        },
        EscapeParamDef {
            name: "shadow_sharpness",
            display_name: "Shadow Sharpness",
            default: 12.0,
            min: 1.0,
            max: 128.0,
            tooltip: "How sharply a shadow's edge falls off. The march already knows \
                      how close it passed to the surface, so the penumbra costs \
                      nothing. The number stands for the inverse of the light's own \
                      SIZE -- a broad light makes soft edges -- so small values \
                      spread the penumbra and large ones harden it.",
            choices: &[],
        },
        EscapeParamDef {
            name: "occlusion",
            display_name: "Occlusion Reach",
            default: 0.15,
            min: 0.0,
            max: 0.5,
            tooltip: "How far to look for the walls that enclose a point, as a \
                      fraction of the whole attractor. It has to be comparable to the \
                      feature you want shaded: small values darken only the tightest \
                      crevices, and a hollow wider than this reads as open -- which, \
                      at that reach, it is. 0 turns occlusion off. Occlusion answers \
                      for the sky; a cavity goes properly dark because the LIGHT \
                      cannot reach into it, and that is Shadows.",
            choices: &[],
        },
        EscapeParamDef {
            name: "w_slice",
            display_name: "Slice Value",
            default: 0.0,
            min: -2.0,
            max: 2.0,
            tooltip: "For a solid of quaternion maps (plan 8.11 step 3): the scalar                       coordinate the 3D picture is a slice of. Sweep it to walk                       through the 4D set. Nothing else reads it.",
            choices: &[],
        },
        EscapeParamDef {
            name: "extent",
            display_name: "Extent",
            default: 0.0,
            min: 0.0,
            max: 100.0,
            tooltip: "The radius of the ball the set is cut at, about its measured \
                      centre; 0 measures it from the flame. For an affine set the \
                      measurement is exact and this changes nothing worth having. \
                      For a set with inversions (a julia of negative distance, \
                      spherical) the set is unbounded, the ball is a cut through \
                      it at the bulk of a sample, and every far-field reading -- \
                      the smooth falloff beside a piece -- is scaled by the \
                      radius. The sample drifts as the flame animates, and the \
                      exterior slides with it: 16 pixels for a \
                      six-degree turn of one transform, measured, while the set \
                      there moved 94. The criterion above shows the measured \
                      value; set it here to hold it through an animation.",
            choices: &[],
        },
    ],
    wgsl: r#"
// Inverse of map i, in three dimensions.
// The Hamilton product of (x, y, z, w) quaternions, scalar w.
fn ifs_qmul(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(
        a.w * b.xyz + b.w * a.xyz + cross(a.xyz, b.xyz),
        a.w * b.w - dot(a.xyz, b.xyz),
    );
}

// A quaternion to an INTEGER power by Hamilton products: exact, where
// the polar form's acos(w/|q|) loses half its digits near the real
// axis -- and the walk's first inverse step lands every slice point
// there, since (xyz, 0)^2 is real. A negative power is the conjugate's
// over |q|^(2|n|).
fn ifs_qpow(q: vec4<f32>, n: f32) -> vec4<f32> {
    let k = i32(round(n));
    var base = q;
    if (k < 0) {
        let m2 = dot(q, q);
        if (m2 < 1e-30) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
        base = vec4<f32>(-q.xyz, q.w) / m2;
    }
    var e = u32(abs(k));
    var out = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    loop {
        if (e == 0u) {
            break;
        }
        if ((e & 1u) == 1u) {
            out = ifs_qmul(out, base);
        }
        base = ifs_qmul(base, base);
        e = e >> 1u;
    }
    return out;
}

// The 3D kernels' inverses on (v, w) (plan 8.11 steps 2 and 3).
// julia3D (kind 1): radius |v|^n, azimuth n*phi, elevation kept, z
// scaled back by |n|. julia3Dz (kind 2): the plane's root on xy, z
// by |n| |v_xy|^(n-1). Both pass w through. quaternion_julia (kind 3):
// the polynomial on the 4D point, its radius exponent d/n undone, c_w
// added here and c_xyz in the pre-inverse's translation.
fn ifs_kernel_inverse3(i: u32, v: vec3<f32>, w: f32) -> vec4<f32> {
    let kind = ifs_maps[i].extra.z;
    let n = ifs_maps[i].extra.w;
    if (kind == 3.0) {
        // Reassemble: projection 1 (Depth) carried the k component and
        // put the scalar in the 3D point, so it goes back in the k
        // slot. A permutation, so no singular value moves with it.
        var q = vec4<f32>(v, w);
        if (ifs_maps[i].extra2.w > 0.5) {
            q = vec4<f32>(v.x, v.y, w, v.z);
        }
        let mag = length(q);
        let d = ifs_maps[i].extra2.y;
        let p = ifs_qpow(q, n);
        let scale = select(0.0, pow(mag, n / d) / pow(mag, n), mag > 1e-30);
        return vec4<f32>(p.xyz * scale, p.w * scale + ifs_maps[i].extra2.x);
    }
    let rxy = length(v.xy);
    let theta = n * ff_atan2(v.y, v.x);
    if (kind == 1.0) {
        let rho_out = length(v);
        if (rho_out < 1e-30) {
            return vec4<f32>(0.0, 0.0, 0.0, w);
        }
        let rho = pow(rho_out, n);
        let sqrt_r2d = rho * (rxy / rho_out);
        let zz = rho * (v.z / rho_out);
        return vec4<f32>(sqrt_r2d * cos(theta), sqrt_r2d * sin(theta), zz * abs(n), w);
    }
    if (rxy < 1e-30) {
        return vec4<f32>(0.0, 0.0, 0.0, w);
    }
    let sqrt_r2d = pow(rxy, n);
    return vec4<f32>(sqrt_r2d * cos(theta), sqrt_r2d * sin(theta), v.z * abs(n) * pow(rxy, n - 1.0), w);
}

// The factor on the row's constant sigma_min at (v, w).
fn ifs_kernel_sigma3(i: u32, v: vec3<f32>, w: f32) -> f32 {
    let kind = ifs_maps[i].extra.z;
    let n = ifs_maps[i].extra.w;
    if (kind == 3.0) {
        let d = ifs_maps[i].extra2.y;
        return pow(max(length(vec4<f32>(v, w)), 1e-30), 1.0 - n / d);
    }
    if (kind == 1.0) {
        return pow(max(length(v), 1e-30), 1.0 - n);
    }
    let rxy = max(length(v.xy), 1e-30);
    let r = pow(rxy, n);
    let z = v.z * abs(n) * pow(rxy, n - 1.0);
    let a = pow(r, 1.0 / n - 1.0) / abs(n);
    let b = z * (1.0 / n - 1.0) * pow(r, 1.0 / n - 2.0) / abs(n);
    let s = a * a + b * b + a * a;
    let d = sqrt(max(s * s - 4.0 * a * a * a * a, 0.0));
    return min(sqrt(max((s - d) * 0.5, 0.0)), a);
}

fn ifs_inv_affine3(i: u32, p: vec3<f32>) -> vec3<f32> {
    let m = ifs_maps[i];
    return vec3<f32>(
        dot(m.r0.xyz, p) + m.r0.w,
        dot(m.r1.xyz, p) + m.r1.w,
        dot(m.r2.xyz, p) + m.r2.w,
    );
}

// Inverse of map i on (p, w): one affine on an affine row (kind 0),
// w through; the post-inverse with 1/weight folded in (on w too, as
// the variation's weight scales the whole quaternion), the kernel's
// inverse, then the pre-inverse on a nonlinear row.
fn ifs_inv_step3(i: u32, p: vec3<f32>, w: f32) -> vec4<f32> {
    let q = ifs_inv_affine3(i, p);
    if (ifs_maps[i].extra.z == 0.0) {
        return vec4<f32>(q, w);
    }
    // The weight scales the whole quaternion, so w is divided by it
    // before the kernel as xyz were by the post-inverse's fold; the
    // row carries that 1/weight in extra2.z (one on a row whose
    // kernel passes w through).
    let u = ifs_kernel_inverse3(i, q, w * ifs_maps[i].extra2.z);
    let m = ifs_maps[i];
    return vec4<f32>(
        dot(m.p0.xyz, u.xyz) + m.p0.w,
        dot(m.p1.xyz, u.xyz) + m.p1.w,
        dot(m.p2.xyz, u.xyz) + m.p2.w,
        u.w,
    );
}

fn ifs_inv_point3(i: u32, p: vec3<f32>, w: f32) -> vec3<f32> {
    return ifs_inv_step3(i, p, w).xyz;
}

// The forward map's sigma_min at the point whose image is (p, w).
fn ifs_inv_sigma3(i: u32, p: vec3<f32>, w: f32) -> f32 {
    let s = ifs_maps[i].extra.x;
    if (ifs_maps[i].extra.z == 0.0) {
        return s;
    }
    return s * ifs_kernel_sigma3(i, ifs_inv_affine3(i, p), w * ifs_maps[i].extra2.z);
}

fn ifs_residual(r: f32, radius: f32, sigma: f32) -> f32 {
    if (!(radius > 0.0) || !(sigma > 0.0) || !(sigma < 1.0)) {
        return 0.0;
    }
    let across = log(r / radius) / log(1.0 / sigma);
    return 1.0 - clamp(across, 0.0, 1.0);
}

struct IfsCand3 {
    q: vec3<f32>,
    point: vec3<f32>,
    sigma: f32,
    bound: f32,
    r: f32,
    addr: f32,
    level: f32,
    color: f32,
    last_sigma: f32,
    flags: u32,
    // The scalar coordinate a quaternion kernel carries (plan 8.11
    // step 3); affine and 3D-root rows pass it through.
    w: f32,
    // The map this path took last, or `IFS_NO_LAST`, for the xaos
    // test (`ifs-general.md` D4). As the planar walk's.
    last: u32,
}

// The walk's radius: the 4D distance to the ball's centre, whose
// scalar coordinate is the globals' aux centre.
//
// Not `length()`: it squares its components, and a polynomial orbit
// reaches 1.8e19 by its ninth level, whose square is past f32. The
// key came out as infinity, the child's bound with it, and with a
// beam of one that was the answer -- the march overshot and missed.
// Measured: the GPU agreed with the CPU at 100% to eight levels and
// found 37 of its 2410 hits at ten. (Calling anything past 1e15 "far"
// instead was tried first and found none at all: the bound needs the
// true radius, 1.8e19 and not 1e30, because sigma times it is the
// convergent tail of the estimate.)
fn ifs_radius4(q: vec3<f32>, w: f32, c: vec3<f32>) -> f32 {
    let d = vec4<f32>(q - c, w - ifs_ball_w());
    let m = max(max(abs(d.x), abs(d.y)), max(abs(d.z), abs(d.w)));
    // Exact and overflow-free where overflow is possible: the largest
    // component times the norm of the vector scaled by it, whose
    // components are at most one. Below that, `length()` itself, so
    // every affine walk's arithmetic -- and every affine preset's
    // bytes -- is exactly what it was.
    if (m > 1e15) {
        return m * length(d / m);
    }
    return length(d);
}

// The walk, in three dimensions. The same algorithm the planar one
// runs -- a beam of addresses, ranked by distance to the ball's
// centre, answered by the smallest bound -- and the same reasons for
// each part. What differs is the arithmetic and that the distance is
// in WORLD units here rather than pixels: a marcher steps by it, so
// it has to be a length in the space the ray is crossing.
// `eps` is the precision the CALLER needs of the distance, in world
// units, and 0 means all of it.
//
// A candidate's bound is a running maximum, and once it has left the
// ball each further level can move that maximum by at most about
// sigma_k * R -- the candidate's remaining contraction times the ball.
// When that is below what the caller can show, the rest of the walk is
// work the picture cannot see. Measured on the sponge at 1080p: the
// answer is within a pixel by level SIX, against a default of 24, at
// every distance from the set. The marcher's steps, the normal, the
// occlusion probes and the shadow rays all pass their own tolerance;
// the one evaluation at the hit point that feeds the COLOURINGS passes
// zero, because the level and the address are only known when a
// candidate escapes and stopping first would report it as interior.
fn ifs_walk3(delta: vec3<f32>, eps_asked: f32) -> IfsResult {
    let c = ifs_ball_centre();
    let radius = ifs_radius();
    let n = ifs_count();
    // The precision early exit is an affine argument (see
    // pack_globals3): a nonlinear solid walks to full depth.
    let eps = select(eps_asked, 0.0, ifs_has_nonlinear());

    var res: IfsResult;
    res.distance = 0.0;
    res.level = 0.0;
    res.address = 0.0;
    res.color = 0.0;
    res.point = vec2<f32>(0.0, 0.0);
    res.escaped = 0u;
    res.depth = 0u;
    if (n == 0u) {
        res.distance = 1e30;
        res.escaped = 1u;
        return res;
    }

    let max_levels = u32(clamp(fparam(0u), 1.0, 256.0));
    let beam = u32(clamp(fparam(1u), 1.0, f32(IFS_MAX_BEAM)));
    let far = max(radius, 1.0) * 1e12;

    // The handover. `delta` is the sample's offset from the TARGET,
    // never its absolute position, and the chain is what turns one
    // into the other: the link's reference is where the target has got
    // to after `link` inverse maps -- walked on the CPU, where the
    // cancellation that costs a bit of the target per level could be
    // paid in real digits -- and its matrix carries the delta the same
    // way. Their SUM is O(1), which is why f32 can hold it however
    // deep the zoom is.
    //
    // A sample close to the target takes a deep link and one out at
    // the bounding sphere takes a shallow one. That is not a
    // compromise: it is the statement that the address prefix
    // containing a point is shorter the further the point is away.
    let link = ifs_pick_link(delta);
    let slots = ifs_beam_slots();
    let base_level = f32(link);

    // The link's candidates, ranked by THIS sample's position as the
    // walk ranks every level, by insertion so no array wider than the
    // beam is declared. A slot flagged empty (bit 2) is padding: a
    // level shallower than the slot count -- the top of the chain has
    // one candidate -- and a padding slot that repeated a real one
    // would fill the beam with copies of itself.
    var live: array<IfsCand3, IFS_MAX_BEAM>;
    var next: array<IfsCand3, IFS_MAX_BEAM>;
    var live_count = 0u;
    // No links at all -- a nonlinear solid, whose maps have no matrix
    // to carry a delta (plan 8.11 step 2) -- and the slots hold
    // whatever was last bound: the walk starts from the delta below.
    let has_chain = ifs_link_levels() > 0u;
    for (var b = 0u; has_chain && b < max(slots, 1u); b = b + 1u) {
        let L = ifs_links[link * slots + b];
        let flags = bitcast<u32>(L.extra.w);
        if ((flags & 4u) != 0u) {
            continue;
        }
        var root: IfsCand3;
        root.q = ifs_link_point(L, delta);
        root.sigma = L.r1.w;
        root.bound = L.r2.w;
        root.w = ifs_slice_w();
        root.r = ifs_radius4(root.q, root.w, c);
        root.addr = L.extra.x;
        root.color = L.esc.w;
        root.last_sigma = L.extra.y;
        root.flags = flags & 3u;
        // Bits 8 and up of the same word are the last map plus one.
        let rlast = flags >> 8u;
        root.last = select(IFS_NO_LAST, rlast - 1u, rlast != 0u);
        // An escape carried by the link keeps the level it happened
        // at. Recomputing it here would report a candidate that left
        // the ball at level three of a fifty-level prefix as leaving
        // at level fifty -- a constant shift through the whole
        // exterior of the picture.
        root.level = L.extra.z;
        root.point = L.esc.xyz;
        if ((root.flags & 1u) == 0u) {
            root.level = 0.0;
            root.point = root.q;
        }
        var pos = live_count;
        for (var i = 0u; i < live_count; i = i + 1u) {
            if (root.r < live[i].r) {
                pos = i;
                break;
            }
        }
        if (pos < beam) {
            var i = min(live_count, beam - 1u);
            loop {
                if (i <= pos) {
                    break;
                }
                live[i] = live[i - 1u];
                i = i - 1u;
            }
            live[pos] = root;
            live_count = min(live_count + 1u, beam);
        }
    }
    if (live_count == 0u) {
        // No usable link (a chain that is empty): walk from the delta
        // itself, which is what link zero would have been.
        var root: IfsCand3;
        root.q = delta + ifs_target_offset() + c;
        root.sigma = 1.0;
        root.bound = -1e30;
        root.w = ifs_slice_w();
        root.r = ifs_radius4(root.q, root.w, c);
        root.addr = 0.0;
        root.color = 0.0;
        root.last_sigma = ifs_mean_sigma();
        root.flags = 0u;
        root.level = 0.0;
        root.point = root.q;
        live[0] = root;
        live_count = 1u;
    }

    var addr_scale = pow(1.0 / f32(n), base_level + 1.0);
    var deepest = -1.0;

    // The best path that has finished: its bound is final and it need
    // not hold a beam slot.
    var best_done: IfsCand3;
    var has_done = false;
    // The deepest level any finished path reached (see the CPU walk).
    var deepest_done = -1.0;
    for (var k = 0u; k < max_levels; k = k + 1u) {
        var all_done = true;
        for (var ci = 0u; ci < live_count; ci = ci + 1u) {
            if ((live[ci].flags & 2u) != 0u) {
                continue;
            }
            let r = ifs_radius4(live[ci].q, live[ci].w, c);
            live[ci].r = r;
            // A level whose term is not a number says nothing about
            // the piece; the bound the path had stands. Folding an
            // overflowed term in is what painted a grand julian's
            // cut-outs -- see `ifs_estimate::fold_level`. `<=` is the
            // Metal-safe test: false for inf and for NaN alike.
            let term = live[ci].sigma * (r - radius);
            if (abs(term) <= 1e37) {
                live[ci].bound = max(live[ci].bound, term);
            }
            if (r > radius && (live[ci].flags & 1u) == 0u) {
                live[ci].flags = live[ci].flags | 1u;
                live[ci].level =
                    base_level + f32(k) + ifs_residual(r, radius, live[ci].last_sigma);
                live[ci].point = live[ci].q;
            }
            // Done when past FAR, or when this candidate can no longer
            // move the answer by what the caller asked for.
            if (!(r < far) || live[ci].sigma * radius < eps) {
                live[ci].flags = live[ci].flags | 2u;
                // Frozen inside the ball: no information (CPU rule).
                if (!(abs(r) <= 1e37) && !(live[ci].bound > 0.0)) {
                    live[ci].bound = 1e38;
                }
            } else {
                all_done = false;
            }
        }
        if (all_done || k + 1u >= max_levels) {
            break;
        }

        var key: array<f32, IFS_MAX_BEAM>;
        var src: array<u32, IFS_MAX_BEAM>;
        var next_count = 0u;
        let stride = n + 1u;
        for (var ci = 0u; ci < live_count; ci = ci + 1u) {
            var first = 0u;
            var last = n;
            if ((live[ci].flags & 2u) != 0u) {
                // Its level, if it escaped. A path that finished without
                // escaping froze inside the ball and says nothing about
                // depth either -- counting it as "unescaped", the
                // deepest possible, flattened the level colouring.
                if ((live[ci].flags & 1u) != 0u) {
                    deepest_done = max(deepest_done, live[ci].level);
                }
                // A done path never expands, so its bound is final. It
                // leaves the beam and is remembered by that bound --
                // kept, it would compete for a slot by a key that is
                // no longer a number, rank last, and be pruned in
                // favour of live paths that lead nowhere. See the CPU
                // walk for the measurement.
                let b = live[ci].bound;
                if (abs(b) <= 1e37 && (!has_done || b < best_done.bound)) {
                    best_done = live[ci];
                    has_done = true;
                }
                continue;
            }
            for (var bi = first; bi < last; bi = bi + 1u) {
                if (bi < n && !ifs_admits(bi, live[ci].last)) {
                    continue;
                }
                var cand_key = live[ci].r;
                if (bi < n) {
                    let s = ifs_inv_step3(bi, live[ci].q, live[ci].w);
                    cand_key = ifs_radius4(s.xyz, s.w, c);
                    if (ifs_weighted_key()) {
                        cand_key = cand_key * live[ci].sigma * ifs_inv_sigma3(bi, live[ci].q, live[ci].w);
                    }
                    if (!(cand_key <= 1e37)) {
                        cand_key = 1e38;
                    }
                }
                var pos = next_count;
                for (var j = 0u; j < next_count; j = j + 1u) {
                    if (cand_key < key[j]) {
                        pos = j;
                        break;
                    }
                }
                if (pos < beam) {
                    var j = min(next_count, beam - 1u);
                    loop {
                        if (j <= pos) {
                            break;
                        }
                        key[j] = key[j - 1u];
                        src[j] = src[j - 1u];
                        j = j - 1u;
                    }
                    key[pos] = cand_key;
                    src[pos] = ci * stride + bi;
                    next_count = min(next_count + 1u, beam);
                }
            }
        }
        for (var k2 = 0u; k2 < next_count; k2 = k2 + 1u) {
            let parent = src[k2] / stride;
            let bi = src[k2] % stride;
            var child = live[parent];
            if (bi < n) {
                let s = ifs_inv_step3(bi, live[parent].q, live[parent].w);
                child.q = s.xyz;
                child.w = s.w;
                child.sigma = live[parent].sigma * ifs_inv_sigma3(bi, live[parent].q, live[parent].w);
                child.last_sigma = ifs_maps[bi].extra.x;
                child.last = bi;
                child.r = ifs_radius4(child.q, child.w, c);
                let cterm = child.sigma * (child.r - radius);
                if (abs(cterm) <= 1e37) {
                    child.bound = max(live[parent].bound, cterm);
                } else {
                    child.bound = live[parent].bound;
                }
                if ((live[parent].flags & 1u) == 0u) {
                    child.addr = live[parent].addr + f32(bi) * addr_scale;
                    // The colour belongs to the FIRST map applied, and
                    // with a seeded start that map was applied on the
                    // CPU -- so this only fires at the very top of the
                    // chain, where the link is the target itself.
                    if (k == 0u && link == 0u) {
                        child.color = ifs_maps[bi].extra.y;
                    }
                }
            }
            next[k2] = child;
        }
        for (var ci = 0u; ci < next_count; ci = ci + 1u) {
            live[ci] = next[ci];
        }
        live_count = next_count;
        addr_scale = addr_scale / f32(n);
    }

    // Among FINITE bounds: a candidate whose bound overflowed has
    // nothing to say, and "infinitely far" is exactly the cut-out.
    var win = 0u;
    for (var ci = 1u; ci < live_count; ci = ci + 1u) {
        let b = live[ci].bound;
        let wb = live[win].bound;
        if (abs(b) <= 1e37 && (!(abs(wb) <= 1e37) || b < wb)) {
            win = ci;
        }
    }
    // A finished path that beats every live one is the answer.
    if (has_done && (!(abs(live[win].bound) <= 1e37) || best_done.bound < live[win].bound)) {
        live[win] = best_done;
    }
    let unescaped = base_level + f32(max_levels);
    for (var ci = 0u; ci < live_count; ci = ci + 1u) {
        var lvl = unescaped;
        if ((live[ci].flags & 1u) != 0u) {
            lvl = live[ci].level;
        }
        deepest = max(deepest, lvl);
    }
    deepest = max(deepest, deepest_done);
    let best = live[win];

    res.distance = max(best.bound, 0.0);
    res.address = best.addr;
    res.color = best.color;
    res.level = deepest;
    res.depth = u32(max(floor(deepest), 0.0));
    res.escaped = best.flags & 1u;
    // The trap coordinate, projected: a colouring reads it against
    // `ifs_centre()`, and in 3D that is the two axes across the view.
    res.point = best.point.xy;
    return res;
}

// What the marcher steps by.
//
// The argument is an offset from the camera's TARGET, not a position.
// Every sample the marcher takes is `eye_rel + dir*t`, both terms of
// which shrink with the zoom, so the offset keeps its relative
// precision where an absolute position would have spent it all on
// leading digits that are the same for the whole frame.
fn ifs_distance_at(delta: vec3<f32>, eps: f32) -> f32 {
    return ifs_walk3(delta, eps).distance;
}

// What the marcher colours with, at the offset it stopped at. Full
// depth: the colourings want the level and the address, which are only
// known once a candidate escapes.
fn ifs_evaluate3(delta: vec3<f32>) -> IfsResult {
    return ifs_walk3(delta, 0.0);
}
"#,
};

// ====================================================================
// Colorings — the four quantities of §2.3, one each
// ====================================================================

/// `d`: the set as a shape, with an exact antialiased edge and
/// optional exterior contour bands. The picture that answers "does a
/// distance render look better than the chaos game".
pub static IFS_DISTANCE: IfsColoringDef = IfsColoringDef {
    name: "ifs_distance",
    display_name: "Distance",
    parameters: &[
        EscapeParamDef {
            name: "edge",
            display_name: "Edge Width",
            default: 1.0,
            min: 0.0,
            max: 8.0,
            tooltip: "Antialiasing width in pixels. The set's boundary is where the \
                      distance crosses zero, so this is a real sub-pixel coverage, \
                      not a blur.",
            choices: &[],
        },
        EscapeParamDef {
            name: "bands",
            display_name: "Contour Spacing",
            default: 0.0,
            min: 0.0,
            max: 40.0,
            tooltip: "Exterior contour bands per unit of log distance. 0 = a flat \
                      exterior (the background shows through).",
            choices: &[],
        },
        EscapeParamDef {
            name: "interior",
            display_name: "Interior Palette",
            default: 0.5,
            min: 0.0,
            max: 1.0,
            tooltip: "Palette position for the set itself. Not 0 by default: most                       palettes are black at their low end, so a set drawn there is                       invisible against the background and the render reads as                       empty.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn ifs_color(res: IfsResult) -> IfsShade {
    let edge = max(cparam(0u), 1e-4);
    // Sub-pixel coverage of the set: 1 inside, 0 a pixel out. The
    // distance is already in pixels, which is what makes this the same
    // edge at every zoom.
    let cover = 1.0 - smoothstep(0.0, edge, res.distance);
    let bands = cparam(1u);
    if (bands <= 0.0) {
        return IfsShade(cparam(2u), cover);
    }
    // Log distance banding: geometric spacing, so the contours stay
    // evenly spaced as the view zooms.
    let t = log(max(res.distance, 1e-30)) * bands * 0.05;
    return IfsShade(mix(t, cparam(2u), cover), 1.0);
}
"#,
};

/// `level`: escape-time bands — nested shells of constant inverse-orbit
/// depth around the set, from the same walk (D9).
///
/// The quantity is Hepting and Hart's discrete escape time, which is
/// itself after Prusinkiewicz and Sandness; see §9 of the plan. This
/// used to claim the look was Fractint's, which nobody here has
/// checked, so it does not say that any more.
pub static IFS_LEVEL: IfsColoringDef = IfsColoringDef {
    name: "ifs_level",
    display_name: "Escape Level",
    parameters: &[
        EscapeParamDef {
            name: "scale",
            display_name: "Palette Scale",
            default: 0.12,
            min: 0.005,
            max: 2.0,
            tooltip: "Palette cycles per inverse-iteration level.",
            choices: &[],
        },
        EscapeParamDef {
            name: "offset",
            display_name: "Palette Offset",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Rotates the band colours.",
            choices: &[],
        },
        EscapeParamDef {
            name: "smooth",
            display_name: "Smooth Bands",
            default: 1.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Use the continuous residual across the annulus (smooth) or \
                      the integer level (hard bands).",
            choices: &["Hard", "Smooth"],
        },
    ],
    wgsl: r#"
fn ifs_color(res: IfsResult) -> IfsShade {
    var lvl = res.level;
    if (cparam(2u) < 0.5) {
        lvl = floor(res.level);
    }
    return IfsShade(lvl * cparam(0u) + cparam(1u), 1.0);
}
"#,
};

/// `address`: the branch taken at each level, as a base-N fraction —
/// the symbolic colouring, and the analogue of a flame's transform
/// colour. `address_mix` generalised past base 2.
///
/// This is the one that survives a deep zoom: every level adds a
/// digit, while the smooth quantities lose contrast (§2.5).
pub static IFS_ADDRESS: IfsColoringDef = IfsColoringDef {
    name: "ifs_address",
    display_name: "Branch Address",
    parameters: &[
        EscapeParamDef {
            name: "source",
            display_name: "Source",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "The full address as a base-N fraction, or just the coarsest \
                      branch's transform colour (which is what a flame would show).",
            choices: &["Address fraction", "First transform colour"],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Palette Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette cycles across the address range.",
            choices: &[],
        },
        EscapeParamDef {
            name: "reach",
            display_name: "Halo Reach",
            default: 6.0,
            min: 0.0,
            max: 200.0,
            tooltip: "How far from the set, in pixels, the colouring stays lit. \
                      The quantity is defined everywhere, but it only MEANS \
                      anything near the attractor — without this the picture is \
                      the exterior's branch partition rather than the set. \
                      0 = no fade, light the whole plane.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn ifs_color(res: IfsResult) -> IfsShade {
    var v = res.address;
    if (cparam(0u) >= 0.5) {
        v = res.color;
    }
    return IfsShade(v * cparam(1u), ifs_halo(res, cparam(2u)));
}
"#,
};

/// `p_K`: the point after the last inversion, trapped in the expanded
/// frame. The orbit-trap vocabulary, which the expansion makes
/// scale-free — the trap is the same size at every zoom depth.
pub static IFS_TRAP: IfsColoringDef = IfsColoringDef {
    name: "ifs_trap",
    display_name: "Orbit Trap",
    parameters: &[
        EscapeParamDef {
            name: "shape",
            display_name: "Trap Shape",
            default: 0.0,
            min: 0.0,
            max: 2.0,
            tooltip: "What the expanded point is measured against.",
            choices: &["Distance to centre", "Cross (axes)", "Angle"],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Palette Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette cycles per unit of trap value.",
            choices: &[],
        },
        EscapeParamDef {
            name: "reach",
            display_name: "Halo Reach",
            default: 6.0,
            min: 0.0,
            max: 200.0,
            tooltip: "How far from the set, in pixels, the colouring stays lit. \
                      The quantity is defined everywhere, but it only MEANS \
                      anything near the attractor — without this the picture is \
                      the exterior's branch partition rather than the set. \
                      0 = no fade, light the whole plane.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn ifs_color(res: IfsResult) -> IfsShade {
    let d = res.point - ifs_centre();
    let shape = i32(cparam(0u));
    var v = length(d);
    if (shape == 1) {
        v = min(abs(d.x), abs(d.y));
    } else if (shape == 2) {
        v = (ff_atan2(d.y, d.x) + 3.14159265) * 0.15915494;
    }
    return IfsShade(v * cparam(1u), ifs_halo(res, cparam(2u)));
}
"#,
};

// ====================================================================
// Registries
// ====================================================================

/// Ordered mode-D registry. **Append-only** — same contract as
/// [`super::FORMULAS`].
pub static IFS_DEFS: &[&IfsDef] = &[&IFS_FLAME, &IFS_FLAME_3D, &IFS_QUATERNION_JULIA];

/// A quaternion Julia set as a solid, needing no flame (plan §8.11,
/// step 1): `q ↦ qⁿ + c` in ℍ on a 3D slice, by the escape-time
/// distance estimate of Hart, Sandin and Kauffman (1989). The solid
/// template supplies the camera, the march, the normals, the shadows,
/// the occlusion and the cache; this supplies `ifs_walk3`.
///
/// The parameter slots the template reads are the solid flame's, in
/// its order -- levels, beam, steps, shadow, shadow sharpness,
/// occlusion -- then this set's own. `beam` means nothing here and
/// keeps its slot so `fparam(2)` is still the march.
pub static IFS_QUATERNION_JULIA: IfsDef = IfsDef {
    name: "quaternion_julia_solid",
    display_name: "Quaternion Julia (Solid)",
    solid: true,
    needs_flame: false,
    default_coloring: "ifs_level",
    presets: &[],
    parameters: &[
        EscapeParamDef {
            name: "levels",
            display_name: "Iterations",
            default: 32.0,
            min: 1.0,
            max: 256.0,
            tooltip: "How many times to apply q -> q^n + c before calling the point \
                      part of the set. The distance is taken where |q| has run past \
                      10^4, so a few beyond the bailout are spent sharpening it.",
            choices: &[],
        },
        EscapeParamDef {
            name: "beam",
            display_name: "Beam Width",
            default: 1.0,
            min: 1.0,
            max: 8.0,
            tooltip: "Unused here: a quaternion power has one inverse orbit. Kept so \
                      the march, shadow and occlusion slots line up with the flame \
                      solid's.",
            choices: &[],
        },
        EscapeParamDef {
            name: "steps",
            display_name: "March Steps",
            default: 96.0,
            min: 4.0,
            max: 512.0,
            tooltip: "How many times a ray may step before giving up.",
            choices: &[],
        },
        EscapeParamDef {
            name: "shadow",
            display_name: "Shadows",
            default: 0.7,
            min: 0.0,
            max: 1.0,
            tooltip: "How dark a traced shadow is. 0 skips the shadow march.",
            choices: &[],
        },
        EscapeParamDef {
            name: "shadow_sharpness",
            display_name: "Shadow Sharpness",
            default: 12.0,
            min: 1.0,
            max: 64.0,
            tooltip: "Penumbra width: higher is harder-edged.",
            choices: &[],
        },
        EscapeParamDef {
            name: "occlusion",
            display_name: "Occlusion Reach",
            default: 0.15,
            min: 0.0,
            max: 1.0,
            tooltip: "How far the ambient occlusion probes, as a fraction of the set.",
            choices: &[],
        },
        EscapeParamDef {
            name: "w_slice",
            display_name: "Slice Value",
            default: 0.0,
            min: -2.0,
            max: 2.0,
            tooltip: "Unused here; the quaternion Julia solid's own slice follows.",
            choices: &[],
        },
        EscapeParamDef {
            name: "cx",
            display_name: "Constant X",
            default: -1.0,
            min: -2.0,
            max: 2.0,
            tooltip: "The scalar (real) part of the Julia constant c. Bourke writes \
                      c scalar-first; his (-1, 0.2, 0, 0) is this at -1.",
            choices: &[],
        },
        EscapeParamDef {
            name: "cy",
            display_name: "Constant Y",
            default: 0.2,
            min: -2.0,
            max: 2.0,
            tooltip: "The i component of c.",
            choices: &[],
        },
        EscapeParamDef {
            name: "cz",
            display_name: "Constant Z",
            default: 0.0,
            min: -2.0,
            max: 2.0,
            tooltip: "The j component of c.",
            choices: &[],
        },
        EscapeParamDef {
            name: "cw",
            display_name: "Constant W",
            default: 0.0,
            min: -2.0,
            max: 2.0,
            tooltip: "The k component of c.",
            choices: &[],
        },
        EscapeParamDef {
            name: "power",
            display_name: "Power",
            default: 2.0,
            min: 2.0,
            max: 8.0,
            tooltip: "The n in q^n + c. 2 is the classic quadratic set.",
            choices: &[],
        },
        EscapeParamDef {
            name: "bailout",
            display_name: "Bailout",
            default: 2.0,
            min: 1.0,
            max: 8.0,
            tooltip: "A point whose orbit passes this radius is outside the set. Also \
                      the radius of the ball the camera frames.",
            choices: &[],
        },
        EscapeParamDef {
            name: "slice_axis",
            display_name: "Slice Axis",
            default: 3.0,
            min: 0.0,
            max: 3.0,
            tooltip: "Which quaternion component the 3D slice pins: 0 = scalar, \
                      1 = i, 2 = j, 3 = k. For a complex c (j = k = 0) the k slice \
                      contains the complex plane and shows the classic solid.",
            choices: &["Scalar", "i", "j", "k"],
        },
        EscapeParamDef {
            name: "w_slice",
            display_name: "Slice Value",
            default: 0.0,
            min: -2.0,
            max: 2.0,
            tooltip: "Where along the slice axis the 3D slice sits. Sweep it to walk \
                      through the 4D solid.",
            choices: &[],
        },
    ],
    wgsl: r#"
// The quaternion as (scalar, i, j, k) in a vec4's (x, y, z, w). A
// power via the polar form: q = |q| (cos a + n^ sin a) with n^ the
// unit vector part, q^n = |q|^n (cos na + n^ sin na).
fn qj_pow(q: vec4<f32>, n: f32) -> vec4<f32> {
    let mag = length(q);
    if (mag < 1e-30) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let rad = pow(mag, n);
    let ang = acos(clamp(q.x / mag, -1.0, 1.0)) * n;
    let vlen = length(q.yzw);
    let nhat = select(vec3<f32>(1.0, 0.0, 0.0), q.yzw / vlen, vlen > 1e-12);
    return vec4<f32>(rad * cos(ang), rad * sin(ang) * nhat);
}

fn ifs_walk3(delta: vec3<f32>, eps: f32) -> IfsResult {
    var res: IfsResult;
    res.distance = 0.0;
    res.level = 0.0;
    res.address = 0.0;
    res.color = 0.5;
    res.point = vec2<f32>(0.0, 0.0);
    res.escaped = 0u;
    res.depth = 0u;

    // The ball is centred on the origin, so the world point is the
    // delta plus the target's offset. f32 and absolute: no deep zoom
    // in this step (plan 8.11 Q3).
    let p3 = delta + ifs_target_offset();
    let levels = u32(clamp(fparam(0u), 1.0, 256.0));
    let c = vec4<f32>(fparam(7u), fparam(8u), fparam(9u), fparam(10u));
    let n = clamp(round(fparam(11u)), 2.0, 8.0);
    let bail = max(fparam(12u), 1.0);
    let axis = u32(clamp(fparam(13u), 0.0, 3.0));
    let slice = fparam(14u);

    // Lift to 4D: the slice axis takes the slice value and the other
    // three take the point, in order.
    var q: vec4<f32>;
    if (axis == 0u) {
        q = vec4<f32>(slice, p3.x, p3.y, p3.z);
    } else if (axis == 1u) {
        q = vec4<f32>(p3.x, slice, p3.y, p3.z);
    } else if (axis == 2u) {
        q = vec4<f32>(p3.x, p3.y, slice, p3.z);
    } else {
        q = vec4<f32>(p3.x, p3.y, p3.z, slice);
    }

    // dq is |dq_k / dq_0|, a scalar because the quaternion norm is
    // multiplicative: each step multiplies it by n |q|^(n-1) (Q1).
    var dq = 1.0;
    var mag = length(q);
    var escape_k = -1.0;
    var escape_mag = 0.0;
    var k = 0u;
    loop {
        if (k >= levels || mag > 1.0e4) {
            break;
        }
        dq = n * pow(mag, n - 1.0) * dq;
        q = qj_pow(q, n) + c;
        mag = length(q);
        k = k + 1u;
        if (escape_k < 0.0 && mag > bail) {
            escape_k = f32(k);
            escape_mag = mag;
        }
    }

    if (escape_k >= 0.0) {
        res.escaped = 1u;
        // Hart's estimate, with his half for the march's sake.
        res.distance = 0.5 * mag * log(mag) / max(dq, 1e-30);
        // The smooth escape count, rising toward the set.
        let smooth_k = escape_k + 1.0 - log(max(log(escape_mag), 1e-30) / log(bail)) / log(n);
        res.level = max(f32(levels) - smooth_k, 0.0);
        res.depth = u32(max(f32(levels) - escape_k, 0.0));
        // The binary decomposition: the escaped quaternion's azimuth
        // in the (i, j) plane.
        res.address = fract(ff_atan2(q.z, q.y) / 6.28318530718 + 0.5);
        res.point = q.yz;
    } else {
        res.level = f32(levels);
        res.depth = levels;
        res.point = q.yz;
    }
    return res;
}

fn ifs_distance_at(delta: vec3<f32>, eps: f32) -> f32 {
    return ifs_walk3(delta, eps).distance;
}

fn ifs_evaluate3(delta: vec3<f32>) -> IfsResult {
    return ifs_walk3(delta, 0.0);
}
"#,
};

/// The packing for a def that needs no flame (plan §8.11 Q3): no
/// maps, and a ball of the bailout's radius at the origin, which is
/// what frames the camera and bounds the march. Everything downstream
/// -- `set_ifs`, the globals, the keys -- works as for a flame.
pub fn pack_standalone(def: &IfsDef, escape: &crate::config::escape::EscapeConfig) -> PackedIfs {
    let radius = def
        .parameters
        .iter()
        .find(|p| p.name == "bailout")
        .map(|p| escape.formula_params.get("bailout").copied().unwrap_or(p.default) as f64)
        .unwrap_or(2.0)
        .max(1e-3);
    let ball2 = crate::scene::ifs_analysis::Ball { centre: [0.0, 0.0], radius };
    let ball3 = crate::scene::ifs_analysis::Ball { centre: [0.0, 0.0, 0.0], radius };
    let ifs = crate::scene::ifs_analysis::Ifs { maps: Vec::new(), final_map: None, frame_radius: ball2.radius, ball: ball2, aux_centre: 0.0, xaos: None };
    let ifs3 = crate::scene::ifs_analysis::Ifs { maps: Vec::new(), final_map: None, frame_radius: ball3.radius, ball: ball3, aux_centre: 0.0, xaos: None };
    let mut globals = [[0.0f32; 4]; 4];
    pack_globals(&ifs, &mut globals);
    PackedIfs {
        globals,
        rows: Vec::new(),
        ifs,
        colors: Vec::new(),
        measure: crate::scene::ifs_estimate::MeasureMaps {
            prob: Vec::new(),
            step: None,
            colour: Vec::new(),
        },
        xaos: vec![0.0; 4],
        solid: Some((ifs3, Vec::new())),
    }
}

/// The packing a config's formula wants: the flame's analysis for a
/// def that reads the flame, the def's own for one that does not.
pub fn pack_for(
    def: &IfsDef,
    config: &crate::config::FractalConfig,
    registry: &crate::variations::VariationRegistry,
) -> Option<PackedIfs> {
    if def.needs_flame {
        // The user's Extent, when set: the radius the set is cut at.
        // 0 (the default) is the measured ball.
        let extent = config
            .escape
            .formula_params
            .get("extent")
            .copied()
            .filter(|e| e.is_finite() && *e > 0.0)
            .map(|e| e as f64);
        pack_flame(&config.flame, registry, extent).ok()
    } else {
        Some(pack_standalone(def, &config.escape))
    }
}

/// Ordered mode-D coloring registry. **Append-only.**
/// `measure`: the flame's own invariant density and colour, read
/// through the inverse walk instead of sampled by a chaos game.
///
/// The other four colourings paint a DISTANCE. This paints the
/// MEASURE -- what the flame actually looks like -- and it does not
/// starve at depth, because no forward sample is drawn at the zoom.
/// See [`ifs-measure-by-inverse-walk.md`](../../docs/projects/ifs-measure-by-inverse-walk.md).
///
/// The walk hands `res.distance` the density per unit area and
/// `res.color` the palette coordinate, which is the one place mode D
/// reuses those two fields for something other than their names.
pub static IFS_MEASURE_COLORING: IfsColoringDef = IfsColoringDef {
    name: "ifs_measure",
    display_name: "Measure",
    parameters: &[
        EscapeParamDef {
            name: "cells",
            display_name: "Lookup Region",
            default: 4.0,
            min: 1.0,
            max: 64.0,
            tooltip: "How many cells of the coarse pass a lineage's region must reach \
                      before its measure is read. One is wrong -- at one cell nothing \
                      is integrated and the density reads a quarter of the truth. \
                      Sixteen suits a set of affine maps, four a curved one, whose \
                      first-order footprint outgrows the map sooner.",
            choices: &[],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Brightness",
            default: 0.0,
            min: 0.0,
            max: 64.0,
            tooltip: "Multiplies the density. ZERO means automatic: the view's own \
                      median density is divided out, so zooming does not change the \
                      exposure. Density per unit area CLIMBS as the zoom deepens -- \
                      six to seven stops over fifteen levels, at a rate set by the \
                      attractor's dimension -- so a fixed value brightens as you go in.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn ifs_color(res: IfsResult) -> IfsShade {
    // `distance` is the density and `color` the palette coordinate:
    // the measure walk reuses the two fields, which is why this
    // colouring only makes sense with it.
    return IfsShade(res.color, res.distance * cparam(1u));
}
"#,
};

pub static IFS_COLORINGS: &[&IfsColoringDef] =
    &[&IFS_DISTANCE, &IFS_LEVEL, &IFS_ADDRESS, &IFS_TRAP, &IFS_MEASURE_COLORING];

/// Look up a mode-D distance function by name. `None` = the name
/// belongs to another mode (or is unknown).
pub fn get_ifs(name: &str) -> Option<&'static IfsDef> {
    IFS_DEFS.iter().find(|f| f.name == name).copied()
}

/// Resolve the coloring for a mode-D render, falling back to the
/// def's declared default when the config still names another
/// registry's entry (the state right after a switch).
pub fn get_ifs_coloring(name: &str, def: &IfsDef) -> &'static IfsColoringDef {
    IFS_COLORINGS.iter().find(|c| c.name == name).copied().unwrap_or_else(|| {
        IFS_COLORINGS
            .iter()
            .find(|c| c.name == def.default_coloring)
            .copied()
            .unwrap_or(IFS_COLORINGS[0])
    })
}

// ====================================================================
// The map buffer (D5)
// ====================================================================

/// One map, as the shader reads it: the INVERSE affine, the smallest
/// singular value of the forward map, and the transform's colour.
///
/// The forward map is never uploaded — the walk only ever inverts, and
/// `σ_min` is the only thing it needs of the forward direction.
///
/// Eighty bytes since the nonlinear maps (plan §8.8 J7, §8.9 S1). An
/// affine row uses the first half and has `kind == 0`, which is what
/// the shader switches on; a nonlinear row's first half is the
/// POST-inverse with `1/w` folded in, its second half the
/// PRE-inverse, its `sigma_min` the constant part of the forward
/// σ_min, multiplied in the shader by the kernel's local factor, and
/// its last vec4 the kernel's parameters.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct IfsMapGpu {
    /// Row-major 2×2 of the inverse: `[a, b, c, d]`.
    pub inv_m: [f32; 4],
    /// Translation of the inverse.
    pub inv_t: [f32; 2],
    /// Smallest singular value of the FORWARD map — the per-level
    /// contraction the distance is scaled by.
    pub sigma_min: f32,
    /// The transform's colour index, for the address colouring.
    pub color: f32,
    /// A nonlinear row's pre-inverse, `[a, b, c, d]`; zero on an
    /// affine row.
    pub pre_m: [f32; 4],
    pub pre_t: [f32; 2],
    /// The kernel: 0 affine, 1 root, 2 spherical, 3 bubble.
    pub kind: f32,
    /// The branch this row follows, of the kernel's preimages.
    pub branch: f32,
    /// The kernel's parameters: a root's signed power and distance.
    pub params: [f32; 4],
    /// What the MEASURE walk needs of this map, and nothing else
    /// reads: `[probability, colour speed, 0, 0]`.
    ///
    /// **A whole `vec4`, not the two floats it uses.** The row's
    /// largest member is a `vec4`, so std430 aligns the struct to 16
    /// and rounds its stride up; at 88 bytes the shader would stride
    /// 96 while Rust packed 88, and every row after the first would
    /// be misread. 80 works today for exactly that reason. The two
    /// spare slots are the change's own margin.
    ///
    /// The probability is a TRANSFORM's, divided by its forward
    /// branch count for a root and NOT for a bubble --
    /// [`crate::scene::ifs_estimate::MeasureMaps`] is where that
    /// lives and where it is explained.
    pub measure: [f32; 4],
}

/// The whole-IFS constants, packed into the `fdata` block the escape
/// params already carry (mode A uses it for CPU-derived formula data,
/// mode B not at all).
///
/// Layout, one `vec4` each:
/// 0. `centre.x, centre.y, radius, map_count`
/// 1. `mean_sigma_min, handover_level, seed_count, addr_scale` — the
///    last three filled in by [`pack_seeds`]
/// 2, 3. reserved
///
/// The FINAL transform is not here. It used to be, because the shader
/// applied its inverse before walking; the CPU's seeding walk applies
/// it now, so by the time the shader starts it is already accounted
/// for and there is nothing to send.
pub fn pack_globals(ifs: &Ifs2, out: &mut [[f32; 4]]) {
    if out.len() < 4 {
        return;
    }
    out[0] = [
        ifs.ball.centre[0] as f32,
        ifs.ball.centre[1] as f32,
        ifs.ball.radius as f32,
        ifs.maps.len() as f32,
    ];
    let mean = if ifs.maps.is_empty() {
        0.5
    } else {
        ifs.maps.iter().map(|m| m.sigma_min).sum::<f64>() / ifs.maps.len() as f64
    };
    out[1] = [mean as f32, 0.0, 0.0, 0.0];
    // x: the beam's ranking key -- 1 for sigma-weighted, which the
    // CPU walk chooses when every map is an inversion. See
    // `ifs_estimate::RankKey::Auto`; the GPU must rank the way the
    // CPU that seeds it ranks, or the handover beam is not the beam
    // the walk would have kept.
    let weighted = crate::scene::ifs_estimate::resolved_key(
        ifs,
        crate::scene::ifs_estimate::RankKey::Auto,
    ) == crate::scene::ifs_estimate::RankKey::Weighted;
    out[2] = [0.0; 4];
    // fdata[3].w: zero-padded in both layouts (the solid's forward
    // vector is a vec3 there), so it is free in both. fdata[2] was the
    // first choice and is the solid's camera eye -- a flag read from
    // `eye.x > 0.5` ranked every solid whose camera sat right of
    // x = 0.5 by the wrong key, caught by the solid agreement gate.
    out[3] = [0.0, 0.0, 0.0, if weighted { 1.0 } else { 0.0 }];
}

/// One 3D map, as the marcher reads it: the INVERSE affine, the
/// smallest singular value of the forward map, and the transform's
/// colour.
///
/// Sixty-four bytes against the planar row's thirty-two, because a
/// 3×3 and a translation is twelve floats where a 2×2 and one is six.
/// The rows are a storage buffer, so the stride is the struct's and
/// nothing else has to agree with it — unlike the records buffer,
/// which mode A and mode D share.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct IfsMap3Gpu {
    /// Row `i` of the inverse 3×3 in `xyz`, and the translation's
    /// `i`-th component in `w`. On a nonlinear row (plan §8.11 step
    /// 2) this is the POST-inverse with `1/w` folded in.
    ///
    /// Eight `vec4`s and no `vec3` anywhere, which is deliberate. A
    /// `vec3<f32>` aligns to SIXTEEN bytes in WGSL and to four in
    /// Rust, so the obvious struct — three padded rows, a `vec3`
    /// translation, two scalars — is eighty bytes on one side and
    /// ninety-six on the other. The shader then reads each map from
    /// the wrong offset and the render comes out as noise that still
    /// looks vaguely like something, which is how this was found.
    pub rows: [[f32; 4]; 3],
    /// `σ_min` of the forward map (its constant part on a nonlinear
    /// row), the transform's colour, the kind (0 affine, 1 julia3D,
    /// 2 julia3Dz) and the signed power.
    pub extra: [f32; 4],
    /// A nonlinear row's pre-inverse, as `rows`; zero on an affine row.
    pub pre: [[f32; 4]; 3],
    /// Padding to one hundred and twenty-eight bytes.
    pub extra2: [f32; 4],
}

/// The 3D rows, in the flame's transform order.
pub fn pack_maps3(ifs: &Ifs3, colors: &[f32]) -> Vec<IfsMap3Gpu> {
    let rows_of = |a: &Affine3| -> [[f32; 4]; 3] {
        let row = |i: usize| [a.m[i][0] as f32, a.m[i][1] as f32, a.m[i][2] as f32, a.t[i] as f32];
        [row(0), row(1), row(2)]
    };
    ifs.maps
        .iter()
        .map(|m| {
            let color = colors.get(m.transform_index).copied().unwrap_or(0.0);
            match m.inverse {
                Map3::Affine(inv) => IfsMap3Gpu {
                    rows: rows_of(&inv),
                    extra: [m.sigma_min as f32, color, 0.0, 0.0],
                    pre: [[0.0; 4]; 3],
                    extra2: [0.0; 4],
                },
                Map3::NonlinearInverse(r) | Map3::Nonlinear(r) => {
                    use crate::scene::ifs_analysis::Kernel3;
                    let scale = 1.0 / r.w;
                    let mut post = r.post_inv;
                    for i in 0..3 {
                        for j in 0..3 {
                            post.m[i][j] *= scale;
                        }
                        post.t[i] *= scale;
                    }
                    // A quaternion row folds c's xyz into the
                    // pre-inverse's translation -- the kernel's inverse
                    // is v^n + c, and pre_inv(u + c_xyz) is one affine
                    // -- and carries c_w and the distance in extra2.
                    let (kind, pre, extra2) = match r.kernel {
                        // z is the factor on the carried w before the
                        // kernel: one for a kernel that passes it through.
                        Kernel3::Root3 { .. } => (1.0, r.pre_inv, [0.0f32, 0.0, 1.0, 0.0]),
                        Kernel3::RootZ3 { .. } => (2.0, r.pre_inv, [0.0, 0.0, 1.0, 0.0]),
                        Kernel3::Quaternion { d, c, depth, .. } => {
                            let mut pre = r.pre_inv;
                            let shift = pre.apply([c[0], c[1], c[2]]);
                            let origin = pre.apply([0.0; 3]);
                            for i in 0..3 {
                                pre.t[i] += shift[i] - origin[i];
                            }
                            // w: the projection, 1 for Depth -- which
                            // coordinate the carried scalar reassembles
                            // into (step 4).
                            let proj = if depth { 1.0f32 } else { 0.0 };
                            (3.0, pre, [c[3] as f32, d as f32, scale as f32, proj])
                        }
                    };
                    IfsMap3Gpu {
                        rows: rows_of(&post),
                        extra: [m.sigma_min as f32, color, kind, r.kernel.power() as f32],
                        pre: rows_of(&pre),
                        extra2,
                    }
                }
            }
        })
        .collect()
}

/// An analysed flame, ready for the shader: the whole-IFS constants
/// and one row per map.
///
/// Built on the CPU whenever the flame changes and handed to the
/// renderer through [`super::EscapeRenderer::set_ifs`], so a video
/// loop analyses once per frame rather than once per pixel.
#[derive(Debug, Clone, PartialEq)]
pub struct PackedIfs {
    /// The transition graph, flat, as [`pack_xaos`] lays it out. One
    /// zero element for a flame without xaos.
    pub xaos: Vec<f32>,
    pub globals: [[f32; 4]; 4],
    pub rows: Vec<IfsMapGpu>,
    /// The f64 analysis the rows were packed from.
    ///
    /// Kept because the reference orbit is walked on the CPU at a
    /// precision the packed f32 rows cannot express — seeding from the
    /// rows would cap the zoom at the rows' own rounding, which is the
    /// wall the seeding exists to move.
    pub ifs: Ifs2,
    /// Transform colours, in map order.
    pub colors: Vec<f32>,
    /// The per-map probability and colour the MEASURE walk needs.
    ///
    /// Kept beside the rows because the handover's PREFIX has to be
    /// folded on the CPU, where a seed's address is an exact list of
    /// branches -- the packed address is a base-N fraction and loses
    /// the tail.
    pub measure: crate::scene::ifs_estimate::MeasureMaps,
    /// The SOLID analysis and its rows, when the flame also qualifies
    /// in three dimensions.
    ///
    /// Separate because qualifying is a different question per
    /// dimension and most flames answer it differently: an
    /// Apophysis-style transform is a perfectly good planar map and
    /// has unit scale in z, so it makes a stack of planes rather than
    /// a solid. Phase 0 measured none of 34 candidates qualifying.
    pub solid: Option<(Ifs3, Vec<IfsMap3Gpu>)>,
}

/// Compare two packed IFSs **by bytes**, not by `PartialEq`.
///
/// The caller re-analyses the flame every frame and asks whether
/// anything changed; a "yes" marks the escape image dirty. Float
/// equality answers yes forever the moment a single NaN is in the
/// packed data — a transform whose colour is NaN is enough — and the
/// app then re-renders a band every frame and never settles, which is
/// indistinguishable from the engine simply being far too slow.
///
/// Bytes compare NaN to itself as equal, which is the question
/// actually being asked: is this the same buffer we already uploaded?
pub fn packed_bytes_eq(a: Option<&PackedIfs>, b: Option<&PackedIfs>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            bytemuck::bytes_of(&x.globals) == bytemuck::bytes_of(&y.globals)
                && bytemuck::cast_slice::<IfsMapGpu, u8>(&x.rows)
                    == bytemuck::cast_slice::<IfsMapGpu, u8>(&y.rows)
        }
        _ => false,
    }
}

/// Analyse a flame and pack it, or say why it does not qualify.
///
/// The 2D criterion: mode D is a plane render in phase 1, and a 3D
/// flame with `preserve_z` off is a planar IFS anyway (see
/// [`crate::scene::ifs_analysis::Space`]).
pub fn pack_flame(
    flame: &crate::scene::transforms::Flame,
    registry: &crate::variations::VariationRegistry,
    extent: Option<f64>,
) -> Result<PackedIfs, Vec<crate::scene::ifs_analysis::Disqualification>> {
    let colors: Vec<f32> = flame.transforms.iter().map(|t| t.color).collect();
    // Either analysis may fail on its own: a flame that is a planar
    // IFS need not be a solid one, and since plan 8.11 step 2 a flame
    // of 3D roots is a solid one and not a planar one. Both failing
    // is what "does not qualify" means; the planar reasons are the
    // ones reported, as the panel's criterion is the planar one.
    let planar = crate::scene::ifs_analysis::analyse_2d(flame, registry)
        .map(|ifs| match extent {
            Some(r) => ifs.with_extent(r),
            None => ifs,
        });
    let solid = crate::scene::ifs_analysis::analyse_3d(flame, registry)
        .ok()
        .map(|ifs3| {
            let ifs3 = match extent {
                Some(r) => ifs3.with_extent(r),
                None => ifs3,
            };
            let rows3 = pack_maps3(&ifs3, &colors);
            (ifs3, rows3)
        });
    let ifs = match (planar, &solid) {
        (Ok(ifs), _) => ifs,
        (Err(why), None) => return Err(why),
        // A solid with no planar reading: an empty plane, whose zero
        // map count draws nothing in the planar formula, and whose
        // ball is the solid's shadow so the view has something to
        // frame.
        (Err(_), Some((ifs3, _))) => crate::scene::ifs_analysis::Ifs {
            maps: Vec::new(),
            final_map: None,
            ball: crate::scene::ifs_analysis::Ball {
                centre: [ifs3.ball.centre[0], ifs3.ball.centre[1]],
                radius: ifs3.ball.radius,
            },
            frame_radius: ifs3.frame_radius,
            aux_centre: 0.0,
            // No maps, so no transitions: the plane is empty here and
            // the solid's own graph is on `ifs3`.
            xaos: None,
        },
    };
    let mut globals = [[0.0f32; 4]; 4];
    pack_globals(&ifs, &mut globals);
    let measure = crate::scene::ifs_estimate::MeasureMaps::of(&ifs, flame);
    let rows = pack_maps(&ifs, &colors, Some(&measure));
    let xaos = pack_xaos(&ifs, &measure);
    Ok(PackedIfs { globals, rows, ifs, colors, measure, xaos, solid })
}

/// The six kernels' inverse Jacobians, and the map-level composition,
/// as WGSL.
///
/// Its own const rather than text inside `IFS_TEMPLATE`, because it
/// depends on nothing but the `IfsMapGpu` rows and `ff_atan2` -- so
/// `the_shader_jacobians_are_the_cpu_ones` can compile it against a
/// twenty-line harness and check the arithmetic, instead of standing
/// up the whole walk to reach it.
///
/// **The measure walk needs this and two cheaper ways round it were
/// measured worse** (measure plan §5i): carrying the pixel's centre
/// and two edge neighbours as a secant parallelogram reads 0.617 of
/// the truth on a grand julian, and a local central difference reads
/// 0.790 on a 6:1:1 gasket. A linearisation that is LOCAL is the
/// point of it.
///
/// Transcribed from [`crate::scene::ifs_analysis::Kernel::inverse_jacobian`],
/// which `the_kernels_jacobians_are_the_derivative` gates against
/// central differences to 3.5e-5.
///
/// WGSL matrices are COLUMN-major: `mat2x2(col0, col1)` and `m[c][r]`,
/// so a row-major `J[i][j] = ∂u_i/∂q_j` packs as
/// `mat2x2(vec2(J00, J10), vec2(J01, J11))`. Getting that backwards
/// transposes every derivative and still renders a picture.
pub(crate) const IFS_JACOBIAN: &str = r#"
// Bubble's radial scale AND its derivative in x = |v|^2, along the
// row's branch, written so nothing cancels (Kernel::bubble_scale):
//
//   inner:  s = 2/(1 + root)        ds = 1/(root*(1 + root)^2)
//   outer:  s = 2(1 + root)/x       ds = -(1 + root)^2/(x^2*root)
//
// with root = sqrt(1 - x). The inner branch's naive form is a
// difference of two numbers either side of 2 and keeps only the digits
// x is below 1 -- three of f32's seven at |v| = 1e-4 -- and its
// derivative then cancels what is left. Measured at 102% wrong before
// this form.
fn ifs_bubble_dscale(r2: f32, branch: f32) -> f32 {
    let root = sqrt(max(1.0 - r2, 0.0));
    let up = 1.0 + root;
    let safe_root = max(root, 1e-30);
    let inner = 1.0 / (safe_root * up * up);
    let x = max(r2, 1e-30);
    let outer = -(up * up) / (x * x * safe_root);
    return select(outer, inner, branch == 0.0);
}

// The kernel's inverse Jacobian at v, along the row's branch.
fn ifs_kernel_jacobian(i: u32, v: vec2<f32>) -> mat2x2<f32> {
    let kind = ifs_maps[i].kind;
    let r2 = dot(v, v);
    if (kind == 1.0) {
        // root: u = |v|^m e^{i n arg v}, m = |n|/d. In the radial and
        // tangential frames the derivative is diag(m, n)*|v|^(m-1),
        // read out of the frame at v and into the one at u:
        // R(psi) diag(m, n) R(-phi).
        let n = ifs_maps[i].params.x;
        let d = ifs_maps[i].params.y;
        let rho = sqrt(max(r2, 1e-30));
        let m = abs(n) / d;
        let scale = pow(rho, m - 1.0);
        let phi = ff_atan2(v.y, v.x);
        let psi = n * phi;
        let cp = cos(phi);
        let sp = sin(phi);
        let cs = cos(psi);
        let ss = sin(psi);
        let a = m * scale;
        let b = n * scale;
        return mat2x2<f32>(
            vec2<f32>(cs * a * cp + ss * b * sp, ss * a * cp - cs * b * sp),
            vec2<f32>(cs * a * sp - ss * b * cp, ss * a * sp + cs * b * cp),
        );
    }
    if (kind == 2.0) {
        // spherical: u = v/|v|^2, J = (I - 2 v v^T/|v|^2)/|v|^2.
        let s = 1.0 / max(r2, 1e-30);
        let off = s * (-2.0 * v.x * v.y * s);
        return mat2x2<f32>(
            vec2<f32>(s * (1.0 - 2.0 * v.x * v.x * s), off),
            vec2<f32>(off, s * (1.0 - 2.0 * v.y * v.y * s)),
        );
    }
    if (kind == 4.0) {
        // hemisphere: u = v*t, t = (1 - |v|^2)^(-1/2); J = t I + t^3 v v^T.
        let t = 1.0 / sqrt(max(1.0 - r2, 1e-30));
        let t3 = t * t * t;
        let off = t3 * v.x * v.y;
        return mat2x2<f32>(
            vec2<f32>(t + t3 * v.x * v.x, off),
            vec2<f32>(off, t + t3 * v.y * v.y),
        );
    }
    if (kind == 5.0) {
        // disc: the chain rule through (|v|, phi), with phi the angle
        // of v from +y, r = phi/pi + branch the ring and the branch's
        // parity the sign of theta.
        let pi = 3.14159265358979;
        let rho = sqrt(max(r2, 1e-30));
        let mb = ifs_maps[i].branch;
        let phi = ff_atan2(v.x, v.y);
        let r = phi / pi + mb;
        let even = fract(mb * 0.5) == 0.0;
        let sgn = select(-1.0, 1.0, even);
        let theta = sgn * pi * rho;
        let st = sin(theta);
        let ct = cos(theta);
        let du_drho = vec2<f32>(r * ct * sgn * pi, -r * st * sgn * pi);
        let du_dphi = vec2<f32>(st / pi, ct / pi);
        let drho = v / rho;
        let dphi = vec2<f32>(v.y, -v.x) / max(r2, 1e-30);
        return mat2x2<f32>(
            vec2<f32>(
                du_drho.x * drho.x + du_dphi.x * dphi.x,
                du_drho.y * drho.x + du_dphi.y * dphi.x,
            ),
            vec2<f32>(
                du_drho.x * drho.y + du_dphi.x * dphi.y,
                du_drho.y * drho.y + du_dphi.y * dphi.y,
            ),
        );
    }
    if (kind == 6.0) {
        // blob: u = P v / s(theta), P the swap; J = P/s + (P v) grad(1/s),
        // grad(1/s) = -(s'/s^2) grad(theta), grad(theta) = (-v_y, v_x)/|v|^2.
        let high = ifs_maps[i].params.x;
        let low = ifs_maps[i].params.y;
        let waves = ifs_maps[i].params.z;
        let theta = ff_atan2(v.y, v.x);
        let sc = low + (high - low) * 0.5 * (sin(waves * theta) + 1.0);
        let ds = (high - low) * 0.5 * waves * cos(waves * theta);
        let pv = vec2<f32>(v.y, v.x);
        let g = -ds / max(sc * sc, 1e-30);
        let grad = vec2<f32>(g * (-v.y / max(r2, 1e-30)), g * (v.x / max(r2, 1e-30)));
        let inv_s = 1.0 / sc;
        return mat2x2<f32>(
            vec2<f32>(pv.x * grad.x, inv_s + pv.y * grad.x),
            vec2<f32>(inv_s + pv.x * grad.y, pv.y * grad.y),
        );
    }
    // bubble: u = v*s(|v|^2), J = s I + 2 s' v v^T.
    let s = ifs_bubble_scale(r2, ifs_maps[i].branch);
    let ds = ifs_bubble_dscale(r2, ifs_maps[i].branch);
    let off = 2.0 * ds * v.x * v.y;
    return mat2x2<f32>(
        vec2<f32>(s + 2.0 * ds * v.x * v.x, off),
        vec2<f32>(off, s + 2.0 * ds * v.y * v.y),
    );
}

// The whole map's inverse Jacobian at q: the shader's chain is
// q -> inv affine (the post-inverse with 1/w folded in) -> kernel
// inverse -> pre affine, so the derivative is pre * J_kernel * inv.
// An affine row has no kernel and is just the one matrix.
fn ifs_map_jacobian(i: u32, q: vec2<f32>) -> mat2x2<f32> {
    let m = ifs_maps[i].inv_m;
    // Row-major [a, b, c, d] as a COLUMN-major mat2x2.
    let am = mat2x2<f32>(vec2<f32>(m.x, m.z), vec2<f32>(m.y, m.w));
    if (ifs_maps[i].kind == 0.0) {
        return am;
    }
    let t = ifs_maps[i].inv_t;
    let v = vec2<f32>(m.x * q.x + m.y * q.y + t.x, m.z * q.x + m.w * q.y + t.y);
    let p = ifs_maps[i].pre_m;
    let bm = mat2x2<f32>(vec2<f32>(p.x, p.z), vec2<f32>(p.y, p.w));
    return bm * (ifs_kernel_jacobian(i, v) * am);
}
"#;

/// The MEASURE walk: the flame's invariant density and colour at one
/// pixel, read through the inverse walk.
///
/// Transcribed from [`crate::scene::ifs_estimate::estimate_measure`],
/// which `the_measure_agrees_with_the_chaos_game` holds to the chaos
/// game's own answer on five fixtures covering every kernel class.
///
/// Spliced only when the measure colouring is selected, so every
/// other mode-D shader is byte-identical without it.
///
/// **It starts from the handover's seed 0 and is correct only at
/// handover level 0**, which is what a shallow zoom takes. The seeds
/// carry a position and a basis but not the probability or the colour
/// accumulators of the prefix that reached them, so a deeper handover
/// would drop those three numbers. Carrying them is the deep-zoom
/// follow-on and is three more floats on `Seed`; the gate forces
/// level 0 through `EscapeRenderer::ifs_force_level`.
pub(crate) const IFS_MEASURE: &str = r#"
// The coarse pass: `ifs::pack_coarse`'s header and grid. The reader is
// `ifs::read_coarse` transcribed, and
// `the_packed_coarse_grid_is_the_measure_it_came_from` is what keeps
// the two from drifting.
@group(1) @binding(3) var<storage, read> ifs_coarse: array<vec2<f32>>;

fn ifs_coarse_at(q: vec2<f32>) -> vec2<f32> {
    let len = arrayLength(&ifs_coarse);
    if (len < 2u) {
        return vec2<f32>(0.0, 0.5);
    }
    let res = ifs_coarse[0].x;
    let radius = ifs_coarse[0].y;
    let centre = ifs_coarse[1];
    if (!(res >= 1.0) || !(radius > 0.0)) {
        return vec2<f32>(0.0, 0.5);
    }
    let cell = 2.0 * radius / res;
    let f = (q - (centre - vec2<f32>(radius, radius))) / cell;
    if (!(f.x >= 0.0) || !(f.y >= 0.0) || !(f.x < res) || !(f.y < res)) {
        return vec2<f32>(0.0, 0.5);
    }
    let i = 2u + u32(f.y) * u32(res) + u32(f.x);
    if (i >= len) {
        return vec2<f32>(0.0, 0.5);
    }
    return ifs_coarse[i];
}

// One lineage: its point, the composed Jacobian as a row-major
// [m00, m01, m10, m11], its probability, and the two running numbers
// the colour fold needs instead of an address -- see
// `estimate_measure` for why the reversed flam3 fold accumulates
// forward.
struct IfsMLive {
    a: vec2<f32>,
    m: vec4<f32>,
    p: f32,
    hp: f32,
    cacc: f32,
    // The map this lineage took last, or `IFS_NO_LAST`. Under xaos it
    // decides both which children are admissible and what each one's
    // probability is, which is a Markov chain rather than a product of
    // independent draws (`ifs-general.md` D4).
    last: u32,
};

fn ifs_m_det(m: vec4<f32>) -> f32 {
    return abs(m.x * m.w - m.y * m.z);
}

// The measure over the region the composed Jacobian describes, and the
// palette coordinate over the SAME samples weighted by it. Reading
// them differently hands a lineage whose footprint straddles a
// populated cell but whose centre sits in an empty one a positive
// weight and the palette's mid-grey fallback.
fn ifs_m_look(l: IfsMLive) -> vec2<f32> {
    var acc = 0.0;
    var col = 0.0;
    for (var sy = 0u; sy < 4u; sy = sy + 1u) {
        for (var sx = 0u; sx < 4u; sx = sx + 1u) {
            let s = (f32(sx) + 0.5) * 0.25 - 0.5;
            let t = (f32(sy) + 0.5) * 0.25 - 0.5;
            let at = l.a + vec2<f32>(l.m.x * s + l.m.y * t, l.m.z * s + l.m.w * t);
            let c = ifs_coarse_at(at);
            acc = acc + c.x;
            col = col + c.x * c.y;
        }
    }
    let d = acc / 16.0;
    return vec2<f32>(d, select(0.5, col / acc, acc > 0.0));
}

// Returns (density per unit area, palette coordinate).
fn ifs_measure(uv: vec2<f32>, cells: f32) -> vec4<f32> {
    let n = ifs_count();
    let beam = u32(clamp(fparam(1u), 1.0, f32(IFS_MAX_BEAM)));
    let max_levels = u32(clamp(fparam(0u), 1.0, 256.0));
    let c = ifs_centre();
    let radius = ifs_radius();

    // The region must reach `cells` coarse cells before the walk
    // reads the measure. One cell is WRONG and was the first version:
    // at one cell neither a point sample nor a footprint integrates
    // anything and the estimator reads 0.24 to 0.89 of the truth.
    let cres = select(1.0, ifs_coarse[0].x, arrayLength(&ifs_coarse) >= 2u);
    let crad = select(1.0, ifs_coarse[0].y, arrayLength(&ifs_coarse) >= 2u);
    let cpx = 2.0 * crad / max(cres, 1.0);
    let want = cpx * cpx * max(cells, 1.0);

    // Start from the HANDOVER, every seed of it.
    //
    // Each seed is a lineage the CPU walked in BigFloat: its position
    // is where that lineage reached, its basis the composed Jacobian
    // with the view folded in -- so `basis/(W,H)` is the preimage of
    // one pixel at that level, which is exactly the `m` this walk
    // carries. And the three the packer folded from its address are
    // what the lineage has ACCUMULATED: the product of its branch
    // probabilities, and the two running numbers of the colour fold.
    //
    // Without those three a deeper handover silently drops the whole
    // prefix, which is why this used to be correct only at level 0.
    let w = max(f32(params.width), 1.0);
    let h = max(f32(params.height), 1.0);
    var live: array<IfsMLive, IFS_MAX_BEAM>;
    var live_count = 0u;
    let nseed = min(max(ifs_seed_count(), 1u), IFS_MAX_BEAM);
    var pixel_area = 0.0;
    for (var j = 0u; j < nseed; j = j + 1u) {
        let sa = ifs_seed(j, 0u);
        let sb = ifs_seed(j, 1u);
        let sd = ifs_seed(j, 3u);
        let sf = ifs_seed(j, 5u);
        let m0 = vec4<f32>(sa.z / w, sa.w / h, sb.x / w, sb.y / h);
        let se = ifs_seed(j, 4u);
        let uu = uv.x * uv.x;
        let uvv = uv.x * uv.y;
        let vv = uv.y * uv.y;
        live[live_count].a = vec2<f32>(
            sa.x + sa.z * uv.x + sa.w * uv.y + se.x * uu + se.z * uvv + sf.x * vv,
            sa.y + sb.x * uv.x + sb.y * uv.y + se.y * uu + se.w * uvv + sf.y * vv,
        );
        live[live_count].m = m0;
        live[live_count].p = sd.w;
        live[live_count].hp = sf.z;
        live[live_count].cacc = sf.w;
        let mfw = bitcast<u32>(ifs_seed(j, 2u).w) >> 8u;
        live[live_count].last = select(IFS_NO_LAST, mfw - 1u, mfw != 0u);
        if (sd.w > 0.0) {
            live_count = live_count + 1u;
        }
    }
    // The PIXEL's own area, from the view -- NOT from a seed's basis.
    //
    // A seed's basis is the view composed with the prefix's Jacobians,
    // so at a deep handover it has already been expanded by the walk.
    // Dividing by it cancels the prefix out of every determinant and
    // the density comes back scaled by it: on a dragon, whose inverse
    // doubles area per level, a handover at level 1 read exactly 2x.
    // `params.span` is the view and nothing else.
    pixel_area = max(abs(params.span.x * params.span.y) / (w * h), 1e-30);

    var acc = 0.0;
    var acc_col = 0.0;
    var naddr = 0u;
    var kmax = 0u;

    for (var k = 0u; k < max_levels; k = k + 1u) {
        if (live_count == 0u) {
            break;
        }
        var next: array<IfsMLive, IFS_MAX_BEAM>;
        var keys: array<f32, IFS_MAX_BEAM>;
        var next_count = 0u;

        for (var ci = 0u; ci < live_count; ci = ci + 1u) {
            let l = live[ci];
            let area = ifs_m_det(l.m);
            if (area >= want) {
                let lk = ifs_m_look(l);
                let wgt = l.p * lk.x * area / pixel_area;
                if (wgt > 0.0) {
                    naddr = naddr + 1u;
                    kmax = max(kmax, k);
                    acc = acc + wgt;
                    acc_col = acc_col + wgt * (lk.y * l.hp + l.cacc);
                }
                continue;
            }
            for (var bi = 0u; bi < n; bi = bi + 1u) {
                if (!ifs_admits(bi, l.last)) {
                    continue;
                }
                let q2 = ifs_inv_point(bi, l.a);
                if (!(abs(q2.x) <= 1e30) || !(abs(q2.y) <= 1e30)) {
                    continue;
                }
                // Outside the ball is outside the attractor, and stays
                // outside under every further inverse: an exact prune.
                if (ifs_radius2(q2, c) > radius * 1.000001) {
                    continue;
                }
                let j = ifs_map_jacobian(bi, l.a);
                // j is column-major: j[0][0]=J00, j[1][0]=J01,
                // j[0][1]=J10, j[1][1]=J11. Child = J * m, row-major.
                let m2 = vec4<f32>(
                    j[0][0] * l.m.x + j[1][0] * l.m.z,
                    j[0][0] * l.m.y + j[1][0] * l.m.w,
                    j[0][1] * l.m.x + j[1][1] * l.m.z,
                    j[0][1] * l.m.y + j[1][1] * l.m.w,
                );
                let a2 = ifs_m_det(m2);
                if (!(a2 > 0.0) || !(a2 <= 1e30)) {
                    continue;
                }
                // Under xaos the step's probability depends on what
                // came before it, and the graph holds the same
                // branch-corrected numbers the map row does. Without
                // one, `ifs_xaos_step` returns a negative and the row
                // stands.
                var pr = ifs_xaos_step(bi, l.last);
                if (pr < 0.0) {
                    pr = ifs_maps[bi].measure.x;
                }
                let sp = ifs_maps[bi].measure.y;
                var child: IfsMLive;
                child.a = q2;
                child.m = m2;
                child.p = l.p * pr;
                // `g_i` is weighted by the product over the prefix
                // BEFORE this map.
                child.cacc = l.cacc
                    + ifs_maps[bi].color * (1.0 - sp) * 0.5 * l.hp;
                child.hp = l.hp * (1.0 + sp) * 0.5;
                child.last = bi;
                if (!(child.p > 0.0)) {
                    continue;
                }

                // The answer is a SUM, so a pruned lineage is lost
                // from it: keep the largest contributions.
                let key = child.p * a2 * ifs_coarse_at(q2).x;
                if (next_count < beam) {
                    next[next_count] = child;
                    keys[next_count] = key;
                    next_count = next_count + 1u;
                } else {
                    var worst = 0u;
                    for (var t = 1u; t < beam; t = t + 1u) {
                        if (keys[t] < keys[worst]) {
                            worst = t;
                        }
                    }
                    if (key > keys[worst]) {
                        next[worst] = child;
                        keys[worst] = key;
                    }
                }
            }
        }
        // Order the survivors by key, descending, as the CPU
        // reference does when it sorts and truncates.
        //
        // Not cosmetic. On a set whose maps are alike -- a dragon, a
        // gasket -- every lineage has the SAME key, and which members
        // of a tied set survive then depends on the order the next
        // level meets them in. Left unordered the shader kept
        // different ones and read 2.37x the CPU's density on a gasket
        // and a colour 0.72 out on a dragon, while a julia dust, whose
        // unequal powers make the keys discriminate, was exact to five
        // decimals either way.
        for (var t = 0u; t + 1u < next_count; t = t + 1u) {
            var best = t;
            for (var u = t + 1u; u < next_count; u = u + 1u) {
                if (keys[u] > keys[best]) {
                    best = u;
                }
            }
            if (best != t) {
                let kt = keys[t];
                keys[t] = keys[best];
                keys[best] = kt;
                let nt = next[t];
                next[t] = next[best];
                next[best] = nt;
            }
        }
        for (var t = 0u; t < next_count; t = t + 1u) {
            live[t] = next[t];
        }
        live_count = next_count;
    }

    return vec4<f32>(acc, select(0.5, acc_col / acc, acc > 0.0), f32(naddr), f32(kmax));
}
"#;

/// Render the flame's own measure over its ball -- D1's coarse pass,
/// from the renderer that draws every other flame rather than from a
/// chaos game written for a test.
///
/// **The palette is an inverse-sRGB grey ramp, and that is the
/// trick.** The chaos game plots `srgb_to_linear(palette(c))`, which
/// is `palette(c)^2.2`, and the accumulator keeps the
/// density-weighted MEAN of it. A ramp storing `t^(1/2.2)` therefore
/// plots exactly `c`, so the accumulator's red channel comes back as
/// the mean palette coordinate and needs no second render or engine
/// change to recover.
///
/// The framing follows `world_to_pixel`: it maps
/// `(p - pan)·zoom·min(w,h)/4` about the centre, so a view spanning
/// the ball exactly is `pan = ball.centre`, `zoom = 2/radius`.
///
/// `res` must be a multiple of 16 so the readback's rows are already
/// 256-byte aligned.
///
/// Returns `None` if the device cannot hold the readback.
#[allow(clippy::too_many_arguments)]
pub fn coarse_measure_for(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    flame: &crate::scene::transforms::Flame,
    ball_centre: [f64; 2],
    ball_radius: f64,
    res: u32,
    batches: u32,
) -> Option<crate::scene::ifs_estimate::CoarseMeasure> {
    use crate::scene::palette::{ColorStop, Palette};
    if res == 0 || res % 16 != 0 || !(ball_radius > 0.0) {
        return None;
    }
    // `t^(1/2.2)` at enough stops that the texture's interpolation
    // follows the curve rather than a chord.
    let ramp = Palette::new(
        "measure ramp",
        (0..64)
            .map(|i| {
                let t = i as f32 / 63.0;
                let v = t.powf(1.0 / 2.2);
                ColorStop { position: t, color: [v, v, v] }
            })
            .collect(),
    );

    let mut config = crate::config::FractalConfig::default();
    config.flame = flame.clone();
    config.render_mode = crate::scene::transforms::RenderMode::TwoD;
    config.pan_x = ball_centre[0] as f32;
    config.pan_y = ball_centre[1] as f32;
    config.zoom = (2.0 / ball_radius) as f32;
    config.rotation = 0.0;
    config.color_mode = crate::scene::palette::ColorMode::Palette;

    let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        res,
        res,
        &config.flame,
        config.palette_size,
    );
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("coarse measure load"),
    });
    renderer.load_config(device, &mut enc, queue, &config, &ramp, 256, 20);
    queue.submit(std::iter::once(enc.finish()));

    const WORKGROUPS: u32 = 256;
    let mut total: u64 = 0;
    for b in 0..batches.max(1) {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("coarse measure batch"),
        });
        let n = renderer.compute_pass(
            &mut enc, queue, device, WORKGROUPS, 256, 20,
            config.zoom, config.pan_x, config.pan_y, config.rotation,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config.speed_factor,
            b == 0, b == 0,
        );
        total += n;
        renderer.accumulate_pass(&mut enc, queue, device, n);
        queue.submit(std::iter::once(enc.finish()));
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    }

    // Read the accumulator: rgb the density-weighted mean colour --
    // here the palette coordinate -- and a the raw hit count.
    let row = (res as u64) * 16;
    let size = row * res as u64;
    if size > device.limits().max_buffer_size {
        renderer.destroy();
        return None;
    }
    let stage = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("coarse measure readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("coarse measure readback"),
    });
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: renderer.accumulation_texture(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &stage,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row as u32),
                rows_per_image: Some(res),
            },
        },
        wgpu::Extent3d { width: res, height: res, depth_or_array_layers: 1 },
    );
    queue.submit(std::iter::once(enc.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    stage.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    if rx.recv().ok()?.is_err() {
        renderer.destroy();
        return None;
    }
    let n = (res * res) as usize;
    let mut hits = vec![0u32; n];
    let mut palette_sum = vec![0.0f64; n];
    {
        let data = stage.slice(..).get_mapped_range();
        let px: &[[f32; 4]] = bytemuck::cast_slice(&data);
        for i in 0..n {
            let a = px[i][3];
            if a > 0.0 && a.is_finite() {
                hits[i] = a as u32;
                // rgb is the MEAN, so the sum is mean x count.
                palette_sum[i] = px[i][0].clamp(0.0, 1.0) as f64 * a as f64;
            }
        }
    }
    stage.unmap();
    renderer.destroy();

    // Normalised by what LANDED, not by what was dispatched.
    //
    // The invariant measure is a probability measure and this
    // estimates it, so the divisor is the sample total that reached
    // the grid. It also makes the result independent of whatever
    // constant the accumulator's alpha carries -- the histogram's
    // `color_scale`, the burn-in the dispatch count includes and the
    // plot does not, the samples a bad-value respawn drops. Counting
    // the dispatch instead read 12.5x the chaos game's density, and
    // chasing which of those constants it was would have been chasing
    // a number that cancels.
    let landed: u64 = hits.iter().map(|h| *h as u64).sum();
    let _ = total;
    Some(crate::scene::ifs_estimate::CoarseMeasure {
        res: res as usize,
        centre: ball_centre,
        radius: ball_radius,
        hits,
        palette_sum,
        samples: landed.max(1),
    })
}

/// How many `vec4`s of the params' `fdata` block one seed occupies.
pub const SEED_VEC4S: usize = 6;
/// Where the seeds start in `fdata`; the whole-IFS constants are below.
/// How many `vec2<f32>` of header sit in front of the coarse grid.
///
/// `[0]` is `(res, radius)` and `[1]` is the grid's centre, so a
/// shader can map a world point to a cell without another uniform.
pub const COARSE_HEADER: usize = 2;

/// Pack a [`CoarseMeasure`] for the shader: a header, then one
/// `vec2<f32>` per cell holding the measure per unit AREA and the mean
/// palette coordinate.
///
/// Density rather than a hit count, because the count means nothing
/// without the sample total and the cell size, and the shader would
/// have to be told both. Divided here once instead.
///
/// The mean palette coordinate falls back to mid-grey in an empty
/// cell. That value is never reached through a footprint read, whose
/// samples are weighted by the density beside them, and an empty cell
/// weighs nothing -- which is the point of reading both from one
/// footprint (measure plan §5g).
pub fn pack_xaos(
    ifs: &Ifs2,
    measure: &crate::scene::ifs_estimate::MeasureMaps,
) -> Vec<f32> {
    // Element 0 is the map count, and ZERO there is what the shader
    // reads as "no graph" -- the ordinary case, where every map may
    // follow every map and a step's probability is the map's own.
    if ifs.xaos.is_none() || ifs.maps.is_empty() {
        return vec![0.0; 4];
    }
    let n = ifs.maps.len();
    let mut out = vec![0.0f32; 1 + n * n + n];
    out[0] = n as f32;
    for i in 0..n {
        for l in 0..n {
            out[1 + i * n + l] = measure.step_probability(i, Some(l as u32)) as f32;
        }
        // The stationary row, which the first level after the
        // handover reads. The walk's own admissibility test is the
        // SIGN of one of these, so both halves have to be packed from
        // the same source as the CPU's -- branch corrections included,
        // since those never change a sign but do change a measure.
        out[1 + n * n + i] = measure.step_probability(i, None) as f32;
    }
    out
}

pub fn pack_coarse(c: &crate::scene::ifs_estimate::CoarseMeasure) -> Vec<[f32; 2]> {
    let cell = c.cell();
    let norm = 1.0 / (c.samples.max(1) as f64 * cell * cell);
    let mut out = Vec::with_capacity(COARSE_HEADER + c.hits.len());
    out.push([c.res as f32, c.radius as f32]);
    out.push([c.centre[0] as f32, c.centre[1] as f32]);
    for i in 0..c.hits.len() {
        let h = c.hits[i];
        out.push([
            (h as f64 * norm) as f32,
            if h > 0 { (c.palette_sum[i] / h as f64) as f32 } else { 0.5 },
        ]);
    }
    out
}

/// Read the packed grid the way the shader will, so the two cannot
/// drift apart without a test saying so.
///
/// This is the `SEED_VEC4S` lesson: the packer went from four words to
/// six and the shader's stride stayed a literal four, which was
/// invisible on a one-seed walk and 11% wrong on eight
/// (`the_shaders_seed_stride_matches_the_packer`). A packing with no
/// reader beside it is a stride waiting to disagree.
pub fn read_coarse(packed: &[[f32; 2]], q: [f32; 2]) -> (f32, f32) {
    if packed.len() < COARSE_HEADER {
        return (0.0, 0.5);
    }
    let res = packed[0][0];
    let radius = packed[0][1];
    let centre = packed[1];
    if !(res >= 1.0) || !(radius > 0.0) {
        return (0.0, 0.5);
    }
    let cell = 2.0 * radius / res;
    let fx = (q[0] - (centre[0] - radius)) / cell;
    let fy = (q[1] - (centre[1] - radius)) / cell;
    if !(fx >= 0.0) || !(fy >= 0.0) || fx >= res || fy >= res {
        return (0.0, 0.5);
    }
    let i = COARSE_HEADER + (fy as usize) * (res as usize) + (fx as usize);
    match packed.get(i) {
        Some(v) => (v[0], v[1]),
        None => (0.0, 0.5),
    }
}

pub const SEED_BASE: usize = 4;
/// The widest beam a seeded walk can hand over, bounded by `fdata`.
pub const MAX_SEEDS: usize = (64 - SEED_BASE) / SEED_VEC4S;

/// Pack the beam's handover state into the params' `fdata` block.
///
/// No new buffer: `fdata` is 64 `vec4`s and the whole-IFS constants use
/// four, which leaves room for fifteen seeds where the beam allows
/// eight.
///
/// Layout per seed, starting at `SEED_BASE + SEED_VEC4S * j`:
/// 0. `position.xy`, `basis[0][0]`, `basis[0][1]`
/// 1. `basis[1][0]`, `basis[1][1]`, `sigma_per_px`, `bound_per_px`
/// 2. `address fraction`, `last_sigma`, `escape level` (−1 = none), `flags`
/// 3. `escape point.xy`, `transform colour`, `prefix probability`
/// 4. `quad[0].xy`, `quad[1].xy` — the delta's quadratic part
/// 5. `quad[2].xy`, `prefix colour product`, `prefix colour sum`
///
/// The last three are the MEASURE walk's, and they are what lets it
/// start from a handover deeper than level 0. A seed carries where
/// its lineage IS; those carry what the lineage has ACCUMULATED --
/// the product of the branch probabilities that reached it, and the
/// two running numbers of the flam3 colour fold. Folded here, from
/// `Seed::address`, because that is an exact list of branches and
/// the packed address is a base-N fraction that loses its tail.
///
/// Six `vec4`s rather than four since the quadratic: a nonlinear
/// inverse drops a second-order term when it carries an offset
/// through its Jacobian alone, and `Q(uv) = C_uu·u² + C_uv·u·v +
/// C_vv·v²` is what replaces it. Ten seeds still fit where the beam
/// allows eight.
///
/// `flags`: bit 0 escaped, bit 1 done.
pub fn pack_seeds(
    measure: &crate::scene::ifs_estimate::MeasureMaps,
    seeds: &crate::scene::ifs_estimate::Seeds,
    n_maps: usize,
    colors: &[f32],
    out: &mut [[f32; 4]],
) {
    if out.len() < 2 {
        return;
    }
    let count = seeds.cands.len().min(MAX_SEEDS);
    // The next digit's weight, after however many the CPU already took.
    let addr_scale = if n_maps == 0 {
        0.0
    } else {
        (1.0f64 / n_maps as f64).powi(seeds.level as i32 + 1)
    };
    out[1] = [
        out[1][0],
        seeds.level as f32,
        count as f32,
        addr_scale as f32,
    ];
    // What the prefix met and could not enter. Zero means "none":
    // a carried gap is a distance and never negative, and the shader
    // reads a non-positive value as no constraint.
    if out.len() > 2 {
        out[2][0] = if seeds.dead_min_per_px.is_finite() {
            (seeds.dead_min_per_px.max(0.0) as f32).max(f32::MIN_POSITIVE)
        } else {
            0.0
        };
    }

    for (j, c) in seeds.cands.iter().take(count).enumerate() {
        let base = SEED_BASE + SEED_VEC4S * j;
        if base + SEED_VEC4S > out.len() {
            break;
        }
        let (esc_level, esc_point, escaped) = match c.escape {
            Some((lvl, p)) => (lvl as f32, [p[0] as f32, p[1] as f32], 1u32),
            None => (-1.0, [0.0, 0.0], 0u32),
        };
        // Bits 0 and 1 are the flags; bits 8 and up are the last map
        // plus one, so zero there means "no last map" -- which is what
        // a handover at level 0 has. Riding in the flags word rather
        // than taking a slot of its own: every one of the six vec4s a
        // seed packs into is full, and a u32 bitcast through an f32
        // carries 24 spare bits exactly.
        let last = c.address.last().map_or(0u32, |&i| i + 1);
        let flags = escaped | if c.done { 2 } else { 0 } | (last << 8);
        let colour = c
            .address
            .first()
            .and_then(|&i| colors.get(i as usize))
            .copied()
            .unwrap_or(0.0);
        out[base] = [
            c.position[0] as f32,
            c.position[1] as f32,
            c.basis[0][0] as f32,
            c.basis[0][1] as f32,
        ];
        out[base + 1] = [
            c.basis[1][0] as f32,
            c.basis[1][1] as f32,
            c.sigma_per_px as f32,
            if c.bound_per_px.is_finite() { c.bound_per_px as f32 } else { -1e30 },
        ];
        out[base + 2] = [
            crate::scene::ifs_estimate::address_fraction(&c.address, n_maps as u32) as f32,
            c.last_sigma as f32,
            esc_level,
            f32::from_bits(flags),
        ];
        // The prefix, folded. `hp` is the product of `(1+s)/2` over
        // the branches so far and `cacc` the running sum of
        // `col·(1−s)/2 · hp_before`, which together are the reversed
        // flam3 fold accumulated forward -- see `estimate_measure`.
        let (mut prob, mut hp, mut cacc) = (1.0f64, 1.0f64, 0.0f64);
        for &m in &c.address {
            let i = m as usize;
            prob *= measure.prob.get(i).copied().unwrap_or(0.0);
            let (cl, sp) = measure.colour.get(i).copied().unwrap_or((0.5, 0.0));
            cacc += cl * (1.0 - sp) * 0.5 * hp;
            hp *= (1.0 + sp) * 0.5;
        }
        out[base + 3] = [esc_point[0], esc_point[1], colour, prob as f32];
        out[base + 4] = [
            c.quad[0][0] as f32,
            c.quad[0][1] as f32,
            c.quad[1][0] as f32,
            c.quad[1][1] as f32,
        ];
        out[base + 5] =
            [c.quad[2][0] as f32, c.quad[2][1] as f32, hp as f32, cacc as f32];
    }
}

/// The 3D twin, and for the same reason: the target is the only thing
/// that needs digits, and only until the chain hands over.
impl crate::scene::ifs_estimate::SeedPoint3 for [super::bigfloat::BigFloat; 3] {
    fn apply_affine3(&self, a: &crate::scene::ifs_analysis::Affine3) -> Self {
        let n = self[0].n_limbs().max(self[1].n_limbs()).max(self[2].n_limbs());
        let big = |v: f64| super::bigfloat::BigFloat::from_f64(v, n);
        let row = |i: usize| {
            big(a.m[i][0])
                .mul(&self[0])
                .add(&big(a.m[i][1]).mul(&self[1]))
                .add(&big(a.m[i][2]).mul(&self[2]))
                .add(&big(a.t[i]))
        };
        [row(0), row(1), row(2)]
    }

    fn distance_to(&self, p: [f64; 3]) -> f64 {
        let d = [
            self[0].to_f64() - p[0],
            self[1].to_f64() - p[1],
            self[2].to_f64() - p[2],
        ];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }

    fn to_f64(&self) -> [f64; 3] {
        [self[0].to_f64(), self[1].to_f64(), self[2].to_f64()]
    }
}

/// A seeding position at arbitrary precision.
///
/// The impl lives here rather than beside the trait because `BigFloat`
/// is the escape engine's, and `scene` compiles without the escape
/// engine. Only the POSITION is big: the map coefficients stay f64,
/// and so does the distance the walk compares against the ball, which
/// is O(1) however precise the point is.
/// Rung 1 of `ifs-nonlinear-perturbation.md` §3, at arbitrary
/// precision: the kernels whose inverse is RATIONAL, so that a
/// `BigFloat` can take the step with multiplication and one
/// reciprocal and nothing else.
///
/// - `spherical`, whose inverse is `v/|v|²`.
/// - a root of integer distance, `|v|^{|n|/d}·e^{i·n·arg v}` with
///   `|d| = 1`. Write `w = v^{|n|}`, which is repeated squaring:
///   `|v|^{|n|}·e^{i·|n|·φ}`. The exponent and the angle are then
///   fixed separately, because the two operations available move
///   one each. `z/|z|²` reciprocates the MAGNITUDE and keeps the
///   argument, so `d < 0` applies it and nothing else does.
///   Conjugation reflects the ANGLE and keeps the magnitude, so
///   `n < 0` applies it and `d` has no say. Reading `z/|z|²` as a
///   complex reciprocal instead -- which reflects the angle too --
///   is a sign error in exactly the `n > 0, d < 0` corner, and is
///   what `the_big_kernel_inverse_is_the_f64_one` caught.
///
/// Everything else -- a fractional root, `disc`, `blob` (sin/cos),
/// `bubble`, `hemisphere` (sqrt) -- returns `None`, and the handover
/// stops where it stopped before. `BigFloat` has no `exp`, `sin` or
/// `sqrt` to build them from yet.
fn big_kernel_inverse(
    kernel: crate::scene::ifs_analysis::Kernel,
    v: &[super::bigfloat::BigFloat; 2],
) -> Option<[super::bigfloat::BigFloat; 2]> {
    use crate::scene::ifs_analysis::Kernel;
    use super::bigfloat::BigComplex;
    let z = BigComplex { re: v[0].clone(), im: v[1].clone() };
    if z.is_zero() {
        return None;
    }
    let invert = |c: &BigComplex| -> Option<BigComplex> {
        let n2 = c.norm_sqr();
        if n2.is_zero() {
            return None;
        }
        let inv = n2.recip();
        Some(BigComplex { re: c.re.mul(&inv), im: c.im.mul(&inv) })
    };
    let out = match kernel {
        Kernel::Spherical => invert(&z)?,
        Kernel::Root { n, d } if d.abs() == 1.0 && n != 0 => {
            let mut acc: Option<BigComplex> = None;
            let mut base = z.clone();
            let mut e = n.unsigned_abs();
            while e > 0 {
                if e & 1 == 1 {
                    acc = Some(match acc {
                        Some(a) => a.mul(&base),
                        None => base.clone(),
                    });
                }
                base = base.mul(&base);
                e >>= 1;
            }
            let mut w = acc?;
            if d < 0.0 {
                w = invert(&w)?;
            }
            if n < 0 {
                w = BigComplex { re: w.re, im: w.im.neg() };
            }
            w
        }
        _ => return None,
    };
    let out = [out.re, out.im];
    out[0].to_f64().is_finite().then_some(())?;
    out[1].to_f64().is_finite().then_some(())?;
    Some(out)
}

impl crate::scene::ifs_estimate::SeedPoint for [super::bigfloat::BigFloat; 2] {
    fn apply_map(&self, m: &crate::scene::ifs_analysis::Map2) -> Option<Self> {
        use crate::scene::ifs_analysis::Map2;
        use crate::scene::ifs_estimate::SeedPoint;
        match m {
            Map2::Affine(a) => Some(self.apply_affine(a)),
            Map2::NonlinearInverse(r) => {
                let (kernel, _branch, pre_inv, post) = r.parts();
                let v = self.apply_affine(&post);
                let u = big_kernel_inverse(kernel, &v)?;
                Some(u.apply_affine(pre_inv))
            }
            // The walk only ever inverts.
            Map2::Nonlinear(_) => None,
        }
    }

    fn apply_affine(&self, a: &Affine2) -> Self {
        let n = self[0].n_limbs().max(self[1].n_limbs());
        let big = |v: f64| super::bigfloat::BigFloat::from_f64(v, n);
        [
            big(a.m[0][0])
                .mul(&self[0])
                .add(&big(a.m[0][1]).mul(&self[1]))
                .add(&big(a.t[0])),
            big(a.m[1][0])
                .mul(&self[0])
                .add(&big(a.m[1][1]).mul(&self[1]))
                .add(&big(a.t[1])),
        ]
    }

    fn distance_to(&self, p: [f64; 2]) -> f64 {
        let dx = self[0].to_f64() - p[0];
        let dy = self[1].to_f64() - p[1];
        (dx * dx + dy * dy).sqrt()
    }

    fn to_f64(&self) -> [f64; 2] {
        [self[0].to_f64(), self[1].to_f64()]
    }
}

/// The view centre, at the precision the zoom asks for.
///
/// `EscapeConfig` keeps the centre as exact decimal strings precisely
/// so this is possible: an f64 centre is quantised to 5.5e-17, which
/// is about 1% of the view by 2⁴⁹ and the whole of it not long after.
/// The walk consumes about one bit of the centre per level, and the
/// handover is at roughly `zoom` levels, so the precision needed grows
/// with the zoom — which is exactly what `limbs_for_view` sizes.
pub fn centre_at_precision(
    escape: &crate::config::escape::EscapeConfig,
) -> Option<[super::bigfloat::BigFloat; 2]> {
    let n = super::fixedpoint::limbs_for_view(
        &escape.center_re,
        &escape.center_im,
        escape.zoom_log2,
    );
    let re = super::fixedpoint::FixedPoint::from_decimal(&escape.center_re, n)?;
    let im = super::fixedpoint::FixedPoint::from_decimal(&escape.center_im, n)?;
    Some([
        super::bigfloat::BigFloat::from_fixed(&re),
        super::bigfloat::BigFloat::from_fixed(&im),
    ])
}

/// The camera's target at whatever precision the zoom asks for.
///
/// The 3D twin of [`centre_at_precision`], and the reason is the same
/// one: after `k` inverse maps the seeding walk has computed
/// `A_k·T + b_k` with `A_k ~ 2ᵏ` and an O(1) answer, so the target
/// pays `k` bits to get there and `k` is about the zoom. An empty
/// axis means the attractor's own centre, exactly as the camera reads
/// it — a centre that is already O(1) and needs no digits.
pub fn target_at_precision(
    escape: &crate::config::escape::EscapeConfig,
    ifs: &crate::scene::ifs_analysis::Ifs3,
) -> Option<[super::bigfloat::BigFloat; 3]> {
    let axes = [
        &escape.cam_target_x,
        &escape.cam_target_y,
        &escape.cam_target_z,
    ];
    let n = axes
        .iter()
        .map(|a| super::fixedpoint::limbs_for_view(a, a, escape.zoom_log2))
        .max()
        .unwrap_or(1);
    let mut out = Vec::with_capacity(3);
    for (k, a) in axes.iter().enumerate() {
        let big = if a.trim().is_empty() {
            super::bigfloat::BigFloat::from_f64(ifs.ball.centre[k], n)
        } else {
            super::bigfloat::BigFloat::from_fixed(&super::fixedpoint::FixedPoint::from_decimal(
                a, n,
            )?)
        };
        out.push(big);
    }
    let mut it = out.into_iter();
    Some([it.next()?, it.next()?, it.next()?])
}

/// The view basis: takes a pixel's normalised offset — the screen
/// spanning [-½, ½] on each axis — to a world offset from the centre.
///
/// The `-span_y` is the template's `d.y = -d.y`: screen y runs down.
pub fn view_basis(span_x: f64, span_y: f64, rotation: f32) -> [[f64; 2]; 2] {
    let (c, s) = ((rotation as f64).cos(), (rotation as f64).sin());
    // rotate(diag(span_x, -span_y))
    [[c * span_x, s * span_y], [s * span_x, -c * span_y]]
}

/// Where a solid render looks from, and which way.
///
/// One place, so the marcher, the panel and anything that later flies
/// the camera cannot disagree about what an angle means.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidCamera {
    pub eye: [f64; 3],
    pub target: [f64; 3],
    pub forward: [f64; 3],
    pub right: [f64; 3],
    pub up: [f64; 3],
    pub fov: f32,
    /// How far the eye sits from the target, in world units.
    pub distance: f64,
    /// The eye's offset from the target — `−forward · distance`.
    ///
    /// Carried rather than derived, because `eye − target` is the
    /// subtraction of two nearly equal numbers and it is EXACTLY the
    /// cancellation the whole seeded path exists to avoid. Measured:
    /// past a zoom of 2⁴⁸ the difference rounds to zero in f64, so
    /// every ray in the frame started at the target itself and the
    /// picture stopped changing with the zoom. This quantity is
    /// products and sums of numbers the same size as itself, so it
    /// keeps its relative precision at any depth.
    pub eye_rel: [f64; 3],
}

/// How many ball radii away `zoom_log2 = 0` puts the eye.
///
/// Chosen so the attractor fills the frame at the default field of
/// view rather than rattling around in it: the ball subtends roughly
/// `2·asin(1/3.2) ≈ 0.64` radians, against a 0.7-radian default.
const FRAME_DISTANCE: f64 = 3.2;


/// Build the camera for a solid render.
///
/// The TARGET carries the precision and the eye is derived from it:
/// `zoom_log2` shortens the distance, the two angles orbit. That is
/// the shape a deep zoom wants — an approach to a point that holds
/// still — and it is why the target is stored as decimal strings while
/// the angles are plain `f32` (D8).
///
/// An empty target means the attractor's own centre, so a flame you
/// have just switched to is framed without being told where it is.
pub fn solid_camera(
    escape: &crate::config::escape::EscapeConfig,
    ifs: &Ifs3,
) -> SolidCamera {
    let axis = |s: &str, fallback: f64| -> f64 {
        if s.trim().is_empty() {
            fallback
        } else {
            s.trim().parse::<f64>().unwrap_or(fallback)
        }
    };
    let target = [
        axis(&escape.cam_target_x, ifs.ball.centre[0]),
        axis(&escape.cam_target_y, ifs.ball.centre[1]),
        axis(&escape.cam_target_z, ifs.ball.centre[2]),
    ];

    // The measured radius: Extent moves the cut, not the camera.
    let r = ifs.frame_radius.max(1e-12);
    let distance = FRAME_DISTANCE * r / 2f64.powf(escape.zoom_log2);

    let (right, up, forward) = solid_frame(
        escape.cam_pitch as f64,
        escape.cam_yaw as f64,
        escape.cam_bank as f64,
        escape.rotation as f64,
    );
    // The eye sits on the sphere of that radius about the target and
    // looks back down the same line.
    let eye_rel = [-forward[0] * distance, -forward[1] * distance, -forward[2] * distance];
    // The absolute eye is for callers that want a position; nothing on
    // the deep-zoom path reads it, and past 2⁴⁸ it IS the target.
    let eye = [
        target[0] + eye_rel[0],
        target[1] + eye_rel[1],
        target[2] + eye_rel[2],
    ];

    SolidCamera {
        eye,
        target,
        forward,
        right,
        up,
        fov: escape.cam_fov.clamp(0.05, 3.0),
        distance,
        eye_rel,
    }
}

/// The solid camera's frame -- `(right, up, forward)`, world-space
/// unit vectors -- from its four angles.
///
/// It is the flame's camera chain, `Rz(roll)·Rx(pitch)·Ry(bank)·
/// Rz(−yaw)` (`build_camera_matrix` in `utilities.wgsl`, and
/// `CameraMatrix::build` in fly mode), with the same meaning for each
/// slot: bank sits between pitch and yaw as it does there, and the
/// roll -- the View's `rotation`, the plane's -- is the outermost
/// factor, a turn of the screen that leaves the other three alone. So
/// what Bank does to a solid is what it does to a 3D flame, and a
/// camera that later flies this one can reuse the fly mode's algebra.
///
/// Two things differ, and both are the escape engine's conventions
/// rather than the flame's. Where zero is: the flame's pitch is
/// measured from looking straight DOWN, its home view; the solid's is
/// measured from the horizon, which was its home view before it had a
/// bank, and the shipped presets and every saved solid carry angles
/// in those terms -- so `pitch_flame = π/2 − pitch` and
/// `yaw_flame = yaw − π/2` on the way in. And handedness: the flame
/// draws its y axis DOWN the screen (Apophysis does), so its frame is
/// the mirror of a physical camera's, while the plane draws Im up and
/// the solid always looked the physical way, right = forward × up.
/// The chain's screen-x row is negated for that, and the roll is
/// applied as `Rz(−rotation)` so that a positive rotation turns the
/// solid's screen the way it turns the plane's
/// (`rotation_rolls_the_solid_screen_as_it_rolls_the_plane`).
///
/// With bank and rotation both zero this is EXACTLY the frame the
/// camera had before (`the_frame_is_what_it_was_with_the_new_angles_at_zero`):
/// the eye on the sphere by elevation and azimuth, world +z's
/// projection for up. It also exists at the poles now, so the pitch
/// clamp that kept the old cross product off them is gone.
///
/// Rows of the matrix: row 0 is screen-right, row 1 is screen-DOWN
/// (pixel y grows downward), row 2 is the camera's +z, which is the
/// direction it looks AWAY from.
pub fn solid_frame(pitch: f64, yaw: f64, bank: f64, rotation: f64) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let m = solid_matrix(pitch, yaw, bank, rotation);
    let right = m[0];
    let up = [-m[1][0], -m[1][1], -m[1][2]];
    let forward = [-m[2][0], -m[2][1], -m[2][2]];
    (right, up, forward)
}

/// The matrix, row-major, world → camera: the flame's chain with the
/// solid's zero, its roll direction, and its handedness (the screen-x
/// row negated).
fn solid_matrix(pitch: f64, yaw: f64, bank: f64, rotation: f64) -> [[f64; 3]; 3] {
    let pitch_f = std::f64::consts::FRAC_PI_2 - pitch;
    let yaw_f = yaw - std::f64::consts::FRAC_PI_2;
    let mut m = mat_mul3(
        mat_mul3(rot_z(-rotation), rot_x(pitch_f)),
        mat_mul3(rot_y(bank), rot_z(-yaw_f)),
    );
    for v in m[0].iter_mut() {
        *v = -*v;
    }
    m
}

fn rot_x(a: f64) -> [[f64; 3]; 3] {
    let (s, c) = a.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

fn rot_y(a: f64) -> [[f64; 3]; 3] {
    let (s, c) = a.sin_cos();
    [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
}

fn rot_z(a: f64) -> [[f64; 3]; 3] {
    let (s, c) = a.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

fn mat_mul3(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    out
}

/// How much world one screen pixel covers at the target's depth, as
/// `mantissa · 2^exponent` -- the step a pan of one pixel moves the
/// target by, along the camera's right or up.
///
/// Symbolic for the same reason the plane's pan delta is: a solid's
/// zoom shrinks the distance without limit and an f64 step underflows
/// past ~2¹⁰⁶⁰ while the target's decimal digits do not. Vertical
/// field of view over the frame's height, which is how `ifs_ray`
/// spreads the rays.
pub fn solid_pixel_step(
    escape: &crate::config::escape::EscapeConfig,
    ifs: &Ifs3,
    height_px: f64,
) -> (f64, i64) {
    // The measured radius: Extent moves the cut, not the camera.
    let r = ifs.frame_radius.max(1e-12);
    let tan_half = (escape.cam_fov.clamp(0.05, 3.0) as f64 * 0.5).tan();
    let x = (2.0 * tan_half * FRAME_DISTANCE * r).log2() - escape.zoom_log2 - height_px.max(1.0).log2();
    let e = x.floor();
    ((x - e).exp2(), e as i64)
}

/// Pack the whole-IFS constants and the camera for a solid render.
///
/// Layout, one `vec4` each:
/// 0. `ball centre xyz, radius`
/// 1. `mean_sigma_min, map_count, 0, 0`
/// 2. `eye xyz, field of view`
/// 3. `forward xyz, 0`
/// 4. `right xyz, 0`
/// 5. `up xyz, 0`
pub fn pack_globals3(
    ifs: &Ifs3,
    cam: &SolidCamera,
    link_levels: usize,
    beam: usize,
    shading: &crate::config::SolidShadingSettings,
    fog: (f32, f32, [f32; 3]),
    out: &mut [[f32; 4]],
) {
    if out.len() < 18 {
        return;
    }
    out[0] = [
        ifs.ball.centre[0] as f32,
        ifs.ball.centre[1] as f32,
        ifs.ball.centre[2] as f32,
        ifs.ball.radius as f32,
    ];
    let mean = if ifs.maps.is_empty() {
        0.5
    } else {
        ifs.maps.iter().map(|m| m.sigma_min).sum::<f64>() / ifs.maps.len() as f64
    };
    // At least one, so the template's "no qualifying flame" guard does
    // not fire on a flame-less def (plan 8.11 Q3); a flame with no
    // maps never gets this far.
    // w: whether any map is nonlinear. The walk's precision early exit
    // -- "this candidate can no longer move the answer by eps" --
    // assumes sigma only shrinks, which a contraction guarantees and a
    // root does not: its forward derivative exceeds one near its
    // critical point, so sigma can grow after the exit has fired, and
    // an unescaped candidate frozen there escapes at full depth with a
    // bound far above eps. Measured as rings of exterior colour on a
    // surface the march had hit. Off for a nonlinear solid.
    let nonlinear = ifs.maps.iter().any(|m| !m.inverse.is_affine());
    out[1] = [mean as f32, ifs.maps.len().max(1) as f32, ifs.aux_centre as f32, if nonlinear { 1.0 } else { 0.0 }];
    // The eye RELATIVE TO THE TARGET, which is the whole of the 3D
    // deep zoom. `eye = target − forward·distance`, so this is
    // `−forward·distance` and its magnitude IS the distance: an f32
    // holds it to a part in ten million however deep the zoom goes,
    // where the absolute eye is quantised against a coordinate of
    // order one and stops resolving a pixel at about 2¹³.
    //
    // Nothing downstream ever forms `target + δ`. That sum is the
    // cancellation the chain exists to avoid, and it is avoided by
    // never writing it.
    out[2] = [
        cam.eye_rel[0] as f32,
        cam.eye_rel[1] as f32,
        cam.eye_rel[2] as f32,
        cam.fov,
    ];
    // w: the beam's ranking key, as the planar packer sets it -- the
    // forward vector's pad, which nothing else reads.
    let weighted = crate::scene::ifs_estimate::resolved_key(
        ifs,
        crate::scene::ifs_estimate::RankKey::Auto,
    ) == crate::scene::ifs_estimate::RankKey::Weighted;
    out[3] = [cam.forward[0] as f32, cam.forward[1] as f32, cam.forward[2] as f32, if weighted { 1.0 } else { 0.0 }];
    out[4] = [cam.right[0] as f32, cam.right[1] as f32, cam.right[2] as f32, 0.0];
    out[5] = [cam.up[0] as f32, cam.up[1] as f32, cam.up[2] as f32, 0.0];
    // The target's offset from the ball's centre, which is what turns
    // a delta back into something the bounding sphere can be tested
    // against. O(1), and only ever used for that test.
    out[6] = [
        (cam.target[0] - ifs.ball.centre[0]) as f32,
        (cam.target[1] - ifs.ball.centre[1]) as f32,
        (cam.target[2] - ifs.ball.centre[2]) as f32,
        link_levels.min(MAX_CHAIN_LINKS) as f32,
    ];
    // The handover cap, which is the whole of the level choice: a link
    // holds a sample while its matrix carries the sample's delta no
    // further than this.
    out[7] = [
        beam.max(1) as f32,
        (ifs.ball.radius * crate::scene::ifs_estimate::HANDOVER_FRACTION) as f32,
        0.0,
        0.0,
    ];
    pack_rig3(cam, shading, fog, out);
}

/// The lighting rig, from the Solid Rendering panel's own settings.
///
/// The panel is the one place lighting is described in this app, and a
/// solid IFS has no business inventing a second vocabulary for it — so
/// mode D reads `SolidShadingSettings` rather than growing its own
/// light controls. What it does NOT do is route its pixels through the
/// shade pass (D7 proposed that): the pass's advantage over a marcher
/// is entirely this rig, which is forty lines, while its geometry —
/// screen-space normals, eight-tap SSAO, splat-resolution shadow maps —
/// is the half a marcher already does better and exactly. Sending the
/// pixels there would cost four full-image buffers and a depth encoding
/// to arrive at the same picture, and a single occlusion channel cannot
/// express a shadow PER LIGHT, which a marcher gets by tracing one.
///
/// Lights are stored in CAMERA space, which is what the splat pipeline
/// shades in; the marcher works in world space, so they are rotated
/// here, once, rather than per pixel.
fn pack_rig3(
    cam: &SolidCamera,
    shading: &crate::config::SolidShadingSettings,
    fog: (f32, f32, [f32; 3]),
    out: &mut [[f32; 4]],
) {
    // An untouched panel says `shading_strength = 0`, which for a FLAME
    // means "classic emissive, no lighting" — a meaningful picture. For
    // a solid it is a flat silhouette, so UNTOUCHED means a default key
    // light rather than no light.
    //
    // Untouched is the whole struct being default, not
    // `shading_strength == 0`. The difference matters: a user who sets
    // the strength to zero deliberately is asking for the unlit
    // silhouette, and a rule keyed on the strength alone would take
    // that away from them — there would be no way to ask for it.
    let any = !crate::config::SolidShadingSettings::is_default(shading);
    let (strength, ambient, diffuse, specular, shininess, ssao) = if any {
        (
            shading.shading_strength,
            shading.ambient,
            shading.diffuse,
            shading.specular,
            shading.shininess,
            shading.ssao_strength,
        )
    } else {
        (1.0, 0.12, 0.88, 0.0, 32.0, 1.0)
    };
    out[8] = [strength, ambient, diffuse, specular];
    out[9] = [shininess, ssao, fog.0, fog.1];
    out[10] = [fog.2[0], fog.2[1], fog.2[2], 0.0];

    let mut n = 0usize;
    for l in shading.lights.iter() {
        if any && (!l.enabled || l.intensity <= 0.0) {
            continue;
        }
        let (az, el) = if any {
            (l.azimuth.to_radians() as f64, l.elevation.to_radians() as f64)
        } else {
            // The panel's own light-0 default, so "untouched" and
            // "light 0 as it ships" are the same picture.
            (35f32.to_radians() as f64, 40f32.to_radians() as f64)
        };
        // Camera space: azimuth 0, elevation 0 is a headlight, azimuth
        // swings about the vertical and elevation tilts up. Rotated to
        // world through the camera's own basis -- +z in camera space
        // points BACK at the eye, which is -forward.
        let c = [el.cos() * az.sin(), el.sin(), el.cos() * az.cos()];
        let dir = [
            cam.right[0] * c[0] + cam.up[0] * c[1] - cam.forward[0] * c[2],
            cam.right[1] * c[0] + cam.up[1] * c[1] - cam.forward[1] * c[2],
            cam.right[2] * c[0] + cam.up[2] * c[1] - cam.forward[2] * c[2],
        ];
        let dir = normalize3(dir);
        let (colour, intensity) = if any {
            (l.color, l.intensity.max(0.0))
        } else {
            ([1.0, 1.0, 1.0], 1.0)
        };
        out[11 + n * 2] = [dir[0] as f32, dir[1] as f32, dir[2] as f32, intensity];
        out[12 + n * 2] = [colour[0], colour[1], colour[2], 0.0];
        n += 1;
        if n == 4 || !any {
            break;
        }
    }
    out[7][2] = n as f32;
}

fn normalize3(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n > 0.0 {
        [v[0] / n, v[1] / n, v[2] / n]
    } else {
        [1.0, 0.0, 0.0]
    }
}

/// The most links a packed chain may hold.
///
/// One link per level, and a level buys about a bit of zoom at σ = ½,
/// so this is the zoom ceiling in another form. Past it the chain is
/// truncated rather than refused: a shorter chain is still SOUND —
/// every link is a valid place to start — it simply hands over
/// earlier and so loses precision, which is the graceful end of a
/// deep zoom rather than a black frame.
pub const MAX_CHAIN_LINKS: usize = 256;

/// One link of the 3D seed chain, GPU layout.
///
/// Six `vec4`s and no `vec3` anywhere, for the reason recorded on
/// [`IfsMap3Gpu`]: a `vec3` aligns to sixteen bytes in WGSL and four
/// in Rust, and the render that mismatch produces is a plausible blob
/// rather than an error.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct IfsLinkGpu {
    /// The reference position in `xyz`, and the matrix's binary
    /// exponent in `w`.
    pub pos: [f32; 4],
    /// Rows of the SCALED matrix in `xyz` — the true matrix is
    /// `mat · 2^pos.w`, applied with `ldexp` so neither half has to be
    /// representable on its own. `w` carries, in order: `log2` of the
    /// matrix's reach, the σ product, and the running bound.
    ///
    /// The reach is kept as a LOGARITHM because the thing itself runs
    /// like `2^level` and overflows f32 halfway down a deep chain,
    /// while the comparison it exists for — reach × |δ| against the
    /// cap — is a sum of logarithms either way.
    pub rows: [[f32; 4]; 3],
    /// Address fraction, σ of the last map, escape level (−1 for
    /// none), and flag bits (bit 0 escaped, bit 1 done).
    pub extra: [f32; 4],
    /// The point this link escaped at in `xyz`, and the transform
    /// colour of its first map in `w`.
    pub esc: [f32; 4],
}

/// Pack a seed chain for the shader, padded to `slots` links a level.
///
/// Levels shallower than the beam is wide -- the top of the chain has
/// one candidate -- have fewer candidates than the slot count, and the
/// short ones are padded with slots flagged EMPTY (bit 2), which the
/// walk's ranking skips. Padding by repeating a real candidate was
/// tried first, as cheaper, and is wrong: the sample ranks the slots
/// by its own position, and a repeated branch fills its beam with
/// copies of itself.
pub fn pack_chain3(
    chain: &crate::scene::ifs_estimate::SeedChain3,
    n_maps: usize,
    colors: &[f32],
    slots: usize,
) -> Vec<IfsLinkGpu> {
    let slots = slots.max(1);
    let levels = chain.levels.len().min(MAX_CHAIN_LINKS);
    let mut out = Vec::with_capacity(levels * slots);
    for cands in chain.levels.iter().take(levels) {
        for b in 0..slots {
            // A level shallower than the slot count pads with EMPTY
            // slots, flagged so the walk skips them. Padding by
            // repeating a real candidate would let a pixel fill its
            // beam with copies of one branch.
            if b >= cands.len() {
                let mut empty = IfsLinkGpu::default();
                empty.extra[3] = f32::from_bits(4);
                out.push(empty);
                continue;
            }
            let c = &cands[b];
            let reach = c.reach.max(f64::MIN_POSITIVE);
            // Split the matrix into a unit-ish part and an exponent.
            // `exp2` of the exponent is never formed — `ldexp` applies
            // it — so the split holds wherever f64 held the matrix.
            let exp = reach.log2().ceil();
            let scale = (-exp).exp2();
            let m = |i: usize, j: usize| (c.matrix[i][j] * scale) as f32;
            let (esc_level, esc_point, escaped) = match c.escape {
                Some((lvl, p)) => (lvl as f32, [p[0] as f32, p[1] as f32, p[2] as f32], 1u32),
                None => (-1.0, [0.0, 0.0, 0.0], 0u32),
            };
            // As `pack_seeds`: bits 8 and up carry the last map plus
            // one, for the solid walk's own xaos test.
            let last = c.address.last().map_or(0u32, |&i| i + 1);
            let flags = escaped | if c.done { 2 } else { 0 } | (last << 8);
            let colour = c
                .address
                .first()
                .and_then(|&i| colors.get(i as usize))
                .copied()
                .unwrap_or(0.0);
            out.push(IfsLinkGpu {
                pos: [
                    c.position[0] as f32,
                    c.position[1] as f32,
                    c.position[2] as f32,
                    exp as f32,
                ],
                rows: [
                    [m(0, 0), m(0, 1), m(0, 2), reach.log2() as f32],
                    [m(1, 0), m(1, 1), m(1, 2), c.sigma as f32],
                    [
                        m(2, 0),
                        m(2, 1),
                        m(2, 2),
                        if c.bound.is_finite() { c.bound as f32 } else { -1e30 },
                    ],
                ],
                extra: [
                    crate::scene::ifs_estimate::address_fraction(&c.address, n_maps as u32) as f32,
                    c.last_sigma as f32,
                    esc_level,
                    f32::from_bits(flags),
                ],
                esc: [esc_point[0], esc_point[1], esc_point[2], colour],
            });
        }
    }
    out
}

/// Whether this formula names a SOLID distance function.
///
/// D2: the 3D controls follow the CONFIG, not the render mode. Escape
/// mode is not three-dimensional; one formula in it is.
pub fn formula_is_solid(name: &str) -> bool {
    get_ifs(name).is_some_and(|d| d.solid)
}

/// The per-map rows of the storage buffer, in the flame's transform
/// order — so a branch index in the address IS a transform index.
pub fn pack_maps(
    ifs: &Ifs2,
    colors: &[f32],
    measure: Option<&crate::scene::ifs_estimate::MeasureMaps>,
) -> Vec<IfsMapGpu> {
    let m4 = |a: &Affine2| [a.m[0][0] as f32, a.m[0][1] as f32, a.m[1][0] as f32, a.m[1][1] as f32];
    let t2 = |a: &Affine2| [a.t[0] as f32, a.t[1] as f32];
    ifs.maps
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let color = colors.get(m.transform_index).copied().unwrap_or(0.0);
            let meas = measure.map_or([0.0f32; 4], |mm| {
                [
                    mm.prob.get(i).copied().unwrap_or(0.0) as f32,
                    mm.colour.get(i).map_or(0.0, |c| c.1) as f32,
                    0.0,
                    0.0,
                ]
            });
            match m.inverse {
                Map2::Affine(inv) => IfsMapGpu {
                    inv_m: m4(&inv),
                    inv_t: t2(&inv),
                    sigma_min: m.sigma_min as f32,
                    color,
                    pre_m: [0.0; 4],
                    pre_t: [0.0; 2],
                    kind: 0.0,
                    branch: 0.0,
                    params: [0.0; 4],
                    measure: meas,
                },
                Map2::NonlinearInverse(r) | Map2::Nonlinear(r) => {
                    // post⁻¹ with the 1/w folded in: the shader's first
                    // affine takes q straight to the point before the
                    // kernel's inverse.
                    let scale = 1.0 / r.w;
                    let post = Affine2 {
                        m: [
                            [r.post_inv.m[0][0] * scale, r.post_inv.m[0][1] * scale],
                            [r.post_inv.m[1][0] * scale, r.post_inv.m[1][1] * scale],
                        ],
                        t: [r.post_inv.t[0] * scale, r.post_inv.t[1] * scale],
                    };
                    use crate::scene::ifs_analysis::Kernel;
                    // z of a unit-disc kernel: the scale of the image
                    // gap (S4), |w| times the post-affine's smallest
                    // stretch.
                    let (post_lo, _) = r.post.singular_values();
                    let gap = (r.w.abs() * post_lo) as f32;
                    let (kind, params) = match r.kernel {
                        // z: the gap scale; w: the hole radius, 0 for
                        // none (a root with a positive distance).
                        Kernel::Root { n, d } => (1.0, [n as f32, d as f32, gap, r.hole as f32]),
                        Kernel::Spherical => (2.0, [0.0, 0.0, gap, r.hole as f32]),
                        Kernel::Bubble => (3.0, [0.0, 0.0, gap, 0.0]),
                        Kernel::Hemisphere => (4.0, [0.0, 0.0, gap, 0.0]),
                        Kernel::Disc => (5.0, [0.0, 0.0, gap, 0.0]),
                        Kernel::Blob { high, low, waves } => (6.0, [high as f32, low as f32, waves as f32, 0.0]),
                    };
                    IfsMapGpu {
                        inv_m: m4(&post),
                        inv_t: t2(&post),
                        sigma_min: m.sigma_min as f32,
                        color,
                        pre_m: m4(&r.pre_inv),
                        pre_t: t2(&r.pre_inv),
                        kind,
                        branch: r.branch as f32,
                        params,
                        measure: meas,
                    }
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    /// The user's Extent reaches the packed ball, and 0 leaves the
    /// measured one alone.
    #[test]
    fn the_extent_is_the_users_when_set() {
        let registry = crate::variations::global_registry();
        let mut cfg = crate::config::FractalConfig::default();
        cfg.flame = super::gpu_tests::sierpinski_flame();
        cfg.escape.formula = "ifs_flame".to_string();
        let def = super::get_ifs("ifs_flame").expect("mode D");
        let measured = super::pack_for(def, &cfg, &registry).expect("packs");
        cfg.escape.formula_params.insert("extent".to_string(), 0.0);
        let zero = super::pack_for(def, &cfg, &registry).expect("packs");
        assert!(super::packed_bytes_eq(Some(&measured), Some(&zero)), "0 is the measured ball");
        cfg.escape.formula_params.insert("extent".to_string(), 7.5);
        let held = super::pack_for(def, &cfg, &registry).expect("packs");
        assert_eq!(held.globals[0][2], 7.5, "the packed radius is the user's");
        assert_eq!(held.ifs.ball.centre, measured.ifs.ball.centre, "the centre is still measured");
        assert_eq!(held.ifs.frame_radius, measured.ifs.ball.radius, "the framing radius is still measured");
        assert!(!super::packed_bytes_eq(Some(&measured), Some(&held)));
    }

    /// On a solid the Extent moves the walk's cut and not the camera:
    /// the camera frames the measured ball.
    #[test]
    fn the_extent_does_not_dolly_the_solid_camera() {
        let registry = crate::variations::global_registry();
        let cfg0 = solid_presets().into_iter().next().expect("a solid preset");
        let mut cfg = cfg0.clone();
        let def = super::get_ifs(&cfg.escape.formula).expect("mode D");
        assert!(def.solid);
        let measured = super::pack_for(def, &cfg, &registry).expect("packs");
        cfg.escape.formula_params.insert("extent".to_string(), 9.0);
        let held = super::pack_for(def, &cfg, &registry).expect("packs");
        let (m3, _) = measured.solid.as_ref().expect("a solid reading");
        let (h3, _) = held.solid.as_ref().expect("a solid reading");
        assert_eq!(h3.ball.radius, 9.0);
        assert_eq!(h3.frame_radius, m3.ball.radius);
        let a = super::solid_camera(&cfg.escape, m3);
        let b = super::solid_camera(&cfg.escape, h3);
        assert_eq!(a.distance, b.distance, "the camera did not move");
        assert_eq!(super::solid_pixel_step(&cfg.escape, m3, 512.0), super::solid_pixel_step(&cfg.escape, h3, 512.0));
    }

    use super::*;
    use crate::scene::ifs_analysis::analyse_2d;
    use crate::scene::transforms::{Flame, Transform};
    use crate::variations::global_registry;
    use std::collections::HashMap;

    fn half(tx: f32, ty: f32, color: f32) -> Transform {
        let mut t = Transform::default();
        t.a = 0.5;
        t.b = 0.0;
        t.c = 0.0;
        t.d = 0.5;
        t.e = tx;
        t.f = ty;
        t.color = color;
        t.variations = HashMap::from([("linear".to_string(), 1.0)]);
        t.variation_order = vec!["linear".to_string()];
        t
    }

    // ---- the solid camera's frame ---------------------------------

    fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    fn close3(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() <= tol)
    }

    /// With bank and rotation at zero the four-angle chain is the
    /// frame the camera had before it had them: the eye on the sphere
    /// by elevation and azimuth, world +z projected for up. Every
    /// shipped solid and every saved one carries its angles in those
    /// terms, so this is what keeps them where they were.
    #[test]
    fn the_frame_is_what_it_was_with_the_new_angles_at_zero() {
        for &pitch in &[-1.5f64, -0.42, 0.0, 0.25, 0.42, 1.2, 1.5] {
            for &yaw in &[0.0f64, 0.9, 2.6, -2.0, 3.1] {
                // The old construction, verbatim.
                let dir = [pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin()];
                let forward = [-dir[0], -dir[1], -dir[2]];
                let right = normalize3(cross3(forward, [0.0, 0.0, 1.0]));
                let up = cross3(right, forward);

                let (r, u, f) = solid_frame(pitch, yaw, 0.0, 0.0);
                assert!(close3(r, right, 1e-12), "right at pitch {pitch} yaw {yaw}: {r:?} vs {right:?}");
                assert!(close3(u, up, 1e-12), "up at pitch {pitch} yaw {yaw}: {u:?} vs {up:?}");
                assert!(close3(f, forward, 1e-12), "forward at pitch {pitch} yaw {yaw}: {f:?} vs {forward:?}");
            }
        }
    }

    /// The chain IS the flame's: `build_camera_matrix` from
    /// `utilities.wgsl`, transcribed, with the call-site slot mapping
    /// (yaw slot ← −roll, pitch ← −pitch, bank ← −bank, roll slot ←
    /// yaw), the solid's re-expression of pitch and yaw, its roll
    /// direction (`roll = −rotation`) and its handedness (the screen-x
    /// row mirrored). So what Bank does here is what it does to a 3D
    /// flame, seen in a mirror.
    #[test]
    fn the_solid_chain_is_the_flames_camera_matrix() {
        fn wgsl(yaw: f64, pitch: f64, bank: f64, roll: f64) -> [[f64; 3]; 3] {
            let (sy, cy) = yaw.sin_cos();
            let (sp, cp) = pitch.sin_cos();
            let (sb, cb) = bank.sin_cos();
            let (sr, cr) = roll.sin_cos();
            // Columns as the shader writes them: column c holds
            // (m[0][c], m[1][c], m[2][c]) of the row-major matrix.
            let col0 = [
                cy * cb * cr - sy * cp * sr + sy * sp * sb * cr,
                -sy * cb * cr - cy * cp * sr + cy * sp * sb * cr,
                sp * sr + cp * sb * cr,
            ];
            let col1 = [
                cy * cb * sr + sy * cp * cr + sy * sp * sb * sr,
                -sy * cb * sr + cy * cp * cr + cy * sp * sb * sr,
                -sp * cr + cp * sb * sr,
            ];
            let col2 = [-cy * sb + sy * sp * cb, sy * sb + cy * sp * cb, cp * cb];
            let mut m = [[0.0; 3]; 3];
            for r in 0..3 {
                m[r] = [col0[r], col1[r], col2[r]];
            }
            m
        }
        for &(pitch, yaw, bank, rotation) in &[
            (0.42f64, 0.0f64, 0.0f64, 0.0f64),
            (0.25, 2.6, 0.7, 0.0),
            (0.42, 0.9, -1.1, 0.3),
            (-0.8, -2.0, 0.4, -2.2),
            (1.3, 3.0, 2.5, 1.0),
        ] {
            let pitch_f = std::f64::consts::FRAC_PI_2 - pitch;
            let yaw_f = yaw - std::f64::consts::FRAC_PI_2;
            let roll = -rotation;
            let mut want = wgsl(-roll, -pitch_f, -bank, yaw_f);
            for v in want[0].iter_mut() {
                *v = -*v;
            }
            let got = solid_matrix(pitch, yaw, bank, rotation);
            for r in 0..3 {
                assert!(
                    close3(got[r], want[r], 1e-12),
                    "row {r} at ({pitch}, {yaw}, {bank}, {rotation}): {:?} vs {:?}",
                    got[r],
                    want[r]
                );
            }
        }
    }

    /// Rotation rolls the screen and nothing else: the view direction
    /// stays, and right and up turn in the screen plane by the angle,
    /// in the direction the plane's rotation turns its own screen
    /// (`escape_screen_to_world` rotates a screen offset by +rotation
    /// into the world, so screen-right lands at `(cos, sin)` in the
    /// unrotated right/up basis).
    #[test]
    fn rotation_rolls_the_solid_screen_as_it_rolls_the_plane() {
        for &(pitch, yaw, bank) in &[(0.42f64, 0.9f64, 0.0f64), (0.25, 2.6, 0.7), (-1.0, -1.0, -0.5)] {
            let (r0, u0, f0) = solid_frame(pitch, yaw, bank, 0.0);
            for &theta in &[0.3f64, -1.2, 2.9] {
                let (r, u, f) = solid_frame(pitch, yaw, bank, theta);
                let (s, c) = theta.sin_cos();
                let want_r = [c * r0[0] + s * u0[0], c * r0[1] + s * u0[1], c * r0[2] + s * u0[2]];
                let want_u = [-s * r0[0] + c * u0[0], -s * r0[1] + c * u0[1], -s * r0[2] + c * u0[2]];
                assert!(close3(f, f0, 1e-12), "the roll moved the view direction");
                assert!(close3(r, want_r, 1e-12), "right at {theta}: {r:?} vs {want_r:?}");
                assert!(close3(u, want_u, 1e-12), "up at {theta}: {u:?} vs {want_u:?}");
            }
        }
    }

    /// Whatever the four angles, the frame is orthonormal and a
    /// physical camera's -- right = forward × up, the plane's
    /// handedness -- including at the poles, which the old cross
    /// product could not reach and the clamp kept it from.
    #[test]
    fn the_frame_is_a_frame_at_every_angle_including_the_poles() {
        let angles = [-std::f64::consts::FRAC_PI_2, -1.0, 0.0, 0.7, std::f64::consts::FRAC_PI_2];
        for &pitch in &angles {
            for &yaw in &[0.0f64, 1.1, -2.5] {
                for &bank in &[0.0f64, 0.8, -2.0] {
                    for &rotation in &[0.0f64, -0.4, 2.0] {
                        let (r, u, f) = solid_frame(pitch, yaw, bank, rotation);
                        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
                        assert!((dot(r, r) - 1.0).abs() < 1e-12 && (dot(u, u) - 1.0).abs() < 1e-12);
                        assert!(dot(r, u).abs() < 1e-12 && dot(r, f).abs() < 1e-12 && dot(u, f).abs() < 1e-12);
                        assert!(close3(cross3(f, u), r, 1e-12), "not a physical camera at ({pitch}, {yaw}, {bank}, {rotation})");
                    }
                }
            }
        }
    }

    /// A pixel's step at the target: the frame's height in world units
    /// over its pixels, as a mantissa and a power of two, agreeing with
    /// the camera's own distance wherever f64 can hold that.
    #[test]
    fn the_pixel_step_is_the_frames_height_over_its_pixels() {
        let guard = global_registry();
        let flame = gpu_tests::menger_flame();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&flame, &guard).expect("qualifies");
        for &zoom in &[0.0f64, 7.5, 40.0, 900.0] {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.zoom_log2 = zoom;
            let cam = solid_camera(&esc, &ifs3);
            let want = 2.0 * (cam.fov as f64 * 0.5).tan() * cam.distance / 480.0;
            let (m, e) = solid_pixel_step(&esc, &ifs3, 480.0);
            let got = m * 2f64.powi(e as i32);
            assert!(
                ((got - want) / want).abs() < 1e-12,
                "zoom {zoom}: {got:e} vs {want:e}"
            );
            assert!((1.0..2.0).contains(&m), "mantissa {m} is not normalised");
        }
        // And past f64: the mantissa is still a number, the exponent
        // carries the depth.
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.zoom_log2 = 5000.0;
        let (m, e) = solid_pixel_step(&esc, &ifs3, 480.0);
        assert!(m.is_finite() && (1.0..2.0).contains(&m) && e < -5000);
    }

    fn square() -> Ifs2 {
        let mut fl = Flame::default();
        fl.transforms = vec![half(0.0, 0.0, 0.1), half(0.5, 0.0, 0.4), half(0.0, 0.5, 0.7)];
        fl.final_transforms.clear();
        fl.xaos = None;
        let guard = global_registry();
        analyse_2d(&fl, &guard).expect("qualifies")
    }

    /// The classical affine IFSs, as flames with a mode-D view.
    ///
    /// Built here rather than hand-written as JSON because the VIEW
    /// comes from the analysis: the bounding ball the walk already
    /// computes is exactly what frames the attractor, so a preset
    /// cannot be committed pointing somewhere the set is not.
    pub(super) fn classical_presets() -> Vec<crate::config::FractalConfig> {
        use std::collections::HashMap;

        fn xform(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, color: f32) -> Transform {
            let mut t = Transform::default();
            t.a = a;
            t.b = b;
            t.c = c;
            t.d = d;
            t.e = e;
            t.f = f;
            t.color = color;
            t.weight = 1.0;
            t.variations = HashMap::from([("linear".to_string(), 1.0)]);
            t.variation_order = vec!["linear".to_string()];
            t
        }
        // A pure scale-and-shift, the shape most classical IFSs are.
        let scale = |s: f32, tx: f32, ty: f32, color: f32| xform(s, 0.0, 0.0, s, tx, ty, color);

        let third = 1.0f32 / 3.0;
        let (c60, s60) = (0.5f32, 0.866_025_4f32);

        // Sierpinski carpet: the 3x3 subdivision minus its centre.
        let mut carpet = Vec::new();
        for i in 0..3u32 {
            for j in 0..3u32 {
                if i == 1 && j == 1 {
                    continue;
                }
                let n = carpet.len() as f32;
                carpet.push(scale(third, i as f32 / 3.0, j as f32 / 3.0, n / 8.0));
            }
        }

        let cases: Vec<(&str, &str, Vec<Transform>)> = vec![
            (
                "Sierpinski Gasket",
                "ifs_address",
                vec![
                    scale(0.5, 0.0, 0.0, 0.1),
                    scale(0.5, 0.5, 0.0, 0.5),
                    scale(0.5, 0.25, 0.5, 0.9),
                ],
            ),
            ("Sierpinski Carpet", "ifs_level", carpet),
            (
                "Heighway Dragon",
                "ifs_distance",
                vec![
                    xform(0.5, -0.5, 0.5, 0.5, 0.0, 0.0, 0.2),
                    xform(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0, 0.8),
                ],
            ),
            (
                "Koch Curve",
                "ifs_address",
                vec![
                    scale(third, 0.0, 0.0, 0.05),
                    xform(third * c60, -third * s60, third * s60, third * c60, third, 0.0, 0.35),
                    xform(third * c60, third * s60, -third * s60, third * c60, 0.5, s60 / 3.0, 0.65),
                    scale(third, 2.0 * third, 0.0, 0.95),
                ],
            ),
        ];

        let registry = global_registry();
        cases
            .into_iter()
            .map(|(name, coloring, transforms)| {
                let mut c = crate::config::FractalConfig::default();
                c.render_mode = crate::scene::transforms::RenderMode::Escape;
                c.flame.name = name.to_string();
                c.flame.transforms = transforms;
                c.flame.final_transforms.clear();
                c.flame.xaos = None;

                let ifs = crate::scene::ifs_analysis::analyse_2d(&c.flame, &registry)
                    .unwrap_or_else(|why| {
                        panic!("{name} must qualify, but: {why:?}");
                    });

                c.escape.formula = "ifs_flame".to_string();
                c.escape.coloring = coloring.to_string();
                // A preset is a still, so the two whose pieces meet keep
                // the exact beam rather than the default: the dragon's
                // two pieces share a boundary and the Koch's four touch
                // end to end, and at the default of 4 the dragon
                // measured 16 pixels short at 384 and the Koch 358 at
                // 1080p. The gasket and the carpet measured identical.
                if name == "Heighway Dragon" || name == "Koch Curve" {
                    c.escape.formula_params.insert("beam".to_string(), 8.0);
                }
                c.escape.center_re = format!("{}", ifs.ball.centre[0]);
                c.escape.center_im = format!("{}", ifs.ball.centre[1]);
                // The home view spans 4 units, so 2.4 radii leaves the
                // attractor a comfortable margin — the same framing the
                // panel's Frame button applies.
                c.escape.zoom_log2 = (4.0 / (ifs.ball.radius * 2.4)).log2();
                // No supersampling. Mode D's edge is antialiased
                // ANALYTICALLY, from the sub-pixel value of the
                // distance, and its other colourings are smooth fields
                // that do not alias — so supersampling would be four
                // times the walk for a picture that already has exact
                // edges.

                // Entering escape mode resets these in the app; a saved
                // config has to carry them, or a flame's Log-calibrated
                // tonemap renders the Linear output invisibly.
                c.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
                c.exposure = crate::config::defaults::DEFAULT_EXPOSURE;
                c.gamma = crate::config::defaults::DEFAULT_GAMMA;
                c
            })
            .collect()
    }

    /// The solid classical IFSs, as flames with a solid-mode view.
    ///
    /// Separate from the planar ones because qualifying is a different
    /// question per dimension: an Apophysis-style transform is a
    /// perfectly good planar map and has unit scale in z, so a 2D
    /// preset loaded into the solid formula renders nothing. Phase 0's
    /// census measured that the other way round — none of 34 shipped
    /// 3D candidates qualified as solid — which is why these had to be
    /// built rather than found.
    pub(super) fn solid_presets() -> Vec<crate::config::FractalConfig> {
        let registry = global_registry();
        [
            // The angles are chosen, not defaulted. Both of these
            // sets have symmetry axes, and a camera on one renders a
            // solid as a flat emblem -- the tetrahedron seen down its
            // axis is exactly the planar Sierpinski gasket, which is a
            // poor advertisement for a renderer whose whole claim is
            // the third dimension.
            ("Sierpinski Tetrahedron", gpu_tests::tetrahedron_flame(), 2.6, 0.25),
            ("Menger Sponge", gpu_tests::menger_flame(), 0.9, 0.42),
        ]
        .into_iter()
        .map(|(name, flame, yaw, pitch)| {
            let mut c = crate::config::FractalConfig::default();
            c.render_mode = crate::scene::transforms::RenderMode::Escape;
            c.flame = flame;
            c.flame.name = name.to_string();

            crate::scene::ifs_analysis::analyse_3d(&c.flame, &registry)
                .unwrap_or_else(|why| panic!("{name} must qualify as solid, but: {why:?}"));

            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            // The camera frames itself: an empty target means the
            // attractor's own centre, and `zoom_log2 = 0` is the
            // distance at which the bounding ball fills the frame. A
            // preset does not have to know where its own solid is.
            c.escape.zoom_log2 = 0.0;
            c.escape.cam_yaw = yaw;
            c.escape.cam_pitch = pitch;

            // The halo is a PLANAR idea — it fades by distance from the
            // set, and on a surface every visible point is ON it — so
            // it would dim the whole solid uniformly.
            c.escape.coloring_params.insert("reach".to_string(), 0.0);

            // A key and a fill, from the Solid Rendering panel's own
            // settings -- so a preset arrives lit the way the panel
            // would describe it, and the panel is live the moment it
            // is opened rather than starting from nothing.
            //
            // White, and no specular, on purpose: the cached recolour
            // carries the lighting as a single scalar, which is the
            // whole answer exactly when the lighting IS a multiplier.
            // A coloured light is three multipliers and a specular is
            // a term added rather than a scale, so either would send a
            // palette drag back through the walk -- about 590 ms a
            // frame at 1080p against 20. Both are one click away for
            // a still.
            c.solid_shading.shading_strength = 1.0;
            c.solid_shading.ambient = 0.2;
            c.solid_shading.diffuse = 0.95;
            c.solid_shading.specular = 0.0;
            c.solid_shading.ssao_strength = 1.0;
            c.solid_shading.lights[0] = crate::config::SolidLight {
                enabled: true,
                azimuth: 35.0,
                elevation: 35.0,
                intensity: 1.0,
                color: [1.0, 1.0, 1.0],
            };
            c.solid_shading.lights[1] = crate::config::SolidLight {
                enabled: true,
                azimuth: -95.0,
                elevation: 10.0,
                intensity: 0.35,
                color: [1.0, 1.0, 1.0],
            };

            c.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
            c.exposure = crate::config::defaults::DEFAULT_EXPOSURE;
            // Not the default gamma of 4, which is a curve for
            // DENSITY. A solid render is not an accumulation being
            // rescued from the dark; it is already an image, and the
            // shader hands the tonemap LINEAR light -- it decodes the
            // palette with `pow(srgb, 2.2)` precisely so the lighting
            // multiplies in linear. So the display curve it wants is
            // the matching sRGB encode, and anything flatter than that
            // lifts the darks until the shading is gone: at gamma 4
            // the eightfold drop from a lit surface to the ambient
            // floor lands inside two deciles of output, which is a
            // sponge with no shadows in its holes.
            c.gamma = 2.2;
            c
        })
        .collect()
    }

    #[test]
    #[ignore = "one-shot generator"]
    fn write_the_classical_ifs_presets() {
        // Through the config's OWN serialiser, not serde directly: it
        // stamps `version`, and a config without one is migrated on
        // load as if it were v2 — which lifts `render_mode` out of the
        // `flame` object and so OVERWRITES the top-level Escape with
        // the flame's absent default. The preset then renders as a
        // FLAME, silently and plausibly, which is how this was missed
        // until a preset drew 539 pixels of chaos game.
        let json: Vec<serde_json::Value> = classical_presets()
            .into_iter()
            .chain(solid_presets())
            .chain(super::gpu_tests::julia_presets())
            .chain(super::gpu_tests::kernel_presets())
            .chain(super::gpu_tests::quaternion_presets())
            .collect::<Vec<_>>()
            .iter()
            .map(|c| {
                serde_json::from_str(&c.to_json().expect("serialise")).expect("reparse")
            })
            .collect();
        let json = serde_json::to_string_pretty(&json).expect("serialise");
        std::fs::create_dir_all("output").expect("output dir");
        std::fs::write("output/ifs-presets.json", json).expect("write");
        println!("wrote output/ifs-presets.json");
    }

    /// The shipped mode-D presets must still qualify, and their saved
    /// view must actually contain the attractor.
    ///
    /// A preset is the one thing a user meets before they know what
    /// the criterion is, so a mode-D preset whose flame stopped
    /// qualifying would render an empty frame with an explanation
    /// they did not ask for. And a preset pointing somewhere the set
    /// is not renders empty for a different reason entirely, which is
    /// worse — nothing says which of the two went wrong.
    #[test]
    fn the_shipped_ifs_presets_still_qualify_and_frame_their_attractor() {
        let registry = global_registry();
        let (mut planar, mut solid) = (0, 0);
        for cfg in crate::resources::presets::load_embedded_presets().expect("presets parse") {
            let Some(def) = get_ifs(&cfg.escape.formula) else { continue };
            let name = cfg.flame.name.clone();
            if !def.needs_flame {
                // No criterion to pass: the set is the def's own, and
                // its view is a camera about the origin.
                assert!(
                    IFS_COLORINGS.iter().any(|c| c.name == cfg.escape.coloring),
                    "preset {name:?} names coloring {:?}, not a mode-D one",
                    cfg.escape.coloring
                );
                continue;
            }

            // A SOLID preset answers a different question: qualifying
            // in 3D is not qualifying in 2D, and its view is a camera
            // rather than a centre and a span.
            if def.solid {
                solid += 1;
                let ifs3 = crate::scene::ifs_analysis::analyse_3d(&cfg.flame, &registry)
                    .unwrap_or_else(|why| {
                        panic!("solid preset {name:?} no longer qualifies: {why:?}")
                    });
                assert!(
                    IFS_COLORINGS.iter().any(|c| c.name == cfg.escape.coloring),
                    "solid preset {name:?} names coloring {:?}, not a mode-D one",
                    cfg.escape.coloring
                );
                let cam = solid_camera(&cfg.escape, &ifs3);
                let subtended = (ifs3.ball.radius / cam.distance).asin();
                assert!(
                    subtended < (cam.fov as f64) * 0.5,
                    "solid preset {name:?} does not frame its attractor: it subtends                      {subtended:.3} against a half-fov of {:.3}",
                    cam.fov as f64 * 0.5
                );
                continue;
            }

            planar += 1;
            let ifs = crate::scene::ifs_analysis::analyse_2d(&cfg.flame, &registry)
                .unwrap_or_else(|why| panic!("preset {name:?} no longer qualifies: {why:?}"));

            // The coloring must belong to mode D, or the render falls
            // back to the def's default and the preset is not the
            // picture it was saved as.
            assert!(
                IFS_COLORINGS.iter().any(|c| c.name == cfg.escape.coloring),
                "preset {name:?} names coloring {:?}, which is not a mode-D coloring",
                cfg.escape.coloring
            );

            // The saved view must hold the whole bounding ball.
            let span_y = 4.0 / 2f64.powf(cfg.escape.zoom_log2);
            let (cx, cy) = cfg.escape.center_f64();
            let dx = cx - ifs.ball.centre[0];
            let dy = cy - ifs.ball.centre[1];
            let off = (dx * dx + dy * dy).sqrt();
            assert!(
                span_y * 0.5 >= ifs.ball.radius + off,
                "preset {name:?} frames {span_y:.4} vertically, but its attractor needs \
                 {:.4} (ball radius {:.4}, view offset {off:.4})",
                (ifs.ball.radius + off) * 2.0,
                ifs.ball.radius
            );
        }
        assert_eq!(planar, 8, "expected eight planar IFS presets (four classical, three julia, one blob), found {planar}");
        assert_eq!(solid, 2, "expected two solid IFS presets, found {solid}");
    }

    #[test]
    fn registry_names_are_unique_and_disjoint_from_the_other_modes() {
        let mut seen = std::collections::HashSet::new();
        for d in IFS_DEFS {
            assert!(seen.insert(d.name), "duplicate mode-D name {}", d.name);
            assert!(
                !crate::escape::FORMULAS.iter().any(|m| m.name == d.name),
                "{} shadows a mode-A formula",
                d.name
            );
            assert!(
                crate::escape::fields::get_field(d.name).is_none(),
                "{} shadows a mode-B field",
                d.name
            );
        }
        seen.clear();
        for c in IFS_COLORINGS {
            assert!(seen.insert(c.name), "duplicate mode-D coloring {}", c.name);
        }
    }

    #[test]
    fn default_colorings_resolve_and_foreign_names_fall_back() {
        for d in IFS_DEFS {
            assert!(
                IFS_COLORINGS.iter().any(|c| c.name == d.default_coloring),
                "{} declares unknown default coloring {}",
                d.name,
                d.default_coloring
            );
            // A mode-A coloring name, the state right after a switch.
            assert_eq!(get_ifs_coloring("smooth", d).name, d.default_coloring);
            assert_eq!(get_ifs_coloring("ifs_level", d).name, "ifs_level");
        }
    }

    /// The GPU tests walk the CPU reference at [`BEAM`] and compare
    /// against a shader that reads its own default. If the def's
    /// default moves and the mirror does not, the comparison quietly
    /// stops comparing like with like.
    #[test]
    fn the_shipped_beam_and_depth_defaults_are_what_the_gpu_tests_mirror() {
        let get = |name: &str| {
            IFS_FLAME
                .parameters
                .iter()
                .find(|p| p.name == name)
                .map(|p| p.default)
                .expect("parameter present")
        };
        assert_eq!(get("beam"), gpu_tests::BEAM as f32);
        assert_eq!(get("levels"), gpu_tests::LEVELS as f32);
    }

    /// Every path that drives an `EscapeRenderer` must hand it the
    /// analysed flame.
    ///
    /// Mode D is the one escape formula that is NOT a function of the
    /// escape config alone, so a render path that never calls
    /// `set_ifs` draws an empty frame — correctly, quietly, and
    /// indistinguishably from a flame that does not qualify. The app's
    /// viewport was exactly that until this test existed.
    #[test]
    fn every_escape_render_path_supplies_the_ifs() {
        for (path, src) in [
            ("src/app/mod.rs", include_str!("../app/mod.rs")),
            ("src/renderer/render.rs", include_str!("../renderer/render.rs")),
        ] {
            assert!(
                src.contains("EscapeRenderer") || src.contains("escape_renderer"),
                "{path} no longer drives the escape renderer; drop it from this list"
            );
            assert!(
                src.contains("set_ifs"),
                "{path} renders escape mode but never calls set_ifs, so mode D                  draws nothing there"
            );
        }
    }

    /// A 1080p mode-D view must BAND, and the bands must be small
    /// enough that the driver never sees a multi-second dispatch.
    ///
    /// This is the test the first cut of mode D did not have. Its cost
    /// was modelled as `levels * maps`, which left out the beam
    /// entirely and was 250x low in absolute terms, so a 1080p view
    /// dispatched all 2 million pixels at once, ground for seconds and
    /// took the driver with it. The breaker that exists for exactly
    /// this cannot help, because it only engages once a render is
    /// banded — and this arithmetic is what decides whether it ever is.
    #[test]
    fn a_1080p_mode_d_view_bands_into_dispatches_a_driver_will_survive() {
        use crate::escape::renderer::{ifs_rows_per_dispatch, IFS_DISPATCH_BUDGET};

        // Measured: 1.5e9 walk steps per second (see the budget's docs).
        const STEPS_PER_MS: u64 = 1_500_000;

        for &(label, w, h) in &[
            ("1080p", 1920u32, 1080u32),
            ("1080p at 2x supersample", 3840, 2160),
            ("4K", 3840, 2160),
            ("8K at 2x supersample", 15360, 8640),
        ] {
            for &(levels, beam, maps) in &[(24u32, 8u32, 3usize), (40, 8, 8), (160, 8, 20)] {
                let rows =
                    ifs_rows_per_dispatch(w, h, levels, beam, maps, IFS_DISPATCH_BUDGET);
                assert!(rows >= 1, "{label}: {rows} rows");
                let steps = (w as u64)
                    * (rows as u64)
                    * (levels as u64)
                    * (beam as u64)
                    * (maps as u64);
                let ms = steps / STEPS_PER_MS;
                assert!(
                    ms < 700,
                    "{label} at levels {levels}, beam {beam}, {maps} maps: a band of \
                     {rows} rows is ~{ms} ms, past the point the breaker calls slow"
                );
            }
        }

        // And it must actually band rather than sending the frame whole.
        let rows = ifs_rows_per_dispatch(1920, 1080, 24, 8, 3, IFS_DISPATCH_BUDGET);
        assert!(rows < 1080, "1080p should band, got {rows} rows of 1080");

        // A cheap view should NOT be chopped up for nothing: banding
        // costs a resolve pass each time.
        let rows = ifs_rows_per_dispatch(512, 512, 24, 1, 2, IFS_DISPATCH_BUDGET);
        assert_eq!(rows, 512, "a small cheap view should render in one dispatch");
    }

    /// A solid view must band too, and raising the march must shrink
    /// the bands rather than lengthen them.
    ///
    /// The marcher walks the distance function once per STEP, so its
    /// per-pixel cost has a factor the planar model does not. Leaving
    /// that out is the same class of mistake that hung a 1080p planar
    /// view: the estimate stays small, the render never bands, and the
    /// driver sees a dispatch measured in seconds.
    #[test]
    fn a_solid_view_bands_and_the_march_is_in_the_estimate() {
        use crate::escape::renderer::{ifs_rows_per_dispatch, IFS_SOLID_BUDGET};
        // The Menger sponge at 1080p and the shipped defaults: twenty
        // maps, depth 24, beam 1, 96 steps -- with two shadowed lights
        // (two more marches), the twelve occlusion probes and the six
        // the normal takes, which is what the caller adds up.
        let rows = ifs_rows_per_dispatch(1920, 1080, 24, 3 * 96 + 12 + 6, 20, IFS_SOLID_BUDGET);
        assert!(rows < 1080, "1080p should band, got {rows} of 1080");
        assert!(rows > 50, "and not into slivers: {rows} rows");

        // Turning shadows off is half the work and must buy back the
        // band, or the second march is not in the estimate at all.
        let unshadowed = ifs_rows_per_dispatch(1920, 1080, 24, 1 * 96 + 12 + 6, 20, IFS_SOLID_BUDGET);
        assert!(
            unshadowed > rows,
            "the shadow march is not in the band estimate ({unshadowed} against {rows})"
        );

        // Asking for a longer march must make the bands smaller.
        let longer = ifs_rows_per_dispatch(1920, 1080, 24, 2 * 512, 20, IFS_SOLID_BUDGET);
        assert!(
            longer < rows,
            "raising the step count did not shrink the band ({longer} against {rows})"
        );
        // So must a wider beam.
        let wider = ifs_rows_per_dispatch(1920, 1080, 24, 8 * 2 * 96, 20, IFS_SOLID_BUDGET);
        assert!(wider < rows, "a wider beam did not shrink the band");

        // And the breaker still reaches it.
        let halved = ifs_rows_per_dispatch(1920, 1080, 24, 2 * 96, 20, IFS_SOLID_BUDGET >> 1);
        assert!(halved < rows, "the budget shift does not reach a solid view");
    }

    /// The breaker's halvings have to reach mode D, or a device that
    /// cannot hold a 300 ms band has no way to say so.
    #[test]
    fn the_budget_shift_still_shrinks_mode_d_bands() {
        use crate::escape::renderer::{ifs_rows_per_dispatch, IFS_DISPATCH_BUDGET};
        let full = ifs_rows_per_dispatch(1920, 1080, 24, 8, 3, IFS_DISPATCH_BUDGET);
        let halved = ifs_rows_per_dispatch(1920, 1080, 24, 8, 3, IFS_DISPATCH_BUDGET >> 1);
        assert!(halved < full, "shift 1 gave {halved} rows against {full}");
        let deep = ifs_rows_per_dispatch(1920, 1080, 24, 8, 3, IFS_DISPATCH_BUDGET >> 6);
        assert!(deep >= 1, "shift 6 must still make progress, got {deep}");
    }

    /// A NaN anywhere in the packed data must not read as "changed".
    ///
    /// The app re-analyses the flame every frame and marks the escape
    /// image dirty when the answer differs. Under `PartialEq` a single
    /// NaN — a transform colour is enough — answers "changed" forever,
    /// so the view re-renders a band every frame and never settles.
    /// That is a permanent redraw loop, and it looks exactly like the
    /// engine being too slow rather than like a bug.
    #[test]
    fn a_nan_in_the_packed_data_does_not_read_as_a_change_every_frame() {
        let ifs = square();
        let colors = vec![0.1f32, 0.4, 0.7];
        let mut packed = PackedIfs {
            xaos: vec![0.0; 4],
            measure: crate::scene::ifs_estimate::MeasureMaps {
                prob: Vec::new(),
                step: None,
                colour: Vec::new(),
            },
            globals: [[0.0; 4]; 4],
            rows: pack_maps(&ifs, &colors, None),
            ifs: ifs.clone(),
            colors,
            solid: None,
        };
        pack_globals(&ifs, &mut packed.globals);

        // Same value, so nothing changed.
        assert!(packed_bytes_eq(Some(&packed), Some(&packed.clone())));
        assert!(packed_bytes_eq(None, None));
        assert!(!packed_bytes_eq(Some(&packed), None));

        // A NaN colour: `PartialEq` would call this a change.
        let mut nanned = packed.clone();
        nanned.rows[1].color = f32::NAN;
        assert!(
            nanned != nanned.clone(),
            "the premise: PartialEq calls a NaN-carrying value different from itself"
        );
        assert!(
            packed_bytes_eq(Some(&nanned), Some(&nanned.clone())),
            "a NaN-carrying pack must still compare equal to itself, or the app              re-renders forever"
        );

        // And a real change is still seen.
        let mut moved = packed.clone();
        moved.rows[0].inv_t[0] += 1.0;
        assert!(!packed_bytes_eq(Some(&packed), Some(&moved)));
        let mut reframed = packed.clone();
        reframed.globals[0][2] += 1.0;
        assert!(!packed_bytes_eq(Some(&packed), Some(&reframed)));
    }

    /// A long exact decimal for `num/den`, by long division.
    pub(super) fn decimal(num: u64, den: u64, digits: usize) -> String {
        let mut out = format!("{}.", num / den);
        let mut r = num % den;
        for _ in 0..digits {
            r *= 10;
            out.push((b'0' + (r / den) as u8) as char);
            r %= den;
        }
        out
    }

    /// The fixed point of S₁∘S₂∘S₃ on the Sierpiński tetrahedron,
    /// which is `(6, 5, 3)/7`.
    ///
    /// Sevenths, and that is the whole point. Every f64 is dyadic, and
    /// the maps here halve — so a dyadic target's inverse orbit lands
    /// exactly on a fixed point and its picture is self-similar at any
    /// precision, which is how the plane's first deep-zoom gate
    /// managed to pass while measuring nothing. A seventh is not
    /// dyadic, so holding it costs real digits.
    fn tetra_cycle_target(limbs: usize) -> [crate::escape::bigfloat::BigFloat; 3] {
        let big = |num: u64| {
            crate::escape::bigfloat::BigFloat::from_fixed(
                &crate::escape::fixedpoint::FixedPoint::from_decimal(
                    &super::tests::decimal(num, 7, 80),
                    limbs,
                )
                .expect("parses"),
            )
        };
        [big(6), big(5), big(3)]
    }

    /// The seeded 3D walk holds its place where f64 cannot — asserted,
    /// with the naive walk as the control.
    ///
    /// `C = S₁∘S₂∘S₃` is a similarity of ratio ⅛ fixing the target and
    /// mapping the attractor into itself, so the set looks the same
    /// about that point at every scale and `d(T + δ)·8ᵏ` is a
    /// CONSTANT. That is the whole gate, and it needs no ground truth
    /// of its own.
    ///
    /// The control is the point. A test that only asserted the seeded
    /// column is constant would also pass if the chain quietly formed
    /// the absolute position like the naive walk does — so the naive
    /// column is required to FAIL, and by a wide margin, before the
    /// seeded column's constancy is allowed to mean anything.
    ///
    /// See [`how_deep_a_seeded_3d_walk_still_knows_where_it_is`] for
    /// the same measurement printed rather than asserted.
    #[test]
    fn a_seeded_3d_walk_holds_a_target_f64_cannot_express() {
        let guard = global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&gpu_tests::tetrahedron_flame(), &guard)
            .expect("qualifies");
        drop(guard);

        let target = tetra_cycle_target(8);
        let t64 = {
            use crate::scene::ifs_estimate::SeedPoint3;
            target.to_f64()
        };
        let dir = {
            let v = [0.37f64, -0.61, 0.7];
            let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            [v[0] / n, v[1] / n, v[2] / n]
        };
        let chain = crate::scene::ifs_estimate::seed_chain3(
            &ifs3,
            target,
            2f64.powi(-140),
            256,
            1,
        );

        let mut reference = 0.0f64;
        let mut worst_seeded = 0.0f64;
        let mut worst_naive = 0.0f64;
        for k in 0..=45u32 {
            let scale = 8f64.powi(-(k as i32));
            let delta = [dir[0] * 0.2 * scale, dir[1] * 0.2 * scale, dir[2] * 0.2 * scale];
            let levels = (3 * k + 40).min(256);

            let seeded = crate::scene::ifs_estimate::estimate_seeded3(
                &ifs3, &chain, delta, levels, 1,
            )
            .distance
                / scale;
            let naive = crate::scene::ifs_estimate::estimate(
                &ifs3,
                [t64[0] + delta[0], t64[1] + delta[1], t64[2] + delta[2]],
                levels,
                1,
            )
            .distance
                / scale;

            if k == 0 {
                reference = seeded;
                assert!(reference > 1e-3, "the sample is not off the set: {reference}");
                assert!(
                    (naive / reference - 1.0).abs() < 1e-9,
                    "the two disagree at the SHALLOW end, where both should be exact: \
                     {naive} against {reference}"
                );
            }
            worst_seeded = worst_seeded.max((seeded / reference - 1.0).abs());
            worst_naive = worst_naive.max((naive / reference - 1.0).abs());
        }

        assert!(
            worst_seeded < 1e-9,
            "the seeded walk drifted by {worst_seeded:.3e} over 45 octaves of ⅛ -- \
             it is not holding the target"
        );
        // The control. Without this the assertion above is also passed
        // by a chain that never left f64.
        assert!(
            worst_naive > 1.0,
            "the NAIVE walk did not fail ({worst_naive:.3e}) -- this fixture is not \
             deep enough to be measuring precision at all, so the line above proves \
             nothing"
        );
    }

    /// How deep the seeded 3D walk still knows where it is.
    ///
    /// `C = S₁∘S₂∘S₃` is a similarity of ratio ⅛ that fixes the target
    /// and maps the attractor into itself, so the set looks the same
    /// about that point at every scale: `d(T + δ/8)` must be `d(T + δ)/8`.
    /// Printing `d · 8ᵏ` therefore prints a CONSTANT, for as long as
    /// the arithmetic can still tell where `T + δ` is — and the level
    /// at which the column stops being constant is the answer.
    ///
    /// Both ways are printed side by side because one of them is the
    /// control. The naive column forms the absolute position in f64
    /// and loses it at about 2⁻⁵², which is the wall; without that
    /// column, a chain that was quietly doing the same thing would
    /// look like a result.
    #[test]
    #[ignore = "prints a measurement"]
    fn how_deep_a_seeded_3d_walk_still_knows_where_it_is() {
        let guard = global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&gpu_tests::tetrahedron_flame(), &guard)
            .expect("qualifies");
        drop(guard);

        let target = tetra_cycle_target(8);
        let t64 = {
            use crate::scene::ifs_estimate::SeedPoint3;
            target.to_f64()
        };
        // An offset in a direction the set actually has structure in.
        let dir = {
            let v = [0.37f64, -0.61, 0.7];
            let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            [v[0] / n, v[1] / n, v[2] / n]
        };

        let chain = crate::scene::ifs_estimate::seed_chain3(
            &ifs3,
            target.clone(),
            2f64.powi(-140),
            256,
            1,
        );
        println!(
            "  chain: {} links, cap {:.4}",
            chain.levels.len(),
            chain.cap
        );
        println!("   k    |delta|     naive f64 * 8^k    seeded * 8^k     link");
        for k in (0..46).step_by(3) {
            let scale = 8f64.powi(-(k as i32));
            let delta = [dir[0] * 0.2 * scale, dir[1] * 0.2 * scale, dir[2] * 0.2 * scale];
            let len = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
            let levels = (3 * k as u32 + 40).min(256);

            let naive = crate::scene::ifs_estimate::estimate(
                &ifs3,
                [t64[0] + delta[0], t64[1] + delta[1], t64[2] + delta[2]],
                levels,
                1,
            )
            .distance;
            let seeded =
                crate::scene::ifs_estimate::estimate_seeded3(&ifs3, &chain, delta, levels, 1);
            let link = chain.level_for(len);
            println!(
                "  {k:>2}  {len:>10.3e}  {:>16.9e}  {:>14.9e}  {link:>4}",
                naive / scale,
                seeded.distance / scale
            );
        }
    }

    /// A view whose CENTRE is off the attractor must still draw the
    /// attractor.
    ///
    /// Reported as "sections disappear when they are mostly
    /// off-screen", which is the same thing from the outside: pan
    /// until the set is at the edge of the frame and the middle of the
    /// view is empty space.
    ///
    /// It was the handover. A seed carries the CENTRE's running bound
    /// and its escape, and the continuation starts every pixel from
    /// them -- so once the centre left the bounding ball, its positive
    /// bound was inherited by pixels INSIDE it, as a running maximum
    /// nothing later could lower. A point sitting exactly on the
    /// attractor then reported the centre's distance to the ball:
    /// measured at 7.333 against a true zero, which renders as empty
    /// space.
    ///
    /// The probe is a point ON the set, kept near the edge of the
    /// frame but inside it, because outside the frame is outside what
    /// the seeding promises. The comparison is the unseeded walk,
    /// which has no handover and so cannot have this fault.
    #[test]
    fn a_view_centred_off_the_set_still_finds_it() {
        let guard = global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&gpu_tests::sierpinski_flame(), &guard)
            .expect("qualifies");
        drop(guard);

        let on_set = ifs.maps[0].forward.fixed_point().expect("contractive");
        let exact = crate::scene::ifs_estimate::estimate(&ifs, on_set, 40, gpu_tests::BEAM)
            .distance;
        assert!(exact < 1e-9, "the probe is not on the set: {exact}");

        // The probe sits at 0.4 of the way to the frame's edge, so the
        // view centre is 0.4 * span from it. At the wide end that puts
        // the centre well outside the ball -- which is the case that
        // failed -- and at the narrow end just beside the set.
        let mut worst: (f64, f64) = (0.0, 0.0);
        for &span in &[8.0f64, 4.0, 2.0, 1.0, 0.5, 0.25, 0.0625] {
            let centre = [on_set[0] + 0.4 * span, on_set[1]];
            let basis = view_basis(span, span, 0.0);
            let px = span / 512.0;
            let seeds = crate::scene::ifs_estimate::seed_beam(
                &ifs,
                centre,
                basis,
                px,
                64,
                gpu_tests::BEAM,
            );
            let uv = [-0.4, 0.0];
            // The seeded walk reports in PIXELS, which is the only
            // form that survives a deep zoom -- back to world units to
            // compare.
            let seeded = crate::scene::ifs_estimate::estimate_seeded(
                &ifs,
                &seeds,
                uv,
                40,
                gpu_tests::BEAM,
            )
            .distance
                * px;
            if seeded > worst.0 {
                worst = (seeded, span);
            }
        }
        assert!(
            worst.0 < 1e-6,
            "a point ON the set reads {:.4} away at span {:.3} -- the seed is handing \
             every pixel the centre's own distance, and the set renders as empty space",
            worst.0,
            worst.1,
        );
    }

    /// How many levels a walk actually NEEDS before its answer stops
    /// changing at pixel precision.
    ///
    /// The walk runs every level it is given -- its only early exit is
    /// a candidate a trillion radii away -- so this asks, for points at
    /// a range of distances from the sponge, at which level the running
    /// bound gets within one pixel of where it will finish. Everything
    /// past that level is work the picture cannot show.
    #[test]
    #[ignore = "prints a measurement"]
    fn how_many_levels_a_walk_actually_needs() {
        let guard = global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&gpu_tests::menger_flame(), &guard)
            .expect("qualifies");
        drop(guard);
        let r = ifs3.ball.radius;
        // A 1080p pixel at the home zoom, in world units.
        let px = 2.0 * (0.35f64).tan() * (3.2 * r) / 1080.0;

        let mut rng = 12345u64;
        let mut next = || {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((rng >> 11) as f64) / ((1u64 << 53) as f64)
        };

        println!("  px = {px:.2e}   (levels needed = first level within one px of the level-40 answer)");
        println!("  distance band      samples   mean needed   p90   max   (of 40)");
        for &(lo, hi) in &[(0.0f64, 0.01), (0.01, 0.05), (0.05, 0.2), (0.2, 0.5), (0.5, 1.0)] {
            let mut needed = Vec::new();
            let mut tries = 0;
            while needed.len() < 400 && tries < 200_000 {
                tries += 1;
                let p = [next() * 1.4 - 0.2, next() * 1.4 - 0.2, next() * 1.4 - 0.2];
                let full = crate::scene::ifs_estimate::estimate(&ifs3, p, 40, 1).distance;
                if full < lo || full >= hi {
                    continue;
                }
                let mut need = 40u32;
                for lv in 1..=40u32 {
                    let d = crate::scene::ifs_estimate::estimate(&ifs3, p, lv, 1).distance;
                    if (full - d).abs() <= px {
                        need = lv;
                        break;
                    }
                }
                needed.push(need);
            }
            needed.sort_unstable();
            let n = needed.len().max(1);
            let mean = needed.iter().map(|&x| x as f64).sum::<f64>() / n as f64;
            println!(
                "  [{lo:>4.2}, {hi:<4.2})       {:>5}     {mean:>6.1}      {:>3}   {:>3}",
                needed.len(),
                needed.get(n * 9 / 10).copied().unwrap_or(0),
                needed.last().copied().unwrap_or(0)
            );
        }
    }

    /// Does the seeded walk draw the same picture as each pixel's own
    /// walk would?
    ///
    /// Reported as glitchiness: sections disappearing as the zoom
    /// changes, structure warping rather than magnifying, regions
    /// swapping in 2D, and Beam Width mattering. The suspicion is the
    /// handover. The CPU walks the beam from the view CENTRE and every
    /// pixel continues from the centre's surviving branches -- so if a
    /// pixel's own nearest piece was pruned from that beam, its
    /// continuation never sees it, reads a distance to some other
    /// piece, and renders as exterior. Which pixels that hits depends
    /// on how deep the handover went, which depends on the zoom: the
    /// picture would change with the zoom in ways that are not
    /// magnification.
    ///
    /// Measured against the unseeded walk at the same world point,
    /// which is exact at these zooms and knows nothing of any centre.
    ///
    /// Before the handover stopped at the first level the view did not
    /// agree on: gasket z8 15%, dragon z8 beam 2 **41%**, dragon z14
    /// beam 1 6%, and 0% in the cells between -- the zoom dependence
    /// the report describes. After: 0% in every cell of both tables.
    #[test]
    #[ignore = "prints a measurement"]
    fn does_the_seeded_walk_agree_with_each_pixels_own() {
        for (name, flame) in [
            ("gasket", gpu_tests::sierpinski_flame()),
            ("dragon", gpu_tests::dragon_flame()),
        ] {
            println!("  {name}");
            println!("   zoom  beam  seeded levels   disagree (inner / outer quarter of the frame)");
            for &zoom in &[2.0f64, 5.0, 8.0, 11.0, 14.0] {
                for &beam in &[1u32, 2, 4, 8] {
                    let (level, inner, outer) = seeded_walk_disagreement(&flame, zoom, beam, 128);
                    println!(
                        "   {zoom:>4}  {beam:>3}       {level:>3}          {:>5.1}%  /  {:>5.1}%",
                        100.0 * inner,
                        100.0 * outer
                    );
                }
            }
        }
    }

    /// The gate on the measurement above: the cells that were worst,
    /// and one that was clean, at a quarter of the resolution.
    #[test]
    fn the_seeded_walk_is_each_pixels_own() {
        for (name, flame) in [
            ("gasket", gpu_tests::sierpinski_flame()),
            ("dragon", gpu_tests::dragon_flame()),
        ] {
            for &(zoom, beam) in &[(8.0f64, 1u32), (8.0, 2), (8.0, 4), (14.0, 1), (14.0, 8)] {
                let (level, inner, outer) = seeded_walk_disagreement(&flame, zoom, beam, 32);
                assert!(
                    inner == 0.0 && outer == 0.0,
                    "{name} z{zoom} beam {beam} (handover at level {level}): \
                     {:.1}% of the inner and {:.1}% of the outer frame disagree",
                    100.0 * inner,
                    100.0 * outer
                );
            }
        }
    }

    /// The fraction of pixels, inner and outer quarter of an `n`² frame,
    /// on which the seeded walk and the pixel's own walk disagree by
    /// more than a pixel or one percent -- with the level the handover
    /// reached.
    fn seeded_walk_disagreement(
        flame: &crate::scene::transforms::Flame,
        zoom: f64,
        beam: u32,
        n: u32,
    ) -> (u32, f64, f64) {
        let guard = global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(flame, &guard).expect("qualifies");
        let r = ifs.ball.radius;
        // A centre on the set but not dyadic, a little off the ball's
        // centre so the beam has something to choose.
        let centre = [ifs.ball.centre[0] + 0.137 * r, ifs.ball.centre[1] - 0.211 * r];
        let span = 4.0 / 2f64.powf(zoom);
        let px = span / n as f64;
        let basis = view_basis(span, span, 0.0);
        let levels = 40u32;
        let seeds = crate::scene::ifs_estimate::seed_beam(&ifs, centre, basis, px, 64, beam);
        let (mut inner, mut inner_bad, mut outer, mut outer_bad) = (0, 0, 0, 0);
        for y in 0..n {
            for x in 0..n {
                let uv = [(x as f64 + 0.5) / n as f64 - 0.5, (y as f64 + 0.5) / n as f64 - 0.5];
                let world = [centre[0] + span * uv[0], centre[1] - span * uv[1]];
                let seeded =
                    crate::scene::ifs_estimate::estimate_seeded(&ifs, &seeds, uv, levels, beam)
                        .distance
                        * px;
                let own =
                    crate::scene::ifs_estimate::estimate(&ifs, world, levels + seeds.level, beam)
                        .distance;
                // A pixel, or one percent, whichever is larger: far
                // outside the set the distance is hundreds of frames
                // and nothing is drawn, and there two equally valid
                // lineages of an overlapping set converge a fraction
                // of a percent apart.
                let bad = (seeded - own).abs() > px.max(0.01 * own);
                let is_inner = uv[0].abs() < 0.25 && uv[1].abs() < 0.25;
                if is_inner {
                    inner += 1;
                    inner_bad += bad as usize;
                } else {
                    outer += 1;
                    outer_bad += bad as usize;
                }
            }
        }
        (seeds.level, inner_bad as f64 / inner as f64, outer_bad as f64 / outer as f64)
    }

    /// The 3D twin: does the seeded chain agree with each sample's own
    /// walk?
    ///
    /// A link commits every sample that uses it to the target's
    /// branches, and a sample can sit up to the cap from the target in
    /// the link's frame -- a quarter of the ball, which on the sponge
    /// is most of a sub-cube. So the samples along a ray at the frame's
    /// edge may need a branch the target's beam pruned, exactly as the
    /// plane's pixels did. Measured against the unseeded walk at the
    /// same world point, which is exact at these zooms.
    ///
    /// Measured: 0% in every cell, with the chain at beam 1 as deep as
    /// before the rule (17 links on the tetrahedron at z14, 11 on the
    /// sponge) and at beams 2 and 4 one or two links -- these sets
    /// tile, so their branches tie and the samples never agree on the
    /// second. See [`seed_chain3`] for why that is accepted.
    #[test]
    #[ignore = "prints a measurement"]
    fn does_the_seeded_chain_agree_with_each_samples_own() {
        for (name, flame) in [
            ("tetrahedron", gpu_tests::tetrahedron_flame()),
            ("menger", gpu_tests::menger_flame()),
        ] {
            println!("  {name}");
            println!("   zoom  beam  links   disagree (inner / outer quarter of the frame)");
            for &zoom in &[2.0f64, 5.0, 8.0, 11.0, 14.0] {
                for &beam in &[1u32, 2, 4] {
                    let (links, inner, outer) = seeded_chain_disagreement(&flame, zoom, beam, 48);
                    println!(
                        "   {zoom:>4}  {beam:>3}   {links:>3}      {:>5.1}%  /  {:>5.1}%",
                        100.0 * inner,
                        100.0 * outer
                    );
                }
            }
        }
    }

    /// The gate on the measurement above, at the shipped beam and at a
    /// wider one, a third of the resolution. The chain must also be
    /// DEEP at the shipped beam: a rule that made the walk exact by
    /// ending the chain at once would pass the agreement and lose the
    /// zoom. The depth expected is the measurement's less the levels a
    /// three-times-coarser finest pixel does not need (17 and 11 at
    /// 48 across; 15 and 9 at 16).
    #[test]
    fn the_seeded_chain_is_each_samples_own() {
        for (name, flame, deep) in [
            ("tetrahedron", gpu_tests::tetrahedron_flame(), 15),
            ("menger", gpu_tests::menger_flame(), 9),
        ] {
            for &(zoom, beam) in &[(8.0f64, 1u32), (14.0, 1), (8.0, 2)] {
                let (links, inner, outer) = seeded_chain_disagreement(&flame, zoom, beam, 16);
                assert!(
                    inner == 0.0 && outer == 0.0,
                    "{name} z{zoom} beam {beam} ({links} links): \
                     {:.1}% of the inner and {:.1}% of the outer frame disagree",
                    100.0 * inner,
                    100.0 * outer
                );
                if zoom == 14.0 && beam == 1 {
                    assert!(links >= deep, "{name} z14 beam 1: {links} links, expected {deep}");
                }
            }
        }
    }

    /// The fraction of samples -- five per ray, from near the eye to
    /// past the target, on an `n`² frame split into its inner and outer
    /// quarter -- on which the seeded chain and the sample's own walk
    /// disagree by more than the finest pixel or one percent, with the
    /// chain's length.
    fn seeded_chain_disagreement(
        flame: &crate::scene::transforms::Flame,
        zoom: f64,
        beam: u32,
        n: u32,
    ) -> (usize, f64, f64) {
        let guard = global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(flame, &guard).expect("qualifies");
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.zoom_log2 = zoom;
        esc.cam_yaw = 0.9;
        esc.cam_pitch = 0.42;
        // A target on the set, off any symmetry.
        esc.cam_target_x = super::tests::decimal(6, 7, 30);
        esc.cam_target_y = super::tests::decimal(5, 7, 30);
        esc.cam_target_z = super::tests::decimal(3, 7, 30);
        let cam = solid_camera(&esc, &ifs3);
        let finest = 2.0 * (cam.fov as f64 * 0.5).tan() * cam.distance / n as f64;
        let chain = crate::scene::ifs_estimate::seed_chain3(&ifs3, cam.target, finest, 64, beam);
        let tan_half = (cam.fov as f64 * 0.5).tan();
        let (mut inner, mut inner_bad, mut outer, mut outer_bad) = (0, 0, 0, 0);
        for y in 0..n {
            for x in 0..n {
                let u = (x as f64 + 0.5) / n as f64 - 0.5;
                let v = (y as f64 + 0.5) / n as f64 - 0.5;
                let mut dir = [0.0f64; 3];
                for k in 0..3 {
                    dir[k] = cam.forward[k] + cam.right[k] * (u * 2.0 * tan_half)
                        - cam.up[k] * (v * 2.0 * tan_half);
                }
                let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                for &f in &[0.5f64, 0.9, 1.0, 1.1, 1.5] {
                    let t = cam.distance * f;
                    let delta = [
                        cam.eye_rel[0] + dir[0] / len * t,
                        cam.eye_rel[1] + dir[1] / len * t,
                        cam.eye_rel[2] + dir[2] / len * t,
                    ];
                    let world = [
                        cam.target[0] + delta[0],
                        cam.target[1] + delta[1],
                        cam.target[2] + delta[2],
                    ];
                    let seeded =
                        crate::scene::ifs_estimate::estimate_seeded3(&ifs3, &chain, delta, 40, beam)
                            .distance;
                    let own = crate::scene::ifs_estimate::estimate(
                        &ifs3,
                        world,
                        40 + chain.levels.len() as u32,
                        beam,
                    )
                    .distance;
                    let bad = (seeded - own).abs() > finest.max(0.01 * own);
                    let is_inner = u.abs() < 0.25 && v.abs() < 0.25;
                    if is_inner {
                        inner += 1;
                        inner_bad += bad as usize;
                    } else {
                        outer += 1;
                        outer_bad += bad as usize;
                    }
                }
            }
        }
        (
            chain.levels.len(),
            inner_bad as f64 / inner as f64,
            outer_bad as f64 / outer as f64,
        )
    }

    /// A deep zoom must hold a centre f64 cannot express.
    ///
    /// The earlier gates all centred somewhere exactly representable —
    /// 0.25, then the origin — which is precisely the thing that hides
    /// a precision wall, and did hide one. This one centres on
    /// **(5/14, 1/7)**, whose decimal expansion repeats forever.
    ///
    /// That point is the fixed point of `m₀∘m₁∘m₂`, a contraction by
    /// ⅛ that maps the attractor into itself — so the gasket around it
    /// is its own image at every scale of eight, and the distance
    /// field in PIXELS at zoom Z and at Z+3 must be the same field.
    /// Self-similarity again, but this time about a centre that only
    /// survives in `BigFloat`: at f64 the same test fails by 2⁵⁰.
    #[test]
    fn a_deep_zoom_holds_a_centre_f64_cannot_express() {
        let guard = global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&gpu_tests::sierpinski_flame(), &guard)
            .expect("qualifies");
        drop(guard);

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.center_re = decimal(5, 14, 400);
        esc.center_im = decimal(1, 7, 400);

        // f64 rounds this to 5.5e-17; the gate is that the render does
        // not.
        assert_ne!(
            esc.center_re.parse::<f64>().expect("parses").to_string(),
            esc.center_re[..20].to_string(),
            "the fixture must not be an f64-exact centre"
        );

        let field_at = |zoom: f64| -> Vec<f64> {
            let mut e = esc.clone();
            e.zoom_log2 = zoom;
            let span = 4.0 / 2f64.powf(zoom);
            let basis = view_basis(span, span, 0.0);
            let px = span / 32.0;
            let centre = centre_at_precision(&e).expect("centre parses");
            let seeds = crate::scene::ifs_estimate::seed_beam(
                &ifs,
                centre,
                basis,
                px,
                (zoom as u32) + 64,
                8,
            );
            let mut out = Vec::new();
            for i in 0..17 {
                for j in 0..17 {
                    let uv = [i as f64 / 16.0 - 0.5, j as f64 / 16.0 - 0.5];
                    out.push(
                        crate::scene::ifs_estimate::estimate_seeded(&ifs, &seeds, uv, 64, 8)
                            .distance,
                    );
                }
            }
            out
        };

        for &zoom in &[6.0f64, 30.0, 60.0, 120.0, 180.0] {
            let here = field_at(zoom);
            let octave = field_at(zoom + 3.0);
            let mut worst: f64 = 0.0;
            for (a, b) in here.iter().zip(&octave) {
                worst = worst.max((a - b).abs() / a.max(*b).max(1.0));
            }
            let spread = here.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                - here.iter().cloned().fold(f64::INFINITY, f64::min);
            println!(
                "  zoom 2^{zoom} vs 2^{}: worst {worst:.3e}, spread {spread:.2} px",
                zoom + 3.0
            );
            assert!(
                worst < 1e-3,
                "at zoom 2^{zoom} the picture is not self-similar three octaves down \
                 (worst {worst:.3e}) -- the centre has lost its precision"
            );
            assert!(spread > 1.0, "at zoom 2^{zoom} the field is flat ({spread:.3} px)");
        }
    }

    /// The centre's PRECISION is what makes the deep zoom work, and
    /// this is the test that says so.
    ///
    /// The self-similarity gate above is necessary but not sufficient,
    /// and the first control written for it proved the point: it asked
    /// whether an f64 centre's picture is self-similar, and it IS —
    /// every f64 is a dyadic rational, and a dyadic centre's inverse
    /// orbit runs out of fractional bits and lands exactly on a fixed
    /// point, so it is self-similar for a completely degenerate
    /// reason. A gate can pass for the wrong reason; this one asks the
    /// question directly instead.
    ///
    /// Two claims, at a zoom where they can be told apart:
    ///
    /// - the answer has CONVERGED in precision — more limbs does not
    ///   change it, so the walk is not still losing the centre;
    /// - f64 gives a DIFFERENT answer — so the precision is load
    ///   bearing, and the gate above is not passing for free.
    #[test]
    fn the_centres_precision_is_what_makes_the_deep_zoom_work() {
        let guard = global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&gpu_tests::sierpinski_flame(), &guard)
            .expect("qualifies");
        drop(guard);

        let re = decimal(5, 14, 400);
        let im = decimal(1, 7, 400);
        const ZOOM: f64 = 120.0;
        let span = 4.0 / 2f64.powf(ZOOM);
        let basis = view_basis(span, span, 0.0);
        let px = span / 32.0;

        let field = |centre: &dyn Fn() -> Box<dyn FnOnce() -> Vec<f64>>| centre()();
        let _ = field;

        let sample = |seeds: &crate::scene::ifs_estimate::Seeds| -> Vec<f64> {
            (0..13)
                .flat_map(|i| (0..13).map(move |j| (i, j)))
                .map(|(i, j)| {
                    let uv = [i as f64 / 12.0 - 0.5, j as f64 / 12.0 - 0.5];
                    crate::scene::ifs_estimate::estimate_seeded(&ifs, &seeds, uv, 64, 8).distance
                })
                .collect()
        };
        let big_at = |limbs: usize| -> Vec<f64> {
            let fp_re = crate::escape::fixedpoint::FixedPoint::from_decimal(&re, limbs).expect("re");
            let fp_im = crate::escape::fixedpoint::FixedPoint::from_decimal(&im, limbs).expect("im");
            let centre = [
                crate::escape::bigfloat::BigFloat::from_fixed(&fp_re),
                crate::escape::bigfloat::BigFloat::from_fixed(&fp_im),
            ];
            sample(&crate::scene::ifs_estimate::seed_beam(
                &ifs, centre, basis, px, 256, 8,
            ))
        };

        let n = crate::escape::fixedpoint::limbs_for_view(&re, &im, ZOOM);
        let at_n = big_at(n);
        let at_2n = big_at(n * 2);
        let worst_precision = at_n
            .iter()
            .zip(&at_2n)
            .map(|(a, b)| (a - b).abs() / a.max(*b).max(1.0))
            .fold(0.0f64, f64::max);
        println!("  {n} limbs vs {} limbs: worst {worst_precision:.3e}", n * 2);
        assert!(
            worst_precision < 1e-9,
            "doubling the precision changed the picture ({worst_precision:.3e}) --              {n} limbs is not enough for zoom 2^{ZOOM}"
        );

        let at_f64 = sample(&crate::scene::ifs_estimate::seed_beam(
            &ifs,
            [re.parse::<f64>().expect("re"), im.parse::<f64>().expect("im")],
            basis,
            px,
            256,
            8,
        ));
        let worst_f64 = at_n
            .iter()
            .zip(&at_f64)
            .map(|(a, b)| (a - b).abs() / a.max(*b).max(1.0))
            .fold(0.0f64, f64::max);
        println!("  {n} limbs vs f64:        worst {worst_f64:.3e}");
        assert!(
            worst_f64 > 1e-3,
            "an f64 centre gave the same picture at zoom 2^{ZOOM}              ({worst_f64:.3e}) -- then precision is not what the deep gate is              measuring, and that gate proves nothing"
        );
    }

    /// The solid row must be the sixty-four bytes the shader reads,
    /// with no `vec3` anywhere.
    ///
    /// A `vec3<f32>` aligns to sixteen bytes in WGSL and four in Rust.
    /// The obvious struct — three padded rows, a `vec3` translation,
    /// two scalars — is eighty bytes here and ninety-six there, so the
    /// shader reads every map but the first from the wrong offset. It
    /// does not fail: it renders a plausible-looking noisy blob, which
    /// is how it was found and why this test exists.
    #[test]
    fn the_solid_row_is_the_layout_the_shader_declares() {
        assert_eq!(std::mem::size_of::<IfsMap3Gpu>(), 128);
        assert_eq!(std::mem::offset_of!(IfsMap3Gpu, rows), 0);
        assert_eq!(std::mem::offset_of!(IfsMap3Gpu, extra), 48);
        assert_eq!(std::mem::offset_of!(IfsMap3Gpu, pre), 64);
        assert_eq!(std::mem::offset_of!(IfsMap3Gpu, extra2), 112);
        // And no vec3 in the shader's declaration either, which is the
        // half a size assertion cannot see.
        let src = crate::escape::assembler::assemble_ifs(&IFS_FLAME_3D, &IFS_ADDRESS, 8);
        let start = src.find("struct IfsMap3Gpu {").expect("declared");
        let decl = &src[start..start + src[start..].find("
}").expect("closes")];
        assert!(
            !decl.contains("vec3"),
            "the solid row declares a vec3, whose alignment differs between              WGSL and Rust: {decl}"
        );
    }

    /// The packed inverse must undo the forward map in the shader's
    /// own arithmetic — a transposed 3×3 renders a plausible wrong
    /// solid, the same way a transposed 2×2 renders a plausible wrong
    /// plane.
    #[test]
    fn packed_solid_inverses_undo_the_forward_maps() {
        let guard = global_registry();
        let flame = gpu_tests::tetrahedron_flame();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&flame, &guard).expect("qualifies");
        drop(guard);
        let colors: Vec<f32> = flame.transforms.iter().map(|t| t.color).collect();
        let rows = pack_maps3(&ifs3, &colors);
        assert_eq!(rows.len(), 4);

        for (row, m) in rows.iter().zip(ifs3.maps.iter()) {
            for &p in &[[0.3f64, -0.8, 0.4], [1.7, 2.4, -1.1], [0.0, 0.0, 0.0]] {
                let fwd = m.forward.apply(p);
                // Exactly what `ifs_inv_point3` computes.
                let back: Vec<f64> = (0..3)
                    .map(|i| {
                        row.rows[i][0] as f64 * fwd[0]
                            + row.rows[i][1] as f64 * fwd[1]
                            + row.rows[i][2] as f64 * fwd[2]
                            + row.rows[i][3] as f64
                    })
                    .collect();
                for k in 0..3 {
                    assert!(
                        (back[k] - p[k]).abs() < 1e-5,
                        "round trip {p:?} -> {fwd:?} -> {back:?}"
                    );
                }
            }
            assert!((row.extra[0] - m.sigma_min as f32).abs() < 1e-6, "sigma");
        }
    }

    /// The camera's frame must be right-handed and orthonormal, or
    /// the marcher's rays are sheared and nothing downstream can tell.
    #[test]
    fn the_solid_camera_is_an_orthonormal_frame_about_its_target() {
        let guard = global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&gpu_tests::tetrahedron_flame(), &guard)
            .expect("qualifies");
        drop(guard);

        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let len = |a: [f64; 3]| dot(a, a).sqrt();

        for &(pitch, yaw, zoom) in &[
            (0.0f32, 0.0f32, 0.0f64),
            (0.42, 0.9, 0.0),
            (-1.2, -2.5, 6.0),
            (1.5533, 3.14, 40.0),
            (-1.5533, 0.0, 200.0),
        ] {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.cam_pitch = pitch;
            esc.cam_yaw = yaw;
            esc.zoom_log2 = zoom;
            let c = solid_camera(&esc, &ifs3);

            for (name, v) in [("forward", c.forward), ("right", c.right), ("up", c.up)] {
                assert!(
                    (len(v) - 1.0).abs() < 1e-9,
                    "{name} is not unit at pitch {pitch} yaw {yaw}: {}",
                    len(v)
                );
            }
            assert!(dot(c.forward, c.right).abs() < 1e-9, "forward/right not square");
            assert!(dot(c.forward, c.up).abs() < 1e-9, "forward/up not square");
            assert!(dot(c.right, c.up).abs() < 1e-9, "right/up not square");

            // And it must LOOK at the target, not merely near it.
            //
            // Recovered by SUBTRACTING the two positions, which is
            // only meaningful while the gap is large enough for an f64
            // to hold: at zoom 2⁴⁰ the eye sits 2.5e-12 from a target
            // near 0.5, and the difference has three significant
            // digits left. That is not the camera losing the
            // direction — it is this check losing it — but it is the
            // same cancellation the SHADER meets, and
            // `a_solid_deep_zoom_is_limited_by_the_eye_not_the_target`
            // measures where.
            if zoom <= 20.0 {
                let to_target = [
                    c.target[0] - c.eye[0],
                    c.target[1] - c.eye[1],
                    c.target[2] - c.eye[2],
                ];
                let d = len(to_target);
                assert!((d - c.distance).abs() < 1e-9 * d.max(1.0), "distance disagrees");
                for k in 0..3 {
                    assert!(
                        (to_target[k] / d - c.forward[k]).abs() < 1e-9,
                        "forward does not point at the target at pitch {pitch} yaw {yaw}"
                    );
                }
            }
        }
    }

    /// Zooming approaches the target; it does not move it.
    ///
    /// That is the whole shape of a deep zoom, and the reason the
    /// target is stored as decimal strings while the distance is
    /// derived: the precision belongs to the point that holds still.
    #[test]
    fn zooming_approaches_the_target_and_leaves_it_alone() {
        let guard = global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&gpu_tests::tetrahedron_flame(), &guard)
            .expect("qualifies");
        drop(guard);

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.cam_target_x = "0.3333333333333333333333333333".to_string();
        esc.cam_target_y = "0.25".to_string();
        esc.cam_target_z = "0.125".to_string();

        let mut last = f64::INFINITY;
        for zoom in [0.0f64, 8.0, 40.0, 120.0] {
            esc.zoom_log2 = zoom;
            let c = solid_camera(&esc, &ifs3);
            assert!(c.distance < last, "zoom {zoom} did not approach");
            last = c.distance;
            // The target is where it was asked to be, to the digits it
            // was given -- not to an f32's worth of them.
            assert!(
                (c.target[0] - 1.0 / 3.0).abs() < 1e-15,
                "target drifted at zoom {zoom}: {}",
                c.target[0]
            );
        }
        // And each doubling of the zoom halves the distance.
        esc.zoom_log2 = 10.0;
        let a = solid_camera(&esc, &ifs3).distance;
        esc.zoom_log2 = 11.0;
        let b = solid_camera(&esc, &ifs3).distance;
        assert!((a / b - 2.0).abs() < 1e-9, "a zoom step is not an octave: {a} vs {b}");
    }

    /// An empty target means the attractor's own centre, so a flame
    /// just switched to is framed without being told where it is.
    #[test]
    fn an_empty_target_frames_the_attractor() {
        let guard = global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&gpu_tests::tetrahedron_flame(), &guard)
            .expect("qualifies");
        drop(guard);
        let esc = crate::config::escape::EscapeConfig::default();
        let c = solid_camera(&esc, &ifs3);
        for k in 0..3 {
            assert!((c.target[k] - ifs3.ball.centre[k]).abs() < 1e-12);
        }
        // At the home zoom the whole attractor is inside the frame.
        let half_angle = (c.fov as f64) * 0.5;
        let subtended = (ifs3.ball.radius / c.distance).asin();
        assert!(
            subtended < half_angle,
            "the attractor subtends {subtended:.3} against a half-fov of {half_angle:.3}"
        );
    }

    /// What limits a solid deep zoom now that the eye is an OFFSET.
    ///
    /// This test used to pin 2¹³, and the reason was the eye: the
    /// marcher was handed an absolute position in f32, which near a
    /// coordinate of order one is quantised to 6e-8, so past 2¹³ a
    /// pixel was finer than the eye's own rounding and every ray in
    /// the frame started from the same wrong place. The camera now
    /// carries `eye − target` directly — as a product of numbers its
    /// own size, never as a subtraction of two nearly equal ones —
    /// and that quantity shrinks WITH the zoom, so it never stops
    /// resolving a pixel.
    ///
    /// What is left is f32's exponent, and the binding constraint is
    /// not the offset itself but any SQUARE of it: the sphere trace
    /// and the link choice both once took a `length`, and a length
    /// squares before it adds. The link choice does not any more (see
    /// `ifs_pick_link`), which is what took the measured agreement
    /// from 2⁶⁴ to 2⁸⁰.
    ///
    /// So this measures the offset against f32's smallest NORMAL
    /// number, and against the smallest whose square is still normal —
    /// the two ceilings, and the gap between them is what the squaring
    /// question is worth. `how_deep_a_solid_render_agrees_with_the_reference`
    /// is the other half: what the picture actually does.
    #[test]
    fn what_limits_a_solid_deep_zoom_now_the_eye_is_an_offset() {
        let guard = global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&gpu_tests::tetrahedron_flame(), &guard)
            .expect("qualifies");
        drop(guard);

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.cam_target_x = "0.3333333333333333333333333333".to_string();

        let mut resolves = 0.0f64;
        let mut normal = 0.0f64;
        let mut square_normal = 0.0f64;
        for zoom in 0..300 {
            esc.zoom_log2 = zoom as f64;
            let c = solid_camera(&esc, &ifs3);
            let off = c.eye_rel[0]
                .abs()
                .max(c.eye_rel[1].abs())
                .max(c.eye_rel[2].abs());
            // One pixel across a 1080-tall frame, at the target.
            let px = 2.0 * (c.fov as f64 * 0.5).tan() * c.distance / 1080.0;
            // The offset keeps its RELATIVE precision, so what it can
            // resolve about itself is a part in ten million of itself
            // -- not of a coordinate of order one, which was the old
            // wall.
            if (off * f32::EPSILON as f64) < px {
                resolves = zoom as f64;
            }
            if off > f32::MIN_POSITIVE as f64 {
                normal = zoom as f64;
            }
            if off * off > f32::MIN_POSITIVE as f64 {
                square_normal = zoom as f64;
            }
        }
        println!(
            "  the eye offset resolves a pixel to 2^{resolves}, stays normal in f32 to \
             2^{normal}, and its SQUARE stays normal only to 2^{square_normal}"
        );

        // The offset resolves a pixel at every zoom this loop reaches:
        // both sides shrink together, so the ratio is a constant and
        // there is no crossing to find. That is the whole point of
        // carrying an offset.
        assert!(
            resolves >= 299.0,
            "the eye offset stopped resolving a pixel at 2^{resolves} -- it should never \
             stop, because a relative precision divided by a relative precision is a \
             constant"
        );
        // And the ceilings, which are about the exponent rather than
        // the mantissa. MEASURED: an f32 holds the offset itself to
        // about 2^125, and its square only to about 2^62 -- so any
        // squaring of a delta halves the reachable zoom, which is
        // exactly what `length()` was doing in the link choice.
        assert!(
            (120.0..=130.0).contains(&normal),
            "f32's normal range for the eye offset moved to 2^{normal}"
        );
        assert!(
            (58.0..=68.0).contains(&square_normal),
            "the squaring ceiling moved to 2^{square_normal} -- it is half the other \
             one, and it is the reason nothing on this path may take a length"
        );
    }

    /// Does the tetrahedron shadow itself at all, and from where?
    ///
    /// Asked on the CPU, of the distance function itself, because the
    /// alternative is reading occlusion off a render -- and a render
    /// that shows no shadows cannot tell a light with nothing to block
    /// it from a march that steps over blockers.
    #[test]
    #[ignore = "prints a measurement"]
    fn how_much_does_the_tetrahedron_shadow_itself() {
        let guard = global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&gpu_tests::tetrahedron_flame(), &guard)
            .expect("qualifies");
        drop(guard);

        // Points ON the set: the fixed points of deep addresses, which
        // is the cheapest way to get a surface sample that is really
        // on the attractor rather than near it.
        let mut surface: Vec<[f64; 3]> = Vec::new();
        let n = ifs3.maps.len();
        for seed in 0..2048usize {
            let mut p = ifs3.ball.centre;
            let mut x = seed * 2654435761 + 12345;
            for _ in 0..40 {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let i = (x >> 33) % n;
                p = ifs3.maps[i].forward.apply(p);
            }
            surface.push(p);
        }

        for el_deg in [2.0f64, 15.0, 40.0, 70.0] {
            for az_deg in [0.0f64, 45.0, 135.0, 225.0, 315.0] {
                let (el, az) = (el_deg.to_radians(), az_deg.to_radians());
                let l = [el.cos() * az.cos(), el.cos() * az.sin(), el.sin()];
                let mut blocked = 0usize;
                let mut min_clear = f64::INFINITY;
                for &p in &surface {
                    // March the true distance function toward the
                    // light, exactly as the shader does.
                    // Start clear of the surface by more than the
                    // hit threshold, or the first sample reports the
                    // point shadowing itself -- which reads as "every
                    // direction is blocked".
                    const EPS: f64 = 2e-4;
                    let mut t = EPS * 20.0;
                    let mut clear: f64 = 1.0;
                    for _ in 0..256 {
                        if t > 4.0 {
                            break;
                        }
                        let q = [p[0] + l[0] * t, p[1] + l[1] * t, p[2] + l[2] * t];
                        let d = crate::scene::ifs_estimate::estimate(&ifs3, q, 24, 4).distance;
                        if d < EPS {
                            clear = 0.0;
                            break;
                        }
                        clear = clear.min(12.0 * d / t);
                        t += d;
                    }
                    if clear < 0.5 {
                        blocked += 1;
                    }
                    min_clear = min_clear.min(clear);
                }
                println!(
                    "  el {el_deg:>4} az {az_deg:>5}: {blocked:>5} of {} points in shadow, \
                     darkest clearance {min_clear:.3}",
                    surface.len()
                );
            }
        }
    }

    /// Only a solid formula gets the 3D controls (D2).
    #[test]
    fn the_camera_belongs_to_the_formula_not_the_mode() {
        assert!(formula_is_solid("ifs_flame_3d"));
        assert!(!formula_is_solid("ifs_flame"));
        assert!(!formula_is_solid("mandelbrot"));
        assert!(!formula_is_solid("weierstrass"));
        assert!(!formula_is_solid("nonexistent"));
    }

    #[test]
    fn lookup_routes_to_mode_d_only() {
        assert!(get_ifs("ifs_flame").is_some());
        assert!(get_ifs("mandelbrot").is_none());
        assert!(get_ifs("weierstrass").is_none());
        assert!(get_ifs("nonexistent").is_none());
    }

    /// The packed inverse must actually invert the forward map: a
    /// transposed or column-major slip here would render a plausible
    /// but wrong picture, which is the hardest kind to notice.
    #[test]
    fn packed_inverses_undo_the_forward_maps_in_the_shader_s_layout() {
        let ifs = square();
        let rows = pack_maps(&ifs, &[0.1, 0.4, 0.7], None);
        assert_eq!(rows.len(), 3);

        for (row, m) in rows.iter().zip(ifs.maps.iter()) {
            for &p in &[[0.3f64, -0.8], [1.7, 2.4], [0.0, 0.0]] {
                let fwd = m.forward.apply(p);
                // Exactly the arithmetic `ifs_inv_point` performs.
                let back = [
                    row.inv_m[0] as f64 * fwd[0] + row.inv_m[1] as f64 * fwd[1]
                        + row.inv_t[0] as f64,
                    row.inv_m[2] as f64 * fwd[0] + row.inv_m[3] as f64 * fwd[1]
                        + row.inv_t[1] as f64,
                ];
                assert!(
                    (back[0] - p[0]).abs() < 1e-6 && (back[1] - p[1]).abs() < 1e-6,
                    "round trip {p:?} -> {fwd:?} -> {back:?}"
                );
            }
        }
    }

    #[test]
    fn packed_rows_carry_sigma_and_the_transform_colour_in_order() {
        let ifs = square();
        let rows = pack_maps(&ifs, &[0.1, 0.4, 0.7], None);
        for r in &rows {
            assert!((r.sigma_min - 0.5).abs() < 1e-6, "sigma {r:?}");
        }
        assert_eq!(rows.iter().map(|r| r.color).collect::<Vec<_>>(), vec![0.1, 0.4, 0.7]);
        // A colour list shorter than the flame (a caller bug) must not
        // panic mid-render.
        let short = pack_maps(&ifs, &[0.1], None);
        assert_eq!(short[2].color, 0.0);
    }

    #[test]
    fn globals_describe_the_ball_and_the_count() {
        let ifs = square();
        let mut out = [[0.0f32; 4]; 8];
        pack_globals(&ifs, &mut out);
        assert_eq!(out[0][3], 3.0, "map count");
        assert!(out[0][2] > 0.0, "radius");
        assert!((out[1][0] - 0.5).abs() < 1e-6, "mean sigma {out:?}");
    }

    /// The seeds must land where the shader's accessors read them,
    /// and must not tread on the globals below.
    #[test]
    fn seeds_pack_where_the_shader_reads_them() {
        let ifs = square();
        let span = 0.5f64;
        let seeds = crate::scene::ifs_estimate::seed_beam(
            &ifs,
            ifs.ball.centre,
            view_basis(span, span, 0.0),
            span / 64.0,
            200,
            4,
        );
        let mut out = [[0.0f32; 4]; 4 + SEED_VEC4S * MAX_SEEDS];
        pack_globals(&ifs, &mut out);
        let mean_before = out[1][0];
        // A packing test: the prefix fold is gated separately, so an
        // empty map table is the right stand-in here.
        let measure = crate::scene::ifs_estimate::MeasureMaps {
            prob: vec![1.0; ifs.maps.len()],
            step: None,
            colour: vec![(0.0, 0.0); ifs.maps.len()],
        };
        pack_seeds(&measure, &seeds, ifs.maps.len(), &[0.1, 0.4, 0.7], &mut out);

        assert_eq!(out[1][0], mean_before, "pack_seeds trod on the mean sigma");
        assert_eq!(out[1][1], seeds.level as f32, "handover level");
        assert_eq!(out[1][2] as usize, seeds.cands.len().min(MAX_SEEDS), "seed count");
        assert!(out[1][3] > 0.0, "address scale");

        // Seed 0's position and basis, where `ifs_seed(0, 0)` looks.
        let c = &seeds.cands[0];
        assert!((out[SEED_BASE][0] - c.position[0] as f32).abs() < 1e-6);
        assert!((out[SEED_BASE][2] - c.basis[0][0] as f32).abs() < 1e-6);
        assert!((out[SEED_BASE + 1][2] - c.sigma_per_px as f32).abs() < 1e-3);
        // And nothing beyond the seeds it wrote.
        let used = SEED_BASE + SEED_VEC4S * seeds.cands.len().min(MAX_SEEDS);
        for v in &out[used..] {
            assert_eq!(*v, [0.0; 4], "wrote past the seeds it has");
        }
    }

    /// The GPU row must match what WGSL's std430 rules read: 80 bytes,
    /// two halves of the same shape and a vec4 of kernel parameters,
    /// with the scalars trailing a vec2 rather than straddling a
    /// 16-byte boundary.
    /// The packed coarse grid reads back as the measure it came from.
    ///
    /// `pack_coarse` flattens a [`CoarseMeasure`] for the shader and
    /// `read_coarse` is the reader the shader will mirror. A packer
    /// with no reader beside it is a stride waiting to disagree --
    /// `SEED_VEC4S` went four to six with the shader's stride left a
    /// literal four, invisible on one seed and 11% wrong on eight.
    #[test]
    fn the_packed_coarse_grid_is_the_measure_it_came_from() {
        use crate::scene::ifs_estimate::CoarseMeasure;
        let res = 32usize;
        let mut c = CoarseMeasure {
            res,
            centre: [0.25, -0.5],
            radius: 1.75,
            hits: vec![0; res * res],
            palette_sum: vec![0.0; res * res],
            samples: 1_000_000,
        };
        // A pattern with holes in it, so the empty-cell path is
        // exercised and not just the populated one.
        for i in 0..res * res {
            if i % 7 != 0 {
                c.hits[i] = (i % 97) as u32;
                c.palette_sum[i] = c.hits[i] as f64 * ((i % 13) as f64 / 13.0);
            }
        }
        let packed = pack_coarse(&c);
        assert_eq!(packed.len(), COARSE_HEADER + res * res);

        let cell = c.cell();
        let mut checked = 0;
        let mut empty = 0;
        for iy in 0..res {
            for ix in 0..res {
                // The cell's middle, and a point near its corner, so a
                // rounding difference in the index would show.
                for (ox, oy) in [(0.5, 0.5), (0.02, 0.98)] {
                    let q = [
                        c.centre[0] - c.radius + (ix as f64 + ox) * cell,
                        c.centre[1] - c.radius + (iy as f64 + oy) * cell,
                    ];
                    let (d, pal) = read_coarse(&packed, [q[0] as f32, q[1] as f32]);
                    let want_d = c.density(q);
                    let want_p = c.palette(q);
                    assert!(
                        (d as f64 - want_d).abs() <= want_d.abs() * 1e-5 + 1e-9,
                        "cell ({ix},{iy}): packed density {d} against {want_d}"
                    );
                    assert!(
                        (pal as f64 - want_p).abs() < 1e-5,
                        "cell ({ix},{iy}): packed palette {pal} against {want_p}"
                    );
                    if c.hits[iy * res + ix] == 0 {
                        empty += 1;
                    }
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, res * res * 2);
        assert!(empty > 0, "the fixture has no empty cells, so that path is untested");

        // Outside the grid reads as no measure, which is what makes an
        // escaped lineage contribute nothing.
        for q in [
            [c.centre[0] - c.radius * 1.5, c.centre[1]],
            [c.centre[0], c.centre[1] + c.radius * 1.5],
        ] {
            let (d, _) = read_coarse(&packed, [q[0] as f32, q[1] as f32]);
            assert_eq!(d, 0.0, "outside the ball must read no measure");
        }
    }

    #[test]
    fn probe_row_colour_order() {
        use crate::scene::transforms::{Flame, Transform};
        let aff = |a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, col: f32| {
            let mut t = Transform::default();
            t.a = a; t.b = b; t.c = c; t.d = d; t.e = e; t.f = f;
            t.color = col;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            t
        };
        let mut flame = Flame::default();
        flame.transforms = vec![
            aff(0.5, -0.5, 0.5, 0.5, 0.0, 0.0, 0.0),
            aff(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0, 1.0),
        ];
        let guard = crate::variations::global_registry();
        let packed = pack_flame(&flame, &guard, None).expect("qualifies");
        drop(guard);
        for (i, r) in packed.rows.iter().enumerate() {
            println!(
                "  row {i}: color {} prob {} speed {} inv_t {:?}",
                r.color, r.measure[0], r.measure[1], r.inv_t
            );
        }
        for (i, m) in packed.ifs.maps.iter().enumerate() {
            println!("  map {i}: transform_index {}", m.transform_index);
        }
    }

    #[test]
    fn probe_packed_prefix() {
        use crate::scene::transforms::{Flame, Transform};
        let aff = |a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, col: f32| {
            let mut t = Transform::default();
            t.a = a; t.b = b; t.c = c; t.d = d; t.e = e; t.f = f;
            t.color = col;
            t.color_speed = 0.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            t
        };
        let mut flame = Flame::default();
        flame.transforms = vec![
            aff(0.5, -0.5, 0.5, 0.5, 0.0, 0.0, 0.0),
            aff(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0, 1.0),
        ];
        let guard = crate::variations::global_registry();
        let packed = pack_flame(&flame, &guard, None).expect("qualifies");
        drop(guard);
        println!("  measure.prob {:?}", packed.measure.prob);
        let span = 0.2;
        let basis = view_basis(span, span, 0.0);
        for level in [0u32, 1, 2] {
            let seeds = crate::scene::ifs_estimate::seed_beam_at(
                &packed.ifs, [0.3f64, 0.2], basis, span / 64.0, 80, 8, level,
            );
            let mut out = [[0.0f32; 4]; 4 + SEED_VEC4S * MAX_SEEDS];
            pack_seeds(&packed.measure, &seeds, packed.rows.len(), &packed.colors, &mut out);
            let n = seeds.cands.len();
            println!(
                "  asked {level} got {} n {n} addrs {:?} probs {:?}",
                seeds.level,
                seeds.cands.iter().map(|c| c.address.clone()).collect::<Vec<_>>(),
                (0..n).map(|j| out[SEED_BASE + SEED_VEC4S * j + 3][3]).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn the_gpu_row_is_the_layout_the_shader_declares() {
        assert_eq!(std::mem::size_of::<IfsMapGpu>(), 96);
        assert_eq!(std::mem::align_of::<IfsMapGpu>(), 4);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, inv_m), 0);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, inv_t), 16);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, sigma_min), 24);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, color), 28);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, pre_m), 32);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, pre_t), 48);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, kind), 56);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, branch), 60);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, params), 64);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, measure), 80);
        // std430 rounds a struct's stride to its largest member's
        // alignment, and `inv_m` is a vec4. A size that is not a
        // multiple of 16 strides differently in the shader than in
        // Rust and misreads every row after the first.
        assert_eq!(std::mem::size_of::<IfsMapGpu>() % 16, 0);
    }
}

/// The transcription gate: the shader's walk against the CPU
/// reference.
///
/// [`crate::scene::ifs_estimate`] gates the ALGORITHM against IFSs
/// with closed-form answers. These gate that the shader computes the
/// same thing — the packing, the reference/delta split, the view
/// mapping and the colouring — which is the half a CPU test cannot
/// reach and the half where a transposed matrix renders a plausible
/// wrong picture.
///
/// Classes, not values: a pixel is compared as inside-the-set or
/// outside-it, never by its exact colour, so nothing here can be
/// perturbed by f32 rounding, a palette change or a tonemap tweak.
#[cfg(test)]
mod gpu_tests {
    use super::*;
    use crate::scene::ifs_estimate::estimate;
    use crate::scene::transforms::{Flame, RenderMode, Transform};
    use crate::variations::global_registry;
    use std::collections::HashMap;

    const W: u32 = 96;
    const H: u32 = 96;
    /// Frames the gasket: centre (0.5, 0.4), vertical span 2.
    const CENTRE: [f64; 2] = [0.5, 0.4];
    const ZOOM_LOG2: f64 = 1.0;
    pub(super) const LEVELS: u32 = 24;
    /// The def parameter default, mirrored so the CPU reference walks
    /// the same beam the shader does.
    pub(super) const BEAM: u32 = 4;

    pub(super) fn device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("adapter");
        // The flame renderer is built on every render_with job
        // whatever the mode, and its compute layout wants nine
        // storage buffers where the default limit is eight.
        let adapter_limits = adapter.limits();
        let mut limits = wgpu::Limits::default();
        limits.max_storage_buffers_per_shader_stage =
            adapter_limits.max_storage_buffers_per_shader_stage;
        limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
        limits.max_buffer_size = adapter_limits.max_buffer_size;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("ifs test"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            ..Default::default()
        }))
        .expect("device")
    }

    fn half(tx: f32, ty: f32) -> Transform {
        let mut t = Transform::default();
        t.a = 0.5;
        t.b = 0.0;
        t.c = 0.0;
        t.d = 0.5;
        t.e = tx;
        t.f = ty;
        t.variations = HashMap::from([("linear".to_string(), 1.0)]);
        t.variation_order = vec!["linear".to_string()];
        t
    }

    pub(super) fn sierpinski_flame() -> Flame {
        let mut fl = Flame::default();
        fl.transforms = vec![half(0.0, 0.0), half(0.5, 0.0), half(0.25, 0.5)];
        fl.final_transforms.clear();
        fl.xaos = None;
        fl
    }

    /// A general affine transform in the analysis module's row-major
    /// reading: `x' = a·x + b·y + e`, `y' = c·x + d·y + f`.
    fn xform(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Transform {
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

    /// Four half-scale maps tiling `[0,1]²`: the attractor is the
    /// filled square, which is the one case with an exact distance.
    fn square_flame() -> Flame {
        flame_of(vec![half(0.0, 0.0), half(0.5, 0.0), half(0.0, 0.5), half(0.5, 0.5)])
    }

    /// The Heighway dragon: two similarities of ratio 1/sqrt(2), each
    /// a 45-degree rotation.
    pub(super) fn dragon_flame() -> Flame {
        flame_of(vec![
            xform(0.5, -0.5, 0.5, 0.5, 0.0, 0.0),
            xform(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0),
        ])
    }

    /// The Koch curve: four similarities of ratio 1/3, two of them
    /// rotated by +/-60 degrees.
    fn koch_flame() -> Flame {
        let s = 1.0f32 / 3.0;
        let (c60, s60) = (0.5f32, 0.8660254f32);
        flame_of(vec![
            xform(s, 0.0, 0.0, s, 0.0, 0.0),
            xform(s * c60, -s * s60, s * s60, s * c60, s, 0.0),
            xform(s * c60, s * s60, -s * s60, s * c60, 0.5, s60 / 3.0),
            xform(s, 0.0, 0.0, s, 2.0 * s, 0.0),
        ])
    }

    /// A flame that fails the criterion: `spherical` is not affine,
    /// and it is the commonest reason in the phase-0 census.
    /// The gasket with one transform folded by `sinusoidal`, which
    /// nothing inverts. (It was `spherical` until plan 8.9 made that a
    /// kernel and the fixture started qualifying.)
    fn folded_flame() -> Flame {
        let mut fl = sierpinski_flame();
        fl.transforms[1].variations = HashMap::from([("sinusoidal".to_string(), 1.0)]);
        fl.transforms[1].variation_order = vec!["sinusoidal".to_string()];
        fl
    }

    fn config_for(flame: Flame) -> crate::config::FractalConfig {
        let mut c = crate::config::FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.flame = flame;
        c.escape.formula = "ifs_flame".to_string();
        c.escape.coloring = "ifs_distance".to_string();
        c.escape.center_re = "0.5".to_string();
        c.escape.center_im = "0.4".to_string();
        c.escape.zoom_log2 = ZOOM_LOG2;
        c.escape.rotation = 0.0;
        c.escape.supersample = 1;
        c.escape.formula_params.insert("levels".to_string(), LEVELS as f32);
        // A mid-palette interior so "lit" is not accidentally black,
        // and no contour bands so luminance IS coverage.
        c.escape.coloring_params.insert("interior".to_string(), 0.5);
        c.escape.coloring_params.insert("bands".to_string(), 0.0);
        c.escape.coloring_params.insert("edge".to_string(), 1.0);
        // What entering escape mode does in the app (render_mode.rs):
        // a flame's Log-calibrated tonemap renders Linear output
        // invisibly.
        c.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
        c.exposure = crate::config::defaults::DEFAULT_EXPOSURE;
        c.gamma = crate::config::defaults::DEFAULT_GAMMA;
        c
    }

    fn render(config: &crate::config::FractalConfig) -> Vec<u8> {
        let (device, queue) = device();
        let job = crate::renderer::RenderJob::new(config, W, H);
        let out = pollster::block_on(crate::renderer::render(
            &device,
            &queue,
            job,
            &mut crate::renderer::NoProgress,
        ))
        .expect("render");
        out.rgba_data
    }

    /// The plane point a pixel centre maps to — the template's
    /// mapping, transcribed.
    fn pixel_to_plane(x: u32, y: u32) -> [f64; 2] {
        let span_y = 4.0 / 2f64.powf(ZOOM_LOG2);
        let span_x = span_y * W as f64 / H as f64;
        let u = (x as f64 + 0.5) / W as f64 - 0.5;
        let v = (y as f64 + 0.5) / H as f64 - 0.5;
        [CENTRE[0] + u * span_x, CENTRE[1] - v * span_y]
    }

    fn brightness(rgba: &[u8], x: u32, y: u32) -> f64 {
        let i = ((y * W + x) * 4) as usize;
        (rgba[i] as f64 + rgba[i + 1] as f64 + rgba[i + 2] as f64) / 765.0
    }

    #[test]
    #[ignore = "needs a GPU"]
    fn the_gpu_walk_agrees_with_the_cpu_reference_on_a_sierpinski() {
        let config = config_for(sierpinski_flame());
        let rgba = render(&config);
        assert_eq!(rgba.len(), (W * H * 4) as usize);

        let guard = global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&config.flame, &guard)
            .expect("Sierpinski qualifies");
        let px = (4.0 / 2f64.powf(ZOOM_LOG2)) / H as f64;

        // Classify by the CPU reference, skipping the boundary band
        // where a sub-pixel coverage is legitimately between the two.
        //
        // The gasket has measure zero, so "distance exactly 0" catches
        // almost nothing at 96x96 (54 pixels, measured): what the
        // render draws is the set THICKENED to the antialiasing width,
        // so the interior class is "within a quarter pixel", where
        // coverage is 0.84 and up.
        let mut inside = Vec::new();
        let mut outside = Vec::new();
        for y in 0..H {
            for x in 0..W {
                let d = estimate(&ifs, pixel_to_plane(x, y), LEVELS, BEAM).distance;
                if d < 0.25 * px {
                    inside.push(brightness(&rgba, x, y));
                } else if d > 3.0 * px {
                    outside.push(brightness(&rgba, x, y));
                }
            }
        }
        println!(
            "interior {} / exterior {} of {} pixels",
            inside.len(),
            outside.len(),
            W * H
        );
        assert!(inside.len() > 150, "too few interior pixels: {}", inside.len());
        assert!(outside.len() > 2000, "too few exterior pixels: {}", outside.len());

        let mean = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len() as f64;
        let (mi, mo) = (mean(&inside), mean(&outside));
        assert!(mi > 0.05, "the set rendered dark ({mi:.4}) -- nothing to compare");
        assert!(
            mi > mo * 8.0 + 0.02,
            "interior ({mi:.4}) and exterior ({mo:.4}) are not separated: the shader \
             is not drawing the set the CPU walk finds"
        );

        // Per pixel, not just on average: split at the midpoint and
        // require the two classifications to agree almost everywhere.
        let cut = (mi + mo) * 0.5;
        let lit_in = inside.iter().filter(|&&b| b > cut).count();
        let lit_out = outside.iter().filter(|&&b| b > cut).count();
        let agree = (lit_in + (outside.len() - lit_out)) as f64
            / (inside.len() + outside.len()) as f64;
        assert!(
            agree > 0.97,
            "GPU and CPU disagree on {:.1}% of pixels (interior lit {lit_in}/{}, \
             exterior lit {lit_out}/{})",
            (1.0 - agree) * 100.0,
            inside.len(),
            outside.len()
        );
    }

    /// Two julia transforms, `±sqrt(p − c₁)` and `±sqrt(p − c₂)`: the
    /// invariant set of a pair of quadratic inverse systems, which no
    /// single Julia set is.
    pub(super) fn julia_pair_flame() -> Flame {
        let mut fl = Flame::default();
        fl.transforms.clear();
        fl.final_transforms.clear();
        fl.xaos = None;
        for (i, c) in [[-0.123f32, 0.745f32], [0.285, 0.01]].into_iter().enumerate() {
            let mut t = Transform::default();
            t.a = 1.0;
            t.b = 0.0;
            t.c = 0.0;
            t.d = 1.0;
            t.e = -c[0];
            t.f = -c[1];
            t.color = i as f32;
            t.variations = HashMap::from([("julia".to_string(), 1.0)]);
            t.variation_order = vec!["julia".to_string()];
            fl.transforms.push(t);
        }
        fl
    }

    /// Gate 2 of plan 8.8: the GPU walk agrees with the CPU estimate
    /// on a root-map IFS, the way it does on the gasket -- the root
    /// kernel and the local sigma are transcriptions, and this is what
    /// checks the transcription.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_gpu_walk_agrees_with_the_cpu_reference_on_a_julia_pair() {
        let config = config_for(julia_pair_flame());
        let rgba = render(&config);
        let guard = global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&config.flame, &guard)
            .expect("a julia pair qualifies");
        assert!(ifs.maps.iter().all(|m| !m.forward.is_affine()), "both maps should be roots");
        let px = (4.0 / 2f64.powf(ZOOM_LOG2)) / H as f64;
        let (mut inside, mut outside) = (Vec::new(), Vec::new());
        for y in 0..H {
            for x in 0..W {
                let d = estimate(&ifs, pixel_to_plane(x, y), LEVELS, BEAM).distance;
                if d < 0.25 * px {
                    inside.push(brightness(&rgba, x, y));
                } else if d > 3.0 * px {
                    outside.push(brightness(&rgba, x, y));
                }
            }
        }
        println!("interior {} / exterior {} of {} pixels", inside.len(), outside.len(), W * H);
        assert!(inside.len() > 150, "too few interior pixels: {}", inside.len());
        assert!(outside.len() > 1500, "too few exterior pixels: {}", outside.len());
        let mean = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len() as f64;
        let (mi, mo) = (mean(&inside), mean(&outside));
        assert!(mi > 0.05, "the set rendered dark ({mi:.4})");
        assert!(mi > mo * 8.0 + 0.02, "interior ({mi:.4}) and exterior ({mo:.4}) are not separated");
        let cut = (mi + mo) * 0.5;
        let lit_in = inside.iter().filter(|&&b| b > cut).count();
        let lit_out = outside.iter().filter(|&&b| b > cut).count();
        let agree = (lit_in + (outside.len() - lit_out)) as f64 / (inside.len() + outside.len()) as f64;
        assert!(
            agree > 0.97,
            "GPU and CPU disagree on {:.1}% of pixels (interior lit {lit_in}/{}, exterior lit {lit_out}/{})",
            (1.0 - agree) * 100.0,
            inside.len(),
            outside.len()
        );
    }

    /// A julia-family transform: `A_pre = (p − c)`, the root at
    /// weight `w`, an optional post-rotation.
    fn julia_xform(variation: &str, c: [f32; 2], w: f32, power: f32, dist: f32, color: f32) -> Transform {
        let mut t = Transform::default();
        t.a = 1.0;
        t.b = 0.0;
        t.c = 0.0;
        t.d = 1.0;
        t.e = -c[0];
        t.f = -c[1];
        t.color = color;
        t.weight = 1.0;
        t.variations = HashMap::from([(variation.to_string(), w)]);
        t.variation_order = vec![variation.to_string()];
        if variation == "julian" {
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", dist);
        }
        t
    }

    /// The julia presets that ship (plan 8.8 gate 4), chosen from the
    /// candidates below after rendering them all: the classic filled
    /// rabbit under the level colouring, which is J2's caveat made
    /// visible; a dendrite under the distance colouring, where the
    /// filled set is thin enough that the picture is its outline; and
    /// a pair of cubic roots under the address colouring, which no
    /// single Julia set is.
    pub(super) fn julia_presets() -> Vec<crate::config::FractalConfig> {
        let chosen = ["Douady Rabbit", "Julia Dendrite", "Cubic Pair"];
        julia_candidates()
            .into_iter()
            .filter(|(name, _, _)| chosen.contains(name))
            .map(|(name, coloring, transforms)| ifs_preset_config(name, coloring, transforms))
            .collect()
    }

    /// Candidate julia presets, rendered for inspection before any
    /// ships (plan 8.8 gate 4). Each is (name, colouring, transforms).
    pub(super) fn julia_candidates() -> Vec<(&'static str, &'static str, Vec<Transform>)> {
        vec![
            ("Douady Rabbit", "ifs_level", vec![julia_xform("julia", [-0.123, 0.745], 1.0, 2.0, 1.0, 0.5)]),
            ("Julia Dendrite", "ifs_distance", vec![julia_xform("julia", [0.36, 0.1], 1.0, 2.0, 1.0, 0.5)]),
            (
                "Julia Pair",
                "ifs_address",
                vec![
                    julia_xform("julia", [-0.123, 0.745], 1.0, 2.0, 1.0, 0.15),
                    julia_xform("julia", [0.285, 0.01], 1.0, 2.0, 1.0, 0.85),
                ],
            ),
            (
                "Julia Pair Distance",
                "ifs_distance",
                vec![
                    julia_xform("julia", [-0.123, 0.745], 1.0, 2.0, 1.0, 0.15),
                    julia_xform("julia", [0.285, 0.01], 1.0, 2.0, 1.0, 0.85),
                ],
            ),
            (
                "Cubic Pair",
                "ifs_address",
                vec![
                    julia_xform("julian", [0.4, 0.3], 1.0, 3.0, 1.0, 0.2),
                    julia_xform("julian", [-0.5, 0.2], 1.0, 3.0, 1.0, 0.8),
                ],
            ),
            (
                "Rabbit and Basilica",
                "ifs_level",
                vec![
                    julia_xform("julia", [-0.123, 0.745], 1.0, 2.0, 1.0, 0.2),
                    julia_xform("julia", [-1.0, 0.0], 1.0, 2.0, 1.0, 0.8),
                ],
            ),
            (
                "Julia Trio",
                "ifs_address",
                vec![
                    julia_xform("julia", [-0.123, 0.745], 1.0, 2.0, 1.0, 0.1),
                    julia_xform("julia", [0.285, 0.01], 1.0, 2.0, 1.0, 0.5),
                    julia_xform("julia", [-0.8, 0.156], 1.0, 2.0, 1.0, 0.9),
                ],
            ),
        ]
    }

    /// Build a mode-D config around a set of transforms the way
    /// `classical_presets` does, framed on the analysed ball.
    pub(super) fn ifs_preset_config(name: &str, coloring: &str, transforms: Vec<Transform>) -> crate::config::FractalConfig {
        let registry = global_registry();
        let mut c = crate::config::FractalConfig::default();
        c.render_mode = crate::scene::transforms::RenderMode::Escape;
        c.flame.name = name.to_string();
        c.flame.transforms = transforms;
        c.flame.final_transforms.clear();
        c.flame.xaos = None;
        let ifs = crate::scene::ifs_analysis::analyse_2d(&c.flame, &registry)
            .unwrap_or_else(|why| panic!("{name} must qualify, but: {why:?}"));
        c.escape.formula = "ifs_flame".to_string();
        c.escape.coloring = coloring.to_string();
        c.escape.center_re = format!("{}", ifs.ball.centre[0]);
        c.escape.center_im = format!("{}", ifs.ball.centre[1]);
        c.escape.zoom_log2 = (4.0 / (ifs.ball.radius * 2.4)).log2();
        c.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
        c.exposure = crate::config::defaults::DEFAULT_EXPOSURE;
        c.gamma = crate::config::defaults::DEFAULT_GAMMA;
        c
    }

    #[test]
    #[ignore = "needs a GPU; writes output/ifs/julia-*.png"]
    fn render_the_julia_candidates_for_inspection() {
        let dir = std::path::Path::new("output/ifs");
        std::fs::create_dir_all(dir).expect("output dir");
        let (device, queue) = device();
        for (name, coloring, transforms) in julia_candidates() {
            let mut c = ifs_preset_config(name, coloring, transforms);
            c.escape.formula_params.insert("levels".to_string(), 64.0);
            let job = crate::renderer::RenderJob::new(&c, 512, 512);
            let out = pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress))
                .expect("render");
            let slug = name.to_lowercase().replace(' ', "-");
            let path = dir.join(format!("julia-{slug}.png"));
            image::save_buffer(&path, &out.rgba_data, 512, 512, image::ColorType::Rgba8).expect("write png");
            println!("    {}", path.display());
        }
    }

    fn kernel_flame(transforms: Vec<(&str, [f32; 6], f32, f32)>) -> Flame {
        let mut fl = Flame::default();
        fl.transforms.clear();
        fl.final_transforms.clear();
        fl.xaos = None;
        for (variation, a, w, color) in transforms {
            let mut t = Transform::default();
            t.a = a[0];
            t.b = a[1];
            t.c = a[2];
            t.d = a[3];
            t.e = a[4];
            t.f = a[5];
            t.color = color;
            t.variations = HashMap::from([(variation.to_string(), w)]);
            t.variation_order = vec![variation.to_string()];
            fl.transforms.push(t);
        }
        fl
    }

    pub(super) fn spherical_ifs_flame() -> Flame {
        kernel_flame(vec![
            ("spherical", [0.0, -1.0, 1.0, 0.0, 1.0, 0.0], 1.0, 0.2),
            ("spherical", [0.0, 1.0, -1.0, 0.0, 0.0, 0.0], 1.0, 0.5),
            ("linear", [0.5, 0.0, 0.0, 0.5, 0.8, 0.0], 1.0, 0.8),
        ])
    }

    pub(super) fn bubble_ifs_flame() -> Flame {
        kernel_flame(vec![
            ("bubble", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.6, 0.2),
            ("bubble", [0.7, 0.7, -0.7, 0.7, 0.0, -0.3], 1.2, 0.5),
            ("linear", [0.5, 0.0, 0.0, 0.5, 1.0, 0.5], 1.0, 0.8),
        ])
    }

    pub(super) fn hemisphere_ifs_flame() -> Flame {
        kernel_flame(vec![
            ("hemisphere", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.5, 0.2),
            ("hemisphere", [0.7, 0.7, -0.7, 0.7, 0.4, 0.0], 1.2, 0.5),
            ("linear", [0.5, 0.0, 0.0, 0.5, 0.8, 0.3], 1.0, 0.8),
        ])
    }

    pub(super) fn disc_ifs_flame() -> Flame {
        kernel_flame(vec![
            ("disc", [0.9, 0.4, -0.4, 0.9, 0.0, 0.0], 1.0, 0.3),
            ("linear", [0.55, 0.0, 0.0, 0.55, 0.0, 0.0], 1.0, 0.8),
        ])
    }

    pub(super) fn blob_ifs_flame() -> Flame {
        let mut fl = kernel_flame(vec![
            ("blob", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 0.8, 0.3),
            ("linear", [0.6, 0.3, -0.3, 0.6, 0.5, 0.0], 1.0, 0.6),
            ("linear", [0.5, 0.0, 0.0, 0.5, -0.4, 0.3], 1.0, 0.9),
        ]);
        fl.transforms[0].set_variation_param("blob", "high", 1.2);
        fl.transforms[0].set_variation_param("blob", "low", 0.5);
        fl.transforms[0].set_variation_param("blob", "waves", 5.0);
        fl
    }

    /// A bounded julia set, as a flame: `julia` after a translation by
    /// `-c`, so the forward map is the inverse iteration of `z² + c`
    /// and the IFS is that set's two branches.
    pub(super) fn julia_ifs_flame(c: [f64; 2]) -> Flame {
        kernel_flame(vec![("julia", [1.0, 0.0, 0.0, 1.0, -c[0] as f32, -c[1] as f32], 1.0, 0.3)])
    }

    /// G5 of `ifs-nonlinear-perturbation.md`: the SHADER agrees with
    /// the walk on a nonlinear set at a zoom deep enough that the
    /// handover is doing the work.
    ///
    /// This is the gate the seeded GPU tests did not have. Every one
    /// of them uses a Sierpinski, whose maps are affine, so the
    /// nonlinear half of `seed_beam` -- and the whole of
    /// `big_kernel_inverse`, which the shader's seeds come out of at
    /// this zoom -- was reached by no GPU test at all. A sign error in
    /// the arbitrary-precision root passed a green run of all 57 of
    /// them; `the_big_kernel_inverse_is_the_f64_one` is the unit gate
    /// for that, and this is the one that catches it from outside.
    ///
    /// Zoom 2^20 is past where f32 stops resolving the view from a
    /// centre of magnitude O(1) -- measured at 3.3 pixels of error
    /// there, and 835 at 2^28 -- so without a prefix the picture is
    /// blocks and this could not agree with anything.
    ///
    /// The Sierpinski runs beside it at the same depth ON PURPOSE. It
    /// exercises the affine path, which must be untouched, and it
    /// pins the MEASUREMENT: with a uv convention wrong here it read
    /// 59.6% on the affine arm and 90.7% on the nonlinear one, and
    /// only having both made it obvious that the fault was in the
    /// test and not in the thing under test.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_gpu_agrees_on_a_nonlinear_set_at_depth() {
        const ZOOM: f64 = 20.0;
        const DEEP_LEVELS: u32 = 60;
        for (name, flame, nonlinear) in [
            ("julia", julia_ifs_flame([-0.4, 0.6]), true),
            ("sierpinski", sierpinski_flame(), false),
        ] {
            let guard = global_registry();
            let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &guard).expect("qualifies");
            drop(guard);
            assert_eq!(
                nonlinear,
                ifs.maps.iter().any(|m| !m.forward.is_affine()),
                "{name} is not the arm it is labelled as"
            );

            // A point on the attractor to centre on, so the view keeps
            // finding structure at depth.
            let mut target = ifs.ball.centre;
            for k in 0..60u32 {
                target = ifs.maps[(k as usize) % ifs.maps.len()].forward.apply(target);
            }

            let mut config = config_for(flame);
            config.escape.center_re = format!("{:?}", target[0]);
            config.escape.center_im = format!("{:?}", target[1]);
            config.escape.zoom_log2 = ZOOM;
            config.escape.formula_params.insert("levels".to_string(), DEEP_LEVELS as f32);
            config.escape.formula_params.insert("beam".to_string(), BEAM as f32);
            let rgba = render(&config);

            // The same seeds the renderer builds, and the same
            // continuation the shader runs.
            let span_y = 4.0 / 2f64.powf(ZOOM);
            let span_x = span_y * W as f64 / H as f64;
            let basis = view_basis(span_x, span_y, 0.0);
            let px = span_y / H as f64;
            let centre = centre_at_precision(&config.escape).expect("centre parses");
            let seeds = crate::scene::ifs_estimate::seed_beam(
                &ifs,
                centre,
                basis,
                px,
                ZOOM as u32 + 64,
                BEAM,
            );
            assert!(
                seeds.level > 0,
                "{name}: the prefix did no work, so this is not testing the handover"
            );

            // `DEEP_LEVELS` steps AFTER the handover, which is what the
            // shader walks (`for k in 0..max_levels`, with `max_levels`
            // the `levels` parameter and no handover subtracted) and
            // what `estimate_seeded` walks. This used to subtract the
            // handover level, so the two walks ended `level` steps
            // apart; the binary agreement below could not see it, and
            // `probe_what_the_f32_term_costs_on_the_gpu` could.
            let after = DEEP_LEVELS;
            let (mut inside, mut outside) = (Vec::new(), Vec::new());
            for y in 0..H {
                for x in 0..W {
                    // `view_basis` already carries the y flip, so uv
                    // runs down the screen with the pixels.
                    let uv = [
                        (x as f64 + 0.5) / W as f64 - 0.5,
                        (y as f64 + 0.5) / H as f64 - 0.5,
                    ];
                    // `estimate_seeded` reports in pixels already.
                    let d = crate::scene::ifs_estimate::estimate_seeded(
                        &ifs, &seeds, uv, after, BEAM,
                    )
                    .distance;
                    if d <= 1.0 {
                        inside.push(brightness(&rgba, x, y));
                    } else {
                        outside.push(brightness(&rgba, x, y));
                    }
                }
            }
            assert!(
                inside.len() > 200 && outside.len() > 200,
                "{name}: the view is not a mix of set and exterior: {} near, {} far",
                inside.len(),
                outside.len()
            );
            let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
            let (mi, mo) = (mean(&inside), mean(&outside));
            let cut = (mi + mo) * 0.5;
            let lit_in = inside.iter().filter(|&&b| b > cut).count();
            let lit_out = outside.iter().filter(|&&b| b > cut).count();
            let agree = (lit_in + (outside.len() - lit_out)) as f64
                / (inside.len() + outside.len()) as f64;
            println!(
                "  {name} at 2^{ZOOM}, handover level {}: CPU near {} / far {}; \
                 GPU lit near {lit_in}, lit far {lit_out}; agreement {:.1}%",
                seeds.level,
                inside.len(),
                outside.len(),
                agree * 100.0
            );
            assert!(
                agree > 0.98,
                "{name}: the shader and the walk agree on only {:.1}% of the view at 2^{ZOOM}",
                agree * 100.0
            );
        }
    }

    /// Gate 3 of plan 8.9: the GPU walk agrees with the CPU estimate
    /// on a spherical IFS and on a bubble IFS, by the gasket's test.
    /// The view is framed on each set's ball rather than the harness
    /// default, which was chosen for the gasket.
    /// R1 of [`ifs-perturbation-delta.md`] §2: what the objective's
    /// f32 term is actually worth, measured on the GPU.
    ///
    /// `seed_beam` chooses where to hand over by minimising a MEASURED
    /// curvature plus a MODELLED f32 cost,
    /// `|q|·2⁻²⁴ / (px · reach_L/reach_0)`. The model was never
    /// checked against anything. `probe_where_a_grand_julian_caps`
    /// checked it against the handover's own ROUNDING -- position,
    /// basis and quadratic to f32, continued in f64 -- and found the
    /// model 3x to 800x above it over 42 rows. But that measurement
    /// prices the rounding at the handover and NOT the f32 arithmetic
    /// of every step after it, which the shader does and f64 does
    /// not, so the truth sits between the two and only the GPU says
    /// where.
    ///
    /// This is that measurement: render at the resolution the model
    /// is quoted at, read the walk's own distance back out of the
    /// recolor cache, and compare it against `estimate_seeded` in f64
    /// from the same seeds. The difference is the rounding plus the
    /// arithmetic, which is the whole of what the term models.
    ///
    /// **The 2^4 row is the control and the point.** At a shallow
    /// zoom the model is nothing and any disagreement is the shader's
    /// walk differing from `estimate_seeded`'s -- the floor this
    /// measurement cannot see past. A deep row is only evidence to
    /// the extent it rises above that floor.
    #[test]
    #[ignore = "needs a GPU; run with --ignored --nocapture"]
    fn probe_what_the_f32_term_costs_on_the_gpu() {
        use crate::scene::transforms::{Flame, Transform};
        // 1080p, because that is the resolution the model's table is
        // quoted at and the term scales with it.
        const RW: u32 = 1920;
        const RH: u32 = 1080;
        const LEVELS: u32 = 80;
        const BEAM: u32 = 5;

        let j = |aff: [f32; 6], w: f32, power: f32| {
            let mut t = Transform::default();
            let [a, b, c, d, e, f] = aff;
            t.a = a;
            t.b = b;
            t.c = c;
            t.d = d;
            t.e = e;
            t.f = f;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("flatten", 1.0);
            t.set_variation("julian", w);
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", -1.0);
            t
        };
        let sq = [0.7071f32, 0.7071, -0.7071, 0.7071, 0.0, 0.0];
        let mut gj = Flame::default();
        gj.transforms = vec![
            j([0.7071, 0.7071, -0.7071, 0.7071, 0.0, -0.3], 1.0, 2.0),
            j(sq, 0.2, 15.0),
            j(sq, 0.3, 8.0),
        ];
        // Two SEEDED controls beside the set under test: a bounded
        // julia and an affine gasket, whose handovers are deep and
        // whose f32 cost is small and known. The 2^4 row's handover
        // is level 0, so it checks the walk and not the seeded path;
        // these check the seeded path, depth accounting included.
        let cases: Vec<(&str, Flame, Vec<f64>, usize)> = vec![
            ("julia", julia_ifs_flame([-0.4, 0.6]), vec![4.0, 20.0, 28.0], 1),
            ("sierpinski", sierpinski_flame(), vec![4.0, 20.0, 28.0], 1),
            ("grand julian", gj, vec![4.0, 20.0, 26.0, 30.0, 33.0, 36.0], 4),
        ];
        for (case, flame, zooms, n_targets) in cases {
        let registry = crate::variations::global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &registry).expect("qualifies");
        drop(registry);

        // Targets on the attractor, by a forward chaos walk with a
        // deterministic branch choice -- self-contained, so this does
        // not depend on another module's test fixture.
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut p = ifs.ball.centre;
        let mut targets: Vec<[f64; 2]> = Vec::new();
        for i in 0..3000 {
            let m = &ifs.maps[(next() * ifs.maps.len() as f64).floor() as usize % ifs.maps.len()];
            let k = match m.forward.nonlinear().map(|n| n.kernel) {
                Some(crate::scene::ifs_analysis::Kernel::Root { n, .. }) => {
                    (next() * n.unsigned_abs() as f64).floor() as u32
                }
                _ => 0,
            };
            p = match &m.forward {
                crate::scene::ifs_analysis::Map2::Nonlinear(n) => n.apply_branch(p, k),
                other => other.apply(p),
            };
            if i > 500 && i % 700 == 0 && p[0].is_finite() && p[1].is_finite() {
                targets.push(p);
            }
        }
        assert!(targets.len() >= 3, "no targets found");
        targets.truncate(n_targets);

        let (device, queue) = device();
        let base = crate::config::FractalConfig::default();
        let pal = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            64,
            64,
            &base.flame,
            base.palette_size,
        );

        println!("{case}:");
        println!(
            "  {:>6} {:>7} | {:>5} {:>12} {:>12} | {:>10} {:>10} {:>10}",
            "target", "zoom", "L", "f32 model", "f32 round", "gpu med", "gpu p90", "gpu max"
        );
        for (t, &target) in targets.iter().enumerate() {
            for &zoom in &zooms {
                let mut config = crate::config::FractalConfig::default();
                config.render_mode = RenderMode::Escape;
                config.flame = flame.clone();
                config.escape.formula = "ifs_flame".to_string();
                config.escape.coloring = "ifs_distance".to_string();
                config.escape.center_re = format!("{:?}", target[0]);
                config.escape.center_im = format!("{:?}", target[1]);
                config.escape.zoom_log2 = zoom;
                config.escape.rotation = 0.0;
                config.escape.supersample = 1;
                config.escape.formula_params.insert("levels".to_string(), LEVELS as f32);
                config.escape.formula_params.insert("beam".to_string(), BEAM as f32);
                let esc = config.escape.clone();

                // The seeds the renderer will build, built here too.
                let span_y = 4.0 / 2f64.powf(zoom);
                let span_x = span_y * RW as f64 / RH as f64;
                let basis = view_basis(span_x, span_y, 0.0);
                let px = span_y / RH as f64;
                let centre = centre_at_precision(&esc).expect("centre parses");
                let seeds = crate::scene::ifs_estimate::seed_beam(
                    &ifs,
                    centre,
                    basis,
                    px,
                    zoom as u32 + 64,
                    BEAM,
                );

                let mut escape = crate::escape::EscapeRenderer::new(&device, RW, RH);
                // The flame itself. Mode D reads it as an IFS through
                // `pack_for`, and without this the renderer has no
                // flame and every pixel reads the empty sentinel.
                let def = get_ifs(&config.escape.formula).expect("ifs_flame");
                let reg = crate::variations::global_registry();
                let packed = pack_for(def, &config, &reg);
                drop(reg);
                assert!(packed.is_some(), "the flame does not qualify");
                escape.set_ifs(packed);
                let mut guard = 0;
                loop {
                    let mut enc = device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor { label: Some("r1") },
                    );
                    let done = escape.render(
                        &device,
                        &queue,
                        &mut enc,
                        &esc,
                        pal.palette_view(),
                        pal.palette_generation(),
                    );
                    queue.submit(std::iter::once(enc.finish()));
                    let _ = device
                        .poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                    if done {
                        break;
                    }
                    guard += 1;
                    assert!(guard < 10_000, "render never settled");
                }
                let recs = escape.read_results_full(&device, &queue).expect("records");
                escape.destroy();

                // The two CPU numbers the model is being judged
                // against. `basis_reach` is private to the estimator,
                // so the corner of the unit square, transcribed.
                let reach = |b: [[f64; 2]; 2]| -> f64 {
                    let x = (b[0][0].abs() + b[0][1].abs()) * 0.5;
                    let y = (b[1][0].abs() + b[1][1].abs()) * 0.5;
                    (x * x + y * y).sqrt()
                };
                let reach0 = reach(basis);
                let model = seeds
                    .cands
                    .iter()
                    .map(|c| {
                        let grown = reach(c.basis) / reach0;
                        if !(grown > 0.0) {
                            return f64::INFINITY;
                        }
                        let mag = c.position[0].hypot(c.position[1]).max(ifs.ball.radius);
                        mag * 5.96e-8 / (px * grown)
                    })
                    .fold(0.0, f64::max);
                let rounded = crate::scene::ifs_estimate::Seeds {
                    level: seeds.level,
                    dead_min_per_px: seeds.dead_min_per_px,
                    cands: seeds
                        .cands
                        .iter()
                        .map(|c| {
                            let r = |x: f64| x as f32 as f64;
                            let mut c = c.clone();
                            c.position = [r(c.position[0]), r(c.position[1])];
                            c.basis = [
                                [r(c.basis[0][0]), r(c.basis[0][1])],
                                [r(c.basis[1][0]), r(c.basis[1][1])],
                            ];
                            c.quad = [
                                [r(c.quad[0][0]), r(c.quad[0][1])],
                                [r(c.quad[1][0]), r(c.quad[1][1])],
                                [r(c.quad[2][0]), r(c.quad[2][1])],
                            ];
                            c
                        })
                        .collect(),
                };

                // Both walk `LEVELS` steps from the seeds: the shader's
                // loop is `0..max_levels` after the handover and does
                // not subtract the handover level, and neither does
                // `estimate_seeded`. The first version of this probe
                // subtracted it, and every deep row compared walks
                // that ended `level` steps apart -- which the level-0
                // control could not see.
                let after = LEVELS;
                let mut round_err = 0.0f64;
                let mut gpu_err: Vec<f64> = Vec::new();
                // A sparse grid: the CPU continuation is the cost
                // here, not the render.
                for gy in 0..27u32 {
                    for gx in 0..48u32 {
                        let x = gx * (RW / 48) + RW / 96;
                        let y = gy * (RH / 27) + RH / 54;
                        let uv = [
                            (x as f64 + 0.5) / RW as f64 - 0.5,
                            (y as f64 + 0.5) / RH as f64 - 0.5,
                        ];
                        let a = crate::scene::ifs_estimate::estimate_seeded(
                            &ifs, &seeds, uv, after, BEAM,
                        )
                        .distance;
                        let b = crate::scene::ifs_estimate::estimate_seeded(
                            &ifs, &rounded, uv, after, BEAM,
                        )
                        .distance;
                        round_err = round_err.max((a - b).abs());
                        let g = recs[(y * RW + x) as usize].z[0] as f64;
                        if a.is_finite() && g.is_finite() {
                            gpu_err.push((a - g).abs());
                        }
                    }
                }
                gpu_err.sort_by(f64::total_cmp);
                let at = |q: f64| -> f64 {
                    if gpu_err.is_empty() {
                        return f64::NAN;
                    }
                    gpu_err[((gpu_err.len() - 1) as f64 * q).round() as usize]
                };
                println!(
                    "  {t:>6} 2^{zoom:<5.0} | {:>5} {model:>12.3} {round_err:>12.3} | \
                     {:>10.3} {:>10.3} {:>10.3}",
                    seeds.level,
                    at(0.5),
                    at(0.9),
                    at(1.0)
                );
            }
        }
        }
    }

    /// Per seed, at the levels the ranking probe finds interesting:
    /// what the f32 model prices and what the bound says about whether
    /// that seed can reach the answer at all. CPU only.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_which_seed_prices_the_level() {
        use crate::scene::transforms::{Flame, Transform};
        const RW: u32 = 1920;
        const RH: u32 = 1080;
        const BEAM: u32 = 5;
        let j = |aff: [f32; 6], w: f32, power: f32| {
            let mut t = Transform::default();
            let [a, b, c, d, e, f] = aff;
            t.a = a; t.b = b; t.c = c; t.d = d; t.e = e; t.f = f;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("flatten", 1.0);
            t.set_variation("julian", w);
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", -1.0);
            t
        };
        let sq = [0.7071f32, 0.7071, -0.7071, 0.7071, 0.0, 0.0];
        let mut flame = Flame::default();
        flame.transforms = vec![
            j([0.7071, 0.7071, -0.7071, 0.7071, 0.0, -0.3], 1.0, 2.0),
            j(sq, 0.2, 15.0),
            j(sq, 0.3, 8.0),
        ];
        let registry = crate::variations::global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &registry).expect("qualifies");
        drop(registry);
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut p = ifs.ball.centre;
        let mut targets: Vec<[f64; 2]> = Vec::new();
        for i in 0..3000 {
            let m = &ifs.maps[(next() * ifs.maps.len() as f64).floor() as usize % ifs.maps.len()];
            let k = match m.forward.nonlinear().map(|n| n.kernel) {
                Some(crate::scene::ifs_analysis::Kernel::Root { n, .. }) => {
                    (next() * n.unsigned_abs() as f64).floor() as u32
                }
                _ => 0,
            };
            p = match &m.forward {
                crate::scene::ifs_analysis::Map2::Nonlinear(n) => n.apply_branch(p, k),
                other => other.apply(p),
            };
            if i > 500 && i % 700 == 0 && p[0].is_finite() && p[1].is_finite() {
                targets.push(p);
            }
        }

        // target 1 at 2^26: the ranking probe's clearest miss -- level
        // 7 modelled at 4.6e11 pixels and rendered 0.161 out, the best
        // of every level.
        let target = targets[1];
        let zoom = 26.0f64;
        let span_y = 4.0 / 2f64.powf(zoom);
        let span_x = span_y * RW as f64 / RH as f64;
        let basis = view_basis(span_x, span_y, 0.0);
        let px = span_y / RH as f64;
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.center_re = format!("{:?}", target[0]);
        esc.center_im = format!("{:?}", target[1]);
        esc.zoom_log2 = zoom;
        let centre = centre_at_precision(&esc).expect("centre");
        let reach = |b: [[f64; 2]; 2]| -> f64 {
            let x = (b[0][0].abs() + b[0][1].abs()) * 0.5;
            let y = (b[1][0].abs() + b[1][1].abs()) * 0.5;
            (x * x + y * y).sqrt()
        };
        let reach0 = reach(basis);
        for level in [4u32, 5, 6, 7, 8] {
            let seeds = crate::scene::ifs_estimate::seed_beam_at(
                &ifs, centre.clone(), basis, px, zoom as u32 + 64, BEAM, level,
            );
            if seeds.level != level {
                println!("level {level}: unreachable");
                continue;
            }
            println!("level {level}, {} seeds:", seeds.cands.len());
            for (k, c) in seeds.cands.iter().enumerate() {
                let grown = reach(c.basis) / reach0;
                let mag = c.position[0].hypot(c.position[1]).max(ifs.ball.radius);
                let e = mag * 5.96e-8 / (px * grown);
                println!(
                    "   seed {k}: |q| {mag:.4e}  grown {grown:.3e}  sigma/px {:.3e}  \
                     bound/px {:.4e}  f32 {e:.3e}  done {}  esc {}",
                    c.sigma_per_px,
                    c.bound_per_px,
                    c.done,
                    c.escape.is_some()
                );
            }
        }
    }

    /// The other half of R1: does the objective ORDER the levels the
    /// way the rendered picture does?
    ///
    /// `probe_what_the_f32_term_costs_on_the_gpu` measures the level
    /// the objective picked. An argmin is only as good as its
    /// ordering, so that says nothing about whether a different level
    /// would have been better -- which is the whole question. This
    /// forces every level the walk can reach
    /// (`EscapeRenderer::ifs_force_level`, which routes
    /// `ensure_ifs_seeds` through `seed_beam_at`), renders each, and
    /// compares:
    ///
    /// - `model`, what the objective believes: the measured curvature
    ///   against the level-0 handover plus the modelled f32 cost, the
    ///   sum it minimises;
    /// - `true`, what the picture is: the RENDERED distance against
    ///   the exact level-0 continuation in f64, at the same absolute
    ///   depth, which contains the curvature and the f32 together.
    ///
    /// The number that matters is the last column: what the
    /// objective's choice costs against the best level available. A
    /// model can be wrong everywhere and still pick right.
    #[test]
    #[ignore = "needs a GPU; run with --ignored --nocapture"]
    fn probe_what_the_f32_term_ranks_on_the_gpu() {
        use crate::scene::transforms::{Flame, Transform};
        const RW: u32 = 1920;
        const RH: u32 = 1080;
        const LEVELS: u32 = 80;
        const BEAM: u32 = 5;

        let j = |aff: [f32; 6], w: f32, power: f32| {
            let mut t = Transform::default();
            let [a, b, c, d, e, f] = aff;
            t.a = a;
            t.b = b;
            t.c = c;
            t.d = d;
            t.e = e;
            t.f = f;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("flatten", 1.0);
            t.set_variation("julian", w);
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", -1.0);
            t
        };
        let sq = [0.7071f32, 0.7071, -0.7071, 0.7071, 0.0, 0.0];
        let mut flame = Flame::default();
        flame.transforms = vec![
            j([0.7071, 0.7071, -0.7071, 0.7071, 0.0, -0.3], 1.0, 2.0),
            j(sq, 0.2, 15.0),
            j(sq, 0.3, 8.0),
        ];
        let registry = crate::variations::global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &registry).expect("qualifies");
        drop(registry);

        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut p = ifs.ball.centre;
        let mut targets: Vec<[f64; 2]> = Vec::new();
        for i in 0..3000 {
            let m = &ifs.maps[(next() * ifs.maps.len() as f64).floor() as usize % ifs.maps.len()];
            let k = match m.forward.nonlinear().map(|n| n.kernel) {
                Some(crate::scene::ifs_analysis::Kernel::Root { n, .. }) => {
                    (next() * n.unsigned_abs() as f64).floor() as u32
                }
                _ => 0,
            };
            p = match &m.forward {
                crate::scene::ifs_analysis::Map2::Nonlinear(n) => n.apply_branch(p, k),
                other => other.apply(p),
            };
            if i > 500 && i % 700 == 0 && p[0].is_finite() && p[1].is_finite() {
                targets.push(p);
            }
        }
        targets.truncate(3);

        let (device, queue) = device();
        let base = crate::config::FractalConfig::default();
        let pal = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            64,
            64,
            &base.flame,
            base.palette_size,
        );
        let probes = [[0.0f64, 0.0], [-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5], [0.5, 0.5]];

        for (t, &target) in targets.iter().enumerate() {
            for &zoom in &[26.0f64, 33.0] {
                let span_y = 4.0 / 2f64.powf(zoom);
                let span_x = span_y * RW as f64 / RH as f64;
                let basis = view_basis(span_x, span_y, 0.0);
                let px = span_y / RH as f64;
                let budget = zoom as u32 + 64;
                let centre = centre_at_precision(&{
                    let mut e = crate::config::escape::EscapeConfig::default();
                    e.center_re = format!("{:?}", target[0]);
                    e.center_im = format!("{:?}", target[1]);
                    e.zoom_log2 = zoom;
                    e
                })
                .expect("centre parses");
                let chosen = crate::scene::ifs_estimate::seed_beam(
                    &ifs, centre.clone(), basis, px, budget, BEAM,
                )
                .level;
                let exact = crate::scene::ifs_estimate::seed_beam_at(
                    &ifs, centre.clone(), basis, px, budget, BEAM, 0,
                );
                let reach = |b: [[f64; 2]; 2]| -> f64 {
                    let x = (b[0][0].abs() + b[0][1].abs()) * 0.5;
                    let y = (b[1][0].abs() + b[1][1].abs()) * 0.5;
                    (x * x + y * y).sqrt()
                };
                let reach0 = reach(basis);

                println!("target {t}, 2^{zoom:.0} (the objective chose level {chosen}):");
                println!(
                    "  {:>5} | {:>11} {:>11} {:>10} {:>11} {:>11} | {:>10} {:>9} {:>5}",
                    "L", "f32 max", "f32 min", "crv", "total max", "total min", "true p90", "near p90", "n"
                );
                let mut rows: Vec<(u32, f64, f64, f64, f64)> = Vec::new();
                for level in 0..=24u32 {
                    let seeds = crate::scene::ifs_estimate::seed_beam_at(
                        &ifs, centre.clone(), basis, px, budget, BEAM, level,
                    );
                    if seeds.level != level {
                        continue;
                    }
                    let model_f32 = seeds
                        .cands
                        .iter()
                        .map(|c| {
                            let grown = reach(c.basis) / reach0;
                            if !(grown > 0.0) {
                                return f64::INFINITY;
                            }
                            let mag = c.position[0].hypot(c.position[1]).max(ifs.ball.radius);
                            mag * 5.96e-8 / (px * grown)
                        })
                        .fold(0.0, f64::max);
                    // The DERIVED f32 term: the bound is `sigma*(r-R)`,
                    // so a position error `dq` costs `sigma*dq`, and
                    // `sigma_per_px` already carries the division by
                    // the pixel. The shipped model divides by the
                    // BASIS expansion instead, which is `1/sigma` only
                    // for a conformal map -- otherwise it is an upper
                    // bound on it, loose by the map's accumulated
                    // anisotropy.
                    // The answer is `min_j b_j`. Perturbing each bound
                    // by its own f32 error `e_j`, the computed answer
                    // is `min_j (b_j + e_j)`, so the deviation from
                    // `b_* = min_j b_j` is at most
                    //   max_j [ e_j - (b_j - b_*) ]+
                    // -- a seed whose bound exceeds the minimum by
                    // more than its own error cannot reach the answer.
                    // Exact, and it needs no sampling: every quantity
                    // is already carried. The shipped model is the
                    // same max WITHOUT the gap subtracted, which
                    // prices a seed sitting 1e18 pixels away as though
                    // it could win.
                    let each: Vec<(f64, f64)> = seeds
                        .cands
                        .iter()
                        .map(|c| {
                            let grown = reach(c.basis) / reach0;
                            let mag = c.position[0].hypot(c.position[1]).max(ifs.ball.radius);
                            let e = if grown > 0.0 {
                                mag * 5.96e-8 / (px * grown)
                            } else {
                                f64::INFINITY
                            };
                            (e, c.bound_per_px)
                        })
                        .collect();
                    // The MINIMUM over seeds, as the candidate the
                    // detail probe suggests: every seed's f32 error is
                    // proportional to its own sigma, the answer is a
                    // minimum over seeds, and the smallest sigma is the
                    // seed that typically achieves it.
                    let model_sigma =
                        each.iter().map(|(e, _)| *e).fold(f64::INFINITY, f64::min);
                    let model_crv = probes
                        .iter()
                        .map(|uv| {
                            let a = crate::scene::ifs_estimate::estimate_seeded(
                                &ifs, &exact, *uv, 48 + level, BEAM,
                            )
                            .distance;
                            let b = crate::scene::ifs_estimate::estimate_seeded(
                                &ifs, &seeds, *uv, 48, BEAM,
                            )
                            .distance;
                            (a - b).abs()
                        })
                        .fold(0.0, f64::max);

                    let mut config = crate::config::FractalConfig::default();
                    config.render_mode = RenderMode::Escape;
                    config.flame = flame.clone();
                    config.escape.formula = "ifs_flame".to_string();
                    config.escape.coloring = "ifs_distance".to_string();
                    config.escape.center_re = format!("{:?}", target[0]);
                    config.escape.center_im = format!("{:?}", target[1]);
                    config.escape.zoom_log2 = zoom;
                    config.escape.supersample = 1;
                    config.escape.formula_params.insert("levels".to_string(), LEVELS as f32);
                    config.escape.formula_params.insert("beam".to_string(), BEAM as f32);
                    let esc = config.escape.clone();

                    let mut escape = crate::escape::EscapeRenderer::new(&device, RW, RH);
                    escape.ifs_force_level = Some(level);
                    let def = get_ifs(&config.escape.formula).expect("ifs_flame");
                    let reg = crate::variations::global_registry();
                    escape.set_ifs(pack_for(def, &config, &reg));
                    drop(reg);
                    let mut guard = 0;
                    loop {
                        let mut enc = device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor { label: Some("rank") },
                        );
                        let done = escape.render(
                            &device,
                            &queue,
                            &mut enc,
                            &esc,
                            pal.palette_view(),
                            pal.palette_generation(),
                        );
                        queue.submit(std::iter::once(enc.finish()));
                        let _ = device
                            .poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                        if done {
                            break;
                        }
                        guard += 1;
                        assert!(guard < 10_000);
                    }
                    let recs = escape.read_results_full(&device, &queue).expect("records");
                    escape.destroy();

                    // The RENDERED answer against the exact one, at the
                    // same absolute depth: curvature and f32 together,
                    // which is what the objective is trying to predict.
                    // Two populations. The whole grid, whose absolute
                    // errors are dominated by the FAR field -- a pixel
                    // 500 out read as 499 is the same colour -- and the
                    // pixels within eight of the set, which is where
                    // the picture is. A denser grid for the near set,
                    // since it is a small fraction of the frame.
                    let mut err: Vec<f64> = Vec::new();
                    let mut near: Vec<f64> = Vec::new();
                    for gy in 0..54u32 {
                        for gx in 0..96u32 {
                            let x = gx * (RW / 96) + RW / 192;
                            let y = gy * (RH / 54) + RH / 108;
                            let coarse = gx % 4 == 0 && gy % 4 == 0;
                            let uv = [
                                (x as f64 + 0.5) / RW as f64 - 0.5,
                                (y as f64 + 0.5) / RH as f64 - 0.5,
                            ];
                            let g = recs[(y * RW + x) as usize].z[0] as f64;
                            // The GPU's own answer says whether this
                            // pixel is worth the f64 continuation.
                            if !coarse && !(g <= 16.0) {
                                continue;
                            }
                            let truth = crate::scene::ifs_estimate::estimate_seeded(
                                &ifs, &exact, uv, LEVELS + level, BEAM,
                            )
                            .distance;
                            if truth.is_finite() && g.is_finite() {
                                if coarse {
                                    err.push((truth - g).abs());
                                }
                                if truth <= 8.0 {
                                    near.push((truth - g).abs());
                                }
                            }
                        }
                    }
                    err.sort_by(f64::total_cmp);
                    near.sort_by(f64::total_cmp);
                    let q90 = |v: &[f64]| -> f64 {
                        if v.len() < 5 {
                            f64::NAN
                        } else {
                            v[((v.len() - 1) as f64 * 0.9).round() as usize]
                        }
                    };
                    let p90 = q90(&err);
                    let near_p90 = q90(&near);
                    let near_n = near.len();
                    let total = model_f32 + model_crv;
                    let total_sigma = model_sigma + model_crv;
                    println!(
                        "  {level:>5} | {model_f32:>11.3e} {model_sigma:>11.3e} \
                         {model_crv:>10.3} {total:>11.3e} {total_sigma:>11.3e} | \
                         {p90:>10.3} {near_p90:>9.3} {near_n:>5}{}",
                        if level == chosen { "  <- chosen" } else { "" }
                    );
                    rows.push((level, total, p90, total_sigma, near_p90));
                }
                type Row = (u32, f64, f64, f64, f64);
                let pick = |key: fn(&Row) -> f64| {
                    rows.iter()
                        .filter(|r| key(r).is_finite())
                        .min_by(|a, b| key(a).total_cmp(&key(b)))
                        .cloned()
                        .unwrap_or((0, 0.0, 0.0, 0.0, 0.0))
                };
                let by_max = pick(|r| r.1);
                let by_min = pick(|r| r.3);
                let best_all = pick(|r| r.2);
                let best_near = pick(|r| r.4);
                let ratio = |a: f64, b: f64| if b > 0.0 { a / b } else { 1.0 };
                println!(
                    "  => whole frame: best L{} at {:.3}; max model L{} ({:.2}x), min model L{} ({:.2}x)",
                    best_all.0,
                    best_all.2,
                    by_max.0,
                    ratio(by_max.2, best_all.2),
                    by_min.0,
                    ratio(by_min.2, best_all.2)
                );
                println!(
                    "  => near the set: best L{} at {:.3}; max model L{} ({:.2}x), min model L{} ({:.2}x)",
                    best_near.0,
                    best_near.4,
                    by_max.0,
                    ratio(by_max.4, best_near.4),
                    by_min.0,
                    ratio(by_min.4, best_near.4)
                );
            }
        }
    }

    /// The shader's kernel Jacobians ARE the f64 ones.
    ///
    /// The measure walk needs `|det D(S_a⁻¹)|` along an address, and
    /// two cheaper ways of getting it without these were measured
    /// worse -- a secant parallelogram of three carried points reads
    /// 0.617 of the truth on a grand julian, a local central
    /// difference reads 0.790 on a 6:1:1 gasket (measure plan §5i).
    /// So the six closed forms are ported, and this is what says the
    /// port is faithful.
    ///
    /// Compiled against a twenty-line harness rather than the whole
    /// walk: `IFS_JACOBIAN` depends on nothing but the map rows and
    /// `ff_atan2`, which is the reason it is its own const.
    ///
    /// **A transposed matrix still renders a picture**, so this
    /// compares every entry and not a norm.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_shader_jacobians_are_the_cpu_ones() {
        use crate::scene::ifs_analysis::Map2;
        let (device, queue) = device();

        // One flame per kernel, so every arm of the switch is reached.
        let jul = |power: f32, dist: f32| {
            let mut t = crate::scene::transforms::Transform::default();
            t.a = 0.7071;
            t.b = 0.7071;
            t.c = -0.7071;
            t.d = 0.7071;
            t.e = 0.2;
            t.f = -0.1;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("flatten", 1.0);
            t.set_variation("julian", 1.0);
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", dist);
            t
        };
        let kern = |name: &str| {
            let mut t = crate::scene::transforms::Transform::default();
            t.a = 0.9;
            t.b = 0.3;
            t.c = -0.3;
            t.d = 0.9;
            t.e = 0.15;
            t.f = -0.2;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation(name, 1.1);
            t
        };
        let mut affine = crate::scene::transforms::Transform::default();
        affine.a = 0.5;
        affine.d = 0.5;
        affine.e = 0.3;
        affine.variations.clear();
        affine.variation_order.clear();
        affine.set_variation("linear", 1.0);

        let cases: Vec<(&str, Vec<crate::scene::transforms::Transform>)> = vec![
            ("root n=3 d=1", vec![jul(3.0, 1.0), affine.clone()]),
            ("root n=15 d=-1", vec![jul(15.0, -1.0), affine.clone()]),
            ("spherical", vec![kern("spherical"), affine.clone()]),
            ("bubble", vec![kern("bubble"), affine.clone()]),
            ("hemisphere", vec![kern("hemisphere"), affine.clone()]),
            ("disc", vec![kern("disc"), affine.clone()]),
            // The blob fixture the kernel's other gates use: the
            // plain one has no invariant ball.
            ("blob", {
                let mut t = kern("blob");
                t.a = 1.0;
                t.b = 0.0;
                t.c = 0.0;
                t.d = 1.0;
                t.e = 0.0;
                t.f = 0.0;
                t.set_variation("blob", 0.8);
                t.set_variation_param("blob", "high", 1.2);
                t.set_variation_param("blob", "low", 0.5);
                t.set_variation_param("blob", "waves", 5.0);
                let mut a2 = affine.clone();
                a2.a = 0.6;
                a2.b = 0.3;
                a2.c = -0.3;
                a2.d = 0.6;
                a2.e = 0.5;
                a2.f = 0.0;
                vec![t, a2]
            }),
        ];

        let harness = format!(
            r#"
struct IfsMapGpu {{
    inv_m: vec4<f32>,
    inv_t: vec2<f32>,
    sigma_min: f32,
    color: f32,
    pre_m: vec4<f32>,
    pre_t: vec2<f32>,
    kind: f32,
    branch: f32,
    params: vec4<f32>,
    measure: vec4<f32>,
}}
@group(0) @binding(0) var<storage, read> ifs_maps: array<IfsMapGpu>;
@group(0) @binding(1) var<storage, read> pts: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read_write> out: array<vec4<f32>>;

fn ff_atan2(y: f32, x: f32) -> f32 {{
    if (y == 0.0 && x == 0.0) {{
        let pi = 3.14159265358979;
        let mag = select(0.0, pi, (bitcast<u32>(x) & 0x80000000u) != 0u);
        return select(mag, -mag, (bitcast<u32>(y) & 0x80000000u) != 0u);
    }}
    return atan2(y, x);
}}

fn ifs_bubble_scale(r2: f32, branch: f32) -> f32 {{
    let root = sqrt(max(1.0 - r2, 0.0));
    let up = 1.0 + root;
    return select(2.0 * up / max(r2, 1e-30), 2.0 / up, branch == 0.0);
}}
{}
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {{
    let n = arrayLength(&pts);
    if (gid.x >= n) {{ return; }}
    let j = ifs_map_jacobian(0u, pts[gid.x]);
    // Row-major out: (J00, J01, J10, J11).
    out[gid.x] = vec4<f32>(j[0][0], j[1][0], j[0][1], j[1][1]);
}}
"#,
            IFS_JACOBIAN
        );

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("jacobian harness"),
            source: wgpu::ShaderSource::Wgsl(harness.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("jacobian harness"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        for (name, transforms) in cases {
            let guard = crate::variations::global_registry();
            let flame = {
                let mut f = crate::scene::transforms::Flame::default();
                f.transforms = transforms;
                f
            };
            let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &guard).expect("qualifies");
            drop(guard);
            let colors: Vec<f32> = flame.transforms.iter().map(|t| t.color).collect();
            let rows = pack_maps(&ifs, &colors, None);

            // Points spread over the ball, skipping any the CPU
            // declines -- a pole or an image edge is not a
            // disagreement, it is a place with no derivative.
            let (bc, br) = (ifs.ball.centre, ifs.ball.radius);
            let mut pts: Vec<[f32; 2]> = Vec::new();
            let mut want: Vec<[f64; 4]> = Vec::new();
            let mut st = 0x2545F4914F6CDD1Du64;
            let mut rnd = move || {
                st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (st >> 11) as f64 / (1u64 << 53) as f64
            };
            while pts.len() < 400 {
                let q = [
                    bc[0] + (rnd() * 2.0 - 1.0) * br,
                    bc[1] + (rnd() * 2.0 - 1.0) * br,
                ];
                let Some(j) = ifs.maps[0].inverse.jacobian(q) else { continue };
                // Near a singularity f32 cannot follow f64 and the
                // comparison says nothing about the transcription.
                if ifs.maps[0].inverse.singular_distance(q) < 1e-2 * br {
                    continue;
                }
                let mag = j.iter().flatten().fold(0.0f64, |a, b| a.max(b.abs()));
                if !(mag > 1e-6) || mag > 1e6 {
                    continue;
                }
                pts.push([q[0] as f32, q[1] as f32]);
                want.push([j[0][0], j[0][1], j[1][0], j[1][1]]);
            }

            let mk = |data: &[u8], usage: wgpu::BufferUsages| {
                use wgpu::util::DeviceExt;
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: data,
                    usage,
                })
            };
            let maps_buf = mk(
                bytemuck::cast_slice(&rows),
                wgpu::BufferUsages::STORAGE,
            );
            let pts_buf = mk(bytemuck::cast_slice(&pts), wgpu::BufferUsages::STORAGE);
            let out_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (pts.len() * 16) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let stage = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (pts.len() * 16) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: maps_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: pts_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: out_buf.as_entire_binding() },
                ],
            });
            let mut enc = device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bg, &[]);
                pass.dispatch_workgroups(pts.len().div_ceil(64) as u32, 1, 1);
            }
            enc.copy_buffer_to_buffer(&out_buf, 0, &stage, 0, (pts.len() * 16) as u64);
            queue.submit(std::iter::once(enc.finish()));
            let (tx, rx) = std::sync::mpsc::channel();
            stage.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            rx.recv().expect("map").expect("map ok");
            let got: Vec<[f32; 4]> = {
                let d = stage.slice(..).get_mapped_range();
                bytemuck::cast_slice::<u8, [f32; 4]>(&d).to_vec()
            };
            stage.unmap();

            let mut worst = 0.0f64;
            for (g, w) in got.iter().zip(&want) {
                let scale = w.iter().fold(0.0f64, |a, b| a.max(b.abs())).max(1e-9);
                for k in 0..4 {
                    worst = worst.max((g[k] as f64 - w[k]).abs() / scale);
                }
            }
            println!("  {name:<16} {} points, worst entry {worst:.2e} relative", got.len());
            assert!(
                worst < 2e-3,
                "{name}: the shader's Jacobian differs from the f64 one by {worst:.2e} \
                 relative -- a transposed or mis-scaled entry still renders a picture"
            );
        }
    }

    /// The shader's MEASURE walk is the f64 one.
    ///
    /// Not against the chaos game -- that is
    /// `the_measure_agrees_with_the_chaos_game`'s job, and it is what
    /// says the ESTIMATOR is right. This says the shader computes the
    /// same estimator: the packing, the coarse lookup, the Jacobian
    /// composition, the stop rule, the beam and the colour fold.
    ///
    /// Forced to handover level 0 through
    /// `EscapeRenderer::ifs_force_level`, because the seeds carry a
    /// position and a basis but not the probability or the colour
    /// accumulators of the prefix that reached them -- so a deeper
    /// handover would silently drop three numbers. Carrying them is
    /// the deep-zoom follow-on.
    #[test]
    #[ignore = "needs a GPU; run with --ignored --nocapture"]
    fn the_shader_measure_walk_is_the_cpu_one() {
        use crate::scene::ifs_estimate::{
            estimate_measure, CoarseMeasure, MeasureMaps, MEASURE_CELLS,
        };
        const RW: u32 = 256;
        const RH: u32 = 256;
        const RES: usize = 256;
        const SAMPLES: usize = 4_000_000;

        let mut affine = crate::scene::transforms::Transform::default();
        affine.variations.clear();
        affine.variation_order.clear();
        affine.set_variation("linear", 1.0);
        let aff = |a: f32, b: f32, c: f32, d: f32, e: f32, f: f32| {
            let mut t = affine.clone();
            t.a = a; t.b = b; t.c = c; t.d = d; t.e = e; t.f = f;
            t
        };
        let jul = |power: f32| {
            let mut t = aff(0.7071, 0.7071, -0.7071, 0.7071, 0.0, -0.3);
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("flatten", 1.0);
            t.set_variation("julian", 1.0);
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", -1.0);
            t
        };
        // Every fixture is asserted. An earlier version reported two
        // of them instead, on the theory that sets whose maps are
        // alike have tied beam keys and so an arbitrary answer. That
        // theory was wrong -- both were the forced level not reaching
        // the walk -- and the cost of believing it was that the gate
        // stopped looking at exactly the two fixtures that had
        // something to say.
        // The third field is whether the maps are CURVED, the fourth
        // the transforms' COLOUR SPEED.
        //
        // The speed was 0 on every fixture until 2026-09-17, and that
        // is how the shader shipped for a day ignoring it entirely --
        // a leftover `let sp = 0.0` where the map row's own value
        // belonged. With every speed zero both sides agreed on a fold
        // that was not being exercised. A non-zero one now runs
        // beside the others.
        //
        // A handover hands over a LINEARISATION: the seed's basis is
        // the Jacobian at the reference, and every pixel of the view
        // continues from it. For an affine map that is exact at any
        // depth, so the affine fixtures are held to the same tolerance
        // at every handover level. For a curved one it is not, and the
        // julia dust reads 1.296 from level 1 on -- a fixed offset
        // from the first nonlinear step, not something that
        // accumulates, since its walk reaches level 1 and no further.
        // That is §5d's finding arriving at the handover, and it is
        // reported rather than asserted past level 0.
        let gasket = || vec![
            { let mut t = aff(0.5, 0.0, 0.0, 0.5, 0.0, 0.0); t.weight = 1.0; t },
            { let mut t = aff(0.5, 0.0, 0.0, 0.5, 0.5, 0.0); t.weight = 1.37; t },
            { let mut t = aff(0.5, 0.0, 0.0, 0.5, 0.25, 0.5); t.weight = 0.61; t },
        ];
        let cases: Vec<(&str, Vec<crate::scene::transforms::Transform>, bool, f32)> = vec![
            ("dragon", vec![
                aff(0.5, -0.5, 0.5, 0.5, 0.0, 0.0),
                aff(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0),
            ], false, 0.0),
            ("gasket", gasket(), false, 0.0),
            ("gasket speed 0.6", gasket(), false, 0.6),
            ("julia dust", vec![jul(2.0), jul(3.0)], true, 0.0),
        ];

        let (device, queue) = device();
        let base = crate::config::FractalConfig::default();
        let pal = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, 64, 64,
            &base.flame, base.palette_size,
        );

        for (name, mut transforms, curved, speed) in cases {
            let n = transforms.len().max(2) - 1;
            for (i, t) in transforms.iter_mut().enumerate() {
                t.color = i as f32 / n as f32;
                t.color_speed = speed;
            }
            let mut flame = crate::scene::transforms::Flame::default();
            flame.transforms = transforms;
            let guard = crate::variations::global_registry();
            let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &guard).expect("qualifies");
            drop(guard);
            let maps = MeasureMaps::of(&ifs, &flame);

            // The coarse pass, built the way the CPU gate builds it.
            let coarse = {
                let mut st = 0x9E3779B97F4A7C15u64;
                let mut rnd = move || {
                    st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    (st >> 11) as f64 / (1u64 << 53) as f64
                };
                let used: std::collections::BTreeSet<usize> =
                    ifs.maps.iter().map(|m| m.transform_index).collect();
                let total: f64 =
                    used.iter().map(|&i| flame.transforms[i].weight as f64).sum();
                let mut out = CoarseMeasure {
                    res: RES,
                    centre: ifs.ball.centre,
                    radius: ifs.ball.radius,
                    hits: vec![0; RES * RES],
                    palette_sum: vec![0.0; RES * RES],
                    samples: SAMPLES as u64,
                };
                let mut q = ifs.ball.centre;
                let mut col = 0.5f64;
                for i in 0..SAMPLES + 1000 {
                    let u = rnd();
                    let mut acc = 0.0;
                    let mut m = ifs.maps.len() - 1;
                    let mut seen: Option<usize> = None;
                    for (k, mp) in ifs.maps.iter().enumerate() {
                        if seen == Some(mp.transform_index) { continue; }
                        seen = Some(mp.transform_index);
                        acc += flame.transforms[mp.transform_index].weight as f64 / total;
                        if u <= acc { m = k; break; }
                    }
                    q = match &ifs.maps[m].forward {
                        crate::scene::ifs_analysis::Map2::Nonlinear(nl) => {
                            let b = match nl.kernel {
                                crate::scene::ifs_analysis::Kernel::Root { n: e, .. } => {
                                    (rnd() * e.unsigned_abs() as f64).floor() as u32
                                }
                                _ => 0,
                            };
                            nl.apply_branch(q, b)
                        }
                        other => other.apply(q),
                    };
                    let t = &flame.transforms[ifs.maps[m].transform_index];
                    let sp = t.color_speed as f64;
                    col = col * (1.0 + sp) * 0.5 + t.color as f64 * (1.0 - sp) * 0.5;
                    if i < 1000 || !q[0].is_finite() || !q[1].is_finite() { continue; }
                    if let Some(c) = out.index_for_test(q) {
                        out.hits[c] += 1;
                        out.palette_sum[c] += col;
                    }
                }
                out
            };

            // A view on the set, framed so the coarse cell is coarser
            // than the view pixel -- the estimator's domain.
            let smp = crate::scene::ifs_estimate::chaos_sample_for_test(&ifs, 20_000);
            let centre = *smp
                .iter()
                .max_by_key(|p| coarse.index_for_test(**p).map_or(0, |i| coarse.hits[i]))
                .expect("samples");
            let zoom = 5.0f64;
            let want_span = 2.0 * ifs.ball.radius / 2f64.powf(zoom);

            // Every handover level the walk can be held at, not just
            // 0.
            //
            // Level 0 is the only one a seed needs nothing of its own
            // for: its prefix is empty. Past it a seed carries one --
            // the product of its branch probabilities and the two
            // running numbers of the colour fold -- and this is what
            // says the packer folded them and the shader read them.
            for forced in [0u32, 1, 2, 4] {
            let mut config = crate::config::FractalConfig::default();
            config.render_mode = RenderMode::Escape;
            config.flame = flame.clone();
            config.escape.formula = "ifs_flame".to_string();
            config.escape.coloring = "ifs_measure".to_string();
            config.escape.center_re = format!("{:?}", centre[0]);
            config.escape.center_im = format!("{:?}", centre[1]);
            config.escape.zoom_log2 = (4.0 / want_span).log2();
            config.escape.supersample = 1;
            config.escape.formula_params.insert("levels".to_string(), 60.0);
            config.escape.formula_params.insert("beam".to_string(), 8.0);
            config.escape.coloring_params.insert("cells".to_string(), MEASURE_CELLS as f32);
            config.escape.coloring_params.insert("scale".to_string(), 1.0);
            let esc = config.escape.clone();
            // The VIEW, derived the way `ensure_ifs_seeds` derives it
            // rather than from the span this gate asked for.
            //
            // Those are not the same number, and assuming they were
            // is what made this gate report the shader 3.4x out on a
            // gasket: the reference was measuring a different view.
            // Everything below reads the renderer's own arithmetic.
            let span_y = 4.0 / esc.zoom_factor();
            let span_x = span_y * RW as f64 / RH as f64;
            let basis = view_basis(span_x, span_y, esc.rotation);
            let px = span_y / RH as f64;

            let mut escape = crate::escape::EscapeRenderer::new(&device, RW, RH);
            escape.ifs_force_level = Some(forced);
            let def = get_ifs(&config.escape.formula).expect("ifs_flame");
            let reg = crate::variations::global_registry();
            escape.set_ifs(pack_for(def, &config, &reg));
            drop(reg);
            escape.set_coarse(&device, &queue, &pack_coarse(&coarse));
            let mut guard = 0;
            loop {
                let mut enc = device.create_command_encoder(&Default::default());
                let done = escape.render(
                    &device, &queue, &mut enc, &esc,
                    pal.palette_view(), pal.palette_generation(),
                );
                queue.submit(std::iter::once(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if done { break; }
                guard += 1;
                assert!(guard < 10_000);
            }
            let recs = escape.read_results_full(&device, &queue).expect("records");
            escape.destroy();

            // `res.distance` is the density and `res.color` the
            // palette coordinate: the measure reuses the two fields.
            let mut derr: Vec<f64> = Vec::new();
            let mut cerr: Vec<f64> = Vec::new();
            for gy in 0..24u32 {
                for gx in 0..24u32 {
                    let x = gx * (RW / 24) + RW / 48;
                    let y = gy * (RH / 24) + RH / 48;
                    let uv = [
                        (x as f64 + 0.5) / RW as f64 - 0.5,
                        (y as f64 + 0.5) / RH as f64 - 0.5,
                    ];
                    // `basis` carries the y flip, so uv runs down the
                    // screen with the pixels -- the convention the
                    // seeded start uses.
                    let world = [
                        centre[0] + basis[0][0] * uv[0] + basis[0][1] * uv[1],
                        centre[1] + basis[1][0] * uv[0] + basis[1][1] * uv[1],
                    ];
                    let want = estimate_measure(
                        &ifs, &maps, &coarse, world, px, 8, MEASURE_CELLS, 60,
                    );
                    let r = &recs[(y * RW + x) as usize];
                    let (gd, gc) = (r.z[0] as f64, r.dz[1] as f64);
                    if want.density > 0.0 && gd > 0.0 {
                        derr.push((gd / want.density).ln().abs());
                        cerr.push((gc - want.palette).abs());
                    }
                }
            }
            // A level the walk cannot reach hands over shallower, and
            // then this is just the shallower case again.
            if derr.len() < 30 {
                println!("  {name:<12} forced {forced}: only {} pixels, skipped", derr.len());
                continue;
            }
            derr.sort_by(f64::total_cmp);
            cerr.sort_by(f64::total_cmp);
            let dm = derr[derr.len() / 2].exp();
            let cm = cerr[cerr.len() / 2];
            // A curved set's handover linearises, so only its level-0
            // walk -- which hands over nothing -- is held exactly.
            let held = !curved || forced == 0;
            println!(
                "  {name:<12} L{forced} {:>4} px | density ratio median {dm:.4} | \
                 colour |err| {cm:.5}{}",
                derr.len(),
                if held { "" } else { "  (curved: the handover linearises)" }
            );
            assert!(
                !held || (0.96..=1.04).contains(&dm),
                "{name} at handover {forced}: the shader's measure reads {dm:.4} of \
                 the f64 walk's"
            );
            assert!(
                !held || cm < 0.01,
                "{name} at handover {forced}: the shader's palette is {cm:.5} off \
                 the f64 walk's"
            );
        }
        }
    }

    /// The rendered coarse pass IS the measure a chaos game gives.
    ///
    /// D1's coarse pass comes from the flame renderer -- the one that
    /// draws every other flame -- and the walk reads it as a density
    /// and a palette coordinate. Two things have to hold and neither
    /// is obvious: the framing has to map the ball onto the grid
    /// exactly, and the inverse-sRGB ramp has to make the
    /// accumulator's mean colour come back as the mean palette
    /// coordinate.
    ///
    /// Compared against a CPU chaos game over the same ball. Not
    /// pixel by pixel -- the two draw different sample sets -- but by
    /// the two properties the walk actually uses: where the measure
    /// IS, and how much of it is there.
    #[test]
    #[ignore = "needs a GPU; run with --ignored --nocapture"]
    fn the_rendered_coarse_pass_is_the_chaos_games() {
        use crate::scene::transforms::{Flame, Transform};
        let aff = |a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, col: f32| {
            let mut t = Transform::default();
            t.a = a; t.b = b; t.c = c; t.d = d; t.e = e; t.f = f;
            t.color = col;
            t.color_speed = 0.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            t
        };
        let cases: Vec<(&str, Vec<Transform>)> = vec![
            ("gasket", vec![
                aff(0.5, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0),
                aff(0.5, 0.0, 0.0, 0.5, 0.5, 0.0, 0.5),
                aff(0.5, 0.0, 0.0, 0.5, 0.25, 0.5, 1.0),
            ]),
            ("dragon", vec![
                aff(0.5, -0.5, 0.5, 0.5, 0.0, 0.0, 0.0),
                aff(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0, 1.0),
            ]),
        ];
        let (device, queue) = device();
        const RES: u32 = 256;

        for (name, transforms) in cases {
            let mut flame = Flame::default();
            flame.transforms = transforms;
            let guard = crate::variations::global_registry();
            let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &guard).expect("qualifies");
            drop(guard);

            let gpu = coarse_measure_for(
                &device, &queue, &flame, ifs.ball.centre, ifs.ball.radius, RES, 24,
            )
            .expect("the coarse pass renders");

            // The CPU reference over the same grid.
            let cpu = {
                let mut st = 0x9E3779B97F4A7C15u64;
                let mut rnd = move || {
                    st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    (st >> 11) as f64 / (1u64 << 53) as f64
                };
                let used: std::collections::BTreeSet<usize> =
                    ifs.maps.iter().map(|m| m.transform_index).collect();
                let total: f64 =
                    used.iter().map(|&i| flame.transforms[i].weight as f64).sum();
                let mut out = crate::scene::ifs_estimate::CoarseMeasure {
                    res: RES as usize,
                    centre: ifs.ball.centre,
                    radius: ifs.ball.radius,
                    hits: vec![0; (RES * RES) as usize],
                    palette_sum: vec![0.0; (RES * RES) as usize],
                    samples: 4_000_000,
                };
                let mut q = ifs.ball.centre;
                let mut col = 0.5f64;
                for i in 0..4_001_000usize {
                    let u = rnd();
                    let mut acc = 0.0;
                    let mut m = ifs.maps.len() - 1;
                    for (k, mp) in ifs.maps.iter().enumerate() {
                        acc += flame.transforms[mp.transform_index].weight as f64 / total;
                        if u <= acc { m = k; break; }
                    }
                    q = ifs.maps[m].forward.apply(q);
                    let t = &flame.transforms[ifs.maps[m].transform_index];
                    col = col * 0.5 + t.color as f64 * 0.5;
                    if i < 1000 { continue; }
                    if let Some(c) = out.index_for_test(q) {
                        out.hits[c] += 1;
                        out.palette_sum[c] += col;
                    }
                }
                out
            };

            // 1. WHERE the measure is: the two must light the same
            // cells. A framing error moves, scales or flips the grid
            // and this is what sees it.
            let (mut both, mut only_g, mut only_c) = (0usize, 0usize, 0usize);
            for i in 0..(RES * RES) as usize {
                match (gpu.hits[i] > 0, cpu.hits[i] > 0) {
                    (true, true) => both += 1,
                    (true, false) => only_g += 1,
                    (false, true) => only_c += 1,
                    _ => {}
                }
            }
            let jaccard = both as f64 / (both + only_g + only_c).max(1) as f64;

            // 2. HOW MUCH is there, and the palette: over the cells
            // both found, which is where a comparison means anything.
            let mut dens: Vec<f64> = Vec::new();
            let mut cerr: Vec<f64> = Vec::new();
            for i in 0..(RES * RES) as usize {
                if gpu.hits[i] < 8 || cpu.hits[i] < 8 {
                    continue;
                }
                let gd = gpu.hits[i] as f64 / gpu.samples as f64;
                let cd = cpu.hits[i] as f64 / cpu.samples as f64;
                dens.push(gd / cd);
                cerr.push(
                    (gpu.palette_sum[i] / gpu.hits[i] as f64
                        - cpu.palette_sum[i] / cpu.hits[i] as f64)
                        .abs(),
                );
            }
            dens.sort_by(f64::total_cmp);
            cerr.sort_by(f64::total_cmp);
            let dm = dens[dens.len() / 2];
            let cm = cerr[cerr.len() / 2];
            println!(
                "  {name:<8} lit both {both} | gpu-only {only_g} cpu-only {only_c} | \
                 overlap {:.3} | density ratio median {dm:.3} | palette |err| {cm:.4}",
                jaccard
            );
            assert!(
                jaccard > 0.85,
                "{name}: the rendered and sampled measures light different cells \
                 (overlap {jaccard:.3}) -- the framing maps the ball onto the grid wrong"
            );
            assert!(
                (0.8..=1.25).contains(&dm),
                "{name}: the rendered measure is {dm:.3} of the sampled one"
            );
            assert!(
                cm < 0.02,
                "{name}: the rendered palette coordinate is {cm:.4} off -- the \
                 inverse-sRGB ramp is not recovering it"
            );
        }
    }

    /// The MEASURE colouring draws something through the ordinary
    /// render path.
    ///
    /// Every other gate for this feature drives the pieces directly.
    /// This one goes through `renderer::render` -- the path the CLI
    /// and the app take -- and is the only thing that would have
    /// caught the colouring shipping BLACK, which is what it did
    /// before `ensure_coarse` was wired: the coarse buffer stayed at
    /// its one dummy element, every lookup read no measure, and every
    /// pixel came back zero.
    ///
    /// It asserts a picture, not a value: lit pixels, more than one
    /// distinct brightness, and the set drawn where the DISTANCE
    /// colouring agrees it is.
    #[test]
    #[ignore = "needs a GPU; run with --ignored --nocapture"]
    fn the_measure_colouring_draws_through_the_render_path() {
        use crate::scene::transforms::{Flame, Transform};
        let jul = |power: f32, ty: f32, col: f32| {
            let mut t = Transform::default();
            t.a = 0.7071; t.b = 0.7071; t.c = -0.7071; t.d = 0.7071; t.f = ty;
            t.color = col;
            t.color_speed = 0.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("flatten", 1.0);
            t.set_variation("julian", 1.0);
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", 1.0);
            t
        };
        let mut flame = Flame::default();
        flame.transforms = vec![jul(2.0, -0.3, 0.0), jul(3.0, 0.2, 1.0)];

        let mut config = config_for(flame);
        config.escape.coloring = "ifs_measure".to_string();
        config.escape.coloring_params.clear();
        config.escape.coloring_params.insert("cells".to_string(), 4.0);
        config.escape.coloring_params.insert("scale".to_string(), 1.0);
        config.escape.zoom_log2 = 1.0;
        config.escape.center_re = "0.0".to_string();
        config.escape.center_im = "0.0".to_string();
        let measure = render(&config);

        let lit = measure
            .chunks(4)
            .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 12)
            .count();
        let shades: std::collections::BTreeSet<u8> =
            measure.chunks(4).map(|p| p[0]).collect();
        println!(
            "  measure: {lit}/{} lit, {} distinct reds",
            (W * H) as usize,
            shades.len()
        );
        assert!(
            lit > 200,
            "the measure colouring drew {lit} lit pixels of {} -- it is rendering \
             black, which is what an unbuilt coarse pass looks like",
            W * H
        );
        assert!(
            shades.len() > 8,
            "the measure drew {} distinct brightnesses -- a flat frame is not a \
             measure",
            shades.len()
        );

        // And it draws the set where the distance colouring does.
        let mut distance_cfg = config.clone();
        distance_cfg.escape.coloring = "ifs_distance".to_string();
        distance_cfg.escape.coloring_params.clear();
        distance_cfg.escape.coloring_params.insert("interior".to_string(), 0.5);
        distance_cfg.escape.coloring_params.insert("bands".to_string(), 0.0);
        distance_cfg.escape.coloring_params.insert("edge".to_string(), 1.0);
        let dist = render(&distance_cfg);

        let bright = |v: &[u8], i: usize| {
            v[i * 4] as u32 + v[i * 4 + 1] as u32 + v[i * 4 + 2] as u32
        };
        let (mut on, mut off) = (0usize, 0usize);
        let (mut on_lit, mut off_lit) = (0usize, 0usize);
        for i in 0..(W * H) as usize {
            if bright(&dist, i) > 12 {
                on += 1;
                if bright(&measure, i) > 12 {
                    on_lit += 1;
                }
            } else {
                off += 1;
                if bright(&measure, i) > 12 {
                    off_lit += 1;
                }
            }
        }
        println!(
            "  where distance says SET: {on_lit}/{on} lit by measure; \
             where it says exterior: {off_lit}/{off}"
        );
        assert!(on > 100 && off > 100, "the view is not a mix of set and exterior");
        // The measure lives ON the set, so it must light far more of
        // the interior than of the exterior.
        let on_rate = on_lit as f64 / on as f64;
        let off_rate = off_lit as f64 / off as f64;
        assert!(
            on_rate > 4.0 * off_rate.max(1e-3),
            "the measure is not concentrated on the set: {on_rate:.3} of the \
             interior lit against {off_rate:.3} of the exterior"
        );
    }

    /// The shader takes the transition graph, and takes it the same
    /// way the CPU does (`ifs-general.md` D4, G5).
    ///
    /// Three renders of one four-map square:
    ///
    /// 1. no xaos;
    /// 2. a UNIFORM xaos of one half everywhere -- which `has_xaos`
    ///    reads as real xaos, so the whole graph path engages, and
    ///    whose every row normalises to the same draw a plain weight
    ///    gives. It must be pixel-identical to (1). All-ones would be
    ///    vacuous: the flame reports no xaos at all and nothing runs.
    /// 3. a four-cycle -- after map `i` only `i+1` may follow. A
    ///    strictly smaller attractor, so it must NOT be identical,
    ///    and its distance field must be the CPU walk's.
    ///
    /// The third assertion is the one with teeth. A shader that
    /// ignored the graph would pass (1) and (2) and fail (3); one
    /// that admitted nothing would fail all three.
    #[test]
    #[ignore = "needs a GPU; run with --ignored --nocapture"]
    fn the_shader_takes_the_transition_graph() {
        use crate::scene::transforms::{Flame, Transform};
        let half = |tx: f32, ty: f32, col: f32| {
            let mut t = Transform::default();
            t.a = 0.5; t.d = 0.5; t.e = tx; t.f = ty;
            t.color = col;
            t.color_speed = 0.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            t
        };
        let build = |xaos: Option<Vec<Vec<f32>>>| {
            let mut flame = Flame::default();
            flame.transforms = vec![
                half(0.0, 0.0, 0.0),
                half(0.5, 0.0, 0.33),
                half(0.0, 0.5, 0.66),
                half(0.5, 0.5, 1.0),
            ];
            flame.xaos = xaos;
            flame
        };
        let cycle: Vec<Vec<f32>> = (0..4)
            .map(|i| (0..4).map(|j| if j == (i + 1) % 4 { 1.0 } else { 0.0 }).collect())
            .collect();

        let guard = crate::variations::global_registry();
        let base = crate::scene::ifs_analysis::analyse_2d(&build(None), &guard).expect("qualifies");
        drop(guard);

        let shot = |flame: Flame| -> Vec<u8> {
            let mut config = config_for(flame);
            config.escape.coloring = "ifs_distance".to_string();
            config.escape.formula_params.insert("levels".to_string(), 24.0);
            config.escape.formula_params.insert("beam".to_string(), 8.0);
            config.escape.center_re = format!("{:?}", base.ball.centre[0]);
            config.escape.center_im = format!("{:?}", base.ball.centre[1]);
            config.escape.zoom_log2 = (4.0 / (2.2 * base.ball.radius)).log2();
            render(&config)
        };

        let plain = shot(build(None));
        let uniform = shot(build(Some(vec![vec![0.5; 4]; 4])));
        let cycled = shot(build(Some(cycle.clone())));

        assert_eq!(
            plain, uniform,
            "a uniform xaos moved pixels -- the graph is meant to be the identity there"
        );
        let moved = plain
            .chunks(4)
            .zip(cycled.chunks(4))
            .filter(|(a, b)| a[0..3] != b[0..3])
            .count();
        assert!(
            moved > plain.len() / 4 / 20,
            "the four-cycle changed only {moved} pixels of {} -- the shader is not \
             reading the graph",
            plain.len() / 4
        );
        println!("  the four-cycle moves {moved} of {} pixels", plain.len() / 4);

        // ...and the field it draws is the CPU walk's. Sampled where
        // the two can be compared: the rendered value is a colouring
        // of the distance, so this compares the SET -- which pixels
        // read as on it -- rather than the number.
        let flame = build(Some(cycle));
        let guard = crate::variations::global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &guard).expect("qualifies");
        drop(guard);
        let mut config = config_for(flame);
        config.escape.coloring = "ifs_distance".to_string();
        config.escape.formula_params.insert("levels".to_string(), 24.0);
        config.escape.formula_params.insert("beam".to_string(), 8.0);
        config.escape.center_re = format!("{:?}", base.ball.centre[0]);
        config.escape.center_im = format!("{:?}", base.ball.centre[1]);
        config.escape.zoom_log2 = (4.0 / (2.2 * base.ball.radius)).log2();
        let (w, h) = (W, H);
        let span_y = 4.0 / config.escape.zoom_factor();
        let span_x = span_y * w as f64 / h as f64;
        let basis = view_basis(span_x, span_y, config.escape.rotation);
        let px = span_y / h as f64;
        let mut disagree = 0usize;
        let mut checked = 0usize;
        for gy in 0..16u32 {
            for gx in 0..16u32 {
                let x = gx * (w / 16) + w / 32;
                let y = gy * (h / 16) + h / 32;
                let uv = [
                    (x as f64 + 0.5) / w as f64 - 0.5,
                    (y as f64 + 0.5) / h as f64 - 0.5,
                ];
                let at = [
                    base.ball.centre[0] + basis[0][0] * uv[0] + basis[0][1] * uv[1],
                    base.ball.centre[1] + basis[1][0] * uv[0] + basis[1][1] * uv[1],
                ];
                let cpu_on =
                    crate::scene::ifs_estimate::estimate(&ifs, at, 24, 8).distance <= 2.0 * px;
                let i = ((y * w + x) * 4) as usize;
                let gpu_on = cycled[i] as u32 + cycled[i + 1] as u32 + cycled[i + 2] as u32 > 24;
                if cpu_on != gpu_on {
                    disagree += 1;
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 256);
        // A few boundary pixels may straddle the threshold, since one
        // side reads a distance and the other a tone-mapped colour.
        assert!(
            disagree <= 6,
            "{disagree} of {checked} sample points disagree about the restricted set"
        );
        println!("  CPU and GPU disagree at {disagree} of {checked} points");
    }

    /// The measure's brightness does not change when you zoom.
    ///
    /// D4, and the reason it needs answering: density per unit AREA
    /// CLIMBS as the zoom deepens, because the measure lives on a set
    /// of dimension below two and `ρ` scales as `2^(z(2−D))`. §5h put
    /// that at 6.5 stops over fifteen zoom levels on a gasket and 7.4
    /// on a grand julian. A fixed brightness therefore blows out as
    /// you go in, and the flam3 tonemap does not help: it normalises
    /// by iterations per pixel, which is iteration-invariant and not
    /// zoom-invariant.
    ///
    /// Brightness 0 is AUTO -- the view's own median density is
    /// divided out -- and this renders the same flame at four zooms
    /// two decades apart and asserts the picture's own brightness
    /// holds. Against the same run with a FIXED brightness, which is
    /// what shows the climb is real and the fix is doing something.
    #[test]
    #[ignore = "needs a GPU; run with --ignored --nocapture"]
    fn the_measures_brightness_holds_across_the_zoom() {
        use crate::scene::transforms::{Flame, Transform};
        let half = |tx: f32, ty: f32, col: f32| {
            let mut t = Transform::default();
            t.a = 0.5; t.d = 0.5; t.e = tx; t.f = ty;
            t.color = col;
            t.color_speed = 0.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            t
        };
        let mut flame = Flame::default();
        flame.transforms = vec![half(0.0, 0.0, 0.0), half(0.5, 0.0, 0.5), half(0.25, 0.5, 1.0)];
        let guard = crate::variations::global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &guard).expect("qualifies");
        drop(guard);
        // A point on the set, so every zoom has measure in frame.
        let target = crate::scene::ifs_estimate::chaos_sample_for_test(&ifs, 20_000)[10_000];

        let mut spread_of: Vec<f64> = Vec::new();
        for auto in [true, false] {
            let mut lit: Vec<f64> = Vec::new();
            for zoom in [2.0f64, 5.0, 8.0, 11.0] {
                let mut config = config_for(flame.clone());
                config.escape.coloring = "ifs_measure".to_string();
                config.escape.coloring_params.clear();
                config.escape.coloring_params.insert("cells".to_string(), 16.0);
                config.escape.coloring_params
                    .insert("scale".to_string(), if auto { 0.0 } else { 0.002 });
                config.escape.center_re = format!("{}", target[0]);
                config.escape.center_im = format!("{}", target[1]);
                config.escape.zoom_log2 =
                    (4.0 / (2.0 * ifs.ball.radius / 2f64.powf(zoom))).log2();
                let rgba = render(&config);
                // The median brightness of the pixels that have any,
                // which is what the eye reads as exposure.
                let mut b: Vec<f64> = rgba
                    .chunks(4)
                    .map(|p| (p[0] as f64 + p[1] as f64 + p[2] as f64) / 765.0)
                    .filter(|v| *v > 0.004)
                    .collect();
                if b.len() < 50 {
                    continue;
                }
                b.sort_by(f64::total_cmp);
                lit.push(b[b.len() / 2]);
            }
            assert!(lit.len() >= 3, "too few usable zooms: {}", lit.len());
            let lo = lit.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = lit.iter().copied().fold(0.0, f64::max);
            let stops = (hi / lo.max(1e-6)).log2();
            println!(
                "  {:<6} median brightness {:?} | spread {stops:.2} stops",
                if auto { "auto" } else { "fixed" },
                lit.iter().map(|v| (v * 1000.0).round() / 1000.0).collect::<Vec<_>>()
            );
            spread_of.push(stops);
        }
        let (auto, fixed) = (spread_of[0], spread_of[1]);
        assert!(
            auto < 0.15,
            "auto brightness drifts {auto:.2} stops across four zooms -- the point \
             of it is that it does not"
        );
        // And the fixed control has to DRIFT, or this gate is passing
        // on a flame that never needed the correction. It first read
        // 0.03 stops because a brightness of 1 saturates every lit
        // pixel to the same value: a control that clips is no control.
        assert!(
            fixed > 3.0 * auto,
            "the fixed control drifts only {fixed:.2} stops against auto's \
             {auto:.2} -- this view does not exercise the climb, so the gate is \
             not measuring the fix"
        );
    }

    /// The reported view (`grand-julian-glitches3.fflame`), GPU
    /// against CPU, pixel by pixel.
    ///
    /// The planar agreement gate above walks whole balls at shallow
    /// depth. This walks the view a user reported wedges in: three
    /// inversions at zoom 7.8, 35 levels, beam 5 -- where f32
    /// overflows `|v|^-15` at |v| < 0.003 and f64 holds to 1e-20, so
    /// the GPU freezes paths about 10^17 times more often than the
    /// CPU reference does. How far the two pictures are apart here is
    /// the number that says whether the residual is the walk's
    /// algorithm or the GPU's arithmetic.
    #[test]
    #[ignore = "needs a GPU; run with --ignored --nocapture"]
    fn the_gpu_agrees_with_the_cpu_on_the_reported_view() {
        use crate::scene::transforms::Transform;
        let j = |aff: [f32; 6], w: f32, power: f32| {
            let mut t = Transform::default();
            t.a = aff[0];
            t.b = aff[1];
            t.c = aff[2];
            t.d = aff[3];
            t.e = aff[4];
            t.f = aff[5];
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("flatten", 1.0);
            t.set_variation("julian", w);
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", -1.0);
            t
        };
        let mut flame = crate::scene::transforms::Flame::default();
        flame.transforms = vec![
            j([0.7071, 0.7071, -0.7071, 0.7071, 0.0, -0.3], 1.0, 2.0),
            j([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0], 0.2, 15.0),
            j([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0], 0.3, 8.0),
        ];
        let registry = crate::variations::global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &registry).expect("qualifies");

        const LEVELS: u32 = 35;
        const BEAM: u32 = 5;
        let zoom = 7.798816f64;
        let rot = 0.7853982f64;
        let centre = [-0.05554435526432207f64, -0.19381778925226906];

        let mut config = crate::config::FractalConfig::default();
        config.render_mode = crate::scene::transforms::RenderMode::Escape;
        config.flame = flame;
        config.escape.formula = "ifs_flame".to_string();
        config.escape.coloring = "ifs_distance".to_string();
        config.escape.center_re = format!("{}", centre[0]);
        config.escape.center_im = format!("{}", centre[1]);
        config.escape.zoom_log2 = zoom;
        config.escape.rotation = rot as f32;
        config.escape.formula_params.insert("levels".into(), LEVELS as f32);
        config.escape.formula_params.insert("beam".into(), BEAM as f32);
        config.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
        config.exposure = crate::config::defaults::DEFAULT_EXPOSURE;
        config.gamma = crate::config::defaults::DEFAULT_GAMMA;
        let rgba = render(&config);

        let span = 4.0 / 2f64.powf(zoom);
        let (cs, sn) = (rot.cos(), rot.sin());
        let plane = |x: u32, y: u32| {
            let u = ((x as f64 + 0.5) / W as f64 - 0.5) * span * W as f64 / H as f64;
            let v = -((y as f64 + 0.5) / H as f64 - 0.5) * span;
            [centre[0] + u * cs - v * sn, centre[1] + u * sn + v * cs]
        };
        // The CPU says which pixels are near the set: within one pixel.
        let px = span / H as f64;
        let (mut inside, mut outside) = (Vec::new(), Vec::new());
        for y in 0..H {
            for x in 0..W {
                let d = estimate(&ifs, plane(x, y), LEVELS, BEAM).distance;
                if d <= px {
                    inside.push(brightness(&rgba, x, y));
                } else {
                    outside.push(brightness(&rgba, x, y));
                }
            }
        }
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
        let (mi, mo) = (mean(&inside), mean(&outside));
        let cut = (mi + mo) * 0.5;
        let lit_in = inside.iter().filter(|&&b| b > cut).count();
        let lit_out = outside.iter().filter(|&&b| b > cut).count();
        let agree = (lit_in + (outside.len() - lit_out)) as f64 / (inside.len() + outside.len()) as f64;
        println!(
            "  reported view: CPU near {} / far {} of {}; GPU lit near {lit_in}, lit far {lit_out}; agreement {:.1}%",
            inside.len(),
            outside.len(),
            W * H,
            agree * 100.0
        );
    }
    #[test]
    #[ignore = "needs a GPU"]
    fn the_gpu_walk_agrees_with_the_cpu_reference_on_spherical_and_bubble() {
        for (name, flame) in [
            ("spherical", spherical_ifs_flame()),
            ("bubble", bubble_ifs_flame()),
            ("hemisphere", hemisphere_ifs_flame()),
            ("disc", disc_ifs_flame()),
            ("blob", blob_ifs_flame()),
        ] {
            let guard = global_registry();
            let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &guard).expect("qualifies");
            drop(guard);
            let mut config = config_for(flame);
            config.escape.center_re = format!("{}", ifs.ball.centre[0]);
            config.escape.center_im = format!("{}", ifs.ball.centre[1]);
            let span = ifs.ball.radius * 2.4;
            config.escape.zoom_log2 = (4.0 / span).log2();
            let rgba = render(&config);
            let px = span / H as f64;
            let plane = |x: u32, y: u32| -> [f64; 2] {
                let u = (x as f64 + 0.5) / W as f64 - 0.5;
                let v = (y as f64 + 0.5) / H as f64 - 0.5;
                [ifs.ball.centre[0] + u * span * W as f64 / H as f64, ifs.ball.centre[1] - v * span]
            };
            let (mut inside, mut outside) = (Vec::new(), Vec::new());
            for y in 0..H {
                for x in 0..W {
                    let d = estimate(&ifs, plane(x, y), LEVELS, BEAM).distance;
                    if d < 0.25 * px {
                        inside.push(brightness(&rgba, x, y));
                    } else if d > 3.0 * px {
                        outside.push(brightness(&rgba, x, y));
                    }
                }
            }
            println!("  {name}: interior {} / exterior {} of {} pixels", inside.len(), outside.len(), W * H);
            assert!(inside.len() > 150, "{name}: too few interior pixels: {}", inside.len());
            assert!(outside.len() > 1500, "{name}: too few exterior pixels: {}", outside.len());
            let mean = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len() as f64;
            let (mi, mo) = (mean(&inside), mean(&outside));
            assert!(mi > 0.05, "{name}: the set rendered dark ({mi:.4})");
            assert!(mi > mo * 8.0 + 0.02, "{name}: interior ({mi:.4}) and exterior ({mo:.4}) are not separated");
            let cut = (mi + mo) * 0.5;
            let lit_in = inside.iter().filter(|&&b| b > cut).count();
            let lit_out = outside.iter().filter(|&&b| b > cut).count();
            let agree = (lit_in + (outside.len() - lit_out)) as f64 / (inside.len() + outside.len()) as f64;
            assert!(
                agree > 0.97,
                "{name}: GPU and CPU disagree on {:.1}% of pixels (interior lit {lit_in}/{}, exterior lit {lit_out}/{})",
                (1.0 - agree) * 100.0,
                inside.len(),
                outside.len()
            );
        }
    }

    /// Candidate spherical and bubble presets (plan 8.9 gate 5), and
    /// the catalogue flame the census names, each rendered as a
    /// distance field AND as the flame it is, side by side.
    pub(super) fn kernel_candidates() -> Vec<(&'static str, &'static str, Flame)> {
        // Inversion in the circle of radius sqrt(w) about c: pre
        // translation -c, spherical at weight w, post translation +c.
        fn inversion(c: [f32; 2], w: f32, color: f32) -> Transform {
            let mut t = Transform::default();
            t.a = 1.0;
            t.b = 0.0;
            t.c = 0.0;
            t.d = 1.0;
            t.e = -c[0];
            t.f = -c[1];
            t.post_affine_enabled = true;
            t.post_a = 1.0;
            t.post_b = 0.0;
            t.post_c = 0.0;
            t.post_d = 1.0;
            t.post_e = c[0];
            t.post_f = c[1];
            t.color = color;
            t.variations = HashMap::from([("spherical".to_string(), w)]);
            t.variation_order = vec!["spherical".to_string()];
            t
        }
        fn contraction(s: f32, e: f32, f: f32, color: f32) -> Transform {
            let mut t = Transform::default();
            t.a = s;
            t.b = 0.0;
            t.c = 0.0;
            t.d = s;
            t.e = e;
            t.f = f;
            t.color = color;
            t.variations = HashMap::from([("linear".to_string(), 1.0)]);
            t.variation_order = vec!["linear".to_string()];
            t
        }
        let flame_of = |transforms: Vec<Transform>| {
            let mut fl = Flame::default();
            fl.transforms = transforms;
            fl.final_transforms.clear();
            fl.xaos = None;
            fl
        };
        // Three unit circles about the corners of an equilateral
        // triangle of side 2 are mutually tangent.
        let h = 3f32.sqrt();
        let mut out = vec![
            ("Inversion Pair", "ifs_distance", spherical_ifs_flame()),
            (
                "Three Circles",
                "ifs_address",
                flame_of(vec![
                    inversion([-1.0, 0.0], 1.0, 0.1),
                    inversion([1.0, 0.0], 1.0, 0.5),
                    inversion([0.0, h], 1.0, 0.9),
                ]),
            ),
            (
                "Three Circles and a Seed",
                "ifs_level",
                flame_of(vec![
                    inversion([-1.0, 0.0], 1.0, 0.1),
                    inversion([1.0, 0.0], 1.0, 0.5),
                    inversion([0.0, h], 1.0, 0.9),
                    contraction(0.3, 0.0, h / 3.0, 0.3),
                ]),
            ),
            ("Bubble Pair", "ifs_distance", bubble_ifs_flame()),
            (
                "Bubble Ring",
                "ifs_address",
                flame_of(vec![
                    contraction(0.5, 0.0, 0.0, 0.5),
                    {
                        let mut t = contraction(1.0, 0.0, 0.0, 0.1);
                        t.variations = HashMap::from([("bubble".to_string(), 2.0)]);
                        t.variation_order = vec!["bubble".to_string()];
                        t.e = 1.0;
                        t
                    },
                    {
                        let mut t = contraction(1.0, 0.0, 0.0, 0.9);
                        t.variations = HashMap::from([("bubble".to_string(), 2.0)]);
                        t.variation_order = vec!["bubble".to_string()];
                        t.e = -1.0;
                        t
                    },
                ]),
            ),
        ];
        out.push(("Hemisphere Pair", "ifs_trap", hemisphere_ifs_flame()));
        out.push(("Disc Spiral", "ifs_address", disc_ifs_flame()));
        out.push(("Blob Flower", "ifs_trap", blob_ifs_flame()));
        for (preset, name) in [
            ("JuliaN Bubble (3D)", "JuliaN Bubble"),
            ("Julian Disc", "Julian Disc"),
            ("Cup (3D)", "Cup"),
        ] {
            if let Some(c) = crate::resources::presets::load_embedded_presets()
                .expect("presets parse")
                .into_iter()
                .find(|c| c.flame.name == preset)
            {
                out.push((name, "ifs_distance", c.flame));
            }
        }
        out
    }

    /// The preset that ships from plan 8.10's candidates: the blob
    /// flower under the distance colouring with contours on and a
    /// dark interior -- the one thin set among the fold kernels, whose
    /// distance field is the picture (0.4% of pixels move between 24
    /// and 48 levels, 0.3% between 48 and 96).
    pub(super) fn kernel_presets() -> Vec<crate::config::FractalConfig> {
        let mut c = ifs_preset_config("Blob Flower", "ifs_distance", blob_ifs_flame().transforms);
        c.escape.coloring_params.insert("bands".to_string(), 12.0);
        c.escape.coloring_params.insert("interior".to_string(), 0.0);
        c.escape.formula_params.insert("levels".to_string(), 48.0);
        vec![c]
    }

    #[test]
    #[ignore = "needs a GPU; writes output/ifs/kern-*.png"]
    fn render_the_kernel_candidates_for_inspection() {
        let dir = std::path::Path::new("output/ifs");
        std::fs::create_dir_all(dir).expect("output dir");
        let (device, queue) = device();
        let guard = global_registry();
        for (name, coloring, flame) in kernel_candidates() {
            let slug = name.to_lowercase().replace(' ', "-");
            let ifs = match crate::scene::ifs_analysis::analyse_2d(&flame, &guard) {
                Ok(ifs) => ifs,
                Err(why) => {
                    println!("    {name}: does not qualify: {why:?}");
                    continue;
                }
            };
            // The distance field.
            let mut c = ifs_preset_config(name, coloring, flame.transforms.clone());
            c.escape.formula_params.insert("levels".to_string(), 48.0);
            let job = crate::renderer::RenderJob::new(&c, 448, 448);
            let out = pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress))
                .expect("render");
            let path = dir.join(format!("kern-{slug}-distance.png"));
            image::save_buffer(&path, &out.rgba_data, 448, 448, image::ColorType::Rgba8).expect("write png");
            // The flame, framed on the same ball.
            let mut f = crate::config::FractalConfig::default();
            f.flame = flame;
            f.flame.name = name.to_string();
            f.pan_x = ifs.ball.centre[0] as f32;
            f.pan_y = ifs.ball.centre[1] as f32;
            f.zoom = (4.0 / (ifs.ball.radius * 2.4)) as f32;
            f.max_iterations = 400;
            let job = crate::renderer::RenderJob::new(&f, 448, 448);
            let out = pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress))
                .expect("render");
            let path = dir.join(format!("kern-{slug}-flame.png"));
            image::save_buffer(&path, &out.rgba_data, 448, 448, image::ColorType::Rgba8).expect("write png");
            println!("    {name}: {} maps, ball r {:.3}", ifs.maps.len(), ifs.ball.radius);
        }
    }

    /// The kernel candidates as loadable `.fflame` files, one per
    /// candidate, so the colourings can be tried in the app rather
    /// than only in the one the sheet rendered.
    ///
    /// Through the config's OWN serialiser, for the reason
    /// `write_the_classical_ifs_presets` documents: a config written
    /// by serde directly carries no `version` and is migrated on load
    /// as if it were v2, which turns a mode-D config back into a
    /// flame.
    #[test]
    #[ignore = "writes output/ifs-candidates/*.fflame"]
    fn write_the_kernel_candidates_as_configs() {
        let dir = std::path::Path::new("output/ifs-candidates");
        std::fs::create_dir_all(dir).expect("output dir");
        for (name, coloring, flame) in kernel_candidates() {
            let mut c = ifs_preset_config(name, coloring, flame.transforms.clone());
            c.escape.formula_params.insert("levels".to_string(), 48.0);
            let slug = name.to_lowercase().replace(' ', "-");
            let path = dir.join(format!("{slug}.fflame"));
            std::fs::write(&path, c.to_json().expect("serialise")).expect("write");
            println!("    {}", path.display());
        }
        // And the julia ones, for the same reason.
        for (name, coloring, transforms) in julia_candidates() {
            let mut c = ifs_preset_config(name, coloring, transforms);
            c.escape.formula_params.insert("levels".to_string(), 64.0);
            let slug = name.to_lowercase().replace(' ', "-");
            let path = dir.join(format!("julia-{slug}.fflame"));
            std::fs::write(&path, c.to_json().expect("serialise")).expect("write");
            println!("    {}", path.display());
        }
    }

    /// Every mode-D colouring over every kernel candidate: what the
    /// one-colouring sheet could have hidden.
    ///
    /// The distance colouring's contour bands are ON here (the sheet
    /// left them at the flat default), because a filled disc under a
    /// flat interior colour is exactly the picture that would hide
    /// structure inside it.
    #[test]
    #[ignore = "needs a GPU; writes output/ifs/sweep-*.png"]
    fn render_every_colouring_of_the_kernel_candidates() {
        let dir = std::path::Path::new("output/ifs");
        std::fs::create_dir_all(dir).expect("output dir");
        let (device, queue) = device();
        for (name, _, flame) in kernel_candidates() {
            let slug = name.to_lowercase().replace(' ', "-");
            for coloring in ["ifs_distance", "ifs_level", "ifs_address", "ifs_trap"] {
                for (tag, tune) in [
                    ("plain", false),
                    ("tuned", true),
                ] {
                    let mut c = ifs_preset_config(name, coloring, flame.transforms.clone());
                    c.escape.formula_params.insert("levels".to_string(), 48.0);
                    if tune {
                        match coloring {
                            // Contours inside the exterior, and an
                            // interior at the palette's dark end so the
                            // set reads as a silhouette.
                            "ifs_distance" => {
                                c.escape.coloring_params.insert("bands".to_string(), 12.0);
                                c.escape.coloring_params.insert("interior".to_string(), 0.0);
                            }
                            // Many more cycles per level: a region that
                            // never escapes has no level, but its
                            // NEIGHBOURS do, and the bands say how the
                            // walk reached them.
                            "ifs_level" => {
                                c.escape.coloring_params.insert("scale".to_string(), 0.8);
                                c.escape.coloring_params.insert("smooth".to_string(), 0.0);
                            }
                            "ifs_address" => {
                                c.escape.coloring_params.insert("source".to_string(), 1.0);
                            }
                            "ifs_trap" => {
                                c.escape.coloring_params.insert("scale".to_string(), 2.0);
                            }
                            _ => {}
                        }
                    }
                    let job = crate::renderer::RenderJob::new(&c, 384, 384);
                    let out = pollster::block_on(crate::renderer::render(
                        &device,
                        &queue,
                        job,
                        &mut crate::renderer::NoProgress,
                    ))
                    .expect("render");
                    let path = dir.join(format!("sweep-{slug}-{coloring}-{tag}.png"));
                    image::save_buffer(&path, &out.rgba_data, 384, 384, image::ColorType::Rgba8)
                        .expect("write png");
                }
            }
            println!("    {name}: eight renders");
        }
    }

    /// Does the structure the trap and address colourings show inside
    /// a nonlinear region depend on the WALK'S DEPTH rather than on
    /// the set?
    ///
    /// It is a fair suspicion: a point that never escapes has no
    /// escape level and no escape point, so its trap coordinate is
    /// wherever the inverse orbit happened to be when the walk gave
    /// up, and its address is however many branches it had taken by
    /// then. If the picture moved with `levels` it would be an
    /// artefact of the parameter. Compared at 24, 48 and 96.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn does_the_interior_structure_depend_on_the_walks_depth() {
        let dir = std::path::Path::new("output/ifs");
        std::fs::create_dir_all(dir).expect("output dir");
        let (device, queue) = device();
        for (name, coloring, flame) in kernel_candidates() {
            if !matches!(name, "Three Circles and a Seed" | "JuliaN Bubble" | "Julian Disc" | "Cup" | "Disc Spiral" | "Blob Flower") {
                continue;
            }
            let coloring = match name {
                "JuliaN Bubble" | "Julian Disc" => "ifs_address",
                "Blob Flower" => "ifs_distance",
                _ => "ifs_trap",
            };
            let mut shots: Vec<(u32, Vec<u8>)> = Vec::new();
            for levels in [24u32, 48, 96] {
                let mut c = ifs_preset_config(name, coloring, flame.transforms.clone());
                c.escape.formula_params.insert("levels".to_string(), levels as f32);
                if coloring == "ifs_address" {
                    c.escape.coloring_params.insert("source".to_string(), 1.0);
                }
                let job = crate::renderer::RenderJob::new(&c, 384, 384);
                let out = pollster::block_on(crate::renderer::render(
                    &device, &queue, job, &mut crate::renderer::NoProgress,
                ))
                .expect("render");
                let path = dir.join(format!("depth-{}-{levels}.png", name.to_lowercase().replace(' ', "-")));
                image::save_buffer(&path, &out.rgba_data, 384, 384, image::ColorType::Rgba8).expect("write png");
                shots.push((levels, out.rgba_data));
            }
            for w in shots.windows(2) {
                let (a, b) = (&w[0], &w[1]);
                let diff = a.1
                    .chunks(4)
                    .zip(b.1.chunks(4))
                    .filter(|(p, q)| {
                        (p[0] as i32 - q[0] as i32).abs()
                            + (p[1] as i32 - q[1] as i32).abs()
                            + (p[2] as i32 - q[2] as i32).abs()
                            > 24
                    })
                    .count();
                println!(
                    "  {name} / {coloring}: levels {} -> {} changes {:.2}% of pixels",
                    a.0,
                    b.0,
                    100.0 * diff as f64 / (384.0 * 384.0)
                );
            }
        }
    }

    /// A quaternion Julia solid config: Bourke's constant on the k
    /// slice unless told otherwise, framed by the standalone packing's
    /// ball at the origin.
    pub(super) fn quaternion_config(c: [f32; 4], slice_axis: f32, coloring: &str) -> crate::config::FractalConfig {
        let mut cfg = crate::config::FractalConfig::default();
        cfg.render_mode = RenderMode::Escape;
        cfg.flame.name = "Quaternion Julia".to_string();
        cfg.escape.formula = IFS_QUATERNION_JULIA.name.to_string();
        cfg.escape.coloring = coloring.to_string();
        cfg.escape.formula_params.insert("cx".to_string(), c[0]);
        cfg.escape.formula_params.insert("cy".to_string(), c[1]);
        cfg.escape.formula_params.insert("cz".to_string(), c[2]);
        cfg.escape.formula_params.insert("cw".to_string(), c[3]);
        cfg.escape.formula_params.insert("slice_axis".to_string(), slice_axis);
        cfg.escape.cam_yaw = 0.9;
        cfg.escape.cam_pitch = 0.42;
        cfg.escape.zoom_log2 = 0.0;
        cfg.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
        cfg.exposure = crate::config::defaults::DEFAULT_EXPOSURE;
        cfg.gamma = crate::config::defaults::DEFAULT_GAMMA;
        cfg
    }

    /// The presets that ship from plan 8.11 step 1: Bourke's
    /// `c = (-1, 0.2, 0, 0)` on the k slice, under the escape-level
    /// colouring. Inspected before shipping
    /// (`render_the_quaternion_julia_for_inspection`).
    pub(super) fn quaternion_presets() -> Vec<crate::config::FractalConfig> {
        let mut c = quaternion_config([-1.0, 0.2, 0.0, 0.0], 3.0, "ifs_level");
        // The ball is the bailout's radius and the set is about half
        // of it: a closer frame than the ball's own.
        c.escape.zoom_log2 = 0.7;
        vec![c]
    }

    /// Plan 8.11 gate 2, the math: with `c = 0` the orbit of `q` is
    /// `|q|^(2^k)`, so the set is exactly the unit ball, and its
    /// silhouette on the pinhole is a disc whose pixel radius the
    /// camera predicts. The march finds the surface where Hart's
    /// estimate says it is; a wrong estimate, a wrong lift to 4D or a
    /// wrong ball would all move this disc.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_quaternion_julia_with_c_zero_is_the_unit_ball() {
        let mut cfg = quaternion_config([0.0, 0.0, 0.0, 0.0], 3.0, "ifs_distance");
        cfg.escape.coloring_params.insert("interior".to_string(), 0.5);
        const N: u32 = 256;
        let (device, queue) = device();
        let job = crate::renderer::RenderJob::new(&cfg, N, N);
        let out = pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress))
            .expect("render");
        let lit = out.rgba_data.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24).count();
        // The camera: the standalone ball has the bailout's radius (2)
        // and the frame distance is 3.2 radii; a unit sphere at that
        // distance subtends asin(1/D), and the vertical field of view
        // spans N pixels.
        let packed = pack_standalone(&IFS_QUATERNION_JULIA, &cfg.escape);
        let ifs3 = packed.solid.as_ref().map(|(i, _)| i).expect("standalone solid");
        let cam = solid_camera(&cfg.escape, ifs3);
        let tan_half = (cam.fov as f64 * 0.5).tan();
        let predicted = (N as f64 / 2.0) * (1.0 / cam.distance).asin().tan() / tan_half;
        let measured = (lit as f64 / std::f64::consts::PI).sqrt();
        println!("  unit ball: {lit} lit pixels, silhouette radius {measured:.2} px, camera predicts {predicted:.2} px");
        // Hart's half (Q1) makes the estimate exactly HALF the true
        // distance on the unit ball -- with c = 0, d = ½·|q0|·ln|q0|
        // for every k -- so the march, which stops at a pixel of
        // estimated distance, stops at two pixels of true distance and
        // the silhouette comes out two pixels wide. Measured +1.97 px
        // at 256; that is the estimate doing what it says, not the
        // camera or the lift being wrong, and it is gated as such.
        let excess = measured - predicted;
        assert!(
            (-0.5..=2.5).contains(&excess),
            "the silhouette is {measured:.2} px where the camera predicts {predicted:.2} (+2 for the half)"
        );
    }

    /// Bourke's set and two others, each under two colourings, for
    /// inspection before a preset ships (plan 8.11 gate 3).
    #[test]
    #[ignore = "needs a GPU; writes output/ifs/quat-*.png"]
    fn render_the_quaternion_julia_for_inspection() {
        let dir = std::path::Path::new("output/ifs");
        std::fs::create_dir_all(dir).expect("output dir");
        let (device, queue) = device();
        for (name, c, axis) in [
            ("bourke", [-1.0f32, 0.2, 0.0, 0.0], 3.0f32),
            ("bourke-i-slice", [-1.0, 0.2, 0.0, 0.0], 1.0),
            ("dendrite", [-0.2, 0.8, 0.0, 0.0], 3.0),
            ("general", [-0.3, 0.5, 0.4, 0.1], 3.0),
        ] {
            for coloring in ["ifs_level", "ifs_distance", "ifs_address"] {
                let cfg = quaternion_config(c, axis, coloring);
                let job = crate::renderer::RenderJob::new(&cfg, 448, 448);
                let out = pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress))
                    .expect("render");
                let lit = out.rgba_data.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24).count();
                let path = dir.join(format!("quat-{name}-{coloring}.png"));
                image::save_buffer(&path, &out.rgba_data, 448, 448, image::ColorType::Rgba8).expect("write png");
                println!("    {} ({lit} lit)", path.display());
            }
        }
    }

    /// A solid of 3D roots: two `julia3D`s and a contracting affine.
    pub(super) fn julia3d_pair_flame() -> Flame {
        let mut fl = Flame::default();
        fl.transforms.clear();
        fl.final_transforms.clear();
        fl.xaos = None;
        for (i, (variation, power, e, f, g, w)) in [
            ("julia3D", 2.0f32, 0.3f32, -0.2f32, 0.1f32, 0.9f32),
            ("julia3D", 2.0, -0.4, 0.3, -0.2, 0.9),
        ]
        .into_iter()
        .enumerate()
        {
            let mut t = Transform::default();
            t.a = 1.0;
            t.b = 0.0;
            t.c = 0.0;
            t.d = 1.0;
            t.e = e;
            t.f = f;
            t.g = g;
            t.color = 0.2 + 0.5 * i as f32;
            t.variations = HashMap::from([(variation.to_string(), w)]);
            t.variation_order = vec![variation.to_string()];
            t.set_variation_param(variation, "power", power);
            fl.transforms.push(t);
        }
        let mut aff = Transform::default();
        aff.a = 1.0;
        aff.b = 0.0;
        aff.c = 0.0;
        aff.d = 1.0;
        aff.e = 0.6;
        aff.f = 0.0;
        aff.g = 0.2;
        aff.color = 0.9;
        aff.variations = HashMap::from([("linear3D".to_string(), 0.5)]);
        aff.variation_order = vec!["linear3D".to_string()];
        fl.transforms.push(aff);
        fl
    }

    pub(super) fn julia3dz_pair_flame() -> Flame {
        let mut fl = julia3d_pair_flame();
        for t in fl.transforms.iter_mut().take(2) {
            t.variations = HashMap::from([("julia3Dz".to_string(), 0.9)]);
            t.variation_order = vec!["julia3Dz".to_string()];
            t.set_variation_param("julia3Dz", "power", 2.0);
        }
        fl
    }

    fn solid_config_for(flame: Flame, coloring: &str) -> crate::config::FractalConfig {
        let mut c = crate::config::FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.flame = flame;
        c.escape.formula = "ifs_flame_3d".to_string();
        c.escape.coloring = coloring.to_string();
        c.escape.zoom_log2 = 0.0;
        c.escape.cam_yaw = 0.9;
        c.escape.cam_pitch = 0.42;
        c.escape.formula_params.insert("levels".to_string(), 24.0);
        c.escape.coloring_params.insert("interior".to_string(), 0.5);
        c.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
        c.exposure = crate::config::defaults::DEFAULT_EXPOSURE;
        c.gamma = crate::config::defaults::DEFAULT_GAMMA;
        c
    }

    /// Plan 8.11 step 2, gate 3: the GPU solid of a julia3D pair
    /// agrees with a CPU march of the same rays. There is no pixel
    /// class for a solid as there is for the plane, so the CPU
    /// sphere-traces each pixel's ray on `estimate` and the hit masks
    /// are compared; the boundary band is where the two may
    /// legitimately differ by their tolerances.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_gpu_solid_agrees_with_a_cpu_march_on_a_julia3d_pair() {
        for (name, flame) in [
            ("julia3D", julia3d_pair_flame()),
            ("julia3Dz", julia3dz_pair_flame()),
            ("quaternion", quaternion_ifs_flame(&[[0.3, 0.0, 0.0, -0.6], [0.0, 0.0, 0.0, -0.5]])),
            ("quaternion-depth", quaternion_ifs_flame_proj(&[[0.0, 0.0, 0.0, 0.25]], 1.0)),
        ] {
            let mut config = solid_config_for(flame, "ifs_distance");
            let levels: u32 = std::env::var("IFS_LEVELS").ok().and_then(|v| v.parse().ok()).unwrap_or(24);
            config.escape.formula_params.insert("levels".to_string(), levels as f32);
            let guard = global_registry();
            let ifs3 = crate::scene::ifs_analysis::analyse_3d(&config.flame, &guard).expect("qualifies");
            drop(guard);
            assert!(ifs3.maps.iter().any(|m| !m.forward.is_affine()));
            let (device, queue) = device();
            const N: u32 = 96;
            let job = crate::renderer::RenderJob::new(&config, N, N);
            let out = pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress))
                .expect("render");
            let cam = solid_camera(&config.escape, &ifs3);
            let tan_half = (cam.fov as f64 * 0.5).tan();
            let (mut agree, mut total, mut gpu_hits, mut cpu_hits) = (0usize, 0usize, 0usize, 0usize);
            let mut cpu_mask = vec![0u8; (N * N) as usize];
            for y in 0..N {
                for x in 0..N {
                    let i = ((y * N + x) * 4) as usize;
                    let gpu_hit = out.rgba_data[i] as u32 + out.rgba_data[i + 1] as u32 + out.rgba_data[i + 2] as u32 > 24;
                    let u = (x as f64 + 0.5) / N as f64 - 0.5;
                    let v = (y as f64 + 0.5) / N as f64 - 0.5;
                    let mut dir = [0.0f64; 3];
                    for k in 0..3 {
                        dir[k] = cam.forward[k] + cam.right[k] * (u * 2.0 * tan_half) - cam.up[k] * (v * 2.0 * tan_half);
                    }
                    let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                    for k in 0..3 {
                        dir[k] /= len;
                    }
                    // The ray against the ball, then a sphere trace.
                    let oc: [f64; 3] = std::array::from_fn(|k| cam.eye[k] - ifs3.ball.centre[k]);
                    let b = oc[0] * dir[0] + oc[1] * dir[1] + oc[2] * dir[2];
                    let c_term = oc[0] * oc[0] + oc[1] * oc[1] + oc[2] * oc[2] - ifs3.ball.radius * ifs3.ball.radius;
                    let disc = b * b - c_term;
                    let mut cpu_hit = false;
                    if disc >= 0.0 {
                        let root = disc.sqrt();
                        let mut t = (-b - root).max(0.0);
                        let t_max = -b + root;
                        let px_at = 2.0 * tan_half / N as f64;
                        for _ in 0..96 {
                            if t > t_max {
                                break;
                            }
                            let p: [f64; 3] = std::array::from_fn(|k| cam.eye[k] + dir[k] * t);
                            // The slice is w = 0, the config's default.
                            let d = crate::scene::ifs_estimate::estimate_aux(&ifs3, p, 0.0, levels, 1).distance;
                            if d < px_at * t {
                                cpu_hit = true;
                                break;
                            }
                            t += d;
                        }
                    }
                    cpu_mask[(y * N + x) as usize] = cpu_hit as u8;
                    total += 1;
                    gpu_hits += gpu_hit as usize;
                    cpu_hits += cpu_hit as usize;
                    agree += (gpu_hit == cpu_hit) as usize;
                }
            }
            let pct = 100.0 * agree as f64 / total as f64;
            println!("  {name}: GPU hits {gpu_hits}, CPU hits {cpu_hits}, agreement {pct:.1}% of {total}");
            if std::env::var("IFS_MASK_DUMP").is_ok() {
                let dir = std::path::Path::new("output/ifs");
                std::fs::create_dir_all(dir).expect("output dir");
                let mut img = vec![0u8; (N * N * 4) as usize];
                for y in 0..N {
                    for x in 0..N {
                        let i = ((y * N + x) * 4) as usize;
                        let gpu_hit = out.rgba_data[i] as u32 + out.rgba_data[i + 1] as u32 + out.rgba_data[i + 2] as u32 > 24;
                        img[i] = if gpu_hit { 255 } else { 0 };
                        img[i + 3] = 255;
                    }
                }
                image::save_buffer(dir.join(format!("mask-{name}-gpu.png")), &img, N, N, image::ColorType::Rgba8).expect("png");
                let mut img = vec![0u8; (N * N * 4) as usize];
                for i in 0..(N * N) as usize {
                    img[i * 4] = if cpu_mask[i] != 0 { 255 } else { 0 };
                    img[i * 4 + 3] = 255;
                }
                image::save_buffer(dir.join(format!("mask-{name}-cpu.png")), &img, N, N, image::ColorType::Rgba8).expect("png");
            }
            assert!(gpu_hits > 300 && cpu_hits > 300, "{name}: too few hits to compare");
            assert!(pct > 95.0, "{name}: GPU and CPU disagree on {:.1}% of pixels", 100.0 - pct);
        }
    }

    /// Every colouring of the two solid candidates, then the depth
    /// check (plan 8.11 step 2's gate 5; 8.9's lesson).
    #[test]
    #[ignore = "needs a GPU; writes output/ifs/solid-kern-*.png"]
    fn render_the_solid_kernel_candidates_for_inspection() {
        let dir = std::path::Path::new("output/ifs");
        std::fs::create_dir_all(dir).expect("output dir");
        let (device, queue) = device();
        for (name, flame) in [
            ("julia3d-pair", julia3d_pair_flame()),
            ("julia3dz-pair", julia3dz_pair_flame()),
            ("quaternion-pair", quaternion_ifs_flame(&[[0.3, 0.0, 0.0, -0.6], [0.0, 0.0, 0.0, -0.5]])),
            ("quaternion-trio", quaternion_ifs_flame(&[[0.3, 0.0, 0.0, -0.6], [0.0, 0.0, 0.0, -0.5], [0.2, 0.0, 0.0, -1.0]])),
            // Projection 1, whose step is the polynomial after a swap
            // and whose sets are its own rather than the Julia ones.
            ("quaternion-depth-a", quaternion_ifs_flame_proj(&[[0.0, 0.0, 0.0, 0.25]], 1.0)),
            ("quaternion-depth-b", quaternion_ifs_flame_proj(&[[0.1, 0.0, 0.0, 0.0]], 1.0)),
            ("quaternion-depth-pair", quaternion_ifs_flame_proj(&[[0.0, 0.0, 0.0, 0.25], [0.1, 0.0, 0.0, 0.0]], 1.0)),
        ] {
            for coloring in ["ifs_distance", "ifs_level", "ifs_address", "ifs_trap"] {
                let c = solid_config_for(flame.clone(), coloring);
                let job = crate::renderer::RenderJob::new(&c, 384, 384);
                let out = pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress))
                    .expect("render");
                let path = dir.join(format!("solid-kern-{name}-{coloring}.png"));
                image::save_buffer(&path, &out.rgba_data, 384, 384, image::ColorType::Rgba8).expect("write png");
            }
            let mut shots = Vec::new();
            for levels in [12u32, 24, 48] {
                let mut c = solid_config_for(flame.clone(), "ifs_address");
                c.escape.formula_params.insert("levels".to_string(), levels as f32);
                let job = crate::renderer::RenderJob::new(&c, 256, 256);
                let out = pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress))
                    .expect("render");
                shots.push((levels, out.rgba_data));
            }
            for w in shots.windows(2) {
                let diff = w[0].1.chunks(4).zip(w[1].1.chunks(4))
                    .filter(|(p, q)| (p[0] as i32 - q[0] as i32).abs() + (p[1] as i32 - q[1] as i32).abs() + (p[2] as i32 - q[2] as i32).abs() > 24)
                    .count();
                println!("  {name} / ifs_address: levels {} -> {} changes {:.2}% of pixels", w[0].0, w[1].0, 100.0 * diff as f64 / (256.0 * 256.0));
            }
        }
    }

    /// Inverse-mode quaternion_julia transforms at the given constants,
    /// identity affines, weight one.
    pub(super) fn quaternion_ifs_flame(cs: &[[f32; 4]]) -> Flame {
        quaternion_ifs_flame_proj(cs, 0.0)
    }

    /// The same, at the variation's projection: 0 puts the vector part
    /// in the picture and hides the scalar, 1 (Depth) puts the scalar
    /// in and hides `k` -- the slice the lobed sets live in.
    pub(super) fn quaternion_ifs_flame_proj(cs: &[[f32; 4]], projection: f32) -> Flame {
        let mut fl = Flame::default();
        fl.transforms.clear();
        fl.final_transforms.clear();
        fl.xaos = None;
        for (i, c) in cs.iter().enumerate() {
            let mut t = Transform::default();
            t.a = 1.0;
            t.b = 0.0;
            t.c = 0.0;
            t.d = 1.0;
            t.e = 0.0;
            t.f = 0.0;
            t.color = (i as f32 + 0.5) / cs.len() as f32;
            t.variations = HashMap::from([("quaternion_julia".to_string(), 1.0)]);
            t.variation_order = vec!["quaternion_julia".to_string()];
            for (k, v) in [("cx", c[0]), ("cy", c[1]), ("cz", c[2]), ("cw", c[3]), ("power", 2.0), ("inverse", 1.0), ("projection", projection)] {
                t.set_variation_param("quaternion_julia", k, v);
            }
            fl.transforms.push(t);
        }
        fl
    }

    /// Plan 8.11 step 3's tying gate: a single inverse-mode
    /// `quaternion_julia` transform with an identity affine IS step
    /// 1's set, so the IFS walk and the standalone def, rendered from
    /// the same eye along the same rays, must agree on the hit mask.
    /// The two distances differ -- Hart's estimate against the
    /// σ-product bound -- so the boundary band may, and the interior
    /// and exterior may not.
    ///
    /// The standalone def's ball is the bailout's radius and the IFS's
    /// is found numerically, so the zoom is adjusted to put both eyes
    /// at the same distance from the same target.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_quaternion_ifs_of_one_transform_is_the_standalone_solid() {
        // -0.6 + 0.3i: in the variation's (i, j, k, scalar) layout that
        // is (0.3, 0, 0, -0.6); in the standalone def's (scalar, i, j,
        // k) layout it is (-0.6, 0.3, 0, 0). The IFS's 3D point is the
        // vector part with the scalar at w = 0, so the standalone must
        // pin its SCALAR (axis 0) at 0 for the two to be the same set.
        let c = [0.3f32, 0.0, 0.0, -0.6];
        let c_std = [c[3], c[0], c[1], c[2]];
        let (device, queue) = device();
        const N: u32 = 128;

        let mut std_cfg = quaternion_config(c_std, 0.0, "ifs_distance");
        std_cfg.escape.cam_target_x = "0".to_string();
        std_cfg.escape.cam_target_y = "0".to_string();
        std_cfg.escape.cam_target_z = "0".to_string();
        std_cfg.escape.formula_params.insert("levels".to_string(), 24.0);
        let std_packed = pack_standalone(&IFS_QUATERNION_JULIA, &std_cfg.escape);
        let std_r = std_packed.solid.as_ref().unwrap().0.ball.radius;

        let flame = quaternion_ifs_flame(&[c]);
        let guard = global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&flame, &guard).expect("qualifies");
        drop(guard);
        let mut ifs_cfg = solid_config_for(flame, "ifs_distance");
        ifs_cfg.escape.cam_target_x = "0".to_string();
        ifs_cfg.escape.cam_target_y = "0".to_string();
        ifs_cfg.escape.cam_target_z = "0".to_string();
        ifs_cfg.escape.cam_yaw = std_cfg.escape.cam_yaw;
        ifs_cfg.escape.cam_pitch = std_cfg.escape.cam_pitch;
        ifs_cfg.escape.formula_params.insert("levels".to_string(), 24.0);
        // Same eye: distance = 3.2 R / 2^zoom for each.
        ifs_cfg.escape.zoom_log2 = std_cfg.escape.zoom_log2 + (ifs3.ball.radius / std_r).log2();
        let cam_std = solid_camera(&std_cfg.escape, &std_packed.solid.as_ref().unwrap().0);
        let cam_ifs = solid_camera(&ifs_cfg.escape, &ifs3);
        assert!((cam_std.distance - cam_ifs.distance).abs() < 1e-9 * cam_std.distance);

        let render = |cfg: &crate::config::FractalConfig| {
            let job = crate::renderer::RenderJob::new(cfg, N, N);
            pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress))
                .expect("render")
                .rgba_data
        };
        let a = render(&std_cfg);
        let b = render(&ifs_cfg);
        let (mut agree, mut hits_a, mut hits_b) = (0usize, 0usize, 0usize);
        for i in 0..(N * N) as usize {
            let la = a[i * 4] as u32 + a[i * 4 + 1] as u32 + a[i * 4 + 2] as u32 > 24;
            let lb = b[i * 4] as u32 + b[i * 4 + 1] as u32 + b[i * 4 + 2] as u32 > 24;
            hits_a += la as usize;
            hits_b += lb as usize;
            agree += (la == lb) as usize;
        }
        let pct = 100.0 * agree as f64 / (N * N) as f64;
        println!("  standalone hits {hits_a}, IFS hits {hits_b}, agreement {pct:.1}% of {}", N * N);
        assert!(hits_a > 500 && hits_b > 500, "too few hits to compare ({hits_a} / {hits_b})");
        assert!(pct > 95.0, "the two arithmetics disagree on {:.1}% of pixels", 100.0 - pct);
    }

    /// The shader's seed stride is `SEED_VEC4S`.
    ///
    /// `ifs_seed(j, w)` indexes `fdata` by a stride written into the
    /// WGSL as a literal, and nothing but this connects it to the
    /// packer's constant. When the delta gained its quadratic part the
    /// stride went from four to six on the Rust side and stayed at
    /// four in the shader, which reads seed 1 onward out of the middle
    /// of seed 0's words. That is INVISIBLE on a one-seed walk -- the
    /// nonlinear GPU gate passed at 99.9% -- and cost a Sierpinski,
    /// which keeps eight, 11% of its view.
    #[test]
    fn the_shaders_seed_stride_matches_the_packer() {
        let src = crate::escape::assembler::IFS_TEMPLATE;
        let want = format!("params.fdata[4u + {}u * j + w]", super::SEED_VEC4S);
        assert!(
            src.contains(&want),
            "the shader's `ifs_seed` does not stride by SEED_VEC4S ({}); looked for {want:?}",
            super::SEED_VEC4S
        );
        // And the block it indexes into has room for what the packer
        // may write.
        assert!(
            SEED_BASE + super::SEED_VEC4S * super::MAX_SEEDS <= 64,
            "the seeds do not fit in the 64 vec4s of `fdata`"
        );
    }

    /// Rung 1 at arbitrary precision is the same map as the f64 one.
    ///
    /// `big_kernel_inverse` rebuilds `|v|^{|n|/d}·e^{i·n·arg v}` out of
    /// integer powers, an inversion and a conjugation, because a
    /// `BigFloat` has multiplication and a reciprocal and no `exp` or
    /// `atan2` to take the direct route with. Which of the two it
    /// applies depends on the signs of `n` and `d` separately -- both
    /// negative is NEITHER, which is the case a sign slip gets wrong
    /// -- so every combination is checked against the f64 formula it
    /// has to reproduce.
    #[test]
    fn the_big_kernel_inverse_is_the_f64_one() {
        use crate::scene::ifs_analysis::Kernel;
        use crate::scene::ifs_estimate::SeedPoint;
        let big = |v: f64| crate::escape::bigfloat::BigFloat::from_f64(v, 6);
        let rung1 = [
            Kernel::Spherical,
            Kernel::Root { n: 2, d: 1.0 },
            Kernel::Root { n: 3, d: 1.0 },
            Kernel::Root { n: -3, d: 1.0 },
            Kernel::Root { n: 1, d: -1.0 },
            Kernel::Root { n: 5, d: -1.0 },
            Kernel::Root { n: -5, d: -1.0 },
            Kernel::Root { n: 15, d: -1.0 },
        ];
        let points = [
            [0.7, 0.3],
            [-1.4, 0.9],
            [0.05, -0.02],
            [-0.3, -0.8],
            [1.0, 0.0],
            [0.0, 1.0],
            [-1.0, 0.0],
            [0.0, -1.0],
            [2.6, -3.1],
        ];
        for k in rung1 {
            for v in points {
                let want = k.inverse(v, 0);
                let got = super::big_kernel_inverse(k, &[big(v[0]), big(v[1])])
                    .unwrap_or_else(|| panic!("{k:?} at {v:?} should be on rung 1"));
                let got = [got[0].to_f64(), got[1].to_f64()];
                let scale = want[0].abs().max(want[1].abs()).max(1e-12);
                assert!(
                    (got[0] - want[0]).abs() / scale < 1e-12
                        && (got[1] - want[1]).abs() / scale < 1e-12,
                    "{k:?} at {v:?}: big {got:?}, f64 {want:?}"
                );
            }
        }

        // And everything else declines, so the handover stops rather
        // than walking a map it cannot take.
        let off_ladder = [
            Kernel::Bubble,
            Kernel::Hemisphere,
            Kernel::Disc,
            Kernel::Blob { high: 1.4, low: 0.3, waves: 3.0 },
            // a fractional root: |v|^(2/3) needs an exp
            Kernel::Root { n: 2, d: 3.0 },
            Kernel::Root { n: 0, d: 1.0 },
        ];
        for k in off_ladder {
            assert!(
                super::big_kernel_inverse(k, &[big(0.4), big(0.2)]).is_none(),
                "{k:?} is not on rung 1 and must decline"
            );
        }
        // The origin has no preimage under an inversion.
        assert!(
            super::big_kernel_inverse(Kernel::Spherical, &[big(0.0), big(0.0)]).is_none()
        );

        // Whole maps, affines and weight included, against the f64
        // walk's own step.
        let guard = global_registry();
        let r = &*guard;
        let mut t = crate::scene::transforms::Transform::default();
        t.a = 0.83;
        t.b = -0.24;
        t.c = 0.31;
        t.d = 0.77;
        t.e = 0.19;
        t.f = -0.12;
        t.variations.clear();
        t.variation_order.clear();
        t.variations.insert("julian".to_string(), 0.6);
        t.variation_order.push("julian".to_string());
        t.set_variation_param("julian", "power", 3.0);
        t.set_variation_param("julian", "dist", -1.0);
        let m = crate::scene::ifs_analysis::transform_map_2d_ordered(
            &t,
            r,
            &t.ordered_variation_names(r),
        )
        .expect("a nonlinear map");
        let inv = m.inverse().expect("invertible");
        drop(guard);
        for q in points {
            let want = inv.apply(q);
            let got = [big(q[0]), big(q[1])].apply_map(&inv).expect("rung 1");
            let got = [got[0].to_f64(), got[1].to_f64()];
            let scale = want[0].abs().max(want[1].abs()).max(1e-12);
            assert!(
                (got[0] - want[0]).abs() / scale < 1e-11
                    && (got[1] - want[1]).abs() / scale < 1e-11,
                "whole map at {q:?}: big {got:?}, f64 {want:?}"
            );
        }
    }

    /// A flame that fails the criterion must render EMPTY, not a
    /// frame-filling interior. `distance == 0` is what a point on the
    /// attractor returns, so "no maps" has to mean far away, not near.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_flame_that_does_not_qualify_renders_nothing() {
        let guard = global_registry();
        assert!(
            pack_flame(&folded_flame(), &guard, None).is_err(),
            "the fixture must actually fail the criterion"
        );
        drop(guard);

        let rgba = render(&config_for(folded_flame()));
        let lit = (0..H)
            .flat_map(|y| (0..W).map(move |x| (x, y)))
            .filter(|&(x, y)| brightness(&rgba, x, y) > 0.02)
            .count();
        assert!(
            lit * 100 < (W * H) as usize,
            "a non-qualifying flame lit {lit} of {} pixels -- it should draw nothing",
            W * H
        );
    }

    /// Depth is the walk's resolution knob: a deeper walk resolves
    /// more of the gasket's holes, so the lit area shrinks. If it did
    /// not, the depth parameter would not be reaching the shader.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_deeper_walk_resolves_more_holes() {
        let lit_at = |levels: u32| {
            let mut c = config_for(sierpinski_flame());
            c.escape.formula_params.insert("levels".to_string(), levels as f32);
            let rgba = render(&c);
            (0..H)
                .flat_map(|y| (0..W).map(move |x| (x, y)))
                .filter(|&(x, y)| brightness(&rgba, x, y) > 0.02)
                .count()
        };
        let shallow = lit_at(2);
        let deep = lit_at(20);
        assert!(shallow > 0 && deep > 0, "nothing rendered: {shallow}, {deep}");
        assert!(
            deep < shallow,
            "depth 20 lit {deep} pixels and depth 2 lit {shallow}: the walk's depth \
             is not reaching the shader"
        );
    }

    /// The atan2 guard the trap colouring needs must be the flame
    /// engine's, byte for byte — a divergent copy is exactly how the
    /// Metal zero-pair bug comes back.
    #[test]
    fn the_atan2_guard_is_identical_to_the_flame_one() {
        let utilities = include_str!("../../shaders/core/utilities.wgsl");
        let body = |src: &str| -> String {
            let start = src.find("fn ff_atan2(").expect("ff_atan2 present");
            let rest = &src[start..];
            let end = rest.find("\n}").expect("closing brace") + 2;
            rest[..end].split_whitespace().collect::<Vec<_>>().join(" ")
        };
        let template = crate::escape::assembler::assemble_ifs(&IFS_FLAME, &IFS_TRAP, 8);
        assert_eq!(
            body(utilities),
            body(&template),
            "the mode-D template's ff_atan2 has drifted from shaders/core/utilities.wgsl"
        );
    }

    /// Every registered combination must compile. A mode-D coloring
    /// that references a field the result struct does not carry is a
    /// shader-creation panic at the moment a user picks it, which is
    /// not where it should be found.
    #[test]
    #[ignore = "needs a GPU"]
    fn every_mode_d_combination_compiles() {
        let (device, _queue) = device();
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        for def in IFS_DEFS {
            for coloring in IFS_COLORINGS {
                // Every beam width too: each is its own compiled
                // shader, and a width the walk has never been compiled
                // at is one it may not compile at.
                // And the solid relight pass, which is a third
                // template with the same colouring spliced in.
                if def.solid {
                    let source = crate::escape::assembler::assemble_ifs_relight(coloring);
                    let _ = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some(&format!("relight|{}", coloring.name)),
                        source: wgpu::ShaderSource::Wgsl(source.into()),
                    });
                }
                for beam in [1u32, 2, 8] {
                    let source = crate::escape::assembler::assemble_ifs(def, coloring, beam);
                    let _ = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some(&format!("{}|{}|b{beam}", def.name, coloring.name)),
                        source: wgpu::ShaderSource::Wgsl(source.into()),
                    });
                }
            }
        }
        let err = pollster::block_on(scope.pop());
        assert!(err.is_none(), "a mode-D shader failed to compile: {err:?}");
    }

    /// What the beam costs on the GPU, so the default is chosen on a
    /// measurement rather than on taste.
    ///
    /// The candidate arrays are function-scope registers, so a wider
    /// beam does not just do more work — it can push the shader into
    /// spilling, which is a cliff rather than a slope. This prints the
    /// slope so the cliff is visible if it appears.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn what_the_beam_width_costs() {
        let (device, queue) = device();
        // Control: the same harness over a mode-A formula. Without it
        // there is no way to tell the walk's cost from the fixed cost
        // of a render_with round trip -- device setup, palette,
        // tonemap, effects and readback.
        {
            let mut c = config_for(dragon_flame());
            c.escape.formula = "mandelbrot".to_string();
            c.escape.coloring = "smooth".to_string();
            c.escape.max_iter = 256;
            let once = || {
                let job = crate::renderer::RenderJob::new(&c, 512, 512);
                pollster::block_on(crate::renderer::render(
                    &device,
                    &queue,
                    job,
                    &mut crate::renderer::NoProgress,
                ))
                .expect("render")
            };
            let _ = once();
            let t0 = web_time::Instant::now();
            let _ = once();
            println!(
                "  mode A (mandelbrot, 256 iter): {:>7.1} ms  <- harness floor",
                t0.elapsed().as_secs_f64() * 1000.0
            );
        }
        for &beam in &[1u32, 2, 4, 8] {
            let mut c = config_for(dragon_flame());
            c.escape.center_re = "0.35".to_string();
            c.escape.center_im = "0.25".to_string();
            c.escape.zoom_log2 = 1.6;
            c.escape.formula_params.insert("levels".to_string(), 40.0);
            c.escape.formula_params.insert("beam".to_string(), beam as f32);
            // One warm render so shader compilation is not in the number.
            let render_once = || {
                let job = crate::renderer::RenderJob::new(&c, 512, 512);
                pollster::block_on(crate::renderer::render(
                    &device,
                    &queue,
                    job,
                    &mut crate::renderer::NoProgress,
                ))
                .expect("render")
            };
            let _ = render_once();
            let t0 = web_time::Instant::now();
            let out = render_once();
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let lit = out
                .rgba_data
                .chunks(4)
                .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                .count();
            println!("  beam {beam}: {ms:>7.1} ms at 512x512, {lit} lit pixels");
        }
    }

    /// A recolour must draw exactly what a full walk would have drawn.
    ///
    /// The cache is only worth having if preferring it is invisible.
    /// This renders a colouring two ways — through the cache, by
    /// switching colouring on a renderer that already walked; and from
    /// scratch, on a renderer that has never seen the view — and
    /// requires the two images to be identical. Not close: identical.
    /// The colouring def is the same static in both paths, so any
    /// difference is the record layout or the template, and both are
    /// mistakes that would show as plausible wrong colours.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_recolour_draws_exactly_what_the_walk_would_have() {
        let (device, queue) = device();
        let base = config_for(sierpinski_flame());

        let render_on = |engines: Option<&mut crate::renderer::RenderEngines>,
                         coloring: &str|
         -> Vec<u8> {
            let mut c = base.clone();
            c.escape.coloring = coloring.to_string();
            let mut job = crate::renderer::RenderJob::new(&c, W, H);
            if let Some(e) = engines {
                job = job.with_engines(e);
            }
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };

        for coloring in ["ifs_distance", "ifs_level", "ifs_address", "ifs_trap"] {
            // A renderer that has already walked this view, then asked
            // for a different colouring: the walk is cached, so this
            // must take the recolor path.
            let mut engines = crate::renderer::RenderEngines::default();
            let _warm = render_on(Some(&mut engines), "ifs_distance");
            let cached = render_on(Some(&mut engines), coloring);
            let path = crate::escape::diag::snapshot().path;

            // A renderer that has never seen it: a full walk.
            let fresh = render_on(None, coloring);

            assert_eq!(
                cached.len(),
                fresh.len(),
                "{coloring}: different image sizes"
            );
            let differing = cached
                .iter()
                .zip(fresh.iter())
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(
                differing, 0,
                "{coloring}: the cached recolour differs from a full walk in \
                 {differing} of {} bytes (path was {path:?})",
                cached.len()
            );
            // And it really did come from the cache -- an equal image
            // proves nothing if both sides walked.
            assert_eq!(
                path, "recolor",
                "{coloring}: expected the recolor path, took {path:?}; the cache is \
                 not being hit and the comparison is vacuous"
            );
        }
    }

    /// The record the walk writes and the record the recolour reads
    /// must be the same declaration.
    ///
    /// They live in two templates, and a field added to one alone would
    /// shift every field after it — silently, into plausible wrong
    /// colours, with no validation error to point at.
    #[test]
    fn both_mode_d_templates_declare_the_same_record() {
        let walk = crate::escape::assembler::assemble_ifs(&IFS_FLAME, &IFS_DISTANCE, 8);
        let solid = crate::escape::assembler::assemble_ifs(&IFS_FLAME_3D, &IFS_DISTANCE, 8);
        let recolor = crate::escape::assembler::assemble_ifs_recolor(&IFS_DISTANCE);
        let decl = |src: &str| -> String {
            let start = src.find("struct IfsRecord {").expect("IfsRecord declared");
            let rest = &src[start..];
            let end = rest.find("\n}").expect("closing brace") + 2;
            rest[..end].split_whitespace().collect::<Vec<_>>().join(" ")
        };
        assert_eq!(
            decl(&walk),
            decl(&recolor),
            "the mode-D walk and recolor templates disagree about IfsRecord"
        );
        // The SOLID walk shares the buffer too, so all three have to
        // agree -- and it is the one that writes the shade word.
        assert_eq!(
            decl(&walk),
            decl(&solid),
            "the planar and solid walks disagree about IfsRecord"
        );

        // And the record must be the 32 bytes the shared results
        // buffer allocates per pixel, or the two modes cannot share it.
        let d = decl(&walk);
        let floats = d.matches("f32").count();
        let uints = d.matches("u32").count();
        let vec2s = d.matches("vec2<f32>").count();
        // vec2<f32> counts once as "vec2<f32>" and once as "f32".
        let words = (floats - vec2s) + uints + vec2s * 2;
        assert_eq!(words, 8, "IfsRecord is {words} words, not the 32 bytes mode A uses: {d}");
    }

    /// What the recolor cache buys, at the size the app runs.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn what_a_colouring_change_costs_with_and_without_the_cache() {
        let (device, queue) = device();
        let cfg = crate::resources::presets::load_embedded_presets()
            .expect("presets parse")
            .into_iter()
            .find(|c| c.flame.name == "Heighway Dragon")
            .expect("the dragon preset");

        let time = |engines: Option<&mut crate::renderer::RenderEngines>, coloring: &str| {
            let mut c = cfg.clone();
            c.escape.coloring = coloring.to_string();
            let mut job = crate::renderer::RenderJob::new(&c, 1920, 1080);
            if let Some(e) = engines {
                job = job.with_engines(e);
            }
            let t0 = web_time::Instant::now();
            let _ = pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render");
            (t0.elapsed().as_secs_f64() * 1000.0, crate::escape::diag::snapshot().path)
        };

        // The floor: a `render_with` round trip at this size does
        // device setup, a flame renderer, the tonemap and effects tail
        // and an 8 MB readback before any escape work happens. Timing
        // without subtracting it measures mostly that -- which is how
        // the beam's cost was first reported wrong.
        let floor = {
            let mut c = cfg.clone();
            c.escape.formula = "mandelbrot".to_string();
            c.escape.coloring = "smooth".to_string();
            let once = || {
                let job = crate::renderer::RenderJob::new(&c, 1920, 1080);
                let _ = pollster::block_on(crate::renderer::render(
                    &device,
                    &queue,
                    job,
                    &mut crate::renderer::NoProgress,
                ))
                .expect("render");
            };
            once();
            let t0 = web_time::Instant::now();
            once();
            t0.elapsed().as_secs_f64() * 1000.0
        };

        let mut engines = crate::renderer::RenderEngines::default();
        let (walk, walk_path) = time(Some(&mut engines), "ifs_distance");
        let (recolour, recolour_path) = time(Some(&mut engines), "ifs_level");
        let (again, again_path) = time(Some(&mut engines), "ifs_address");
        // A reading at or below zero means the recolour costs less
        // than the control's own iteration -- it is free at this
        // resolution, not negative.
        println!("  harness floor    {floor:>7.1} ms  (mode A, subtracted below)");
        println!(
            "  first walk       {:>7.1} ms of work  ({walk_path})",
            walk - floor
        );
        println!(
            "  colouring change {:>7.1} ms of work  ({recolour_path})",
            recolour - floor
        );
        println!(
            "  and another      {:>7.1} ms of work  ({again_path})",
            again - floor
        );
        assert_eq!(recolour_path, "recolor");
        assert_eq!(again_path, "recolor");
        let walk_work = walk - floor;
        let recolour_work = recolour - floor;
        assert!(
            walk_work > 0.0 && recolour_work * 4.0 < walk_work,
            "the cache saved little: {recolour_work:.0} ms of work against a              {walk_work:.0} ms walk"
        );
    }

    /// How deep mode D claims to zoom, as a `zoom_log2`.
    ///
    /// MEASURED, not chosen.
    ///
    /// It was **20** while the shader walked from `params.center`, a
    /// `vec2<f32>` quantised to about 6e-8 near 0.28: at 2²² that is
    /// 6% of the view and at 2²⁶ the whole of it, so the render agreed
    /// with the reference at chance. §2.5's reference orbit moved it
    /// here — the CPU walks the shared prefix and the shader continues
    /// from a handover where f32 is enough.
    ///
    /// **40 is now the f64 CENTRE's limit, not the walk's.** The
    /// seeding is f64, so the centre is quantised to 5.5e-17 and that
    /// is about 1% of the view by 2⁴⁹. `FixedPoint::from_decimal` and
    /// `limbs_for_view` are what raise it further, and the config
    /// already stores the centre as an exact decimal string for
    /// exactly that.
    pub(super) const DEEP_ZOOM_LIMIT: f64 = 200.0;

    /// How deep the render agrees with the reference.
    ///
    /// The reference is the SEEDED CPU walk at the precision the zoom
    /// asks for — `seed_beam` + `estimate_seeded`, which
    /// `the_centres_precision_is_what_makes_the_deep_zoom_work` shows
    /// has converged in precision and which a direct f64 walk cannot
    /// match past about 2⁴⁵.
    ///
    /// The centre is **(5/14, 1/7)**, which repeats forever in decimal
    /// and is the fixed point of `m₀∘m₁∘m₂`, so the view keeps finding
    /// structure however far it zooms. Earlier versions of this sweep
    /// centred on 0.25 and then on a depth-40 attractor point; the
    /// first is f32-exact and reported no wall at all, the second is
    /// only within 2⁻⁴⁰ of the set and ran out from under the view.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn how_deep_the_render_agrees_with_the_reference() {
        let guard = global_registry();
        let flame = sierpinski_flame();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &guard).expect("qualifies");
        drop(guard);

        let re = super::tests::decimal(5, 14, 400);
        let im = super::tests::decimal(1, 7, 400);
        let mut deepest = 0.0f64;

        println!("  zoom  depth  agreement  (interior/exterior pixels)");
        for &zoom in &[0.0f64, 16.0, 32.0, 64.0, 96.0, 128.0, 160.0, 200.0] {
            // Depth has to keep up with the zoom or the walk cannot
            // resolve what the view shows: at σ = ½ each level buys one
            // bit. Past 256 the def's own ceiling binds.
            let levels = (zoom as u32 + 24).min(256);
            let mut c = config_for(flame.clone());
            c.escape.formula_params.insert("levels".to_string(), levels as f32);
            c.escape.center_re = re.clone();
            c.escape.center_im = im.clone();
            c.escape.zoom_log2 = zoom;

            let rgba = {
                let (device, queue) = device();
                let job = crate::renderer::RenderJob::new(&c, W, H);
                pollster::block_on(crate::renderer::render(
                    &device,
                    &queue,
                    job,
                    &mut crate::renderer::NoProgress,
                ))
                .expect("render")
                .rgba_data
            };

            let dir = std::path::Path::new("output/ifs");
            std::fs::create_dir_all(dir).expect("output dir");
            image::save_buffer(
                dir.join(format!("deep-2e{zoom}.png")),
                &rgba,
                W,
                H,
                image::ColorType::Rgba8,
            )
            .expect("write png");

            let span_y = 4.0 / 2f64.powf(zoom);
            let span_x = span_y * W as f64 / H as f64;
            let seeds = crate::scene::ifs_estimate::seed_beam(
                &ifs,
                centre_at_precision(&c.escape).expect("centre parses"),
                view_basis(span_x, span_y, 0.0),
                span_y / H as f64,
                zoom as u32 + 64,
                BEAM,
            );

            let mut inside = Vec::new();
            let mut outside = Vec::new();
            for y in 0..H {
                for x in 0..W {
                    let uv = [
                        (x as f64 + 0.5) / W as f64 - 0.5,
                        (y as f64 + 0.5) / H as f64 - 0.5,
                    ];
                    // Already in pixels, like the shader's.
                    let d = crate::scene::ifs_estimate::estimate_seeded(
                        &ifs,
                        &seeds,
                        uv,
                        levels,
                        BEAM,
                    )
                    .distance;
                    let b = brightness(&rgba, x, y);
                    if d < 0.25 {
                        inside.push(b);
                    } else if d > 3.0 {
                        outside.push(b);
                    }
                }
            }
            if inside.len() < 50 || outside.len() < 50 {
                println!(
                    "  {zoom:>4}  {levels:>5}  (too few of one class: {} in, {} out)",
                    inside.len(),
                    outside.len()
                );
                continue;
            }
            let mean = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len() as f64;
            let (mi, mo) = (mean(&inside), mean(&outside));
            let cut = (mi + mo) * 0.5;
            let agree = (inside.iter().filter(|&&b| b > cut).count()
                + outside.iter().filter(|&&b| b <= cut).count())
                as f64
                / (inside.len() + outside.len()) as f64;
            println!(
                "  {zoom:>4}  {levels:>5}  {:>6.1}%     ({} in, {} out)",
                agree * 100.0,
                inside.len(),
                outside.len()
            );
            if zoom <= DEEP_ZOOM_LIMIT {
                assert!(
                    agree > 0.99,
                    "at zoom 2^{zoom} the render agrees with the reference on only \
                     {:.1}% of pixels, inside the depth this build claims",
                    agree * 100.0
                );
                deepest = deepest.max(zoom);
            }
        }
        assert!(
            deepest >= DEEP_ZOOM_LIMIT,
            "the sweep never reached the claimed limit of 2^{DEEP_ZOOM_LIMIT}"
        );
    }

    /// A 1080p mode-D render must finish, and finish in bands.
    ///
    /// The arithmetic is gated separately
    /// (`a_1080p_mode_d_view_bands_into_dispatches_a_driver_will_survive`);
    /// this is the end-to-end version, because the failure it exists
    /// for was not a slow render but a driver reset, and no unit test
    /// on a row count can see that.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn a_1080p_render_of_every_shipped_preset_finishes() {
        let (device, queue) = device();
        for cfg in crate::resources::presets::load_embedded_presets().expect("presets parse") {
            if get_ifs(&cfg.escape.formula).is_none() {
                continue;
            }
            let t0 = web_time::Instant::now();
            let job = crate::renderer::RenderJob::new(&cfg, 1920, 1080);
            let out = pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render");
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let lit = out
                .rgba_data
                .chunks(4)
                .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                .count();
            println!("  {:22} {ms:>7.1} ms at 1920x1080, {lit} lit", cfg.flame.name);
            assert!(lit > 1920 * 1080 / 400, "{:?} rendered blank", cfg.flame.name);
            // Not a performance target — a hang detector. The failure
            // this guards took seconds per frame and reset the driver.
            assert!(ms < 8000.0, "{:?} took {ms:.0} ms at 1080p", cfg.flame.name);
        }
    }

    /// Render the shipped mode-D presets exactly as they are saved —
    /// their own view, colouring and supersampling. This is what a
    /// user meets, so it is what gets looked at.
    #[test]
    #[ignore = "needs a GPU; writes images for inspection"]
    fn render_the_shipped_presets_for_inspection() {
        let dir = std::path::Path::new("output/ifs");
        std::fs::create_dir_all(dir).expect("output dir");
        let mut seen = 0;
        for cfg in crate::resources::presets::load_embedded_presets().expect("presets parse") {
            if get_ifs(&cfg.escape.formula).is_none() {
                continue;
            }
            seen += 1;
            let (device, queue) = device();
            let job = crate::renderer::RenderJob::new(&cfg, 512, 512);
            let out = pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render");
            // A preset that renders an empty frame is the failure this
            // exists to catch: the criterion and the framing are both
            // gated elsewhere, but "qualifies and is in view" does not
            // by itself mean "draws something".
            let lit = out
                .rgba_data
                .chunks(4)
                .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                .count();
            let slug = cfg.flame.name.to_lowercase().replace(' ', "-");
            let path = dir.join(format!("preset-{slug}.png"));
            image::save_buffer(&path, &out.rgba_data, 512, 512, image::ColorType::Rgba8)
                .expect("write png");
            println!("wrote {} ({lit} lit)", path.display());
            // A curve is measure zero, so the bar is low -- this is
            // catching an EMPTY frame, not judging composition.
            assert!(
                lit > 400,
                "preset {:?} lit only {lit} pixels",
                cfg.flame.name
            );
        }
        assert_eq!(seen, 11, "expected eleven IFS presets (four classical, two solid, three julia, one blob, one quaternion), found {seen}");
    }

    /// A 3D flame: the XY affine is identity plus a translation and
    /// `linear3D` at half weight scales all three axes, so the map is
    /// `p ↦ (p + v)/2` and its fixed point is `v`.
    ///
    /// `linear` will not do. An Apophysis-style transform leaves the z
    /// SCALE at one, which is why phase 0's census found none of 34
    /// three-dimensional candidates qualifying as solid: their
    /// attractor is a stack of planes, not a body.
    fn half3(v: [f32; 3], colour: f32) -> Transform {
        let mut t = Transform::default();
        t.a = 1.0;
        t.b = 0.0;
        t.c = 0.0;
        t.d = 1.0;
        t.e = v[0];
        t.f = v[1];
        t.g = v[2];
        t.color = colour;
        t.variations = HashMap::from([("linear3D".to_string(), 0.5)]);
        t.variation_order = vec!["linear3D".to_string()];
        t
    }

    fn solid_flame(transforms: Vec<Transform>) -> Flame {
        //  and  are CONFIG fields, not flame
        // ones, since the v3 migration -- and `analyse_3d` reads
        // neither: it composes the 3D affine and asks whether it
        // contracts, which is a property of the transforms alone.
        let mut fl = Flame::default();
        fl.transforms = transforms;
        fl.final_transforms.clear();
        fl.xaos = None;
        fl
    }

    /// The planar gasket, for tests that compare the two walks.
    pub(super) fn gpu_tests_sierpinski() -> Flame {
        sierpinski_flame()
    }

    /// Four half-scale maps to alternating cube corners.
    pub(super) fn tetrahedron_flame() -> Flame {
        solid_flame(vec![
            half3([0.0, 0.0, 0.0], 0.08),
            half3([1.0, 1.0, 0.0], 0.36),
            half3([1.0, 0.0, 1.0], 0.64),
            half3([0.0, 1.0, 1.0], 0.92),
        ])
    }

    /// Twenty maps at a third: the Menger sponge, the plan's §4
    /// picture — "twenty affine maps looks like a Menger sponge, not
    /// like a point cloud with lights on it".
    pub(super) fn menger_flame() -> Flame {
        let mut ts = Vec::new();
        for i in 0..3i32 {
            for j in 0..3i32 {
                for k in 0..3i32 {
                    // The sponge keeps a cell unless it is centred on
                    // two or more axes.
                    let mid = (i == 1) as i32 + (j == 1) as i32 + (k == 1) as i32;
                    if mid >= 2 {
                        continue;
                    }
                    let n = ts.len() as f32;
                    let mut t = Transform::default();
                    t.a = 1.0;
                    t.d = 1.0;
                    // p -> (p + 2v)/3 has fixed point v, so the
                    // translation is twice the corner at this scale.
                    t.e = i as f32;
                    t.f = j as f32;
                    t.g = k as f32;
                    t.color = n / 20.0;
                    t.variations =
                        HashMap::from([("linear3D".to_string(), 1.0 / 3.0)]);
                    t.variation_order = vec!["linear3D".to_string()];
                    ts.push(t);
                }
            }
        }
        solid_flame(ts)
    }

    /// The solid renders, for inspection. This is §0's first item:
    /// whether a distance march looks better than the splat pipeline
    /// with lights on it.
    #[test]
    #[ignore = "needs a GPU; writes images for inspection"]
    fn render_the_solid_ifss_for_inspection() {
        let dir = std::path::Path::new("output/ifs");
        std::fs::create_dir_all(dir).expect("output dir");

        let guard = global_registry();
        for (name, flame) in [("tetrahedron", tetrahedron_flame()), ("menger", menger_flame())]
        {
            let ifs3 = crate::scene::ifs_analysis::analyse_3d(&flame, &guard)
                .unwrap_or_else(|why| panic!("{name} must qualify as solid: {why:?}"));
            println!("  {name}: {} maps, radius {:.3}", ifs3.maps.len(), ifs3.ball.radius);

            // The geometry, checked on the CPU rather than read off a
            // render: both of these sets have their middle cell
            // removed, so the centre of the unit cube is a HOLE. A
            // low-contrast colouring can hide that, and a wrong map
            // layout can fake it.
            let middle = crate::scene::ifs_estimate::estimate(
                &ifs3,
                [0.5, 0.5, 0.5],
                30,
                8,
            )
            .distance;
            println!("    centre of the cube is {middle:.4} from the set");
            assert!(
                middle > 0.05,
                "{name}: the centre should be a hole, got {middle:.4}"
            );
            // And a corner is ON it.
            let corner = crate::scene::ifs_estimate::estimate(
                &ifs3,
                ifs3.maps[0].forward.fixed_point().expect("contractive"),
                30,
                8,
            );
            assert!(!corner.escaped, "{name}: a fixed point should be on the set");

            for coloring in ["ifs_address", "ifs_distance", "ifs_level"] {
                let mut c = config_for(flame.clone());
                c.escape.formula = "ifs_flame_3d".to_string();
                c.escape.coloring = coloring.to_string();
                c.escape.zoom_log2 = 0.0;
                c.escape.cam_yaw = 0.9;
                c.escape.formula_params.insert("levels".to_string(), 20.0);
                c.escape.coloring_params.insert("reach".to_string(), 0.0);
                c.escape.coloring_params.insert("interior".to_string(), 0.5);

                let (device, queue) = device();
                let job = crate::renderer::RenderJob::new(&c, 448, 448);
                let out = pollster::block_on(crate::renderer::render(
                    &device,
                    &queue,
                    job,
                    &mut crate::renderer::NoProgress,
                ))
                .expect("render");
                let lit = out
                    .rgba_data
                    .chunks(4)
                    .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                    .count();
                let path = dir.join(format!("solid-{name}-{coloring}.png"));
                image::save_buffer(&path, &out.rgba_data, 448, 448, image::ColorType::Rgba8)
                    .expect("write png");
                println!("    {} ({lit} lit)", path.display());
                assert!(
                    lit > 448 * 448 / 100,
                    "{name}/{coloring} rendered almost nothing ({lit} lit)"
                );
            }
        }
    }

    /// The solid's camera angles, rendered for inspection: the shipped
    /// sponge preset as is, then banked, then rolled, then panned a
    /// third of the frame to the right. The roll must be a turn of
    /// the picture and nothing else; the pan must slide it.
    #[test]
    #[ignore = "needs a GPU; writes output/ifs/solid-camera-*.png"]
    fn render_the_solid_camera_angles_for_inspection() {
        let dir = std::path::Path::new("output/ifs");
        std::fs::create_dir_all(dir).expect("output dir");
        let base = crate::resources::presets::load_embedded_presets()
            .expect("presets parse")
            .into_iter()
            .find(|c| c.escape.formula == "ifs_flame_3d" && c.flame.transforms.len() == 20)
            .expect("the sponge ships");
        let guard = global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&base.flame, &guard).expect("qualifies");
        drop(guard);
        let cam = solid_camera(&base.escape, &ifs3);
        let (m, e) = solid_pixel_step(&base.escape, &ifs3, 448.0);
        let step = m * 2f64.powi(e as i32);

        let mut panned = base.clone();
        for k in 0..3 {
            let v = ifs3.ball.centre[k] - 150.0 * step * cam.right[k];
            *[&mut panned.escape.cam_target_x, &mut panned.escape.cam_target_y, &mut panned.escape.cam_target_z][k] =
                format!("{v}");
        }
        let mut banked = base.clone();
        banked.escape.cam_bank = 0.5;
        let mut rolled = base.clone();
        rolled.escape.rotation = 0.5;

        let (device, queue) = device();
        for (name, c) in [("base", &base), ("bank", &banked), ("roll", &rolled), ("pan", &panned)] {
            let job = crate::renderer::RenderJob::new(c, 448, 448);
            let out = pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render");
            let path = dir.join(format!("solid-camera-{name}.png"));
            image::save_buffer(&path, &out.rgba_data, 448, 448, image::ColorType::Rgba8)
                .expect("write png");
            println!("    {}", path.display());
        }
    }

    /// Moving the camera must change the picture on a renderer that
    /// has already drawn one.
    ///
    /// Reported from the app: the camera sliders did nothing, and
    /// resizing the window fixed it. Two causes, both about a pass
    /// that is allowed to continue when it should start over —
    ///
    /// - the recolor cache's key had no camera in it, so a camera
    ///   change HIT the cache and re-coloured the old geometry. The
    ///   picture could not change until something else invalidated
    ///   the key, and a resize is exactly that;
    /// - a solid walk wrote a record only where a ray HIT, so every
    ///   pixel that missed kept the previous view's record, and a
    ///   re-colour painted the last frame's solid into this frame's
    ///   empty space.
    ///
    /// The fresh render is the control: a renderer that has never seen
    /// the view cannot have a stale anything.
    #[test]
    #[ignore = "needs a GPU"]
    fn moving_the_camera_changes_the_picture_through_the_cache() {
        let (device, queue) = device();
        let shot = |engines: Option<&mut crate::renderer::RenderEngines>,
                    yaw: f32,
                    coloring: &str|
         -> Vec<u8> {
            let mut c = config_for(menger_flame());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = coloring.to_string();
            c.escape.cam_yaw = yaw;
            c.escape.coloring_params.insert("reach".to_string(), 0.0);
            c.escape.formula_params.insert("levels".to_string(), 14.0);
            let mut job = crate::renderer::RenderJob::new(&c, 160, 160);
            if let Some(e) = engines {
                job = job.with_engines(e);
            }
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };

        // A renderer that has already drawn one view, then asked for
        // another camera.
        let mut engines = crate::renderer::RenderEngines::default();
        let _first = shot(Some(&mut engines), 0.0, "ifs_address");
        let reused = shot(Some(&mut engines), 1.3, "ifs_address");
        let fresh = shot(None, 1.3, "ifs_address");
        let differing = reused.iter().zip(&fresh).filter(|(a, b)| a != b).count();
        assert_eq!(
            differing, 0,
            "a camera change on a warm renderer differs from a fresh one in              {differing} of {} bytes -- the pass did not restart",
            reused.len()
        );

        // And a COLOURING change must still take the cache, or the
        // fix has simply disabled it.
        let recoloured = shot(Some(&mut engines), 1.3, "ifs_level");
        let path = crate::escape::diag::snapshot().path;
        assert_eq!(path, "recolor", "a colouring change stopped using the cache");
        let fresh_level = shot(None, 1.3, "ifs_level");
        // Byte-identity, again. It was relaxed to a last-place
        // tolerance while a solid's record carried its lighting as one
        // scalar (the walk summed a term per light, the recolour
        // multiplied by the scalar, and the two rounded differently).
        // Both paths end in the same relight pass over the same
        // geometry now, so there is no second arithmetic route to
        // round differently and nothing to tolerate.
        let differing = recoloured.iter().zip(&fresh_level).filter(|(a, b)| a != b).count();
        assert_eq!(
            differing, 0,
            "the cached recolour of a solid differs from a fresh walk in {differing} of \
             {} bytes -- stale records, or the walk and the relight disagree about the rig",
            recoloured.len(),
        );

        // The same for the walk's own parameters. `levels` is in the
        // keys already, but it is what the report named, so it is what
        // the test names. Its own renderer, warmed at the depth it is
        // about to leave -- sharing the one above would compare two
        // states that never matched.
        let mut warm = crate::renderer::RenderEngines::default();
        let _ = shot(Some(&mut warm), 1.3, "ifs_address");
        let deeper = {
            let mut c = config_for(menger_flame());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            c.escape.cam_yaw = 1.3;
            c.escape.coloring_params.insert("reach".to_string(), 0.0);
            c.escape.formula_params.insert("levels".to_string(), 22.0);
            let job = crate::renderer::RenderJob::new(&c, 160, 160)
                .with_engines(&mut warm);
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let fresh_deeper = {
            let mut c = config_for(menger_flame());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            c.escape.cam_yaw = 1.3;
            c.escape.coloring_params.insert("reach".to_string(), 0.0);
            c.escape.formula_params.insert("levels".to_string(), 22.0);
            let job = crate::renderer::RenderJob::new(&c, 160, 160);
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let differing = deeper.iter().zip(&fresh_deeper).filter(|(a, b)| a != b).count();
        assert_eq!(
            differing, 0,
            "a depth change on a warm renderer differs from a fresh one in {differing}              bytes"
        );
    }

    /// Does a solid render need the beam it is paying for?
    ///
    /// The planar default is 8, and phase 1 measured why: an
    /// attractor whose pieces share a boundary is rendered wrong
    /// without it. But the shipped solids TILE — the sponge's twenty
    /// sub-cubes are disjoint, the tetrahedron's four meet at points —
    /// and a march pays the beam on every step, not once per pixel.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn what_a_solid_render_pays_for_its_beam() {
        let (device, queue) = device();
        for (name, flame) in [("tetrahedron", tetrahedron_flame()), ("menger", menger_flame())] {
            let mut prev: Option<Vec<u8>> = None;
            for beam in [1u32, 2, 4, 8] {
                let mut c = config_for(flame.clone());
                c.escape.formula = "ifs_flame_3d".to_string();
                c.escape.coloring = "ifs_address".to_string();
                c.escape.cam_yaw = 0.9;
                c.escape.coloring_params.insert("reach".to_string(), 0.0);
                c.escape.formula_params.insert("beam".to_string(), beam as f32);
                let once = || {
                    let job = crate::renderer::RenderJob::new(&c, 256, 256);
                    pollster::block_on(crate::renderer::render(
                        &device,
                        &queue,
                        job,
                        &mut crate::renderer::NoProgress,
                    ))
                    .expect("render")
                    .rgba_data
                };
                let _ = once();
                let t0 = web_time::Instant::now();
                let px = once();
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                let differing = prev
                    .as_ref()
                    .map(|p: &Vec<u8>| p.iter().zip(&px).filter(|(a, b)| a != b).count());
                match differing {
                    Some(d) => println!(
                        "  {name:<12} beam {beam}: {ms:>7.1} ms, {d} bytes differ from the                          narrower one"
                    ),
                    None => println!("  {name:<12} beam {beam}: {ms:>7.1} ms"),
                }
                prev = Some(px);
            }
        }
    }

    /// A shadow must darken SOME of the lit surface and leave the rest
    /// alone.
    ///
    /// That is the assertion that separates a shadow from a dimmer
    /// switch, and it is the failure worth guarding: a march that
    /// starts on the surface reports every point as occluding itself,
    /// which darkens everything uniformly and still looks like a
    /// change. So the test asks for both populations at once — pixels
    /// the light reaches untouched, and pixels it does not — and
    /// separately that no pixel got BRIGHTER, because a shadow can
    /// only take light away.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_shadow_darkens_some_of_the_lit_surface_and_not_all_of_it() {
        let (device, queue) = device();
        let shot = |shadow: f32| -> Vec<u8> {
            let mut c = config_for(tetrahedron_flame());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            c.escape.cam_yaw = 0.9;
            c.escape.coloring_params.insert("reach".to_string(), 0.0);
            c.escape.formula_params.insert("levels".to_string(), 18.0);
            c.escape.formula_params.insert("shadow".to_string(), shadow);
            // Measured through a LINEAR display curve, so the byte is
            // proportional to the light that reached the surface. The
            // default gamma of 4 is a display choice and it compresses
            // the eightfold drop from a lit surface to the ambient
            // floor into about two deciles of output -- which would
            // make every threshold below a statement about the tone
            // curve rather than about the shadow. The exposure is set
            // so nothing clips, checked rather than assumed.
            c.gamma = 1.0;
            c.exposure = 0.6;
            let job = crate::renderer::RenderJob::new(&c, 192, 192);
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };

        let plain = shot(0.0);
        let shadowed = shot(1.0);
        let lum = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;

        let mut surface = 0usize;
        let mut untouched = 0usize;
        let mut darkened = 0usize;
        let mut brighter = 0usize;
        for (a, b) in plain.chunks(4).zip(shadowed.chunks(4)) {
            let (la, lb) = (lum(a), lum(b));
            if la <= 24 {
                continue;
            }
            surface += 1;
            // +3 of slack for the rounding of three 8-bit channels,
            // not for a trend: a trend shows up in the count.
            if lb > la + 3 {
                brighter += 1;
            }
            let ratio = lb as f64 / la as f64;
            if ratio > 0.98 {
                untouched += 1;
            } else if ratio < 0.75 {
                darkened += 1;
            }
        }

        assert!(surface > 192 * 192 / 20, "nothing rendered: {surface} lit pixels");
        let clipped = plain.chunks(4).filter(|p| p[..3].iter().any(|&v| v == 255)).count();
        assert!(
            clipped * 50 < surface,
            "{clipped} of {surface} lit pixels are saturated -- at this exposure the \
             picture cannot show a shadow, so nothing below is a measurement of one"
        );
        assert_eq!(brighter, 0, "{brighter} of {surface} pixels got BRIGHTER under a shadow");
        assert!(
            darkened * 20 > surface,
            "only {darkened} of {surface} lit pixels fell into shadow -- \
             a shadow that shadows nothing"
        );
        assert!(
            untouched * 20 > surface,
            "only {untouched} of {surface} lit pixels kept their light -- \
             this is a dimmer, not a shadow"
        );
    }

    /// The sharpness knob must move the PENUMBRA, not the shadow.
    ///
    /// The soft edge is the one thing this costs nothing for — the
    /// march already measured how close it passed — so the honest test
    /// is that the partially-lit population shrinks as the knob rises,
    /// while the fully-dark one does not move with it.
    #[test]
    #[ignore = "needs a GPU"]
    fn sharpening_a_shadow_narrows_its_penumbra() {
        let (device, queue) = device();
        let shot = |k: f32| -> Vec<u8> {
            let mut c = config_for(tetrahedron_flame());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            c.escape.cam_yaw = 0.9;
            c.escape.coloring_params.insert("reach".to_string(), 0.0);
            c.escape.formula_params.insert("levels".to_string(), 18.0);
            c.escape.formula_params.insert("shadow".to_string(), 1.0);
            c.escape.formula_params.insert("shadow_sharpness".to_string(), k);
            // Measured through a LINEAR display curve, so the byte is
            // proportional to the light that reached the surface. The
            // default gamma of 4 is a display choice and it compresses
            // the eightfold drop from a lit surface to the ambient
            // floor into about two deciles of output -- which would
            // make every threshold below a statement about the tone
            // curve rather than about the shadow. The exposure is set
            // so nothing clips, checked rather than assumed.
            c.gamma = 1.0;
            c.exposure = 0.6;
            let job = crate::renderer::RenderJob::new(&c, 192, 192);
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let lum = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;

        let reference = {
            let mut c = config_for(tetrahedron_flame());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            c.escape.cam_yaw = 0.9;
            c.escape.coloring_params.insert("reach".to_string(), 0.0);
            c.escape.formula_params.insert("levels".to_string(), 18.0);
            c.escape.formula_params.insert("shadow".to_string(), 0.0);
            // Measured through a LINEAR display curve, so the byte is
            // proportional to the light that reached the surface. The
            // default gamma of 4 is a display choice and it compresses
            // the eightfold drop from a lit surface to the ambient
            // floor into about two deciles of output -- which would
            // make every threshold below a statement about the tone
            // curve rather than about the shadow. The exposure is set
            // so nothing clips, checked rather than assumed.
            c.gamma = 1.0;
            c.exposure = 0.6;
            let job = crate::renderer::RenderJob::new(&c, 192, 192);
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };

        let penumbra = |px: &[u8]| {
            reference
                .chunks(4)
                .zip(px.chunks(4))
                .filter(|(a, b)| {
                    let la = lum(a);
                    if la <= 24 {
                        return false;
                    }
                    let r = lum(b) as f64 / la as f64;
                    (0.45..=0.95).contains(&r)
                })
                .count()
        };

        let soft = penumbra(&shot(2.0));
        let hard = penumbra(&shot(96.0));
        assert!(soft > 0, "no partially-lit pixels at all -- the penumbra is missing");
        assert!(
            hard * 2 < soft,
            "sharpening left {hard} partially-lit pixels against {soft} soft ones -- \
             the knob is not reaching the penumbra"
        );
    }

    /// What the shadow march actually does to the picture, as a
    /// distribution rather than a look.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn how_the_shadow_march_lands() {
        let (device, queue) = device();
        for (subject, flame) in
            [("tetra", tetrahedron_flame()), ("menger", menger_flame())]
        {
        let shot = |shadow: f32, k: f32, el: f32, steps: f32, beam: f32| -> Vec<u8> {
            let mut c = config_for(flame.clone());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            c.escape.cam_yaw = 0.9;
            c.escape.coloring_params.insert("reach".to_string(), 0.0);
            c.escape.formula_params.insert("levels".to_string(), 18.0);
            c.escape.formula_params.insert("shadow".to_string(), shadow);
            c.escape.formula_params.insert("shadow_sharpness".to_string(), k);
            c.escape.formula_params.insert("sun_elevation".to_string(), el);
            c.escape.formula_params.insert("steps".to_string(), steps);
            c.escape.formula_params.insert("beam".to_string(), beam);
            c.gamma = 1.0;
            c.exposure = 0.6;
            let job = crate::renderer::RenderJob::new(&c, 192, 192);
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let lum = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;

        for &(k, el, steps, beam) in &[
            (12.0f32, 40.0f32, 96.0f32, 1.0f32),
            (12.0, 15.0, 96.0, 1.0),
            (2.0, 40.0, 96.0, 1.0),
        ] {
            let plain = shot(0.0, k, el, steps, beam);
            let dark = shot(1.0, k, el, steps, beam);
            let mut bins = [0usize; 11];
            let mut surface = 0usize;
            for (a, b) in plain.chunks(4).zip(dark.chunks(4)) {
                let la = lum(a);
                if la <= 24 {
                    continue;
                }
                surface += 1;
                let r = (lum(b) as f64 / la as f64).clamp(0.0, 1.0);
                bins[(r * 10.0).round() as usize] += 1;
            }
            let pct: Vec<String> = bins
                .iter()
                .map(|n| format!("{:>4.1}", 100.0 * *n as f64 / surface.max(1) as f64))
                .collect();
            let mut abs = [0usize; 11];
            for a in plain.chunks(4) {
                let la = lum(a);
                if la > 24 {
                    abs[(la as f64 / 765.0 * 10.0).round() as usize] += 1;
                }
            }
            let apct: Vec<String> = abs
                .iter()
                .map(|n| format!("{:>4.1}", 100.0 * *n as f64 / surface.max(1) as f64))
                .collect();
            println!(
                "  {subject:<7} k={k:<5} el={el:<5} beam={beam} steps={steps:<5}: {surface} lit\n     \
                 ratio    {}\n     absolute {}",
                pct.join(" "),
                apct.join(" ")
            );
        }
        }
    }

    /// How deep a SOLID zoom agrees with the reference — asked of the
    /// picture, not of the arithmetic.
    ///
    /// At each zoom the same rays are marched on the CPU, seeded from
    /// the same chain, and the hits compared with the render's. The
    /// reference runs in f64 against the shader's f32, which is where
    /// its authority comes from; that it is sound at these depths is
    /// not taken on trust either, because the fixture is EXACTLY
    /// self-similar (see below) and a reference that had drifted
    /// would stop repeating. It does repeat, to the pixel, across
    /// sixteen cycles.
    ///
    /// The target is a point ON the attractor with an eventually
    /// periodic address — the fixed point of S₁∘S₂∘S₃, at (6,5,3)/7.
    /// The sevenths are the point. A dyadic target is the trap the
    /// plane's deep-zoom gate fell into: every f64 is dyadic, and a
    /// dyadic target's inverse orbit lands exactly on a fixed point,
    /// so the picture is self-similar whatever the precision. And
    /// because that composition is a similarity of ratio ⅛, the
    /// picture repeats every three octaves of zoom — which is what
    /// makes the reference auditable.
    ///
    /// Asserted to **2⁸⁰**, measured clean to there and printed
    /// beyond. Before the chain this was 2¹², and the three faults
    /// between those numbers are recorded in the plan.
    #[test]
    #[ignore = "needs a GPU"]
    fn how_deep_a_solid_render_agrees_with_the_reference() {
        const N: u32 = 96;
        let guard = global_registry();
        let flame = tetrahedron_flame();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&flame, &guard).expect("qualifies");
        drop(guard);

        let (device, queue) = device();
        println!("  zoom  levels   agree   (hit / miss on the reference)");
        // Gated to 2^80; printed past it, because what happens past
        // it is a graceful loss of precision rather than a cliff and
        // the number it happens at is worth watching.
        const GATED_TO: f64 = 80.0;
        let mut worst_gated = 100.0f64;
        let mut worst_interior = 0usize;
        for &zoom in &[0.0f64, 16.0, 32.0, 48.0, 64.0, 80.0, 96.0, 112.0, 140.0] {
            let levels = (zoom as u32 + 24).min(256);
            let mut c = config_for(flame.clone());
            c.escape.formula = "ifs_flame_3d".to_string();
            // The DISTANCE colouring, with a mid-palette interior: a
            // hit is lit whatever address it has. The address
            // colouring maps some addresses near black, and "lit"
            // would then be measuring the palette rather than the
            // geometry.
            c.escape.coloring = "ifs_distance".to_string();
            c.escape.coloring_params.insert("interior".to_string(), 0.5);
            c.escape.coloring_params.insert("bands".to_string(), 0.0);
            c.escape.coloring_params.insert("edge".to_string(), 1.0);
            c.escape.formula_params.insert("levels".to_string(), levels as f32);
            // Lighting OFF, as flat ambient. This test asks where the
            // SURFACE is, and it reads that off "is the pixel lit" --
            // so a surface turned away from the key light and sitting
            // in a crevice would be counted as absent, which is a
            // statement about the rig rather than about the geometry.
            // (Measured: it read 68% agreement at 2^16, where the
            // geometry is exact.)
            c.solid_shading.shading_strength = 1.0;
            c.solid_shading.ambient = 1.0;
            c.solid_shading.diffuse = 0.0;
            c.solid_shading.specular = 0.0;
            c.solid_shading.ssao_strength = 0.0;
            c.escape.cam_target_x = super::tests::decimal(6, 7, 60);
            c.escape.cam_target_y = super::tests::decimal(5, 7, 60);
            c.escape.cam_target_z = super::tests::decimal(3, 7, 60);
            c.escape.zoom_log2 = zoom;

            let job = crate::renderer::RenderJob::new(&c, N, N);
            let rgba = pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data;

            // The same rays, marched on the CPU. Transcribed from the
            // solid template rather than approximated -- a reference
            // that marches differently measures the difference between
            // two marchers, not the precision of one -- and seeded
            // from the same chain, because the shader's total depth is
            // its link plus its level count and a reference that
            // stopped shallower would read the difference in DEPTH as
            // a difference in precision.
            let cam = solid_camera(&c.escape, &ifs3);
            let finest = 2.0 * (cam.fov as f64 * 0.5).tan() * cam.distance / N as f64;
            let chain = crate::scene::ifs_estimate::seed_chain3(
                &ifs3,
                super::target_at_precision(&c.escape, &ifs3).expect("target parses"),
                finest,
                (zoom as u32 + 64).min(super::MAX_CHAIN_LINKS as u32),
                1,
            );
            let tan_half = (cam.fov as f64 * 0.5).tan();
            let steps = 96u32;
            {
                let d = cam.eye_rel[0]
                    .hypot(cam.eye_rel[1])
                    .hypot(cam.eye_rel[2]);
                println!(
                    "        chain {} links, cap {:.4}, eye |delta| {:.3e}, link {}, finest {:.3e}",
                    chain.levels.len(),
                    chain.cap,
                    d,
                    chain.level_for(d),
                    finest
                );
            }
            let mut refhit = vec![false; (N * N) as usize];
            let mut agree = 0usize;
            let mut hits = 0usize;
            for y in 0..N {
                for x in 0..N {
                    let u = (x as f64 + 0.5) / N as f64 - 0.5;
                    let v = (y as f64 + 0.5) / N as f64 - 0.5;
                    let mut dir = [0.0f64; 3];
                    for k in 0..3 {
                        dir[k] = cam.forward[k] + cam.right[k] * (u * 2.0 * tan_half)
                            - cam.up[k] * (v * 2.0 * tan_half);
                    }
                    let n = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                    for d in dir.iter_mut() {
                        *d /= n;
                    }

                    // Everything below is an offset from the TARGET,
                    // exactly as the shader has it.
                    let eye_rel = cam.eye_rel;
                    let to_ball = [
                        cam.target[0] - ifs3.ball.centre[0],
                        cam.target[1] - ifs3.ball.centre[1],
                        cam.target[2] - ifs3.ball.centre[2],
                    ];
                    let oc = [
                        eye_rel[0] + to_ball[0],
                        eye_rel[1] + to_ball[1],
                        eye_rel[2] + to_ball[2],
                    ];
                    let b = oc[0] * dir[0] + oc[1] * dir[1] + oc[2] * dir[2];
                    let cc = oc[0] * oc[0] + oc[1] * oc[1] + oc[2] * oc[2]
                        - ifs3.ball.radius * ifs3.ball.radius;
                    let disc = b * b - cc;
                    let mut hit = false;
                    if disc >= 0.0 {
                        let root = disc.sqrt();
                        let mut t = (-b - root).max(0.0);
                        let t_max = -b + root;
                        let px_at = 2.0 * tan_half / N as f64;
                        for _ in 0..steps {
                            if t > t_max {
                                break;
                            }
                            let q = [
                                eye_rel[0] + dir[0] * t,
                                eye_rel[1] + dir[1] * t,
                                eye_rel[2] + dir[2] * t,
                            ];
                            let d = crate::scene::ifs_estimate::estimate_seeded3(
                                &ifs3, &chain, q, levels, 1,
                            )
                            .distance;
                            // The floor is only against t = 0; a
                            // larger one would find the surface early
                            // once the whole view is smaller than it,
                            // which is the shader bug this test found.
                            if d < (px_at * t).max(1e-300) {
                                hit = true;
                                break;
                            }
                            t += d;
                        }
                    }
                    refhit[(y * N + x) as usize] = hit;
                    if hit {
                        hits += 1;
                    }
                    let i = ((y * N + x) * 4) as usize;
                    let lit = rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32 > 24;
                    if lit == hit {
                        agree += 1;
                    }
                }
            }
            let total = (N * N) as usize;
            let pct = 100.0 * agree as f64 / total as f64;
            // Where the disagreement sits. A precision fault shows at
            // the SILHOUETTE, where a pixel is a hair from the
            // surface either way; a structural fault is spread through
            // the interior.
            let mut edge_dis = 0usize;
            let mut interior_dis = 0usize;
            for y in 1..N - 1 {
                for x in 1..N - 1 {
                    let at = |xx: u32, yy: u32| refhit[(yy * N + xx) as usize];
                    let me = at(x, y);
                    let boundary = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .any(|(dx, dy)| at((x as i32 + dx) as u32, (y as i32 + dy) as u32) != me);
                    let i = ((y * N + x) * 4) as usize;
                    let lit = rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32 > 24;
                    if lit != me {
                        if boundary {
                            edge_dis += 1;
                        } else {
                            interior_dis += 1;
                        }
                    }
                }
            }
            println!(
                "        disagreement: {edge_dis} at the silhouette, {interior_dis} inside"
            );
            if zoom <= GATED_TO {
                worst_gated = worst_gated.min(pct);
                worst_interior = worst_interior.max(interior_dis);
            }
            println!(
                "  2^{zoom:<4} {levels:<7} {pct:>5.1}%   ({hits} hit / {} miss)",
                total - hits
            );
        }
        // Two bounds, because they catch different things. The
        // reference walks at full depth and the render walks to a
        // hundredth of a pixel, so a handful of SILHOUETTE pixels may
        // land on the other side of a hit -- measured at one to five
        // of 9216 per zoom. A precision fault shows there. A
        // structural fault -- a wrong link, a squared length, an
        // absolute floor -- shows INSIDE the surface, and there the
        // tolerance is zero.
        assert!(
            worst_interior == 0,
            "a solid render disagrees with the reference INSIDE the surface at \
             {worst_interior} pixels somewhere up to 2^{GATED_TO} -- that is structural, \
             not precision: the seed chain, the link choice or an epsilon regressed"
        );
        assert!(
            worst_gated >= 99.8,
            "a solid render disagrees with the reference at {worst_gated:.1}% somewhere \
             up to 2^{GATED_TO} -- more than a hundredth of a pixel at the silhouette \
             accounts for; if you raised the ceiling, raise GATED_TO and say so"
        );
    }

    /// A solid walk lights itself from the Solid Rendering panel.
    ///
    /// Three separate claims, because three separate things could be
    /// wired wrong and each fails silently: the lights have to REACH
    /// the shader, they have to be in the right space, and a change to
    /// them has to restart the render rather than sit behind a cache.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_panels_lights_steer_a_solid_walk() {
        let (device, queue) = device();
        let shot = |f: &dyn Fn(&mut crate::config::FractalConfig)| -> Vec<u8> {
            let mut c = config_for(tetrahedron_flame());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            c.escape.cam_yaw = 0.9;
            c.escape.coloring_params.insert("reach".to_string(), 0.0);
            c.escape.formula_params.insert("levels".to_string(), 18.0);
            c.gamma = 1.0;
            c.exposure = 0.6;
            f(&mut c);
            let job = crate::renderer::RenderJob::new(&c, 160, 160);
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let lum = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;
        let differing = |a: &[u8], b: &[u8]| {
            a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| lum(x) != lum(y)).count()
        };

        // An untouched panel is the fallback rig -- one white key
        // light -- because a solid with no light is a silhouette, and
        // "the user has set nothing" is not a request for one.
        let untouched = shot(&|_| {});
        let surface = untouched.chunks(4).filter(|p| lum(p) > 24).count();
        assert!(surface > 160 * 160 / 20, "nothing rendered: {surface}");

        // Moving the key light moves the shading.
        let moved = shot(&|c| {
            c.solid_shading.shading_strength = 1.0;
            c.solid_shading.specular = 0.0;
            c.solid_shading.lights[0].enabled = true;
            c.solid_shading.lights[0].azimuth = -70.0;
            c.solid_shading.lights[0].elevation = 5.0;
        });
        assert!(
            differing(&untouched, &moved) * 5 > surface,
            "moving the key light changed {} of {surface} lit pixels -- the panel is \
             not reaching the walk",
            differing(&untouched, &moved)
        );

        // A SECOND light can only add light, never remove it: this is
        // the assertion that catches a light landing in the wrong slot
        // or the wrong space, which "the picture changed" would not.
        let two = shot(&|c| {
            c.solid_shading.shading_strength = 1.0;
            c.solid_shading.specular = 0.0;
            c.solid_shading.lights[0].enabled = true;
            c.solid_shading.lights[0].azimuth = -70.0;
            c.solid_shading.lights[0].elevation = 5.0;
            c.solid_shading.lights[1].enabled = true;
            c.solid_shading.lights[1].azimuth = 120.0;
            c.solid_shading.lights[1].elevation = 30.0;
            c.solid_shading.lights[1].intensity = 0.8;
        });
        let mut darker = 0usize;
        let mut brighter = 0usize;
        for (a, b) in moved.chunks(4).zip(two.chunks(4)) {
            if lum(a) <= 24 {
                continue;
            }
            if lum(b) < lum(a) - 3 {
                darker += 1;
            } else if lum(b) > lum(a) + 3 {
                brighter += 1;
            }
        }
        assert_eq!(darker, 0, "{darker} pixels got DARKER when a light was added");
        assert!(
            brighter * 10 > surface,
            "adding a second light brightened only {brighter} of {surface} pixels"
        );

        // A coloured light has to reach the picture as a COLOUR.
        let red = shot(&|c| {
            c.solid_shading.shading_strength = 1.0;
            c.solid_shading.specular = 0.0;
            c.solid_shading.lights[0].enabled = true;
            c.solid_shading.lights[0].color = [1.0, 0.2, 0.2];
        });
        let white = shot(&|c| {
            c.solid_shading.shading_strength = 1.0;
            c.solid_shading.specular = 0.0;
            c.solid_shading.lights[0].enabled = true;
        });
        let ratio = |px: &[u8]| {
            let (mut r, mut b) = (0u64, 0u64);
            for p in px.chunks(4) {
                if lum(p) > 24 {
                    r += p[0] as u64;
                    b += p[2] as u64;
                }
            }
            r as f64 / b.max(1) as f64
        };
        assert!(
            ratio(&red) > ratio(&white) * 1.2,
            "a red light did not redden the picture: {:.3} against {:.3}",
            ratio(&red),
            ratio(&white)
        );
    }

    /// A shadowed solid must not collapse as the eye closes in, and
    /// the zoom it survives must not depend on the RESOLUTION.
    ///
    /// Reported from a Menger sponge: a hair of extra zoom and a whole
    /// section went to its ambient floor, and it arrived sooner at
    /// higher supersampling. That pairing is the diagnosis. The shadow
    /// ray's start offset is a few pixels and its hit threshold was a
    /// fixed fraction of the attractor, so the two crossed — and past
    /// the crossing every ray reported itself blocked by the surface it
    /// started on. Shrinking the pixel, by zooming or by sampling
    /// harder, brought the crossing closer.
    ///
    /// So this renders the same approach at two pixel sizes and asks
    /// for two things a collapse cannot give: that the picture keeps
    /// its variation, and that the two sizes agree about how bright it
    /// is. Either alone would be passed by a fault that darkened both
    /// equally, or by one that only ever fired at one resolution.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_shadowed_solid_survives_the_eye_reaching_its_surface() {
        let (device, queue) = device();
        let shot = |zoom: f64, n: u32| -> Vec<u8> {
            let mut c = config_for(menger_flame());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            c.escape.coloring_params.insert("reach".to_string(), 0.0);
            c.escape.cam_pitch = 0.611;
            c.escape.cam_yaw = 0.785;
            c.escape.zoom_log2 = zoom;
            c.escape.formula_params.insert("shadow".to_string(), 1.0);
            c.solid_shading.shading_strength = 1.0;
            c.solid_shading.ambient = 0.05;
            c.solid_shading.lights[0].enabled = true;
            let job = crate::renderer::RenderJob::new(&c, n, n);
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let stats = |px: &[u8]| {
            let v: Vec<f64> = px
                .chunks(4)
                .map(|p| p[0] as f64 + p[1] as f64 + p[2] as f64)
                .collect();
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            let var = v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / v.len() as f64;
            (mean, var.sqrt())
        };

        // Up to the zoom where the eye reaches the bounding sphere --
        // `FRAME_DISTANCE` is 3.2, so that is log2(3.2) = 1.68. Past
        // it the camera is inside the solid and a flat frame is the
        // honest answer, which is why the sweep stops there.
        for step in 0..=8 {
            let zoom = 1.60 + 0.01 * step as f64;
            let (m_small, s_small) = stats(&shot(zoom, 128));
            let (m_large, s_large) = stats(&shot(zoom, 320));
            assert!(
                s_large > 40.0 && s_small > 40.0,
                "at zoom {zoom:.2} the picture flattened: spread {s_small:.1} at 128px \
                 and {s_large:.1} at 320px -- a shadow that blocks everything"
            );
            let ratio = m_large / m_small.max(1.0);
            assert!(
                (0.85..=1.18).contains(&ratio),
                "at zoom {zoom:.2} the two resolutions disagree about brightness by \
                 {ratio:.2}x ({m_small:.0} at 128px against {m_large:.0} at 320px) -- \
                 the shadow is keyed to the pixel rather than to the geometry"
            );
        }
    }

    /// Where a solid render's time goes, by switching each part off.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn where_a_solid_renders_time_goes() {
        let (device, queue) = device();
        let run = |name: &str, f: &dyn Fn(&mut crate::config::FractalConfig)| {
            let mut c = config_for(menger_flame());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            c.escape.cam_yaw = 0.9;
            c.escape.coloring_params.insert("reach".to_string(), 0.0);
            c.solid_shading.shading_strength = 1.0;
            c.solid_shading.lights[0].enabled = true;
            f(&mut c);
            let once = || {
                let job = crate::renderer::RenderJob::new(&c, 512, 512);
                pollster::block_on(crate::renderer::render(
                    &device,
                    &queue,
                    job,
                    &mut crate::renderer::NoProgress,
                ))
                .expect("render")
                .rgba_data
            };
            let _ = once();
            let t0 = web_time::Instant::now();
            let _ = once();
            let _ = once();
            let ms = t0.elapsed().as_secs_f64() * 500.0;
            println!("  {name:<44} {ms:>8.1} ms");
        };
        run("defaults (levels 24, steps 96, ao, shadow .7)", &|_| {});
        run("shadow off", &|c| {
            c.escape.formula_params.insert("shadow".to_string(), 0.0);
        });
        run("shadow off, occlusion off", &|c| {
            c.escape.formula_params.insert("shadow".to_string(), 0.0);
            c.escape.formula_params.insert("occlusion".to_string(), 0.0);
        });
        run("shadow off, occlusion off, levels 12", &|c| {
            c.escape.formula_params.insert("shadow".to_string(), 0.0);
            c.escape.formula_params.insert("occlusion".to_string(), 0.0);
            c.escape.formula_params.insert("levels".to_string(), 12.0);
        });
        run("shadow off, occlusion off, levels 6", &|c| {
            c.escape.formula_params.insert("shadow".to_string(), 0.0);
            c.escape.formula_params.insert("occlusion".to_string(), 0.0);
            c.escape.formula_params.insert("levels".to_string(), 6.0);
        });
        run("shadow off, occlusion off, steps 32", &|c| {
            c.escape.formula_params.insert("shadow".to_string(), 0.0);
            c.escape.formula_params.insert("occlusion".to_string(), 0.0);
            c.escape.formula_params.insert("steps".to_string(), 32.0);
        });
        run("defaults at 256x256 (for the pixel scaling)", &|c| {
            c.escape.supersample = 1;
        });
    }

    /// A lighting change on a solid is a RELIGHT, and it is exact.
    ///
    /// Every input the rig applies at relight -- intensity, colour,
    /// ambient, diffuse, specular, shininess, occlusion strength,
    /// shadow strength, fog -- must hit the cache and come out
    /// byte-identical to a fresh walk, because both end in the same
    /// relight pass over the same geometry. That includes the three
    /// the old scalar cache had to refuse: a coloured light, a
    /// specular, a fog.
    ///
    /// And every input the WALK reads -- a light's direction, whether
    /// it is on, whether shadows are traced at all -- must miss the
    /// cache, re-walk, and still match a fresh render. The two lists
    /// are the whole design, so both are asserted.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_lighting_change_on_a_solid_is_a_relight_and_it_is_exact() {
        let (device, queue) = device();
        let shot = |engines: Option<&mut crate::renderer::RenderEngines>,
                    f: &dyn Fn(&mut crate::config::FractalConfig)|
         -> Vec<u8> {
            let mut c = config_for(menger_flame());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            c.escape.cam_yaw = 0.9;
            c.escape.coloring_params.insert("reach".to_string(), 0.0);
            c.escape.formula_params.insert("levels".to_string(), 14.0);
            c.solid_shading.shading_strength = 1.0;
            c.solid_shading.lights[0].enabled = true;
            c.solid_shading.lights[1].enabled = true;
            f(&mut c);
            let mut job = crate::renderer::RenderJob::new(&c, 160, 160);
            if let Some(e) = engines {
                job = job.with_engines(e);
            }
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let diff = |a: &[u8], b: &[u8]| a.iter().zip(b).filter(|(x, y)| x != y).count();

        let mut engines = crate::renderer::RenderEngines::default();
        let base = shot(Some(&mut engines), &|_| {});

        // Relight-only inputs: cache hit, exact.
        let relights: Vec<(&str, Box<dyn Fn(&mut crate::config::FractalConfig)>)> = vec![
            ("intensity", Box::new(|c| c.solid_shading.lights[0].intensity = 1.7)),
            ("a coloured light", Box::new(|c| c.solid_shading.lights[1].color = [1.0, 0.3, 0.2])),
            ("ambient", Box::new(|c| c.solid_shading.ambient = 0.5)),
            ("diffuse", Box::new(|c| c.solid_shading.diffuse = 0.4)),
            ("specular", Box::new(|c| { c.solid_shading.specular = 0.6; c.solid_shading.shininess = 12.0; })),
            ("occlusion strength", Box::new(|c| c.solid_shading.ssao_strength = 0.2)),
            ("shadow strength", Box::new(|c| { c.escape.formula_params.insert("shadow".to_string(), 0.25); })),
            ("fog", Box::new(|c| { c.fog_strength = 0.8; c.fog_start = 0.2; c.background_color = [0.1, 0.2, 0.4]; })),
        ];
        for (name, f) in &relights {
            let warm = shot(Some(&mut engines), f.as_ref());
            let path = crate::escape::diag::snapshot().path;
            assert_eq!(path, "recolor", "changing {name} did not take the cache");
            let fresh = shot(None, f.as_ref());
            let d = diff(&warm, &fresh);
            assert_eq!(
                d, 0,
                "changing {name} through the cache differs from a fresh render in {d} bytes"
            );
            assert!(diff(&warm, &base) > 0, "changing {name} changed nothing");
        }

        // Geometry inputs: cache miss, re-walk, still exact.
        let walks: Vec<(&str, Box<dyn Fn(&mut crate::config::FractalConfig)>)> = vec![
            ("a light's direction", Box::new(|c| c.solid_shading.lights[0].azimuth = -60.0)),
            ("a light switched off", Box::new(|c| c.solid_shading.lights[1].enabled = false)),
            ("shadows off entirely", Box::new(|c| { c.escape.formula_params.insert("shadow".to_string(), 0.0); })),
            ("occlusion reach", Box::new(|c| { c.escape.formula_params.insert("occlusion".to_string(), 0.3); })),
        ];
        for (name, f) in &walks {
            let warm = shot(Some(&mut engines), f.as_ref());
            let path = crate::escape::diag::snapshot().path;
            assert_ne!(path, "recolor", "changing {name} took the cache, but the walk reads it");
            let fresh = shot(None, f.as_ref());
            let d = diff(&warm, &fresh);
            assert_eq!(d, 0, "changing {name} on a warm renderer differs from fresh in {d} bytes");
        }
    }

    /// What a lighting edit costs, against the walk it replaces.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn what_a_lighting_change_costs_with_the_geometry_cache() {
        let (device, queue) = device();
        let mut c = config_for(menger_flame());
        c.escape.formula = "ifs_flame_3d".to_string();
        c.escape.coloring = "ifs_address".to_string();
        c.escape.cam_yaw = 0.9;
        c.escape.coloring_params.insert("reach".to_string(), 0.0);
        c.solid_shading.shading_strength = 1.0;
        c.solid_shading.lights[0].enabled = true;
        let mut engines = crate::renderer::RenderEngines::default();
        let mut go = |c: &crate::config::FractalConfig, engines: &mut crate::renderer::RenderEngines| {
            let job = crate::renderer::RenderJob::new(c, 512, 512).with_engines(engines);
            let t0 = web_time::Instant::now();
            let _ = pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render");
            (t0.elapsed().as_secs_f64() * 1000.0, crate::escape::diag::snapshot().path)
        };
        let _ = go(&c, &mut engines);
        let (walk_ms, walk_path) = go(&c, &mut engines);
        c.solid_shading.lights[0].intensity = 1.5;
        let (relight_ms, relight_path) = go(&c, &mut engines);
        c.solid_shading.lights[0].azimuth = 80.0;
        let (rewalk_ms, rewalk_path) = go(&c, &mut engines);
        println!("  a repeat render      {walk_ms:>8.1} ms  ({walk_path})");
        println!("  light intensity      {relight_ms:>8.1} ms  ({relight_path})");
        println!("  light direction      {rewalk_ms:>8.1} ms  ({rewalk_path})");
    }

    /// The interaction preview: a quarter of the walks, and no trace
    /// of it once the full render lands.
    ///
    /// Three claims. A preview is materially cheaper than a full
    /// render of the same frame -- measured, because "stride 2" could
    /// dispatch a quarter of the threads and still be paid for
    /// somewhere else. A preview's pixels come from the block's one
    /// walk, so the picture it draws is the full picture at a coarser
    /// grid rather than a different picture. And a full render that
    /// FOLLOWS a preview on the same renderer is byte-identical to one
    /// that never previewed: the preview's records must not
    /// masquerade as a full pass's, which is what putting the stride
    /// in both keys is for.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_preview_is_a_quarter_of_the_walks_and_leaves_no_trace() {
        let (device, queue) = device();
        // The control: a mode-A render through the same harness, which
        // is the fixed cost of device, palette, tonemap and readback.
        // Without it the ratio below compares floors, not walks -- at
        // beam 4 the planar walk is a few milliseconds over a
        // thirty-odd millisecond floor, and the first version of this
        // test failed on exactly that.
        let floor_ms = {
            let mut c = config_for(dragon_flame());
            c.escape.formula = "mandelbrot".to_string();
            c.escape.coloring = "smooth".to_string();
            let once = || {
                let job = crate::renderer::RenderJob::new(&c, 384, 384);
                let _ = pollster::block_on(crate::renderer::render(
                    &device,
                    &queue,
                    job,
                    &mut crate::renderer::NoProgress,
                ))
                .expect("render");
            };
            once();
            let t0 = web_time::Instant::now();
            once();
            t0.elapsed().as_secs_f64() * 1000.0
        };
        for (name, flame, formula) in [
            ("solid", menger_flame(), "ifs_flame_3d"),
            ("planar", gpu_tests_sierpinski(), "ifs_flame"),
        ] {
            let shot = |engines: &mut crate::renderer::RenderEngines, preview: bool| -> (Vec<u8>, f64) {
                let mut c = config_for(flame.clone());
                c.escape.formula = formula.to_string();
                c.escape.coloring = "ifs_address".to_string();
                c.escape.cam_yaw = 0.9;
                c.escape.coloring_params.insert("reach".to_string(), 0.0);
                c.solid_shading.shading_strength = 1.0;
                c.solid_shading.lights[0].enabled = true;
                if let Some(e) = engines.escape.as_mut() {
                    e.set_preview(preview);
                }
                let job = crate::renderer::RenderJob::new(&c, 384, 384).with_engines(engines);
                let t0 = web_time::Instant::now();
                let px = pollster::block_on(crate::renderer::render(
                    &device,
                    &queue,
                    job,
                    &mut crate::renderer::NoProgress,
                ))
                .expect("render")
                .rgba_data;
                (px, t0.elapsed().as_secs_f64() * 1000.0)
            };

            // Each timed render must be a WALK, not a cache hit: a
            // repeat of the same frame on a warm renderer takes the
            // relight path in a few milliseconds and would make any
            // preview look slow. So the timed renders alternate --
            // preview, full, preview, full -- and each is a miss on
            // the other's records. The first pair also pays the
            // pipeline compiles and is discarded.
            let mut engines = crate::renderer::RenderEngines::default();
            let (_, _) = shot(&mut engines, true);
            let (_, _) = shot(&mut engines, false);
            let (fast, fast_ms) = shot(&mut engines, true);
            let (full, full_ms) = shot(&mut engines, false);
            let (_, fast2_ms) = shot(&mut engines, true);
            let (again, _) = shot(&mut engines, false);
            let fast_ms = fast_ms.min(fast2_ms);
            let full_walk = full_ms - floor_ms;
            let fast_walk = fast_ms - floor_ms;
            println!(
                "  {name:<7} full {full_ms:>7.1} ms   preview {fast_ms:>7.1} ms   \
                 (floor {floor_ms:.1}; walks {full_walk:.1} and {fast_walk:.1})"
            );

            // Cheaper -- of the WALK, with the floor taken out. A
            // preview that cost as much as a full render would mean
            // the stride reached the dispatch and nothing else. When
            // the walk itself is within the noise of the floor there
            // is nothing to measure, and the two picture assertions
            // below still hold it to account.
            if full_walk > 10.0 {
                assert!(
                    fast_walk * 2.0 < full_walk,
                    "{name}: the preview's walk ({fast_walk:.1} ms) is not materially cheaper \
                     than the full one ({full_walk:.1} ms)"
                );
            }

            // Blocky, not different: sample the preview's 2x2 blocks
            // against the full render's mean over the same block. Most
            // blocks must agree closely -- edges legitimately differ.
            let lum = |p: &[u8], x: usize, y: usize| {
                let i = (y * 384 + x) * 4;
                p[i] as f64 + p[i + 1] as f64 + p[i + 2] as f64
            };
            let mut close = 0usize;
            let mut total = 0usize;
            for by in (0..384).step_by(2) {
                for bx in (0..384).step_by(2) {
                    let mean_full = (lum(&full, bx, by) + lum(&full, bx + 1, by)
                        + lum(&full, bx, by + 1) + lum(&full, bx + 1, by + 1)) / 4.0;
                    let mean_fast = (lum(&fast, bx, by) + lum(&fast, bx + 1, by)
                        + lum(&fast, bx, by + 1) + lum(&fast, bx + 1, by + 1)) / 4.0;
                    if mean_full > 24.0 || mean_fast > 24.0 {
                        total += 1;
                        if (mean_full - mean_fast).abs() < 60.0 {
                            close += 1;
                        }
                    }
                }
            }
            assert!(
                close * 10 > total * 8,
                "{name}: only {close} of {total} lit blocks agree between the preview and \
                 the full render -- the preview is drawing something else"
            );

            // No trace.
            let differing = again.iter().zip(&full).filter(|(a, b)| a != b).count();
            assert_eq!(
                differing, 0,
                "{name}: a full render after a preview differs from one that never previewed \
                 in {differing} bytes -- the preview's records leaked into the full pass"
            );
        }
    }

    /// What the rig costs, per light.
    ///
    /// The whole price of this design is one shadow march per LIGHT
    /// per lit pixel, so the number that matters is how the render
    /// scales with the count -- and whether turning shadows off buys
    /// it all back, which is what tells a user which knob to reach
    /// for.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn what_a_solid_render_pays_per_light() {
        let (device, queue) = device();
        for (name, flame) in [("tetrahedron", tetrahedron_flame()), ("menger", menger_flame())] {
            for shadow in [1.0f32, 0.0] {
                for lights in 1..=4usize {
                    let mut c = config_for(flame.clone());
                    c.escape.formula = "ifs_flame_3d".to_string();
                    c.escape.coloring = "ifs_address".to_string();
                    c.escape.cam_yaw = 0.9;
                    c.escape.coloring_params.insert("reach".to_string(), 0.0);
                    c.escape.formula_params.insert("shadow".to_string(), shadow);
                    c.solid_shading.shading_strength = 1.0;
                    for i in 0..lights {
                        c.solid_shading.lights[i].enabled = true;
                        c.solid_shading.lights[i].azimuth = 35.0 + 80.0 * i as f32;
                        c.solid_shading.lights[i].elevation = 40.0 - 10.0 * i as f32;
                        c.solid_shading.lights[i].intensity = 1.0 / lights as f32;
                    }
                    let once = || {
                        let job = crate::renderer::RenderJob::new(&c, 256, 256);
                        pollster::block_on(crate::renderer::render(
                            &device,
                            &queue,
                            job,
                            &mut crate::renderer::NoProgress,
                        ))
                        .expect("render")
                        .rgba_data
                    };
                    let _ = once();
                    let t0 = web_time::Instant::now();
                    let _ = once();
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    println!(
                        "  {name:<12} shadow {shadow:.0}  {lights} light(s): {ms:>8.1} ms"
                    );
                }
            }
        }
    }

    /// What each beam width gives up on the overlapping sets, so the
    /// planar default can be chosen on a measurement.
    ///
    /// The shipped planar default is 8 and it costs three and a half
    /// times beam 1 (measured: 128 ms against 37 at 512²). For a
    /// tiling set the beam changes nothing; it exists for the sets
    /// whose pieces OVERLAP, where one address reads too far. This
    /// renders the three D9 overlap fixtures and the dragon at every
    /// width and reports how many pixels differ from the widest, and
    /// by how much -- the number the default should be chosen on.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn what_each_planar_beam_width_gives_up() {
        use std::collections::HashMap;
        let xform = |a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, colour: f32| {
            let mut t = Transform::default();
            t.a = a;
            t.b = b;
            t.c = c;
            t.d = d;
            t.e = e;
            t.f = f;
            t.color = colour;
            t.variations = HashMap::from([("linear".to_string(), 1.0)]);
            t.variation_order = vec!["linear".to_string()];
            t
        };
        let flame_of = |ts: Vec<Transform>| {
            let mut fl = Flame::default();
            fl.transforms = ts;
            fl.final_transforms.clear();
            fl.xaos = None;
            fl
        };
        let cases: Vec<(&str, Flame)> = vec![
            ("fat-gasket", flame_of(vec![
                xform(0.6, 0.0, 0.0, 0.6, 0.0, 0.0, 0.1),
                xform(0.6, 0.0, 0.0, 0.6, 0.4, 0.0, 0.5),
                xform(0.6, 0.0, 0.0, 0.6, 0.2, 0.4, 0.9),
            ])),
            ("overlapping-band", flame_of(vec![
                xform(0.7, 0.0, 0.0, 0.7, 0.0, 0.0, 0.2),
                xform(0.7, 0.0, 0.0, 0.7, 0.3, 0.3, 0.8),
            ])),
            ("turned-pair", flame_of(vec![
                xform(0.46, -0.46, 0.46, 0.46, 0.0, 0.0, 0.2),
                xform(0.46, 0.46, -0.46, 0.46, 0.5, 0.1, 0.8),
            ])),
            ("dragon", dragon_flame()),
            ("gasket (tiles)", sierpinski_flame()),
        ];
        let (device, queue) = device();
        println!("  set               beam   ms     px differing from beam 8   >24 lum");
        for (name, flame) in cases {
            let mut widest: Option<Vec<u8>> = None;
            for beam in [8u32, 4, 2, 1] {
                let mut c = config_for(flame.clone());
                c.escape.coloring = "ifs_distance".to_string();
                c.escape.formula_params.insert("beam".to_string(), beam as f32);
                let once = || {
                    let job = crate::renderer::RenderJob::new(&c, 384, 384);
                    pollster::block_on(crate::renderer::render(
                        &device,
                        &queue,
                        job,
                        &mut crate::renderer::NoProgress,
                    ))
                    .expect("render")
                    .rgba_data
                };
                let _ = once();
                let t0 = web_time::Instant::now();
                let px = once();
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                let lum = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;
                let (differ, big) = match &widest {
                    Some(w) => {
                        let d: Vec<i32> = w
                            .chunks(4)
                            .zip(px.chunks(4))
                            .map(|(a, b)| (lum(a) - lum(b)).abs())
                            .collect();
                        (d.iter().filter(|&&x| x > 0).count(), d.iter().filter(|&&x| x > 24).count())
                    }
                    None => (0, 0),
                };
                println!("  {name:<16} {beam:>3}  {ms:>6.1}   {differ:>8}                 {big:>6}");
                if widest.is_none() {
                    widest = Some(px);
                }
            }
        }
    }

    /// What the second march costs.
    #[test]
    #[ignore = "needs a GPU; prints a measurement"]
    fn what_a_solid_render_pays_for_its_shadows() {
        let (device, queue) = device();
        for (name, flame) in [("tetrahedron", tetrahedron_flame()), ("menger", menger_flame())] {
            for shadow in [0.0f32, 1.0] {
                let mut c = config_for(flame.clone());
                c.escape.formula = "ifs_flame_3d".to_string();
                c.escape.coloring = "ifs_address".to_string();
                c.escape.cam_yaw = 0.9;
                c.escape.coloring_params.insert("reach".to_string(), 0.0);
                c.escape.formula_params.insert("shadow".to_string(), shadow);
                let once = || {
                    let job = crate::renderer::RenderJob::new(&c, 256, 256);
                    pollster::block_on(crate::renderer::render(
                        &device,
                        &queue,
                        job,
                        &mut crate::renderer::NoProgress,
                    ))
                    .expect("render")
                    .rgba_data
                };
                let _ = once();
                let t0 = web_time::Instant::now();
                let _ = once();
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                println!("  {name:<12} shadow {shadow:.0}: {ms:>8.1} ms");
            }
        }
    }

    /// Turning the camera must turn the picture, and zooming must
    /// fill more of the frame.
    ///
    /// The marcher reads a basis, not a policy, so nothing downstream
    /// would notice a camera that silently ignored its angles — the
    /// render would simply always look the same, which reads as a
    /// fixed viewpoint rather than a broken one.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_camera_steers_the_solid_render() {
        let (device, queue) = device();
        let shot = |yaw: f32, pitch: f32, zoom: f64| -> Vec<u8> {
            let mut c = config_for(tetrahedron_flame());
            c.escape.formula = "ifs_flame_3d".to_string();
            c.escape.coloring = "ifs_address".to_string();
            c.escape.cam_yaw = yaw;
            c.escape.cam_pitch = pitch;
            c.escape.zoom_log2 = zoom;
            c.escape.formula_params.insert("levels".to_string(), 16.0);
            let job = crate::renderer::RenderJob::new(&c, 128, 128);
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let lit = |px: &[u8]| {
            px.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24).count()
        };

        let base = shot(0.0, 0.42, 0.0);
        assert!(lit(&base) > 128 * 128 / 20, "nothing rendered: {}", lit(&base));

        let turned = shot(1.1, 0.42, 0.0);
        let differing = base.iter().zip(&turned).filter(|(a, b)| a != b).count();
        assert!(
            differing > base.len() / 10,
            "yaw changed only {differing} of {} bytes -- the camera is not steering",
            base.len()
        );

        let tilted = shot(0.0, -0.9, 0.0);
        let differing = base.iter().zip(&tilted).filter(|(a, b)| a != b).count();
        assert!(differing > base.len() / 10, "pitch does not steer");

        // Closer fills more of the frame.
        let near = shot(0.0, 0.42, 1.0);
        assert!(
            lit(&near) > lit(&base),
            "zooming in lit {} against {} -- the eye is not approaching",
            lit(&near),
            lit(&base)
        );
    }

    /// D9's pictures: the three overlapping IFSs, greedy against beam,
    /// distance and level.
    ///
    /// The plan says this is decided by looking, so this is the
    /// looking. Writes to the gitignored `output/ifs/`.
    #[test]
    #[ignore = "needs a GPU; writes images for inspection"]
    fn render_the_overlapping_ifss_for_d9() {
        use std::collections::HashMap;
        let xform = |a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, colour: f32| {
            let mut t = Transform::default();
            t.a = a;
            t.b = b;
            t.c = c;
            t.d = d;
            t.e = e;
            t.f = f;
            t.color = colour;
            t.variations = HashMap::from([("linear".to_string(), 1.0)]);
            t.variation_order = vec!["linear".to_string()];
            t
        };
        let flame_of = |ts: Vec<Transform>| {
            let mut fl = Flame::default();
            fl.transforms = ts;
            fl.final_transforms.clear();
            fl.xaos = None;
            fl
        };

        // The same three the CPU measurement uses.
        let cases: Vec<(&str, Flame)> = vec![
            (
                "fat-gasket",
                flame_of(vec![
                    xform(0.6, 0.0, 0.0, 0.6, 0.0, 0.0, 0.1),
                    xform(0.6, 0.0, 0.0, 0.6, 0.4, 0.0, 0.5),
                    xform(0.6, 0.0, 0.0, 0.6, 0.2, 0.4, 0.9),
                ]),
            ),
            (
                "overlapping-band",
                flame_of(vec![
                    xform(0.7, 0.0, 0.0, 0.7, 0.0, 0.0, 0.2),
                    xform(0.7, 0.0, 0.0, 0.7, 0.3, 0.3, 0.8),
                ]),
            ),
            (
                "turned-pair",
                flame_of(vec![
                    xform(0.46, -0.46, 0.46, 0.46, 0.0, 0.0, 0.2),
                    xform(0.46, 0.46, -0.46, 0.46, 0.5, 0.1, 0.8),
                ]),
            ),
        ];

        let guard = global_registry();
        for (name, flame) in cases {
            let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &guard)
                .unwrap_or_else(|why| panic!("{name} must qualify: {why:?}"));
            for beam in [1u32, 8] {
                for coloring in ["ifs_distance", "ifs_level"] {
                    let mut c = config_for(flame.clone());
                    c.escape.coloring = coloring.to_string();
                    c.escape.center_re = format!("{:.17}", ifs.ball.centre[0]);
                    c.escape.center_im = format!("{:.17}", ifs.ball.centre[1]);
                    c.escape.zoom_log2 = (4.0 / (ifs.ball.radius * 2.4)).log2();
                    c.escape.formula_params.insert("levels".to_string(), 40.0);
                    c.escape.formula_params.insert("beam".to_string(), beam as f32);
                    c.escape.coloring_params.insert("interior".to_string(), 0.5);

                    let (device, queue) = device();
                    let job = crate::renderer::RenderJob::new(&c, 384, 384);
                    let out = pollster::block_on(crate::renderer::render(
                        &device,
                        &queue,
                        job,
                        &mut crate::renderer::NoProgress,
                    ))
                    .expect("render");
                    let dir = std::path::Path::new("output/ifs");
                    std::fs::create_dir_all(dir).expect("output dir");
                    let path = dir.join(format!("d9-{name}-beam{beam}-{coloring}.png"));
                    image::save_buffer(&path, &out.rgba_data, 384, 384, image::ColorType::Rgba8)
                        .expect("write png");
                    let lit = out
                        .rgba_data
                        .chunks(4)
                        .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                        .count();
                    println!("  {} ({lit} lit)", path.display());
                }
            }
        }
    }

    /// Render the classical affine IFSs for inspection (plan phase 1:
    /// "the classical presets render to their textbook pictures,
    /// inspected and baselined"). Writes to the gitignored `output/`.
    #[test]
    #[ignore = "needs a GPU; writes images for inspection"]
    fn render_the_classical_ifss_for_inspection() {
        let dir = std::path::Path::new("output/ifs");
        std::fs::create_dir_all(dir).expect("output dir");

        let cases: Vec<(&str, Flame, [f64; 2], f64)> = vec![
            ("sierpinski", sierpinski_flame(), [0.5, 0.4], 1.0),
            ("square", square_flame(), [0.5, 0.5], 1.2),
            ("dragon", dragon_flame(), [0.35, 0.25], 1.6),
            ("koch", koch_flame(), [0.5, 0.12], 2.0),
        ];
        let colorings = ["ifs_distance", "ifs_level", "ifs_address", "ifs_trap"];

        for (name, flame, centre, zoom) in cases {
            for coloring in colorings {
                let mut c = config_for(flame.clone());
                c.escape.coloring = coloring.to_string();
                c.escape.center_re = centre[0].to_string();
                c.escape.center_im = centre[1].to_string();
                c.escape.zoom_log2 = zoom;
                c.escape.supersample = 2;
                c.escape.formula_params.insert("levels".to_string(), 40.0);
                let (device, queue) = device();
                let job = crate::renderer::RenderJob::new(&c, 512, 512);
                let out = pollster::block_on(crate::renderer::render(
                    &device,
                    &queue,
                    job,
                    &mut crate::renderer::NoProgress,
                ))
                .expect("render");
                let path = dir.join(format!("{name}-{coloring}.png"));
                image::save_buffer(
                    &path,
                    &out.rgba_data,
                    512,
                    512,
                    image::ColorType::Rgba8,
                )
                .expect("write png");
                println!("wrote {}", path.display());
            }
        }
    }
}
