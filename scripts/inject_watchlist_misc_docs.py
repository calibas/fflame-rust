"""Apply doc-comments + per-param descriptions to watchlist_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "TRADE": (
        "Two-disc swap — defines two discs (one at `(r1+d1, 0)` with radius `r1`, one at `(-(r2+d2), 0)` with radius `r2`). Points inside the right disc get warped to the corresponding position in the left disc, and vice versa; points outside both pass through.",
        ["Michael Faber"],
    ),
    "VORON": (
        "Voronoi-cell snap with hash noise — scans the 3×3 grid of cells around the input, generates 1–`num` deterministic Voronoi site positions per cell via a bit-mixed integer hash, finds the nearest site, and lerps the input toward it by factor `k`. Produces classic Voronoi cellular patterns.",
        ["eralex61"],
    ),
    "SQUIRCULAR": (
        "Squircular Möbius warp — maps the input through a \"squircle\"-style transformation (intermediate between a circle and a square). The variation weight enters non-linearly in the body, so the output shape changes qualitatively with weight rather than just scaling.",
        None,
    ),
    "FLUX": (
        "VVAR-shift Möbius warp — computes `xpw = x + w` and `xmw = x − w` (where `w` is the variation weight), then emits a Möbius-style radial × angular combination: `r = w·(2 + spread)·sqrt(sqrt(y² + xpw²) / sqrt(y² + xmw²))` and `a = (atan2(y, xmw) − atan2(y, xpw)) / 2`. Produces flux-like field-line patterns between two virtual poles at `±w`.",
        ["meckie"],
    ),
    "RAYS": (
        "RNG-driven ray spread — picks a random angle `ang = w · rand · π`, then emits `(tan(ang) · r · cos(x), tan(ang) · r · sin(y))` with `r = w / (x² + y²)`. The tangent term creates spiky rays radiating in random angular directions.",
        ["Z+"],
    ),
    "RAYS1": (
        "Cotangent + (2/π)² ray spread — computes `u = cot(sqrt(x²+y²)) + w · (2/π)²`, then emits `(u·t/x, u·t/y)` where `t = x² + y²`. Produces concentric-ring ray patterns driven by the cotangent's pole structure.",
        ["Raykoid666"],
    ),
    "LOONIE2": (
        "N-sided loonie — generalizes the standard `loonie` warp to N-sided star/circle hybrids. Computes a maximum projection across `sides` rotations, then mixes with a circular term via `circle` and optionally folds with a star pattern via `star`. Inside the squared-weight threshold the input scales outward; outside, it passes through.",
        ["DarkBeam"],
    ),
    "FOURTH": (
        "4-quadrant compound — applies a different variation in each quadrant of the input: `(+,+)` uses spherical, `(+,−)` uses loonie (with squared-weight threshold), `(−,+)` uses lazysusan (shift + spin + twist), `(−,−)` is linear pass-through. Useful for combining four distinct behaviors in a single transform.",
        ["guagapunyaimel"],
    ),
}

PARAM_DOC = {
    ("TRADE", "r1"): "Right-disc radius.",
    ("TRADE", "d1"): "Right-disc center offset from origin — center sits at `(r1 + d1, 0)`.",
    ("TRADE", "r2"): "Left-disc radius.",
    ("TRADE", "d2"): "Left-disc center offset from origin — center sits at `(-(r2 + d2), 0)`.",

    ("VORON", "k"): "Lerp factor toward the nearest Voronoi site. 1 = snap fully to site; 0 = pass through unchanged.",
    ("VORON", "step"): "Voronoi cell size.",
    ("VORON", "num"): "Maximum sites per cell (1–5). Actual count per cell is hashed from the cell index.",
    ("VORON", "xseed"): "Hash seed for X-coordinate site generation.",
    ("VORON", "yseed"): "Hash seed for Y-coordinate site generation.",

    ("FLUX", "spread"): "Output magnitude scale, offset from a base value of 2.",

    ("LOONIE2", "sides"): "Polygon side count (≥ 1).",
    ("LOONIE2", "star"): "Star-fold rotation amount (scaled by −π/2 internally).",
    ("LOONIE2", "circle"): "Circularity mixing factor: 0 = pure star/polygon shape, 1 = pure circle.",

    ("FOURTH", "spin"): "Lazysusan-quadrant rotation amount, in radians.",
    ("FOURTH", "space"): "Lazysusan-quadrant radial nudge for the outside-threshold case.",
    ("FOURTH", "twist"): "Lazysusan-quadrant additional rotation, proportional to distance from the threshold edge.",
    ("FOURTH", "x"): "Lazysusan-quadrant X center offset.",
    ("FOURTH", "y"): "Lazysusan-quadrant Y center offset.",
}

PATH = "src/variations/defs/watchlist_misc.rs"
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
