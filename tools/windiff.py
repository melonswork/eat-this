"""Per-frame Random() counts of the original (the tracer's calls.tsv) against the port's --trace,
for one CHECK window or a frame range:

    python3 tools/windiff.py LEVEL1_2 107          (window 107)
    python3 tools/windiff.py LEVEL1_2 3700 3760    (frames)

Run the tracer far enough first (mise run trace -- ... --windows N). Prints the frames that differ
with the original's callers.
"""

import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# Call sites inside drawing code that saves and restores RandSeed (no effect on the game).
IGNORE = {"1070:8c72", "1070:34db", "1070:091f", "1068:3112", "1070:51af"}  # draw-time Randoms under a saved seed: the big alien's shield (FUN_1070_7eb2), the hovering machine (30b6), the electric fence/bot (06f6), the meat hooks (4b9b)


def main():
    name = sys.argv[1]
    if len(sys.argv) == 3:
        w = int(sys.argv[2])
        a, b = 35 * (w - 1) + 1, 35 * w
    else:
        a, b = int(sys.argv[2]), int(sys.argv[3])
    orig = Counter()
    callers = defaultdict(list)
    for line in (ROOT / "work/tracer" / name / "calls.tsv").read_text().splitlines()[1:]:
        f = line.split("\t")
        fr = int(f[1])
        if a <= fr <= b and f[3] not in IGNORE:
            orig[fr] += 1
            callers[fr].append(f"{f[2]}({f[5]})")
    demo = ROOT / "work/cd/BIN/USERMAPS" / f"{name}.DMO"
    if not demo.exists():  # a demo made by tools/mkdemo.py
        demo = ROOT / "work/demos/BIN/USERMAPS" / f"{name}.DMO"
    out = subprocess.run(
        [str(ROOT / "port/target/release/eatthis"), "--headless", "--demo", str(demo), "--trace", str(a), str(b + 1)],
        capture_output=True, text=True, cwd=ROOT / "port",
    )
    port = Counter()
    for line in (out.stdout + out.stderr).splitlines():
        if line.startswith("frame "):
            p = line.split()
            port[int(p[1])] = int(p[-1])
    # The tracer labels a frame at its input read (in HeroUpdate): the hero's calls carry the
    # port's frame number, the objects' calls (later in the frame) one less.
    for fr in range(a, b + 1):
        if orig[fr] != port[fr] or orig[fr]:
            mark = "<--" if orig[fr] != port[fr] else ""
            print(f"{fr:6}  original {orig[fr]:3}  port {port[fr]:3}  {mark:3} {' '.join(callers[fr])[:200]}")


main()
