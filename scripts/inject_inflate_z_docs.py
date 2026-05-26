"""Apply doc-comments + per-param descriptions to inflate_z.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

BERLIN = ["Larry Berlin"]

DOC = {
    "INFLATEZ_1": (
        "Z-axis inflation #1 — adds `sin(atan2(y,x)) − 2y` to the Z output. XY pass through unchanged. The angular sine plus linear-Y term produces a saddle-shaped Z surface.",
        BERLIN,
    ),
    "INFLATEZ_2": (
        "Z-axis inflation #2 — adds `0.25 − (2x + 2y)/3` to the Z output. XY pass through unchanged. Z is a tilted plane depending on `x + y`.",
        BERLIN,
    ),
    "INFLATEZ_3": (
        "Z-axis inflation #3 — adds `0.2 · (π − atan2(y,x)) · cos(3·atan2(y,x) + (y − x))` to the Z output. An angular-cosine modulated radial term producing a wavy 3D surface.",
        BERLIN,
    ),
    "INFLATEZ_4": (
        "Z-axis inflation #4 — adds `±(π/2 − atan2(y,x)) · 0.25` to the Z output, with the sign chosen randomly each iteration. Produces a Z surface that's a stochastic mirror of the input angle.",
        BERLIN,
    ),
    "INFLATEZ_5": (
        "Z-axis inflation #5 — adds `cos(π/2 − atan2(y,x)) / 2 = sin(atan2(y,x)) / 2` to the Z output. The simplest of the family — a sinusoidal Z surface.",
        BERLIN,
    ),
    "INFLATEZ_6": (
        "Z-axis inflation #6 — adds `1.5 − acos(sin(atan2(y,x)) · atan2(y,x) · sin(y − x) · 0.5)` to the Z output. The most complex of the family — an arc-cosine of a triple-product term.",
        BERLIN,
    ),
    "FOCI_3D": (
        "3D extension of the `foci` variation — emits `(expx − expnx, sin(y), sin(boot)) / (expx + expnx − cos(y)·cos(boot))` where `expx = e^x / 2, expnx = e^(−x) / 2`, and `boot = z` (or `atan2(y, x)` when `z = 0`, the 2D fallback). Adds a depth dimension to the classic foci warp.",
        BERLIN,
    ),
    "SINTRANGE": (
        "Sin × (squared − weighted-radius) — per axis emits `sin(coord) · (coord² + w − (x² + y²)·w)`. The trailing weighted-radius subtraction modulates the local sin profile based on distance from the origin.",
        ["Ffey"],
    ),
}

PARAM_DOC = {
    ("SINTRANGE", "w"): "Weight on the radius term `(x²+y²)·w` and the constant offset `+w`. Distinct from the variation's outer weight (VVAR).",
}

PATH = "src/variations/defs/inflate_z.rs"
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
