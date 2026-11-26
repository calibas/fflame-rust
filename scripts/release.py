#!/usr/bin/env python3
"""Release automation you invoke, not automation that invokes itself.

    python scripts/release.py preflight          # three repositories ready, tools reachable
    python scripts/release.py check              # run every fast gate
    python scripts/release.py check --fix        # ...and regenerate what is stale
    python scripts/release.py check --full       # ...plus the visual suite and the API integration test
    python scripts/release.py gallery            # the gallery modules' own tests, before they deploy
    python scripts/release.py changelog          # from git, since the last tag
    python scripts/release.py notes              # the changelog into NOTES.md, for editing
    python scripts/release.py sync-api           # the generated files into ../ffapi, its tests
    python scripts/release.py sync-web           # the web builds + editor page into ../ffbrowser
    python scripts/release.py deploy-api         # build and restart the API on the server; curated sync
    python scripts/release.py deploy-web         # upload the web builds, build the site, check it
    python scripts/release.py publish            # tag vX.Y.Z, GitHub release with the zips here
    python scripts/release.py ship 0.6.0         # all of it in order; --from STAGE resumes; --dry-run
    python scripts/release.py build              # every shipping artifact
    python scripts/release.py version 0.5.0      # bump every version together
    python scripts/release.py versions           # show them; exit 1 if they disagree
    python scripts/release.py                    # check, then changelog preview

The stages and their order are docs/projects/release-streamlining.md.
Machine-local settings live in an untracked release.local.toml beside
this repository's Cargo.toml, never in the docs:

    [repos]                      # only if they are not beside this one
    ffapi = "c:/projects/ffapi"
    ffbrowser = "c:/projects/ffbrowser"

    [server]
    ssh = "user@host"            # what `ssh` and the deploy stages connect to
    key = "~/.ssh/id_ed25519"    # optional: ssh -i
    site = "https://example.com" # the public site, to check what it serves
    ffbrowser_dir = "/home/user/ffbrowser"   # the server's ffbrowser checkout
    ffapi_dir = "/home/user/ffapi"           # the server's ffapi checkout
    api_port = 3000              # optional: where /health answers on the server

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
# version bump. pyfflame carries three: its engine module's crate, the
# wheel's (pyproject.toml) and the package's own `__version__`.
_MANIFEST_VERSION = re.compile(r'^(version\s*=\s*")([^"]+)(")', re.M)
VERSION_FILES = [
    ("Cargo.toml", _MANIFEST_VERSION, 1),
    ("wasm/python/Cargo.toml", _MANIFEST_VERSION, 1),
    ("python/pyproject.toml", _MANIFEST_VERSION, 1),
    ("python/pyfflame/__init__.py", re.compile(r'^(__version__\s*=\s*")([^"]+)(")', re.M), 1),
    ("wasm/render/Cargo.toml", _MANIFEST_VERSION, 1),
    ("wasm/script/Cargo.toml", _MANIFEST_VERSION, 1),
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
    Gate("generated catalog",
         ["cargo", "test", "--release", "--lib", "catalog_is_current"],
         fix=(["cargo", "test", "--release", "--lib", "catalog_is_current"],
              {"UPDATE_CATALOG": "1"}),
         why="the API summarises configs from docs/generated/engine-catalog.json"),
    Gate("shader dumps",
         ["cargo", "test", "--release", "--lib", "canonical_shader_dumps"],
         fix=(["cargo", "test", "--release", "--lib", "canonical_shader_dumps"],
              {"UPDATE_SHADER_DUMPS": "1"}),
         why="generated WGSL changed"),
    Gate("doc links", [sys.executable, "scripts/check_doc_links.py"],
         why="live docs pointing nowhere"),
    # The gallery modules: one check of both crates (render and script,
    # every engine and no web app or terrain), the build the library
    # gets nowhere else.
    #
    # A check, not the crates' test runs: those compile the library with
    # codegen (2026-11-03: script parity 125s, renderer 12s).
    # Incremental, a check is 14-15s after an edit and 3s with none.
    # What their runs asserted is kept where it costs nothing:
    # - script + seed reproducing the CLI's flames: the main suite
    #   compares the same fixtures (`shipped_scripts_give_the_clis_
    #   flames_for_their_seeds`);
    # - the rest -- the renderer drawing every engine on a GPU, its
    #   clamps and device reuse, the script module's parameters and
    #   envelope -- runs before the modules deploy: `release.py gallery`.
    Gate("gallery modules", lambda: gallery_compiles(),
         why="the library without the web app or terrain: a cfg on the wrong item compiles everywhere else"),
    # pyfflame's engine: the one WASI build, with code (`browser` versus
    # wasm32 cfgs, its storage and fetch stubs) nothing else compiles.
    # The PyO3 crate it replaced fell out of every gate and stopped
    # compiling unnoticed. The check runs from the crate's directory,
    # whose .cargo/config.toml names the target.
    Gate("python module (WASI)", ["cargo", "check"], cwd="wasm/python",
         why="pyfflame's engine; needs `rustup target add wasm32-wasip1`"),
]

# The gallery crates, each a workspace of its own sharing
# wasm/target-gallery.
GALLERY_CRATES = ["render", "script"]


def gallery_compiles():
    """Every gallery crate and its tests compile, at its own features.

    Run from the crate directory, NOT --manifest-path from the root: that
    is how cargo finds wasm/.cargo/config.toml, which shares one target
    directory between the gallery crates and drops the git provenance
    that used to invalidate them on every commit. Under `gallery-test`,
    the profile the tests build with, and incremental -- the profile
    inherits release, which turns it off, and a check without it redoes
    the library's whole front end per crate (50s after an edit, against
    14-15s with it)."""
    for crate in GALLERY_CRATES:
        ok, out = run(["cargo", "check", "--profile", "gallery-test", "--tests"],
                      env={"CARGO_INCREMENTAL": "1"}, cwd=f"wasm/{crate}")
        if not ok:
            return False, f"wasm/{crate}:\n{out}"
    return True, ""


# The gallery modules' own tests, run before they deploy (`release.py
# gallery`), not in every check: each compiles the library at its
# module's feature set with codegen, a minute or two a crate. wasm/script's
# CLI parity covers the module's parameters and envelope; wasm/render's
# smoke renders every engine (a Flame Attractor that needs the catalog
# among them), its clamps and device reuse (a regression froze real
# galleries once). The GPU tests skip without an adapter. `--profile gallery-test`, not `--release`: the crates' own
# [profile.release] is tuned to ship a small .wasm (opt-level="z", fat
# LTO, one codegen unit), and testing under it spent minutes optimizing
# the parent crate for download size (178s -> 73s from the profile).
GALLERY_TESTS = [
    Gate("gallery script parity", ["cargo", "test", "--profile", "gallery-test"], cwd="wasm/script",
         why="script + seed is a shareable artifact; drift redefines every published seed"),
    Gate("gallery renderer smoke", ["cargo", "test", "--profile", "gallery-test"], cwd="wasm/render",
         why="the device-reuse regression froze real galleries once already"),
    # pyfflame: its module built and packaged, then its tests -- the old
    # PyO3 package's, which the WASI one passes unchanged, and rendering.
    Gate("python package", lambda: python_tests(),
         why="pyfflame's API and renders; GPU tests skip without an adapter"),
]


def python_tests():
    ok, out = run([sys.executable, "python/build.py"])
    if not ok:
        return False, out
    return run([sys.executable, "-m", "pytest", "-q", "tests"], cwd="python")


# The release's own gates (`check --full`): minutes each, and each needs
# something the fast gates do not -- a GPU and a closed editor for the
# visual suite, a local ffapi checkout and its database for the API test.
# Out of the plain `check` so it stays a thing to run between commits.
RELEASE_GATES = [
    Gate("visual suite", lambda: visual_suite(),
         why="~375 renders against the baselines; `python tests/visual/run_tests.py` for the report"),
    Gate("API integration", lambda: run([sys.executable, "scripts/api_integration.py",
                                         "--ffapi", str(sibling("ffapi"))]),
         why="the client's calls against a local ffapi (scripts/api_integration.py)"),
]


def visual_suite():
    """Build the binary the suite renders with, then run it. The suite
    does not build, and a running editor locks the binary, which turns a
    failed build into a stale one being measured."""
    if editor_running():
        return False, "FractalArtEditor is running: it locks the binary the suite renders with"
    ok, out = run(["cargo", "build", "--release", "--bin", "FractalArtEditor"])
    if not ok:
        return False, out
    return run([sys.executable, "tests/visual/run_tests.py"])


def cmd_check(args):
    print(f"{BOLD}Gates{RESET}  {DIM}(nothing is fixed unless you pass --fix){RESET}\n")
    gates = GATES + (RELEASE_GATES if args.full else [])
    if run_gates(gates, args):
        return 1
    print(f"\n{GREEN}{BOLD}All gates pass.{RESET}")
    print(f"{DIM}Not covered here — they need a GPU and a few minutes:{RESET}")
    if not args.full:
        print(f"{DIM}  release.py check --full   (the visual suite and the API integration test){RESET}")
    print(f"{DIM}  python scripts/run_benchmarks.py --quick{RESET}")
    print(f"{DIM}  cargo run --release --bin variation_probe   "
          f"(regenerate if variation math changed){RESET}")
    print(f"{DIM}  python scripts/release.py gallery   (before the gallery modules deploy){RESET}")
    return 0


def cmd_gallery(args):
    print(f"{BOLD}Gallery modules' tests{RESET}  {DIM}(before they deploy){RESET}\n")
    if run_gates(GALLERY_TESTS, args):
        return 1
    print(f"\n{GREEN}{BOLD}All gallery tests pass.{RESET}")
    return 0


def run_gates(gates, args):
    """Run `gates` in order, fixing what `--fix` may; print each, and the
    failures' output. Whether any failed."""
    failed = []
    for g in gates:
        t0 = time.time()
        ok, out = g.cmd() if callable(g.cmd) else run(g.cmd, cwd=g.cwd)
        secs = time.time() - t0

        if ok is None:
            # Not applicable here (a setting not made, a tool this
            # platform lacks): said, and not a failure.
            print(f"  {YELLOW}skip{RESET}  {g.name:<22} {DIM}{out}{RESET}")
            continue
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
            lines = [l for l in out.strip().split("\n") if l.strip()]
            # A test's panic says why; the tail is cargo's summary of
            # which binary failed. So the panics first, cut to fit.
            for i, l in enumerate(lines):
                if "panicked at" in l:
                    for p in lines[i:i + 2]:
                        print("   " + (p if len(p) <= 400 else p[:400] + " …"))
            print("\n".join("   " + l for l in lines[-15:]))
        return True
    return False


