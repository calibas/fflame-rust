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
//   & ^       pitch down / up       \\ /   roll left / right   (3D!)
//   [ ]       start / end a branch
//   other letters (X, Y...) recurse without drawing
//
// Using any of & ^ \\ / switches to the 3D turtle automatically: the
// flame goes to 3D render mode and the camera tilts so the depth shows.
//
// SOME TO TRY
//   Fractal plant   axiom X   X=F-[[X]+X]+F[+FX]-X   F=FF   angle 22.5
//   Bush            axiom F   F=FF-[-F+F+F]+[+F-F-F]        angle 22.5
//   Sticks          axiom X   X=F[+X]F[-X]+X   F=FF         angle 20
//   Seaweed         axiom F   F=F[+FF][-FF]F[-F][+F]F       angle 25
//   3D tree         axiom X   X=F[\\+X][/-X][&X]FX   F=FF   angle 25

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
    // Any pitch or roll command switches to the 3D turtle: pieces come
    // back as full 3D matrices (a 2D segment cannot carry roll).
    let is_3d = lsystem_uses_3d(axiom, rules);

    let plant = #{};
    if is_3d {
        plant = lsystem_pieces3(axiom, rules, angle);
    } else {
        plant = lsystem_plant_pieces(axiom, rules, angle);
    }

    if plant.branches.len() == 0 {
        print("Could not build a plant from this rule.");
        if plant.note != "" { print(plant.note); }
    } else {
        let mirror_symbol = lsystem_mirror_symbol(rules, axiom);

        // Deepest bracket level, for colouring. 2D pieces store depth at
        // index 4, 3D pieces at index 12.
        let depth_at = if is_3d { 12 } else { 4 };
        let max_depth = 1.0;
        for b in plant.branches {
            if b[depth_at] > max_depth { max_depth = b[depth_at]; }
        }

        // Piece scale: 2D from the segment span, 3D from the matrix's
        // first column (the heading image — isotropic for branches).
        let piece_len = |q| {
            if is_3d {
                sqrt(q[0] * q[0] + q[3] * q[3] + q[6] * q[6])
            } else {
                let dx = q[2] - q[0];
                let dy = q[3] - q[1];
                sqrt(dx * dx + dy * dy)
            }
        };

        // Branch maps: each carries the whole plant onto one recursion
        // site. Weighted by size, so big limbs get their share of the
        // density and twigs stay wispy. (Mirror partners are folded into
        // the 3D matrices by the extraction; 2D applies the flag here.)
        for k in 0..plant.branches.len() {
            let b = plant.branches[k];
            let t = flame.add_transform();
            if is_3d {
                t.set_matrix3d(b, 1.0);
            } else {
                let mirrored = mirror_symbol != "" && segment_symbol(b) == mirror_symbol;
                t.set_segment(b, mirrored);
            }
            t.weight = piece_len.call(b) + 0.05;
            if colour_by_depth {
                // Trunk at the palette's start, deepest twigs at its end.
                t.color = b[depth_at] / max_depth;
            }
        }

        // Stem maps: squashed copies of the whole plant laid along each
        // drawn segment — the Barnsley rachis trick. Skipped entirely at
        // stem weight 0.
        if stem_weight > 0.0 {
            for k in 0..plant.stems.len() {
                let sgm = plant.stems[k];
                let t = flame.add_transform();
                if is_3d {
                    t.set_matrix3d(sgm, stem_thickness);
                } else {
                    t.set_segment(sgm, false, stem_thickness);
                }
                t.weight = stem_weight * (piece_len.call(sgm) + 0.05);
                if colour_by_depth {
                    t.color = sgm[depth_at] / max_depth;
                }
            }
        }

        // Frame on the plant's own bounding box; in 3D also switch the
        // render mode, keep z across iterations (matrix3D is useless
        // without it), and tilt the camera so the depth is visible.
        if is_3d {
            let box = lsystem_bounds3(axiom, rules, 6, angle);
            let extent = box[3] - box[0];
            if (box[4] - box[1]) > extent { extent = box[4] - box[1]; }
            if extent > 0.0 {
                config.set("pan_x", (box[0] + box[3]) * 0.5);
                config.set("pan_y", (box[1] + box[4]) * 0.5);
                config.set("zoom", 2.1 / extent);
            }
            config.set("render_mode", "3d");
            config.set("preserve_z", true);
            config.set("perspective_strength", 0.12);
            // Radians, not degrees — the View panel converts for display.
            config.set("camera_rotation_x", 30.0 * PI() / 180.0);
        } else {
            let box = lsystem_bounds(axiom, rules, 6, angle);
            let extent = box[2] - box[0];
            if (box[3] - box[1]) > extent { extent = box[3] - box[1]; }
            if extent > 0.0 {
                config.set("pan_x", (box[0] + box[2]) * 0.5);
                config.set("pan_y", (box[1] + box[3]) * 0.5);
                // Roomier margin than the curve script: the measured
                // bounds are the depth-6 drawing, and a plant's limit
                // attractor reaches somewhat beyond it.
                config.set("zoom", 2.1 / extent);
            }
        }

        print("Grew a plant: " + plant.branches.len() + " branch site(s), "
            + plant.stems.len() + " stem segment(s)."
            + if is_3d { " (3D)" } else { "" });
        if !is_3d {
            print("contractiveness: " + flame.contractiveness());
        }
    }
}
