// The built-in Diverging colouring as a field colouring of your own (the
// Escape panel's Edit as Rhai): a signed field on a diverging palette --
// zero mid-palette, the sign picking the half, an atan squash keeping the
// extremes inside it.

const PARAMS = #{
    scale: #{
        value: 3.0, min: 0.01, max: 100.0,
        caption: "Scale",
        hint: "Sensitivity: how fast the field value saturates toward the palette's ends.",
    },
};

const INFO = #{ title: "Diverging" };

fn shade(sum, grad, n, p) {
    // (-inf, inf) to (0, 1), 0 at exactly 0.5; the 0.999 keeps the
    // wrapped palette's two ends from meeting.
    0.5 + 0.999 * atan(sum * p.scale) / 3.14159265
}
