// Gnarl
//
// The classic Apophysis "gnarl" recipe: organic, curling, coral-like
// tendrils. The whole effect is one idea — a transform that repeats
// almost exactly (tiny rotation, tiny shrink) with a low-frequency
// ripple (waves2) folded in. Iterating that thousands of times turns
// straight edges into curls.
//
// Small parameter changes swing the result a lot; that's the point.
// Re-run the same seed while dragging the sliders to watch one flame
// deform continuously.

script("Gnarl", "generator");

// Ripple frequency: how many waves fit across the frame. Low = broad
// swells, high = fine crinkles.
let frequency = param("frequency", 8.0, 0.5, 16.0);
// Ripple depth. The sweet spot is subtle — past ~0.15 the curls tear
// into spray.
let depth = param("depth", 0.09, 0.005, 0.3);
// Degrees the main transform turns per iteration. The curl driver:
// 0 gives straight ripples, more gives tighter spirals.
let twist = param("twist", 6.0, -15.0, 15.0);
// How hard each iteration contracts. Closer to 1.0 = longer tendrils
// (and slower convergence).
let shrink = param("shrink", 0.99, 0.9, 1.0);
// A second gnarl layer at a detuned frequency, for interference.
let layers = param_int("layers", 2, 1, 3);
// The seed shape the gnarl chews on (also frames the image).
let shape = param_choice("shape", ["Julian", "Eyefish", "Bubble"], 0);
let palette_mode = param_choice("palette", ["Generated", "Keep current"], 0);

// The gnarl transform must DOMINATE: what makes tendrils is a point
// staying inside this map for dozens of consecutive iterations, each
// one nudging it along a curl. Weight is that residence time. At 4:1
// odds a run averages ~4 steps and the result is fuzz; at 20:1 runs
// are long enough to draw whole filaments.
let gnarl_weight = 20.0;

for i in 0..layers {
    let t = flame.add_transform();

    // The gnarl core is waves2 ALONE. Its formula is x + scale*sin(y*freq)
    // — the identity is already inside it, so adding linear on top would
    // double every point per iteration and blow the attractor apart
    // (measured: that renders as a huge near-black haze). The affine
    // carries the twist and shrink; waves2 carries the ripple. NO
    // translation — the layers must share the origin as their fixed
    // point or they smear each other out.
    t.add_variation("waves2", 1.0);
    // Layers get slightly different frequencies — the interference
    // between 3.0 and 3.7 waves is where the organic look comes from.
    let f = frequency * (1.0 + 0.23 * i);
    t.set_variation_param("waves2.freqx", f);
    t.set_variation_param("waves2.freqy", f * rand(0.8, 1.25));
    t.set_variation_param("waves2.scalex", depth);
    t.set_variation_param("waves2.scaley", depth * rand(0.7, 1.3));

    t.rotate(twist + rand(-1.0, 1.0));
    t.scale(shrink);

    // The first layer dominates; extra layers season it.
    t.weight = if i == 0 { gnarl_weight } else { rand(1.0, 3.0) };
    t.color = rand(0.0, 1.0);
    // Slow color drift along the tendrils rather than a hard jump per
    // iteration — gnarls read best with long smooth gradients.
    t.color_speed = 0.9;
}

// The seed shape: keeps re-injecting points into a compact region for
// the gnarl map to chew on, and bounds the image. Scaled well down so
// the interesting curls happen inside the frame.
let t = flame.add_transform();
if shape == "Julian" {
    t.add_variation("julian", rand(0.5, 0.8));
    t.set_variation_param("julian.power", rand_int(2, 5));
} else if shape == "Eyefish" {
    t.add_variation("eyefish", rand(0.5, 0.8));
} else {
    t.add_variation("bubble", rand(0.5, 0.8));
}
t.scale(rand(0.4, 0.6));
t.rotate(rand(0.0, 360.0));
t.weight = 1.0;
t.color = rand(0.0, 1.0);

if palette_mode == "Generated" {
    // Gnarls suit tight, moody palettes — one hue family, dark end.
    run_script("random_palette", #{
        scheme: pick(["Monochromatic", "Analogous"]),
        base: "#000000",
        stops: 4,
        spread: 0.25,
        dark_end: true,
        shuffle_slices: 0,
        shuffle_amount: 0.0,
        hue_noise: 15.0,
        sat_noise: 0.15,
        val_noise: 0.25,
    }, seed());
}

config.set("brightness", 4.0);
config.set("gamma", 4.0);

print("gnarl: " + layers + " layer(s), frequency " + frequency + ", twist " + twist + "°");
print("drag `depth` and `twist` and re-run — small changes travel a long way here");
