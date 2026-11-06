/// Hydrogen electron-orbital isosurface attractor.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Electron Orbital, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    n: #{
        value: 3, min: 1, max: 8,
        caption: "N (Shell)",
        hint: "Principal quantum number — the shell. Higher n = larger orbital with more radial node spheres.",
    },
    l: #{
        value: 2, min: 0, max: 7,
        caption: "L (Subshell)",
        hint: "Azimuthal quantum number — the subshell shape family: 0 = s (spheres), 1 = p (dumbbells), 2 = d, 3 = f, … Clamped to n − 1.",
    },
    m: #{
        value: 0, min: -7, max: 7,
        caption: "M (Orientation)",
        hint: "Magnetic quantum number, real form — selects the lobe orientation/count within the subshell (e.g. l = 2: m = 0 is the d_z² donut-and-lobes, ±1/±2 the four-leaf clovers). Clamped to ±l.",
    },
    size: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Size",
        hint: "Spatial scale. The Bohr radius is auto-fitted so the orbital's mean radius is ~0.75·size for every n.",
    },
    iso: #{
        value: 2.0, min: 0.2, max: 8.0,
        caption: "Iso Depth",
        hint: "Isosurface level in decades below the orbital's peak density. Small values hug the density maxima (tight lobes); larger values inflate the shell outward and merge lobes.",
    },
    slice_z: #{
        value: 0.0, min: -2.0, max: 2.0,
        caption: "Slice Z",
        hint: "Slice position. 2D render mode: z of the planar cross-section. 3D mode with Slice Thickness > 0: center of the z-slab. Orbitals with a nodal plane at z = 0 (odd in z, e.g. p_z) are empty exactly there — nudge the slice or pick another m.",
    },
    slice_thickness: #{
        value: 0.0, min: 0.0, max: 2.0,
        caption: "Slice Thickness",
        hint: "Slab thickness around Slice Z. 2D: 0 cuts an exact plane; > 0 samples each point's slice z within the slab (X-ray-style superposition). 3D: 0 disables slicing (full volume); > 0 confines the attractor to the slab.",
    },
    steps: #{
        value: 3, min: 1, max: 6,
        caption: "Steps",
        hint: "Newton iterations toward the isosurface per call. 1 is soft and cloud-like; 3+ lands points crisply on the shell.",
    },
    strength: #{
        value: 0.9, min: 0.0, max: 1.0,
        caption: "Strength",
        hint: "Blend between the untouched input point (0) and the fully projected point (1). Lower values give the fuzzy electron-cloud look.",
    },
    jitter: #{
        value: 0.0, min: 0.0, max: 0.2,
        caption: "Jitter",
        hint: "Isotropic random offset added after projection — shell thickness / probability-cloud grain. 0 keeps the isosurface razor thin.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Distance", "Phase", "Depth"],
        caption: "Color Mode",
        hint: "Direct-color source, applied through the transform's Direct Color slider. Distance: palette 1 on the shell fading away from it. Phase: the classic quantum-chemistry look — lobes colored by the sign of ψ, opposite phases at opposite palette ends. Depth: colors by the output z.",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Color Scale",
        hint: "Contrast for the direct-color modes: Distance falloff sharpness, Phase saturation.",
    },
};

const INFO = #{ title: "Electron Orbital", name: "rhai_electron_orbital", category: "advanced_2d" };

fn orbital_laguerre(k, alpha, x) {
    if (k <= 0) {
        return 1.0;
    }
    let lm1 = 1.0;
    let lc = 1.0 + alpha - x;
    for i in 2..=k {
        let fi = to_float(i);
        let lnext = ((2.0 * fi - 1.0 + alpha - x) * lc - (fi - 1.0 + alpha) * lm1) / fi;
        lm1 = lc;
        lc = lnext;
    }
    lc
}

