// Polyhedron radial-support helpers — shared by the `polyhedron`
// (surface projection) and `polyhedron_volume` (solid occluder)
// variations. Included once by the shader builder when either is
// active, so the two can coexist in one flame without duplicate
// symbols.
//
// Every solid is normalized to CIRCUMRADIUS 1 and centered at the
// origin. Faces are stored as vec4(unit normal, support distance) in
// one flat array (generated + verified to ~4e-16 by the polyhedra_gen
// scratchpad script); the radial support function along unit `dir` is
// r(dir) = 1 / max_i((n_i . dir)+ / d_i).
//
// Two artistic controls hook into the same evaluation:
//   * bevel — replaces the hard max with a face-center-normalized
//     p-norm smooth max: edges and corners round off continuously
//     while face centers stay exactly at distance d.
//   * stellation — mixes the denominator toward the SECOND-largest
//     face dot. Stellations lie in the same extended face planes as
//     the core solid (in 2D: pentagon -> pentagram), so this grows
//     true flat-faced spikes: dodecahedron -> small stellated
//     dodecahedron, octahedron -> stella octangula, etc. Spike ridges
//     stay sharp even under bevel (the mix target is the hard second
//     max — arguably the right look). The cube's face planes are
//     parallel, so its "stellation" diverges along the axes; a radius
//     cap (6x circumradius) keeps that and similar degenerate spikes
//     finite.
//
// Shape ids (must match the defs' enum order):
//   0 Tetrahedron        1 Cube               2 Octahedron
//   3 Dodecahedron       4 Icosahedron        5 Star Tetrahedron
//   6 Cuboctahedron      7 Rhombic Dodecahedron
//   8 Truncated Octahedron
// The star tetrahedron is the union of two point-reflected tetrahedra
// (star-shaped about the center): r = max of the two tetra radials.

