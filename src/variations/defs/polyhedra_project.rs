//! Polyhedron projection + solid-occluder variations (original).
//!
//! The `bubble` + `sphere_volume` pairing generalized to polyhedra:
//! project a fractal onto the SURFACE of a chosen solid with
//! [`POLYHEDRON`], and seal its interior for solid-rendering occlusion
//! with [`POLYHEDRON_VOLUME`] — same shape / size / center / rotation
//! params on both, so they pair exactly like bubble + sphere_volume.
//!
//! **Surface** (`polyhedron`): applies bubble's exact inverse-
//! stereographic sphere map (the whole plane onto the unit sphere),
//! then radially reprojects each sphere point onto the solid's surface
//! via its radial support function `r(dir) = min_i d_i/(n_i·dir)` —
//! the fractal is painted onto the polyhedron exactly as bubble paints
//! it onto a sphere. `spherify` blends the radial back toward the
//! sphere (1 = plain bubble), morphing solid ↔ sphere continuously.
//! Use as a final transform for the classic "fractal on a solid" look.
//!
//! **Volume** (`polyhedron_volume`): identical mechanics to
//! `sphere_volume` — a pure side-effect geometry emitter
//! (`VolumeSideEmit`): seals the depth buffer and shadow maps,
//! contributes nothing to the variation sum or image colors. The shell
//! conforms to the surface (radial fraction of the local surface
//! distance), so `thickness < 1` is a skin of even depth over every
//! face. Direction-uniform sampling slightly under-weights corners
//! relative to true volume-uniform — irrelevant for sealing geometry.
//!
//! Shapes: the five Platonic solids, the star tetrahedron (stella
//! octangula — union of two point-reflected tetrahedra), and three
//! bonus Archimedean/Catalan solids with clean face closed forms
//! (cuboctahedron, rhombic dodecahedron, truncated octahedron). All
//! normalized to circumradius 1 so `size` means the same thing bubble's
//! unit sphere does. Geometry lives in `shaders/core/polyhedra.wgsl`,
//! included once when either variation is active (they share it
//! without symbol collisions).
//!
//! No JWildfire/Apophysis counterpart — original to this project.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

const SHAPES: &[&str] = &[
    "Tetrahedron",
    "Cube",
    "Octahedron",
    "Dodecahedron",
    "Icosahedron",
    "Star Tetrahedron",
    "Cuboctahedron",
    "Rhombic Dodecahedron",
    "Truncated Octahedron",
];

/// Polyhedron surface projection (the bubble sphere map generalized to
/// regular polyhedra).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static POLYHEDRON: VariationDef = VariationDef {
    name: "polyhedron",
    aliases: &[],
    display_name: "Polyhedron",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("shape", "Shape", enum, 3, SHAPES, "Which solid to project onto. All are normalized to circumradius 1 (times Size), centered like bubble's unit sphere."),
        param!("size", "Size", unlimited_float, 1.0, 0.0, 10.0, "Circumradius of the solid. 1 matches bubble's unit sphere — a paired polyhedron_volume with the same size seals it exactly."),
        param!("spherify", "Spherify", float, 0.0, 0.0, 1.0, "Blend between the polyhedron surface (0) and the sphere (1 = plain bubble). Animates a solid melting into a ball."),
        param!("cx", "Center X", unlimited_float, 0.0, -5.0, 5.0, "World-space X of the solid's center."),
        param!("cy", "Center Y", unlimited_float, 0.0, -5.0, 5.0, "World-space Y of the solid's center."),
        param!("cz", "Center Z", unlimited_float, 0.0, -5.0, 5.0, "World-space Z of the solid's center."),
        param!("rx", "Rotate X", angle, 0.0, "Rotation of the solid about the X axis, in degrees."),
        param!("ry", "Rotate Y", angle, 0.0, "Rotation of the solid about the Y axis, in degrees."),
        param!("rz", "Rotate Z", angle, 0.0, "Rotation of the solid about the Z axis, in degrees. Applied as Rz·Ry·Rx."),
        param!("bevel", "Bevel", float, 0.0, 0.0, 1.0, "Rounds edges and corners (face-center-normalized smooth max): 0 = razor-sharp, 1 = marshmallow. Face centers stay put — only edges are cut."),
        param!("stellation", "Stellation", float, 0.0, 0.0, 1.0, "Grows true flat-faced spikes in the solid's extended face planes: dodecahedron → small stellated dodecahedron, octahedron → stella octangula. The cube's parallel faces make its spikes degenerate (radius-capped prisms along the axes)."),
    ],
    // 2D: bubble's disc squeeze, warped by the solid's z = 0
    // cross-section — the plane maps into a polygon-shaped disc.
    wgsl_2d: r#"
fn variation_polyhedron(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let shape = u32(get_param(xform_id, variation_id, 0u));
    let size = get_param(xform_id, variation_id, 1u);
    let spherify = get_param(xform_id, variation_id, 2u);
    let cx = get_param(xform_id, variation_id, 3u);
    let cy = get_param(xform_id, variation_id, 4u);
    let rx = get_param(xform_id, variation_id, 6u);
    let ry = get_param(xform_id, variation_id, 7u);
    let rz = get_param(xform_id, variation_id, 8u);
    let bevel = get_param(xform_id, variation_id, 9u);
    let stellation = get_param(xform_id, variation_id, 10u);

    let r2 = dot(p, p);
    let rr = r2 / 4.0 + 1.0;
    let s2 = p / rr;
    let len = max(length(s2), 1e-9);
    let dl = polyhedra_inverse_rotate(vec3<f32>(s2 / len, 0.0), rx, ry, rz);
    var rad = polyhedra_radial(dl, shape, bevel, stellation);
    rad = mix(rad, 1.0, spherify);
    return vec2<f32>(cx, cy) + s2 * (rad * size);
}
"#,
    wgsl_3d: r#"