# ------------------------------------------------------------- changelog

def last_tag():
    ok, out = run(["git", "describe", "--tags", "--abbrev=0"])
    return out.strip() if ok and out.strip() else None


def cmd_changelog(args):
    text, notes = changelog(args.since, args.all)
    if text is None:
        print(f"{RED}git log failed:{RESET} {notes[0]}", file=sys.stderr)
        return 1
    print(text)
    for n in notes:
        print(f"{DIM}# {n}{RESET}", file=sys.stderr)
    return 0


def cmd_notes(args):
    """The changelog, written to NOTES.md for editing (RELEASE.md §4.1).
    It is hand-edited, so an existing one is never overwritten unasked."""
    path = ROOT / "NOTES.md"
    if path.exists() and not args.force:
        print(f"{RED}NOTES.md exists{RESET} {DIM}(it may hold your edits): "
              f"--force to replace it{RESET}", file=sys.stderr)
        return 1
    text, notes = changelog(args.since, args.all)
    if text is None:
        print(f"{RED}git log failed:{RESET} {notes[0]}", file=sys.stderr)
        return 1
    path.write_bytes(text.encode("utf-8"))
    print(f"{GREEN}wrote{RESET} NOTES.md")
    for n in notes:
        print(f"{DIM}  {n}{RESET}")
    print(f"{DIM}  Edit it down to what a user would notice; it is the GitHub release's notes "
          f"and is not committed.{RESET}")
    return 0


def changelog(since=None, show_all=False):
    """(markdown, notes about it), or (None, [error]) when git fails."""
    since = since or last_tag()
    rng = f"{since}..HEAD" if since else "HEAD"
    ok, out = run(["git", "log", "--format=%H%x00%s", rng])
    if not ok:
        return None, [out]

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

    if not show_all:
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

    if other and show_all:
        lines.append("### other")
        for sha, text in other:
            lines.append(f"- {text} ({sha})")
        lines.append("")

    n = sum(len(v) for v in groups.values())
    skipped = sum(len(v) for v in hidden.values()) + (0 if show_all else len(other))
    notes = [f"{n} entries"
             + (f", {skipped} internal/unparsed hidden — --all to see them" if skipped else "")]
    if not since:
        notes.append("No tags yet, so this is the whole history. "
                     "Tag a release and the next run is the diff.")
    return "\n".join(lines), notes


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


# ------------------------------------------------- settings and siblings

LOCAL_CONFIG = ROOT / "release.local.toml"


def local_config():
    """release.local.toml as a dict ({} when there is none). Untracked:
    the server's address and anything else that is this machine's."""
    if not LOCAL_CONFIG.exists():
        return {}
    import tomllib
    with open(LOCAL_CONFIG, "rb") as f:
        return tomllib.load(f)


