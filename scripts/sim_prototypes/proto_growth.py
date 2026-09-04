"""Eden, ballistic deposition, percolation labelling, Packard snowflake.

Phase-0 measurement for simulation mode: the Tier-1 growth and
labelling models. Unlike the reaction-diffusion set these mostly DO
terminate, so the number wanted is an exact step count, and for two of
them the catalogue only has an order of magnitude:

  - Eden: "~ radius / p" -- measured here against p.
  - Ballistic deposition: "~ grid height" -- measured.
  - Percolation label propagation: "O(diameter) ... ~10^3 steps at
    1024^2" -- measured at p_c, where the cluster is fractal and its
    CHEMICAL distance is much longer than its diameter. This is the one
    the estimate is most likely to be wrong about.
  - Packard snowflake: "steps ~ radius" -- measured, on the offset-row
    hex lattice the shader will use.

Writes PNGs and a JSON row set into output/sim_proto/.
"""
import json
import os
import time

import numpy as np
from PIL import Image

OUT = "output/sim_proto"
os.makedirs(OUT, exist_ok=True)
N = 256


def save_gray(name, a, lo=None, hi=None):
    f = np.asarray(a, dtype=np.float64)
    lo = f.min() if lo is None else lo
    hi = f.max() if hi is None else hi
    span = hi - lo if hi > lo else 1.0
    Image.fromarray((np.clip((f - lo) / span, 0, 1) * 255).astype(np.uint8)).save(
        os.path.join(OUT, name))


