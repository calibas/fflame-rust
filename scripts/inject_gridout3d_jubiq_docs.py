"""Apply doc-comments + per-param descriptions to gridout3d_misc.rs and jubiq_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "GRIDOUT_3D": (
        "gridout3d_misc.rs",
        (
            "Eight-region grid offset — partitions the XY plane into 8 wedges based on `round(p.x)·xx` and `round(p.y)·yy` (the rounding turns the param space into a coarse grid), then adds or subtracts a per-region offset triplet to `(x, y, z)`. The 8 regions split each quadrant along its diagonal; for `y ≤ 0` (regions A-D) the Z offset is added, for `y > 0` (regions E-H) the Z offset is multiplied into Z instead.\n\n"
            "Region table (input X/Y signs after gate-rounding by `xx`/`yy`):\n\n"
            "| Region | Condition (`y`, `x` are gate-rounded) | X op | Y op | Z op |\n"
            "|---|---|---|---|---|\n"
            "| A | `y ≤ 0`, `x > 0`, `-y ≥ x`  | `+xa` | `+ya` | `+za` |\n"
            "| B | `y ≤ 0`, `x > 0`, `-y < x`  | `+xb` | `+yb` | `+zb` |\n"
            "| C | `y ≤ 0`, `x ≤ 0`, `y ≤ x`   | `+xc` | `+yc` | `+zc` |\n"
            "| D | `y ≤ 0`, `x ≤ 0`, `y > x`   | `+xd` | `-yd` | `+zd` |\n"
            "| E | `y > 0`, `x > 0`, `y ≥ x`   | `-xe` | `+ye` | `·ze` |\n"
            "| F | `y > 0`, `x > 0`, `y < x`   | `+xf` | `+yf` | `·zf` |\n"
            "| G | `y > 0`, `x ≤ 0`, `y ≥ -x`  | `-xg` | `+yg` | `·zg` |\n"
            "| H | `y > 0`, `x ≤ 0`, `y < -x`  | `+xh` | `-yh` | `·zh` |\n\n"
            "Sign flips on `-yd`, `-xe`, `-xg`, `-yh` and the add-vs-multiply Z split between A-D and E-H are preserved verbatim from upstream."
        ),
        ["Michael Faber", "Joel Faber", "DarkBeam", "Brad Stefanov"],
    ),
    "JUBIQ": (
        "jubiq_misc.rs",
        "Hybrid of julian2 and Mobiq — first applies a quaternion Möbius transform `M(q) = (A·q + B) / (C·q + D)` in 4D (with `q = (FTx, FTy, FTz, 0)`), then a `julian`-style angular pick (one of `|power|` equally-spaced angles, scaled by `dist`) and radial scaling, then wraps the XY output with an affine (`a..f`). In 3D, the Z output adds the quaternion Z component to a preserve-Z radial inversion term derived from `1 / (r3d^1.5)`. Mathematically a chimera; visually a way to combine the quaternion-Möbius distortion of `mobiq` with the radial-symmetry blow-up of `julian2`.",
        ["Brad Stefanov"],
    ),
}

# --- gridout3D region helpers ---
REGIONS = {
    "a": ("y ≤ 0, x > 0, -y ≥ x", "+", "+", "+"),
    "b": ("y ≤ 0, x > 0, -y < x", "+", "+", "+"),
    "c": ("y ≤ 0, x ≤ 0, y ≤ x",  "+", "+", "+"),
    "d": ("y ≤ 0, x ≤ 0, y > x",  "+", "-", "+"),
    "e": ("y > 0, x > 0, y ≥ x",  "-", "+", "*"),
    "f": ("y > 0, x > 0, y < x",  "+", "+", "*"),
    "g": ("y > 0, x ≤ 0, y ≥ -x", "-", "+", "*"),
    "h": ("y > 0, x ≤ 0, y < -x", "+", "-", "*"),
}

OP_VERB = {
    ("x", "+"): "added to X",
    ("x", "-"): "subtracted from X",
    ("y", "+"): "added to Y",
    ("y", "-"): "subtracted from Y",
    ("z", "+"): "added to Z",
    ("z", "*"): "multiplied into Z (regions E-H multiply; A-D add)",
}

PARAM_DOC = {
    ("GRIDOUT_3D", "xx"): "X gate scale: input `x` is rounded then multiplied by `xx` before the region comparison. Setting to 0 collapses all regions onto `x = 0` (only the y-half decision still applies).",
    ("GRIDOUT_3D", "yy"): "Y gate scale: input `y` is rounded then multiplied by `yy` before the region comparison. Setting to 0 collapses all regions onto `y = 0` (only the x-sign decision still applies).",

    ("JUBIQ", "power"): "Julia angular branch count (integer). 0 collapses the variation to the origin. Otherwise: picks one of `|power|` equally-spaced angles per iteration and divides the polar angle by `power`.",
    ("JUBIQ", "dist"): "Radial exponent multiplier. Combines with `power` to set the radial power as `0.5 · dist / power` (negative `dist` flips the radial sense).",
    ("JUBIQ", "a"): "Output XY affine wrapper, component A (X-to-X scale). Applied to the input `(p.x, p.y)` and added to the Möbius output.",
    ("JUBIQ", "b"): "Output XY affine wrapper, component B (Y-to-X scale).",
    ("JUBIQ", "c"): "Output XY affine wrapper, component C (X-to-Y scale).",
    ("JUBIQ", "d"): "Output XY affine wrapper, component D (Y-to-Y scale).",
    ("JUBIQ", "e"): "Output XY affine wrapper, component E (X translation).",
    ("JUBIQ", "f"): "Output XY affine wrapper, component F (Y translation).",
}

# Generate the 24 gridout3D region params programmatically
for letter, (cond, x_op, y_op, z_op) in REGIONS.items():
    upper = letter.upper()
    PARAM_DOC[("GRIDOUT_3D", f"x{letter}")] = f"Region {upper} ({cond}) X offset, {OP_VERB[('x', x_op)]}."
    PARAM_DOC[("GRIDOUT_3D", f"y{letter}")] = f"Region {upper} ({cond}) Y offset, {OP_VERB[('y', y_op)]}."
    PARAM_DOC[("GRIDOUT_3D", f"z{letter}")] = f"Region {upper} ({cond}) Z offset, {OP_VERB[('z', z_op)]}."

# Generate the 16 jubiq quaternion params programmatically
QUAT_ROLE = {
    "a": "Möbius numerator quaternion A (`M(q) = (A·q + B) / (C·q + D)`)",
    "b": "Möbius numerator quaternion B",
    "c": "Möbius denominator quaternion C",
    "d": "Möbius denominator quaternion D",
}
COMP_ROLE = {
    "t": "scalar (t)",
    "x": "X",
    "y": "Y",
    "z": "Z",
}
for q_letter, q_role in QUAT_ROLE.items():
    for comp, comp_role in COMP_ROLE.items():
        PARAM_DOC[("JUBIQ", f"q{q_letter}{comp}")] = f"{q_role}, {comp_role} component."


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


files = {}
for static_name, (path_rel, body, authors) in DOC.items():
    files.setdefault(path_rel, {})[static_name] = (body, authors)

for path_rel, doc_map in files.items():
    full_path = f"src/variations/defs/{path_rel}"
    with open(full_path, "rb") as f:
        src = f.read().decode("utf-8")

    inserted = 0
    already = 0
    for name, (body, authors) in doc_map.items():
        target = f"\npub static {name}: VariationDef = VariationDef {{"
        if target not in src:
            print(f"  WARN: no match for {name} in {path_rel}")
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
            if paragraph.startswith("|") or paragraph.strip() == "":
                # Don't word-wrap markdown table rows or blanks
                lines.append(f"/// {paragraph}".rstrip())
            else:
                wrapped = textwrap.fill(paragraph, width=72)
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
    print(f"  {path_rel} pass1: inserted {inserted}, already {already}")

    pinserted = 0
    palready = 0
    for (static_name, param_name), desc in PARAM_DOC.items():
        if static_name not in doc_map:
            continue
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
    print(f"  {path_rel} pass2: injected {pinserted}, already {palready}")

    with open(full_path, "wb") as f:
        f.write(src.encode("utf-8"))
