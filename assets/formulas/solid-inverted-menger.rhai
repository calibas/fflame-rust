// The Menger sponge turned inside out: every point of space inverted in
// the unit sphere before the sponge is asked about it. Use it from Escape
// mode ("Use as solid").
//
// It shows a solid calling a built-in flame variation:
// `vary("spherical3D", v)` applies the variation's 3D body to the vector
// `v` (here the inversion, `v / |v|^2`), on the GPU. A variation's
// parameters, when it has some, follow in a map --
// `vary("julia3D", v, #{ power: p.power })` -- each a number or one of the
// solid's own parameters.
//
// A variation stretches space, and the distance a solid gives must be in
// the space the camera marches through. The inversion's stretch at `pos`
// is `1 / |pos|^2`, but a step is not a point: the stretch changes along
// it. The sponge `d` away from the inverted point `q` is at least
// `d |pos|^2 / (1 + d |pos|)` away from `pos` (`distance` below). The
// stretch at `pos` alone, `d |pos|^2`, can be twice that where `d` is
// long beside `|q|`, and the march steps through the solid there. For a
// variation whose stretch you do not know, divide the distance by a
// margin instead: the march then takes smaller, safer steps.

const PARAMS = #{
    scale: #{ value: 3.0, min: 1.1, max: 4.0, caption: "Scale", hint: "How much each iteration enlarges space after the mirrors" },
    offset: #{ value: vec3(1.0, 1.0, 1.0), caption: "Offset", hint: "The corner each iteration scales away from" },
};

// The sponge fills |q| <= |offset| about the origin and its middle is
// hollow, so its inversion lies between 1/|offset| and the middle's
// inverse: within 3 of the origin.
const INFO = #{ title: "Inverted Menger (Rhai)", radius: 3.0 };

fn prepare(p) {
    #{ radius: max(length(p.offset), 1e-3), shift: p.offset * (p.scale - 1.0) }
}

// The parts in descending order: the three diagonal mirrors.
fn sorted(v) {
    let a = if v.x < v.y { vec3(v.y, v.x, v.z) } else { v };
    let b = if a.x < a.z { vec3(a.z, a.y, a.x) } else { a };
    if b.y < b.z { vec3(b.x, b.z, b.y) } else { b }
}

fn start(pos, p) {
    // The inversion, and how much it shrank space here. A point the
    // inversion throws past the ball is already out.
    let q = vary("spherical3D", pos);
    #{ z: q, r2: dot(pos, pos), k: 0, out: length(q) > p.radius }
}

fn step(s, p) {
    let z = sorted(abs(s.z)) * p.scale - p.shift;
    // The sponge: the middle layer's cubes move back up.
    s.z = if z.z < -0.5 * p.shift.z { vec3(z.x, z.y, z.z + p.shift.z) } else { z };
    s.k += 1;
    if length(s.z) > p.radius {
        s.out = true;
    }
    s
}

fn escaped(s, p) {
    length(s.z) > 1000.0 * p.radius
}

// The sponge's distance (each step stretches by at most the scale), taken
// back through the inversion: `|pos|^2 / (1 + d |pos|)`, the least the
// inversion shrinks a step of `d` from here (see the top).
fn distance(s, p) {
    if s.out {
        let d = max(length(s.z) - p.radius, 0.0) * p.scale ** (-s.k.to_float());
        d * s.r2 / (1.0 + d * sqrt(s.r2))
    } else {
        0.0
    }
}
