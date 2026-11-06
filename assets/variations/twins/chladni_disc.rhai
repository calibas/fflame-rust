/// Circular-plate Chladni figure attractor (Bessel nodal lines).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Chladni Disc, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    n: #{
        value: 3, min: 0, max: 8,
        caption: "Diametric Lines",
        hint: "Angular mode number of the first mode — its count of straight nodal lines through the center. 0 contributes pure concentric rings.",
    },
    m: #{
        value: 2, min: 1, max: 8,
        caption: "Radial Circles",
        hint: "Radial mode number of the first mode — its count of nodal circles inside the plate (the rim is the m-th).",
    },
    n2: #{
        value: 5, min: 0, max: 8,
        caption: "Diametric Lines 2",
        hint: "Angular mode number of the second mode. The figure comes from the interference of the two modes — a single pure mode is always just spokes crossing rings; two modes with different symmetry bend the nodal set into the wavy flower and star figures of real plates.",
    },
    m2: #{
        value: 1, min: 1, max: 8,
        caption: "Radial Circles 2",
        hint: "Radial mode number of the second mode.",
    },
    a: #{
        value: 1.0, min: -2.0, max: 2.0,
        caption: "Mix A",
        hint: "Amplitude of the first mode.",
    },
    b: #{
        value: 0.6, min: -2.0, max: 2.0,
        caption: "Mix B",
        hint: "Amplitude of the second mode. 0 disables the interference (pure first mode); the a : b ratio morphs continuously between the two pure figures through every hybrid in between.",
    },
    radius: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Radius",
        hint: "Plate radius: both modes place a nodal circle exactly at this distance, so the rim stays a node while the interior interferes. The ripple field continues beyond it.",
    },
    phase: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Phase",
        hint: "Rotates the first mode's diametric-line pattern, in degrees.",
    },
    phase2: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Phase 2",
        hint: "Rotates the second mode independently — misaligning the two symmetry axes breaks mirror symmetry and gives spiral-flavored figures.",
    },
    steps: #{
        value: 3, min: 1, max: 6,
        caption: "Steps",
        hint: "Newton iterations toward the nodal set per call. 1 is soft and halo-like; 3+ lands points crisply on the figure.",
    },
    strength: #{
        value: 0.9, min: 0.0, max: 1.0,
        caption: "Strength",
        hint: "Blend between the untouched input point (0) and the fully projected point (1).",
    },
    jitter: #{
        value: 0.0, min: 0.0, max: 0.2,
        caption: "Jitter",
        hint: "Isotropic random offset added after projection — a sand-grain look. 0 keeps the nodal curves razor thin.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Distance", "Amplitude", "Mode Mix"],
        caption: "Color Mode",
        hint: "Direct-color source, applied through the transform's Direct Color slider. Distance: palette position 1 on the nodal curves fading to 0 away from them (great at low Strength). Amplitude: colors the regions between curves by signed vibration phase — adjacent regions of a real plate vibrate in opposite phase. Mode Mix: colors along the figure by the two modes' signed push-pull balance (varies along the nodal curves, unlike a ratio which is constant on them).",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Color Scale",
        hint: "Contrast for the direct-color modes: Distance falloff sharpness, Amplitude saturation. No effect when Color Mode is Off.",
    },
};

const INFO = #{ title: "Chladni Disc", name: "rhai_chladni_disc", category: "advanced_2d", z: "preserve_z" };

fn chladni_disc_bessj0(x) {
    let ax = abs(x);
    if (ax < 8.0) {
        let y = x * x;
        let p1 = -2957821389.0 + y * (7062834065.0 + y * (-512359803.6 + y * (10879881.29 + y * (-86327.92757 + y * 228.4622733))));
        let p2 = 40076544269.0 + y * (745249964.8 + y * (7189466.438 + y * (47447.26470 + y * (226.1030244 + y))));
        return p1 / p2;
    }
    let z = 8.0 / ax;
    let y = z * z;
    let xx = ax - 0.785398164;
    let p1 = 1.0 + y * (-0.1098628627e-2 + y * (0.2734510407e-4 + y * (-0.2073370639e-5 + y * 0.2093887211e-6)));
    let p2 = -0.1562499995e-1 + y * (0.1430488765e-3 + y * (-0.6911147651e-5 + y * (0.7621095161e-6 + y * (-0.934935152e-7))));
    sqrt(0.636619772 / ax) * (cos(xx) * p1 - z * sin(xx) * p2)
}

