//! `cymatics3d` — 3D standing-wave cymatics attractor (original).
//!
//! Extends the [`chladni`](super::chladni) idea into the third
//! dimension: the standing-wave field of a rectangular resonator with
//! mode numbers (m, n, l) is the three-term cyclic superposition
//!
//! ```text
//! f(x,y,z) = a·s(m,x)·s(n,y)·s(l,z)
//!          + b·s(n,x)·s(l,y)·s(m,z)
//!          + c·s(l,x)·s(m,y)·s(n,z)
//! ```
//!
//! where the basis `s(k,t)` is selected by the `walls` parameter:
//! `cos(π·k·t/size)` (free / antinode walls, the default) or
//! `sin(π·k·t/size)` (fixed / node walls). The distinction matters: in
//! the sine basis every term carries a sine of every coordinate, so
//! the coordinate planes and their period grid (`x = j·size`, …) are
//! identically nodal — the attractor dutifully collects points onto
//! big flat planes through the origin. The cosine basis has antinodes
//! there instead, leaving a nodal set of purely curved surfaces (and
//! it matches real center-driven Chladni plates, which have free
//! edges).
//!
//! **Nodal Surface** mode Newton-projects points onto `f = 0` in 3D —
//! the chaos game condenses onto the nodal *surfaces* of the resonator
//! (the 3D analogue of sand collecting on a plate's nodal lines), and
//! the interference between the cyclically-permuted terms bends those
//! surfaces the same way the two-term mix bends the 2D figures. Pairs
//! naturally with solid rendering + lighting.
//!
//! **Heightfield** mode is the water-surface look instead: it leaves
//! x/y alone and pulls z toward the 2D plate field
//! `a·s(m,x)·s(n,y) + b·s(n,x)·s(m,y)` — a standing Faraday-wave
//! surface. The z write is a strength-blend toward the surface, not an
//! additive offset — the chaos game applies a transform to its own
//! output arbitrarily many times in a row, and an additive offset with
//! pass-through x/y accumulates without bound. Compose with
//! [`chladni`](super::chladni) on the same transform to also gather
//! points onto the wave's nodal lines.
//!
//! **Slicing** (`slice_z` + `slice_thickness`): in 2D render mode the
//! variation shows the planar slice of the 3D nodal set at
//! `z = slice_z`; thickness > 0 samples each point's slice z within
//! the slab, superimposing nearby slices like an X-ray projection.
//! In 3D Nodal Surface mode thickness > 0 confines the attractor to
//! the z-slab via alternating projection (Newton step, then z-clamp —
//! each iteration ends with the clamp so results stay inside);
//! thickness 0 leaves the full volume unrestricted. Heightfield has
//! nothing to slice and passes through in 2D. With sine walls the
//! `z = 0` plane is itself nodal, so a zero slice degenerates to
//! "everything is a node"; the cosine default has no such planes.
//!
//! Direct color: *Distance* and *Amplitude* (same semantics as
//! `chladni`; Heightfield's Amplitude colors by wave height), plus
//! *Depth* — colors by the output z (the sampled slice z in 2D), which
//! reveals depth structure and colors slab slices by position.
//!
//! No JWildfire/Apophysis equivalent — original to this project.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static CYMATICS3D: VariationDef = VariationDef {
    name: "cymatics3d",
    aliases: &[],
    display_name: "Cymatics 3D",
    // Advanced2D (not Full3D) deliberately: Full3D variations are
    // filtered out of 2D shaders (shader_builder_v2 active-set filter),
    // and the 2D slice body is a first-class feature of this variation.
    // AlwaysZ still gives true-3D z semantics in 3D mode, and it keeps
    // the cymatics family (chladni, chladni_disc) together in the UI.
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("m", "Mode M", float, 3.0, 1.0, 20.0, "First mode number of the resonator. Integer values give clean standing-wave structure; fractional values morph between figures."),
        param!("n", "Mode N", float, 5.0, 1.0, 20.0, "Second mode number."),
        param!("l", "Mode L", float, 2.0, 1.0, 20.0, "Third mode number. The three modes are cycled across x/y/z by the three field terms, so all three matter in every axis."),
        param!("a", "Mix A", float, 1.0, -2.0, 2.0, "Amplitude of the first term, s(m,x)·s(n,y)·s(l,z)."),
        param!("b", "Mix B", float, 0.7, -2.0, 2.0, "Amplitude of the second (cyclically permuted) term. The interference between terms is what bends the nodal surfaces — with b = c = 0 you get a plain rectangular cell grid. In Heightfield mode this is the second term of the 2D wave."),
        param!("c", "Mix C", float, 0.5, -2.0, 2.0, "Amplitude of the third cyclic term. Unused in Heightfield mode."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Spatial scale: one period of the field spans 2·size units in each axis."),
        param!("mode", "Mode", enum, 0, &["Nodal Surface", "Heightfield"], "Nodal Surface: Newton-project points onto the 3D nodal surfaces of the resonator (the 3D sand-on-a-plate analogue; pairs well with solid rendering). Heightfield: displace z toward the 2D plate field — a standing water-wave surface; x/y pass through."),
        param!("walls", "Walls", enum, 0, &["Antinode (Free)", "Node (Fixed)"], "Boundary condition of the wave basis. Antinode (cosine): coordinate planes are antinodes — the nodal set is purely curved surfaces, like a real free-edged plate. Node (sine): the origin planes and their period grid are themselves nodes — adds flat planes through the pattern."),
        param!("height", "Height", float, 0.5, -2.0, 2.0, "Heightfield mode only: amplitude of the z displacement."),
        param!("slice_z", "Slice Z", float, 0.25, -2.0, 2.0, "Slice position. 2D render mode: z of the planar slice through the 3D nodal set. 3D Nodal Surface mode with Slice Thickness > 0: center of the z-slab the attractor is confined to. With Node (sine) walls avoid multiples of size — those planes are themselves nodal and the slice degenerates."),
        param!("slice_thickness", "Slice Thickness", float, 0.0, 0.0, 2.0, "Slab thickness around Slice Z. 2D render mode: 0 cuts a razor-thin plane; > 0 samples each point's slice z randomly within the slab — an X-ray-style projection through it. 3D Nodal Surface mode: 0 disables slicing (full volume); > 0 confines the attractor to the slab."),
        param!("steps", "Steps", int, 3.0, 1.0, 6.0, "Newton iterations toward the nodal surface per call (Nodal Surface mode). 1 is soft; 3+ lands points crisply on the surface."),
        param!("strength", "Strength", float, 0.9, 0.0, 1.0, "Nodal Surface: blend between the untouched input point (0) and the fully projected point (1). Heightfield: blend between the input z and the wave surface — 1 lands points exactly on the surface."),
        param!("jitter", "Jitter", float, 0.0, 0.0, 0.2, "Isotropic random offset added after projection — grain thickness for the surfaces. 0 keeps them razor thin."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Distance", "Amplitude", "Depth"], "Direct-color source, applied through the transform's Direct Color slider. Distance: palette position 1 on the nodal set fading to 0 away from it. Amplitude: signed field value — anti-phase regions of the resonator get opposite palette ends (in Heightfield mode this colors by wave height). Depth: colors by the output point's z — reveals depth structure of the surfaces (in 2D, the sampled slice z)."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 4.0, "Contrast for the direct-color modes: Distance falloff sharpness, Amplitude saturation."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Both bodies share the field helper (self-contained per string — only
// one body is compiled per flame). The basis pair (value, derivative):
//   sin basis: s(k,t) = sin(k·t̂),  s' ∝  cos(k·t̂)
//   cos basis: s(k,t) = cos(k·t̂),  s' ∝ −sin(k·t̂)
// selected per call via `cos_basis`. Same stability guards as
// `chladni`: epsilon in |∇f|² and a step-length clamp of half the
// longest nodal cell. The 2D body evaluates the 3D field on the
// z = slice_z plane and projects within it (the in-plane gradient),
// giving a planar slice of the nodal surfaces.

const WGSL_3D: &str = r#"
// Resonator field at q: vec4(f, ∂f/∂x, ∂f/∂y, ∂f/∂z).
fn cymatics3d_field(q: vec3<f32>, m: f32, n: f32, l: f32, a: f32, b: f32, c: f32, size: f32, cos_basis: bool) -> vec4<f32> {
    let pi = 3.14159265359;
    let s = q / size * pi;
    // Nine (basis, derivative) pairs — mode × axis.
    let mx_s = sin(m * s.x); let mx_c = cos(m * s.x);
    let nx_s = sin(n * s.x); let nx_c = cos(n * s.x);
    let lx_s = sin(l * s.x); let lx_c = cos(l * s.x);
    let my_s = sin(m * s.y); let my_c = cos(m * s.y);
    let ny_s = sin(n * s.y); let ny_c = cos(n * s.y);
    let ly_s = sin(l * s.y); let ly_c = cos(l * s.y);
    let mz_s = sin(m * s.z); let mz_c = cos(m * s.z);
    let nz_s = sin(n * s.z); let nz_c = cos(n * s.z);
    let lz_s = sin(l * s.z); let lz_c = cos(l * s.z);
    let smx = select(mx_s, mx_c, cos_basis); let dmx = select(mx_c, -mx_s, cos_basis);
    let snx = select(nx_s, nx_c, cos_basis); let dnx = select(nx_c, -nx_s, cos_basis);
    let slx = select(lx_s, lx_c, cos_basis); let dlx = select(lx_c, -lx_s, cos_basis);
    let smy = select(my_s, my_c, cos_basis); let dmy = select(my_c, -my_s, cos_basis);
    let sny = select(ny_s, ny_c, cos_basis); let dny = select(ny_c, -ny_s, cos_basis);
    let sly = select(ly_s, ly_c, cos_basis); let dly = select(ly_c, -ly_s, cos_basis);
    let smz = select(mz_s, mz_c, cos_basis); let dmz = select(mz_c, -mz_s, cos_basis);
    let snz = select(nz_s, nz_c, cos_basis); let dnz = select(nz_c, -nz_s, cos_basis);
    let slz = select(lz_s, lz_c, cos_basis); let dlz = select(lz_c, -lz_s, cos_basis);
    let k = pi / size;
    let f = a * smx * sny * slz + b * snx * sly * smz + c * slx * smy * snz;
    return vec4<f32>(
        f,
        k * (a * m * dmx * sny * slz + b * n * dnx * sly * smz + c * l * dlx * smy * snz),
        k * (a * n * smx * dny * slz + b * l * snx * dly * smz + c * m * slx * dmy * snz),
        k * (a * l * smx * sny * dlz + b * m * snx * sly * dmz + c * n * slx * smy * dnz),
    );
}

fn variation_cymatics3d(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let m = get_param(xform_id, variation_id, 0u);
    let n = get_param(xform_id, variation_id, 1u);
    let l = get_param(xform_id, variation_id, 2u);
    let a = get_param(xform_id, variation_id, 3u);
    let b = get_param(xform_id, variation_id, 4u);
    let c = get_param(xform_id, variation_id, 5u);
    let size = max(get_param(xform_id, variation_id, 6u), 1e-6);
    let mode = u32(get_param(xform_id, variation_id, 7u));
    let cos_basis = get_param(xform_id, variation_id, 8u) < 0.5;
    let height = get_param(xform_id, variation_id, 9u);
    let slice_z = get_param(xform_id, variation_id, 10u);
    let slice_thickness = get_param(xform_id, variation_id, 11u);
    let steps = i32(get_param(xform_id, variation_id, 12u));
    let strength = get_param(xform_id, variation_id, 13u);
    let jitter = get_param(xform_id, variation_id, 14u);
    let dc_mode = u32(get_param(xform_id, variation_id, 15u));
    let dc_scale = get_param(xform_id, variation_id, 16u);

    let pi = 3.14159265359;
    // Longest nodal-cell dimension (smallest mode number).
    let cell = size / max(min(min(m, n), l), 1e-3);

    var out: vec3<f32>;
    if (mode == 1u) {
        // Heightfield: pull z toward the 2D plate-field surface; x/y
        // untouched. A blend (not an additive offset) so repeated
        // self-application converges onto the surface instead of
        // accumulating displacement without bound.
        let s = p.xy / size * pi;
        let mx_s = sin(m * s.x); let mx_c = cos(m * s.x);
        let nx_s = sin(n * s.x); let nx_c = cos(n * s.x);
        let my_s = sin(m * s.y); let my_c = cos(m * s.y);
        let ny_s = sin(n * s.y); let ny_c = cos(n * s.y);
        let smx = select(mx_s, mx_c, cos_basis); let dmx = select(mx_c, -mx_s, cos_basis);
        let snx = select(nx_s, nx_c, cos_basis); let dnx = select(nx_c, -nx_s, cos_basis);
        let smy = select(my_s, my_c, cos_basis); let dmy = select(my_c, -my_s, cos_basis);
        let sny = select(ny_s, ny_c, cos_basis); let dny = select(ny_c, -ny_s, cos_basis);
        let f2d = a * smx * sny + b * snx * smy;
        out = vec3<f32>(p.xy, mix(p.z, height * f2d, strength));
        if (dc_mode == 1u) {
            let k = pi / size;
            let g = vec2<f32>(
                k * (a * m * dmx * sny + b * n * dnx * smy),
                k * (a * n * smx * dny + b * m * snx * dmy),
            );
            let dist = abs(f2d) / (length(g) + 1e-6);
            *vc = exp(-6.0 * dc_scale * dist / cell);
        } else if (dc_mode == 2u) {
            let f_norm = abs(a) + abs(b) + 1e-6;
            *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * f2d / f_norm);
        }
    } else {
        // Nodal Surface: damped-Newton projection onto f = 0 in 3D.
        // With slice_thickness > 0 the attractor is confined to a
        // z-slab around slice_z by alternating projection: Newton step
        // toward the surface, then clamp z into the slab. Each
        // iteration ends with the clamp, so the result is always
        // inside the slab and (where the surface crosses it) on the
        // surface.
        let half_t = 0.5 * slice_thickness;
        let max_step = 0.5 * cell;
        var q = p;
        for (var i = 0; i < steps; i = i + 1) {
            let fd = cymatics3d_field(q, m, n, l, a, b, c, size, cos_basis);
            let g = fd.yzw;
            var step = fd.x * g / (dot(g, g) + 1e-6);
            let sl = length(step);
            if (sl > max_step) {
                step = step * (max_step / sl);
            }
            q = q - step;
            if (half_t > 0.0) {
                q.z = clamp(q.z, slice_z - half_t, slice_z + half_t);
            }
        }
        if (dc_mode == 1u) {
            let fd = cymatics3d_field(p, m, n, l, a, b, c, size, cos_basis);
            let dist = abs(fd.x) / (length(fd.yzw) + 1e-6);
            *vc = exp(-6.0 * dc_scale * dist / cell);
        } else if (dc_mode == 2u) {
            let fd = cymatics3d_field(p, m, n, l, a, b, c, size, cos_basis);
            let f_norm = abs(a) + abs(b) + abs(c) + 1e-6;
            *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * fd.x / f_norm);
        }
        out = mix(p, q, strength);
    }

    // Depth color — uses the OUTPUT z (post-projection / heightfield),
    // so it reflects where the point landed, not where it came from.
    if (dc_mode == 3u) {
        *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * out.z / size);
    }

    if (jitter > 0.0) {
        out = out + vec3<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return out;
}
"#;

