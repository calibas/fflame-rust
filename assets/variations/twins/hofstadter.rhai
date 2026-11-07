/// Hofstadter-butterfly spectrum attractor (electron on a 2D lattice in
/// a magnetic field).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Hofstadter Butterfly, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    size: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Size",
        hint: "Spatial scale: the full butterfly (E from −4 to 4, flux from 0 to 1) spans size×size units, centered on the origin. The flux axis wraps, so the butterfly tiles vertically.",
    },
    depth: #{
        value: 20, min: 2, max: 96,
        caption: "Max Denominator",
        hint: "Largest flux denominator q considered. Higher values add finer flux rows (each with q bands) — more hierarchy levels, thinner structure, and proportionally more computation.",
    },
    coupling: #{
        value: 1.0, min: 0.2, max: 3.0,
        caption: "Coupling",
        hint: "Almost-Mathieu coupling λ. 1 is the classic self-dual butterfly; < 1 fattens the bands (metallic regime), > 1 thins them (insulating regime).",
    },
    hierarchy: #{
        value: 2.0, min: 0.5, max: 4.0,
        caption: "Hierarchy",
        hint: "Farey-basin weight exponent: flux snaps to the rational minimizing q^hierarchy·|α − p/q|. 2 is the natural Diophantine metric; lower values let fine rationals grab more territory (denser rows), higher values favor the simple fractions.",
    },
    snap: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Flux Snap",
        hint: "How hard the y coordinate snaps to its rational flux row. 1 collapses points onto discrete rows (the classic look); lower values leave vertical haze between rows.",
    },
    steps: #{
        value: 3, min: 1, max: 6,
        caption: "Steps",
        hint: "Newton iterations walking the energy to the nearest band edge. Points already inside a band never move.",
    },
    strength: #{
        value: 0.9, min: 0.0, max: 1.0,
        caption: "Strength",
        hint: "Blend between the untouched input point (0) and the fully projected point (1).",
    },
    jitter: #{
        value: 0.0, min: 0.0, max: 0.2,
        caption: "Jitter",
        hint: "Isotropic random offset added after projection. 0 keeps the band rows razor thin.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Distance", "Order"],
        caption: "Color Mode",
        hint: "Direct-color source, applied through the transform's Direct Color slider. Distance: palette 1 inside the spectrum, fading with log-distance into the gaps. Order: colors each flux row by its denominator q — the Farey hierarchy painted across the palette (coarse rationals at one end, fine structure at the other).",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Color Scale",
        hint: "Contrast for the direct-color modes: Distance falloff sharpness, Order spread.",
    },
};

const INFO = #{ title: "Hofstadter Butterfly", name: "rhai_hofstadter", category: "advanced_2d", slow: ["depth"], z: "preserve_z" };

fn hofstadter_best_rational(alpha, qmax, w) {
    let best_p = 0.0;
    let best_q = 1.0;
    let best_score = 1.0e30;
    for q in 1..=qmax {
        let fq = to_float(q);
        let p = to_float(floor(alpha * fq + 0.5));
        let err = abs(alpha - p / fq);
        let score = pow(fq, w) * err;
        if (score < best_score) {
            best_score = score;
            best_p = p;
            best_q = fq;
        }
    }
    complex(best_p, best_q)
}

fn hofstadter_disc(e, p, q, lambda) {
    let two_pi = 6.28318530718;
    let theta = 3.14159265359 / (2.0 * q);
    let m00 = 1.0;
    let m01 = 0.0;
    let m10 = 0.0;
    let m11 = 1.0;
    let d00 = 0.0;
    let d01 = 0.0;
    let d10 = 0.0;
    let d11 = 0.0;
    let scale = 0.0;
    let iq = to_int(q);
    for j in 0..iq {
        let v = e - 2.0 * lambda * cos(two_pi * p * to_float(j) / q + theta);
        let n00 = v * m00 - m10;
        let n01 = v * m01 - m11;
        let e00 = m00 + v * d00 - d10;
        let e01 = m01 + v * d01 - d11;
        d10 = d00;
        d11 = d01;
        d00 = e00;
        d01 = e01;
        m10 = m00;
        m11 = m01;
        m00 = n00;
        m01 = n01;
        let mag = max(max(max(abs(m00), abs(m01)), max(abs(m10), abs(m11))), max(max(abs(d00), abs(d01)), max(abs(d10), abs(d11))));
        if (mag > 1.0e10) {
            let inv = 1.0 / mag;
            m00 = m00 * inv;
            m01 = m01 * inv;
            m10 = m10 * inv;
            m11 = m11 * inv;
            d00 = d00 * inv;
            d01 = d01 * inv;
            d10 = d10 * inv;
            d11 = d11 * inv;
            scale = scale + ln(mag);
        }
    }
    let tr = m00 + m11;
    let trp = d00 + d11;
    let sgn = (if tr >= 0.0 { 1.0 } else { -1.0 });
    let dln = trp / (sgn * max(1.0e-12, abs(tr)));
    vec3(ln(abs(tr) + 1.0e-20) + scale, dln, sgn)
}

