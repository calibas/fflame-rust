/// Glynn-set with two-radius branching — generator picks between two
/// circles (`r1` and `r2 = radius²/r1`) with probability γ.
///
/// # Authors
/// - eralex61

// Glynn Sim 3, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: -5.0, max: 5.0, free: true,
        caption: "Radius",
        hint: "Main Glynn-set inversion radius.",
    },
    thickness: #{
        value: 0.1, min: 0.0, max: 1.0,
        caption: "Thickness",
        hint: "Outer-circle thickness — combined with radius to form `r1` and `r2 = radius² / r1`.",
    },
    contrast: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Contrast",
        hint: "Probability scaling for the inversion-vs-pass-through branch.",
    },
    pow: #{
        value: 1.5, min: -10.0, max: 10.0, free: true,
        caption: "Pow",
        hint: "Power for the contrast probability.",
    },
};

const INFO = #{ title: "Glynn Sim 3", name: "rhai_glynnsim3", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let radius = p.radius;
    let thickness = p.thickness;
    let pow_p = p.pow;
    let r1 = radius + thickness;
    let safe_r1 = (if r1 == 0.0 { 1.0e-30 } else { r1 });
    let r2 = (radius * radius) / safe_r1;
    #{ r1: r1, r2: r2, gamma: r1 / max(1.0e-30, r1 + r2), abs_pow: abs(pow_p) }
}

fn variation(z, p) {
    let radius = p.radius;
    let contrast = p.contrast;
    let r1 = p.r1;
    let r2 = p.r2;
    let gamma = p.gamma;
    let abs_pow = p.abs_pow;
    let r = sqrt(z.re * z.re + z.im * z.im);
    let alpha = radius / max(1.0e-30, r);
    if (r < r1) {
        let phi = 6.28318530717959 * rand();
        let rr = r2;
        if (rand() < gamma) {
            rr = r1;
        }
        return complex(rr * cos(phi), rr * sin(phi));
    }
    if (rand() > contrast * pow(alpha, abs_pow)) {
        return z;
    }
    complex(alpha * alpha * z.re, alpha * alpha * z.im)
}
