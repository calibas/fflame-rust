// Round Spher, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Round Spher", name: "rhai_roundspher" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let two_over_pi_sq = 0.40528473456935106;
    let d = max(z.re * z.re + z.im * z.im, 1.0e-30);
    let e = 1.0 / d + two_over_pi_sq;
    let safe_e = (if abs(e) < 1.0e-30 { 1.0e-30 } else { e });
    let fx = w * (w / d * z.re / safe_e);
    let fy = w * (w / d * z.im / safe_e);
    complex(fx * inv_w, fy * inv_w)
}
