"""Generate src/escape/presets.rs from the visual-regression configs.

Those configs are already rendered and hash-compared every run, so a
preset built from one is known to produce a picture -- which is the
property that matters and the one invented values would not have.
"""
import json, io, os

# formula -> [(display name, config stem)], chosen for being a good
# STARTING POINT rather than a regression probe: the deep-zoom and
# threshold configs are excluded on purpose.
SELECTION = {
    "mandelbrot": [
        ("Smooth", "mandelbrot-smooth"),
        ("Seahorse Valley", "mandelbrot-seahorse-zoom"),
        ("Distance Estimate", "mandelbrot-de"),
        ("Orbit Trap", "mandelbrot-orbit-trap"),
        ("Stripe Average", "mandelbrot-stripe"),
        ("Normal Map Relief", "normal-map-relief"),
        ("Julia Dragon", "julia-dragon"),
    ],
    "multibrot": [("Power 4", "multibrot-4")],
    "burning_ship": [("Classic", "burning-ship"), ("Celtic", "ship-celtic")],
    "tricorn": [("Classic", "tricorn")],
    "phoenix": [("Classic", "phoenix-classic")],
    "manowar": [("Classic", "manowar")],
    "lambda": [("Parameter Plane", "lambda-plane"), ("Julia, Distance", "lambda-de-julia")],
    "lambda_sine": [("Parameter Plane", "lambda-sine-plane"), ("Bouquet", "lambda-sine-bouquet")],
    "feather": [("Classic", "feather")],
    "mcmullen": [("Carpet", "mcmullen-carpet")],
    "magnet": [("Magnet 1", "magnet-1")],
    "newton": [("Cubic Roots", "newton-3"), ("Relaxed", "newton-relaxed")],
    "nova": [("Nova 3", "nova-3"), ("Sphere Average", "nova4-sphere")],
    "novaretti": [
        ("Classic", "novaretti"),
        ("Period", "novaretti-period"),
        ("Julia Trap", "novaretti-julia"),
    ],
    "collatz": [("Classic", "collatz")],
    "ducks": [
        ("Parameter Plane", "ducks-param"),
        ("Julia", "ducks-julia"),
        ("Secant", "ducks-sec"),
    ],
    "kaliset": [("Glow", "kaliset-glow")],
    "origami": [
        ("Butterfly", "origami-butterfly"),
        ("Relief", "origami-relief"),
        ("Soft Relief", "origami-relief-soft"),
    ],
    "lattes": [("Variant 0", "lattes-v0"), ("Variant 2", "lattes-v2")],
    "barnsley": [("M3", "barnsley-m3"), ("M1 Julia", "barnsley-m1-julia")],
    "cactus": [("Classic", "cactus")],
    "exponential": [("Classic", "exponential")],
    "littlewood": [("Classic", "littlewood")],
    "spider": [("Classic", "spider")],
    "tetration": [("Classic", "tetration"), ("Period", "tetration-period")],
    "trig": [("Sine", "trig-sin")],
}

def rs_f32(v):
    s = f"{float(v):.6}"
    return s if ("." in s or "e" in s or "E" in s) else s + ".0"

def rs_f64(v):
    s = repr(float(v))
    return s if ("." in s or "e" in s or "E" in s) else s + ".0"

def pairs(d):
    if not d:
        return "&[]"
    inner = ", ".join(f'("{k}", {rs_f32(v)})' for k, v in sorted(d.items()))
    return "&[" + inner + "]"

out = []
out.append('''//! Named starting points for each escape formula.
//!
//! GENERATED from the visual-regression configs
//! (`tests/visual/configs/escape/`) by
//! `scripts/gen_escape_presets.py`, then committed. Those configs are
//! rendered and hash-compared on every suite run, so a preset built
//! from one is known to produce a picture — which is the property
//! that matters, and the one hand-invented values would not have.
//!
//! The FIRST preset of a formula is its default: what a switch to
//! that formula applies. Everything a formula needs to look like
//! itself travels together — view, iteration budget, coloring, and
//! both parameter sets — because that is exactly what does not carry
//! over from the formula you were just looking at.

use super::EscapePreset;
''')

for formula, items in SELECTION.items():
    const = formula.upper()
    lines = [f"pub static {const}: &[EscapePreset] = &["]
    for display, stem in items:
        path = f"tests/visual/configs/escape/{stem}.fflame"
        d = json.load(io.open(path, encoding="utf-8"))
        e = d.get("escape", {})
        assert e.get("formula", "mandelbrot") == formula, (stem, e.get("formula"))
        julia = e.get("julia", False)
        jr, ji = e.get("julia_re", 0.0), e.get("julia_im", 0.0)
        lines.append("    EscapePreset {")
        lines.append(f'        name: "{display}",')
        lines.append(f'        center_re: "{e.get("center_re", "0")}",')
        lines.append(f'        center_im: "{e.get("center_im", "0")}",')
        lines.append(f'        zoom_log2: {rs_f64(e.get("zoom_log2", 0.0))},')
        lines.append(f'        max_iter: {int(e.get("max_iter", 256))},')
        lines.append(f'        coloring: "{e.get("coloring", "smooth")}",')
        lines.append(
            f'        julia: {f"Some(({rs_f32(jr)}, {rs_f32(ji)}))" if julia else "None"},'
        )
        lines.append(f'        formula_params: {pairs(e.get("formula_params"))},')
        lines.append(f'        coloring_params: {pairs(e.get("coloring_params"))},')
        lines.append("    },")
    lines.append("];")
    out.append("\n".join(lines))

io.open("src/escape/presets.rs", "w", encoding="utf-8", newline="\n").write(
    "\n\n".join(out) + "\n"
)
print("wrote src/escape/presets.rs with", sum(len(v) for v in SELECTION.values()),
      "presets across", len(SELECTION), "formulas")
