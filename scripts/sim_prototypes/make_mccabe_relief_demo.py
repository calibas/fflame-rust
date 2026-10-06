"""Write McCabe relief demo configs into output/mccabe-relief/
(mccabe-multiscale plan, section 10): Scale Memory with a Relief layer
over it under Hard light, at a few settings.

    python scripts/sim_prototypes/make_mccabe_relief_demo.py
    target/release/FractalArtEditor export -i output/mccabe-relief -o output/mccabe-relief/png
"""
import copy
import json
import os

BASE = "tests/visual/configs/sim/mccabe-memory.fflame"
OUT = "output/mccabe-relief"

# name -> (relief params, scale memory brightness range)
VARIANTS = {
    "none": (None, 0.5),
    "defaults": ({}, 0.0),
    "raw": ({"softness": 0.0}, 0.0),
    "soft4": ({"softness": 4.0}, 0.0),
    "lambert": ({"model": 1.0}, 0.0),
    "age": ({"channel": 2.0, "height": 0.05}, 0.0),
}


def main():
    cfg = json.load(open(BASE))
    os.makedirs(OUT, exist_ok=True)
    sim = cfg["sim"]
    sim["grid"] = {"fixed": {"width": 512, "height": 512}}
    sim["steps"] = 300
    sim["model_params"].update({"scales": 5.0, "base_radius": 2.0, "memory": 0.17})
    for name, (relief, value_scale) in VARIANTS.items():
        c = copy.deepcopy(cfg)
        c["flame"]["name"] = f"relief-{name}"
        s = c["sim"]
        base = {"source": 0, "coloring": "scale_memory",
                "coloring_params": {"scales": 5.0, "value_scale": value_scale, "brightness": 0.0}}
        s["color_layers"] = [base]
        if relief is not None:
            s["color_layers"].append({"source": 0, "coloring": "relief", "coloring_params": relief,
                                      "blend": "hard_light"})
        json.dump(c, open(f"{OUT}/{name}.fflame", "w"), indent=1)
        print("wrote", f"{OUT}/{name}.fflame")


if __name__ == "__main__":
    main()
