/// Cosine-wave radial distortion — applies a radial cosine wave centered at
/// `(centerx, centery)` with adjustable `frequency`, `velocity` (phase),
/// `amplitude`, and `scale`. Linearly interpolates between two phase-
/// shifted positions via `phase`. `fixed_dist_calc` toggles whether the
/// distance uses Euclidean `sqrt(x² + y²)` or the upstream-quirk product
/// `sqrt(x²·y²)`.
///
/// # Authors
/// - Xyrus02

// Ripple, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    frequency: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Frequency",
        hint: "Spatial frequency of the cosine wave (scaled by 5 internally).",
    },
    velocity: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Velocity",
        hint: "Phase velocity multiplier (× the internal `phase·2π − π` term).",
    },
    amplitude: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Amplitude",
        hint: "Wave amplitude (× 0.01 internally).",
    },
    centerx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Center X",
        hint: "X-axis center of the radial wave.",
    },
    centery: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Center Y",
        hint: "Y-axis center of the radial wave.",
    },
    phase: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Phase",
        hint: "Phase interpolation factor: 0 = first phase-shifted position, 1 = second.",
    },
    scale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Input pre-scale factor.",
    },
    fixed_dist_calc: #{
        value: false,
        caption: "Fixed Dist Calc",
        hint: "When on, use Euclidean distance `sqrt(x² + y²)`. When off, the upstream-quirk product `sqrt(x²·y²)`.",
    },
};

const INFO = #{ title: "Ripple", name: "rhai_ripple", category: "advanced_2d", phase: "any", replace: true };

fn prepare(p) {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let f = p.frequency * 5.0;
    let a_local = p.amplitude * 0.01;
    let p_local = p.phase * two_pi - pi;
    let s = (if abs(p.scale) < 1.0e-30 { 1.0e-30 } else { p.scale });
    let vxp = p.velocity * p_local;
    let pxa = p_local * a_local;
    let pixa = (pi - p_local) * a_local;
    #{ _f: f, _p: p_local, _s: s, _vxp: vxp, _pxa: pxa, _pixa: pixa }
}

fn variation(z, p) {
    let centerx = p.centerx;
    let centery = p.centery;
    let fixed_dc = to_float(p.fixed_dist_calc);
    let _f = p._f;
    let _p = p._p;
    let _s = p._s;
    let _vxp = p._vxp;
    let _pxa = p._pxa;
    let _pixa = p._pixa;
    let _is = 1.0 / _s;
    let x = z.re * _s - centerx;
    let y = z.im * _s + centery;
    let d = 0.0;
    if (fixed_dc != 0.0) {
        d = sqrt(x * x + y * y);
    } else {
        d = sqrt(x * x * y * y);
    }
    d = max(1.0e-30, d);
    let nx = x / d;
    let ny = y / d;
    let wave = cos(_f * d - _vxp);
    let d1 = wave * _pxa + d;
    let d2 = wave * _pixa + d;
    let u1 = centerx + nx * d1;
    let v1 = -centery + ny * d1;
    let u2 = centerx + nx * d2;
    let v2 = -centery + ny * d2;
    let lx = u1 + (u2 - u1) * _p;
    let ly = v1 + (v2 - v1) * _p;
    complex(lx * _is, ly * _is)
}
