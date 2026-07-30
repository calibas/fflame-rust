// Random Palette
//
// Builds a palette from a colour-theory scheme and puts it on the flame
// you have open. Run it on its own, press Batch for a set to choose
// from, or call it from another script with run_script("random_palette").
//
// HOW IT WORKS
// A scheme is a set of hue relationships to one base colour, which is
// why the colours are mixed in HSV: complementary is the hue opposite
// (+180), triadic is the two a third of the way round (+120, +240),
// analogous is the near neighbours. Saturation and value are jittered a
// little around the base so the result reads as one family rather than
// a swatch chart.
//
// The palette is then converted to FIXED mode — 256 evenly spaced slots
// instead of a handful of gradient stops. That is what makes the last
// two stages simple: with every slot the same width, slicing the palette
// up is array work, and noise lands evenly instead of bunching wherever
// the stops happened to sit.
//
// SHUFFLE
// Cuts the palette into slices and reorders them. A flame reads the
// palette along its colour axis, so shuffling breaks the smooth sweep
// into bands that repeat and jump — the difference between a gradient
// and something with structure. Amount controls how many slices actually
// move, so it runs from a single swapped pair up to a full reshuffle.
//
// NOISE
// Jitters each slot in H, S and V separately. Hue noise is in degrees;
// the other two are fractions. Separate axes matter: nudging hue keeps
// the brightness structure and scatters the colour, while nudging value
// keeps the colours and roughens the light. The same jitter applied to
// R, G and B would just make mud.
//
// The base colour is random unless you pick one.

script("Random Palette", "modifier", ["palette"]);

let scheme = param_choice("scheme", [
    "Complementary", "Analogous", "Triadic", "Split complementary", "Monochromatic"
], 0);
let base_in = param_color("base", "#000000");
let stops = param_int("stops", 5, 2, 12);
let spread = param("spread", 0.35, 0.0, 1.0);
let dark_end = param_bool("dark_end", true);

let slices = param_int("shuffle_slices", 1, 1, 64);
let shuffle_amount = param("shuffle_amount", 1.0, 0.0, 1.0);

let hue_noise = param("hue_noise", 0.0, 0.0, 180.0);
let sat_noise = param("sat_noise", 0.0, 0.0, 1.0);
let val_noise = param("val_noise", 0.0, 0.0, 1.0);

// A black default means "surprise me" — a real base colour is unlikely
// to be pure black, and it keeps the parameter honest without needing a
// separate "randomise?" checkbox.
let base = base_in;
if base.h == 0.0 && base.s == 0.0 && base.v == 0.0 {
    base = color_hsv(rand(0.0, 360.0), rand(0.55, 1.0), rand(0.7, 1.0));
}

// Hue offsets that define each scheme.
let offsets = [0.0, 180.0];
if scheme == "Analogous" {
    offsets = [-30.0, 0.0, 30.0];
} else if scheme == "Triadic" {
    offsets = [0.0, 120.0, 240.0];
} else if scheme == "Split complementary" {
    offsets = [0.0, 150.0, 210.0];
} else if scheme == "Monochromatic" {
    offsets = [0.0];
}

let colors = [];
if dark_end {
    // A dark first stop gives the tone mapping somewhere to fall away
    // to; without it low-density regions read as flat colour.
    colors.push(color_hsv(base.h, base.s * 0.6, 0.04));
}
for i in 0..stops {
    let hue = base.h + offsets[i % offsets.len()] + rand(-8.0, 8.0) * spread;
    let sat = base.s + rand(-0.3, 0.15) * spread;
    let val = base.v + rand(-0.35, 0.1) * spread;
    colors.push(color_hsv(hue, sat, val));
}

let title = "Generated · " + scheme;

// Lay it out as 256 even slots before touching it further.
flame.set_palette_colors(title, colors);
flame.palette_to_fixed();
let slots = flame.palette_colors();

// ---- shuffle: cut into slices, reorder some of them ----
if slices > 1 && shuffle_amount > 0.0 {
    let n = slots.len();
    let size = n / slices;

    // Build the slice order, then swap pairs within it. Swapping a
    // fraction of the slices — rather than shuffling the lot and
    // blending back — is what makes Amount read as "how disturbed is
    // this", with a single swapped pair at the bottom of the range.
    let order = [];
    for i in 0..slices { order.push(i); }
    let swaps = (shuffle_amount * slices.to_float()).to_int();
    if swaps < 1 { swaps = 1; }
    for k in 0..swaps {
        let i = rand_int(0, slices - 1);
        let j = rand_int(0, slices - 1);
        let tmp = order[i];
        order[i] = order[j];
        order[j] = tmp;
    }

    let shuffled = [];
    for k in 0..slices {
        let src = order[k] * size;
        for o in 0..size {
            shuffled.push(slots[src + o]);
        }
    }
    // Integer division can leave a few slots over; keep the tail so the
    // palette stays its full length rather than quietly shrinking.
    while shuffled.len() < n {
        shuffled.push(slots[shuffled.len()]);
    }
    slots = shuffled;
}

// ---- noise: jitter each slot in H, S and V ----
if hue_noise > 0.0 || sat_noise > 0.0 || val_noise > 0.0 {
    let noisy = [];
    for k in 0..slots.len() {
        let c = slots[k];
        noisy.push(color_hsv(
            c.h + rand(-hue_noise, hue_noise),
            c.s + rand(-sat_noise, sat_noise),
            c.v + rand(-val_noise, val_noise)));
    }
    slots = noisy;
}

flame.set_palette_fixed(title, slots);

print(scheme + " on " + base + " — " + slots.len() + " slots");
if slices > 1 && shuffle_amount > 0.0 {
    print("shuffled " + slices + " slices");
}
if hue_noise > 0.0 || sat_noise > 0.0 || val_noise > 0.0 {
    print("noise  H " + hue_noise + "  S " + sat_noise + "  V " + val_noise);
}
