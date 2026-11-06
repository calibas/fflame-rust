// Round Spher 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Round Spher 3D", name: "rhai_roundspher3d" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let two_over_pi_sq = 0.40528473456935106;
    let r2 = z.re * z.re + z.im * z.im;
    let d = max(r2, 1.0e-30);
    let e = 1.0 / d + two_over_pi_sq;
    let safe_e = (if abs(e) < 1.0e-30 { 1.0e-30 } else { e });
    let de = d * safe_e;
    complex(w * z.re / de, w * z.im / de)
}

fn variation3(v, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let two_over_pi_sq = 0.40528473456935106;
    let r2 = v.x * v.x + v.y * v.y;
    let f = sqrt(r2);
    let tempTZ = (if v.z == 0.0 { cos(f) } else { v.z });
    let inject_z = (if ctx.accum.z == 0.0 { cos(f) } else { 0.0 }) * inv_w;
    let d = max(r2 + tempTZ * tempTZ, 1.0e-30);
    let e = 1.0 / d + two_over_pi_sq;
    let safe_e = (if abs(e) < 1.0e-30 { 1.0e-30 } else { e });
    let de = d * safe_e;
    vec3(w * v.x / de, w * v.y / de, inject_z + w * tempTZ / de)
}
