/// The Hofstadter butterfly's full 3D spectral body.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Hofstadter 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    size: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Size",
        hint: "Spatial scale: the butterfly (E −4..4, flux 0..1) spans size×size in x/y, and one full phase (or coupling) period spans size in z. Flux and phase both wrap, so the body tiles.",
    },
    depth: #{
        value: 20, min: 2, max: 96,
        caption: "Max Denominator",
        hint: "Largest flux denominator q considered — hierarchy depth, band count per row, and compute cost.",
    },
    coupling: #{
        value: 1.0, min: 0.2, max: 3.0,
        caption: "Coupling",
        hint: "Almost-Mathieu coupling λ (at z = 0 in Coupling axis mode). 1 is the classic self-dual butterfly.",
    },
    hierarchy: #{
        value: 2.0, min: 0.5, max: 4.0,
        caption: "Hierarchy",
        hint: "Farey-basin weight exponent for the flux snap (see hofstadter).",
    },
    snap: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Flux Snap",
        hint: "How hard y snaps to its rational flux row.",
    },
    axis: #{
        value: 0, choices: ["Phase", "Coupling"],
        caption: "Z Axis",
        hint: "Meaning of the z dimension. Phase: the Bloch phase — bands sweep sinusoidally with z and the z-union is the classic butterfly (the honest 3D spectral body). Coupling: λ sweeps across z — the metal-to-insulator transition rendered as depth.",
    },
    coupling_span: #{
        value: 1.0, min: 0.1, max: 3.0,
        caption: "Coupling Span",
        hint: "Coupling axis mode only: how many decades λ sweeps per size of z (λ_eff = λ·10^(span·z/size)).",
    },
    slice_z: #{
        value: 0.25, min: -2.0, max: 2.0,
        caption: "Slice Z",
        hint: "Slice position. 2D render mode: the phase (or coupling) slice, φ = 2π·slice_z/size. 3D with Slice Thickness > 0: center of the z-slab.",
    },
    slice_thickness: #{
        value: 0.0, min: 0.0, max: 2.0,
        caption: "Slice Thickness",
        hint: "Slab thickness around Slice Z. 2D: > 0 superimposes nearby phase slices (approaches the classic θ-union butterfly). 3D: > 0 confines the attractor to the slab; 0 = full volume.",
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
        hint: "Isotropic random offset added after projection.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Distance", "Order", "Depth"],
        caption: "Color Mode",
        hint: "Direct-color source. Distance: palette 1 inside the spectrum, fading into the gaps. Order: flux-row denominator q — the Farey hierarchy across the palette. Depth: colors by the output z (phase / coupling position).",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Color Scale",
        hint: "Contrast for the direct-color modes.",
    },
};

const INFO = #{ title: "Hofstadter 3D", name: "rhai_hofstadter3d", category: "advanced_2d", slow: ["depth"] };

