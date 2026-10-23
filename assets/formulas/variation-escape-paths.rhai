// Halayka's escape paths, as a 3D flame variation (S. Halayka, "Visualizing
// the escape paths of quaternion fractals", 2018; the source is
// github.com/sjhalayka/bezier_fractal). Each point it gives lies on the
// path of an orbit that stays: a starting point on a lattice in the box
// whose orbit keeps |Z| within the threshold for all its iterations, the
// orbit Z0 .. Zn the control points of a Bezier curve, and the point the
// curve's at a random t in [0, Length] -- the paper draws the first fifth
// of each path. Its colour is t over Length, so each path runs from one
// end of the palette at its start to the other.
//
// It ignores the point it is given: use it as the only variation of a 3D
// flame's only transform, with Direct Color at 1 for its colours. The
// flame's points draw the paths as fibres.
//
// It shows quaternions in a 3D variation: `quat(w, x, y, z)`, `*`
// Hamilton's product, `pow`, `sin`, `inv` and the rest (see
// solid-halayka-quaternions.rhai), the steps Halayka's (2009). The path's
// point is summed as the orbit runs, the k-th point weighted C(n, k) t^k
// (1 - t)^(n - k), so no array holds the orbit.

const PARAMS = #{
    step: #{ value: 0, caption: "Step", choices: ["Z^n + C", "sin Z + C sin Z", "cos Z + C cos Z", "exp(Z^2) + C", "inv(sinh Z) + C inv(sinh Z)", "C inv(sinh Z) cosh Z"], hint: "Which of Halayka's steps the orbits take" },
    c: #{ value: quat(0.3, 0.5, 0.4, 0.2), caption: "C", hint: "The constant, scalar part first: Halayka's (0.3, 0.5, 0.4, 0.2)" },
    power: #{ value: 2.0, min: 2.0, max: 8.0, caption: "Power", hint: "The n of Z^n + C (the paper's figures 1 to 4 are 2 to 5)" },
    threshold: #{ value: 4.0, min: 1.0, max: 16.0, caption: "Threshold", hint: "An orbit whose |Z| passes it escapes, and draws no path" },
    iterations: #{ value: 8, min: 1, max: 16, caption: "Iterations", hint: "How many steps each orbit takes: its path's control points, one more" },
    box: #{ value: 1.5, min: 0.1, max: 4.0, caption: "Box", hint: "The half-width of the box the paths start in (Halayka's -1.5 to 1.5)" },
    spacing: #{ value: 0.05, min: 0.0, max: 0.5, caption: "Lattice", hint: "The spacing of the paths' starting points: Halayka's lattice. 0 starts them anywhere, a haze rather than fibres" },
    length: #{ value: 0.2, min: 0.01, max: 0.95, caption: "Path Length", hint: "How much of each path is drawn, from its start (Halayka's 0.2)" },
};

const INFO = #{ title: "Escape Paths (Rhai)", name: "rhai_escape_paths" };

// One of Halayka's steps. A helper reads only its arguments: the
// parameters it needs come as values.
fn next(z, step, c, power) {
    if step == 0 {
        pow(z, power) + c
    } else if step == 1 {
        sin(z) + c * sin(z)
    } else if step == 2 {
        cos(z) + c * cos(z)
    } else if step == 3 {
        exp(z * z) + c
    } else if step == 4 {
        inv(sinh(z)) + c * inv(sinh(z))
    } else {
        c * (inv(sinh(z)) * cosh(z))
    }
}

fn variation3(v, p) {
    let t = rand() * p.length;
    let u = 1.0 - t;
    let n = p.iterations;
    let found = false;
    let point = vec3(0.0, 0.0, 0.0);
    let tries = 0;
    // A start whose orbit stays: a few draws, as most of the box escapes.
    while !found && tries < 24 {
        tries += 1;
        let a = (vec3(rand(), rand(), rand()) * 2.0 - vec3(1.0)) * p.box;
        if p.spacing > 0.0 {
            a = (floor(a / p.spacing) + vec3(0.5)) * p.spacing;
        }
        let z = quat(a.x, a.y, a.z, 0.0);
        let w = u ** n.to_float();
        let sum = z * w;
        let stays = true;
        let k = 0;
        while k < n {
            if stays {
                z = next(z, p.step, p.c, p.power);
                w = w * (n - k).to_float() / (k + 1).to_float() * t / u;
                sum = sum + z * w;
                // Written so a step with no number escapes too.
                stays = length(z) <= p.threshold;
            }
            k += 1;
        }
        if stays {
            found = true;
            // The path in the space of the quaternion's first three parts,
            // as the paper draws it.
            point = vec3(sum.w, sum.x, sum.y);
        }
    }
    // No start found: far out of the picture.
    let at = if found { point } else { vec3(0.0, 0.0, 1.0e4) };
    #{ z: at, color: t / p.length }
}
