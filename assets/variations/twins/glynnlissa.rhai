/// Glynn warp combined with a Lissajous curve — inside the cutoff radius
/// the output samples a Lissajous point (when `radius1 ≥ 0`) or a small
/// jittered circle (when `radius1 < 0`). Outside the cutoff, applies a
/// Glynn-style power-warp with a probabilistic toggle, then a secondary
/// Lissajous fill if the warped point falls inside the inner Lissajous
/// disc.

// Glynn Lissa, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Outer cutoff radius. Points inside use the inner curve; outside use the Glynn power-warp.",
    },
    radius1: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Radius 1",
        hint: "Inner-curve sampling radius. Negative values trigger a small-circle fallback inside the cutoff.",
    },
    thickness: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Thickness",
        hint: "Inner-circle thickness (used when `radius1 < 0`).",
    },
    phi1: #{
        value: 0.0, min: -360.0, max: 360.0, free: true,
        caption: "Phi 1",
        hint: "Inner-curve center angle, in degrees.",
    },
    a: #{
        value: 3.0, min: -50.0, max: 50.0, free: true,
        caption: "A",
        hint: "Lissajous X-axis frequency.",
    },
    b: #{
        value: 2.0, min: -50.0, max: 50.0, free: true,
        caption: "B",
        hint: "Lissajous Y-axis frequency.",
    },
    width: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Width",
        hint: "Y-jitter amplitude (random per-iteration scatter on the inner-curve sample).",
    },
    phase: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Phase",
        hint: "Lissajous phase offset on the X frequency term.",
    },
    scale: #{
        value: 0.71, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Inner-curve scale factor.",
    },
    pow: #{
        value: 1.5, min: -10.0, max: 10.0, free: true,
        caption: "Pow",
        hint: "Glynn power exponent (absolute value used internally).",
    },
    contrast: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Contrast",
        hint: "Glynn warp probability threshold — higher = more aggressive warping outside the cutoff.",
    },
};

const INFO = #{ title: "Glynn Lissa", name: "rhai_glynnlissa", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let phi_rad = p.phi1 * pi / 180.0;
    #{ x1: p.radius * cos(phi_rad), y1: p.radius * sin(phi_rad), abs_pow: abs(p.pow) }
}

fn variation(w, p) {
    let radius = p.radius;
    let radius1 = p.radius1;
    let thickness = p.thickness;
    let a_p = p.a;
    let b_p = p.b;
    let width = p.width;
    let phase = p.phase;
    let scale = p.scale;
    let contrast = p.contrast;
    let x1 = p.x1;
    let y1 = p.y1;
    let abs_pow = p.abs_pow;
    let two_pi = 6.28318530717959;
    let t = rand() * two_pi;
    let lx = sin(a_p * t + phase);
    let ly = sin(b_p * t);
    let r = sqrt(w.re * w.re + w.im * w.im);
    let y_jit = rand() - 0.5;
    if (r < abs(radius)) {
        if (radius1 >= 0.0) {
            return complex((lx * scale + width * y_jit) * radius1 + x1, (ly * scale + width * y_jit) * radius1 + y1);
        }
        let cr = radius1 * (thickness + (1.0 - thickness) * rand());
        let phi = two_pi * rand();
        return complex(cr * cos(phi) + x1, cr * sin(phi) + y1);
    }
    let alpha = abs(radius) / max(1.0e-30, r);
    let xi = 0.0;
    let yi = 0.0;
    if (rand() > contrast * pow(max(1.0e-30, alpha), abs_pow)) {
        xi = w.re;
        yi = w.im;
    } else {
        xi = alpha * alpha * w.re;
        yi = alpha * alpha * w.im;
    }
    let dx = xi - x1;
    let dy = yi - y1;
    let z = dx * dx + dy * dy;
    if (z < radius1 * radius1) {
        return complex((lx * scale + width * y_jit) * radius1 + x1, (ly * scale + width * y_jit) * radius1 + y1);
    }
    complex(xi, yi)
}