fn hofstadter3d_best_rational(alpha, qmax, w) {
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

fn hofstadter3d_disc(e, p, q, lambda) {
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

fn hofstadter3d_ln_threshold(q, lambda) {
    let t = q * ln(lambda);
    if (t > 20.0) {
        return 0.69314718 + t;
    }
    ln(2.0 + 2.0 * exp(t))
}

fn hofstadter3d_phase_band(e, p, q, lambda, phi) {
    let d = hofstadter3d_disc(e, p, q, lambda);
    let cphi = cos(phi);
    let lc = 0.69314718 + q * ln(lambda) + ln(abs(cphi) + 1.0e-20);
    let sc = (if cphi >= 0.0 { 1.0 } else { -1.0 });
    let m = max(d.x, lc);
    let f_s = d.z * exp(d.x - m) + sc * exp(lc - m);
    let g = ln(abs(f_s) + 1.0e-20) + m - 0.69314718;
    let dg = d.y * d.z * exp(d.x - m) / (f_s + (if f_s < 0.0 { -1.0e-12 } else { 1.0e-12 }));
    complex(g, dg)
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let size = max(1.0e-6, p.size);
    let depth = to_int(to_float(p.depth));
    let lambda = max(1.0e-3, p.coupling);
    let hierarchy = p.hierarchy;
    let snap = p.snap;
    let axis = u32(to_float(p.axis));
    let span = p.coupling_span;
    let slice_z = p.slice_z;
    let slice_thickness = p.slice_thickness;
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let e = 4.0 * z.re / size;
    let u = z.im / size + 0.5;
    let tile = to_float(floor(u));
    let alpha = u - tile;
    let z_eval = slice_z + (rand() - 0.5) * slice_thickness;
    let pq = hofstadter3d_best_rational(alpha, max(depth, 1), hierarchy);
    let fp = pq.re;
    let fq = pq.im;
    let phi = 6.28318530718 * z_eval / size;
    let lambda_eff = lambda * pow(10.0, span * z_eval / size);
    let max_step = 4.0 / fq;
    let g0 = 0.0;
    for i in 0..steps {
        let g = 0.0;
        let dg = 0.0;
        if (axis == u32(0)) {
            let b = hofstadter3d_phase_band(e, fp, fq, lambda, phi);
            g = b.re;
            dg = b.im;
        } else {
            let d = hofstadter3d_disc(e, fp, fq, lambda_eff);
            g = d.x - hofstadter3d_ln_threshold(fq, lambda_eff);
            dg = d.y;
        }
        if (i == 0) {
            g0 = g;
        }
        if (g <= 0.0) {
            break;
        }
        let step = g * dg / (dg * dg + 1.0e-6);
        step = clamp(step, -max_step, max_step);
        e = e - step;
    }
    if (dc_mode == u32(1)) {
        color = exp(-dc_scale * max(0.0, g0));
    } else if (dc_mode == u32(2)) {
        color = pow((fq - 1.0) / max(1.0, to_float(depth) - 1.0), 1.0 / max(0.1, dc_scale));
    } else if (dc_mode == u32(3)) {
        color = 0.5 + 0.5 * tanh(2.0 * dc_scale * z_eval / size);
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
    let axis = u32(to_float(p.axis));
    let span = p.coupling_span;
    let slice_z = p.slice_z;
    let slice_thickness = p.slice_thickness;
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let e = 4.0 * v.x / size;
    let u = v.y / size + 0.5;
    let tile = to_float(floor(u));
    let alpha = u - tile;
    let zz = v.z;
    let half_t = 0.5 * slice_thickness;
    if (half_t > 0.0) {
        zz = clamp(zz, slice_z - half_t, slice_z + half_t);
    }
    let pq = hofstadter3d_best_rational(alpha, max(depth, 1), hierarchy);
    let fp = pq.re;
    let fq = pq.im;
    let phi = 6.28318530718 * zz / size;
    let lambda_eff = lambda * pow(10.0, span * zz / size);
    let max_step = 4.0 / fq;
    let g0 = 0.0;
    for i in 0..steps {
        let g = 0.0;
        let dg = 0.0;
        if (axis == u32(0)) {
            let b = hofstadter3d_phase_band(e, fp, fq, lambda, phi);
            g = b.re;
            dg = b.im;
        } else {
            let d = hofstadter3d_disc(e, fp, fq, lambda_eff);
            g = d.x - hofstadter3d_ln_threshold(fq, lambda_eff);
            dg = d.y;
        }
        if (i == 0) {
            g0 = g;
        }
        if (g <= 0.0) {
            break;
        }
        let step = g * dg / (dg * dg + 1.0e-6);
        step = clamp(step, -max_step, max_step);
        e = e - step;
    }
    let x_row = e * size / 4.0;
    let y_row = (tile + fp / fq - 0.5) * size;
    let out = vec3(mix(v.x, x_row, strength), mix(v.y, y_row, snap * strength), zz);
    if (dc_mode == u32(1)) {
        color = exp(-dc_scale * max(0.0, g0));
    } else if (dc_mode == u32(2)) {
        color = pow((fq - 1.0) / max(1.0, to_float(depth) - 1.0), 1.0 / max(0.1, dc_scale));
    } else if (dc_mode == u32(3)) {
        color = 0.5 + 0.5 * tanh(2.0 * dc_scale * out.z / size);
    }
    if (jitter > 0.0) {
        out = out + vec3(rand() - 0.5, rand() - 0.5, rand() - 0.5) * (2.0 * jitter);
    }
    #{ z: out, color: color }
}
