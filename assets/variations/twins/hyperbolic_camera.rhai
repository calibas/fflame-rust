/// Hyperbolic camera: convert between explicit 2D/3D hyperbolic models
/// (Poincaré, Klein, half-plane/space, hyperboloid, equidistant, band)
/// and apply a hyperbolic isometry — fly-through-hyperbolic-space as a
/// final transform; 2D charts of 3D content keep depth as z.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Hyperbolic Camera, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    in_model: #{
        value: 0, choices: ["Poincaré Disk (2D)", "Klein Disk (2D)", "Half-Plane (2D)", "Hyperboloid (2D)", "Equidistant (2D)", "Band (2D)", "Poincaré Ball (3D)", "Klein Ball (3D)", "Half-Space (3D)", "Hyperboloid (3D)", "Equidistant (3D)", "Equal-Area (2D)", "Equal-Area (3D)"],
        caption: "In Model",
        hint: "How the incoming point is interpreted. 2D charts read xy (input z passes in as the third hyperboloid coordinate — planar content becomes a positionable H3 object). 3D charts read the full point. In 2D render mode, 3D selections demote to their 2D counterparts. Half-Plane height = y; Half-Space height = z (the Kleinian H3 space-mode convention); Hyperboloid is the raw Minkowski spatial part (Gans model); Equidistant has Euclidean radius = true hyperbolic distance (the azimuthal equidistant projection, as on the UN-emblem world map); as an In Model it is also the natural way to import flat non-hyperbolic content — radial distances and angles at the center carry over exactly (the exponential map); Band is the conformal strip (log half-plane); Equal-Area is the hyperbolic Lambert azimuthal (radius = 2*sinh(d/2)) — image area is faithful to intrinsic hyperbolic area.",
    },
    out_model: #{
        value: 0, choices: ["Poincaré Disk (2D)", "Klein Disk (2D)", "Half-Plane (2D)", "Hyperboloid (2D)", "Equidistant (2D)", "Band (2D)", "Poincaré Ball (3D)", "Klein Ball (3D)", "Half-Space (3D)", "Hyperboloid (3D)", "Equidistant (3D)", "Equal-Area (2D)", "Equal-Area (3D)"],
        caption: "Out Model",
        hint: "The model the point is re-projected into. Selecting a 2D chart in 3D render mode projects the H3 point geodesically onto the equatorial plane for the chart and emits the dropped spatial component as z — a 2D projection of 3D hyperbolic content that keeps depth. Poincaré → Klein straightens tiling edges into chords; → Half-Plane/Space unrolls against a boundary; → Hyperboloid gives the unbounded Gans funnel; → Equidistant keeps distances radially true (azimuthal equidistant — no boundary crush); → Band unrolls into an infinite conformal frieze ribbon; → Equal-Area keeps image area faithful to hyperbolic area (unbounded, grows as e^(d/2) — between Equidistant and Gans).",
    },
    tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Translate X",
        hint: "Hyperbolic translation along x, in units of hyperbolic distance (rapidity of the Lorentz boost). Moving the observer: the scene slides toward the opposite boundary, crowding conformally.",
    },
    ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Translate Y",
        hint: "Hyperbolic translation along y.",
    },
    tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Translate Z",
        hint: "Hyperbolic translation along z (3D render mode only). With a 2D in-chart this lifts the planar content off the equatorial plane.",
    },
    rot_x: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Rotate X",
        hint: "Rotation about the x axis through the model center (3D only). With a 2D in-chart this tilts the planar content through H3.",
    },
    rot_y: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Rotate Y",
        hint: "Rotation about the y axis through the model center (3D only).",
    },
    rot_z: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Rotate Z",
        hint: "Rotation about the z axis (the in-plane rotation in 2D).",
    },
    size: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Size",
        hint: "Model scale in world units: input is divided by it, output multiplied — ball models occupy radius `size`.",
    },
    curvature: #{
        value: -1.0, min: -100.0, max: -0.01, free: true,
        caption: "Curvature K",
        hint: "Sectional curvature the camera experiences (K < 0; content is interpreted at K = -1). The content's intrinsic scale is locked to the curvature radius 1/sqrt(-K) while the camera's ruler stays fixed — the exact radial rescale d' = d/sqrt(-K) of hyperbolic distance about the OBSERVER (applied after the isometry, so the camera position is the fixed point). K = -100: the horizon crowds inward, objects shrink faster, area growth explodes. K = -0.01: the world flattens toward Euclidean — content huddles at the model center, parallels barely diverge, the horizon recedes. Only possible because hyperbolic geometry has an intrinsic scale — flat space has no such dial.",
    },
    h_zoom: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Hyperbolic Zoom",
        hint: "Moves the IDEAL BOUNDARY instead of the field of view — Euclidean zoom rescales everything, hyperbolic zoom changes how much of hyperbolic infinity is compressed into the image while the scale at the observer stays fixed. The azimuthal out-charts (Poincaré, Klein, Hyperboloid/Gans) are one projective family R = k*2*rho/(1 + a*rho^2); this slides `a` from the chart's native value (Klein 1, Poincaré 0, Gans -1). Positive: the boundary circle recedes off-frame — past Gans (e.g. Poincaré at +1.5) visual infinity sits at FINITE hyperbolic distance and only that ball of the plane is shown, at true scale. Negative: all of infinity compresses into an ever smaller circle. No effect on Equidistant/Equal-Area/Half-Plane/Band out-charts or on in-charts.",
    },
    zoom_wrap: #{
        value: false,
        caption: "Zoom Wrap",
        hint: "When Hyperbolic Zoom puts visual infinity at finite distance (past Gans), content beyond it is physically invisible — off by default it flies off-frame along its own ray (clipped). On: the raw projective behavior — beyond-horizon content wraps antipodally back into frame as an inverted exterior corona around the horizon ring. Incorrect as a projection, striking as an effect.",
    },
    shell_min: #{
        value: 0.0, min: 0.0, max: 20.0,
        caption: "Shell Min",
        hint: "Hide points closer than this hyperbolic distance from the observer (measured after the isometry and curvature — the experienced distance). With Shell Max this isolates a constant-distance shell: a true hyperbolic annulus (2D) or ball shell (3D) around the camera. 0 = no near clip.",
    },
    shell_max: #{
        value: 0.0, min: 0.0, max: 20.0,
        caption: "Shell Max",
        hint: "Hide points farther than this hyperbolic distance from the observer; 0 = no far clip. Hyperbolic area grows exponentially with radius, so successive shells hold dramatically more content — sweeping the shell outward is a tour through the tiling's generations.",
    },
};

