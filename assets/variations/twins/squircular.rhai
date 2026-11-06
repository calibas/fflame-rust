/// Squircular Möbius warp — maps the input through a "squircle"-style
/// transformation (intermediate between a circle and a square). The
/// variation weight enters non-linearly in the body, so the output shape
/// changes qualitatively with weight rather than just scaling.

// Squircular, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Squircular", name: "rhai_squircular", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let u = z.re;
    let v = z.im;
    let safe_v = (if abs(v) < 1.0e-30 { 1.0e-30 } else { v });
    let safe_u = (if abs(u) < 1.0e-30 { 1.0e-30 } else { u });
    let r0 = u * u + v * v;
    let rs = sqrt(max(0.0, r0));
    let xs = (if u > 0.0 { 1.0 } else { -1.0 });
    let r1 = sqrt(max(0.0, w * w * r0 - 4.0 * u * u * v * v));
    let inner = 1.0 + u * u / (safe_v * safe_v) - rs / (w * safe_v * safe_v) * r1;
    let r = sqrt(max(0.0, inner)) * 0.7071067811865476;
    let fx = xs * r;
    let fy = v / safe_u * r;
    complex(fx * inv_w, fy * inv_w)
}
