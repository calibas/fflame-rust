/// Elliptic rotation — adds `rotate` to the elliptic ν coordinate and wraps
/// the result into `[-π, π]`. Uses the `xmax·cos(ν)` and
/// `sqrt(xmax²−1)·sin(ν)` output form (matching `ecollide`) rather than the
/// cosh/sinh form used by the other E-series variations.
///
/// # Authors
/// - Michael Faber

// ERotate, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    rotate: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Rotate",
        hint: "Angular shift on ν, in radians.",
    },
};

const INFO = #{ title: "ERotate", name: "rhai_erotate", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let rotate = p.rotate;
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let tmp = z.im * z.im + z.re * z.re + 1.0;
    let tmp2 = 2.0 * z.re;
    let xmax = max(1.0, (sqrt(max(0.0, tmp + tmp2)) + sqrt(max(0.0, tmp - tmp2))) * 0.5);
    let t = clamp(z.re / xmax, -1.0, 1.0);
    let nu = acos(t);
    if (z.im < 0.0) {
        nu = -nu;
    }
    let v = nu + rotate + pi;
    nu = (v - to_float(floor(v / two_pi)) * two_pi) - pi;
    complex(xmax * cos(nu), sqrt(max(0.0, xmax - 1.0)) * sqrt(max(0.0, xmax + 1.0)) * sin(nu))
}

fn variation3(q, p) {
    let rotate = p.rotate;
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let tmp = q.y * q.y + q.x * q.x + 1.0;
    let tmp2 = 2.0 * q.x;
    let xmax = max(1.0, (sqrt(max(0.0, tmp + tmp2)) + sqrt(max(0.0, tmp - tmp2))) * 0.5);
    let t = clamp(q.x / xmax, -1.0, 1.0);
    let nu = acos(t);
    if (q.y < 0.0) {
        nu = -nu;
    }
    let v = nu + rotate + pi;
    nu = (v - to_float(floor(v / two_pi)) * two_pi) - pi;
    vec3(xmax * cos(nu), sqrt(max(0.0, xmax - 1.0)) * sqrt(max(0.0, xmax + 1.0)) * sin(nu), q.z)
}
