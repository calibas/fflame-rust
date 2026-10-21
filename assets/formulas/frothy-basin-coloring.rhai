// The Frothy Basin colouring: which attractor a pixel's orbit landed on,
// a band of the palette each, and how soon, a shade within the band --
// Fractint's colouring of its frothybasin type (Xfractint 20.04p16,
// common/miscfrac.c, calcfroth's 256-colour branch). Fractint drew it
// with its froth3 and froth6 maps, a hue to a band; any palette of three
// (or six) parts reads as they did.
//
// The formula's `a` and Mapping decide the lines the orbits land on; a
// colouring reads only the orbit, so it takes them again: set them as
// the formula's. A pixel that left, or never landed, takes the solid
// colour (Fractint's colour 0).

const PARAMS = #{
    a: #{ value: 1.028713768218725, caption: "a", hint: "The formula's a" },
    mapping: #{ value: 0, caption: "Mapping", choices: ["Once (3 basins)", "Twice (6 basins)"], hint: "The formula's Mapping" },
    shade: #{ value: 0, caption: "Shade", choices: ["By count, to the band's length", "By count over the iteration limit"], hint: "Fractint's two schemes: the count up to the band's length (85 colours of 256 for three basins), or the count over the whole iteration limit (its alternate colouring)" },
};

const INFO = #{ title: "Frothy Basin", region: "outside" };

// Which attractor's line `z` has landed on, 1 to 6, or 0 for none:
// Fractint's test and its numbering, on the formula's lines and marks
// (frothy-basin.rhai, `froth_landed`).
fn froth_attractor(z, a, twice) {
    let close = 1.0e-6;
    let slope = sqrt(3.0);
    // Fractint's critical a, 1.0287137682187249: a parameter is kept in
    // f32, which rounds that value itself just past it, so the test
    // allows the rounding -- at the default there are three basins.
    let three = a.abs() <= 1.02871388;
    let n = if three { if twice { 6 } else { 3 } } else { if twice { 3 } else { 2 } };
    let k = 0.75 * a * a;
    let t1 = 0.25 - 0.5 - k;
    let t2 = t1 * t1 - t1 - k;
    let t3 = t2 * t2 - t2 - k;
    let t4 = t3 * t3 - t3 - k;
    let y0 = a / 2.0;
    let s = slope / 2.0;
    let x = z.re;
    let y = z.im;
    if (y0 - y).abs() < close && x >= t1 && x <= t2 {
        if (!twice && n == 2) || (twice && n == 3) {
            1
        } else if x <= t3 {
            1
        } else if x >= t4 {
            if twice { 2 } else { 1 }
        } else {
            0
        }
    } else if (slope * x - a - y).abs() < close && x <= -0.5 * t1 + y0 * s && x >= -0.5 * t2 + y0 * s {
        if !twice && n == 2 {
            2
        } else if twice && n == 3 {
            3
        } else if x >= -0.5 * t3 + y0 * s {
            if twice { 4 } else { 2 }
        } else if x <= -0.5 * t4 + y0 * s {
            if twice { 6 } else { 3 }
        } else {
            0
        }
    } else if (-slope * x - a - y).abs() < close && x <= -0.5 * t1 - y0 * s && x >= -0.5 * t2 - y0 * s {
        if !twice && n == 2 {
            2
        } else if twice && n == 3 {
            2
        } else if x >= -0.5 * t3 - y0 * s {
            if twice { 5 } else { 3 }
        } else if x <= -0.5 * t4 - y0 * s {
            if twice { 3 } else { 2 }
        } else {
            0
        }
    } else {
        0
    }
}

fn finish(s, z, p, ctx) {
    let twice = p.mapping == 1;
    let basin = froth_attractor(z, p.a, twice);
    // Fractint's bands: 255 colours over at least three, its two- and
    // three-basin pictures alike; a pixel's shade the count, at least 1.
    let three = p.a.abs() <= 1.02871388;
    let bands = if three && twice { 6.0 } else { 3.0 };
    let shades = if bands == 6.0 { 42.0 } else { 85.0 };
    let n = to_float(ctx.numiter);
    let count = if p.shade == 0 {
        if n > shades { shades } else { n }
    } else {
        (shades * n / to_float(ctx.maxiter)).floor()
    };
    let shade = if count < 1.0 { 1.0 } else { count };
    if basin == 0 {
        #{ index: 0.0, solid: true }
    } else {
        #{ index: (shade + shades * to_float(basin - 1)) / 256.0, solid: false }
    }
}
