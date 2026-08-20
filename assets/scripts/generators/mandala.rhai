// Mandala
//
// Builds a flame with exact N-fold rotational symmetry — the classic
// mandala / snowflake / rose-window look.
//
// The trick is worth knowing: symmetry does NOT come from placing N
// copies of everything. One extra transform whose affine is a pure
// rotation by 360/N does the whole job — the chaos game keeps passing
// points through it, which orbits every motif around the center. Add a
// mirror transform and the symmetry group doubles.

script("Mandala2", "generator");

// How many arms the mandala has.
let symmetry = param_int("symmetry", 6, 2, 16);
// Mirror as well — kaleidoscope symmetry instead of pure rotation.
let mirror = param_bool("mirror", true);
// How many distinct motifs orbit the center.
let motifs = param_int("motifs", 2, 1, 4);
// How far from the center the motifs sit.
let radius = param("radius", 0.9, 0.2, 2.0);
// A soft circular core at the center, drawn with julian.
let center_core = param_bool("center_core", true);
let palette_mode = param_choice("palette", ["Generated", "IQ Procedural", "Keep current"], 1);

// The motifs: small, varied, placed off-center. Their shapes are what
// the symmetry multiplies, so variety here is variety everywhere.
let pool = [
    "spherical", "swirl", "horseshoe", "polar", "heart", "disc",
    "spiral", "diamond", "julia", "fisheye", "rings", "fan", "bent",
];

let motif_weight_total = 0.0;
for i in 0..motifs {
    let t = flame.add_transform();
    t.add_variation("linear", rand(0.4, 0.9));
    t.add_variation(pick(pool), rand(0.3, 0.9));

    // Place it at a random angle and roughly `radius` out. The symmetry
    // transform will sweep it around the full circle regardless of
    // where it starts.
    let angle = rand(0.0, 360.0);
    let r = radius * rand(0.7, 1.1);
    let rad = angle * PI() / 180.0;
    t.translate(r * cos(rad), r * sin(rad));
    t.scale(rand(0.25, 0.5));
    t.rotate(rand(0.0, 360.0));

    t.weight = rand(0.5, 1.0);
	if (i > 0) {
    	t.color = (i + 1)/motifs;
	} else {
		t.color = 0.0;
	}
    motif_weight_total += t.weight;
}

if center_core {
    let t = flame.add_transform();
    // julian with a matching power makes a core that agrees with the
    // arm count instead of fighting it.
    t.add_variation("julian", rand(0.3, 0.6));
    t.set_variation_param("julian.power", symmetry);
    t.scale(rand(0.35, 0.6));
    t.weight = rand(0.3, 0.6);
    t.color = rand(0.0, 1.0);
    motif_weight_total += t.weight;
}

// THE symmetry transform: a pure rotation by 360/N, nothing else.
//
// Its weight matters: the chaos game picks transforms by weight, and
// every visit here moves a point one sector around. Matching the total
// motif weight means points spend about half their time orbiting —
// enough that all N sectors fill in evenly.
//
// color_speed = 1.0 keeps the palette position unchanged through the
// rotation (the color update is old*(1+s)/2 + this.color*(1-s)/2, so
// s=1 is "leave it alone"). Without this, every orbit drags all colors
// toward one value and the arms go monochrome.
let sym = flame.add_transform();
sym.add_variation("linear", 1.0);
let theta = 2.0 * PI() / symmetry;
sym.set_affine(cos(theta), -sin(theta), sin(theta), cos(theta), 0.0, 0.0);
sym.weight = motif_weight_total * 6;
sym.color_speed = 1.0;

if mirror {
    // Reflection across the X axis. Together with the rotation this
    // generates the dihedral group — 2N-fold kaleidoscope symmetry.
    let m = flame.add_transform();
    m.add_variation("linear", 1.0);
    m.set_affine(1.0, 0.0, 0.0, -1.0, 0.0, 0.0);
    m.weight = motif_weight_total * 0.5;
    m.color_speed = 1.0;
}

if palette_mode == "Generated" {
    run_script("random_palette", #{
        scheme: pick(["Analogous", "Complementary", "Triadic"]),
        base: "#000000",
        stops: 5,
        spread: 0.3,
        dark_end: true,
        shuffle_slices: 0,
        shuffle_amount: 0.0,
        hue_noise: 20.0,
        sat_noise: 0.1,
        val_noise: 0.15,
    }, seed());
}
if (palette_mode == "IQ Procedural") {
  run_script("iq_palette", #{
    preset: "Random"
  }, seed());
}

config.set("saturation", rand(1.0, 1.5));

let arms = if mirror { symmetry * 2 } else { symmetry };
print("mandala with " + symmetry + "-fold rotation" + if mirror { " + mirror (" + arms + " arms)" } else { "" });
print("the symmetry lives in ONE rotation transform — try changing its weight in the Transforms panel");
