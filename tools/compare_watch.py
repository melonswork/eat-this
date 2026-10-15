"""Compare the port's --dump with the tracer's --watch, frame by frame.

    compare_watch.py TRACER_WATCH.tsv PORT_DUMP.tsv [--from N] [--show K]

Prints the first frames where any shared column differs, with both rows side by side.
"""

import argparse
import csv


def load(path):
    with open(path) as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    return {int(r["frame"]): r for r in rows}


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("tracer")
    ap.add_argument("port")
    ap.add_argument("--from", dest="start", type=int, default=1)
    ap.add_argument("--show", type=int, default=6, help="frames to show around the first difference")
    o = ap.parse_args()
    a, b = load(o.tracer), load(o.port)
    cols = [c for c in next(iter(a.values())) if c != "frame" and c in next(iter(b.values()))]
    for f in sorted(a):
        if f < o.start or f not in b:
            continue
        diff = [c for c in cols if a[f][c] != b[f][c]]
        if diff:
            print(f"first difference at frame {f}: {', '.join(diff)}")
            print("frame  " + "  ".join(f"{c:>18}" for c in cols))
            for g in range(max(o.start, f - 2), f + o.show):
                if g in a and g in b:
                    print(f"{g:5}  " + "  ".join(f"{a[g][c] + ('' if a[g][c] == b[g][c] else '/' + b[g][c]):>18}" for c in cols))
            break
    else:
        print(f"no difference in {len(cols)} columns over frames {o.start}..{max(a)}")
