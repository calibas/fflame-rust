/// Random-branched Glynn-set warp — splits into 4 branches per iteration
/// based on radius and a random coin flip. Produces the characteristic
/// organic Glynn fractal shapes.
///
/// # Authors
/// - Michael Faber

// Glynnia, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Glynnia", name: "rhai_glynnia", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let half_sqrt2 = 0.7071067811865476;
    let r = sqrt(z.re * z.re + z.im * z.im);
    if (r >= 1.0) {
        if (rand() > 0.5) {
            let d = sqrt(max(0.0, r + z.re));
            if (d == 0.0) {
                return complex(0.0, 0.0);
            }
            return complex(half_sqrt2 * d, -half_sqrt2 / d * z.im);
        }
        let d = r + z.re;
        let dx = sqrt(max(0.0, r * (z.im * z.im + d * d)));
        if (dx == 0.0) {
            return complex(0.0, 0.0);
        }
        return complex(d / dx, z.im / dx);
    }
    if (rand() > 0.5) {
        let d = sqrt(max(0.0, r + z.re));
        if (d == 0.0) {
            return complex(0.0, 0.0);
        }
        return complex(-half_sqrt2 * d, -half_sqrt2 / d * z.im);
    }
    let d = r + z.re;
    let dx = sqrt(max(0.0, r * (z.im * z.im + d * d)));
    if (dx == 0.0) {
        return complex(0.0, 0.0);
    }
    complex(-d / dx, z.im / dx)
}
