/// Parametric Möbius strip surface — maps the input `(x, y)` onto a Möbius
/// strip with adjustable `radius`, `width`, and number of `twists`.
/// `radial_mode` controls how x (the around-the-strip coordinate) handles
/// out-of-range values; `width_mode` does the same for y (the across-the-
/// strip coordinate). Optional rotation around X and Y axes via
/// `rotate_x`/`rotate_y`.
///
/// # Authors
/// - slobo777
/// - chronologicaldot
/// - CozyG

// Mobius Strip, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Strip's central-circle radius.",
    },
    width: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Width",
        hint: "Strip width.",
    },
    twists: #{
        value: 1, min: -10, max: 10,
        caption: "Twists",
        hint: "Number of half-twists in the strip (odd = Möbius topology, even = orientable ring).",
    },
    range_x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Range X",
        hint: "Range of the X input mapped onto the strip's circumference.",
    },
    range_y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Range Y",
        hint: "Range of the Y input mapped onto the strip's width.",
    },
    rotate_x: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Rotate X",
        hint: "Rotation around the X axis (in units of 2π).",
    },
    rotate_y: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Rotate Y",
        hint: "Rotation around the Y axis (in units of 2π).",
    },
    modify_z: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Modify Z",
        hint: "Z output scale factor. 0 disables Z output (only relevant in 3D mode).",
    },
    width_mode: #{
        value: 0, choices: ["Wrap", "Clamp", "Hide", "Leave"],
        caption: "Width Mode",
        hint: "Y out-of-range behavior. Leave passes the original point through unchanged.",
    },
    radial_mode: #{
        value: 0, choices: ["Wrap", "Clamp", "Hide", "Leave"],
        caption: "Radial Mode",
        hint: "X out-of-range behavior.",
    },
};

const INFO = #{ title: "Mobius Strip", name: "rhai_mobius_strip", category: "full_3d", phase: "any" };

fn prepare(p) {
    let two_pi = 6.28318530717959;
    #{ rotx_sin: sin(p.rotate_x * two_pi), rotx_cos: cos(p.rotate_x * two_pi), roty_sin: sin(p.rotate_y * two_pi), roty_cos: cos(p.rotate_y * two_pi) }
}

fn variation(z, p, ctx) {
    let hidden = ctx.hide;
    let radius = p.radius;
    let width = p.width;
    let twists = to_float(p.twists);
    let range_x = p.range_x;
    let range_y = p.range_y;
    let modify_z = p.modify_z;
    let width_mode = to_int(to_float(p.width_mode));
    let radial_mode = to_int(to_float(p.radial_mode));
    let rotx_sin = p.rotx_sin;
    let rotx_cos = p.rotx_cos;
    let roty_sin = p.roty_sin;
    let roty_cos = p.roty_cos;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let pi = 3.14159265358979;
    let xmax = range_x * 0.5;
    let ymax = range_y * 0.5;
    let xmin = -xmax;
    let ymin = -ymax;
    let x = z.re;
    let y = z.im;
    if (radial_mode == 2 && (x > xmax || x < xmin)) {
        hidden = true;
        return #{ z: complex(0.0, 0.0), hide: hidden };
    }
    if (width_mode == 2 && (y > ymax || y < ymin)) {
        hidden = true;
        return #{ z: complex(0.0, 0.0), hide: hidden };
    }
    if (radial_mode == 3 && (x > xmax || x < xmin)) {
        return #{ z: complex(z.re * inv_w, z.im * inv_w), hide: hidden };
    }
    if (width_mode == 3 && (y > ymax || y < ymin)) {
        return #{ z: complex(z.re * inv_w, z.im * inv_w), hide: hidden };
    }
    let t = 0.0;
    if (abs(range_x) < 1.0e-30) {
        t = 0.0;
    } else {
        if (radial_mode == 0) {
            x = x - to_float(trunc(x / xmax)) * xmax;
        } else if (radial_mode == 1) {
            x = clamp(x, xmin, xmax);
        }
        t = x * pi / xmax + pi;
    }
    let s = 0.0;
    if (abs(range_y) < 1.0e-30) {
        s = 0.0;
    } else {
        if (width_mode == 0) {
            y = y - to_float(trunc(y / ymax)) * ymax;
        } else if (width_mode == 1) {
            y = clamp(y, ymin, ymax);
        }
        s = y * (width * 0.5) / ymax;
    }
    let half_t = twists * t * 0.5;
    let chs = cos(half_t);
    let r_off = radius + s * chs;
    let mx = r_off * cos(t);
    let my = r_off * sin(t);
    let rx = mx;
    let ry = my * roty_cos;
    let rz = -my * roty_sin;
    let mx_out = rx * rotx_cos - rz * rotx_sin;
    let my_out = ry;
    #{ z: complex(mx_out, my_out), hide: hidden }
}

