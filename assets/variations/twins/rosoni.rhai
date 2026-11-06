/// Clipping/rotational kaleidoscope — inside a `cutoff` radius, iterates
/// the input through up to `maxiter` rotations of `2π/maxiter`, XOR-
/// toggling a `cerc` flag based on whether each rotated point falls inside
/// an inner test shape (circle/square or lemniscate/angle, controlled by
/// `altshapes`). The output is the position at iteration `sweetiter` if the
/// toggle ends true; otherwise the input passes through. Outside the
/// cutoff, the input always passes through unchanged.
///
/// # Authors
/// - DarkBeam

// Rosoni, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    maxiter: #{
        value: 25, min: 1, max: 100,
        caption: "Max Iter",
        hint: "Number of rotation iterations (each rotates by `2π/maxiter`).",
    },
    sweetiter: #{
        value: 3, min: 0, max: 100,
        caption: "Sweet Iter",
        hint: "Which iteration's position is used as the output when the toggle ends true.",
    },
    altshapes: #{
        value: false,
        caption: "Alt Shapes",
        hint: "When on, use the lemniscate/angle inner-shape family. When off, circle/square.",
    },
    cutoff: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cutoff",
        hint: "Outer-region radius. Positive = circular cutoff; negative = square (Chebyshev) cutoff.",
    },
    radius: #{
        value: 0.4, min: -10.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Inner-shape size. Positive = filled-shape test; negative = wireframe via Chebyshev distance or atan2 angle.",
    },
    dx: #{
        value: 0.6, min: -10.0, max: 10.0, free: true,
        caption: "DX",
        hint: "Inner-shape X offset (also acts as the rotation pivot).",
    },
    dy: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "DY",
        hint: "Inner-shape Y offset.",
    },
};

const INFO = #{ title: "Rosoni", name: "rhai_rosoni", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let two_pi = 6.28318530717959;
    let phi = two_pi / max(1.0, to_float(p.maxiter));
    #{ sina: sin(phi), cosa: cos(phi) }
}

fn variation(z, p) {
    let maxiter_f = max(1.0, to_float(p.maxiter));
    let sweetiter = to_int(to_float(p.sweetiter));
    let altshapes = to_int(to_float(p.altshapes));
    let cutoff = p.cutoff;
    let radius = p.radius;
    let dx = p.dx;
    let dy = p.dy;
    let sina = p.sina;
    let cosa = p.cosa;
    let inv_pi = 0.31830988618379;
    let r = sqrt(z.re * z.re + z.im * z.im) - cutoff;
    if (cutoff < 0.0) {
        r = max(abs(z.re), abs(z.im)) + cutoff;
    }
    if (r > 0.0) {
        return z;
    }
    let cerc = dx > 0.0;
    let xrt = z.re;
    let yrt = z.im;
    let sweetx = xrt;
    let sweety = yrt;
    let maxiter_i = to_int(maxiter_f);
    for i in 0..maxiter_i {
        let r2 = 0.0;
        if (altshapes == 0) {
            let dxx = xrt - dx;
            let dyy = yrt - dy;
            r2 = dxx * dxx + dyy * dyy - radius * radius;
            if (radius < 0.0) {
                r2 = max(abs(xrt - dx), abs(z.im - dy)) + radius;
            }
        } else {
            let dxx = xrt - dx;
            let dyy = yrt - dy;
            r2 = (if (xrt - dx) < 0.0 { -(xrt - dx) } else { (dyy * dyy) - dxx * dxx * (radius * radius - dxx * dxx) });
            if (radius < 0.0) {
                r2 = abs(atan2(z.im - dy, dxx)) * inv_pi + radius;
            }
        }
        if (r2 <= 0.0) {
            cerc = !cerc;
        }
        if (i == sweetiter) {
            sweetx = xrt;
            sweety = yrt;
        }
        let swp = xrt * cosa - yrt * sina;
        yrt = xrt * sina + yrt * cosa;
        xrt = swp;
    }
    if (cerc) {
        if (sweetiter == 0) {
            let fx = (if dy != 0.0 { -sweetx } else { sweetx });
            return complex(fx, -sweety);
        }
        return complex(sweetx, sweety);
    }
    z
}
