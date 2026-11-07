/// Sin-wave additive distortion — adds `sin(2π·freq·coord) / (2π·freq)` to
/// each axis. Amplitude is inversely proportional to frequency, so higher-
/// frequency ripples are smaller. With `freq = 0`, the term degenerates to
/// `sin(coord)`.
///
/// # Authors
/// - timothy-vincent
/// - DarkBeam

// Pressure Wave, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x_freq: #{
        value: 1.0, min: -50.0, max: 50.0, free: true,
        caption: "X freq",
        hint: "X-axis sine frequency. Amplitude scales as `1 / (2π·freq)`; 0 degenerates to `sin(x)`.",
    },
    y_freq: #{
        value: 1.0, min: -50.0, max: 50.0, free: true,
        caption: "Y freq",
        hint: "Y-axis sine frequency. Same scaling rule as x_freq.",
    },
};

const INFO = #{ title: "Pressure Wave", name: "rhai_pressure_wave", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn prepare(p) {
    let two_pi = 6.28318530717959;
    let pwx = 1.0;
    let pwy = 1.0;
    if (p.x_freq != 0.0) {
        pwx = p.x_freq * two_pi;
    }
    if (p.y_freq != 0.0) {
        pwy = p.y_freq * two_pi;
    }
    #{ pwx: pwx, pwy: pwy, ipwx: 1.0 / pwx, ipwy: 1.0 / pwy }
}

fn variation(z, p) {
    let pwx = p.pwx;
    let pwy = p.pwy;
    let ipwx = p.ipwx;
    let ipwy = p.ipwy;
    complex(z.re + ipwx * sin(pwx * z.re), z.im + ipwy * sin(pwy * z.im))
}

fn variation3(v, p) {
    let pwx = p.pwx;
    let pwy = p.pwy;
    let ipwx = p.ipwx;
    let ipwy = p.ipwy;
    vec3(v.x + ipwx * sin(pwx * v.x), v.y + ipwy * sin(pwy * v.y), v.z)
}
