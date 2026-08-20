// Procedural Palette (Inigo Quilez)
//
// Builds a palette from four cosine waves — Inigo Quilez's procedural
// palette formula, the one behind a great many shader gradients.
//
// THE FORMULA
//   colour(t) = a + b * cos(2*PI * (c*t + d))
//
// t runs 0..1 along the palette, and each of a, b, c and d is a triple:
// one value per channel, so red, green and blue are three independent
// cosine waves. That independence is the whole trick — three waves
// slightly out of step sweep through hues no single hue ramp produces.
//
//   a   Offset      the average colour: where the wave sits
//   b   Amplitude   how far it swings: contrast and saturation
//   c   Frequency   how many colour cycles fit in the palette
//   d   Phase       where each channel starts in its cycle
//
// The classic rainbow is a = b = 0.5, c = 1, d = (0, 0.33, 0.67): three
// identical waves pushed a third of a cycle apart, which is 120° — the
// same triadic relationship the Random Palette script rotates hues by,
// arrived at from the other direction.
//
// PRESETS
// The seven named presets are Quilez's own published sets. Custom uses
// the parameters below; the others ignore them and print their values,
// so a preset you like can be copied into Custom and adjusted.
//
// Random rolls its own set. It is seeded like everything else, so a roll
// you like comes back from the same seed — and Mutate in the Scripts
// panel runs consecutive seeds, which is the quickest way to see a
// batch and pick one.
//
// Frequencies above 1 repeat the palette, which a flame reads as banding
// along its colour axis — often more interesting than a single sweep.
// Non-integer frequencies do not join up end to end; that discontinuity
// is a feature if you want a hard edge in the palette.

script("Procedural Palette", "modifier", ["palette"]);

let preset = param_choice("preset", [
    "Rainbow", "Yellow-pink", "Green-blue", "Orange-blue",
    "Pastel", "Bright", "Dark earth", "Random", "Custom"
], 7);

let offset = param_color("offset", "#808080");
let amplitude = param_color("amplitude", "#808080");
let freq_r = param("freq_r", 1.0, 0.0, 6.0);
let freq_g = param("freq_g", 1.0, 0.0, 6.0);
let freq_b = param("freq_b", 1.0, 0.0, 6.0);
let phase_r = param("phase_r", 0.0, 0.0, 1.0);
let phase_g = param("phase_g", 0.33, 0.0, 1.0);
let phase_b = param("phase_b", 0.67, 0.0, 1.0);

// Quilez's published sets, as [a, b, c, d] with three values each.
let sets = #{
    "Rainbow":     [[0.5,0.5,0.5],[0.5,0.5,0.5],[1.0,1.0,1.0],[0.0,0.33,0.67]],
    "Yellow-pink": [[0.5,0.5,0.5],[0.5,0.5,0.5],[1.0,1.0,1.0],[0.0,0.10,0.20]],
    "Green-blue":  [[0.5,0.5,0.5],[0.5,0.5,0.5],[1.0,1.0,1.0],[0.3,0.20,0.20]],
    "Orange-blue": [[0.5,0.5,0.5],[0.5,0.5,0.5],[1.0,1.0,0.5],[0.8,0.90,0.30]],
    "Pastel":      [[0.5,0.5,0.5],[0.5,0.5,0.5],[1.0,0.7,0.4],[0.0,0.15,0.20]],
    "Bright":      [[0.5,0.5,0.5],[0.5,0.5,0.5],[2.0,1.0,0.0],[0.5,0.20,0.25]],
    "Dark earth":  [[0.8,0.5,0.4],[0.2,0.4,0.2],[2.0,1.0,1.0],[0.0,0.25,0.25]],
};

let a = [offset.r, offset.g, offset.b];
let b = [amplitude.r, amplitude.g, amplitude.b];
let c = [freq_r, freq_g, freq_b];
let d = [phase_r, phase_g, phase_b];

if preset == "Random" {
    // Offset and amplitude near a half: that is where the cosine uses
    // the whole 0..1 range without spending most of its swing clipped
    // flat at either end. Quilez's own sets nearly all sit there.
    let base_a = rand(0.42, 0.58);
    let base_b = rand(0.26, 0.46);
    a = [base_a + rand(-0.06, 0.06), base_a + rand(-0.06, 0.06), base_a + rand(-0.06, 0.06)];
    b = [base_b + rand(-0.08, 0.08), base_b + rand(-0.08, 0.08), base_b + rand(-0.08, 0.08)];

    // Whole-number frequencies join up end to end; a fraction leaves a
    // seam where the palette wraps. Both are useful, so weight the roll
    // towards whole numbers and let a fraction turn up sometimes.
    let f = pick([1.0, 1.0, 1.0, 2.0, 2.0, 3.0, 0.5, 1.5]);
    c = [f, f, f];
    if rand() < 0.45 {
        // Channels on different frequencies drift in and out of step,
        // which is what produces the sets that do not look like a plain
        // hue ramp.
        c = [f, f * pick([1.0, 0.5, 2.0]), f * pick([1.0, 0.5, 2.0])];
    }

    // Phase spread is what makes the hues travel: three waves in step
    // give a grey ramp, a third of a cycle apart gives the rainbow.
    let p0 = rand(0.0, 1.0);
    d = [p0, p0 + rand(0.12, 0.5), p0 + rand(0.12, 0.5) + rand(0.12, 0.5)];
} else if preset != "Custom" {
    let s = sets[preset];
    a = s[0]; b = s[1]; c = s[2]; d = s[3];
}

// Sample the formula at 256 evenly spaced points — the same layout the
// Palette Editor's Fixed mode uses, so what comes out is exactly what
// gets stored rather than a gradient re-interpolated behind your back.
let two_pi = 2.0 * PI();
let slots = [];
for i in 0..256 {
    let t = i.to_float() / 255.0;
    slots.push(color(
        a[0] + b[0] * cos(two_pi * (c[0] * t + d[0])),
        a[1] + b[1] * cos(two_pi * (c[1] * t + d[1])),
        a[2] + b[2] * cos(two_pi * (c[2] * t + d[2]))));
}

flame.set_palette_fixed("Procedural · " + preset, slots);

print("colour(t) = a + b * cos(2pi * (c*t + d))");
print("  a offset    " + a[0] + ", " + a[1] + ", " + a[2]);
print("  b amplitude " + b[0] + ", " + b[1] + ", " + b[2]);
print("  c frequency " + c[0] + ", " + c[1] + ", " + c[2]);
print("  d phase     " + d[0] + ", " + d[1] + ", " + d[2]);
if preset == "Random" {
    print("Rolled from seed " + seed() + " — copy the values above into Custom to keep them.");
} else if preset != "Custom" {
    print("Preset values — choose Custom to edit them.");
}
