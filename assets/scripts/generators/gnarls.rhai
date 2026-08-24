// Gnarl
//
// The classic Apophysis "gnarl" recipe: organic, curling, coral-like
// tendrils. The whole effect is one idea — a transform that repeats
// almost exactly (tiny rotation, tiny shrink) with a low-frequency
// ripple (waves2) folded in. Iterating that thousands of times turns
// straight edges into curls.
//


script("Gnarls", "generator");

// The seed shape the gnarl chews on (also frames the image).
let shape = param_choice("shape", ["Random", "Julian", "Eyefish", "Bubble"], 0);
let palette_mode = param_choice("palette", ["Generated", "IQ Procedural", "Keep current"], 1);

let scale_max = param("scale_max", 0.13, 0.0, 2.0);
let freq_max = param("freq_max", 6.0, -100.0, 100.0);
let random_rotation = param_bool("random_rotation", true);

let t1 = flame.add_transform();
t1.color = 0.0;
t1.color_speed = rand(0.8,1.0);
t1.weight = rand(5.0,50.0);
if (random_rotation && chance(0.2)) {
	t1.rotate(rand(-1.0,1.0));
}
if (random_rotation && chance(0.2)) {
	t1.post_affine_enabled = true;
	t1.post_a = rand(0.95,1.05);
	t1.post_b = rand(-0.05,0.05);
	t1.post_c = rand(-0.05,0.05);
	t1.post_d = rand(0.95,1.05);
}
if (random_rotation && chance(0.2)) {
	t1.rotate(pick([90.0,180.0,-90.0]));
}
t1.add_variation("waves2", rand(0.99,0.999));
t1.set_variation_param("waves2","freqx",rand(-freq_max,freq_max));
t1.set_variation_param("waves2","freqy",rand(-freq_max,freq_max));
t1.set_variation_param("waves2","scalex",rand(-scale_max,scale_max));
t1.set_variation_param("waves2","scaley",rand(-scale_max,scale_max));

// The seed shape: keeps re-injecting points into a compact region for
// the gnarl map to chew on, and bounds the image. Scaled well down so
// the interesting curls happen inside the frame.
if shape == "Random" {
	shape = pick(["Julian", "Eyefish", "Bubble"]);
}
let t = flame.add_transform();
if shape == "Julian" {
    t.add_variation("julian", rand(0.5, 1.5));
    t.set_variation_param("julian.power", rand_int(2, 5));
} else if shape == "Eyefish" {
    t.add_variation("eyefish", rand(0.5, 1.5));
} else {
    t.add_variation("bubble", rand(0.5, 1.5));
}
t.scale(rand(0.5, 1.5));
t.rotate(rand(0.0, 360.0));
if (chance(0.4)) {
	t.translate(rand(-2.0,2.0),rand(-2.0,2.0));
}
t.weight = 1.0;
t.color = rand(0.9, 1.0);

if palette_mode == "Generated" {
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
if (palette_mode == "IQ Procedural") {
  run_script("iq_palette", #{
    preset: "Random"
  }, seed());
}

config.set("saturation", rand(1.0, 1.5));
config.set("zoom", rand(0.5,1.0));