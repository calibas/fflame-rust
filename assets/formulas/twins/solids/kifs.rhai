// The built-in KIFS (Solid) as a solid of your own (the Escape panel's
// Edit as Rhai): Knighty's kaleidoscopic IFS -- each iteration a turn, a
// fold into one piece of space, a turn, and a scaling about the offset --
// with the distance its scaling bounds. Each step is the built-in's
// arithmetic, in its order, so the two draw alike.
//
// The colourings read the start of the orbit: the orbit trap the closest
// approach within Colour Depth (its xy), the level the iteration it falls
// on and how close it came, and the address which of the fold's mirrors
// fired, a base-8 digit an iteration for eight folds from where Address
// Symmetry starts it (`branch`, read as the walk ends), the first of
// them the colour.

const PARAMS = #{
    // The template's, declared to keep the built-in's defaults and words.
    levels: #{
        value: 12.0, min: 1.0, max: 64.0,
        caption: "Iterations",
        hint: "How many folds and scalings before calling the point part of the set. Each one resolves the set a scale finer: past about a dozen the detail is below a pixel.",
    },
    beam: #{
        value: 1.0, min: 1.0, max: 8.0,
        caption: "Beam Width",
        hint: "Unused here: a fold has one orbit. Kept so the march, shadow and occlusion slots line up with the flame solid's.",
    },
    steps: #{ value: 128.0, min: 4.0, max: 512.0, caption: "March Steps", hint: "How many times a ray may step before giving up." },
    w_slice: #{ value: 0.0, min: -2.0, max: 2.0, caption: "Slice Value", hint: "Unused here: a KIFS is three-dimensional. Kept for the slot." },
    fold: #{
        value: 3,
        caption: "Fold",
        hint: "How each iteration folds space into one piece of it: the tetrahedron's three plane folds (the Sierpinski tetrahedron), each axis's mirror (cubic), the mirrors and the diagonal ones (octahedral), those with the sponge's correction (Menger), or the mirrors of the icosahedron or the dodecahedron (golden-ratio planes).",
        choices: ["Tetrahedral", "Cubic", "Octahedral", "Menger", "Icosahedral", "Dodecahedral"],
    },
    scale: #{
        value: 3.0, min: 1.1, max: 4.0,
        caption: "Scale",
        hint: "How much each iteration enlarges space about the offset. 2 for the tetrahedron, 3 for the sponge; between, the pieces overlap or part.",
    },
    offset_x: #{ value: 1.0, min: -2.0, max: 2.0, caption: "Offset X", hint: "The point each iteration scales about: the set's corner." },
    offset_y: #{ value: 1.0, min: -2.0, max: 2.0, caption: "Offset Y", hint: "The offset's y." },
    offset_z: #{ value: 1.0, min: -2.0, max: 2.0, caption: "Offset Z", hint: "The offset's z." },
    rot1_x: #{
        value: 0.0, min: -180.0, max: 180.0,
        caption: "Pre-Fold Rotation X",
        hint: "Degrees about x before each fold (Knighty's rotation): small angles twist the set into new shapes.",
    },
    rot1_y: #{ value: 0.0, min: -180.0, max: 180.0, caption: "Pre-Fold Rotation Y", hint: "Degrees about y before each fold." },
    rot1_z: #{ value: 0.0, min: -180.0, max: 180.0, caption: "Pre-Fold Rotation Z", hint: "Degrees about z before each fold." },
    rot2_x: #{ value: 0.0, min: -180.0, max: 180.0, caption: "Post-Fold Rotation X", hint: "Degrees about x after each fold, before the scaling." },
    rot2_y: #{ value: 0.0, min: -180.0, max: 180.0, caption: "Post-Fold Rotation Y", hint: "Degrees about y after each fold." },
    rot2_z: #{ value: 0.0, min: -180.0, max: 180.0, caption: "Post-Fold Rotation Z", hint: "Degrees about z after each fold." },
    colour_depth: #{
        value: 3.0, min: 1.0, max: 12.0,
        caption: "Colour Depth",
        hint: "How many of the first iterations the colourings read: the orbit trap's closest approach and the level's bands. The surface is drawn a pixel from the set, and past a few iterations that pixel decides the orbit, so deeper colours change with the camera. Raise it when zoomed in.",
    },
    address_symmetry: #{
        value: 1,
        caption: "Address Symmetry",
        hint: "Which folds the branch address starts from. Mirror images share their orbit after the first fold, so starting after it colours every mirror copy alike; after the second, each piece's copies too.",
        choices: ["Off", "Whole object", "Each piece"],
    },
};

