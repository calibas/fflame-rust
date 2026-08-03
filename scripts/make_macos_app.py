#!/usr/bin/env python3
"""Assemble the macOS .app bundle.

    python scripts/make_macos_app.py                 # from target/dist
    python scripts/make_macos_app.py --profile release
    python scripts/make_macos_app.py --zip           # ...and a distributable zip

macOS only. Reads the version from Cargo.toml so the bundle cannot drift
from the binary it wraps.

# Why a bundle at all, when Windows gets a zip

The icon, for one: macOS has no equivalent of the Windows resource table,
and reads an app's icon from `Contents/Info.plist` -> `CFBundleIconFile`.
A loose binary can never show one. A bundle also gives the app a name in
the menu bar and the Dock, and a stable identity (`CFBundleIdentifier`)
for preferences and window restoration.

# Where the resources go, and why it matters

`assets/` lands in `Contents/Resources/`, which is Apple's convention and
is also the third candidate `resources::resource_path` looks in. That is
load-bearing: Finder launches a bundled app with the working directory
set to `/`, so the repo-relative paths the code uses resolve to nothing
without it, and the app comes up silently missing palette packs, shipped
scripts and CJK fonts.

`shaders/` is deliberately NOT copied. Every shader is embedded in the
binary; the on-disk tree is a developer override only, so shipping it
would add half a megabyte that only serves to let a stray edit change
what a released app renders.

# Signing

Ad-hoc (`codesign -s -`), which needs no developer account. That is not
Gatekeeper approval and does not pretend to be: a user who downloads this
still has to clear quarantine by hand. It is here because the bundle is
modified after the linker signed the executable, and on Apple Silicon a
stale or absent signature can stop the app launching at all.

Real distribution needs a Developer ID and notarization. See
docs/RELEASE.md section 5.
"""

import argparse
import plistlib
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APP_NAME = "Fractal Art Editor"
BINARY = "FractalArtEditor"
BUNDLE_ID = "com.fractalsforall.fractalarteditor"
# The oldest macOS we claim to run on. wgpu's Metal backend and the
# window-management APIs winit uses are all comfortably older than this;
# 11.0 is the floor where Apple Silicon exists at all.
MIN_MACOS = "11.0"


def cargo_version() -> str:
    text = (ROOT / "Cargo.toml").read_text()
    m = re.search(r'^\s*version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if not m:
        sys.exit("could not read version from Cargo.toml")
    return m.group(1)


def build_bundle(profile: str, out_dir: Path) -> Path:
    exe = ROOT / "target" / profile / BINARY
    if not exe.exists():
        sys.exit(
            f"missing {exe.relative_to(ROOT)}\n"
            f"  build it first:  cargo build --profile {profile}"
        )

    icon = ROOT / "assets" / "branding" / "icon.icns"
    if not icon.exists():
        sys.exit(
            f"missing {icon.relative_to(ROOT)}\n"
            f"  generate it first:  python scripts/make_icons.py"
        )

    app = out_dir / f"{APP_NAME}.app"
    if app.exists():
        shutil.rmtree(app)
    macos = app / "Contents" / "MacOS"
    resources = app / "Contents" / "Resources"
    macos.mkdir(parents=True)
    resources.mkdir(parents=True)

    shutil.copy2(exe, macos / BINARY)
    shutil.copy2(icon, resources / "icon.icns")
    # See the module docstring: assets yes, shaders no.
    shutil.copytree(ROOT / "assets", resources / "assets")

    version = cargo_version()
    plist = {
        "CFBundleName": APP_NAME,
        "CFBundleDisplayName": APP_NAME,
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleExecutable": BINARY,
        # No extension: macOS appends .icns itself.
        "CFBundleIconFile": "icon",
        "CFBundlePackageType": "APPL",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": version,
        # Without this the window is drawn at 1x and upscaled — blurry on
        # every Mac made in the last decade.
        "NSHighResolutionCapable": True,
        "LSMinimumSystemVersion": MIN_MACOS,
        "NSHumanReadableCopyright": "Fractals for All",
    }
    with open(app / "Contents" / "Info.plist", "wb") as fp:
        plistlib.dump(plist, fp)

    if shutil.which("codesign"):
        result = subprocess.run(
            ["codesign", "--force", "--deep", "--sign", "-", str(app)],
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            print(f"  warning: ad-hoc signing failed: {result.stderr.strip()}",
                  file=sys.stderr)
        else:
            print("  ad-hoc signed")
    else:
        print("  warning: no codesign; the bundle may not launch on Apple Silicon",
              file=sys.stderr)

    return app


def main() -> int:
    if sys.platform != "darwin":
        sys.exit("macOS only (needs plistlib bundle layout + codesign)")

    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--profile", default="dist",
                    help="cargo profile to package (default: dist)")
    ap.add_argument("--out", default="target/macos",
                    help="output directory (default: target/macos)")
    ap.add_argument("--zip", action="store_true",
                    help="also write a distributable .zip beside the bundle")
    args = ap.parse_args()

    out_dir = ROOT / args.out
    out_dir.mkdir(parents=True, exist_ok=True)

    app = build_bundle(args.profile, out_dir)
    size = sum(f.stat().st_size for f in app.rglob("*") if f.is_file())
    print(f"  {app.relative_to(ROOT)}  ({size / 1e6:.0f} MB)")

    if args.zip:
        version = cargo_version()
        # NOT Path.with_suffix here: a version like 0.4.4 makes ".4-macos"
        # look like an extension, and the name comes out as "...-0.4.zip".
        zip_path = out_dir / f"{APP_NAME.replace(' ', '')}-{version}-macos.zip"
        # `ditto` preserves the bundle's structure and extended attributes;
        # `zip -r` and shutil.make_archive can both mangle a .app.
        if zip_path.exists():
            zip_path.unlink()
        subprocess.run(
            ["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent",
             str(app), str(zip_path)],
            check=True,
        )
        print(f"  {zip_path.relative_to(ROOT)}  "
              f"({zip_path.stat().st_size / 1e6:.0f} MB)")
        print()
        print("  Unsigned: a user who downloads this must clear quarantine —")
        print("  System Settings > Privacy & Security > 'Open Anyway', or")
        print("  xattr -d com.apple.quarantine '/Applications/{}.app'".format(APP_NAME))

    return 0


if __name__ == "__main__":
    sys.exit(main())
