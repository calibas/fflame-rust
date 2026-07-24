//! `surface_group` — genus-2 surface group: octagon tiling with
//! quasi-Fuchsian bend (original).
//!
//! The fundamental group of a genus-2 surface, uniformized as the
//! side-pairing group of the regular hyperbolic octagon of the {8,8}
//! tiling (vertex angle π/4, all eight corners one vertex cycle).
//! Opposite sides are paired by four hyperbolic translations
//!
//! ```text
//! g_k = R(kπ/4) · T · R(−kπ/4),   k = 0..3
//! T = [[cosh c, sinh c], [sinh c, cosh c]],   c = arccosh(cot π/8) ≈ 1.5286
//! ```
//!
//! (trace 2·(1+√2); the genus-2 relation
//! `g₀g₁⁻¹g₂g₃⁻¹g₀⁻¹g₁g₂⁻¹g₃ = I` verified numerically). Chaos game
//! over the eight elements with honeycomb-style seed machinery
//! (vertex orbit at radius tanh(R_v/2) ≈ 0.841, edge skeleton, face
//! fan; geodesic `thickness`) stamps the octagon tiling across the
//! disk, as in [`von_dyck`](super::von_dyck).
//!
//! **Why this group and not a triangle group: it deforms.** Triangle
//! groups are rigid, but a genus-2 surface group has a genuine
//! 6-dimensional Teichmüller space and a 12-dimensional quasi-Fuchsian
//! deformation space in SL(2,ℂ). The Bend sliders make the pairing
//! translation complex, `c → c + bend_re + i·bend_im`: at 0 the exact
//! Fuchsian octagon tiling (limit set a round circle); small imaginary
//! bend pushes the representation off the Fuchsian locus and the
//! orbit's boundary fragments into fractal quasi-circle-like dust
//! while the interior tiling shears — the tessellation and the
//! Kleinian fractal as two faces of one group. (The uniform bend
//! applied to all four pairings is a slice through the representation
//! variety, not a certified quasi-Fuchsian path — discreteness fails
//! off a measure-zero-boundary set, and the chaos game renders the
//! resulting near-orbits regardless, exactly as Bagula's loose groups
//! do.)
//!
//! `space` = Hyperbolic H3: the Poincaré extension — seeds are planted
//! on the hemisphere dome over the unit circle (where H2 embeds in
//! upper half-space), so the octagon tiling drapes over a curved dome;
//! bent groups act genuinely 3-dimensionally.
//!
//! Uses `Feature::NeedsMobiusLib` (`shaders/core/su_mobius.wgsl`).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Genus-2 surface group: the octagon tiling with a quasi-Fuchsian bend
/// into SL(2,ℂ).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static SURFACE_GROUP: VariationDef = VariationDef {
    name: "surface_group",
    aliases: &[],
    display_name: "Surface Group",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::NeedsMobiusLib, Feature::AlwaysZ],
    // Slot 0: previous generator index. Slot 1: color register.
    // Slot 2: walk depth.
    state_count: 3,
    wgsl_state_init: None,
    parameters: &[
        param!("bend_re", "Bend re", unlimited_float, 0.0, -1.0, 1.0, "Real part added to the pairing translation length c = arccosh(cot π/8). 0 is the exact genus-2 octagon group; nonzero changes the octagon's side length (the corners no longer close up exactly — cone-angle shear in the tiling)."),
        param!("bend_im", "Bend im", unlimited_float, 0.0, -1.0, 1.0, "Imaginary part of the pairing translation — the quasi-Fuchsian-style bend into SL(2,ℂ). 0 keeps the group Fuchsian (limit set a round circle); small values fragment the boundary into fractal quasi-circle dust while the interior tiling shears. Large values leave discreteness entirely — wilder, denser orbits."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Radius of the Poincaré disk in world units."),
        param!("steps", "Steps", int, 2.0, 1.0, 8.0, "Group elements applied per call (backtrack-avoiding random walk)."),
        param!("seed", "Seed", enum, 0, &["Input", "Vertices", "Edges", "Faces"], "What the walk stamps through the tiling: the incoming flame measure, the octagon's vertex orbit (8 corners, one orbit), its edge skeleton, or face-fan fragments."),
        param!("thickness", "Thickness", float, 0.0, 0.0, 2.0, "Seed modes: geodesic tangent-space offset by exact hyperbolic distance — balls, tubes, slabs of uniform hyperbolic radius."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Generator", "Steps"], "Direct-color source (needs the transform's Direct Color > 0). Generator: each of the 8 side-pairings has its own palette position, blended through a persistent register at Color Speed. Steps: palette cycles with the walk depth since the last reseed (wraps, so deep levels stay distinct)."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "Generator mode: pull strength toward each pairing's palette position. Steps mode: palette advance per reflection (cyclic — wraps instead of saturating)."),
        param!("space", "Space", enum, 0, &["Euclidean", "Hyperbolic H3"], "3D render mode only. Euclidean: the disk tiling in the xy plane (z passes through). Hyperbolic H3: the Poincaré extension — seeds plant on the hemisphere dome over the unit circle, draping the octagon tiling over a curved dome; bent (complex) groups act genuinely 3-dimensionally."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Constants: c = arccosh(cot(pi/8)) = 1.5285709 (pairing half-length),
// vertex Euclidean radius tanh(R_v/2) = 0.8408964 at angles (2k+1)pi/8.
// Helper block duplicated into both bodies (one compiles per flame).
const WGSL_2D: &str = r#"
fn sg_mdot(a: vec3<f32>, b: vec3<f32>) -> f32 {
    return a.x * b.x + a.y * b.y - a.z * b.z;
}

fn sg_lift(u: vec2<f32>) -> vec3<f32> {
    let den = max(1.0 - dot(u, u), 1e-6);
    return vec3<f32>(2.0 * u / den, (1.0 + dot(u, u)) / den);
}

fn sg_proj(h: vec3<f32>) -> vec2<f32> {
    return h.xy / (1.0 + max(h.z, 1.0));
}

fn sg_hnorm(v: vec3<f32>) -> vec3<f32> {
    return v / sqrt(max(-sg_mdot(v, v), 1e-6));
}

fn sg_vertex(k: u32) -> vec2<f32> {
    let th = (2.0 * f32(k) + 1.0) * 0.39269908;   // (2k+1) pi/8
    return 0.8408964 * vec2<f32>(cos(th), sin(th));
}

// Side-pairing k (0..3) or its inverse (4..7):
// R(k pi/4) T(±c) R(-k pi/4) = [[cosh c, e^{i k pi/4} sinh c],
//                               [e^{-i k pi/4} sinh c, cosh c]]
// with complex c = 1.5285709 + bend (inverse: negate c).
fn sg_gen(k: u32, bend: vec2<f32>) -> SuMat {
    let base = k % 4u;
    let sgn = select(1.0, -1.0, k >= 4u);
    let c = sgn * (vec2<f32>(1.5285709, 0.0) + bend);
    let ch = vec2<f32>(cosh(c.x) * cos(c.y), sinh(c.x) * sin(c.y));
    let sh = vec2<f32>(sinh(c.x) * cos(c.y), cosh(c.x) * sin(c.y));
    let th = f32(base) * 0.78539816;   // k pi/4
    let e = vec2<f32>(cos(th), sin(th));
    let ei = vec2<f32>(cos(th), -sin(th));
    return SuMat(ch, cmul(e, sh), cmul(ei, sh), ch);
}

fn variation_surface_group(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let bend = vec2<f32>(get_param(xform_id, variation_id, 0u), get_param(xform_id, variation_id, 1u));
    let size = max(get_param(xform_id, variation_id, 2u), 1e-6);
    let steps = i32(get_param(xform_id, variation_id, 3u));
    let seed_mode = u32(get_param(xform_id, variation_id, 4u));
    let thickness = get_param(xform_id, variation_id, 5u);
    let dc_mode = u32(get_param(xform_id, variation_id, 6u));
    let dc_scale = get_param(xform_id, variation_id, 7u);
    let color_speed = get_param(xform_id, variation_id, 8u);

    var depth = get_state(xform_id, variation_id, 2u);
    var a: vec2<f32>;
    if (seed_mode != 0u && rng_nextf(rng) < 0.1) {
        var m: vec3<f32>;
        if (seed_mode == 1u) {
            m = sg_lift(sg_vertex(min(u32(rng_nextf(rng) * 8.0), 7u)));
        } else if (seed_mode == 2u) {
            let k = min(u32(rng_nextf(rng) * 8.0), 7u);
            let t = rng_nextf(rng);
            m = sg_hnorm(mix(sg_lift(sg_vertex(k)), sg_lift(sg_vertex((k + 1u) % 8u)), t));
        } else {
            // Face fan: center + adjacent vertex pair.
            let k = min(u32(rng_nextf(rng) * 8.0), 7u);
            let u = rng_nextf(rng);
            let v = rng_nextf(rng) * (1.0 - u);
            let h0 = vec3<f32>(0.0, 0.0, 1.0);
            let h1 = sg_lift(sg_vertex(k));
            let h2 = sg_lift(sg_vertex((k + 1u) % 8u));
            m = sg_hnorm(h0 + u * (h1 - h0) + v * (h2 - h0));
        }
        if (thickness > 0.0) {
            let ph = rng_nextf(rng) * 6.28318530718;
            let tt = thickness * sqrt(rng_nextf(rng));
            var e1 = vec3<f32>(1.0, 0.0, 0.0);
            e1 = e1 + sg_mdot(e1, m) * m;
            e1 = e1 / sqrt(max(sg_mdot(e1, e1), 1e-6));
            var e2 = vec3<f32>(0.0, 1.0, 0.0);
            e2 = e2 + sg_mdot(e2, m) * m;
            e2 = e2 - sg_mdot(e2, e1) * e1;
            e2 = e2 / sqrt(max(sg_mdot(e2, e2), 1e-6));
            let u = cos(ph) * e1 + sin(ph) * e2;
            m = cosh(tt) * m + sinh(tt) * u;
        }
        a = sg_proj(m);
        depth = 0.0;
    } else {
        a = p / size;
        let r2 = dot(a, a);
        if (r2 >= 1.0) { a = a / (r2 + 1e-9); }
    }

    var prev = u32(get_state(xform_id, variation_id, 0u));
    var creg = get_state(xform_id, variation_id, 1u);
    for (var i = 0; i < steps; i = i + 1) {
        var k = min(u32(rng_nextf(rng) * 8.0), 7u);
        if (prev < 8u && k == (prev + 4u) % 8u) { k = (k + 1u) % 8u; }
        a = su_apply_plain(sg_gen(k, bend), a);
        prev = k;
        if (dc_mode == 1u) {
            creg = mix(creg, fract((f32(k) + 0.5) / 8.0 * dc_scale), color_speed);
        }
    }
    set_state(xform_id, variation_id, 0u, f32(prev));
    depth = depth + f32(steps);
    set_state(xform_id, variation_id, 2u, depth);

    if (dc_mode == 1u) {
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    } else if (dc_mode == 2u) {
        // Cyclic: advances and WRAPS with walk depth, so adjacent
        // depths stay distinct even in a narrow palette (the old
        // saturating sweep pinned all deep detail to the palette end).
        *vc = fract(depth * color_speed * 0.1 * dc_scale);
    }
    return a * size;
}
"#;

const WGSL_3D: &str = r#"
fn sg_mdot(a: vec3<f32>, b: vec3<f32>) -> f32 {
    return a.x * b.x + a.y * b.y - a.z * b.z;
}

fn sg_lift(u: vec2<f32>) -> vec3<f32> {
    let den = max(1.0 - dot(u, u), 1e-6);
    return vec3<f32>(2.0 * u / den, (1.0 + dot(u, u)) / den);
}

fn sg_proj(h: vec3<f32>) -> vec2<f32> {
    return h.xy / (1.0 + max(h.z, 1.0));
}

fn sg_hnorm(v: vec3<f32>) -> vec3<f32> {
    return v / sqrt(max(-sg_mdot(v, v), 1e-6));
}

fn sg_vertex(k: u32) -> vec2<f32> {
    let th = (2.0 * f32(k) + 1.0) * 0.39269908;
    return 0.8408964 * vec2<f32>(cos(th), sin(th));
}

fn sg_gen(k: u32, bend: vec2<f32>) -> SuMat {
    let base = k % 4u;
    let sgn = select(1.0, -1.0, k >= 4u);
    let c = sgn * (vec2<f32>(1.5285709, 0.0) + bend);
    let ch = vec2<f32>(cosh(c.x) * cos(c.y), sinh(c.x) * sin(c.y));
    let sh = vec2<f32>(sinh(c.x) * cos(c.y), cosh(c.x) * sin(c.y));
    let th = f32(base) * 0.78539816;
    let e = vec2<f32>(cos(th), sin(th));
    let ei = vec2<f32>(cos(th), -sin(th));
    return SuMat(ch, cmul(e, sh), cmul(ei, sh), ch);
}

fn variation_surface_group(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let bend = vec2<f32>(get_param(xform_id, variation_id, 0u), get_param(xform_id, variation_id, 1u));
    let size = max(get_param(xform_id, variation_id, 2u), 1e-6);
    let steps = i32(get_param(xform_id, variation_id, 3u));
    let seed_mode = u32(get_param(xform_id, variation_id, 4u));
    let thickness = get_param(xform_id, variation_id, 5u);
    let dc_mode = u32(get_param(xform_id, variation_id, 6u));
    let dc_scale = get_param(xform_id, variation_id, 7u);
    let color_speed = get_param(xform_id, variation_id, 8u);
    let space = u32(get_param(xform_id, variation_id, 9u));

    var depth = get_state(xform_id, variation_id, 2u);
    var a3 = vec3<f32>(p.xy, p.z) / size;
    if (seed_mode != 0u && rng_nextf(rng) < 0.1) {
        var m: vec3<f32>;
        if (seed_mode == 1u) {
            m = sg_lift(sg_vertex(min(u32(rng_nextf(rng) * 8.0), 7u)));
        } else if (seed_mode == 2u) {
            let k = min(u32(rng_nextf(rng) * 8.0), 7u);
            let t = rng_nextf(rng);
            m = sg_hnorm(mix(sg_lift(sg_vertex(k)), sg_lift(sg_vertex((k + 1u) % 8u)), t));
        } else {
            let k = min(u32(rng_nextf(rng) * 8.0), 7u);
            let u = rng_nextf(rng);
            let v = rng_nextf(rng) * (1.0 - u);
            let h0 = vec3<f32>(0.0, 0.0, 1.0);
            let h1 = sg_lift(sg_vertex(k));
            let h2 = sg_lift(sg_vertex((k + 1u) % 8u));
            m = sg_hnorm(h0 + u * (h1 - h0) + v * (h2 - h0));
        }
        if (thickness > 0.0) {
            let ph = rng_nextf(rng) * 6.28318530718;
            let tt = thickness * sqrt(rng_nextf(rng));
            var e1 = vec3<f32>(1.0, 0.0, 0.0);
            e1 = e1 + sg_mdot(e1, m) * m;
            e1 = e1 / sqrt(max(sg_mdot(e1, e1), 1e-6));
            var e2 = vec3<f32>(0.0, 1.0, 0.0);
            e2 = e2 + sg_mdot(e2, m) * m;
            e2 = e2 - sg_mdot(e2, e1) * e1;
            e2 = e2 / sqrt(max(sg_mdot(e2, e2), 1e-6));
            let u = cos(ph) * e1 + sin(ph) * e2;
            m = cosh(tt) * m + sinh(tt) * u;
        }
        if (space == 1u) {
            // H3: seed on the hemisphere dome (hyperboloid / time
            // component) — restores height every reseed; see von_dyck.
            a3 = vec3<f32>(m.xy / m.z, 1.0 / m.z);
        } else {
            a3 = vec3<f32>(sg_proj(m), a3.z);
        }
        depth = 0.0;
    } else {
        let r2 = dot(a3.xy, a3.xy);
        if (r2 >= 1.0 && space == 0u) { a3 = vec3<f32>(a3.xy / (r2 + 1e-9), a3.z); }
    }

    var prev = u32(get_state(xform_id, variation_id, 0u));
    var creg = get_state(xform_id, variation_id, 1u);
    for (var i = 0; i < steps; i = i + 1) {
        var k = min(u32(rng_nextf(rng) * 8.0), 7u);
        if (prev < 8u && k == (prev + 4u) % 8u) { k = (k + 1u) % 8u; }
        let g = sg_gen(k, bend);
        if (space == 1u) { a3 = su_apply_plain3(g, a3); }
        else { a3 = vec3<f32>(su_apply_plain(g, a3.xy), a3.z); }
        prev = k;
        if (dc_mode == 1u) {
            creg = mix(creg, fract((f32(k) + 0.5) / 8.0 * dc_scale), color_speed);
        }
    }
    set_state(xform_id, variation_id, 0u, f32(prev));
    depth = depth + f32(steps);
    set_state(xform_id, variation_id, 2u, depth);

    if (dc_mode == 1u) {
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    } else if (dc_mode == 2u) {
        // Cyclic: advances and WRAPS with walk depth, so adjacent
        // depths stay distinct even in a narrow palette (the old
        // saturating sweep pinned all deep detail to the palette end).
        *vc = fract(depth * color_speed * 0.1 * dc_scale);
    }
    return a3 * size;
}
"#;
