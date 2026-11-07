/// Adds random scatter that varies with distance from a chosen center
/// point. Closer points get less scatter (or more, with `invert`); the
/// random distribution shape is selectable. Runs before the rest of the
/// variations.

// Pre Falloff2, the built-in variation's twin, translated from its WGSL.

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

const INFO = #{ title: "Pre Falloff2", name: "rhai_pre_falloff2", category: "advanced_2d", phase: "pre", z: "preserve_z" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let scatter = p.scatter;
    let mindist = p.mindist;
    let mul_x = p.mul_x;
    let mul_y = p.mul_y;
    let x0 = p.x0;
    let y0 = p.y0;
    let invert = to_float(p.invert);
    let blurtype = to_float(p.type);
    let dx = z.re - x0;
    let dy = z.im - y0;
    let dist = sqrt(dx * dx + dy * dy);
    let factor = 0.0;
    if invert > 0.5 {
        factor = (if dist < mindist { dist / mindist } else { 1.0 });
    } else {
        factor = (if dist > mindist { mindist / dist } else { 1.0 });
    }
    let sx = 0.0;
    let sy = 0.0;
    let blurtype_int = to_int(blurtype + 0.5);
    if blurtype_int == 0 {
        let r = scatter * factor;
        let angle = rand() * 2.0 * PI();
        let d = rand() * r;
        sx = d * cos(angle);
        sy = d * sin(angle);
    } else if blurtype_int == 1 {
        let r = scatter * factor;
        let angle = rand() * 2.0 * PI();
        let d = (rand() + rand()) * 0.5 * r;
        sx = d * cos(angle);
        sy = d * sin(angle);
    } else {
        let r = scatter * factor;
        let angle = rand() * 2.0 * PI();
        let d = sqrt(-ln(rand() + 1.0e-10)) * r;
        sx = d * cos(angle);
        sy = d * sin(angle);
    }
    complex(z.re + sx * mul_x, z.im + sy * mul_y)
}

fn variation3(v, p) {
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
    let blurtype = to_float(p.type);
    let dx = v.x - x0;
    let dy = v.y - y0;
    let dz = v.z - z0;
    let dist = sqrt(dx * dx + dy * dy + dz * dz);
    let factor = 0.0;
    if invert > 0.5 {
        factor = (if dist < mindist { dist / mindist } else { 1.0 });
    } else {
        factor = (if dist > mindist { mindist / dist } else { 1.0 });
    }
    let sx = 0.0;
    let sy = 0.0;
    let sz = 0.0;
    let blurtype_int = to_int(blurtype + 0.5);
    if blurtype_int == 0 {
        let r = scatter * factor;
        let theta = rand() * 2.0 * PI();
        let phi = acos(2.0 * rand() - 1.0);
        let d = rand() * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    } else if blurtype_int == 1 {
        let r = scatter * factor;
        let theta = rand() * 2.0 * PI();
        let phi = acos(2.0 * rand() - 1.0);
        let d = (rand() + rand()) * 0.5 * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    } else {
        let r = scatter * factor;
        let theta = rand() * 2.0 * PI();
        let phi = acos(2.0 * rand() - 1.0);
        let d = sqrt(-ln(rand() + 1.0e-10)) * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    }
    vec3(v.x + sx * mul_x, v.y + sy * mul_y, v.z + sz * mul_z)
}
