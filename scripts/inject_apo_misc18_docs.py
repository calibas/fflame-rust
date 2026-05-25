"""Apply doc-comments + per-param descriptions to apo_misc18.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "LAZYSENSEN": (
        "Per-axis floor-and-flip — for each axis with non-zero scale, negates the coordinate when `floor(coord · scale)` parity matches a sign-dependent rule (parity ≠ 0 for non-negative; parity = 0 for negative). Produces a zig-zag mirror pattern whose stripe width is controlled by the per-axis scale.",
        ["bezo97"],
    ),
    "SPHERECROP": (
        "3D version of `circlecrop` — tests whether the input lies inside a sphere of radius `radius` centered at `(x, y, z)`. Inside, the input passes through scaled by weight. Outside, behavior depends on `zero`: 1 = hide (collapse to origin), 0 = scatter onto the sphere surface (with `scatter_area` randomization).",
        ["Xyrus02"],
    ),
    "XHEART_BLUR_WF": (
        "Random heart-shape blur — picks a uniform random point in `[-2, 2]²` and runs it through the `xheart` Möbius-style heart warp (rotated by `angle`, with Y-stretch controlled by `ratio`). Produces a scattered heart-silhouette splat that's independent of the iteration's input position.",
        None,
    ),
}

PARAM_DOC = {
    ("LAZYSENSEN", "scale_x"): "X-axis stripe scale. 0 disables the X flip; the stripe width is `1/|scale_x|`.",
    ("LAZYSENSEN", "scale_y"): "Y-axis stripe scale.",
    ("LAZYSENSEN", "scale_z"): "Z-axis stripe scale (3D only).",

    ("SPHERECROP", "radius"): "Sphere radius.",
    ("SPHERECROP", "x"): "X center of the sphere.",
    ("SPHERECROP", "y"): "Y center of the sphere.",
    ("SPHERECROP", "z"): "Z center of the sphere.",
    ("SPHERECROP", "scatter_area"): "Random scatter band along the sphere surface. 0 = snap to surface; ±1 = scatter across full half-radius.",
    ("SPHERECROP", "zero"): "Behavior outside the sphere: 1 = hide (collapse to origin), 0 = scatter onto surface.",

    ("XHEART_BLUR_WF", "angle"): "Rotation angle of the heart shape (scaled by π/8 internally and offset from π/4).",
    ("XHEART_BLUR_WF", "ratio"): "Y-axis stretch factor (added to a base of 6 to control heart roundness, same as `xheart`).",
}

PATH = "src/variations/defs/apo_misc18.rs"
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
