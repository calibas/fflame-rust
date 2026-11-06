/// Onion-shaped 3D transform — smoothly transitions a circle into a
/// (negative) exponential curve at a `meeting_pt`, projecting the result
/// onto a sphere. Below the meeting point the input lies on a sphere of
/// radius `cos(t)`; above it, on an exponential decay curve. `top_crop`
/// optionally truncates the exponential tail.
///
/// # Authors
/// - chronologicaldot

// Onion 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    meeting_pt: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Meeting Pt",
        hint: "Where the circle and exponential curves meet (in radians).",
    },
    circle_a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Circle A",
        hint: "Radius scale for the circle branch (controls overall onion radius).",
    },
    circle_b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Circle B",
        hint: "Radius scale for the exponential branch (controls Z output).",
    },
    shift_x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift X",
        hint: "X center offset (subtracted from input, added back to output).",
    },
    shift_y: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift Y",
        hint: "Y center offset.",
    },
    shift_z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift Z",
        hint: "Z center offset — unused in the body, preserved as a parameter for preset compatibility.",
    },
    top_crop: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Top Crop",
        hint: "Crops Z above this value in the exponential branch. 0 disables cropping.",
    },
    stretch: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Stretch",
        hint: "Radial divisor — `t = sqtr / stretch − π/2`.",
    },
};

const INFO = #{ title: "Onion 2", name: "rhai_onion2", category: "full_3d", phase: "any" };

fn variation(z, p, ctx) {
    let meeting_pt = p.meeting_pt;
    let circle_a = p.circle_a;
    let shift_x = p.shift_x;
    let shift_y = p.shift_y;
    let top_crop = p.top_crop;
    let stretch = p.stretch;
    let pi_2 = 1.5707963267948966;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let safe_stretch = (if abs(stretch) < 1.0e-30 { 1.0e-30 } else { stretch });
    let inix = z.re - shift_x;
    let iniy = z.im - shift_y;
    let sqtr = max(sqrt(inix * inix + iniy * iniy), 1.0e-30);
    let t = sqtr / safe_stretch - pi_2;
    let r = 0.0;
    if (t > meeting_pt) {
        r = cos(t);
        let tmpt = tan(meeting_pt);
        if (abs(tmpt) > 1.0e-30) {
            let z_local = exp(cos(meeting_pt) - r) / tmpt + sin(meeting_pt) - 1.0 / tmpt;
            if (z_local > top_crop && top_crop > 0.0) {
                r = 0.0;
            }
        }
    } else {
        r = cos(t);
    }
    r = r * circle_a * w;
    let fx = r * (inix / sqtr) + shift_x;
    let fy = r * (iniy / sqtr) + shift_y;
    complex(fx * inv_w, fy * inv_w)
}

fn variation3(v, p, ctx) {
    let meeting_pt = p.meeting_pt;
    let circle_a = p.circle_a;
    let circle_b = p.circle_b;
    let shift_x = p.shift_x;
    let shift_y = p.shift_y;
    let top_crop = p.top_crop;
    let stretch = p.stretch;
    let pi_2 = 1.5707963267948966;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let safe_stretch = (if abs(stretch) < 1.0e-30 { 1.0e-30 } else { stretch });
    let inix = v.x - shift_x;
    let iniy = v.y - shift_y;
    let iniz = v.z;
    let sqtr = max(sqrt(inix * inix + iniy * iniy + iniz * iniz), 1.0e-30);
    let t = sqtr / safe_stretch - pi_2;
    let r = 0.0;
    let z = 0.0;
    if (t > meeting_pt) {
        r = cos(t);
        let tmpt = tan(meeting_pt);
        if (abs(tmpt) > 1.0e-30) {
            z = exp(cos(meeting_pt) - r) / tmpt + sin(meeting_pt) - 1.0 / tmpt;
            if (z > top_crop && top_crop > 0.0) {
                z = top_crop;
                r = 0.0;
            }
        } else {
            z = top_crop;
        }
    } else {
        r = cos(t);
        z = sin(t);
    }
    r = r * circle_a * w;
    z = z * circle_b * w;
    let fx = r * (inix / sqtr);
    let fy = r * (iniy / sqtr);
    let fz = z;
    fx = fx + r * iniz * (inix / sqtr);
    fy = fy + r * iniz * (iniy / sqtr);
    let denom = max(r * r + z * z, 1.0e-30);
    fz = fz + iniz * z / denom;
    fx = fx + shift_x;
    fy = fy + shift_y;
    vec3(fx * inv_w, fy * inv_w, fz * inv_w)
}
