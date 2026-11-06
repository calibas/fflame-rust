/// Glynn-set inversion with an offset-circle generator — points inside the
/// radius spawn new points on a circle at `(x1, y1)`; outside, points get
/// inverted with probabilistic threshold.
///
/// # Authors
/// - eralex61

// Glynn Sim 1, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: -5.0, max: 5.0, free: true,
        caption: "Radius",
        hint: "Main Glynn-set inversion radius.",
    },
    radius1: #{
        value: 0.1, min: -5.0, max: 5.0, free: true,
        caption: "Radius 1",
        hint: "Offset-circle radius for the generator.",
    },
    phi1: #{
        value: 110.0, min: -360.0, max: 360.0, free: true,
        caption: "Phi 1",
        hint: "Angular position of the offset circle (degrees).",
    },
    thickness: #{
        value: 0.1, min: 0.0, max: 1.0,
        caption: "Thickness",
        hint: "Circle thickness — fraction of the radius (0 = points on the boundary, 1 = filled disc).",
    },
    pow: #{
        value: 1.5, min: -10.0, max: 10.0, free: true,
        caption: "Pow",
        hint: "Power for the contrast probability — higher concentrates points near the boundary.",
    },
    contrast: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Contrast",
        hint: "Probability scaling for the inversion-vs-pass-through branch.",
    },
};

const INFO = #{ title: "Glynn Sim 1", name: "rhai_glynnsim1", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let radius = p.radius;
    let phi1 = p.phi1;
    let pow_p = p.pow;
    let a = 3.14159265358979 * phi1 / 180.0;
    #{ x1: radius * cos(a), y1: radius * sin(a), abs_pow: abs(pow_p) }
}

fn variation(z, p) {
    let radius = p.radius;
    let radius1 = p.radius1;
    let thickness = p.thickness;
    let contrast = p.contrast;
    let x1 = p.x1;
    let y1 = p.y1;
    let abs_pow = p.abs_pow;
    let r = sqrt(z.re * z.re + z.im * z.im);
    let alpha = radius / max(1.0e-30, r);
    let tx = z.re;
    let ty = z.im;
    let generate = false;
    if (r < radius) {
        generate = true;
    } else {
        if (rand() > contrast * pow(alpha, abs_pow)) {
            tx = z.re;
            ty = z.im;
        } else {
            tx = alpha * alpha * z.re;
            ty = alpha * alpha * z.im;
        }
        let zd = (tx - x1) * (tx - x1) + (ty - y1) * (ty - y1);
        if (zd < radius1 * radius1) {
            generate = true;
        }
    }
    if (generate) {
        let rr = radius1 * (thickness + (1.0 - thickness) * rand());
        let phi = 6.28318530717959 * rand();
        tx = rr * cos(phi) + x1;
        ty = rr * sin(phi) + y1;
    }
    complex(tx, ty)
}
