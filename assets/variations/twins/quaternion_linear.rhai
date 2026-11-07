/// A full 4D affine q' = M·q + t on the point q = (x,y,z,w).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Quaternion Linear, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    m00: #{
        value: 1.0, min: -2.0, max: 2.0, free: true,
        caption: "M00 (x'<-x)",
        hint: "Row 0 of the 4x4 matrix M — the coefficients producing x'. m03 is the w->x coupling the 3D affine can't express.",
    },
    m01: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M01 (x'<-y)",
        hint: "x' coefficient on y.",
    },
    m02: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M02 (x'<-z)",
        hint: "x' coefficient on z.",
    },
    m03: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M03 (x'<-w)",
        hint: "x' coefficient on w (4th-column coupling; needs 3D).",
    },
    m10: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M10 (y'<-x)",
        hint: "y' coefficient on x.",
    },
    m11: #{
        value: 1.0, min: -2.0, max: 2.0, free: true,
        caption: "M11 (y'<-y)",
        hint: "y' coefficient on y.",
    },
    m12: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M12 (y'<-z)",
        hint: "y' coefficient on z.",
    },
    m13: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M13 (y'<-w)",
        hint: "y' coefficient on w.",
    },
    m20: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M20 (z'<-x)",
        hint: "z' coefficient on x. 3D only.",
    },
    m21: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M21 (z'<-y)",
        hint: "z' coefficient on y. 3D only.",
    },
    m22: #{
        value: 1.0, min: -2.0, max: 2.0, free: true,
        caption: "M22 (z'<-z)",
        hint: "z' coefficient on z. 3D only.",
    },
    m23: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M23 (z'<-w)",
        hint: "z' coefficient on w. 3D only.",
    },
    m30: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M30 (w'<-x)",
        hint: "Row 3 — the w-coupling the built-in affine lacks. w' coefficient on x. 3D only.",
    },
    m31: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M31 (w'<-y)",
        hint: "w' coefficient on y. 3D only.",
    },
    m32: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "M32 (w'<-z)",
        hint: "w' coefficient on z. 3D only.",
    },
    m33: #{
        value: 1.0, min: -2.0, max: 2.0, free: true,
        caption: "M33 (w'<-w)",
        hint: "w' coefficient on w. 3D only.",
    },
    tx: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Translate X",
        hint: "4D offset added to x'.",
    },
    ty: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Translate Y",
        hint: "4D offset added to y'.",
    },
    tz: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Translate Z",
        hint: "4D offset added to z'. 3D only.",
    },
    tw: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Translate W",
        hint: "4D offset added to w'. 3D only.",
    },
    projection: #{
        value: 0, min: 0, max: 2, free: true,
        caption: "Projection",
        hint: "How the 4D result maps to the plotted 3D point (3D only). 0 = Vector (drop w), 1 = Depth (surface w as z), 2 = Perspective (divide xyz by 1-w).",
    },
    w_color: #{
        value: 0.0, min: 0.0, max: 8.0,
        caption: "Color by W",
        hint: "0 = off. >0 = write a palette index from the 4th coordinate. Needs the transform's direct_color > 0.",
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

const INFO = #{ title: "Quaternion Linear", name: "rhai_quaternion_linear", category: "advanced_2d" };

fn srgb_to_linear(c) {
    let t1 = max(c, vec3(0.0, 0.0, 0.0));
    let t2 = vec3(2.2, 2.2, 2.2);
    vec3(pow(t1.x, t2.x), pow(t1.y, t2.y), pow(t1.z, t2.z))
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let rgb = ctx.rgb;
    let m00 = p.m00;
    let m01 = p.m01;
    let m10 = p.m10;
    let m11 = p.m11;
    let tx = p.tx;
    let ty = p.ty;
    let out = complex(m00 * z.re + m01 * z.im + tx, m10 * z.re + m11 * z.im + ty);
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
    let row0 = vec4(p.m00, p.m01, p.m02, p.m03);
    let row1 = vec4(p.m10, p.m11, p.m12, p.m13);
    let row2 = vec4(p.m20, p.m21, p.m22, p.m23);
    let row3 = vec4(p.m30, p.m31, p.m32, p.m33);
    let t = vec4(p.tx, p.ty, p.tz, p.tw);
    let r = vec4((row0.x * q.x + row0.y * q.y + row0.z * q.z + row0.w * q.w), (row1.x * q.x + row1.y * q.y + row1.z * q.z + row1.w * q.w), (row2.x * q.x + row2.y * q.y + row2.z * q.z + row2.w * q.w), (row3.x * q.x + row3.y * q.y + row3.z * q.z + row3.w * q.w)) + t;
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
