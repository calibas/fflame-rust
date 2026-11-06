/// The filled quaternion Julia set by escape-time membership (Paul
/// Bourke's method).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Quaternion Julia (Solid), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    cx: #{
        value: 0.2, min: -2.0, max: 2.0,
        caption: "Constant X",
        hint: "Vector-i component of the Julia constant c.",
    },
    cy: #{
        value: 0.0, min: -2.0, max: 2.0,
        caption: "Constant Y",
        hint: "Vector-j component of c. 3D only.",
    },
    cz: #{
        value: 0.0, min: -2.0, max: 2.0,
        caption: "Constant Z",
        hint: "Vector-k component of c. 3D only.",
    },
    cw: #{
        value: -1.0, min: -2.0, max: 2.0,
        caption: "Constant W",
        hint: "Scalar (real) component of c. Bourke writes c scalar-first, so his c=(-1,0.2,0,0) is cw=-1, cx=0.2. 2D mode ignores this (uses cx+i*cy).",
    },
    power: #{
        value: 2, min: 2, max: 8,
        caption: "Power (N)",
        hint: "The Julia power n in q^n + c. 2 = the classic quadratic set; 3+ = higher-order. n=2 uses the exact Hamilton product.",
    },
    escape_iters: #{
        value: 16, min: 2, max: 64,
        caption: "Escape Iterations",
        hint: "Membership-test depth: how many times to apply q -> q^n + c before declaring a sample interior. Higher = sharper boundary (fewer false-interior points) but thinner surviving interior and slower.",
    },
    bailout: #{
        value: 2.0, min: 1.0, max: 8.0,
        caption: "Bailout",
        hint: "Escape radius: a sample escapes (and is hidden) once |q| exceeds this. ~2 is standard for |c|~1.",
    },
    sample_range: #{
        value: 2.0, min: 0.2, max: 4.0,
        caption: "Sample Range",
        hint: "Radius of the BALL the samples are drawn from (a ball, not a cube — so no box-face clipping). The whole set lies within |q| <= bailout, so leaving this at the bailout radius (2) guarantees the entire cross-section is covered. Shrink it ONLY to trade coverage for density on a set you know is smaller — too small clips (now as a sphere, not a cube).",
    },
    w_slice: #{
        value: 0.0, min: -2.0, max: 2.0,
        caption: "Slice Value",
        hint: "Position of the cutting hyperplane along the Slice Axis (3D only). Sweep it (e.g. -0.6..0.6) to walk Bourke's slice series through the 4D solid.",
    },
    surface: #{
        value: 0.03, min: 0.0, max: 0.5,
        caption: "Surface Shell",
        hint: "0 = fill the SOLID interior (shows the object's form). >0 = plot only the boundary SHELL of this WORLD-SPACE thickness, found with a distance estimator (the exterior points within `surface` of the set), hiding the interior and deep exterior. This reveals Bourke's fractal surface detail at uniform density; ~0.02-0.05 is a crisp skin, larger = thicker/fuzzier.",
    },
    w_color: #{
        value: 2.0, min: 0.0, max: 8.0, free: true,
        caption: "Color Scale",
        hint: "0 = off. >0 = write a palette index per Color Mode: escape-time bands (mode 0, scale = band frequency) or depth-into-shell (mode 1, scale = contrast exponent). Needs the transform's direct_color > 0. (Crawl + mode 0 colors by radius instead — the escape count isn't retained across rejected steps.)",
    },
    crawl: #{
        value: false,
        caption: "Surface Crawl",
        hint: "OFF = uniform Monte-Carlo sampling (simple, but ~97% of samples miss a thin surface shell). ON = a per-thread walk that stays pinned to the boundary via the distance estimator, so nearly every step plots a useful surface point — a big speedup for SURFACE renders. Needs Surface Shell > 0 (uses 0.02 if left at 0). Leave OFF for solid fill (uniform is already efficient there).",
    },
    crawl_step: #{
        value: 0.04, min: 0.005, max: 0.3,
        caption: "Crawl Step",
        hint: "Crawl mode only: size of each random step along the surface. Smaller = hugs the surface tighter but traverses slower; larger = faster coverage but looser. ~0.02-0.06 works well; scale it down for finer detail.",
    },
    slice_axis: #{
        value: 2, min: 0, max: 3, free: true,
        caption: "Slice Axis",
        hint: "Which quaternion component the cutting hyperplane pins to Slice Value: 0 = x/i, 1 = y/j, 2 = z/k, 3 = w/real. Default 2 (k): for a complex c (cy=cz=0, e.g. the default -1+0.2i) that slice CONTAINS the complex plane, giving the classic detailed Bourke solid. The real-axis slice (3) of a complex c is ALWAYS a smooth featureless ball of revolution — only useful for fully general constants. For vector-axis slices the real axis is plotted as screen Y so the Julia detail faces the camera.",
    },
    color_mode: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Color Mode",
        hint: "What Color Scale writes to the palette index. 0 = escape-time bands: fract(escape_fraction * scale) — wraps, so the palette needs black at position 0 to keep the outer shell dark. 1 = depth-into-shell: (1 - DE/surface)^scale — monotone, true surface at the palette's TOP end, shell edge at 0, no wrapping; any dark-at-0 palette just works, and Color Scale acts as a contrast exponent (>1 concentrates brightness at the surface). Shell mode only (solid interiors keep the base color).",
    },
};

