#!/usr/bin/env python3
"""Release automation you invoke, not automation that invokes itself.

    python scripts/release.py check              # run every gate
    python scripts/release.py check --fix        # ...and regenerate what is stale
    python scripts/release.py changelog          # from git, since the last tag
    python scripts/release.py build              # every shipping artifact
    python scripts/release.py version 0.5.0      # bump all four crates together
    python scripts/release.py                    # check, then changelog preview

Nothing here runs on push, on commit, or on a timer. That is the point:
the work CI would do is worth automating, the *triggering* is what gets
in the way. Run it when you are releasing, or when you want to know
whether you could.

Exit codes: 0 all good, 1 a gate failed. So it can be wired into a hook
later if that ever seems worth it, without being one now.
"""

import argparse
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Every crate carrying a version, and the line that holds it. Kept
# together because the release procedure recommends bumping in lockstep
# — they are built from one commit and are not independently useful.
VERSION_FILES = [
    ("Cargo.toml", re.compile(r'^(version\s*=\s*")([^"]+)(")', re.M)),
    ("python/pyproject.toml", re.compile(r'^(version\s*=\s*")([^"]+)(")', re.M)),
    ("wasm/render/Cargo.toml", re.compile(r'^(version\s*=\s*")([^"]+)(")', re.M)),
    ("wasm/script/Cargo.toml", re.compile(r'^(version\s*=\s*")([^"]+)(")', re.M)),
]

# `name: subject` — 141 of the last 150 commits use it, so a changelog
# can be grouped by it. The rest fall into "Other", which is honest:
# they are mostly merge commits and a few older subjects.
SUBJECT = re.compile(r"^(?P<area>[a-z][a-z0-9/_-]*): (?P<text>.+)$")

# Areas nobody reading a changelog cares about, and why. Not dropped
# silently — `--all` shows them.
INTERNAL = {"docs", "tools", "benchmarks", "tests", "chore", "ci"}

# The Windows console defaults to cp1252, where an em-dash is a fatal
# UnicodeEncodeError rather than a mangled character. This file's prose
# is full of them, so fix the stream once instead of policing the text.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")
    except (AttributeError, ValueError):
        pass  # already utf-8, or not a real stream (piped, captured)

# Colour only when a terminal is watching. Piping into a file or a pager
# should not fill it with escape codes.
if sys.stdout.isatty() and os.environ.get("TERM") != "dumb":
    RESET, BOLD, RED, GREEN, YELLOW, DIM = (
        "\033[0m", "\033[1m", "\033[31m", "\033[32m", "\033[33m", "\033[2m"
    )
else:
    RESET = BOLD = RED = GREEN = YELLOW = DIM = ""


def run(cmd, capture=True, env=None):
    """Run a command from the repo root. Returns (ok, output)."""
    full_env = {**os.environ, **(env or {})}
    try:
        p = subprocess.run(
            cmd, cwd=ROOT, shell=isinstance(cmd, str), env=full_env,
            capture_output=capture, text=True,
        )
    except FileNotFoundError as e:
        return False, str(e)
    out = ((p.stdout or "") + (p.stderr or "")) if capture else ""
    return p.returncode == 0, out


# ----------------------------------------------------------------- gates

class Gate:
    """One thing that must pass, and how to repair it if it can be."""

    def __init__(self, name, cmd, fix=None, why=""):
        self.name, self.cmd, self.fix, self.why = name, cmd, fix, why


GATES = [
    Gate("unit tests", ["cargo", "test", "--release", "--lib"],
         why="the suite"),
    Gate("wasm builds", ["cargo", "check", "--release",
                         "--target", "wasm32-unknown-unknown", "--lib"],
         why="the web app is a shipping surface, and only this catches it"),
    Gate("generated contract",
         ["cargo", "test", "--release", "--lib", "contract_is_current"],
         fix=(["cargo", "test", "--release", "--lib", "contract_is_current"],
              {"UPDATE_CONTRACT": "1"}),
         why="the API reads docs/generated/engine-contract.json"),
    Gate("shader dumps",
         ["cargo", "test", "--release", "--lib", "canonical_shader_dumps"],
         fix=(["cargo", "test", "--release", "--lib", "canonical_shader_dumps"],
              {"UPDATE_SHADER_DUMPS": "1"}),
         why="generated WGSL changed"),
    Gate("doc links", [sys.executable, "scripts/check_doc_links.py"],
         why="live docs pointing nowhere"),
    # The gallery crates. wasm/script's CLI-parity fixtures are the
    # guard for "script + seed reproduces a flame byte-for-byte" — the
    # public determinism promise — and until now ran in no automated
    # step (RELEASE.md §5 said "run them by hand until that is fixed";
    # this is the fix). wasm/render's smoke tests skip cleanly on a
    # machine with no GPU adapter. First run pays each crate's compile;
    # cached after.
    Gate("gallery script parity",
         ["cargo", "test", "--release",
          "--manifest-path", "wasm/script/Cargo.toml"],
         why="script + seed is a shareable artifact; drift redefines every published seed"),
    Gate("gallery renderer smoke",
         ["cargo", "test", "--release",
          "--manifest-path", "wasm/render/Cargo.toml"],
         why="the device-reuse regression froze real galleries once already"),
]


