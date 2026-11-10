// The built-in Standard Map FTLE as a field of your own (the Escape
// panel's Edit as Rhai): the Chirikov standard map from the point
// (theta, I) -- I <- I + K sin theta, theta <- theta + I -- carrying the
// tangent 2x2 through its Jacobian, renormalized each term; the field is
// the mean log growth, near 0 on the islands and positive in the sea.

const PARAMS = #{
    k: #{
        value: 1.0, min: 0.0, max: 10.0,
        caption: "Kick Strength",
        hint: "Chirikov K. Islands dominate below ~0.97; the chaotic sea takes over above it.",
    },
};

const INFO = #{ title: "Standard Map FTLE", coloring: "field_diverging" };

// The point, and the tangent matrix, the identity.
fn start(pos, p) {
    #{ theta: pos.re, i: pos.im, m00: 1.0, m01: 0.0, m10: 0.0, m11: 1.0, value: 0.0 }
}

fn term(s, p, ctx) {
    let k = p.k;
    // theta grows without bound, so it is reduced as the built-in's is.
    let th = reduce(s.theta);
    let i_new = s.i + k * sin(th);
    let t_new = s.theta + i_new;
    // The Jacobian at the point before the step; M <- J M.
    let kc = k * cos(th);
    let j00 = 1.0 + kc;
    let n00 = j00 * s.m00 + 1.0 * s.m10;
    let n01 = j00 * s.m01 + 1.0 * s.m11;
    let n10 = kc * s.m00 + 1.0 * s.m10;
    let n11 = kc * s.m01 + 1.0 * s.m11;
    let nrm = max(max(abs(n00), abs(n01)), max(abs(n10), abs(n11)));
    let scale = max(nrm, 1.0e-20);
    s.value = ln(scale) / to_float(max(u32(ctx.maxiter), u32(1)));
    s.theta = t_new;
    s.i = i_new;
    s.m00 = n00 / scale;
    s.m01 = n01 / scale;
    s.m10 = n10 / scale;
    s.m11 = n11 / scale;
    s
}