const POLYHEDRA_FACES: array<vec4<f32>, 90> = array<vec4<f32>, 90>(
    vec4<f32>(-0.577350269, -0.577350269, -0.577350269, 0.333333333),
    vec4<f32>(-0.577350269, 0.577350269, 0.577350269, 0.333333333),
    vec4<f32>(0.577350269, -0.577350269, 0.577350269, 0.333333333),
    vec4<f32>(0.577350269, 0.577350269, -0.577350269, 0.333333333),
    vec4<f32>(1.000000000, 0.000000000, 0.000000000, 0.577350269),
    vec4<f32>(-1.000000000, 0.000000000, 0.000000000, 0.577350269),
    vec4<f32>(0.000000000, 1.000000000, 0.000000000, 0.577350269),
    vec4<f32>(0.000000000, -1.000000000, 0.000000000, 0.577350269),
    vec4<f32>(0.000000000, 0.000000000, 1.000000000, 0.577350269),
    vec4<f32>(0.000000000, 0.000000000, -1.000000000, 0.577350269),
    vec4<f32>(-0.577350269, -0.577350269, -0.577350269, 0.577350269),
    vec4<f32>(-0.577350269, -0.577350269, 0.577350269, 0.577350269),
    vec4<f32>(-0.577350269, 0.577350269, -0.577350269, 0.577350269),
    vec4<f32>(-0.577350269, 0.577350269, 0.577350269, 0.577350269),
    vec4<f32>(0.577350269, -0.577350269, -0.577350269, 0.577350269),
    vec4<f32>(0.577350269, -0.577350269, 0.577350269, 0.577350269),
    vec4<f32>(0.577350269, 0.577350269, -0.577350269, 0.577350269),
    vec4<f32>(0.577350269, 0.577350269, 0.577350269, 0.577350269),
    vec4<f32>(0.000000000, -0.525731112, -0.850650808, 0.794654472),
    vec4<f32>(0.000000000, -0.525731112, 0.850650808, 0.794654472),
    vec4<f32>(0.000000000, 0.525731112, -0.850650808, 0.794654472),
    vec4<f32>(0.000000000, 0.525731112, 0.850650808, 0.794654472),
    vec4<f32>(-0.850650808, 0.000000000, -0.525731112, 0.794654472),
    vec4<f32>(-0.850650808, 0.000000000, 0.525731112, 0.794654472),
    vec4<f32>(0.850650808, 0.000000000, -0.525731112, 0.794654472),
    vec4<f32>(0.850650808, 0.000000000, 0.525731112, 0.794654472),
    vec4<f32>(-0.525731112, -0.850650808, 0.000000000, 0.794654472),
    vec4<f32>(-0.525731112, 0.850650808, 0.000000000, 0.794654472),
    vec4<f32>(0.525731112, -0.850650808, 0.000000000, 0.794654472),
    vec4<f32>(0.525731112, 0.850650808, 0.000000000, 0.794654472),
    vec4<f32>(-0.577350269, -0.577350269, -0.577350269, 0.794654472),
    vec4<f32>(-0.577350269, -0.577350269, 0.577350269, 0.794654472),
    vec4<f32>(-0.577350269, 0.577350269, -0.577350269, 0.794654472),
    vec4<f32>(-0.577350269, 0.577350269, 0.577350269, 0.794654472),
    vec4<f32>(0.577350269, -0.577350269, -0.577350269, 0.794654472),
    vec4<f32>(0.577350269, -0.577350269, 0.577350269, 0.794654472),
    vec4<f32>(0.577350269, 0.577350269, -0.577350269, 0.794654472),
    vec4<f32>(0.577350269, 0.577350269, 0.577350269, 0.794654472),
    vec4<f32>(0.000000000, -0.934172359, -0.356822090, 0.794654472),
    vec4<f32>(0.000000000, -0.934172359, 0.356822090, 0.794654472),
    vec4<f32>(0.000000000, 0.934172359, -0.356822090, 0.794654472),
    vec4<f32>(0.000000000, 0.934172359, 0.356822090, 0.794654472),
    vec4<f32>(-0.356822090, 0.000000000, -0.934172359, 0.794654472),
    vec4<f32>(-0.356822090, 0.000000000, 0.934172359, 0.794654472),
    vec4<f32>(0.356822090, 0.000000000, -0.934172359, 0.794654472),
    vec4<f32>(0.356822090, 0.000000000, 0.934172359, 0.794654472),
    vec4<f32>(-0.934172359, -0.356822090, 0.000000000, 0.794654472),
    vec4<f32>(-0.934172359, 0.356822090, 0.000000000, 0.794654472),
    vec4<f32>(0.934172359, -0.356822090, 0.000000000, 0.794654472),
    vec4<f32>(0.934172359, 0.356822090, 0.000000000, 0.794654472),
    vec4<f32>(1.000000000, 0.000000000, 0.000000000, 0.707106781),
    vec4<f32>(-1.000000000, 0.000000000, 0.000000000, 0.707106781),
    vec4<f32>(0.000000000, 1.000000000, 0.000000000, 0.707106781),
    vec4<f32>(0.000000000, -1.000000000, 0.000000000, 0.707106781),
    vec4<f32>(0.000000000, 0.000000000, 1.000000000, 0.707106781),
    vec4<f32>(0.000000000, 0.000000000, -1.000000000, 0.707106781),
    vec4<f32>(-0.577350269, -0.577350269, -0.577350269, 0.816496581),
    vec4<f32>(-0.577350269, -0.577350269, 0.577350269, 0.816496581),
    vec4<f32>(-0.577350269, 0.577350269, -0.577350269, 0.816496581),
    vec4<f32>(-0.577350269, 0.577350269, 0.577350269, 0.816496581),
    vec4<f32>(0.577350269, -0.577350269, -0.577350269, 0.816496581),
    vec4<f32>(0.577350269, -0.577350269, 0.577350269, 0.816496581),
    vec4<f32>(0.577350269, 0.577350269, -0.577350269, 0.816496581),
    vec4<f32>(0.577350269, 0.577350269, 0.577350269, 0.816496581),
    vec4<f32>(-0.707106781, -0.707106781, 0.000000000, 0.707106781),
    vec4<f32>(-0.707106781, 0.000000000, -0.707106781, 0.707106781),
    vec4<f32>(-0.707106781, 0.000000000, 0.707106781, 0.707106781),
    vec4<f32>(-0.707106781, 0.707106781, 0.000000000, 0.707106781),
    vec4<f32>(0.000000000, -0.707106781, -0.707106781, 0.707106781),
    vec4<f32>(0.000000000, -0.707106781, 0.707106781, 0.707106781),
    vec4<f32>(0.000000000, 0.707106781, -0.707106781, 0.707106781),
    vec4<f32>(0.000000000, 0.707106781, 0.707106781, 0.707106781),
    vec4<f32>(0.707106781, -0.707106781, 0.000000000, 0.707106781),
    vec4<f32>(0.707106781, 0.000000000, -0.707106781, 0.707106781),
    vec4<f32>(0.707106781, 0.000000000, 0.707106781, 0.707106781),
    vec4<f32>(0.707106781, 0.707106781, 0.000000000, 0.707106781),
    vec4<f32>(1.000000000, 0.000000000, 0.000000000, 0.894427191),
    vec4<f32>(-1.000000000, 0.000000000, 0.000000000, 0.894427191),
    vec4<f32>(0.000000000, 1.000000000, 0.000000000, 0.894427191),
    vec4<f32>(0.000000000, -1.000000000, 0.000000000, 0.894427191),
    vec4<f32>(0.000000000, 0.000000000, 1.000000000, 0.894427191),
    vec4<f32>(0.000000000, 0.000000000, -1.000000000, 0.894427191),
    vec4<f32>(-0.577350269, -0.577350269, -0.577350269, 0.774596669),
    vec4<f32>(-0.577350269, -0.577350269, 0.577350269, 0.774596669),
    vec4<f32>(-0.577350269, 0.577350269, -0.577350269, 0.774596669),
    vec4<f32>(-0.577350269, 0.577350269, 0.577350269, 0.774596669),
    vec4<f32>(0.577350269, -0.577350269, -0.577350269, 0.774596669),
    vec4<f32>(0.577350269, -0.577350269, 0.577350269, 0.774596669),
    vec4<f32>(0.577350269, 0.577350269, -0.577350269, 0.774596669),
    vec4<f32>(0.577350269, 0.577350269, 0.577350269, 0.774596669),
);