def sibling(name):
    """Another repository of the release, beside this one unless
    release.local.toml's [repos] says where."""
    where = local_config().get("repos", {}).get(name)
    return Path(where).resolve() if where else ROOT.parent / name


SIBLINGS = ["ffapi", "ffbrowser"]


def server():
    """release.local.toml's [server]: where the deploy stages go."""
    return local_config().get("server", {})


def missing_settings(*keys):
    """The [server] keys a stage needs that release.local.toml lacks, as
    an error message, or None."""
    lacking = [k for k in keys if not server().get(k)]
    if not lacking:
        return None
    where = LOCAL_CONFIG.relative_to(ROOT)
    return f"{where} has no [server] {', '.join(lacking)} (its format: release.py's header)"


def ssh_cmd(remote, tty=False):
    """`ssh` to the server running `remote` (one shell string). With
    `tty`, a terminal for a password prompt (sudo); otherwise BatchMode,
    so a missing key fails rather than waiting for a password."""
    s = server()
    cmd = ["ssh", "-o", "ConnectTimeout=15"]
    if s.get("key"):
        cmd += ["-i", os.path.expanduser(s["key"])]
    cmd += ["-t"] if tty else ["-o", "BatchMode=yes"]
    return cmd + [s["ssh"], remote]


def remote(script, tty=False, capture=False, stdin=None):
    """Run `script` on the server. Streams to this terminal unless
    `capture`. (ok, output)."""
    cmd = ssh_cmd(script, tty=tty)
    p = subprocess.run(cmd, input=stdin, capture_output=capture or stdin is not None)
    out = b"" if not (capture or stdin is not None) else (p.stdout or b"") + (p.stderr or b"")
    return p.returncode == 0, out.decode("utf-8", errors="replace")


def confirm(lines):
    """Say what is about to happen and ask. Only `yes` goes on."""
    print(f"\n{BOLD}About to:{RESET}")
    for l in lines:
        print(f"  - {l}")
    try:
        answer = input(f"\nType {BOLD}yes{RESET} to go on: ")
    except EOFError:
        answer = ""
    if answer.strip().lower() != "yes":
        print(f"{YELLOW}Stopped; nothing was done.{RESET}")
        return False
    return True


def fetch_url(url):
    """(status, body bytes) for a GET, past any cache (a fresh query)."""
    import urllib.error
    import urllib.request
    sep = "&" if "?" in url else "?"
    req = urllib.request.Request(f"{url}{sep}t={int(time.time())}",
                                 headers={"User-Agent": "release.py", "Cache-Control": "no-cache"})
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()
    except OSError as e:
        return None, str(e).encode()


def editor_running():
    """Whether a FractalArtEditor is running: on Windows it locks the
    binary a release build writes and the visual suite renders with."""
    if sys.platform == "win32":
        ok, out = run(["tasklist", "/FI", "IMAGENAME eq FractalArtEditor.exe", "/NH"])
        return ok and "FractalArtEditor.exe" in out
    ok, _ = run(["pgrep", "-x", "FractalArtEditor"])
    return ok


# ------------------------------------------------------------- preflight

def repo_ready(path, fetch):
    """A repository a release can be built from: on main, nothing
    uncommitted or untracked, level with origin/main. (ok, the problems)."""
    if not (path / ".git").exists():
        return False, f"{path}: not a git repository ([repos] in release.local.toml says where)"
    problems = []
    _, branch = run(["git", "branch", "--show-current"], cwd=path)
    if branch.strip() != "main":
        problems.append(f"on `{branch.strip() or '(detached HEAD)'}`, not main")
    _, status = run(["git", "status", "--porcelain"], cwd=path)
    dirty = [l for l in status.splitlines() if l.strip()]
    if dirty:
        problems.append(f"{len(dirty)} uncommitted or untracked:")
        problems += ["  " + l for l in dirty[:8]] + (["  ..."] if len(dirty) > 8 else [])
    if fetch:
        ok, out = run(["git", "fetch", "--quiet", "origin"], cwd=path)
        if not ok:
            problems.append(f"git fetch failed: {out.strip()}")
    ok, out = run(["git", "rev-list", "--left-right", "--count", "HEAD...origin/main"], cwd=path)
    if ok:
        ahead, behind = (int(n) for n in out.split())
        if ahead:
            problems.append(f"{ahead} commit(s) not pushed")
        if behind:
            problems.append(f"{behind} commit(s) behind origin/main")
    else:
        problems.append("no origin/main to compare with")
    return not problems, "\n".join(problems)


def ssh_reachable():
    if not server().get("ssh"):
        return None, "no [server] ssh in release.local.toml (the deploy stages need it)"
    return run(ssh_cmd("true"))


def cmd_preflight(args):
    print(f"{BOLD}Preflight{RESET}  {DIM}(nothing is changed but the remote-tracking "
          f"refs a fetch updates){RESET}\n")
    fetch = not args.no_fetch
    gates = [Gate(f"{name} ready", lambda p=path: repo_ready(p, fetch),
                  why="clean, on main, level with origin")
             for name, path in [("fflame-rust", ROOT)] + [(n, sibling(n)) for n in SIBLINGS]]
    gates += [
        Gate("versions agree", lambda: check_versions(),
             why="`release.py versions` lists them"),
        Gate("editor closed", lambda: (False, "close FractalArtEditor") if editor_running() else (True, ""),
             why="a running editor locks the binary the build and the visual suite use"),
        Gate("gh signed in", ["gh", "auth", "status"],
             why="`publish` tags and creates the GitHub release with gh"),
        Gate("server reachable", lambda: ssh_reachable(),
             why="the deploy stages run over ssh (BatchMode: a key, no password prompt)"),
    ]
    if run_gates(gates, argparse.Namespace(fix=False)):
        return 1
    print(f"\n{GREEN}{BOLD}Ready.{RESET}")
    return 0


# -------------------------------------------------------------- sync-api

# What the API copies from here (RELEASE.md §3): the file, and the
# binary that writes it, or None for one committed here and kept current
# by a gate. They land in ffapi's docs/generated/ under the same name.
SYNC_FILES = [
    ("docs/generated/engine-contract.json", None),
    ("docs/generated/engine-catalog.json", None),
    ("output/variations-corpus.json", "export_variations_json"),
    ("output/scripts-corpus.json", "export_scripts_json"),
    ("output/effects-corpus.json", "export_effects_json"),
]

# Not copied: the curated manifest is posted to a server, by ffapi's
# tools/sync_curated.py, at deploy time. Written here so it is current.
CURATED_MANIFEST = ("output/curated-manifest.json", "export_curated_json")

# ffapi's tests that read the copied files. The first and the last open
# the database in ffapi's .env (its test database).
FFAPI_TESTS = ["contract_conformance", "catalog_conformance", "corpus_preflight", "curated_sync"]


