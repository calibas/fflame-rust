"""Apply doc-comments + per-param descriptions to misc_extras4.rs.

One-off script for the variations-bulk-metadata project. Idempotent.

whorl already had a # Authors block and was edited manually to
prepend its description; this script handles the other 7 + all
per-param descriptions."""
import re
import textwrap

DOC = {
    "ANAMORPHCYL": (
        "Anamorphic cylinder warp — wraps the input around a cylinder. Outputs `(f·cos(k·x), f·sin(k·x))` where `f = a·(y + b)`. The X coordinate drives the angular position (× frequency `k`), and Y drives the radial position (offset by `b`, scaled by `a`).",
        ["Sosa"],
    ),
    "SVF": (
        "3D single-value function — combines trigonometric terms in a fixed pattern: `cos(y) · cos(n·y) · (cos(x), sin(x), sin(y))`. The `n` frequency parameter controls how many oscillations appear along the Y axis.",
        ["gossamer light"],
    ),
    "SHREDLIN": (
        "Linear shred grid — splits each axis into tiles of size `distance`, then within each tile compresses the position by `width` and shifts by `(0.5 − sign_offset) · (1 − width)`. Produces a discontinuous shred-like grid pattern.",
        ["Zy0rg"],
    ),
    "SHREDRAD": (
        "Radial shred — splits the angular space into `n` wedges of width `2π/n`, then within each wedge compresses the angular position by `width`. The radius passes through unchanged. Radial analogue of `shredlin`.",
        ["Zy0rg"],
    ),
    "XHEART": (
        "Heart-shaped Möbius warp — applies a Möbius-like inversion `(bx·x, by·y)` where `bx = 4/(r²+4)` and `by = (6+2·ratio)/(r²+4)`, then rotates the result by `π/4 + π/8·angle`. The Y axis is flipped where the rotated X is negative, producing a heart-shaped silhouette.",
        ["Xyrus02"],
    ),
    "STWIN": (
        "Sin-weighted twin warp — adds a sin-modulated correction `(x²−y²)·sin(2π·distort·(x+y+offset_xy·0.1)) / (x²+y²)` to both the X and Y outputs (the same correction term is applied identically to each axis).",
        ["Apophysis Plugin Pack"],
    ),
    "DEVIL_WARP": (
        "Power-warp with rmin/rmax clamp — computes a complex radial term `r = (x² + r²·b·y²)^warp − (y² + r²·a·x²)^warp` (with `r² = 1/(x²+y²)`), clamps it to `[rmin, rmax]`, scales it by `effect`, and emits `(x·(1+r), y·(1+r))`. The clamp prevents the power expression from blowing up at large or singular inputs.",
        ["DarkBeam"],
    ),
}

PARAM_DOC = {
    ("ANAMORPHCYL", "a"): "Radial scale (multiplies the entire output magnitude).",
    ("ANAMORPHCYL", "b"): "Y offset added before the radial multiplication.",
    ("ANAMORPHCYL", "k"): "Angular frequency of the X coordinate.",

    ("SVF", "n"): "Frequency multiplier on Y in the inner `cos(n·y)` term.",

    ("SHREDLIN", "xdistance"): "X-axis tile size.",
    ("SHREDLIN", "xwidth"): "X-axis intra-tile compression. 1 = no shred; 0 = collapse to tile center.",
    ("SHREDLIN", "ydistance"): "Y-axis tile size.",
    ("SHREDLIN", "ywidth"): "Y-axis intra-tile compression.",

    ("SHREDRAD", "n"): "Number of angular wedges.",
    ("SHREDRAD", "width"): "Intra-wedge compression. 1 = no shred; 0 = collapse to wedge boundary.",

    ("XHEART", "angle"): "Rotation angle of the heart shape (scaled by π/8 internally and offset from π/4).",
    ("XHEART", "ratio"): "Y-axis stretch factor (added to a base of 6 to control heart roundness).",

    ("STWIN", "distort"): "Frequency multiplier on the sin term (× 2π internally).",
    ("STWIN", "offset_xy"): "Phase offset added to the sin argument (× 0.1 internally).",
    ("STWIN", "offset_x2"): "Additive offset on x² (× 0.0001 internally). Prevents division by zero at the origin.",
    ("STWIN", "offset_y2"): "Additive offset on y² (× 0.0001 internally).",

    ("WHORL", "inside"): "Angular-shift coefficient applied where `r < w`.",
    ("WHORL", "outside"): "Angular-shift coefficient applied where `r ≥ w`.",

    ("DEVIL_WARP", "a"): "x² weight in the second power term.",
    ("DEVIL_WARP", "b"): "y² weight in the first power term.",
    ("DEVIL_WARP", "effect"): "Scale on the final radial-warp magnitude.",
    ("DEVIL_WARP", "warp"): "Power exponent for both radial terms.",
    ("DEVIL_WARP", "rmin"): "Lower clamp on the radial-warp magnitude.",
    ("DEVIL_WARP", "rmax"): "Upper clamp on the radial-warp magnitude.",
}

