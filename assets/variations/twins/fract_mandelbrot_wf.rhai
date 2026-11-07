/// Mandelbrot escape-time fractal (JWildfire's
/// [`FractMandelbrotWFFunc`](../../../output/variation-jwf-source/FractMandelbrotWFFunc.java)).
/// Iterator is `z = z^power + c` where `c` is the random seed point
/// itself — the standard Mandelbrot construction. `power` selects
/// between four sub-iterators (2, 3, 4, ≥5).
///
/// # Authors
/// - Andreas Maschke

// Fract Mandelbrot (JWF), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    max_iter: #{
        value: 100, min: 1, max: 100000, free: true,
        caption: "Max Iter",
        hint: "Maximum escape iterations per starting point. Higher values resolve finer detail near the fractal boundary but linearly increase per-sample cost. **GPU-clamped to 500** — values above 500 still parse and round-trip with JWildfire `.flame` XML but the inner loop runs no more than 500 iterations to stay within the GPU TDR budget.",
    },
    xmin: #{
        value: -1.6, min: -100.0, max: 100.0, free: true,
        caption: "X Min",
        hint: "Lower X bound of the random-seed sampling rectangle. The variation picks (x0, y0) uniformly in [xmin, xmax] × [ymin, ymax] each call (or uses the affine input when `color_only` is on).",
    },
    xmax: #{
        value: 1.6, min: -100.0, max: 100.0, free: true,
        caption: "X Max",
        hint: "Upper X bound of the random-seed sampling rectangle.",
    },
    ymin: #{
        value: -1.2, min: -100.0, max: 100.0, free: true,
        caption: "Y Min",
        hint: "Lower Y bound of the random-seed sampling rectangle.",
    },
    ymax: #{
        value: 1.2, min: -100.0, max: 100.0, free: true,
        caption: "Y Max",
        hint: "Upper Y bound of the random-seed sampling rectangle.",
    },
    buddhabrot_mode: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Buddhabrot Mode",
        hint: "0 = standard escape-time render (pick random seed, count iterations). 1 = Buddhabrot trajectory render (NOT yet implemented; accepted for round-trip fidelity but falls back to mode 0).",
    },
    power: #{
        value: 2, min: 2, max: 32, free: true,
        caption: "Power",
        hint: "Exponent on `z` in `z ← z^power + c` (`c` = random seed). **GPU-clamped to [2, 8]**. Powers 2/3/4 use closed-form expansion (fastest); 5..8 add (power - 5) extra complex multiplies per step. Values above 8 in the .flame XML round-trip but render as power 8.",
    },
    direct_color: #{
        value: 1, min: 0, max: 1, free: true,
        caption: "Direct Color",
        hint: "When non-zero, the per-iteration color register `vc` is offset by `iter_count / max_iter`. Drives a palette-position gradient from the escape time.",
    },
    scalez: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "Scale Z",
        hint: "Multiplier on the escape-time → Z mapping. Combined with the variation amount as the final Z gain. JWildfire divides by 10 internally — we match that.",
    },
    clip_iter_min: #{
        value: 3, min: 0, max: 10000, free: true,
        caption: "Clip Iter Min",
        hint: "Lower escape-count threshold. If `iter_count <= clip_iter_min`, the seed is rejected and we retry (up to `max_clip_iter` times). Filters out near-boundary fast-escape points.",
    },
    clip_iter_max: #{
        value: -5, min: -10000, max: 10000, free: true,
        caption: "Clip Iter Max",
        hint: "Upper escape-count threshold *relative to max_iter*. If negative and `iter_count >= max_iter + clip_iter_max`, the seed is rejected. Filters out deep-interior points that never escape.",
    },
    max_clip_iter: #{
        value: 3, min: 1, max: 100, free: true,
        caption: "Max Clip Iter",
        hint: "Number of seed-rejection retries before giving up and hiding the point. Higher = more uniform coverage at the cost of doubled/tripled per-sample work in sparse areas. **GPU-clamped to 4**. When `color_only` is on, retries are forced to 1 (every retry would produce the same iter_count with a fixed seed).",
    },
    buddhabrot_min_iter: #{
        value: 7, min: 0, max: 10000, free: true,
        caption: "Buddhabrot Min Iter",
        hint: "Buddhabrot pre-skip count. Trajectories shorter than this are rejected before plotting. Unused in v1 (Buddhabrot mode not implemented).",
    },
    scale: #{
        value: 3.0, min: -100.0, max: 100.0, free: true,
        caption: "Scale",
        hint: "Multiplier on the (x0 + offsetx, y0 + offsety) XY output. The seed lives in the JWildfire sampling rectangle ([xmin..xmax] etc.), so `scale` brings it back into flame-space.",
    },
    offsetx: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Offset X",
        hint: "X translation added before the `scale` multiply.",
    },
    offsety: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Offset Y",
        hint: "Y translation.",
    },
    offsetz: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Offset Z",
        hint: "Z translation added after the `scalez/10 · iter_ratio` term (3D only).",
    },
    z_fill: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Z Fill",
        hint: "Probability of jittering Z between this step's escape count and the previous step's (lerps with a fresh random). 0 = off. Smooths the discrete Z layers from integer iter counts into a continuous gradient.",
    },
    z_logscale: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Z Log Scale",
        hint: "When 1, Z uses `log₁₀(1 + iter_ratio)` instead of the raw `iter_ratio`. Compresses deep-interior points into a narrower band.",
    },
    color_only: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Color Only",
        hint: "When 1, the variation reads the affine input as the escape seed (instead of picking randomly) and contributes nothing to XY — only Z and direct color. Lets you overlay an escape-time color/depth map onto another variation's XY shape.",
    },
};

