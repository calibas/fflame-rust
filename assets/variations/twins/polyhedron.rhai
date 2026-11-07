/// Polyhedron surface projection (the bubble sphere map generalized to
/// regular polyhedra).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Polyhedron, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    shape: #{
        value: 3, choices: ["Tetrahedron", "Cube", "Octahedron", "Dodecahedron", "Icosahedron", "Star Tetrahedron", "Cuboctahedron", "Rhombic Dodecahedron", "Truncated Octahedron"],
        caption: "Shape",
        hint: "Which solid to project onto. All are normalized to circumradius 1 (times Size), centered like bubble's unit sphere.",
    },
    size: #{
        value: 1.0, min: 0.0, max: 10.0, free: true,
        caption: "Size",
        hint: "Circumradius of the solid. 1 matches bubble's unit sphere — a paired polyhedron_volume with the same size seals it exactly.",
    },
    spherify: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Spherify",
        hint: "Blend between the polyhedron surface (0) and the sphere (1 = plain bubble). Animates a solid melting into a ball.",
    },
    cx: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Center X",
        hint: "World-space X of the solid's center.",
    },
    cy: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Center Y",
        hint: "World-space Y of the solid's center.",
    },
    cz: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Center Z",
        hint: "World-space Z of the solid's center.",
    },
    rx: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Rotate X",
        hint: "Rotation of the solid about the X axis, in degrees.",
    },
    ry: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Rotate Y",
        hint: "Rotation of the solid about the Y axis, in degrees.",
    },
    rz: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Rotate Z",
        hint: "Rotation of the solid about the Z axis, in degrees. Applied as Rz·Ry·Rx.",
    },
    bevel: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Bevel",
        hint: "Rounds edges and corners (face-center-normalized smooth max): 0 = razor-sharp, 1 = marshmallow. Face centers stay put — only edges are cut.",
    },
    stellation: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Stellation",
        hint: "Grows true flat-faced spikes in the solid's extended face planes: dodecahedron → small stellated dodecahedron, octahedron → stella octangula. The cube's parallel faces make its spikes degenerate (radius-capped prisms along the axes).",
    },
};

const INFO = #{ title: "Polyhedron", name: "rhai_polyhedron", category: "advanced_2d", phase: "any" };

fn polyhedra_face_range(shape) {
    let t1 = shape;
    if t1 == u32(0) || t1 == u32(5) {
        [u32(0), u32(4)]
    } else if t1 == u32(1) {
        [u32(4), u32(6)]
    } else if t1 == u32(2) {
        [u32(10), u32(8)]
    } else if t1 == u32(3) {
        [u32(18), u32(12)]
    } else if t1 == u32(4) {
        [u32(30), u32(20)]
    } else if t1 == u32(6) {
        [u32(50), u32(14)]
    } else if t1 == u32(7) {
        [u32(64), u32(12)]
    } else if t1 == u32(8) {
        [u32(76), u32(14)]
    } else {
        [u32(4), u32(6)]
    }
}

