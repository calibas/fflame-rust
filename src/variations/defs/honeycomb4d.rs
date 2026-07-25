//! `honeycomb4d` — hyperbolic H⁴ honeycomb reflection walk, Schläfli
//! {p,q,r,s} (original).
//!
//! The 4D sibling of [`honeycomb`](super::honeycomb): tessellations of
//! hyperbolic 4-space from four Schläfli sliders. The five compact
//! regular H⁴ honeycombs — {5,3,3,4}, {5,3,3,5}, {4,3,3,5}, {3,3,3,5},
//! {5,3,3,3} — plus the Euclidean {4,3,3,4} family and every other
//! [p,q,r,s] kaleidoscope. Five orthoscheme mirrors in R^{4,1} by the
//! same triangular Minkowski Gram–Schmidt (the linear-diagram Coxeter
//! matrix makes n3 lose its y component and n4 everything but the
//! last two slots, so the chain stays closed-form); reflections are
//! linear; the walk, Wythoff variants, seed modes, geodesic
//! thickness, and color modes all mirror the 3D variation.
//!
//! State: the point lives on the 4-ball; its 4th coordinate rides the
//! per-thread `point_w` register (`Feature::NeedsW`) and the xyz
//! shadow is plotted — the honest-4D-state pattern of `polychoron` /
//! `menger`. WGSL has no vec5 — and the shader builder
//! extracts only top-level `fn` blocks from variation sources, so no
//! struct declarations either: hyperboloid vectors are carried as
//! parallel `(vec4 spatial, f32 time)` variable pairs.
//!
//! Scope note: the triangular completion assumes the [p,q] and
//! [p,q,r] subgroups are spherical — true for every compact and the
//! common Euclidean {p,q,r,s}. Exotic combos (hyperbolic-cell 4D
//! families) degrade via clamps rather than branching into the full
//! case analysis the 3D variation carries; extend if ever needed.
//!
//! No JWildfire/Apophysis counterpart — original to this project.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Hyperbolic H⁴ honeycomb reflection walk (Schläfli {p,q,r,s}).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static HONEYCOMB4D: VariationDef = VariationDef {
    name: "honeycomb4d",
    aliases: &[],
    display_name: "Hyperbolic Honeycomb 4D",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsW, Feature::WritesColor, Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    // State slot 0: Mirror color register. Slot 1: walk depth (Steps).
    state_count: 2,
    wgsl_state_init: None,
    parameters: &[
        param!("p", "P", int, 5.0, 2.0, 12.0, "First Schläfli number. Compact hyperbolic H⁴ honeycombs: {5,3,3,4}, {5,3,3,5}, {4,3,3,5}, {3,3,3,5}, {5,3,3,3}; {4,3,3,4} is the Euclidean tesseractic honeycomb. Other combinations degrade to spherical/Euclidean/non-discrete kaleidoscopes."),
        param!("q", "Q", int, 3.0, 2.0, 12.0, "Second Schläfli number ({p,q,r} is the 4-polytope cell)."),
        param!("r", "R", int, 3.0, 2.0, 12.0, "Third Schläfli number (4D). In 2D render mode this is the third triangle angle — (p,q,r) becomes the general Fuchsian triangle group; r = 2 gives the {p,q} tiling."),
        param!("s", "S", int, 4.0, 2.0, 12.0, "Fourth Schläfli number: cells around each 2-face."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Radius of the 4-ball model in world units."),
        param!("steps", "Steps", int, 2.0, 1.0, 8.0, "Random mirror reflections per call (5 mirrors)."),
        param!("projection", "Projection", enum, 0, &["Poincaré", "Beltrami–Klein", "Half-Space"], "Model of H⁴ for the plotted (and fed-forward) 4-ball point; the xyz shadow is plotted and the 4th ball coordinate rides the per-thread w register. Half-Space uses the 4th coordinate as the height above the floor hyperplane."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Mirror", "Depth", "Steps"], "Direct-color source. Mirror: each of the 5 orthoscheme mirrors has its own palette position, blended through the persistent color register at Color Speed. Depth: output z. Steps: palette cycles with the walk depth since the last reseed (wraps, so deep levels stay distinct)."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "Mirror mode: pull strength toward each mirror's palette position. Steps mode: palette advance per reflection (cyclic — wraps instead of saturating)."),
        param!("seed", "Seed", enum, 2, &["Input", "Vertices", "Edges", "Faces"], "What the walk stamps through the honeycomb: the incoming flame measure, the vertex orbit, the edge skeleton, or flag-fragment faces (see honeycomb — same semantics, one dimension up)."),
        param!("thickness", "Thickness", float, 0.01, 0.0, 2.0, "Seed modes: geodesic tangent-space offset by exact hyperbolic distance — balls / tubes / slabs with uniform hyperbolic radius (see honeycomb)."),
        param!("variant", "Variant", enum, 0, &["Normal", "Rectified", "Truncated", "Bitruncated", "Cantellated", "Cantitruncated", "Runcitruncated", "Runcicantellated", "Omnitruncated"], "Wythoff variant: ring mask over the first four Coxeter nodes (the fifth stays unringed). Same construction as honeycomb, one dimension up."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_3D: &str = r#"
// 5-component Minkowski vectors for R^(4,1) are carried as parallel
// (vec4 spatial, f32 time) pairs: the shader builder extracts only
// top-level `fn` blocks from variation sources, so struct declarations
// would be dropped.
fn honeycomb4d_mdot(a_s: vec4<f32>, a_t: f32, b_s: vec4<f32>, b_t: f32) -> f32 {
    return dot(a_s, b_s) - a_t * b_t;
}

fn variation_honeycomb4d(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let sp = get_param(xform_id, variation_id, 0u);
    let sq = get_param(xform_id, variation_id, 1u);
    let sr = get_param(xform_id, variation_id, 2u);
    let ss = get_param(xform_id, variation_id, 3u);
    let size = max(get_param(xform_id, variation_id, 4u), 1e-6);
    let steps = i32(get_param(xform_id, variation_id, 5u));
    let projection = u32(get_param(xform_id, variation_id, 6u));
    let dc_mode = u32(get_param(xform_id, variation_id, 7u));
    let dc_scale = get_param(xform_id, variation_id, 8u);
    let color_speed = get_param(xform_id, variation_id, 9u);
    let seed_mode = u32(get_param(xform_id, variation_id, 10u));
    let thickness = get_param(xform_id, variation_id, 11u);
    let variant = u32(get_param(xform_id, variation_id, 12u));
    var depth = get_state(xform_id, variation_id, 1u);

    // Orthoscheme mirrors for the linear diagram p-q-r-s. Non-adjacent
    // Coxeter entries are 2 (orthogonal), which empties the slots the
    // triangular chain skips: n3 has no y component, n4 only the last
    // two. Assumes spherical [p,q] and [p,q,r] subgroups (all compact
    // H4 honeycombs); the max() clamps degrade everything else.
    let pi = 3.14159265359;
    let cp = cos(pi / max(sp, 2.0));
    let cq = cos(pi / max(sq, 2.0));
    let cr = cos(pi / max(sr, 2.0));
    let cs = cos(pi / max(ss, 2.0));
    let s1 = sqrt(max(1.0 - cp * cp, 1e-6));
    let y2 = -cq / s1;
    let z2 = sqrt(max(1.0 - y2 * y2, 1e-6));
    let z3 = -cr / z2;
    let u3 = sqrt(max(1.0 - z3 * z3, 1e-6));
    let u4 = -cs / u3;
    let t4 = sqrt(max(u4 * u4 - 1.0, 0.0));
    var mirror_s = array<vec4<f32>, 5>(
        vec4<f32>(1.0, 0.0, 0.0, 0.0),
        vec4<f32>(-cp, s1, 0.0, 0.0),
        vec4<f32>(0.0, y2, z2, 0.0),
        vec4<f32>(0.0, 0.0, z3, u3),
        vec4<f32>(0.0, 0.0, 0.0, u4),
    );
    var mirror_t = array<f32, 5>(0.0, 0.0, 0.0, 0.0, t4);

    var xs: vec4<f32>;
    var xt: f32;
    if (seed_mode != 0u && rng_nextf(rng) < 0.1) {
        // Wythoff generator over the first four nodes (see honeycomb).
        var mask = 1u;
        switch variant {
            case 1u: { mask = 2u; }
            case 2u: { mask = 3u; }
            case 3u: { mask = 6u; }
            case 4u: { mask = 5u; }
            case 5u: { mask = 7u; }
            case 6u: { mask = 11u; }
            case 7u: { mask = 13u; }
            case 8u: { mask = 15u; }
            default: { mask = 1u; }
        }
        let rc0 = select(0.0, -1.0, (mask & 1u) != 0u);
        let rc1 = select(0.0, -1.0, (mask & 2u) != 0u);
        let rc2 = select(0.0, -1.0, (mask & 4u) != 0u);
        let rc3 = select(0.0, -1.0, (mask & 8u) != 0u);
        // Triangular back-substitution down the mirror chain; node 4
        // is always unringed (<P,n4> = 0).
        let px = rc0;
        let py = (rc1 + cp * px) / s1;
        let pz = (rc2 - y2 * py) / max(z2, 1e-6);
        let pu = (rc3 - z3 * pz) / max(u3, 1e-6);
        let pt = u4 * pu / max(t4, 1e-6);
        var pms = vec4<f32>(px, py, pz, pu);
        var pmt = pt;
        if (pmt < 0.0) {
            pms = -pms;
            pmt = -pmt;
        }
        var nn = sqrt(max(-honeycomb4d_mdot(pms, pmt, pms, pmt), 1e-6));
        let wps = pms / nn;
        let wpt = pmt / nn;
        var ms = wps;
        var mt = wpt;
        if (seed_mode >= 2u) {
            let nring = f32(countOneBits(mask));
            var pick = i32(min(rng_nextf(rng) * nring, nring - 0.5));
            var mi = 0u;
            for (var i = 0u; i < 4u; i = i + 1u) {
                if ((mask & (1u << i)) != 0u) {
                    if (pick == 0) { mi = i; break; }
                    pick = pick - 1;
                }
            }
            // Edge foot: wp projected onto mirror mi.
            let d1 = honeycomb4d_mdot(wps, wpt, mirror_s[mi], mirror_t[mi]);
            var f1s = wps - d1 * mirror_s[mi];
            var f1t = wpt - d1 * mirror_t[mi];
            nn = sqrt(max(-honeycomb4d_mdot(f1s, f1t, f1s, f1t), 1e-6));
            f1s = f1s / nn;
            f1t = f1t / nn;
            if (seed_mode == 2u) {
                let tt = rng_nextf(rng);
                ms = mix(wps, f1s, tt);
                mt = mix(wpt, f1t, tt);
            } else {
                var mj = (mi + 1u + min(u32(rng_nextf(rng) * 4.0), 3u)) % 5u;
                for (var ttry = 0u; ttry < 4u; ttry = ttry + 1u) {
                    if (mj != mi && abs(honeycomb4d_mdot(f1s, f1t, mirror_s[mj], mirror_t[mj])) > 1e-4) { break; }
                    mj = (mj + 1u) % 5u;
                    if (mj == mi) { mj = (mj + 1u) % 5u; }
                }
                let d2 = honeycomb4d_mdot(f1s, f1t, mirror_s[mj], mirror_t[mj]);
                var f2s = f1s - d2 * mirror_s[mj];
                var f2t = f1t - d2 * mirror_t[mj];
                nn = sqrt(max(-honeycomb4d_mdot(f2s, f2t, f2s, f2t), 1e-6));
                f2s = f2s / nn;
                f2t = f2t / nn;
                let u = rng_nextf(rng);
                let v = rng_nextf(rng) * (1.0 - u);
                ms = wps + u * (f1s - wps) + v * (f2s - wps);
                mt = wpt + u * (f1t - wpt) + v * (f2t - wpt);
            }
        }
        nn = sqrt(max(-honeycomb4d_mdot(ms, mt, ms, mt), 1e-6));
        xs = ms / nn;
        xt = mt / nn;
        if (thickness > 0.0) {
            // Geodesic tangent offset (see honeycomb): random spatial
            // direction projected to the tangent space, then
            // x' = cosh(T)x + sinh(T)u.
            // Minkowski-orthonormal tangent frame + uniform S^3
            // direction (Marsaglia). Projecting a Euclidean direction
            // biases toward the seed's own spatial direction at depth
            // — exactly where the Wythoff variants sit — squashing
            // the tubes (see honeycomb).
            var e1s = vec4<f32>(1.0, 0.0, 0.0, 0.0);
            var e1t = 0.0;
            var cd = honeycomb4d_mdot(e1s, e1t, xs, xt);
            e1s = e1s + cd * xs; e1t = e1t + cd * xt;
            nn = sqrt(max(honeycomb4d_mdot(e1s, e1t, e1s, e1t), 1e-6));
            e1s = e1s / nn; e1t = e1t / nn;
            var e2s = vec4<f32>(0.0, 1.0, 0.0, 0.0);
            var e2t = 0.0;
            cd = honeycomb4d_mdot(e2s, e2t, xs, xt);
            e2s = e2s + cd * xs; e2t = e2t + cd * xt;
            cd = honeycomb4d_mdot(e2s, e2t, e1s, e1t);
            e2s = e2s - cd * e1s; e2t = e2t - cd * e1t;
            nn = sqrt(max(honeycomb4d_mdot(e2s, e2t, e2s, e2t), 1e-6));
            e2s = e2s / nn; e2t = e2t / nn;
            var e3s = vec4<f32>(0.0, 0.0, 1.0, 0.0);
            var e3t = 0.0;
            cd = honeycomb4d_mdot(e3s, e3t, xs, xt);
            e3s = e3s + cd * xs; e3t = e3t + cd * xt;
            cd = honeycomb4d_mdot(e3s, e3t, e1s, e1t);
            e3s = e3s - cd * e1s; e3t = e3t - cd * e1t;
            cd = honeycomb4d_mdot(e3s, e3t, e2s, e2t);
            e3s = e3s - cd * e2s; e3t = e3t - cd * e2t;
            nn = sqrt(max(honeycomb4d_mdot(e3s, e3t, e3s, e3t), 1e-6));
            e3s = e3s / nn; e3t = e3t / nn;
            var e4s = vec4<f32>(0.0, 0.0, 0.0, 1.0);
            var e4t = 0.0;
            cd = honeycomb4d_mdot(e4s, e4t, xs, xt);
            e4s = e4s + cd * xs; e4t = e4t + cd * xt;
            cd = honeycomb4d_mdot(e4s, e4t, e1s, e1t);
            e4s = e4s - cd * e1s; e4t = e4t - cd * e1t;
            cd = honeycomb4d_mdot(e4s, e4t, e2s, e2t);
            e4s = e4s - cd * e2s; e4t = e4t - cd * e2t;
            cd = honeycomb4d_mdot(e4s, e4t, e3s, e3t);
            e4s = e4s - cd * e3s; e4t = e4t - cd * e3t;
            nn = sqrt(max(honeycomb4d_mdot(e4s, e4t, e4s, e4t), 1e-6));
            e4s = e4s / nn; e4t = e4t / nn;
            let uu = rng_nextf(rng);
            let th1 = rng_nextf(rng) * 6.28318530718;
            let th2 = rng_nextf(rng) * 6.28318530718;
            let ra = sqrt(1.0 - uu);
            let rb = sqrt(uu);
            let d1 = ra * cos(th1);
            let d2 = ra * sin(th1);
            let d3 = rb * cos(th2);
            let d4 = rb * sin(th2);
            let us = d1 * e1s + d2 * e2s + d3 * e3s + d4 * e4s;
            let ut = d1 * e1t + d2 * e2t + d3 * e3t + d4 * e4t;
            let ch = cosh(thickness);
            let sh = sinh(thickness);
            xs = ch * xs + sh * us;
            xt = ch * xt + sh * ut;
        }
        depth = 0.0;
    } else {
        // Lift 4-ball -> hyperboloid; the 4th ball coordinate comes
        // from the per-thread w register. Outside points fold in by
        // inversion.
        var b = vec4<f32>(p, point_w) / size;
        if (projection == 2u) {
            // Half-space: 4th coordinate = height above the floor.
            let w = max(abs(b.w), 1e-6);
            let bs = vec3<f32>(b.x, b.y, b.z) / w;
            let s2 = b.x * b.x + b.y * b.y + b.z * b.z + w * w;
            let t = (1.0 + s2) / (2.0 * w);
            xs = vec4<f32>(bs, t - 1.0 / w);
            xt = t;
        } else {
            var r2 = dot(b, b);
            if (r2 >= 1.0) {
                b = b / (r2 + 1e-9);
                r2 = dot(b, b);
            }
            if (projection == 1u) {
                let t = 1.0 / sqrt(max(1.0 - r2, 1e-9));
                xs = b * t;
                xt = t;
            } else {
                let d = max(1.0 - r2, 1e-9);
                xs = 2.0 * b / d;
                xt = (1.0 + r2) / d;
            }
        }
    }

    // Random mirror walk (never the same mirror twice in a row).
    var creg = get_state(xform_id, variation_id, 0u);
    var prev = 99u;
    for (var i = 0; i < steps; i = i + 1) {
        var idx = min(u32(rng_nextf(rng) * 5.0), 4u);
        if (idx == prev) {
            idx = (idx + 1u + min(u32(rng_nextf(rng) * 4.0), 3u)) % 5u;
        }
        prev = idx;
        let d = honeycomb4d_mdot(xs, xt, mirror_s[idx], mirror_t[idx]);
        xs = xs - 2.0 * d * mirror_s[idx];
        xt = xt - 2.0 * d * mirror_t[idx];
        if (dc_mode == 1u) {
            creg = mix(creg, fract((f32(idx) + 0.5) / 5.0 * dc_scale), color_speed);
        }
    }
    if (dc_mode == 1u) {
        set_state(xform_id, variation_id, 0u, creg);
        *vc = creg;
    }

    // Project back; plot the xyz shadow, park the 4th ball coordinate
    // in the w register.
    depth = depth + f32(steps);
    set_state(xform_id, variation_id, 1u, depth);
    let t = max(xt, 1.0);
    var out4: vec4<f32>;
    if (projection == 1u) {
        out4 = (xs / t) * size;
    } else if (projection == 2u) {
        let iw = max(xt - xs.w, 1e-6);
        out4 = vec4<f32>(xs.x / iw, xs.y / iw, xs.z / iw, 1.0 / iw) * size;
    } else {
        out4 = (xs / (1.0 + t)) * size;
    }
    point_w_out = out4.w;
    if (dc_mode == 2u) {
        *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * out4.z / size);
    } else if (dc_mode == 3u) {
        if (seed_mode != 0u) {
            // Population-proportional (same reasoning as
            // sphere_packing): reseeding makes the generation index
            // g = depth/steps geometrically distributed, so a linear
            // ramp bunches most samples into a short stretch of
            // palette. Mapping g to the MIDPOINT of its
            // cumulative-probability interval [F(g-1), F(g)] gives
            // each generation a slot spaced by how many samples it
            // holds, filling the palette. q = 1 - 0.1 mirrors the
            // reseed probability used above; keep the two in sync.
            let g = max(depth / max(f32(steps), 1.0), 1.0);
            let q = 0.9;
            let t = 1.0 - pow(q, g - 1.0) * (1.0 + q) * 0.5;
            *vc = fract(t * dc_scale);
        } else {
            // This seed mode never reseeds, so depth grows without
            // bound: cycle instead (a saturating map would pin every
            // deep level to the palette end).
            *vc = fract(depth * color_speed * 0.1 * dc_scale);
        }
    }
    return out4.xyz;
}
"#;

// 2D render mode: the {p,q} disc tiling (same as `honeycomb`'s 2D
// body would give) — the extra Schläfli numbers have nothing to
// project onto a plane, so fall back to the triangle group.
const WGSL_2D: &str = r#"
fn honeycomb4d_mdot2(a: vec3<f32>, b: vec3<f32>) -> f32 {
    return a.x * b.x + a.y * b.y - a.z * b.z;
}

fn variation_honeycomb4d(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let sp = get_param(xform_id, variation_id, 0u);
    let sq = get_param(xform_id, variation_id, 1u);
    let sr = get_param(xform_id, variation_id, 2u);
    let size = max(get_param(xform_id, variation_id, 4u), 1e-6);
    let steps = i32(get_param(xform_id, variation_id, 5u));
    let projection = u32(get_param(xform_id, variation_id, 6u));
    let dc_mode = u32(get_param(xform_id, variation_id, 7u));
    let dc_scale = get_param(xform_id, variation_id, 8u);
    let color_speed = get_param(xform_id, variation_id, 9u);
    var depth = get_state(xform_id, variation_id, 1u);

    // General (p,q,r) triangle group in 2D render mode (r = 2 recovers
    // the {p,q} tiling); see honeycomb. s is unused here.
    let pi = 3.14159265359;
    let cp = cos(pi / max(sp, 2.0));
    let cq = cos(pi / max(sq, 2.0));
    let cr = cos(pi / max(sr, 2.0));
    let s1 = sqrt(max(1.0 - cp * cp, 1e-6));
    let n2x = -cr;
    let n2y = (-cq - cp * cr) / s1;
    let w2 = sqrt(max(n2x * n2x + n2y * n2y - 1.0, 0.0));
    var mirrors = array<vec3<f32>, 3>(
        vec3<f32>(1.0, 0.0, 0.0),
        vec3<f32>(-cp, s1, 0.0),
        vec3<f32>(n2x, n2y, w2),
    );

    let seed_mode = u32(get_param(xform_id, variation_id, 10u));
    let thickness = get_param(xform_id, variation_id, 11u);
    let variant = u32(get_param(xform_id, variation_id, 12u));

    var x: vec3<f32>;
    if (seed_mode != 0u && rng_nextf(rng) < 0.1) {
        // Wythoff generator on the triangle group (variant's low ring
        // bits), then edge-foot / face-fragment sampling — the same
        // machinery as `honeycomb`'s 2D body.
        var mask = 1u;
        switch variant {
            case 1u: { mask = 2u; }
            case 2u: { mask = 3u; }
            case 3u: { mask = 6u; }
            case 4u: { mask = 5u; }
            case 5u, 8u: { mask = 7u; }
            case 6u: { mask = 3u; }
            case 7u: { mask = 5u; }
            default: { mask = 1u; }
        }
        let rc0 = select(0.0, -1.0, (mask & 1u) != 0u);
        let rc1 = select(0.0, -1.0, (mask & 2u) != 0u);
        let rc2 = select(0.0, -1.0, (mask & 4u) != 0u);
        let px = rc0;
        let py = (rc1 + cp * px) / s1;
        let w2s = max(w2, 1e-6);
        let pt = (n2x * px + n2y * py - rc2) / w2s;
        var pm = vec3<f32>(px, py, pt);
        if (pm.z < 0.0) { pm = -pm; }
        let wp = pm / sqrt(max(-honeycomb4d_mdot2(pm, pm), 1e-6));
        var m = wp;
        if (seed_mode >= 2u) {
            let nring = f32(countOneBits(mask));
            var pick = i32(min(rng_nextf(rng) * nring, nring - 0.5));
            var mi = 0u;
            for (var i = 0u; i < 3u; i = i + 1u) {
                if ((mask & (1u << i)) != 0u) {
                    if (pick == 0) { mi = i; break; }
                    pick = pick - 1;
                }
            }
            var f1 = wp - honeycomb4d_mdot2(wp, mirrors[mi]) * mirrors[mi];
            f1 = f1 / sqrt(max(-honeycomb4d_mdot2(f1, f1), 1e-6));
            if (seed_mode == 2u) {
                m = mix(wp, f1, rng_nextf(rng));
            } else {
                var mj = (mi + 1u + min(u32(rng_nextf(rng) * 2.0), 1u)) % 3u;
                for (var ttry = 0u; ttry < 2u; ttry = ttry + 1u) {
                    if (mj != mi && abs(honeycomb4d_mdot2(f1, mirrors[mj])) > 1e-4) { break; }
                    mj = (mj + 1u) % 3u;
                    if (mj == mi) { mj = (mj + 1u) % 3u; }
                }
                var f2 = f1 - honeycomb4d_mdot2(f1, mirrors[mj]) * mirrors[mj];
                f2 = f2 / sqrt(max(-honeycomb4d_mdot2(f2, f2), 1e-6));
                let u = rng_nextf(rng);
                let v = rng_nextf(rng) * (1.0 - u);
                m = wp + u * (f1 - wp) + v * (f2 - wp);
            }
        }
        x = m / sqrt(max(-honeycomb4d_mdot2(m, m), 1e-6));
        if (thickness > 0.0) {
            // Geodesic tangent offset, disc-filled (see honeycomb 2D).
            let ph = rng_nextf(rng) * 6.28318530718;
            let tt = thickness * sqrt(rng_nextf(rng));
            // Minkowski-orthonormal tangent frame (see honeycomb).
            var e1 = vec3<f32>(1.0, 0.0, 0.0);
            e1 = e1 + honeycomb4d_mdot2(e1, x) * x;
            e1 = e1 / sqrt(max(honeycomb4d_mdot2(e1, e1), 1e-6));
            var e2 = vec3<f32>(0.0, 1.0, 0.0);
            e2 = e2 + honeycomb4d_mdot2(e2, x) * x;
            e2 = e2 - honeycomb4d_mdot2(e2, e1) * e1;
            e2 = e2 / sqrt(max(honeycomb4d_mdot2(e2, e2), 1e-6));
            let u = cos(ph) * e1 + sin(ph) * e2;
            x = cosh(tt) * x + sinh(tt) * u;
        }
        depth = 0.0;
    } else {
        var b = p / size;
        var r2 = dot(b, b);
        if (r2 >= 1.0) {
            b = b / (r2 + 1e-9);
            r2 = dot(b, b);
        }
        if (projection == 1u) {
            let t = 1.0 / sqrt(max(1.0 - r2, 1e-9));
            x = vec3<f32>(b * t, t);
        } else {
            let d = max(1.0 - r2, 1e-9);
            x = vec3<f32>(2.0 * b / d, (1.0 + r2) / d);
        }
    }

    var creg = get_state(xform_id, variation_id, 0u);
    var prev = 99u;
    for (var i = 0; i < steps; i = i + 1) {
        var idx = min(u32(rng_nextf(rng) * 3.0), 2u);
        if (idx == prev) {
            idx = (idx + 1u + min(u32(rng_nextf(rng) * 2.0), 1u)) % 3u;
        }
        prev = idx;
        let n = mirrors[idx];
        x = x - 2.0 * honeycomb4d_mdot2(x, n) * n;
        if (dc_mode == 1u) {
            creg = mix(creg, fract((f32(idx) + 0.5) / 3.0 * dc_scale), color_speed);
        }
    }
    if (dc_mode == 1u) {
        set_state(xform_id, variation_id, 0u, creg);
        *vc = creg;
    }

    depth = depth + f32(steps);
    set_state(xform_id, variation_id, 1u, depth);
    let t = max(x.z, 1.0);
    var out: vec2<f32>;
    if (projection == 1u) {
        out = (x.xy / t) * size;
    } else {
        out = (x.xy / (1.0 + t)) * size;
    }
    if (dc_mode == 2u) {
        *vc = fract(length(out) / size * dc_scale);
    } else if (dc_mode == 3u) {
        if (seed_mode != 0u) {
            // Population-proportional (same reasoning as
            // sphere_packing): reseeding makes the generation index
            // g = depth/steps geometrically distributed, so a linear
            // ramp bunches most samples into a short stretch of
            // palette. Mapping g to the MIDPOINT of its
            // cumulative-probability interval [F(g-1), F(g)] gives
            // each generation a slot spaced by how many samples it
            // holds, filling the palette. q = 1 - 0.1 mirrors the
            // reseed probability used above; keep the two in sync.
            let g = max(depth / max(f32(steps), 1.0), 1.0);
            let q = 0.9;
            let t = 1.0 - pow(q, g - 1.0) * (1.0 + q) * 0.5;
            *vc = fract(t * dc_scale);
        } else {
            // This seed mode never reseeds, so depth grows without
            // bound: cycle instead (a saturating map would pin every
            // deep level to the palette end).
            *vc = fract(depth * color_speed * 0.1 * dc_scale);
        }
    }
    return out;
}
"#;
