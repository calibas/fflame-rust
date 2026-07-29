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
// The base colour is random unless you pick one.

script("Random Palette", "modifier", ["palette"]);

let scheme = param_choice("scheme", [
    "Complementary", "Analogous", "Triadic", "Split complementary", "Monochromatic"
], 0);
let base_in = param_color("base", "#000000");
let stops = param_int("stops", 5, 2, 12);
let spread = param("spread", 0.35, 0.0, 1.0);
let dark_end = param_bool("dark_end", true);

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

flame.set_palette_colors("Generated · " + scheme, colors);
print(scheme + " on " + base + " — " + colors.len() + " stops");