fn hofstadter_ln_threshold(q, lambda) {
    let t = q * ln(lambda);
    if (t > 20.0) {
        return 0.69314718 + t;
    }
    ln(2.0 + 2.0 * exp(t))
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let size = max(1.0e-6, p.size);
    let depth = to_int(to_float(p.depth));
    let lambda = max(1.0e-3, p.coupling);
    let hierarchy = p.hierarchy;
    let snap = p.snap;
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let e = 4.0 * z.re / size;
    let u = z.im / size + 0.5;
    let tile = to_float(floor(u));
    let alpha = u - tile;
    let pq = hofstadter_best_rational(alpha, max(depth, 1), hierarchy);
    let fp = pq.re;
    let fq = pq.im;
    let ln_thr = hofstadter_ln_threshold(fq, lambda);
    let max_step = 4.0 / fq;
    let g0 = 0.0;
    for i in 0..steps {
        let d = hofstadter_disc(e, fp, fq, lambda);
        let g = d.x - ln_thr;
        if (i == 0) {
            g0 = g;
        }
        if (g <= 0.0) {
            break;
        }
        let step = g * d.y / (d.y * d.y + 1.0e-6);
        step = clamp(step, -max_step, max_step);
        e = e - step;
    }
    if (dc_mode == u32(1)) {
        color = exp(-dc_scale * max(0.0, g0));
    } else if (dc_mode == u32(2)) {
        color = pow((fq - 1.0) / max(1.0, to_float(depth) - 1.0), 1.0 / max(0.1, dc_scale));
    }
    let x_row = e * size / 4.0;
    let y_row = (tile + fp / fq - 0.5) * size;
    let out = complex(mix(z.re, x_row, strength), mix(z.im, y_row, snap * strength));
    if (jitter > 0.0) {
        out = out + complex(rand() - 0.5, rand() - 0.5) * (2.0 * jitter);
    }
    #{ z: out, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let size = max(1.0e-6, p.size);
    let depth = to_int(to_float(p.depth));
    let lambda = max(1.0e-3, p.coupling);
    let hierarchy = p.hierarchy;
    let snap = p.snap;
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let e = 4.0 * v.x / size;
    let u = v.y / size + 0.5;
    let tile = to_float(floor(u));
    let alpha = u - tile;
    let pq = hofstadter_best_rational(alpha, max(depth, 1), hierarchy);
    let fp = pq.re;
    let fq = pq.im;
    let ln_thr = hofstadter_ln_threshold(fq, lambda);
    let max_step = 4.0 / fq;
    let g0 = 0.0;
    for i in 0..steps {
        let d = hofstadter_disc(e, fp, fq, lambda);
        let g = d.x - ln_thr;
        if (i == 0) {
            g0 = g;
        }
        if (g <= 0.0) {
            break;
        }
        let step = g * d.y / (d.y * d.y + 1.0e-6);
        step = clamp(step, -max_step, max_step);
        e = e - step;
    }
    if (dc_mode == u32(1)) {
        color = exp(-dc_scale * max(0.0, g0));
    } else if (dc_mode == u32(2)) {
        color = pow((fq - 1.0) / max(1.0, to_float(depth) - 1.0), 1.0 / max(0.1, dc_scale));
    }
    let x_row = e * size / 4.0;
    let y_row = (tile + fp / fq - 0.5) * size;
    let out = complex(mix(v.x, x_row, strength), mix(v.y, y_row, snap * strength));
    if (jitter > 0.0) {
        out = out + complex(rand() - 0.5, rand() - 0.5) * (2.0 * jitter);
    }
    #{ z: vec3(out.re, out.im, v.z), color: color }
}
