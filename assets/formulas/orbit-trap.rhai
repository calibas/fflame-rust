// An orbit trap, as a Rhai colouring: how close each pixel's orbit
// comes to a point, kept in the state from pass to pass. Where it comes
// closer than `Width`, the pixel is left solid.

const PARAMS = #{
    point: #{ value: complex(0.0, 0.0), caption: "Trap point" },
    width: #{ value: 0.0, min: 0.0, caption: "Width", hint: "Closer than this leaves the pixel solid" },
    scale: #{ value: 2.0, caption: "Scale" },
};

const INFO = #{ title: "Point Trap" };

fn start(z, p) {
    #{ closest: (z - p.point).abs() }
}

fn step(s, z, p) {
    let d = (z - p.point).abs();
    if d < s.closest {
        s.closest = d;
    }
    s
}

fn finish(s, z, p) {
    if s.closest < p.width {
        #{ solid: true }
    } else {
        s.closest.sqrt() * p.scale
    }
}
