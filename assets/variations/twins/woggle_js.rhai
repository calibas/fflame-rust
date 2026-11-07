/// N-tile fold attractor — for each iteration picks a random tile index `c
/// ∈ [0, m)`, computes the angle `θ = 2π·c/m`, and applies a sign-dependent
/// fold based on tile parity. Each tile contributes a different rotated
/// map; the result is an N-fold woggle pattern.
///
/// # Authors
/// - Jesus Sosa

// Woggle (JS), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    m: #{
        value: 2, min: 2, max: 12,
        caption: "M",
        hint: "Number of tiles (2-12). Each iteration samples one of `m` evenly-spaced angles.",
    },
};

const INFO = #{ title: "Woggle (JS)", name: "rhai_woggle_js", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let m = clamp(to_int(to_float(p.m)), 2, 12);
    let mf = to_float(m);
    let two_pi = 6.28318530717959;
    let r = sqrt(1.25) * sqrt(mf);
    let safe_r = (if abs(r) < 1.0e-30 { 1.0e-30 } else { r });
    let c = to_int(rand() * mf);
    let c_clamped = clamp(c, 0, m - 1);
    let theta = two_pi * to_float(c_clamped) / mf;
    let a_c = cos(theta);
    let b_c = sin(theta);
    let r2 = sqrt(z.re * z.re + z.im * z.im);
    let safe_r2 = (if abs(r2) < 1.0e-30 { 1.0e-30 } else { r2 });
    let ra = 1.0 / (sqrt(3.0) * safe_r2);
    let x = 0.0;
    let y = 0.0;
    if ((c_clamped & 1) == 0) {
        x = -z.re / safe_r + ra * z.im / safe_r + a_c;
        y = -ra * z.re / safe_r - z.im / safe_r + b_c;
    } else {
        x = z.re / safe_r + ra * z.im / safe_r + a_c;
        y = -ra * z.re / safe_r + z.im / safe_r + b_c;
    }
    complex(x, y)
}
