// Matrix 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xx: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "XX",
        hint: "Matrix entry: input x to output x.",
    },
    xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "XY",
        hint: "Matrix entry: input y to output x.",
    },
    xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "XZ",
        hint: "Matrix entry: input z to output x.",
    },
    yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "YX",
        hint: "Matrix entry: input x to output y.",
    },
    yy: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "YY",
        hint: "Matrix entry: input y to output y.",
    },
    yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "YZ",
        hint: "Matrix entry: input z to output y.",
    },
    zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "ZX",
        hint: "Matrix entry: input x to output z.",
    },
    zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "ZY",
        hint: "Matrix entry: input y to output z.",
    },
    zz: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "ZZ",
        hint: "Matrix entry: input z to output z.",
    },
    tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "TX",
        hint: "Translation x.",
    },
    ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "TY",
        hint: "Translation y.",
    },
    tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "TZ",
        hint: "Translation z.",
    },
};

const INFO = #{ title: "Matrix 3D", name: "rhai_matrix3d" };

fn variation(z, p) {
    let xx = p.xx;
    let xy = p.xy;
    let yx = p.yx;
    let yy = p.yy;
    let tx = p.tx;
    let ty = p.ty;
    complex(xx * z.re + xy * z.im + tx, yx * z.re + yy * z.im + ty)
}

fn variation3(v, p) {
    let xx = p.xx;
    let xy = p.xy;
    let xz = p.xz;
    let yx = p.yx;
    let yy = p.yy;
    let yz = p.yz;
    let zx = p.zx;
    let zy = p.zy;
    let zz = p.zz;
    let tx = p.tx;
    let ty = p.ty;
    let tz = p.tz;
    vec3(xx * v.x + xy * v.y + xz * v.z + tx, yx * v.x + yy * v.y + yz * v.z + ty, zx * v.x + zy * v.y + zz * v.z + tz)
}
