"""Value-level check of the whole run: every Random() call (range and RESULT) of the tracer's
calls.tsv against the port's ET_RNG_TSV dump, in call order. A matching RandSeed per window only
proves the call COUNT; two calls swapped inside a window still change the values the objects get.

    mise run trace -- work/cd/BIN/USERMAPS/<DEMO>.DMO          (whole demo)
    python3 tools/valdiff.py <DEMO>
"""
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IGNORE = ("1070:8c72", "1070:34db", "1070:091f", "1068:3112")  # draw-time Randoms under a saved seed (see windiff.py)
d = sys.argv[1]
tsv = ROOT / "work" / f"port_rng_{d}.tsv"
subprocess.run(
    f"cd {ROOT}/port && ET_RNG_TSV={tsv} ./target/release/eatthis --headless --demo ../work/cd/BIN/USERMAPS/{d}.DMO >/dev/null 2>&1",
    shell=True,
)
o = []
for line in (ROOT / "work/tracer" / d / "calls.tsv").read_text().splitlines()[1:]:
    f = line.split("\t")
    if f[3] not in IGNORE:
        o.append((int(f[1]), int(f[5]), int(f[6]), f[2]))
p = []
for line in open(tsv):
    f = line.rstrip("\n").split("\t")
    p.append((int(f[0]), int(f[1]), int(f[2]), f[3].split("::")[-1]))
n = min(len(o), len(p))
bad = [k for k in range(n) if o[k][1:3] != p[k][1:3]]
print(d, "calls: original", len(o), "port", len(p), "- value mismatches:", len(bad))
if bad:
    i = bad[0]
    print("first at call", i)
    for k in range(max(0, i - 3), i + 4):
        print("  orig", o[k], "\n  port", p[k])
