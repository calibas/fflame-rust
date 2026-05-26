"""Apply doc-comments + per-param descriptions to dc_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "DC_CYLINDER": (
        "Direct-color cylinder warp — wraps the input around a cylinder via `sin(x + rr·sin(a))` on X with a Gaussian-approximated Y blur (`rr = blur · (sum of 4 uniforms − 2)`). 3D mode adds a `cos(x + rr·cos(a))` Z component. The cpp source also writes a color value (TC); that color write is dropped per the writes_color compromise.",
        ["FracFx"],
    ),
    "DC_CYLINDER2": (
        "Variant of `dc_cylinder` — same XY/Z structure but the Y output is `rr · y · y_scale` (multiplicative) instead of `rr + y · y_scale` (additive). The cpp source also writes color; the color write is dropped.",
        ["FracFx"],
    ),
    "DC_TRIANGLE": (
        "Barycentric-coordinate triangle mapping — uses the transform's affine `(a, b)` and `(-c, -d)` as the triangle edge basis, with `(e, f)` as the origin vertex. Tests whether the input lies inside the triangle via barycentric coordinates `(u, v)`. Inside, passes through. Outside, either collapses to origin (`zero_edges = 1`) or scatters by `scatter_area` from the nearest edge.",
        None,
    ),
}

CYLINDER_PARAMS = {
    "offset": "Cylinder phase offset. Init precomputes `offset · π` (but this isn't read in the body — kept for cpp parity).",
    "angle": "Cylinder rotation angle (unused in body — kept for cpp parity).",
    "scale": "Cylinder scale. Init precomputes `1/scale` (but this isn't read in the body — kept for cpp parity).",
    "x": "X-axis output scale (multiplies the `sin(x + rr·sin(a))` term).",
    "y": "Y-axis output scale (multiplies the input Y in the output).",
    "blur": "Gaussian blur amplitude — multiplies the sum-of-4-uniforms approximation.",
}

PARAM_DOC = {}
for static_name in ("DC_CYLINDER", "DC_CYLINDER2"):
    for k, v in CYLINDER_PARAMS.items():
        PARAM_DOC[(static_name, k)] = v

PARAM_DOC[("DC_TRIANGLE", "scatter_area")] = "Random scatter amount applied to points outside the triangle. Clamped to `[-1, 1]` internally."
PARAM_DOC[("DC_TRIANGLE", "zero_edges")] = "1 = collapse out-of-triangle points to origin; 0 = scatter them by `scatter_area`."

PATH = "src/variations/defs/dc_misc.rs"
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
