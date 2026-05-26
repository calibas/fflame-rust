"""Apply doc-comments + per-param descriptions to subflame.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "SUBFLAME_WF": (
        "Subflame — nested chaos-game variation. Owns a complete inner flame definition (referenced by `subflame_id` into `FractalConfig.subflames`); during each step of the parent flame's chaos game it advances a *nested* chaos game by one iteration on the subflame's IFS and uses the resulting point as the variation's output. Classified as a **blur** variation in JWildfire: it ignores both the input `p` (the parent chaos-game state) and the variation `amount` — users scale/rotate the subflame via the parent xform's post-affine instead, though `scale`/`angle`/`offset_*` are also kept here for round-trip fidelity with existing JWildfire / Apophysis flame files. Per-thread state (5 slots) carries the subflame's chaos-game point, current xform index, and color scalar across iterations. The nested chaos-game step is implemented in [`shaders/core/subflame.wgsl`](../../shaders/core/subflame.wgsl), injected by the shader builder when `subflame_wf` is active.",
        ["Andreas Maschke"],
    ),
}

PARAM_DOC = {
    ("SUBFLAME_WF", "subflame_id"): "Index into `FractalConfig.subflames` (0..MAX_SUBFLAMES-1). Selects which inner flame definition this variation iterates. Not really an enum — `MAX_SUBFLAMES` is a config-time constant in `gpu/buffers.rs` and the param range tracks it.",
    ("SUBFLAME_WF", "scale"): "Scale factor applied to the subflame's per-step XY output (and Z output in 3D mode) before adding to the parent's accumulator. JWildfire's spec recommends using the parent xform's post-affine for this instead; the param is preserved for round-trip fidelity with files that set it.",
    ("SUBFLAME_WF", "angle"): "Rotation angle (degrees) applied to the subflame's XY output before adding to the parent's accumulator. Same round-trip-fidelity note as `scale`.",
    ("SUBFLAME_WF", "offset_x"): "X translation added to the subflame's output after scale + rotation.",
    ("SUBFLAME_WF", "offset_y"): "Y translation.",
    ("SUBFLAME_WF", "offset_z"): "Z translation (3D mode only — added to the subflame's Z after `scale · q.z` and `colorscale_z · q.w`).",
    ("SUBFLAME_WF", "colorscale_z"): "Multiplier applied to the subflame's color scalar (0..1) and added to the Z output. JWildfire's `colorscale_wf`-style depth-from-color mechanism — non-zero values let the subflame's color drive a Z offset for pseudo-3D effects in 2D-classified subflames. 3D mode only.",
    ("SUBFLAME_WF", "color_mode"): "How the subflame's color scalar interacts with the parent's color register. -1 = Off (default; leave parent's color alone), 0 = Direct (overwrite parent's vc with subflame's color). Modes 1-4 are JWildfire's CM_RED/GREEN/BLUE/BRIGHTNESS — declared in the param range but currently silently no-op'd (treated as Off); v1 only implements Off and Direct.",
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


PATH = "src/variations/defs/subflame.rs"
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
