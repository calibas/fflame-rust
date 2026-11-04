"""Build pyfflame's engine module (wasm/python, for WASI) and put it in
the package; with --wheel, build the wheel as well.

    python python/build.py [--wheel]

Needs the Rust target: `rustup target add wasm32-wasip1`. The wheel is
pure Python plus the module, so one wheel serves every platform that
wasmtime and wgpu-py support.
"""

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRATE = ROOT / "wasm" / "python"
BUILT = ROOT / "wasm" / "target-gallery" / "wasm32-wasip1" / "release" / "fflame_python.wasm"
PACKAGED = ROOT / "python" / "pyfflame" / "fflame.wasm"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--wheel", action="store_true", help="also build the wheel into python/dist")
    args = ap.parse_args()

    # From the crate's directory: cargo finds wasm/python/.cargo/config.toml
    # (the WASI target and its flags) and wasm/.cargo/config.toml (the
    # shared target directory) by the current directory.
    subprocess.run(["cargo", "build", "--release"], cwd=CRATE, check=True)
    shutil.copyfile(BUILT, PACKAGED)
    print(f"{PACKAGED.relative_to(ROOT)}: {PACKAGED.stat().st_size / 1e6:.1f} MB")

    if args.wheel:
        # No build isolation: the installed setuptools builds it, rather
        # than one pip fetches for the occasion.
        subprocess.run([sys.executable, "-m", "pip", "wheel", ".", "--no-deps", "--no-build-isolation", "-w", "dist"],
                       cwd=ROOT / "python", check=True)


if __name__ == "__main__":
    main()
