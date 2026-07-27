// Decompose Group
//
// Some variations pack a whole symmetry group into a single variation:
// they hold several Möbius transformations and pick one at random each
// iteration. This pulls them apart into ordinary transforms, one per
// generator.
//
// The picture stays the same. What changes is what you can DO with it —
// each generator gets its own colour, weight, animation tracks and
// triangle-editor handle, and the group's word rule becomes xaos.
//
// Works on: schottky_group, apollonian_gasket.
// Open a flame using one of those, then run this.

script("Decompose Group", "modifier");

// The "never immediately undo the last generator" rule. Without it the
// walk backtracks constantly and the limit set fills in slowly.
let keep_word_rule = param_bool("keep_word_rule", true);

// Give each generator its own slice of the palette — the thing the packed
// form can't do.
let colour_generators = param_bool("colour_generators", true);

// Find whichever packed group is present.
let source = -1;
let kind = "";
for i in 0..flame.transform_count() {
    let t = flame.transform(i);
    if t.has_variation("schottky_group") {
        source = i;
        kind = "schottky_group";
    } else if t.has_variation("apollonian_gasket") {
        source = i;
        kind = "apollonian_gasket";
    }
}

if source < 0 {
    print("No packed group found. Open a flame using schottky_group or apollonian_gasket.");
} else {
    let t = flame.transform(source);
    let gens = [];

    if kind == "schottky_group" {
        // Read the circles and marking straight off the variation, so the
        // decomposition matches whatever is on screen.
        let circles = [
            [t.variation_param("schottky_group.c1_x"), t.variation_param("schottky_group.c1_y"), t.variation_param("schottky_group.r1")],
            [t.variation_param("schottky_group.c2_x"), t.variation_param("schottky_group.c2_y"), t.variation_param("schottky_group.r2")],
            [t.variation_param("schottky_group.c3_x"), t.variation_param("schottky_group.c3_y"), t.variation_param("schottky_group.r3")],
            [t.variation_param("schottky_group.c4_x"), t.variation_param("schottky_group.c4_y"), t.variation_param("schottky_group.r4")],
        ];
        gens = schottky_generators(
            circles,
            t.variation_param("schottky_group.twist_a"),
            t.variation_param("schottky_group.twist_b"),
        );
    } else {
        gens = apollonian_generators(
            t.variation_param("apollonian_gasket.qc_deform") > 0.5,
            t.variation_param("apollonian_gasket.conj_angle"),
            t.variation_param("apollonian_gasket.conj_hyper"),
            t.variation_param("apollonian_gasket.qc_strength"),
        );
    }

    // Generators come back as [a, b, a-inverse, b-inverse] — the same
    // order the packed variation uses, which is what makes the word rule
    // below line up.
    flame.clear_transforms();
    flame.clear_xaos();

    for k in 0..gens.len() {
        let g = flame.add_transform();
        g.set_mobius(gens[k]);
        g.weight = 1.0;
        if colour_generators {
            // to_float() on both: k / gens.len() is INTEGER division, so
            // every generator would land on colour 0 (and render black).
            g.color = k.to_float() / gens.len().to_float();
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

    print("Decomposed " + kind + " into " + gens.len() + " Möbius generators.");
}
