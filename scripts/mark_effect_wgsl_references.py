"""Mark each one-pass effect's WGSL as the reference its Rhai twin replaced.

One-shot codemod for docs/archive/projects/rhai-plugins.md, P2 step 7: the
built-in effects are their Rhai twins (assets/effects/*.rhai) in every
build with the compiler, so an edit to the WGSL no longer reaches the
app. A note after each WGSL file's title line says so, and each twin's
header says it is the built-in. Idempotent: a file already rewritten is
left alone.

    python scripts/mark_effect_wgsl_references.py
"""

import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent

# shader file -> the twin's name (hue_cycle.wgsl is the `hue_shift` effect).
# The replaced ones only (effects::custom::BUILTIN_TWINS); one pending
# (PENDING_TWINS) has its WGSL still what the app runs.
TWINS = {
    "color/vignette.wgsl": "vignette",
    "color/film_grain.wgsl": "film_grain",
    "color/chromatic_aberration.wgsl": "chromatic_aberration",
    "color/sobel_edges.wgsl": "sobel_edges",
    "color/hue_cycle.wgsl": "hue_shift",
    "color/plasma.wgsl": "plasma",
    "color/tunnel.wgsl": "tunnel",
    "color/kaleidoscope.wgsl": "kaleidoscope",
    "color/julia.wgsl": "julia",
    "color/worley_noise.wgsl": "worley_noise",
    "color/simplex_noise.wgsl": "simplex_noise",
    "color/domain_warp.wgsl": "domain_warp",
    "density/density_blur.wgsl": "density_blur",
    "density/bilateral_blur.wgsl": "bilateral_blur",
    "density/sharpen.wgsl": "sharpen",
}

MARK = "The REFERENCE, not what the app runs"


def note(name: str, eol: str) -> str:
    lines = [
        "//",
        f"// {MARK}: the built-in is its Rhai twin,",
        f"// assets/effects/{name}.rhai (docs/main/EFFECTS.md, Rhai effects). This",
        "// WGSL is what a build without the compiler (`lang`) registers, and what",
        "// the twin is held to (`the_rhai_twins_match_their_wgsl`): an edit here",
        "// changes neither the app nor the twin.",
    ]
    return "".join(line + eol for line in lines)


OLD_TWIN_HEADER = [
    "// The Rhai twin of shaders/effects/{rel}",
    "// (docs/archive/projects/rhai-plugins.md, P2): the same parameters, in the same",
    "// order, so a config cannot tell which it got.",
]

NEW_TWIN_HEADER = [
    "// The built-in `{name}` effect (docs/main/EFFECTS.md, Rhai effects): the",
    "// app runs this file -- on desktop the copy beside the app, so an edit",
    "// shows on the next start. It replaced shaders/effects/{rel}, whose",
    "// pixels, parameters and order it keeps, so a config cannot tell which",
    "// it got.",
]


def rewrite_twin(rel: str, name: str) -> None:
    path = ROOT / "assets" / "effects" / f"{name}.rhai"
    text = path.read_bytes().decode("utf-8")
    eol = "\r\n" if "\r\n" in text else "\n"
    old = eol.join(line.format(rel=rel, name=name) for line in OLD_TWIN_HEADER)
    if old not in text:
        print(f"twin header already rewritten: {name}.rhai")
        return
    new = eol.join(line.format(rel=rel, name=name) for line in NEW_TWIN_HEADER)
    path.write_bytes(text.replace(old, new, 1).encode("utf-8"))
    print(f"twin header rewritten: {name}.rhai")


def main() -> None:
    for rel, name in TWINS.items():
        rewrite_twin(rel, name)
        path = ROOT / "shaders" / "effects" / rel
        text = path.read_bytes().decode("utf-8")
        if MARK in text:
            print(f"already marked: {rel}")
            continue
        eol = "\r\n" if "\r\n" in text else "\n"
        first, rest = text.split(eol, 1)
        path.write_bytes((first + eol + note(name, eol) + rest).encode("utf-8"))
        print(f"marked: {rel}")


if __name__ == "__main__":
    main()
