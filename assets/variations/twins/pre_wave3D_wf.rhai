/// Pre-phase 3D wave perturbation — displaces the affine input by a sine-
/// wave amplitude `amp = w · exp(-dl · damping) · sin(2π·dl + phase)` where
/// `dl = distance/wavelen`. The displacement direction depends on `axis`:
/// planar modes (XY/YZ/ZX) push perpendicular to the plane; radial mode
/// pushes along the unit vector from the center. Damping is disabled when
/// `|damping| ≈ 0`.

// Pre Wave 3D WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    axis: #{
        value: 0, choices: ["XY → Z", "YZ → X", "ZX → Y", "Radial"],
        caption: "Axis",
        hint: "Wave plane and displacement axis. Plane modes wave one coordinate in and out of the other two; Radial waves along the unit vector from the origin.",
    },
    wavelen: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Wavelength",
        hint: "Wavelength of the sine perturbation.",
    },
    phase: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Phase",
        hint: "Phase offset on the sine argument.",
    },
    damping: #{
        value: 0.01, min: -10.0, max: 10.0, free: true,
        caption: "Damping",
        hint: "Exponential damping rate (proportional to `dl`). 0 disables damping.",
    },
    centre_x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Centre X",
        hint: "X coordinate of the wave center (subtracted from input before the distance calculation).",
    },
    centre_y: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Centre Y",
        hint: "Y coordinate of the wave center.",
    },
    centre_z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Centre Z",
        hint: "Z coordinate of the wave center.",
    },
};

const INFO = #{ title: "Pre Wave 3D WF", name: "rhai_pre_wave3d_wf", category: "full_3d", phase: "pre", z: "preserve_z" };

fn variation(z, p, ctx) {
    let axis = to_int(to_float(p.axis));
    let wavelen = p.wavelen;
    let phase = p.phase;
    let damping = p.damping;
    let cx = p.centre_x;
    let cy = p.centre_y;
    let cz = p.centre_z;
    let w = ctx.weight;
    let two_pi = 6.28318530717959;
    let dx = z.re - cx;
    let dy = z.im - cy;
    let dz = -cz;
    let r = 0.0;
    if (axis == 3) {
        r = sqrt(dx * dx + dy * dy + dz * dz);
    } else if (axis == 1) {
        r = sqrt(dy * dy + dz * dz);
    } else if (axis == 2) {
        r = sqrt(dz * dz + dx * dx);
    } else {
        r = sqrt(dx * dx + dy * dy);
    }
    let safe_wavelen = (if abs(wavelen) < 1.0e-30 { 1.0e-30 } else { wavelen });
    let dl = r / safe_wavelen;
    let amplitude = w;
    if (abs(damping) > 1.0e-6) {
        amplitude = amplitude * exp(-dl * damping);
    }
    let amp = amplitude * sin(two_pi * dl + phase);
    if (axis == 3) {
        let safe_r = (if r < 1.0e-30 { 1.0e-30 } else { r });
        return complex(z.re + dx / safe_r * amp, z.im + dy / safe_r * amp);
    }
    if (axis == 1) {
        return complex(z.re + amp, z.im);
    }
    if (axis == 2) {
        return complex(z.re, z.im + amp);
    }
    complex(z.re, z.im)
}

fn variation3(v, p, ctx) {
    let axis = to_int(to_float(p.axis));
    let wavelen = p.wavelen;
    let phase = p.phase;
    let damping = p.damping;
    let cx = p.centre_x;
    let cy = p.centre_y;
    let cz = p.centre_z;
    let w = ctx.weight;
    let two_pi = 6.28318530717959;
    let dx = v.x - cx;
    let dy = v.y - cy;
    let dz = v.z - cz;
    let r = 0.0;
    if (axis == 3) {
        r = sqrt(dx * dx + dy * dy + dz * dz);
    } else if (axis == 1) {
        r = sqrt(dy * dy + dz * dz);
    } else if (axis == 2) {
        r = sqrt(dz * dz + dx * dx);
    } else {
        r = sqrt(dx * dx + dy * dy);
    }
    let safe_wavelen = (if abs(wavelen) < 1.0e-30 { 1.0e-30 } else { wavelen });
    let dl = r / safe_wavelen;
    let amplitude = w;
    if (abs(damping) > 1.0e-6) {
        amplitude = amplitude * exp(-dl * damping);
    }
    let amp = amplitude * sin(two_pi * dl + phase);
    if (axis == 3) {
        let safe_r = (if r < 1.0e-30 { 1.0e-30 } else { r });
        return vec3(v.x + dx / safe_r * amp, v.y + dy / safe_r * amp, v.z + dz / safe_r * amp);
    }
    if (axis == 1) {
        return vec3(v.x + amp, v.y, v.z);
    }
    if (axis == 2) {
        return vec3(v.x, v.y + amp, v.z);
    }
    vec3(v.x, v.y, v.z + amp)
}
