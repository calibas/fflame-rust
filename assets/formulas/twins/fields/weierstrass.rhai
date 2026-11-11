// The built-in Weierstrass Field as a field of your own (the Escape
// panel's Edit as Rhai): the Besicovitch-Ursell lacunary surface
// F(x, y) = sum a^n g(b^n x + phase) g(b^n y + phase), a term an octave,
// with the gradient term by term for Hillshade's light. Each term is the
// built-in's arithmetic, in its order.

const PARAMS = #{
    a: #{
        value: 0.55, min: 0.05, max: 0.95,
        caption: "Amplitude Ratio",
        hint: "Per-octave amplitude factor a. Roughness rises toward 1 (the sum stays convergent below 1).",
    },
    b: #{
        value: 2.0, min: 1.2, max: 8.0,
        caption: "Frequency Ratio",
        hint: "Per-octave frequency factor b (the lacunary sequence bⁿ). Classic Weierstrass needs ab ≥ 1 for nowhere-differentiability.",
    },
    generator: #{
        value: 0,
        caption: "Generator",
        hint: "0 = cosine, 1 = sine, 2 = triangle wave (Takagi/blancmange).",
        choices: ["Cosine", "Sine", "Triangle (Takagi)"],
    },
    phase: #{ value: 0.0, min: 0.0, max: 6.28318, caption: "Phase", hint: "Phase offset added inside every octave's generator." },
};

const INFO = #{ title: "Weierstrass Field", gradient: true, coloring: "field_hillshade" };

// The generator at an angle already reduced to [-pi, pi], and its slope.
fn g(t, gen) {
    if gen == 0 {
        cos(t)
    } else if gen == 1 {
        sin(t)
    } else {
        // A triangle wave, period 2 pi, range [-1, 1].
        let f = fract(t * 0.15915494);
        1.0 - 4.0 * abs(f - 0.5)
    }
}

fn dg(t, gen) {
    if gen == 0 {
        -sin(t)
    } else if gen == 1 {
        cos(t)
    } else {
        // The triangle wave's slope: +-4 / 2 pi, by its half-period.
        let f = fract(t * 0.15915494);
        if f > 0.5 { -0.63661977 } else { 0.63661977 }
    }
}

// The running octave scales, a^n and b^n.
fn start(pos, p) {
    #{ pos: pos, an: 1.0, bn: 1.0, value: 0.0, grad: complex(0.0, 0.0) }
}

fn term(s, p) {
    let gen = p.generator;
    let tx = reduce(s.bn * s.pos.re + p.phase);
    let ty = reduce(s.bn * s.pos.im + p.phase);
    let gx = g(tx, gen);
    let gy = g(ty, gen);
    s.value = s.an * gx * gy;
    s.grad = complex(s.an * s.bn * dg(tx, gen) * gy, s.an * s.bn * gx * dg(ty, gen));
    s.an = s.an * p.a;
    s.bn = s.bn * p.b;
    s
}
