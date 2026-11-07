/// Elliptic-coordinate Julia — converts the input to elliptic coordinates
/// `(μ, ν)`, divides both by `power`, adds a random angular branch
/// `2π·floor(rand·|power|)/power` to ν, then emits `(cosh(μ)·cos(ν),
/// sinh(μ)·sin(ν))`. With negative `power`, the input is first inverted
/// through the unit circle. Produces N-fold rotationally symmetric Julia-
/// style elliptic patterns.
///
/// # Authors
/// - Michael Faber

// E-Julia, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 2, min: -100, max: 100,
        caption: "Power",
        hint: "Number of angular branches. Negative values invert the input through the unit circle first.",
    },
};

const INFO = #{ title: "E-Julia", name: "rhai_ejulia", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ sign: (if to_float(p.power) < 0.0 { -1.0 } else { 1.0 }) }
}

fn variation(z, p) {
    let power = to_float(p.power);
    let sign = p.sign;
    let two_pi = 6.28318530717959;
    let safe_power = (if abs(power) < 1.0e-30 { 1.0 } else { power });
    let abs_power = max(1.0, abs(safe_power));
    let r2 = z.im * z.im + z.re * z.re;
    let x = 0.0;
    if (sign == 1.0) {
        x = z.re;
    } else {
        r2 = 1.0 / max(1.0e-30, r2);
        x = z.re * r2;
    }
    let tmp = r2 + 1.0;
    let tmp2 = 2.0 * x;
    let xmax_raw = (sqrt(max(0.0, tmp + tmp2)) + sqrt(max(0.0, tmp - tmp2))) * 0.5;
    let xmax = max(1.0, xmax_raw);
    let mu = acosh(xmax) / safe_power;
    let t = clamp(x / xmax, -1.0, 1.0);
    let nu = acos(t);
    if (z.im < 0.0) {
        nu = -nu;
    }
    nu = nu / safe_power + two_pi / safe_power * to_float(floor(rand() * abs_power));
    complex(cosh(mu) * cos(nu), sinh(mu) * sin(nu))
}
