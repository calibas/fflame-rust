/// Hyperbolic tiling via Möbius reflections in the Poincaré disc.
/// Reflects the sample point across three hyperbolic lines (one
/// straight, two circular arcs at `π/8` and a `√(√2 + 1)` radius
/// circle perpendicular to the unit disc), counting reflections to
/// produce a black/white tile pattern.
///
/// 2 params: `density_pixels`, `gradient`. Output is grayscale (the
/// JWildfire source returns `vec3(color, color, color)`).
///
/// **Note on control flow**: JWildfire's source uses nested loops
/// with `continue` and early-termination flags. We mirror the
/// structure exactly so the output matches.
/// 
/// # Authors
/// - Jesus Sosa

// GLSL Hyperbolic Tile, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution.",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.** This variation produces grayscale output regardless of palette.",
    },
};

const INFO = #{ title: "GLSL Hyperbolic Tile", name: "rhai_glsl_hyperbolictile", category: "plugin", phase: "any", z: "never" };

fn hyperbolictile_tocart(polar) {
    complex(polar.re * cos(polar.im), polar.re * sin(polar.im))
}

fn hyperbolictile_topolar(c) {
    complex(sqrt(c.re * c.re + c.im * c.im), atan2(c.im, c.re))
}

fn hyperbolictile_mirror(line, point) {
    if (line.re < 100.0) {
        let linecenter = hyperbolictile_tocart(line);
        let cart = hyperbolictile_tocart(point);
        let dx = cart.re - linecenter.re;
        let dy = cart.im - linecenter.im;
        let r1 = line.re * line.re - 1.0;
        let r2 = max(1.0e-32, dx * dx + dy * dy);
        let rr = r1 / r2;
        hyperbolictile_topolar(linecenter + complex(dx * rr, dy * rr))
    } else {
        complex(point.re, -point.im + 2.0 * line.im + 3.14159265)
    }
}

fn variation(z, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let x = i_f + 0.5;
    let y = j_f + 0.5;
    let position = complex(2.0 * x / resolution - 1.0, 2.0 * y / resolution - 1.0);
    let pi = 3.14159265;
    let pi_2 = pi * 0.5;
    let pi_3_4 = pi * 0.75;
    let l1 = complex(1000.0, 0.0);
    let l2 = complex(1000.0, pi / 8.0);
    let l3 = complex(sqrt(sqrt(2.0) + 1.0), pi_2);
    let pp = hyperbolictile_topolar(position);
    let color = 0.0;
    if (pp.re > 1.0) {
        color = 0.0;
    } else {
        let g2 = 0;
        let k1 = 1;
        for i in 0..7 {
            if (k1 == 0) {
                continue;
            }
            let k = 1;
            for j in 1..8 {
                if (k == 0 || (pp.im >= pi_2 && pp.im < pi_3_4)) {
                    k = 0;
                } else {
                    g2 = g2 + 1;
                    pp = hyperbolictile_mirror(l2, hyperbolictile_mirror(l1, pp));
                }
            }
            let p1 = hyperbolictile_mirror(l3, pp);
            if (p1.im >= pi_2 && p1.im < pi_3_4) {
                k1 = 0;
            }
            pp = p1;
        }
        color = to_float(g2 - 2 * (g2 / 2));
    }
    rgb = vec3(color, color, color);
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(v, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let position = complex(2.0 * (i_f + 0.5) / resolution - 1.0, 2.0 * (j_f + 0.5) / resolution - 1.0);
    let pi = 3.14159265;
    let pi_2 = pi * 0.5;
    let pi_3_4 = pi * 0.75;
    let l1 = complex(1000.0, 0.0);
    let l2 = complex(1000.0, pi / 8.0);
    let l3 = complex(sqrt(sqrt(2.0) + 1.0), pi_2);
    let pp = hyperbolictile_topolar(position);
    let color = 0.0;
    if (pp.re > 1.0) {
        color = 0.0;
    } else {
        let g2 = 0;
        let k1 = 1;
        for i in 0..7 {
            if (k1 == 0) {
                continue;
            }
            let k = 1;
            for j in 1..8 {
                if (k == 0 || (pp.im >= pi_2 && pp.im < pi_3_4)) {
                    k = 0;
                } else {
                    g2 = g2 + 1;
                    pp = hyperbolictile_mirror(l2, hyperbolictile_mirror(l1, pp));
                }
            }
            let p1 = hyperbolictile_mirror(l3, pp);
            if (p1.im >= pi_2 && p1.im < pi_3_4) {
                k1 = 0;
            }
            pp = p1;
        }
        color = to_float(g2 - 2 * (g2 / 2));
    }
    rgb = vec3(color, color, color);
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, v.z), rgb: rgb }
}