// (offset, count) into POLYHEDRA_FACES per shape id; the star
// tetrahedron (5) uses the tetra range on +dir and -dir.
fn polyhedra_face_range(shape: u32) -> vec2<u32> {
    switch shape {
        case 0u, 5u: { return vec2<u32>(0u, 4u); }
        case 1u: { return vec2<u32>(4u, 6u); }
        case 2u: { return vec2<u32>(10u, 8u); }
        case 3u: { return vec2<u32>(18u, 12u); }
        case 4u: { return vec2<u32>(30u, 20u); }
        case 6u: { return vec2<u32>(50u, 14u); }
        case 7u: { return vec2<u32>(64u, 12u); }
        case 8u: { return vec2<u32>(76u, 14u); }
        default: { return vec2<u32>(4u, 6u); }
    }
}

// Support denominators along `dir` for one convex face range:
// vec2(max of s_i = (n_i . dir)+ / d_i — smooth-maxed when bevel > 0 —
// and the hard second-largest s_i, the stellation target).
fn polyhedra_denom(dir: vec3<f32>, off: u32, cnt: u32, bevel: f32) -> vec2<f32> {
    var m1 = 0.0;
    var m2 = 0.0;
    var imax = 0u;
    for (var i = 0u; i < cnt; i = i + 1u) {
        let f = POLYHEDRA_FACES[off + i];
        let s = max(dot(f.xyz, dir), 0.0) / f.w;
        if (s > m1) {
            m2 = m1;
            m1 = s;
            imax = i;
        } else if (s > m2) {
            m2 = s;
        }
    }
    var m = m1;
    var m2_eff = m2;
    if (bevel > 0.0) {
        // p-norm smooth max (terms scaled by m1 so pow stays in
        // (0, 1]), normalized by the same p-norm evaluated along face
        // 0's normal so face centers keep their exact distance — the
        // bevel only cuts edges and corners inward.
        let p = mix(24.0, 3.0, clamp(bevel, 0.0, 1.0));
        // Second-max p-norm sums are accumulated WITHOUT the max term
        // (psum2 / qsum2) rather than as "full sum - 1": at high p the
        // non-max terms are ~1e-7 or smaller and the subtraction
        // cancels to zero in f32, tearing the spike surface apart at
        // low bevel values.
        var psum2 = 0.0;
        var qsum2 = 0.0;
        var qm1 = 0.0;
        var qm2 = 0.0;
        let n0 = POLYHEDRA_FACES[off].xyz;
        let d0 = POLYHEDRA_FACES[off].w;
        for (var i = 0u; i < cnt; i = i + 1u) {
            let f = POLYHEDRA_FACES[off + i];
            let s = max(dot(f.xyz, dir), 0.0) / f.w;
            if (i != imax) {
                psum2 = psum2 + pow(max(s / m1, 1e-6), p);
            }
            let q = max(dot(f.xyz, n0), 0.0) / f.w;
            if (i != 0u) {
                qsum2 = qsum2 + pow(max(q * d0, 1e-6), p);
            }
            if (q > qm1) {
                qm2 = qm1;
                qm1 = q;
            } else if (q > qm2) {
                qm2 = q;
            }
        }
        // Smooth max of the core (the max term contributes exactly 1).
        m = m1 * pow(1.0 + psum2, 1.0 / p) / pow(1.0 + qsum2, 1.0 / p);

        // Smooth SECOND max, so bevel also rounds the stellation's
        // spike tips and reentrant ridges. Rescaling by the hard
        // second max at n0 (qm2) keeps the spike-tip length exactly at
        // the unbeveled stellation height.
        let raw2 = m1 * pow(max(psum2, 1e-30), 1.0 / p);
        let ref2 = (1.0 / d0) * pow(max(qsum2, 1e-30), 1.0 / p);
        if (qm2 > 1e-6 && ref2 > 1e-9) {
            m2_eff = raw2 * (qm2 / ref2);
        } else {
            // Degenerate stellation (cube: no second positive face at
            // the tip direction) — leave the raw smooth second max.
            m2_eff = raw2;
        }
    }
    return vec2<f32>(m, m2_eff);
}