const INFO = #{ title: "KIFS (Solid)", coloring: "ifs_level", slow: ["levels", "beam", "steps"] };

// A turn by Euler angles in degrees, x first, then y, then z, as the
// built-in's `kifs_rotation` (rz ry rx), a row at a time: rx = [1 0 0;
// 0 cx -sx; 0 sx cx], ry = [cy 0 sy; 0 1 0; -sy 0 cy], rz = [cz -sz 0;
// sz cz 0; 0 0 1].
fn row_a(ax, ay, az) {
    let k = 0.017453292519943295;
    let cx = cos(ax * k);
    let sx = sin(ax * k);
    let cy = cos(ay * k);
    let sy = sin(ay * k);
    let cz = cos(az * k);
    let sz = sin(az * k);
    vec3(cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx)
}

fn row_b(ax, ay, az) {
    let k = 0.017453292519943295;
    let cx = cos(ax * k);
    let sx = sin(ax * k);
    let cy = cos(ay * k);
    let sy = sin(ay * k);
    let cz = cos(az * k);
    let sz = sin(az * k);
    vec3(sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx)
}

fn row_c(ax, ay, az) {
    let k = 0.017453292519943295;
    let cx = cos(ax * k);
    let sx = sin(ax * k);
    let cy = cos(ay * k);
    let sy = sin(ay * k);
    vec3(-sy, cy * sx, cy * cx)
}

// Every fold and turn fixes the origin, so a point past |offset| escapes
// and the set lies within it: the ball, as the built-in's.
fn prepare(p) {
    let ball = max(sqrt(p.offset_x * p.offset_x + p.offset_y * p.offset_y + p.offset_z * p.offset_z), 1.0e-3);
    let turn1 = if p.rot1_x != 0.0 || p.rot1_y != 0.0 || p.rot1_z != 0.0 { 1.0 } else { 0.0 };
    let turn2 = if p.rot2_x != 0.0 || p.rot2_y != 0.0 || p.rot2_z != 0.0 { 1.0 } else { 0.0 };
    #{
        ball: ball, bail: 1000.0 * ball,
        turn1: turn1,
        r1a: row_a(p.rot1_x, p.rot1_y, p.rot1_z),
        r1b: row_b(p.rot1_x, p.rot1_y, p.rot1_z),
        r1c: row_c(p.rot1_x, p.rot1_y, p.rot1_z),
        turn2: turn2,
        r2a: row_a(p.rot2_x, p.rot2_y, p.rot2_z),
        r2b: row_b(p.rot2_x, p.rot2_y, p.rot2_z),
        r2c: row_c(p.rot2_x, p.rot2_y, p.rot2_z),
    }
}

fn start(pos, p) {
    #{
        z: pos, k: 0, out: false,
        trap: complex(0.0, 0.0), trap_r: 1.0e30, trap_k: 0, level: 0.0, color: 0.5,
        branch: 0.0, place: 0.125,
    }
}