const INFO = #{ title: "Hyperbolic Camera", name: "rhai_hyperbolic_camera", category: "advanced_2d" };

fn hycam_to_hyp2(u_in, model) {
    let u = u_in;
    if (model == u32(0)) {
        let r2 = u.norm_sqr();
        if (r2 >= 1.0) {
            u = u / (r2 + 1.0e-9);
        }
        let den = max(1.0e-6, 1.0 - u.norm_sqr());
        let t1 = 2.0 * u / den;
        return vec3(t1.re, t1.im, (1.0 + u.norm_sqr()) / den);
    }
    if (model == u32(1)) {
        let r2 = u.norm_sqr();
        if (r2 >= 1.0) {
            u = u / (r2 + 1.0e-9);
        }
        let s = sqrt(max(1.0e-9, 1.0 - u.norm_sqr()));
        let t2 = u / s;
        return vec3(t2.re, t2.im, 1.0 / s);
    }
    if (model == u32(2)) {
        let w = max(1.0e-6, abs(u.im));
        let hx = u.re / w;
        let s2 = u.re * u.re + w * w;
        let t = (1.0 + s2) / (2.0 * w);
        return vec3(hx, t - 1.0 / w, t);
    }
    if (model == u32(4)) {
        let d = u.abs();
        if (d < 1.0e-9) {
            return vec3(0.0, 0.0, 1.0);
        }
        let t3 = (sinh(d) / d) * u;
        return vec3(t3.re, t3.im, cosh(d));
    }
    if (model == u32(5)) {
        let by = clamp(u.im, -1.5507, 1.5507);
        let ex = exp(clamp(u.re, -20.0, 20.0));
        let hp = ex * complex(-sin(by), cos(by));
        let w = max(1.0e-6, hp.im);
        let s2 = hp.re * hp.re + w * w;
        let t = (1.0 + s2) / (2.0 * w);
        return vec3(hp.re / w, t - 1.0 / w, t);
    }
    if (model == u32(6)) {
        let r2 = u.norm_sqr();
        let t4 = u * sqrt(1.0 + 0.25 * r2);
        return vec3(t4.re, t4.im, 1.0 + 0.5 * r2);
    }
    vec3(u.re, u.im, sqrt(1.0 + u.norm_sqr()))
}

