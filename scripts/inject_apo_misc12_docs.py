"""Apply doc-comments + per-param descriptions to apo_misc12.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "RINGS": (
        "Modular-radius ring warp — folds the input radius around `2·dx` cycles where `dx = (transform's affine e coefficient)² + ε`, producing concentric rings. Reads the affine `e` coefficient directly (not a normal variation parameter), so the ring spacing changes with the transform's pre-affine.",
        None,
    ),
    "RIPPLED": (
        "Tanh/cos ripple — emits `(tanh(x²+y²) · x, cos(x²+y²) · y)`. Soft saturation along X (via tanh) combined with cosine modulation along Y produces a ripple/wave pattern.",
        ["Raykoid666"],
    ),
    "WAFFLE": (
        "Waffle grid scatter — picks one of 5 sampling modes uniformly, then samples from a `slices × slices` grid with configurable per-axis line thickness. The output is rotated by `rotation`. Produces a woven-pattern scatter.",
        ["Jed Kelsey"],
    ),
    "STRIPFIT": (
        "Strip-fit fold — folds the Y axis into `[-1, 1]` strips (via mod-2 wrapping), with each fold adding a horizontal shift on X proportional to `dx · stripe_count`. The trailing X shift wasn't VVAR-scaled in upstream — handled via the divide-out pattern.",
        ["DarkBeam"],
    ),
}

PARAM_DOC = {
    ("WAFFLE", "slices"): "Number of grid divisions per axis.",
    ("WAFFLE", "xthickness"): "X-axis line thickness (0 = grid lines only, 1 = solid fill).",
    ("WAFFLE", "ythickness"): "Y-axis line thickness.",
    ("WAFFLE", "rotation"): "Output rotation in radians.",

    ("STRIPFIT", "dx"): "Per-strip horizontal shift amount (scaled by `-0.5` internally).",
}

PATH = "src/variations/defs/apo_misc12.rs"
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
