// L-System Curve
//
// Turns a rewriting rule into a flame. Edit the axiom, rules and angle in
// the panel — you should not need to touch the script.
//
// HOW IT WORKS
// An L-system grows a curve by repeatedly replacing parts with smaller
// paths. This script finds the self-similar pieces of ONE generation and
// makes each one a transform; the chaos game then draws the curve those
// transforms agree on, to infinite depth, for free. Two constructions
// are tried, automatically:
//
//   EDGE rewriting (Koch, dragon, Levy...): the rule replaces a drawn
//   edge with a shorter path — the drawn depth-1 segments are the pieces.
//
//   NODE rewriting (Hilbert, Peano — the space-fillers): the rule
//   rewrites variables, and the drawn steps are just connections. The
//   pieces live at each VARIABLE occurrence instead: one copy of the
//   whole curve per occurrence.
//
// Not supported: rules whose variables are two genuinely different
// curves (not mirror images of each other) — that needs more than a flat
// set of transforms, and the script will say so.
//
// SYMBOLS  (Prusinkiewicz & Lindenmayer, "The Algorithmic Beauty of
// Plants" — the convention most sources follow)
//   F G A B   draw forward
//   f g       move forward WITHOUT drawing
//   + -       turn left / right by Angle
//   & ^       pitch down / up      \\ /   roll left / right   (3D!)
//   |         turn around
//   [ ]       start / end a branch
//   any other letter (X, Y, ...) is a variable: it only drives rewriting
//
// A word of honesty about 3D CURVES: many 3D edge rules have no stable
// self-similar limit — the rotations compound so the shape never
// settles, and the faithful answer is a degenerate or drifting form.
// That is the mathematics, not a bug; it is why 3D L-system art is
// nearly always PLANTS (see the L-System Plant script), which brackets
// hold stable.
//
// Sources disagree about which letters draw. If you copy a rule that uses
// lower-case f and g as DRAWING symbols, upper-case them — otherwise
// nothing is drawn and the script will tell you so.
//
// RULE FORMAT
//   symbol=replacement        e.g.  F=F+F--F+F
// Leave a rule blank if you do not need it.
//
// SOME TO TRY
//   Koch          axiom F   F=F+F--F+F                 angle 60
//   Koch square   axiom F   F=F+F-F-F+F                angle 90
//   Koch island   axiom F   F=F+F-F-FF+F+F-F           angle 90
//   Dragon        axiom F   F=F+G   G=F-G              angle 90
//   Sierpinski    axiom F   F=+G-F-G+   G=-F+G+F-      angle 60
//   Levy C        axiom F   F=+F--F+                   angle 45
//   Hilbert       axiom X   X=-YF+XFX+FY-   Y=+XF-YFY-FX+   angle 90
//   Peano         axiom X   X=XFYFX+F+YFXFY-F-XFYFX   Y=YFXFY-F-XFYFX+F+YFXFY   angle 90

script("L-System Curve", "generator");

let axiom = param_string("axiom", "F");
let rule_1 = param_string("rule_1", "F=F+F--F+F");
let rule_2 = param_string("rule_2", "");
let rule_3 = param_string("rule_3", "");
let rule_4 = param_string("rule_4", "");
let rule_5 = param_string("rule_5", "");
let angle = param("angle", 60.0, 0.0, 180.0);

