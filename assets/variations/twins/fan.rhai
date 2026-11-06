/// Affine-driven fan sweep — wraps points into pie wedges sized by the
/// affine's X-translation field (`e`). The wedge angle and rotation are
/// read directly from the transform's affine matrix.
///
/// # Authors
/// - Scott Draves

// Fan, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Fan", name: "rhai_fan", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let dx = 3.14159265358979 * ctx.e * ctx.e + 1.0e-6;
    let dx2 = 0.5 * dx;
    let theta = atan2(z.re, z.im);
    let m = theta + ctx.f - to_float(trunc((theta + ctx.f) / dx)) * dx;
    let a = 0.0;
    if (m > dx2) {
        a = theta - dx2;
    } else {
        a = theta + dx2;
    }
    let r = sqrt(z.re * z.re + z.im * z.im);
    complex(r * cos(a), r * sin(a))
}
