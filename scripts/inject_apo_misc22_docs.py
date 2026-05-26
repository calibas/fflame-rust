"""Apply doc-comments + per-param descriptions to apo_misc22.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "DC_CARPET": (
        "Randomized fractal-carpet warp — splits the input into a fractional part `frac(|coord|)` (toward-zero rounding) and adds a per-axis random `±1` cell offset, then passes through the transform's pre-affine `(a·x + b·y + e, c·x + d·y + f)`. Produces a recursive Sierpinski-carpet-style scatter pattern when combined with self-similar transforms.",
        None,
    ),
    "POST_POINT_SYMMETRY_WF": (
        "Post-phase N-fold rotational symmetry — each iteration picks a random rotation index in `[0, order)` and rotates the accumulator point by `idx · 2π/order` around `(centre_x, centre_y)`. Produces N-fold rotational symmetry around a configurable center.",
        None,
    ),
    "CPOW3_WF": (
        "Randomized complex-power warp (CPow3 family) — combines a complex-power radial-angular mapping with discrete and continuous angular spreads, plus a secondary multiplicative random offset on the final angle. Per-iteration: random shift of the angle index `n` (truncated to integer if `discrete_spread = 1`), a probabilistic angle subtraction, and a `spread2 · rand + offset2` random multiplier on the output angle.",
        ["CozyG"],
    ),
}

PARAM_DOC = {
    ("DC_CARPET", "origin"): "Unused in the body — preserved as a parameter for cpp parity and preset compatibility.",
    ("DC_CARPET", "iterations"): "Unused in the body — preserved as a parameter for cpp parity.",

    ("POST_POINT_SYMMETRY_WF", "centre_x"): "X center of the rotational symmetry.",
    ("POST_POINT_SYMMETRY_WF", "centre_y"): "Y center of the rotational symmetry.",
    ("POST_POINT_SYMMETRY_WF", "order"): "Number of rotational-symmetry orders (≥ 1). 3 = 3-fold rotation, 4 = 4-fold, etc.",

    ("CPOW3_WF", "r"): "Complex-power magnitude.",
    ("CPOW3_WF", "a"): "Complex-power angle (scaled by π/2 internally).",
    ("CPOW3_WF", "divisor"): "Divides both r and a contributions; controls how the power scales.",
    ("CPOW3_WF", "spread"): "Range of the random angle index `n`. Each iteration picks `n` uniformly in `[0, spread)`.",
    ("CPOW3_WF", "discrete_spread"): "≥ 1 = truncate `n` to an integer (discrete angular branches); < 1 = continuous angle.",
    ("CPOW3_WF", "spread2"): "Range of the secondary multiplicative random offset on the final angle.",
    ("CPOW3_WF", "offset2"): "Constant offset added to the secondary random multiplier.",
}

PATH = "src/variations/defs/apo_misc22.rs"
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
