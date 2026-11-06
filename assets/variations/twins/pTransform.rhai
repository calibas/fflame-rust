// P Transform, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    rotate: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Rotate",
        hint: "Angular rotation added to θ, in radians.",
    },
    power: #{
        value: 1, min: -50, max: 50,
        caption: "Power",
        hint: "Radial divisor. When `use_log` is off, this is the reciprocal scaling factor on the input radius.",
    },
    move: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Move",
        hint: "Additive offset on ρ.",
    },
    split: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Split",
        hint: "Asymmetric ρ offset, added when x ≥ 0 and subtracted when x < 0.",
    },
    use_log: #{
        value: true,
        caption: "Use Log",
        hint: "When on, apply a log/exp transform around the polar radius ρ (true log-polar). When off, use linear radius.",
    },
};

const INFO = #{ title: "P Transform", name: "rhai_ptransform" };

fn variation(z, p) {
    let rotate = p.rotate;
    let power = to_float(p.power);
    let move_p = p.move;
    let split = p.split;
    let use_log = to_float(p.use_log);
    let r0 = sqrt(z.re * z.re + z.im * z.im) + 1.0e-6;
    let safe_power = (if power == 0.0 { 1.0e-30 } else { power });
    let rho = (if use_log != 0.0 { ln(r0) } else { r0 }) / safe_power + move_p;
    let theta = atan2(z.re, z.im) + rotate;
    if (z.re >= 0.0) {
        rho = rho + split;
    } else {
        rho = rho - split;
    }
    if (use_log != 0.0) {
        rho = exp(rho);
    }
    complex(rho * cos(theta), rho * sin(theta))
}