fn hycam_from_hyp2(h, model, hz, wrap) {
    let t = max(1.0, h.z);
    if (abs(hz) > 1.0e-9 && (model == u32(0) || model == u32(1) || model == u32(3))) {
        let pv = complex(h.x, h.y) / (1.0 + t);
        let a = -hz;
        let k = 0.5;
        if (model == u32(1)) {
            a = 1.0 - hz;
            k = 1.0;
        }
        if (model == u32(3)) {
            a = -1.0 - hz;
            k = 1.0;
        }
        let dn = 1.0 + a * pv.norm_sqr();
        if (wrap) {
            if (abs(dn) < 1.0e-4) {
                dn = (if dn < 0.0 { -1.0e-4 } else { 1.0e-4 });
            }
        } else {
            dn = max(1.0e-4, dn);
        }
        return pv * (2.0 * k / dn);
    }
    if (model == u32(0)) {
        return complex(h.x, h.y) / (1.0 + t);
    }
    if (model == u32(1)) {
        return complex(h.x, h.y) / t;
    }
    if (model == u32(2)) {
        let iw = max(1.0e-6, t - h.y);
        return complex(h.x / iw, 1.0 / iw);
    }
    if (model == u32(4)) {
        let sn = complex(h.x, h.y).abs();
        if (sn < 1.0e-9) {
            return complex(0.0, 0.0);
        }
        return (acosh(t) / sn) * complex(h.x, h.y);
    }
    if (model == u32(5)) {
        let iw = max(1.0e-6, t - h.y);
        let hp = complex(h.x / iw, 1.0 / iw);
        return complex(0.5 * ln(max(1.0e-20, hp.norm_sqr())), atan2(hp.im, hp.re) - 1.5707963);
    }
    if (model == u32(6)) {
        return complex(h.x, h.y) * sqrt(2.0 / (1.0 + t));
    }
    complex(h.x, h.y)
}

fn hycam_to_hyp3(u_in, c) {
    let u = u_in;
    if (c == u32(0)) {
        let r2 = dot(u, u);
        if (r2 >= 1.0) {
            u = u / (r2 + 1.0e-9);
        }
        let den = max(1.0e-6, 1.0 - dot(u, u));
        let t1 = 2.0 * u / den;
        return vec4(t1.x, t1.y, t1.z, (1.0 + dot(u, u)) / den);
    }
    if (c == u32(1)) {
        let r2 = dot(u, u);
        if (r2 >= 1.0) {
            u = u / (r2 + 1.0e-9);
        }
        let s = sqrt(max(1.0e-9, 1.0 - dot(u, u)));
        let t2 = u / s;
        return vec4(t2.x, t2.y, t2.z, 1.0 / s);
    }
    if (c == u32(2)) {
        let w = max(1.0e-6, abs(u.z));
        let hx = complex(u.x, u.y) / w;
        let s2 = complex(u.x, u.y).norm_sqr() + w * w;
        let t = (1.0 + s2) / (2.0 * w);
        return vec4(hx.re, hx.im, t - 1.0 / w, t);
    }
    if (c == u32(4)) {
        let d = length(u);
        if (d < 1.0e-9) {
            return vec4(0.0, 0.0, 0.0, 1.0);
        }
        let t3 = (sinh(d) / d) * u;
        return vec4(t3.x, t3.y, t3.z, cosh(d));
    }
    if (c == u32(5)) {
        let r2 = dot(u, u);
        let t4 = u * sqrt(1.0 + 0.25 * r2);
        return vec4(t4.x, t4.y, t4.z, 1.0 + 0.5 * r2);
    }
    vec4(u.x, u.y, u.z, sqrt(1.0 + dot(u, u)))
}

