/// Sierpinski-like fractal dot pattern. Applies `iterations` fold +
/// rotate + shrink steps to the sample point, then draws a small dot
/// if the final point is inside a circle of `circleSize = dotsize / (3 · 2^maxiter)`.
///
/// 10 params (no `seed` or `time`).
/// 
/// # Authors
/// - Jesus Sosa

// GLSL FractalDots, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution.",
    },
    iterations: #{
        value: 9, min: 1, max: 20, free: true,
        caption: "Iterations",
        hint: "Soft early-exit threshold for the inner loop. The loop runs at most `max_iterations` times but breaks when `i > iterations`. **GPU-clamped to ≤ 20**.",
    },
    dot_size: #{
        value: 400.0, min: 1.0, max: 1000.0, free: true,
        caption: "Dot Size",
        hint: "Numerator of the circle-test radius: `circleSize = dot_size / (3 · 2^max_iterations)`. Higher = larger visible dots.",
    },
    max_iterations: #{
        value: 5, min: 1, max: 20, free: true,
        caption: "Max Iterations",
        hint: "Outer iteration cap and exponent in `circleSize`. **GPU-clamped to ≤ 20**.",
    },
    complexity: #{
        value: 0.5, min: -5.0, max: 5.0, free: true,
        caption: "Complexity",
        hint: "Per-iteration translation vector (`uv -= vec2(complexity)`). Negative values move detail inward.",
    },
    pattern: #{
        value: 2.0, min: 0.1, max: 10.0, free: true,
        caption: "Pattern",
        hint: "Per-iteration spacing divisor (`s /= pattern`). 1.0 = no spacing change; higher = tighter.",
    },
    spacing: #{
        value: 0.05, min: -2.0, max: 2.0, free: true,
        caption: "Spacing",
        hint: "Initial fold offset (`uv = abs(uv) - spacing`).",
    },
    rotate1: #{
        value: 0.0, min: -6.28, max: 6.28, free: true,
        caption: "Rotate 1",
        hint: "Global pre-iteration rotation angle (radians).",
    },
    rotate2: #{
        value: 0.0, min: -6.28, max: 6.28, free: true,
        caption: "Rotate 2",
        hint: "Per-iteration rotation angle (radians).",
    },
    zoom: #{
        value: 1.0, min: 0.1, max: 10.0, free: true,
        caption: "Zoom",
        hint: "Global zoom factor (`uv *= zoom`).",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.** Output is monochrome — `color = vec3(1)` inside the dot, `vec3(0)` outside.",
    },
};

const INFO = #{ title: "GLSL FractalDots", name: "rhai_glsl_fractaldots", category: "plugin", phase: "any", z: "never" };

fn fractaldots_rot(uv, a) {
    let c = cos(a);
    let s = sin(a);
    complex(uv.re * c - uv.im * s, uv.im * c + uv.re * s)
}

fn variation(z, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let iterations = min(u32(to_float(p.iterations)), u32(20));
    let dot_size = p.dot_size;
    let max_iter = min(u32(to_float(p.max_iterations)), u32(20));
    let complexity = p.complexity;
    let pattern_raw = p.pattern;
    let spacing = p.spacing;
    let rotate1 = p.rotate1;
    let rotate2 = p.rotate2;
    let zoom = p.zoom;
    let pattern = (if pattern_raw > 0.0 { 1.0 } else if pattern_raw < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-6, abs(pattern_raw));
    let circle_size = dot_size / (3.0 * pow(2.0, to_float(max_iter)));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let uv = complex((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    uv = fractaldots_rot(uv, rotate1);
    uv = uv * zoom;
    let s = spacing;
    for i in 0..to_int(max_iter) {
        let t1 = complex(abs(uv.re), abs(uv.im));
        uv = complex(t1.re - s, t1.im - s);
        uv = uv - complex(complexity, complexity);
        uv = fractaldots_rot(uv, rotate2);
        s = s / pattern;
        if (iterations < i) {
            break;
        }
    }
    let c = (if uv.abs() <= circle_size { 1.0 } else { 0.0 });
    rgb = vec3(c, c, c);
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(v, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let iterations = min(u32(to_float(p.iterations)), u32(20));
    let dot_size = p.dot_size;
    let max_iter = min(u32(to_float(p.max_iterations)), u32(20));
    let complexity = p.complexity;
    let pattern_raw = p.pattern;
    let spacing = p.spacing;
    let rotate1 = p.rotate1;
    let rotate2 = p.rotate2;
    let zoom = p.zoom;
    let pattern = (if pattern_raw > 0.0 { 1.0 } else if pattern_raw < 0.0 { -1.0 } else { 0.0 }) * max(1.0e-6, abs(pattern_raw));
    let circle_size = dot_size / (3.0 * pow(2.0, to_float(max_iter)));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let uv = complex((i_f + 0.5) / resolution - 0.5, (j_f + 0.5) / resolution - 0.5);
    uv = fractaldots_rot(uv, rotate1);
    uv = uv * zoom;
    let s = spacing;
    for i in 0..to_int(max_iter) {
        let t1 = complex(abs(uv.re), abs(uv.im));
        uv = complex(t1.re - s, t1.im - s);
        uv = uv - complex(complexity, complexity);
        uv = fractaldots_rot(uv, rotate2);
        s = s / pattern;
        if (iterations < i) {
            break;
        }
    }
    let c = (if uv.abs() <= circle_size { 1.0 } else { 0.0 });
    rgb = vec3(c, c, c);
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, v.z), rgb: rgb }
}
