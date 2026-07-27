// L-System Curve
//
// Turns a rewriting rule into a flame.
//
// An L-system grows a curve by repeatedly replacing each edge with a
// small path. If you take just ONE generation of that replacement and
// shrink it onto the original edge, each piece becomes a transform — and
// the flame's chaos game draws the curve those transforms agree on, to
// infinite depth, for free. That is the whole trick: the rule becomes the
// transforms.
//
// To use your own rule, edit `axiom`, `rules` and `angle` below. The
// turtle understands:
//   F G A B  draw forward        f g  move without drawing
//   +  -     turn left / right   |    turn around
//   [  ]     start / end a branch

script("L-System Curve", "generator");

let preset = param_choice(
    "curve",
    ["Koch", "Koch Square", "Dragon", "Sierpinski", "Levy C"],
    0,
);
let colour_pieces = param_bool("colour_pieces", true);

// Each preset is an axiom, a rewriting rule, and a turn angle.
let axiom = "F";
let rules = #{};
let angle = 60.0;

if preset == "Koch" {
    rules = #{ "F": "F+F--F+F" };
    angle = 60.0;
} else if preset == "Koch Square" {
    rules = #{ "F": "F+F-F-F+F" };
    angle = 90.0;
} else if preset == "Dragon" {
    // Two symbols that both draw, rewriting into each other.
    axiom = "F";
    rules = #{ "F": "F+G", "G": "F-G" };
    angle = 90.0;
} else if preset == "Sierpinski" {
    // Arrowhead construction: A and B both draw, and swap roles.
    axiom = "A";
    rules = #{ "A": "B-A-B", "B": "A+B+A" };
    angle = 60.0;
} else {
    // Levy C curve.
    rules = #{ "F": "+F--F+" };
    angle = 45.0;
}

// ONE generation: the pieces that replace a single edge.
let expanded = lsystem(axiom, rules, 1);

// Shrink the whole path onto the segment from (0,0) to (1,0). Each piece
// is then smaller than the whole, which is what makes the chaos game
// converge instead of flying apart.
let pieces = normalize_segments(turtle(expanded, angle));

for k in 0..pieces.len() {
    let t = flame.add_transform();
    // The transform that carries the unit edge onto this piece.
    t.set_segment(pieces[k]);
    t.weight = 1.0;
    if colour_pieces {
        t.color = k.to_float() / pieces.len().to_float();
    }
}

// Frame the view. The depth-1 pieces show where the transforms are, but
// not where the CURVE ends up — the dragon, for instance, folds well
// outside its two starting edges. So measure a deeper expansion, which
// normalises into the same coordinates because a self-similar curve keeps
// the same start and end however far you expand it.
let deep = normalize_segments(turtle(lsystem(axiom, rules, 6), angle));
let minx = 0.0; let maxx = 1.0; let miny = 0.0; let maxy = 0.0;
for k in 0..deep.len() {
    let d = deep[k];
    if d[0] < minx { minx = d[0]; }
    if d[2] < minx { minx = d[2]; }
    if d[0] > maxx { maxx = d[0]; }
    if d[2] > maxx { maxx = d[2]; }
    if d[1] < miny { miny = d[1]; }
    if d[3] < miny { miny = d[3]; }
    if d[1] > maxy { maxy = d[1]; }
    if d[3] > maxy { maxy = d[3]; }
}
let width = maxx - minx;
let height = maxy - miny;
let extent = width;
if height > extent { extent = height; }
config.set("pan_x", (minx + maxx) * 0.5);
config.set("pan_y", (miny + maxy) * 0.5);
config.set("zoom", 2.7 / extent);   // leaves a margin

print("Built " + preset + " from " + pieces.len() + " pieces.");
print("contractiveness: " + flame.contractiveness());
