/// Adds randomness in both rotation (spin) and scale (zoom) around the
/// origin. The angle slider controls the mix — 0° = pure zoom blur,
/// 90° = balanced, 180° = pure rotational blur.
///
/// flam3 / JWildfire store this parameter in half-turn units
/// (`spin = sin(angle · π/2)`, so 1.0 = pure spin, JWF default 0.5 =
/// balanced) — our degrees relate by `degrees = jwf_value × 180`,
/// converted at the .flame XML boundary (see `flame_xml.rs`).
///
/// Weight semantics follow JWF's `RadialBlurFunc.transform`: `pAmount`
/// scales spin/zoom only, while the `ra` rotation term and the `−1` in
/// `rz` stay unscaled. The WGSL bakes the weight in and pre-divides by
/// it to cancel the dispatcher's outer multiply (idisc pattern).
///
/// # Authors
/// - Scott Draves
/// - Andreas Maschke

// Radial Blur, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    angle: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle",
        hint: "Spin/zoom balance. 0 degrees = pure zoom blur, 180 degrees = pure rotational blur, 90 degrees = balanced mix.",
    },
};

const INFO = #{ title: "Radial Blur", name: "rhai_radial_blur", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let half_angle_rad = p.angle * 3.14159265358979 / 360.0;
    #{ d0: sin(half_angle_rad), d1: cos(half_angle_rad) }
}

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let spin = w * p.d0;
    let zoom = w * p.d1;
    let rnd_g = rand() + rand() + rand() + rand() - 2.0;
    let ra = sqrt(z.re * z.re + z.im * z.im);
    let alpha = atan2(z.im, z.re) + spin * rnd_g;
    let rz = zoom * rnd_g - 1.0;
    complex((ra * cos(alpha) + rz * z.re) * inv_w, (ra * sin(alpha) + rz * z.im) * inv_w)
}