def _same_text(a, b):
    """Equal but for line endings: ffapi checks these out CRLF on Windows
    (core.autocrlf), and git would see no change between the two."""
    return a.replace(b"\r\n", b"\n") == b.replace(b"\r\n", b"\n")


def cmd_sync_api(args):
    ffapi = sibling("ffapi")
    dest = ffapi / "docs" / "generated"
    print(f"{BOLD}Syncing the API{RESET}  {DIM}({ffapi}; nothing is committed there){RESET}\n")
    if not dest.is_dir():
        print(f"{RED}No {dest}{RESET} {DIM}([repos] in release.local.toml says where ffapi is){RESET}")
        return 1
    # Never over someone's work: the copies replace these files whole.
    _, status = run(["git", "status", "--porcelain", "--", "docs/generated"], cwd=ffapi)
    if status.strip() and not args.dry_run:
        print(f"{RED}ffapi's docs/generated has uncommitted changes:{RESET}\n{status}"
              f"{DIM}Commit or discard them first; this replaces those files.{RESET}")
        return 1

    # 1. Ours current: the committed two by their gates (stale ones are
    #    `check --fix`'s to regenerate, with a diff to read), the rest
    #    written fresh.
    gates = [Gate("contract, catalog",
                  ["cargo", "test", "--release", "--lib", "--", "contract_is_current", "catalog_is_current"],
                  why="stale: `release.py check --fix`, read the diff, commit, then sync")]
    for rel, binary in SYNC_FILES + [CURATED_MANIFEST]:
        if binary:
            gates.append(Gate(binary,
                              ["cargo", "run", "--release", "--quiet", "--bin", binary],
                              why=f"cargo run --release --bin {binary}"))
    if run_gates(gates, argparse.Namespace(fix=False)):
        return 1

    # 2. Copy what differs, keeping the line endings ffapi's copy has.
    print()
    changed = []
    for rel, _ in SYNC_FILES:
        ours = (ROOT / rel).read_bytes()
        target = dest / Path(rel).name
        theirs = target.read_bytes() if target.exists() else None
        if theirs is not None and _same_text(ours, theirs):
            print(f"  {DIM}same   {Path(rel).name}{RESET}")
            continue
        changed.append(target.name)
        if args.dry_run:
            print(f"  {YELLOW}differs{RESET} {target.name}")
            continue
        if theirs is not None and b"\r\n" in theirs:
            ours = ours.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")
        target.write_bytes(ours)
        print(f"  {GREEN}copied{RESET} {target.name}")

    if args.dry_run:
        print(f"\n{DIM}Dry run: nothing copied, ffapi's tests not run.{RESET}")
        return 0

    # 3. ffapi's tests that read them. A failure after a copy is the point:
    #    the API's side must follow (a vocabulary, the summary's rules).
    if not args.skip_tests:
        print(f"\n{BOLD}ffapi's tests{RESET}  {DIM}({', '.join(FFAPI_TESTS)}; "
              f"the database in its .env){RESET}\n")
        cmd = ["cargo", "test"] + [a for t in FFAPI_TESTS for a in ("--test", t)]
        if run_gates([Gate("ffapi conformance", lambda: run(cmd, cwd=ffapi),
                           why="the API disagrees with the files it was given")],
                     argparse.Namespace(fix=False)):
            print(f"\n{DIM}The copies are in place in {dest}; fix the API's side, "
                  f"or `git checkout docs/generated` there to undo.{RESET}")
            return 1

    print()
    if changed:
        _, stat = run(["git", "diff", "--stat", "--", "docs/generated"], cwd=ffapi)
        print(stat.rstrip())
        print(f"\n{GREEN}{BOLD}Synced.{RESET} {DIM}Read the diff and commit it in ffapi.{RESET}")
    else:
        print(f"{GREEN}{BOLD}Nothing to sync:{RESET} {DIM}ffapi's copies are current.{RESET}")
    print(f"{DIM}The curated manifest ({CURATED_MANIFEST[0]}) is posted at deploy: "
          f"ffapi's tools/sync_curated.py.{RESET}")
    return 0


# ------------------------------------------------------ sync-web, deploys

# The web builds a deploy uploads: this repository's output directory,
# where it goes in the server's ffbrowser checkout, and where the site
# serves it (ffbrowser's docs/DEPLOYMENT.md). Each carries version.json.
WEB_BUILDS = [
    ("pkg", "public/editor/pkg", "editor/pkg"),
    ("wasm/render/pkg", "public/wasm/render", "wasm/render"),
    ("wasm/script/pkg", "public/wasm/script", "wasm/script"),
]

# The editor's page around pkg/: ffbrowser keeps its own tracked copy in
# public/editor/, which drifted while only pkg/ was uploaded. `sync-web`
# copies these there for a commit; `deploy-web` refuses while they differ.
EDITOR_FILES = ["index.html", "css", "js"]
EDITOR_DEST = Path("public/editor")


def _files_under(base, names):
    """{relative path: bytes} for the files and directory trees `names`."""
    out = {}
    for name in names:
        p = base / name
        if p.is_file():
            out[name] = p.read_bytes()
        elif p.is_dir():
            for f in sorted(p.rglob("*")):
                if f.is_file():
                    out[f.relative_to(base).as_posix()] = f.read_bytes()
    return out


def editor_drift():
    """How ffbrowser's copy of the editor page differs from ours: a list of
    (relative path, what), empty when it matches."""
    ours = _files_under(ROOT, EDITOR_FILES)
    theirs = _files_under(sibling("ffbrowser") / EDITOR_DEST, EDITOR_FILES)
    drift = []
    for rel in sorted(set(ours) | set(theirs)):
        if rel not in theirs:
            drift.append((rel, "missing there"))
        elif rel not in ours:
            drift.append((rel, "only there"))
        elif not _same_text(ours[rel], theirs[rel]):
            drift.append((rel, "differs"))
    return drift


def copy_builds(browser, dry_run):
    """The three web builds into ffbrowser's public/, where its dev server
    serves them and a deploy puts them on the server (gitignored there, so
    nothing to commit). Each replaces its directory whole. Says which
    commit each was built from, and warns when that is not HEAD."""
    import json
    import shutil
    _, head = run(["git", "rev-parse", "--short=8", "HEAD"])
    for rel, dest, _ in WEB_BUILDS:
        src, target = ROOT / rel, browser / dest
        info_path = src / "version.json"
        if not info_path.exists():
            print(f"  {RED}missing{RESET}  {rel}/version.json {DIM}(`release.py build`){RESET}")
            return False
        info = json.loads(info_path.read_text(encoding="utf-8"))
        built = f"{info.get('version')} {info.get('git_hash')} {info.get('profile')}, {info.get('built')}"
        stale = info.get("git_hash") != head.strip() or info.get("git_dirty")
        note = f"  {YELLOW}(not HEAD {head.strip()}){RESET}" if stale else ""
        if dry_run:
            print(f"  {DIM}would copy{RESET} {rel} -> {dest}  {DIM}{built}{RESET}{note}")
            continue
        if target.exists():
            shutil.rmtree(target)
        shutil.copytree(src, target)
        print(f"  {GREEN}copied{RESET} {rel} -> {dest}  {DIM}{built}{RESET}{note}")
    return True


