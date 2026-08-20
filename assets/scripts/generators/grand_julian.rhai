// Grand JuliaN Random Generator
//
script("Grand JuliaN Random", "generator");

// Values you can adjust before running. These show up as sliders.
//   param(name, default, lowest, highest)
let target_contraction = param("target_contraction", 0.0, -2.0, 2.0);
let palette_scheme = param_choice("palette_scheme", [
    "Monochromatic", "Complementary", "Analogous", "Triadic", "Split complementary", "IQ Procedural"
], 5);
let intensity = param("intensity", 1.0, 0.01, 10.0);

config.set("rotation", pick([0.7854, -0.7854]));
config.set("saturation", rand(1.0, 1.3));
if (chance(0.2)) {
    config.set("zoom", rand(1.3,2.0));
    config.set("pan_x", rand(-1.0,1.0));
    config.set("pan_y", rand(-1.0,1.0));
}

let t1 = flame.add_transform();
t1.color = 0.0;
let t1_blur_size = rand(0.2, 0.45) * intensity;
if (chance(0.9)) {
	if (chance(0.5)) {
		t1.add_variation("blur", t1_blur_size);
	} else {
		t1.add_variation("bubble", t1_blur_size);
		t1.add_variation("pre_blur", rand(0.1,3.0));
	}
} else {
	if (chance(0.5)) {
		t1.add_variation("pie3D", t1_blur_size);
    	t1.set_variation_param("pie3D","slices",rand_int(1,10));
		t1.set_variation_param("pie3D","thickness",rand(0.1,0.9));
		t1.set_variation_param("pie3D","rotation",rand_int(0,360));
	} else {
		t1.add_variation("starblur", t1_blur_size);
    	t1.set_variation_param("starblur","power",rand_int(2,15));
		t1.set_variation_param("starblur","range",rand(0.1,0.9));
	}
}

let t2 = flame.add_transform();
t2.color = 1.0;
t2.color_speed = rand(0.2,0.8);
t2.weight = rand(5.0, 20.0);
t2.add_variation("julian", 1.0);
t2.set_variation_param("julian", "power", 2);
t2.set_variation_param("julian", "dist", -1.0);
if (chance(0.8)) {
	t2.rotate(-45.0);
} else {
	t2.rotate(rand(0.0,360.0));
}
t2.translate(0.0, rand(-1.0,-0.2) * intensity);

if (chance(0.75)) {
    let t3 = flame.add_transform();
    t3.color = rand(0.25, 0.75);
    t3.color_speed = -0.5;
    t3.add_variation("julian", t1_blur_size);
    t3.set_variation_param("julian", "power", 22);
    t3.set_variation_param("julian", "dist", -1.0);
    if (chance(0.5)) {
        let t4 = flame.add_transform();
        t4.color = rand(0.25, 0.75);
        t4.color_speed = -0.5;
        t4.add_variation("julian", t1_blur_size * rand(1.1,1.3));
        t4.set_variation_param("julian", "power", 12);
        t4.set_variation_param("julian", "dist", -1.0);
    }
}

if (chance(0.25)) {
    let f1 = flame.add_final_transform();
    f1.add_variation("bipolar", 1.0);
	f1.set_variation_param("bipolar","shift",rand(0.0,2.0));
	f1.attach_to_all();
	config.set("rotation", 1.57);
	config.set("zoom", 2.0);
    config.set("pan_x", 0.0);
	config.set("pan_y", 0.0);
}

if (palette_scheme != "IQ Procedural") {
  run_script("random_palette", #{
    scheme: palette_scheme,
    base: "#000000",          // black means "pick one for me"
    stops: 5,
    spread: 0.35,
    dark_end: true,
    shuffle_slices: 5,
    shuffle_amount: 1.0,
    hue_noise: 10.0,
    sat_noise: 0.0,
    val_noise: 0.2,
  }, seed());
} else {
  run_script("iq_palette", #{
    preset: "Random"
  }, seed());
}

flame.set_contractiveness(target_contraction);