def run_eden(p, seed_kind="point", steps=4000, seed=1, snaps=()):
    """Each empty site with an occupied neighbour fills with probability p."""
    rg = np.random.default_rng(seed)
    occ = np.zeros((N, N), dtype=bool)
    age = np.zeros((N, N), dtype=np.int32)
    if seed_kind == "point":
        occ[N // 2, N // 2] = True
    else:
        occ[-1, :] = True
    t0 = time.time()
    done = None
    for s in range(1, steps + 1):
        # Periodic in x, NOT in y. np.roll wraps, and with a line seed
        # on the bottom row that wrap put growth straight into row 0 on
        # step 1 -- the run "reached the far edge" immediately. The
        # shipped model has the same choice to make per axis.
        up = np.roll(occ, 1, 0); up[0, :] = False
        dn = np.roll(occ, -1, 0); dn[-1, :] = False
        nb = up | dn | np.roll(occ, 1, 1) | np.roll(occ, -1, 1)
        grow = (~occ) & nb & (rg.random((N, N)) < p)
        # A step that grows nothing is NOT the end: at p = 0.05 a
        # four-neighbour front has an 81% chance of adding nothing on
        # any given step, and an earlier version of this stopped at
        # step 1 and reported a one-cell cluster.
        occ |= grow
        age[grow] = s
        # Stop at the edge -- past that the shape is the box, not the
        # model. A line seed occupies the bottom row, so that side is
        # excluded or the run ends on step 1.
        # A line seed fills the whole bottom row, so it already touches
        # the left and right edges on step 0; only the far edge counts.
        hit_edge = (occ[0, :].any() if seed_kind == "line" else
                    (occ[0, :].any() or occ[-1, :].any()
                     or occ[:, 0].any() or occ[:, -1].any()))
        if hit_edge:
            done = s
            break
        if s in snaps:
            save_gray(f"eden_p{p}_{s}.png", age)
    save_gray(f"eden_p{p}.png", age)
    return {"model": "eden", "params": f"p={p} seed={seed_kind}",
            "steps_to_edge": done, "filled": float(occ.mean()),
            "seconds": round(time.time() - t0, 1)}


def run_ballistic(p, sideways=True, steps=4000, seed=2):
    """h(i) <- max(h(i-1), h(i)+1, h(i+1)) for columns that receive."""
    rg = np.random.default_rng(2 if seed is None else seed)
    h = np.zeros(N, dtype=np.int64)
    arrival = np.zeros((N, N), dtype=np.int32)
    t0 = time.time()
    done = None
    for s in range(1, steps + 1):
        hit = rg.random(N) < p
        if sideways:
            cand = np.maximum(np.maximum(np.roll(h, 1), np.roll(h, -1)), h + 1)
        else:
            cand = h + 1
        newh = np.where(hit, cand, h)
        for i in np.nonzero(hit)[0]:
            y = int(newh[i]) - 1
            if 0 <= y < N:
                arrival[N - 1 - y, i] = s
        h = newh
        if h.max() >= N - 1:
            done = s
            break
    save_gray(f"ballistic_p{p}_{'side' if sideways else 'rand'}.png", arrival)
    # Family-Vicsek interface width, the thing that makes it KPZ.
    w = float(np.std(h))
    return {"model": "ballistic_deposition",
            "params": f"p={p} sideways={sideways}",
            "steps_to_fill": done, "interface_width": round(w, 2),
            "mean_height": float(h.mean()),
            "seconds": round(time.time() - t0, 1)}


def run_percolation(p, size=256, steps=20000, seed=3):
    """Label propagation: each open site takes the min label of its open
    4-neighbours, until nothing changes. Steps = the longest CHEMICAL
    path in the cluster, not the geometric diameter."""
    rg = np.random.default_rng(seed)
    open_ = rg.random((size, size)) < p
    lab = np.where(open_, np.arange(size * size).reshape(size, size), -1).astype(np.int64)
    big = size * size + 1
    t0 = time.time()
    rounds = None
    for s in range(1, steps + 1):
        v = np.where(open_, lab, big)
        m = v.copy()
        for sh, ax in ((1, 0), (-1, 0), (1, 1), (-1, 1)):
            r = np.roll(v, sh, axis=ax)
            # Zero boundary: do not wrap labels around the edge.
            if ax == 0:
                if sh == 1:
                    r[0, :] = big
                else:
                    r[-1, :] = big
            else:
                if sh == 1:
                    r[:, 0] = big
                else:
                    r[:, -1] = big
            m = np.minimum(m, r)
        nxt = np.where(open_, m, -1)
        if np.array_equal(nxt, lab):
            rounds = s
            break
        lab = nxt
    sizes = np.bincount(lab[open_].astype(np.int64).ravel()) if open_.any() else np.array([0])
    biggest = int(sizes.max()) if sizes.size else 0
    save_gray(f"percolation_p{p}.png", np.where(open_, (lab % 997), 0))
    return {"model": "percolation", "params": f"p={p} grid={size}",
            "label_rounds": rounds, "open_fraction": float(open_.mean()),
            "largest_cluster_frac": biggest / float(size * size),
            "seconds": round(time.time() - t0, 1)}


HEX_EVEN = [(-1, 0), (-1, -1), (0, -1), (0, 1), (1, -1), (1, 0)]
HEX_ODD = [(-1, 0), (-1, 1), (0, -1), (0, 1), (1, 0), (1, 1)]


def hex_neighbour_sum(a):
    """Six neighbours on an offset-row hex lattice (odd rows shifted).

    The row parity changes the offsets, which is exactly the awkward
    part the shader has to get right, so the prototype does it the same
    way rather than pretending the lattice is square."""
    n = np.zeros_like(a, dtype=np.int64)
    rows = np.arange(a.shape[0])
    odd = (rows % 2 == 1)[:, None]
    for (dy, dx_even), (_, dx_odd) in zip(HEX_EVEN, HEX_ODD):
        shifted_even = np.roll(np.roll(a, -dy, 0), -dx_even, 1)
        shifted_odd = np.roll(np.roll(a, -dy, 0), -dx_odd, 1)
        n += np.where(odd, shifted_odd, shifted_even).astype(np.int64)
    return n


def run_packard(rule_set, steps=200, seed_at=None):
    """A vacant cell freezes when its count of frozen hex neighbours is in S."""
    frozen = np.zeros((N, N), dtype=np.int64)
    c = seed_at or (N // 2, N // 2)
    frozen[c] = 1
    age = np.zeros((N, N), dtype=np.int32)
    age[c] = 1
    mask = np.zeros(7, dtype=bool)
    for k in rule_set:
        mask[k] = True
    t0 = time.time()
    done = None
    for s in range(1, steps + 1):
        cnt = hex_neighbour_sum(frozen)
        grow = (frozen == 0) & mask[np.clip(cnt, 0, 6)]
        if not grow.any():
            done = s
            break
        frozen[grow] = 1
        age[grow] = s
        if frozen[2, :].any() or frozen[-3, :].any() or \
           frozen[:, 2].any() or frozen[:, -3].any():
            done = s
            break
    name = "".join(str(k) for k in sorted(rule_set))
    save_gray(f"packard_{name}.png", age)
    return {"model": "packard_snowflake", "params": f"S={{{','.join(map(str, sorted(rule_set)))}}}",
            "steps_to_edge": done, "filled": float((frozen > 0).mean()),
            "seconds": round(time.time() - t0, 1)}


def main():
    rows = []
    print("Eden")
    for p in (1.0, 0.3, 0.05):
        rows.append(run_eden(p, snaps=(200,) if p == 0.3 else ()))
    rows.append(run_eden(0.3, "line"))
    print("ballistic deposition")
    rows.append(run_ballistic(0.5, True))
    rows.append(run_ballistic(0.5, False))
    print("percolation (p_c = 0.592746)")
    for p in (0.5, 0.592746, 0.65):
        rows.append(run_percolation(p))
    # The catalogue estimates "~10^3 steps at 1024^2". Label propagation
    # costs the longest CHEMICAL path, which at p_c is much longer than
    # the geometric diameter, so measure the scaling instead of assuming
    # it is linear in L.
    # SAMPLE-TO-SAMPLE VARIANCE IS THE HEADLINE HERE. At p_c the
    # spanning cluster is critical, so its longest chemical path swings
    # wildly between seeds -- two 256 grids measured 332 and 1,232
    # rounds. A single sample cannot support a scaling fit, so take
    # several per size and report the spread, because the step budget
    # has to cover the bad draw, not the median one.
    print("percolation size scaling at p_c (5 seeds per size)")
    Ls, meds, worst = [], [], []
    for L in (128, 256, 512):
        got = [run_percolation(0.592746, size=L, seed=100 + L + i)["label_rounds"]
               for i in range(5)]
        got = [g for g in got if g]
        Ls.append(float(L))
        meds.append(float(np.median(got)))
        worst.append(float(max(got)))
        rows.append({"model": "percolation", "params": f"p_c grid={L} x5 seeds",
                     "label_rounds": int(np.median(got)),
                     "rounds_min": int(min(got)), "rounds_max": int(max(got)),
                     "seconds": 0.0})
        print(f"    L={L}: median {np.median(got):,.0f}, range {min(got):,}-{max(got):,}")
    e = np.polyfit(np.log(Ls), np.log(meds), 1)[0]
    pred = float(np.exp(np.polyval(np.polyfit(np.log(Ls), np.log(meds), 1),
                                   np.log(1024.0))))
    ew = np.polyfit(np.log(Ls), np.log(worst), 1)[0]
    predw = float(np.exp(np.polyval(np.polyfit(np.log(Ls), np.log(worst), 1),
                                    np.log(1024.0))))
    print(f"    median rounds ~ L^{e:.2f} -> L=1024: {pred:,.0f}")
    print(f"    worst  rounds ~ L^{ew:.2f} -> L=1024: {predw:,.0f}")
    print("Packard snowflake")
    for rs in ([1], [1, 3], [1, 3, 4]):
        rows.append(run_packard(rs))

    with open(os.path.join(OUT, "growth_step_costs.json"), "w") as f:
        json.dump(rows, f, indent=1)
    print(f"\n{'model':<22}{'params':<26}{'steps':>9}{'sec':>7}  extra")
    for r in rows:
        steps = r.get("steps_to_edge") or r.get("steps_to_fill") or r.get("label_rounds")
        extra = ""
        for k in ("filled", "interface_width", "largest_cluster_frac"):
            if k in r:
                extra += f"{k}={r[k]:.4g}  "
        print(f"{r['model']:<22}{r['params']:<26}{str(steps):>9}{r['seconds']:>7}  {extra}")


if __name__ == "__main__":
    main()