def cmd_sync_web(args):
    browser = sibling("ffbrowser")
    dest = browser / EDITOR_DEST
    print(f"{BOLD}Syncing the web builds and the editor page into ffbrowser{RESET}  "
          f"{DIM}({browser}; nothing is committed there){RESET}\n")
    if not dest.is_dir():
        print(f"{RED}No {dest}{RESET} {DIM}([repos] in release.local.toml says where ffbrowser is){RESET}")
        return 1
    if not args.page_only:
        if not copy_builds(browser, args.dry_run):
            return 1
        print()
    drift = editor_drift()
    if not drift:
        print(f"{GREEN}{BOLD}The editor page matches{RESET} {DIM}ffbrowser's copy; nothing to commit.{RESET}")
        return 0
    for rel, what in drift:
        print(f"  {YELLOW}{what:<13}{RESET} {rel}")
    if args.dry_run:
        print(f"\n{DIM}Dry run: nothing copied.{RESET}")
        return 0
    pathspecs = [(EDITOR_DEST / n).as_posix() for n in EDITOR_FILES]
    _, status = run(["git", "status", "--porcelain", "--", *pathspecs], cwd=browser)
    if status.strip():
        print(f"\n{RED}ffbrowser has uncommitted changes there:{RESET}\n{status}"
              f"{DIM}Commit or discard them first; this replaces those files.{RESET}")
        return 1
    for rel, what in drift:
        target = dest / rel
        if what == "only there":
            target.unlink()
            continue
        data = (ROOT / rel).read_bytes()
        if target.exists() and b"\r\n" in target.read_bytes():
            data = data.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    _, stat = run(["git", "diff", "--stat", "--", *pathspecs], cwd=browser)
    print("\n" + stat.rstrip())
    print(f"\n{GREEN}{BOLD}Synced.{RESET} {DIM}Read the diff, commit and push it in ffbrowser "
          f"before deploy-web.{RESET}")
    return 0


def build_matches_head(rel):
    """(ok, why) for a web build's version.json against this checkout:
    the version and commit it says, and not built from a dirty tree."""
    path = ROOT / rel / "version.json"
    if not path.exists():
        return False, f"{rel}/version.json missing: `release.py build`"
    import json
    info = json.loads(path.read_text(encoding="utf-8"))
    _, head = run(["git", "rev-parse", "--short=8", "HEAD"])
    want = (read_version(), head.strip())
    have = (info.get("version"), info.get("git_hash"))
    if have != want:
        return False, f"{rel} is {have[0]} {have[1]}, this checkout {want[0]} {want[1]}: `release.py build`"
    if info.get("git_dirty"):
        return False, f"{rel} was built from a tree with uncommitted changes"
    return True, f"{info['version']} {info['git_hash']} {info.get('profile')}"


def changed_typings(site):
    """The gallery modules' .d.ts files whose live copies differ: the
    museum's src/museum/wasmTypes.ts mirrors them by hand."""
    changed = []
    for rel, _, live in WEB_BUILDS[1:]:
        for f in sorted((ROOT / rel).glob("*.d.ts")):
            status, body = fetch_url(f"{site.rstrip('/')}/{live}/{f.name}")
            if status != 200:
                changed.append(f"{live}/{f.name} (live: {status or 'unreachable'})")
            elif not _same_text(f.read_bytes(), body):
                changed.append(f"{live}/{f.name}")
    return changed


def upload_dir(local, remote_dir):
    """`local` into `remote_dir` on the server, whole: unpacked beside it
    and swapped in, so the site never serves half an upload and files the
    new build lacks do not linger. tar over ssh: no rsync needed here."""
    import io
    import shlex
    import tarfile
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        for f in sorted(local.rglob("*")):
            if f.is_file():
                tar.add(f, arcname=f.relative_to(local).as_posix())
    d = shlex.quote(remote_dir)
    script = (f"set -e; d={d}; rm -rf \"$d.new\" \"$d.old\"; mkdir -p \"$d.new\"; "
              f"tar -xzf - -C \"$d.new\"; if [ -e \"$d\" ]; then mv \"$d\" \"$d.old\"; fi; "
              f"mv \"$d.new\" \"$d\"; rm -rf \"$d.old\"")
    return remote(script, stdin=buf.getvalue())


def cmd_deploy_web(args):
    print(f"{BOLD}Deploying the web app and the gallery modules{RESET}\n")
    lacking = missing_settings("ssh", "site", "ffbrowser_dir")
    if lacking:
        print(f"{RED}{lacking}{RESET}")
        return 1
    s = server()
    site, base = s["site"].rstrip("/"), s["ffbrowser_dir"].rstrip("/")
    browser = sibling("ffbrowser")

    gates = [Gate(f"{rel} built", lambda r=rel: build_matches_head(r),
                  why="each build's version.json says this checkout")
             for rel, _, _ in WEB_BUILDS]
    gates.append(Gate("editor page in ffbrowser",
                      lambda: (lambda d: (not d, "\n".join(f"{w}: {r}" for r, w in d)))(editor_drift()),
                      why="`release.py sync-web`, then commit and push in ffbrowser"))
    gates.append(Gate("ffbrowser ready", lambda: repo_ready(browser, fetch=True),
                      why="the server pulls origin/main: clean, on main, pushed"))
    if run_gates(gates, argparse.Namespace(fix=False)):
        return 1

    typings = changed_typings(site)
    if typings and not args.types_checked:
        print(f"\n{RED}The gallery modules' typings changed:{RESET}")
        print("\n".join(f"  {t}" for t in typings))
        print(f"{DIM}ffbrowser's src/museum/wasmTypes.ts mirrors them by hand. Update it, push, "
              f"and run again with --types-checked.{RESET}")
        return 1

    _, local_head = run(["git", "rev-parse", "HEAD"], cwd=browser)
    plan = [f"pull ffbrowser {local_head.strip()[:8]} into {s['ssh']}:{base}"]
    plan += [f"upload {rel}/ to {base}/{dest}/ (swapped in whole)" for rel, dest, _ in WEB_BUILDS]
    plan += [f"build there (`npm ci` if package-lock.json changed, `npm run build`)",
             f"check {site} serves each build's version.json and the editor page"]
    if args.dry_run:
        print(f"\n{BOLD}Would:{RESET}")
        print("\n".join(f"  - {l}" for l in plan))
        return 0
    if not confirm(plan):
        return 1

    import shlex
    q = shlex.quote(base)
    ok, out = remote(f"set -e; cd {q}; git rev-parse HEAD; git pull --ff-only -q; git rev-parse HEAD",
                     capture=True)
    heads = [l.strip() for l in out.splitlines() if re.fullmatch(r"[0-9a-f]{40}", l.strip())]
    if not ok or len(heads) != 2:
        print(f"{RED}git pull on the server failed:{RESET}\n{out}")
        return 1
    old, new = heads
    if new != local_head.strip():
        print(f"{RED}The server's ffbrowser is at {new[:8]}, ours {local_head.strip()[:8]}.{RESET}")
        return 1
    print(f"  {GREEN}pulled{RESET} ffbrowser {old[:8]} -> {new[:8]}")

    for rel, dest, _ in WEB_BUILDS:
        ok, out = upload_dir(ROOT / rel, f"{base}/{dest}")
        if not ok:
            print(f"{RED}Upload of {rel} failed:{RESET}\n{out}")
            return 1
        print(f"  {GREEN}uploaded{RESET} {rel} -> {dest}")

    print(f"\n{DIM}Building on the server…{RESET}")
    ok, _ = remote(f"set -e; cd {q}; "
                   f"if ! git diff --quiet {old} {new} -- package-lock.json; then npm ci; fi; "
                   f"npm run build")
    if not ok:
        print(f"{RED}The server's build failed.{RESET} {DIM}The uploads are in place; "
              f"the site still serves the previous dist/.{RESET}")
        return 1

    print()
    return 0 if verify_web(site) else 1


