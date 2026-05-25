"""Apply doc-comments + per-param descriptions to misc_extras2.rs.

One-off script for the variations-bulk-metadata project. Idempotent.

bent2 and oscilloscope already had # Authors blocks and were edited
manually to prepend descriptions; this script handles the other 4 +
all per-param descriptions."""
import re
import textwrap

DOC = {
    "COLLIDEOSCOPE": (
        "Radial branch-and-mod (kaleidoscope-style) — converts the input to polar `(r, a)`, splits the angle into `num` equal sectors, and mirrors/shifts alternating sectors based on the parity of the sector index, with a per-sector angular offset `a`. Produces a kaleidoscope-like radial tiling.",
        ["Michael Faber"],
    ),
    "MCARPET": (
        "Bubble warp with twist and tilt — applies a bubble-style radial scaling `r = 1 / (r²/4 + 1)` to each axis with independent scale factors, then adds a quadratic twist term to X and a linear tilt term to Y.",
        ["FracFx"],
    ),
    "LINEART3D": (
        "Per-axis sign-preserving power — applies `sign(coord) · |coord|^pow` to each axis independently, with separate exponents per axis. Preserves the sign while compressing or expanding the magnitude.",
        ["FractalDesire"],
    ),
    "FIBONACCI2": (
        "Binet-formula complex Fibonacci — extends the Fibonacci sequence to complex inputs via the closed form `F(z) = (φ^z − (−φ)^(−z)) / √5`, with `sc` scaling the magnitude and `sc2` scaling the polar-radius exponential growth rate. Produces spiral patterns based on the golden ratio.",
        ["Larry Berlin"],
    ),
}

PARAM_DOC = {
    ("COLLIDEOSCOPE", "a"): "Per-sector angular offset — the parity-based mirror is shifted by ±a.",
    ("COLLIDEOSCOPE", "num"): "Number of angular sectors. Higher = finer kaleidoscope.",

    ("BENT2", "x"): "X-axis scale applied where x < 0. Positive x passes through unchanged.",
    ("BENT2", "y"): "Y-axis scale applied where y < 0. Positive y passes through unchanged.",

    ("MCARPET", "x"): "X-axis bubble scale.",
    ("MCARPET", "y"): "Y-axis bubble scale.",
    ("MCARPET", "twist"): "Quadratic twist amount on X output (subtracts `twist · x²`).",
    ("MCARPET", "tilt"): "Linear tilt amount on Y output (adds `tilt · x`).",

    ("LINEART3D", "powX"): "X-axis power exponent.",
    ("LINEART3D", "powY"): "Y-axis power exponent.",
    ("LINEART3D", "powZ"): "Z-axis power exponent (3D only).",

    ("OSCILLOSCOPE", "separation"): "Vertical offset of the band threshold, added to the cosine envelope.",
    ("OSCILLOSCOPE", "frequency"): "Cosine frequency (multiplied by 2π internally).",
    ("OSCILLOSCOPE", "amplitude"): "Cosine amplitude.",
    ("OSCILLOSCOPE", "damping"): "Exponential damping rate. Values near zero disable the damping term.",

    ("FIBONACCI2", "sc"): "Output magnitude scale.",
    ("FIBONACCI2", "sc2"): "Polar-radius exponential scale — controls how quickly the magnitude grows along X.",
}

PATH = "src/variations/defs/misc_extras2.rs"
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