const INFO = #{ title: "Fract Mandelbrot (JWF)", name: "rhai_fract_mandelbrot_wf", category: "plugin", phase: "any", slow: ["max_iter"], z: "preserve_z" };

fn fractwf_next_iter(x, y, xs, ys, start_x, start_y, cust_a, cust_b, power, kind) {
    let t1 = (kind);
    if t1 == u32(0) {
        let nx = (x - xs + ys) * cust_a - (y - 2.0 * x * y) * cust_b;
        let ny = (x - xs + ys) * cust_b + (y - 2.0 * x * y) * cust_a;
        complex(nx, ny)
    } else if t1 == u32(1) {
        complex(xs - ys + cust_a, 2.0 * x * y + cust_b)
    } else if t1 == u32(2) {
        complex(xs * x - 3.0 * x * ys + cust_a, 3.0 * xs * y - ys * y + cust_b)
    } else if t1 == u32(3) {
        complex(xs * xs + ys * ys - 6.0 * xs * ys + cust_a, 4.0 * x * y * (xs - ys) + cust_b)
    } else if t1 == u32(4) {
        let nx = x * (xs * xs - 10.0 * xs * ys + 5.0 * ys * ys);
        let ny = y * (ys * ys - 10.0 * xs * ys + 5.0 * xs * xs);
        for k in 5..to_int(power) {
            let xa = x * nx - y * ny;
            let ya = x * ny + nx * y;
            nx = xa;
            ny = ya;
        }
        complex(nx + cust_a, ny + cust_b)
    } else if t1 == u32(5) {
        complex(xs - ys + start_x, 2.0 * x * y + start_y)
    } else if t1 == u32(6) {
        complex(xs * x - 3.0 * x * ys + start_x, 3.0 * xs * y - ys * y + start_y)
    } else if t1 == u32(7) {
        complex(xs * xs + ys * ys - 6.0 * xs * ys + start_x, 4.0 * x * y * (xs - ys) + start_y)
    } else if t1 == u32(8) {
        let nx = x * (xs * xs - 10.0 * xs * ys + 5.0 * ys * ys);
        let ny = y * (ys * ys - 10.0 * xs * ys + 5.0 * xs * xs);
        for k in 5..to_int(power) {
            let xa = x * nx - y * ny;
            let ya = x * ny + nx * y;
            nx = xa;
            ny = ya;
        }
        complex(nx + start_x, ny + start_y)
    } else if t1 == u32(9) {
        let denom = xs + ys;
        let nx = (start_x * x - start_y * y) - (start_x * x + start_y * y) / denom;
        let ny = (start_x * y + start_y * x) + (start_x * y - start_y * x) / denom;
        complex(nx, ny)
    } else if t1 == u32(10) {
        let denom = xs + ys;
        let nx = cust_a * x - cust_b * y - ((cust_a * x + cust_b * y) / denom);
        let ny = cust_a * y + cust_b * x + ((cust_a * y - cust_b * x) / denom);
        complex(nx, ny)
    } else if t1 == u32(11) {
        let nx = (xs - ys) * cust_a - (2.0 * x * y) * cust_b - 1.0;
        let ny = (xs - ys) * cust_b + (2.0 * x * y) * cust_a;
        complex(nx, ny)
    } else {
        complex(x, y)
    }
}

