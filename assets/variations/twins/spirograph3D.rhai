/// 3D spirograph curve sampler — picks a random `t ∈ [tmin, tmax]` and
/// emits a 3D spirograph trajectory `((a+b)·cos(t) − c·cos((a+b)/b·t),
/// (a+b)·sin(t) − c·sin((a+b)/b·t), c·sin((a+b)/b·t))`. The `mode`
/// parameter selects one of 5 width-jitter patterns added to each axis.

// Spirograph 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Outer wheel radius.",
    },
    b: #{
        value: -0.3, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Inner wheel radius (signed; negative produces an internal spirograph).",
    },
    c: #{
        value: 0.4, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Pen-arm length.",
    },
    tmin: #{
        value: 0.0, min: -1000.0, max: 1000.0, free: true,
        caption: "T Min",
        hint: "Minimum value of the random parameter `t`.",
    },
    tmax: #{
        value: 1000.0, min: -10000.0, max: 10000.0, free: true,
        caption: "T Max",
        hint: "Maximum value of the random parameter `t`.",
    },
    width: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Width",
        hint: "Per-axis jitter magnitude (interpretation depends on `mode`).",
    },
    mode: #{
        value: 0, choices: ["Uniform", "Phased Sin", "Independent", "Gaussian", "X Only"],
        caption: "Mode",
        hint: "Random jitter pattern applied to the width offset.",
    },
    direct_color: #{
        value: false,
        caption: "Direct Color",
        hint: "When on, writes `fmod(t / 2π, 1)` to the color register so the palette follows the curve parameter `t`. Visible color requires the transform's Direct Color slider > 0.",
    },
};

const INFO = #{ title: "Spirograph 3D", name: "rhai_spirograph3d", category: "full_3d", phase: "any" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let a = p.a;
    let b = p.b;
    let c_p = p.c;
    let tmin = p.tmin;
    let tmax = p.tmax;
    let width = p.width;
    let mode = to_int(to_float(p.mode));
    let two_pi = 6.28318530717959;
    let t = (tmax - tmin) * rand() + tmin;
    let w1 = 0.0;
    let w2 = 0.0;
    if (mode == 0) {
        let r = width * rand() - width * 0.5;
        w1 = r;
        w2 = r;
    } else if (mode == 1) {
        let r = width * rand() - width * 0.5;
        w2 = r * sin(36.0 * t + two_pi / 3.0);
        w1 = r * sin(36.0 * t);
    } else if (mode == 2) {
        w1 = width * rand() - width * 0.5;
        w2 = width * rand() - width * 0.5;
    } else if (mode == 3) {
        w1 = width * (rand() + rand() + rand() + rand() - 2.0) * 0.5;
        w2 = width * (rand() + rand() + rand() + rand() - 2.0) * 0.5;
    } else if (mode == 4) {
        w1 = (if rand() < 0.5 { width } else { -width });
        w2 = 0.0;
    }
    let safe_b = (if abs(b) < 1.0e-30 { 1.0e-30 } else { b });
    let ab = a + b;
    let abdb = ab / safe_b;
    let x1 = ab * cos(t) - c_p * cos(abdb * t);
    let y1 = ab * sin(t) - c_p * sin(abdb * t);
    if (to_int(to_float(p.direct_color)) != 0) {
        let craw = t / two_pi;
        color = craw - to_float(trunc(craw));
    }
    #{ z: complex(x1 + w1, y1 + w2), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let a = p.a;
    let b = p.b;
    let c_p = p.c;
    let tmin = p.tmin;
    let tmax = p.tmax;
    let width = p.width;
    let mode = to_int(to_float(p.mode));
    let two_pi = 6.28318530717959;
    let t = (tmax - tmin) * rand() + tmin;
    let w1 = 0.0;
    let w2 = 0.0;
    let w3 = 0.0;
    if (mode == 0) {
        let r = width * rand() - width * 0.5;
        w1 = r;
        w2 = r;
        w3 = r;
    } else if (mode == 1) {
        let r = width * rand() - width * 0.5;
        w2 = r * sin(36.0 * t + two_pi / 3.0);
        w3 = r * sin(36.0 * t + 2.0 * two_pi / 3.0);
        w1 = r * sin(36.0 * t);
    } else if (mode == 2) {
        w1 = width * rand() - width * 0.5;
        w2 = width * rand() - width * 0.5;
        w3 = width * rand() - width * 0.5;
    } else if (mode == 3) {
        w1 = width * (rand() + rand() + rand() + rand() - 2.0) * 0.5;
        w2 = width * (rand() + rand() + rand() + rand() - 2.0) * 0.5;
        w3 = width * (rand() + rand() + rand() + rand() - 2.0) * 0.5;
    } else if (mode == 4) {
        w1 = (if rand() < 0.5 { width } else { -width });
        w2 = 0.0;
        w3 = 0.0;
    }
    let safe_b = (if abs(b) < 1.0e-30 { 1.0e-30 } else { b });
    let ab = a + b;
    let abdb = ab / safe_b;
    let x1 = ab * cos(t) - c_p * cos(abdb * t);
    let y1 = ab * sin(t) - c_p * sin(abdb * t);
    let z1 = c_p * sin(abdb * t);
    if (to_int(to_float(p.direct_color)) != 0) {
        let craw = t / two_pi;
        color = craw - to_float(trunc(craw));
    }
    #{ z: vec3(x1 + w1, y1 + w2, z1 + w3), color: color }
}
