/// Two-wave directional vibration — adds the sum of two independent sine
/// waves to the input, where each wave's direction, angle, frequency, and
/// amplitude are themselves modulated by sinusoids of the projected
/// coordinate along the wave's direction. Each wave has 5 base params
/// (`dir`, `angle`, `freq`, `amp`, `phase`) plus 4 modulation pairs
/// (`dm`/`dmfreq` for direction, `tm`/`tmfreq` for angle, `fm`/`fmfreq` for
/// frequency, `am`/`amfreq` for amplitude) — 13 params per wave × 2 waves =
/// 26 user params total, which was the first variation to exceed the old
/// 16-slot per-variation buffer ceiling and motivated switching to the
/// packed-variation-params buffer layout.
///
/// # Authors
/// - FarDareisMai
/// - Brad Stefanov

// Vibration 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    dir: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Direction",
        hint: "Wave 1: direction angle (radians) along which the wave propagates. The input is projected onto this direction before phase computation.",
    },
    angle: #{
        value: 1.5708, min: -10.0, max: 10.0, free: true,
        caption: "Angle",
        hint: "Wave 1: output rotation angle (radians) — controls which direction in the output plane the wave's displacement points.",
    },
    freq: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Frequency",
        hint: "Wave 1: spatial frequency. Smaller = longer wavelength.",
    },
    amp: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Amplitude",
        hint: "Wave 1: amplitude (peak displacement magnitude).",
    },
    phase: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Phase",
        hint: "Wave 1: phase offset in cycles; converted internally to radians as `2π·phase/freq`.",
    },
    dir2: #{
        value: 1.5708, min: -10.0, max: 10.0, free: true,
        caption: "Direction 2",
        hint: "Wave 2: direction angle. See `dir`.",
    },
    angle2: #{
        value: 1.5708, min: -10.0, max: 10.0, free: true,
        caption: "Angle 2",
        hint: "Wave 2: output rotation angle. See `angle`.",
    },
    freq2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Frequency 2",
        hint: "Wave 2: spatial frequency. See `freq`.",
    },
    amp2: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Amplitude 2",
        hint: "Wave 2: amplitude. See `amp`.",
    },
    phase2: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Phase 2",
        hint: "Wave 2: phase offset. See `phase`.",
    },
    dm: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Dir Modulation",
        hint: "Wave 1: direction modulation depth. The propagation direction wobbles by `dm · cos(d · dmfreq · 2π)` where `d` is the projected coordinate.",
    },
    dmfreq: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Dir Mod Freq",
        hint: "Wave 1: spatial frequency of the direction-modulation wobble.",
    },
    tm: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Angle Modulation",
        hint: "Wave 1: output-angle modulation depth (same shape as `dm` but applied to the output rotation angle).",
    },
    tmfreq: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Angle Mod Freq",
        hint: "Wave 1: spatial frequency of the output-angle modulation.",
    },
    fm: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Freq Modulation",
        hint: "Wave 1: frequency-modulation depth. Modulates the wave's phase term by `fm · cos(d · fmfreq · 2π) / freq`.",
    },
    fmfreq: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Freq Mod Freq",
        hint: "Wave 1: spatial frequency of the frequency modulation.",
    },
    am: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Amp Modulation",
        hint: "Wave 1: amplitude-modulation depth. Multiplies amplitude by `1 + am · cos(d · amfreq · 2π)`.",
    },
    amfreq: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Amp Mod Freq",
        hint: "Wave 1: spatial frequency of the amplitude modulation.",
    },
    d2m: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Dir 2 Modulation",
        hint: "Wave 2: direction modulation depth. See `dm`.",
    },
    d2mfreq: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Dir 2 Mod Freq",
        hint: "Wave 2: direction modulation frequency. See `dmfreq`.",
    },
    t2m: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Angle 2 Modulation",
        hint: "Wave 2: angle modulation depth. See `tm`.",
    },
    t2mfreq: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Angle 2 Mod Freq",
        hint: "Wave 2: angle modulation frequency. See `tmfreq`.",
    },
    f2m: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Freq 2 Modulation",
        hint: "Wave 2: frequency modulation depth. See `fm`.",
    },
    f2mfreq: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Freq 2 Mod Freq",
        hint: "Wave 2: frequency modulation frequency. See `fmfreq`.",
    },
    a2m: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Amp 2 Modulation",
        hint: "Wave 2: amplitude modulation depth. See `am`.",
    },
    a2mfreq: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Amp 2 Mod Freq",
        hint: "Wave 2: amplitude modulation frequency. See `amfreq`.",
    },
};