// Attractor: the pieces become ordinary transforms and the chaos game
// draws the INFINITE-depth limit — for a space-filling rule that limit is
// a filled square. Path: one lsystem_path variation draws the curve at a
// chosen finite depth — the iconic Hilbert maze, a depth-6 snowflake —
// with the palette running along the path.
let output = param_choice("output", ["Attractor (infinite depth)", "Path (finite depth)"], 0);
let path_iterations = param_int("path_iterations", 5, 1, 12);
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

    let pieces = [];
    let construction = "";
    let node_note = "";

    // Any pitch or roll command switches to the 3D construction: pieces
    // come back as full 3D matrices (a flat segment cannot carry roll).
    let is_3d = lsystem_uses_3d(axiom, rules);
    if is_3d {
        let nres = lsystem_pieces3(axiom, rules, angle);
        node_note = nres.note;
        if nres.branches.len() >= 2 {
            pieces = nres.branches;
            construction = "3d";
        }
    }

    // A single-variable axiom announces a node-rewriting system, so try
    // that construction FIRST. Order matters: Peano's connective F-steps
    // happen to pass the edge shrink test (its rule walks further than
    // one step), and edge mode would quietly build a curve out of the
    // connectors instead of the Peano curve.
    let axiom_is_variable = axiom.len() == 1 && (axiom in rules) && !"FGABfg".contains(axiom);
    if !is_3d && axiom_is_variable {
        let nres = lsystem_node_pieces(axiom, rules, angle);
        node_note = nres.note;
        if nres.pieces.len() >= 2 {
            pieces = nres.pieces;
            construction = "node";
        }
    }

    // EDGE construction: usable when the depth-1 drawn segments all
    // SHRINK relative to the whole. If they do not, this is not an
    // edge-rewriting system and the pieces would never converge.
    // A graph-directed verdict from the node attempt means the drawn
    // segments are connectors between variables — edge mode would build
    // a curve out of the connective tissue (the Peano trap), so skip it
    // and let the graph construction below take the system instead.
    if !is_3d && construction == "" && drawn.len() >= 2 && !node_note.contains("graph-directed") {
        try {
            let cand = normalize_segments(drawn);
            let shrinks = true;
            for k in 0..cand.len() {
                let d = cand[k];
                let vx = d[2] - d[0];
                let vy = d[3] - d[1];
                if vx * vx + vy * vy >= 0.998 { shrinks = false; }
            }
            pieces = cand;
            if shrinks { construction = "edge"; }
        } catch (e) {
            // A closed depth-1 path has no unit-segment frame; the node
            // construction below may still apply.
        }
    }

    // NODE construction as a fallback for everything else (a drawing
    // axiom whose pieces did not shrink, say).
    if !is_3d && construction == "" && !axiom_is_variable {
        let nres = lsystem_node_pieces(axiom, rules, angle);
        node_note = nres.note;
        if nres.pieces.len() >= 2 {
            pieces = nres.pieces;
            construction = "node";
        }
    }

    // GRAPH construction: systems whose variables are genuinely
    // different curves (not mirror pairs) are graph-directed IFSs. A
    // flame expresses those through XAOS — one transform per occurrence,
    // allowed to follow another only when it consumes the type the other
    // produced — with opacity hiding every type except the axiom's, so
    // only the wanted curve plots while the others drive the dynamics
    // invisibly.
    let graph_pieces = [];
    if construction == "" && node_note.contains("graph-directed") {
        let gres = lsystem_graph_pieces(axiom, rules, angle);
        if gres.pieces.len() >= 2 {
            graph_pieces = gres.pieces;
            construction = "graph";
            pieces = gres.pieces;
        } else if gres.note != "" {
            node_note = gres.note;
        }
    }

    // Last resort: build the non-shrinking edge pieces anyway (with a
    // warning below) so there is something to look at and adjust.
    if construction == "" && pieces.len() >= 2 {
        construction = "edge";
    }

    if pieces.len() < 2 {
        print("One generation of this rule gives: " + expanded);
        print("That draws " + drawn.len() + " segment(s), and at least 2 are needed.");
        print("Check the drawing symbols: F G A B draw, lower-case f and g move without drawing,");
        print("and any other letter is ignored. The rule may also be written for a different program.");
        if node_note != "" {
            print("The node-rewriting construction was tried too: " + node_note);
        }
    } else {
        // Some systems pair two symbols whose rules are mirror images
        // (Sierpinski's F/G, Hilbert's X/Y). Both make identical
        // endpoints, so the reflection has to come from the symbol, or
        // the attractor comes out as the wrong curve. Detected, not asked.
        let mirror_symbol = lsystem_mirror_symbol(rules, axiom);

        // The attractor's bounding box, used for framing and (in path
        // mode) for the anchor point at its centre.
        let box = [0.0, 0.0, 1.0, 0.0];
        if !is_3d {
            box = lsystem_bounds(axiom, rules, 6, angle);
        }

        let path_mode = output == "Path (finite depth)" && pieces.len() <= 12;
        if output == "Path (finite depth)" && pieces.len() > 12 {
            print("Path mode holds at most 12 maps and this rule needs " + pieces.len() + " — building the attractor instead.");
        }

        let mirrored_count = 0;
        if construction == "graph" {
            // One transform per occurrence; xaos carries the word
            // structure; opacity shows only the axiom's curve.
            let n = graph_pieces.len();
            for k in 0..n {
                let q = graph_pieces[k];
                let t = flame.add_transform();
                t.set_matrix3d(q, 1.0);
                let sc = sqrt(q[0] * q[0] + q[3] * q[3] + q[6] * q[6]);
                t.weight = sc + 0.05;
                t.opacity = if q[14] == axiom { 1.0 } else { 0.0 };
                if colour_pieces {
                    t.color = k.to_float() / n.to_float();
                }
            }
            for from in 0..n {
                for to in 0..n {
                    // occ(next) == owner(prev): the next map must consume
                    // what the last one produced.
                    let allowed = graph_pieces[to][13] == graph_pieces[from][14];
                    flame.set_xaos(from, to, if allowed { 1.0 } else { 0.0 });
                }
            }
        } else if is_3d && path_mode {
            // One transform, the 3D path variation: twelve coefficients
            // per map, composed by the digits of the curve parameter.
            let t = flame.add_transform();
            t.add_variation("lsystem_path_3D", 1.0);
            t.set_variation_param("lsystem_path_3D.iterations", path_iterations);
            t.set_variation_param("lsystem_path_3D.map_count", pieces.len());
            t.set_variation_param("lsystem_path_3D.connect", 1.0);
            t.set_variation_param("lsystem_path_3D.dc", if colour_pieces { 1.0 } else { 0.0 });
            t.weight = 1.0;
            let names = ["xx", "xy", "xz", "yx", "yy", "yz", "zx", "zy", "zz", "tx", "ty", "tz"];
            for k in 0..pieces.len() {
                let q = pieces[k];
                for j in 0..12 {
                    t.set_variation_param("lsystem_path_3D.m" + k + "_" + names[j], q[j]);
                }
            }
        } else if is_3d {
            // 3D attractor: one matrix3D transform per piece. Mirror
            // partners are already folded into the matrices.
            for k in 0..pieces.len() {
                let q = pieces[k];
                let t = flame.add_transform();
                t.set_matrix3d(q, 1.0);
                t.weight = 1.0;
                if colour_pieces {
                    t.color = k.to_float() / pieces.len().to_float();
                }
            }
        } else if path_mode {
            // One transform carrying the whole finite-depth curve. No
            // geometry is baked: the variation composes the maps by the
            // digits of the curve parameter, so Iterations stays a live
            // slider in the variation's own parameters.
            let t = flame.add_transform();
            t.add_variation("lsystem_path", 1.0);
            t.set_variation_param("lsystem_path.iterations", path_iterations);
            t.set_variation_param("lsystem_path.map_count", pieces.len());
            t.set_variation_param("lsystem_path.connect", 1.0);
            t.set_variation_param("lsystem_path.dc", if colour_pieces { 1.0 } else { 0.0 });
            if construction == "node" {
                // Space-filling curves must draw the CENTRE chain: their
                // cell spans lie on the boundary lattice and overlap
                // (doubled lines, little boxes). The anchor is the
                // attractor's centre, whose image in every cell is that
                // cell's centre — the classic self-avoiding drawing.
                t.set_variation_param("lsystem_path.anchored", 1.0);
                t.set_variation_param("lsystem_path.anchor_x", (box[0] + box[2]) * 0.5);
                t.set_variation_param("lsystem_path.anchor_y", (box[1] + box[3]) * 0.5);
            }
            t.weight = 1.0;
            for k in 0..pieces.len() {
                let d = pieces[k];
                let dx = d[2] - d[0];
                let dy = d[3] - d[1];
                let mirrored = mirror_symbol != "" && segment_symbol(pieces[k]) == mirror_symbol;
                if mirrored { mirrored_count += 1; }
                // The similarity carrying the unit edge onto this piece —
                // the same matrix set_segment builds, mirror included.
                t.set_variation_param("lsystem_path.m" + k + "_a", dx);
                t.set_variation_param("lsystem_path.m" + k + "_c", dy);
                if mirrored {
                    t.set_variation_param("lsystem_path.m" + k + "_b", dy);
                    t.set_variation_param("lsystem_path.m" + k + "_d", -dx);
                } else {
                    t.set_variation_param("lsystem_path.m" + k + "_b", -dy);
                    t.set_variation_param("lsystem_path.m" + k + "_d", dx);
                }
                t.set_variation_param("lsystem_path.m" + k + "_e", d[0]);
                t.set_variation_param("lsystem_path.m" + k + "_f", d[1]);
            }
        } else {
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
        }

        // Frame the view on the CURVE, not on the depth-1 pieces — the
        // dragon folds well outside its two starting edges. (Bounds were
        // measured above, in Rust, so a deep expansion never becomes a
        // script-side array.)
        if is_3d {
            let b3 = lsystem_bounds3(axiom, rules, 6, angle);
            let extent = b3[3] - b3[0];
            if (b3[4] - b3[1]) > extent { extent = b3[4] - b3[1]; }
            if extent > 0.0 {
                config.set("pan_x", (b3[0] + b3[3]) * 0.5);
                config.set("pan_y", (b3[1] + b3[4]) * 0.5);
                config.set("zoom", 2.2 / extent);
            }
            config.set("render_mode", "3d");
            config.set("preserve_z", true);
            config.set("perspective_strength", 0.12);
            config.set("camera_rotation_x", 30.0);
        } else {
            let extent = box[2] - box[0];
            if (box[3] - box[1]) > extent { extent = box[3] - box[1]; }
            if extent > 0.0 {
                config.set("pan_x", (box[0] + box[2]) * 0.5);
                config.set("pan_y", (box[1] + box[3]) * 0.5);
                config.set("zoom", 2.7 / extent);
            }
        }

        if construction == "graph" {
            print("Graph-directed system: " + graph_pieces.len() + " maps across its curve types.");
            print("The word structure runs through xaos; only the axiom's curve is visible.");
        } else if construction == "3d" {
            print("3D rule: " + pieces.len() + " pieces with full 3D frames.");
        } else if construction == "node" {
            print("Space-filling (node-rewriting) rule: " + pieces.len() + " copies of the whole curve, one per variable occurrence.");
        } else {
            print("Built " + pieces.len() + " pieces from " + expanded);
        }
        if mirrored_count > 0 {
            print("Mirror pair found: the " + mirrored_count + " piece(s) drawn by " + mirror_symbol + " are reflected.");
        }

        if construction == "graph" {
            // matrix3D leaves the transform affine at identity, so the
            // usual balance metric is blind here; report piece scales.
            let acc = 0.0;
            for k in 0..graph_pieces.len() {
                let q = graph_pieces[k];
                let sc = sqrt(q[0] * q[0] + q[3] * q[3] + q[6] * q[6]);
                if sc > 0.000001 { acc += ln(sc); }
            }
            print("mean piece log-scale: " + acc / graph_pieces.len());
        } else if is_3d && !path_mode {
            // flame.contractiveness() reads the transform affine, which
            // matrix3D leaves at identity — compute from the pieces
            // instead so the number still means something.
            let acc = 0.0;
            for k in 0..pieces.len() {
                let q = pieces[k];
                let sc = sqrt(q[0] * q[0] + q[3] * q[3] + q[6] * q[6]);
                if sc > 0.000001 { acc += ln(sc); }
            }
            print("mean piece log-scale: " + acc / pieces.len());
        } else if path_mode {
            print("Path mode: drawing the curve at depth " + path_iterations
                + " (" + pieces.len() + "^" + path_iterations + " segments)."
                + " Iterations is a live parameter on the lsystem_path variation.");
        } else {
            // A converging IFS needs pieces SMALLER than the whole. If
            // they are not, say so plainly rather than leaving a flame
            // that quietly is not the curve that was asked for. (Path
            // mode has no such requirement — a finite path needs no
            // convergence, which is exactly why space-fillers want it.)
            let balance = flame.contractiveness();
            print("contractiveness: " + balance);
            if balance >= -0.0001 {
                print("");
                print("WARNING: these pieces do not shrink, so the chaos game has nothing to");
                print("converge to and the result will not be the curve you expect.");
                if node_note != "" {
                    print("The node-rewriting construction was tried too: " + node_note);
                }
            }
        }
    }
}
