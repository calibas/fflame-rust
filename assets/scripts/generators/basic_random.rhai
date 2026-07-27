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
let add_plasma = param_bool("add_plasma", false);

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

    // Where it sits, and how big it is.
    t.translate(rand(-spread, spread), rand(-spread, spread));
    // Keep this BELOW 1.0. Each transform has to shrink the picture, or
    // the points race off to infinity and you get a formless haze instead
    // of a fractal. (This is the one rule worth not breaking.)
    t.scale(rand(0.3, 0.8));
    t.rotate(rand(0.0, 360.0));

    // Which part of the palette it draws from (0.0 to 1.0).
    t.color = rand(0.0, 1.0);
}

// Anything can be made conditional. chance(0.5) is true half the time.
if add_plasma && chance(0.5) {
    flame.add_effect("plasma");
}

// Any setting from a saved .fflame file can be set by name — open one
// in a text editor to see what else there is.
config.set("brightness", 4.0);

print("built a flame with " + count + " transforms");
