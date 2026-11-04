"""SPIKE: render configs through the WASI module (fflame_host.py) and
through the native CLI, and compare the pictures.

    python wasm/python/compare.py [--size WxH] CONFIG.fflame...

Writes output/python-spike/compare/<name>-{wasi,native,diff}.png and a
table: the mean absolute difference (0-255), the share of pixels off by
more than 8, the same on 8x8 block means (what survives sampling noise),
and the render times (WASI: wall and the engine's own; native: the
engine's own, from the PNG). A failing config gets a fresh module, since
a panic leaves the instance unusable.
"""

import argparse
import os
import shutil
import struct
import subprocess
import sys
import time

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(__file__))
import fflame_host as fh  # noqa: E402

OUT = "output/python-spike/compare"
EXE = os.path.abspath("target/release/FractalArtEditor.exe")


def png_text(path):
    d = open(path, "rb").read()
    out, i = {}, 8
    while i < len(d):
        n, = struct.unpack(">I", d[i:i + 4])
        if d[i + 4:i + 8] == b"tEXt":
            k, _, v = d[i + 8:i + 8 + n].partition(b"\x00")
            out[k.decode()] = v.decode(errors="replace")
        i += 12 + n
    return out


def native(cfg, name, w, h):
    work = os.path.join(OUT, "native-" + name)
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work + "/in")
    shutil.copy(cfg, work + "/in/")
    r = subprocess.run([EXE, "export", "-i", work + "/in", "-o", work, "--width", str(w), "--height", str(h)],
                       capture_output=True, text=True)
    pngs = [f for f in os.listdir(work) if f.endswith(".png")]
    if not pngs:
        return None, None, (r.stdout + r.stderr).strip().splitlines()[-1:]
    path = os.path.join(OUT, name + "-native.png")
    shutil.move(os.path.join(work, pngs[0]), path)
    shutil.rmtree(work, ignore_errors=True)
    ms = png_text(path).get("RenderTime", "?").removesuffix("ms")
    return path, ms, None


def blocks(x):
    h, w = x.shape[0] // 8 * 8, x.shape[1] // 8 * 8
    return x[:h, :w].reshape(h // 8, 8, w // 8, 8, 3).mean((1, 3))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--size", default="400x300")
    ap.add_argument("configs", nargs="+")
    a = ap.parse_args()
    w, h = map(int, a.size.split("x"))
    os.makedirs(OUT, exist_ok=True)
    eng = fh.Engine()
    print(f"{'config':40s} {'mean':>6s} {'>8':>6s} {'blocks':>6s} {'wasi wall':>9s} {'wasi eng':>8s} {'native':>8s}")
    for cfg in a.configs:
        name = os.path.splitext(os.path.basename(cfg))[0]
        text = open(cfg, encoding="utf-8").read()
        t0 = time.perf_counter()
        try:
            px, info = eng.render(text, w, h)
        except Exception as e:  # a panic traps the module: start a new one
            msg = str(e).strip().splitlines()
            print(f"{name:40s} WASI FAILED: {msg[0][:150] if msg else type(e).__name__}")
            eng = fh.Engine()
            continue
        wall = (time.perf_counter() - t0) * 1000
        wasi_path = os.path.join(OUT, name + "-wasi.png")
        fh.write_png(wasi_path, px, info["width"], info["height"])
        npath, nms, err = native(cfg, name, w, h)
        if npath is None:
            print(f"{name:40s} native failed: {err}")
            continue
        x = np.asarray(Image.open(npath).convert("RGB")).astype(np.float64)
        y = np.asarray(Image.open(wasi_path).convert("RGB")).astype(np.float64)
        if x.shape != y.shape:
            print(f"{name:40s} sizes differ: native {x.shape} wasi {y.shape}")
            continue
        d = np.abs(x - y)
        Image.fromarray(np.clip(d * 4, 0, 255).astype(np.uint8)).save(os.path.join(OUT, name + "-diff.png"))
        print(f"{name:40s} {d.mean():6.2f} {(d.max(-1) > 8).mean():6.3f} {np.abs(blocks(x) - blocks(y)).mean():6.2f} "
              f"{wall:7.0f}ms {info['ms']:6.0f}ms {nms:>6s}ms", flush=True)


if __name__ == "__main__":
    main()
