"""Apply doc-comments + per-param descriptions to apo_misc13.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "Q_ODE": (
        "Quadratic ODE warp — a 12-parameter quadratic polynomial mapping. Emits `(q01 + q02·x + q03·x² + q04·xy + q05·y + q06·y², q07 + q08·x + q09·x² + q10·xy + q11·y + q12·y²)`. Implements a general quadratic vector field in 2D — useful for modeling phase-space portraits of 2D ODEs.",
        ["DarkBeam"],
    ),
    "RIPPLE": (
        "Cosine-wave radial distortion — applies a radial cosine wave centered at `(centerx, centery)` with adjustable `frequency`, `velocity` (phase), `amplitude`, and `scale`. Linearly interpolates between two phase-shifted positions via `phase`. `fixed_dist_calc` toggles whether the distance uses Euclidean `sqrt(x² + y²)` or the upstream-quirk product `sqrt(x²·y²)`.",
        ["Xyrus02"],
    ),
    "SCRY2": (
        "N-sided scry — generalizes the `scry` variation to N-sided star/circle hybrids. Computes a max-projection across `sides` rotations, mixes with a circular term via `circle`, optionally folds with a star pattern via `star`. The output is `(x, y) / (r1 · (r2 + 1/w))` — `1/w` appears internally, so the divide-out pattern handles it.",
        ["DarkBeam"],
    ),
}

PARAM_DOC = {
    ("Q_ODE", "q_ode01"): "Constant term on X output.",
    ("Q_ODE", "q_ode02"): "x linear coefficient on X output.",
    ("Q_ODE", "q_ode03"): "x² coefficient on X output.",
    ("Q_ODE", "q_ode04"): "xy coefficient on X output.",
    ("Q_ODE", "q_ode05"): "y linear coefficient on X output.",
    ("Q_ODE", "q_ode06"): "y² coefficient on X output.",
    ("Q_ODE", "q_ode07"): "Constant term on Y output.",
    ("Q_ODE", "q_ode08"): "x linear coefficient on Y output.",
    ("Q_ODE", "q_ode09"): "x² coefficient on Y output.",
    ("Q_ODE", "q_ode10"): "xy coefficient on Y output.",
    ("Q_ODE", "q_ode11"): "y linear coefficient on Y output.",
    ("Q_ODE", "q_ode12"): "y² coefficient on Y output.",

    ("RIPPLE", "frequency"): "Spatial frequency of the cosine wave (scaled by 5 internally).",
    ("RIPPLE", "velocity"): "Phase velocity multiplier (× the internal `phase·2π − π` term).",
    ("RIPPLE", "amplitude"): "Wave amplitude (× 0.01 internally).",
    ("RIPPLE", "centerx"): "X-axis center of the radial wave.",
    ("RIPPLE", "centery"): "Y-axis center of the radial wave.",
    ("RIPPLE", "phase"): "Phase interpolation factor: 0 = first phase-shifted position, 1 = second.",
    ("RIPPLE", "scale"): "Input pre-scale factor.",
    ("RIPPLE", "fixed_dist_calc"): "Distance formula selector: 0 = product `sqrt(x²·y²)`, 1 = Euclidean `sqrt(x² + y²)`.",

    ("SCRY2", "sides"): "Polygon side count.",
    ("SCRY2", "star"): "Star-fold rotation amount (scaled by `-π/2` internally).",
    ("SCRY2", "circle"): "Circularity mixing factor: 0 = pure star/polygon, 1 = pure circle.",
}

PATH = "src/variations/defs/apo_misc13.rs"
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
