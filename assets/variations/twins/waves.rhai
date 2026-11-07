/// Adds sine-wave displacement to each coordinate, using the affine
/// matrix's own b/c/d/f fields as wave parameters. Inherits its frequency
/// and amplitude from the transform itself rather than from extra sliders.
///
/// # Authors
/// - Scott Draves

// Waves, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Waves", name: "rhai_waves", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    complex(z.re + ctx.b * sin(z.im / (ctx.e * ctx.e + 1.0e-6)), z.im + ctx.d * sin(z.re / (ctx.f * ctx.f + 1.0e-6)))
}