fn step(s, p) {
    let fold = p.fold;
    let sc = clamp(p.scale, 1.1, 4.0);
    let o = vec3(p.offset_x, p.offset_y, p.offset_z);
    let depth = to_int(clamp(round_even(p.colour_depth), 1.0, 12.0));
    let skip = to_int(clamp(round_even(to_float(p.address_symmetry)), 0.0, 2.0));
    let z = s.z;
    if p.turn1 > 0.5 {
        z = vec3(dot(p.r1a, z), dot(p.r1b, z), dot(p.r1c, z));
    }
    // The fold, and which of its mirrors fired: a base-8 digit.
    let digit = 0;
    if fold == 4 {
        // The icosahedral fold: once, into the group's fundamental domain
        // (abs, then n1, n2, n3, n2), then each iteration abs and n1.
        let phi = 1.618033988749895;
        let n1 = normalize(vec3(-phi, phi - 1.0, 1.0));
        let n2 = normalize(vec3(1.0, -phi, phi + 1.0));
        let n3 = vec3(0.0, 0.0, -1.0);
        if s.k == 0 {
            z = abs(z);
            z = z - n1 * (2.0 * max(dot(z, n1), 0.0));
            z = z - n2 * (2.0 * max(dot(z, n2), 0.0));
            z = z - n3 * (2.0 * max(dot(z, n3), 0.0));
            z = z - n2 * (2.0 * max(dot(z, n2), 0.0));
        }
        if z.x < 0.0 { digit += 1; }
        if z.y < 0.0 { digit += 1; }
        if z.z < 0.0 { digit += 1; }
        z = abs(z);
        let t = dot(z, n1);
        if t > 0.0 {
            z = z - n1 * (2.0 * t);
            digit += 4;
        }
    } else if fold == 5 {
        // The dodecahedral fold: three passes of its three mirrors; the
        // digit says which the first pass fired.
        let phi = 1.618033988749895;
        let n1 = normalize(vec3(-1.0, phi - 1.0, 1.0 / (phi - 1.0)));
        let n2 = normalize(vec3(phi - 1.0, 1.0 / (phi - 1.0), -1.0));
        let n3 = normalize(vec3(1.0 / (phi - 1.0), -1.0, phi - 1.0));
        let pass = 0;
        while pass < 3 {
            let t1 = dot(z, n1);
            if t1 < 0.0 {
                z = z - n1 * (2.0 * t1);
                if pass == 0 { digit = digit | 1; }
            }
            let t2 = dot(z, n2);
            if t2 < 0.0 {
                z = z - n2 * (2.0 * t2);
                if pass == 0 { digit = digit | 2; }
            }
            let t3 = dot(z, n3);
            if t3 < 0.0 {
                z = z - n3 * (2.0 * t3);
                if pass == 0 { digit = digit | 4; }
            }
            pass += 1;
        }
    } else if fold == 0 {
        // The tetrahedron's three plane folds.
        if z.x + z.y < 0.0 {
            z = vec3(-z.y, -z.x, z.z);
            digit = digit | 1;
        }
        if z.x + z.z < 0.0 {
            z = vec3(-z.z, z.y, -z.x);
            digit = digit | 2;
        }
        if z.y + z.z < 0.0 {
            z = vec3(z.x, -z.z, -z.y);
            digit = digit | 4;
        }
    } else {
        // Each axis's mirror; past the cubic, the diagonal ones too.
        if z.x < 0.0 { digit = digit | 1; }
        if z.y < 0.0 { digit = digit | 2; }
        if z.z < 0.0 { digit = digit | 4; }
        z = abs(z);
        if fold >= 2 {
            if z.x < z.y { z = vec3(z.y, z.x, z.z); }
            if z.x < z.z { z = vec3(z.z, z.y, z.x); }
            if z.y < z.z { z = vec3(z.x, z.z, z.y); }
        }
    }
    if p.turn2 > 0.5 {
        z = vec3(dot(p.r2a, z), dot(p.r2b, z), dot(p.r2c, z));
    }
    z = z * sc - o * (sc - 1.0);
    // The sponge: the middle layer's cubes move back up.
    if fold == 3 && z.z < -0.5 * o.z * (sc - 1.0) {
        z = vec3(z.x, z.y, z.z + o.z * (sc - 1.0));
    }
    if s.k >= skip && s.k < skip + 8 {
        s.branch += to_float(digit) * s.place;
        s.place = s.place * 0.125;
    }
    if s.k == skip {
        s.color = (to_float(digit) + 0.5) * 0.125;
    }
    s.z = z;
    s.k += 1;
    let r = length(z);
    if s.k <= depth {
        let zt = trap_point(z);
        let rt = length(zt);
        if rt < s.trap_r {
            s.trap_r = rt;
            s.trap_k = s.k;
            s.trap = complex(zt.x, zt.y);
        }
    }
    // The band: the iteration of the closest approach, and how close.
    s.level = to_float(s.trap_k) + clamp(s.trap_r / p.ball, 0.0, 0.999);
    if r > p.ball {
        s.out = true;
    }
    s
}

// Past the set, a few more for the estimate's sake.
fn escaped(s, p) {
    length(s.z) > p.bail
}

// A lower bound: each step stretches distances by at most the scale.
fn distance(s, p) {
    if s.out {
        max(length(s.z) - p.ball, 0.0) * pow(clamp(p.scale, 1.1, 4.0), -to_float(s.k))
    } else {
        0.0
    }
}
