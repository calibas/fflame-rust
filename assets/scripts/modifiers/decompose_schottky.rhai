// Decompose Schottky
//
// The `schottky_group` variation packs a whole group into one variation:
// it holds four Möbius transformations and picks one at random each
// iteration. This pulls them apart into four ordinary transforms, one per
// generator.
//
// Nothing about the picture changes. What changes is what you can DO with
// it — each generator now has its own colour, weight, animation tracks and
// triangle-editor handle, and the group's word rule becomes xaos.
//
// Open a flame using schottky_group, then run this.

script("Decompose Schottky", "modifier");

// The group's "never immediately undo the last generator" rule. Without
// it the walk backtracks constantly and the limit set fills in slowly.
let keep_word_rule = param_bool("keep_word_rule", true);

// Give each generator its own slice of the palette. The packed variation
// can only colour by its own rules; separate transforms can do anything.
let colour_generators = param_bool("colour_generators", true);

// Find the transform carrying the packed variation.
let source = -1;
for i in 0..flame.transform_count() {
    if flame.transform(i).has_variation("schottky_group") {
        source = i;
    }
}

if source < 0 {
    // Nothing to do — say so rather than silently producing an empty flame.
    print("No schottky_group variation found. Open a flame that uses it first.");
} else {
    let t = flame.transform(source);

    // Read the circles and marking straight off the packed variation, so
    // the decomposition matches whatever is on screen.
    let circles = [
        [t.variation_param("schottky_group.c1_x"), t.variation_param("schottky_group.c1_y"), t.variation_param("schottky_group.r1")],
        [t.variation_param("schottky_group.c2_x"), t.variation_param("schottky_group.c2_y"), t.variation_param("schottky_group.r2")],
        [t.variation_param("schottky_group.c3_x"), t.variation_param("schottky_group.c3_y"), t.variation_param("schottky_group.r3")],
        [t.variation_param("schottky_group.c4_x"), t.variation_param("schottky_group.c4_y"), t.variation_param("schottky_group.r4")],
    ];
    let twist_a = t.variation_param("schottky_group.twist_a");
    let twist_b = t.variation_param("schottky_group.twist_b");

    // [a, b, a-inverse, b-inverse] — the same order the packed variation
    // stores them in, which is what makes the word rule below line up.
    let gens = schottky_generators(circles, twist_a, twist_b);

    flame.clear_transforms();
    flame.clear_xaos();

    for k in 0..4 {
        let g = flame.add_transform();
        g.set_mobius(gens[k]);
        g.weight = 1.0;
        if colour_generators {
            // Four evenly spaced palette positions.
            g.color = k / 4.0;
        }
    }

    if keep_word_rule {
        for from in 0..4 {
            let row = avoid_xaos_row(from);
            for to in 0..4 {
                flame.set_xaos(from, to, row[to]);
            }
        }
    }

    print("Decomposed into 4 Möbius generators.");
}
