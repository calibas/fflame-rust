"""Apply doc-comments + per-param descriptions to stub_recoveries2.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "DISC3": (
        "Disc-style warp with 8 tunable knobs — computes a per-axis-weighted radial argument `rPI = π · sqrt(x²·d·e + y²·f·g)`, then emits `(sin(rPI)·a, cos(rPI)·b)` scaled by `atan2(y,x)/π · c · h`. The 8 parameters give independent control over the X/Y radial weights, the sin/cos amplitudes, and the overall output scale.",
        ["Brad Stefanov"],
    ),
    "PROJECTIVE": (
        "Linear-fractional projective transform — applies a 2D homography `out = ((A1·x + B1·y + C1) / U, (A2·x + B2·y + C2) / U)` where `U = A·x + B·y + C`. With identity coefficients reduces to the input; arbitrary coefficients implement any 2D projective warp.",
        ["eralex61"],
    ),
    "TQMIRROR": (
        "Quadrant-based fold-mirror — dispatches to one of four behaviors based on the input's position relative to per-axis thresholds: swap-or-pass on the outer boundary (controlled by `type`), additive shift in the third quadrant, scaled diagonal-mirror in a central region, or identity elsewhere. The variation weight is used both as a comparison threshold and an additive offset, so output can change shape qualitatively as weight changes sign.",
        ["Anderson", "Brad Stefanov"],
    ),
    "INTERSECTION": (
        "Random row-or-column tile blur — flips a coin to decide whether to tile along rows or columns. On a row pass, X is shifted by `xtilesize · round(±xwidth · log(rand))` to a random nearby tile, while Y gets piecewise fmod-folded around `xmod1`. Columns work analogously. Produces grid-like tile-shifted blur patterns.",
        ["Brad Stefanov"],
    ),
}

PARAM_DOC = {
    ("DISC3", "a"): "Sin amplitude scale on the X output.",
    ("DISC3", "b"): "Cos amplitude scale on the Y output.",
    ("DISC3", "c"): "Angular-term scale (multiplier on `atan2(y,x)/π`).",
    ("DISC3", "d"): "X-axis radial-weight first factor (multiplied with `e` for the x² term).",
    ("DISC3", "e"): "X-axis radial-weight second factor.",
    ("DISC3", "f"): "Y-axis radial-weight first factor (multiplied with `g` for the y² term).",
    ("DISC3", "g"): "Y-axis radial-weight second factor.",
    ("DISC3", "h"): "Overall output scale (multiplied into both X and Y).",

    ("PROJECTIVE", "a"): "Denominator X coefficient.",
    ("PROJECTIVE", "b"): "Denominator Y coefficient.",
    ("PROJECTIVE", "c"): "Denominator constant term.",
    ("PROJECTIVE", "a1"): "Numerator X coefficient for X output.",
    ("PROJECTIVE", "b1"): "Numerator Y coefficient for X output.",
    ("PROJECTIVE", "c1"): "Numerator constant term for X output.",
    ("PROJECTIVE", "a2"): "Numerator X coefficient for Y output.",
    ("PROJECTIVE", "b2"): "Numerator Y coefficient for Y output.",
    ("PROJECTIVE", "c2"): "Numerator constant term for Y output.",

    ("TQMIRROR", "a"): "Y threshold for the central-region diagonal-mirror branch.",
    ("TQMIRROR", "b"): "X offset added to the central-region's right-side gate.",
    ("TQMIRROR", "c"): "Y offset added to the central-region's top-side gate.",
    ("TQMIRROR", "d"): "X offset added to the outer-boundary swap gate.",
    ("TQMIRROR", "e"): "Y offset added to the outer-boundary swap gate.",
    ("TQMIRROR", "f"): "Additive X shift in the third-quadrant branch (scaled by weight).",
    ("TQMIRROR", "g"): "Additive Y shift in the third-quadrant branch.",
    ("TQMIRROR", "h"): "Scale factor on the central-region diagonal-mirror output.",
    ("TQMIRROR", "type"): "Outer-boundary branch selector: 0 = swap (x↔y), 1 = pass through.",

    ("INTERSECTION", "xwidth"): "Row-branch X-shift magnitude (multiplied by `log(rand)`).",
    ("INTERSECTION", "xtilesize"): "Row-branch X tile size — final X is `xtilesize · (x + shift)`.",
    ("INTERSECTION", "xmod1"): "Row-branch Y fold threshold.",
    ("INTERSECTION", "xmod2"): "Row-branch Y fold period (multiplied with `xmod1` to form the modulus).",
    ("INTERSECTION", "xheight"): "Row-branch Y output scale.",
    ("INTERSECTION", "yheight"): "Column-branch Y-shift magnitude.",
    ("INTERSECTION", "ytilesize"): "Column-branch Y tile size.",
    ("INTERSECTION", "ymod1"): "Column-branch X fold threshold.",
    ("INTERSECTION", "ymod2"): "Column-branch X fold period.",
    ("INTERSECTION", "ywidth"): "Column-branch X output scale.",
}

PATH = "src/variations/defs/stub_recoveries2.rs"
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