// Radial distance for one convex face range, with the stellation mix
// done in RADIUS space (mixing denominators back-loads all the spike
// growth near stellation = 1; radius mixing makes the slider feel
// linear). The 6x-circumradius cap keeps degenerate spikes (the
// cube's parallel face planes) finite.
fn polyhedra_radial_one(dir: vec3<f32>, off: u32, cnt: u32, bevel: f32, stellation: f32) -> f32 {
    let d = polyhedra_denom(dir, off, cnt, bevel);
    var r = 1.0 / max(d.x, 1e-6);
    if (stellation > 0.0) {
        let r_spike = min(1.0 / max(d.y, 1e-6), 6.0);
        r = mix(r, r_spike, clamp(stellation, 0.0, 1.0));
    }
    return min(r, 6.0);
}

fn polyhedra_radial(dir: vec3<f32>, shape: u32, bevel: f32, stellation: f32) -> f32 {
    let rc = polyhedra_face_range(shape);
    var r = polyhedra_radial_one(dir, rc.x, rc.y, bevel, stellation);
    if (shape == 5u) {
        // Star tetrahedron: union of the tetra and its point
        // reflection -> max of the two radials.
        r = max(r, polyhedra_radial_one(-dir, rc.x, rc.y, bevel, stellation));
    }
    return r;
}

// Map a world-space direction into the solid's local frame. The solid
// is rotated by intrinsic Euler angles Rz(rz)*Ry(ry)*Rx(rx) (degrees),
// so sampling its radial function along world `dir` evaluates along
// R^-1 dir = Rx(-rx)*Ry(-ry)*Rz(-rz)*dir.
fn polyhedra_inverse_rotate(dir: vec3<f32>, rx: f32, ry: f32, rz: f32) -> vec3<f32> {
    let d2r = 0.01745329252;
    var d = dir;
    let cz = cos(-rz * d2r);
    let sz = sin(-rz * d2r);
    d = vec3<f32>(cz * d.x - sz * d.y, sz * d.x + cz * d.y, d.z);
    let cy = cos(-ry * d2r);
    let sy = sin(-ry * d2r);
    d = vec3<f32>(cy * d.x + sy * d.z, d.y, -sy * d.x + cy * d.z);
    let cx = cos(-rx * d2r);
    let sx = sin(-rx * d2r);
    d = vec3<f32>(d.x, cx * d.y - sx * d.z, sx * d.y + cx * d.z);
    return d;
}
