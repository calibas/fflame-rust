/// 3D surface plot — samples a random `(x, z)` in the configured rectangle,
/// evaluates a preset height formula `y = f(x, z)`, and emits `(x, y, z)`.
/// The 7 presets are JWildfire's stock yplot3d formulas (ripples, waves,
/// radial decay patterns). Direct-color maps the x/y/z position (or x·z)
/// onto the palette. Custom JWF formulas are not supported — only presets.
///
/// # Authors
/// - Andreas Maschke

// Y-Plot 3D WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    preset_id: #{
        value: 0, min: -1, max: 6,
        caption: "Preset ID",
        hint: "Which baked formula preset to plot (0-6). JWF's default is a random preset; ours is 0 for determinism. -1 means a custom JWF formula we can't evaluate — renders as y = 0.",
    },
    xmin: #{
        value: -3.0, min: -10.0, max: 10.0, free: true,
        caption: "X Min",
        hint: "Lower X bound of the sampled rectangle (swapped with xmax if larger).",
    },
    xmax: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "X Max",
        hint: "Upper X bound of the sampled rectangle.",
    },
    ymin: #{
        value: -4.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Min",
        hint: "Lower Y bound — only used to normalize the Y color mode (the surface itself is unclamped).",
    },
    ymax: #{
        value: 4.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Max",
        hint: "Upper Y bound for the Y color mode.",
    },
    zmin: #{
        value: -2.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Min",
        hint: "Lower Z bound of the sampled rectangle (swapped with zmax if larger).",
    },
    zmax: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Max",
        hint: "Upper Z bound of the sampled rectangle.",
    },
    direct_color: #{
        value: true,
        caption: "Direct Color",
        hint: "When on, writes the color register from the sample position per Color Mode. Visible color requires the transform's Direct Color slider > 0.",
    },
    color_mode: #{
        value: 3, choices: ["Colormap (unsupported)", "X", "Y", "Z", "X×Z"],
        caption: "Color Mode",
        hint: "Position→palette mapping. X/Y/Z normalize the corresponding coordinate over its range; X×Z multiplies the two. Colormap needs an image file (unsupported) and falls back to clamping the incoming color, as JWF does with no map loaded.",
    },
    blend_colormap: #{
        value: 1, min: 0, max: 1,
        caption: "Blend Colormap",
        hint: "Unused — colormap images are unsupported. Preserved for JWF XML round-trip (JWF default 1).",
    },
    displ_amount: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Displ Amount",
        hint: "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 0.1).",
    },
    blend_displ_map: #{
        value: 1, min: 0, max: 1,
        caption: "Blend Displ Map",
        hint: "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 1).",
    },
    param_a: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param A",
        hint: "Formula input `param_a` — none of the 7 stock presets use it. Preserved for JWF XML round-trip.",
    },
    param_b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param B",
        hint: "Formula input `param_b` — unused by the stock presets.",
    },
    param_c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param C",
        hint: "Formula input `param_c` — unused by the stock presets.",
    },
    param_d: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param D",
        hint: "Formula input `param_d` — unused by the stock presets.",
    },
    param_e: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param E",
        hint: "Formula input `param_e` — unused by the stock presets.",
    },
    param_f: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param F",
        hint: "Formula input `param_f` — unused by the stock presets.",
    },
};

const INFO = #{ title: "Y-Plot 3D WF", name: "rhai_yplot3d_wf", category: "full_3d", phase: "any" };

fn prepare(p) {
    let out = array(6, 0.0);
    let xmin = p.xmin;
    let xmax = p.xmax;
    if (xmin > xmax) {
        let t = xmin;
        xmin = xmax;
        xmax = t;
    }
    out[0] = xmin;
    out[1] = xmax - xmin;
    let ymin = p.ymin;
    let ymax = p.ymax;
    if (ymin > ymax) {
        let t = ymin;
        ymin = ymax;
        ymax = t;
    }
    out[2] = ymin;
    out[3] = ymax - ymin;
    let zmin = p.zmin;
    let zmax = p.zmax;
    if (zmin > zmax) {
        let t = zmin;
        zmin = zmax;
        zmax = t;
    }
    out[4] = zmin;
    out[5] = zmax - zmin;
    #{ xmin_s: out[0], dx: out[1], ymin_s: out[2], dy: out[3], zmin_s: out[4], dz: out[5] }
}

fn yplot3d_wf_formula(id, x, z) {
    let pi = 3.14159265358979;
    let r2 = x * x + z * z;
    if (id == 0) {
        return sin(2.0 * exp(-4.0 * r2));
    }
    if (id == 1) {
        return cos(x * z * 12.0) / 6.0;
    }
    if (id == 2) {
        return cos(sqrt(r2) * 14.0) * exp(-2.0 * r2);
    }
    if (id == 3) {
        let safe_z = (if abs(z) < 1.0e-30 { 1.0e-30 } else { z });
        return cos(atan(x / safe_z) * 8.0) / 4.0 * sin(sqrt(r2) * 3.0);
    }
    if (id == 4) {
        return (sin(x) * sin(x) + cos(z) * cos(z)) / (5.0 + r2);
    }
    if (id == 5) {
        return cos(2.0 * pi * (x + z)) * (1.0 - sqrt(r2));
    }
    if (id == 6) {
        return sin(r2);
    }
    0.0
}

fn variation(w, p, ctx) {
    let color = ctx.color;
    let preset_id = to_int(to_float(p.preset_id));
    let direct_color = to_int(to_float(p.direct_color));
    let color_mode = to_int(to_float(p.color_mode));
    let xmin_s = p.xmin_s;
    let dx = p.dx;
    let ymin_s = p.ymin_s;
    let dy = p.dy;
    let zmin_s = p.zmin_s;
    let dz = p.dz;
    let rand_u = rand();
    let rand_v = rand();
    let x = xmin_s + rand_u * dx;
    let z = zmin_s + rand_v * dz;
    let y = yplot3d_wf_formula(preset_id, x, z);
    if (direct_color > 0) {
        let c = color;
        if (color_mode == 1) {
            c = (x - xmin_s) / dx;
        } else if (color_mode == 2) {
            c = (y - ymin_s) / dy;
        } else if (color_mode == 4) {
            c = (x - xmin_s) / dx * (z - zmin_s) / dz;
        } else if (color_mode != 0) {
            c = (z - zmin_s) / dz;
        }
        color = clamp(c, 0.0, 1.0);
    }
    #{ z: complex(x, y), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let preset_id = to_int(to_float(p.preset_id));
    let direct_color = to_int(to_float(p.direct_color));
    let color_mode = to_int(to_float(p.color_mode));
    let xmin_s = p.xmin_s;
    let dx = p.dx;
    let ymin_s = p.ymin_s;
    let dy = p.dy;
    let zmin_s = p.zmin_s;
    let dz = p.dz;
    let rand_u = rand();
    let rand_v = rand();
    let x = xmin_s + rand_u * dx;
    let z = zmin_s + rand_v * dz;
    let y = yplot3d_wf_formula(preset_id, x, z);
    if (direct_color > 0) {
        let c = color;
        if (color_mode == 1) {
            c = (x - xmin_s) / dx;
        } else if (color_mode == 2) {
            c = (y - ymin_s) / dy;
        } else if (color_mode == 4) {
            c = (x - xmin_s) / dx * (z - zmin_s) / dz;
        } else if (color_mode != 0) {
            c = (z - zmin_s) / dz;
        }
        color = clamp(c, 0.0, 1.0);
    }
    #{ z: vec3(x, y, z), color: color }
}
