"""Apply doc-comments + per-param descriptions to apo_misc20.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "CANNABISCURVE_WF": (
        "Cannabis-curve polar plot — emits a point on the cannabis curve `r = (1 + 0.9·cos 8a) · (1 + 0.1·cos 24a) · (0.9 + 0.1·cos 200a) · (1 + sin a)`. The curve is documented at [MathWorld](https://mathworld.wolfram.com/CannabisCurve.html) by Eric W. Weisstein. When `filled = 1`, randomizes the radius to fill the interior of the curve.",
        None,
    ),
    "SPHERICAL3D_WF": (
        "3D spherical inversion with adjustable exponent — emits `(x, y, z) / r^exponent` where `r² = x² + y² + z²`. With `exponent = 2` (the default) this reduces to a standard 3D spherical inversion; other values produce stronger or weaker radial scaling. `invert` flips the sign of the output.",
        None,
    ),
    "SWIRL3D_WF": (
        "3D swirl with Z modulation — re-emits the input radius and angle in cartesian form (the XY output is effectively the input swapped, per the cpp's `atan2(x, y)` convention) plus a Z output `sin(6·cos(rad) − n·ang)` that introduces a sinusoidal Z modulation parameterized by `n`.",
        None,
    ),
}

PARAM_DOC = {
    ("CANNABISCURVE_WF", "filled"): "1 = fill the curve interior by randomizing the radius per iteration; 0 = trace only the curve outline.",

    ("SPHERICAL3D_WF", "invert"): "1 = flip the sign of the output (inverts through the origin); 0 = standard direction.",
    ("SPHERICAL3D_WF", "exponent"): "Radial-inversion exponent. 2 = standard spherical (`r⁻²`); higher = stronger inverse; lower = weaker.",

    ("SWIRL3D_WF", "n"): "Angular multiplier on the Z-output sine: `sin(6·cos(rad) − n·ang)`.",
}

PATH = "src/variations/defs/apo_misc20.rs"
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
