"""Apply doc-comments + per-param descriptions to butterfly_fay_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "BUTTERFLY_FAY": (
        "Parametric butterfly curve — emits points on a butterfly curve `r = ½·(exp(cos t) − 2·cos 4t − sin⁵(t/12) + offset)` driven by the input angle `t = cycles · atan2(y, x)`. Based on the Butterfly curve discovered ~1988 by Temple H. Fay. Routes through one of 6 output modes (controlled by `outer_mode`/`inner_mode`) depending on whether the input lies inside or outside the curve, with optional `fill` randomization.",
        ["CozyG"],
    ),
}

PARAM_DOC = {
    ("BUTTERFLY_FAY", "cycles"): "Number of butterfly-curve cycles per full input rotation. 0 falls back to π² internally.",
    ("BUTTERFLY_FAY", "offset"): "Additive offset on the curve radius formula.",
    ("BUTTERFLY_FAY", "unified_inner_outer"): "1 = always use the outer mode/spread/ratio; 0 = pick based on whether the input is inside or outside the curve.",
    ("BUTTERFLY_FAY", "outer_mode"): "Output mode for points outside the curve. 0-5; same enum as `inner_mode`.",
    ("BUTTERFLY_FAY", "inner_mode"): "Output mode for points inside the curve. 0-5; same enum as `outer_mode`.",
    ("BUTTERFLY_FAY", "outer_spread"): "Outer-mode spread amount (interpretation depends on `outer_mode`).",
    ("BUTTERFLY_FAY", "inner_spread"): "Inner-mode spread amount (interpretation depends on `inner_mode`).",
    ("BUTTERFLY_FAY", "outer_spread_ratio"): "X-vs-Y ratio for outer-mode spread.",
    ("BUTTERFLY_FAY", "inner_spread_ratio"): "X-vs-Y ratio for inner-mode spread.",
    ("BUTTERFLY_FAY", "spread_split"): "Multiplier on the input radius used to decide inner vs outer (compared against the curve radius).",
    ("BUTTERFLY_FAY", "fill"): "Random fill amount added to the curve radius. 0 disables; non-zero triggers an RNG call.",
}

PATH = "src/variations/defs/butterfly_fay_misc.rs"
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
