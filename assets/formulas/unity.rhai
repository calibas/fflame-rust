// Unity, Mark Peterson's Fractint type: "you won't find this in any
// fractal books unless they saw it here first - Mark invented it!"
// Ported from Xfractint 20.04p16 (common/fractals.c, UnityfpFractal; its
// table entry in common/fractalp.c).
//
// Each pass takes r^2 = x^2 + y^2, then y as (2 - r^2) x, and x as
// (2 - r^2) times that new y -- Fractint's order, kept. A pixel stops
// when r^2 passes 2, or when its orbit comes within a pixel of the unit
// circle: |r^2 - 1| under Fractint's `ddelmin`, the pixel's size. Here
// the pixel is a row of the view at Rows rows (Fractint's 480 by
// default), so the picture is Fractint's at that height whatever the
// window's, and deep zoom derives its equations (it reads no image
// size). The count of passes is the picture. Fractint tests a point before it maps
// it, the pixel itself first: each pass keeps the point it mapped
// (`s.prev`) and the test reads that, so the count is Fractint's --
// outside radius sqrt 2 every pixel stops on the first pass, as there --
// and deep zoom carries it as a convergent formula's previous z.

const PARAMS = #{
    rows: #{ value: 480.0, min: 50.0, max: 4000.0, caption: "Rows", hint: "The picture height whose pixel is the unit circle's tolerance (Fractint's 480 at 640 x 480): fewer rows, a wider ring of settled orbits" },
};

const INFO = #{ title: "Unity", center: complex(0.0, 0.0), magn: 1.3333333333333333, maxiter: 150 };

fn start(pixel, p, ctx) {
    #{ z: pixel, prev: pixel }
}

fn step(s, p) {
    s.prev = s.z;
    let r2 = s.z.norm_sqr();
    let y = (2.0 - r2) * s.z.re;
    let x = (2.0 - r2) * y;
    s.z = complex(x, y);
    s
}

fn escaped(s, p, ctx) {
    let r2 = s.prev.norm_sqr();
    r2 > 2.0 || (r2 - 1.0).abs() < 4.0 / (ctx.magn * p.rows)
}
