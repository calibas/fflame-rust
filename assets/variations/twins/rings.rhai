/// Modular-radius ring warp — folds the input radius around `2·dx` cycles
/// where `dx = (transform's affine e coefficient)² + ε`, producing
/// concentric rings. Reads the affine `e` coefficient directly (not a
/// normal variation parameter), so the ring spacing changes with the
/// transform's pre-affine.
///
/// # Authors
/// - Scott Draves

// Rings, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Rings", name: "rhai_rings", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let dx = ctx.e * ctx.e + 1.0e-30;
    let r0 = sqrt(z.re * z.re + z.im * z.im) + 1.0e-30;
    let r = r0 + dx - to_float(floor((r0 + dx) / (2.0 * dx))) * 2.0 * dx - dx + r0 * (1.0 - dx);
    complex(r * z.im / r0 * inv_w, r * z.re / r0 * inv_w)
}
