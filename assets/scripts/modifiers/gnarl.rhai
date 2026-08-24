// Make Gnarly
//
// Adds one heavily weighted waves2 transform to the flame you already
// have — the classic "gnarl" treatment. Random wave frequencies and
// scales (bounded by the sliders) bend space into flowing, dendritic
// wisps; random_rotation sometimes tilts the wave or adds a slight
// post-affine skew for variety. Works best on flames with strong
// structure to distort.
script("Gnarl", "modifier");

// Values you can adjust before running. These show up as sliders.
//   param(name, default, lowest, highest)
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