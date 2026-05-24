"""Apply doc-comments + per-param descriptions to exp_log.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

# (description, list of author lines)
DOC = {
    "EXP": (
        "Complex exponential — `e^x · (cos(y), sin(y))`. Stretches the plane exponentially along X while wrapping Y onto a unit-circle phase.",
        ["cothe"],
    ),
    "LOG_DB": (
        "Complex log with random period jitter on the imaginary part. Like the basic Log variation but the angle output gets shifted by a random multiple of π (configured via `fix_period`), producing repeating-strip patterns.",
        ["DarkBeam"],
    ),
    "LOG_TILE2": (
        "3D log-tiled spreader — each axis is independently shifted by a random integer drawn from `log(uniform)` rounded to the nearest integer. Produces a stamped tile effect with geometric falloff.",
        ["Zy0rg"],
    ),
    "TILE_LOG": (
        "1D version of Log Tile2 — only shifts along X with the same random log-of-uniform trick. Y (and Z) pass through unchanged.",
        ["Zy0rg"],
    ),
}

PARAM_DOC = {
    ("LOG_DB", "base"): "Logarithm base. Larger values compress the output, smaller values stretch it. Mirrors the basic Log variation's `base`.",
    ("LOG_DB", "fix_period"): "How much random vertical shift gets added each iteration. 0 = no shift; higher = more striping.",
    ("LOG_TILE2", "spreadx"): "X-axis tile spacing. The random integer from the log-of-uniform draw is multiplied by this value.",
    ("LOG_TILE2", "spready"): "Y-axis tile spacing.",
    ("LOG_TILE2", "spreadz"): "Z-axis tile spacing (3D mode only).",
    ("TILE_LOG", "spread"): "Tile spacing along X. The random integer from the log-of-uniform draw is multiplied by this value.",
}

PATH = "src/variations/defs/exp_log.rs"
with open(PATH, "rb") as f:
    src = f.read().decode("utf-8")

# PASS 1: insert doc-comments + # Authors
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
    lines.append("///")
    lines.append("/// # Authors")
    for author in authors:
        lines.append(f"/// - {author}")
    doc = "\n".join(lines)
    src = src.replace(target, f"\n{doc}\npub static {name}: VariationDef = VariationDef {{", 1)
    inserted += 1
print(f"  pass1: inserted {inserted}, already {already}")

# PASS 2: per-param descriptions (longhand-compact form here)
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
