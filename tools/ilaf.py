"""Image Line Archive File (1995): the container every Eat This data file (.MAP .TIL .SPR .LIT .MIX ...) uses.

Layout (little-endian, Turbo Pascal-style length-prefixed strings):

    0x00  char[30]  "Image Line Archive File (1995)"
    0x1e  u8        0x1a (DOS EOF, stops `type`)
    0x1f  u32       offset of the directory
    0x23  u16       ?
    0x25  u16       number of entries
    0x27  u16       ?
    0x29  string[40] description, e.g. "VGA TILE GROUP"
    dir:  entries of 22 bytes: u32 offset, u32 size, u8 flags, string[12] name
"""

import struct
import sys
from dataclasses import dataclass

MAGIC = b"Image Line Archive File (1995)\x1a"
ENTRY_SIZE = 22


@dataclass
class Entry:
    name: str
    offset: int
    size: int
    flags: int
    data: bytes


def pstring(buf: bytes, at: int) -> str:
    return buf[at + 1 : at + 1 + buf[at]].decode("latin-1")


class Archive:
    def __init__(self, data: bytes):
        if not data.startswith(MAGIC):
            raise ValueError("not an Image Line archive")
        self.raw = data
        dir_at, self.unk23, count, self.unk27 = struct.unpack_from("<IHHH", data, 0x1F)
        self.description = pstring(data, 0x29)
        self.entries: list[Entry] = []
        for i in range(count):
            at = dir_at + i * ENTRY_SIZE
            off, size, flags = struct.unpack_from("<IIB", data, at)
            self.entries.append(Entry(pstring(data, at + 9), off, size, flags, data[off : off + size]))

    @classmethod
    def open(cls, path: str) -> "Archive":
        with open(path, "rb") as f:
            return cls(f.read())

    def __getitem__(self, name: str) -> Entry:
        for e in self.entries:
            if e.name == name:
                return e
        raise KeyError(name)


if __name__ == "__main__":
    for path in sys.argv[1:]:
        a = Archive.open(path)
        print(f"{path}: {a.description!r} unk23={a.unk23} unk27={a.unk27}")
        for e in a.entries:
            print(f"  {e.name:<12} off={e.offset:#07x} size={e.size:6} flags={e.flags}  {e.data[:16].hex(' ')}")
