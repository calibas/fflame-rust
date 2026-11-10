// The built-in Quaternion Julia (Solid) as a solid of your own (the
// Escape panel's Edit as Rhai): q <- q^n + c over the quaternions, the 3D
// slice pinning one of q's four parts, with Hart's half of the distance
// estimate. Each step is the built-in's arithmetic, in its order -- the
// power by the polar form -- so the two draw alike.
//
// The colourings read the start of the orbit, within Colour Depth, in
// terms every turn of the vector part keeps: the orbit trap the closest
// approach as (scalar part, vector part's length), the level log2 of the
// running derivative, and the address which branch of the n-th root the
// orbit is on, a base-n digit an iteration, then the angle in its sector.

const PARAMS = #{
    // The template's, declared to keep the built-in's defaults and words.
    levels: #{
        value: 32.0, min: 1.0, max: 256.0,
        caption: "Iterations",
        hint: "How many times to apply q -> q^n + c before calling the point part of the set. The distance is taken where |q| has run past 10^4, so a few beyond the bailout are spent sharpening it.",
    },
    beam: #{
        value: 1.0, min: 1.0, max: 8.0,
        caption: "Beam Width",
        hint: "Unused here: a quaternion power has one inverse orbit. Kept so the march, shadow and occlusion slots line up with the flame solid's.",
    },
    steps: #{ value: 96.0, min: 4.0, max: 512.0, caption: "March Steps", hint: "How many times a ray may step before giving up." },
    w_slice: #{
        value: 0.0, min: -2.0, max: 2.0,
        caption: "Slice Value",
        hint: "Where along the slice axis the 3D slice sits. Sweep it to walk through the 4D solid.",
    },
    cx: #{
        value: -1.0, min: -2.0, max: 2.0,
        caption: "Constant X",
        hint: "The scalar (real) part of the Julia constant c. Bourke writes c scalar-first; his (-1, 0.2, 0, 0) is this at -1.",
    },
    cy: #{ value: 0.2, min: -2.0, max: 2.0, caption: "Constant Y", hint: "The i component of c." },
    cz: #{ value: 0.0, min: -2.0, max: 2.0, caption: "Constant Z", hint: "The j component of c." },
    cw: #{ value: 0.0, min: -2.0, max: 2.0, caption: "Constant W", hint: "The k component of c." },
    power: #{ value: 2.0, min: 2.0, max: 8.0, caption: "Power", hint: "The n in q^n + c. 2 is the classic quadratic set." },
    bailout: #{
        value: 2.0, min: 1.0, max: 8.0,
        caption: "Bailout",
        hint: "A point whose orbit passes this radius is outside the set. Also the radius of the ball the camera frames.",
    },
    slice_axis: #{
        value: 3,
        caption: "Slice Axis",
        hint: "Which quaternion component the 3D slice pins: 0 = scalar, 1 = i, 2 = j, 3 = k. For a complex c (j = k = 0) the k slice contains the complex plane and shows the classic solid.",
        choices: ["Scalar", "i", "j", "k"],
    },
    colour_depth: #{
        value: 3.0, min: 1.0, max: 12.0,
        caption: "Colour Depth",
        hint: "How many of the first iterations the colourings read: the orbit trap's closest approach, the level's bands and the address's angle. The surface is drawn a pixel from the set, and past a few iterations that pixel decides the orbit, so deeper colours change with the camera. Raise it when zoomed in.",
    },
};

const INFO = #{ title: "Quaternion Julia (Solid)", coloring: "ifs_level", slow: ["levels", "beam", "steps"] };

// The ball the camera frames: the bailout's radius.
fn prepare(p) {
    #{ ball: max(p.bailout, 1.0e-3) }
}

// q^n by the polar form, as the built-in's `qj_pow`: |q|^n (cos na +
// n^ sin na), n^ the unit vector part (i where there is none).
fn qpow(q, n) {
    let mag = length(q);
    if mag < 1.0e-30 {
        quat(0.0, 0.0, 0.0, 0.0)
    } else {
        let rad = pow(mag, n);
        let ang = acos(clamp(q.w / mag, -1.0, 1.0)) * n;
        let vlen = length(q.v);
        let nhat = if vlen > 1.0e-12 { q.v / vlen } else { vec3(1.0, 0.0, 0.0) };
        let sv = rad * sin(ang);
        quat(rad * cos(ang), sv * nhat.x, sv * nhat.y, sv * nhat.z)
    }
}

fn start(pos, p) {
    // Lift to 4D: the slice axis takes the slice value and the other
    // three the point, in order.
    let sl = p.w_slice;
    let q = if p.slice_axis == 0 {
        quat(sl, pos.x, pos.y, pos.z)
    } else if p.slice_axis == 1 {
        quat(pos.x, sl, pos.y, pos.z)
    } else if p.slice_axis == 2 {
        quat(pos.x, pos.y, sl, pos.z)
    } else {
        quat(pos.x, pos.y, pos.z, sl)
    };
    #{ z: q, dr: 1.0, k: 0, out: false, trap: complex(0.0, 0.0), trap_r: 1.0e30, color: 0.5, address: 0.0, span: 1.0 }
}

fn step(s, p) {
    let n = clamp(round_even(p.power), 2.0, 8.0);
    let depth = to_int(clamp(round_even(p.colour_depth), 1.0, 12.0));
    // |dq| is multiplicative: n |q|^(n-1) each step.
    s.dr = n * pow(length(s.z), n - 1.0) * s.dr;
    s.z = qpow(s.z, n) + quat(p.cx, p.cy, p.cz, p.cw);
    let mag = length(s.z);
    s.k += 1;
    if s.k <= depth {
        let vt = trap_point(s.z.v);
        let mt = sqrt(s.z.w * s.z.w + dot(vt, vt));
        if mt < s.trap_r {
            s.trap_r = mt;
            s.trap = complex(s.z.w, length(vt));
        }
        let sector = atan2(length(s.z.v), s.z.w) * n * 0.31830988618;
        if s.k < depth {
            s.span = s.span / n;
            s.address += min(to_float(floor(sector)), n - 1.0) * s.span;
        } else {
            s.address += sector / n * s.span;
        }
    }
    if mag > max(p.bailout, 1.0) {
        s.out = true;
    }
    s
}

fn escaped(s, p) {
    length(s.z) > 1.0e4
}

// Hart's estimate, with his half for the march's sake; nothing where the
// orbit never passed the bailout.
fn distance(s, p) {
    if s.out {
        let mag = length(s.z);
        0.5 * mag * ln(mag) / max(s.dr, 1.0e-30)
    } else {
        0.0
    }
}
