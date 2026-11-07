/// 3D version of Butterfly — same XY butterfly curve plus a Z component
/// that scales with the radial distance times `|2y|`.

// Butterfly 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Butterfly 3D", name: "rhai_butterfly3d", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let k = 1.302940031741119;
    let y2 = z.im * 2.0;
    let r = k * sqrt(abs(z.im * z.re) / (1.0e-30 + z.re * z.re + y2 * y2));
    complex(r * z.re, r * y2)
}

fn variation3(v, p) {
    let k = 1.302940031741119;
    let y2 = v.y * 2.0;
    let r = k * sqrt(abs(v.y * v.x) / (1.0e-30 + v.x * v.x + y2 * y2));
    let z_out = r * abs(y2) * sqrt(v.x * v.x + v.y * v.y) / 4.0;
    vec3(r * v.x, r * y2, z_out)
}