PATH = "src/variations/defs/misc_extras4.rs"
with open(PATH, "rb") as f:
    src = f.read().decode("utf-8")

inserted = 0
already = 0
for name, (body, authors) in DOC.items():
    target = f"\npub static {name}: VariationDef = VariationDef {{"
    if target not in src:
        print(f"  WARN: no match for {name}")
        continue
    idx = src.find(target)
    prefix = src[:idx]
    last_nl = prefix.rfind("\n")
    if last_nl != -1:
        prev_line = src[last_nl + 1:idx + 1].strip()
        if prev_line.startswith("///"):
            already += 1
            continue
    lines = []
    for paragraph in body.split("\n"):
        wrapped = textwrap.fill(paragraph, width=72) if paragraph.strip() else ""
        for line in wrapped.split("\n"):
            lines.append(f"/// {line}".rstrip())
    if authors:
        lines.append("///")
        lines.append("/// # Authors")
        for author in authors:
            lines.append(f"/// - {author}")
    doc = "\n".join(lines)
    src = src.replace(target, f"\n{doc}\npub static {name}: VariationDef = VariationDef {{", 1)
    inserted += 1
print(f"  pass1: inserted {inserted}, already {already}")


def find_macro_close(text: str, open_paren_idx: int) -> int:
    assert text[open_paren_idx] == "("
    depth = 1
    i = open_paren_idx + 1
    n = len(text)
    while i < n:
        c = text[i]
        if c == '"':
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                    continue
                if text[i] == '"':
                    i += 1
                    break
                i += 1
            continue
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


pinserted = 0
palready = 0
for (static_name, param_name), desc in PARAM_DOC.items():
    start_pattern = f"pub static {static_name}: VariationDef"
    start_idx = src.find(start_pattern)
    if start_idx == -1:
        continue
    next_static = src.find("\npub static ", start_idx + 1)
    end_idx = next_static if next_static != -1 else len(src)
    block = src[start_idx:end_idx]

    head = f'param!("{param_name}"'
    head_idx = block.find(head)
    if head_idx == -1:
        print(f"  WARN: no param {static_name}.{param_name}")
        continue
    open_idx = block.find("(", head_idx)
    close_idx = find_macro_close(block, open_idx)
    if close_idx == -1:
        print(f"  WARN: unbalanced param {static_name}.{param_name}")
        continue
    inner = block[open_idx + 1:close_idx]
    depth = 0
    in_str = False
    commas = 0
    j = 0
    while j < len(inner):
        ch = inner[j]
        if in_str:
            if ch == "\\":
                j += 2
                continue
            if ch == '"':
                in_str = False
        else:
            if ch == '"':
                in_str = True
            elif ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
            elif ch == "," and depth == 0:
                commas += 1
        j += 1
    if commas >= 6:
        palready += 1
        continue

    new_block = (
        block[:close_idx]
        + f', "{desc}"'
        + block[close_idx:]
    )
    src = src[:start_idx] + new_block + src[end_idx:]
    pinserted += 1
print(f"  pass2: injected {pinserted}, already {palready}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
