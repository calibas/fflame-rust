// Halley, Fractint's Halley map: Halley's root-finding method on
//   F(z) = z^(a+1) - z = z (z^a - 1),
// whose roots are 0 and the a-th roots of unity, each pixel stepping
//   z' = z - r F / (F' - F'' F / (2 F')),
// r the relaxation. A pixel stops when |z|^2 settles: the change in it
// under Epsilon (the program's test). Ported from Iterated Dynamics (the
// GPL Fractint successor; libid/fractals/halley.cpp, its defaults and
// view from fractalp.cpp).

const PARAMS = #{
    order: #{ value: 6, min: 2, max: 30, caption: "Order", hint: "a in z (z^a - 1): the roots, and so the basins, are 0 and the a-th roots of unity" },
    relax: #{ value: complex(1.0, 0.0), caption: "Relaxation", hint: "How much of each Halley step is taken (1 is the method itself)" },
    epsilon: #{ value: 0.0001, min: 0.0, caption: "Epsilon", hint: "A pixel stops when |z|^2 changes by less than this in a step" },
};

const INFO = #{ title: "Halley", center: complex(0.0, 0.0), magn: 1.3333333333333333, maxiter: 150 };

fn start(pixel, p, ctx) {
    #{ z: pixel, prev: pixel }
}

fn step(s, p) {
    let z = s.z;
    // z^(a-1), z^a, z^(a+1) (a power, not the program's loop of
    // products, which would keep the formula from deep zoom).
    let za1 = z ** (p.order - 1);
    let za = z * za1;
    let fx = z * za - z;
    let ap1 = (p.order + 1).to_float();
    let f2 = za1 * (ap1 * p.order.to_float());
    let f1 = za * ap1 - complex(1.0, 0.0);
    let numer1 = (f2 * fx) / (f1 + f1);
    s.prev = z;
    s.z = z - p.relax * (fx / (f1 - numer1));
    s
}

fn escaped(s, p) {
    (s.z.norm_sqr() - s.prev.norm_sqr()).abs() < p.epsilon
}
