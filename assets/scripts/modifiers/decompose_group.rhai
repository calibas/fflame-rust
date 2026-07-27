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
// Works on: schottky_group, apollonian_gasket, sphere_packing,
// klein_group.
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
    } else if t.has_variation("sphere_packing") {
        source = i;
        kind = "sphere_packing";
    } else if t.has_variation("klein_group") {
        source = i;
        kind = "klein_group";
    }
}

if source < 0 {
    print("No packed group found. Open a flame using schottky_group, apollonian_gasket, sphere_packing or klein_group.");
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
    } else if kind == "sphere_packing" {
        gens = [];   // filled below; packings emit inversions, not matrices
    } else if kind == "klein_group" {
        // Indra's Pearls: two generators and their inverses, built by one
        // of seven recipes. Order here is [a, a-inverse, b, b-inverse] —
        // NOT the Schottky order — because the word rule keys off it.
        gens = klein_generators(
            t.variation_param("klein_group.recipe").to_int(),
            t.variation_param("klein_group.a_re"),
            t.variation_param("klein_group.a_im"),
            t.variation_param("klein_group.b_re"),
            t.variation_param("klein_group.b_im"),
            t.variation_weight("klein_group"),
        );
    } else {
        gens = apollonian_generators(
            t.variation_param("apollonian_gasket.qc_deform") > 0.5,
            t.variation_param("apollonian_gasket.conj_angle"),
            t.variation_param("apollonian_gasket.conj_hyper"),
            t.variation_param("apollonian_gasket.qc_strength"),
        );
    }

    if kind == "sphere_packing" {
        // A packing is a REFLECTION group: each generator is an inversion
        // in one sphere, not a Möbius matrix. Inversions are their own
        // inverse, so the word rule blocks repeating a mirror rather than
        // following one with its opposite.
        let mirrors = sphere_packing_mirrors(
            t.variation_param("sphere_packing.mode").to_int(),
            t.variation_param("sphere_packing.size"),
            t.variation_param("sphere_packing.ring_n").to_int(),
            t.variation_param("sphere_packing.ring_scale"),
            t.variation_param("sphere_packing.cap_scale"),
            t.variation_param("sphere_packing.size_jitter"),
            t.variation_param("sphere_packing.ring_tilt"),
            config.get("render_mode") == "3d",
        );

        flame.clear_transforms();
        flame.clear_xaos();

        for k in 0..mirrors.len() {
            let m = mirrors[k];
            let g = flame.add_transform();
            g.set_inversion([m[0], m[1], m[2]], m[3]);
            g.weight = 1.0;
            if colour_generators {
                g.color = k.to_float() / mirrors.len().to_float();
            }
        }

        if keep_word_rule {
            for from in 0..mirrors.len() {
                let row = repeat_xaos_row(from, mirrors.len());
                for to in 0..mirrors.len() {
                    flame.set_xaos(from, to, row[to]);
                }
            }
        }

        print("Decomposed " + kind + " into " + mirrors.len() + " sphere inversions.");
    } else {
        // Generators come back as [a, b, a-inverse, b-inverse] — the same
        // order the packed variation uses, which is what makes the word
        // rule below line up.
        flame.clear_transforms();
        flame.clear_xaos();

        for k in 0..gens.len() {
            let g = flame.add_transform();
            g.set_mobius(gens[k]);
            g.weight = 1.0;
            if colour_generators {
                // to_float() on both: k / gens.len() is INTEGER division,
                // so every generator would land on colour 0 (render black).
                g.color = k.to_float() / gens.len().to_float();
            }
        }

        if keep_word_rule {
            for from in 0..4 {
                // Three groups, three different "don't backtrack" rules.
                // klein_group excludes the inverse and draws uniformly from
                // the rest; the others redraw into the next generator, which
                // doubles its share. avoid_xaos_row encodes the latter.
                let row = [];
                if kind == "klein_group" {
                    // [a, a-inverse, b, b-inverse]: the inverse is from ^ 1.
                    row = exclude_xaos_row(from ^ 1, 4);
                } else {
                    row = avoid_xaos_row(from);
                }
                for to in 0..4 {
                    flame.set_xaos(from, to, row[to]);
                }
            }
        }

        print("Decomposed " + kind + " into " + gens.len() + " Möbius generators.");
    }
}
