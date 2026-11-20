"""Compare the port's sound plays with the original's (re/sound.md).

    python3 tools/sounddiff.py GRINDER [--all]

Needs work/tracer/<DEMO>/sounds.tsv (any `mise run trace` of the demo writes it) and runs the
port headless with ET_SOUND_TSV. Plays are matched by sound id within +-1 frame (the tracer
labels object calls a frame early); matched plays must have the same volumes and pitch.
Prints the totals, the original's unmatched plays by caller (what the port doesn't play yet),
the port's extra plays, and value mismatches.
"""
import collections
import os
import subprocess
import sys
from pathlib import Path


def demo_path(name):
    for root in ("work/cd/BIN/USERMAPS", "work/demos/BIN/USERMAPS"):
        p = Path(root) / f"{name}.DMO"
        if p.exists():
            return p
    sys.exit(f"no demo {name}")


def load(path, orig):
    rows = []
    with open(path) as f:
        next(f)
        for line in f:
            c = line.rstrip("\n").split("\t")
            if c[1] != "play":
                continue
            frame, sid = int(c[0]), int(c[2])
            if orig:
                flags = int(c[6], 16)
                vol = (int(c[3]), int(c[4])) if flags & 0x600 else None
                pitch = int(c[5]) if flags & 0x100 else None
                rows.append((frame, sid, vol, pitch, c[7]))
            else:
                vol = None if c[3] == "-" else (int(c[3]), int(c[4]))
                pitch = None if c[5] == "-" else int(c[5])
                rows.append((frame, sid, vol, pitch, ""))
    return rows


def main():
    name = sys.argv[1]
    show_all = "--all" in sys.argv
    orig = load(Path("work/tracer") / name / "sounds.tsv", True)
    out = Path("work") / f"port_sound_{name}.tsv"
    env = dict(os.environ, ET_SOUND_TSV=str(out.resolve()))
    subprocess.run(["./target/release/eatthis", "--headless", "--demo", f"../{demo_path(name)}"], cwd="port", env=env, capture_output=True)
    port = load(out, False)
    used = [False] * len(port)
    by_id = collections.defaultdict(list)
    for i, r in enumerate(port):
        by_id[r[1]].append(i)
    missing, bad = [], []
    matched = 0
    for o in orig:
        hit = None
        for d in (0, 1, -1, 2):
            for i in by_id[o[1]]:
                if not used[i] and port[i][0] == o[0] + d:
                    hit = i
                    break
            if hit is not None:
                break
        if hit is None:
            missing.append(o)
            continue
        used[hit] = True
        matched += 1
        p = port[hit]
        if p[2] != o[2] or p[3] != o[3]:
            bad.append((o, p))
    extra = [p for i, p in enumerate(port) if not used[i]]
    print(f"{name}: original {len(orig)} plays, port {len(port)}; matched {matched}, missing {len(missing)}, extra {len(extra)}, value mismatches {len(bad)}")
    by_caller = collections.Counter((o[4].split(" <- ")[-1].split("+")[0], o[1]) for o in missing)
    for (caller, sid), n in by_caller.most_common(None if show_all else 25):
        print(f"  missing {n:4} x id {sid:3} from {caller}")
    for p in extra[: None if show_all else 10]:
        print(f"  extra: frame {p[0]} id {p[1]} vol {p[2]} pitch {p[3]}")
    for o, p in bad[: None if show_all else 10]:
        print(f"  value: frame {o[0]} id {o[1]}: original vol {o[2]} pitch {o[3]} ({o[4]}), port vol {p[2]} pitch {p[3]}")


if __name__ == "__main__":
    main()
