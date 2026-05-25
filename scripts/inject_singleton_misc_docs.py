"""Apply doc-comments + per-param descriptions to singleton_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent.

modulus and circlize already had # Authors blocks (Apophysis Plugin
Pack) and were edited manually to prepend descriptions; this script
handles the other 6 + all per-param descriptions."""
import re
import textwrap

DOC = {
    "CORNERS": (
        "Quadrant-based power warp — squares the input per axis, raises each to a power (`xpower + xypower` on X, `ypower + xypower` on Y), and multiplies by `multx`/`multy`. The sign of the input chooses whether the result is added or subtracted, and a constant `xwidth`/`ywidth` is added to the output. With `logmode = 1`, the squared input is first run through a log-base transform before the pow.",
        ["Whittaker Courtney"],
    ),
    "OCTAGON": (
        "Octagonal-tile warp — applies three sequential contributions: a radial term `r = 1/(x⁴ + y⁴)` (folded for `r < 2`), a Manhattan-distance reciprocal term, and a sign-based shift by `x`/`y`/`z`. The combined effect tiles the plane (or volume) with an octagonal pattern.",
        ["FracFx"],
    ),
    "CIRCUS": (
        "Radial radius scale — points inside the unit circle (`r ≤ 1`) are scaled outward by `scale`; points outside are scaled inward by `1/scale`. Creates a deliberate discontinuity at the unit circle.",
        ["Michael Faber"],
    ),
    "CIRCLIZE2": (
        "Variant of `circlize` — same L∞-square-to-circle mapping, but `hole` is folded into the variation-weighted radius so the whole formula factors cleanly through the outer weight. Same visual result as `circlize` for matching parameters, with subtly different weight semantics.",
        ["Michael Faber"],
    ),
    "ATAN_VAR": (
        "Per-axis arctangent warp with mode selector — applies `(2/π)·atan(stretch · coord)` to one or both axes (controlled by `mode`). Saturates the input toward ±1, with `stretch` controlling how quickly the saturation kicks in.",
        ["FractalDesire", "Brad Stefanov"],
    ),
    "MURL": (
        "Complex-power Möbius warp — treats the input as a complex number `z = x + iy`, computes `c·z^power + 1`, and applies the Möbius transform `z' = (c+1)·z / (c·z^power + 1)`. With `power = 1` and `c → 0` it reduces to identity; large `c` produces strong Möbius distortion.",
        ["Zueuk"],
    ),
}

PARAM_DOC = {
    ("CORNERS", "xwidth"): "Constant X offset added per quadrant (signed by the input's X sign).",
    ("CORNERS", "ywidth"): "Constant Y offset added per quadrant.",
    ("CORNERS", "multx"): "X-axis multiplier on the squared input before the pow.",
    ("CORNERS", "multy"): "Y-axis multiplier on the squared input before the pow.",
    ("CORNERS", "xpower"): "X-axis power exponent (combined additively with `xypower`).",
    ("CORNERS", "ypower"): "Y-axis power exponent (combined additively with `xypower`).",
    ("CORNERS", "xypower"): "Additional power offset added to both `xpower` and `ypower`.",
    ("CORNERS", "logmode"): "Formula selector: 0 = `pow(x², …)`, 1 = `pow(log_base(x²·mult + 3), …) − 1.33`.",
    ("CORNERS", "log_base"): "Log base used by the `logmode = 1` formula. Default ≈ e.",

    ("MODULUS", "x"): "X-axis half-width — points outside ±x get mod-wrapped back inside.",
    ("MODULUS", "y"): "Y-axis half-width.",

    ("OCTAGON", "x"): "X-axis sign-shift amount added at the end (signed by `sign(x)`).",
    ("OCTAGON", "y"): "Y-axis sign-shift amount.",
    ("OCTAGON", "z"): "Z-axis sign-shift amount (3D only).",

    ("CIRCUS", "scale"): "Inner-disc (`r ≤ 1`) scaling factor. Outside the disc, the reciprocal `1/scale` is used.",

    ("CIRCLIZE", "hole"): "Center-hole radial offset. Larger values produce a bigger central gap.",

    ("CIRCLIZE2", "hole"): "Center-hole radial offset (folded into the weighted radius).",

    ("ATAN_VAR", "mode"): "Which axes get arctangent-transformed: 0 = Y only, 1 = X only, 2 = both.",
    ("ATAN_VAR", "stretch"): "Pre-atan input scaling. Higher = sharper saturation toward ±1.",

    ("MURL", "c"): "Möbius coefficient. Rescaled internally by `1/(power−1)` when `power ≠ 1`.",
    ("MURL", "power"): "Complex power applied to the input. Integer values produce rotational symmetries; `power = 1` reduces to a simpler Möbius warp.",
}

PATH = "src/variations/defs/singleton_misc.rs"
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
