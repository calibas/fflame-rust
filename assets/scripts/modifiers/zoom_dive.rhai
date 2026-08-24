// Zoom Dive
//
// Animates a zoom into the flame you have open — the "endless dive"
// every fractal video wants. Only movement; the flame is untouched.
//
// The one non-obvious thing this script knows: zoom is perceived as a
// RATE — each second should MULTIPLY the zoom by the same factor. A
// linear track from 1 to 100 looks like slamming on the brakes: the
// first doubling takes a fifth of a second, the last one takes half
// the video. The track is therefore set to "exponential"
// interpolation, which moves by equal ratio per unit time — and works
// the same zooming out as in.
//
// Run it, then open the Animation panel and press play (or Space).

script("Zoom Dive Animation", "modifier", ["norng"]);

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

// Equal ratio per unit time — see the header. Two keys per leg is all
// the exponential track needs.
anim.interpolation("zoom", "exponential");

let leg = if and_back { seconds / 2.0 } else { seconds };
anim.key("zoom", 0.0, start, "linear");
anim.key("zoom", leg, target, "linear");
if and_back {
    // The return leg: same curve run backward, landing exactly on the
    // start value so the loop closes without a cut.
    anim.key("zoom", seconds, start, "linear");
}

print("zoom " + start + " -> " + target + " over " + (if and_back { leg } else { seconds }) + "s"
    + if and_back { ", then back — loops seamlessly" } else { "" });
print("open the Animation panel to play it (Space), or export a video");
