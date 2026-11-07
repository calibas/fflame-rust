/// General 4D rotation of the point by q' = â·q·b̂.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Quaternion Rotation, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    ax: #{
        value: 0.3, min: -1.0, max: 1.0,
        caption: "Axis X",
        hint: "Vector-i component of the rotation quaternion a (normalized at runtime).",
    },
    ay: #{
        value: 0.0, min: -1.0, max: 1.0,
        caption: "Axis Y",
        hint: "Vector-j component of a.",
    },
    az: #{
        value: 0.0, min: -1.0, max: 1.0,
        caption: "Axis Z",
        hint: "Vector-k component of a.",
    },
    aw: #{
        value: 0.95, min: -1.0, max: 1.0,
        caption: "Scalar W",
        hint: "Scalar component of a. With the vector part, sets the rotation angle (aw = cos(θ/2)).",
    },
    projection: #{
        value: 0, min: 0, max: 2, free: true,
        caption: "Projection",
        hint: "How the rotated 4D point maps to the plotted 3D point (3D only). 0 = Vector (drop w), 1 = Depth (surface w as z), 2 = Perspective (divide xyz by 1-w).",
    },
    w_color: #{
        value: 0.0, min: 0.0, max: 8.0,
        caption: "Color by W",
        hint: "0 = off. >0 = write a palette index from the 4th coordinate (3D: fract(w * scale); 2D: fract(|z| * scale)), revealing it as COLOR. Needs the transform's direct_color > 0.",
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
    bx: #{
        value: 0.0, min: -1.0, max: 1.0,
        caption: "Right Axis X",
        hint: "Vector-i component of the RIGHT quaternion b in q' = a*q*b (normalized at runtime). Default b = (0,0,0,1) is the identity — pure left rotation. Set b = a-conjugate (negate bx/by/bz vs ax/ay/az, bw = aw) for an ordinary 3D rotation that leaves w untouched. 3D only.",
    },
    by: #{
        value: 0.0, min: -1.0, max: 1.0,
        caption: "Right Axis Y",
        hint: "Vector-j component of b. 3D only.",
    },
    bz: #{
        value: 0.0, min: -1.0, max: 1.0,
        caption: "Right Axis Z",
        hint: "Vector-k component of b. 3D only.",
    },
    bw: #{
        value: 1.0, min: -1.0, max: 1.0,
        caption: "Right Scalar W",
        hint: "Scalar component of b (bw = cos of the right half-angle). Every 4D rotation is some pair (a, b) — combining left and right spans the whole rotation group SO(4). 3D only.",
    },
};

const INFO = #{ title: "Quaternion Rotation", name: "rhai_quaternion_rotation", category: "advanced_2d" };

fn srgb_to_linear(c) {
    let t1 = max(c, vec3(0.0, 0.0, 0.0));
    let t2 = vec3(2.2, 2.2, 2.2);
    vec3(pow(t1.x, t2.x), pow(t1.y, t2.y), pow(t1.z, t2.z))
}

fn qrot_qmul(a, b) {
    vec4(a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y, a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x, a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w, a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z)
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let rgb = ctx.rgb;
    let ax = p.ax;
    let aw = p.aw;
    let ang = 2.0 * atan2(ax, aw);
    let cs = cos(ang);
    let sn = sin(ang);
    let out = complex(z.re * cs - z.im * sn, z.re * sn + z.im * cs);
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
    let a_raw = vec4(p.ax, p.ay, p.az, p.aw);
    let a = normalize(a_raw + vec4(0.0, 0.0, 0.0, 1.0e-9));
    let b_raw = vec4(p.bx, p.by, p.bz, p.bw);
    let b = normalize(b_raw + vec4(0.0, 0.0, 0.0, 1.0e-9));
    let r = qrot_qmul(qrot_qmul(a, vec4(v.x, v.y, v.z, ctx.w)), b);
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
