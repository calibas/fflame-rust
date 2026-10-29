#!/usr/bin/env python3
"""Render a preview PNG of every config file in a directory.

    python scripts/make_previews.py DIR                  # 480x480, beside each file
    python scripts/make_previews.py DIR --size 256       # 256x256
    python scripts/make_previews.py DIR --size 640x360
    python scripts/make_previews.py DIR -o previews/     # somewhere else

Reads what the app opens: .fflame (one config or an array), .flame
(Apophysis / JWildfire XML, one flame or several), and .anim / .json
animations, which preview the base config they embed -- their tracks are
not played.

Each preview is named after its file -- `Ocean.fflame` makes `Ocean.png`
-- and a file holding several configs makes one each, numbered from 1:
`Ocean-1.png`, `Ocean-2.png`, ... Nothing is ever overwritten: an image
that already exists is skipped with a warning, as is every config that
cannot be previewed. So two files of one name (`Ocean.fflame`,
`Ocean.flame`) make one preview, and the second is skipped.

Renders through the CLI's `export`, one file at a time, so a preview is
what an export of that config makes, its embedded metadata included.
Build the binary first: `cargo build --release`.
"""

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
EXE = "FractalArtEditor.exe" if sys.platform == "win32" else "FractalArtEditor"
DEFAULT_BINARY = REPO / "target" / "release" / EXE

READABLE = {".fflame", ".flame", ".anim", ".json"}

# A <flame ...> start tag, not <flames>.
FLAME_TAG = re.compile(rb"<flame[\s>/]")


class Skip(Exception):
    """Why a file makes no preview."""


def parse_size(text):
    m = re.fullmatch(r"(\d+)(?:x(\d+))?", text.strip().lower())
    if not m or int(m.group(1)) == 0 or int(m.group(2) or 1) == 0:
        raise argparse.ArgumentTypeError(f"`{text}` is not a size: N for a square, or WxH")
    width = int(m.group(1))
    return width, int(m.group(2) or width)


def write_config(tmp, n, config):
    path = tmp / f"config-{n}.fflame"
    path.write_text(json.dumps(config, ensure_ascii=False), encoding="utf-8")
    return path


def export_inputs(path, tmp):
    """What the CLI exports for `path`'s previews, one file per config: the
    file itself, or each of its configs written alone into `tmp`. Raises
    Skip when there is nothing to preview."""
    ext = path.suffix.lower()

    if ext == ".flame":
        # Bytes, not text: each flame is cut out verbatim, so the CLI
        # reads it exactly as it would read the whole file.
        data = path.read_bytes()
        starts = [m.start() for m in FLAME_TAG.finditer(data)]
        if not starts:
            raise Skip("it holds no <flame>")
        if len(starts) == 1:
            return [path]
        inputs = []
        for n, start in enumerate(starts, 1):
            end = data.find(b"</flame>", start)
            if end < 0:
                raise Skip(f"its <flame> number {n} is never closed")
            cut = tmp / f"flame-{n}.flame"
            cut.write_bytes(data[start:end + len(b"</flame>")])
            inputs.append(cut)
        return inputs

    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as e:
        raise Skip(f"it is not readable JSON ({e})")

    if ext in (".anim", ".json") and isinstance(value, dict) and (
        "tracks" in value or "base_config" in value
    ):
        config = value.get("base_config")
        if not isinstance(config, dict):
            raise Skip("it is an animation with no embedded config")
        # An embedded config without a version was written when the format
        # was v2 (`deserialize_base_config`), but a .fflame without one is
        # read as v0: say so, or it gets the wrong migrations.
        if "version" not in config:
            config = {**config, "version": 2}
        return [write_config(tmp, 1, config)]

    if isinstance(value, list):
        if not value or not all(isinstance(c, dict) for c in value):
            raise Skip("it is a list, but not of configs")
        return [write_config(tmp, n, c) for n, c in enumerate(value, 1)]

    if isinstance(value, dict) and (ext == ".fflame" or "flame" in value):
        return [path]
    raise Skip("it is not a config or an animation")


def last_line(result):
    """The CLI's last word on a failed export."""
    for text in (result.stderr, result.stdout):
        lines = [l.strip() for l in text.splitlines() if l.strip()]
        if lines:
            return lines[-1]
    return f"exit code {result.returncode}"


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("directory")
    ap.add_argument("--size", type=parse_size, default=(480, 480), metavar="N|WxH",
                    help="preview size: N for a square, or WxH (default: 480)")
    ap.add_argument("-o", "--output", help="where the previews go (default: beside the files)")
    ap.add_argument("--binary", type=Path, default=DEFAULT_BINARY,
                    help=f"the app's executable (default: {DEFAULT_BINARY.relative_to(REPO)})")
    args = ap.parse_args()

    # The CLI's messages carry non-ASCII marks; a console that cannot
    # show them gets a placeholder rather than a crash.
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(errors="replace")

    source_dir = Path(args.directory)
    if not source_dir.is_dir():
        raise SystemExit(f"{source_dir} is not a directory")
    if not args.binary.is_file():
        raise SystemExit(f"{args.binary} not found -- build it with `cargo build --release`")
    out_dir = Path(args.output) if args.output else source_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    files = sorted(p for p in source_dir.iterdir()
                   if p.is_file() and p.suffix.lower() in READABLE)
    if not files:
        print(f"No .fflame, .flame, .anim or .json files in {source_dir}")
        return

    width, height = args.size
    made, existed, failed = 0, 0, 0

    def warn(label, why):
        print(f"WARNING: {label}: {why}", file=sys.stderr, flush=True)

    for i, path in enumerate(files, 1):
        label = f"[{i}/{len(files)}] {path.name}"
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            try:
                inputs = export_inputs(path, tmp)
            except Skip as e:
                warn(label, f"skipped, {e}")
                failed += 1
                continue
            except OSError as e:
                warn(label, f"skipped, it cannot be read ({e})")
                failed += 1
                continue

            # One config keeps the file's name; several are numbered from 1.
            if len(inputs) == 1:
                targets = [out_dir / f"{path.stem}.png"]
            else:
                targets = [out_dir / f"{path.stem}-{n}.png" for n in range(1, len(inputs) + 1)]

            for n, (source, target) in enumerate(zip(inputs, targets), 1):
                if target.exists():
                    warn(label, f"skipped {target.name}, it already exists")
                    existed += 1
                    continue
                print(f"{label} -> {target.name}", flush=True)

                rendered = tmp / f"preview-{n}.png"
                result = subprocess.run(
                    [str(args.binary), "export", "-i", str(source.resolve()), "-o", str(rendered),
                     "-w", str(width), "-H", str(height)],
                    capture_output=True, text=True, encoding="utf-8", errors="replace", cwd=REPO,
                )
                if result.returncode != 0 or not rendered.is_file():
                    warn(label, f"skipped {target.name}, the render failed: {last_line(result)}")
                    failed += 1
                    continue
                try:
                    # "x" creates the file or fails: never an overwrite, even
                    # of one that appeared while this rendered.
                    with open(target, "xb") as f:
                        f.write(rendered.read_bytes())
                except FileExistsError:
                    warn(label, f"skipped {target.name}, it appeared while rendering")
                    existed += 1
                    continue
                made += 1

    print(f"\n{made} preview(s) written to {out_dir}; "
          f"{existed + failed} skipped ({existed} already existed, {failed} could not be made)")
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
