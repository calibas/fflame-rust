// A pass-dependent step: the Mandelbrot and the Burning Ship in turn,
// its cycle declared so deep zoom keeps a reference per phase.

const INFO = #{ title: "Alternating", cycle: #{ prefix: 0, period: 2 } };

fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), c: pixel }
}

fn step(s, p, pass) {
    if pass % 2 == 0 {
        s.z = s.z ** 2 + s.c;
    } else {
        s.z = fold(s.z) ** 2 + s.c;
    }
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > 4.0
}
