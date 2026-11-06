// Sphere NJA, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    circle_a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Circle A",
        hint: "Declared in the upstream source but unused in the body. Preserved as a parameter for preset compatibility.",
    },
    circle_b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Circle B",
        hint: "Declared but unused (same as `circle_a`).",
    },
    shift_x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift X",
        hint: "X-axis center offset (subtracted from input, added back to output).",
    },
    shift_y: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift Y",
        hint: "Y-axis center offset.",
    },
    shift_z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift Z",
        hint: "Z-axis center offset.",
    },
    stretch: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Stretch",
        hint: "Radial-to-angular scaling factor in `t = sqtr/stretch − π/2`.",
    },
};

const INFO = #{ title: "Sphere NJA", name: "rhai_sphere_nja" };

fn variation(z, p, ctx) {
    let shift_x = p.shift_x;
    let shift_y = p.shift_y;
    let stretch = p.stretch;
    let pi_2 = 1.5707963267948966;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let safe_stretch = (if abs(stretch) < 1.0e-30 { 1.0e-30 } else { stretch });
    let inix = z.re - shift_x;
    let iniy = z.im - shift_y;
    let sqtr = max(sqrt(inix * inix + iniy * iniy), 1.0e-30);
    let t = sqtr / safe_stretch - pi_2;
    let r = cos(t) * w;
    let fx = r * inix / sqtr + shift_x;
    let fy = r * iniy / sqtr + shift_y;
    complex(fx * inv_w, fy * inv_w)
}

fn variation3(v, p, ctx) {
    let shift_x = p.shift_x;
    let shift_y = p.shift_y;
    let stretch = p.stretch;
    let pi_2 = 1.5707963267948966;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let safe_stretch = (if abs(stretch) < 1.0e-30 { 1.0e-30 } else { stretch });
    let inix = v.x - shift_x;
    let iniy = v.y - shift_y;
    let iniz = v.z;
    let sqtr = max(sqrt(inix * inix + iniy * iniy + iniz * iniz), 1.0e-30);
    let t = sqtr / safe_stretch - pi_2;
    let r = cos(t) * w;
    let z_p = sin(t) * w;
    let fx = r * inix / sqtr;
    let fy = r * iniy / sqtr;
    let fz = z_p;
    fx = fx + r * iniz * inix / sqtr;
    fy = fy + r * iniz * iniy / sqtr;
    let denom = max(r * r + z_p * z_p, 1.0e-30);
    fz = fz + iniz * z_p / denom;
    fx = fx + shift_x;
    fy = fy + shift_y;
    vec3(fx * inv_w, fy * inv_w, fz * inv_w)
}
