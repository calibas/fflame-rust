/// Adds random scatter that varies with distance from a chosen center
/// point. Closer points get less scatter (or more, with `invert`); the
/// random distribution shape is selectable.

// Falloff2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    scatter: #{
        value: 1.0, min: 1.0e-6, max: 10.0, free: true, exponential: false,
        caption: "Scatter",
        hint: "Maximum random scatter applied at full strength.",
    },
    mindist: #{
        value: 0.5, min: 0.0, max: 10.0, free: true,
        caption: "Min Distance",
        hint: "Distance from the center where the falloff kicks in. Points inside this radius get full strength scatter.",
    },
    mul_x: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Multiply X",
        hint: "How strongly the scatter affects the X axis (0 = ignore, 1 = full).",
    },
    mul_y: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Multiply Y",
        hint: "How strongly the scatter affects the Y axis (0 = ignore, 1 = full).",
    },
    mul_z: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Multiply Z",
        hint: "How strongly the scatter affects the Z axis (0 = ignore, 1 = full). 3D mode only.",
    },
    mul_c: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Multiply Color",
        hint: "Color-channel scatter strength. Currently unused — direct color writing is not wired up for this variation.",
    },
    x0: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X Center",
        hint: "X coordinate of the falloff center.",
    },
    y0: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Center",
        hint: "Y coordinate of the falloff center.",
    },
    z0: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Center",
        hint: "Z coordinate of the falloff center.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "When on, flips the falloff direction — full scatter applies far from the center, nothing near it.",
    },
    type: #{
        value: 0, choices: ["Uniform", "Triangular", "Gaussian"],
        caption: "Blur Type",
        hint: "Random distribution shape. Triangular is smoother; Gaussian concentrates near zero.",
    },
};

const INFO = #{ title: "Falloff2", name: "rhai_falloff2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let scatter = p.scatter;
    let mindist = p.mindist;
    let mul_x = p.mul_x;
    let mul_y = p.mul_y;
    let x0 = p.x0;
    let y0 = p.y0;
    let invert = to_float(p.invert);
    let blur_type = to_float(p.type);
    let rmax = 0.04 * scatter;
    let d = sqrt((z.re - x0) * (z.re - x0) + (z.im - y0) * (z.im - y0));
    if (invert > 0.5) {
        d = 1.0 - d;
    }
    if (d < 0.0) {
        d = 0.0;
    }
    d = (d - mindist) * rmax;
    if (d < 0.0) {
        d = 0.0;
    }
    if (blur_type < 0.5) {
        let rand_x = rand();
        let rand_y = rand();
        complex(z.re + mul_x * rand_x * d, z.im + mul_y * rand_y * d)
    } else if (blur_type < 1.5) {
        let r_in = sqrt(z.re * z.re + z.im * z.im) + 1.0e-6;
        let phi = atan2(z.im, z.re) + mul_y * rand() * d;
        let r = r_in + mul_x * rand() * d;
        complex(r * cos(phi), r * sin(phi))
    } else {
        let phi = d * rand() * PI();
        let r = d * rand();
        complex(z.re + mul_x * r * cos(phi), z.im + mul_y * r * sin(phi))
    }
}

fn variation3(v, p) {
    let result = vec3(0.0, 0.0, 0.0);
    let PI = 3.14159265359;
    let scatter = p.scatter;
    let mindist = p.mindist;
    let mul_x = p.mul_x;
    let mul_y = p.mul_y;
    let mul_z = p.mul_z;
    let x0 = p.x0;
    let y0 = p.y0;
    let z0 = p.z0;
    let invert = to_float(p.invert);
    let blur_type = to_float(p.type);
    let rmax = 0.04 * scatter;
    let d = sqrt((v.x - x0) * (v.x - x0) + (v.y - y0) * (v.y - y0) + (v.z - z0) * (v.z - z0));
    if (invert > 0.5) {
        d = 1.0 - d;
    }
    if (d < 0.0) {
        d = 0.0;
    }
    d = (d - mindist) * rmax;
    if (d < 0.0) {
        d = 0.0;
    }
    if (blur_type < 0.5) {
        let rand_x = rand();
        let rand_y = rand();
        let rand_z = rand();
        result = vec3(v.x + mul_x * rand_x * d, v.y + mul_y * rand_y * d, v.z + mul_z * rand_z * d);
    } else if (blur_type < 1.5) {
        let r_in = sqrt(v.x * v.x + v.y * v.y + v.z * v.z) + 1.0e-6;
        let sigma = asin(v.z / r_in) + mul_z * rand() * d;
        let phi = atan2(v.y, v.x) + mul_y * rand() * d;
        let r = r_in + mul_x * rand() * d;
        result = vec3(r * cos(sigma) * cos(phi), r * cos(sigma) * sin(phi), r * sin(sigma));
    } else {
        let sigma = d * rand() * 2.0 * PI();
        let phi = d * rand() * PI();
        let r = d * rand();
        result = vec3(v.x + mul_x * r * cos(sigma) * cos(phi), v.y + mul_y * r * cos(sigma) * sin(phi), v.z + mul_z * r * sin(sigma));
    }
    result
}
