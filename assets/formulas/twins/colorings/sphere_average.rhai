// Sphere Average, the built-in colouring's twin: the mean chordal
// distance (on the Riemann sphere) from the orbit to a target point,
//
//   d(z, t) = 2 |z - t| / (sqrt(1+|z|^2) sqrt(1+|t|^2)),
//
// bounded by 2 and honest where orbits run far out, as on the maps
// whose Julia set is the whole sphere. `stride` averages every nth
// iterate only, which shows the dynamics of f^n rather than f.

const PARAMS = #{
    target_re: #{
        value: 0.0, min: -8.0, max: 8.0,
        caption: "Target (re)",
        hint: "The sphere point distances are measured to, as a complex number. The origin and 1 are the usual choices; a point ON the attractor picks out where the orbit spends its time.",
    },
    target_im: #{
        value: 0.0, min: -8.0, max: 8.0,
        caption: "Target (im)",
        hint: "Imaginary part of the target point.",
    },
    at_infinity: #{
        value: 0, choices: ["Origin", "Infinity (north pole)"],
        caption: "Target infinity",
        hint: "Measure to the sphere's north pole instead: d = 2/sqrt(1+|z|^2). Infinity is an ordinary point in the chordal metric, so this is a legitimate target rather than a special case.",
    },
    stride: #{
        value: 1.0, min: 1.0, max: 64.0,
        caption: "Iterate stride",
        hint: "Average over every nth orbit point only, which shows the dynamics of f^n rather than f. 1 is the plain orbit.",
    },
    scale: #{
        value: 1.0, min: 0.01, max: 64.0,
        caption: "Scale",
        hint: "Palette distance per unit of mean chordal distance. The metric is bounded by 2, so the whole useful range lands inside one palette turn at 1.0.",
    },
};

const INFO = #{ title: "Sphere Average" };

// The sum of the accepted distances, and the count of CALLS: with a
// stride the two counts differ, and the accepted one is derived at the
// end, since the call index cannot be.
fn start(z, p) {
    #{ total: 0.0, calls: 0.0 }
}

fn step(s, z, p) {
    let stride = max(to_int(p.stride), 1);
    let calls = to_int(s.calls);
    if stride == 1 || calls % stride == 0 {
        let zz = z.re * z.re + z.im * z.im;
        let d = if p.at_infinity == 1 {
            // Chordal distance to infinity (the north pole).
            2.0 * (1.0 / sqrt(1.0 + zz))
        } else {
            let t = complex(p.target_re, p.target_im);
            let dz = z - t;
            2.0 * sqrt(dz.re * dz.re + dz.im * dz.im) * (1.0 / sqrt((1.0 + zz) * (1.0 + t.re * t.re + t.im * t.im)))
        };
        s.total = s.total + d;
    }
    s.calls = s.calls + 1.0;
    s
}

fn finish(s, z, p) {
    let stride = max(p.stride, 1.0);
    let samples = max(ceil(max(s.calls, 0.0) / stride), 1.0);
    (s.total / samples) * p.scale
}
