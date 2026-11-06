// EMod, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Half-width of the μ band to mod-wrap.",
    },
    distance: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Distance",
        hint: "Additional μ offset, in units of `radius`.",
    },
};

const INFO = #{ title: "EMod", name: "rhai_emod" };

fn variation(z, p) {
    let radius = p.radius;
    let distance = p.distance;
    let tmp = z.im * z.im + z.re * z.re + 1.0;
    let tmp2 = 2.0 * z.re;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    let mu = acosh(xmax);
    let t = clamp(z.re / xmax, -1.0, 1.0);
    let nu = acos(t);
    if (z.im < 0.0) {
        nu = -nu;
    }
    if (mu < radius && -mu < radius) {
        let two_r = 2.0 * radius;
        let safe_two_r = (if abs(two_r) < 1.0e-30 { 1.0e-30 } else { two_r });
        if (nu > 0.0) {
            let v = mu + radius + distance * radius;
            mu = (v - to_float(floor(v / safe_two_r)) * safe_two_r) - radius;
        } else {
            let v = mu - radius - distance * radius;
            mu = (v - to_float(floor(v / safe_two_r)) * safe_two_r) + radius;
        }
    }
    complex(cosh(mu) * cos(nu), sinh(mu) * sin(nu))
}
