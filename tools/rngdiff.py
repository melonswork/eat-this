"""First Random() whose range or result differs between the tracer (calls.tsv) and the port, over
the whole run in call order (frame labels can be one frame off, so they are only shown):

    ET_RNG_TSV=/tmp/p.tsv port/target/release/eatthis --headless --demo ... ; python3 tools/rngdiff.py LEVEL1_2 /tmp/p.tsv [context]
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
orig = []
for line in (ROOT / "work/tracer" / sys.argv[1] / "calls.tsv").read_text().splitlines()[1:]:
    f = line.split("\t")
    if int(f[1]) == 0:
        continue
    orig.append((int(f[5]), int(f[6]), int(f[1]), f[2]))
port = []
for line in open(sys.argv[2]):
    f = line.rstrip("\n").split("\t")
    if int(f[0]) == 0:
        continue
    port.append((int(f[1]), int(f[2]), int(f[0]), f[3]))
ctx = int(sys.argv[3]) if len(sys.argv) > 3 else 4
for i in range(min(len(orig), len(port))):
    if orig[i][:2] != port[i][:2]:
        print(f"call #{i}: orig frame {orig[i][2]}, port frame {port[i][2]}")
        for j in range(max(0, i - ctx), min(i + ctx, len(orig), len(port))):
            mark = ">>" if j == i else "  "
            print(mark, "orig", orig[j], "\n  ", "port", port[j])
        break
else:
    print("no difference in", min(len(orig), len(port)), "calls")