fn orbital_legendre(l, m, x) {
    let somx2 = sqrt(max(0.0, 1.0 - x * x));
    let pmm = 1.0;
    let fact = 1.0;
    for i in 0..m {
        pmm = pmm * (-fact) * somx2;
        fact = fact + 2.0;
    }
    if (l == m) {
        return pmm;
    }
    let pmmp1 = x * (2.0 * to_float(m) + 1.0) * pmm;
    let pll = pmmp1;
    for ll in m + 2..=l {
        let fll = to_float(ll);
        pll = (x * (2.0 * fll - 1.0) * pmmp1 - (fll + to_float(m) - 1.0) * pmm) / (fll - to_float(m));
        pmm = pmmp1;
        pmmp1 = pll;
    }
    pll
}

fn orbital_psi(pos, n, l, ms, size) {
    let r = max(1.0e-9, length(pos));
    let ct = clamp(pos.z / r, -1.0, 1.0);
    let rho = 4.0 * to_float(n) * r / size;
    let radial = exp(-0.5 * rho) * pow(rho, to_float(l)) * orbital_laguerre(n - l - 1, to_float(2 * l + 1), rho);
    let ma = abs(ms);
    let ang = orbital_legendre(l, ma, ct);
    if (ms != 0) {
        let phi = atan2(pos.y, pos.x);
        if (ms > 0) {
            ang = ang * cos(to_float(ma) * phi);
        } else {
            ang = ang * sin(to_float(ma) * phi);
        }
    }
    radial * ang
}

fn orbital_u(pos, n, l, ms, size) {
    let psi = orbital_psi(pos, n, l, ms, size);
    ln(psi * psi + 1.0e-30)
}

