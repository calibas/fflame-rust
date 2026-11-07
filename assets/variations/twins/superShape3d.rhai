/// Two-axis Gielis super-shape projected onto a 3D surface — each iteration
/// picks two random angles `ρ` and `φ` (uniform in `[0, rho]` and `[-phi,
/// phi]`) and evaluates the Gielis super-shape formula `r(θ) =
/// (|cos(m·θ/4)/a|^n2 + |sin(m·θ/4)/b|^n3)^(-1/n1)` independently for each
/// angle (subscript 1 for the ρ axis, 2 for the φ axis). The two radii `r1`
/// and `r2` then combine into either a spherical product (`r1·r2·cos·cos`,
/// etc.) or a toroidal sum (`cos·(r1 + r2·cos)`, etc.) based on
/// `toroidmap`. Optional `spiral` term advances the ρ-axis radius linearly
/// with the ρ angle to produce a spiraling output.
///
/// # Authors
/// - David Young

// SuperShape 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    rho: #{
        value: 9.9, min: -100.0, max: 100.0, free: true,
        caption: "Rho",
        hint: "Angular range (radians) for uniform random sampling of the ρ axis. Larger values sweep through more of the shape per iteration; default 9.9 (~3π).",
    },
    phi: #{
        value: 2.5, min: -100.0, max: 100.0, free: true,
        caption: "Phi",
        hint: "Angular range (radians) for the φ axis (sampled in `[-phi, phi]`). Default 2.5 (~0.8π).",
    },
    m1: #{
        value: 6.0, min: -50.0, max: 50.0, free: true,
        caption: "M1",
        hint: "Petal count for the ρ-axis Gielis shape — controls rotational symmetry. Higher values produce more spokes.",
    },
    m2: #{
        value: 3.0, min: -50.0, max: 50.0, free: true,
        caption: "M2",
        hint: "Petal count for the φ-axis Gielis shape.",
    },
    a1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A1",
        hint: "Cosine-term width scaling for the ρ-axis shape. Smaller values pinch the cos-side petals narrower.",
    },
    a2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A2",
        hint: "Cosine-term width scaling for the φ-axis shape.",
    },
    b1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B1",
        hint: "Sine-term width scaling for the ρ-axis shape.",
    },
    b2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B2",
        hint: "Sine-term width scaling for the φ-axis shape.",
    },
    n1_1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "N1.1",
        hint: "Outer exponent for the ρ-axis shape: final radius is `pr^(-1/n1_1)`. Controls overall radial blow-up — small values produce sharp spikes, larger values rounder shapes.",
    },
    n1_2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "N1.2",
        hint: "Outer exponent for the φ-axis shape.",
    },
    n2_1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "N2.1",
        hint: "Cosine-term exponent for the ρ-axis shape — controls petal sharpness on the cos side.",
    },
    n2_2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "N2.2",
        hint: "Cosine-term exponent for the φ-axis shape.",
    },
    n3_1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "N3.1",
        hint: "Sine-term exponent for the ρ-axis shape — controls petal sharpness on the sin side.",
    },
    n3_2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "N3.2",
        hint: "Sine-term exponent for the φ-axis shape.",
    },
    spiral: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Spiral",
        hint: "Spiral phase advance: `r1` is incremented by `spiral · ρ`, producing a linear radial growth with angle. 0 disables.",
    },
    toroidmap: #{
        value: false,
        caption: "Toroidal",
        hint: "When on, toroidal projection (`cos·(r1 + r2·cos)`, etc.). When off, spherical product (`r1·r2·cos·cos`, etc.). Both modes use `r2·sin(φ)` for Z.",
    },
};

const INFO = #{ title: "SuperShape 3D", name: "rhai_supershape3d", category: "full_3d", phase: "any" };

fn prepare(p) {
    let two_over_pi = 0.6366197723675814;
    let rho = p.rho;
    let phi = p.phi;
    let m1 = p.m1;
    let m2 = p.m2;
    let a1 = p.a1;
    let a2 = p.a2;
    let b1 = p.b1;
    let b2 = p.b2;
    let n1_1 = p.n1_1;
    let n1_2 = p.n1_2;
    let n2_1 = p.n2_1;
    let n2_2 = p.n2_2;
    let n3_1 = p.n3_1;
    let n3_2 = p.n3_2;
    #{ n1n_1: -1.0 / (if abs(n1_1) < 1.0e-30 { 1.0e-30 } else { n1_1 }), n1n_2: -1.0 / (if abs(n1_2) < 1.0e-30 { 1.0e-30 } else { n1_2 }), m4_1: m1 * 0.25, m4_2: m2 * 0.25, an2_1: pow(abs(1.0 / (if abs(a1) < 1.0e-30 { 1.0e-30 } else { a1 })), n2_1), an2_2: pow(abs(1.0 / (if abs(a2) < 1.0e-30 { 1.0e-30 } else { a2 })), n2_2), bn3_1: pow(abs(1.0 / (if abs(b1) < 1.0e-30 { 1.0e-30 } else { b1 })), n3_1), bn3_2: pow(abs(1.0 / (if abs(b2) < 1.0e-30 { 1.0e-30 } else { b2 })), n3_2), rho_pi: rho * two_over_pi, phi_pi: phi * two_over_pi }
}

