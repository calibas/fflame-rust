"""Apply doc-comments + per-param descriptions to trig_bs.rs.

One-off script for the variations-bulk-metadata project. Idempotent.
All variations: original math by cothe, parameterization by Brad
Stefanov."""
import re
import textwrap

# (description, list of author lines)
DOC = {
    "SIN2_BS":  ("Parameterized Sin — independent scaling on each sin/cos/sinh/cosh term. At defaults (1.0), reduces to Sin.",),
    "COS2_BS":  ("Parameterized Cos. At defaults (1.0), reduces to Cos.",),
    "TAN2_BS":  ("Parameterized Tan. At defaults (2.0, matching the upstream doubling), reduces to Tan.",),
    "SEC2_BS":  ("Parameterized Sec (1/cos). At defaults (1.0), reduces to Sec.",),
    "CSC2_BS":  ("Parameterized Csc (1/sin). At defaults (1.0), reduces to Csc.",),
    "COT2_BS":  ("Parameterized Cot (cos/sin). At defaults (2.0), reduces to Cot.",),
    "SINH2_BS": ("Parameterized Sinh. At defaults (1.0), reduces to Sinh.",),
    "COSH2_BS": ("Parameterized Cosh. At defaults (1.0), reduces to Cosh.",),
    "TANH2_BS": ("Parameterized Tanh. At defaults (2.0), reduces to Tanh.",),
    "COTH2_BS": ("Parameterized Coth. At defaults (2.0), reduces to Coth.",),
    "SECH2_BS": ("Parameterized Sech. At defaults (1.0), reduces to Sech (with the same JWildfire formula quirk noted in trig.rs).",),
    "CSCH2_BS": ("Parameterized Csch. At defaults (1.0), reduces to Csch.",),
    "EXP2_BS":  ("Parameterized complex exponential — `e^(x·x1)` modulated by `sin(y·y1)` and `cos(y·y2)`. At defaults (1.0), reduces to the unparameterized complex exp.",),
}

AUTHORS = ["cothe", "Brad Stefanov"]

# Per-param descriptions — uniform across the 12 trig variants.
COMMON_PARAM_DOC = {
    "x1": "Scales the argument of `sin(x)` in the internal computation.",
    "x2": "Scales the argument of `cos(x)` in the internal computation.",
    "y1": "Scales the argument of `sinh(y)` in the internal computation.",
    "y2": "Scales the argument of `cosh(y)` in the internal computation.",
}

# exp2_bs is the odd one out
EXP_PARAM_DOC = {
    "x1": "Scales the X exponent — output uses `exp(x · x1)`.",
    "y1": "Scales the argument of `sin(y)`.",
    "y2": "Scales the argument of `cos(y)`.",
}

# Build the full PARAM_DOC dict
PARAM_DOC = {}
TRIG_BS_VARIATIONS = [
    "SIN2_BS", "COS2_BS", "TAN2_BS", "SEC2_BS", "CSC2_BS", "COT2_BS",
    "SINH2_BS", "COSH2_BS", "TANH2_BS", "COTH2_BS", "SECH2_BS", "CSCH2_BS",
]
for static_name in TRIG_BS_VARIATIONS:
    for param_name, desc in COMMON_PARAM_DOC.items():
        PARAM_DOC[(static_name, param_name)] = desc

for param_name, desc in EXP_PARAM_DOC.items():
    PARAM_DOC[("EXP2_BS", param_name)] = desc

PATH = "src/variations/defs/trig_bs.rs"
with open(PATH, "rb") as f:
    src = f.read().decode("utf-8")

# PASS 1: insert doc-comments + # Authors
inserted = 0
already = 0
for name, (body, *_) in DOC.items():
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
    for author in AUTHORS:
        lines.append(f"/// - {author}")
    doc = "\n".join(lines)
    src = src.replace(target, f"\n{doc}\npub static {name}: VariationDef = VariationDef {{", 1)
    inserted += 1
print(f"  pass1: inserted {inserted}, already {already}")

# PASS 2: per-param descriptions (longhand only here)
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
