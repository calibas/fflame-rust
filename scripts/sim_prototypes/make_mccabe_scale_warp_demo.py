"""Write McCabe per-scale warp demo configs into output/mccabe-scale-warp/
(mccabe-multiscale plan, section 10): the prototype's three maps, and a
few more, with Scale Memory colour and relief.

    python scripts/sim_prototypes/make_mccabe_scale_warp_demo.py
    target/release/FractalArtEditor export -i output/mccabe-scale-warp -o output/mccabe-scale-warp/png
"""
import copy
import json
import os

BASE = "tests/visual/configs/sim/mccabe-memory.fflame"
OUT = "output/mccabe-scale-warp"

# name -> {param: value}, scales 0-4 (radii 2..32).
VARIANTS = {
    "none": {},
    "rotate-alt-3deg": {f"s{i}_warp_rotation": (0.052 if i % 2 == 0 else -0.052) for i in range(5)},
    "zoom-coarse": {"s2_warp_zoom": 1.02, "s3_warp_zoom": 1.04, "s4_warp_zoom": 1.06},
    "swirl-rising": {f"s{i}_warp_flow": s for i, s in enumerate([0.05, 0.1, 0.2, 0.3, 0.4])},
    "swirl-coarse-only": {"s3_warp_flow": 0.3, "s4_warp_flow": -0.3},
    "pan-coarse": {"s4_warp_pan_x": 1.5, "s3_warp_pan_y": -1.0},
}


def main():
    cfg = json.load(open(BASE))
    os.makedirs(OUT, exist_ok=True)
    sim = cfg["sim"]
    sim["grid"] = {"fixed": {"width": 512, "height": 512}}
    sim["steps"] = 300
    sim["model_params"].update({"scales": 5.0, "base_radius": 2.0, "memory": 0.17})
    sim["color_layers"] = [
        {"source": 0, "coloring": "scale_memory",
         "coloring_params": {"scales": 5.0, "value_scale": 0.0, "brightness": 0.0}},
        {"source": 0, "coloring": "relief", "coloring_params": {}, "blend": "hard_light"},
    ]
    for name, warps in VARIANTS.items():
        c = copy.deepcopy(cfg)
        c["flame"]["name"] = f"scale-warp-{name}"
        c["sim"]["model_params"].update(warps)
        json.dump(c, open(f"{OUT}/{name}.fflame", "w"), indent=1)
        print("wrote", f"{OUT}/{name}.fflame")


if __name__ == "__main__":
    main()
