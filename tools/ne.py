"""Win16 New Executable (NE) parser: segments, relocations, entry table, imported module names.

    python tools/ne.py work/cd/ETLAUNCH.EXE     summary
"""

import struct
import sys
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class Reloc:
    src: int  # 0 low byte, 2 selector, 3 far pointer, 5 offset
    kind: int  # 0 internal, 1 import ordinal, 2 import name, 3 OS fixup
    additive: bool
    offset: int  # first location (chained unless additive)
    a: int  # internal: segment (0xff = movable); import: module index (1-based)
    b: int  # internal: offset or entry ordinal; import: ordinal or name offset


@dataclass
class Segment:
    num: int  # 1-based
    data: bytes
    flags: int
    minalloc: int
    relocs: list[Reloc] = field(default_factory=list)

    @property
    def is_data(self):
        return bool(self.flags & 1)


class NE:
    def __init__(self, path):
        d = Path(path).read_bytes()
        self.raw = d
        ne = struct.unpack_from("<I", d, 0x3C)[0]
        self.ne = ne
        (
            magic,
            _ver,
            _rev,
            enttab,
            entlen,
            _crc,
            self.flags,
            self.autodata,
            self.heap,
            self.stack,
            self.ip,
            self.cs,
            self.sp,
            self.ss,
            nseg,
            nmod,
            _nrsize,
            segtab,
            _restab,
            _resnames,
            modref,
            impnames,
            _nrestab,
            _nmovable,
            align,
            _nres,
        ) = struct.unpack_from("<2sBBHHIHHHHHHHHHHHHHHHHIHHH", d, ne)
        assert magic == b"NE"
        self.modules = []
        for i in range(nmod):
            off = struct.unpack_from("<H", d, ne + modref + 2 * i)[0]
            self.modules.append(self._pstr(ne + impnames + off))
        self.impnames = ne + impnames
        self.segments = []
        for i in range(nseg):
            sector, length, flags, minalloc = struct.unpack_from("<HHHH", d, ne + segtab + 8 * i)
            start = sector << align
            length = length or 0x10000
            minalloc = minalloc or 0x10000
            data = d[start : start + length] if sector else b""
            seg = Segment(i + 1, data, flags, minalloc)
            if sector and flags & 0x100:
                p = start + length
                (n,) = struct.unpack_from("<H", d, p)
                for k in range(n):
                    src, fl, off, a, b = struct.unpack_from("<BBHHH", d, p + 2 + 8 * k)
                    if fl & 3 == 0:
                        a &= 0xFF
                    seg.relocs.append(Reloc(src, fl & 3, bool(fl & 4), off, a, b))
            self.segments.append(seg)
        self.entries = self._entries(ne + enttab, entlen)

    def _pstr(self, p):
        n = self.raw[p]
        return self.raw[p + 1 : p + 1 + n].decode("latin-1")

    def import_name(self, off):
        return self._pstr(self.impnames + off)

    def _entries(self, p, length):
        """ordinal -> (segment, offset)"""
        out = {}
        end = p + length
        ordinal = 1
        while p < end:
            count, ind = self.raw[p], self.raw[p + 1]
            p += 2
            if count == 0:
                break
            for _ in range(count):
                if ind == 0:
                    pass
                elif ind == 0xFF:  # movable: flags, int 3fh, seg, offset
                    _fl, _int, seg, off = struct.unpack_from("<BHBH", self.raw, p)
                    out[ordinal] = (seg, off)
                    p += 6
                else:
                    _fl, off = struct.unpack_from("<BH", self.raw, p)
                    out[ordinal] = (ind, off)
                    p += 3
                ordinal += 1
        return out


if __name__ == "__main__":
    ne = NE(sys.argv[1])
    print("modules:", ne.modules)
    print(f"entry {ne.cs}:{ne.ip:04x} stack {ne.ss}:{ne.sp:04x} autodata {ne.autodata} heap {ne.heap:#x} stack {ne.stack:#x}")
    for s in ne.segments:
        print(f"seg {s.num:2} {'DATA' if s.is_data else 'CODE'} len {len(s.data):#6x} min {s.minalloc:#7x} flags {s.flags:#06x} relocs {len(s.relocs)}")
