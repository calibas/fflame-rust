"""Apply doc-comments + per-param descriptions to curliecue2_misc.rs and farblur_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "CURLIECUE2": (
        "curliecue2_misc.rs",
        "Self-iterating Curlicue walker — ignores the input position and emits a deterministic point sequence from a 4-slot per-thread state machine `(x, y, θ, φ)`. Each iteration advances `(x, y)` by `0.001 · (cos φ, sin φ)`, then updates `φ ← (θ + φ) mod 2π` and `θ ← (θ + 2π·speed) mod 2π`. With `speed` set to an irrational fraction (e.g. golden ratio), the trajectory traces the classical curlicue fractal of Berry and Goldberg. Upstream cpp randomizes `speed` once at flame init via `GOODRAND_01`; we expose it as a user parameter for reproducibility.",
        ["Jesus Sosa"],
    ),
    "FARBLUR": (
        "farblur_misc.rs",
        "Far-distance random blur — emits a spherical-coordinate random offset whose magnitude scales with the squared distance of the running accumulator from a configurable origin, so points further from the origin receive proportionally more blur. The scale factor is modulated by a rolling buffer of 4 random samples (`r[0..3]`), refreshed one slot per iteration; their sum minus 2 gives a slowly-varying signed multiplier. In 2D the missing Z accumulator collapses the `dz` term to a constant `(0 - z_origin)²`, so `z_origin` becomes a constant additive offset on the radial magnitude.",
        ["zephyrtronium"],
    ),
}

PARAM_DOC = {
    ("CURLIECUE2", "speed"): "Angular velocity of the curlicue walker as a fraction of 2π per iteration. Small irrational values (e.g. golden-ratio fractions) produce the classical curlicue fractal pattern; rational values produce closed loops.",

    ("FARBLUR", "x"): "X-axis blur scale. Multiplies the X component of the random spherical offset.",
    ("FARBLUR", "y"): "Y-axis blur scale.",
    ("FARBLUR", "z"): "Z-axis blur scale (3D only).",
    ("FARBLUR", "x_origin"): "Origin X coordinate. The accumulator's distance from this point (squared) modulates the blur magnitude — larger distance → more blur.",
    ("FARBLUR", "y_origin"): "Origin Y coordinate.",
    ("FARBLUR", "z_origin"): "Origin Z coordinate. In 2D mode this becomes a constant offset (the 2D accumulator has no Z, so `dz` reduces to `-z_origin`).",
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
