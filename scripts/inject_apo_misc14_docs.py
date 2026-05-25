"""Apply doc-comments + per-param descriptions to apo_misc14.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "WAVES2_RADIAL": (
        "Radially-falloff variant of `waves2` — applies the `waves2` per-axis sine offsets (`sin(y·freqx)·scalex` on X, `sin(x·freqy)·scaley` on Y) with a smooth radial falloff: full effect below the `null` radius, zero above the `distance` radius, linearly interpolated in between.",
        ["Tatyana Zabanova", "Brad Stefanov"],
    ),
    "SPLIPTIC_BS": (
        "Elliptic-coordinate warp with random Y-sign + constant offsets — converts the input to elliptic coordinates, emits `v · atan2(a, b)` on X (sign-shifted by `±x_p`), and `±v · log(xmax + sqrt(xmax−1))` on Y (sign chosen randomly each iteration, with `±y_p` offset). A Stefanov-tuned elliptic warp.",
        ["Brad Stefanov"],
    ),
    "POINCARE3D": (
        "Poincaré-disc-style 3D hyperbolic-tiling generator — projects the input through a Möbius inversion centered at `c = (-r·cos(a·π/2)·cos(b·π/2), r·sin(a·π/2)·cos(b·π/2), -r·sin(b·π/2))`. The `r, a, b` parameters control the center's distance from origin and its angular position on a sphere of radius `r`.",
        ["Zueuk"],
    ),
}

PARAM_DOC = {
    ("WAVES2_RADIAL", "w2r_scalex"): "X-axis sine amplitude.",
    ("WAVES2_RADIAL", "w2r_scaley"): "Y-axis sine amplitude.",
    ("WAVES2_RADIAL", "w2r_freqx"): "X-axis sine frequency.",
    ("WAVES2_RADIAL", "w2r_freqy"): "Y-axis sine frequency.",
    ("WAVES2_RADIAL", "w2r_null"): "Inner radius — full effect below this distance from the origin.",
    ("WAVES2_RADIAL", "w2r_distance"): "Outer radius — zero effect above this distance.",

    ("SPLIPTIC_BS", "x"): "X-axis constant offset. Sign added when input x ≥ 0, subtracted when input x < 0.",
    ("SPLIPTIC_BS", "y"): "Y-axis constant offset. Sign chosen by the same random branch that picks the Y-output sign.",

    ("POINCARE3D", "r"): "Center distance from origin (radius of the inversion center).",
    ("POINCARE3D", "a"): "Azimuthal angle of the inversion center (in units of π/2).",
    ("POINCARE3D", "b"): "Polar angle of the inversion center (in units of π/2).",
}

PATH = "src/variations/defs/apo_misc14.rs"
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
