// Kaleidoscope
//
// Gives the flame you have open exact N-fold rotational symmetry —
// Apophysis' "Add symmetry" as a script, so you can see how little it
// takes.
//
// It adds ONE transform whose affine is a pure rotation by 360/N (and
// optionally one mirror). The chaos game keeps passing points through
// it, which sweeps everything already in the flame around the center.
// Nothing you built is touched or copied.
//
// Running it twice stacks a second symmetry group on top; that is
// usually mud. Undo first, or treat "run once" as part of the recipe.

script("Kaleidoscope", "modifier", ["norng"]);

let order = param_int("order", 6, 2, 16);
let mirror = param_bool("mirror", false);
// How strongly the symmetry pulls, as a fraction of the flame's total
// weight. 1.0 = points orbit about half the time, which fills all
// sectors evenly. Lower leaves the original sector brighter than its
// copies — sometimes exactly what you want.
let strength = param("strength", 1.0, 0.2, 2.0);

// Weight the symmetry against what is actually in the flame.
let total = 0.0;
for i in 0..flame.transform_count() {
    total += flame.transform(i).weight;
}
if total <= 0.0 { total = 1.0; }

// The rotation. color_speed = 1.0 leaves the palette position alone as
// points orbit (the color update is old*(1+s)/2 + this.color*(1-s)/2,
// so s=1 is "no change") — without it every orbit drags all color
// toward one value and the arms go monochrome.
let sym = flame.add_transform();
sym.add_variation("linear", 1.0);
let theta = 2.0 * PI() / order;
sym.set_affine(cos(theta), -sin(theta), sin(theta), cos(theta), 0.0, 0.0);
sym.weight = total * strength;
sym.color_speed = 1.0;

if mirror {
    // Reflection across the X axis; with the rotation this generates
    // the dihedral group — a true kaleidoscope (2N arms).
    let m = flame.add_transform();
    m.add_variation("linear", 1.0);
    m.set_affine(1.0, 0.0, 0.0, -1.0, 0.0, 0.0);
    m.weight = total * strength * 0.5;
    m.color_speed = 1.0;
}

let arms = if mirror { order * 2 } else { order };
print("added " + order + "-fold rotation" + if mirror { " + mirror (" + arms + " arms)" } else { "" });
print("symmetry is centered on the origin — pan your motif off-center for visible arms");
