"""Apply doc-comments + per-param descriptions to apo_misc10.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "MASK": (
        "Sin/cos masked warp — emits `(sin³(xf), cos(xf)·sin²(xf)) · (cosh(yf) + ushift) / (x² + y²)` where `xf = xscale·x + xshift` and `yf = yscale·y + yshift`. The sin-power factors create lobed masking; `cosh(yf)` modulates the overall magnitude based on Y.",
        ["Raykoid666", "CozyG"],
    ),
    "OVOID3D": (
        "3D ovoid — emits `(x·x_scale, y·y_scale, z·z_scale) / T` where `T = x² + y² + z² + ε`. Generalizes spherical inversion to per-axis scaling.",
        ["Larry Berlin"],
    ),
    "MURL2": (
        "Variant of `murl` — applies a more involved complex Möbius mapping: power-transforms the input, adds 1, then power-transforms the result back, then divides by squared magnitude. The `2/power` exponent in the internal `vp = w · (c+1)^(2/power)` factor adjusts the output magnitude per power value.",
        ["Zueuk"],
    ),
    "MINKQM": (
        "Minkowski's question-mark function — applies the Stern-Brocot tree iteration of Minkowski's `?(x)` to each axis separately. The function maps rationals with simple continued-fraction expansions to dyadic rationals, producing a self-similar staircase pattern. Parameters `a, b, c, dd, e` seed the SB tree recursion; `f` sets the iteration count.",
        ["DarkBeam", "Brad Stefanov"],
    ),
}

PARAM_DOC = {
    ("MASK", "xshift"): "Additive offset on X before the sin term.",
    ("MASK", "yshift"): "Additive offset on Y before the cosh term.",
    ("MASK", "ushift"): "Constant offset added to `cosh(yf)`.",
    ("MASK", "xscale"): "Multiplier on X before the sin term.",
    ("MASK", "yscale"): "Multiplier on Y before the cosh term.",

    ("OVOID3D", "x"): "X-axis scale on the inverted output.",
    ("OVOID3D", "y"): "Y-axis scale.",
    ("OVOID3D", "z"): "Z-axis scale (3D only).",

    ("MURL2", "c"): "Möbius coefficient — controls the strength of the Möbius distortion.",
    ("MURL2", "power"): "Power exponent. 0 falls back to a degenerate case with a high invp.",

    ("MINKQM", "a"): "Initial denominator `q` of the SB-tree recursion.",
    ("MINKQM", "b"): "Initial offset on the SB-tree numerator `r`.",
    ("MINKQM", "c"): "Initial denominator `s` of the SB-tree recursion.",
    ("MINKQM", "dd"): "Initial step size on the output `y`.",
    ("MINKQM", "e"): "Step decay factor — multiplied with `d` each iteration. 0.5 gives the standard Minkowski function.",
    ("MINKQM", "f"): "Iteration count. Default 20 yields about 1e-6 precision.",
}

PATH = "src/variations/defs/apo_misc10.rs"
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
