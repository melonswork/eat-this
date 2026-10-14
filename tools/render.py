"""Render Eat This tile groups and maps to PNG.

    render.py tiles TILES/CITY01.TIL out.png
    render.py map MAPS/CITY01.MAP out.png [plane ...]
    render.py sprites SPRITES/GRANNY.SPR out.png
"""

import sys
from pathlib import Path

from PIL import Image

from formats import TILE, Map, SpriteGroup, TileGroup

BIN = Path("work/cd/BIN")
TRANSPARENT = 0


def tile_image(group: TileGroup, index: int) -> Image.Image:
    img = Image.frombytes("P", (TILE, TILE), group.tiles[index])
    img.putpalette(group.palette)
    return img


def render_tiles(path: Path, out: Path, columns: int = 16) -> None:
    group = TileGroup.load(path)
    rows = (len(group.tiles) + columns - 1) // columns
    sheet = Image.new("P", (columns * TILE, rows * TILE))
    sheet.putpalette(group.palette)
    for i, pixels in enumerate(group.tiles):
        sheet.paste(Image.frombytes("P", (TILE, TILE), pixels), ((i % columns) * TILE, (i // columns) * TILE))
    sheet.save(out)


def render_map(path: Path, out: Path, planes: list[int] | None = None) -> None:
    m = Map.load(path)
    groups = {i: TileGroup.load(BIN / "TILES" / name) for i, name in enumerate(m.groups) if name}
    pal = next(iter(groups.values())).palette
    canvas = Image.new("P", (m.width * TILE, m.height * TILE))
    canvas.putpalette(pal)
    missing = set()
    for p in planes if planes is not None else range(len(m.planes)):
        for i, cell in enumerate(m.planes[p]):
            g, t = cell >> 8, cell & 0xFF
            group = groups.get(g)
            if group is None or t >= len(group.tiles):
                missing.add(cell)
                continue
            tile = Image.frombytes("P", (TILE, TILE), group.tiles[t])
            mask = Image.frombytes("L", (TILE, TILE), bytes(255 if px != TRANSPARENT else 0 for px in group.tiles[t]))
            canvas.paste(tile, ((i % m.width) * TILE, (i // m.width) * TILE), mask)
    canvas.save(out)
    if missing:
        print(f"{len(missing)} cells reference missing tiles, e.g. {sorted(missing)[:8]}", file=sys.stderr)


def render_sprites(path: Path, out: Path, width: int = 1024, pad: int = 2) -> None:
    group = SpriteGroup.load(path)
    frames = [s for s in group.sprites if s]
    x = y = row_h = 0
    places = []
    for s in frames:
        if x + s.width > width:
            x, y, row_h = 0, y + row_h + pad, 0
        places.append((x, y))
        x, row_h = x + s.width + pad, max(row_h, s.height)
    sheet = Image.new("P", (width, y + row_h))
    sheet.putpalette(group.palette or [v for i in range(256) for v in (i, i, i)])
    for s, at in zip(frames, places):
        sheet.paste(Image.frombytes("P", (s.width, s.height), s.pixels), at)
    sheet.save(out)


if __name__ == "__main__":
    kind, src, dst, *rest = sys.argv[1:]
    if kind == "sprites":
        render_sprites(Path(src), Path(dst))
    elif kind == "tiles":
        render_tiles(Path(src), Path(dst))
    else:
        render_map(Path(src), Path(dst), [int(p) for p in rest] or None)