fn polyhedra_denom(dir, off, cnt, bevel) {
    let tbl_polyhedra_faces = [-0.577350269, -0.577350269, -0.577350269, 0.333333333, -0.577350269, 0.577350269, 0.577350269, 0.333333333, 0.577350269, -0.577350269, 0.577350269, 0.333333333, 0.577350269, 0.577350269, -0.577350269, 0.333333333, 1.000000000, 0.000000000, 0.000000000, 0.577350269, -1.000000000, 0.000000000, 0.000000000, 0.577350269, 0.000000000, 1.000000000, 0.000000000, 0.577350269, 0.000000000, -1.000000000, 0.000000000, 0.577350269, 0.000000000, 0.000000000, 1.000000000, 0.577350269, 0.000000000, 0.000000000, -1.000000000, 0.577350269, -0.577350269, -0.577350269, -0.577350269, 0.577350269, -0.577350269, -0.577350269, 0.577350269, 0.577350269, -0.577350269, 0.577350269, -0.577350269, 0.577350269, -0.577350269, 0.577350269, 0.577350269, 0.577350269, 0.577350269, -0.577350269, -0.577350269, 0.577350269, 0.577350269, -0.577350269, 0.577350269, 0.577350269, 0.577350269, 0.577350269, -0.577350269, 0.577350269, 0.577350269, 0.577350269, 0.577350269, 0.577350269, 0.000000000, -0.525731112, -0.850650808, 0.794654472, 0.000000000, -0.525731112, 0.850650808, 0.794654472, 0.000000000, 0.525731112, -0.850650808, 0.794654472, 0.000000000, 0.525731112, 0.850650808, 0.794654472, -0.850650808, 0.000000000, -0.525731112, 0.794654472, -0.850650808, 0.000000000, 0.525731112, 0.794654472, 0.850650808, 0.000000000, -0.525731112, 0.794654472, 0.850650808, 0.000000000, 0.525731112, 0.794654472, -0.525731112, -0.850650808, 0.000000000, 0.794654472, -0.525731112, 0.850650808, 0.000000000, 0.794654472, 0.525731112, -0.850650808, 0.000000000, 0.794654472, 0.525731112, 0.850650808, 0.000000000, 0.794654472, -0.577350269, -0.577350269, -0.577350269, 0.794654472, -0.577350269, -0.577350269, 0.577350269, 0.794654472, -0.577350269, 0.577350269, -0.577350269, 0.794654472, -0.577350269, 0.577350269, 0.577350269, 0.794654472, 0.577350269, -0.577350269, -0.577350269, 0.794654472, 0.577350269, -0.577350269, 0.577350269, 0.794654472, 0.577350269, 0.577350269, -0.577350269, 0.794654472, 0.577350269, 0.577350269, 0.577350269, 0.794654472, 0.000000000, -0.934172359, -0.356822090, 0.794654472, 0.000000000, -0.934172359, 0.356822090, 0.794654472, 0.000000000, 0.934172359, -0.356822090, 0.794654472, 0.000000000, 0.934172359, 0.356822090, 0.794654472, -0.356822090, 0.000000000, -0.934172359, 0.794654472, -0.356822090, 0.000000000, 0.934172359, 0.794654472, 0.356822090, 0.000000000, -0.934172359, 0.794654472, 0.356822090, 0.000000000, 0.934172359, 0.794654472, -0.934172359, -0.356822090, 0.000000000, 0.794654472, -0.934172359, 0.356822090, 0.000000000, 0.794654472, 0.934172359, -0.356822090, 0.000000000, 0.794654472, 0.934172359, 0.356822090, 0.000000000, 0.794654472, 1.000000000, 0.000000000, 0.000000000, 0.707106781, -1.000000000, 0.000000000, 0.000000000, 0.707106781, 0.000000000, 1.000000000, 0.000000000, 0.707106781, 0.000000000, -1.000000000, 0.000000000, 0.707106781, 0.000000000, 0.000000000, 1.000000000, 0.707106781, 0.000000000, 0.000000000, -1.000000000, 0.707106781, -0.577350269, -0.577350269, -0.577350269, 0.816496581, -0.577350269, -0.577350269, 0.577350269, 0.816496581, -0.577350269, 0.577350269, -0.577350269, 0.816496581, -0.577350269, 0.577350269, 0.577350269, 0.816496581, 0.577350269, -0.577350269, -0.577350269, 0.816496581, 0.577350269, -0.577350269, 0.577350269, 0.816496581, 0.577350269, 0.577350269, -0.577350269, 0.816496581, 0.577350269, 0.577350269, 0.577350269, 0.816496581, -0.707106781, -0.707106781, 0.000000000, 0.707106781, -0.707106781, 0.000000000, -0.707106781, 0.707106781, -0.707106781, 0.000000000, 0.707106781, 0.707106781, -0.707106781, 0.707106781, 0.000000000, 0.707106781, 0.000000000, -0.707106781, -0.707106781, 0.707106781, 0.000000000, -0.707106781, 0.707106781, 0.707106781, 0.000000000, 0.707106781, -0.707106781, 0.707106781, 0.000000000, 0.707106781, 0.707106781, 0.707106781, 0.707106781, -0.707106781, 0.000000000, 0.707106781, 0.707106781, 0.000000000, -0.707106781, 0.707106781, 0.707106781, 0.000000000, 0.707106781, 0.707106781, 0.707106781, 0.707106781, 0.000000000, 0.707106781, 1.000000000, 0.000000000, 0.000000000, 0.894427191, -1.000000000, 0.000000000, 0.000000000, 0.894427191, 0.000000000, 1.000000000, 0.000000000, 0.894427191, 0.000000000, -1.000000000, 0.000000000, 0.894427191, 0.000000000, 0.000000000, 1.000000000, 0.894427191, 0.000000000, 0.000000000, -1.000000000, 0.894427191, -0.577350269, -0.577350269, -0.577350269, 0.774596669, -0.577350269, -0.577350269, 0.577350269, 0.774596669, -0.577350269, 0.577350269, -0.577350269, 0.774596669, -0.577350269, 0.577350269, 0.577350269, 0.774596669, 0.577350269, -0.577350269, -0.577350269, 0.774596669, 0.577350269, -0.577350269, 0.577350269, 0.774596669, 0.577350269, 0.577350269, -0.577350269, 0.774596669, 0.577350269, 0.577350269, 0.577350269, 0.774596669];
    let m1 = 0.0;
    let m2 = 0.0;
    let imax = u32(0);
    for i_n in 0..to_int(cnt) {
        let i = u32(i_n);
        let t1 = off + i;
        let f = vec4(tbl_polyhedra_faces[4 * t1 + 0], tbl_polyhedra_faces[4 * t1 + 1], tbl_polyhedra_faces[4 * t1 + 2], tbl_polyhedra_faces[4 * t1 + 3]);
        let s = max(0.0, dot(vec3(f.x, f.y, f.z), dir)) / f.w;
        if (s > m1) {
            m2 = m1;
            m1 = s;
            imax = i;
        } else if (s > m2) {
            m2 = s;
        }
    }
    let m = m1;
    let m2_eff = m2;
    if (bevel > 0.0) {
        let p = mix(24.0, 3.0, clamp(bevel, 0.0, 1.0));
        let psum2 = 0.0;
        let qsum2 = 0.0;
        let qm1 = 0.0;
        let qm2 = 0.0;
        let t2 = vec4(tbl_polyhedra_faces[4 * off + 0], tbl_polyhedra_faces[4 * off + 1], tbl_polyhedra_faces[4 * off + 2], tbl_polyhedra_faces[4 * off + 3]);
        let n0 = vec3(t2.x, t2.y, t2.z);
        let d0 = vec4(tbl_polyhedra_faces[4 * off + 0], tbl_polyhedra_faces[4 * off + 1], tbl_polyhedra_faces[4 * off + 2], tbl_polyhedra_faces[4 * off + 3]).w;
        for i_n in 0..to_int(cnt) {
            let i = u32(i_n);
            let t3 = off + i;
            let f = vec4(tbl_polyhedra_faces[4 * t3 + 0], tbl_polyhedra_faces[4 * t3 + 1], tbl_polyhedra_faces[4 * t3 + 2], tbl_polyhedra_faces[4 * t3 + 3]);
            let s = max(0.0, dot(vec3(f.x, f.y, f.z), dir)) / f.w;
            if (i != imax) {
                psum2 = psum2 + pow(max(1.0e-6, s / m1), p);
            }
            let q = max(0.0, dot(vec3(f.x, f.y, f.z), n0)) / f.w;
            if (i != u32(0)) {
                qsum2 = qsum2 + pow(max(1.0e-6, q * d0), p);
            }
            if (q > qm1) {
                qm2 = qm1;
                qm1 = q;
            } else if (q > qm2) {
                qm2 = q;
            }
        }
        m = m1 * pow(1.0 + psum2, 1.0 / p) / pow(1.0 + qsum2, 1.0 / p);
        let raw2 = m1 * pow(max(1.0e-30, psum2), 1.0 / p);
        let ref2 = (1.0 / d0) * pow(max(1.0e-30, qsum2), 1.0 / p);
        if (qm2 > 1.0e-6 && ref2 > 1.0e-9) {
            m2_eff = raw2 * (qm2 / ref2);
        } else {
            m2_eff = raw2;
        }
    }
    complex(m, m2_eff)
}

