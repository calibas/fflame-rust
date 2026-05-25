"""Apply doc-comments + per-param descriptions to apo_misc21.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "HEART_WF": (
        "Polar heart-curve mapping — maps the input radius `r` and angle `a` onto a heart-shaped curve via `nx = ±0.001·(-t² + 40t + 1200)·sin(πt/180)·r` and `ny = -0.001·(-t² + 40t + 400)·cos(πt/180)·r` where `t = |a|/π · 60 · scale_r − shift_t` (capped at 60). The sign of `a` selects the left vs right half of the heart, each with its own radial scale.",
        None,
    ),
    "POST_ZTRANSLATE_WF": (
        "Post-phase Z translate — adds the variation weight to the Z coordinate. The XY coordinates pass through unchanged. Useful for offsetting the Z position of a transform's accumulated output.",
        None,
    ),
    "POST_MIRROR_WF": (
        "Post-phase per-axis mirroring — for each enabled axis (`xaxis`, `yaxis`, `zaxis`), with 50% probability per iteration flips the corresponding output coordinate via `coord = −scale · (coord + shift)`. Each axis's RNG draw is independent, so multiple axes can flip in the same iteration.",
        None,
    ),
}

PARAM_DOC = {
    ("HEART_WF", "scale_x"): "X output scale factor (multiplies the final X output).",
    ("HEART_WF", "scale_t"): "Unused in body — preserved as a parameter for cpp parity and preset compatibility.",
    ("HEART_WF", "shift_t"): "T parameter offset subtracted from the `|a|/π · 60 · scale_r` expression.",
    ("HEART_WF", "scale_r_left"): "Radial scale for the left half of the heart (input angle `a < 0`).",
    ("HEART_WF", "scale_r_right"): "Radial scale for the right half (input angle `a ≥ 0`).",

    ("POST_MIRROR_WF", "xaxis"): "1 = enable X-axis mirror branch; 0 = disable.",
    ("POST_MIRROR_WF", "yaxis"): "1 = enable Y-axis mirror branch; 0 = disable.",
    ("POST_MIRROR_WF", "zaxis"): "1 = enable Z-axis mirror branch (3D only); 0 = disable.",
    ("POST_MIRROR_WF", "xshift"): "X mirror offset — applied as `-scale·(x + xshift)` when the X branch fires.",
    ("POST_MIRROR_WF", "yshift"): "Y mirror offset.",
    ("POST_MIRROR_WF", "zshift"): "Z mirror offset.",
    ("POST_MIRROR_WF", "xscale"): "X output scale (applied in both the X-branch and Y-branch paths).",
    ("POST_MIRROR_WF", "yscale"): "Y output scale (applied in both branch paths).",
}

PATH = "src/variations/defs/apo_misc21.rs"
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
