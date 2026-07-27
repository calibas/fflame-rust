// Basic Random Flame
//
// A starting point for your own generators. Every line below is safe to
// change — try different numbers and re-run with the same seed to see
// exactly what each one does.
//
// Run it from the command line with:
//   fractal_flame_wgpu generate --script basic_random.rhai --seed 42

// Every script starts with its name and kind.
//   "generator" builds a flame from scratch
//   "modifier"  changes the flame you already have open
script("Basic Random", "generator");

// Values you can adjust before running. These show up as sliders.
//   param(name, default, lowest, highest)
let min_transforms = param_int("min_transforms", 2, 1, 10);
let max_transforms = param_int("max_transforms", 4, 1, 10);
let spread = param("spread", 1.0, 0.0, 3.0);
let curviness = param("curviness", 0.6, 0.0, 1.0);
let target_contraction = param("target_contraction", -0.25, -1.5, 0.5);
let add_plasma = param_bool("add_plasma", false);
let palette_mode = param_choice("palette", ["Keep current", "Random"], 0);

// How many transforms this flame gets. rand_int picks a whole number.
let count = rand_int(min_transforms, max_transforms);

for i in 0..count {
    let t = flame.add_transform();

    // Every transform needs at least one variation. "linear" is the
    // plainest one; the interesting shapes come from the others.
    t.add_variation("linear", 1.0);

    // Mix in a curved variation sometimes — this is what turns a pile of
    // triangles into a flame. Add names to the list to widen the palette
    // of shapes; the Variations panel lists every one available.
    if chance(curviness) {
        t.add_variation(pick(["spherical", "swirl", "julian", "horseshoe", "sinusoidal"]), rand(0.3, 1.0));
    }

    // How often this transform is chosen. Higher = more visible.
    t.weight = rand(0.5, 2.0);

    // Where it sits, and how big it is. Scaling ABOVE 1.0 is fine and
    // often interesting — a transform that expands is not a mistake.
    t.translate(rand(-spread, spread), rand(-spread, spread));
    t.scale(rand(0.4, 1.3));
    t.rotate(rand(0.0, 360.0));

    // Which part of the palette it draws from (0.0 to 1.0).
    t.color = rand(0.0, 1.0);
}

// Anything can be made conditional. chance(0.5) is true half the time.
if add_plasma && chance(0.5) {
    flame.add_effect("plasma");
}

// What decides whether a flame resolves into structure instead of haze
// is not any single transform, but the WEIGHTED AVERAGE across all of
// them: expansions have to be paid for by contractions elsewhere (or by
// being chosen rarely — weight is a probability). Below zero converges.
//
// Read it with flame.contractiveness(); this sets it, scaling every
// transform by one shared factor so each keeps its own character.
// Try 0.0 or a positive value to see the difference.
flame.set_contractiveness(target_contraction);

// Caveat worth knowing: this measures the affine part only. Curved
// variations change the picture completely — `spherical` keeps its
// output bounded no matter how expansive the affine feeding it is — so
// treat the number as a guide, not a verdict.

// Colours. Doing nothing keeps the palette you already have; you can
// also name one, or take a random one from the library. Random still
// reproduces from the seed, so a flame you like comes back exactly.
//   flame.set_palette("Fire");            // by name
//   flame.set_palette(pick(palette_names()));
if palette_mode == "Random" {
    flame.random_palette();
}

// Any setting from a saved .fflame file can be set by name — open one
// in a text editor to see what else there is.
config.set("brightness", 4.0);

print("built a flame with " + count + " transforms");
