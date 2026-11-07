"""Write `variations::twins::TWINS` from the files in
`assets/variations/twins/` (rhai-plugins P7 step 3): every twin, the ones
written by hand first, then the translated in name order. Run after
`translate_variation_twins.py` adds files."""
import os, re

DIR = "assets/variations/twins"
SRC = "src/variations/twins.rs"
HAND = ["julia", "rings2", "curl", "flipcircle", "sinusoidal3d", "exp", "hyperbolic"]

names = sorted(f[:-5] for f in os.listdir(DIR) if f.endswith(".rhai"))
translated = [n for n in names if n not in HAND]
lines = ["pub const TWINS: &[(&str, &str)] = &[", "    // Written by hand (V1)."]
lines += [f'    ("{n}", include_str!("../../assets/variations/twins/{n}.rhai")),' for n in HAND if n in names]
lines += ["    // Translated from the built-ins' WGSL (V2, `scripts/translate_variation_twins.py`)."]
lines += [f'    ("{n}", include_str!("../../assets/variations/twins/{n}.rhai")),' for n in translated]
lines += ["];"]
t = open(SRC, encoding="utf-8").read()
t = re.sub(r"pub const TWINS: &\[\(&str, &str\)\] = &\[\n.*?\n\];", "\n".join(lines), t, count=1, flags=re.S)
open(SRC, "w", encoding="utf-8", newline="\n").write(t)
print(f"{len(names)} twins: {len(HAND)} by hand, {len(translated)} translated")
