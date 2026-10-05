"""Write McCabe "lean on purpose" demo configs into output/mccabe-lean/
(mccabe-multiscale plan, section 9): Exact discs, the table filled from
the ladder, and each scale's discs stretched at an angle.

    python scripts/sim_prototypes/make_mccabe_lean_demo.py
    target/release/FractalArtEditor export -i output/mccabe-lean -o output/mccabe-lean/png
"""
import copy
import json
import os

BASE = "tests/visual/configs/sim/mccabe-memory.fflame"
OUT = "output/mccabe-lean"

LEANS = {
    "round": [(1, 0)] * 5,
    "all-2to1-0deg": [(2, 0)] * 5,
    "alt-0-90": [(2, 0), (2, 90), (2, 0), (2, 90), (2, 0)],
    "coarse-3to1-0-60-120": [(1, 0), (1, 0), (3, 0), (3, 60), (3, 120)],
    "36deg-apart": [(2, 0), (2, 36), (2, 72), (2, 108), (2, 144)],
}


def main():
    cfg = json.load(open(BASE))
    os.makedirs(OUT, exist_ok=True)
    sim = cfg["sim"]
    sim["grid"] = {"fixed": {"width": 512, "height": 512}}
    sim["steps"] = 300
    p = sim["model_params"]
    p.update({"scales": 5.0, "base_radius": 2.0, "ratio": 2.0, "amount": 0.05,
              "amount_min": 0.01, "symmetry": 0.0, "memory": 0.17, "averaging": 2.0,
              "layout": 1.0})
    # The ladder's table, as the panel's switch fills it.
    for i in range(6):
        t = min(i / 4, 1.0)
        p[f"s{i}_radius"] = 2.0 * (1 << i)
        p[f"s{i}_ratio"] = 2.0
        p[f"s{i}_amount"] = 0.05 * (1 - t) + 0.01 * t
        p[f"s{i}_weight"] = 1.0
        p[f"s{i}_symmetry"] = 0.0
    for name, lean in LEANS.items():
        c = copy.deepcopy(cfg)
        # The export names its PNG after the flame.
        c["flame"]["name"] = f"lean-{name}"
        for i, (stretch, angle) in enumerate(lean):
            c["sim"]["model_params"][f"s{i}_stretch"] = float(stretch)
            c["sim"]["model_params"][f"s{i}_angle"] = float(angle)
        json.dump(c, open(f"{OUT}/{name}.fflame", "w"), indent=1)
        print("wrote", f"{OUT}/{name}.fflame")


if __name__ == "__main__":
    main()
