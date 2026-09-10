"""Physarum transport networks (Jones 2010) validation prototype.

Phase-4 measurement. Unlike most of the catalogue's entries this one
had its paper available (`output/pdf/Physarum.2010.16.2.pdf`), so the
rule and the constants below are READ rather than remembered, and
Table 1 confirms every value the catalogue had recorded from a
secondary source:

    SA     22.5 or 45 deg   sensor angle from forward
    RA     45 deg           agent rotation angle
    SO     9 pixels         sensor offset distance
    SW     1 pixel          sensor width
    SS     1 pixel/step     step size
    depT   5                deposit per step
    decayT 0.1              trail decay factor
    diffK  3                diffusion kernel size (3x3 mean)
    %p     3-15             population as a percentage of image area
    pCD    0                probability of a random direction change
    sMin   0                sensitivity threshold
    Boundary  Periodic

Section 2.1's loop: every agent attempts to move forward one step;
after every agent has moved, the population senses; the trail map is
diffused after every system step.

WHAT THIS PROTOTYPE SETTLED, and the reason the shipped model looks
the way it does: whether Jones' occupancy EXCLUSION can be dropped.
The paper says an agent that cannot move (because the next site is
occupied) stays put, deposits nothing, and picks a new random
heading, and that a stuck agent is what "represents the immobile gel
matrix". The catalogue's GPU sketch left it out, because exclusion is
order-dependent unless it is resolved by an atomic minimum over agent
indices -- a second pass and a second buffer.

It cannot be dropped. Run both ways on the same seed and the same
Table 1 parameters: WITHOUT exclusion the population collapses onto a
handful of thick arcs, and WITH it the same parameters give the
polygonal transport network of the paper's figures. So the shipped
model pays for the second pass, and resolves the claim by atomic
minimum so the run still reproduces exactly.

Writes PNGs and a JSON row set into output/sim_proto/.
"""
import json
import os
import sys
import time

import numpy as np
from PIL import Image

OUT = "output/sim_proto"
os.makedirs(OUT, exist_ok=True)


def save(name, f):
    a = np.asarray(f, dtype=np.float64)
    hi = np.percentile(a, 99.5) if a.max() > 0 else 1.0
    img = np.clip(a / max(hi, 1e-9), 0, 1)
    Image.fromarray((img * 255).astype(np.uint8)).save(f"{OUT}/{name}.png")


def diffuse_decay(trail, decay):
    """Jones' 3x3 mean filter, then the decay factor. Periodic."""
    acc = np.zeros_like(trail)
    for dy in (-1, 0, 1):
        for dx in (-1, 0, 1):
            acc += np.roll(np.roll(trail, dy, 0), dx, 1)
    return acc * (1.0 / 9.0) * (1.0 - decay)


def run(n=256, pct=5.0, sa_deg=22.5, ra_deg=45.0, so=9.0, ss=1.0,
        dep=5.0, decay=0.1, steps=600, exclusion=False, seed=1, snaps=()):
    rng = np.random.default_rng(seed)
    count = int(n * n * pct / 100.0)
    pos = rng.random((count, 2)) * n
    head = rng.random(count) * 2 * np.pi
    trail = np.zeros((n, n))
    sa = np.radians(sa_deg)
    ra = np.radians(ra_deg)

    def sense(p, h, off):
        q = p + np.stack([np.cos(h + off), np.sin(h + off)], 1) * so
        qi = np.mod(np.round(q).astype(int), n)
        return trail[qi[:, 1], qi[:, 0]]

    for step in range(1, steps + 1):
        # Sense three forward sensors, then turn.
        f = sense(pos, head, 0.0)
        l = sense(pos, head, +sa)
        r = sense(pos, head, -sa)
        turn = np.zeros(count)
        # Jones: forward strongest -> keep; one side strongest -> turn
        # that way; both sides beat forward -> turn randomly.
        both = (f < l) & (f < r)
        turn = np.where((l > r) & ~both, +ra, turn)
        turn = np.where((r > l) & ~both, -ra, turn)
        coin = rng.random(count) < 0.5
        turn = np.where(both, np.where(coin, +ra, -ra), turn)
        head = head + turn

        nxt = pos + np.stack([np.cos(head), np.sin(head)], 1) * ss
        ni = np.mod(np.round(nxt).astype(int), n)

        if exclusion:
            # One agent per cell, resolved by the LOWEST agent index so
            # the outcome does not depend on evaluation order -- the
            # only form of exclusion a GPU can reproduce exactly.
            claim = np.full((n, n), np.iinfo(np.int32).max, dtype=np.int64)
            np.minimum.at(claim, (ni[:, 1], ni[:, 0]), np.arange(count))
            won = claim[ni[:, 1], ni[:, 0]] == np.arange(count)
        else:
            won = np.ones(count, dtype=bool)

        pos = np.where(won[:, None], np.mod(nxt, n), pos)
        # A blocked agent takes a new random heading and deposits
        # nothing.
        head = np.where(won, head, rng.random(count) * 2 * np.pi)
        di = np.mod(np.round(pos).astype(int), n)
        np.add.at(trail, (di[won, 1], di[won, 0]), dep)

        trail = diffuse_decay(trail, decay)
        if step in snaps:
            save(f"phys_{'excl' if exclusion else 'free'}_{step}", trail)
    return trail, pos, count


def network_score(trail):
    """Fraction of the total trail in the brightest 5% of cells.

    NOT a network detector, and worth saying so: it was written as one
    and it is backwards. Measured, the run WITHOUT exclusion scores
    0.752 and the run with it 0.358 -- and the pictures show the
    opposite of what that suggests. Without exclusion the population
    collapses onto a handful of heavy arcs, which concentrates the mass
    and scores high; with it the mass is spread along many filaments of
    a real network, which scores low. Concentration is what a COLLAPSE
    looks like. The images decided this, not the number, and the number
    is kept only because it separates the two runs."""
    flat = np.sort(trail.ravel())[::-1]
    top = int(len(flat) * 0.05)
    return float(flat[:top].sum() / max(flat.sum(), 1e-9))


if __name__ == "__main__":
    rows = []
    print("Jones Table 1 defaults, 256^2, periodic")
    print(f"{'mode':<6} {'SA':>6} {'%p':>5} {'agents':>7} {'top5%':>7} {'sd':>8}  {'s/1k steps':>10}")
    for exclusion in (False, True):
        for sa in (22.5, 45.0):
            t0 = time.time()
            trail, pos, count = run(
                sa_deg=sa, exclusion=exclusion, steps=600,
                snaps=(100, 600) if sa == 22.5 else ())
            sc = network_score(trail)
            secs = (time.time() - t0) / 0.6
            mode = "excl" if exclusion else "free"
            print(f"{mode:<6} {sa:>6} {5.0:>5} {count:>7} {sc:>7.3f} {trail.std():>8.4f} {secs:>10.1f}")
            rows.append(dict(mode=mode, SA=sa, pct=5.0, agents=count,
                             top5_mass=sc, sd=float(trail.std())))
    with open(f"{OUT}/physarum.json", "w") as fh:
        json.dump(rows, fh, indent=1)
    print(f"\nwrote {OUT}/physarum.json and phys_*.png")
