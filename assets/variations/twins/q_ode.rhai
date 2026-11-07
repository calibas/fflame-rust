/// Quadratic ODE warp — a 12-parameter quadratic polynomial mapping. Emits
/// `(q01 + q02·x + q03·x² + q04·xy + q05·y + q06·y², q07 + q08·x + q09·x² +
/// q10·xy + q11·y + q12·y²)`. Implements a general quadratic vector field
/// in 2D — useful for modeling phase-space portraits of 2D ODEs.
///
/// # Authors
/// - DarkBeam

// Q ODE, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    q_ode01: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 01",
        hint: "Constant term on X output.",
    },
    q_ode02: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 02",
        hint: "x linear coefficient on X output.",
    },
    q_ode03: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 03",
        hint: "x² coefficient on X output.",
    },
    q_ode04: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 04",
        hint: "xy coefficient on X output.",
    },
    q_ode05: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 05",
        hint: "y linear coefficient on X output.",
    },
    q_ode06: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 06",
        hint: "y² coefficient on X output.",
    },
    q_ode07: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 07",
        hint: "Constant term on Y output.",
    },
    q_ode08: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 08",
        hint: "x linear coefficient on Y output.",
    },
    q_ode09: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 09",
        hint: "x² coefficient on Y output.",
    },
    q_ode10: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 10",
        hint: "xy coefficient on Y output.",
    },
    q_ode11: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 11",
        hint: "y linear coefficient on Y output.",
    },
    q_ode12: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Q ODE 12",
        hint: "y² coefficient on Y output.",
    },
};

const INFO = #{ title: "Q ODE", name: "rhai_q_ode", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let q01 = p.q_ode01;
    let q02 = p.q_ode02;
    let q03 = p.q_ode03;
    let q04 = p.q_ode04;
    let q05 = p.q_ode05;
    let q06 = p.q_ode06;
    let q07 = p.q_ode07;
    let q08 = p.q_ode08;
    let q09 = p.q_ode09;
    let q10 = p.q_ode10;
    let q11 = p.q_ode11;
    let q12 = p.q_ode12;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let fx_cpp = q01 + w * q02 * z.re + q03 * z.re * z.re + q04 * z.re * z.im + q05 * z.im + q06 * z.im * z.im;
    let fy_cpp = q07 + q08 * z.re + q09 * z.re * z.re + q10 * z.re * z.im + w * q11 * z.im + q12 * z.im * z.im;
    complex(fx_cpp * inv_w, fy_cpp * inv_w)
}