fn hycam_from_hyp3(h, c, hz, wrap) {
    let t = max(1.0, h.w);
    if (abs(hz) > 1.0e-9 && (c == u32(0) || c == u32(1) || c == u32(3))) {
        let pv = vec3(h.x, h.y, h.z) / (1.0 + t);
        let a = -hz;
        let k = 0.5;
        if (c == u32(1)) {
            a = 1.0 - hz;
            k = 1.0;
        }
        if (c == u32(3)) {
            a = -1.0 - hz;
            k = 1.0;
        }
        let dn = 1.0 + a * dot(pv, pv);
        if (wrap) {
            if (abs(dn) < 1.0e-4) {
                dn = (if dn < 0.0 { -1.0e-4 } else { 1.0e-4 });
            }
        } else {
            dn = max(1.0e-4, dn);
        }
        return pv * (2.0 * k / dn);
    }
    if (c == u32(0)) {
        return vec3(h.x, h.y, h.z) / (1.0 + t);
    }
    if (c == u32(1)) {
        return vec3(h.x, h.y, h.z) / t;
    }
    if (c == u32(2)) {
        let iw = max(1.0e-6, t - h.z);
        let t1 = complex(h.x, h.y) / iw;
        return vec3(t1.re, t1.im, 1.0 / iw);
    }
    if (c == u32(4)) {
        let sn = length(vec3(h.x, h.y, h.z));
        if (sn < 1.0e-9) {
            return vec3(0.0, 0.0, 0.0);
        }
        return (acosh(t) / sn) * vec3(h.x, h.y, h.z);
    }
    if (c == u32(5)) {
        return vec3(h.x, h.y, h.z) * sqrt(2.0 / (1.0 + t));
    }
    vec3(h.x, h.y, h.z)
}

fn variation(w, p, ctx) {
    let hidden = ctx.hide;
    let in_model = u32(to_float(p.in_model));
    let out_model = u32(to_float(p.out_model));
    if (in_model >= u32(11)) {
        in_model = u32(6);
    } else if (in_model >= u32(6)) {
        in_model = in_model - u32(6);
    }
    if (out_model >= u32(11)) {
        out_model = u32(6);
    } else if (out_model >= u32(6)) {
        out_model = out_model - u32(6);
    }
    let tx = p.tx;
    let ty = p.ty;
    let rot_z = p.rot_z * 0.01745329252;
    let size = max(1.0e-6, p.size);
    let curv = sqrt(clamp(-p.curvature, 1.0e-4, 1.0e4));
    let h_zoom = p.h_zoom;
    let zoom_wrap = to_float(p.zoom_wrap) > 0.5;
    let h = hycam_to_hyp2(w / size, in_model);
    if (rot_z != 0.0) {
        let cz = cos(rot_z);
        let sz = sin(rot_z);
        h = vec3(cz * h.x - sz * h.y, sz * h.x + cz * h.y, h.z);
    }
    let d = sqrt(tx * tx + ty * ty);
    if (d > 1.0e-9) {
        let n = complex(tx, ty) / d;
        let t1 = complex(h.x, h.y);
        let par = (t1.re * n.re + t1.im * n.im);
        let perp = complex(h.x, h.y) - par * n;
        let ch = cosh(d);
        let sh = sinh(d);
        let t2 = perp + (ch * par + sh * h.z) * n;
        h = vec3(t2.re, t2.im, sh * par + ch * h.z);
    }
    if (abs(curv - 1.0) > 1.0e-6) {
        let sn = complex(h.x, h.y).abs();
        if (sn > 1.0e-9) {
            let dd = min(40.0, acosh(max(1.0, h.z)) / curv);
            let t3 = (sinh(dd) / sn) * complex(h.x, h.y);
            h = vec3(t3.re, t3.im, cosh(dd));
        }
    }
    let shell_min = p.shell_min;
    let shell_max = p.shell_max;
    if (shell_min > 0.0 || shell_max > 0.0) {
        let dcam = acosh(max(1.0, h.z));
        let smax = (if shell_max <= 0.0 { 1.0e9 } else { shell_max });
        if (dcam < shell_min || dcam > smax) {
            hidden = true;
        }
    }
    #{ z: hycam_from_hyp2(h, out_model, h_zoom, zoom_wrap) * size, hide: hidden }
}

