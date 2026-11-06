// Whorl, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    inside: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Inside",
        hint: "Angular-shift coefficient applied where `r < w`.",
    },
    outside: #{
        value: 0.2, min: -10.0, max: 10.0, free: true,
        caption: "Outside",
        hint: "Angular-shift coefficient applied where `r ≥ w`.",
    },
};

const INFO = #{ title: "Whorl", name: "rhai_whorl" };

fn variation(z, p, ctx) {
    let inside = p.inside;
    let outside = p.outside;
    let w = ctx.weight;
    let r = sqrt(z.re * z.re + z.im * z.im) + 1.0e-6;
    let denom = w - r;
    let safe_denom = (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
    let a = atan2(z.re, z.im);
    if (r < w) {
        a = a + inside / safe_denom;
    } else {
        a = a + outside / safe_denom;
    }
    complex(r * cos(a), r * sin(a))
}
