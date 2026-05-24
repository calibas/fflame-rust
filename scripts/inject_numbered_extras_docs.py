"""Apply doc-comments + per-param descriptions to numbered_extras.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "BIPOLAR2": (
        "Bipolar coordinates with 9 user-tunable scaling/offset knobs — much more configurable than the basic Bipolar.",
        ["Apophysis Plugin Pack", "Brad Stefanov"],
    ),
    "BLOB3D": (
        "3D version of Blob — same wavy boundary as Blob, plus a Z component that modulates with the same waves pattern.",
        None,
    ),
    "CIRCULAR2": (
        "Circular with user-tunable hash multipliers — same shape as Circular but exposes the `(12.9898, 78.233)` magic numbers as `xx` / `yy` parameters.",
        ["Tatyana Zabanova", "Brad Stefanov"],
    ),
}

PARAM_DOC = {
    ("BIPOLAR2", "shift"): "Vertical offset added to the bipolar angle output.",
    ("BIPOLAR2", "a"): "Inner offset for the radius² term.",
    ("BIPOLAR2", "b"): "X scaling for the log term's numerator/denominator.",
    ("BIPOLAR2", "c"): "Scaling on the angle output.",
    ("BIPOLAR2", "d"): "Offset inside the atan2 denominator.",
    ("BIPOLAR2", "e"): "Y scaling inside the atan2 numerator.",
    ("BIPOLAR2", "f1"): "Output X scaling factor.",
    ("BIPOLAR2", "g1"): "Outer scaling on the squared radius.",
    ("BIPOLAR2", "h"): "Output Y scaling factor.",

    ("BLOB3D", "low"): "Inner radius — how close the bumps recede in the troughs.",
    ("BLOB3D", "high"): "Outer radius — how far the bumps reach at their peaks.",
    ("BLOB3D", "waves"): "Number of bumps around the perimeter.",

    ("CIRCULAR2", "angle"): "Maximum rotation per iteration (degrees).",
    ("CIRCULAR2", "seed"): "Random seed for the hash term — change to vary the pattern.",
    ("CIRCULAR2", "xx"): "X-axis multiplier for the hash. Default 12.9898 matches the standard Circular.",
    ("CIRCULAR2", "yy"): "Y-axis multiplier for the hash. Default 78.233 matches the standard Circular.",
}

PATH = "src/variations/defs/numbered_extras.rs"
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

# Macro form
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
print(f"  pass2: injected {pinserted}, already {palready}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