fn chladni_disc_bessj1(x) {
    let ax = abs(x);
    if (ax < 8.0) {
        let y = x * x;
        let p1 = x * (72362614232.0 + y * (-7895059235.0 + y * (242396853.1 + y * (-2972611.439 + y * (15704.48260 + y * (-30.16036606))))));
        let p2 = 144725228442.0 + y * (2300535178.0 + y * (18583304.74 + y * (99447.43394 + y * (376.9991397 + y))));
        return p1 / p2;
    }
    let z = 8.0 / ax;
    let y = z * z;
    let xx = ax - 2.356194491;
    let p1 = 1.0 + y * (0.183105e-2 + y * (-0.3516396496e-4 + y * (0.2457520174e-5 + y * (-0.240337019e-6))));
    let p2 = 0.04687499995 + y * (-0.2002690873e-3 + y * (0.8449199096e-5 + y * (-0.88228987e-6 + y * 0.105787412e-6)));
    sqrt(0.636619772 / ax) * (cos(xx) * p1 - z * sin(xx) * p2)
}

fn chladni_disc_bessjn(n, x) {
    if (n == 0) {
        return chladni_disc_bessj0(x);
    }
    if (n == 1) {
        return chladni_disc_bessj1(x);
    }
    let ax = abs(x);
    if (ax < 1.0e-8) {
        return 0.0;
    }
    let tox = 2.0 / ax;
    let ans = 0.0;
    if (ax > to_float(n)) {
        let bjm = chladni_disc_bessj0(ax);
        let bj = chladni_disc_bessj1(ax);
        for j in 1..n {
            let bjp = to_float(j) * tox * bj - bjm;
            bjm = bj;
            bj = bjp;
        }
        ans = bj;
    } else {
        let m_start = 2 * ((n + to_int(sqrt(40.0 * to_float(n)))) / 2);
        let jsum = false;
        let sum = 0.0;
        let bjp = 0.0;
        let bj = 1.0;
        let j = m_start;
        while j > 0 {
            let bjm = to_float(j) * tox * bj - bjp;
            bjp = bj;
            bj = bjm;
            if (abs(bj) > 1.0e10) {
                bj = bj * 1.0e-10;
                bjp = bjp * 1.0e-10;
                ans = ans * 1.0e-10;
                sum = sum * 1.0e-10;
            }
            if (jsum) {
                sum = sum + bj;
            }
            jsum = !jsum;
            if (j == n) {
                ans = bjp;
            }
            j = j - 1;
        }
        sum = 2.0 * sum - bj;
        ans = ans / sum;
    }
    ans
}

fn chladni_disc_jn_deriv(n, x) {
    let xs = max(1.0e-6, x);
    let jn = chladni_disc_bessjn(n, xs);
    let jnp = 0.0;
    if (n == 0) {
        jnp = -chladni_disc_bessj1(xs);
    } else {
        jnp = chladni_disc_bessjn(n - 1, xs) - to_float(n) / xs * jn;
    }
    complex(jn, jnp)
}

fn chladni_disc_mode_k(n, m, radius) {
    let mu = 4.0 * to_float(n) * to_float(n);
    let beta = (m + to_float(n) * 0.5 - 0.25) * 3.14159265359;
    let b8 = 8.0 * beta;
    let jnm = beta - (mu - 1.0) / b8 - 4.0 * (mu - 1.0) * (7.0 * mu - 31.0) / (3.0 * b8 * b8 * b8);
    jnm / radius
}

fn chladni_disc_mode(r, theta, n, k, phase) {
    let jd = chladni_disc_jn_deriv(n, k * r);
    let ang = to_float(n) * (theta - phase);
    let ca = cos(ang);
    vec3(jd.re * ca, k * jd.im * ca, -to_float(n) * jd.re * sin(ang))
}

fn chladni_disc_project(p, n1, k1, ph1, a, n2, k2, ph2, b, steps) {
    let max_step = 0.5 * 3.14159265359 / max(k1, k2);
    let q = p;
    for i in 0..steps {
        let r = max(1.0e-6, q.abs());
        let theta = atan2(q.im, q.re);
        let fd = a * chladni_disc_mode(r, theta, n1, k1, ph1) + b * chladni_disc_mode(r, theta, n2, k2, ph2);
        let ct = q.re / r;
        let st = q.im / r;
        let g = complex(fd.y * ct - fd.z / r * st, fd.y * st + fd.z / r * ct);
        let step = fd.x * g / (g.norm_sqr() + 1.0e-6);
        let sl = step.abs();
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }
    q
}

