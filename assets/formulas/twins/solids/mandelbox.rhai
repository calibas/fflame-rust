// The built-in Mandelbox (Solid) as a solid of your own (the Escape
// panel's Edit as Rhai): Tom Lowe's box and sphere folds, z <- s fold(z)
// + c, with Syntopia's distance estimate at 0.95 of itself. Each step is
// the built-in's arithmetic, in its order, so the two draw alike.
//
// The colourings read the start of the orbit: the running scale `dr`
// (Escape Level), the closest approach of `z` (Orbit Trap), and the
// `address` -- each iteration's digit in base 12, how many axes the box
// fold reflected (0 to 3) times 3 plus which of the sphere fold's three
// cases applied, the first iteration in the first place.

const PARAMS = #{
    scale: #{
        value: 2.0, min: -4.0, max: 4.0,
        caption: "Scale",
        hint: "How much each iteration enlarges space after the folds. 2 is Lowe's box; negative scales (-1.5 is the classic) turn it inside out. Between -1 and 1 nothing escapes, so the magnitude is held to 1.05 at least.",
    },
    min_radius: #{
        value: 0.5, min: 0.0, max: 2.0,
        caption: "Minimum Radius",
        hint: "Inside this radius the sphere fold scales linearly; between it and the fixed radius, it inverts.",
    },
    fixed_radius: #{
        value: 1.0, min: 0.1, max: 3.0,
        caption: "Fixed Radius",
        hint: "The radius the sphere fold inverts in.",
    },
    fold_limit: #{
        value: 1.0, min: 0.1, max: 3.0,
        caption: "Folding Limit",
        hint: "Where the box fold reflects each axis.",
    },
    mode: #{
        value: 0,
        caption: "Mode",
        hint: "The Mandelbox adds the point itself each iteration; its Julia adds the constant below, the same for every point.",
        choices: ["Mandelbox", "Julia"],
    },
    cx: #{ value: 0.0, min: -4.0, max: 4.0, caption: "Julia X", hint: "The Julia constant's x." },
    cy: #{ value: 0.0, min: -4.0, max: 4.0, caption: "Julia Y", hint: "The Julia constant's y." },
    cz: #{ value: 0.0, min: -4.0, max: 4.0, caption: "Julia Z", hint: "The Julia constant's z." },
    // The template's Colour Depth, declared to keep the built-in's hint.
    colour_depth: #{
        value: 3.0, min: 1.0, max: 12.0,
        caption: "Colour Depth",
        hint: "How many of the first iterations the colourings read: the orbit trap's closest approach, the level's stretch and the address. The surface is drawn a pixel from the set, and past a few iterations that pixel decides the orbit, so deeper colours change with the camera. Raise it when zoomed in.",
    },
};

const INFO = #{ title: "Mandelbox (Solid)" };

// The ball the set lies in, as the built-in's (`mandelbox_radius`): the
// cube a point past which escapes, reached at its corners.
fn prepare(p) {
    let s = max(abs(p.scale), 1.05);
    let l = max(p.fold_limit, 0.1);
    let cube = if p.mode > 0 {
        let c = max(max(abs(p.cx), abs(p.cy)), abs(p.cz));
        (2.0 * l * s + c) / (s - 1.0)
    } else {
        2.0 * l * (s + 1.0) / (s - 1.0)
    };
    #{ ball: min(cube * sqrt(3.0), 256.0) }
}

fn start(pos, p) {
    let c = if p.mode > 0 { vec3(p.cx, p.cy, p.cz) } else { pos };
    #{ z: pos, c: c, dr: 1.0, address: 0.0, place: 1.0 / 12.0 }
}

fn step(s, p) {
    // |s| at least 1.05, its sign kept.
    let s_in = p.scale;
    let sc = if s_in < 0.0 { min(s_in, -1.05) } else { max(s_in, 1.05) };
    let min_r = max(p.min_radius, 0.0);
    let fixed_r = max(p.fixed_radius, 0.1);
    let min_r2 = max(min(min_r * min_r, fixed_r * fixed_r), 1.0e-6);
    let fixed_r2 = fixed_r * fixed_r;
    let limit = max(p.fold_limit, 0.1);
    let z = s.z;
    // How many axes the box fold reflects.
    let axes = 0;
    if abs(z.x) > limit { axes += 1; }
    if abs(z.y) > limit { axes += 1; }
    if abs(z.z) > limit { axes += 1; }
    z = clamp(z, -limit, limit) * 2.0 - z;
    let r2 = dot(z, z);
    let fold_case = 0;
    if r2 < min_r2 {
        let t = fixed_r2 / min_r2;
        z = z * t;
        s.dr = s.dr * t;
        fold_case = 2;
    } else if r2 < fixed_r2 {
        let t = fixed_r2 / r2;
        z = z * t;
        s.dr = s.dr * t;
        fold_case = 1;
    }
    s.z = z * sc + s.c;
    s.dr = s.dr * abs(sc) + (if p.mode > 0 { 0.0 } else { 1.0 });
    s.address = s.address + to_float(axes * 3 + fold_case) * s.place;
    s.place = s.place / 12.0;
    s
}

fn escaped(s, p) {
    length(s.z) > 100.0
}

fn distance(s, p) {
    0.95 * length(s.z) / max(abs(s.dr), 1.0e-30)
}
