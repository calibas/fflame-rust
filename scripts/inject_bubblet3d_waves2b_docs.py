"""Apply doc-comments + per-param descriptions to bubblet3d_misc.rs and waves2b_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "BUBBLE_T3D": (
        "bubblet3d_misc.rs",
        "3D bubble inversion with stripes and Z-axis hole — first does a bubble (sphere) inversion of `(x, y)` (`/= (x²+y²)/4 + 1`), optionally erases or rotates wedge-shaped stripes around the XY plane (count via `number_of_stripes`, width-ratio via `ratio_of_stripes`), and finally erases or blends a cap on the Z axis based on `angle_of_hole` (with `exponent_z` controlling the Z bubble shape). `modus_blur = 0` does hard cutouts; `modus_blur = 1` does a smooth angular blend (only effective for the Z hole when `exponent_z == 1`). Negative `number_of_stripes` or `angle_of_hole` inverts which side of the boundary is drawn vs blanked.",
        ["FractalDesire"],
    ),
    "WAVES2B": (
        "waves2b_misc.rs",
        "Per-axis sine-perturbation displacement (generalization of `waves2`) — adds a wave term to each axis driven by the *other* axis's coordinate. The wave function is chosen by the per-axis power param: `pw > 1e-4` or `pw < -1e-4` → power-mode sine `sin(sgn(c)·|c|^pw·freq)`; `pw ∈ [0, 1e-4)` → Jacobi `sn` (with modulus `jacok`); `pw ∈ (-1e-4, 0)` → Bessel `J1`. Per-axis amplitude is a smooth interpolation between `scale*` (at origin) and `scaleinf*` (at infinity), controlled by `unity`.",
        ["DarkBeam"],
    ),
}

PARAM_DOC = {
    ("BUBBLE_T3D", "number_of_stripes"): "Number of angular stripe sectors around the XY plane. 0 disables stripes entirely. Negative value inverts the stripe pattern (swap drawn vs blanked sectors).",
    ("BUBBLE_T3D", "ratio_of_stripes"): "Width fraction of the drawn stripe within each sector, clamped to [0.01, 1.99]. 1.0 = stripes and gaps are equal width.",
    ("BUBBLE_T3D", "angle_of_hole"): "Half-angle of the Z-axis hole in degrees. Negative value inverts which side of the hole is drawn vs blanked.",
    ("BUBBLE_T3D", "exponent_z"): "Z-axis bubble shape exponent. 1.0 = standard inversion; smaller values flatten the bubble, larger values elongate it.",
    ("BUBBLE_T3D", "symmetry_z"): "When 1, mirror the Z hole symmetrically across the XY plane (clamps `|angle_of_hole|` to < 180°). When 0, apply the hole only to one Z-pole.",
    ("BUBBLE_T3D", "modus_blur"): "0 = hard cutout (zero out points inside stripes/hole). 1 = smooth blend via angular rotation (only effective for the Z hole when `exponent_z == 1`; otherwise falls back to hard cutout).",

    ("WAVES2B", "freqx"): "Frequency of the wave applied to the X axis (driven by Y).",
    ("WAVES2B", "freqy"): "Frequency of the wave applied to the Y axis (driven by X).",
    ("WAVES2B", "pwx"): "X-axis power exponent — also selects the wave function. `> 1e-4` or `< -1e-4` = power-mode sine of `|c|^pw`; `[0, 1e-4)` = Jacobi `sn`; `(-1e-4, 0)` = Bessel `J1`.",
    ("WAVES2B", "pwy"): "Y-axis power exponent — same threshold-based mode selector as `pwx`.",
    ("WAVES2B", "scalex"): "X-axis amplitude at the origin (full damping).",
    ("WAVES2B", "scaleinfx"): "X-axis amplitude at infinity (the limiting amplitude as `x → ±∞`).",
    ("WAVES2B", "scaley"): "Y-axis amplitude at the origin.",
    ("WAVES2B", "scaleinfy"): "Y-axis amplitude at infinity.",
    ("WAVES2B", "unity"): "Crossover scale for the amplitude interpolation between `scale*` and `scaleinf*`. Larger values keep the origin-amplitude active over a wider region.",
    ("WAVES2B", "jacok"): "Modulus `k` for the Jacobi `sn` mode (only used when `pwx` or `pwy` falls in `[0, 1e-4)`).",
}


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
