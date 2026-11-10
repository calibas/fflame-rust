// The built-in Markus–Lyapunov as a field of your own (the Escape panel's
// Edit as Rhai): the logistic map x <- r x (1 - x) with r switching
// between A (the point's x) and B (its y) along a periodic letter
// sequence; the field is the Lyapunov exponent, the mean of
// log |r (1 - 2x)| after a transient -- below zero stable, above it chaos.

const PARAMS = #{
    seq_bits: #{
        value: 2.0, min: 0.0, max: 1023.0,
        caption: "Sequence Bits",
        hint: "The A/B forcing sequence as a bit pattern (bit n of the integer: 0 = A, 1 = B), read LSB-first over the sequence length. 2 with length 2 = the classic \"AB\".",
    },
    seq_len: #{
        value: 2.0, min: 1.0, max: 10.0,
        caption: "Sequence Length",
        hint: "How many bits of the pattern form the repeating sequence.",
    },
    warmup: #{
        value: 50.0, min: 0.0, max: 500.0,
        caption: "Warmup",
        hint: "Transient iterations discarded before the exponent accumulates (lets the orbit settle onto its attractor).",
    },
};

const INFO = #{ title: "Markus–Lyapunov", coloring: "field_diverging" };

fn start(pos, p) {
    #{ pos: pos, x: 0.5, n: 0, value: 0.0 }
}

fn term(s, p, ctx) {
    // Which letter this term's r is: bit (n mod length) of the pattern.
    let len = max(u32(p.seq_len), u32(1));
    let bits = u32(p.seq_bits);
    let use_b = ((bits >> (u32(s.n) % len)) & u32(1)) != u32(0);
    let r = if use_b { s.pos.im } else { s.pos.re };
    let x = s.x;
    let x_next = clamp(r * x * (1.0 - x), 0.0, 1.0);
    let warmup = u32(p.warmup);
    let terms = u32(ctx.maxiter);
    s.value = 0.0;
    if u32(s.n) >= warmup && terms > warmup {
        // Normalized as it goes: the sum is the mean exponent.
        let d = abs(r * (1.0 - 2.0 * x_next));
        s.value = ln(max(d, 1.0e-20)) / to_float(terms - warmup);
    }
    s.x = x_next;
    s.n += 1;
    s
}
