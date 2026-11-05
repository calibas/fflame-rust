// External Rays, the built-in colouring's twin (Fraktaler 3's
// `flying-fish` example): 1 where the escaped z's angle, in turns, lies
// within a band of a whole number of turns -- the external rays -- and 0
// off them. The band narrows by (1 / degree)^f, f the escape fraction,
// so a ray stays continuous across the escape bands as the angle
// doubles.

const PARAMS = #{
    width: #{
        value: 0.05, min: 0.001, max: 0.5,
        caption: "Width",
        hint: "Half the width of a ray, in turns of the escape angle, where the orbit has just escaped (Fraktaler 3 draws 0.05).",
    },
    count: #{
        value: 1.0, min: 1.0, max: 64.0,
        caption: "Rays",
        hint: "How many rays per turn of the angle: 1 is the one through angle 0, 2 adds the one through a half turn, and so on.",
    },
};

const INFO = #{ title: "External Rays", region: "outside", bounded: true, bailout: 1.0e4 };

fn finish(s, z, p, ctx) {
    let m = max(round_even(p.count), 1.0);
    let t = z.arg() * 0.15915494;
    // The distance in turns from the nearest k / m.
    let off = abs(fract(m * t + 0.5) - 0.5) / m;
    let band = p.width * pow(1.0 / max(ctx.degree, 1.0001), ctx.escape_fraction);
    if off < band { 1.0 } else { 0.0 }
}
