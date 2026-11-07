/// General 3D affine transform — applies translation, per-axis scaling,
/// yaw/pitch/roll rotation, and optional shear in a single combined
/// transform. The 15 parameters cover translate (3), scale (3), rotate (3
/// in degrees), and shear (6 cross-axis terms). The body automatically
/// detects whether shear is significant (`|sxy| + |sxz| + ... > ε`) and
/// skips the sheared path when all 6 shear params are below threshold.
///
/// # Authors
/// - Framelet

// Affine 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    translateX: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Translate X",
        hint: "X-axis translation (added to the final output).",
    },
    translateY: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Translate Y",
        hint: "Y-axis translation.",
    },
    translateZ: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Translate Z",
        hint: "Z-axis translation (3D only).",
    },
    scaleX: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale X",
        hint: "X-axis scale (multiplies x before rotation).",
    },
    scaleY: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Y",
        hint: "Y-axis scale.",
    },
    scaleZ: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Z",
        hint: "Z-axis scale.",
    },
    rotateX: #{
        value: 0.0, min: -360.0, max: 360.0, free: true,
        caption: "Rotate X",
        hint: "Rotation around the X axis (pitch), in degrees.",
    },
    rotateY: #{
        value: 0.0, min: -360.0, max: 360.0, free: true,
        caption: "Rotate Y",
        hint: "Rotation around the Y axis (yaw), in degrees.",
    },
    rotateZ: #{
        value: 0.0, min: -360.0, max: 360.0, free: true,
        caption: "Rotate Z",
        hint: "Rotation around the Z axis (roll), in degrees.",
    },
    shearXY: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shear XY",
        hint: "Shear factor applied to Y in the X output.",
    },
    shearXZ: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shear XZ",
        hint: "Shear factor applied to Z in the X output.",
    },
    shearYX: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shear YX",
        hint: "Shear factor applied to X in the Y output.",
    },
    shearYZ: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shear YZ",
        hint: "Shear factor applied to Z in the Y output.",
    },
    shearZX: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shear ZX",
        hint: "Shear factor applied to X in the Z output.",
    },
    shearZY: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shear ZY",
        hint: "Shear factor applied to Y in the Z output.",
    },
};

const INFO = #{ title: "Affine 3D", name: "rhai_affine3d", category: "full_3d", phase: "any" };

fn prepare(p) {
    let d2r = 0.017453292519943295;
    let rx = p.rotateX;
    let ry = p.rotateY;
    let rz = p.rotateZ;
    let shxy = p.shearXY;
    let shxz = p.shearXZ;
    let shyx = p.shearYX;
    let shyz = p.shearYZ;
    let shzx = p.shearZX;
    let shzy = p.shearZY;
    let has_shear = abs(shxy) + abs(shxz) + abs(shyx) + abs(shyz) + abs(shzx) + abs(shzy) > 1.0e-6;
    #{ sinx: sin(rx * d2r), cosx: cos(rx * d2r), siny: sin(ry * d2r), cosy: cos(ry * d2r), sinz: sin(rz * d2r), cosz: cos(rz * d2r), has_shear: (if has_shear { 1.0 } else { 0.0 }) }
}

fn variation(w, p) {
    let tx = p.translateX;
    let ty = p.translateY;
    let sx_p = p.scaleX;
    let sy_p = p.scaleY;
    let sz_p = p.scaleZ;
    let shxy = p.shearXY;
    let shxz = p.shearXZ;
    let shyx = p.shearYX;
    let shyz = p.shearYZ;
    let shzx = p.shearZX;
    let shzy = p.shearZY;
    let sinx = p.sinx;
    let cosx = p.cosx;
    let siny = p.siny;
    let cosy = p.cosy;
    let sinz = p.sinz;
    let cosz = p.cosz;
    let has_shear = p.has_shear > 0.5;
    let x = w.re;
    let y = w.im;
    let z = 0.0;
    if (has_shear) {
        let mx = shxy * sy_p * y + shxz * sz_p * z + sx_p * x;
        let my = shyx * sx_p * x + shyz * sz_p * z + sy_p * y;
        let mz = shzx * sx_p * x + shzy * sy_p * y + sz_p * z;
        let nx = cosz * (cosy * mx + siny * (sinx * my + cosx * mz)) - sinz * (cosx * my - sinx * mz) + tx;
        let ny = sinz * (cosy * mx + siny * (sinx * my + cosx * mz)) + cosz * (cosx * my - sinx * mz) + ty;
        return complex(nx, ny);
    }
    let nx = cosz * (cosy * sx_p * x + siny * (cosx * sz_p * z + sinx * sy_p * y)) - sinz * (cosx * sy_p * y - sinx * sz_p * z) + tx;
    let ny = sinz * (cosy * sx_p * x + siny * (cosx * sz_p * z + sinx * sy_p * y)) + cosz * (cosx * sy_p * y - sinx * sz_p * z) + ty;
    complex(nx, ny)
}

fn variation3(v, p) {
    let tx = p.translateX;
    let ty = p.translateY;
    let tz = p.translateZ;
    let sx_p = p.scaleX;
    let sy_p = p.scaleY;
    let sz_p = p.scaleZ;
    let shxy = p.shearXY;
    let shxz = p.shearXZ;
    let shyx = p.shearYX;
    let shyz = p.shearYZ;
    let shzx = p.shearZX;
    let shzy = p.shearZY;
    let sinx = p.sinx;
    let cosx = p.cosx;
    let siny = p.siny;
    let cosy = p.cosy;
    let sinz = p.sinz;
    let cosz = p.cosz;
    let has_shear = p.has_shear > 0.5;
    let x = v.x;
    let y = v.y;
    let z = v.z;
    if (has_shear) {
        let mx = shxy * sy_p * y + shxz * sz_p * z + sx_p * x;
        let my = shyx * sx_p * x + shyz * sz_p * z + sy_p * y;
        let mz = shzx * sx_p * x + shzy * sy_p * y + sz_p * z;
        let nx = cosz * (cosy * mx + siny * (sinx * my + cosx * mz)) - sinz * (cosx * my - sinx * mz) + tx;
        let ny = sinz * (cosy * mx + siny * (sinx * my + cosx * mz)) + cosz * (cosx * my - sinx * mz) + ty;
        let nz = -siny * mx + cosy * (sinx * my + cosx * mz) + tz;
        return vec3(nx, ny, nz);
    }
    let nx = cosz * (cosy * sx_p * x + siny * (cosx * sz_p * z + sinx * sy_p * y)) - sinz * (cosx * sy_p * y - sinx * sz_p * z) + tx;
    let ny = sinz * (cosy * sx_p * x + siny * (cosx * sz_p * z + sinx * sy_p * y)) + cosz * (cosx * sy_p * y - sinx * sz_p * z) + ty;
    let nz = -siny * sx_p * x + cosy * (cosx * sz_p * z + sinx * sy_p * y) + tz;
    vec3(nx, ny, nz)
}
