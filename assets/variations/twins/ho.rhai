/// 3D hyperbolic-octahedron mapping — outputs `(cos(x)·cos(y))^xpow +
/// xpow·cos(x)·cos(y) + 0.25·atan2(y², z²)` on X, with analogous formulas
/// on Y and Z. The power-plus-linear-plus-arctangent combination traces an
/// octahedral hyperbolic shape in 3D space.
///
/// # Authors
/// - Larry Berlin

// HO, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xpow: #{
        value: 3.0, min: -10.0, max: 10.0, free: true,
        caption: "X Power",
        hint: "Exponent applied to `cos(x)·cos(y)` in the X output.",
    },
    ypow: #{
        value: 3.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Power",
        hint: "Exponent applied to `sin(x)·cos(y)` in the Y output.",
    },
    zpow: #{
        value: 3.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Power",
        hint: "Exponent applied to `sin(y)` in the Z output.",
    },
};

const INFO = #{ title: "HO", name: "rhai_ho", category: "full_3d", phase: "any" };

fn sign_or_one(v) {
    (if v == 0.0 { 1.0 } else { (if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 }) })
}

fn sign_or_one_3d(v) {
    (if v == 0.0 { 1.0 } else { (if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 }) })
}

fn variation(z, p) {
    let xpow = p.xpow;
    let ypow = p.ypow;
    let u = z.re;
    let v = z.im;
    let cu = cos(u);
    let su = sin(u);
    let cv = cos(v);
    let sv = sin(v);
    let cucv = cu * cv;
    let sucv = su * cv;
    let at_x = atan2(v * v, 0.0);
    let at_y = atan2(u * u, 0.0);
    let x = pow(max(1.0e-30, abs(cucv)), xpow) * sign_or_one(cucv) + cucv * xpow + 0.25 * at_x;
    let y = pow(max(1.0e-30, abs(sucv)), ypow) * sign_or_one(sucv) + sucv * ypow + 0.25 * at_y;
    complex(x, y)
}

fn variation3(q, p) {
    let xpow = p.xpow;
    let ypow = p.ypow;
    let zpow = p.zpow;
    let u = q.x;
    let v = q.y;
    let w = q.z;
    let cu = cos(u);
    let su = sin(u);
    let cv = cos(v);
    let sv = sin(v);
    let cucv = cu * cv;
    let sucv = su * cv;
    let at_x = atan2(v * v, w * w);
    let at_y = atan2(u * u, w * w);
    let x = pow(max(1.0e-30, abs(cucv)), xpow) * sign_or_one_3d(cucv) + cucv * xpow + 0.25 * at_x;
    let y = pow(max(1.0e-30, abs(sucv)), ypow) * sign_or_one_3d(sucv) + sucv * ypow + 0.25 * at_y;
    let z = pow(max(1.0e-30, abs(sv)), zpow) * sign_or_one_3d(sv) + sv * zpow;
    vec3(x, y, z)
}
