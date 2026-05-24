"""Apply doc-comments + per-param descriptions to pre_phase.rs and
post_phase.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    # pre_phase.rs
    "PRE_ZSCALE": "Scales the Z coordinate before the rest of the variations run. The variation's weight is the scale factor — weight 2.0 doubles depth, 0.5 halves it.",
    "PRE_ZTRANSLATE": "Shifts the Z coordinate up or down before the rest of the variations run. The variation's weight is the offset.",
    "PRE_SPHERICAL": "Same math as Spherical (inverts through the unit circle) but runs before the rest of the variations instead of contributing to the weighted sum.",
    "PRE_SINUSOIDAL": "Same math as Sinusoidal (sine on each axis) but runs before the rest of the variations. The variation's weight scales the output.",
    "PRE_DISC": "Same math as Disc (wraps the plane onto a disc) but runs before the rest of the variations. The variation's weight scales the result.",
    "PRE_BWRAPS": "Wraps the plane into a grid of soft bubbles, each with its own internal twist. Runs before the rest of the variations so the rest see the bubbled coordinates.",
    "PRE_CROP": "Constrains points to a rectangle before the rest of the variations run. Points outside the rectangle either collapse to zero or get scattered along the nearest edge.",
    "PRE_FALLOFF2": "Adds random scatter that varies with distance from a chosen center point. Closer points get less scatter (or more, with `invert`); the random distribution shape is selectable. Runs before the rest of the variations.",
    # post_phase.rs
    "POST_BWRAPS": "Same as Pre Bwraps but runs after all other variations — the bubble grid is applied to the final output coordinates.",
    "POST_CROP": "Same as Pre Crop but runs after all other variations — the rectangle constraint is applied to the final output coordinates.",
    "POST_FALLOFF2": "Same as Pre Falloff2 but runs after all other variations — the distance-based scatter is applied to the final output coordinates.",
    "POST_CURL": "Same math as Curl (complex polynomial twist) but runs after all other variations, distorting the final output coordinates.",
    "POST_CURL3D": "3D version of Post Curl — applies a complex polynomial twist along all three axes after all other variations have run. Each axis has its own twist coefficient.",
}

PARAM_DOC = {
    # bwraps (same params for pre/post)
    ("PRE_BWRAPS", "cellsize"): "Width of each grid cell — the plane is divided into cells of this size, each becoming a bubble.",
    ("PRE_BWRAPS", "space"): "Gap between cells. 0 = no gap; positive values push the bubbles apart.",
    ("PRE_BWRAPS", "gain"): "How strongly each bubble wraps its contents inward.",
    ("PRE_BWRAPS", "inner_twist"): "Rotation (in degrees) applied at the center of each bubble.",
    ("PRE_BWRAPS", "outer_twist"): "Rotation (in degrees) applied at the edge of each bubble.",
    ("POST_BWRAPS", "cellsize"): "Width of each grid cell — the plane is divided into cells of this size, each becoming a bubble.",
    ("POST_BWRAPS", "space"): "Gap between cells. 0 = no gap; positive values push the bubbles apart.",
    ("POST_BWRAPS", "gain"): "How strongly each bubble wraps its contents inward.",
    ("POST_BWRAPS", "inner_twist"): "Rotation (in degrees) applied at the center of each bubble.",
    ("POST_BWRAPS", "outer_twist"): "Rotation (in degrees) applied at the edge of each bubble.",

    # crop (same params for pre/post)
    ("PRE_CROP", "left"): "Left edge of the rectangle the points are constrained to.",
    ("PRE_CROP", "top"): "Top edge of the rectangle.",
    ("PRE_CROP", "right"): "Right edge of the rectangle.",
    ("PRE_CROP", "bottom"): "Bottom edge of the rectangle.",
    ("PRE_CROP", "scatter_area"): "Width of the random scatter band along the rectangle's edges. 0 = points snap exactly to the edge.",
    ("PRE_CROP", "zero"): "When on, points outside the rectangle collapse to the origin. When off, they scatter back to the nearest edge.",
    ("POST_CROP", "left"): "Left edge of the rectangle the points are constrained to.",
    ("POST_CROP", "top"): "Top edge of the rectangle.",
    ("POST_CROP", "right"): "Right edge of the rectangle.",
    ("POST_CROP", "bottom"): "Bottom edge of the rectangle.",
    ("POST_CROP", "scatter_area"): "Width of the random scatter band along the rectangle's edges. 0 = points snap exactly to the edge.",
    ("POST_CROP", "zero"): "When on, points outside the rectangle collapse to the origin. When off, they scatter back to the nearest edge.",

    # falloff2 (same params for pre/post)
    ("PRE_FALLOFF2", "scatter"): "Maximum random scatter applied at full strength.",
    ("PRE_FALLOFF2", "mindist"): "Distance from the center where the falloff kicks in. Points inside this radius get full strength scatter.",
    ("PRE_FALLOFF2", "mul_x"): "How strongly the scatter affects the X axis (0 = ignore, 1 = full).",
    ("PRE_FALLOFF2", "mul_y"): "How strongly the scatter affects the Y axis (0 = ignore, 1 = full).",
    ("PRE_FALLOFF2", "mul_z"): "How strongly the scatter affects the Z axis (0 = ignore, 1 = full). 3D mode only.",
    ("PRE_FALLOFF2", "mul_c"): "Color-channel scatter strength. Currently unused — direct color writing is not wired up for this variation.",
    ("PRE_FALLOFF2", "x0"): "X coordinate of the falloff center.",
    ("PRE_FALLOFF2", "y0"): "Y coordinate of the falloff center.",
    ("PRE_FALLOFF2", "z0"): "Z coordinate of the falloff center.",
    ("PRE_FALLOFF2", "invert"): "When on, flips the falloff direction — full scatter applies far from the center, nothing near it.",
    ("PRE_FALLOFF2", "type"): "Random distribution shape. 0 = uniform, 1 = triangular (smoother), 2 = gaussian (concentrated near zero).",
    ("POST_FALLOFF2", "scatter"): "Maximum random scatter applied at full strength.",
    ("POST_FALLOFF2", "mindist"): "Distance from the center where the falloff kicks in. Points inside this radius get full strength scatter.",
    ("POST_FALLOFF2", "mul_x"): "How strongly the scatter affects the X axis (0 = ignore, 1 = full).",
    ("POST_FALLOFF2", "mul_y"): "How strongly the scatter affects the Y axis (0 = ignore, 1 = full).",
    ("POST_FALLOFF2", "mul_z"): "How strongly the scatter affects the Z axis (0 = ignore, 1 = full). 3D mode only.",
    ("POST_FALLOFF2", "mul_c"): "Color-channel scatter strength. Currently unused — direct color writing is not wired up for this variation.",
    ("POST_FALLOFF2", "x0"): "X coordinate of the falloff center.",
    ("POST_FALLOFF2", "y0"): "Y coordinate of the falloff center.",
    ("POST_FALLOFF2", "z0"): "Z coordinate of the falloff center.",
    ("POST_FALLOFF2", "invert"): "When on, flips the falloff direction — full scatter applies far from the center, nothing near it.",
    ("POST_FALLOFF2", "type"): "Random distribution shape. 0 = uniform, 1 = triangular (smoother), 2 = gaussian (concentrated near zero).",

    # post_curl
    ("POST_CURL", "c1"): "Linear twist strength. Stronger = tighter curl around the center.",
    ("POST_CURL", "c2"): "Quadratic twist strength. Adds a second-order curl that grows away from the origin.",

    # post_curl3d
    ("POST_CURL3D", "cx"): "Twist strength along the X axis.",
    ("POST_CURL3D", "cy"): "Twist strength along the Y axis.",
    ("POST_CURL3D", "cz"): "Twist strength along the Z axis.",
}

# Group target statics by file so the script knows where to look
FILES = {
    "src/variations/defs/pre_phase.rs": [
        "PRE_ZSCALE", "PRE_ZTRANSLATE", "PRE_SPHERICAL", "PRE_SINUSOIDAL",
        "PRE_DISC", "PRE_BWRAPS", "PRE_CROP", "PRE_FALLOFF2",
    ],
    "src/variations/defs/post_phase.rs": [
        "POST_BWRAPS", "POST_CROP", "POST_FALLOFF2", "POST_CURL", "POST_CURL3D",
    ],
}

for path, statics in FILES.items():
    with open(path, "rb") as f:
        src = f.read().decode("utf-8")

    inserted = 0
    already = 0
    for name in statics:
        target = f"\npub static {name}: VariationDef = VariationDef {{"
        if target not in src:
            print(f"  WARN: no match for {name} in {path}")
            continue
        idx = src.find(target)
        prefix = src[:idx]
        last_nl = prefix.rfind("\n")
        if last_nl != -1:
            prev_line = src[last_nl + 1:idx + 1].strip()
            if prev_line.startswith("///"):
                already += 1
                continue
        body = DOC[name]
        lines = []
        for paragraph in body.split("\n"):
            wrapped = textwrap.fill(paragraph, width=72) if paragraph.strip() else ""
            for line in wrapped.split("\n"):
                lines.append(f"/// {line}".rstrip())
        doc = "\n".join(lines)
        src = src.replace(target, f"\n{doc}\npub static {name}: VariationDef = VariationDef {{", 1)
        inserted += 1
    print(f"  {path}: inserted {inserted} doc-comments, already {already}")

    # PASS 2: per-param descriptions
    pinserted = 0
    palready = 0
    for (static_name, param_name), desc in PARAM_DOC.items():
        if static_name not in statics:
            continue
        start_pattern = f"pub static {static_name}: VariationDef"
        start_idx = src.find(start_pattern)
        if start_idx == -1:
            print(f"  WARN: no static {static_name}")
            continue
        next_static = src.find("\npub static ", start_idx + 1)
        end_idx = next_static if next_static != -1 else len(src)
        block = src[start_idx:end_idx]

        # Two flavors: macro-form `param!("name", ...)` and longhand
        # `VariationParamDef { name: "...", ... }`. Handle longhand by
        # replacing `description: None,`; handle macro by appending the
        # description arg before the closing `)`.

        # Longhand pattern first
        long_pat = re.compile(
            r'(VariationParamDef\s*\{[^}]*?name:\s*"' + re.escape(param_name) + r'"[^}]*?)description:\s*None,',
            re.DOTALL,
        )
        new_block, n = long_pat.subn(
            lambda m: m.group(1) + f'description: Some("{desc}"),',
            block, count=1,
        )

        if n == 0:
            # Try macro form: param!("name", "Display", typeword, default, ...)
            # We need to inject the description as the last positional argument
            # before the closing `)`. The param! macro has overload arms that
            # accept a trailing $desc:expr, so just append it.
            # Two macro shapes:
            #   param!("n", "D", typeword, default)             -> 4 args
            #   param!("n", "D", typeword, default, min, max)   -> 6 args
            #   bool/angle use the 4-arg form.
            # Match the whole `param!(...)` call for our param name.
            macro_pat = re.compile(
                r'(param!\(\s*"' + re.escape(param_name) + r'"\s*,[^)]*?)(\))',
                re.DOTALL,
            )
            new_block, n2 = macro_pat.subn(
                lambda m: m.group(1) + f', "{desc}"' + m.group(2),
                block, count=1,
            )
            if n2 == 0:
                # Try already-injected detection
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
    print(f"  {path}: injected {pinserted} param descriptions, already {palready}")

    with open(path, "wb") as f:
        f.write(src.encode("utf-8"))