const WGSL_2D: &str = r#"
// Resonator field at q: vec4(f, ∂f/∂x, ∂f/∂y, ∂f/∂z).
fn cymatics3d_field(q: vec3<f32>, m: f32, n: f32, l: f32, a: f32, b: f32, c: f32, size: f32, cos_basis: bool) -> vec4<f32> {
    let pi = 3.14159265359;
    let s = q / size * pi;
    // Nine (basis, derivative) pairs — mode × axis.
    let mx_s = sin(m * s.x); let mx_c = cos(m * s.x);
    let nx_s = sin(n * s.x); let nx_c = cos(n * s.x);
    let lx_s = sin(l * s.x); let lx_c = cos(l * s.x);
    let my_s = sin(m * s.y); let my_c = cos(m * s.y);
    let ny_s = sin(n * s.y); let ny_c = cos(n * s.y);
    let ly_s = sin(l * s.y); let ly_c = cos(l * s.y);
    let mz_s = sin(m * s.z); let mz_c = cos(m * s.z);
    let nz_s = sin(n * s.z); let nz_c = cos(n * s.z);
    let lz_s = sin(l * s.z); let lz_c = cos(l * s.z);
    let smx = select(mx_s, mx_c, cos_basis); let dmx = select(mx_c, -mx_s, cos_basis);
    let snx = select(nx_s, nx_c, cos_basis); let dnx = select(nx_c, -nx_s, cos_basis);
    let slx = select(lx_s, lx_c, cos_basis); let dlx = select(lx_c, -lx_s, cos_basis);
    let smy = select(my_s, my_c, cos_basis); let dmy = select(my_c, -my_s, cos_basis);
    let sny = select(ny_s, ny_c, cos_basis); let dny = select(ny_c, -ny_s, cos_basis);
    let sly = select(ly_s, ly_c, cos_basis); let dly = select(ly_c, -ly_s, cos_basis);
    let smz = select(mz_s, mz_c, cos_basis); let dmz = select(mz_c, -mz_s, cos_basis);
    let snz = select(nz_s, nz_c, cos_basis); let dnz = select(nz_c, -nz_s, cos_basis);
    let slz = select(lz_s, lz_c, cos_basis); let dlz = select(lz_c, -lz_s, cos_basis);
    let k = pi / size;
    let f = a * smx * sny * slz + b * snx * sly * smz + c * slx * smy * snz;
    return vec4<f32>(
        f,
        k * (a * m * dmx * sny * slz + b * n * dnx * sly * smz + c * l * dlx * smy * snz),
        k * (a * n * smx * dny * slz + b * l * snx * dly * smz + c * m * slx * dmy * snz),
        k * (a * l * smx * sny * dlz + b * m * snx * sly * dmz + c * n * slx * smy * dnz),
    );
}