def verify_web(site):
    """What the site serves is what was built: each build's version.json,
    and the editor page."""
    import json
    good = True
    for rel, _, live in WEB_BUILDS:
        want = json.loads((ROOT / rel / "version.json").read_text(encoding="utf-8"))
        status, body = fetch_url(f"{site}/{live}/version.json")
        try:
            have = json.loads(body) if status == 200 else None
        except ValueError:
            have = None
        same = have is not None and all(have.get(k) == want.get(k) for k in ("version", "git_hash", "built"))
        good &= same
        # A missing file is the SPA's own page with a 200 (nginx's
        # try_files fallback), so "not JSON" is "not there".
        seen = (f"{have.get('version')} {have.get('git_hash')} built {have.get('built')}" if have
                else "no version.json" if status == 200 else f"status {status}")
        print(f"  {GREEN + 'live' if same else RED + 'STALE'}{RESET}  {live:<12} {DIM}{seen}{RESET}")
    status, body = fetch_url(f"{site}/editor/index.html")
    same = status == 200 and _same_text((ROOT / "index.html").read_bytes(), body)
    good &= same
    print(f"  {GREEN + 'live' if same else RED + 'STALE'}{RESET}  editor page  {DIM}(status {status}){RESET}")
    print(f"\n{GREEN + BOLD + 'Deployed.' if good else RED + BOLD + 'The site does not serve what was built.'}"
          f"{RESET}")
    return good


def cmd_deploy_api(args):
    print(f"{BOLD}Deploying the API{RESET}\n")
    lacking = missing_settings("ssh", "site", "ffapi_dir")
    if lacking:
        print(f"{RED}{lacking}{RESET}")
        return 1
    s = server()
    site, base, port = s["site"].rstrip("/"), s["ffapi_dir"].rstrip("/"), int(s.get("api_port", 3000))
    ffapi = sibling("ffapi")
    if run_gates([Gate("ffapi ready", lambda: repo_ready(ffapi, fetch=True),
                       why="the server pulls origin/main: clean, on main, pushed")],
                 argparse.Namespace(fix=False)):
        return 1

    _, local_head = run(["git", "rev-parse", "HEAD"], cwd=ffapi)
    manifest = ROOT / CURATED_MANIFEST[0]
    token = os.environ.get("FFAPI_TOKEN")
    plan = [f"pull ffapi {local_head.strip()[:8]} into {s['ssh']}:{base}",
            "build it there (`cargo clean -p ffapi --release && cargo build --release`: "
            "migrations embed at compile time)",
            "stop the service, copy the binary to /opt/ffapi, start it (sudo: a password prompt)",
            f"wait for /health on port {port}; show {site}/api/v1/app/latest"]
    if token and manifest.exists():
        plan.append("the curated sync: a dry run, its report, then --write only if you say so")
    else:
        plan.append("the curated sync is left for you (FFAPI_TOKEN unset or no manifest: `release.py sync-api`)")
    if args.dry_run:
        print(f"{BOLD}Would:{RESET}")
        print("\n".join(f"  - {l}" for l in plan))
        return 0
    if not confirm(plan):
        return 1

    import shlex
    q = shlex.quote(base)
    cargo_env = '. "$HOME/.cargo/env" 2>/dev/null || true'
    ok, out = remote(f"set -e; cd {q}; git pull --ff-only -q; git rev-parse HEAD", capture=True)
    head = out.strip().splitlines()[-1] if out.strip() else ""
    if not ok or head != local_head.strip():
        print(f"{RED}The server's ffapi is not at ours ({local_head.strip()[:8]}):{RESET}\n{out}")
        return 1
    print(f"  {GREEN}pulled{RESET} ffapi {head[:8]}")

    print(f"\n{DIM}Building on the server…{RESET}")
    ok, _ = remote(f"set -e; {cargo_env}; cd {q}; cargo clean -p ffapi --release; cargo build --release")
    if not ok:
        print(f"{RED}The build failed;{RESET} {DIM}the running API is untouched.{RESET}")
        return 1

    print(f"\n{DIM}Swapping the binary (sudo)…{RESET}")
    ok, _ = remote(f"cd {q} && sudo service ffapi stop && sudo cp target/release/ffapi /opt/ffapi/ "
                   f"&& sudo chown -R ffapi:ffapi /opt/ffapi && sudo service ffapi start", tty=True)
    if not ok:
        print(f"{RED}The swap failed.{RESET} {DIM}Check `sudo systemctl status ffapi` on the server.{RESET}")
        return 1

    ok, out = remote(f"for i in $(seq 60); do curl -fsS http://127.0.0.1:{port}/health && exit 0; "
                     f"sleep 1; done; exit 1", capture=True)
    if not ok:
        print(f"{RED}No /health after 60s:{RESET} {DIM}`journalctl -u ffapi -n 50` on the server.{RESET}\n{out}")
        return 1
    print(f"  {GREEN}healthy{RESET} {DIM}{out.strip()[:80]}{RESET}")
    status, body = fetch_url(f"{site}/api/v1/app/latest")
    print(f"  {DIM}/app/latest: {status} {body.decode('utf-8', errors='replace')[:120]}{RESET}")

    if token and manifest.exists():
        sync = [sys.executable, str(ffapi / "tools" / "sync_curated.py"), str(manifest), "--api", site]
        print(f"\n{BOLD}Curated sync, dry run{RESET}\n")
        if subprocess.run(sync).returncode != 0:
            print(f"{RED}The dry run failed or reported a hash mismatch;{RESET} {DIM}nothing written.{RESET}")
            return 1
        if confirm(["write the curated sync the report above describes (sync_curated.py --write)"]):
            if subprocess.run(sync + ["--write"]).returncode != 0:
                return 1
    print(f"\n{GREEN}{BOLD}API deployed.{RESET}")
    return 0


