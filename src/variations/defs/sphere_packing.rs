//! `sphere_packing` — Apollonian & ring sphere/circle packings via
//! inversions, with tangency-graph seeding (original).
//!
//! The genuinely 3D Kleinian: where the Möbius-family variations act by
//! SL(2,ℂ) on the plane (their limit sets are inherently planar), this
//! one acts by **sphere inversions in ℝ³** — honest 3D conformal maps —
//! and its limit sets are true 2D-fractal surfaces in space.
//!
//! Configurations (`mode`):
//! - **Apollonian (Dual)** — the exact packing: inversions in the five
//!   dual spheres of the symmetric Soddy configuration (outer unit
//!   sphere + four tetrahedral inner spheres of curvature 1+√(3/2);
//!   duals orthogonal to each 4-subset through their tangency points,
//!   verified numerically to ~1e-15). Limit set = the Apollonian
//!   sphere packing; the 2D body is the classic gasket one dimension
//!   down (outer circle + three 120° inner, four dual circles).
//! - **Tangent Spheres** — the five Soddy spheres themselves as
//!   mirrors: a sibling reflection group, denser residual fractal.
//! - **Ring** — a free family: N spheres around the equator, each
//!   kissing the outer sphere and (at Ring Scale 1) its neighbours
//!   (closed-form tangency: r = sin(π/N)/(1+sin(π/N))). Ring Scale
//!   shrinks/grows them (they slide along the outer sphere, staying
//!   tangent to it); Size Jitter de-uniformizes the radii
//!   per-sphere. Mirrors = outer + ring.
//! - **Ring + Caps** (3D) — adds two polar cap spheres, sized to kiss
//!   the outer sphere and the ring exactly (closed form, scaled by Cap
//!   Scale). In 2D this mode renders as Ring.
//!
//! Seeding (`seed` + `reseed`): the packing spheres are exact subsets
//! of the limit set for the tangent configurations, so planting points
//! on them renders crisply from the first plot (the von_dyck lesson).
//! - **Surfaces** — random point on a random configuration sphere.
//! - **Centers** — the sphere centers as vertices: the orbit stamps a
//!   fractal point cloud through the packing. (Inversions do not map
//!   centers to centers, so this is the centers' own orbit fractal —
//!   a constellation, not "every packed sphere's center".)
//! - **Edges** — the TANGENCY GRAPH: straight segments joining the
//!   centers of kissing spheres (the inner tetrahedron's 6 edges for
//!   Soddy; the ring cycle + cap spokes for rings), like honeycomb's
//!   edge skeleton. `thickness` fattens seeds into balls/tubes
//!   (Euclidean jitter — inversive geometry has no canonical metric).
//!
//! A sphere inversion is just `x ↦ c + r²(x−c)/|x−c|²` — no
//! quaternions, no shared Möbius lib needed.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Apollonian & ring sphere packings (3D) / circle packings (2D) via
/// inversions, with tangency-graph seeding — the genuinely 3D Kleinian.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static SPHERE_PACKING: VariationDef = VariationDef {
    name: "sphere_packing",
    aliases: &[],
    display_name: "Sphere Packing",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::AlwaysZ],
    // Slot 0: previous mirror index (inversions are involutions — block
    // immediate repeats). Slot 1: color register. Slot 2: generation
    // counter. Slots 3..6: the carried sphere (radius, center) that the
    // Curvature mode transports exactly through every inversion.
    state_count: 7,
    wgsl_state_init: None,
    parameters: &[
        param!("mode", "Mode", enum, 0, &["Apollonian (Dual)", "Tangent Spheres", "Ring", "Ring + Caps"], "The mirror configuration. Apollonian: inversions in the five dual spheres of the symmetric Soddy configuration — the limit set is the exact Apollonian sphere packing (gasket in 2D). Tangent Spheres: the five Soddy spheres themselves as mirrors — denser sibling fractal. Ring: N spheres around the equator kissing the outer sphere (and each other at Ring Scale 1) — a freely tunable packing family. Ring + Caps: adds two polar cap spheres kissing outer + ring (3D; renders as Ring in 2D)."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Radius of the outer sphere/circle in world units."),
        param!("avoid_repeat", "Avoid Repeat", bool, true, "Block choosing the same mirror twice in a row (an inversion is an involution, so a repeat cancels to the identity). Off = all mirrors equally likely every call."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Mirror", "Mirror Blend", "Curvature", "Generation", "Angle"], "Direct-color source (needs the transform's Direct Color > 0). Mirror: the last inversion applied — coarse flat regions (each mirror maps everything into its own ball; Color Speed unused). Mirror Blend: register pulled toward each mirror's slot after EVERY inversion — Color Speed is the region size (1 = coarse basins, lower = each step refines one hierarchy level). Curvature: the canonical Apollonian coloring — the carried sphere is transported through every inversion in closed form, so each sphere of the packing gets its own palette position by SIZE (needs Reseed > 0). Generation (uses Color Scale as palette cycles; ignores Color Speed, since spacing is derived from Reseed): palette sweeps with the number of inversions since the last reseed — colors by hierarchy depth. Angle: azimuth of the output point."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "Mirror Blend: blend rate = color region size (1 = coarse per-mirror basins, low = fine cells from deep itinerary). Generation: palette sweep rate per hierarchy level."),
        param!("steps", "Steps", int, 3.0, 1.0, 8.0, "Inversions applied per call (only the last point is plotted). Higher values let respawned points converge onto the packing before plotting — suppresses pre-convergence haze — at proportional GPU cost."),
        param!("reseed", "Reseed", float, 0.25, 0.0, 0.5, "Probability per call of planting the point on the seed geometry (see Seed) before the walk. The packing spheres are exact limit-set subsets for the tangent configurations, so seeded points render crisply from the first plot. 0 = pure feed-through chaos game."),
        param!("ring_n", "Ring N", int, 6.0, 2.0, 16.0, "Ring modes: number of spheres around the equator. At Ring Scale 1 neighbours kiss exactly (r = sin(π/N)/(1+sin(π/N)))."),
        param!("ring_scale", "Ring Scale", float, 1.0, 0.3, 1.5, "Ring modes: radius multiplier on the tangent ring size. Each sphere stays kissing the OUTER sphere (it slides outward as it shrinks); < 1 opens dust gaps between neighbours, > 1 overlaps them — different fractal regimes."),
        param!("cap_scale", "Cap Scale", float, 1.0, 0.0, 1.5, "Ring + Caps: radius multiplier on the polar cap spheres (exact outer+ring tangency at 1; 0 removes them)."),
        param!("size_jitter", "Size Jitter", float, 0.0, 0.0, 1.0, "Ring modes: deterministic per-sphere radius variation (each ring sphere shrinks by up to this fraction, hashed by index) — de-uniformizes the packing while every sphere keeps kissing the outer sphere."),
        param!("seed", "Seed", enum, 0, &["Surfaces", "Centers", "Edges"], "What Reseed plants. Surfaces: random point on a random configuration sphere (exact limit-set subset). Centers: the sphere centers as a vertex constellation — their orbit is its own fractal cloud. Edges: the tangency graph — straight segments joining centers of kissing spheres (the inner tetrahedron for Soddy, the ring cycle + cap spokes for rings), honeycomb-style."),
        param!("thickness", "Thickness", float, 0.0, 0.0, 0.5, "Euclidean jitter radius fattening Centers into balls and Edges into tubes."),
        param!("ring_tilt", "Ring Tilt", angle, 0.0, "Ring modes (3D): alternate spheres tilt up/down in latitude by this angle — an antiprism crown instead of a flat equatorial ring. Outer-sphere tangency is preserved exactly for any tilt; the sphere radius auto-shrinks to whichever is tighter of adjacent-pair tangency and SAME-parity-pair tangency (same-side spheres crowd toward the pole as tilt grows — without the cap the mirrors overlap and the group degenerates into blur). Even Ring N closes the crown perfectly; odd N leaves one seam pair."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Baked Soddy constants (verified in the CPU prototype):
// 3D: inner curvature 1+√(3/2) → r = 0.4494897, centers 0.5505103·tet/√3
//     = ±0.3178372 per component; duals D0 = (0|0.3178372),
//     D1..D4 = ∓√3·tet | 2√2.
// 2D: inner curvature 1+2/√3 → r = 0.4641016, centers 0.5358984 at
//     90°/210°/330°; duals D0 = (0|0.2679492), D1..D3 opposite, r = √3.
// Ring formulas (exact, hand-verified N=6): r_tan = sin(π/N)/(1+sin(π/N)),
// center distance d = 1 − r (kissing the outer sphere for any r);
// caps: ρ = (d²+1−r²)/(2(1+r)) at (0,0,±(1−ρ)).
const WGSL_2D: &str = r#"
fn sp_hash01(k: u32) -> f32 {
    return fract(sin(f32(k) * 127.1 + 311.7) * 43758.5453);
}

// Configuration (tangent/seed) circle k for the active mode.
fn sp_conf2(mode: u32, k: u32, n: u32, rs: f32, jit: f32) -> vec3<f32> {
    if (mode >= 2u) {
        if (k == 0u) { return vec3<f32>(0.0, 0.0, 1.0); }
        let i = k - 1u;
        let rt = sin(3.14159265359 / f32(n)) / (1.0 + sin(3.14159265359 / f32(n)));
        let r = rt * rs * (1.0 - jit * sp_hash01(i));
        let d = 1.0 - r;
        let a = 6.28318530718 * f32(i) / f32(n);
        return vec3<f32>(d * cos(a), d * sin(a), r);
    }
    // Soddy tangent circles.
    switch k {
        case 0u: { return vec3<f32>(0.0, 0.0, 1.0); }
        case 1u: { return vec3<f32>(0.0, 0.5358984, 0.4641016); }
        case 2u: { return vec3<f32>(-0.4641016, -0.2679492, 0.4641016); }
        default: { return vec3<f32>(0.4641016, -0.2679492, 0.4641016); }
    }
}

// Mirror circle k: Soddy duals for mode 0, else the configuration.
fn sp_mirror2(mode: u32, k: u32, n: u32, rs: f32, jit: f32) -> vec3<f32> {
    if (mode != 0u) { return sp_conf2(mode, k, n, rs, jit); }
    switch k {
        case 0u: { return vec3<f32>(0.0, 0.0, 0.2679492); }
        case 1u: { return vec3<f32>(0.0, -2.0, 1.7320508); }
        case 2u: { return vec3<f32>(1.7320508, 1.0, 1.7320508); }
        default: { return vec3<f32>(-1.7320508, 1.0, 1.7320508); }
    }
}

fn variation_sphere_packing(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let size = max(get_param(xform_id, variation_id, 1u), 1e-6);
    let avoid = get_param(xform_id, variation_id, 2u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 3u));
    let dc_scale = get_param(xform_id, variation_id, 4u);
    let color_speed = get_param(xform_id, variation_id, 5u);
    let steps = i32(get_param(xform_id, variation_id, 6u));
    let reseed = get_param(xform_id, variation_id, 7u);
    let ring_n = u32(clamp(get_param(xform_id, variation_id, 8u), 2.0, 16.0));
    let ring_scale = get_param(xform_id, variation_id, 9u);
    let jit = get_param(xform_id, variation_id, 11u);
    let seed_mode = u32(get_param(xform_id, variation_id, 12u));
    let thickness = get_param(xform_id, variation_id, 13u);

    // Configuration size (2D: caps mode renders as ring), mirror count.
    var scnt = 4u;
    if (mode >= 2u) { scnt = 1u + ring_n; }
    let mcnt = scnt;   // 2D: dual count (4) == tangent count (4)

    var x = p / size;
    // Carried circle (radius, center) transported through every
    // inversion for the Curvature coloring.
    var crad = get_state(xform_id, variation_id, 3u);
    var ccen = vec2<f32>(get_state(xform_id, variation_id, 4u), get_state(xform_id, variation_id, 5u));
    var depth = get_state(xform_id, variation_id, 2u);
    if (rng_nextf(rng) < reseed) {
        if (seed_mode == 0u) {
            // Surface of a random configuration circle.
            let cs = sp_conf2(mode, min(u32(rng_nextf(rng) * f32(scnt)), scnt - 1u), ring_n, ring_scale, jit);
            let a = rng_nextf(rng) * 6.28318530718;
            x = cs.xy + cs.z * vec2<f32>(cos(a), sin(a));
            crad = cs.z; ccen = cs.xy;
        } else if (seed_mode == 1u) {
            // Center vertex (skip the outer circle's center).
            let k = 1u + min(u32(rng_nextf(rng) * f32(scnt - 1u)), scnt - 2u);
            let cs = sp_conf2(mode, k, ring_n, ring_scale, jit);
            x = cs.xy;
            crad = cs.z; ccen = cs.xy;
        } else {
            // Tangency-graph edge: cycle of the inner circles.
            let inner = scnt - 1u;
            let e = min(u32(rng_nextf(rng) * f32(inner)), inner - 1u);
            let s0 = sp_conf2(mode, 1u + e, ring_n, ring_scale, jit);
            let c1 = sp_conf2(mode, 1u + (e + 1u) % inner, ring_n, ring_scale, jit).xy;
            x = mix(s0.xy, c1, rng_nextf(rng));
            crad = s0.z; ccen = s0.xy;
        }
        depth = 0.0;
        if (thickness > 0.0) {
            let a = rng_nextf(rng) * 6.28318530718;
            x = x + thickness * sqrt(rng_nextf(rng)) * vec2<f32>(cos(a), sin(a));
        }
    }

    var prev = u32(get_state(xform_id, variation_id, 0u));
    var creg = get_state(xform_id, variation_id, 1u);
    var k = 0u;
    for (var i = 0; i < steps; i = i + 1) {
        k = min(u32(rng_nextf(rng) * f32(mcnt)), mcnt - 1u);
        if (avoid && prev < mcnt && k == prev) { k = (k + 1u) % mcnt; }
        let cr = sp_mirror2(mode, k, ring_n, ring_scale, jit);
        let v = x - cr.xy;
        let nn = max(dot(v, v), 1e-12);
        x = cr.xy + (cr.z * cr.z / nn) * v;
        // Transport the carried circle: image center c_m + r²u/D,
        // radius r²ρ/|D|, u = c − c_m, D = |u|² − ρ².
        let u2 = ccen - cr.xy;
        let dd = dot(u2, u2) - crad * crad;
        let add = max(abs(dd), 1e-12);
        ccen = cr.xy + (cr.z * cr.z * sign(dd) / add) * u2;
        crad = cr.z * cr.z * crad / add;
        // Per-step itinerary blend: Color Speed = region size.
        creg = mix(creg, (f32(k) + 0.5) / f32(mcnt), color_speed);
        prev = k;
    }
    set_state(xform_id, variation_id, 0u, f32(prev));
    depth = depth + f32(steps);
    set_state(xform_id, variation_id, 2u, depth);
    set_state(xform_id, variation_id, 3u, crad);
    set_state(xform_id, variation_id, 4u, ccen.x);
    set_state(xform_id, variation_id, 5u, ccen.y);

    if (dc_mode == 1u) {
        *vc = fract((f32(k) + 0.5) / f32(mcnt) * dc_scale);
    } else if (dc_mode == 2u) {
        set_state(xform_id, variation_id, 1u, creg);
        *vc = fract(creg * dc_scale);
    } else if (dc_mode == 3u) {
        *vc = fract(0.5 + 0.15 * dc_scale * log(1.0 / max(crad, 1e-9)));
    } else if (dc_mode == 4u) {
        // Generation: palette position by POPULATION, not by a linear
        // ramp in generation count.
        //
        // A point reseeds with probability `reseed` per call, so the
        // generation index g = depth/steps is geometrically
        // distributed: at Reseed 0.5 half of ALL samples are g = 1, a
        // quarter g = 2, and so on. A linear ramp therefore crushed
        // ~95% of the image into a sliver of palette near the start
        // (the reported "extremely limited range") no matter how the
        // sliders were set — cranking Color Scale only aliased the
        // first few generations on top of each other.
        //
        // Mapping g to the MIDPOINT of its cumulative-probability
        // interval [F(g-1), F(g)] = 1 - q^(g-1)(1+q)/2, q = 1-reseed,
        // gives each generation its own palette slot spaced by how
        // many samples it actually holds. The full palette is used at
        // any Reseed setting, and the dominant generations land on
        // well-separated colors instead of neighbouring shades.
        let g = max(depth / max(f32(steps), 1.0), 1.0);
        let q = 1.0 - clamp(reseed, 0.02, 1.0);
        let t = 1.0 - pow(q, g - 1.0) * (1.0 + q) * 0.5;
        *vc = fract(t * dc_scale);
    } else if (dc_mode == 5u) {
        *vc = fract((atan2(x.y, x.x) * 0.15915494 + 0.5) * dc_scale);
    }
    return x * size;
}
"#;

const WGSL_3D: &str = r#"
fn sp_hash01(k: u32) -> f32 {
    return fract(sin(f32(k) * 127.1 + 311.7) * 43758.5453);
}

// Configuration (tangent/seed) sphere k for the active mode.
// Ring layout: 0 = outer, 1..N = ring, N+1 / N+2 = caps (mode 3).
fn sp_conf3(mode: u32, k: u32, n: u32, rs: f32, cs: f32, jit: f32, tilt: f32) -> vec4<f32> {
    if (mode >= 2u) {
        if (k == 0u) { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
        // Tangent radius from the TRUE angle between adjacent centers:
        // alternating +-tilt latitudes give cos(theta) =
        // cos^2(tilt)*cos(2pi/N) - sin^2(tilt); r = sin(theta/2)/(1+sin(theta/2)).
        let ct = cos(tilt);
        let st = sin(tilt);
        // Adjacent (opposite-latitude) pair angle.
        let cth = ct * ct * cos(6.28318530718 / f32(n)) - st * st;
        let sh2 = sqrt(max(0.5 * (1.0 - cth), 1e-6));
        var rt = sh2 / (1.0 + sh2);
        if (n >= 3u) {
            // Same-parity pair (i, i+2) — both tilted the SAME way, so
            // they crowd toward the pole as tilt grows. Cap the radius
            // at their tangency too: overlapping mirrors make the
            // reflection group non-discrete (dense blur).
            let cth2 = ct * ct * cos(12.5663706144 / f32(n)) + st * st;
            let shp = sqrt(max(0.5 * (1.0 - cth2), 1e-6));
            rt = min(rt, shp / (1.0 + shp));
        }
        if (mode == 3u && k > n) {
            // Polar caps: kiss the outer sphere and the up-tilted ring
            // spheres exactly (reduces to the flat formula at tilt 0).
            let rn = rt * rs;
            let dn = 1.0 - rn;
            let rho = (dn * dn + 1.0 - rn * rn - 2.0 * dn * st) / (2.0 * max(1.0 + rn - dn * st, 1e-4));
            let cr = max(rho * cs, 1e-4);
            let h = 1.0 - cr;
            let sgn = select(1.0, -1.0, k == n + 2u);
            return vec4<f32>(0.0, 0.0, sgn * h, cr);
        }
        let i = k - 1u;
        let r = rt * rs * (1.0 - jit * sp_hash01(i));
        let d = 1.0 - r;
        let a = 6.28318530718 * f32(i) / f32(n);
        let ph = select(tilt, -tilt, (i & 1u) == 1u);
        return vec4<f32>(d * cos(a) * cos(ph), d * sin(a) * cos(ph), d * sin(ph), r);
    }
    // Soddy tangent spheres: outer + tetrahedral inner.
    switch k {
        case 0u: { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
        case 1u: { return vec4<f32>(0.3178372, 0.3178372, 0.3178372, 0.4494897); }
        case 2u: { return vec4<f32>(0.3178372, -0.3178372, -0.3178372, 0.4494897); }
        case 3u: { return vec4<f32>(-0.3178372, 0.3178372, -0.3178372, 0.4494897); }
        default: { return vec4<f32>(-0.3178372, -0.3178372, 0.3178372, 0.4494897); }
    }
}

// Mirror sphere k: Soddy duals for mode 0, else the configuration.
fn sp_mirror3(mode: u32, k: u32, n: u32, rs: f32, cs: f32, jit: f32, tilt: f32) -> vec4<f32> {
    if (mode != 0u) { return sp_conf3(mode, k, n, rs, cs, jit, tilt); }
    switch k {
        case 0u: { return vec4<f32>(0.0, 0.0, 0.0, 0.3178372); }
        case 1u: { return vec4<f32>(-1.7320508, -1.7320508, -1.7320508, 2.8284271); }
        case 2u: { return vec4<f32>(-1.7320508, 1.7320508, 1.7320508, 2.8284271); }
        case 3u: { return vec4<f32>(1.7320508, -1.7320508, 1.7320508, 2.8284271); }
        default: { return vec4<f32>(1.7320508, 1.7320508, -1.7320508, 2.8284271); }
    }
}

// Tangency-graph edge e as a pair of configuration indices.
// Soddy: the 6 inner-tetrahedron edges. Ring: the N-cycle; Ring+Caps
// adds 2N cap spokes.
fn sp_edge3(mode: u32, e: u32, n: u32) -> vec2<u32> {
    if (mode >= 2u) {
        if (mode == 3u && e >= n) {
            let i = e - n;
            let cap = n + 1u + (i / n);
            return vec2<u32>(cap, 1u + (i % n));
        }
        return vec2<u32>(1u + e, 1u + (e + 1u) % n);
    }
    switch e {
        case 0u: { return vec2<u32>(1u, 2u); }
        case 1u: { return vec2<u32>(1u, 3u); }
        case 2u: { return vec2<u32>(1u, 4u); }
        case 3u: { return vec2<u32>(2u, 3u); }
        case 4u: { return vec2<u32>(2u, 4u); }
        default: { return vec2<u32>(3u, 4u); }
    }
}

fn variation_sphere_packing(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let size = max(get_param(xform_id, variation_id, 1u), 1e-6);
    let avoid = get_param(xform_id, variation_id, 2u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 3u));
    let dc_scale = get_param(xform_id, variation_id, 4u);
    let color_speed = get_param(xform_id, variation_id, 5u);
    let steps = i32(get_param(xform_id, variation_id, 6u));
    let reseed = get_param(xform_id, variation_id, 7u);
    let ring_n = u32(clamp(get_param(xform_id, variation_id, 8u), 2.0, 16.0));
    let ring_scale = get_param(xform_id, variation_id, 9u);
    let cap_scale = get_param(xform_id, variation_id, 10u);
    let jit = get_param(xform_id, variation_id, 11u);
    let seed_mode = u32(get_param(xform_id, variation_id, 12u));
    let thickness = get_param(xform_id, variation_id, 13u);
    let tilt = get_param(xform_id, variation_id, 14u) * 0.01745329252;

    // Configuration and mirror counts, edge count.
    var scnt = 5u;
    if (mode == 2u) { scnt = 1u + ring_n; }
    else if (mode == 3u) { scnt = 3u + ring_n; }
    let mcnt = scnt;   // mode 0: dual count (5) == tangent count (5)
    var ecnt = 6u;
    if (mode == 2u) { ecnt = ring_n; }
    else if (mode == 3u) { ecnt = 3u * ring_n; }

    var x = p / size;
    // Carried sphere (radius, center) transported through every
    // inversion for the Curvature coloring.
    var crad = get_state(xform_id, variation_id, 3u);
    var ccen = vec3<f32>(get_state(xform_id, variation_id, 4u), get_state(xform_id, variation_id, 5u), get_state(xform_id, variation_id, 6u));
    var depth = get_state(xform_id, variation_id, 2u);
    if (rng_nextf(rng) < reseed) {
        if (seed_mode == 0u) {
            let sp = sp_conf3(mode, min(u32(rng_nextf(rng) * f32(scnt)), scnt - 1u), ring_n, ring_scale, cap_scale, jit, tilt);
            let za = rng_nextf(rng) * 2.0 - 1.0;
            let ph = rng_nextf(rng) * 6.28318530718;
            let sa = sqrt(max(1.0 - za * za, 0.0));
            x = sp.xyz + sp.w * vec3<f32>(sa * cos(ph), sa * sin(ph), za);
            crad = sp.w; ccen = sp.xyz;
        } else if (seed_mode == 1u) {
            let k = 1u + min(u32(rng_nextf(rng) * f32(scnt - 1u)), scnt - 2u);
            let sp = sp_conf3(mode, k, ring_n, ring_scale, cap_scale, jit, tilt);
            x = sp.xyz;
            crad = sp.w; ccen = sp.xyz;
        } else {
            let e = min(u32(rng_nextf(rng) * f32(ecnt)), ecnt - 1u);
            let ij = sp_edge3(mode, e, ring_n);
            let s0 = sp_conf3(mode, ij.x, ring_n, ring_scale, cap_scale, jit, tilt);
            let c1 = sp_conf3(mode, ij.y, ring_n, ring_scale, cap_scale, jit, tilt).xyz;
            x = mix(s0.xyz, c1, rng_nextf(rng));
            crad = s0.w; ccen = s0.xyz;
        }
        depth = 0.0;
        if (thickness > 0.0) {
            let za = rng_nextf(rng) * 2.0 - 1.0;
            let ph = rng_nextf(rng) * 6.28318530718;
            let sa = sqrt(max(1.0 - za * za, 0.0));
            x = x + thickness * pow(rng_nextf(rng), 0.3333333) * vec3<f32>(sa * cos(ph), sa * sin(ph), za);
        }
    }

    var prev = u32(get_state(xform_id, variation_id, 0u));
    var creg = get_state(xform_id, variation_id, 1u);
    var k = 0u;
    for (var i = 0; i < steps; i = i + 1) {
        k = min(u32(rng_nextf(rng) * f32(mcnt)), mcnt - 1u);
        if (avoid && prev < mcnt && k == prev) { k = (k + 1u) % mcnt; }
        let sp = sp_mirror3(mode, k, ring_n, ring_scale, cap_scale, jit, tilt);
        let v = x - sp.xyz;
        let nn = max(dot(v, v), 1e-12);
        x = sp.xyz + (sp.w * sp.w / nn) * v;
        let u3 = ccen - sp.xyz;
        let dd = dot(u3, u3) - crad * crad;
        let add = max(abs(dd), 1e-12);
        ccen = sp.xyz + (sp.w * sp.w * sign(dd) / add) * u3;
        crad = sp.w * sp.w * crad / add;
        creg = mix(creg, (f32(k) + 0.5) / f32(mcnt), color_speed);
        prev = k;
    }
    set_state(xform_id, variation_id, 0u, f32(prev));
    depth = depth + f32(steps);
    set_state(xform_id, variation_id, 2u, depth);
    set_state(xform_id, variation_id, 3u, crad);
    set_state(xform_id, variation_id, 4u, ccen.x);
    set_state(xform_id, variation_id, 5u, ccen.y);
    set_state(xform_id, variation_id, 6u, ccen.z);

    if (dc_mode == 1u) {
        *vc = fract((f32(k) + 0.5) / f32(mcnt) * dc_scale);
    } else if (dc_mode == 2u) {
        set_state(xform_id, variation_id, 1u, creg);
        *vc = fract(creg * dc_scale);
    } else if (dc_mode == 3u) {
        *vc = fract(0.5 + 0.15 * dc_scale * log(1.0 / max(crad, 1e-9)));
    } else if (dc_mode == 4u) {
        // Generation: palette position by POPULATION, not by a linear
        // ramp in generation count.
        //
        // A point reseeds with probability `reseed` per call, so the
        // generation index g = depth/steps is geometrically
        // distributed: at Reseed 0.5 half of ALL samples are g = 1, a
        // quarter g = 2, and so on. A linear ramp therefore crushed
        // ~95% of the image into a sliver of palette near the start
        // (the reported "extremely limited range") no matter how the
        // sliders were set — cranking Color Scale only aliased the
        // first few generations on top of each other.
        //
        // Mapping g to the MIDPOINT of its cumulative-probability
        // interval [F(g-1), F(g)] = 1 - q^(g-1)(1+q)/2, q = 1-reseed,
        // gives each generation its own palette slot spaced by how
        // many samples it actually holds. The full palette is used at
        // any Reseed setting, and the dominant generations land on
        // well-separated colors instead of neighbouring shades.
        let g = max(depth / max(f32(steps), 1.0), 1.0);
        let q = 1.0 - clamp(reseed, 0.02, 1.0);
        let t = 1.0 - pow(q, g - 1.0) * (1.0 + q) * 0.5;
        *vc = fract(t * dc_scale);
    } else if (dc_mode == 5u) {
        *vc = fract((atan2(x.y, x.x) * 0.15915494 + 0.5) * dc_scale);
    }
    return x * size;
}
"#;
