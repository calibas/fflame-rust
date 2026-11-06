/// Classic fisheye distortion — `2r/(r+1)` radial scaling that pulls
/// distant points inward. Note: preserves a long-standing Apophysis X/Y
/// swap bug; use Eyefish for the corrected form.
///
/// # Authors
/// - Scott Draves

// Fisheye, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Fisheye", name: "rhai_fisheye", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r0 = sqrt(z.re * z.re + z.im * z.im);
    let r = 2.0 * r0 / (r0 + 1.0);
    let denom = r0 + 1.0e-6;
    complex(r * z.im / denom, r * z.re / denom)
}