# --------------------------------------------------------------- publish

def release_zips(version):
    """The desktop zips of `version` that exist here (each is built on its
    own machine: whichever this one made)."""
    return [p for p in (ROOT / "target" / "windows" / f"FractalArtEditor-{version}-windows.zip",
                        ROOT / "target" / "macos" / f"FractalArtEditor-{version}-macos.zip")
            if p.exists()]


def cmd_publish(args):
    """Tag, push the tag, and create the GitHub release with the zips
    here. Run again on the other machine, it adds that machine's zip to
    the release. Last of the stages: the server's update notice reads the
    newest GitHub release, so it must not exist before the deploys."""
    version = read_version()
    tag = f"v{version}"
    print(f"{BOLD}Publishing {tag}{RESET}\n")
    zips = release_zips(version)
    ok, _ = run(["gh", "release", "view", tag, "--json", "tagName"])
    if ok:
        # The release exists: this is the second machine adding its zip.
        _, out = run(["gh", "release", "view", tag, "--json", "assets", "--jq", ".assets[].name"])
        have = set(out.split())
        new = [z for z in zips if z.name not in have]
        if not new:
            print(f"{GREEN}{tag} is published{RESET} {DIM}with {', '.join(sorted(have)) or 'no zips'}; "
                  f"nothing here to add.{RESET}")
            return 0
        if args.dry_run:
            print(f"{BOLD}Would:{RESET} upload {', '.join(z.name for z in new)} to {tag}")
            return 0
        if not confirm([f"upload {', '.join(z.name for z in new)} to the GitHub release {tag}"]):
            return 1
        ok, out = run(["gh", "release", "upload", tag, *map(str, new)])
        print(f"{GREEN}uploaded{RESET}" if ok else f"{RED}upload failed:{RESET}\n{out}")
        return 0 if ok else 1

    notes = ROOT / "NOTES.md"
    gates = [
        Gate("fflame-rust ready", lambda: repo_ready(ROOT, fetch=True),
             why="the tag goes on the pushed main"),
        Gate("versions agree", lambda: check_versions(), why="`release.py versions`"),
        Gate("gh signed in", ["gh", "auth", "status"], why="the release is created with gh"),
        Gate("tag is new", lambda: (lambda ok_out: (not ok_out[1].strip(), f"{tag} exists"))(
            run(["git", "tag", "--list", tag])), why="a tag is never moved"),
        Gate("NOTES.md", lambda: (notes.exists(), "missing: `release.py notes`, then edit it"),
             why="the release's notes"),
        Gate("a zip to upload", lambda: (bool(zips), f"no FractalArtEditor-{version}-*.zip: `release.py build`"),
             why="the macOS one can follow from that machine (`publish` again there)"),
    ]
    if run_gates(gates, argparse.Namespace(fix=False)):
        return 1
    _, head = run(["git", "rev-parse", "--short=8", "HEAD"])
    plan = [f"tag {head.strip()} as {tag} (annotated) and push the tag to origin",
            f"create the GitHub release {tag}, notes from NOTES.md, with {', '.join(z.name for z in zips)}",
            "the server's update notice follows within the hour (it reads the newest release)"]
    if args.dry_run:
        print(f"\n{BOLD}Would:{RESET}")
        print("\n".join(f"  - {l}" for l in plan))
        return 0
    if not confirm(plan):
        return 1
    for cmd in (["git", "tag", "-a", tag, "-m", version],
                ["git", "push", "origin", tag],
                ["gh", "release", "create", tag, "--title", version, "--notes-file", str(notes),
                 *map(str, zips)]):
        ok, out = run(cmd)
        if not ok:
            print(f"{RED}{' '.join(cmd[:3])} failed:{RESET}\n{out}")
            return 1
        print(f"  {GREEN}done{RESET} {' '.join(cmd[:3])}")
    print(f"\n{GREEN}{BOLD}Published {tag}.{RESET} {DIM}On the other machine: build, then "
          f"`release.py publish` adds its zip.{RESET}")
    return 0


# ------------------------------------------------------------------ ship

def pause(message):
    """Wait for a person: Enter goes on, anything else stops."""
    try:
        answer = input(f"\n{YELLOW}{message}{RESET}\n  Enter to go on, or type stop: ")
    except EOFError:
        answer = "stop"
    return answer.strip() == ""


