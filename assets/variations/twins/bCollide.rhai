/// Bipolar warp with σ-axis branch-and-mod — converts the input to bipolar
/// coordinates `(τ, σ)`, splits σ into `num` equal arcs, and mirrors
/// alternating arcs around their boundaries with a per-arc offset `a`.
/// Produces a bipolar tiling with controllable wedge count.
///
/// # Authors
/// - Michael Faber

// BCollide, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    num: #{
        value: 1, min: 1, max: 50,
        caption: "N",
        hint: "Number of σ wedges. Higher = finer tiling.",
    },
    a: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Per-wedge angular offset.",
    },
};

const INFO = #{ title: "BCollide", name: "rhai_bcollide", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let num = max(to_float(p.num), 1.0);
    let a = p.a;
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let pi_n = pi / num;
    let a_n = pi * a / num;
    let n_invpi = num * inv_pi;
    let tau = 0.5 * (ln(max((z.re + 1.0) * (z.re + 1.0) + z.im * z.im, 1.0e-30)) - ln(max((z.re - 1.0) * (z.re - 1.0) + z.im * z.im, 1.0e-30)));
    let sigma = pi - atan2(z.im, z.re + 1.0) - atan2(z.im, 1.0 - z.re);
    let alt = to_float(floor(sigma * n_invpi));
    if (to_int(alt) % 2 == 0) {
        sigma = alt * pi_n + ((sigma + a_n) - to_float(floor((sigma + a_n) / pi_n)) * pi_n);
    } else {
        sigma = alt * pi_n + ((sigma - a_n) - to_float(floor((sigma - a_n) / pi_n)) * pi_n);
    }
    let temp = cosh(tau) - cos(sigma);
    let inv_t = 1.0 / (if abs(temp) < 1.0e-30 { 1.0e-30 } else { temp });
    complex(sinh(tau) * inv_t, sin(sigma) * inv_t)
}