fn polyhedra_radial_one(dir, off, cnt, bevel, stellation) {
    let d = polyhedra_denom(dir, off, cnt, bevel);
    let r = 1.0 / max(1.0e-6, d.re);
    if (stellation > 0.0) {
        let r_spike = min(6.0, 1.0 / max(1.0e-6, d.im));
        r = mix(r, r_spike, clamp(stellation, 0.0, 1.0));
    }
    min(6.0, r)
}

fn polyhedra_radial(dir, shape, bevel, stellation) {
    let rc = polyhedra_face_range(shape);
    let r = polyhedra_radial_one(dir, rc[0], rc[1], bevel, stellation);
    if (shape == u32(5)) {
        r = max(r, polyhedra_radial_one(-dir, rc[0], rc[1], bevel, stellation));
    }
    r
}

fn polyhedra_inverse_rotate(dir, rx, ry, rz) {
    let d2r = 0.01745329252;
    let d = dir;
    let cz = cos(-rz * d2r);
    let sz = sin(-rz * d2r);
    d = vec3(cz * d.x - sz * d.y, sz * d.x + cz * d.y, d.z);
    let cy = cos(-ry * d2r);
    let sy = sin(-ry * d2r);
    d = vec3(cy * d.x + sy * d.z, d.y, -sy * d.x + cy * d.z);
    let cx = cos(-rx * d2r);
    let sx = sin(-rx * d2r);
    d = vec3(d.x, cx * d.y - sx * d.z, sx * d.y + cx * d.z);
    d
}

