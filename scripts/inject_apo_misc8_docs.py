"""Apply doc-comments + per-param descriptions to apo_misc8.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "CSC_SQUARED": (
        "Cosecant-squared power scale — computes a csc-style intermediate `csc = csc_div / (cos(x/cos_div) · tan(x/tan_div))`, then a per-axis scale factor `f = (csc² + π·pi_mult)^csc_pow + csc_add`, then emits `(x·f, y·f·scaley)`. The csc/cos/tan composition produces sharp pole structures.",
        ["Whittaker Courtney"],
    ),
    "HYPERBOLICELLIPSE": (
        "Hyperbolic-elliptic mapping — emits `(sinh(x) · cos(a·y), cosh(x) · sin(a·y))`. Echoes the parametric ellipse `(cos t, sin t)` but with hyperbolic radial scaling along X.",
        None,
    ),
    "LAYERED_SPIRAL": (
        "Radial spiral — emits `(x · radius · cos(r² + ε), x · radius · sin(r² + ε))` where `r² = x² + y²`. The angular position rotates with squared radius, producing tightly wound layered spirals.",
        ["Will Evans"],
    ),
    "ATAN2_SPIRALS": (
        "14-parameter atan2-driven spiral generator — combines two `atan2` calls (one over per-power radial terms, one over per-divisor xy positions) and a sine wrap, with separate weights and offsets per component. X output is mirrored across `±π` based on the sign of x.",
        ["Whittaker Courtney"],
    ),
    "GRIDOUT2": (
        "8-cell quadrant routing — rounds the input to a grid (with separate cell sizes `c, d` per axis), then chooses one of 8 octant-style cell-boundary directions to add `±a` to X or `±b` to Y. Produces a grid-tile shifted pattern.",
        ["Michael Faber", "Joel Faber", "Brad Stefanov", "DarkBeam"],
    ),
}

PARAM_DOC = {
    ("CSC_SQUARED", "csc_div"): "Numerator of the csc fraction.",
    ("CSC_SQUARED", "cos_div"): "Divisor on x in the cos term `cos(x/cos_div)`.",
    ("CSC_SQUARED", "tan_div"): "Divisor on x in the tan term `tan(x/tan_div)`.",
    ("CSC_SQUARED", "csc_pow"): "Exponent on `csc² + π·pi_mult`.",
    ("CSC_SQUARED", "pi_mult"): "Coefficient on the π offset added to `csc²` before the pow.",
    ("CSC_SQUARED", "csc_add"): "Additive offset on the per-axis scale.",
    ("CSC_SQUARED", "scaley"): "Y-axis scale multiplier (applied after the per-axis scale).",

    ("HYPERBOLICELLIPSE", "a"): "Frequency multiplier on Y in the cos/sin arguments.",

    ("LAYERED_SPIRAL", "radius"): "Radial scale on the spiral magnitude.",

    ("ATAN2_SPIRALS", "r_mult"): "Multiplier on the first atan2's numerator (r-term).",
    ("ATAN2_SPIRALS", "r_add"): "Offset on the first atan2's numerator.",
    ("ATAN2_SPIRALS", "xy2_mult"): "Multiplier on the first atan2's denominator (xy²-term).",
    ("ATAN2_SPIRALS", "xy2_add"): "Offset on the first atan2's denominator.",
    ("ATAN2_SPIRALS", "x_mult"): "Output multiplier on the first atan2.",
    ("ATAN2_SPIRALS", "x_add"): "Output additive offset on X.",
    ("ATAN2_SPIRALS", "yx_div"): "Divisor on x in the second atan2.",
    ("ATAN2_SPIRALS", "yx_add"): "Additive offset on x in the second atan2.",
    ("ATAN2_SPIRALS", "yy_div"): "Divisor on y in the second atan2.",
    ("ATAN2_SPIRALS", "yy_add"): "Additive offset on y in the second atan2.",
    ("ATAN2_SPIRALS", "sin_add"): "Phase offset on the sine wrap around the second atan2.",
    ("ATAN2_SPIRALS", "y_mult"): "Output multiplier on Y.",
    ("ATAN2_SPIRALS", "r_power"): "Exponent on the radial term `r = (x²+y²)^r_power`.",
    ("ATAN2_SPIRALS", "x2y2_pow"): "Exponent on the xy² term `xy² = (x²+y²)^x2y2_pow`.",

    ("GRIDOUT2", "a"): "X-axis shift magnitude.",
    ("GRIDOUT2", "b"): "Y-axis shift magnitude.",
    ("GRIDOUT2", "c"): "X-axis cell size (multiplied with `round(x)` before the octant routing).",
    ("GRIDOUT2", "d"): "Y-axis cell size (multiplied with `round(y)`).",
}

PATH = "src/variations/defs/apo_misc8.rs"
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