const INFO = #{ title: "Quaternion Julia (Solid)", name: "rhai_quaternion_julia_set", category: "advanced_2d", slow: ["escape_iters"] };

fn init(p) {
    #{ st0: 0.0, st1: 0.0, st2: 0.0, dec: 0.0, st4: 0.0 }
}

fn qjset_qmul(a, b) {
    vec4(a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y, a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x, a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w, a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z)
}

fn qjset_qpow(q, n) {
    let mag = sqrt(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w) + 1.0e-12;
    let rad = pow(mag, n);
    let ang = acos(clamp(q.w / mag, -1.0, 1.0)) * n;
    let vlen = length(vec3(q.x, q.y, q.z));
    let nhat = (if vlen > 1.0e-9 { vec3(q.x, q.y, q.z) / vlen } else { vec3(1.0, 0.0, 0.0) });
    let t1 = rad * sin(ang) * nhat;
    vec4(t1.x, t1.y, t1.z, rad * cos(ang))
}

fn qjset_sample_ball(range) {
    let costheta = 2.0 * rand() - 1.0;
    let phi = 6.28318530718 * rand();
    let rr = range * pow(rand(), 0.33333333);
    let sintheta = sqrt(max(0.0, 1.0 - costheta * costheta));
    vec3(rr * sintheta * cos(phi), rr * sintheta * sin(phi), rr * costheta)
}

fn qjset_rand_dir() {
    let costheta = 2.0 * rand() - 1.0;
    let phi = 6.28318530718 * rand();
    let sintheta = sqrt(max(0.0, 1.0 - costheta * costheta));
    vec3(sintheta * cos(phi), sintheta * sin(phi), costheta)
}

fn qjset_q4(pos, sval, axis) {
    let t1 = (axis);
    if t1 == u32(0) {
        vec4(sval, pos.x, pos.z, pos.y)
    } else if t1 == u32(1) {
        vec4(pos.x, sval, pos.z, pos.y)
    } else if t1 == u32(2) {
        vec4(pos.x, pos.z, sval, pos.y)
    } else {
        vec4(pos.x, pos.y, pos.z, sval)
    }
}

