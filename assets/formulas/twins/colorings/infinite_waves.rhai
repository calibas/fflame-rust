// Infinite Waves, the built-in colouring's twin (Kalles Fraktaler's
// multi-wave colouring, `KF_InfiniteWaves`): no palette, but sinusoids of
// the iteration value, each driving the hue, the saturation or the
// brightness, the waves on each part averaged. A wave of period P is
// sin(pi iter / P) / 2 + 1/2; a negative period the constant -P / 100.

const PARAMS = #{
    scale: #{
        value: 1.0, min: 0.0001, max: 100.0, exponential: true,
        caption: "Scale",
        hint: "Multiplies the iteration value before the waves read it (Kalles Fraktaler's 1 / Iteration Division).",
    },
    offset: #{
        value: 0.0, min: -10000.0, max: 10000.0,
        caption: "Offset",
        hint: "Added to the iteration value, which shifts every wave's phase (Kalles Fraktaler's Color Offset).",
    },
    smooth: #{
        value: 1, choices: ["Stepped", "Smooth"],
        caption: "Waves",
        hint: "Smooth follows the continuous iteration value; Stepped takes it at whole iterations, so every band is flat.",
    },
    blend: #{
        value: 0, choices: ["Off", "Half"],
        caption: "Palette",
        hint: "Off draws the waves alone. Half mixes the palette in 50/50, a cycle per 1024 of the iteration value, as Kalles Fraktaler's Blend does.",
    },
    wave1: #{
        value: 100.0, min: -100.0, max: 100000.0,
        caption: "Wave 1",
        hint: "The wave's period in iterations (a full cycle is twice it). Negative is a constant, -period/100. 0 is no wave.",
    },
    wave1_channel: #{
        value: 0, choices: ["Hue", "Saturation", "Brightness"],
        caption: "Wave 1 drives",
        hint: "Which part of the colour the wave drives. The waves on each are averaged; one with none is 0 (no saturation wave is grey).",
    },
    wave2: #{
        value: 111.0, min: -100.0, max: 100000.0,
        caption: "Wave 2",
        hint: "The wave's period in iterations (a full cycle is twice it). Negative is a constant, -period/100. 0 is no wave.",
    },
    wave2_channel: #{
        value: 1, choices: ["Hue", "Saturation", "Brightness"],
        caption: "Wave 2 drives",
        hint: "Which part of the colour the wave drives. The waves on each are averaged; one with none is 0 (no saturation wave is grey).",
    },
    wave3: #{
        value: 123.0, min: -100.0, max: 100000.0,
        caption: "Wave 3",
        hint: "The wave's period in iterations (a full cycle is twice it). Negative is a constant, -period/100. 0 is no wave.",
    },
    wave3_channel: #{
        value: 2, choices: ["Hue", "Saturation", "Brightness"],
        caption: "Wave 3 drives",
        hint: "Which part of the colour the wave drives. The waves on each are averaged; one with none is 0 (no saturation wave is grey).",
    },
    wave4: #{
        value: 0.0, min: -100.0, max: 100000.0,
        caption: "Wave 4",
        hint: "The wave's period in iterations (a full cycle is twice it). Negative is a constant, -period/100. 0 is no wave.",
    },
    wave4_channel: #{
        value: 0, choices: ["Hue", "Saturation", "Brightness"],
        caption: "Wave 4 drives",
        hint: "Which part of the colour the wave drives. The waves on each are averaged; one with none is 0 (no saturation wave is grey).",
    },
    wave5: #{
        value: 0.0, min: -100.0, max: 100000.0,
        caption: "Wave 5",
        hint: "The wave's period in iterations (a full cycle is twice it). Negative is a constant, -period/100. 0 is no wave.",
    },
    wave5_channel: #{
        value: 0, choices: ["Hue", "Saturation", "Brightness"],
        caption: "Wave 5 drives",
        hint: "Which part of the colour the wave drives. The waves on each are averaged; one with none is 0 (no saturation wave is grey).",
    },
    wave6: #{
        value: 0.0, min: -100.0, max: 100000.0,
        caption: "Wave 6",
        hint: "The wave's period in iterations (a full cycle is twice it). Negative is a constant, -period/100. 0 is no wave.",
    },
    wave6_channel: #{
        value: 0, choices: ["Hue", "Saturation", "Brightness"],
        caption: "Wave 6 drives",
        hint: "Which part of the colour the wave drives. The waves on each are averaged; one with none is 0 (no saturation wave is grey).",
    },
};

const INFO = #{ title: "Infinite Waves", region: "outside" };