fn variation3(v, p, ctx) {
    let hidden = ctx.hide;
    let radius = p.radius;
    let width = p.width;
    let twists = to_float(p.twists);
    let range_x = p.range_x;
    let range_y = p.range_y;
    let modify_z = p.modify_z;
    let width_mode = to_int(to_float(p.width_mode));
    let radial_mode = to_int(to_float(p.radial_mode));
    let rotx_sin = p.rotx_sin;
    let rotx_cos = p.rotx_cos;
    let roty_sin = p.roty_sin;
    let roty_cos = p.roty_cos;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let pi = 3.14159265358979;
    let xmax = range_x * 0.5;
    let ymax = range_y * 0.5;
    let xmin = -xmax;
    let ymin = -ymax;
    let x = v.x;
    let y = v.y;
    if (radial_mode == 2 && (x > xmax || x < xmin)) {
        hidden = true;
        return #{ z: vec3(0.0, 0.0, 0.0), hide: hidden };
    }
    if (width_mode == 2 && (y > ymax || y < ymin)) {
        hidden = true;
        return #{ z: vec3(0.0, 0.0, 0.0), hide: hidden };
    }
    if (radial_mode == 3 && (x > xmax || x < xmin)) {
        return #{ z: vec3(v.x * inv_w, v.y * inv_w, 0.0), hide: hidden };
    }
    if (width_mode == 3 && (y > ymax || y < ymin)) {
        return #{ z: vec3(v.x * inv_w, v.y * inv_w, 0.0), hide: hidden };
    }
    let t = 0.0;
    if (abs(range_x) < 1.0e-30) {
        t = 0.0;
    } else {
        if (radial_mode == 0) {
            x = x - to_float(trunc(x / xmax)) * xmax;
        } else if (radial_mode == 1) {
            x = clamp(x, xmin, xmax);
        }
        t = x * pi / xmax + pi;
    }
    let s = 0.0;
    if (abs(range_y) < 1.0e-30) {
        s = 0.0;
    } else {
        if (width_mode == 0) {
            y = y - to_float(trunc(y / ymax)) * ymax;
        } else if (width_mode == 1) {
            y = clamp(y, ymin, ymax);
        }
        s = y * (width * 0.5) / ymax;
    }
    let half_t = twists * t * 0.5;
    let chs = cos(half_t);
    let shs = sin(half_t);
    let r_off = radius + s * chs;
    let mx = r_off * cos(t);
    let my = r_off * sin(t);
    let mz = s * shs;
    let rx = mx;
    let ry = my * roty_cos + mz * roty_sin;
    let rz = mz * roty_cos - my * roty_sin;
    let mx_out = rx * rotx_cos - rz * rotx_sin;
    let my_out = ry;
    let mz_out = rz * rotx_cos + rx * rotx_sin;
    let z_factor = (if abs(modify_z) < 1.0e-30 { 0.0 } else { modify_z });
    #{ z: vec3(mx_out, my_out, mz_out * z_factor), hide: hidden }
}
