/// Polar wave-blur — emits points on a circle whose radius is a wave
/// function `(acos(±rnd) − (n+1)·π + phase) / (count·π)` of a uniform-
/// random `rnd ∈ [-1, 1]`, with a random integer `n ∈ [0, count)`. The
/// angular position is uniform. 3D mode adds a Z component `(cos(rnd) −
/// 0.5) · amplitude_z`, optionally exponentially damped by `damping_z`.

// Wave Blur WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    count: #{
        value: 5, min: 1, max: 100,
        caption: "Count",
        hint: "Maximum integer multiplier of π in the wave-radius formula (≥ 1).",
    },
    amplitude_z: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Amplitude Z",
        hint: "Z-axis wave amplitude (3D only). 0 zeroes the Z output.",
    },
    phase: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Phase",
        hint: "Phase offset added to the wave-radius numerator.",
    },
    damping_z: #{
        value: 0.0, min: 0.0, max: 10.0, free: true,
        caption: "Damping Z",
        hint: "Exponential damping rate on Z (clamped ≥ 0 in upstream). 0 disables damping.",
    },
    color_scale: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Color Scale",
        hint: "Color register multiplier in `clamp(acos(rnd) · color_scale + color_offset, 0, 1)`. Active when `direct_color` is on.",
    },
    color_offset: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Color Offset",
        hint: "Color register offset added after the `acos(rnd) · color_scale` term.",
    },
    direct_color: #{
        value: false,
        caption: "Direct Color",
        hint: "When on, writes the color register from `acos(rnd)` scaled by `color_scale` and shifted by `color_offset`, clamped to `[0, 1]`. Visible color requires the transform's Direct Color slider > 0.",
    },
};

const INFO = #{ title: "Wave Blur WF", name: "rhai_waveblur_wf", category: "full_3d", phase: "any" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let count_p = max(to_int(to_float(p.count)), 1);
    let phase = p.phase;
    let color_scale = p.color_scale;
    let color_offset = p.color_offset;
    let direct_color = to_int(to_float(p.direct_color));
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let ang = rand() * two_pi;
    let rnd = 1.0 - 2.0 * rand();
    let n = to_int(rand() * to_float(count_p));
    let r = 0.0;
    if (rand() < 0.5) {
        r = acos(rnd) - to_float(n + 1) * pi + phase;
    } else {
        r = acos(-rnd) - to_float(n + 1) * pi + phase;
    }
    let scale = 1.0 / (to_float(count_p) * pi);
    r = r * scale;
    if (direct_color != 0) {
        color = clamp(acos(rnd) * color_scale + color_offset, 0.0, 1.0);
    }
    #{ z: complex(r * cos(ang), r * sin(ang)), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let count_p = max(to_int(to_float(p.count)), 1);
    let amplitude_z = p.amplitude_z;
    let phase = p.phase;
    let damping_z = p.damping_z;
    let color_scale = p.color_scale;
    let color_offset = p.color_offset;
    let direct_color = to_int(to_float(p.direct_color));
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let ang = rand() * two_pi;
    let rnd = 1.0 - 2.0 * rand();
    let n = to_int(rand() * to_float(count_p));
    let r = 0.0;
    if (rand() < 0.5) {
        r = acos(rnd) - to_float(n + 1) * pi + phase;
    } else {
        r = acos(-rnd) - to_float(n + 1) * pi + phase;
    }
    let scale = 1.0 / (to_float(count_p) * pi);
    r = r * scale;
    if (direct_color != 0) {
        color = clamp(acos(rnd) * color_scale + color_offset, 0.0, 1.0);
    }
    let nz = 0.0;
    if (abs(amplitude_z) > 1.0e-30) {
        let zamp = (cos(rnd) - 0.5) * amplitude_z;
        if (abs(damping_z) > 1.0e-6) {
            let dmp = r * w * damping_z;
            zamp = zamp * exp(dmp);
        }
        nz = zamp;
    }
    #{ z: vec3(r * cos(ang), r * sin(ang), nz), color: color }
}
