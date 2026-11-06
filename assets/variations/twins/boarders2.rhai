/// Variant of Boarders with 3 tunable knobs — scale factor `c` and per-
/// direction `left`/`right` offsets controlling the border-push behavior.
///
/// # Authors
/// - Xyrus02

// Boarders 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    c: #{
        value: 0.4, min: -5.0, max: 5.0, free: true,
        caption: "C",
        hint: "Cell scale factor — how much the in-cell offset shrinks toward the cell center.",
    },
    left: #{
        value: 0.65, min: -5.0, max: 5.0, free: true,
        caption: "Left",
        hint: "Border push-out distance — how far points get pushed toward the nearest cell border.",
    },
    right: #{
        value: 0.35, min: -5.0, max: 5.0, free: true,
        caption: "Right",
        hint: "Threshold for border behavior vs pass-through. Higher = more points get pushed to borders.",
    },
};

const INFO = #{ title: "Boarders 2", name: "rhai_boarders2", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn prepare(p) {
    let c0 = max(1.0e-6, abs(p.c));
    let cl0 = max(1.0e-6, abs(p.left));
    let cr0 = max(1.0e-6, abs(p.right));
    #{ d0: c0, cl: c0 * cl0, cr: c0 + c0 * cr0 }
}

fn variation(z, p) {
    let c = p.d0;
    let cl = p.cl;
    let cr = p.cr;
    let round_x = round_even(z.re);
    let round_y = round_even(z.im);
    let off_x = z.re - round_x;
    let off_y = z.im - round_y;
    if (rand() >= cr) {
        return complex(off_x * c + round_x, off_y * c + round_y);
    }
    if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            return complex(off_x * c + round_x + cl, off_y * c + round_y + cl * off_y / off_x);
        }
        return complex(off_x * c + round_x - cl, off_y * c + round_y - cl * off_y / off_x);
    }
    if (off_y >= 0.0) {
        return complex(off_x * c + round_x + off_x / off_y * cl, off_y * c + round_y + cl);
    }
    complex(off_x * c + round_x - off_x / off_y * cl, off_y * c + round_y - cl)
}

fn variation3(v, p) {
    let c = p.d0;
    let cl = p.cl;
    let cr = p.cr;
    let round_x = round_even(v.x);
    let round_y = round_even(v.y);
    let off_x = v.x - round_x;
    let off_y = v.y - round_y;
    if (rand() >= cr) {
        return vec3(off_x * c + round_x, off_y * c + round_y, v.z);
    }
    if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            return vec3(off_x * c + round_x + cl, off_y * c + round_y + cl * off_y / off_x, v.z);
        }
        return vec3(off_x * c + round_x - cl, off_y * c + round_y - cl * off_y / off_x, v.z);
    }
    if (off_y >= 0.0) {
        return vec3(off_x * c + round_x + off_x / off_y * cl, off_y * c + round_y + cl, v.z);
    }
    vec3(off_x * c + round_x - off_x / off_y * cl, off_y * c + round_y - cl, v.z)
}
