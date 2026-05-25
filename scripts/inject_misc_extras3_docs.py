"""Apply doc-comments + per-param descriptions to misc_extras3.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "OSCILLOSCOPE2": (
        "2D extension of `oscilloscope` — adds a Y-driven sinusoidal perturbation `perturbation · sin(2π·freqy·y)` to the X-axis cosine argument, and flips BOTH X and Y when the input falls inside the band (vs `oscilloscope`, which only flips Y). Produces a 2D oscilloscope-trace masking pattern.",
        ["Apophysis Plugin Pack", "DarkBeam"],
    ),
    "LINEART": (
        "Per-axis sign-preserving power (2D) — applies `sign(coord) · |coord|^pow` to each of X and Y independently, with separate exponents per axis. The 2D analogue of `lineart3d`.",
        ["FractalDesire"],
    ),
    "PHOENIX_JULIA": (
        "Phoenix-Julia N-th-root sampler with X/Y distortion — distorts the input by `(1 + x_distort, 1 + y_distort)`, scales the resulting angle by `1/power`, adds a random branch `n · 2π/power` (with `n` random from `0` to `power − 1`), and emits at radius `r^(dist/power)` in the new angular direction.",
        ["TyrantWave"],
    ),
    "POW_BLOCK": (
        "Generalized N-th root of `(x²+y²)^p` with random branch — computes the input angle, scales it by `1/denominator`, adds a random `root · 2π · k/denominator` branch (with `k` random from `0` to `denominator − 1`), multiplies the whole angle by `numerator`, and emits at radius governed by `numerator / (2·denominator) · (correctd/correctn)`.",
        ["cothe", "DarkBeam"],
    ),
}

PARAM_DOC = {
    ("OSCILLOSCOPE2", "separation"): "Vertical offset of the band threshold.",
    ("OSCILLOSCOPE2", "frequencyx"): "X-axis cosine frequency (multiplied by 2π internally).",
    ("OSCILLOSCOPE2", "frequencyy"): "Y-axis sine frequency for the perturbation term (multiplied by 2π).",
    ("OSCILLOSCOPE2", "amplitude"): "Cosine amplitude.",
    ("OSCILLOSCOPE2", "perturbation"): "Magnitude of the Y-driven X-perturbation.",
    ("OSCILLOSCOPE2", "damping"): "Exponential damping rate. Values near zero disable the damping term.",

    ("LINEART", "powX"): "X-axis power exponent.",
    ("LINEART", "powY"): "Y-axis power exponent.",

    ("PHOENIX_JULIA", "power"): "Number of angular branches — `floor(rand · power)` picks the branch each iteration.",
    ("PHOENIX_JULIA", "dist"): "Radial-power exponent — output magnitude is `r^(dist/power)`.",
    ("PHOENIX_JULIA", "x_distort"): "X-axis pre-rotation distortion (multiplier on X before angle computation, offset by +1).",
    ("PHOENIX_JULIA", "y_distort"): "Y-axis pre-rotation distortion (offset by +1).",

    ("POW_BLOCK", "numerator"): "Output-angle multiplier and effective power numerator.",
    ("POW_BLOCK", "denominator"): "Branch count — `floor(rand · denominator)` picks the branch.",
    ("POW_BLOCK", "correctn"): "Power-correction numerator (divides the effective exponent).",
    ("POW_BLOCK", "correctd"): "Power-correction denominator (multiplies the effective exponent).",
    ("POW_BLOCK", "root"): "Per-branch angular offset multiplier (× 2π).",
}

PATH = "src/variations/defs/misc_extras3.rs"
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
