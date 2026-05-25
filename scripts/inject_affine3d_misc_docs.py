"""Apply doc-comments + per-param descriptions to affine3d_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "AFFINE3D": (
        "General 3D affine transform — applies translation, per-axis scaling, yaw/pitch/roll rotation, and optional shear in a single combined transform. The 15 parameters cover translate (3), scale (3), rotate (3 in degrees), and shear (6 cross-axis terms). The body automatically detects whether shear is significant (`|sxy| + |sxz| + ... > ε`) and skips the sheared path when all 6 shear params are below threshold.",
        ["Framelet"],
    ),
}

PARAM_DOC = {
    ("AFFINE3D", "translateX"): "X-axis translation (added to the final output).",
    ("AFFINE3D", "translateY"): "Y-axis translation.",
    ("AFFINE3D", "translateZ"): "Z-axis translation (3D only).",
    ("AFFINE3D", "scaleX"): "X-axis scale (multiplies x before rotation).",
    ("AFFINE3D", "scaleY"): "Y-axis scale.",
    ("AFFINE3D", "scaleZ"): "Z-axis scale.",
    ("AFFINE3D", "rotateX"): "Rotation around the X axis (pitch), in degrees.",
    ("AFFINE3D", "rotateY"): "Rotation around the Y axis (yaw), in degrees.",
    ("AFFINE3D", "rotateZ"): "Rotation around the Z axis (roll), in degrees.",
    ("AFFINE3D", "shearXY"): "Shear factor applied to Y in the X output.",
    ("AFFINE3D", "shearXZ"): "Shear factor applied to Z in the X output.",
    ("AFFINE3D", "shearYX"): "Shear factor applied to X in the Y output.",
    ("AFFINE3D", "shearYZ"): "Shear factor applied to Z in the Y output.",
    ("AFFINE3D", "shearZX"): "Shear factor applied to X in the Z output.",
    ("AFFINE3D", "shearZY"): "Shear factor applied to Y in the Z output.",
}

PATH = "src/variations/defs/affine3d_misc.rs"
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