fn fractwf_iterate_count(x0, y0, cust_a, cust_b, power, max_iter, kind) {
    let x = x0;
    let y = y0;
    let xs = x * x;
    let ys = y * y;
    let i = u32(0);
    while true {
        if (i >= max_iter || xs + ys >= 4.0) {
            break;
        }
        let r = fractwf_next_iter(x, y, xs, ys, x0, y0, cust_a, cust_b, power, kind);
        x = r.re;
        y = r.im;
        xs = x * x;
        ys = y * y;
        i = i + u32(1);
    }
    i
}

fn fractwf_iterate_body(affine_p, xmin, xmax, ymin, ymax, cust_a, cust_b, max_iter, max_clip_iter, clip_iter_min, clip_iter_max, color_only, power, kind) {
    let x0 = 0.0;
    let y0 = 0.0;
    let iter_count = u32(0);
    let hidden = false;
    let max_iter_safe = min(max_iter, u32(500));
    let clip_max_user = min(max(max_clip_iter, u32(1)), u32(4));
    let clip_max = (if color_only > u32(0) { u32(1) } else { clip_max_user });
    for i in 0..to_int(clip_max) {
        if (color_only > u32(0)) {
            x0 = affine_p.re;
            y0 = affine_p.im;
        } else {
            x0 = (xmax - xmin) * rand() + xmin;
            y0 = (ymax - ymin) * rand() + ymin;
        }
        iter_count = fractwf_iterate_count(x0, y0, cust_a, cust_b, power, max_iter_safe, kind);
        let i_signed = to_int(iter_count);
        let too_high = (clip_iter_max < 0) && (i_signed >= to_int(max_iter_safe) + clip_iter_max);
        let too_low = (clip_iter_min > 0) && (i_signed <= clip_iter_min);
        if (too_high || too_low) {
            if (i == clip_max - u32(1)) {
                hidden = true;
            }
        } else {
            break;
        }
    }
    [x0, y0, to_float(iter_count), (if hidden { 1.0 } else { 0.0 })]
}

fn fractwf_apply_direct_color(iter_count, max_iter_f) {
    let new_vc = this + iter_count / max_iter_f;
    if (new_vc > 1.0) {
        new_vc = new_vc - 1.0;
    }
    if (new_vc < 0.0) {
        new_vc = 0.0;
    }
    if (new_vc > 1.0) {
        new_vc = 1.0;
    }
    this = new_vc;
}

fn fractwf_variation_body_2d(affine_p, max_iter, xmin, xmax, ymin, ymax, cust_a, cust_b, direct_color, clip_iter_min, clip_iter_max, max_clip_iter, scale, offsetx, offsety, color_only, power, kind) {
    let r = fractwf_iterate_body(affine_p, xmin, xmax, ymin, ymax, cust_a, cust_b, max_iter, max_clip_iter, clip_iter_min, clip_iter_max, color_only, power, kind);
    if (r[3] > 0.5) {
        return complex(0.0, 0.0);
    }
    if (direct_color != u32(0)) {
        this.fractwf_apply_direct_color(r[2], to_float(max_iter));
    }
    if (color_only > u32(0)) {
        return complex(0.0, 0.0);
    }
    complex(scale * (r[0] + offsetx), scale * (r[1] + offsety))
}