fn variation3(v, p, ctx) {
    let hidden = ctx.hide;
    let in_model = u32(to_float(p.in_model));
    let out_model = u32(to_float(p.out_model));
    let tx = p.tx;
    let ty = p.ty;
    let tz = p.tz;
    let rot_x = p.rot_x * 0.01745329252;
    let rot_y = p.rot_y * 0.01745329252;
    let rot_z = p.rot_z * 0.01745329252;
    let size = max(1.0e-6, p.size);
    let curv = sqrt(clamp(-p.curvature, 1.0e-4, 1.0e4));
    let h_zoom = p.h_zoom;
    let zoom_wrap = to_float(p.zoom_wrap) > 0.5;
    let u = v / size;
    let h = vec4(0.0, 0.0, 0.0, 0.0);
    if (in_model < u32(6) || in_model == u32(11)) {
        let h2 = hycam_to_hyp2(complex(u.x, u.y), (if in_model == u32(11) { u32(6) } else { in_model }));
        h = vec4(h2.x, h2.y, u.z, sqrt(1.0 + h2.x * h2.x + h2.y * h2.y + u.z * u.z));
    } else {
        h = hycam_to_hyp3(u, (if in_model == u32(12) { u32(5) } else { in_model - u32(6) }));
    }
    if (rot_x != 0.0) {
        let c = cos(rot_x);
        let s = sin(rot_x);
        h = vec4(h.x, c * h.y - s * h.z, s * h.y + c * h.z, h.w);
    }
    if (rot_y != 0.0) {
        let c = cos(rot_y);
        let s = sin(rot_y);
        h = vec4(c * h.x + s * h.z, h.y, -s * h.x + c * h.z, h.w);
    }
    if (rot_z != 0.0) {
        let c = cos(rot_z);
        let s = sin(rot_z);
        h = vec4(c * h.x - s * h.y, s * h.x + c * h.y, h.z, h.w);
    }
    let d = sqrt(tx * tx + ty * ty + tz * tz);
    if (d > 1.0e-9) {
        let n = vec3(tx, ty, tz) / d;
        let par = dot(vec3(h.x, h.y, h.z), n);
        let perp = vec3(h.x, h.y, h.z) - par * n;
        let ch = cosh(d);
        let sh = sinh(d);
        let t1 = perp + (ch * par + sh * h.w) * n;
        h = vec4(t1.x, t1.y, t1.z, sh * par + ch * h.w);
    }
    if (abs(curv - 1.0) > 1.0e-6) {
        let sn = length(vec3(h.x, h.y, h.z));
        if (sn > 1.0e-9) {
            let dd = min(40.0, acosh(max(1.0, h.w)) / curv);
            let t3 = (sinh(dd) / sn) * vec3(h.x, h.y, h.z);
            h = vec4(t3.x, t3.y, t3.z, cosh(dd));
        }
    }
    let shell_min = p.shell_min;
    let shell_max = p.shell_max;
    if (shell_min > 0.0 || shell_max > 0.0) {
        let dcam = acosh(max(1.0, h.w));
        let smax = (if shell_max <= 0.0 { 1.0e9 } else { shell_max });
        if (dcam < shell_min || dcam > smax) {
            hidden = true;
        }
    }
    if (out_model < u32(6) || out_model == u32(11)) {
        let t2 = sqrt(1.0 + h.x * h.x + h.y * h.y);
        let out2 = hycam_from_hyp2(vec3(h.x, h.y, t2), (if out_model == u32(11) { u32(6) } else { out_model }), h_zoom, zoom_wrap);
        return #{ z: vec3(out2.re, out2.im, h.z) * size, hide: hidden };
    }
    #{ z: hycam_from_hyp3(h, (if out_model == u32(12) { u32(5) } else { out_model - u32(6) }), h_zoom, zoom_wrap) * size, hide: hidden }
}
