// Delta A, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Delta A", name: "rhai_deltaa" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let denom = sqrt(max(z.im * z.im + (z.re - 1.0) * (z.re - 1.0), 1.0e-30));
    let num = sqrt(max(z.im * z.im + (z.re + 1.0) * (z.re + 1.0), 1.0e-30));
    let avgr = w * (num / denom);
    let avga = (atan2(z.im, z.re - 1.0) - atan2(z.im, z.re + 1.0)) * 0.5;
    complex(avgr * cos(avga) * inv_w, avgr * sin(avga) * inv_w)
}
