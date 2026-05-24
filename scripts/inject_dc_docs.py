"""Apply doc-comments + per-param descriptions to dc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "DC_LINEAR": (
        "Pass-through positioning (linear) with direct-color writes — colors each iteration based on a rotated linear projection of the post-variation point. Output position is unchanged from the input; effect only visible when the transform's Direct Color slider is > 0.",
        ["Xyrus02"],
    ),
    "DC_BUBBLE": (
        "Apophysis Bubble warp (spherical projection) with direct-color writes — colors each iteration based on the squared distance from a configurable center point. Same XY warp as Bubble, plus per-iteration color modulation. Color effect only visible when the transform's Direct Color slider is > 0.",
        ["Xyrus02"],
    ),
}

PARAM_DOC = {
    ("DC_LINEAR", "offset"): "Offset added to the projected coordinate before computing color.",
    ("DC_LINEAR", "angle"): "Rotation angle (degrees) for the projection axis.",
    ("DC_LINEAR", "scale"): "Scaling factor on the projection — larger compresses the color gradient, smaller stretches it.",

    ("DC_BUBBLE", "centerx"): "X coordinate of the radial color center.",
    ("DC_BUBBLE", "centery"): "Y coordinate of the radial color center.",
    ("DC_BUBBLE", "scale"): "Scaling factor on the squared distance — larger compresses the color gradient, smaller stretches it.",
}

PATH = "src/variations/defs/dc.rs"
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

# Both files use the macro form (param!(...)) so use macro-append pattern.
pinserted = 0
palready = 0
for (static_name, param_name), desc in PARAM_DOC.items():
    start_pattern = f"pub static {static_name}: VariationDef"
    start_idx = src.find(start_pattern)
    if start_idx == -1:
        print(f"  WARN: no static {static_name}")
        continue
    next_static = src.find("\npub static ", start_idx + 1)
    end_idx = next_static if next_static != -1 else len(src)
    block = src[start_idx:end_idx]

    macro_pat = re.compile(
        r'(param!\(\s*"' + re.escape(param_name) + r'"\s*,[^)]*?)(\))',
        re.DOTALL,
    )
    new_block, n = macro_pat.subn(
        lambda m: m.group(1) + f', "{desc}"' + m.group(2),
        block, count=1,
    )
    if n == 0:
        already_pat = re.compile(
            r'param!\(\s*"' + re.escape(param_name) + r'"[^)]*"' + re.escape(desc[:10]),
            re.DOTALL,
        )
        if already_pat.search(block):
            palready += 1
        else:
            print(f"  WARN: no param {static_name}.{param_name}")
        continue
    src = src[:start_idx] + new_block + src[end_idx:]
    pinserted += 1
print(f"  pass2: injected {pinserted}, already {palready}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
