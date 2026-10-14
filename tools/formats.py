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
