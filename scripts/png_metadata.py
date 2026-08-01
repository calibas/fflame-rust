#!/usr/bin/env python3
"""Read the metadata this app embeds in exported PNGs.

Every export carries build info, render settings and the **complete
FractalConfig** in tEXt chunks (see `src/png_metadata.rs`). So any
exported image is a reproducible artifact: the config that made it
travels with it.

    python scripts/png_metadata.py IMAGE.png              # list everything
    python scripts/png_metadata.py IMAGE.png --config     # just the config JSON
    python scripts/png_metadata.py IMAGE.png -o out.fflame

No dependencies and no build — tEXt is uncompressed key/NUL/value, so
this works on a PNG from any version of the app, including one built by
someone else.
"""

import argparse
import struct
import sys
import zlib


def read_text_chunks(path):
    """Every tEXt/zTXt/iTXt chunk, in file order, as (key, value)."""
    with open(path, "rb") as f:
        data = f.read()

    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path} is not a PNG")

    out = []
    pos = 8
    while pos + 8 <= len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        ctype = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + length]

        if ctype == b"tEXt":
            key, _, value = body.partition(b"\x00")
            out.append((key.decode("latin-1"), value.decode("latin-1")))
        elif ctype == b"zTXt":
            key, _, rest = body.partition(b"\x00")
            # rest = compression method byte, then the deflate stream
            try:
                value = zlib.decompress(rest[1:]).decode("latin-1")
                out.append((key.decode("latin-1"), value))
            except zlib.error as e:
                out.append((key.decode("latin-1"), f"<unreadable: {e}>"))
        elif ctype == b"iTXt":
            # key \0 compressed_flag comp_method \0 lang \0 translated \0 text
            key, _, rest = body.partition(b"\x00")
            compressed = rest[:1] == b"\x01"
            rest = rest[2:]
            _, _, rest = rest.partition(b"\x00")  # language tag
            _, _, text = rest.partition(b"\x00")  # translated keyword
            if compressed:
                try:
                    text = zlib.decompress(text)
                except zlib.error:
                    pass
            out.append((key.decode("latin-1"), text.decode("utf-8", "replace")))
        elif ctype == b"IEND":
            break

        pos += 12 + length  # length + type + body + CRC
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("image")
    ap.add_argument("--config", action="store_true",
                    help="print only the embedded FractalConfig JSON")
    ap.add_argument("-o", "--output", help="write the config to a .fflame file")
    args = ap.parse_args()

    chunks = read_text_chunks(args.image)
    if not chunks:
        raise SystemExit(f"{args.image} carries no text metadata")

    by_key = dict(chunks)

    if args.config or args.output:
        config = by_key.get("Config")
        if config is None:
            raise SystemExit(
                f"{args.image} has no `Config` chunk — it was not written by this app, "
                "or predates config embedding"
            )
        if args.output:
            with open(args.output, "w", encoding="utf-8") as f:
                f.write(config)
            print(f"Wrote {args.output} ({len(config)} bytes)", file=sys.stderr)
            # The checksum is over the config as written; report it so a
            # mismatch is visible rather than assumed away.
            if "ConfigChecksum" in by_key:
                print(f"Recorded checksum: {by_key['ConfigChecksum']}", file=sys.stderr)
        else:
            print(config)
        return

    width = max(len(k) for k, _ in chunks)
    for key, value in chunks:
        if key == "Config":
            # Thousands of characters of JSON; summarise unless asked.
            value = f"<{len(value)} bytes of JSON — use --config or -o to extract>"
        elif len(value) > 200:
            value = value[:197] + "..."
        print(f"{key:<{width}}  {value}")


if __name__ == "__main__":
    main()
