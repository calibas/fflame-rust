"""Per-param descriptions for wedge_extended.rs.

Variation-level docs handled manually (existing # Authors blocks
prepended). This script just injects PARAM_DOC."""
import re

PARAM_DOC = {
    ("WEDGE_JULIA", "power"): "Number of Julia branches. Higher = more arms.",
    ("WEDGE_JULIA", "dist"): "Radial distance scaling — pushes arms inward or outward.",
    ("WEDGE_JULIA", "count"): "Number of wedge sectors around the center.",
    ("WEDGE_JULIA", "angle"): "Wedge sector rotation, in radians.",

    ("WEDGE_SPH", "angle"): "Wedge sector rotation, in radians.",
    ("WEDGE_SPH", "hole"): "Radial offset added after the inversion. Positive opens a hole at the center; negative compresses inward.",
    ("WEDGE_SPH", "count"): "Number of wedge sectors around the center.",
    ("WEDGE_SPH", "swirl"): "Extra rotation that grows with distance — gives the wedges a spiral.",
}

PATH = "src/variations/defs/wedge_extended.rs"
with open(PATH, "rb") as f:
    src = f.read().decode("utf-8")

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
        already_pat = re.compile(
            r'param!\(\s*"' + re.escape(param_name) + r'"[^)]*"' + re.escape(desc[:10]),
            re.DOTALL,
        )
        if already_pat.search(block):
            palready += 1
        else:
            print(f"  WARN: no param {static_name}.{param_name}")
        continue
    src = src[:start_idx] + new_block + src[end_idx:]
    pinserted += 1
print(f"  injected {pinserted}, already {palready}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
