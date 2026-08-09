// Zoom Dive
//
// Animates a zoom into the flame you have open — the "endless dive"
// every fractal video wants. Only movement; the flame is untouched.
//
// The one non-obvious thing this script knows: animation keys
// interpolate LINEARLY, but zoom is perceived as a RATE (each second
// should multiply the zoom by the same factor). A straight line from 1
// to 100 looks like slamming on the brakes — the first doubling takes a
// fifth of a second, the last one takes half the video. So this lays
// down a chain of keys along a geometric curve instead, and the dive
// feels constant-speed.
//
// Run it, then open the Animation panel and press play (or Space).

script("Zoom Dive", "modifier", ["norng"]);

let seconds = param("seconds", 12.0, 2.0, 120.0);
// Total magnification over the whole dive. 10 is a gentle push,
// 1000 is three orders of magnitude down the rabbit hole.
let factor = param("factor", 50.0, 2.0, 100000.0);
// Dive in, or pull back out.
let direction = param_choice("direction", ["Dive in", "Pull back"], 0);
// Return home at the end, so the video loops without a cut.
let and_back = param_bool("and_back", false);

// Start from the zoom the flame already has, so frame one is the image
// you are looking at. (Absent from the config means it's at the
// default, 1.0 — settings are only stored when they differ.)
let start = 1.0;
try { start = config.get("zoom"); } catch {}

let target = if direction == "Dive in" { start * factor } else { start / factor };

anim.name = "Zoom Dive";
anim.duration = seconds;

// The geometric chain. 16 segments per leg is enough that the
// linear-between-keys error is invisible (under 1% per segment at
// factor 1000).
let segs = 16;
let leg = if and_back { seconds / 2.0 } else { seconds };

for i in 0..(segs + 1) {
    let frac = i.to_float() / segs;
    // zoom_i = start * (target/start)^frac — equal RATIO per step.
    let z = start * (target / start) ** frac;
    anim.key("zoom", leg * frac, z, "linear");
}

if and_back {
    // Mirror the chain for the return leg. The last outbound key and
    // the first return key coincide at t = seconds/2.
    for i in 1..(segs + 1) {
        let frac = i.to_float() / segs;
        let z = start * (target / start) ** (1.0 - frac);
        anim.key("zoom", leg + leg * frac, z, "linear");
    }
}

print("zoom " + start + " -> " + target + " over " + (if and_back { leg } else { seconds }) + "s"
    + if and_back { ", then back — loops seamlessly" } else { "" });
print("open the Animation panel to play it (Space), or export a video");
