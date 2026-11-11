// An animation track that smooths a jumpy input: here random jitter,
// but the same works for a signal -- an audio band, a MIDI control.
//
// Smoothing needs to know how much time passed, which a state's step
// gives: `dt`, always 1/240 s. Each step moves the smoothed value part of
// the way toward the input.

const PARAMS = #{
    amount: #{ value: 0.3, min: 0.0, max: 2.0, caption: "Amount", hint: "How far the smoothed jitter moves the zoom." },
    jitter: #{ value: 8.0, min: 0.5, max: 30.0, caption: "Jitter Rate", hint: "How many times a second the raw input jumps." },
    smoothing: #{ value: 0.4, min: 0.01, max: 3.0, caption: "Smoothing", hint: "Seconds the smoothed value takes to close about two thirds of a gap." },
};

const INFO = #{ title: "Smoothed Jitter" };

const TARGETS = #{ zoom: "add" };

fn start(p, ctx) {
    #{ y: 0.0 }
}

fn step(s, dt, p, ctx) {
    // The input at the step's time, in -1 to 1. To smooth a signal
    // instead, read it here, 0 to 1: let x = ctx.signal("bass");
    let x = noise(ctx.time * p.jitter);
    s.y += (x - s.y) * (1.0 - exp(-dt / p.smoothing));
    s
}

fn value(t, s, p, ctx) {
    #{ zoom: p.amount * s.y }
}
