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


def stale_inputs(exe):
    """Source paths newer than `exe`, worst offender first.

    The zip pairs one compiled binary with a fresh copy of `assets/`,
    so the two can disagree: a stale exe still carries whatever it
    embedded via `include_str!` (palette packs, shaders, scripts,
    locales) at the time it was built. That is not hypothetical — a
    palette pack deleted from the repo reappeared in a packaged build
    because the exe predated the deletion by an hour.
    """
    exe_mtime = exe.stat().st_mtime
    roots = ["src", "shaders", "assets", "locales"]
    files = ["Cargo.toml", "Cargo.lock", "build.rs"]

    newer = []
    for r in roots:
        d = ROOT / r
        if d.is_dir():
            newer += [f for f in d.rglob("*")
                      if f.is_file() and f.stat().st_mtime > exe_mtime]
    for name in files:
        f = ROOT / name
        if f.is_file() and f.stat().st_mtime > exe_mtime:
            newer.append(f)

    newer.sort(key=lambda f: f.stat().st_mtime, reverse=True)
    return [f.relative_to(ROOT).as_posix() for f in newer]


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

    stale = stale_inputs(exe)
    if stale:
        listed = "\n".join(f"    {p}" for p in stale[:5])
        more = f"\n    ...and {len(stale) - 5} more" if len(stale) > 5 else ""
        sys.exit(
            f"{exe.relative_to(ROOT)} is older than its inputs:\n{listed}{more}\n\n"
            f"  Assets are copied into the zip fresh from the repo, but anything\n"
            f"  the binary EMBEDS (palette packs, shaders, scripts, locales) is\n"
            f"  baked in at compile time. Packaging now would ship fresh assets\n"
            f"  around a stale exe - which is how a deleted palette pack came\n"
            f"  back to life in a shipped build.\n\n"
            f"  rebuild first:  cargo build --profile {args.profile}"
        )

    version = cargo_version()
    out_dir = ROOT / args.out
    out_dir.mkdir(parents=True, exist_ok=True)

    # A single top-level folder inside the zip, so extracting anywhere
    # gives one tidy directory rather than spraying files into Downloads.
    root_name = f"{STEM}-{version}"
    zip_path = out_dir / f"{STEM}-{version}-windows.zip"
    if zip_path.exists():
        # Windows keeps a share lock while Explorer is browsing INSIDE the
        # zip, or while an app is running from a folder extracted through
        # the shell view. Both are the normal way to test a release build,
        # so this is a routine collision, not a broken tree - say what to
        # close instead of a PermissionError traceback after a 7-minute
        # rebuild.
        try:
            zip_path.unlink()
        except PermissionError:
            sys.exit(
                f"cannot replace {zip_path.relative_to(ROOT)} - another process holds it\n\n"
                f"  Usually Explorer browsing inside the zip, or the app still\n"
                f"  running from a folder opened through the zip view. Close it\n"
                f"  and re-run; the binary is already built, so this is quick.\n"
                f"  Or package elsewhere:  --out target/windows-2"
            )

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
