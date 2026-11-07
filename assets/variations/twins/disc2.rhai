/// Variant of Disc with extra twist and rotation. The `rot` slider scales
/// the wave frequency; `twist` adds rotational drift that amplifies beyond
/// ±2π.
/// 
/// # Authors
/// - Z+

// Disc2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    rot: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Rot",
        hint: "Wave frequency for the disc pattern — multiplied by π internally.",
    },
    twist: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Twist",
        hint: "Rotational drift added to the disc output. Beyond ±2π the effect amplifies linearly.",
    },
};

const INFO = #{ title: "Disc2", name: "rhai_disc2", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let rot = p.rot;
    let twist = p.twist;
    let sinadd = sin(twist);
    let cosadd = cos(twist) - 1.0;
    if (twist > two_pi) {
        let k = 1.0 + twist - two_pi;
        cosadd = cosadd * k;
        sinadd = sinadd * k;
    }
    if (twist < -two_pi) {
        let k = 1.0 + twist + two_pi;
        cosadd = cosadd * k;
        sinadd = sinadd * k;
    }
    #{ timespi: rot * pi, cosadd: cosadd, sinadd: sinadd }
}

fn variation(z, p) {
    let pi = 3.14159265358979;
    let timespi = p.timespi;
    let cosadd = p.cosadd;
    let sinadd = p.sinadd;
    let t = timespi * (z.re + z.im);
    let r = atan2(z.re, z.im) / pi;
    complex((sin(t) + cosadd) * r, (cos(t) + sinadd) * r)
}