def cmd_check(args):
    print(f"{BOLD}Gates{RESET}  {DIM}(nothing is fixed unless you pass --fix){RESET}\n")
    failed = []
    for g in GATES:
        t0 = time.time()
        ok, out = run(g.cmd)
        secs = time.time() - t0

        if ok:
            print(f"  {GREEN}pass{RESET}  {g.name:<22} {DIM}{secs:5.1f}s{RESET}")
            continue

        if args.fix and g.fix:
            fix_cmd, fix_env = g.fix
            print(f"  {YELLOW}fix {RESET}  {g.name:<22} {DIM}regenerating…{RESET}")
            fixed, _ = run(fix_cmd, env=fix_env)
            if fixed:
                # Regenerating is not the same as being right. Say so —
                # a generated file that changed is a diff somebody has
                # to read, which is the entire reason these gates exist
                # rather than the files just being rebuilt every time.
                print(f"        {YELLOW}regenerated — READ THE DIFF before committing{RESET}")
                continue
            print(f"        {RED}could not regenerate{RESET}")

        print(f"  {RED}FAIL{RESET}  {g.name:<22} {DIM}{g.why}{RESET}")
        failed.append((g, out))

    if failed:
        print(f"\n{RED}{BOLD}{len(failed)} gate(s) failed{RESET}")
        for g, out in failed:
            print(f"\n{BOLD}── {g.name}{RESET}")
            tail = [l for l in out.strip().split("\n") if l.strip()][-15:]
            print("\n".join("   " + l for l in tail))
        return 1

    print(f"\n{GREEN}{BOLD}All gates pass.{RESET}")
    print(f"{DIM}Not covered here — they need a GPU and a few minutes:{RESET}")
    print(f"{DIM}  cargo build --release && python tests/visual/run_tests.py{RESET}")
    print(f"{DIM}  python scripts/run_benchmarks.py --quick{RESET}")
    print(f"{DIM}  cargo run --release --bin variation_probe   "
          f"(regenerate if variation math changed){RESET}")
    return 0


# ------------------------------------------------------------- changelog

def last_tag():
    ok, out = run(["git", "describe", "--tags", "--abbrev=0"])
    return out.strip() if ok and out.strip() else None


def cmd_changelog(args):
    since = args.since or last_tag()
    rng = f"{since}..HEAD" if since else "HEAD"
    ok, out = run(["git", "log", "--format=%H%x00%s", rng])
    if not ok:
        print(f"{RED}git log failed:{RESET} {out}", file=sys.stderr)
        return 1

    groups, other = {}, []
    for line in out.strip().split("\n"):
        if not line.strip():
            continue
        sha, _, subject = line.partition("\0")
        if subject.startswith("Merge "):
            continue  # a merge says nothing a reader wants
        m = SUBJECT.match(subject)
        if m:
            groups.setdefault(m["area"], []).append((sha[:8], m["text"]))
        else:
            other.append((sha[:8], subject))

    if not args.all:
        hidden = {a: v for a, v in groups.items() if a in INTERNAL}
        groups = {a: v for a, v in groups.items() if a not in INTERNAL}
    else:
        hidden = {}

    version = read_version()
    header = f"## {version}" + (f" — since {since}" if since else " — first release")
    lines = [header, ""]

    for area in sorted(groups):
        lines.append(f"### {area}")
        for sha, text in groups[area]:
            lines.append(f"- {text} ({sha})")
        lines.append("")

    if other and args.all:
        lines.append("### other")
        for sha, text in other:
            lines.append(f"- {text} ({sha})")
        lines.append("")

    print("\n".join(lines))

    n = sum(len(v) for v in groups.values())
    skipped = sum(len(v) for v in hidden.values()) + (0 if args.all else len(other))
    print(f"{DIM}# {n} entries"
          + (f", {skipped} internal/unparsed hidden — --all to see them" if skipped else "")
          + f"{RESET}", file=sys.stderr)
    if not since:
        print(f"{DIM}# No tags yet, so this is the whole history. "
              f"Tag a release and the next run is the diff.{RESET}", file=sys.stderr)
    return 0


# --------------------------------------------------------------- version

def read_version():
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    m = VERSION_FILES[0][1].search(text)
    return m.group(2) if m else "unknown"