fn fractwf_compute_z(iter_count, max_iter_f, scalez, offsetz, z_logscale, z_fill) {
    let ic_ratio = iter_count / max_iter_f;
    let scalez10 = scalez * 0.1;
    let log10 = 1.0 / ln(10.0);
    let z = 0.0;
    if (z_logscale == u32(1)) {
        z = scalez10 * ln(1.0 + ic_ratio) * log10 + offsetz;
    } else {
        z = scalez10 * ic_ratio + offsetz;
    }
    if (z_fill > 1.0e-6 && rand() < z_fill) {
        let prev_ratio = (iter_count - 1.0) / max_iter_f;
        let prev_z = 0.0;
        if (z_logscale == u32(1)) {
            prev_z = scalez10 * ln(1.0 + prev_ratio) * log10 + offsetz;
        } else {
            prev_z = scalez10 * prev_ratio + offsetz;
        }
        z = (prev_z - z) * rand() + z;
    }
    z
}

fn fractwf_variation_body_3d(affine_p, max_iter, xmin, xmax, ymin, ymax, cust_a, cust_b, direct_color, scalez, clip_iter_min, clip_iter_max, max_clip_iter, scale, offsetx, offsety, offsetz, z_fill, z_logscale, color_only, power, kind) {
    let r = fractwf_iterate_body(complex(affine_p.x, affine_p.y), xmin, xmax, ymin, ymax, cust_a, cust_b, max_iter, max_clip_iter, clip_iter_min, clip_iter_max, color_only, power, kind);
    if (r[3] > 0.5) {
        return vec3(0.0, 0.0, 0.0);
    }
    if (direct_color != u32(0)) {
        this.fractwf_apply_direct_color(r[2], to_float(max_iter));
    }
    let z = scale * fractwf_compute_z(r[2], to_float(max_iter), scalez, offsetz, z_logscale, z_fill);
    let ox = 0.0;
    let oy = 0.0;
    if (color_only == u32(0)) {
        ox = scale * (r[0] + offsetx);
        oy = scale * (r[1] + offsety);
    }
    vec3(ox, oy, z)
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let power_i = to_int(to_float(p.power));
    let power = u32(clamp(power_i, 2, 8));
    let kind = u32(8);
    if (power == u32(2)) {
        kind = u32(5);
    } else if (power == u32(3)) {
        kind = u32(6);
    } else if (power == u32(4)) {
        kind = u32(7);
    }
    #{ z: color.fractwf_variation_body_2d(z, u32(to_float(p.max_iter)), p.xmin, p.xmax, p.ymin, p.ymax, 0.0, 0.0, u32(to_float(p.direct_color)), to_int(to_float(p.clip_iter_min)), to_int(to_float(p.clip_iter_max)), u32(to_float(p.max_clip_iter)), p.scale, p.offsetx, p.offsety, u32(to_float(p.color_only)), power, kind), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let power_i = to_int(to_float(p.power));
    let power = u32(clamp(power_i, 2, 8));
    let kind = u32(8);
    if (power == u32(2)) {
        kind = u32(5);
    } else if (power == u32(3)) {
        kind = u32(6);
    } else if (power == u32(4)) {
        kind = u32(7);
    }
    #{ z: color.fractwf_variation_body_3d(v, u32(to_float(p.max_iter)), p.xmin, p.xmax, p.ymin, p.ymax, 0.0, 0.0, u32(to_float(p.direct_color)), p.scalez, to_int(to_float(p.clip_iter_min)), to_int(to_float(p.clip_iter_max)), u32(to_float(p.max_clip_iter)), p.scale, p.offsetx, p.offsety, p.offsetz, p.z_fill, u32(to_float(p.z_logscale)), u32(to_float(p.color_only)), power, kind), color: color }
}
