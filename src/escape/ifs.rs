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
fn ifs_walk3(delta: vec3<f32>) -> IfsResult {
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

    var live: array<IfsCand3, IFS_MAX_BEAM>;
    var next: array<IfsCand3, IFS_MAX_BEAM>;
    var live_count = min(max(slots, 1u), beam);
    for (var b = 0u; b < live_count; b = b + 1u) {
        let L = ifs_links[link * slots + b];
        var root: IfsCand3;
        root.q = ifs_link_point(L, delta);
        root.sigma = L.r1.w;
        root.bound = L.r2.w;
        root.r = length(root.q - c);
        root.addr = L.extra.x;
        root.color = L.esc.w;
        root.last_sigma = L.extra.y;
        root.flags = bitcast<u32>(L.extra.w);
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
        live[b] = root;
    }

    var addr_scale = pow(1.0 / f32(n), base_level + 1.0);
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
                live[ci].level =
                    base_level + f32(k) + ifs_residual(r, radius, live[ci].last_sigma);
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

    var win = 0u;
    for (var ci = 1u; ci < live_count; ci = ci + 1u) {
        if (live[ci].bound < live[win].bound) {
            win = ci;
        }
    }
    let unescaped = base_level + f32(max_levels);
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
//
// The argument is an offset from the camera's TARGET, not a position.
// Every sample the marcher takes is `eye_rel + dir*t`, both terms of
// which shrink with the zoom, so the offset keeps its relative
// precision where an absolute position would have spent it all on
// leading digits that are the same for the whole frame.
fn ifs_distance_at(delta: vec3<f32>) -> f32 {
    return ifs_walk3(delta).distance;
}

// What the marcher colours with, at the offset it stopped at.
fn ifs_evaluate3(delta: vec3<f32>) -> IfsResult {
    return ifs_walk3(delta);
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

/// Short of the pole by this much, where an up vector does not exist.
const PITCH_LIMIT: f32 = 1.5533;

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

    let r = ifs.ball.radius.max(1e-12);
    let distance = FRAME_DISTANCE * r / 2f64.powf(escape.zoom_log2);

    let pitch = escape.cam_pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT) as f64;
    let yaw = escape.cam_yaw as f64;
    // The eye sits on the sphere of that radius about the target; the
    // view looks back down the same line.
    let dir = [
        pitch.cos() * yaw.cos(),
        pitch.cos() * yaw.sin(),
        pitch.sin(),
    ];
    let eye_rel = [dir[0] * distance, dir[1] * distance, dir[2] * distance];
    // The absolute eye is for callers that want a position; nothing on
    // the deep-zoom path reads it, and past 2⁴⁸ it IS the target.
    let eye = [
        target[0] + eye_rel[0],
        target[1] + eye_rel[1],
        target[2] + eye_rel[2],
    ];
    let forward = [-dir[0], -dir[1], -dir[2]];

    // World up is +z, which is the axis a flame treats as depth. The
    // pitch clamp is what keeps this from being parallel to the view.
    let right = normalize3(cross3(forward, [0.0, 0.0, 1.0]));
    let up = cross3(right, forward);

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
    out[1] = [mean as f32, ifs.maps.len() as f32, 0.0, 0.0];
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
    out[3] = [cam.forward[0] as f32, cam.forward[1] as f32, cam.forward[2] as f32, 0.0];
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

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
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

/// Pack a seed chain for the shader, padded to `beam` links a level.
///
/// Levels shallower than the beam is wide have fewer candidates than
/// the slot count, and the short ones are filled by REPEATING the last
/// candidate rather than by marking slots empty. A duplicate costs one
/// redundant walk and changes no answer — the result is a minimum over
/// candidates — where an empty-slot flag would put a branch in the
/// inner loop of every sample at every depth.
pub fn pack_chain3(
    chain: &crate::scene::ifs_estimate::SeedChain3,
    n_maps: usize,
    colors: &[f32],
    beam: usize,
) -> Vec<IfsLinkGpu> {
    let beam = beam.max(1);
    let levels = chain.levels.len().min(MAX_CHAIN_LINKS);
    let mut out = Vec::with_capacity(levels * beam);
    for cands in chain.levels.iter().take(levels) {
        for b in 0..beam {
            // Repeat the last rather than leave a hole.
            let c = &cands[b.min(cands.len() - 1)];
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
            let flags = escaped | if c.done { 2 } else { 0 };
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
        assert_eq!(planar, 4, "expected four planar IFS presets, found {planar}");
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
        // maps, depth 24, beam 1, 96 steps -- and shadows on, which is
        // a second march of the same length, so the caller passes
        // twice the step count.
        let rows = ifs_rows_per_dispatch(1920, 1080, 24, 2 * 96, 20, IFS_SOLID_BUDGET);
        assert!(rows < 1080, "1080p should band, got {rows} of 1080");
        assert!(rows > 50, "and not into slivers: {rows} rows");

        // Turning shadows off is half the work and must buy back the
        // band, or the second march is not in the estimate at all.
        let unshadowed = ifs_rows_per_dispatch(1920, 1080, 24, 1 * 96, 20, IFS_SOLID_BUDGET);
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
        let solid = crate::escape::assembler::assemble_ifs(&IFS_FLAME_3D, &IFS_DISTANCE);
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
        assert_eq!(seen, 6, "expected six IFS presets, found {seen}");
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
        // Not byte-identity, and the reason is arithmetic rather than
        // staleness: the fresh walk builds its lighting by summing a
        // term per light, while the recolour multiplies the new albedo
        // by the single factor the record carries. Both compute the
        // same number and they round differently in the last place.
        // What a STALE record looks like is nothing like that -- it is
        // whole regions of the previous view, so the bound here is on
        // both how many bytes may differ and by how much.
        let worst = recoloured
            .iter()
            .zip(&fresh_level)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        let differing = recoloured.iter().zip(&fresh_level).filter(|(a, b)| a != b).count();
        assert!(
            worst <= 1 && differing * 1000 < recoloured.len(),
            "the cached recolour of a solid differs from a fresh walk in {differing} of \
             {} bytes, worst by {worst} -- a last-place rounding difference is at most \
             1, so this is stale records rather than arithmetic",
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
            if zoom <= GATED_TO {
                worst_gated = worst_gated.min(pct);
            }
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
            println!(
                "  2^{zoom:<4} {levels:<7} {pct:>5.1}%   ({hits} hit / {} miss)",
                total - hits
            );
        }
        assert!(
            worst_gated >= 100.0,
            "a solid render disagrees with the reference at {worst_gated:.1}% somewhere \
             up to 2^{GATED_TO} -- if this DROPPED, the seed chain regressed; if you \
             raised the ceiling, raise GATED_TO and say so"
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
