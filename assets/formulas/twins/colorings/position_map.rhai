// Position Map, the built-in colouring's twin (McCabe's origami
// colouring, as McDonald ported it): a colour source projected onto the
// folded paper -- here a plasma, two sine waves at the orbit's final
// position -- shifted by the orbit's branch address, a binary fraction
// of which steps moved the point.

const PARAMS = #{
    freq_x: #{
        value: 3.0, min: 0.05, max: 64.0,
        caption: "Frequency X",
        hint: "Horizontal frequency of the projected colour source, in cycles per unit of the folded plane.",
    },
    freq_y: #{
        value: 2.0, min: 0.05, max: 64.0,
        caption: "Frequency Y",
        hint: "Vertical frequency of the projected colour source.",
    },
    address_mix: #{
        value: 0.0, min: 0.0, max: 8.0,
        caption: "Address mix",
        hint: "How strongly the orbit's branch address (which iterations moved the point) shifts the palette. This is the channel that keeps detail alive under deep zoom; 0 is the pure projected source.",
    },
};

const INFO = #{ title: "Position Map", bounded: true };

fn start(z, p) { #{ address: 0.0 } }

// 0.5 into the low bit when this step moved the point: an unmoved point
// comes back from the formula exact, so the compare is reliable.
fn step(s, z, p, ctx) {
    let moved = if z.re != ctx.prev.re || z.im != ctx.prev.im { 0.5 } else { 0.0 };
    s.address = s.address * 0.5 + moved;
    s
}

fn finish(s, z, p) {
    let plasma = 0.5
        + 0.25 * sin(6.2831853 * p.freq_x * z.re)
        + 0.25 * sin(6.2831853 * (p.freq_y * z.im + 0.3));
    fract(plasma + s.address * p.address_mix)
}
