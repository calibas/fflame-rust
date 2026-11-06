/// Minkowski camera: Poincaré-group observer positioning (spacetime
/// translation, rotation, boost/velocity) + W-fold projection modes
/// (drop / stack / time-perspective / retarded light-arrival /
/// interval-radial). Designed for final transforms; no color params.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Minkowski Camera, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    w_mode: #{
        value: 0, choices: ["Drop W", "W to Z", "Time Perspective", "Retarded", "Interval Radial"],
        caption: "W Fold",
        hint: "How the 4D spacetime point folds into the 3D plot. Drop W: orthographic spatial slice. W to Z: stack time along z (z += Strength*w) — causal structure becomes height. Time Perspective: divide space by (1 - Strength*w), the time-axis analogue of the perspective divide — future magnifies, past recedes. Retarded: light-arrival view, radius r + Strength*w (Strength 1 = the physical backward light cone; negative radii fold through the origin). Interval Radial: radius = sqrt(|interval|) — the light cone collapses to the origin, causal shells become spheres.",
    },
    tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Translate X",
        hint: "Observer position x (subtracted before rotation/boost).",
    },
    ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Translate Y",
        hint: "Observer position y.",
    },
    tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Translate Z",
        hint: "Observer position z (3D).",
    },
    tw: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Translate W",
        hint: "Observer position in TIME — slides which 'now' the camera sits at; with Retarded or Time Perspective folding this scrubs through the content's causal history.",
    },
    rot_x: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Rotate X",
        hint: "Spatial rotation about x (3D).",
    },
    rot_y: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Rotate Y",
        hint: "Spatial rotation about y (3D).",
    },
    rot_z: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Rotate Z",
        hint: "Spatial rotation about z.",
    },
    bx: #{
        value: 0.0, min: -3.0, max: 3.0,
        caption: "Boost X",
        hint: "Observer velocity: rapidity vector x component. Speed = tanh of the vector's length; direction = the vector. A moving camera sees relativistic aberration — content crowds toward the direction of motion. In 2D this is the boost along x.",
    },
    by: #{
        value: 0.0, min: -3.0, max: 3.0,
        caption: "Boost Y",
        hint: "Rapidity vector y component.",
    },
    bz: #{
        value: 0.0, min: -3.0, max: 3.0,
        caption: "Boost Z",
        hint: "Rapidity vector z component (3D).",
    },
    strength: #{
        value: 1.0, min: -4.0, max: 4.0,
        caption: "Strength",
        hint: "Intensity of the selected W Fold (W to Z stack height, Time Perspective strength, Retarded light-delay factor).",
    },
    size: #{
        value: 1.0, min: 0.05, max: 4.0,
        caption: "Size",
        hint: "Output scale.",
    },
};

const INFO = #{ title: "Minkowski Camera", name: "rhai_minkowski_camera", category: "advanced_2d" };

fn variation(z, p) {
    let w_mode = u32(to_float(p.w_mode));
    let tx = p.tx;
    let tw = p.tw;
    let bx = p.bx;
    let strength = p.strength;
    let size = p.size;
    let x = z.re - tx;
    let t = z.im - tw;
    if (bx != 0.0) {
        let ch = cosh(bx);
        let sh = sinh(bx);
        let nx = x * ch + t * sh;
        t = x * sh + t * ch;
        x = nx;
    }
    if (w_mode == u32(2)) {
        let dn = 1.0 - strength * t;
        if (abs(dn) < 0.05) {
            dn = (if dn < 0.0 { -0.05 } else { 0.05 });
        }
        x = x / dn;
    } else if (w_mode == u32(3)) {
        x = (if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }) * (abs(x) + strength * t);
    } else if (w_mode == u32(4)) {
        let q = x * x - t * t;
        x = (if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }) * sqrt(abs(q));
    }
    complex(x, t) * size
}

fn variation3(v, p, ctx) {
    let w_out = 0.0;
    let w_mode = u32(to_float(p.w_mode));
    let tx = p.tx;
    let ty = p.ty;
    let tz = p.tz;
    let tw = p.tw;
    let rx = p.rot_x * 0.01745329252;
    let ry = p.rot_y * 0.01745329252;
    let rz = p.rot_z * 0.01745329252;
    let bv = vec3(p.bx, p.by, p.bz);
    let strength = p.strength;
    let size = p.size;
    let s = v - vec3(tx, ty, tz);
    let t = ctx.w - tw;
    if (rx != 0.0) {
        let c = cos(rx);
        let sn = sin(rx);
        s = vec3(s.x, c * s.y - sn * s.z, sn * s.y + c * s.z);
    }
    if (ry != 0.0) {
        let c = cos(ry);
        let sn = sin(ry);
        s = vec3(c * s.x + sn * s.z, s.y, -sn * s.x + c * s.z);
    }
    if (rz != 0.0) {
        let c = cos(rz);
        let sn = sin(rz);
        s = vec3(c * s.x - sn * s.y, sn * s.x + c * s.y, s.z);
    }
    let rap = length(bv);
    if (rap > 1.0e-6) {
        let n = bv / rap;
        let par = dot(s, n);
        let ch = cosh(rap);
        let sh = sinh(rap);
        let np = par * ch + t * sh;
        t = par * sh + t * ch;
        s = s + (np - par) * n;
    }
    if (w_mode == u32(1)) {
        s.z = s.z + strength * t;
    } else if (w_mode == u32(2)) {
        let dn = 1.0 - strength * t;
        if (abs(dn) < 0.05) {
            dn = (if dn < 0.0 { -0.05 } else { 0.05 });
        }
        s = s / dn;
    } else if (w_mode == u32(3)) {
        let r = length(s);
        if (r > 1.0e-9) {
            s = (s / r) * (r + strength * t);
        }
    } else if (w_mode == u32(4)) {
        let r = length(s);
        if (r > 1.0e-9) {
            let q = r * r - t * t;
            s = (s / r) * sqrt(abs(q));
        }
    }
    w_out = t;
    #{ z: s * size, w: w_out }
}
