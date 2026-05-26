"""Apply doc-comments + per-param descriptions to bipolar_series.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

FABER = ["Michael Faber"]

DOC = {
    "BCOLLIDE": (
        "Bipolar warp with σ-axis branch-and-mod — converts the input to bipolar coordinates `(τ, σ)`, splits σ into `num` equal arcs, and mirrors alternating arcs around their boundaries with a per-arc offset `a`. Produces a bipolar tiling with controllable wedge count.",
        FABER,
    ),
    "BMOD": (
        "Bipolar warp with τ-axis clamp-and-mod — converts to bipolar coordinates, then wraps τ around within `±radius` (offset by `distance·radius`). Points with τ outside the band pass through unchanged.",
        FABER,
    ),
    "BSWIRL": (
        "Bipolar swirl — adds `τ·out + in/τ` to the bipolar σ coordinate. The combination of an additive (`out`) and reciprocal (`in`) term produces a hyperbolic-style swirl pattern aligned with the bipolar grid.",
        FABER,
    ),
    "BARYCENTROID": (
        "Barycentric-coordinate warp — treats the input as the third vertex of a triangle whose other two vertices are user-defined: `V0 = (a, b)`, `V1 = (c, d)`, `V2 = (x, y)`. Computes barycentric coordinates `(u, v)` of `(x, y)` in the triangle, then emits `(sign(u)·sqrt(u²+x²), sign(v)·sqrt(v²+y²))`.",
        ["Xyrus02"],
    ),
    "ECOLLIDE": (
        "Elliptic warp with ν-axis branch-and-mod — converts to elliptic coordinates `(μ, ν)`, splits ν into `num` equal wedges, and mirrors alternating wedges with a per-wedge offset. Elliptic analogue of `bcollide`.",
        FABER,
    ),
    "EMOD": (
        "Elliptic warp with μ-axis clamp-and-mod — wraps μ around within `±radius` (offset by `distance·radius`). The sign of ν determines the mod direction. Elliptic analogue of `bmod`.",
        FABER,
    ),
    "ESWIRL": (
        "Elliptic swirl — adds `μ·out + in/μ` to the elliptic ν coordinate. Elliptic analogue of `bswirl`.",
        FABER,
    ),
    "ESCALE": (
        "Elliptic scale + angular wrap — scales μ by `scale` and applies a scale-plus-angle modular operation to ν, wrapping the result into `[-π, π]`. Useful for periodic ellipse-tiled patterns.",
        FABER,
    ),
    "EPUSH": (
        "Elliptic push — additively offsets μ by `push`, multiplicatively scales μ by `dist`, and rotates ν by `rotate`.",
        FABER,
    ),
    "EROTATE": (
        "Elliptic rotation — adds `rotate` to the elliptic ν coordinate and wraps the result into `[-π, π]`. Uses the `xmax·cos(ν)` and `sqrt(xmax²−1)·sin(ν)` output form (matching `ecollide`) rather than the cosh/sinh form used by the other E-series variations.",
        FABER,
    ),
}

PARAM_DOC = {
    ("BCOLLIDE", "num"): "Number of σ wedges. Higher = finer tiling.",
    ("BCOLLIDE", "a"): "Per-wedge angular offset.",

    ("BMOD", "radius"): "Half-width of the τ band to mod-wrap.",
    ("BMOD", "distance"): "Additional τ offset applied before mod, in units of `radius`.",

    ("BSWIRL", "in_p"): "Reciprocal-τ swirl coefficient — adds `in/τ` to σ.",
    ("BSWIRL", "out_p"): "Linear-τ swirl coefficient — adds `τ·out` to σ.",

    ("BARYCENTROID", "a"): "V0 X coordinate (first triangle vertex).",
    ("BARYCENTROID", "b"): "V0 Y coordinate.",
    ("BARYCENTROID", "c"): "V1 X coordinate (second triangle vertex).",
    ("BARYCENTROID", "d"): "V1 Y coordinate.",

    ("ECOLLIDE", "num"): "Number of ν wedges.",
    ("ECOLLIDE", "a"): "Per-wedge angular offset.",

    ("EMOD", "radius"): "Half-width of the μ band to mod-wrap.",
    ("EMOD", "distance"): "Additional μ offset, in units of `radius`.",

    ("ESWIRL", "in_p"): "Reciprocal-μ swirl coefficient — adds `in/μ` to ν.",
    ("ESWIRL", "out_p"): "Linear-μ swirl coefficient — adds `μ·out` to ν.",

    ("ESCALE", "scale"): "Multiplicative scale on μ; also scales the ν mod-wrap window.",
    ("ESCALE", "angle"): "Angular offset applied to ν before scaling, in degrees.",

    ("EPUSH", "push"): "Additive offset on μ.",
    ("EPUSH", "dist"): "Multiplicative scale on μ.",
    ("EPUSH", "rotate"): "Angular shift on ν, in radians.",

    ("EROTATE", "rotate"): "Angular shift on ν, in radians.",
}

PATH = "src/variations/defs/bipolar_series.rs"
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