fn qjset_eval(pos, sval, axis, c, n, is_sq, bail2, maxit, need_de) {
    let q = qjset_q4(pos, sval, axis);
    let dr = 1.0;
    let esc_i = maxit;
    let escaped = false;
    for i in 0..maxit {
        if (need_de) {
            let ql = sqrt(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w);
            if (is_sq) {
                dr = 2.0 * ql * dr;
            } else {
                dr = n * pow(ql, n - 1.0) * dr;
            }
        }
        if (is_sq) {
            q = qjset_qmul(q, q) + c;
        } else {
            q = qjset_qpow(q, n) + c;
        }
        if ((q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w) > bail2) {
            escaped = true;
            esc_i = i;
            break;
        }
    }
    let frac = to_float(esc_i) / to_float(maxit);
    if (!escaped) {
        return vec3(1.0e3, 0.0, frac);
    }
    if (!need_de) {
        return vec3(0.0, 1.0, frac);
    }
    let ql = sqrt(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w);
    vec3(0.5 * ql * ln(ql) / max(1.0e-9, dr), 1.0, frac)
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let hidden = ctx.hide;
    let range = p.sample_range;
    let rr = range * sqrt(rand());
    let phi = 6.28318530718 * rand();
    let sx = rr * cos(phi);
    let sy = rr * sin(phi);
    let cx = p.cx;
    let cy = p.cy;
    let n = to_float(p.power);
    let bail2 = pow(p.bailout, 2.0);
    let maxit = to_int(to_float(p.escape_iters) + 0.5);
    let surface = p.surface;
    let need_de = surface > 0.0;
    let zx = sx;
    let zy = sy;
    let dr = 1.0;
    let escaped = false;
    let esc_i = maxit;
    for i in 0..maxit {
        if (need_de) {
            let zl = sqrt(zx * zx + zy * zy);
            if (n == 2.0) {
                dr = 2.0 * zl * dr;
            } else {
                dr = n * pow(zl, n - 1.0) * dr;
            }
        }
        let r = pow(zx * zx + zy * zy, n * 0.5);
        let th = atan2(zy, zx) * n;
        zx = r * cos(th) + cx;
        zy = r * sin(th) + cy;
        if (zx * zx + zy * zy > bail2) {
            escaped = true;
            esc_i = i;
            break;
        }
    }
    let de = 1.0e9;
    if (escaped && need_de) {
        let zl = sqrt(zx * zx + zy * zy);
        de = 0.5 * zl * ln(zl) / max(1.0e-9, dr);
    }
    let wcol = p.w_color;
    let cmode = u32(to_float(p.color_mode) + 0.5);
    if (surface <= 0.0) {
        if (escaped) {
            hidden = true;
        }
    } else if (!escaped || de > surface) {
        hidden = true;
    } else if (wcol > 1.0e-6) {
        if (cmode == u32(1)) {
            color = pow(clamp(1.0 - de / surface, 0.0, 1.0), wcol);
        } else {
            color = fract((to_float(esc_i) / to_float(maxit)) * wcol);
        }
    }
    #{ z: complex(sx, sy), color: color, hide: hidden }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let hidden = ctx.hide;
    let c = vec4(p.cx, p.cy, p.cz, p.cw);
    let n = to_float(p.power);
    let is_sq = abs(n - 2.0) < 0.5;
    let maxit = to_int(to_float(p.escape_iters) + 0.5);
    let bail2 = pow(p.bailout, 2.0);
    let range = p.sample_range;
    let wsl = p.w_slice;
    let surface = p.surface;
    let wcol = p.w_color;
    let crawl = to_float(p.crawl) > 0.5;
    let axis = min(u32(to_float(p.slice_axis) + 0.5), u32(3));
    let cmode = u32(to_float(p.color_mode) + 0.5);
    if (!crawl) {
        let pos = qjset_sample_ball(range);
        let ev = qjset_eval(pos, wsl, axis, c, n, is_sq, bail2, maxit, surface > 0.0);
        let de = ev.x;
        let escaped = ev.y > 0.5;
        if (surface <= 0.0) {
            if (escaped) {
                hidden = true;
            }
        } else if (!escaped || de > surface) {
            hidden = true;
        } else if (wcol > 1.0e-6) {
            if (cmode == u32(1)) {
                color = pow(clamp(1.0 - de / surface, 0.0, 1.0), wcol);
            } else {
                color = fract(ev.z * wcol);
            }
        }
        return #{ z: pos, color: color, hide: hidden };
    }
    let surf = (if surface <= 0.0 { 0.02 } else { surface });
    let step = p.crawl_step;
    let pc = vec3(this.st0, this.st1, this.st2);
    let dec = this.dec;
    if (this.st4 < 0.5) {
        pc = qjset_sample_ball(range);
        dec = qjset_eval(pc, wsl, axis, c, n, is_sq, bail2, maxit, true).x;
        this.st4 = 1.0;
    } else if (rand() < 0.01) {
        pc = qjset_sample_ball(range);
        dec = qjset_eval(pc, wsl, axis, c, n, is_sq, bail2, maxit, true).x;
    } else {
        let pn = pc + step * qjset_rand_dir();
        if (dot(pn, pn) <= range * range) {
            let den = qjset_eval(pn, wsl, axis, c, n, is_sq, bail2, maxit, true).x;
            if (den <= dec || den < surf) {
                pc = pn;
                dec = den;
            }
        }
    }
    this.st0 = pc.x;
    this.st1 = pc.y;
    this.st2 = pc.z;
    this.dec = dec;
    if (dec >= surf) {
        hidden = true;
    } else if (wcol > 1.0e-6) {
        if (cmode == u32(1)) {
            color = pow(clamp(1.0 - dec / surf, 0.0, 1.0), wcol);
        } else {
            color = fract(length(pc) * wcol);
        }
    }
    #{ z: pc, color: color, hide: hidden }
}
