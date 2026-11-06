/// Glynn-set with arc-segment generator — like GlynnSim1 but the generator
/// spawns points along an arc instead of a full circle. `phi1` / `phi2`
/// define the arc bounds.
///
/// # Authors
/// - eralex61

// Glynn Sim 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: -5.0, max: 5.0, free: true,
        caption: "Radius",
        hint: "Main Glynn-set inversion radius.",
    },
    thickness: #{
        value: 0.1, min: 0.0, max: 1.0,
        caption: "Thickness",
        hint: "Arc thickness — fraction of the radius.",
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
    phi1: #{
        value: 110.0, min: -360.0, max: 360.0, free: true,
        caption: "Phi 1",
        hint: "Start angle of the arc segment (degrees).",
    },
    phi2: #{
        value: 150.0, min: -360.0, max: 360.0, free: true,
        caption: "Phi 2",
        hint: "End angle of the arc segment (degrees).",
    },
};

const INFO = #{ title: "Glynn Sim 2", name: "rhai_glynnsim2", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let radius = p.radius;
    let thickness = p.thickness;
    let pow_p = p.pow;
    let phi1 = p.phi1;
    let phi2 = p.phi2;
    let pi180 = 3.14159265358979 / 180.0;
    let phi10 = pi180 * phi1;
    let phi20 = pi180 * phi2;
    #{ phi10: phi10, gamma: thickness * (2.0 * radius + thickness) / max(1.0e-30, radius + thickness), delta: phi20 - phi10, abs_pow: abs(pow_p), rad_plus_thick: radius + thickness }
}

fn variation(z, p) {
    let radius = p.radius;
    let contrast = p.contrast;
    let phi10 = p.phi10;
    let gamma = p.gamma;
    let delta = p.delta;
    let abs_pow = p.abs_pow;
    let rad_plus_thick = p.rad_plus_thick;
    let r = sqrt(z.re * z.re + z.im * z.im);
    let alpha = radius / max(1.0e-30, r);
    if (r < radius) {
        let r2 = rad_plus_thick - gamma * rand();
        let phi = phi10 + delta * rand();
        return complex(r2 * cos(phi), r2 * sin(phi));
    }
    if (rand() > contrast * pow(alpha, abs_pow)) {
        return z;
    }
    complex(alpha * alpha * z.re, alpha * alpha * z.im)
}
