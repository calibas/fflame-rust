"""Splice the generated mode-D presets into `assets/presets.fflame`.

The generator (`escape::ifs::gpu_tests::write_the_classical_ifs_presets`,
an `#[ignore]`d one-shot) writes `output/ifs-presets.json` through the
config's own serialiser -- which is the part that matters, because a
config written by `serde_json` directly carries no `version` field and
is migrated on load as if it were v2, silently turning a mode-D preset
back into a flame.

This merges those entries into the shipped preset file BY NAME, so
regenerating one preset does not disturb the others or their order.

    cargo test --release --lib write_the_classical_ifs_presets -- --ignored
    python scripts/merge_ifs_presets.py
"""
import io
import json

GENERATED = "output/ifs-presets.json"
SHIPPED = "assets/presets.fflame"

new = json.load(io.open(GENERATED, encoding="utf-8"))
old = json.load(io.open(SHIPPED, encoding="utf-8"))

by_name = {}
for c in new:
    name = c.get("flame", {}).get("name")
    assert name, "a generated preset has no name"
    by_name[name] = c

merged, replaced = [], []
for c in old:
    name = c.get("flame", {}).get("name")
    if name in by_name:
        merged.append(by_name.pop(name))
        replaced.append(name)
    else:
        merged.append(c)

added = list(by_name)
merged.extend(by_name.values())

io.open(SHIPPED, "w", encoding="utf-8", newline="\n").write(
    json.dumps(merged, indent=2) + "\n"
)
print(f"{len(merged)} presets: replaced {replaced}, added {added}")