fn variation(w, p, ctx) {
    let color = ctx.color;
    let n1 = to_int(to_float(p.n));
    let m1 = to_float(p.m);
    let n2 = to_int(to_float(p.n2));
    let m2 = to_float(p.m2);
    let a = p.a;
    let b = p.b;
    let radius = max(1.0e-3, p.radius);
    let ph1 = p.phase * 0.01745329252;
    let ph2 = p.phase2 * 0.01745329252;
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let k1 = chladni_disc_mode_k(n1, m1, radius);
    let k2 = chladni_disc_mode_k(n2, m2, radius);
    let q = chladni_disc_project(w, n1, k1, ph1, a, n2, k2, ph2, b, steps);
    if (dc_mode != u32(0)) {
        let r0 = max(1.0e-6, w.abs());
        let th0 = atan2(w.im, w.re);
        let f1 = chladni_disc_mode(r0, th0, n1, k1, ph1);
        let f2 = chladni_disc_mode(r0, th0, n2, k2, ph2);
        let fd = a * f1 + b * f2;
        if (dc_mode == u32(1)) {
            let ct = w.re / r0;
            let st = w.im / r0;
            let g = complex(fd.y * ct - fd.z / r0 * st, fd.y * st + fd.z / r0 * ct);
            let dist = abs(fd.x) / (g.abs() + 1.0e-6);
            let cell = 3.14159265359 / max(k1, k2);
            color = exp(-6.0 * dc_scale * dist / cell);
        } else if (dc_mode == u32(2)) {
            let f_norm = 0.6 * (abs(a) + abs(b)) + 1.0e-6;
            color = 0.5 + 0.5 * tanh(2.0 * dc_scale * fd.x / f_norm);
        } else {
            let f_norm = 0.6 * (abs(a) + abs(b)) + 1.0e-6;
            color = 0.5 + 0.5 * tanh(2.0 * dc_scale * (a * f1.x - b * f2.x) / f_norm);
        }
    }
    let out = mix(w, q, strength);
    if (jitter > 0.0) {
        out = out + complex(rand() - 0.5, rand() - 0.5) * (2.0 * jitter);
    }
    #{ z: out, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let n1 = to_int(to_float(p.n));
    let m1 = to_float(p.m);
    let n2 = to_int(to_float(p.n2));
    let m2 = to_float(p.m2);
    let a = p.a;
    let b = p.b;
    let radius = max(1.0e-3, p.radius);
    let ph1 = p.phase * 0.01745329252;
    let ph2 = p.phase2 * 0.01745329252;
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let k1 = chladni_disc_mode_k(n1, m1, radius);
    let k2 = chladni_disc_mode_k(n2, m2, radius);
    let q = chladni_disc_project(complex(v.x, v.y), n1, k1, ph1, a, n2, k2, ph2, b, steps);
    if (dc_mode != u32(0)) {
        let r0 = max(1.0e-6, complex(v.x, v.y).abs());
        let th0 = atan2(complex(v.x, v.y).im, complex(v.x, v.y).re);
        let f1 = chladni_disc_mode(r0, th0, n1, k1, ph1);
        let f2 = chladni_disc_mode(r0, th0, n2, k2, ph2);
        let fd = a * f1 + b * f2;
        if (dc_mode == u32(1)) {
            let ct = complex(v.x, v.y).re / r0;
            let st = complex(v.x, v.y).im / r0;
            let g = complex(fd.y * ct - fd.z / r0 * st, fd.y * st + fd.z / r0 * ct);
            let dist = abs(fd.x) / (g.abs() + 1.0e-6);
            let cell = 3.14159265359 / max(k1, k2);
            color = exp(-6.0 * dc_scale * dist / cell);
        } else if (dc_mode == u32(2)) {
            let f_norm = 0.6 * (abs(a) + abs(b)) + 1.0e-6;
            color = 0.5 + 0.5 * tanh(2.0 * dc_scale * fd.x / f_norm);
        } else {
            let f_norm = 0.6 * (abs(a) + abs(b)) + 1.0e-6;
            color = 0.5 + 0.5 * tanh(2.0 * dc_scale * (a * f1.x - b * f2.x) / f_norm);
        }
    }
    let out = mix(complex(v.x, v.y), q, strength);
    if (jitter > 0.0) {
        out = out + complex(rand() - 0.5, rand() - 0.5) * (2.0 * jitter);
    }
    #{ z: vec3(out.re, out.im, v.z), color: color }
}
