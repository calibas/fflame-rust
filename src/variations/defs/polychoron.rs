//! `polychoron` — chaos-game gaskets on the regular 4-polytopes
//! (original).
//!
//! Runs the vertex chaos game (`q' = s·q + (1−s)·size·R·v`, a random
//! vertex each round) on any of the six regular convex polychora,
//! entirely inside one variation — the 4D generalization of the
//! Sierpinski-gasket transform banks. That matters for the big ones:
//! the 120-cell has **600 vertices**, far past `MAX_TRANSFORMS`, and
//! even the 600-cell's 120 transforms would drag; here a vertex pick
//! is one random index into a baked table
//! (`shaders/core/polychora.wgsl`, generated + verified to unit
//! circumradius by script).
//!
//! The 4th coordinate rides the per-thread `point_w` register
//! (`Feature::NeedsW`) — the fed-forward state is an honest 4D point
//! and the xyz orthographic shadow is plotted. `rot_xw/yw/zw` rotate
//! the whole 4D attractor exactly (the uniform scale commutes with
//! rotations, so rotating the vertex table rotates the gasket);
//! without them, axis-aligned shapes cast degenerate shadows.
//! `steps` composes several chaos-game rounds per call — same
//! quality/perf lever as [`menger`](super::menger).
//!
//! `contraction` is the scale factor `s` toward the origin per round
//! (1/3 matches the classic gasket look; larger values fill toward the
//! polytope's solid hull, smaller ones scatter to vertex dust — for
//! vertex-rich shapes like the 120-cell, higher values connect the
//! structure).
//!
//! Direct color: *Cell* by the chosen vertex index, *W* by the 4th
//! coordinate (depth strata of the shadow).
//!
//! No JWildfire/Apophysis counterpart — original to this project.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static POLYCHORON: VariationDef = VariationDef {
    name: "polychoron",
    aliases: &[],
    display_name: "Polychoron",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsW, Feature::WritesColor, Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("shape", "Shape", enum, 2, &["5-Cell", "8-Cell (Tesseract)", "16-Cell", "24-Cell", "120-Cell", "600-Cell"], "Which regular 4-polytope's vertices to play the chaos game on. 16-Cell is the hyper-octahedron, 120-Cell the hyper-dodecahedron (600 vertices), 600-Cell the hyper-icosahedron (120 vertices); the 24-Cell is unique to 4D."),
        param!("contraction", "Contraction", float, 0.3333, 0.05, 0.9, "Scale factor per chaos-game round: q' = contraction·q + (1−contraction)·vertex. 1/3 gives the classic gasket; larger values fill toward the solid hull (useful for the vertex-rich 120-Cell), smaller ones scatter toward vertex dust."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Circumradius of the polytope."),
        param!("steps", "Steps", int, 2.0, 1.0, 4.0, "Chaos-game rounds per call — sharper attractor for the same iteration budget."),
        param!("rot_xw", "Rotate XW", angle, 20.0, "Rotation in the x–w plane, degrees. Rotates the entire 4D gasket before its shadow is taken — without any 4D rotation, axis-aligned shapes cast degenerate shadows."),
        param!("rot_yw", "Rotate YW", angle, 12.0, "Rotation in the y–w plane, degrees."),
        param!("rot_zw", "Rotate ZW", angle, 7.0, "Rotation in the z–w plane, degrees."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Cell", "W"], "Direct-color source (needs the transform's Direct Color > 0). Cell: color by the chosen vertex index. W: color by the 4th coordinate — depth strata of the 4D shadow."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes (wrapped with fract)."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_polychoron(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let shape = u32(get_param(xform_id, variation_id, 0u));
    let s = get_param(xform_id, variation_id, 1u);
    let size = get_param(xform_id, variation_id, 2u);
    let steps = i32(get_param(xform_id, variation_id, 3u));
    let dc_mode = u32(get_param(xform_id, variation_id, 7u));
    let dc_scale = get_param(xform_id, variation_id, 8u);

    // 2D: chaos game on the xy shadow of the vertex table.
    let rc = polychora_range(shape);
    var q = p;
    var code = 0.0;
    for (var i = 0; i < steps; i = i + 1) {
        let idx = min(u32(rng_nextf(rng) * f32(rc.y)), rc.y - 1u);
        let v = POLYCHORA_VERTS[rc.x + idx];
        q = s * q + (1.0 - s) * size * v.xy;
        code = (f32(idx) + 0.5) / f32(rc.y);
    }
    if (dc_mode != 0u) {
        *vc = fract(code * dc_scale);
    }
    return q;
}
"#;

const WGSL_3D: &str = r#"
fn variation_polychoron(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let shape = u32(get_param(xform_id, variation_id, 0u));
    let s = get_param(xform_id, variation_id, 1u);
    let size = get_param(xform_id, variation_id, 2u);
    let steps = i32(get_param(xform_id, variation_id, 3u));
    let d2r = 0.01745329252;
    let axw = get_param(xform_id, variation_id, 4u) * d2r;
    let ayw = get_param(xform_id, variation_id, 5u) * d2r;
    let azw = get_param(xform_id, variation_id, 6u) * d2r;
    let dc_mode = u32(get_param(xform_id, variation_id, 7u));
    let dc_scale = get_param(xform_id, variation_id, 8u);

    let cxw = cos(axw); let sxw = sin(axw);
    let cyw = cos(ayw); let syw = sin(ayw);
    let czw = cos(azw); let szw = sin(azw);

    let rc = polychora_range(shape);
    var q = vec4<f32>(p, point_w);
    var code = 0.0;
    for (var i = 0; i < steps; i = i + 1) {
        let idx = min(u32(rng_nextf(rng) * f32(rc.y)), rc.y - 1u);
        var v = POLYCHORA_VERTS[rc.x + idx] * size;
        // Rotate the vertex into the attractor's 4D orientation (the
        // uniform contraction commutes with rotations, so this rotates
        // the whole gasket exactly).
        v = vec4<f32>(cxw * v.x - sxw * v.w, v.y, v.z, sxw * v.x + cxw * v.w);
        v = vec4<f32>(v.x, cyw * v.y - syw * v.w, v.z, syw * v.y + cyw * v.w);
        v = vec4<f32>(v.x, v.y, czw * v.z - szw * v.w, szw * v.z + czw * v.w);
        q = s * q + (1.0 - s) * v;
        code = (f32(idx) + 0.5) / f32(rc.y);
    }
    point_w_out = q.w;
    if (dc_mode == 1u) {
        *vc = fract(code * dc_scale);
    } else if (dc_mode == 2u) {
        *vc = fract(q.w * dc_scale);
    }
    return q.xyz;
}
"#;
