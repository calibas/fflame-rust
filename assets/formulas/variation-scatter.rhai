// A flame variation that scatters each point into a ring around it and
// colours it by its distance from the centre, in bands.
//
// `rand()` draws a number in [0, 1) from the flame's own stream. Ending
// with a map, `#{ z: point, color: index }`, also gives the point a
// palette index, which the transform's Direct Color blends in.

const PARAMS = #{
    inner: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Inner", hint: "The ring's inner radius" },
    outer: #{ value: 0.04, min: 0.0, max: 2.0, caption: "Outer", hint: "The ring's outer radius" },
    bands: #{ value: 1.5, min: 0.0, max: 10.0, caption: "Bands", hint: "How many palette cycles per unit of distance from the centre" },
};

const INFO = #{ title: "Scatter (Rhai)", name: "rhai_scatter" };

fn variation(z, p) {
    let a = rand() * 2.0 * PI();
    let r = p.inner + (p.outer - p.inner) * rand();
    let k = z.abs() * p.bands;
    #{ z: z + complex(r * cos(a), r * sin(a)), color: k - floor(k) }
}
