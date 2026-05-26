"""Apply doc-comments + per-param descriptions to dc_carpet3d_misc.rs and vibration2_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "DC_CARPET3D": (
        "dc_carpet3d_misc.rs",
        "3D Sierpinski-carpet IFS — picks a random corner of the unit square by independent ±1 sign choices on X and Y (with per-axis offsets `stretch_x`/`stretch_y`), then runs the result through the transform's affine and scales by `scale_x`/`scale_y`. Originally a direct-color variation with color-coupled Z output (`z = color · scale_z + offset_z`); since we don't write color, the Z output collapses to a constant `offset_z` bump and the color/scale_z/reset_z params are kept only for interface parity with the C++/Java original.",
        ["Xyrus02", "Brad Stefanov"],
    ),
    "VIBRATION2": (
        "vibration2_misc.rs",
        "Two-wave directional vibration — adds the sum of two independent sine waves to the input, where each wave's direction, angle, frequency, and amplitude are themselves modulated by sinusoids of the projected coordinate along the wave's direction. Each wave has 5 base params (`dir`, `angle`, `freq`, `amp`, `phase`) plus 4 modulation pairs (`dm`/`dmfreq` for direction, `tm`/`tmfreq` for angle, `fm`/`fmfreq` for frequency, `am`/`amfreq` for amplitude) — 13 params per wave × 2 waves = 26 user params total, which was the first variation to exceed the old 16-slot per-variation buffer ceiling and motivated switching to the packed-variation-params buffer layout.",
        ["FarDareisMai", "Brad Stefanov"],
    ),
}

PARAM_DOC = {
    # dc_carpet3D
    ("DC_CARPET3D", "origin"): "Original `origin` parameter that controlled color in the C++/Java source. Unused in the Rust port since color writes are dropped — kept for interface parity.",
    ("DC_CARPET3D", "color_a"): "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity.",
    ("DC_CARPET3D", "color_b"): "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity.",
    ("DC_CARPET3D", "color_c"): "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity.",
    ("DC_CARPET3D", "color_d"): "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity.",
    ("DC_CARPET3D", "color_e"): "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity.",
    ("DC_CARPET3D", "color_f"): "Color parameter (unused — color writes dropped). Kept for cpp/Java interface parity.",
    ("DC_CARPET3D", "stretch_x"): "Magnitude of the ±1 X corner offset added to the input. Larger values pull the carpet's corners further apart along X.",
    ("DC_CARPET3D", "stretch_y"): "Magnitude of the ±1 Y corner offset added to the input.",
    ("DC_CARPET3D", "scale_x"): "Post-affine X scale applied to the carpet position.",
    ("DC_CARPET3D", "scale_y"): "Post-affine Y scale applied to the carpet position.",
    ("DC_CARPET3D", "scale_z"): "Color-to-Z coupling multiplier (unused — color writes dropped). Kept for cpp/Java interface parity.",
    ("DC_CARPET3D", "offset_z"): "Constant Z bump added to the output per iteration. Originally a fallback added to the color-derived Z; in this port it's the only contribution to Z.",
    ("DC_CARPET3D", "reset_z"): "Color-driven Z reset switch (unused — color writes dropped). Kept for cpp/Java interface parity.",

    # vibration2 — wave 1
    ("VIBRATION2", "dir"): "Wave 1: direction angle (radians) along which the wave propagates. The input is projected onto this direction before phase computation.",
    ("VIBRATION2", "angle"): "Wave 1: output rotation angle (radians) — controls which direction in the output plane the wave's displacement points.",
    ("VIBRATION2", "freq"): "Wave 1: spatial frequency. Smaller = longer wavelength.",
    ("VIBRATION2", "amp"): "Wave 1: amplitude (peak displacement magnitude).",
    ("VIBRATION2", "phase"): "Wave 1: phase offset in cycles; converted internally to radians as `2π·phase/freq`.",
    # vibration2 — wave 2
    ("VIBRATION2", "dir2"): "Wave 2: direction angle. See `dir`.",
    ("VIBRATION2", "angle2"): "Wave 2: output rotation angle. See `angle`.",
    ("VIBRATION2", "freq2"): "Wave 2: spatial frequency. See `freq`.",
    ("VIBRATION2", "amp2"): "Wave 2: amplitude. See `amp`.",
    ("VIBRATION2", "phase2"): "Wave 2: phase offset. See `phase`.",
    # vibration2 — modulation (wave 1)
    ("VIBRATION2", "dm"): "Wave 1: direction modulation depth. The propagation direction wobbles by `dm · cos(d · dmfreq · 2π)` where `d` is the projected coordinate.",
    ("VIBRATION2", "dmfreq"): "Wave 1: spatial frequency of the direction-modulation wobble.",
    ("VIBRATION2", "tm"): "Wave 1: output-angle modulation depth (same shape as `dm` but applied to the output rotation angle).",
    ("VIBRATION2", "tmfreq"): "Wave 1: spatial frequency of the output-angle modulation.",
    ("VIBRATION2", "fm"): "Wave 1: frequency-modulation depth. Modulates the wave's phase term by `fm · cos(d · fmfreq · 2π) / freq`.",
    ("VIBRATION2", "fmfreq"): "Wave 1: spatial frequency of the frequency modulation.",
    ("VIBRATION2", "am"): "Wave 1: amplitude-modulation depth. Multiplies amplitude by `1 + am · cos(d · amfreq · 2π)`.",
    ("VIBRATION2", "amfreq"): "Wave 1: spatial frequency of the amplitude modulation.",
    # vibration2 — modulation (wave 2)
    ("VIBRATION2", "d2m"): "Wave 2: direction modulation depth. See `dm`.",
    ("VIBRATION2", "d2mfreq"): "Wave 2: direction modulation frequency. See `dmfreq`.",
    ("VIBRATION2", "t2m"): "Wave 2: angle modulation depth. See `tm`.",
    ("VIBRATION2", "t2mfreq"): "Wave 2: angle modulation frequency. See `tmfreq`.",
    ("VIBRATION2", "f2m"): "Wave 2: frequency modulation depth. See `fm`.",
    ("VIBRATION2", "f2mfreq"): "Wave 2: frequency modulation frequency. See `fmfreq`.",
    ("VIBRATION2", "a2m"): "Wave 2: amplitude modulation depth. See `am`.",
    ("VIBRATION2", "a2mfreq"): "Wave 2: amplitude modulation frequency. See `amfreq`.",
}


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


files = {}
for static_name, (path_rel, body, authors) in DOC.items():
    files.setdefault(path_rel, {})[static_name] = (body, authors)

for path_rel, doc_map in files.items():
    full_path = f"src/variations/defs/{path_rel}"
    with open(full_path, "rb") as f:
        src = f.read().decode("utf-8")

    inserted = 0
    already = 0
    for name, (body, authors) in doc_map.items():
        target = f"\npub static {name}: VariationDef = VariationDef {{"
        if target not in src:
            print(f"  WARN: no match for {name} in {path_rel}")
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
    print(f"  {path_rel} pass1: inserted {inserted}, already {already}")

    pinserted = 0
    palready = 0
    for (static_name, param_name), desc in PARAM_DOC.items():
        if static_name not in doc_map:
            continue
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
    print(f"  {path_rel} pass2: injected {pinserted}, already {palready}")

    with open(full_path, "wb") as f:
        f.write(src.encode("utf-8"))
