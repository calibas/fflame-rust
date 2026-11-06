/// Radial shred — splits the angular space into `n` wedges of width `2π/n`,
/// then within each wedge compresses the angular position by `width`. The
/// radius passes through unchanged. Radial analogue of `shredlin`.
///
/// # Authors
/// - Zy0rg

// Shredrad, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    n: #{
        value: 4.0, min: 0.001, max: 100.0, free: true,
        caption: "N",
        hint: "Number of angular wedges.",
    },
    width: #{
        value: 0.5, min: -1.0, max: 1.0,
        caption: "Width",
        hint: "Intra-wedge compression. 1 = no shred; 0 = collapse to wedge boundary.",
    },
};

const INFO = #{ title: "Shredrad", name: "rhai_shredrad", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let n = p.n;
    let safe_n = (if abs(n) < 1.0e-6 { 1.0e-6 } else { n });
    let two_pi = 6.28318530717959;
    #{ sa: two_pi / safe_n }
}

fn variation(z, p) {
    let sw = p.width;
    let sa = p.sa;
    let pi = 3.14159265358979;
    let three_pi = 9.42477796076938;
    let ang = atan2(z.re, z.im);
    let rad = sqrt(z.re * z.re + z.im * z.im) + 1.0e-6;
    let safe_sa = (if abs(sa) < 1.0e-30 { 1.0e-30 } else { sa });
    let xang = (ang + three_pi + sa * 0.5) / safe_sa;
    let xang_f = to_float(trunc(xang));
    let zang = ((xang - xang_f) * sw + xang_f) * sa - pi - sa * 0.5 * sw;
    complex(rad * cos(zang), rad * sin(zang))
}
