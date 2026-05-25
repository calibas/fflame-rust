"""Apply doc-comments + per-param descriptions to apo_misc11.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "SWIRL3": (
        "Log-spiral swirl — emits `(rad · cos(ang), rad · sin(ang))` where `rad = sqrt(x²+y²) + ε` and `ang = atan2(x, y) + log(rad) · shift`. The `log(rad)` term makes the angular offset grow linearly with the log of radius, producing a logarithmic-spiral swirl.",
        None,
    ),
    "WDISC": (
        "Wedge disc — emits `r · (cos a, sin a)` where `a = π / (sqrt(x²+y²) + 1)` and `r = atan2(y, x) / π`. Points with `r > 0` get their `a` reflected via `π − a`. Maps the input plane onto a folded-disc pattern.",
        ["Michael Faber"],
    ),
    "SPH3D": (
        "3D spherical with per-axis scales — emits `(x, y, z) / (x_scale²·x² + y_scale²·y² + zz² + ε)` where `zz = x_scale · z` (upstream typo: cpp+Java both use `x_scale` instead of `z_scale` in the Z denominator term, preserved). Per-axis scales tune the asymmetry of the inverse-distance scaling.",
        ["Xyrus02"],
    ),
    "INVSQUIRCULAR": (
        "Inverse `squircular` — undoes the squircular Möbius warp. The body has the variation weight `w²` appearing nonlinearly inside `r2 = sqrt(r₀ · (w²·r₀ − 4u²v²) / w)`, so the shape changes qualitatively with weight.",
        None,
    ),
    "SPHERE_NJA": (
        "Parametric sphere — uses `t = sqrt(x²+y²+z²)/stretch − π/2` to parameterize a sphere with `cos(t)` radius. Per-axis output mixes cos/sin terms with the input position and configurable shift offsets, producing a 3D sphere centered at `(shift_x, shift_y, 0)`.",
        ["Nicolaus Anderson"],
    ),
}

PARAM_DOC = {
    ("SWIRL3", "shift"): "Logarithmic spiral coefficient. Larger = tighter spiral.",

    ("SPH3D", "x"): "X-axis scale on the inverse-distance denominator.",
    ("SPH3D", "y"): "Y-axis scale.",
    ("SPH3D", "z"): "Z-axis scale — note: due to an upstream typo the body actually uses `x_scale` for `zz`, so this parameter is effectively unused. Preserved for preset compatibility.",

    ("SPHERE_NJA", "circle_a"): "Declared in the upstream source but unused in the body. Preserved as a parameter for preset compatibility.",
    ("SPHERE_NJA", "circle_b"): "Declared but unused (same as `circle_a`).",
    ("SPHERE_NJA", "shift_x"): "X-axis center offset (subtracted from input, added back to output).",
    ("SPHERE_NJA", "shift_y"): "Y-axis center offset.",
    ("SPHERE_NJA", "shift_z"): "Z-axis center offset.",
    ("SPHERE_NJA", "stretch"): "Radial-to-angular scaling factor in `t = sqtr/stretch − π/2`.",
}

PATH = "src/variations/defs/apo_misc11.rs"
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
