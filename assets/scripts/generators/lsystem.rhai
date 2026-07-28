// L-System Curve
//
// Turns a rewriting rule into a flame. Edit the axiom, rules and angle in
// the panel — you should not need to touch the script.
//
// HOW IT WORKS
// An L-system grows a curve by repeatedly replacing each edge with a
// small path. Take just ONE generation of that replacement, shrink it
// back onto the edge it replaced, and each piece becomes a transform.
// The chaos game then draws the curve those transforms agree on, to
// infinite depth, for free. The rule becomes the transforms.
//
// SYMBOLS  (Prusinkiewicz & Lindenmayer, "The Algorithmic Beauty of
// Plants" — the convention most sources follow)
//   F G A B   draw forward
//   f g       move forward WITHOUT drawing
//   + -       turn left / right by Angle
//   |         turn around
//   [ ]       start / end a branch
//   any other letter is ignored by the turtle and only drives rewriting
//
// Sources disagree about which letters draw. If you copy a rule that uses
// lower-case f and g as DRAWING symbols, upper-case them — otherwise
// nothing is drawn and the script will tell you so.
//
// RULE FORMAT
//   symbol=replacement        e.g.  F=F+F--F+F
// Leave a rule blank if you do not need it.
//
// WHAT THIS CANNOT DO
// The construction needs an EDGE-rewriting rule: one generation must
// replace a single edge with a shorter path. Space-filling curves —
// Hilbert, Peano — rewrite NODES instead, and their pieces come out the
// same length as the edge they replace. Nothing shrinks, the chaos game
// has nothing to converge to, and the script says so.
//
// SOME TO TRY
//   Koch          axiom F   F=F+F--F+F                 angle 60
//   Koch square   axiom F   F=F+F-F-F+F                angle 90
//   Koch island   axiom F   F=F+F-F-FF+F+F-F           angle 90
//   Dragon        axiom F   F=F+G   G=F-G              angle 90
//   Sierpinski    axiom F   F=+G-F-G+   G=-F+G+F-      angle 60
//   Levy C        axiom F   F=+F--F+                   angle 45

script("L-System Curve", "generator");

let axiom = param_string("axiom", "F");
let rule_1 = param_string("rule_1", "F=F+F--F+F");
let rule_2 = param_string("rule_2", "");
let rule_3 = param_string("rule_3", "");
let rule_4 = param_string("rule_4", "");
let rule_5 = param_string("rule_5", "");
let angle = param("angle", 60.0, 0.0, 180.0);
let colour_pieces = param_bool("colour_pieces", true);

// Gather the non-blank rules. A rule is one symbol, "=", a replacement.
let rules = #{};
let bad = "";
for entry in [rule_1, rule_2, rule_3, rule_4, rule_5] {
    // Rhai's trim() edits the string in place and returns nothing, so
    // these are statements rather than expressions.
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
    print("A rule is one symbol, an equals sign, then its replacement — for example  F=F+F--F+F");
} else if rules.is_empty() {
    print("No rules entered, so there is nothing to build.");
} else {
    // ONE generation: the pieces that replace a single edge.
    let expanded = lsystem(axiom, rules, 1);
    let drawn = turtle(expanded, angle);

    if drawn.len() < 2 {
        // Usually the rule draws with symbols this turtle does not treat
        // as drawing. Report it rather than guessing at what was meant.
        print("One generation of this rule gives: " + expanded);
        print("That draws " + drawn.len() + " segment(s), and at least 2 are needed.");
        print("Check the drawing symbols: F G A B draw, lower-case f and g move without drawing,");
        print("and any other letter is ignored. The rule may also be written for a different program.");
    } else {
        // Shrink the whole path onto the segment from (0,0) to (1,0), so
        // every piece is smaller than the whole. That is what makes the
        // chaos game converge instead of flying apart.
        let pieces = normalize_segments(drawn);

        // Some curves are built from two symbols whose rules are mirror
        // images (Sierpinski: F=+G-F-G+, G=-F+G+F-). Both draw the same
        // endpoints, so the reflection has to come from the symbol, or
        // the attractor comes out as the wrong curve. Detected, not asked.
        let mirror_symbol = lsystem_mirror_symbol(rules, axiom);

        let mirrored_count = 0;
        for k in 0..pieces.len() {
            let t = flame.add_transform();
            let mirrored = mirror_symbol != "" && segment_symbol(pieces[k]) == mirror_symbol;
            if mirrored { mirrored_count += 1; }
            t.set_segment(pieces[k], mirrored);
            t.weight = 1.0;
            if colour_pieces {
                t.color = k.to_float() / pieces.len().to_float();
            }
        }

        // Frame the view on the CURVE, not on the depth-1 pieces — the
        // dragon folds well outside its two starting edges. Measured in
        // Rust, which keeps a deep expansion (Hilbert quadruples every
        // generation) from ever becoming a script-side array.
        let box = lsystem_bounds(axiom, rules, 6, angle);
        let extent = box[2] - box[0];
        if (box[3] - box[1]) > extent { extent = box[3] - box[1]; }
        if extent > 0.0 {
            config.set("pan_x", (box[0] + box[2]) * 0.5);
            config.set("pan_y", (box[1] + box[3]) * 0.5);
            config.set("zoom", 2.7 / extent);
        }

        print("Built " + pieces.len() + " pieces from " + expanded);
        if mirrored_count > 0 {
            print("Mirror pair found: the " + mirrored_count + " piece(s) drawn by " + mirror_symbol + " are reflected.");
        }

        // A converging IFS needs pieces SMALLER than the whole. If they
        // are not, say so plainly — this is what happens with
        // space-filling rules, where one generation replaces an edge with
        // pieces the same length as itself.
        let balance = flame.contractiveness();
        print("contractiveness: " + balance);
        if balance >= -0.0001 {
            print("");
            print("WARNING: these pieces do not shrink, so the chaos game has nothing to");
            print("converge to and the result will not be the curve you expect.");
            print("That is the signature of a space-filling rule (Hilbert, Peano), which");
            print("rewrites nodes rather than edges. This construction needs a rule whose");
            print("one generation replaces an edge with a SHORTER path.");
        }
    }
}
