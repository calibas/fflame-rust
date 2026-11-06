// Inv Squircular, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inv Squircular", name: "rhai_invsquircular" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let safe_w = (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let u = z.re;
    let v = z.im;
    let r0 = u * u + v * v;
    let inner = r0 * (safe_w * safe_w * r0 - 4.0 * u * u * v * v) / safe_w;
    let r2 = sqrt(max(inner, 0.0));
    let r = sqrt(max(r0 - r2, 0.0)) * 0.7071067811865476;
    let safe_u = (if abs(u) < 1.0e-30 { 1.0e-30 } else { u });
    let safe_v = (if abs(v) < 1.0e-30 { 1.0e-30 } else { v });
    complex(r / safe_u * inv_w, r / safe_v * inv_w)
}
