"""Apply doc-comments + per-param descriptions to pre_post_bridges.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "PRE_CURL": (
        "Pre-phase version of Curl — applies the same complex-polynomial twist as Curl but before the rest of the variations run.",
        ["Xyrus02"],
    ),
    "POST_JULIAQ": (
        "Post-phase version of JuliaQ — applies the rational-power Julia branching after all other variations have run.",
        ["Zueuk"],
    ),
    "POST_JULIA3DQ": (
        "Post-phase version of Julia3DQ — applies the 3D rational-power Julia branching after all other variations have run.",
        ["Zueuk"],
    ),
}

PARAM_DOC = {
    ("PRE_CURL", "c1"): "Linear twist strength. Stronger = tighter curl around the center.",
    ("PRE_CURL", "c2"): "Quadratic twist strength. Adds a second-order curl that grows away from the origin.",

    ("POST_JULIAQ", "power"): "Number of Julia branches in the rational power.",
    ("POST_JULIAQ", "divisor"): "Rational-power divisor. Combined with `power` lets you pick non-integer branch counts (e.g. power=3, divisor=2 → 1.5 branches).",

    ("POST_JULIA3DQ", "power"): "Number of Julia branches.",
    ("POST_JULIA3DQ", "divisor"): "Rational-power divisor.",
}

PATH = "src/variations/defs/pre_post_bridges.rs"
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