def cmd_version(args):
    new = args.number
    if not re.fullmatch(r"\d+\.\d+\.\d+", new):
        print(f"{RED}`{new}` is not x.y.z{RESET}", file=sys.stderr)
        return 1

    print(f"{BOLD}Bumping every crate to {new}{RESET}  "
          f"{DIM}(lockstep — see docs/RELEASE.md §1){RESET}\n")
    for rel, pattern in VERSION_FILES:
        path = ROOT / rel
        if not path.exists():
            print(f"  {YELLOW}skip{RESET}  {rel} {DIM}(missing){RESET}")
            continue
        text = path.read_text(encoding="utf-8")
        m = pattern.search(text)
        if not m:
            print(f"  {RED}FAIL{RESET}  {rel} {DIM}no version line{RESET}")
            return 1
        old = m.group(2)
        if args.dry_run:
            print(f"  {DIM}would set{RESET} {rel:<28} {old} -> {new}")
            continue
        # Only the FIRST match: a Cargo.toml's `[dependencies]` can hold
        # version strings too, and rewriting those would be a disaster
        # wearing the costume of a version bump.
        path.write_text(pattern.sub(rf"\g<1>{new}\g<3>", text, count=1), encoding="utf-8")
        print(f"  {GREEN}set {RESET}  {rel:<28} {old} -> {new}")

    if not args.dry_run:
        print(f"\n{DIM}Cargo.lock updates on the next build. "
              f"Commit both.{RESET}")
    return 0


# ----------------------------------------------------------------- build

BUILDS = [
    ("desktop (dist)", ["cargo", "build", "--profile", "dist", "--bin", "FractalArtEditor"],
     "~7-10 min: LTO, one codegen unit, stripped"),
    ("web app", ["bash", "build-wasm.sh"], "wasm-bindgen -> pkg/"),
    ("gallery: render", ["wasm-pack", "build", "--target", "web", "--release"],
     "in wasm/render"),
    ("gallery: script", ["wasm-pack", "build", "--target", "web", "--release"],
     "in wasm/script"),
    ("python wheel", ["maturin", "build", "--release"], "in python/"),
]

# Host-specific packaging. Separate from BUILDS because each CONSUMES
# the desktop build rather than being one, and neither can run on the
# other's platform. Ordering matters — they package whatever dist binary
# exists, so a stale one is silently shippable if they run first.
PACKAGE = {
    "darwin": ("macOS bundle", [sys.executable, "scripts/make_macos_app.py", "--zip"],
               "-> target/macos/*.app + .zip"),
    "win32": ("Windows zip", [sys.executable, "scripts/make_windows_zip.py"],
              "-> target/windows/*.zip"),
}


def cmd_build(args):
    print(f"{BOLD}Building every shipping surface{RESET}\n")
    cwds = {"gallery: render": "wasm/render",
            "gallery: script": "wasm/script",
            "python wheel": "python"}
    builds = list(BUILDS)
    if sys.platform in PACKAGE:
        # After the desktop build, which it packages.
        builds.insert(1, PACKAGE[sys.platform])
    failed = []
    for name, cmd, note in builds:
        if args.only and args.only not in name:
            continue
        sub = cwds.get(name)
        print(f"  {DIM}{note}{RESET}")
        t0 = time.time()
        p = subprocess.run(cmd, cwd=ROOT / sub if sub else ROOT,
                           capture_output=True, text=True)
        secs = time.time() - t0
        if p.returncode == 0:
            print(f"  {GREEN}built{RESET} {name:<20} {DIM}{secs:6.1f}s{RESET}\n")
        else:
            print(f"  {RED}FAIL {RESET} {name:<20} {DIM}{secs:6.1f}s{RESET}")
            tail = (p.stdout + p.stderr).strip().split("\n")[-8:]
            print("\n".join("        " + l for l in tail) + "\n")
            failed.append(name)

    if failed:
        print(f"{RED}{BOLD}{len(failed)} build(s) failed:{RESET} {', '.join(failed)}")
        return 1
    print(f"{GREEN}{BOLD}All surfaces built.{RESET}")
    print(f"{DIM}Building is not shipping — smoke-test each one "
          f"(docs/RELEASE.md §4.5).{RESET}")
    return 0


def main():
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd")

    c = sub.add_parser("check", help="run every gate")
    c.add_argument("--fix", action="store_true",
                   help="regenerate stale generated files (still read the diff)")
    c.set_defaults(fn=cmd_check)

    g = sub.add_parser("changelog", help="generate from git history")
    g.add_argument("--since", help="tag or ref (default: last tag)")
    g.add_argument("--all", action="store_true", help="include docs/tools/merges")
    g.set_defaults(fn=cmd_changelog)

    v = sub.add_parser("version", help="bump every crate in lockstep")
    v.add_argument("number")
    v.add_argument("--dry-run", action="store_true")
    v.set_defaults(fn=cmd_version)

    b = sub.add_parser("build", help="build every shipping surface")
    b.add_argument("--only", help="substring of one surface's name")
    b.set_defaults(fn=cmd_build)

    args = ap.parse_args()
    if not args.cmd:
        # Bare invocation: the question "could I release right now?"
        rc = cmd_check(argparse.Namespace(fix=False))
        print()
        cmd_changelog(argparse.Namespace(since=None, all=False))
        return rc
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
