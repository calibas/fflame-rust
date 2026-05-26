"""Apply doc-comments + per-param descriptions to misc_2d.rs.

One-off script for the variations-bulk-metadata project. Idempotent.

split and stripes already had # Authors blocks and were edited manually
to prepend descriptions; this script handles the other 6 + all
per-param descriptions (split & stripes included)."""
import re
import textwrap

DOC = {
    "SQUIRREL": (
        "Cos/tan composite warp — outputs `(cos(s)·tan(x), sin(s)·tan(y))` where `s = sqrt(a·x² + b·y²)`. The tangent terms produce vertical and horizontal stripes near `x = π/2 + kπ` etc.",
        ["Raykoid666"],
    ),
    "SHIFT": (
        "Rotated additive shift — adds a 2D shift vector to the input, where the shift direction is rotated by `angle` degrees. Equivalent to translating in a rotated frame.",
        ["Tatyana Zabanova", "Brad Stefanov"],
    ),
    "PRESSURE_WAVE": (
        "Sin-wave additive distortion — adds `sin(2π·freq·coord) / (2π·freq)` to each axis. Amplitude is inversely proportional to frequency, so higher-frequency ripples are smaller. With `freq = 0`, the term degenerates to `sin(coord)`.",
        ["timothy-vincent", "DarkBeam"],
    ),
    "SPHERICALN": (
        "N-power spherical with random branch — combines a spherical inversion `1/r^dist` with a random angular shift `n · 2π / ⌊|power|⌋` where `n = ⌊power · rand⌋`. Equivalent to spherical-inverting the input and then rotating by one of `⌊|power|⌋` random angles.",
        None,
    ),
    "SPLIGON": (
        "Polygonal spike-tiler — snaps the input angle to the nearest of `sides` polygon spokes (with a per-spoke offset from `i`), then adds a unit-length spike at that angle to the input. Creates a tiling of spikes radiating at regular angles.",
        None,
    ),
    "TILE_HLP": (
        "Tile-stripe blur — divides X into stripes of given `width`, and per iteration either shifts X by ±width to a neighboring stripe (with the probability driven by stripe position and a uniform random) or leaves it alone. Y passes through. Produces blurred banding along X.",
        ["Zy0rg", "Tatyana Zabanova", "Brad Stefanov"],
    ),
}

PARAM_DOC = {
    ("SPLIT", "xsize"): "X-axis frequency of the mirror-flip cosine.",
    ("SPLIT", "ysize"): "Y-axis frequency of the mirror-flip cosine.",

    ("SQUIRREL", "a"): "X² weight in the radius term.",
    ("SQUIRREL", "b"): "Y² weight in the radius term.",

    ("STRIPES", "space"): "Compression factor for X within each stripe — 1 collapses to integer values, 0 disables compression.",
    ("STRIPES", "warp"): "Parabolic Y-bend amplitude as a function of local stripe offset.",

    ("SHIFT", "shift_x"): "X-axis shift amount (in the rotated frame).",
    ("SHIFT", "shift_y"): "Y-axis shift amount (in the rotated frame).",
    ("SHIFT", "angle"): "Rotation angle of the shift vector, in degrees.",

    ("PRESSURE_WAVE", "x_freq"): "X-axis sine frequency. Amplitude scales as `1 / (2π·freq)`; 0 degenerates to `sin(x)`.",
    ("PRESSURE_WAVE", "y_freq"): "Y-axis sine frequency. Same scaling rule as x_freq.",

    ("SPHERICALN", "power"): "Number of angular branches — `⌊|power|⌋` determines the spoke count, and each iteration picks one at random.",
    ("SPHERICALN", "dist"): "Radial-power exponent — output radius is `1 / r^dist`. `dist = 1` reduces to a standard spherical inversion.",

    ("SPLIGON", "sides"): "Number of polygon spokes.",
    ("SPLIGON", "i"): "Spike angular offset — rotates the spike direction within its spoke sector.",

    ("TILE_HLP", "width"): "Stripe width along X. Smaller width = more, narrower stripes.",
}

PATH = "src/variations/defs/misc_2d.rs"
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
