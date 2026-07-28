// Hilbert Curve 3D
//
// The cube-filling Hilbert curve, as a flame.
//
// A 3D Hilbert curve visits all eight octants of the cube in
// face-adjacent order, each octant holding a half-scale copy of the
// whole — eight affine maps whose exits chain into entries. The maps
// here are constructed (and verified) rather than copied from a paper:
// a search over the cube's 48 symmetries picks, octant by octant, the
// first that keeps the curve connected.
//
// Path shows the curve itself at a chosen depth, with the palette
// running along it — 8^depth segments, so depth 4 is 4096 segments and
// depth 6 is over 260,000; raise it until your image resolution stops
// resolving the weave. Attractor is the infinite-depth limit, which for
// a space-filling curve is the SOLID CUBE — correct, and mostly useful
// with solid rendering and lighting.

script("Hilbert Curve 3D", "generator");

let output = param_choice("output", ["Path (finite depth)", "Attractor (solid cube)"], 0);
let iterations = param_int("iterations", 4, 1, 8);
let colour = param_bool("colour_along_curve", true);

let maps = hilbert3d_maps();

if output == "Path (finite depth)" {
    let t = flame.add_transform();
    t.add_variation("lsystem_path_3D", 1.0);
    t.set_variation_param("lsystem_path_3D.iterations", iterations);
    t.set_variation_param("lsystem_path_3D.map_count", maps.len());
    t.set_variation_param("lsystem_path_3D.connect", 1.0);
    t.set_variation_param("lsystem_path_3D.dc", if colour { 1.0 } else { 0.0 });
    // Centre chain, not cell spans: spans lie on the cell-edge lattice
    // and overlap (three-way joins, phantom dead ends — the same disease
    // the 2D Hilbert had). The cube's centre images to every cell's
    // centre, and corner-chaining guarantees consecutive cells share a
    // face, so every centre-chain segment is a clean axis-aligned step.
    t.set_variation_param("lsystem_path_3D.anchored", 1.0);
    t.set_variation_param("lsystem_path_3D.anchor_x", 0.5);
    t.set_variation_param("lsystem_path_3D.anchor_y", 0.5);
    t.set_variation_param("lsystem_path_3D.anchor_z", 0.5);
    t.weight = 1.0;
    let names = ["xx", "xy", "xz", "yx", "yy", "yz", "zx", "zy", "zz", "tx", "ty", "tz"];
    for k in 0..maps.len() {
        let m = maps[k];
        for j in 0..12 {
            t.set_variation_param("lsystem_path_3D.m" + k + "_" + names[j], m[j]);
        }
    }
    print("Hilbert curve at depth " + iterations + ": 8^" + iterations + " segments.");
    print("Iterations is a live parameter on the lsystem_path_3D variation.");
} else {
    for k in 0..maps.len() {
        let t = flame.add_transform();
        t.set_matrix3d(maps[k], 1.0);
        t.weight = 1.0;
        if colour {
            t.color = k.to_float() / maps.len().to_float();
        }
    }
    print("Attractor mode: the infinite-depth limit is the solid cube,");
    print("coloured by octant. Try solid rendering.");
}

// The curve lives in the unit cube; look at it from a corner-ish angle.
config.set("render_mode", "3d");
config.set("preserve_z", true);
config.set("perspective_strength", 0.15);
config.set("camera_rotation_x", 35.0);
config.set("camera_rotation_y", -30.0);
config.set("pan_x", 0.5);
config.set("pan_y", 0.5);
config.set("zoom", 2.2);
