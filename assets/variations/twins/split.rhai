/// Coin-flip mirror per axis — flips Y based on the sign of
/// `cos(x · xsize · π)`, and flips X based on the sign of
/// `cos(y · ysize · π)`. Produces a tiled mirroring pattern whose tile
/// spacing is controlled by `xsize` and `ysize`.
///
/// # Authors
/// - Apophysis Plugin Pack

// Split, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xsize: #{
        value: 0.4, min: -10.0, max: 10.0, free: true,
        caption: "X size",
        hint: "X-axis frequency of the mirror-flip cosine.",
    },
    ysize: #{
        value: 0.6, min: -10.0, max: 10.0, free: true,
        caption: "Y size",
        hint: "Y-axis frequency of the mirror-flip cosine.",
    },
};

const INFO = #{ title: "Split", name: "rhai_split", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let xsize = p.xsize;
    let ysize = p.ysize;
    let pi = 3.14159265358979;
    let oy = z.im;
    if (cos(z.re * xsize * pi) < 0.0) {
        oy = -z.im;
    }
    let ox = z.re;
    if (cos(z.im * ysize * pi) < 0.0) {
        ox = -z.re;
    }
    complex(ox, oy)
}