def cmd_ship(args):
    """Every stage in order (release-streamlining.md §2), stopping at the
    first failure and pausing where a person must act. `--from` resumes."""
    version = args.number
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        print(f"{RED}`{version}` is not x.y.z{RESET}", file=sys.stderr)
        return 1
    ns = argparse.Namespace
    dry = args.dry_run

    def bump():
        if read_version() == version:
            print(f"{DIM}Already {version}.{RESET}")
            return 0
        return cmd_version(ns(number=version, dry_run=dry))

    def notes():
        if (ROOT / "NOTES.md").exists():
            print(f"{DIM}NOTES.md exists; kept.{RESET}")
        elif dry:
            print(f"{DIM}Would write NOTES.md from the changelog, and wait while you edit it.{RESET}")
        elif cmd_notes(ns(since=None, all=False, force=False)):
            return 1
        return 0 if dry or pause("Edit NOTES.md down to what a user would notice.") else 1

    def commit():
        _, status = run(["git", "status", "--porcelain"])
        if not status.strip():
            return 0
        print(f"{DIM}Uncommitted here (the version bump, lock files):{RESET}\n{status}")
        if dry:
            return 0
        if not pause("Commit these on main (\"Release " + version + "\") and push, so the builds "
                     "and the tag name one pushed commit."):
            return 1
        ready, problems = repo_ready(ROOT, fetch=True)
        if not ready:
            print(f"{RED}Not ready to build from:{RESET}\n{problems}")
        return 0 if ready else 1

    def synced(name, fn):
        def stage():
            # page_only: in a release the builds come later and deploy-web
            # uploads them; only the tracked editor page needs a commit.
            if fn(ns(dry_run=dry, skip_tests=False, page_only=True)):
                return 1
            path = sibling(name)
            _, status = run(["git", "status", "--porcelain"], cwd=path)
            if status.strip() and not dry and not pause(
                    f"Commit and push the sync in {name} (merge to its main as you do)."):
                return 1
            return 0
        return stage

    def smoke():
        return 0 if dry or pause("Smoke-test each artifact (RELEASE.md §4.5).") else 1

    stages = [
        ("preflight", lambda: cmd_preflight(ns(no_fetch=False))),
        ("version", bump),
        ("notes", notes),
        ("check", lambda: cmd_check(ns(fix=False, full=True))),
        ("commit", commit),
        ("sync-api", synced("ffapi", cmd_sync_api)),
        ("sync-web", synced("ffbrowser", cmd_sync_web)),
        ("build", lambda: cmd_build(ns(only=None))),
        ("smoke", smoke),
        ("deploy-api", lambda: cmd_deploy_api(ns(dry_run=dry))),
        ("deploy-web", lambda: cmd_deploy_web(ns(dry_run=dry, types_checked=args.types_checked))),
        ("publish", lambda: cmd_publish(ns(dry_run=dry))),
    ]
    names = [n for n, _ in stages]
    if args.from_stage and args.from_stage not in names:
        print(f"{RED}No stage `{args.from_stage}`:{RESET} {', '.join(names)}", file=sys.stderr)
        return 1
    start = names.index(args.from_stage) if args.from_stage else 0
    if dry:
        # A dry run checks and says; it builds nothing, so the stages that
        # only make things (check, build) are named, not run.
        print(f"{DIM}Dry run: check and build are skipped; every other stage checks and says "
              f"what it would do.{RESET}")
    for name, stage in stages[start:]:
        print(f"\n{BOLD}━━ {name}{RESET}")
        if dry and name in ("check", "build"):
            print(f"{DIM}(skipped in a dry run){RESET}")
            continue
        if stage():
            print(f"\n{RED}{BOLD}Stopped at {name}.{RESET} {DIM}Fix it, then "
                  f"`release.py ship {version} --from {name}`.{RESET}")
            return 1
    print(f"\n{GREEN}{BOLD}{version} shipped.{RESET}")
    return 0


# ----------------------------------------------------------------- build

# (name, command, note, directory). The two gallery modules are two
# wasm-pack builds, each run from its crate directory, where cargo finds
# wasm/.cargo/config.toml and its shared target directory.
BUILDS = [
    ("desktop (dist)", ["cargo", "build", "--profile", "dist", "--bin", "FractalArtEditor"],
     "~7-10 min: LTO, one codegen unit, stripped", None),
    ("web app", ["bash", "build-wasm.sh"], "wasm-bindgen -> pkg/", None),
    ("gallery: render", ["wasm-pack", "build", "--target", "web", "--release"],
     "in wasm/render", "wasm/render"),
    ("gallery: script", ["wasm-pack", "build", "--target", "web", "--release"],
     "in wasm/script", "wasm/script"),
    ("python wheel", [sys.executable, "python/build.py", "--wheel"],
     "the WASI module + one pure-Python wheel -> python/dist", None),
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
        if p.returncode == 0 and name.startswith("gallery:"):
            # wasm-pack recreates pkg/, so after it: what deploy-web reads
            # back from the live site (the web app's comes from
            # build-wasm.sh itself).
            ok, out = run([sys.executable, "scripts/write_version_json.py",
                           f"{sub}/pkg", "--profile", "release"])
            if not ok:
                p.returncode, p.stdout, p.stderr = 1, out, ""
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

    pf = sub.add_parser("preflight", help="all three repositories ready; gh and the server reachable")
    pf.add_argument("--no-fetch", action="store_true",
                    help="compare with the remote-tracking refs as they are")
    pf.set_defaults(fn=cmd_preflight)

    c = sub.add_parser("check", help="run every gate")
    c.add_argument("--fix", action="store_true",
                   help="regenerate stale generated files (still read the diff)")
    c.add_argument("--full", action="store_true",
                   help="also the visual suite and the API integration test (minutes; a GPU, "
                        "the editor closed, ffapi beside this repository)")
    c.set_defaults(fn=cmd_check)

    sa = sub.add_parser("sync-api", help="copy the generated files into ffapi and run its tests")
    sa.add_argument("--dry-run", action="store_true",
                    help="write ours and say what differs; copy nothing, run nothing in ffapi")
    sa.add_argument("--skip-tests", action="store_true", help="copy, but leave ffapi's tests")
    sa.set_defaults(fn=cmd_sync_api)

    sw = sub.add_parser("sync-web", help="copy the web builds and the editor page into ffbrowser")
    sw.add_argument("--dry-run", action="store_true", help="say what differs; copy nothing")
    sw.add_argument("--page-only", action="store_true",
                    help="only the editor page (index.html, css, js), not the three builds")
    sw.set_defaults(fn=cmd_sync_web)

    dw = sub.add_parser("deploy-web", help="upload the web app and gallery modules, build the site, check it")
    dw.add_argument("--dry-run", action="store_true", help="run the checks and say what it would do")
    dw.add_argument("--types-checked", action="store_true",
                    help="the gallery typings changed and ffbrowser's wasmTypes.ts follows them")
    dw.set_defaults(fn=cmd_deploy_web)

    da = sub.add_parser("deploy-api", help="build and restart the API on the server, then the curated sync")
    da.add_argument("--dry-run", action="store_true", help="run the checks and say what it would do")
    da.set_defaults(fn=cmd_deploy_api)

    pb = sub.add_parser("publish", help="tag, push the tag, create the GitHub release (or add this machine's zip)")
    pb.add_argument("--dry-run", action="store_true", help="run the checks and say what it would do")
    pb.set_defaults(fn=cmd_publish)

    sh = sub.add_parser("ship", help="every stage in order, pausing where a person must act")
    sh.add_argument("number", help="the version to release, x.y.z")
    sh.add_argument("--from", dest="from_stage", help="resume at this stage")
    sh.add_argument("--dry-run", action="store_true",
                    help="every stage's checks and plans; nothing built, changed or sent")
    sh.add_argument("--types-checked", action="store_true", help="passed to deploy-web")
    sh.set_defaults(fn=cmd_ship)

    n = sub.add_parser("notes", help="the changelog into NOTES.md, for editing")
    n.add_argument("--since", help="tag or ref (default: last tag)")
    n.add_argument("--all", action="store_true", help="include docs/tools/merges")
    n.add_argument("--force", action="store_true", help="replace an existing NOTES.md")
    n.set_defaults(fn=cmd_notes)

    gl = sub.add_parser("gallery", help="the gallery modules' and pyfflame's own tests, before they ship")
    gl.set_defaults(fn=cmd_gallery, fix=False)

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
        rc = cmd_check(argparse.Namespace(fix=False, full=False))
        print()
        cmd_changelog(argparse.Namespace(since=None, all=False))
        return rc
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
