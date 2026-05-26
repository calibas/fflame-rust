"""Apply doc-comments + per-param descriptions to init_ports.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "TARGET": (
        "Alternates two angular rotations on concentric log-spaced rings — points in 'even' rings rotate by `even`, points in 'odd' rings by `odd`. Creates a target-like pattern of rotation bands.",
        ["Michael Faber"],
    ),
    "YIN_YANG": (
        "Yin-yang pattern — inside the unit disc, points get reflected onto the iconic two-droplet curve at the given radius. Optional `dual_t` randomly picks between two rotations per iteration (yin and yang each get their own twist). Outside the disc, points either pass through or get discarded.",
        ["dark-beam"],
    ),
}

PARAM_DOC = {
    ("TARGET", "even"): "Rotation angle (degrees) applied to even-numbered rings.",
    ("TARGET", "odd"): "Rotation angle (degrees) applied to odd-numbered rings.",
    ("TARGET", "size"): "Ring spacing — controls how thick each ring is in log-radius space.",

    ("YIN_YANG", "radius"): "Radius of the two yin/yang droplet centers, 0-1.",
    ("YIN_YANG", "ang1"): "Rotation angle for the first half, in half-turns (multiples of π).",
    ("YIN_YANG", "ang2"): "Rotation angle for the second half, in half-turns. Only used when `dual_t` is on.",
    ("YIN_YANG", "dual_t"): "When on, randomly picks between the two rotations each iteration so yin and yang each get their own twist. When off, always uses `ang1`.",
    ("YIN_YANG", "outside"): "When on, points outside the unit circle pass through unchanged. When off, they're discarded.",
}

PATH = "src/variations/defs/init_ports.rs"
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
