/// Phyllotaxis spiral of regular polygons — scatters `nPoints` n-gons along a
/// golden-angle sunflower spiral, scaled by distance from the centre, and
/// plots a random point inside a random floret. The floret's distance-scale
/// drives the colour, fading the spiral through the palette.
///
/// # Authors
/// - Jesus Sosa
/// - FractalDesire

// Sunflower, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    nPoints: #{
        value: 500, min: 10, max: 1000,
        caption: "Points",
        hint: "Number of polygons (florets) on the spiral. More points fill the disc more densely.",
    },
    shape: #{
        value: 10, min: 3, max: 20,
        caption: "Shape",
        hint: "Number of sides on each floret polygon (3 = triangle, 4 = square, …).",
    },
    scale: #{
        value: 0.02, min: 0.0, max: 100.0, free: true,
        caption: "Scale",
        hint: "Base size of each floret. Multiplied by the per-floret distance factor `sc`, so florets shrink toward the rim (or centre, when inverted).",
    },
    angle: #{
        value: 180.0, min: 0.0, max: 360.0, free: true,
        caption: "Angle",
        hint: "Phyllotaxis angle seed. Multiplied by the golden-angle factor `(3 − √5)`; near 180 gives the classic sunflower packing.",
    },
    color: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "Color",
        hint: "Unused — JWildfire overwrites the colour with the per-floret distance factor. Kept for `.flame` round-trip parity.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "When on, florets grow toward the rim instead of the centre (`sc = r/rmax` instead of `1 − r/rmax`); also flips the colour gradient.",
    },
};

const INFO = #{ title: "Sunflower", name: "rhai_sunflower", category: "plugin", phase: "any" };

fn sunflower_sample(shape) {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let n_edges = to_float(max(shape, 3));
    let mid_angle = two_pi / n_edges;
    let tan90 = tan(pi * 0.5 + mid_angle * 0.5);
    let half = mid_angle * 0.5;
    let sin_h = sin(half);
    let cos_h = cos(half);
    let arc_tan1 = 13.0 / pow(n_edges, 1.3);
    let arc_tan2 = 2.0 * atan(arc_tan1 / -2.0);
    let sector = to_float(floor(rand() * n_edges));
    let frac = atan(arc_tan1 * (rand() - 0.5)) / arc_tan2 + 0.5;
    let ang_xy = (frac + sector) * mid_angle;
    let sx = sin(ang_xy);
    let sy = cos(ang_xy);
    let ang_r = ang_xy - mid_angle * to_float(floor(ang_xy / mid_angle));
    let x_tmp = tan90 / (tan90 - tan(ang_r));
    let y_tmp = x_tmp * tan(ang_r);
    let len_outer = sqrt(x_tmp * x_tmp + y_tmp * y_tmp);
    let ran = sqrt(rand()) * len_outer;
    let px = sx * ran;
    let py = sy * ran;
    complex(cos_h * px - sin_h * py, sin_h * px + cos_h * py)
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let n_points = max(to_int(to_float(p.nPoints)), 1);
    let shape = to_int(to_float(p.shape));
    let scale = p.scale;
    let angle = p.angle;
    let invert = to_int(to_float(p.invert));
    let pi = 3.14159265358979;
    let i = to_float(floor(rand() * to_float(n_points)));
    let ang = angle * (3.0 - sqrt(5.0));
    let rmax = sqrt(to_float(n_points) + 1.0) / 30.0;
    let r = sqrt(i + 1.0) / 30.0;
    let t = (i + 1.0) * ang * pi / 180.0;
    let cx = r * cos(t) / rmax;
    let cy = r * sin(t) / rmax;
    let sc = 1.0 - r / rmax;
    if (invert == 1) {
        sc = r / rmax;
    }
    let poly_scale = scale * sc;
    color = sc;
    let s = sunflower_sample(shape);
    #{ z: complex(s.re * poly_scale + cx, s.im * poly_scale + cy), color: color }
}
