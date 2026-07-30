// Mutate
//
// Makes a variation on the flame you have open, the way Apophysis'
// mutation window does. Press Mutate to get a batch of them, then pick
// the one you like from the Fractal Browser — the original is included
// first so you can compare (and back out if none are better).
//
// Each result is named by its seed, so a mutation you liked can always
// be produced again from this script.

script("Mutate", "modifier");

// How far each mutation strays. Small values wander, large values jump.
let strength = param("strength", 0.35, 0.0, 1.0);

// What is allowed to change.
let mode = param_choice(
    "mode",
    ["Everything", "Shape", "Variations", "Weights", "Colours"],
    0,
);

// Keep the flame roughly as contractive as it started, so mutations
// explore SHAPE instead of drifting into an over-expanded haze. See
// flame.contractiveness() in the docs.
let keep_balance = param_bool("keep_balance", true);

// Variations a new one is drawn from. Picking from all 500+ mostly
// produces mud, so this is a deliberately tame pool — add your
// favourites, or swap in variation_names() if you want chaos.
let pool = [
    "spherical", "swirl", "horseshoe", "polar", "handkerchief", "heart",
    "disc", "spiral", "hyperbolic", "diamond", "julia", "bent",
    "waves", "fisheye", "popcorn", "exponential", "power", "cosine",
    "rings", "fan", "blob", "pdj", "curl", "ngon", "sinusoidal",
];

let all = mode == "Everything";
let balance_before = flame.contractiveness();

for i in 0..flame.transform_count() {
    let t = flame.transform(i);

    if all || mode == "Shape" {
        t.translate(rand(-strength, strength) * 0.5, rand(-strength, strength) * 0.5);
        t.rotate(rand(-strength, strength) * 45.0);
        t.scale(1.0 + rand(-strength, strength) * 0.4);
    }

    if all || mode == "Weights" {
        let w = t.weight * (1.0 + rand(-strength, strength) * 0.8);
        // Never let a transform vanish entirely — a zero-weight transform
        // stops contributing and the mutation looks like a deletion.
        if w < 0.05 { w = 0.05; }
        t.weight = w;
    }

    if all || mode == "Colours" {
        let c = t.color + rand(-strength, strength) * 0.5;
        while c < 0.0 { c += 1.0; }
        while c > 1.0 { c -= 1.0; }
        t.color = c;
    }

    if all || mode == "Variations" {
        // Nudge what's already there. add_variation() with an existing
        // name just sets its weight.
        for name in t.variation_names() {
            let w = t.variation_weight(name) * (1.0 + rand(-strength, strength) * 0.6);
            t.add_variation(name, w);
        }
        // Occasionally introduce something new. This is what makes
        // mutation explore rather than just jiggle.
        if chance(strength * 0.4) {
            t.add_variation(pick(pool), 0.1 + rand(0.0, 0.5) * strength);
        }
    }
}

if keep_balance && flame.transform_count() > 0 {
    flame.set_contractiveness(balance_before);
}
