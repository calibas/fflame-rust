/// N-th-root-branched wedge — combines JuliaN angular branching (`power`,
/// random branch per iteration) with wedge sectoring (`angle`, `count`).
///
/// # Authors
/// - Apophysis Plugin Pack

// Wedge Julia, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 7.0, min: -20.0, max: 20.0, free: true,
        caption: "Power",
        hint: "Number of Julia branches. Higher = more arms.",
    },
    dist: #{
        value: 0.2, min: -10.0, max: 10.0, free: true,
        caption: "Distance",
        hint: "Radial distance scaling — pushes arms inward or outward.",
    },
    count: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Count",
        hint: "Number of wedge sectors around the center.",
    },
    angle: #{
        value: 0.3, min: -10.0, max: 10.0, free: true,
        caption: "Angle",
        hint: "Wedge sector rotation, in radians.",
    },
};

const INFO = #{ title: "Wedge Julia", name: "rhai_wedge_julia", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let power = p.power;
    let dist = p.dist;
    let count = p.count;
    let angle = p.angle;
    let inv_two_pi = 0.15915494309189535;
    let safe_power = (if power == 0.0 { 1.0e-30 } else { power });
    #{ cf: 1.0 - angle * count * inv_two_pi, r_n: abs(power), cn: dist / safe_power * 0.5 }
}

fn variation(z, p) {
    let power = p.power;
    let count = p.count;
    let angle = p.angle;
    let cf = p.cf;
    let r_n = p.r_n;
    let cn = p.cn;
    let inv_two_pi = 0.15915494309189535;
    let two_pi = 6.28318530717959;
    let r = pow(max(1.0e-30, z.re * z.re + z.im * z.im), cn);
    let t_rnd = to_float(floor(r_n * rand()));
    let safe_power = (if power == 0.0 { 1.0e-30 } else { power });
    let a = (atan2(z.re, z.im) + two_pi * t_rnd) / safe_power;
    let c = to_float(floor((count * a + 3.14159265358979) * inv_two_pi));
    a = a * cf + c * angle;
    let sa = sin(a);
    let ca = cos(sa);
    complex(r * ca, r * sa)
}