// A wave at the iteration value: sin(pi iter / period), its argument
// reduced to a turn first so a large count keeps its phase in f32.
fn wave(period, iter) {
    if period < 0.0 {
        -period / 100.0
    } else {
        sin(6.2831855 * fract(iter / (2.0 * period))) * 0.5 + 0.5
    }
}

// One part of hsv2rgb (lolengine), as Kalles Fraktaler has it: `k` the
// part's offset, mix written as WGSL's.
fn hsv_part(hue, sat, val, k) {
    let q = abs(fract(hue + k) * 6.0 - 3.0);
    val * (1.0 + (clamp(q - 1.0, 0.0, 1.0) - 1.0) * sat)
}

// Kalles Fraktaler's iteration value.
fn finish(s, z, p, ctx) {
    (to_float(ctx.numiter) + 1.0 - ctx.escape_fraction) * p.scale + p.offset
}

fn color(s, z, p, ctx) {
    let iter = if p.smooth == 1 { ctx.value } else { to_float(floor(ctx.value)) };
    // Each wave on its part, in order; a period of 0 is no wave.
    let hue_sum = 0.0;
    let hue_n = 0.0;
    let sat_sum = 0.0;
    let sat_n = 0.0;
    let val_sum = 0.0;
    let val_n = 0.0;
    if p.wave1 != 0.0 {
        let g = wave(p.wave1, iter);
        if p.wave1_channel == 0 {
            hue_sum = hue_sum + g;
            hue_n = hue_n + 1.0;
        } else if p.wave1_channel == 1 {
            sat_sum = sat_sum + g;
            sat_n = sat_n + 1.0;
        } else {
            val_sum = val_sum + g;
            val_n = val_n + 1.0;
        }
    }
    if p.wave2 != 0.0 {
        let g = wave(p.wave2, iter);
        if p.wave2_channel == 0 {
            hue_sum = hue_sum + g;
            hue_n = hue_n + 1.0;
        } else if p.wave2_channel == 1 {
            sat_sum = sat_sum + g;
            sat_n = sat_n + 1.0;
        } else {
            val_sum = val_sum + g;
            val_n = val_n + 1.0;
        }
    }
    if p.wave3 != 0.0 {
        let g = wave(p.wave3, iter);
        if p.wave3_channel == 0 {
            hue_sum = hue_sum + g;
            hue_n = hue_n + 1.0;
        } else if p.wave3_channel == 1 {
            sat_sum = sat_sum + g;
            sat_n = sat_n + 1.0;
        } else {
            val_sum = val_sum + g;
            val_n = val_n + 1.0;
        }
    }
    if p.wave4 != 0.0 {
        let g = wave(p.wave4, iter);
        if p.wave4_channel == 0 {
            hue_sum = hue_sum + g;
            hue_n = hue_n + 1.0;
        } else if p.wave4_channel == 1 {
            sat_sum = sat_sum + g;
            sat_n = sat_n + 1.0;
        } else {
            val_sum = val_sum + g;
            val_n = val_n + 1.0;
        }
    }
    if p.wave5 != 0.0 {
        let g = wave(p.wave5, iter);
        if p.wave5_channel == 0 {
            hue_sum = hue_sum + g;
            hue_n = hue_n + 1.0;
        } else if p.wave5_channel == 1 {
            sat_sum = sat_sum + g;
            sat_n = sat_n + 1.0;
        } else {
            val_sum = val_sum + g;
            val_n = val_n + 1.0;
        }
    }
    if p.wave6 != 0.0 {
        let g = wave(p.wave6, iter);
        if p.wave6_channel == 0 {
            hue_sum = hue_sum + g;
            hue_n = hue_n + 1.0;
        } else if p.wave6_channel == 1 {
            sat_sum = sat_sum + g;
            sat_n = sat_n + 1.0;
        } else {
            val_sum = val_sum + g;
            val_n = val_n + 1.0;
        }
    }
    let hue = hue_sum / max(hue_n, 1.0);
    let sat = sat_sum / max(sat_n, 1.0);
    let val = val_sum / max(val_n, 1.0);
    let r = hsv_part(hue, sat, val, 1.0);
    let g = hsv_part(hue, sat, val, 2.0 / 3.0);
    let b = hsv_part(hue, sat, val, 1.0 / 3.0);
    // Half the palette mixed back in, a cycle per 1024, in display space.
    let mixed = if p.blend == 1 {
        let pal = palette_srgb(fract(ctx.value / 1024.0));
        vec3(r + (pal.x - r) * 0.5, g + (pal.y - g) * 0.5, b + (pal.z - b) * 0.5)
    } else {
        vec3(r, g, b)
    };
    // Display space, decoded as the palette is.
    vec3(pow(max(mixed.x, 0.0), 2.2), pow(max(mixed.y, 0.0), 2.2), pow(max(mixed.z, 0.0), 2.2))
}
