// L-System Plant
//
// Grows a plant from a bracketed rewriting rule — the fern-and-bush side
// of L-systems, where [ and ] branch. Edit the axiom, rules and angle in
// the panel; rules paste in from any reference that uses the usual
// notation.
//
// HOW IT WORKS
// A plant rule is self-similar at its RECURSION SITES: the plant is its
// stem plus smaller copies of itself at every occurrence of the recursing
// symbol. That is exactly how Barnsley's fern works — each recursion site
// becomes a transform, and the drawn stem segments become SQUASHED
// transforms that lay a flattened copy of the whole plant along the stem
// (which is how the fern draws its rachis). The chaos game then grows the
// plant to infinite depth.
//
// SYMBOLS  (Prusinkiewicz & Lindenmayer, "The Algorithmic Beauty of
// Plants")
//   F G A B   draw forward          f g   move without drawing
//   + -       turn left / right     |     turn around
//   [ ]       start / end a branch
//   other letters (X, Y...) recurse without drawing
//
// SOME TO TRY
//   Fractal plant   axiom X   X=F-[[X]+X]+F[+FX]-X   F=FF   angle 22.5
//   Bush            axiom F   F=FF-[-F+F+F]+[+F-F-F]        angle 22.5
//   Sticks          axiom X   X=F[+X]F[-X]+X   F=FF         angle 20
//   Seaweed         axiom F   F=F[+FF][-FF]F[-F][+F]F       angle 25

script("L-System Plant", "generator");

let axiom = param_string("axiom", "X");
let rule_1 = param_string("rule_1", "X=F-[[X]+X]+F[+FX]-X");
let rule_2 = param_string("rule_2", "F=FF");
let rule_3 = param_string("rule_3", "");
let rule_4 = param_string("rule_4", "");
let angle = param("angle", 22.5, 0.0, 90.0);

// How much of the density goes to the stems, and how wide they spread.
// Thickness 0 is a pure line, like the fern's rachis; raising it leafs
// the stems out with a flattened copy of the whole plant.
let stem_weight = param("stem_weight", 0.35, 0.0, 2.0);
let stem_thickness = param("stem_thickness", 0.06, 0.0, 0.5);
let colour_by_depth = param_bool("colour_by_depth", true);

// Gather the non-blank rules. A rule is one symbol, "=", a replacement.
let rules = #{};
let bad = "";
for entry in [rule_1, rule_2, rule_3, rule_4] {
    let text = entry;
    text.trim();
    if text != "" {
        let eq = text.index_of("=");
        if eq < 1 {
            bad = text;
        } else {
            let symbol = text.sub_string(0, eq);
            symbol.trim();
            let replacement = text.sub_string(eq + 1);
            replacement.trim();
            if symbol.len() != 1 {
                bad = text;
            } else {
                rules[symbol] = replacement;
            }
        }
    }
}

if bad != "" {
    print("Could not read this rule: " + bad);
    print("A rule is one symbol, an equals sign, then its replacement — for example  X=F[+X]F[-X]+X");
} else if rules.is_empty() {
    print("No rules entered, so there is nothing to grow.");
} else {
    let plant = lsystem_plant_pieces(axiom, rules, angle);

    if plant.branches.len() == 0 {
        print("Could not build a plant from this rule.");
        if plant.note != "" { print(plant.note); }
    } else {
        let mirror_symbol = lsystem_mirror_symbol(rules, axiom);

        // Deepest bracket level, for colouring.
        let max_depth = 1.0;
        for b in plant.branches {
            if b[4] > max_depth { max_depth = b[4]; }
        }

        // Branch maps: each carries the whole plant onto one recursion
        // site. Weighted by size, so big limbs get their share of the
        // density and twigs stay wispy.
        for k in 0..plant.branches.len() {
            let b = plant.branches[k];
            let t = flame.add_transform();
            let mirrored = mirror_symbol != "" && segment_symbol(b) == mirror_symbol;
            t.set_segment(b, mirrored);
            let dx = b[2] - b[0];
            let dy = b[3] - b[1];
            let len = sqrt(dx * dx + dy * dy);
            t.weight = len + 0.05;
            if colour_by_depth {
                // Trunk at the palette's start, deepest twigs at its end.
                t.color = b[4] / max_depth;
            }
        }

        // Stem maps: squashed copies of the whole plant laid along each
        // drawn segment — the Barnsley rachis trick. Skipped entirely at
        // stem weight 0.
        if stem_weight > 0.0 {
            for k in 0..plant.stems.len() {
                let sgm = plant.stems[k];
                let t = flame.add_transform();
                t.set_segment(sgm, false, stem_thickness);
                let dx = sgm[2] - sgm[0];
                let dy = sgm[3] - sgm[1];
                t.weight = stem_weight * (sqrt(dx * dx + dy * dy) + 0.05);
                if colour_by_depth {
                    t.color = sgm[4] / max_depth;
                }
            }
        }

        // Frame on the plant's own bounding box.
        let box = lsystem_bounds(axiom, rules, 6, angle);
        let extent = box[2] - box[0];
        if (box[3] - box[1]) > extent { extent = box[3] - box[1]; }
        if extent > 0.0 {
            config.set("pan_x", (box[0] + box[2]) * 0.5);
            config.set("pan_y", (box[1] + box[3]) * 0.5);
            // Roomier margin than the curve script: the measured bounds
            // are the depth-6 drawing, and a plant's limit attractor
            // reaches somewhat beyond it (each copy sprouts copies).
            config.set("zoom", 2.1 / extent);
        }

        print("Grew a plant: " + plant.branches.len() + " branch site(s), "
            + plant.stems.len() + " stem segment(s).");
        print("contractiveness: " + flame.contractiveness());
    }
}
