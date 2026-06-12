/// `octapol` — octagon-with-polar-core shape warp (Georg K., 2011-ish).
///
/// The input (pre-scaled by 0.15) is tested against three zones built
/// from `s` and `t`:
///
///   1. A center circle of radius `0.7071·s·|radius|`: points inside
///      are blended toward polar coordinates `(φ, r)` by
///      `log((r/rad)²) · polarweight` — the "pol" half of the name.
///   2. Four corner triangles of an octagon outline (vertices A..L
///      derived from `s`/`t`): points inside pass through and get
///      doubled by the trailing add.
///   3. Everything else: the running variation accumulator is
///      CLOBBERED to zero before the trailing add — verbatim JWF
///      behavior (`pVarTP.x = pVarTP.y = 0`), which wipes prior
///      variations' contributions on this xform. Reproduced via
///      `Feature::NeedsAccum` + returning `(x, y) − accum/w` so the
///      dispatcher's `accum += w·f` lands on exactly `amount·(x, y)`.
///
/// Port quirk kept faithfully: the Java also tests four axis-aligned
/// rects (H–K, J–D, A–J, K–E), but their corner arguments are inverted
/// for positive `s`/`t` (e.g. requires `p.y ≥ s/2` AND `p.y ≤ −s/2`),
/// so they can never hit and only the triangles contribute. We port
/// them as-is so negative `s`/`t` values — where some flip back into
/// validity — behave identically to JWildfire.
///
/// Source:
/// [`output/variation-jwf-source/OctapolFunc.java`](../../../output/variation-jwf-source/OctapolFunc.java).
///
/// # Authors
/// - Xyrus02

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static OCTAPOL: VariationDef = VariationDef {
    name: "octapol",
    aliases: &[],
    display_name: "Octapol",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsAccum],
    parameters: &[
        param!("polarweight", "Polar Weight", unlimited_float, 0.0, -5.0, 5.0, "Blend toward polar coordinates inside the center circle. 0 = pass-through; larger magnitudes warp circle content onto the (φ, r) plane, scaled by log((r/rad)²) so the effect strengthens toward the center."),
        param!("radius", "Radius", unlimited_float, 1.0, -3.0, 3.0, "Center-circle size factor. Effective radius is 0.7071 · s · |radius|; 0 disables the polar core entirely."),
        param!("s", "S", unlimited_float, 0.5, -2.0, 2.0, "Octagon edge size — half the length of the axis-aligned edges, and the scale of the corner triangles."),
        param!("t", "T", unlimited_float, 0.5, -2.0, 2.0, "Octagon extent — how far the diagonal edges reach beyond the inner square. The bounding half-size is s/2 + t."),
    ],
    // 2 derived values at slots 4..6:
    //   4: a    (s·0.5 + t — bounding-square half size)
    //   5: rad  (0.707106781 · s · |radius| — center-circle radius)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_octapol(user: array<f32, 4>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = user[2] * 0.5 + user[3];
    out[1] = 0.707106781 * user[2] * abs(user[1]);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Shared helper semantics (duplicated in both mode blocks — a compiled
// shader only ever includes one of them):
//
//   octapol_hits_rect(tl, br, p): verbatim JWF — `p ≥ tl && p ≤ br`
//     componentwise, including the inverted-corner degeneracy at the
//     call sites (see module docs).
//   octapol_hits_triangle(a, b, c, p): barycentric inside test with
//     v0 = c−a, v1 = b−a; strict `u > 0 && v > 0 && u+v < 1`, and the
//     JWF `denom != 0` guard mapping degenerate triangles to (0, 0)
//     (a miss).
const WGSL_2D: &str = r#"
fn octapol_hits_rect(tl: vec2<f32>, br: vec2<f32>, p: vec2<f32>) -> bool {
    return p.x >= tl.x && p.y >= tl.y && p.x <= br.x && p.y <= br.y;
}

fn octapol_hits_triangle(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, p: vec2<f32>) -> bool {
    let v0 = c - a;
    let v1 = b - a;
    let v2 = p - a;
    let d00 = dot(v0, v0);
    let d01 = dot(v0, v1);
    let d02 = dot(v0, v2);
    let d11 = dot(v1, v1);
    let d12 = dot(v1, v2);
    let denom = d00 * d11 - d01 * d01;
    var u = 0.0;
    var v = 0.0;
    if (denom != 0.0) {
        u = (d11 * d02 - d01 * d12) / denom;
        v = (d00 * d12 - d01 * d02) / denom;
    }
    return ((u + v) < 1.0) && (u > 0.0) && (v > 0.0);
}

fn variation_octapol(p: vec2<f32>, accum: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let polarweight = get_param(xform_id, variation_id, 0u);
    let s = get_param(xform_id, variation_id, 2u);
    let t = get_param(xform_id, variation_id, 3u);
    let a = get_param(xform_id, variation_id, 4u);
    let rad = get_param(xform_id, variation_id, 5u);

    // JWF scales the input by 0.15 for x/y (z stays unscaled).
    let q = vec2<f32>(p.x * 0.15, p.y * 0.15);

    // Octagon vertices (JWF precomputes these in init(); they're
    // cheap adds, so we derive them from s/t per call).
    let hs = 0.5 * s;
    let va = vec2<f32>(-hs, hs + t);
    let vb = vec2<f32>(hs, hs + t);
    let vc_ = vec2<f32>(t, hs);
    let vd = vec2<f32>(t, -hs);
    let ve = vec2<f32>(hs, -hs - t);
    let vf = vec2<f32>(-hs, -hs - t);
    let vg = vec2<f32>(-t, -hs);
    let vh = vec2<f32>(-t, hs);
    let vi = vec2<f32>(-hs, hs);
    let vj = vec2<f32>(hs, hs);
    let vk = vec2<f32>(-hs, -hs);
    let vl = vec2<f32>(hs, -hs);

    let r = length(q);
    if (rad > 0.0 && r <= rad) {
        // Polar core. JWF: out += amount·lerp((x,y), (φ,r), rd·pw),
        // then the trailing += amount·(x,y) — fold both into the
        // return so the dispatcher's single weight multiply matches.
        let rd = log((r / rad) * (r / rad));
        let phi = atan2(q.y, q.x);
        let m = rd * polarweight;
        return vec2<f32>(mix(q.x, phi, m) + q.x, mix(q.y, r, m) + q.y);
    } else if (abs(q.x) <= a && abs(q.y) <= a) {
        if (octapol_hits_rect(vh, vk, q) || octapol_hits_rect(vj, vd, q) ||
            octapol_hits_rect(va, vj, q) || octapol_hits_rect(vk, ve, q) ||
            octapol_hits_triangle(vi, va, vh, q) ||
            octapol_hits_triangle(vj, vb, vc_, q) ||
            octapol_hits_triangle(vl, vd, ve, q) ||
            octapol_hits_triangle(vk, vf, vg, q)) {
            // Shape hit: amount·(x,y) from the branch + the trailing
            // amount·(x,y) = 2·amount·(x,y).
            return 2.0 * q;
        }
    }
    // Miss (inside square but outside shapes, or outside square):
    // JWF CLOBBERS the accumulator (pVarTP.x = pVarTP.y = 0) and then
    // the trailing add lands amount·(x,y). With dispatch
    // `accum += w·f`, return (x,y) − accum/w to reproduce the wipe.
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    return q - accum * inv_w;
}
"#;

const WGSL_3D: &str = r#"
fn octapol_hits_rect(tl: vec2<f32>, br: vec2<f32>, p: vec2<f32>) -> bool {
    return p.x >= tl.x && p.y >= tl.y && p.x <= br.x && p.y <= br.y;
}

fn octapol_hits_triangle(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, p: vec2<f32>) -> bool {
    let v0 = c - a;
    let v1 = b - a;
    let v2 = p - a;
    let d00 = dot(v0, v0);
    let d01 = dot(v0, v1);
    let d02 = dot(v0, v2);
    let d11 = dot(v1, v1);
    let d12 = dot(v1, v2);
    let denom = d00 * d11 - d01 * d01;
    var u = 0.0;
    var v = 0.0;
    if (denom != 0.0) {
        u = (d11 * d02 - d01 * d12) / denom;
        v = (d00 * d12 - d01 * d02) / denom;
    }
    return ((u + v) < 1.0) && (u > 0.0) && (v > 0.0);
}

fn variation_octapol(p: vec3<f32>, accum: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let polarweight = get_param(xform_id, variation_id, 0u);
    let s = get_param(xform_id, variation_id, 2u);
    let t = get_param(xform_id, variation_id, 3u);
    let a = get_param(xform_id, variation_id, 4u);
    let rad = get_param(xform_id, variation_id, 5u);

    // JWF scales the input by 0.15 for x/y; z passes through at full
    // scale (the dispatcher's weight multiply provides JWF's
    // `pVarTP.z += pAmount·z`, and the JWF clobber only zeroes x/y).
    let q = vec2<f32>(p.x * 0.15, p.y * 0.15);

    let hs = 0.5 * s;
    let va = vec2<f32>(-hs, hs + t);
    let vb = vec2<f32>(hs, hs + t);
    let vc_ = vec2<f32>(t, hs);
    let vd = vec2<f32>(t, -hs);
    let ve = vec2<f32>(hs, -hs - t);
    let vf = vec2<f32>(-hs, -hs - t);
    let vg = vec2<f32>(-t, -hs);
    let vh = vec2<f32>(-t, hs);
    let vi = vec2<f32>(-hs, hs);
    let vj = vec2<f32>(hs, hs);
    let vk = vec2<f32>(-hs, -hs);
    let vl = vec2<f32>(hs, -hs);

    let r = length(q);
    if (rad > 0.0 && r <= rad) {
        let rd = log((r / rad) * (r / rad));
        let phi = atan2(q.y, q.x);
        let m = rd * polarweight;
        return vec3<f32>(mix(q.x, phi, m) + q.x, mix(q.y, r, m) + q.y, p.z);
    } else if (abs(q.x) <= a && abs(q.y) <= a) {
        if (octapol_hits_rect(vh, vk, q) || octapol_hits_rect(vj, vd, q) ||
            octapol_hits_rect(va, vj, q) || octapol_hits_rect(vk, ve, q) ||
            octapol_hits_triangle(vi, va, vh, q) ||
            octapol_hits_triangle(vj, vb, vc_, q) ||
            octapol_hits_triangle(vl, vd, ve, q) ||
            octapol_hits_triangle(vk, vf, vg, q)) {
            return vec3<f32>(2.0 * q, p.z);
        }
    }
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    return vec3<f32>(q - accum.xy * inv_w, p.z);
}
"#;
