"""Apply doc-comments + per-param descriptions to hexaplay3d_misc.rs and hexnix3d_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "HEXAPLAY_3D": (
        "hexaplay3d_misc.rs",
        "Hexagonal vertex sequencer for snowflake designs. Places successive iterations at the 6 vertices of a unit hexagon (when internal state `rswtch ≤ 1`) or the 3 vertices of an inscribed triangle (when `rswtch > 1`), cycling through them with internal `fcycle`/`bcycle` counters and re-randomizing `rswtch ∈ {0, 1, 2}` each time a cycle completes. `majp` controls Z-plane behavior: `|majp| ≤ 1` puts all points on a single Z plane; `|majp| > 1` splits into two planes separated by `±(|majp| - 1) · 0.5` along Z, sign picked randomly per iteration. Uses `needs_accum + needs_transform` to implement cpp's replacement-style FPx/FPy via the `(desired - accum) / weight` workaround.",
        ["Larry Berlin"],
    ),
    "HEXNIX_3D": (
        "hexnix3d_misc.rs",
        "Animation-friendly sister of `hexaplay3D`. Adds: a `smooth` factor that fades between pass-through and full hex effect when `|weight| ≤ 0.5`; three majplane modes (`|majp| ≤ 1` single plate / `1 < |majp| < 2` transition / `|majp| ≥ 2` split planes with boost `(|majp| - 2) · 0.5`) with extra negative-`majp` branches that flip Z for animation transitions; a `3side` parameter that scales the inscribed triangle vertices independently; randomized (rather than sequential) vertex selection to reduce stepping artifacts; and a rotation-and-blend X/Y formula (rather than hexaplay's translate-to-vertex). Vertex layouts also differ — `seg60` y-signs mirrored and `seg120` rotated 30°.",
        ["Larry Berlin"],
    ),
}

PARAM_DOC = {
    # hexaplay3D
    ("HEXAPLAY_3D", "majp"): "Major-plane threshold for Z behavior. `|majp| ≤ 1` = all points on a single Z plane (no Z split). `|majp| > 1` = points split into two planes separated by `±(|majp| - 1) · 0.5` along Z; sign picked randomly per iteration. Unused in 2D mode (Z param).",
    ("HEXAPLAY_3D", "scale"): "Input-blend scale. Internally pre-multiplied by 0.5; the X/Y output is `(accum · (scale - 1) + p · scale) / weight + vertex_offset`.",
    ("HEXAPLAY_3D", "zlift"): "Z input scale: `oz = p.z · 0.5 · zlift / weight`. Unused in 2D mode.",

    # hexnix3D
    ("HEXNIX_3D", "majp"): "Major-plane threshold for Z behavior (3 modes based on `|majp|`): ≤ 1 = single plate (Z from `zlift` only); 1 < |majp| < 2 = transition mode (with extra negative-`majp` Z-flip branches for animation); ≥ 2 = split planes with boost = `(|majp| - 2) · 0.5`. Negative `majp` adds Z-flip branches in modes 1 and 2 that mirror the accumulator across Z=0 for animation transitions. Unused in 2D mode.",
    ("HEXNIX_3D", "scale"): "Input-blend scale for the rotation-and-blend X/Y formula. The body mixes `(accum + p)` rotated by the chosen vertex angle with the vertex itself, scaled by `scale`.",
    ("HEXNIX_3D", "zlift"): "Z input scale. Combined with `scale` and `p.z` to set the baseline Z output. Unused in 2D mode.",
    ("HEXNIX_3D", "3side"): "Inscribed-triangle vertex scale. Multiplies the 3-vertex (triangle) branch's `weight · vertex` term, letting the triangle and hexagon contributions be scaled independently. Hexagon branch ignores this param.",
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