fn orbital_u_ref(n, l, ms, size) {
    let rp = (3.0 * to_float(n * n) - to_float(l * (l + 1))) * size / (4.0 * to_float(n * n));
    let inv_sqrt2 = 0.70710678;
    let u = orbital_u(vec3(0.0, 0.0, rp), n, l, ms, size);
    u = max(u, orbital_u(vec3(rp * inv_sqrt2, 0.0, rp * inv_sqrt2), n, l, ms, size));
    u = max(u, orbital_u(vec3(rp * inv_sqrt2, rp * inv_sqrt2, 0.0), n, l, ms, size));
    u = max(u, orbital_u(vec3(0.0, rp * inv_sqrt2, rp * inv_sqrt2), n, l, ms, size));
    u
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let n = max(to_int(to_float(p.n)), 1);
    let l = clamp(to_int(to_float(p.l)), 0, n - 1);
    let ms = clamp(to_int(to_float(p.m)), -l, l);
    let size = max(1.0e-6, p.size);
    let iso = p.iso;
    let slice_z = p.slice_z;
    let slice_thickness = p.slice_thickness;
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let u_target = orbital_u_ref(n, l, ms, size) - iso * 2.302585;
    let h = 0.005 * size;
    let max_step = 0.5 * size / to_float(n);
    let z_eval = slice_z + (rand() - 0.5) * slice_thickness;
    let q = z;
    for i in 0..steps {
        let q3 = vec3(q.re, q.im, z_eval);
        let f = orbital_u(q3, n, l, ms, size) - u_target;
        let g = complex(orbital_u(q3 + vec3(h, 0.0, 0.0), n, l, ms, size) - orbital_u(q3 - vec3(h, 0.0, 0.0), n, l, ms, size), orbital_u(q3 + vec3(0.0, h, 0.0), n, l, ms, size) - orbital_u(q3 - vec3(0.0, h, 0.0), n, l, ms, size)) / (2.0 * h);
        let step = f * g / (g.norm_sqr() + 1.0e-9);
        let sl = step.abs();
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }
    let out = mix(z, q, strength);
    if (dc_mode == u32(1)) {
        let p3 = vec3(z.re, z.im, z_eval);
        let f = orbital_u(p3, n, l, ms, size) - u_target;
        let g = complex(orbital_u(p3 + vec3(h, 0.0, 0.0), n, l, ms, size) - orbital_u(p3 - vec3(h, 0.0, 0.0), n, l, ms, size), orbital_u(p3 + vec3(0.0, h, 0.0), n, l, ms, size) - orbital_u(p3 - vec3(0.0, h, 0.0), n, l, ms, size)) / (2.0 * h);
        let dist = abs(f) / (g.abs() + 1.0e-6);
        color = exp(-6.0 * dc_scale * dist / (0.25 * size));
    } else if (dc_mode == u32(2)) {
        let psi_q = orbital_psi(vec3(out.re, out.im, z_eval), n, l, ms, size);
        let psi_iso = exp(0.5 * u_target);
        color = 0.5 + 0.5 * tanh(dc_scale * psi_q / (psi_iso + 1.0e-30));
    } else if (dc_mode == u32(3)) {
        color = 0.5 + 0.5 * tanh(2.0 * dc_scale * z_eval / size);
    }
    if (jitter > 0.0) {
        out = out + complex(rand() - 0.5, rand() - 0.5) * (2.0 * jitter);
    }
    #{ z: out, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let n = max(to_int(to_float(p.n)), 1);
    let l = clamp(to_int(to_float(p.l)), 0, n - 1);
    let ms = clamp(to_int(to_float(p.m)), -l, l);
    let size = max(1.0e-6, p.size);
    let iso = p.iso;
    let slice_z = p.slice_z;
    let slice_thickness = p.slice_thickness;
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let u_target = orbital_u_ref(n, l, ms, size) - iso * 2.302585;
    let h = 0.005 * size;
    let max_step = 0.5 * size / to_float(n);
    let half_t = 0.5 * slice_thickness;
    let q = v;
    for i in 0..steps {
        let f = orbital_u(q, n, l, ms, size) - u_target;
        let g = vec3(orbital_u(q + vec3(h, 0.0, 0.0), n, l, ms, size) - orbital_u(q - vec3(h, 0.0, 0.0), n, l, ms, size), orbital_u(q + vec3(0.0, h, 0.0), n, l, ms, size) - orbital_u(q - vec3(0.0, h, 0.0), n, l, ms, size), orbital_u(q + vec3(0.0, 0.0, h), n, l, ms, size) - orbital_u(q - vec3(0.0, 0.0, h), n, l, ms, size)) / (2.0 * h);
        let step = f * g / (dot(g, g) + 1.0e-9);
        let sl = length(step);
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
        if (half_t > 0.0) {
            q.z = clamp(q.z, slice_z - half_t, slice_z + half_t);
        }
    }
    let out = mix(v, q, strength);
    if (dc_mode == u32(1)) {
        let f = orbital_u(v, n, l, ms, size) - u_target;
        let g = vec3(orbital_u(v + vec3(h, 0.0, 0.0), n, l, ms, size) - orbital_u(v - vec3(h, 0.0, 0.0), n, l, ms, size), orbital_u(v + vec3(0.0, h, 0.0), n, l, ms, size) - orbital_u(v - vec3(0.0, h, 0.0), n, l, ms, size), orbital_u(v + vec3(0.0, 0.0, h), n, l, ms, size) - orbital_u(v - vec3(0.0, 0.0, h), n, l, ms, size)) / (2.0 * h);
        let dist = abs(f) / (length(g) + 1.0e-6);
        color = exp(-6.0 * dc_scale * dist / (0.25 * size));
    } else if (dc_mode == u32(2)) {
        let psi_q = orbital_psi(out, n, l, ms, size);
        let psi_iso = exp(0.5 * u_target);
        color = 0.5 + 0.5 * tanh(dc_scale * psi_q / (psi_iso + 1.0e-30));
    } else if (dc_mode == u32(3)) {
        color = 0.5 + 0.5 * tanh(2.0 * dc_scale * out.z / size);
    }
    if (jitter > 0.0) {
        out = out + vec3(rand() - 0.5, rand() - 0.5, rand() - 0.5) * (2.0 * jitter);
    }
    #{ z: out, color: color }
}
