#!/usr/bin/env python3
"""Write version.json beside a web build, so a deploy can be checked.

    python scripts/write_version_json.py DIR [--profile NAME]

DIR is a build's output directory (pkg/, wasm/render/pkg, wasm/script/pkg).
The file says which version, commit and profile the build came from:

    {"version": "0.6.0", "git_hash": "2f21e242", "git_dirty": false,
     "profile": "dist", "built": "2026-11-24T18:03:11Z"}

`release.py deploy-web` reads it back from the live site after an upload: a
page still serving the old one is a half-finished or cached upload, which a
person loading the page cannot tell from a working one
(docs/projects/release-streamlining.md, *Details*).

Called by build-wasm.sh / build-wasm.bat and by `release.py build` after each
gallery module. Standard library only.
"""

import argparse
import datetime
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def git(*args):
    p = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
    return p.stdout.strip() if p.returncode == 0 else None


def app_version():
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
    return m.group(1) if m else "unknown"


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir")
    ap.add_argument("--profile", default="release")
    args = ap.parse_args()

    out = Path(args.dir)
    if not out.is_absolute():
        out = ROOT / out
    if not out.is_dir():
        print(f"write_version_json: {out} is not a directory", file=sys.stderr)
        return 1
    status = git("status", "--porcelain", "--untracked-files=no")
    info = {
        "version": app_version(),
        "git_hash": git("rev-parse", "--short=8", "HEAD") or "unknown",
        "git_dirty": bool(status) if status is not None else None,
        "profile": args.profile,
        "built": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }
    (out / "version.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
    print(f"  {out.relative_to(ROOT) if out.is_relative_to(ROOT) else out}/version.json: "
          f"{info['version']} {info['git_hash']}{' (dirty)' if info['git_dirty'] else ''} {info['profile']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
