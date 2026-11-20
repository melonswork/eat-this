"""Patch a .MAP (Image Line archive) for oracle scenarios: move the hero's start, set cells.

    python3 tools/mapedit.py IN.MAP OUT.MAP [--start X Y] [--cell PLANE TX TY VALUE]...

The changed entries are re-packed with a literal-only LZW stream (a clear code every 200 bytes,
so the code width stays 9 bits) and appended to the file; their directory records are rewritten
in place. Never point OUT at the CD files (work/cd): write under work/demos.
"""

import argparse
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from ilaf import ENTRY_SIZE, Archive  # noqa: E402
from lzw import decompress  # noqa: E402


def lzw_literal(data: bytes) -> bytes:
    codes = []
    for i in range(0, len(data), 200):
        codes.append(0x100)
        codes.extend(data[i : i + 200])
    codes.append(0x101)
    bits, nbits = 0, 0
    out = bytearray(struct.pack("<I", len(data)))
    for c in codes:
        bits |= c << nbits
        nbits += 9
        while nbits >= 8:
            out.append(bits & 0xFF)
            bits >>= 8
            nbits -= 8
    if nbits:
        out.append(bits & 0xFF)
    return bytes(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("dst")
    ap.add_argument("--start", nargs=2, type=int)
    ap.add_argument("--cell", nargs=4, type=lambda s: int(s, 0), action="append", default=[])
    a = ap.parse_args()
    if "work/cd" in str(Path(a.dst).resolve()):
        raise SystemExit("refusing to write into work/cd")
    raw = bytearray(Path(a.src).read_bytes())
    arc = Archive(bytes(raw))
    get = lambda n: bytearray(decompress(arc[n].data) if arc[n].flags else arc[n].data)  # noqa: E731
    header = get("MAP HEADER")
    width = struct.unpack_from("<H", header, 2)[0]
    changed = {}
    if a.start:
        struct.pack_into("<hh", header, 6, *a.start)
        changed["MAP HEADER"] = header
    for plane, tx, ty, value in a.cell:
        name = f"PLANE {plane}"
        p = changed.get(name) or get(name)
        struct.pack_into("<H", p, (ty * width + tx) * 2, value)
        changed[name] = p
    dir_at = struct.unpack_from("<I", raw, 0x1F)[0]
    for i, e in enumerate(arc.entries):
        if e.name in changed:
            blob = lzw_literal(bytes(changed[e.name]))
            assert decompress(blob) == bytes(changed[e.name])
            at = dir_at + i * ENTRY_SIZE
            struct.pack_into("<IIB", raw, at, len(raw), len(blob), 1)
            raw += blob
    Path(a.dst).parent.mkdir(parents=True, exist_ok=True)
    Path(a.dst).write_bytes(raw)


if __name__ == "__main__":
    main()
