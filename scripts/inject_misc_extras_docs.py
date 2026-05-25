"""Apply doc-comments + per-param descriptions to misc_extras.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "HO": (
        "3D hyperbolic-octahedron mapping — outputs `(cos(x)·cos(y))^xpow + xpow·cos(x)·cos(y) + 0.25·atan2(y², z²)` on X, with analogous formulas on Y and Z. The power-plus-linear-plus-arctangent combination traces an octahedral hyperbolic shape in 3D space.",
        ["Larry Berlin"],
    ),
    "CHUNK": (
        "Quadratic-conic mask — evaluates `r = a·x² + b·xy + c·y² + d·x + e·y + f` (a general 2D quadratic) and either keeps or discards the input based on whether `r` is positive or negative (controlled by `mode`). The boundary `r = 0` is a conic section — an ellipse, parabola, or hyperbola, depending on coefficients.",
        ["zephyrtronium"],
    ),
    "PTRANSFORM": (
        "Log-polar transform — maps the input to `(ρ·cos(θ), ρ·sin(θ))` where `ρ` is the input radius (optionally log-transformed) divided by `power` and then optionally exponentiated. `θ` is the input angle plus `rotate`. `split` adds an asymmetric ρ offset based on the sign of x. With `use_log` enabled this becomes a true log-polar transform; disabled, it's a simple radius rescaling.",
        None,
    ),
    "RATIONAL3": (
        "Degree-3 complex-rational warp — evaluates a complex-valued rational function of `x + i·y`, with degree-3 polynomial numerator (coefficients a, b, c, d) and denominator (e, f, g, h). The output is the complex division `numerator / denominator`.",
        ["Xyrus", "CozyG"],
    ),
    "TILE_REVERSE": (
        "Random-mirror tile blur — shifts the input along one axis by ±space (direction chosen by coin flip). With `reversal = 1` the same axis is mirrored as well. `vertical` selects which axis gets the tiling treatment.",
        ["Whittaker Courtney"],
    ),
    "ORTHO": (
        "Orthogonal Möbius warp — maps points inside the unit disc through an orthogonal-circle Möbius transformation (branch chosen by the sign of x). Outside the disc, the input is first inverted through the unit circle, the same transform is applied, then re-inverted. Produces hyperbolic-tiling-like patterns.",
        ["Michael Faber"],
    ),
}

PARAM_DOC = {
    ("HO", "xpow"): "Exponent applied to `cos(x)·cos(y)` in the X output.",
    ("HO", "ypow"): "Exponent applied to `sin(x)·cos(y)` in the Y output.",
    ("HO", "zpow"): "Exponent applied to `sin(y)` in the Z output.",

    ("CHUNK", "a"): "x² coefficient.",
    ("CHUNK", "b"): "x·y coefficient.",
    ("CHUNK", "c"): "y² coefficient.",
    ("CHUNK", "d"): "x linear coefficient.",
    ("CHUNK", "e"): "y linear coefficient.",
    ("CHUNK", "f"): "Constant offset.",
    ("CHUNK", "mode"): "Which side of `r = 0` to keep: 0 = keep where r ≤ 0, 1 = keep where r > 0.",

    ("PTRANSFORM", "rotate"): "Angular rotation added to θ, in radians.",
    ("PTRANSFORM", "power"): "Radial divisor. When `use_log` is off, this is the reciprocal scaling factor on the input radius.",
    ("PTRANSFORM", "move"): "Additive offset on ρ.",
    ("PTRANSFORM", "split"): "Asymmetric ρ offset, added when x ≥ 0 and subtracted when x < 0.",
    ("PTRANSFORM", "use_log"): "Whether to apply log/exp around ρ: 0 = linear radius, 1 = true log-polar.",

    ("RATIONAL3", "a"): "Numerator coefficient on the `x³ − 3xy²` term.",
    ("RATIONAL3", "b"): "Numerator coefficient on the `x² − y²` term.",
    ("RATIONAL3", "c"): "Numerator linear coefficient on x.",
    ("RATIONAL3", "d"): "Numerator constant term.",
    ("RATIONAL3", "e"): "Denominator coefficient on the `x³ − 3xy²` term.",
    ("RATIONAL3", "f"): "Denominator coefficient on the `x² − y²` term.",
    ("RATIONAL3", "g"): "Denominator linear coefficient on x.",
    ("RATIONAL3", "h"): "Denominator constant term.",

    ("TILE_REVERSE", "space"): "Tile offset along the active axis.",
    ("TILE_REVERSE", "reversal"): "When equal to 1.0, the active axis is also mirrored. Any other value passes through unchanged.",
    ("TILE_REVERSE", "vertical"): "Tiling axis selector: 0 = horizontal, 1 = vertical.",

    ("ORTHO", "in_p"): "Branch-selector multiplier for the inside-disc Möbius transform.",
    ("ORTHO", "out_p"): "Branch-selector multiplier for the outside-disc Möbius transform.",
}

PATH = "src/variations/defs/misc_extras.rs"
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
    """Find the matching ) for the ( at open_paren_idx, respecting string literals."""
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
