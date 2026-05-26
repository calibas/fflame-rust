"""Apply doc-comments + per-param descriptions to apo_misc19.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "MOBIUS_STRIP": (
        "Parametric Möbius strip surface — maps the input `(x, y)` onto a Möbius strip with adjustable `radius`, `width`, and number of `twists`. `radial_mode` controls how x (the around-the-strip coordinate) handles out-of-range values; `width_mode` does the same for y (the across-the-strip coordinate). Optional rotation around X and Y axes via `rotate_x`/`rotate_y`.",
        ["slobo777", "chronologicaldot", "CozyG"],
    ),
    "CIRCLE_LINEAR": (
        "Grid of linear blobs — divides the input plane into a `2·Sc × 2·Sc` cell grid and per-cell uses deterministic hash noise to either pass through, scale by `K`, or remap to a noise-driven ring radius. `Dens1` and `Dens2` control the density and ratio of the two non-identity branches; `Reverse` swaps which sub-branch gets the scale and which gets the ring map.",
        None,
    ),
}

PARAM_DOC = {
    ("MOBIUS_STRIP", "radius"): "Strip's central-circle radius.",
    ("MOBIUS_STRIP", "width"): "Strip width.",
    ("MOBIUS_STRIP", "twists"): "Number of half-twists in the strip (odd = Möbius topology, even = orientable ring).",
    ("MOBIUS_STRIP", "range_x"): "Range of the X input mapped onto the strip's circumference.",
    ("MOBIUS_STRIP", "range_y"): "Range of the Y input mapped onto the strip's width.",
    ("MOBIUS_STRIP", "rotate_x"): "Rotation around the X axis (in units of 2π).",
    ("MOBIUS_STRIP", "rotate_y"): "Rotation around the Y axis (in units of 2π).",
    ("MOBIUS_STRIP", "modify_z"): "Z output scale factor. 0 disables Z output (only relevant in 3D mode).",
    ("MOBIUS_STRIP", "width_mode"): "Y out-of-range behavior: 0 = wrap, 1 = clamp, 2 = hide, 3 = leave (pass through).",
    ("MOBIUS_STRIP", "radial_mode"): "X out-of-range behavior: same 4-mode enum as `width_mode`.",

    ("CIRCLE_LINEAR", "Sc"): "Cell size — each cell occupies a `2·Sc × 2·Sc` region.",
    ("CIRCLE_LINEAR", "K"): "Linear-scale factor for the non-identity branch.",
    ("CIRCLE_LINEAR", "Dens1"): "Per-cell density threshold (probability that the cell is 'active' and modifies the output).",
    ("CIRCLE_LINEAR", "Dens2"): "Sub-density ratio for the ring-vs-scale split inside an active cell.",
    ("CIRCLE_LINEAR", "Reverse"): "When > 0, swaps which sub-branch gets the scale vs the ring map.",
    ("CIRCLE_LINEAR", "X"): "Unused in the body — preserved for cpp parity and preset compatibility.",
    ("CIRCLE_LINEAR", "Y"): "Unused in the body — preserved for cpp parity and preset compatibility.",
    ("CIRCLE_LINEAR", "Seed"): "Hash seed for the per-cell deterministic noise.",
}

PATH = "src/variations/defs/apo_misc19.rs"
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
