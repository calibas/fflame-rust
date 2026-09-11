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
use crate::scene::ifs_analysis::{Affine2, Ifs2, Ifs3};

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

/// The loaded flame's attractor, by Hart's inverse iteration.
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
            default: 8.0,
            min: 1.0,
            max: 8.0,
            tooltip: "How many branch addresses the walk follows at once. Every \
                      address bounds the distance to ITS piece and the truth is the \
                      smallest, so following one can only read too far — which \
                      erodes an attractor whose pieces share a boundary. 1 is the \
                      greedy walk. Measured on the Heighway dragon, whose two \
                      pieces share a boundary: 1 renders less than half its area, \
                      4 leaves a scatter of holes, 8 is exact — for 68% more time \
                      than 1. An IFS with disjoint pieces (a Sierpiński, a Koch) \
                      is already exact at 1, so lower it if the picture does not \
                      change.",
            choices: &[],
        },
    ],
    wgsl: r#"
// Inverse of map i.
fn ifs_inv_point(i: u32, p: vec2<f32>) -> vec2<f32> {
    let m = ifs_maps[i].inv_m;
    let t = ifs_maps[i].inv_t;
    return vec2<f32>(
        m.x * p.x + m.y * p.y + t.x,
        m.z * p.x + m.w * p.y + t.y,
    );
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

    // Seed from the reference orbit the CPU walked. Every one of these
    // candidates is where the view centre's own beam had got to, and
    // `basis` carries this pixel's offset the same distance.
    var live: array<IfsCand, IFS_MAX_BEAM>;
    var next: array<IfsCand, IFS_MAX_BEAM>;
    var live_count = min(ifs_seed_count(), beam);
    if (live_count == 0u) {
        live_count = 1u;
    }
    for (var j = 0u; j < live_count; j = j + 1u) {
        let a = ifs_seed(j, 0u);
        let b = ifs_seed(j, 1u);
        let d = ifs_seed(j, 2u);
        let e = ifs_seed(j, 3u);
        var cand: IfsCand;
        // position + basis * uv, the basis already composed with the
        // view so this one multiply is the whole delta.
        cand.q = vec2<f32>(
            a.x + a.z * uv.x + a.w * uv.y,
            a.y + b.x * uv.x + b.y * uv.y,
        );
        cand.sigma = b.z;
        cand.bound = b.w;
        cand.r = length(cand.q - c);
        cand.addr = d.x;
        cand.last_sigma = d.y;
        cand.level = d.z;
        cand.flags = bitcast<u32>(d.w);
        cand.point = vec2<f32>(e.x, e.y);
        cand.color = e.z;
        live[j] = cand;
    }

    for (var k = 0u; k < max_levels; k = k + 1u) {
        var all_done = true;
        for (var ci = 0u; ci < live_count; ci = ci + 1u) {
            if ((live[ci].flags & 2u) != 0u) {
                continue;
            }
            let r = length(live[ci].q - c);
            live[ci].r = r;
            live[ci].bound = max(live[ci].bound, live[ci].sigma * (r - radius));

            if (r > radius && (live[ci].flags & 1u) == 0u) {
                live[ci].flags = live[ci].flags | 1u;
                live[ci].level =
                    f32(handover + k) + ifs_residual(r, radius, live[ci].last_sigma);
                live[ci].point = live[ci].q;
            }

            if (!(r < far)) {
                live[ci].flags = live[ci].flags | 2u;
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
                first = n;
                last = n + 1u;
            }
            for (var bi = first; bi < last; bi = bi + 1u) {
                var cand_key = live[ci].r;
                if (bi < n) {
                    cand_key = length(ifs_inv_point(bi, live[ci].q) - c);
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
                child.q = ifs_inv_point(bi, live[parent].q);
                child.sigma = live[parent].sigma * ifs_maps[bi].sigma_min;
                child.last_sigma = ifs_maps[bi].sigma_min;
                child.r = key[k2];
                // Score the child as it is made: one that inherited
                // only its parent's bound would rank identically to
                // all its siblings.
                child.bound = max(live[parent].bound, child.sigma * (child.r - radius));
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
    var win = 0u;
    for (var ci = 1u; ci < live_count; ci = ci + 1u) {
        if (live[ci].bound < live[win].bound) {
            win = ci;
        }
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

    res.distance = max(best.bound, 0.0);
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
            default: 8.0,
            min: 1.0,
            max: 8.0,
            tooltip: "How many branch addresses the walk follows at once. Every address \
                      bounds the distance to ITS piece and the truth is the smallest, so \
                      following one can only read too far -- which a marcher turns into \
                      a surface that is not there.",
            choices: &[],
        },
        EscapeParamDef {
            name: "steps",
            display_name: "March Steps",
            default: 96.0,
            min: 4.0,
            max: 512.0,
            tooltip: "How many times a ray may step before giving up. A ray that runs \
                      out is left as a miss, and the count also drives the ambient \
                      occlusion -- a ray that needed many small steps was squeezing \
                      through structure.",
            choices: &[],
        },
    ],
    wgsl: r#"
// Inverse of map i, in three dimensions.
fn ifs_inv_point3(i: u32, p: vec3<f32>) -> vec3<f32> {
    let m = ifs_maps[i];
    return vec3<f32>(
        dot(m.r0.xyz, p) + m.r0.w,
        dot(m.r1.xyz, p) + m.r1.w,
        dot(m.r2.xyz, p) + m.r2.w,
    );
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
}

// The walk, in three dimensions. The same algorithm the planar one
// runs -- a beam of addresses, ranked by distance to the ball's
// centre, answered by the smallest bound -- and the same reasons for
// each part. What differs is the arithmetic and that the distance is
// in WORLD units here rather than pixels: a marcher steps by it, so
// it has to be a length in the space the ray is crossing.
fn ifs_walk3(p0: vec3<f32>) -> IfsResult {
    let c = ifs_ball_centre();
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
        res.distance = 1e30;
        res.escaped = 1u;
        return res;
    }

    let max_levels = u32(clamp(fparam(0u), 1.0, 256.0));
    let beam = u32(clamp(fparam(1u), 1.0, f32(IFS_MAX_BEAM)));
    let far = max(radius, 1.0) * 1e12;

    var live: array<IfsCand3, IFS_MAX_BEAM>;
    var next: array<IfsCand3, IFS_MAX_BEAM>;
    var live_count = 1u;
    var root: IfsCand3;
    root.q = p0;
    root.point = p0;
    root.sigma = 1.0;
    root.bound = -1e30;
    root.r = length(p0 - c);
    root.addr = 0.0;
    root.level = 0.0;
    root.color = 0.0;
    root.last_sigma = ifs_mean_sigma();
    root.flags = 0u;
    live[0] = root;

    var addr_scale = 1.0 / f32(n);
    var deepest = -1.0;

    for (var k = 0u; k < max_levels; k = k + 1u) {
        var all_done = true;
        for (var ci = 0u; ci < live_count; ci = ci + 1u) {
            if ((live[ci].flags & 2u) != 0u) {
                continue;
            }
            let r = length(live[ci].q - c);
            live[ci].r = r;
            live[ci].bound = max(live[ci].bound, live[ci].sigma * (r - radius));
            if (r > radius && (live[ci].flags & 1u) == 0u) {
                live[ci].flags = live[ci].flags | 1u;
                live[ci].level = f32(k) + ifs_residual(r, radius, live[ci].last_sigma);
                live[ci].point = live[ci].q;
            }
            if (!(r < far)) {
                live[ci].flags = live[ci].flags | 2u;
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
                first = n;
                last = n + 1u;
            }
            for (var bi = first; bi < last; bi = bi + 1u) {
                var cand_key = live[ci].r;
                if (bi < n) {
                    cand_key = length(ifs_inv_point3(bi, live[ci].q) - c);
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
                child.q = ifs_inv_point3(bi, live[parent].q);
                child.sigma = live[parent].sigma * ifs_maps[bi].extra.x;
                child.last_sigma = ifs_maps[bi].extra.x;
                child.r = key[k2];
                child.bound = max(live[parent].bound, child.sigma * (child.r - radius));
                if ((live[parent].flags & 1u) == 0u) {
                    child.addr = live[parent].addr + f32(bi) * addr_scale;
                    if (k == 0u) {
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

    var win = 0u;
    for (var ci = 1u; ci < live_count; ci = ci + 1u) {
        if (live[ci].bound < live[win].bound) {
            win = ci;
        }
    }
    let unescaped = f32(max_levels);
    for (var ci = 0u; ci < live_count; ci = ci + 1u) {
        var lvl = unescaped;
        if ((live[ci].flags & 1u) != 0u) {
            lvl = live[ci].level;
        }
        deepest = max(deepest, lvl);
    }
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
fn ifs_distance_at(p: vec3<f32>) -> f32 {
    return ifs_walk3(p).distance;
}

// What the marcher colours with, at the point it stopped.
fn ifs_evaluate3(p: vec3<f32>) -> IfsResult {
    return ifs_walk3(p);
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

/// `level`: Hepting–Hart escape-time bands — the Fractint
/// "escape-time Sierpiński" look, from the same walk (D9).
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
pub static IFS_DEFS: &[&IfsDef] = &[&IFS_FLAME, &IFS_FLAME_3D];

/// Ordered mode-D coloring registry. **Append-only.**
pub static IFS_COLORINGS: &[&IfsColoringDef] =
    &[&IFS_DISTANCE, &IFS_LEVEL, &IFS_ADDRESS, &IFS_TRAP];

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
    out[2] = [0.0; 4];
    out[3] = [0.0; 4];
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
    /// `i`-th component in `w`.
    ///
    /// Four `vec4`s and no `vec3` anywhere, which is deliberate. A
    /// `vec3<f32>` aligns to SIXTEEN bytes in WGSL and to four in
    /// Rust, so the obvious struct — three padded rows, a `vec3`
    /// translation, two scalars — is eighty bytes on one side and
    /// ninety-six on the other. The shader then reads each map from
    /// the wrong offset and the render comes out as noise that still
    /// looks vaguely like something, which is how this was found.
    pub rows: [[f32; 4]; 3],
    /// `σ_min` of the forward map, the transform's colour, and the
    /// padding that makes the stride sixty-four.
    pub extra: [f32; 4],
}

/// The 3D rows, in the flame's transform order.
pub fn pack_maps3(ifs: &Ifs3, colors: &[f32]) -> Vec<IfsMap3Gpu> {
    ifs.maps
        .iter()
        .map(|m| {
            let inv = m.inverse;
            let row = |i: usize| {
                [
                    inv.m[i][0] as f32,
                    inv.m[i][1] as f32,
                    inv.m[i][2] as f32,
                    inv.t[i] as f32,
                ]
            };
            IfsMap3Gpu {
                rows: [row(0), row(1), row(2)],
                extra: [
                    m.sigma_min as f32,
                    colors.get(m.transform_index).copied().unwrap_or(0.0),
                    0.0,
                    0.0,
                ],
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
) -> Result<PackedIfs, Vec<crate::scene::ifs_analysis::Disqualification>> {
    let ifs = crate::scene::ifs_analysis::analyse_2d(flame, registry)?;
    let colors: Vec<f32> = flame.transforms.iter().map(|t| t.color).collect();
    let mut globals = [[0.0f32; 4]; 4];
    pack_globals(&ifs, &mut globals);
    let rows = pack_maps(&ifs, &colors);
    // The solid analysis is attempted and allowed to fail: a flame
    // that is a planar IFS need not be a solid one.
    let solid = crate::scene::ifs_analysis::analyse_3d(flame, registry)
        .ok()
        .map(|ifs3| {
            let rows3 = pack_maps3(&ifs3, &colors);
            (ifs3, rows3)
        });
    Ok(PackedIfs { globals, rows, ifs, colors, solid })
}

/// How many `vec4`s of the params' `fdata` block one seed occupies.
pub const SEED_VEC4S: usize = 4;
/// Where the seeds start in `fdata`; the whole-IFS constants are below.
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
/// 3. `escape point.xy`, `transform colour`, unused
///
/// `flags`: bit 0 escaped, bit 1 done.
pub fn pack_seeds(
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

    for (j, c) in seeds.cands.iter().take(count).enumerate() {
        let base = SEED_BASE + SEED_VEC4S * j;
        if base + SEED_VEC4S > out.len() {
            break;
        }
        let (esc_level, esc_point, escaped) = match c.escape {
            Some((lvl, p)) => (lvl as f32, [p[0] as f32, p[1] as f32], 1u32),
            None => (-1.0, [0.0, 0.0], 0u32),
        };
        let flags = escaped | if c.done { 2 } else { 0 };
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
        out[base + 3] = [esc_point[0], esc_point[1], colour, 0.0];
    }
}

/// A seeding position at arbitrary precision.
///
/// The impl lives here rather than beside the trait because `BigFloat`
/// is the escape engine's, and `scene` compiles without the escape
/// engine. Only the POSITION is big: the map coefficients stay f64,
/// and so does the distance the walk compares against the ball, which
/// is O(1) however precise the point is.
impl crate::scene::ifs_estimate::SeedPoint for [super::bigfloat::BigFloat; 2] {
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

/// The view basis: takes a pixel's normalised offset — the screen
/// spanning [-½, ½] on each axis — to a world offset from the centre.
///
/// The `-span_y` is the template's `d.y = -d.y`: screen y runs down.
pub fn view_basis(span_x: f64, span_y: f64, rotation: f32) -> [[f64; 2]; 2] {
    let (c, s) = ((rotation as f64).cos(), (rotation as f64).sin());
    // rotate(diag(span_x, -span_y))
    [[c * span_x, s * span_y], [s * span_x, -c * span_y]]
}

/// The whole-IFS constants and the camera, for a solid render.
///
/// Layout, one `vec4` each:
/// 0. `ball centre xyz, radius`
/// 1. `mean_sigma_min, map_count, 0, 0`
/// 2. `eye xyz, field of view (radians)`
/// 3. `forward xyz, 0`
/// 4. `right xyz, 0`
/// 5. `up xyz, 0`
///
/// The camera is here rather than shared with the flame's because the
/// flame's fields are `f32` — the same wall the planar view centre hit
/// at 2²² (D8). A camera of its own can carry its position the way the
/// escape view carries its centre.
pub fn pack_globals3(ifs: &Ifs3, eye: [f64; 3], fov: f32, out: &mut [[f32; 4]]) {
    if out.len() < 6 {
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
    out[1] = [mean as f32, ifs.maps.len() as f32, 0.0, 0.0];

    // Look at the attractor. A fixed frame for now: phase 3's camera
    // controls replace this, and the marcher does not care which built
    // the basis.
    let to = ifs.ball.centre;
    let fwd = normalize3([to[0] - eye[0], to[1] - eye[1], to[2] - eye[2]]);
    // Any up that is not parallel to the view; z is the odd axis out
    // in a flame, so prefer it and fall back when looking along it.
    let world_up = if fwd[2].abs() > 0.9 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
    let right = normalize3(cross3(fwd, world_up));
    let up = cross3(right, fwd);

    out[2] = [eye[0] as f32, eye[1] as f32, eye[2] as f32, fov];
    out[3] = [fwd[0] as f32, fwd[1] as f32, fwd[2] as f32, 0.0];
    out[4] = [right[0] as f32, right[1] as f32, right[2] as f32, 0.0];
    out[5] = [up[0] as f32, up[1] as f32, up[2] as f32, 0.0];
}

fn normalize3(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n > 0.0 {
        [v[0] / n, v[1] / n, v[2] / n]
    } else {
        [0.0, 0.0, 1.0]
    }
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Where to stand for a solid render, until D8's camera config lands.
///
/// The frame is derived from the attractor's own bounding ball, so any
/// qualifying flame is in view without being told where it is; the
/// escape view's `zoom_log2` moves the eye in and `rotation` orbits
/// it, so the two controls that already exist do something sensible.
/// A real camera replaces this and the marcher will not notice — it
/// reads a basis, not a policy.
pub fn preview_camera(ifs: &Ifs3, zoom_log2: f64, rotation: f32) -> ([f64; 3], f32) {
    let r = ifs.ball.radius.max(1e-6);
    let dist = 3.2 * r / 2f64.powf(zoom_log2);
    let a = rotation as f64;
    // Slightly above the equator, so the silhouette is not symmetric
    // and the shading has something to do.
    let dir = normalize3([a.cos() * 0.86, a.sin() * 0.86, 0.42]);
    (
        [
            ifs.ball.centre[0] + dir[0] * dist,
            ifs.ball.centre[1] + dir[1] * dist,
            ifs.ball.centre[2] + dir[2] * dist,
        ],
        0.7,
    )
}

/// The per-map rows of the storage buffer, in the flame's transform
/// order — so a branch index in the address IS a transform index.
pub fn pack_maps(ifs: &Ifs2, colors: &[f32]) -> Vec<IfsMapGpu> {
    ifs.maps
        .iter()
        .map(|m| {
            let inv: Affine2 = m.inverse;
            IfsMapGpu {
                inv_m: [
                    inv.m[0][0] as f32,
                    inv.m[0][1] as f32,
                    inv.m[1][0] as f32,
                    inv.m[1][1] as f32,
                ],
                inv_t: [inv.t[0] as f32, inv.t[1] as f32],
                sigma_min: m.sigma_min as f32,
                color: colors.get(m.transform_index).copied().unwrap_or(0.0),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
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
        let mut checked = 0;
        for cfg in crate::resources::presets::load_embedded_presets().expect("presets parse") {
            if get_ifs(&cfg.escape.formula).is_none() {
                continue;
            }
            checked += 1;
            let name = cfg.flame.name.clone();
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
        assert_eq!(checked, 4, "expected the four classical IFS presets, found {checked}");
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
            globals: [[0.0; 4]; 4],
            rows: pack_maps(&ifs, &colors),
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
        assert_eq!(std::mem::size_of::<IfsMap3Gpu>(), 64);
        assert_eq!(std::mem::offset_of!(IfsMap3Gpu, rows), 0);
        assert_eq!(std::mem::offset_of!(IfsMap3Gpu, extra), 48);
        // And no vec3 in the shader's declaration either, which is the
        // half a size assertion cannot see.
        let src = crate::escape::assembler::assemble_ifs(&IFS_FLAME_3D, &IFS_ADDRESS);
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
        let rows = pack_maps(&ifs, &[0.1, 0.4, 0.7]);
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
        let rows = pack_maps(&ifs, &[0.1, 0.4, 0.7]);
        for r in &rows {
            assert!((r.sigma_min - 0.5).abs() < 1e-6, "sigma {r:?}");
        }
        assert_eq!(rows.iter().map(|r| r.color).collect::<Vec<_>>(), vec![0.1, 0.4, 0.7]);
        // A colour list shorter than the flame (a caller bug) must not
        // panic mid-render.
        let short = pack_maps(&ifs, &[0.1]);
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
        pack_seeds(&seeds, ifs.maps.len(), &[0.1, 0.4, 0.7], &mut out);

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

    /// The GPU row must match what WGSL's std430 rules read: 32 bytes,
    /// with the scalars trailing a vec2 rather than straddling a
    /// 16-byte boundary.
    #[test]
    fn the_gpu_row_is_the_layout_the_shader_declares() {
        assert_eq!(std::mem::size_of::<IfsMapGpu>(), 32);
        assert_eq!(std::mem::align_of::<IfsMapGpu>(), 4);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, inv_m), 0);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, inv_t), 16);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, sigma_min), 24);
        assert_eq!(std::mem::offset_of!(IfsMapGpu, color), 28);
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
    pub(super) const BEAM: u32 = 8;

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
    fn dragon_flame() -> Flame {
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
    fn spherical_flame() -> Flame {
        let mut fl = sierpinski_flame();
        fl.transforms[1].variations = HashMap::from([("spherical".to_string(), 1.0)]);
        fl.transforms[1].variation_order = vec!["spherical".to_string()];
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

    /// A flame that fails the criterion must render EMPTY, not a
    /// frame-filling interior. `distance == 0` is what a point on the
    /// attractor returns, so "no maps" has to mean far away, not near.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_flame_that_does_not_qualify_renders_nothing() {
        let guard = global_registry();
        assert!(
            pack_flame(&spherical_flame(), &guard).is_err(),
            "the fixture must actually fail the criterion"
        );
        drop(guard);

        let rgba = render(&config_for(spherical_flame()));
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
        let template = crate::escape::assembler::assemble_ifs(&IFS_FLAME, &IFS_TRAP);
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
                let source = crate::escape::assembler::assemble_ifs(def, coloring);
                let _ = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(&format!("{}|{}", def.name, coloring.name)),
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });
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
        let walk = crate::escape::assembler::assemble_ifs(&IFS_FLAME, &IFS_DISTANCE);
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
        assert_eq!(seen, 4, "expected the four classical IFS presets, found {seen}");
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
    fn menger_flame() -> Flame {
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
                c.escape.rotation = 0.9;
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
