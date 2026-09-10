// Simulation Sweep
//
// Starts from a measured preset, wanders its parameters, and runs the
// grid. This is the simulation counterpart to the flame generators
// beside it: same script(), same param() sliders, same seeded
// randomness — it drives the sim global instead of the flame one.
//
// Run it from the command line with:
//   FractalArtEditor generate --script sim_sweep.rhai --seed 42

script("Simulation Sweep", "generator");

// ---------------------------------------------------------------- //
// What to sweep.
//
// Gray-Scott is the default because its two numbers — feed and kill —
// are the most-studied parameter plane in the subject, and small moves
// in it change the pattern completely: spots, a maze, dividing cells,
// or a field that dies to uniform grey.
//
// Every model in the registry works here. `sim.models()` lists all 31;
// docs/projects/simulation-catalog.md says what each one's parameters
// mean and where the rule comes from.
// ---------------------------------------------------------------- //
let model = param_choice("model", [
    "gray_scott", "brusselator", "fitzhugh_nagumo", "swift_hohenberg",
    "cahn_hilliard", "hodgepodge",
], 0);

// The grid is the information, not the resolution. A bigger grid is
// not a bigger picture of the same pattern — it is MORE pattern at the
// same feature size, because the cell is the unit the rule works in.
let grid = param_int("grid", 512, 128, 2048);

// Multiplies the preset's measured step count. Reaction-diffusion
// needs thousands of steps before the pattern is the pattern.
let run_length = param("run_length", 1.0, 0.25, 4.0);

// How far to wander from the preset. 0 reproduces it exactly, which is
// the honest starting point — change it once you have seen what the
// preset does on its own.
let spread = param("spread", 0.3, 0.0, 1.0);

// param_choice returns the chosen option's TEXT, not its index.
let palette_mode = param_choice("palette", ["Generated", "Keep current"], 0);

// ---------------------------------------------------------------- //
// Start from a preset, not from nothing.
//
// A model's parameters are not independent knobs. Most of the plane is
// uniform grey and the interesting region is a thin curve through it,
// so a script that picked numbers at random would almost always
// produce a flat field. Every preset in the registry has been RUN and
// inspected, and carries its own measured step count, its dt and the
// colouring its state layout wants.
// ---------------------------------------------------------------- //
sim.model(model);

// The real preset names, per model.
let presets = #{
    gray_scott:      ["coral", "maze", "worms", "mitosis"],
    brusselator:     ["turing_spots", "oscillating"],
    fitzhugh_nagumo: ["spiral"],
    swift_hohenberg: ["labyrinth", "spots"],
    cahn_hilliard:   ["labyrinth", "droplets"],
    hodgepodge:      ["spirals", "dewdney"],
};

let chosen = pick(presets[model]);
sim.preset(chosen);
print("preset: " + chosen);

// ---------------------------------------------------------------- //
// The sweep.
//
// Multiplicative, not additive: a feed rate of 0.055 and a diffusion
// of 0.16 are not the same KIND of number, and adding one delta to
// both would move one imperceptibly and push the other out of the
// stable region entirely.
//
// Reading each parameter back rather than hard-coding a default keeps
// this honest when a preset or a registry default changes — and it is
// the preset's value we want to wander from, not the model's.
//
// Written inline rather than as a `fn`: a Rhai function cannot see
// script variables, so `config` and `sim` are simply not in scope
// inside one. (Worth knowing before you factor a loop out into a
// helper and watch it throw.)
// ---------------------------------------------------------------- //

// Which parameters exist depends on the model. These are the ones that
// move the picture, under the names the registry actually uses — the
// papers' letters and the code's names are not always the same.
let sweepable = #{
    gray_scott:      ["feed", "kill"],
    brusselator:     ["feed_a", "feed_b"],
    fitzhugh_nagumo: ["a", "b", "tau"],
    swift_hohenberg: ["wavelength", "drive", "asymmetry"],
    cahn_hilliard:   ["mobility", "gamma"],
    hodgepodge:      ["k1", "k2", "g"],
};

// How far each model can be pushed before its pattern dies, MEASURED
// by rendering each preset at a range of jitters and checking the
// image is not one flat colour. These are the fractions at spread = 1.
//
// Gray-Scott is the outlier by more than an order of magnitude, and
// that is the subject rather than an accident: its interesting region
// is a thin curve through a plane that is otherwise uniform grey.
// Measured on `mitosis`, +2% on feed and kill together is already a
// blank image, while `coral` survives +5% and dies by +8%. The others
// hold their pattern at ±30% — Cahn-Hilliard barely changes at all,
// because its parameters set the RATE of coarsening and the run
// simply gets further along.
let jitter_scale = #{
    gray_scott:      0.02,
    brusselator:     0.30,
    fitzhugh_nagumo: 0.30,
    swift_hohenberg: 0.25,
    cahn_hilliard:   0.30,
    hodgepodge:      0.30,
};

let amount = spread * jitter_scale[model];
if amount > 0.0 {
    print("sweeping (+/-" + (amount * 100.0).to_int() + "%):");
    for name in sweepable[model] {
        let key = "sim.model_params." + name;
        // A value equal to the model's default is omitted from the
        // saved config, so a miss here means "this preset left it at
        // the default" and there is nothing to wander FROM. Caught
        // narrowly and reported — a bare `catch { }` wrapped around a
        // whole block will happily swallow a typo and leave you
        // wondering why nothing moved. (It did, while this was
        // written.)
        let base = ();
        try { base = config.get(key); } catch { print("  " + name + ": preset left it at the default, skipped"); }
        if base != () {
            let v = base * (1.0 + rand(-amount, amount));
            sim.param(name, v);
            print("  " + name + ": " + base + " -> " + v);
        }
    }
}

// ---------------------------------------------------------------- //
// The run.
// ---------------------------------------------------------------- //
sim.grid(grid, grid);
sim.seed(rand_int(1, 1000000));

// NOT setting init here, deliberately. The preset already chose one
// and its choice is measured: FitzHugh-Nagumo's constants give spirals
// from a cut wavefront and a FLAT FIELD from noise, so overriding the
// preset's init with a favourite is how you ship a picture of nothing.
// Same for the boundary and the colouring.
//
// If you want a different starting field, say so AFTER the preset and
// know that you are overriding a measurement:
//   sim.init("blobs");        // several patches, not per-cell noise
//   sim.boundary("zero");     // a sink — growth models want this

// Scale the preset's measured count rather than replacing it.
// `config.get` throws on a value equal to its default, so guard it —
// see the gotchas in SCRIPTING-GUIDE.md.
let steps = 4000;
try { steps = config.get("sim.steps"); } catch { }
let final_steps = (steps.to_float() * run_length).to_int();
sim.steps(final_steps);
print("steps: " + final_steps);

if palette_mode == "Generated" {
    // A simulation colouring maps a channel through the palette, so
    // the palette IS the look — more directly than in a flame, where
    // the structure carries most of it. Analogous schemes read as a
    // material; complementary ones separate the two species.
    run_script("random_palette", #{
        scheme: pick(["Analogous", "Monochromatic", "Complementary"]),
    });
}
