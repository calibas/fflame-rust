/// Post-phase axis-symmetry mirror — with 50% probability per iteration,
/// mirrors the post-phase accumulator across the chosen axis (X/Y/Z) at a
/// configurable center, with optional `rotation` applied to the mirrored
/// point. The other 50% adds a half-weight shift in the same direction (no
/// mirror). Each iteration's coin flip independently chooses mirror vs
/// shift.

// Post Axis Symmetry WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    axis: #{
        value: 0, choices: ["X", "Y", "Z"],
        caption: "Axis",
        hint: "Axis of symmetry to mirror across. Z is 3D-only.",
    },
    centre_x: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Centre X",
        hint: "X coordinate of the symmetry plane center.",
    },
    centre_y: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Centre Y",
        hint: "Y coordinate of the symmetry plane center.",
    },
    centre_z: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Centre Z",
        hint: "Z coordinate of the symmetry plane center (3D only).",
    },
    rotation: #{
        value: 30.0, min: -360.0, max: 360.0, free: true,
        caption: "Rotation",
        hint: "Post-mirror rotation angle, in degrees. Full circle = 360. Init precomputes `sin/cos(rotation · 2π/180 / 2)`.",
    },
};

const INFO = #{ title: "Post Axis Symmetry WF", name: "rhai_post_axis_symmetry_wf", category: "full_3d", phase: "post" };

fn prepare(p) {
    let two_pi = 6.28318530717959;
    let a = p.rotation * two_pi / 180.0 * 0.5;
    #{ sina: sin(a), cosa: cos(a) }
}

fn variation(z, p, ctx) {
    let axis = to_int(to_float(p.axis));
    let cx = p.centre_x;
    let cy = p.centre_y;
    let sina = p.sina;
    let cosa = p.cosa;
    let w = ctx.weight;
    let halve = w * 0.5;
    let do_rotate = abs(sina) > 1.0e-6;
    let x = z.re;
    let y = z.im;
    if (axis == 0) {
        let dx0 = z.re - cx;
        if (rand() < 0.5) {
            x = cx + dx0 + halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa + dya * sina;
                y = cy + dya * cosa - dxa * sina;
            }
        } else {
            x = cx - dx0 - halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa - dya * sina;
                y = cy + dya * cosa + dxa * sina;
            }
        }
    } else if (axis == 1) {
        let dy0 = z.im - cy;
        if (rand() < 0.5) {
            y = cy + dy0 + halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa + dya * sina;
                y = cy + dya * cosa - dxa * sina;
            }
        } else {
            y = cy - dy0 - halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa - dya * sina;
                y = cy + dya * cosa + dxa * sina;
            }
        }
    }
    complex(x, y)
}

fn variation3(v, p, ctx) {
    let axis = to_int(to_float(p.axis));
    let cx = p.centre_x;
    let cy = p.centre_y;
    let cz = p.centre_z;
    let sina = p.sina;
    let cosa = p.cosa;
    let w = ctx.weight;
    let halve = w * 0.5;
    let do_rotate = abs(sina) > 1.0e-6;
    let x = v.x;
    let y = v.y;
    let z = v.z;
    if (axis == 0) {
        let dx0 = v.x - cx;
        if (rand() < 0.5) {
            x = cx + dx0 + halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa + dya * sina;
                y = cy + dya * cosa - dxa * sina;
            }
        } else {
            x = cx - dx0 - halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa - dya * sina;
                y = cy + dya * cosa + dxa * sina;
            }
        }
    } else if (axis == 1) {
        let dy0 = v.y - cy;
        if (rand() < 0.5) {
            y = cy + dy0 + halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa + dya * sina;
                y = cy + dya * cosa - dxa * sina;
            }
        } else {
            y = cy - dy0 - halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa - dya * sina;
                y = cy + dya * cosa + dxa * sina;
            }
        }
    } else {
        let dz0 = v.z - cz;
        if (rand() < 0.5) {
            z = cz + dz0 + halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dza = z - cz;
                x = cx + dxa * cosa + dza * sina;
                z = cy + dza * cosa - dxa * sina;
            }
        } else {
            z = cz - dz0 - halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dza = z - cz;
                x = cx + dxa * cosa - dza * sina;
                z = cy + dza * cosa + dxa * sina;
            }
        }
    }
    vec3(x, y, z)
}
