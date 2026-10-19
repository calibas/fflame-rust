// The Menger sponge as a kaleidoscopic IFS, written as a Rhai solid: the
// built-in KIFS (Solid) at fold "Menger", scale 3, offset (1, 1, 1), pixel
// for pixel. Use it from Escape mode ("Use as solid").
//
// It shows what a solid can do beyond `start`, `step`, `escaped` and
// `distance` (see solid-mandelbox.rhai for those):
// - `prepare(p)` computes values once per picture, from the parameters,
//   and `step` reads them as `p.name`: here the ball and the shift, one of
//   them a vector.
// - A function parameter (`function: "abs"`) is a menu in the panel; on a
//   vector, `p.mirror(v)` takes it part by part (the real part of the
//   function of each). Pick another to bend the sponge.
// - A helper can give a vector, and an `if` can be one: `sorted` below.

const PARAMS = #{
    scale: #{ value: 3.0, min: 1.1, max: 4.0, caption: "Scale", hint: "How much each iteration enlarges space after the mirrors" },
    offset: #{ value: vec3(1.0, 1.0, 1.0), caption: "Offset", hint: "The corner each iteration scales away from" },
    mirror: #{ function: "abs", caption: "Mirror", hint: "The fold, applied to each part: abs is the Menger's" },
};

// The ball: every mirror fixes the origin, so a point past |offset| escapes.
const INFO = #{ title: "KIFS Menger (Rhai)", radius: 1.7320508 };

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
    #{ z: pos, k: 0, out: false }
}

fn step(s, p) {
    let z = sorted(p.mirror(s.z)) * p.scale - p.shift;
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

// Each step stretches distances by at most the scale: past the ball, the
// distance left over, shrunk back by every step taken.
fn distance(s, p) {
    if s.out { max(length(s.z) - p.radius, 0.0) * p.scale ** (-s.k.to_float()) } else { 0.0 }
}