fn variation_polyhedron(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let shape = u32(get_param(xform_id, variation_id, 0u));
    let size = get_param(xform_id, variation_id, 1u);
    let spherify = get_param(xform_id, variation_id, 2u);
    let cx = get_param(xform_id, variation_id, 3u);
    let cy = get_param(xform_id, variation_id, 4u);
    let cz = get_param(xform_id, variation_id, 5u);
    let rx = get_param(xform_id, variation_id, 6u);
    let ry = get_param(xform_id, variation_id, 7u);
    let rz = get_param(xform_id, variation_id, 8u);
    let bevel = get_param(xform_id, variation_id, 9u);
    let stellation = get_param(xform_id, variation_id, 10u);

    // Bubble's exact sphere map (Apophysis): the output is exactly the
    // unit sphere, so it doubles as the projection direction.
    let r2 = dot(p.xy, p.xy);
    let rr = r2 / 4.0 + 1.0;
    let s = vec3<f32>(p.x / rr, p.y / rr, 2.0 / rr - 1.0);

    let dl = polyhedra_inverse_rotate(s, rx, ry, rz);
    var rad = polyhedra_radial(dl, shape, bevel, stellation);
    rad = mix(rad, 1.0, spherify);
    return vec3<f32>(cx, cy, cz) + s * (rad * size);
}
"#,
};

/// Polyhedron solid-occluder: a depth-only side-emit companion to
/// polyhedron.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static POLYHEDRON_VOLUME: VariationDef = VariationDef {
    name: "polyhedron_volume",
    aliases: &[],
    display_name: "Polyhedron Volume (Solid)",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::AlwaysZ, Feature::VolumeSideEmit],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("shape", "Shape", enum, 3, SHAPES, "Which solid to emit. Match the paired polyhedron variation's shape."),
        param!("size", "Size", unlimited_float, 1.0, 0.0, 10.0, "Circumradius of the emitted solid (match the paired polyhedron's size)."),
        param!("thickness", "Thickness", float, 1.0, 0.0, 1.0, "Shell depth as a fraction of the local surface distance: 1 = filled solid, small values = a sealed skin of even depth over every face."),
        param!("cx", "Center X", unlimited_float, 0.0, -5.0, 5.0, "Solid center X."),
        param!("cy", "Center Y", unlimited_float, 0.0, -5.0, 5.0, "Solid center Y."),
        param!("cz", "Center Z", unlimited_float, 0.0, -5.0, 5.0, "Solid center Z."),
        param!("rx", "Rotate X", angle, 0.0, "Rotation about X, degrees (match the paired polyhedron)."),
        param!("ry", "Rotate Y", angle, 0.0, "Rotation about Y, degrees."),
        param!("rz", "Rotate Z", angle, 0.0, "Rotation about Z, degrees. Applied as Rz·Ry·Rx."),
        param!("bevel", "Bevel", float, 0.0, 0.0, 1.0, "Edge rounding — match the paired polyhedron's bevel so the sealed volume matches the surface."),
        param!("stellation", "Stellation", float, 0.0, 0.0, 1.0, "Spike growth — match the paired polyhedron's stellation."),
    ],
    // 2D: no volume exists — same no-op as sphere_volume.
    wgsl_2d: r#"
fn variation_polyhedron_volume(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    return vec2<f32>(0.0);
}
"#,
    wgsl_3d: r#"
fn variation_polyhedron_volume(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let shape = u32(get_param(xform_id, variation_id, 0u));
    let size = get_param(xform_id, variation_id, 1u);
    let thickness = clamp(get_param(xform_id, variation_id, 2u), 0.0, 1.0);
    let cx = get_param(xform_id, variation_id, 3u);
    let cy = get_param(xform_id, variation_id, 4u);
    let cz = get_param(xform_id, variation_id, 5u);
    let rx = get_param(xform_id, variation_id, 6u);
    let ry = get_param(xform_id, variation_id, 7u);
    let rz = get_param(xform_id, variation_id, 8u);
    let bevel = get_param(xform_id, variation_id, 9u);
    let stellation = get_param(xform_id, variation_id, 10u);

    // Uniform direction on the sphere.
    let z = rng_nextf(rng) * 2.0 - 1.0;
    let phi = rng_nextf(rng) * 6.28318530718;
    let rxy = sqrt(max(1.0 - z * z, 0.0));
    let dir = vec3<f32>(rxy * cos(phi), rxy * sin(phi), z);

    // Local surface distance along this direction, then a
    // surface-conformal shell position (cube-root radial like
    // sphere_volume keeps thick shells depth-uniform).
    let dl = polyhedra_inverse_rotate(dir, rx, ry, rz);
    let rmax = polyhedra_radial(dl, shape, bevel, stellation) * size;
    let ri = 1.0 - thickness;
    let ri3 = ri * ri * ri;
    let r = rmax * pow(mix(ri3, 1.0, rng_nextf(rng)), 0.33333333);

    volume_side_point = vec3<f32>(cx, cy, cz) + dir * r;
    volume_side_flag = true;
    // Pure side effect: contribute NOTHING to the variation sum.
    return vec3<f32>(0.0);
}
"#,
};
