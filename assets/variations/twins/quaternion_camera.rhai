/// A 4D camera for the quaternion variations (rotates the full q =
/// (x,y,z,w) before projection).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Quaternion Camera (4D), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    cam_x: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Camera X",
        hint: "4D eye position, x/i component (subtracted from the point before rotation/projection).",
    },
    cam_y: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Camera Y",
        hint: "4D eye position, y/j component.",
    },
    cam_z: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Camera Z",
        hint: "4D eye position, z/k component.",
    },
    cam_w: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Camera W",
        hint: "4D eye position along the 4th axis — THE slider this variation exists for. Sweep it to fly through the object in w; with Perspective > 0, parts you pass are clipped away like a real fly-through.",
    },
    rot_xw: #{
        value: 0.0, min: -3.1416, max: 3.1416, free: true,
        caption: "Rotate XW",
        hint: "Rotation of the (x, w) plane in radians — turns the view into the 4th dimension (a '4D yaw'). The ordinary xy/xz/yz rotations belong to the engine's 3D camera.",
    },
    rot_yw: #{
        value: 0.0, min: -3.1416, max: 3.1416, free: true,
        caption: "Rotate YW",
        hint: "Rotation of the (y, w) plane in radians (a '4D pitch').",
    },
    rot_zw: #{
        value: 0.0, min: -3.1416, max: 3.1416, free: true,
        caption: "Rotate ZW",
        hint: "Rotation of the (z, w) plane in radians.",
    },
    persp: #{
        value: 1.0, min: 0.0, max: 4.0,
        caption: "Perspective",
        hint: "4D perspective strength: p' = xyz / (1 - persp*w) after the eye transform — a pinhole on the w axis. 0 = orthographic (drop w). Higher = stronger w-foreshortening and a closer eye; points behind the eye are hidden.",
    },
    w_depth: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "W to Depth",
        hint: "Shear the camera-space w into the plotted DEPTH: z' = z + w_depth*w (before the perspective divide). This hands w to the engine's entire per-sample depth stack — DoF blur, depth fog, far-density fade, depth-density compensation — so their existing View-panel sliders become 4th-dimension effects: w-driven blur, w-driven dimming/fade. 0 = off.",
    },
    model: #{
        value: 0, choices: ["Off", "Poincaré H3", "Half-Space H3", "Poincaré H4", "Half-Space H4"],
        caption: "Hyperbolic Model",
        hint: "Hyperbolic model conversion applied BEFORE the eye/rotation/projection pipeline. Feed it Beltrami–Klein coordinates (the honeycomb variations' Klein projection — Klein is information-preserving, so the camera can re-derive any model from it). H3 modes convert xyz and pass w through; H4 modes convert the full (xyz, w) 4-ball — Half-Space H4 puts the height in w, ready for the pinhole and W-to-Depth stages. Off = plain 4D camera.",
    },
};

const INFO = #{ title: "Quaternion Camera (4D)", name: "rhai_quaternion_camera", category: "advanced_2d" };

fn variation(z, p, ctx) {
    let hidden = ctx.hide;
    #{ z: z, hide: hidden }
}

fn variation3(v, p, ctx) {
    let hidden = ctx.hide;
    let w_out = 0.0;
    let qin = vec4(v.x, v.y, v.z, ctx.w);
    let model = u32(to_float(p.model));
    if (model == u32(1) || model == u32(2)) {
        let b = vec3(qin.x, qin.y, qin.z);
        let r2 = min(dot(b, b), 1.0 - 1.0e-6);
        let t = 1.0 / sqrt(1.0 - r2);
        if (model == u32(1)) {
            let t1 = b * (t / (1.0 + t));
            qin = vec4(t1.x, t1.y, t1.z, qin.w);
        } else {
            let dz = max(1.0e-6, 1.0 - b.z);
            qin = vec4(b.x / dz, b.y / dz, 1.0 / (t * dz), qin.w);
        }
    } else if (model == u32(3) || model == u32(4)) {
        let b4 = qin;
        let r2 = min((b4.x * b4.x + b4.y * b4.y + b4.z * b4.z + b4.w * b4.w), 1.0 - 1.0e-6);
        let t = 1.0 / sqrt(1.0 - r2);
        if (model == u32(3)) {
            qin = b4 * (t / (1.0 + t));
        } else {
            let dw = max(1.0e-6, 1.0 - b4.w);
            qin = vec4(b4.x / dw, b4.y / dw, b4.z / dw, 1.0 / (t * dw));
        }
    }
    let q = qin - vec4(p.cam_x, p.cam_y, p.cam_z, p.cam_w);
    let axw = p.rot_xw;
    let cxw = cos(axw);
    let sxw = sin(axw);
    q = vec4(cxw * q.x + sxw * q.w, q.y, q.z, cxw * q.w - sxw * q.x);
    let ayw = p.rot_yw;
    let cyw = cos(ayw);
    let syw = sin(ayw);
    q = vec4(q.x, cyw * q.y + syw * q.w, q.z, cyw * q.w - syw * q.y);
    let azw = p.rot_zw;
    let czw = cos(azw);
    let szw = sin(azw);
    q = vec4(q.x, q.y, czw * q.z + szw * q.w, czw * q.w - szw * q.z);
    w_out = q.w;
    let wd = p.w_depth;
    q.z = q.z + wd * q.w;
    let persp = p.persp;
    if (persp < 1.0e-6) {
        return #{ z: vec3(q.x, q.y, q.z), hide: hidden, w: w_out };
    }
    let denom = 1.0 - persp * q.w;
    if (denom < 1.0e-3) {
        hidden = true;
        return #{ z: vec3(q.x, q.y, q.z), hide: hidden, w: w_out };
    }
    #{ z: vec3(q.x, q.y, q.z) / denom, hide: hidden, w: w_out }
}