const INFO = #{ title: "Vibration 2", name: "rhai_vibration2", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn v2_modulate(amp, freq, x) {
    let two_pi = 6.28318530717959;
    amp * cos(x * freq * two_pi)
}

fn v2_wave(p, dir, angle, freq, amp, phase, dm, dmfreq, tm, tmfreq, fm, fmfreq, am, amfreq) {
    let two_pi = 6.28318530717959;
    let d_along_dir = p.re * cos(dir) + p.im * sin(dir);
    let dir_l = dir + v2_modulate(dm, dmfreq, d_along_dir);
    let angle_l = angle + v2_modulate(tm, tmfreq, d_along_dir);
    let safe_freq = (if abs(freq) < 1.0e-30 { 1.0e-30 } else { freq });
    let freq_l = v2_modulate(fm, fmfreq, d_along_dir) / safe_freq;
    let amp_l = amp + amp * v2_modulate(am, amfreq, d_along_dir);
    let total_angle = angle_l + dir_l;
    let cos_dir = cos(dir_l);
    let sin_dir = sin(dir_l);
    let cos_tot = cos(total_angle);
    let sin_tot = sin(total_angle);
    let scaled_freq = freq * two_pi;
    let phase_shift = two_pi * phase / safe_freq;
    d_along_dir = p.re * cos_dir + p.im * sin_dir;
    let local_amp = amp_l * sin(d_along_dir * scaled_freq + freq_l + phase_shift);
    complex(local_amp * cos_tot, local_amp * sin_tot)
}

fn variation(z, p) {
    let dir = p.dir;
    let angle = p.angle;
    let freq = p.freq;
    let amp = p.amp;
    let phase = p.phase;
    let dir2 = p.dir2;
    let angle2 = p.angle2;
    let freq2 = p.freq2;
    let amp2 = p.amp2;
    let phase2 = p.phase2;
    let dm = p.dm;
    let dmfreq = p.dmfreq;
    let tm = p.tm;
    let tmfreq = p.tmfreq;
    let fm = p.fm;
    let fmfreq = p.fmfreq;
    let am = p.am;
    let amfreq = p.amfreq;
    let d2m = p.d2m;
    let d2mfreq = p.d2mfreq;
    let t2m = p.t2m;
    let t2mfreq = p.t2mfreq;
    let f2m = p.f2m;
    let f2mfreq = p.f2mfreq;
    let a2m = p.a2m;
    let a2mfreq = p.a2mfreq;
    let w1 = v2_wave(z, dir, angle, freq, amp, phase, dm, dmfreq, tm, tmfreq, fm, fmfreq, am, amfreq);
    let w2 = v2_wave(z, dir2, angle2, freq2, amp2, phase2, d2m, d2mfreq, t2m, t2mfreq, f2m, f2mfreq, a2m, a2mfreq);
    z + w1 + w2
}

fn variation3(v, p) {
    let dir = p.dir;
    let angle = p.angle;
    let freq = p.freq;
    let amp = p.amp;
    let phase = p.phase;
    let dir2 = p.dir2;
    let angle2 = p.angle2;
    let freq2 = p.freq2;
    let amp2 = p.amp2;
    let phase2 = p.phase2;
    let dm = p.dm;
    let dmfreq = p.dmfreq;
    let tm = p.tm;
    let tmfreq = p.tmfreq;
    let fm = p.fm;
    let fmfreq = p.fmfreq;
    let am = p.am;
    let amfreq = p.amfreq;
    let d2m = p.d2m;
    let d2mfreq = p.d2mfreq;
    let t2m = p.t2m;
    let t2mfreq = p.t2mfreq;
    let f2m = p.f2m;
    let f2mfreq = p.f2mfreq;
    let a2m = p.a2m;
    let a2mfreq = p.a2mfreq;
    let w1 = v2_wave(complex(v.x, v.y), dir, angle, freq, amp, phase, dm, dmfreq, tm, tmfreq, fm, fmfreq, am, amfreq);
    let w2 = v2_wave(complex(v.x, v.y), dir2, angle2, freq2, amp2, phase2, d2m, d2mfreq, t2m, t2mfreq, f2m, f2mfreq, a2m, a2mfreq);
    vec3(v.x + w1.re + w2.re, v.y + w1.im + w2.im, v.z)
}
