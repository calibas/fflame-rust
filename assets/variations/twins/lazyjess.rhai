/// N-gon rotate-and-flip warp — splits behavior based on whether the input
/// lies inside or outside an inscribed regular N-gon. Inside, the point is
/// rotated by `spin` and, if the rotated point is still inside, kept;
/// otherwise it's flipped into the `corner`-indexed corner sector. Outside
/// the N-gon, the point gets a radial nudge by `space`. The n = 2 special
/// case uses an axis-aligned line-segment test instead of the polygon test.
///
/// # Authors
/// - FarDareisMai

// Lazy Jess, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    n: #{
        value: 4, min: 2, max: 50,
        caption: "N",
        hint: "Polygon vertex count. The n = 2 special case uses a line-segment test instead of a true polygon.",
    },
    spin: #{
        value: 3.1415927, min: -10.0, max: 10.0, free: true,
        caption: "Spin",
        hint: "Rotation applied to points inside the N-gon, in radians.",
    },
    space: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Space",
        hint: "Radial nudge applied to points outside the N-gon (added to the radius, scaled inversely by `modulus`).",
    },
    corner: #{
        value: 1, min: 1, max: 50,
        caption: "Corner",
        hint: "Which of the N corners to flip to when the post-rotation inside-test fails (1-based).",
    },
};

const INFO = #{ title: "Lazy Jess", name: "rhai_lazyjess", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let n = max(2.0, to_float(p.n));
    let corner = to_float(p.corner);
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let vertex = pi * (n - 2.0) / (2.0 * n);
    let pie_slice = two_pi / n;
    #{ vertex: vertex, sin_vertex: sin(vertex), pie_slice: pie_slice, corner_rot: (corner - 1.0) * pie_slice }
}

fn variation(z, p, ctx) {
    let n = max(2.0, to_float(p.n));
    let spin = p.spin;
    let space = p.space;
    let vertex = p.vertex;
    let sin_vertex = p.sin_vertex;
    let pie_slice = p.pie_slice;
    let corner_rot = p.corner_rot;
    let half_slice = pie_slice * 0.5;
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let sqrt2 = 1.4142135623730951;
    let w = ctx.weight;
    let fx = 0.0;
    let fy = 0.0;
    let modulus = sqrt(z.re * z.re + z.im * z.im);
    if (n == 2.0) {
        if (abs(z.re) < w) {
            let theta = atan2(z.im, z.re) + spin;
            let x = w * modulus * cos(theta);
            let y = w * modulus * sin(theta);
            if (abs(x) < w) {
                fx = x;
                fy = y;
            } else {
                theta = atan2(z.im, z.re) - spin + corner_rot;
                fx = w * modulus * cos(theta);
                fy = -w * modulus * sin(theta);
            }
        } else {
            let m2 = 1.0 + space / max(1.0e-30, modulus);
            fx = w * m2 * z.re;
            fy = w * m2 * z.im;
        }
    } else {
        let theta_in = atan2(z.im, z.re) + two_pi;
        let theta_diff_in = theta_in + half_slice - to_float(floor((theta_in + half_slice) / pie_slice)) * pie_slice;
        let denom_in = sin(pi - theta_diff_in - vertex);
        let r_in = w * sqrt2 * sin_vertex / (if abs(denom_in) < 1.0e-30 { 1.0e-30 } else { denom_in });
        if (modulus < r_in) {
            let theta_rot = atan2(z.im, z.re) + spin + two_pi;
            let x = w * modulus * cos(theta_rot);
            let y = w * modulus * sin(theta_rot);
            let theta_diff_r = theta_rot + half_slice - to_float(floor((theta_rot + half_slice) / pie_slice)) * pie_slice;
            let denom_r = sin(pi - theta_diff_r - vertex);
            let r_r = w * sqrt2 * sin_vertex / (if abs(denom_r) < 1.0e-30 { 1.0e-30 } else { denom_r });
            let m_rot = sqrt(x * x + y * y);
            if (m_rot < r_r) {
                fx = x;
                fy = y;
            } else {
                let theta_flip = atan2(z.im, z.re) - spin + corner_rot + two_pi;
                fx = w * modulus * cos(theta_flip);
                fy = -w * modulus * sin(theta_flip);
            }
        } else {
            let m2 = 1.0 + space / max(1.0e-30, modulus);
            fx = w * m2 * z.re;
            fy = w * m2 * z.im;
        }
    }
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    complex(fx * inv_w, fy * inv_w)
}
