"""Apply doc-comments + per-param descriptions to maurer_hyper.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "MAURER_ROSE": (
        "Maurer-rose curve sampler — a Maurer rose steps around a rhodonea (rose) curve at fixed angular increments and connects consecutive samples with straight lines. This variation picks a random point on the nearest Maurer line, on one of its endpoints, or directly on the underlying rose curve, with the mix between the three modes controlled by relative weights.",
        ["CozyG"],
    ),
    "HYPERCROP": (
        "N-gon corner-cropping warp — snaps the input angle to the nearest n-gon spoke, finds that spoke's corner, and tests whether the input lies inside a small disc around the corner. Inside, behavior depends on `zero`: snap to corner, collapse to origin, or scatter around the disc edge. Outside, the point passes through.",
        ["tatasz", "Brad Stefanov"],
    ),
}

PARAM_DOC = {
    ("MAURER_ROSE", "kn"): "K numerator — the rose's petal ratio is `kn/kd`.",
    ("MAURER_ROSE", "kd"): "K denominator.",
    ("MAURER_ROSE", "c"): "Constant offset added to the rose radius. Shifts the curve outward.",
    ("MAURER_ROSE", "line_count"): "Number of Maurer-rose line segments per cycle.",
    ("MAURER_ROSE", "line_offset_degrees"): "Angular step between successive samples on the rhodonea, in degrees. Together with line_count, controls how many cycles wrap around the rose.",
    ("MAURER_ROSE", "show_lines"): "Relative weight of sampling along the Maurer line segments.",
    ("MAURER_ROSE", "show_points"): "Relative weight of sampling at the segment endpoints.",
    ("MAURER_ROSE", "show_curve"): "Relative weight of sampling directly on the underlying rhodonea curve.",
    ("MAURER_ROSE", "line_thickness"): "Random jitter width around line samples (×100).",
    ("MAURER_ROSE", "point_thickness"): "Random scatter radius around endpoint samples (×100).",
    ("MAURER_ROSE", "curve_thickness"): "Random jitter width around rose-curve samples (×100).",

    ("HYPERCROP", "n"): "Number of n-gon sides (≥ 3).",
    ("HYPERCROP", "rad"): "Radius of the corner-cropping disc, relative to the n-gon corner radius.",
    ("HYPERCROP", "zero"): "Behavior inside the corner disc. `> 1.5` snaps to the corner; `> 0.5` collapses to origin; else scatters around the disc edge.",
}

PATH = "src/variations/defs/maurer_hyper.rs"
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

    # Find: param!("paramname"  in this block, then balance parens to find close.
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
    # Check if a description (a string literal in last arg) is already present.
    # Easy heuristic: count top-level commas; canonical form has 5 args (name,
    # display, type, default, min, max) = 5 commas. >5 means desc already added.
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
