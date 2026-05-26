"""Apply doc-comments + per-param descriptions to erf_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent.

edisc and curve already had # Authors blocks and were edited manually
to prepend descriptions; this script handles the other 6 + all
per-param descriptions."""
import re
import textwrap

DOC = {
    "ERF": (
        "2D component-wise erf — applies the error function `erf(coord)` to each axis independently. Saturates the input smoothly toward ±1, like a soft clip.",
        ["zephyrtronium", "DarkBeam"],
    ),
    "ERF3D": (
        "3D component-wise erf — same as `erf` but with `erf(z)` applied to the Z axis too.",
        ["zephyrtronium", "DarkBeam"],
    ),
    "D_SPHERICAL": (
        "Random-blend spherical/linear — each iteration flips a weighted coin: with probability `d_spher_weight` applies a spherical inversion `(x, y) / (x²+y²)`, otherwise passes through unchanged. Smoothly blends the two effects across many samples.",
        ["Tatyana Zabanova"],
    ),
    "DUSTPOINT": (
        "3-point pivot/overlap IFS triangle — picks one of three sub-transformations uniformly: a Y-sign-flipped polar pivot, a scale-to-origin by 1/3, or a scale-by-1/3 with X offset of 2/3. Together these implement an IFS that draws a Sierpinski-like dust pattern with a circular pivot.",
        ["Jesus Sosa"],
    ),
    "DELTAA": (
        "Radial ratio + half angle difference — emits `r · (cos a, sin a)` where `r = sqrt(y² + (x+1)²) / sqrt(y² + (x−1)²)` (ratio of distances to two foci at `(∓1, 0)`) and `a` is half the angular difference between the focus-angles. Produces conformal-mapping-style patterns.",
        ["Michael Faber"],
    ),
    "ELLIPTIC2": (
        "Elliptic warp with 11 knobs — heavily parameterized elliptic variation. Computes `xmax = c·(sqrt(x²+y²+a1+b1·x) + sqrt(x²+y²+a1−b1·x))`, then emits `(v·atan2(a, b) + ps, ±v·log(xmax + sqrt(xmax − f or g)))` with the Y branch chosen by random vs `e`. The 11 parameters give fine-grained control over the elliptic shape.",
        ["Brad Stefanov"],
    ),
}

PARAM_DOC = {
    ("D_SPHERICAL", "d_spher_weight"): "Probability of applying the spherical inversion (vs linear pass-through).",

    ("CURVE", "xamp"): "X-axis gaussian bump amplitude.",
    ("CURVE", "yamp"): "Y-axis gaussian bump amplitude.",
    ("CURVE", "xlength"): "X-axis gaussian width — the Y-direction decay scale of the X bump.",
    ("CURVE", "ylength"): "Y-axis gaussian width — the X-direction decay scale of the Y bump.",

    ("ELLIPTIC2", "a1"): "Constant offset added to `x² + y²` in the elliptic-radius formula.",
    ("ELLIPTIC2", "a2"): "Scale on the `atan2` argument `a = (x/xmax) · a2`.",
    ("ELLIPTIC2", "a3"): "Phase shift on the X output (scaled by −π/2).",
    ("ELLIPTIC2", "b1"): "Multiplier on x in the xmax sqrt-difference.",
    ("ELLIPTIC2", "b2"): "Scale on `sqrt(d − a²)` in the b term.",
    ("ELLIPTIC2", "c"): "Overall scale on xmax.",
    ("ELLIPTIC2", "d"): "Constant in the `sqrt(d − a²)` term.",
    ("ELLIPTIC2", "e"): "Probability threshold for the Y output sign.",
    ("ELLIPTIC2", "f"): "Sqrt offset for the positive-Y branch (`sqrt(xmax − f)`).",
    ("ELLIPTIC2", "g"): "Sqrt offset for the negative-Y branch (`sqrt(xmax − g)`).",
    ("ELLIPTIC2", "h"): "V-multiplier — output is scaled by `v = w · h / π`.",
}

PATH = "src/variations/defs/erf_misc.rs"
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
