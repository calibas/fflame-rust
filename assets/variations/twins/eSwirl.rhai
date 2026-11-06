/// Elliptic swirl — adds `μ·out + in/μ` to the elliptic ν coordinate.
/// Elliptic analogue of `bswirl`.
///
/// # Authors
/// - Michael Faber

// ESwirl, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    in_p: #{
        value: 1.2, min: -10.0, max: 10.0, free: true,
        caption: "In",
        hint: "Reciprocal-μ swirl coefficient — adds `in/μ` to ν.",
    },
    out_p: #{
        value: 0.2, min: -10.0, max: 10.0, free: true,
        caption: "Out",
        hint: "Linear-μ swirl coefficient — adds `μ·out` to ν.",
    },
};

const INFO = #{ title: "ESwirl", name: "rhai_eswirl", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let in_p = p.in_p;
    let out_p = p.out_p;
    let tmp = z.im * z.im + z.re * z.re + 1.0;
    let tmp2 = 2.0 * z.re;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    let mu = acosh(xmax);
    let safe_mu = (if abs(mu) < 1.0e-30 { 1.0e-30 } else { mu });
    let t = clamp(z.re / xmax, -1.0, 1.0);
    let nu = acos(t);
    if (z.im < 0.0) {
        nu = -nu;
    }
    nu = nu + mu * out_p + in_p / safe_mu;
    complex(cosh(mu) * cos(nu), sinh(mu) * sin(nu))
}
