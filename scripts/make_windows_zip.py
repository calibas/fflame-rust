#!/usr/bin/env python3
"""Assemble the Windows distributable zip.

    python scripts/make_windows_zip.py                  # from target/dist
    python scripts/make_windows_zip.py --profile release

Windows only by default (`--any-platform` overrides, for testing the
layout). Reads the version from Cargo.toml so the zip cannot drift from
the binary it wraps.

# Why a script for something as simple as a zip

Because the contents are not obvious, and getting them wrong fails
*quietly*:

- **`assets/` must be there.** Presets, palette packs and the CJK font
  are loaded from disk at startup. Without them the app comes up, so
  nothing looks broken — it is just missing half its content.
- **`shaders/` must NOT be there.** Every shader is embedded in the
  binary; the on-disk tree is a developer override that takes
  precedence. Shipping it adds half a megabyte whose only effect is to
  let a stray edit change what a released app renders. That is the
  failure this script exists to prevent — the omission is deliberate and
  looks like a mistake to anyone packaging by hand.

A portable zip rather than an installer: it needs no elevation, leaves
no uninstall entry to rot, and the app stores its data in %APPDATA%
rather than beside the exe — so deleting the folder is a complete
uninstall.

# Signing

None. An unsigned exe gets a SmartScreen warning on first run. See
docs/RELEASE.md section 5.
"""

import argparse
import re
import shutil
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BINARY = "FractalArtEditor.exe"
STEM = "FractalArtEditor"


def cargo_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^\s*version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if not m:
        sys.exit("could not read version from Cargo.toml")
    return m.group(1)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--profile", default="dist",
                    help="cargo profile to package (default: dist)")
    ap.add_argument("--out", default="target/windows",
                    help="output directory (default: target/windows)")
    ap.add_argument("--any-platform", action="store_true",
                    help="package on a non-Windows host (layout testing)")
    args = ap.parse_args()

    if sys.platform != "win32" and not args.any_platform:
        sys.exit("Windows only (no cross-compilation); --any-platform to override")

    exe = ROOT / "target" / args.profile / BINARY
    if not exe.exists():
        sys.exit(
            f"missing {exe.relative_to(ROOT)}\n"
            f"  build it first:  cargo build --profile {args.profile}"
        )

    version = cargo_version()
    out_dir = ROOT / args.out
    out_dir.mkdir(parents=True, exist_ok=True)

    # A single top-level folder inside the zip, so extracting anywhere
    # gives one tidy directory rather than spraying files into Downloads.
    root_name = f"{STEM}-{version}"
    zip_path = out_dir / f"{STEM}-{version}-windows.zip"
    if zip_path.exists():
        zip_path.unlink()

    assets = ROOT / "assets"
    if not assets.is_dir():
        sys.exit("missing assets/ — the app needs it at runtime")

    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as z:
        z.write(exe, f"{root_name}/{BINARY}")
        # See the module docstring: assets yes, shaders no.
        for f in sorted(assets.rglob("*")):
            if f.is_file():
                z.write(f, f"{root_name}/{f.relative_to(ROOT).as_posix()}")

    print(f"  {zip_path.relative_to(ROOT)}  "
          f"({zip_path.stat().st_size / 1e6:.0f} MB)")
    print()
    print("  Unsigned: SmartScreen warns on first run —")
    print("  'More info' > 'Run anyway'.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
