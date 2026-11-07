/// 4D quaternion Julia-N map q' = qⁿ + c.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Quaternion Julia (N), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    cx: #{
        value: -0.2, min: -2.0, max: 2.0,
        caption: "Constant X",
        hint: "Vector-i component of the Julia constant quaternion c.",
    },
    cy: #{
        value: 0.4, min: -2.0, max: 2.0,
        caption: "Constant Y",
        hint: "Vector-j component of c.",
    },
    cz: #{
        value: 0.0, min: -2.0, max: 2.0,
        caption: "Constant Z",
        hint: "Vector-k component of c. 3D only.",
    },
    cw: #{
        value: 0.0, min: -2.0, max: 2.0,
        caption: "Constant W",
        hint: "Scalar component of c (drives the 4th dimension). 3D only.",
    },
    power: #{
        value: 2, min: -16, max: 16,
        caption: "Power (N)",
        hint: "The Julia power n: q^n + c forward, or the n-branch root (q-c)^(1/n) inverse. n=2 quadratic (rabbit), n=3 cubic, etc. |n| sets the number of inverse root branches.",
    },
    dist: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "Distance",
        hint: "Radial exponent multiplier for the inverse root: magnitude = |q-c|^(dist/n). 1.0 = the true n-th root (real Julia set); other values distort radially (the flam3 `julian` effect). No effect on the forward map.",
    },
    projection: #{
        value: 0, min: 0, max: 2, free: true,
        caption: "Projection",
        hint: "How the 4D result maps to the plotted 3D point (3D only). 0 = Vector (drop w), 1 = Depth (surface w as z; z and w swap), 2 = Perspective (divide xyz by 1-w). The return both plots AND feeds forward, so each mode is effectively a different attractor.",
    },
    inverse: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Inverse (Julia set)",
        hint: "0 = forward q^n + c (an IFS attractor, NOT a Julia set under the chaos game). 1 = inverse (q - c)^(1/n) with a random branch: the Inverse Iteration Method, which converges to the actual Julia set. Use inverse=1 with an identity affine and weight 1.0.",
    },
    w_color: #{
        value: 0.0, min: 0.0, max: 8.0,
        caption: "Color by W",
        hint: "0 = off. >0 = write a palette index from the 4th coordinate (3D: fract(w * scale); 2D: fract(|z| * scale)), revealing the hidden dimension as COLOR without altering the attractor. Needs the transform's direct_color > 0.",
    },
    w_bright: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Brightness by W",
        hint: "0 = off. Scales the sample's palette color by (1 + w_bright*w): positive = high-w structure glows brighter (feeds the Glow post-effect nicely), negative = it dims. Hue-preserving; 3D only. Needs the transform's direct_color > 0.",
    },
    w_sat: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Saturation by W",
        hint: "0 = off. Shifts the sample's color saturation by (1 + w_sat*w) around its luminance: negative w_sat washes high-w structure toward gray, >1 total over-saturates. Hue-preserving; 3D only. Needs the transform's direct_color > 0.",
    },
};

const INFO = #{ title: "Quaternion Julia (N)", name: "rhai_quaternion_julia", category: "advanced_2d" };

fn srgb_to_linear(c) {
    let t1 = max(c, vec3(0.0, 0.0, 0.0));
    let t2 = vec3(2.2, 2.2, 2.2);
    vec3(pow(t1.x, t2.x), pow(t1.y, t2.y), pow(t1.z, t2.z))
}

fn qjulia_qpow(q, n) {
    let mag = sqrt(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w) + 1.0e-12;
    let rad = pow(mag, n);
    let ang = acos(clamp(q.w / mag, -1.0, 1.0)) * n;
    let vlen = length(vec3(q.x, q.y, q.z));
    let nhat = (if vlen > 1.0e-9 { vec3(q.x, q.y, q.z) / vlen } else { vec3(1.0, 0.0, 0.0) });
    let t1 = rad * sin(ang) * nhat;
    vec4(t1.x, t1.y, t1.z, rad * cos(ang))
}