fn variation(z, p) {
    let shape = u32(to_float(p.shape));
    let size = p.size;
    let spherify = p.spherify;
    let cx = p.cx;
    let cy = p.cy;
    let rx = p.rx;
    let ry = p.ry;
    let rz = p.rz;
    let bevel = p.bevel;
    let stellation = p.stellation;
    let r2 = z.norm_sqr();
    let rr = r2 / 4.0 + 1.0;
    let s2 = z / rr;
    let len = max(1.0e-9, s2.abs());
    let t1 = s2 / len;
    let dl = polyhedra_inverse_rotate(vec3(t1.re, t1.im, 0.0), rx, ry, rz);
    let rad = polyhedra_radial(dl, shape, bevel, stellation);
    rad = mix(rad, 1.0, spherify);
    complex(cx, cy) + s2 * (rad * size)
}

fn variation3(v, p) {
    let shape = u32(to_float(p.shape));
    let size = p.size;
    let spherify = p.spherify;
    let cx = p.cx;
    let cy = p.cy;
    let cz = p.cz;
    let rx = p.rx;
    let ry = p.ry;
    let rz = p.rz;
    let bevel = p.bevel;
    let stellation = p.stellation;
    let r2 = complex(v.x, v.y).norm_sqr();
    let rr = r2 / 4.0 + 1.0;
    let s = vec3(v.x / rr, v.y / rr, 2.0 / rr - 1.0);
    let dl = polyhedra_inverse_rotate(s, rx, ry, rz);
    let rad = polyhedra_radial(dl, shape, bevel, stellation);
    rad = mix(rad, 1.0, spherify);
    vec3(cx, cy, cz) + s * (rad * size)
}
