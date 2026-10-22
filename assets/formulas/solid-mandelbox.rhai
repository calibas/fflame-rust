// The Mandelbox as a solid written in Rhai: Tom Lowe's box and sphere
// folds, with Syntopia's distance estimate. Use it from Escape mode
// ("Use as solid"): it renders in mode D like the built-in solids, with
// their lighting, path tracer and colourings.
//
// A solid is four functions over a state that holds `z`, the iterate, a
// vector (`vec3`): `start` sets it from the point `pos` being measured,
// `step` is one iteration, `escaped` says when to stop, and `distance`
// gives the estimate of how far `pos` is from the set. A `dr` in the
// state (the running derivative) colours the Escape Level; the closest
// approach of `z` over the first iterations colours the Orbit Trap.
// `INFO.radius` is the ball the set lies in: the march starts there.

const PARAMS = #{
    scale: #{ value: 2.0, min: 1.05, max: 4.0, caption: "Scale", hint: "How much each iteration enlarges space after the folds" },
    min_radius: #{ value: 0.5, min: 0.0, max: 2.0, caption: "Minimum Radius", hint: "Inside this radius the sphere fold scales linearly" },
    fixed_radius: #{ value: 1.0, min: 0.1, max: 3.0, caption: "Fixed Radius", hint: "The radius the sphere fold inverts in" },
    fold_limit: #{ value: 1.0, min: 0.1, max: 3.0, caption: "Folding Limit", hint: "Where the box fold reflects each axis" },
};

// The ball at scale 2: 2 L (s + 1) / (s - 1), times the root of 3.
const INFO = #{ title: "Mandelbox (Rhai)", radius: 10.4 };

fn start(pos, p) {
    #{ z: pos, c: pos, dr: 1.0 }
}

fn step(s, p) {
    // The box fold: each axis reflected about the folding limit.
    let lim = p.fold_limit;
    let z = clamp(s.z, -lim, lim) * 2.0 - s.z;
    // The sphere fold: scaled up inside the minimum radius, inverted
    // between it and the fixed one.
    let r2 = dot(z, z);
    let fr2 = p.fixed_radius * p.fixed_radius;
    let mr2 = max(min(p.min_radius * p.min_radius, fr2), 1e-6);
    let t = 1.0;
    if r2 < mr2 {
        t = fr2 / mr2;
    } else if r2 < fr2 {
        t = fr2 / r2;
    }
    s.z = z * t * p.scale + s.c;
    s.dr = s.dr * t * abs(p.scale) + 1.0;
    s
}

fn escaped(s, p) {
    length(s.z) > 100.0
}

fn distance(s, p) {
    0.95 * length(s.z) / abs(s.dr)
}
