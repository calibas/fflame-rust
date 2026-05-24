"""Apply doc-comments + per-param descriptions to extended.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

# (description, optional list of author lines)
DOC = {
    "ZTRANSLATE": (
        "Shifts the Z coordinate during the variation pass. The variation's weight is the offset — set the weight to control how far each point moves up or down.",
        None,
    ),
    "JULIA3D": (
        "3D version of Julia — splits the output into `power` randomly-chosen branches in both XY and Z. Generates intricate 3D Julia-set fractals.",
        ["Joel Faber"],
    ),
    "FALLOFF2": (
        "Adds random scatter that varies with distance from a chosen center point. Closer points get less scatter (or more, with `invert`); the random distribution shape is selectable.",
        None,
    ),
    "WEDGE": (
        "Slices the plane into N pie wedges, each compressed and offset by the chosen angle. Adds an optional swirl that increases with distance and a `hole` radial offset.",
        None,
    ),
    "EPISPIRAL": (
        "Maps the plane onto an epicycloid spiral pattern. `n` controls how many lobes, `thickness` adds randomness, `holes` punches gaps in the pattern.",
        None,
    ),
    "BWRAPS": (
        "Wraps the plane into a grid of soft bubbles, each with its own internal twist. Same shape as Pre Bwraps and Post Bwraps but applied in the normal weighted-sum phase.",
        None,
    ),
    "JULIASCOPE": (
        "Kaleidoscope variant of Julia — splits the angle into `power` branches with random sign-flipping, producing symmetric mirror-like patterns.",
        None,
    ),
    "JULIA3DZ": (
        "3D variant of Julia where the Z coordinate gets folded along with the XY. Produces 3D fractals stretched along the depth axis.",
        None,
    ),
    "CURL3D": (
        "3D version of Curl — applies a complex polynomial twist along all three axes. Each axis has its own twist coefficient.",
        None,
    ),
    "RADIAL_BLUR": (
        "Adds randomness in both rotation (spin) and scale (zoom) around the origin. The angle slider controls the mix — 0 degrees = pure zoom, 90 degrees = pure spin, 45 degrees = balanced.",
        None,
    ),
    "BLUR_CIRCLE": (
        "Replaces the input with a uniformly random point inside the unit circle. Like Blur, but with a sharp circular boundary instead of a soft gradient.",
        None,
    ),
    "BLUR_ZOOM": (
        "Random zoom blur radiating from a chosen center point. The `length` slider controls how far points get pushed; `x` and `y` set the center.",
        None,
    ),
    "BLUR_PIXELIZE": (
        "Snaps points to a grid of pixel-sized cells, then adds random offset within each cell. Produces a mosaic effect.",
        None,
    ),
    "SEPARATION": (
        "Pushes points away from the X and Y axes by configurable amounts, with separate inside/outside offsets. Creates a split, mirrored look.",
        None,
    ),
    "MOBIUS": (
        "Möbius transformation in the complex plane — `(Az + B) / (Cz + D)`. The eight real/imaginary coefficients (A, B, C, D) control the conformal warping; classic hyperbolic-geometry effect.",
        None,
    ),
    "CROP": (
        "Constrains points to a rectangle. Points outside either collapse to zero or get scattered along the nearest edge, depending on `zero`.",
        None,
    ),
}

PARAM_DOC = {
    ("JULIA3D", "power"): "Number of branches in the 3D Julia output. Higher = more arms; negative values flip the rotation.",

    ("FALLOFF2", "scatter"): "Maximum random scatter applied at full strength.",
    ("FALLOFF2", "mindist"): "Distance from the center where the falloff kicks in. Points inside this radius get full strength scatter.",
    ("FALLOFF2", "mul_x"): "How strongly the scatter affects the X axis (0 = ignore, 1 = full).",
    ("FALLOFF2", "mul_y"): "How strongly the scatter affects the Y axis (0 = ignore, 1 = full).",
    ("FALLOFF2", "mul_z"): "How strongly the scatter affects the Z axis (0 = ignore, 1 = full). 3D mode only.",
    ("FALLOFF2", "mul_c"): "Color-channel scatter strength. Currently unused — direct color writing is not wired up for this variation.",
    ("FALLOFF2", "x0"): "X coordinate of the falloff center.",
    ("FALLOFF2", "y0"): "Y coordinate of the falloff center.",
    ("FALLOFF2", "z0"): "Z coordinate of the falloff center.",
    ("FALLOFF2", "invert"): "When on, flips the falloff direction — full scatter applies far from the center, nothing near it.",
    ("FALLOFF2", "type"): "Random distribution shape. 0 = uniform, 1 = triangular (smoother), 2 = gaussian (concentrated near zero).",

    ("WEDGE", "angle"): "Wedge angle in degrees — how wide each pie slice is before compression.",
    ("WEDGE", "hole"): "Radial offset added to the output. Positive pushes the pattern outward, negative pulls it inward.",
    ("WEDGE", "count"): "Number of pie wedges arranged around the center.",
    ("WEDGE", "swirl"): "Extra rotation that grows with distance. 0 = no swirl, positive = curves arms outward.",

    ("EPISPIRAL", "n"): "Number of lobes in the spiral pattern.",
    ("EPISPIRAL", "thickness"): "Random thickness of each lobe. 0 = razor-thin curves, higher = wider bands.",
    ("EPISPIRAL", "holes"): "Radial offset that punches gaps in the pattern.",

    ("BWRAPS", "cellsize"): "Width of each grid cell — the plane is divided into cells of this size, each becoming a bubble.",
    ("BWRAPS", "space"): "Gap between cells. 0 = no gap; positive values push the bubbles apart.",
    ("BWRAPS", "gain"): "How strongly each bubble wraps its contents inward.",
    ("BWRAPS", "inner_twist"): "Rotation (in degrees) applied at the center of each bubble.",
    ("BWRAPS", "outer_twist"): "Rotation (in degrees) applied at the edge of each bubble.",

    ("JULIASCOPE", "power"): "Number of mirror branches. Higher = more reflections; negative values invert the rotation.",
    ("JULIASCOPE", "dist"): "Radial scaling factor. 1.0 is balanced; larger values push arms outward.",

    ("JULIA3DZ", "power"): "Number of Julia branches in the 3D output. Higher = more arms.",

    ("CURL3D", "cx"): "Twist strength along the X axis.",
    ("CURL3D", "cy"): "Twist strength along the Y axis.",
    ("CURL3D", "cz"): "Twist strength along the Z axis.",

    ("RADIAL_BLUR", "angle"): "Spin/zoom balance. 0 degrees = pure zoom blur, 90 degrees = pure rotational blur, 45 degrees = balanced mix.",

    ("BLUR_ZOOM", "length"): "Maximum zoom distance. Larger values streak points further outward from the center.",
    ("BLUR_ZOOM", "x"): "X coordinate of the zoom center.",
    ("BLUR_ZOOM", "y"): "Y coordinate of the zoom center.",

    ("BLUR_PIXELIZE", "size"): "Pixel cell size. Smaller = finer grid; larger = chunkier pixels.",
    ("BLUR_PIXELIZE", "scale"): "How much each point can jitter within its cell. 0 = points snap to cell centers; 1 = points scatter across the cell.",

    ("SEPARATION", "x"): "How far to push points away from the X axis on either side.",
    ("SEPARATION", "y"): "How far to push points away from the Y axis on either side.",
    ("SEPARATION", "xinside"): "Inside offset along X — adjusts how the separation looks near the axis.",
    ("SEPARATION", "yinside"): "Inside offset along Y — adjusts how the separation looks near the axis.",

    ("MOBIUS", "re_a"): "Real component of complex coefficient A in `(Az + B)/(Cz + D)`.",
    ("MOBIUS", "im_a"): "Imaginary component of complex coefficient A.",
    ("MOBIUS", "re_b"): "Real component of complex coefficient B.",
    ("MOBIUS", "im_b"): "Imaginary component of complex coefficient B.",
    ("MOBIUS", "re_c"): "Real component of complex coefficient C.",
    ("MOBIUS", "im_c"): "Imaginary component of complex coefficient C.",
    ("MOBIUS", "re_d"): "Real component of complex coefficient D.",
    ("MOBIUS", "im_d"): "Imaginary component of complex coefficient D.",

    ("CROP", "left"): "Left edge of the rectangle the points are constrained to.",
    ("CROP", "top"): "Top edge of the rectangle.",
    ("CROP", "right"): "Right edge of the rectangle.",
    ("CROP", "bottom"): "Bottom edge of the rectangle.",
    ("CROP", "scatter_area"): "Width of the random scatter band along the rectangle's edges. 0 = points snap exactly to the edge.",
    ("CROP", "zero"): "When on, points outside the rectangle collapse to the origin. When off, they scatter back to the nearest edge.",
}

PATH = "src/variations/defs/extended.rs"
with open(PATH, "rb") as f:
    src = f.read().decode("utf-8")

# PASS 1: insert doc comments before each `pub static <NAME>:` line. Idempotent.
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

# PASS 2: per-param descriptions (handles both macro and longhand)
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

    # Longhand pattern
    long_pat = re.compile(
        r'(VariationParamDef\s*\{[^}]*?name:\s*"' + re.escape(param_name) + r'"[^}]*?)description:\s*None,',
        re.DOTALL,
    )
    new_block, n = long_pat.subn(
        lambda m: m.group(1) + f'description: Some("{desc}"),',
        block, count=1,
    )

    if n == 0:
        # Macro form: append as trailing arg
        macro_pat = re.compile(
            r'(param!\(\s*"' + re.escape(param_name) + r'"\s*,[^)]*?)(\))',
            re.DOTALL,
        )
        new_block, n2 = macro_pat.subn(
            lambda m: m.group(1) + f', "{desc}"' + m.group(2),
            block, count=1,
        )
        if n2 == 0:
            already_long = re.compile(
                r'VariationParamDef\s*\{[^}]*?name:\s*"' + re.escape(param_name) + r'"[^}]*?description:\s*Some\(',
                re.DOTALL,
            )
            already_macro = re.compile(
                r'param!\(\s*"' + re.escape(param_name) + r'"[^)]*"' + re.escape(desc[:10]),
                re.DOTALL,
            )
            if already_long.search(block) or already_macro.search(block):
                palready += 1
            else:
                print(f"  WARN: no param {static_name}.{param_name}")
            continue

    src = src[:start_idx] + new_block + src[end_idx:]
    pinserted += 1
print(f"  pass2: injected {pinserted}, already {palready}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
