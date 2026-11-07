/// Glynn warp combined with a Gielis super-shape — same structure as
/// `glynnlissa` but uses a Gielis super-shape `r⁻¹ · (cos φ, sin φ)` (with
/// `m, n1, n2, n3` shape parameters) as the inner-curve sample instead of a
/// Lissajous. The Gielis super-shape can produce a wide variety of
/// symmetric organic shapes (stars, polygons, flowers, etc.) depending on
/// the parameters.

// Glynn S-Shape, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Outer cutoff radius. Points inside use the super-shape curve; outside use the Glynn power-warp.",
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
    m: #{
        value: 5.0, min: -50.0, max: 50.0, free: true,
        caption: "M",
        hint: "Super-shape symmetry parameter (number of lobes).",
    },
    n1: #{
        value: 1.7, min: -10.0, max: 10.0, free: true,
        caption: "N1",
        hint: "Super-shape exponent 1 — outer envelope shape (controls overall sharpness).",
    },
    n2: #{
        value: 1.7, min: -10.0, max: 10.0, free: true,
        caption: "N2",
        hint: "Super-shape exponent 2 — controls the cos-side shape.",
    },
    n3: #{
        value: 1.7, min: -10.0, max: 10.0, free: true,
        caption: "N3",
        hint: "Super-shape exponent 3 — controls the sin-side shape.",
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

const INFO = #{ title: "Glynn S-Shape", name: "rhai_glynnsshape", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let phi_rad = p.phi1 * pi / 180.0;
    #{ x1: p.radius * cos(phi_rad), y1: p.radius * sin(phi_rad), abs_pow: abs(p.pow) }
}

fn glynnSShape_super(m, n1, n2, n3, phi) {
    let half_phi = m * phi * 0.25;
    let t1 = pow(max(1.0e-30, abs(cos(half_phi))), n2);
    let t2 = pow(max(1.0e-30, abs(sin(half_phi))), n3);
    let safe_n1 = (if abs(n1) < 1.0e-30 { 1.0e-30 } else { n1 });
    let r = pow(max(1.0e-30, t1 + t2), 1.0 / safe_n1);
    if (abs(r) < 1.0e-30) {
        return complex(0.0, 0.0);
    }
    let inv_r = 1.0 / r;
    complex(inv_r * cos(phi), inv_r * sin(phi))
}

fn variation(w, p) {
    let radius = p.radius;
    let radius1 = p.radius1;
    let thickness = p.thickness;
    let m = p.m;
    let n1 = p.n1;
    let n2 = p.n2;
    let n3 = p.n3;
    let scale = p.scale;
    let contrast = p.contrast;
    let x1 = p.x1;
    let y1 = p.y1;
    let abs_pow = p.abs_pow;
    let two_pi = 6.28318530717959;
    let phi = rand() * two_pi;
    let sp = glynnSShape_super(m, n1, n2, n3, phi);
    let r = sqrt(w.re * w.re + w.im * w.im);
    if (r < abs(radius)) {
        if (radius1 >= 0.0) {
            return complex(sp.re * scale * radius1 + x1, sp.im * scale * radius1 + y1);
        }
        let cr = radius1 * (thickness + (1.0 - thickness) * rand());
        let cphi = two_pi * rand();
        return complex(cr * cos(cphi) + x1, cr * sin(cphi) + y1);
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
        return complex(sp.re * scale * radius1 + x1, sp.im * scale * radius1 + y1);
    }
    complex(xi, yi)
}
