"""Render a strip of sprite frames by index: frames.py SPRITES/PPHERO.SPR out.png 0x81-0x94 [0xa0-0xa8 ...]"""

import sys
from pathlib import Path

from PIL import Image

from formats import SpriteGroup


def parse(spec: str) -> list[int]:
    a, _, b = spec.partition("-")
    return list(range(int(a, 0), int(b or a, 0) + 1))


if __name__ == "__main__":
    src, dst, *specs = sys.argv[1:]
    group = SpriteGroup.load(Path(src))
    idx = [i for s in specs for i in parse(s)]
    frames = [(i, group.sprites[i] if i < len(group.sprites) else None) for i in idx]
    w = sum((s.width if s else 8) + 2 for _, s in frames)
    h = max((s.height for _, s in frames if s), default=8)
    strip = Image.new("P", (w, h))
    strip.putpalette(group.palette or [v for i in range(256) for v in (i, i, i)])
    x = 0
    for i, s in frames:
        if s:
            strip.paste(Image.frombytes("P", (s.width, s.height), s.pixels), (x, h - s.height))
        print(f"{i:#x}: {'%dx%d hotspot %s' % (s.width, s.height, s.hotspot) if s else 'EMPTY'}")
        x += (s.width if s else 8) + 2
    strip.save(dst)