fn variation(z, p) {
    let n2_1 = p.n2_1;
    let n2_2 = p.n2_2;
    let n3_1 = p.n3_1;
    let n3_2 = p.n3_2;
    let spiral = p.spiral;
    let toroidmap = to_int(to_float(p.toroidmap));
    let n1n_1 = p.n1n_1;
    let n1n_2 = p.n1n_2;
    let m4_1 = p.m4_1;
    let m4_2 = p.m4_2;
    let an2_1 = p.an2_1;
    let an2_2 = p.an2_2;
    let bn3_1 = p.bn3_1;
    let bn3_2 = p.bn3_2;
    let rho_pi = p.rho_pi;
    let phi_pi = p.phi_pi;
    let rho1 = rand() * rho_pi;
    let phi1 = rand() * phi_pi;
    let sign_pick = rand();
    if (sign_pick < 0.5) {
        phi1 = -phi1;
    }
    let sinr = sin(rho1);
    let cosr = cos(rho1);
    let sinp = sin(phi1);
    let cosp = cos(phi1);
    let a_r = m4_1 * rho1;
    let msinr = sin(a_r);
    let mcosr = cos(a_r);
    let a_p = m4_2 * phi1;
    let msinp = sin(a_p);
    let mcosp = cos(a_p);
    let pr1 = an2_1 * pow(max(1.0e-30, abs(mcosr)), n2_1) + bn3_1 * pow(max(1.0e-30, abs(msinr)), n3_1);
    let pr2 = an2_2 * pow(max(1.0e-30, abs(mcosp)), n2_2) + bn3_2 * pow(max(1.0e-30, abs(msinp)), n3_2);
    let r1 = pow(max(1.0e-30, pr1), n1n_1) + spiral * rho1;
    let r2 = pow(max(1.0e-30, pr2), n1n_2);
    if (toroidmap == 1) {
        return complex(cosr * (r1 + r2 * cosp), sinr * (r1 + r2 * cosp));
    }
    complex(r1 * cosr * r2 * cosp, r1 * sinr * r2 * cosp)
}

fn variation3(v, p) {
    let n2_1 = p.n2_1;
    let n2_2 = p.n2_2;
    let n3_1 = p.n3_1;
    let n3_2 = p.n3_2;
    let spiral = p.spiral;
    let toroidmap = to_int(to_float(p.toroidmap));
    let n1n_1 = p.n1n_1;
    let n1n_2 = p.n1n_2;
    let m4_1 = p.m4_1;
    let m4_2 = p.m4_2;
    let an2_1 = p.an2_1;
    let an2_2 = p.an2_2;
    let bn3_1 = p.bn3_1;
    let bn3_2 = p.bn3_2;
    let rho_pi = p.rho_pi;
    let phi_pi = p.phi_pi;
    let rho1 = rand() * rho_pi;
    let phi1 = rand() * phi_pi;
    let sign_pick = rand();
    if (sign_pick < 0.5) {
        phi1 = -phi1;
    }
    let sinr = sin(rho1);
    let cosr = cos(rho1);
    let sinp = sin(phi1);
    let cosp = cos(phi1);
    let a_r = m4_1 * rho1;
    let msinr = sin(a_r);
    let mcosr = cos(a_r);
    let a_p = m4_2 * phi1;
    let msinp = sin(a_p);
    let mcosp = cos(a_p);
    let pr1 = an2_1 * pow(max(1.0e-30, abs(mcosr)), n2_1) + bn3_1 * pow(max(1.0e-30, abs(msinr)), n3_1);
    let pr2 = an2_2 * pow(max(1.0e-30, abs(mcosp)), n2_2) + bn3_2 * pow(max(1.0e-30, abs(msinp)), n3_2);
    let r1 = pow(max(1.0e-30, pr1), n1n_1) + spiral * rho1;
    let r2 = pow(max(1.0e-30, pr2), n1n_2);
    if (toroidmap == 1) {
        return vec3(cosr * (r1 + r2 * cosp), sinr * (r1 + r2 * cosp), r2 * sinp);
    }
    vec3(r1 * cosr * r2 * cosp, r1 * sinr * r2 * cosp, r2 * sinp)
}
