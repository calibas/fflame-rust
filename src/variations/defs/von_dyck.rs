//! `von_dyck` — hyperbolic (p,q,r) rotation groups as a Möbius chaos
//! game with tessellation seeding (original).
//!
//! The von Dyck group D(p,q,r) = ⟨x,y,z | xᵖ = y^q = z^r = xyz = 1⟩ —
//! the orientation-preserving half of the (p,q,r) triangle group —
//! realized as elliptic Möbius rotations by 2π/p, 2π/q, 2π/r about the
//! vertices of a hyperbolic (π/p, π/q, π/r) triangle in the Poincaré
//! disk (vertex positions from the hyperbolic law of cosines; the
//! relation x·y·z = ±I verified numerically, all rotations
//! counterclockwise). Hyperbolic iff 1/p + 1/q + 1/r < 1; default
//! (2,3,7) is the minimal-area Hurwitz triangle.
//!
//! A cocompact Fuchsian group's LIMIT SET is a round circle — proven
//! the hard way in this codebase (the discarded first fuchsian_triangle
//! attempt) — so the naked chaos game is structureless. What renders
//! the TILING is `honeycomb`'s seed machinery, ported here onto Möbius
//! orbits: `seed` plants the triangle's vertex orbit / edge skeleton /
//! face fragments (geodesic mixing on the hyperboloid, `thickness`
//! for uniform-hyperbolic-radius balls and tubes) and the group stamps
//! it across the disk. `Input` feeds the incoming flame measure
//! through instead, like honeycomb.
//!
//! What the Möbius framing adds over `honeycomb`'s Minkowski
//! reflections:
//! - **Mirror** (one anti-Möbius reflection z ↦ z̄ in the A–B edge,
//!   `su_apply_anti`) upgrades D(p,q,r) to the full triangle group
//!   Δ*(p,q,r) — an orientation-reversing generator honeycomb's
//!   reflection walk has natively but Möbius machinery now supports
//!   for ANY group.
//! - **QC Deform**: Bagula's triquasiconformal conjugation applied to
//!   every generator — the even→uneven warp for tilings (a global
//!   Möbius change of coordinates; triangle groups are RIGID, so this
//!   is the only deformation they admit).
//! - **Hyperbolic H3**: the Poincaré extension — the 2D Fuchsian group
//!   acting on upper half-space stamps the tiling into 3D "chimney"
//!   columns (not `honeycomb4d`'s genuine {p,q,r} honeycombs — a
//!   different object).
//!
//! Uses `Feature::NeedsMobiusLib` (`shaders/core/su_mobius.wgsl`).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static VON_DYCK: VariationDef = VariationDef {
    name: "von_dyck",
    aliases: &[],
    display_name: "Von Dyck",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::NeedsMobiusLib],
    // Slot 0: previous generator index (avoid_reversal analogue built
    // into the walk). Slot 1: color register. Slot 2: walk depth.
    state_count: 3,
    wgsl_state_init: None,
    parameters: &[
        param!("p", "P", int, 2.0, 2.0, 24.0, "Order of the rotation about vertex A (angle π/p there). (p,q,r) is hyperbolic iff 1/p + 1/q + 1/r < 1; the default (2,3,7) is the minimal Hurwitz triangle."),
        param!("q", "Q", int, 3.0, 2.0, 24.0, "Order of the rotation about vertex B (angle π/q)."),
        param!("r", "R", int, 7.0, 2.0, 24.0, "Order of the rotation about vertex C (angle π/r)."),
        param!("mirror", "Mirror", bool, false, "Add the reflection in the A–B edge (an anti-Möbius generator z ↦ z̄) — upgrades the rotation group D(p,q,r) to the FULL triangle group Δ*(p,q,r), doubling the stamped copies (each triangle plus its mirror image)."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Radius of the Poincaré disk in world units."),
        param!("steps", "Steps", int, 2.0, 1.0, 8.0, "Group elements applied per call (backtrack-avoiding random walk)."),
        param!("seed", "Seed", enum, 0, &["Input", "Vertices", "Edges", "Faces"], "What the walk stamps through the tiling: the incoming flame measure, the triangle's vertex orbit, its edge skeleton (geodesic segments), or face fragments — honeycomb's seed machinery on Möbius orbits."),
        param!("thickness", "Thickness", float, 0.0, 0.0, 2.0, "Seed modes: geodesic tangent-space offset by exact hyperbolic distance — balls at vertices, tubes along edges, slabs on faces, all with uniform hyperbolic radius."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Generator", "Steps"], "Direct-color source (needs the transform's Direct Color > 0). Generator: each group generator has its own palette position, blended through a persistent register at Color Speed. Steps: palette sweeps across the walk depth since the last reseed."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "Generator mode: pull strength toward each generator's palette position. Steps mode: how much of the palette the mean walk traverses."),
        param!("space", "Space", enum, 0, &["Planar", "Hyperbolic H3"], "3D render mode only. Planar: the disk tiling in the xy plane (z passes through). Hyperbolic H3: the Poincaré extension — the 2D group acts on upper half-space (height = z), stamping the tiling into 3D chimney columns over the disk."),
        param!("qc_deform", "QC Deform", bool, false, "Conjugate every generator by Bagula's triquasiconformal C = dk(δ)·s0·qf(θ+iη) — the even→uneven warp applied to a tiling. Triangle groups are rigid (their deformation space is a point), so this global Möbius warp is the only deformation they admit."),
        param!("conj_angle", "Angle", angle, 45.0, "Elliptic rotation θ in the conjugator (QC Deform on)."),
        param!("conj_hyper", "Hyper Angle", angle, 0.0, "Hyperbolic rotation η in the conjugator (QC Deform on)."),
        param!("qc_strength", "QC Strength", float, 1.0, 0.1, 2.0, "Quasiconformal δ in dk = [[1+iδ,1],[1,1−iδ]] (QC Deform on)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Helper block shared verbatim by both bodies (only one compiles per
// flame). Hyperboloid model (x, y | t) for geodesic seed mixing; the
// WALK itself is pure Möbius on disk coordinates.
const WGSL_2D: &str = r#"
fn vd_mdot(a: vec3<f32>, b: vec3<f32>) -> f32 {
    return a.x * b.x + a.y * b.y - a.z * b.z;
}

fn vd_lift(u: vec2<f32>) -> vec3<f32> {
    let den = max(1.0 - dot(u, u), 1e-6);
    return vec3<f32>(2.0 * u / den, (1.0 + dot(u, u)) / den);
}

fn vd_proj(h: vec3<f32>) -> vec2<f32> {
    return h.xy / (1.0 + max(h.z, 1.0));
}

fn vd_hnorm(v: vec3<f32>) -> vec3<f32> {
    return v / sqrt(max(-vd_mdot(v, v), 1e-6));
}

// Elliptic rotation about disk point w by angle phi (lam = e^{i phi/2}):
// T(w)·diag(lam, conj lam)·T(w)^-1 with T(w) = [[1, w], [conj w, 1]].
// Relation x·y·z = ±I verified for counterclockwise phi at all vertices.
fn vd_rot(w: vec2<f32>, lam: vec2<f32>) -> SuMat {
    let lami = vec2<f32>(lam.x, -lam.y);
    let w2 = dot(w, w);
    let den = max(1.0 - w2, 1e-6);
    return SuMat(
        (lam - w2 * lami) / den,
        cmul(w, lami - lam) / den,
        cmul(vec2<f32>(w.x, -w.y), lam - lami) / den,
        (lami - w2 * lam) / den);
}

// Generator k: 0..2 = rotations at A/B/C by +2pi/n, 3..5 = inverses.
fn vd_gen(k: u32, A: vec2<f32>, B: vec2<f32>, C: vec2<f32>, sp: f32, sq: f32, sr: f32) -> SuMat {
    let base = k % 3u;
    let sgn = select(1.0, -1.0, k >= 3u);
    var w = A; var n = sp;
    if (base == 1u) { w = B; n = sq; }
    else if (base == 2u) { w = C; n = sr; }
    let half = sgn * 3.14159265359 / n;   // phi/2 with phi = 2*pi/n
    return vd_rot(w, vec2<f32>(cos(half), sin(half)));
}

fn variation_von_dyck(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let sp = max(get_param(xform_id, variation_id, 0u), 2.0);
    let sq = max(get_param(xform_id, variation_id, 1u), 2.0);
    let sr = max(get_param(xform_id, variation_id, 2u), 2.0);
    let mirror = get_param(xform_id, variation_id, 3u) > 0.5;
    let size = max(get_param(xform_id, variation_id, 4u), 1e-6);
    let steps = i32(get_param(xform_id, variation_id, 5u));
    let seed_mode = u32(get_param(xform_id, variation_id, 6u));
    let thickness = get_param(xform_id, variation_id, 7u);
    let dc_mode = u32(get_param(xform_id, variation_id, 8u));
    let dc_scale = get_param(xform_id, variation_id, 9u);
    let color_speed = get_param(xform_id, variation_id, 10u);
    let deform = get_param(xform_id, variation_id, 12u) > 0.5;
    let theta = get_param(xform_id, variation_id, 13u) * 0.01745329252;
    let eta = get_param(xform_id, variation_id, 14u) * 0.01745329252;
    let delta = get_param(xform_id, variation_id, 15u);

    // Triangle vertices: A at the origin, B on the real axis, C at
    // angle pi/p — hyperbolic law of cosines for the side lengths.
    let pi = 3.14159265359;
    let ap = pi / sp; let aq = pi / sq; let ar = pi / sr;
    let coshAB = (cos(ar) + cos(ap) * cos(aq)) / max(sin(ap) * sin(aq), 1e-6);
    let coshAC = (cos(aq) + cos(ap) * cos(ar)) / max(sin(ap) * sin(ar), 1e-6);
    let rB = tanh(0.5 * acosh(max(coshAB, 1.0)));
    let rC = tanh(0.5 * acosh(max(coshAC, 1.0)));
    let A = vec2<f32>(0.0, 0.0);
    let B = vec2<f32>(rB, 0.0);
    let C = rC * vec2<f32>(cos(ap), sin(ap));

    var cnt = 6u;
    if (mirror) { cnt = 7u; }
    var cj = SuMat(vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0));
    var cji = cj;
    if (deform) {
        cj = su_conjugator(theta, eta, delta);
        cji = su_matinv(cj);
    }

    var depth = get_state(xform_id, variation_id, 2u);
    var a: vec2<f32>;
    if (seed_mode != 0u && rng_nextf(rng) < 0.1) {
        // Plant a fundamental-domain seed (honeycomb's machinery on
        // Möbius orbits): vertex orbit, edge skeleton, or face fill.
        var m: vec3<f32>;
        if (seed_mode == 1u) {
            let pick = min(u32(rng_nextf(rng) * 3.0), 2u);
            var w = A;
            if (pick == 1u) { w = B; } else if (pick == 2u) { w = C; }
            m = vd_lift(w);
        } else if (seed_mode == 2u) {
            let pick = min(u32(rng_nextf(rng) * 3.0), 2u);
            var u0 = A; var u1 = B;
            if (pick == 1u) { u0 = B; u1 = C; }
            else if (pick == 2u) { u0 = C; u1 = A; }
            let t = rng_nextf(rng);
            m = vd_hnorm(mix(vd_lift(u0), vd_lift(u1), t));
        } else {
            let u = rng_nextf(rng);
            let v = rng_nextf(rng) * (1.0 - u);
            let hA = vd_lift(A); let hB = vd_lift(B); let hC = vd_lift(C);
            m = vd_hnorm(hA + u * (hB - hA) + v * (hC - hA));
        }
        if (thickness > 0.0) {
            // Uniform-hyperbolic-radius disc offset in the tangent
            // plane (Minkowski-orthonormal frame, as in honeycomb).
            let ph = rng_nextf(rng) * 6.28318530718;
            let tt = thickness * sqrt(rng_nextf(rng));
            var e1 = vec3<f32>(1.0, 0.0, 0.0);
            e1 = e1 + vd_mdot(e1, m) * m;
            e1 = e1 / sqrt(max(vd_mdot(e1, e1), 1e-6));
            var e2 = vec3<f32>(0.0, 1.0, 0.0);
            e2 = e2 + vd_mdot(e2, m) * m;
            e2 = e2 - vd_mdot(e2, e1) * e1;
            e2 = e2 / sqrt(max(vd_mdot(e2, e2), 1e-6));
            let u = cos(ph) * e1 + sin(ph) * e2;
            m = cosh(tt) * m + sinh(tt) * u;
        }
        a = vd_proj(m);
        depth = 0.0;
    } else {
        a = p / size;
        let r2 = dot(a, a);
        if (r2 >= 1.0) { a = a / (r2 + 1e-9); }
    }

    var prev = u32(get_state(xform_id, variation_id, 0u));
    var creg = get_state(xform_id, variation_id, 1u);
    for (var i = 0; i < steps; i = i + 1) {
        var k = min(u32(rng_nextf(rng) * f32(cnt)), cnt - 1u);
        // Backtrack-avoid: rotation k inverts to (k+3)%6; the mirror
        // (k = 6) is an involution and blocks itself.
        var invp = prev;
        if (prev < 6u) { invp = (prev + 3u) % 6u; }
        if (prev < cnt && k == invp) { k = (k + 1u) % cnt; }
        if (k == 6u) {
            // Reflection in the A–B edge: anti-Möbius z ↦ conj z,
            // conjugated as C ∘ conj ∘ C⁻¹ when deformed.
            if (deform) {
                var t = su_apply_plain(cji, a);
                t = vec2<f32>(t.x, -t.y);
                a = su_apply_plain(cj, t);
            } else {
                a = vec2<f32>(a.x, -a.y);
            }
        } else {
            let g = vd_gen(k, A, B, C, sp, sq, sr);
            if (deform) { a = su_apply_m(g, a, cj, cji); }
            else { a = su_apply_plain(g, a); }
        }
        prev = k;
        if (dc_mode == 1u) {
            creg = mix(creg, fract((f32(k) + 0.5) / f32(cnt) * dc_scale), color_speed);
        }
    }
    set_state(xform_id, variation_id, 0u, f32(prev));
    depth = depth + f32(steps);
    set_state(xform_id, variation_id, 2u, depth);

    if (dc_mode == 1u) {
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    } else if (dc_mode == 2u) {
        let t = clamp(depth * color_speed / (10.0 * f32(steps)), 0.0, 1.0);
        *vc = fract(min(t * dc_scale, 0.999));
    }
    return a * size;
}
"#;

const WGSL_3D: &str = r#"
fn vd_mdot(a: vec3<f32>, b: vec3<f32>) -> f32 {
    return a.x * b.x + a.y * b.y - a.z * b.z;
}

fn vd_lift(u: vec2<f32>) -> vec3<f32> {
    let den = max(1.0 - dot(u, u), 1e-6);
    return vec3<f32>(2.0 * u / den, (1.0 + dot(u, u)) / den);
}

fn vd_proj(h: vec3<f32>) -> vec2<f32> {
    return h.xy / (1.0 + max(h.z, 1.0));
}

fn vd_hnorm(v: vec3<f32>) -> vec3<f32> {
    return v / sqrt(max(-vd_mdot(v, v), 1e-6));
}

fn vd_rot(w: vec2<f32>, lam: vec2<f32>) -> SuMat {
    let lami = vec2<f32>(lam.x, -lam.y);
    let w2 = dot(w, w);
    let den = max(1.0 - w2, 1e-6);
    return SuMat(
        (lam - w2 * lami) / den,
        cmul(w, lami - lam) / den,
        cmul(vec2<f32>(w.x, -w.y), lam - lami) / den,
        (lami - w2 * lam) / den);
}

fn vd_gen(k: u32, A: vec2<f32>, B: vec2<f32>, C: vec2<f32>, sp: f32, sq: f32, sr: f32) -> SuMat {
    let base = k % 3u;
    let sgn = select(1.0, -1.0, k >= 3u);
    var w = A; var n = sp;
    if (base == 1u) { w = B; n = sq; }
    else if (base == 2u) { w = C; n = sr; }
    let half = sgn * 3.14159265359 / n;
    return vd_rot(w, vec2<f32>(cos(half), sin(half)));
}

fn variation_von_dyck(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let sp = max(get_param(xform_id, variation_id, 0u), 2.0);
    let sq = max(get_param(xform_id, variation_id, 1u), 2.0);
    let sr = max(get_param(xform_id, variation_id, 2u), 2.0);
    let mirror = get_param(xform_id, variation_id, 3u) > 0.5;
    let size = max(get_param(xform_id, variation_id, 4u), 1e-6);
    let steps = i32(get_param(xform_id, variation_id, 5u));
    let seed_mode = u32(get_param(xform_id, variation_id, 6u));
    let thickness = get_param(xform_id, variation_id, 7u);
    let dc_mode = u32(get_param(xform_id, variation_id, 8u));
    let dc_scale = get_param(xform_id, variation_id, 9u);
    let color_speed = get_param(xform_id, variation_id, 10u);
    let space = u32(get_param(xform_id, variation_id, 11u));
    let deform = get_param(xform_id, variation_id, 12u) > 0.5;
    let theta = get_param(xform_id, variation_id, 13u) * 0.01745329252;
    let eta = get_param(xform_id, variation_id, 14u) * 0.01745329252;
    let delta = get_param(xform_id, variation_id, 15u);

    let pi = 3.14159265359;
    let ap = pi / sp; let aq = pi / sq; let ar = pi / sr;
    let coshAB = (cos(ar) + cos(ap) * cos(aq)) / max(sin(ap) * sin(aq), 1e-6);
    let coshAC = (cos(aq) + cos(ap) * cos(ar)) / max(sin(ap) * sin(ar), 1e-6);
    let rB = tanh(0.5 * acosh(max(coshAB, 1.0)));
    let rC = tanh(0.5 * acosh(max(coshAC, 1.0)));
    let A = vec2<f32>(0.0, 0.0);
    let B = vec2<f32>(rB, 0.0);
    let C = rC * vec2<f32>(cos(ap), sin(ap));

    var cnt = 6u;
    if (mirror) { cnt = 7u; }
    var cj = SuMat(vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0));
    var cji = cj;
    if (deform) {
        cj = su_conjugator(theta, eta, delta);
        cji = su_matinv(cj);
    }

    var depth = get_state(xform_id, variation_id, 2u);
    var a3 = vec3<f32>(p.xy, p.z) / size;
    if (seed_mode != 0u && rng_nextf(rng) < 0.1) {
        var m: vec3<f32>;
        if (seed_mode == 1u) {
            let pick = min(u32(rng_nextf(rng) * 3.0), 2u);
            var w = A;
            if (pick == 1u) { w = B; } else if (pick == 2u) { w = C; }
            m = vd_lift(w);
        } else if (seed_mode == 2u) {
            let pick = min(u32(rng_nextf(rng) * 3.0), 2u);
            var u0 = A; var u1 = B;
            if (pick == 1u) { u0 = B; u1 = C; }
            else if (pick == 2u) { u0 = C; u1 = A; }
            let t = rng_nextf(rng);
            m = vd_hnorm(mix(vd_lift(u0), vd_lift(u1), t));
        } else {
            let u = rng_nextf(rng);
            let v = rng_nextf(rng) * (1.0 - u);
            let hA = vd_lift(A); let hB = vd_lift(B); let hC = vd_lift(C);
            m = vd_hnorm(hA + u * (hB - hA) + v * (hC - hA));
        }
        if (thickness > 0.0) {
            let ph = rng_nextf(rng) * 6.28318530718;
            let tt = thickness * sqrt(rng_nextf(rng));
            var e1 = vec3<f32>(1.0, 0.0, 0.0);
            e1 = e1 + vd_mdot(e1, m) * m;
            e1 = e1 / sqrt(max(vd_mdot(e1, e1), 1e-6));
            var e2 = vec3<f32>(0.0, 1.0, 0.0);
            e2 = e2 + vd_mdot(e2, m) * m;
            e2 = e2 - vd_mdot(e2, e1) * e1;
            e2 = e2 / sqrt(max(vd_mdot(e2, e2), 1e-6));
            let u = cos(ph) * e1 + sin(ph) * e2;
            m = cosh(tt) * m + sinh(tt) * u;
        }
        a3 = vec3<f32>(vd_proj(m), a3.z);
        depth = 0.0;
    } else {
        let r2 = dot(a3.xy, a3.xy);
        if (r2 >= 1.0 && space == 0u) { a3 = vec3<f32>(a3.xy / (r2 + 1e-9), a3.z); }
    }

    var prev = u32(get_state(xform_id, variation_id, 0u));
    var creg = get_state(xform_id, variation_id, 1u);
    for (var i = 0; i < steps; i = i + 1) {
        var k = min(u32(rng_nextf(rng) * f32(cnt)), cnt - 1u);
        var invp = prev;
        if (prev < 6u) { invp = (prev + 3u) % 6u; }
        if (prev < cnt && k == invp) { k = (k + 1u) % cnt; }
        if (k == 6u) {
            if (space == 1u) {
                if (deform) {
                    var t = su_apply_plain3(cji, a3);
                    t = vec3<f32>(t.x, -t.y, t.z);
                    a3 = su_apply_plain3(cj, t);
                } else {
                    a3 = vec3<f32>(a3.x, -a3.y, a3.z);
                }
            } else {
                if (deform) {
                    var t = su_apply_plain(cji, a3.xy);
                    t = vec2<f32>(t.x, -t.y);
                    a3 = vec3<f32>(su_apply_plain(cj, t), a3.z);
                } else {
                    a3 = vec3<f32>(a3.x, -a3.y, a3.z);
                }
            }
        } else {
            let g = vd_gen(k, A, B, C, sp, sq, sr);
            if (space == 1u) {
                if (deform) { a3 = su_apply_m3(g, a3, cj, cji); }
                else { a3 = su_apply_plain3(g, a3); }
            } else {
                if (deform) { a3 = vec3<f32>(su_apply_m(g, a3.xy, cj, cji), a3.z); }
                else { a3 = vec3<f32>(su_apply_plain(g, a3.xy), a3.z); }
            }
        }
        prev = k;
        if (dc_mode == 1u) {
            creg = mix(creg, fract((f32(k) + 0.5) / f32(cnt) * dc_scale), color_speed);
        }
    }
    set_state(xform_id, variation_id, 0u, f32(prev));
    depth = depth + f32(steps);
    set_state(xform_id, variation_id, 2u, depth);

    if (dc_mode == 1u) {
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    } else if (dc_mode == 2u) {
        let t = clamp(depth * color_speed / (10.0 * f32(steps)), 0.0, 1.0);
        *vc = fract(min(t * dc_scale, 0.999));
    }
    return a3 * size;
}
"#;
