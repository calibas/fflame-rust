"""Apply doc-comments + per-param descriptions to heavy_init.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "CPOW2": (
        "Range-clamped variant of CPow — combines a complex-power Julia (with `re + ai` exponent set via the `r` and `a` parameters) with random branch selection across `range` rotational sectors.",
        ["Zueuk"],
    ),
    "CPOW3": (
        "Logarithm-shifted variant of CPow. Replaces the `i` parameter with `d` (taken through a log transform) and adds a `spread` slider for log-distributed angle perturbation.",
        ["Zueuk"],
    ),
    "DISC2": (
        "Variant of Disc with extra twist and rotation. The `rot` slider scales the wave frequency; `twist` adds rotational drift that amplifies beyond ±2π.",
        None,
    ),
}

PARAM_DOC = {
    ("CPOW2", "r"): "Magnitude of the complex exponent.",
    ("CPOW2", "a"): "Argument (angle) of the complex exponent — multiplied by π/2 internally, so `a = 1` gives a 90° rotation.",
    ("CPOW2", "divisor"): "Number of rotational branches.",
    ("CPOW2", "range"): "Random branch range — each iteration picks an integer in `[0, range)` to shift the angle.",

    ("CPOW3", "r"): "Magnitude of the complex exponent.",
    ("CPOW3", "d"): "Logarithmic argument scaling. Negative values are absorbed via `-log(-d)`.",
    ("CPOW3", "divisor"): "Number of rotational branches.",
    ("CPOW3", "spread"): "Logarithmic spread of the random angle perturbation.",

    ("DISC2", "rot"): "Wave frequency for the disc pattern — multiplied by π internally.",
    ("DISC2", "twist"): "Rotational drift added to the disc output. Beyond ±2π the effect amplifies linearly.",
}

PATH = "src/variations/defs/heavy_init.rs"
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
