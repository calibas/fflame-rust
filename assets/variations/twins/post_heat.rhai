/// Post-phase 3D heat-distortion — converts the accumulated point to
/// spherical coordinates `(r, θ, φ)`, applies sin-based perturbations to
/// each (period/phase/amplitude per axis), then re-projects to cartesian.
/// Inspired by heat-distortion atmospheric effects: each spherical
/// coordinate gets `coord += amp · sin(2π·coord/period + phase/period)`.
///
/// # Authors
/// - zephyrtronium

// Post Heat, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    theta_period: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Theta Period",
        hint: "Period of the θ (azimuthal angle) sine perturbation. 0 disables theta perturbation.",
    },
    theta_phase: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Theta Phase",
        hint: "Phase offset on the θ sine.",
    },
    theta_amp: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Theta Amp",
        hint: "Amplitude of the θ sine perturbation.",
    },
    phi_period: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Phi Period",
        hint: "Period of the φ (polar angle) sine perturbation. 0 disables phi perturbation.",
    },
    phi_phase: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Phi Phase",
        hint: "Phase offset on the φ sine.",
    },
    phi_amp: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Phi Amp",
        hint: "Amplitude of the φ sine perturbation.",
    },
    r_period: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "R Period",
        hint: "Period of the radial sine perturbation. 0 disables r perturbation.",
    },
    r_phase: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "R Phase",
        hint: "Phase offset on the r sine.",
    },
    r_amp: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "R Amp",
        hint: "Amplitude of the radial sine perturbation.",
    },
};

const INFO = #{ title: "Post Heat", name: "rhai_post_heat", category: "full_3d", phase: "post" };

fn variation(z, p, ctx) {
    let theta_period = p.theta_period;
    let theta_phase = p.theta_phase;
    let theta_amp = p.theta_amp;
    let r_period = p.r_period;
    let r_phase = p.r_phase;
    let r_amp = p.r_amp;
    let two_pi = 6.28318530717959;
    let w = ctx.weight;
    let tx = (if abs(theta_period) > 1.0e-30 { 1.0 / theta_period } else { 0.0 });
    let rx = (if abs(r_period) > 1.0e-30 { 1.0 / r_period } else { 0.0 });
    let at = w * theta_amp;
    let bt = two_pi * tx;
    let ct = theta_phase * tx;
    let ar = w * r_amp;
    let br = two_pi * rx;
    let cr = r_phase * rx;
    let r = sqrt(z.re * z.re + z.im * z.im);
    let atant = atan2(z.im, z.re);
    r = r + ar * sin(br * r + cr);
    let sint_arg = at * sin(bt * r + ct) + atant;
    complex(r * cos(sint_arg), r * sin(sint_arg))
}

fn variation3(v, p, ctx) {
    let theta_period = p.theta_period;
    let theta_phase = p.theta_phase;
    let theta_amp = p.theta_amp;
    let phi_period = p.phi_period;
    let phi_phase = p.phi_phase;
    let phi_amp = p.phi_amp;
    let r_period = p.r_period;
    let r_phase = p.r_phase;
    let r_amp = p.r_amp;
    let two_pi = 6.28318530717959;
    let w = ctx.weight;
    let tx = (if abs(theta_period) > 1.0e-30 { 1.0 / theta_period } else { 0.0 });
    let px = (if abs(phi_period) > 1.0e-30 { 1.0 / phi_period } else { 0.0 });
    let rx = (if abs(r_period) > 1.0e-30 { 1.0 / r_period } else { 0.0 });
    let at = w * theta_amp;
    let bt = two_pi * tx;
    let ct = theta_phase * tx;
    let ap = w * phi_amp;
    let bp = two_pi * px;
    let cp = phi_phase * px;
    let ar = w * r_amp;
    let br = two_pi * rx;
    let cr = r_phase * rx;
    let r = sqrt(v.x * v.x + v.y * v.y + v.z * v.z);
    let atant = atan2(v.y, v.x);
    r = r + ar * sin(br * r + cr);
    let sint_arg = at * sin(bt * r + ct) + atant;
    let cost = cos(sint_arg);
    let sint = sin(sint_arg);
    let safe_r = (if abs(r) > 1.0e-30 { v.z / r } else { 0.0 });
    let acosp = clamp(safe_r, -1.0, 1.0);
    let sinp_arg = ap * sin(bp * r + cp) + acos(acosp);
    let cosp = cos(sinp_arg);
    let sinp = sin(sinp_arg);
    vec3(r * cost * sinp, r * sint * sinp, r * cosp)
}
