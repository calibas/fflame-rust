// An animation track with a state: the zoom follows another zoom track
// on a spring, lagging behind each move and overshooting where it stops.
//
// How to use it: add a keyframe track on Zoom with the motion you want
// (or a signal track), then this track. Script tracks run after keyframe
// and signal tracks, whatever the order of the list, so this one reads
// the keyframed zoom with ctx.get("zoom") and writes its sprung version
// over it. Without a zoom track under it, it reads the fractal's own
// zoom, which does not move, and so neither does the spring.
//
// A track with `start` and `step` keeps a state, stepped 240 times a
// second from time zero: the same time always gives the same state,
// whether you play, scrub or export, at any frame rate.

const PARAMS = #{
    stiffness: #{ value: 40.0, min: 1.0, max: 400.0, caption: "Stiffness", hint: "How hard the spring pulls toward the keyframed zoom." },
    damping: #{ value: 6.0, min: 0.0, max: 50.0, caption: "Damping", hint: "How quickly it stops swinging. Low values overshoot more." },
};

const INFO = #{ title: "Springy Zoom" };

const TARGETS = #{ zoom: "set" };

// The state at time zero: at the zoom, at rest.
fn start(p, ctx) {
    #{ x: ctx.get("zoom"), v: 0.0 }
}

// One step on: `ctx.get` reads the zoom under this track at the step's
// time -- the keyframes, else the fractal's own.
fn step(s, dt, p, ctx) {
    let a = p.stiffness * (ctx.get("zoom") - s.x) - p.damping * s.v;
    s.v += a * dt;
    s.x += s.v * dt;
    s
}

fn value(t, s, p, ctx) {
    #{ zoom: s.x }
}
