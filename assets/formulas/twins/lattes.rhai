// The built-in Lattès, rational maps that are chaotic on the whole
// sphere, as a formula of your own (the Escape panel's Edit as Formula).
// The map has no c: the pixel is the start, on either plane. It never
// escapes: the colourings that read the orbit draw it.

const PARAMS = #{
    variant: #{
        value: 0,
        caption: "Variant",
        hint: "0: (z^2-a)^2 / 4z(z-1)(z-a). 1: (z + 1/z)/2i, which uses no a. 2: (z^3+a)/(a z^3+1).",
        choices: ["(z^2-a)^2 / 4z(z-1)(z-a)", "(z + 1/z)/2i", "(z^3+a)/(a z^3+1)"],
    },
    a_re: #{ value: -0.5, min: -4.0, max: 4.0, caption: "a (re)", hint: "The map's modulus. Variant 2 is a Lattès map at the cube root of unity exp(2*pi*i/3) = -0.5 + 0.866i, which is the default; variant 0 is one for any a outside {0, 1}." },
    a_im: #{ value: 0.8660254, min: -4.0, max: 4.0, caption: "a (im)", hint: "Imaginary part of the modulus." },
};

const INFO = #{ title: "Lattès", center: complex(0.0, 0.0), maxiter: 5 };

fn start(pixel, p) {
    #{ z: pixel }
}

fn step(s, p) {
    let z = s.z;
    let a = complex(p.a_re, p.a_im);
    let r2 = z.norm_sqr();
    s.z = if r2 > 1e12 {
        // Past the built-in's bound, each map's value at infinity.
        if p.variant == 0 {
            z * 0.25
        } else if p.variant == 1 {
            z * complex(0.0, -0.5)
        } else {
            1.0 / a
        }
    } else if p.variant == 1 {
        if r2 < 1e-30 { complex(1e7, 0.0) } else { (z + 1.0 / z) * complex(0.0, -0.5) }
    } else {
        let num = if p.variant == 0 { (z * z - a) * (z * z - a) } else { z * z * z + a };
        let den = if p.variant == 0 { 4.0 * ((z * (z - 1.0)) * (z - a)) } else { a * (z * z * z) + 1.0 };
        // The built-in's guard at a pole.
        if den.norm_sqr() < 1e-30 { complex(1e7, 0.0) } else { num / den }
    };
    s
}
