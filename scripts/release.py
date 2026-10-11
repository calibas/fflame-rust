#!/usr/bin/env python3
"""Release automation you invoke, not automation that invokes itself.

    python scripts/release.py check              # run every gate
    python scripts/release.py check --fix        # ...and regenerate what is stale
    python scripts/release.py changelog          # from git, since the last tag
    python scripts/release.py build              # every shipping artifact
    python scripts/release.py version 0.5.0      # bump every version together
    python scripts/release.py versions           # show them; exit 1 if they disagree
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

# Every file carrying the app version, the pattern that finds it, and
# how many matches are the version (1: the first; 0: all of them). Kept
# together because the release procedure bumps in lockstep — they are
# built from one commit and are not independently useful. The first
# entry is the app's own and is what the others are checked against.
#
# A manifest's version is its FIRST `version =` line only: a Cargo.toml
# can hold version strings further down (a `[dependencies.x]` table),
# and rewriting those would be a disaster wearing the costume of a
# version bump. python/Cargo.toml matters as much as pyproject.toml:
# pyfflame.__version__ is CARGO_PKG_VERSION, read from the Cargo.toml.
_MANIFEST_VERSION = re.compile(r'^(version\s*=\s*")([^"]+)(")', re.M)
VERSION_FILES = [
    ("Cargo.toml", _MANIFEST_VERSION, 1),
    ("python/Cargo.toml", _MANIFEST_VERSION, 1),
    ("python/pyproject.toml", _MANIFEST_VERSION, 1),
    ("wasm/render/Cargo.toml", _MANIFEST_VERSION, 1),
    ("wasm/script/Cargo.toml", _MANIFEST_VERSION, 1),
    ("wasm/flame/Cargo.toml", _MANIFEST_VERSION, 1),
    ("wasm/escape/Cargo.toml", _MANIFEST_VERSION, 1),
    ("wasm/sim/Cargo.toml", _MANIFEST_VERSION, 1),
    # The web app's cache-busters on css/ and js/ (every `?v=`): browsers
    # keep those through ordinary reloads, so a new release must change
    # the URL or a stale vkb.js silently downgrades the keyboard.
    ("index.html", re.compile(r'(\?v=)(\d+\.\d+\.\d+)()'), 0),
]

# Tracked files that look like they carry a version. Each must be in
# VERSION_FILES, so a crate added later cannot quietly fall out of the
# lockstep the way wasm/flame, wasm/escape, wasm/sim and python/ once did.
VERSION_GLOBS = ["*Cargo.toml", "*pyproject.toml"]

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


def run(cmd, capture=True, env=None, cwd=None):
    """Run a command from the repo root (or `cwd`). Returns (ok, output)."""
    full_env = {**os.environ, **(env or {})}
    try:
        p = subprocess.run(
            cmd, cwd=(ROOT / cwd) if cwd else ROOT,
            shell=isinstance(cmd, str), env=full_env,
            capture_output=capture, text=True,
        )
    except FileNotFoundError as e:
        return False, str(e)
    out = ((p.stdout or "") + (p.stderr or "")) if capture else ""
    return p.returncode == 0, out


# ----------------------------------------------------------------- gates

class Gate:
    """One thing that must pass, and how to repair it if it can be.

    `cmd` is a command to run, or a function returning (ok, output) for
    a gate that is cheaper to answer here than in a subprocess."""

    def __init__(self, name, cmd, fix=None, why="", cwd=None):
        self.name, self.cmd, self.fix, self.why = name, cmd, fix, why
        # Run from this directory instead of the repo root. The gallery
        # gates need it: cargo discovers .cargo/config.toml by current
        # directory, so `--manifest-path` from the root would miss
        # wasm/.cargo/config.toml and rebuild into the old per-crate
        # target directories at full cost.
        self.cwd = cwd


GATES = [
    # First because it takes no time: one version across every crate,
    # and every version-carrying manifest in the list that bumps them.
    Gate("versions agree", lambda: check_versions(),
         why="one version for everything (RELEASE.md §1); `release.py versions` lists them"),
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
    # machine with no GPU adapter.
    #
    # Run from the crate directory, NOT --manifest-path from the root:
    # that is how cargo finds wasm/.cargo/config.toml, which shares one
    # target directory between the gallery crates and drops the git
    # provenance that used to invalidate them on every commit. And
    # `--profile gallery-test`, not `--release`: the crates' own
    # [profile.release] is tuned to ship a small .wasm (opt-level="z",
    # fat LTO, one codegen unit), so testing under it spent minutes
    # optimizing the parent crate for download size. Measured on the
    # parent-crate-changed case: 178s -> 73s from the profile alone.
    Gate("gallery script parity",
         ["cargo", "test", "--profile", "gallery-test"],
         cwd="wasm/script",
         why="script + seed is a shareable artifact; drift redefines every published seed"),
    Gate("gallery renderer smoke",
         ["cargo", "test", "--profile", "gallery-test"],
         cwd="wasm/render",
         why="the device-reuse regression froze real galleries once already"),
    # The single-engine modules, which are also the engine feature
    # matrix. wasm/escape, wasm/flame and wasm/sim each link the
    # library with default-features off and ONE engine on, and a
    # `#[cfg]` that drifted off its item is invisible in every other
    # build. That is not hypothetical: `render_sim` and `register_sim`
    # both silently required the ESCAPE feature for months, because a
    # doc comment was inserted between an escape item's `#[cfg]` and
    # the item, leaving the attribute on the simulation item that
    # followed. Nothing failed until the first build with simulation
    # and no escape.
    #
    # These were `cargo check --lib --no-default-features --features
    # engine-X` from the root (~20 s each), which catches that bug
    # class but left the crates' own smoke tests running nowhere — and
    # those are the only check that the dropped engines really are gone
    # (the point of the crates) and that the module still renders. A
    # test build compiles the library at exactly the same feature set,
    # so it replaces the check rather than adding to it. The price is
    # codegen instead of a check: estimated from wasm/render's 73 s
    # above, about a minute more per crate when the parent crate
    # changed (not measured for these three). The smoke tests skip
    # without a GPU adapter.
    Gate("gallery sim smoke",
         ["cargo", "test", "--profile", "gallery-test"],
         cwd="wasm/sim",
         why="engine-sim alone: a cfg on the wrong item compiles everywhere else"),
    Gate("gallery escape smoke",
         ["cargo", "test", "--profile", "gallery-test"],
         cwd="wasm/escape",
         why="engine-escape alone, and the flame catalog must be gone"),
    Gate("gallery flame smoke",
         ["cargo", "test", "--profile", "gallery-test"],
         cwd="wasm/flame",
         why="engine-flame alone, and an escape config must be refused"),
]


def cmd_check(args):
    print(f"{BOLD}Gates{RESET}  {DIM}(nothing is fixed unless you pass --fix){RESET}\n")
    failed = []
    for g in GATES:
        t0 = time.time()
        ok, out = g.cmd() if callable(g.cmd) else run(g.cmd, cwd=g.cwd)
        secs = time.time() - t0

        if ok:
            print(f"  {GREEN}pass{RESET}  {g.name:<22} {DIM}{secs:5.1f}s{RESET}")
            continue

        if args.fix and g.fix:
            fix_cmd, fix_env = g.fix
            print(f"  {YELLOW}fix {RESET}  {g.name:<22} {DIM}regenerating…{RESET}")
            fixed, _ = run(fix_cmd, env=fix_env, cwd=g.cwd)
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

def _read(path):
    # Bytes, not text mode: text mode would turn a checkout's CRLF into
    # the platform's newline on the way back out, and a version bump
    # must change one string, not every line ending in the file.
    return path.read_bytes().decode("utf-8")


def versions_in(rel, pattern, count):
    """The version strings `rel` carries (empty if it has none), or
    None when the file is missing."""
    path = ROOT / rel
    if not path.exists():
        return None
    found = [m.group(2) for m in pattern.finditer(_read(path))]
    return found[:count] if count else found


def read_version():
    rel, pattern, count = VERSION_FILES[0]
    found = versions_in(rel, pattern, count)
    return found[0] if found else "unknown"


def unlisted_manifests():
    """Tracked manifests that VERSION_FILES does not list, or None when
    git cannot say (a source tarball)."""
    ok, out = run(["git", "ls-files", "--", *VERSION_GLOBS])
    if not ok:
        return None
    listed = {rel for rel, _, _ in VERSION_FILES}
    return [l.strip() for l in out.splitlines()
            if l.strip() and l.strip() not in listed]


def check_versions():
    """The `versions agree` gate. Returns (ok, report)."""
    want = read_version()
    lines, ok = [], True
    for rel, pattern, count in VERSION_FILES:
        found = versions_in(rel, pattern, count)
        if found is None:
            lines.append(f"{rel:<28} MISSING")
            ok = False
        elif not found:
            lines.append(f"{rel:<28} no version found")
            ok = False
        else:
            bad = sorted({v for v in found if v != want})
            lines.append(f"{rel:<28} {', '.join(sorted(set(found)))}"
                         + (f"   <- expected {want}" if bad else ""))
            ok = ok and not bad
    extra = unlisted_manifests()
    if extra is None:
        lines.append("(git unavailable: tracked manifests not checked against the list)")
    for rel in extra or []:
        lines.append(f"{rel:<28} not in release.py's VERSION_FILES, so `version` would skip it")
        ok = False
    return ok, "\n".join(lines)


def cmd_versions(args):
    ok, report = check_versions()
    print(f"{BOLD}Versions{RESET}  {DIM}(the first is the app's; every other must match){RESET}\n")
    print("\n".join("  " + l for l in report.split("\n")))
    if not ok:
        print(f"\n{RED}They disagree.{RESET} "
              f"{DIM}`release.py version {read_version()}` brings them into line.{RESET}")
        return 1
    print(f"\n{GREEN}All agree.{RESET}")
    return 0


def cmd_version(args):
    new = args.number
    if not re.fullmatch(r"\d+\.\d+\.\d+", new):
        print(f"{RED}`{new}` is not x.y.z{RESET}", file=sys.stderr)
        return 1

    print(f"{BOLD}Bumping every version to {new}{RESET}  "
          f"{DIM}(lockstep — see docs/RELEASE.md §1){RESET}\n")

    # Two passes: read and validate EVERY file, then write. One loop
    # that did both left the files before a bad one already rewritten —
    # half a bump, which is the state the lockstep exists to prevent.
    planned, failed = [], False
    for rel, pattern, count in VERSION_FILES:
        path = ROOT / rel
        if not path.exists():
            print(f"  {RED}FAIL{RESET}  {rel} {DIM}missing{RESET}")
            failed = True
            continue
        text = _read(path)
        found = versions_in(rel, pattern, count)
        if not found:
            print(f"  {RED}FAIL{RESET}  {rel} {DIM}no version found{RESET}")
            failed = True
            continue
        old = ", ".join(sorted(set(found)))
        planned.append((rel, path, pattern.sub(rf"\g<1>{new}\g<3>", text, count=count), old))
    extra = unlisted_manifests()
    for rel in extra or []:
        print(f"  {RED}FAIL{RESET}  {rel} {DIM}carries a version but is not in VERSION_FILES{RESET}")
        failed = True
    if failed:
        print(f"\n{RED}Nothing written.{RESET} {DIM}Fix the list above, then run it again.{RESET}")
        return 1

    for rel, path, text, old in planned:
        if args.dry_run:
            print(f"  {DIM}would set{RESET} {rel:<28} {old} -> {new}")
            continue
        path.write_bytes(text.encode("utf-8"))
        print(f"  {GREEN}set {RESET}  {rel:<28} {old} -> {new}")

    if not args.dry_run:
        print(f"\n{DIM}The Cargo.lock files update on the next build of each "
              f"crate. Commit them too.{RESET}")
    return 0


# ----------------------------------------------------------------- build

# (name, command, note, directory). The five gallery modules are
# five wasm-pack builds — the three single-engine ones are the same
# source as wasm/render with fewer engines, and each is a module of its
# own to ship. Each runs from its crate directory, where cargo finds
# wasm/.cargo/config.toml and its shared target directory.
BUILDS = [
    ("desktop (dist)", ["cargo", "build", "--profile", "dist", "--bin", "FractalArtEditor"],
     "~7-10 min: LTO, one codegen unit, stripped", None),
    ("web app", ["bash", "build-wasm.sh"], "wasm-bindgen -> pkg/", None),
    ("gallery: render", ["wasm-pack", "build", "--target", "web", "--release"],
     "in wasm/render", "wasm/render"),
    ("gallery: script", ["wasm-pack", "build", "--target", "web", "--release"],
     "in wasm/script", "wasm/script"),
    ("gallery: flame", ["wasm-pack", "build", "--target", "web", "--release"],
     "in wasm/flame", "wasm/flame"),
    ("gallery: escape", ["wasm-pack", "build", "--target", "web", "--release"],
     "in wasm/escape", "wasm/escape"),
    ("gallery: sim", ["wasm-pack", "build", "--target", "web", "--release"],
     "in wasm/sim", "wasm/sim"),
    ("python wheel", ["maturin", "build", "--release"], "in python/", "python"),
]

# Host-specific packaging. Separate from BUILDS because each CONSUMES
# the desktop build rather than being one, and neither can run on the
# other's platform. Ordering matters — they package whatever dist binary
# exists, so a stale one is silently shippable if they run first.
PACKAGE = {
    "darwin": ("macOS bundle", [sys.executable, "scripts/make_macos_app.py", "--zip"],
               "-> target/macos/*.app + .zip", None),
    "win32": ("Windows zip", [sys.executable, "scripts/make_windows_zip.py"],
              "-> target/windows/*.zip", None),
}


def cmd_build(args):
    print(f"{BOLD}Building every shipping surface{RESET}\n")
    builds = list(BUILDS)
    if sys.platform in PACKAGE:
        # After the desktop build, which it packages.
        builds.insert(1, PACKAGE[sys.platform])
    failed = []
    for name, cmd, note, sub in builds:
        if args.only and args.only not in name:
            continue
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

    vs = sub.add_parser("versions", help="list every version; exit 1 if they disagree")
    vs.set_defaults(fn=cmd_versions)

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
