// Both planes and the fold: the Burning Ship, its Julia sets by the
// panel's Julia toggle (`plane.julia`, `plane.c`).

const PARAMS = #{ bailout: 4.0 };
const INFO = #{ title: "Burning Ship, both planes", center: complex(-0.5, -0.5) };

fn start(pixel, p, plane) {
    let z0 = if plane.julia { pixel } else { complex(0.0, 0.0) };
    let c = if plane.julia { plane.c } else { pixel };
    #{ z: z0, c: c }
}

fn step(s, p) {
    s.z = fold(s.z) ** 2 + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
