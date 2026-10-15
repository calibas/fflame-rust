// Every statement and parameter kind a Rhai formula has: carried state
// (`prev`), a field set by assignment in `start`, `let`, `let` from an
// `if`, `if`/`else if`/`else`, `while`, `do ... until`, `for` over a
// range, op-assignment, a map returned from `step`, a helper; complex,
// int (with choices), bool and function parameters.

const PARAMS = #{
    k: #{ value: complex(0.5667, -0.1), caption: "Feedback" },
    mode: #{ value: 1, min: 0, max: 2, choices: ["Plain", "Damped", "Twisted"] },
    flip_it: #{ value: false, caption: "Flip" },
    f: #{ function: "sin", caption: "Function" },
    rounds: #{ value: 2, min: 1, max: 4 },
};

const INFO = #{ title: "Constructs" };

fn damping(mode) {
    let half = 0.5;
    if mode == 1 { half } else if mode == 2 { half * 0.5 } else { 1.0 }
}

fn start(pixel, p) {
    let s = #{ z: pixel * 0.1, prev: complex(0.0, 0.0) };
    s.c = pixel;
    s
}

fn step(s, p) {
    let t = s.z;
    let d = damping(p.mode);
    let acc = complex(0.0, 0.0);
    for i in 0..p.rounds {
        acc += s.prev * (0.25 * d);
    }
    let j = 0;
    do {
        j += 1;
    } until j >= 2;
    let w = 0;
    while w < 3 {
        w += 1;
    }
    let twist = if p.flip_it { flip(s.z) } else { s.z };
    let next = twist * twist + s.c + acc * p.k + p.f(s.z) * 0.01;
    #{ z: next, prev: t }
}

fn escaped(s, p) {
    let r = s.z.norm_sqr();
    r > 16.0
}
