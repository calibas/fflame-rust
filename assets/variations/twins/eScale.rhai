/// Elliptic scale + angular wrap — scales μ by `scale` and applies a scale-
/// plus-angle modular operation to ν, wrapping the result into `[-π, π]`.
/// Useful for periodic ellipse-tiled patterns.
///
/// # Authors
/// - Michael Faber

// EScale, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    scale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Multiplicative scale on μ; also scales the ν mod-wrap window.",
    },
    angle: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle",
        hint: "Angular offset applied to ν before scaling, in degrees.",
    },
};

const INFO = #{ title: "EScale", name: "rhai_escale", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let scale = p.scale;
    let angle_deg = p.angle;
    let angle = angle_deg * 3.14159265358979 / 180.0;
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let tmp = z.im * z.im + z.re * z.re + 1.0;
    let tmp2 = 2.0 * z.re;
    let xmax = max(1.0, (sqrt(max(0.0, tmp + tmp2)) + sqrt(max(0.0, tmp - tmp2))) * 0.5);
    let mu = acosh(xmax);
    let t = clamp(z.re / xmax, -1.0, 1.0);
    let nu = acos(t);
    if (z.im < 0.0) {
        nu = -nu;
    }
    mu = mu * scale;
    let two_pi_s = two_pi * scale;
    let safe_tps = (if abs(two_pi_s) < 1.0e-30 { 1.0e-30 } else { two_pi_s });
    let v1 = scale * (nu + pi + angle);
    let v1m = v1 - to_float(floor(v1 / safe_tps)) * safe_tps - angle - scale * pi;
    nu = v1m - to_float(floor(v1m / two_pi)) * two_pi;
    if (nu > pi) {
        nu = nu - two_pi;
    }
    if (nu < -pi) {
        nu = nu + two_pi;
    }
    complex(cosh(mu) * cos(nu), sinh(mu) * sin(nu))
}
