"""Apply doc-comments + per-param descriptions to hypertile.rs.

One-off script for the variations-bulk-metadata project. Idempotent.
All variations attributed to Zueuk."""
import re
import textwrap

DOC = {
    "HYPERTILE": "Maps the plane onto a {p, q} hyperbolic tiling via Möbius transformation. `n` picks which tile of the tiling is targeted (deterministic).",
    "HYPERTILE1": "Maps the plane onto a {p, q} hyperbolic tiling — random tile per iteration. The rotation picking the tile is applied first.",
    "HYPERTILE2": "Variant of Hypertile1 that applies the rotation last instead of first. Equivalent math up to ordering, but the per-iteration distribution looks visually distinct.",
    "HYPERTILE3D": "3D version of Hypertile — Möbius reflection through a sphere on the unit-disc boundary. `n` picks which tile (deterministic).",
    "HYPERTILE3D1": "3D version of Hypertile1 — random 3D tile per iteration.",
    "HYPERTILE3D2": "3D version of Hypertile2 — tile centered on the real axis, with per-iteration random XY rotation applied after the Möbius warp.",
}

# Shared param descriptions across the family.
COMMON_PARAM_DOC = {
    "p": "First Schläfli symbol — number of sides per tile (3 = triangle, 4 = square, 5 = pentagon, etc.).",
    "q": "Second Schläfli symbol — number of tiles meeting at each vertex. For a valid hyperbolic tiling, `(p − 2)(q − 2) > 4`.",
    "n": "Index of which tile to target — deterministic tile selector.",
}

# Build PARAM_DOC per variation.
PARAM_DOC = {}
ALL_VARIATIONS = list(DOC.keys())
for static_name in ALL_VARIATIONS:
    for param_name, desc in COMMON_PARAM_DOC.items():
        PARAM_DOC[(static_name, param_name)] = desc

PATH = "src/variations/defs/hypertile.rs"
with open(PATH, "rb") as f:
    src = f.read().decode("utf-8")

inserted = 0
already = 0
for name, body in DOC.items():
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
    lines.append("///")
    lines.append("/// # Authors")
    lines.append("/// - Zueuk")
    doc = "\n".join(lines)
    src = src.replace(target, f"\n{doc}\npub static {name}: VariationDef = VariationDef {{", 1)
    inserted += 1
print(f"  pass1: inserted {inserted}, already {already}")

# Macro-form per-param descriptions (all hypertile use param!(...))
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

    macro_pat = re.compile(
        r'(param!\(\s*"' + re.escape(param_name) + r'"\s*,[^)]*?)(\))',
        re.DOTALL,
    )
    new_block, n = macro_pat.subn(
        lambda m: m.group(1) + f', "{desc}"' + m.group(2),
        block, count=1,
    )
    if n == 0:
        # Param may not exist on this variation (e.g. `n` on hypertile1/2)
        # or already done. Check for already-done; else silently skip.
        already_pat = re.compile(
            r'param!\(\s*"' + re.escape(param_name) + r'"[^)]*"' + re.escape(desc[:10]),
            re.DOTALL,
        )
        if already_pat.search(block):
            palready += 1
        continue
    src = src[:start_idx] + new_block + src[end_idx:]
    pinserted += 1
print(f"  pass2: injected {pinserted}, already {palready}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
