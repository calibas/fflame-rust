/// Acrylic-style smudge pattern. Three RGB channels each iterate
/// `(p.x, p.y)` through trig perturbations driven by params p1..p4,
/// then final color comes from `sin(p5·p.x²) + sin(p6·p.y²)` mapped
/// through `cos(col · F*)`.
///
/// 13 params: standard 4 + 6 perturbation knobs (p1..p6) + 3 color
/// frequencies (fr/fg/fb).
/// 
/// # Authors
/// - Jesus Sosa

// GLSL Acrilic, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution. See module-level doc.",
    },
    seed: #{
        value: 10000, min: 0, max: 10000, free: true,
        caption: "Seed",
        hint: "JWildfire CPU-only — set `time` directly.",
    },
    time: #{
        value: 0.0, min: -10000.0, max: 10000.0, free: true,
        caption: "Time",
        hint: "Animation parameter. Embedded into every perturbation step's sin/cos arguments.",
    },
    steps: #{
        value: 30, min: 1, max: 100, free: true,
        caption: "Steps",
        hint: "Inner perturbation iteration count. **GPU-clamped to ≤ 100.** Higher = more swirling distortion.",
    },
    p1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "p1",
        hint: "Perturbation knob 1. Frequency multiplier on `p.y` in the X-update's sin term.",
    },
    p2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "p2",
        hint: "Perturbation knob 2. Inverse-time scale (`time / (p2·i)`). p2 = 0 blows up — set away from 0.",
    },
    p3: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "p3",
        hint: "Perturbation knob 3. Frequency multiplier on `p.x` in the Y-update's cos term.",
    },
    p4: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "p4",
        hint: "Perturbation knob 4. Inverse-time scale in Y-update.",
    },
    p5: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "p5",
        hint: "Color formula knob 5. Final per-channel value uses `sin(p5 · p.x²)`.",
    },
    p6: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "p6",
        hint: "Color formula knob 6. `sin(p6 · p.y²)`.",
    },
    fr: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Red Fac.",
        hint: "Red-channel final cosine frequency.",
    },
    fg: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Green Fac.",
        hint: "Green-channel frequency.",
    },
    fb: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Blue Fac.",
        hint: "Blue-channel frequency.",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.**",
    },
};

const INFO = #{ title: "GLSL Acrilic", name: "rhai_glsl_acrilic", category: "plugin", phase: "any", slow: ["steps"], z: "never" };

fn variation(w, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let steps = min(u32(to_float(p.steps)), u32(100));
    let p1 = p.p1;
    let p2_raw = p.p2;
    let p3 = p.p3;
    let p4_raw = p.p4;
    let p5 = p.p5;
    let p6 = p.p6;
    let fr = p.fr;
    let fg = p.fg;
    let fb = p.fb;
    let p2 = (if p2_raw > 0.0 { 1.0 } else if p2_raw < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-6, abs(p2_raw));
    let p4 = (if p4_raw > 0.0 { 1.0 } else if p4_raw < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-6, abs(p4_raw));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let col = vec3(0.0, 0.0, 0.0);
    for j in 0..3 {
        let pp = complex(0.7 * (i_f + 0.5) / resolution, 0.7 * (j_f + 0.5) / resolution);
        let jf = to_float(j);
        for i in 1..to_int(steps) {
            let ife = to_float(i);
            let denom = ife + jf;
            pp = complex(pp.re + 0.1 / denom * sin(ife * p1 * pp.im + time + cos((time / (p2 * ife)) * ife + jf)), pp.im);
            pp = complex(pp.re, pp.im + 0.1 / denom * cos(ife * p3 * pp.re + time + sin((time / (p4 * ife)) * ife + jf)));
        }
        let v = sin(p5 * pp.re * pp.re) + sin(p6 * pp.im * pp.im);
        if (j == u32(0)) {
            col.x = v;
        } else if (j == u32(1)) {
            col.y = v;
        } else {
            col.z = v;
        }
    }
    rgb = vec3(cos(col.x * fr), cos(col.y * fg), cos(col.z * fb));
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(q, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let steps = min(u32(to_float(p.steps)), u32(100));
    let p1 = p.p1;
    let p2_raw = p.p2;
    let p3 = p.p3;
    let p4_raw = p.p4;
    let p5 = p.p5;
    let p6 = p.p6;
    let fr = p.fr;
    let fg = p.fg;
    let fb = p.fb;
    let p2 = (if p2_raw > 0.0 { 1.0 } else if p2_raw < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-6, abs(p2_raw));
    let p4 = (if p4_raw > 0.0 { 1.0 } else if p4_raw < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-6, abs(p4_raw));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let col = vec3(0.0, 0.0, 0.0);
    for j in 0..3 {
        let pp = complex(0.7 * (i_f + 0.5) / resolution, 0.7 * (j_f + 0.5) / resolution);
        let jf = to_float(j);
        for i in 1..to_int(steps) {
            let ife = to_float(i);
            let denom = ife + jf;
            pp = complex(pp.re + 0.1 / denom * sin(ife * p1 * pp.im + time + cos((time / (p2 * ife)) * ife + jf)), pp.im);
            pp = complex(pp.re, pp.im + 0.1 / denom * cos(ife * p3 * pp.re + time + sin((time / (p4 * ife)) * ife + jf)));
        }
        let v = sin(p5 * pp.re * pp.re) + sin(p6 * pp.im * pp.im);
        if (j == u32(0)) {
            col.x = v;
        } else if (j == u32(1)) {
            col.y = v;
        } else {
            col.z = v;
        }
    }
    rgb = vec3(cos(col.x * fr), cos(col.y * fg), cos(col.z * fb));
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, q.z), rgb: rgb }
}
