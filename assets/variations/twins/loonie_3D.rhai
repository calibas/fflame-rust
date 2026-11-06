// Loonie 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Loonie 3D", name: "rhai_loonie_3d" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let sqrvvar = w * w;
    let ef_z = atan2(z.im, z.re);
    let r2 = z.re * z.re + z.im * z.im + ef_z * ef_z;
    if (r2 < sqrvvar) {
        let scale = sqrt(max(sqrvvar / max(r2, 1.0e-30) - 1.0, 0.0));
        return complex(scale * z.re, scale * z.im);
    }
    z
}

fn variation3(v, p, ctx) {
    let w = ctx.weight;
    let sqrvvar = w * w;
    let ef_z = v.z;
    if (ef_z == 0.0) {
        ef_z = atan2(v.y, v.x);
    }
    let r2 = v.x * v.x + v.y * v.y + ef_z * ef_z;
    if (r2 < sqrvvar) {
        let scale = sqrt(max(sqrvvar / max(r2, 1.0e-30) - 1.0, 0.0));
        return vec3(scale * v.x, scale * v.y, scale * ef_z * 0.5);
    }
    vec3(v.x, v.y, ef_z * 0.5)
}
