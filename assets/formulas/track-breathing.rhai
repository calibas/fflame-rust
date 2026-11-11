// An animation track written in Rhai: a slow breath on the zoom and a
// gentle sway of the rotation, on top of whatever the keyframes say.
//
// "Use as track" puts it in the animation; its parameters are in the
// track editor (Edit on its row). "add" adds to the value under the
// track -- the keyframed value, else the fractal's own -- so moving the
// zoom yourself moves where it breathes from.

const PARAMS = #{
    depth: #{ value: 0.1, min: 0.0, max: 1.0, caption: "Depth", hint: "How far the zoom breathes in and out." },
    rate: #{ value: 0.25, min: 0.01, max: 4.0, caption: "Rate", hint: "Breaths a second." },
    sway: #{ value: 0.05, min: 0.0, max: 0.5, caption: "Sway", hint: "How far the rotation swings either way, in radians." },
};

const INFO = #{ title: "Breathing" };

// What it writes, and how: "set" replaces the value under it, "add"
// adds to it, "scale" multiplies it.
const TARGETS = #{ zoom: "add", rotation: "add" };

fn value(t, p, ctx) {
    let phase = 6.2831853 * p.rate * t;
    #{ zoom: p.depth * sin(phase), rotation: p.sway * sin(0.5 * phase) }
}