fn variation_cymatics3d(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let m = get_param(xform_id, variation_id, 0u);
    let n = get_param(xform_id, variation_id, 1u);
    let l = get_param(xform_id, variation_id, 2u);
    let a = get_param(xform_id, variation_id, 3u);
    let b = get_param(xform_id, variation_id, 4u);
    let c = get_param(xform_id, variation_id, 5u);
    let size = max(get_param(xform_id, variation_id, 6u), 1e-6);
    let mode = u32(get_param(xform_id, variation_id, 7u));
    let cos_basis = get_param(xform_id, variation_id, 8u) < 0.5;
    let slice_z = get_param(xform_id, variation_id, 10u);
    let slice_thickness = get_param(xform_id, variation_id, 11u);
    let steps = i32(get_param(xform_id, variation_id, 12u));
    let strength = get_param(xform_id, variation_id, 13u);
    let jitter = get_param(xform_id, variation_id, 14u);
    let dc_mode = u32(get_param(xform_id, variation_id, 15u));
    let dc_scale = get_param(xform_id, variation_id, 16u);

    // Heightfield has no 2D projection — pass through.
    if (mode == 1u) {
        return p;
    }

    let cell = size / max(min(min(m, n), l), 1e-3);
    let max_step = 0.5 * cell;

    // Slice z for this point: exact plane at thickness 0, or sampled
    // uniformly within the slab — superimposing nearby slices like an
    // X-ray projection through it.
    let z_eval = slice_z + (rng_nextf(rng) - 0.5) * slice_thickness;

    // Project within the z = z_eval plane onto the slice of the 3D
    // nodal set — the in-plane gradient components only.
    var q = p;
    for (var i = 0; i < steps; i = i + 1) {
        let fd = cymatics3d_field(vec3<f32>(q, z_eval), m, n, l, a, b, c, size, cos_basis);
        let g = fd.yz;
        var step = fd.x * g / (dot(g, g) + 1e-6);
        let sl = length(step);
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }

    if (dc_mode == 1u) {
        let fd = cymatics3d_field(vec3<f32>(p, z_eval), m, n, l, a, b, c, size, cos_basis);
        let dist = abs(fd.x) / (length(fd.yz) + 1e-6);
        *vc = exp(-6.0 * dc_scale * dist / cell);
    } else if (dc_mode == 2u) {
        let fd = cymatics3d_field(vec3<f32>(p, z_eval), m, n, l, a, b, c, size, cos_basis);
        let f_norm = abs(a) + abs(b) + abs(c) + 1e-6;
        *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * fd.x / f_norm);
    } else if (dc_mode == 3u) {
        // Depth: the sampled slice z — with thickness > 0 this colors
        // each superimposed slice by its position in the slab.
        *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * z_eval / size);
    }

    var out = mix(p, q, strength);
    if (jitter > 0.0) {
        out = out + vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return out;
}
"#;
