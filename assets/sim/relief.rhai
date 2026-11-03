// The relief stage's height field lit: a tilt toward a light, or a
// Lambert shade from a light at an elevation, grey about mid-grey for a
// blend over another colouring.
//
// The Rhai twin of the built-in `relief` colouring
// (src/sim/colorings.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters and the same picture. `channel` and `softness` are the
// relief stage's: what it smooths into the height this reads.

const PARAMS = #{
    channel: #{
        value: 0, choices: ["x", "y", "z", "w"], caption: "Height from",
        hint: "Which channel of the source layer is the height. McCabe: x is the pattern, z the step its winning scale last changed (age).",
    },
    softness: #{
        value: 2.0, min: 0.0, max: 8.0, caption: "Softness",
        hint: "Gaussian smoothing of the height, in cells, before its slope is taken. Raw, the finest detail's speckle dominates; 2 gives raised cells with their nested texture.",
    },
    height: #{
        value: 12.0, min: 0.0, max: 100.0, caption: "Height",
        hint: "Vertical exaggeration of the slope. Its useful range depends on the model's values: McCabe's field runs -1 to 1.",
    },
    light: #{
        value: 135.0, min: 0.0, max: 360.0, caption: "Light angle",
        hint: "Where the light comes from, in degrees counter-clockwise from east: 135 is the upper left, the cartographic convention. Lit from the other side, raised detail reads as sunken.",
    },
    model: #{
        value: 0, choices: ["Tilt", "Lambert"], caption: "Lighting",
        hint: "Tilt: the slope toward the light, symmetric in light and shade. Lambert: the light raised by the elevation, so slopes facing away fall into shadow sooner the lower it is.",
    },
    elevation: #{
        value: 30.0, min: 1.0, max: 89.0, caption: "Light elevation",
        hint: "The light's height above the horizon in degrees, for Lambert.",
    },
    shadow: #{
        value: 0.8, min: 0.0, max: 1.0, caption: "Shadow",
        hint: "How dark the side facing away from the light goes.",
    },
    highlight: #{
        value: 0.6, min: 0.0, max: 1.0, caption: "Highlight",
        hint: "How light the side facing the light goes.",
    },
};
const INFO = #{
    title: "Relief",
    id: "relief",
    description: "One channel as a height, smoothed and lit: grey, centred on mid-grey, for the colour stack under Hard light, where it shades whatever is beneath. Add it with Add relief.",
};

fn cell_color(cell, p) {
    let gx = -cell.relief.y * p.height;
    let gy = cell.relief.z * p.height;
    let a = p.light * (3.14159265358979 / 180.0);
    let lx = cos(a);
    let ly = sin(a);
    let norm = 1.0 / sqrt(1.0 + (gx * gx + gy * gy));
    let response = (gx * lx + gy * ly) * norm;
    if p.model == 1 {
        let e = clamp(p.elevation, 1.0, 89.0) * (3.14159265358979 / 180.0);
        let se = sin(e);
        let ce = cos(e);
        let lambert = max((gx * (lx * ce) + gy * (ly * ce) + se) * norm, 0.0);
        response = if lambert >= se { (lambert - se) / max(1.0 - se, 0.0001) } else { (lambert - se) / max(se, 0.0001) };
    }
    let hi = clamp(response, 0.0, 1.0) * p.highlight;
    let lo = clamp(-response, 0.0, 1.0) * p.shadow;
    let v = 0.5 + 0.5 * (hi - lo);
    rgba(v, v, v, 1.0)
}
