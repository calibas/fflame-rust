"""Apply doc-comments + per-param descriptions to apo_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "XERF": (
        "3D piecewise erf / 1/r² — per-axis, if `|coord| ≥ 2` the output is `coord / r²` (spherical inversion); otherwise it's `erf(coord)`. Combines sigmoid saturation near the origin with spherical inversion far away.",
        ["zephyrtronium", "DarkBeam"],
    ),
    "INVERTED_JULIA": (
        "Inverted Julia warp — 9-parameter Julia variant with adjustable inward center. Computes `z = (x² + y²·y2_mult)^power + x2y2_add`, picks a random hemisphere via `q = atan2(...)/2 + π·floor(2·rand)`, then emits `cos(z·cos_mult) · (sin q, cos q · y_mult) / z / center`.",
        ["Whittaker Courtney"],
    ),
    "IDISC": (
        "Angle-radius disc inversion — emits `(r·cos a, r·sin a)` where `a = π / (sqrt(x²+y²) + 1)` and `r = atan2(y, x) · w/π`. Swaps the roles of radius and angle in the output relative to a standard polar-to-cartesian mapping.",
        ["Michael Faber"],
    ),
    "CONIC": (
        "Conic-section sampler — emits `r · (x, y)` where `r = w · (rand − holes) · eccentricity / (1 + ecc · x/s) / s` and `s = sqrt(x²+y²)`. The `1/(1 + ecc·cos θ)` term is the standard polar equation of a conic section.",
        ["cyberxaos"],
    ),
    "POWER": (
        "Power warp — emits `r^(x/r) · (y/r, x/r)` where `r = sqrt(x²+y²)`. The exponent depends on the input's angle (`cos θ`), and the output coordinates are swapped relative to the input (a 90° rotation; cpp's xy-swap quirk, preserved).",
        None,
    ),
    "ROUNDSPHER": (
        "Rounded spherical inversion — softens the standard spherical inversion `(x, y)/r²` by adding `(2/π)²` to the reciprocal-of-radius term, yielding `(w·x, w·y) / (1 + (2/π)²·r²)`. Smooths out the singularity at the origin.",
        ["Raykoid666"],
    ),
    "CHECKS": (
        "Checkerboard cell-shift — divides space into a grid of cells of size `size`, classifies each cell as odd/even, and applies a different per-axis shift in each parity class. Optionally jitters one component of the shift by `rnd`.",
        ["Keeps", "Xyrus02"],
    ),
    "CONE": (
        "Julia + hemisphere mix forming a cone — combines a Julia-style angular pick `π·floor(weight · rand)·radius2 + atan2(y,x)·radius1` with a hemisphere-style radial term `r = size2 / sqrt(x²·warp + y² + size1)`, plus a configurable `height` Z output. The result traces a cone-shaped surface in 3D.",
        ["Brad Stefanov"],
    ),
}

PARAM_DOC = {
    ("INVERTED_JULIA", "power"): "Exponent on the squared-radius base term `(x² + y²·y2_mult)`.",
    ("INVERTED_JULIA", "y2_mult"): "Multiplier on y² in the base term.",
    ("INVERTED_JULIA", "a2x_mult"): "Multiplier on x in the angle term.",
    ("INVERTED_JULIA", "a2y_mult"): "Multiplier on y in the angle term.",
    ("INVERTED_JULIA", "a2y_add"): "Additive offset on y in the angle term.",
    ("INVERTED_JULIA", "cos_mult"): "Frequency multiplier on z in the cosine modulator.",
    ("INVERTED_JULIA", "y_mult"): "Y output scaling.",
    ("INVERTED_JULIA", "center"): "Divisor on the output radius. Higher = tighter pattern.",
    ("INVERTED_JULIA", "x2y2_add"): "Additive offset on the base term (added after the pow).",

    ("CONIC", "eccentricity"): "Conic eccentricity. 0 = circle, 1 = parabola, > 1 = hyperbola.",
    ("CONIC", "holes"): "Random shift offset subtracted from the per-iteration random. Larger = sparser pattern.",

    ("CHECKS", "x"): "X-cell-shift magnitude.",
    ("CHECKS", "y"): "Y-cell-shift magnitude.",
    ("CHECKS", "size"): "Cell grid size.",
    ("CHECKS", "rnd"): "Random jitter magnitude on the cell shift.",

    ("CONE", "radius1"): "Inner-radius multiplier on the input angle.",
    ("CONE", "radius2"): "Outer-radius multiplier on the random branch offset.",
    ("CONE", "size1"): "Squared-radius offset in the denominator.",
    ("CONE", "size2"): "Output radius scale (numerator of `r`).",
    ("CONE", "ywave"): "Y-axis frequency multiplier in `sin(xx·ywave)`.",
    ("CONE", "xwave"): "X-axis frequency multiplier in `cos(xx·xwave)`.",
    ("CONE", "height"): "Z output scale (3D only — controls cone height).",
    ("CONE", "warp"): "X² weight in the denominator. Controls the aspect ratio of the cone.",
    ("CONE", "weight"): "Number of random angular branches — `floor(weight · rand)` picks the branch.",
}

PATH = "src/variations/defs/apo_misc.rs"
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
