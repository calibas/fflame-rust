"""Apply doc-comments + per-param descriptions to apo_misc9.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "EJULIA": (
        "Elliptic-coordinate Julia — converts the input to elliptic coordinates `(μ, ν)`, divides both by `power`, adds a random angular branch `2π·floor(rand·|power|)/power` to ν, then emits `(cosh(μ)·cos(ν), sinh(μ)·sin(ν))`. With negative `power`, the input is first inverted through the unit circle. Produces N-fold rotationally symmetric Julia-style elliptic patterns.",
        ["Michael Faber"],
    ),
    "EMOTION": (
        "Elliptic-coordinate motion — converts the input to elliptic coordinates `(μ, ν)`, adds `move` to μ (with sign determined by ν), folds μ negative back onto positive (reflecting ν), then adds `rotate` to ν.",
        ["Michael Faber"],
    ),
    "FLOWER_DB": (
        "Flower with stem — emits a flower-shaped XY pattern (radius modulated by `|spread + sin(petals·t)| · cos(petal_split·petals·t)`) plus a stem Z that grows downward with thickness controlled by `stem_thickness`. Optionally folds the petal Z upward outside a radius, and caps the stem at a maximum length.",
        ["CozyG", "DarkBeam"],
    ),
    "JULIAN2": (
        "JuliaN with affine pre-transform — applies a 2D affine `(X, Y) = (a·x + b·y + e, c·x + d·y + f)` first, then runs standard JuliaN: picks a random angular branch from `|power|`, computes the new angle `(atan2(Y, X) + 2π·k)/power`, and emits at radius `(X²+Y²)^(dist/(2·power))`.",
        ["Xyrus02"],
    ),
}

PARAM_DOC = {
    ("EJULIA", "power"): "Number of angular branches. Negative values invert the input through the unit circle first.",

    ("EMOTION", "move"): "Additive offset on μ (sign chosen by the sign of ν).",
    ("EMOTION", "rotate"): "Additive offset on ν.",

    ("FLOWER_DB", "petals"): "Number of petal lobes (angular frequency of the sin modulator).",
    ("FLOWER_DB", "petal_split"): "Frequency multiplier on the inner cosine that creates per-petal sub-divisions.",
    ("FLOWER_DB", "petal_spread"): "Constant offset added to the sin modulator. Controls how filled the petals appear.",
    ("FLOWER_DB", "stem_thickness"): "Magnitude of the Z output. Negative values point the stem downward.",
    ("FLOWER_DB", "stem_length"): "Maximum stem length. 0 disables the cap (stem extends without bound).",
    ("FLOWER_DB", "petal_fold_strength"): "Z-axis fold strength applied to petals outside `petal_fold_radius`.",
    ("FLOWER_DB", "petal_fold_radius"): "Radial threshold beyond which the petal fold kicks in.",

    ("JULIAN2", "power"): "Number of angular branches. 0 produces zero output.",
    ("JULIAN2", "dist"): "Radial-power exponent — output radius is `(X²+Y²)^(dist/(2·power))`.",
    ("JULIAN2", "a"): "Pre-affine xx coefficient.",
    ("JULIAN2", "b"): "Pre-affine xy coefficient.",
    ("JULIAN2", "c"): "Pre-affine yx coefficient.",
    ("JULIAN2", "d"): "Pre-affine yy coefficient.",
    ("JULIAN2", "e"): "Pre-affine x offset.",
    ("JULIAN2", "f"): "Pre-affine y offset.",
}

PATH = "src/variations/defs/apo_misc9.rs"
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
