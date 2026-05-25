"""Apply doc-comments + per-param descriptions to classic_blades_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "ARCH": (
        "RNG-driven angle bend — picks a random angle `ang = rand · w · π`, then emits `(w · sin(ang), w · sin²(ang) / cos(ang))`. The `sin²/cos = sin · tan` term creates a smooth arch shape with a vertical asymptote.",
        ["Scott Draves"],
    ),
    "BI_LINEAR": (
        "Coordinate swap — outputs `(y, x)`. The simplest possible 2D coordinate operation; useful as a building block in combination with other variations.",
        None,
    ),
    "BLADE": (
        "RNG-driven blade fan — picks a random radius `r = rand · w · sqrt(x² + y²)`, then emits `(w · x · (cos r + sin r), w · x · (cos r − sin r))`. Both output axes are driven by the input X, so the result spreads along the X axis like blades of a fan.",
        ["Z+"],
    ),
    "BLADE3D": (
        "3D extension of `blade` — same X/Y outputs as `blade`, plus a Z output `w · y · (sin r − cos r)` driven by input Y. Completes the 3D blade structure.",
        ["Z+"],
    ),
    "SQUARIZE": (
        "Angle-pack square map — converts polar coordinates `(s, a)` to a position on a square of side `s` by treating `q = 4·s·a/π` as a perimeter parameter and dispatching to one of 5 edge-segment branches. Effectively wraps the unit circle around a unit square.",
        ["Michael Faber"],
    ),
    "SQUISH": (
        "Square map with cell mod power — extends `squarize` with a random cell selection: adds an `8·s·floor(power · rand)` offset to the perimeter parameter before dispatch, then divides by `power`. Produces `power` discrete tiles of the squarize pattern.",
        ["Michael Faber"],
    ),
    "TWOFACE": (
        "Half-spherical / half-pass — points with `x ≤ 0` get scaled output `w · (x, y)`; points with `x > 0` get spherical-inverted (`w/(x²+y²) · (x, y)`). Combines a linear left side with a spherical right side — hence the name.",
        None,
    ),
    "TWINTRIAN": (
        "RNG twin trigonal — picks a random `r = rand · w · sqrt(x² + y²)`, computes `diff = log₁₀(sin² r) + cos r` (forced to −30 when degenerate), then emits `(w · x · diff, w · x · (diff − sin r · π))`. The log term creates twin trigonometric interference patterns.",
        ["Z+"],
    ),
    "UNPOLAR": (
        "Exp/sin polar inversion — outputs `(w/(2π) · exp(y) · sin x, w/(2π) · exp(y) · cos x)`. The inverse of `polar` — converts log-polar coordinates back to cartesian.",
        None,
    ),
}

PARAM_DOC = {
    ("SQUISH", "power"): "Number of discrete tiles the squarize pattern is divided into (≥ 2).",
}

PATH = "src/variations/defs/classic_blades_misc.rs"
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
