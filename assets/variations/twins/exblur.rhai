/// 3D Gaussian blur centered at an offset origin — computes spherical
/// coordinates around `(x_origin, -y_origin, z_origin)`, jitters by two
/// random angles plus a Gaussian-approximated scalar (sum of 4 uniforms −
/// 2), and scales by `pow(distance², dist)`. The blur intensity grows with
/// distance from the offset origin.
///
/// # Authors
/// - zephyrtronium
/// - DarkBeam

// Ex Blur, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    dist: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Dist",
        hint: "Power exponent on the squared-distance scaling. Controls how the blur intensity grows with distance from the origin.",
    },
    r: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "R",
        hint: "Scaling factor on the perpendicular jitter.",
    },
    x_origin: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X Origin",
        hint: "X center of the blur (subtracted from input X).",
    },
    y_origin: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Origin",
        hint: "Y center of the blur — note the upstream sign convention: added to input Y rather than subtracted.",
    },
    z_origin: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Origin",
        hint: "Z center of the blur (subtracted from input Z; 3D only).",
    },
};

const INFO = #{ title: "Ex Blur", name: "rhai_exblur", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let dist = p.dist;
    let r_p = p.r;
    let x_origin = p.x_origin;
    let y_origin = p.y_origin;
    let two_pi = 6.28318530717959;
    let ox = z.re - x_origin;
    let oy = z.im + y_origin;
    let n = max(1.0e-30, ox * ox + oy * oy);
    let gauss = rand() + rand() + rand() + rand() - 2.0;
    let rr = pow(n, dist) * gauss;
    let theta = atan2(oy, ox);
    let phi = 1.5707963267948966;
    let cu = cos(theta);
    let su = sin(theta);
    let sv = sin(phi);
    let theta_u = rand() * two_pi;
    let cru = cos(theta_u);
    let sru = sin(theta_u);
    let theta_v = rand() * two_pi;
    let srv = sin(theta_v);
    let rsrv = r_p * srv;
    complex(rr * (sv * cu + rsrv * cru), rr * (sv * su + rsrv * sru))
}

fn variation3(v, p) {
    let dist = p.dist;
    let r_p = p.r;
    let x_origin = p.x_origin;
    let y_origin = p.y_origin;
    let z_origin = p.z_origin;
    let two_pi = 6.28318530717959;
    let ox = v.x - x_origin;
    let oy = v.y + y_origin;
    let oz = v.z - z_origin;
    let n = max(1.0e-30, ox * ox + oy * oy + oz * oz);
    let gauss = rand() + rand() + rand() + rand() - 2.0;
    let rr = pow(n, dist) * gauss;
    let theta = atan2(oy, ox);
    let phi = acos(clamp(oz / sqrt(n), -1.0, 1.0));
    let cu = cos(theta);
    let su = sin(theta);
    let cv = cos(phi);
    let sv = sin(phi);
    let theta_u = rand() * two_pi;
    let cru = cos(theta_u);
    let sru = sin(theta_u);
    let theta_v = rand() * two_pi;
    let crv = cos(theta_v);
    let srv = sin(theta_v);
    let rsrv = r_p * srv;
    vec3(rr * (sv * cu + rsrv * cru), rr * (sv * su + rsrv * sru), rr * (cv + r_p * crv))
}
