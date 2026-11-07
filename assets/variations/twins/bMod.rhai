/// Bipolar warp with τ-axis clamp-and-mod — converts to bipolar
/// coordinates, then wraps τ around within `±radius` (offset by
/// `distance·radius`). Points with τ outside the band pass through
/// unchanged.
///
/// # Authors
/// - Michael Faber

// BMod, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Half-width of the τ band to mod-wrap.",
    },
    distance: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Distance",
        hint: "Additional τ offset applied before mod, in units of `radius`.",
    },
};

const INFO = #{ title: "BMod", name: "rhai_bmod", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let radius = p.radius;
    let distance = p.distance;
    let pi = 3.14159265358979;
    let tau = 0.5 * (ln(max(1.0e-30, (z.re + 1.0) * (z.re + 1.0) + z.im * z.im)) - ln(max(1.0e-30, (z.re - 1.0) * (z.re - 1.0) + z.im * z.im)));
    let sigma = pi - atan2(z.im, z.re + 1.0) - atan2(z.im, 1.0 - z.re);
    if (tau < radius && -tau < radius) {
        let two_r = 2.0 * radius;
        let safe_two_r = (if abs(two_r) < 1.0e-30 { 1.0e-30 } else { two_r });
        let v = tau + radius + distance * radius;
        tau = (v - to_float(floor(v / safe_two_r)) * safe_two_r) - radius;
    }
    let temp = cosh(tau) - cos(sigma);
    if (temp == 0.0) {
        return complex(0.0, 0.0);
    }
    complex(sinh(tau) / temp, sin(sigma) / temp)
}
