"""Apply doc-comments + per-param descriptions to simple_classics.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "EXP2": (
        "Exponential polar warp — emits `exp(x·π) · (cos(y·π), sin(y·π))`. The X coordinate drives an exponential radial scaling, and Y drives the angular position.",
        None,
    ),
    "EXPONENTIAL": (
        "Classic exponential variation — emits `exp(x − 1) · (cos(π·y), sin(π·y))`. Same shape as `exp2` but with a slightly different exponential argument (subtracts 1 so the unit radius lies at x = 1 instead of x = 0).",
        None,
    ),
    "FLIPY": (
        "Y-flip when x > 0 — leaves the input alone when `x ≤ 0`; flips the sign of Y when `x > 0`. Useful as a building block for left/right asymmetric flames.",
        ["Michael Faber"],
    ),
    "FUNNEL": (
        "Tangent · secant + offset funnel — per axis, emits `tanh(coord) · (sec(coord) + effect · π)`. The hyperbolic tangent saturates the input toward ±1, and the secant term creates poles where the input's cosine hits zero.",
        ["Raykoid666"],
    ),
    "INVPOLAR": (
        "Inverse polar — emits `(1 + y) · (sin(π·x), cos(π·x))`. Treats X as the angle (in units of π) and `1 + y` as the radius — the inverse of the `polar` variation.",
        None,
    ),
    "PERSPECTIVE": (
        "2D perspective projection — applies a perspective foreshortening with `t = 1 / (dist − y · sin(angle·π/2))`, then emits `(dist · x · t, dist · cos(angle·π/2) · y · t)`. The `angle` parameter tilts the projection plane; `dist` sets the viewing distance.",
        None,
    ),
    "LINE": (
        "Project to a random spot on a 3D line — picks a random distance `r = rand · w` and emits along a unit vector `(cos(δπ)·cos(φπ), sin(δπ)·cos(φπ), sin(φπ))`. The 2D output drops the Z component.",
        ["ChronologicalDot"],
    ),
    "HOLESQ": (
        "Square hole — outside the square `|x| + |y| > 1` the input passes through; inside, the dominant-axis coordinate gets folded toward `±0.5` based on its sign and the magnitude of the perpendicular coordinate. Produces a square-hole-shaped scatter pattern.",
        ["DarkBeam"],
    ),
}

PARAM_DOC = {
    ("FUNNEL", "effect"): "Additive angular offset on each axis (scaled by π). Higher = more horizontal/vertical bias.",

    ("PERSPECTIVE", "angle"): "Tilt of the projection plane (in units of π/2). 0 = parallel (no perspective); 1 = perpendicular.",
    ("PERSPECTIVE", "dist"): "Viewing distance from the projection plane. Larger = milder perspective effect.",

    ("LINE", "delta"): "Azimuthal angle of the line direction (in units of π).",
    ("LINE", "phi"): "Polar angle of the line direction (in units of π). When `δ = φ = 0`, the line projects onto the X axis.",
}

PATH = "src/variations/defs/simple_classics.rs"
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
