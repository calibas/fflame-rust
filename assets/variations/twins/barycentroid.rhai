/// Barycentric-coordinate warp — treats the input as the third vertex of a
/// triangle whose other two vertices are user-defined: `V0 = (a, b)`, `V1 =
/// (c, d)`, `V2 = (x, y)`. Computes barycentric coordinates `(u, v)` of
/// `(x, y)` in the triangle, then emits `(sign(u)·sqrt(u²+x²),
/// sign(v)·sqrt(v²+y²))`.
///
/// # Authors
/// - Xyrus02

// Barycentroid, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "V0.x",
        hint: "V0 X coordinate (first triangle vertex).",
    },
    b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "V0.y",
        hint: "V0 Y coordinate.",
    },
    c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "V1.x",
        hint: "V1 X coordinate (second triangle vertex).",
    },
    d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "V1.y",
        hint: "V1 Y coordinate.",
    },
};

const INFO = #{ title: "Barycentroid", name: "rhai_barycentroid", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let dot00 = a * a + b * b;
    let dot01 = a * c + b * d;
    let dot02 = a * z.re + b * z.im;
    let dot11 = c * c + d * d;
    let dot12 = c * z.re + d * z.im;
    let denom = dot00 * dot11 - dot01 * dot01;
    let inv_denom = 1.0 / (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
    let sgn_u = (if u == 0.0 { 0.0 } else { (if u > 0.0 { 1.0 } else if u < 0.0 { -1.0 } else { 0.0 }) });
    let sgn_v = (if v == 0.0 { 0.0 } else { (if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 }) });
    complex(sqrt(u * u + z.re * z.re) * sgn_u, sqrt(v * v + z.im * z.im) * sgn_v)
}

fn variation3(q, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let dot00 = a * a + b * b;
    let dot01 = a * c + b * d;
    let dot02 = a * q.x + b * q.y;
    let dot11 = c * c + d * d;
    let dot12 = c * q.x + d * q.y;
    let denom = dot00 * dot11 - dot01 * dot01;
    let inv_denom = 1.0 / (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
    let sgn_u = (if u == 0.0 { 0.0 } else { (if u > 0.0 { 1.0 } else if u < 0.0 { -1.0 } else { 0.0 }) });
    let sgn_v = (if v == 0.0 { 0.0 } else { (if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 }) });
    vec3(sqrt(u * u + q.x * q.x) * sgn_u, sqrt(v * v + q.y * q.y) * sgn_v, q.z)
}
