// EPush, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    push: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Push",
        hint: "Additive offset on μ.",
    },
    dist: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Dist",
        hint: "Multiplicative scale on μ.",
    },
    rotate: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Rotate",
        hint: "Angular shift on ν, in radians.",
    },
};

const INFO = #{ title: "EPush", name: "rhai_epush" };

fn variation(z, p) {
    let push = p.push;
    let dist = p.dist;
    let rotate = p.rotate;
    let tmp = z.im * z.im + z.re * z.re + 1.0;
    let tmp2 = 2.0 * z.re;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    let mu = acosh(xmax);
    let t = clamp(z.re / xmax, -1.0, 1.0);
    let nu = acos(t);
    if (z.im < 0.0) {
        nu = -nu;
    }
    nu = nu + rotate;
    mu = mu * dist + push;
    complex(cosh(mu) * cos(nu), sinh(mu) * sin(nu))
}
