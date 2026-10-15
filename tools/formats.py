"""Decoders for Eat This data files. Every file is an Image Line archive (see ilaf.py)."""

import struct
from dataclasses import dataclass
from pathlib import Path

from ilaf import Archive
from lzw import decompress

TILE = 16


def entry(archive: Archive, name: str) -> bytes:
    e = archive[name]
    return decompress(e.data) if e.flags else e.data


def palette(raw: bytes) -> list[int]:
    """768 bytes of 6-bit VGA RGB -> flat 8-bit RGB list for PIL's putpalette."""
    return [(v << 2) | (v >> 4) for v in raw]


@dataclass
class TileGroup:
    tiles: list[bytes]  # 256 bytes each, 16x16 indexed pixels, row-major
    infos: list[bytes]  # 16 bytes per tile: solidity, functions, animation (see TILES.TXT)
    palette: list[int]

    @classmethod
    def load(cls, path: Path) -> "TileGroup":
        a = Archive.open(str(path))
        data, infos = entry(a, "TILE DATA"), entry(a, "TILE INFOS")
        return cls(
            [data[i : i + TILE * TILE] for i in range(0, len(data), TILE * TILE)],
            [infos[i : i + 16] for i in range(0, len(infos), 16)],
            palette(entry(a, "PALETTE")),
        )


@dataclass
class Map:
    width: int
    height: int
    header: bytes
    planes: list[list[int]]  # width*height u16 cells: high byte = tile group slot, low byte = tile
    background: str  # picture behind the planes (.LBM)
    groups: list[str | None]  # 16 tile group slots (.TIL)
    objects: bytes

    def object_records(self) -> list[list[int]]:
        """OBJECTS: MAP HEADER u16 @10 records of {u8 n; i16 words[n + 1]}, words[0] = type
        (iterated by FUN_10f0_5d1e, interpreted by LevelInit and the factory FUN_1070_d6be)."""
        count = struct.unpack_from("<H", self.header, 10)[0]
        out, at = [], 0
        for _ in range(count):
            n = self.objects[at] + 1
            out.append(list(struct.unpack_from(f"<{n}h", self.objects, at + 1)))
            at += 1 + 2 * n
        return out

    @classmethod
    def load(cls, path: Path) -> "Map":
        a = Archive.open(str(path))
        header = entry(a, "MAP HEADER")
        width, height = struct.unpack_from("<HH", header, 2)
        planes = []
        for i in range(3):
            try:
                raw = entry(a, f"PLANE {i}")
            except KeyError:
                break
            planes.append(list(struct.unpack(f"<{width * height}H", raw)))
        names = entry(a, "FILENAMES")
        slots = [names[i + 1 : i + 1 + names[i]].decode("latin-1") for i in range(0, len(names), 13)]
        return cls(width, height, header, planes, slots[0], [s or None for s in slots[1:]], entry(a, "OBJECTS"))


@dataclass
class Sprite:
    width: int
    height: int
    hotspot: tuple[int, int]  # offset of the top-left corner from the sprite's anchor
    extra: bytes  # u16-length block after the hotspot: hitboxes (u16 count + 18 bytes each), ...
    spans: list[tuple[int, int]]  # per row: (opaque width | 0x8000, first opaque x)
    pixels: bytes  # width*height indexed, 0 = transparent

    @classmethod
    def parse(cls, d: bytes) -> "Sprite":
        w, h, hx, hy, ext = struct.unpack_from("<HHhhH", d)
        at = 10 + ext
        spans, rows = [], []
        for _ in range(h):
            spans.append(struct.unpack_from("<HH", d, at))
            rows.append(d[at + 4 : at + 4 + w])
            at += 4 + w
        return cls(w, h, (hx, hy), d[10:at - h * (w + 4)], spans, b"".join(rows))


@dataclass
class SpriteGroup:
    sprites: list[Sprite | None]
    palette: list[int] | None

    @classmethod
    def load(cls, path: Path) -> "SpriteGroup":
        a = Archive.open(str(path))
        sprites = [
            Sprite.parse(entry(a, e.name)) if e.size else None
            for e in a.entries
            if e.name.startswith("SPRITE ") or e.name == "EMPTY"
        ]
        try:
            pal = palette(entry(a, "PALETTE"))
        except KeyError:
            pal = None
        return cls(sprites, pal)


@dataclass
class Picture:
    """An IFF picture (the level backgrounds in BIN/PICTURES): FORM PBM (chunky) or ILBM (planar)."""

    width: int
    height: int
    pixels: bytes  # width*height indexed
    palette: list[int]  # flat 8-bit RGB, from CMAP
    masking: int  # BMHD masking: 0 none, 1 mask plane, 2 transparent colour
    transparent: int  # BMHD transparent colour index

    @classmethod
    def load(cls, path: Path) -> "Picture":
        d = Path(path).read_bytes()
        if d[:4] != b"FORM" or d[8:12] not in (b"PBM ", b"ILBM"):
            raise ValueError("not an IFF PBM/ILBM")
        planar = d[8:12] == b"ILBM"
        at, end = 12, min(len(d), 8 + struct.unpack_from(">I", d, 4)[0])
        w = h = nplanes = masking = comp = transp = 0
        cmap, body = b"", b""
        while at + 8 <= end:
            tag, size = d[at : at + 4], struct.unpack_from(">I", d, at + 4)[0]
            chunk = d[at + 8 : at + 8 + size]
            if tag == b"BMHD":
                w, h = struct.unpack_from(">HH", chunk, 0)
                nplanes, masking, comp = chunk[8], chunk[9], chunk[10]
                transp = struct.unpack_from(">H", chunk, 12)[0]
            elif tag == b"CMAP":
                cmap = chunk
            elif tag == b"BODY":
                body = chunk
            at += 8 + size + (size & 1)  # chunks are padded to even length
        # CMAP is 8 bits per channel; the game keeps the top 6 (FUN_10f0_0b6d shifts it right by 2)
        pal = palette(bytes(v >> 2 for v in cmap.ljust(768, b"\0")[:768]))
        rows = []
        pos = 0
        if planar:
            stride = ((w + 15) // 16) * 2
            planes = nplanes + (1 if masking == 1 else 0)
            row_len = stride * planes
        else:
            row_len = w
        for _ in range(h):
            if comp == 1:
                row, pos = byterun1(body, pos, row_len)
            else:
                row, pos = body[pos : pos + row_len], pos + row_len
            if planar:
                px = bytearray(w)
                for p in range(nplanes):
                    line = row[p * stride : (p + 1) * stride]
                    for x in range(w):
                        px[x] |= ((line[x >> 3] >> (7 - (x & 7))) & 1) << p
                row = bytes(px)
            rows.append(row[:w])
        return cls(w, h, b"".join(rows), pal, masking, transp)


def byterun1(buf: bytes, pos: int, n: int) -> tuple[bytes, int]:
    """Unpack one scan line (n bytes) of IFF ByteRun1: control c >= 0 copies c + 1 bytes,
    c in -127..-1 repeats the next byte 1 - c times, -128 does nothing."""
    out = bytearray()
    while len(out) < n and pos < len(buf):
        c = buf[pos]
        pos += 1
        if c < 128:
            out += buf[pos : pos + c + 1]
            pos += c + 1
        elif c > 128:
            out += buf[pos : pos + 1] * (257 - c)
            pos += 1
    return bytes(out[:n]), pos
