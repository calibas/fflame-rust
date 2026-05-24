"""Apply doc-comments + per-param descriptions to shapes.rs.

One-off script for the variations-bulk-metadata project. Idempotent.
PIE's variation-level doc was added manually first (the existing
# Authors block needed prepending, not replacement)."""
import re
import textwrap

# (description, list of author lines or None)
DOC = {
    "TANCOS": (
        "Tan/cos blend — X gets scaled by tanh of the squared radius, Y by cos of the squared radius. Produces wavy concentric ring patterns.",
        ["Raykoid666"],
    ),
    "TANGENT": (
        "Real tan-by-cos — X is `sin(x)/cos(y)`, Y is `tan(y)`. Note: not the same as Tan from trig.rs (which is the complex tangent).",
        None,
    ),
    "TANGENT3D": (
        "3D extension of Tangent. Adds `z' = tan(x)` so the variation contributes depth modulation along the X coordinate.",
        None,
    ),
    "SECANT2": (
        "Variant of secant with a sign-dependent constant offset on Y. Passes X through; Y becomes `1/cos(r) ± 1` depending on the sign of `cos(r)`. Produces banded patterns with a sharp jump at the cos-sign boundary.",
        None,
    ),
    "COSINE": (
        "Complex cosine of `π·x + iy` — output is `(cos(πx)·cosh(y), -sin(πx)·sinh(y))`. Horizontally periodic with vertical exponential growth.",
        None,
    ),
    "PETAL": (
        "Petal shape — `(cos(x)·bx, cos(x)·by)` where `bx, by` are cubed sine/cosine products of `(x, y)`. Produces flower-like radial structures.",
        ["Raykoid666"],
    ),
    "CARDIOID": (
        "Parameterized cardioid (heart-shaped curve). The `a` parameter controls how many lobes/cusps the shape has.",
        ["Michael Faber"],
    ),
    "HELIX": (
        "3D helix — winds the (x, y) coordinates around the Z axis with the given frequency and width. In 2D mode (z = 0) collapses to a simple horizontal shift by `width`.",
        ["zy0rg"],
    ),
    "HELICOID": (
        "3D helicoid — rotates (x, y) by an angle proportional to Z, preserving the radius from the origin. In 2D mode collapses to identity.",
        ["zy0rg"],
    ),
    "PARABOLA": (
        "Randomly-amplitude parabola. Output X is height-scaled `sin²(r)` times a uniform random; Y is width-scaled `cos(r)` times another uniform. Produces blurry parabolic arcs.",
        ["cyberxaos"],
    ),
    "PIE3D": (
        "3D version of Pie — same pie-slice splatter as Pie but adds `z' = r·sin(r)` for depth modulation.",
        None,
    ),
}

PARAM_DOC = {
    ("CARDIOID", "a"): "Number of cusps/lobes in the cardioid shape. 1 = standard heart, 2 = figure-eight, higher values add more lobes.",

    ("HELIX", "frequency"): "How many full turns the helix makes per unit of Z.",
    ("HELIX", "width"): "Radius of the helical winding around the Z axis.",

    ("HELICOID", "frequency"): "How fast the (x, y) plane rotates as Z increases. Larger = tighter spiral.",

    ("PARABOLA", "width"): "Horizontal scaling of the parabolic envelope.",
    ("PARABOLA", "height"): "Vertical scaling of the parabolic envelope.",

    ("PIE", "slices"): "Number of pie wedges (1-64).",
    ("PIE", "rotation"): "Rotation angle of the whole pie in degrees.",
    ("PIE", "thickness"): "Wedge thickness within its slice. 0 = razor-thin spokes, 1 = wedges fill their entire slice.",

    ("PIE3D", "slices"): "Number of pie wedges (1-64).",
    ("PIE3D", "rotation"): "Rotation angle of the whole pie in degrees.",
    ("PIE3D", "thickness"): "Wedge thickness within its slice. 0 = razor-thin spokes, 1 = wedges fill their entire slice.",
}

PATH = "src/variations/defs/shapes.rs"
with open(PATH, "rb") as f:
    src = f.read().decode("utf-8")

# PASS 1: insert doc-comments
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

# PASS 2: per-param descriptions (longhand-compact form)
pinserted = 0
palready = 0
for (static_name, param_name), desc in PARAM_DOC.items():
    start_pattern = f"pub static {static_name}: VariationDef"
    start_idx = src.find(start_pattern)
    if start_idx == -1:
        print(f"  WARN: no static {static_name}")
        continue
    next_static = src.find("\npub static ", start_idx + 1)
    end_idx = next_static if next_static != -1 else len(src)
    block = src[start_idx:end_idx]

    pdef_pat = re.compile(
        r'(VariationParamDef\s*\{[^}]*?name:\s*"' + re.escape(param_name) + r'"[^}]*?)description:\s*None\s*\}',
        re.DOTALL,
    )
    new_block, n = pdef_pat.subn(
        lambda m: m.group(1) + f'description: Some("{desc}") }}',
        block, count=1,
    )

    if n == 0:
        already_pat = re.compile(
            r'VariationParamDef\s*\{[^}]*?name:\s*"' + re.escape(param_name) + r'"[^}]*?description:\s*Some\(',
            re.DOTALL,
        )
        if already_pat.search(block):
            palready += 1
        else:
            print(f"  WARN: no param {static_name}.{param_name}")
        continue

    src = src[:start_idx] + new_block + src[end_idx:]
    pinserted += 1
print(f"  pass2: injected {pinserted}, already {palready}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
