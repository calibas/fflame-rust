/// Crown / Roger Bagula attractor — sums 14 iterations of `(sin(a^k · ±t),
/// cos(a^k · ±t)) / |a|^(b·k)` to form a complex-valued accumulator. Per-
/// iteration `t` is sampled uniformly from `[-π, π]`. The Z output is
/// `|wt|^4` (the squared magnitude squared). Based on Paul Bourke's Crown /
/// Roger Bagula attractor.
///
/// # Authors
/// - Jesus Sosa

// Crown (JS), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 5.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Base of the exponential growth (`a^k` in the iteration). Default 5.",
    },
    b: #{
        value: 0.63092977, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Power decay rate (`|a|^(b·k)` in the denominator). Default log(2)/log(3) ≈ 0.631.",
    },
};

const INFO = #{ title: "Crown (JS)", name: "rhai_crown_js", category: "full_3d" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let a = p.a;
    let b = p.b;
    let pi = 3.14159265358979;
    let t = -pi + 2.0 * pi * rand();
    let wt_re = 0.0;
    let wt_im = 0.0;
    let pow_a_k = a;
    let sign_k = -1.0;
    let safe_a = (if abs(a) < 1.0e-30 { 1.0e-30 } else { a });
    for k in 1..15 {
        let kf = to_float(k);
        let denom = pow(max(1.0e-30, abs(safe_a)), b * kf);
        let safe_denom = (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
        let th = pow_a_k * sign_k * t;
        wt_re = wt_re + sin(th) / safe_denom;
        wt_im = wt_im + cos(th) / safe_denom;
        pow_a_k = pow_a_k * a;
        sign_k = -sign_k;
    }
    let w = ctx.weight;
    let fpx = ctx.accum.re + w * wt_re;
    let fpy = ctx.accum.im + w * wt_im;
    let craw = abs(fpx * fpx + fpy * fpy);
    color = craw - to_float(floor(craw));
    #{ z: complex(wt_re, wt_im), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let a = p.a;
    let b = p.b;
    let pi = 3.14159265358979;
    let t = -pi + 2.0 * pi * rand();
    let wt_re = 0.0;
    let wt_im = 0.0;
    let pow_a_k = a;
    let sign_k = -1.0;
    let safe_a = (if abs(a) < 1.0e-30 { 1.0e-30 } else { a });
    for k in 1..15 {
        let kf = to_float(k);
        let denom = pow(max(1.0e-30, abs(safe_a)), b * kf);
        let safe_denom = (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
        let th = pow_a_k * sign_k * t;
        wt_re = wt_re + sin(th) / safe_denom;
        wt_im = wt_im + cos(th) / safe_denom;
        pow_a_k = pow_a_k * a;
        sign_k = -sign_k;
    }
    let mag2 = wt_re * wt_re + wt_im * wt_im;
    let z = mag2 * mag2;
    let w = ctx.weight;
    let fpx = ctx.accum.x + w * wt_re;
    let fpy = ctx.accum.y + w * wt_im;
    let craw = abs(fpx * fpx + fpy * fpy);
    color = craw - to_float(floor(craw));
    #{ z: vec3(wt_re, wt_im, z), color: color }
}