fn qjulia_qroot(q, n, branch, dist) {
    let mag = sqrt(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w) + 1.0e-12;
    let rad = pow(mag, dist / n);
    let ang = (acos(clamp(q.w / mag, -1.0, 1.0)) + branch * 6.28318530718) / n;
    let vlen = length(vec3(q.x, q.y, q.z));
    let nhat = (if vlen > 1.0e-9 { vec3(q.x, q.y, q.z) / vlen } else { vec3(1.0, 0.0, 0.0) });
    let t1 = rad * sin(ang) * nhat;
    vec4(t1.x, t1.y, t1.z, rad * cos(ang))
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let rgb = ctx.rgb;
    let cx = p.cx;
    let cy = p.cy;
    let n = to_float(p.power);
    let dist = p.dist;
    let safe_n = (if abs(n) < 1.0e-6 { 1.0e-6 } else { n });
    let out = complex(0.0, 0.0);
    if (to_float(p.inverse) > 0.5) {
        let d = z - complex(cx, cy);
        let branch = to_float(floor(rand() * max(1.0, abs(n))));
        let theta = (atan2(d.im, d.re) + branch * 6.28318530718) / safe_n;
        out = pow(d.abs(), dist / safe_n) * complex(cos(theta), sin(theta));
    } else {
        let theta = atan2(z.im, z.re) * n;
        out = pow(z.abs(), n) * complex(cos(theta), sin(theta)) + complex(cx, cy);
    }
    let wcol = p.w_color;
    if (wcol > 1.0e-6) {
        color = fract(out.abs() * wcol);
    }
    #{ z: out, color: color, rgb: rgb }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let rgb_v = ctx.rgb;
    let w_out = 0.0;
    let result = vec3(0.0, 0.0, 0.0);
    let q = vec4(v.x, v.y, v.z, ctx.w);
    let c = vec4(p.cx, p.cy, p.cz, p.cw);
    let n = to_float(p.power);
    let dist = p.dist;
    let safe_n = (if abs(n) < 1.0e-6 { 1.0e-6 } else { n });
    let r = vec4(0.0, 0.0, 0.0, 0.0);
    if (to_float(p.inverse) > 0.5) {
        let branch = to_float(floor(rand() * max(1.0, abs(n))));
        r = qjulia_qroot(q - c, safe_n, branch, dist);
    } else {
        r = qjulia_qpow(q, n) + c;
    }
    let wcol = p.w_color;
    if (wcol > 1.0e-6) {
        color = fract(r.w * wcol);
    }
    let wb = p.w_bright;
    let ws = p.w_sat;
    if (abs(wb) > 1.0e-6 || abs(ws) > 1.0e-6) {
        let srgb = palette_srgb(clamp(color, 0.0, 1.0));
        let col = srgb_to_linear(srgb) * clamp(1.0 + wb * r.w, 0.0, 4.0);
        let luma = dot(col, vec3(0.2126, 0.7152, 0.0722));
        col = mix(vec3(luma, luma, luma), col, clamp(1.0 + ws * r.w, 0.0, 2.0));
        rgb_v = col;
    }
    let mode = u32(to_float(p.projection) + 0.5);
    let t2 = (mode);
    if t2 == u32(1) {
        w_out = r.z;
        result = vec3(r.x, r.y, r.w);
    } else if t2 == u32(2) {
        w_out = r.w;
        let denom = 1.0 - r.w;
        let safe = (if abs(denom) < 1.0e-3 { 1.0e-3 } else { denom });
        result = vec3(r.x, r.y, r.z) / safe;
    } else {
        w_out = r.w;
        result = vec3(r.x, r.y, r.z);
    }
    #{ z: result, color: color, rgb: rgb_v, w: w_out }
}
