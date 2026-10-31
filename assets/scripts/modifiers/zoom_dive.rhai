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
// ESCAPE-TIME FRACTALS ZOOM ON A DIFFERENT SETTING, and the difference
// is not just the name. A flame's zoom is a multiplier; an escape
// view's Escape.ZoomLog10 is that multiplier's EXPONENT — zoom 9
// means 10^9, as the View panel shows it. Equal ratio per unit time is
// therefore equal INCREMENT per unit time here, so this leg takes a
// LINEAR track and adds log10(factor) rather than multiplying. Running
// the flame's exponential curve over an exponent would accelerate
// absurdly: a 50x dive from zoom 6 would end at 10^300.
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

// Which engine is open decides which setting moves. `render_mode` is
// always stored, but read it defensively: a hand-written config could
// be missing anything.
let is_escape = false;
try { is_escape = config.get("render_mode") == "escape"; } catch {}

let track = "zoom";
let curve = "exponential";
let start = 1.0;
let target = 1.0;

if is_escape {
    track = "Escape.ZoomLog10";
    // Linear, because the value is already an exponent — see the
    // header.
    curve = "linear";
    // `escape.zoom` is the engine's base 2; the track is base 10.
    start = escape.zoom * ln(2.0) / ln(10.0);
    // log10(factor): Rhai has ln, so divide by ln 10.
    let decades = ln(factor) / ln(10.0);
    target = if direction == "Dive in" { start + decades } else { start - decades };
    // The view stops meaning anything below about 10^-2.4 (2^-8: the
    // whole set is a speck by then), and the app clamps there anyway.
    // Clamping HERE keeps the loop closing on the value it started from.
    let floor = -8.0 * ln(2.0) / ln(10.0);
    if target < floor { target = floor; }
} else {
    // Start from the zoom the flame already has, so frame one is the
    // image you are looking at. (Absent from the config means it's at
    // the default, 1.0 — settings are only stored when they differ.)
    try { start = config.get("zoom"); } catch {}
    target = if direction == "Dive in" { start * factor } else { start / factor };
}

anim.name = "Zoom Dive";
anim.duration = seconds;

// Equal ratio per unit time — see the header. Two keys per leg is all
// either track needs.
anim.interpolation(track, curve);

let leg = if and_back { seconds / 2.0 } else { seconds };
anim.key(track, 0.0, start, "linear");
anim.key(track, leg, target, "linear");
if and_back {
    // The return leg: same curve run backward, landing exactly on the
    // start value so the loop closes without a cut.
    anim.key(track, seconds, start, "linear");
}

print((if is_escape { "escape zoom 10^" } else { "zoom " }) + start
    + " -> " + (if is_escape { "10^" } else { "" }) + target
    + " over " + (if and_back { leg } else { seconds }) + "s"
    + if and_back { ", then back — loops seamlessly" } else { "" });
print("open the Animation panel to play it (Space), or export a video");
