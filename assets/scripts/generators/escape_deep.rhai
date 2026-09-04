// Escape-time deep dive: a known-good location for each formula that
// can actually zoom, with the iteration budget scaled to the depth.
//
// The locations are not random. A randomly chosen centre at zoom 25 is
// almost always a flat wash — deep structure lives on the boundary,
// which is a measure-zero set — so each of these was rendered and
// checked against an exact orbit before being written down here.
//
// Only formulas with a perturbation tier are offered. The other
// seventeen stop resolving around zoom 14, where the direct path's f32
// pixel mapping runs out, and a "deep dive" that quietly stops going
// deeper is worse than one that does not offer the choice.

script("Escape deep dive", "generator");

let which = param_choice("formula",
    ["mandelbrot", "phoenix", "manowar", "tricorn", "burning_ship"], 0);
let depth = param("zoom", 20.0, 4.0, 40.0);
let detail = param_int("iterations", 4000, 200, 60000);
let style = param_choice("coloring",
    ["smooth", "escape_count", "stripe_average", "distance_estimate"], 0);

escape.formula(which);
escape.coloring(style);
escape.zoom = depth;
escape.max_iter = detail;

// Centre per formula. Strings, not numbers: past about zoom 50 an f64
// centre has fewer digits than the view needs, and the picture stops
// sharpening without saying why.
if which == "mandelbrot" {
    // Seahorse valley: filigree at every depth.
    escape.center("-0.74364388703715", "0.13182590420531");
} else if which == "phoenix" {
    escape.center("-1.1543534481639833789918460777111",
                  "0.6293282132782021964135047790493");
    escape.param("p_re", -0.5);
} else if which == "manowar" {
    escape.center("-0.03909223238627110991", "0.00103869199752806211");
} else if which == "tricorn" {
    escape.center("-0.90755797705302632", "0.10050898208800299");
} else {
    escape.center("-1.5716", "0.0");
}

// The smooth value runs with the iteration COUNT, not with the cap:
// most pixels escape in a few hundred iterations however high the cap
// is set, so scaling by 1/max_iter lands the whole image in one colour
// band. 0.02 gives a few palette cycles over a typical count range,
// which is what the shipped escape configs use.
if style == "smooth" || style == "escape_count" {
    escape.coloring_param("scale", 0.02);
}
