"""The sprites one frame of the original draws against the port's, as two multisets.

    python3 tools/drawdiff.py DEMO FRAME

Runs `dumpframe.py --draws` (cached in work/frames/<DEMO>/frame<N>.draws) and the port with ET_DRAWLOG=1; prints the
draws only one side has (sprite, screen position, mode, flips, steps; off-screen ones by more than 100 px are left out).
A port draw that clips (platforms) or cuts a sprite itself isn't in its log.
"""
import argparse
import collections
import os
import re
import subprocess
from pathlib import Path

ap = argparse.ArgumentParser()
ap.add_argument("demo")
ap.add_argument("frame", type=int)
o = ap.parse_args()
R = Path(__file__).resolve().parent.parent
d, n = o.demo, o.frame
demo = R / f"work/demos/BIN/USERMAPS/{d}.DMO"
if not demo.exists():
    demo = R / f"work/cd/BIN/USERMAPS/{d}.DMO"
f = R / f"work/frames/{d}/frame{n}.draws"
if not f.exists():
    subprocess.run(["uv", "run", "python", "-W", "ignore", "tools/dumpframe.py", str(demo.relative_to(R)), str(n), "--draws"], cwd=R, check=True, capture_output=True)
env = {**os.environ, "SDL_VIDEODRIVER": "dummy", "SDL_AUDIODRIVER": "dummy", "ET_DRAWLOG": "1"}
port = subprocess.run(["./target/release/eatthis", "--mute", "--demo", str(demo), "--shot", str(n), "/tmp/drawdiff_%d.ppm"], cwd=R / "port", env=env, capture_output=True, text=True).stderr
rx = re.compile(r"(\S+) (0x\w+) at \((-?\d+), (-?\d+)\) mode (0x\w+) colour (0x\w+)")


def key(l):
    m = rx.match(l)
    if not m or not (-100 < int(m[3]) < 420 and -100 < int(m[4]) < 300):
        return None
    mode = int(m[5], 16)
    s = re.search(r"steps (0x\w+) (0x\w+)", l)
    steps = tuple(sorted(s.groups())) if s else ()
    return (int(m[2], 16), int(m[3]), int(m[4]), mode, re.search(r'flips (..)', l)[1], steps)


def load(lines):
    out = collections.defaultdict(list)
    for l in lines:
        k = key(l)
        if k:
            out[k].append(l)
    return out


a, b = load(f.read_text().splitlines()), load(port.splitlines())
for name, x, y in (("orig only", a, b), ("port only", b, a)):
    for k in x:
        for l in x[k][len(y.get(k, ())):]:
            print(f"{name}: {l}")
