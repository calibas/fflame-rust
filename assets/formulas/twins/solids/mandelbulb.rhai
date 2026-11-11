// The built-in Mandelbulb (Solid) as a solid of your own (the Escape
// panel's Edit as Rhai): White and Nylander's power in spherical
// coordinates, z <- z^n + c, with Hart's half of the distance estimate,
// as Syntopia writes it. Each step is the built-in's arithmetic, in its
// order, so the two draw alike.
//
// The colourings read the start of the orbit, within Colour Depth, in
// terms the bulb's turns about the z axis keep: the orbit trap the closest
// approach as (z, |xy|), the level log2 of the running derivative per
// power of r, and the address which hemisphere each iterate falls in, then
// its polar angle at the last.

const PARAMS = #{
    // The template's, declared to keep the built-in's defaults.
    levels: #{ value: 12.0, min: 1.0, max: 64.0, caption: "Iterations", hint: "How many powers before calling the point part of the set." },
    steps: #{ value: 160.0, min: 4.0, max: 512.0, caption: "March Steps", hint: "How many times a ray may step before giving up." },
    power: #{
        value: 8.0, min: 2.0, max: 16.0,
        caption: "Power",
        hint: "The n in White and Nylander's power: 8 is the bulb everyone knows, 2 the quadratic one.",
    },
    mode: #{
        value: 0,
        caption: "Mode",
        hint: "The Mandelbulb adds the point itself each iteration; its Julia adds the constant below, the same for every point.",
        choices: ["Mandelbulb", "Julia"],
    },
    cx: #{ value: 0.0, min: -2.0, max: 2.0, caption: "Julia X", hint: "The Julia constant's x." },
    cy: #{ value: 0.0, min: -2.0, max: 2.0, caption: "Julia Y", hint: "The Julia constant's y." },
    cz: #{ value: 0.0, min: -2.0, max: 2.0, caption: "Julia Z", hint: "The Julia constant's z." },
    colour_depth: #{
        value: 3.0, min: 1.0, max: 12.0,
        caption: "Colour Depth",
        hint: "How many of the first iterations the colourings read: the orbit trap's closest approach, the level's stretch and the address. The surface is drawn a pixel from the set, and past a few iterations that pixel decides the orbit, so deeper colours change with the camera. Raise it when zoomed in.",
    },
    step_factor: #{
        value: 1.0, min: 0.25, max: 1.0,
        caption: "Step Factor",
        hint: "Scales the distance the march steps by. The bulb's estimate is not a bound: beside thin parts it was measured at 1.7 times the true distance, so a ray can step past a hair. Lower it if thin parts break up, at the cost of more steps.",
    },
};

const INFO = #{ title: "Mandelbulb (Solid)", slow: ["levels", "steps"] };

// The ball, as the built-in's (`mandelbulb_radius`): past 2^(1/(n-1)) an
// orbit grows, and for a Julia also past 1 + |c|.
fn prepare(p) {
    let n = clamp(p.power, 2.0, 16.0);
    let bulb = pow(2.0, 1.0 / (n - 1.0));
    let ball = if p.mode > 0 {
        max(bulb, 1.0 + sqrt(p.cx * p.cx + p.cy * p.cy + p.cz * p.cz))
    } else {
        bulb
    };
    // Where the walk stops: past the ball, where `ln r` has settled.
    #{ ball: ball, bail: max(2.0 * ball, 64.0) }
}

fn start(pos, p) {
    let c = if p.mode > 0 { vec3(p.cx, p.cy, p.cz) } else { pos };
    #{
        z: pos, c: c, dr: 1.0, k: 0, out: false,
        trap: complex(0.0, 0.0), trap_r: 1.0e30, level: 0.0, color: 0.5,
        address: 0.0, place: 0.5,
    }
}

fn step(s, p) {
    let n = clamp(p.power, 2.0, 16.0);
    let depth = to_int(clamp(round_even(p.colour_depth), 1.0, 12.0));
    let z = s.z;
    let r = length(z);
    let theta = acos(clamp(z.z / max(r, 1.0e-30), -1.0, 1.0));
    let phi = atan2(z.y, z.x);
    let rn1 = if r > 0.0 { pow(r, n - 1.0) } else { 0.0 };
    s.dr = rn1 * n * s.dr + (if p.mode > 0 { 0.0 } else { 1.0 });
    let zr = if r > 0.0 { pow(r, n) } else { 0.0 };
    let th = theta * n;
    let ph = phi * n;
    s.z = vec3(sin(th) * cos(ph), sin(ph) * sin(th), cos(th)) * zr + s.c;
    let r1 = length(s.z);
    s.k += 1;
    if s.k <= depth {
        s.level = log2(max(s.dr, 1.0)) / (n - 1.0);
        let zt = trap_point(s.z);
        let rt = length(zt);
        if rt < s.trap_r {
            s.trap_r = rt;
            s.trap = complex(zt.z, sqrt(zt.x * zt.x + zt.y * zt.y));
        }
        if s.k < depth {
            if s.z.z < 0.0 {
                s.address += s.place;
            }
            s.place = s.place * 0.5;
        } else {
            s.address += acos(clamp(s.z.z / max(r1, 1.0e-30), -1.0, 1.0)) * 0.31830988618 * s.place;
        }
    }
    if r1 > p.ball {
        s.out = true;
    }
    s
}

// Past the ball the orbit escapes; the estimate is taken further out,
// where ln r has settled, and short of where a power of 16 overflows f32.
fn escaped(s, p) {
    length(s.z) > p.bail
}

// Hart's estimate, with his half: not a bound for the bulb, so the step
// factor is the user's margin. Nothing where the orbit never left the ball.
fn distance(s, p) {
    if s.out {
        let r = length(s.z);
        clamp(p.step_factor, 0.25, 1.0) * max(0.5 * ln(r) * r / max(s.dr, 1.0e-30), 0.0)
    } else {
        0.0
    }
}
