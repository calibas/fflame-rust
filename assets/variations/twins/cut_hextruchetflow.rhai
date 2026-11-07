/// Hex-truchet flow stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut HexTruchet Flow, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    randomize: #{
        value: 0.0, min: 0.0, max: 1000000.0, free: true,
        caption: "Randomize (seed)",
        hint: "Integer seed; shifts the pattern via `seed·sin(seed·180/π)`.",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call (the usual stencil mode).",
    },
    grid: #{
        value: false,
        caption: "Grid",
        hint: "Overlay the hex-cell edges into the mask.",
    },
    zoom: #{
        value: 10.0, min: 0.1, max: 50.0,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom − shift`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut HexTruchet Flow", name: "rhai_cut_hextruchetflow", category: "plugin", phase: "any", z: "preserve_z", replace: true };

fn cuthtf_PixToHex(p) {
    let sqrt3 = 1.73205;
    let c = vec3(0.0, 0.0, 0.0);
    let t0 = complex((1.0 / sqrt3) * p.re - (1.0 / 3.0) * p.im, (2.0 / 3.0) * p.im);
    c.x = t0.re;
    c.z = t0.im;
    c.y = -c.x - c.z;
    let t3 = vec3(c.x + 0.5, c.y + 0.5, c.z + 0.5);
    let r0 = vec3(to_float(floor(t3.x)), to_float(floor(t3.y)), to_float(floor(t3.z)));
    let dr = abs(r0 - c);
    let t1 = vec3(dr.y, dr.z, dr.x);
    let t2 = vec3(dr.z, dr.x, dr.y);
    let r = r0 - vec3((if dr.x < t1.x { 0.0 } else { 1.0 }), (if dr.y < t1.y { 0.0 } else { 1.0 }), (if dr.z < t1.z { 0.0 } else { 1.0 })) * vec3((if dr.x < t2.x { 0.0 } else { 1.0 }), (if dr.y < t2.y { 0.0 } else { 1.0 }), (if dr.z < t2.z { 0.0 } else { 1.0 })) * dot(r0, vec3(1.0, 1.0, 1.0));
    complex(r.x, r.z)
}

fn cuthtf_HexToPix(h) {
    let sqrt3 = 1.73205;
    complex(sqrt3 * (h.re + 0.5 * h.im), 1.5 * h.im)
}

fn cuthtf_HexEdgeDist(p0) {
    let sqrt3 = 1.73205;
    let p = complex(abs(p0.re), abs(p0.im));
    (sqrt3 / 2.0) - p.re + 0.5 * min(0.0, p.re - sqrt3 * p.im)
}

fn cuthtf_Hashfv2(p) {
    let t1 = complex(37.0, 39.0);
    fract(sin((p.re * t1.re + p.im * t1.im)) * 43758.54)
}

fn cuthtf_ShowScene(p, grid) {
    let sqrt3 = 1.73205;
    let col = vec3(0.0, 0.0, 0.0);
    let w = vec3(0.0, 0.0, 0.0);
    let cId = cuthtf_PixToHex(p);
    let pc = cuthtf_HexToPix(cId);
    let dir = 2.0 * (if 0.5 < cuthtf_Hashfv2(cId) { 0.0 } else { 1.0 }) - 1.0;
    let t0 = pc + complex(0.0, -dir);
    w.x = t0.re;
    w.y = t0.im;
    w.z = (t0 - p).norm_sqr();
    let q = pc + complex(sqrt3 / 2.0, 0.5 * dir);
    let d = (q - p).norm_sqr();
    if (d < w.z) {
        w = vec3(q.re, q.im, d);
    }
    q = pc + complex(-sqrt3 / 2.0, 0.5 * dir);
    d = (q - p).norm_sqr();
    if (d < w.z) {
        w = vec3(q.re, q.im, d);
    }
    w.z = abs(sqrt(w.z) - 0.5);
    d = cuthtf_HexEdgeDist(p - pc);
    if (grid == 1) {
        let t1 = clamp((d - 0.01) / (0.02 - 0.01), 0.0, 1.0);
        col = vec3(1.0, 1.0, 1.0) * mix(1.0, (if d < 1.0 { 0.0 } else { 1.0 }), (t1 * t1 * (3.0 - 2.0 * t1)));
    }
    if (w.z < 0.25) {
        col = vec3(1.0, 1.0, 1.0);
    }
    col
}

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let PI = 3.14159265359;
    let randomize = p.randomize;
    let mode = to_int(to_float(p.mode));
    let grid = to_int(to_float(p.grid));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let tol = 0.9;
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = rand() - 0.5;
        y = rand() - 0.5;
    }
    let shiftxy = randomize * sin(randomize * 180.0 / PI());
    let uv = complex(x * zoom - shiftxy, y * zoom - shiftxy);
    let col = cuthtf_ShowScene(uv, grid).x;
    let hidden = false;
    if (invert == 0) {
        if (col > tol) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (col <= tol) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}

fn variation3(v, p, ctx) {
    let hide_v = ctx.hide;
    let PI = 3.14159265359;
    let randomize = p.randomize;
    let mode = to_int(to_float(p.mode));
    let grid = to_int(to_float(p.grid));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let tol = 0.9;
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = v.x;
        y = v.y;
    } else {
        x = rand() - 0.5;
        y = rand() - 0.5;
    }
    let shiftxy = randomize * sin(randomize * 180.0 / PI());
    let uv = complex(x * zoom - shiftxy, y * zoom - shiftxy);
    let col = cuthtf_ShowScene(uv, grid).x;
    let hidden = false;
    if (invert == 0) {
        if (col > tol) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (col <= tol) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: vec3(x, y, v.z), hide: hide_v }
}
