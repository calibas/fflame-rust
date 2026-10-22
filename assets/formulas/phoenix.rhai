// The Phoenix: each pass feeds the one before back in. The state
// carries the previous iterate, `prev`, beside z.

const PARAMS = #{
    p: #{ value: complex(-0.5, 0.0), caption: "Feedback", hint: "How much of the previous z each pass adds" },
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout" },
};

const INFO = #{ title: "Phoenix", center: complex(0.0, 0.0), maxiter: 300 };

fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), prev: complex(0.0, 0.0), c: pixel }
}

fn step(s, p) {
    // A new map: every value reads the state before any is set.
    #{ z: s.z * s.z + s.c + p.p * s.prev, prev: s.z }
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
