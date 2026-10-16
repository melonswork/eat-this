"""Run the original game (work/cd/ETLAUNCH.EXE) under an x86 emulator, play a recorded demo, and
log every Borland Pascal Random call with its call site, game frame and CHECK window.

    uv run python tools/tracer.py work/cd/BIN/USERMAPS/GRINDER.DMO [--windows N] [--out DIR]

The game boots normally (tools/win16.py stubs Windows, DOS file I/O reads work/cd), runs its
menu for a few frames, and then starts the demo level the way the user-maps menu does
(FUN_1080_17fa with the map path). From then on every GameFrame is the original code. RandSeed
is compared with the demo's CHECK entries at the input reads where the recorder sampled them
(frames 35k+1, see re/demo.md).

Output (default work/tracer/<DEMO>/): calls.tsv (one line per Random call), windows.txt (per
window: count, CHECK verdict, call sites by frame), callers.txt (totals). See re/tracer.md.
"""

import argparse
import bisect
import re
import sys
import time
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from demo import CHECK_PERIOD, Demo  # noqa: E402
from win16 import Win16  # noqa: E402
from win16api import FileSystem  # noqa: E402

CD = Path("work/cd")
DECOMP = Path("work/decomp/ETLAUNCH.EXE.c")
NAMES = Path("re/names.csv")

GAME_FRAME = (0x1080, 0x309A)
GAME_FRAME_IN_GAME = (0x1088, 0xB5FF)
START_LEVEL = (0x1080, 0x17FA)  # what the user-maps menu calls with the .map path
DEMO_LOAD = (0x1098, 0x0537)
HERO_RESET = (0x1088, 0x0442)
DEMO_PLAY_FRAME = (0x1098, 0x06C2)
DEMO_STOP = (0x1098, 0x0668)
RANDOM_NEXT = (0x1168, 0x1570)  # every Random variant near-calls this first
RANDOM = (0x1168, 0x1513)
RANDOM_ENTRIES = {0x1516: "Random", 0x1533: "RandomL", 0x1550: "RandomR"}  # return addresses in 1168
RAND_SEED = 0x2398
LEVEL_FRAMES = 0x617E  # u32, +1 at the end of every GameFrameInGame
DS = 0x1170


class Symbols:
    """Function starts from the exported decompilation, names from re/names.csv."""

    def __init__(self):
        starts = {}
        if DECOMP.exists():
            for m in re.finditer(r"^// ===== (\S+) @ ([0-9a-f]{4}):([0-9a-f]{4})$", DECOMP.read_text(), re.M):
                starts[(int(m.group(2), 16), int(m.group(3), 16))] = m.group(1)
        if NAMES.exists():
            for line in NAMES.read_text().splitlines()[1:]:
                addr, name = line.split(",")[:2]
                seg, off = addr.split(":")
                if seg.lower() != "1170":
                    starts[(int(seg, 16), int(off, 16))] = name
        self.by_seg = {}
        for (seg, off), name in sorted(starts.items()):
            self.by_seg.setdefault(seg, []).append((off, name))

    def name(self, seg, off):
        fns = self.by_seg.get(seg, [])
        i = bisect.bisect_right(fns, (off, "￿")) - 1
        if i < 0:
            return f"{seg:04x}:{off:04x}"
        start, name = fns[i]
        return f"{name}+{off - start:#x}"


def demo_files(folder):
    folder = Path(folder)
    scr = next(p for p in folder.iterdir() if p.suffix.upper() == ".SCR")
    rec = next(p for p in folder.iterdir() if p.suffix.upper() == ".REC")
    game_path = f".\\Bin\\usermaps\\{folder.name}\\{scr.stem}.map"
    return game_path, rec


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("demo", help="a demo folder, e.g. work/cd/BIN/USERMAPS/GRINDER.DMO")
    ap.add_argument("--windows", type=int, default=0, help="stop after this many CHECK windows (0 = whole demo)")
    ap.add_argument("--out", help="output folder (default work/tracer/<demo>)")
    ap.add_argument("--keep-going", action="store_true", help="don't stop at the first CHECK mismatch")
    ap.add_argument("--menu-frames", type=int, default=5, help="menu frames to run before starting the demo")
    ap.add_argument("--pre-seed", type=lambda x: int(x, 0), help="RandSeed for HeroReset's four draws before DemoLoad zeroes it")
    ap.add_argument("--ini", action="append", default=[], metavar="KEY=VALUE", help="EatThis.ini setting, e.g. GameDetail=4")
    ap.add_argument("--watch", default="", help="comma-separated globals (names from re/names.csv or 1170:xxxx) to log as i16 at the start of every game frame into watch.tsv")
    ap.add_argument("--dump-mem", action="append", default=[], metavar="SEL:OFF:LEN", help="hex-dump memory into mem_SEL_OFF.bin when the demo starts (DemoLoad), e.g. 1170:07b2:400")
    ap.add_argument("--entry", action="append", default=[], metavar="NAME|SEL:OFF", help="log every entry into this function with the frame and its callers (BP chain) into entries.txt")
    ap.add_argument("--self", type=lambda v: int(v, 0), default=0, metavar="LEN", help="with --entry: also log the first LEN bytes of the far pointer argument on top of the stack (a method's Self) as signed words")
    ap.add_argument("--dump-groups", action="store_true", help="at DemoLoad, write the sprite group slot table (1170:6188) with the first bytes of each group into groups.txt")
    ap.add_argument("--platforms", action="store_true", help="log the first 0x40 bytes (as i16) of every platform (gPlatforms, 1170:4662) at the start of every game frame into platforms.tsv")
    ap.add_argument("--particles", default="", metavar="A-B", help="dump the particle array (1170:25b0, count 1170:0918) at the start of frames A..B into particles.tsv: frame, index, x, y, age, life, mask(+34), state(+3e), callback(+42), mover(+4a)")
    ap.add_argument("--writes", default="", help="comma-separated globals; log every write to them (frame, CS:IP, function, new value) into writes.tsv")
    ap.add_argument("-v", "--verbose", action="store_true", help="log file opens and Windows calls")
    o = ap.parse_args()

    game_path, rec = demo_files(o.demo)
    demo = Demo.load(rec)
    expected = demo.rng_calls()
    name = Path(o.demo).name.split(".")[0]
    out = Path(o.out or f"work/tracer/{name}")
    out.mkdir(parents=True, exist_ok=True)
    syms = Symbols()

    log = print if o.verbose else (lambda *a: None)
    emu = Win16(CD / "ETLAUNCH.EXE", log=log)
    fs = FileSystem(CD, Path("work/tracer/overlay"))
    for kv in o.ini:
        k, v = kv.split("=", 1)
        fs.ini[k.upper()] = v
    emu.setup_process(fs)

    st = {
        "menu": 0,  # GameFrame calls before the demo starts
        "started": False,  # DemoLoad seen: RandSeed was just zeroed
        "frame": 0,  # GameFrameInGame calls since DemoLoad (0 = level setup)
        "played": 0,  # DemoPlayFrame calls
        "window": 1,  # CHECK window of the next Random call
        "calls": [],  # (frame, caller) in the current window
        "windows": [],  # (k, calls, seed, expected seed, ok)
        "done": False,
    }
    names = {}
    for line in Path("re/names.csv").read_text().splitlines()[1:]:
        addr, name = line.split(",")[:2]
        names[name] = addr
    watch = [(w, int((names.get(w) or w).split(":")[1], 16)) for w in o.watch.split(",") if w]
    watch_f = open(out / "watch.tsv", "w") if watch else None
    if watch_f:
        watch_f.write("frame\t" + "\t".join(w for w, _ in watch) + "\n")
    # (name, linear address): selector s has linear base (s >> 3) << 16 (see win16.py).
    writes = []
    for w in (w for w in o.writes.split(",") if w):
        sel, off = (int(v, 16) for v in (names.get(w) or w).split(":"))
        writes.append((w, ((sel >> 3) << 16) + off))
    writes_f = open(out / "writes.tsv", "w") if writes else None
    part_f = open(out / "particles.tsv", "w") if o.particles else None
    part_range = tuple(int(v) for v in o.particles.split("-")) if o.particles else (0, 0)
    calls_f = open(out / "calls.tsv", "w")
    calls_f.write("window\tframe\tcaller\tsite\tkind\tn\tresult\tseed\n")

    def stop(why):
        st["done"] = why
        emu.uc.emu_stop()

    def on_game_frame(e):
        if not st["started"]:
            st["menu"] += 1
            if st["menu"] == o.menu_frames:
                s = e.scratch
                e.wb(s, 0x800, bytes([len(game_path)]) + game_path.encode())
                log(f"starting {game_path}")
                e.call_far(*START_LEVEL, (s, 0x800), regs={"ds": DS})
            elif st["menu"] > o.menu_frames + 400:
                stop("the demo never started")

    plat_f = open(out / "platforms.tsv", "w") if o.platforms else None

    def on_in_game(e):
        if st["started"]:
            st["frame"] += 1
            if plat_f:
                for i in range(e.rw(DS, 0x0EBC)):
                    off, sel = e.rw(DS, 0x4662 + 4 * i), e.rw(DS, 0x4664 + 4 * i)
                    if sel:
                        ws = [e.rw(sel, off + 2 * k) for k in range(0x20)]
                        plat_f.write(f"{st['frame']}\t{i}\t" + "\t".join(str(v - 0x10000 if v & 0x8000 else v) for v in ws) + "\n")
            if part_f and part_range[0] <= st["frame"] <= part_range[1]:
                for i in range(e.rw(DS, 0x0918)):
                    off, sel = e.rw(DS, 0x25B0 + 4 * i), e.rw(DS, 0x25B2 + 4 * i)
                    ws = [e.rw(sel, off + k) for k in (8, 0xA, 0x24, 0x22, 0x34, 0x3E, 0x42, 0x4A)]
                    part_f.write(f"{st['frame']}\t{i}\t" + "\t".join(str(v - 0x10000 if v & 0x8000 else v) for v in ws) + "\n")
            if watch_f:
                vals = [e.rw(DS, a) for _, a in watch]
                watch_f.write(f"{st['frame']}\t" + "\t".join(str(v - 0x10000 if v & 0x8000 else v) for v in vals) + "\n")

    def on_demo_load(e):
        st.update(started=True, frame=0, played=0, window=1, calls=[])
        if o.dump_groups:
            with open(out / "groups.txt", "w") as g:
                for slot in range(0x100):
                    off, sel = e.rw(DS, 0x6188 + 4 * slot), e.rw(DS, 0x618a + 4 * slot)
                    if sel:
                        head = bytes(e.rb(sel, off, 0x30))
                        g.write(f"{slot:#04x} {sel:04x}:{off:04x} {head.hex(' ')} {head!r}\n")
        for spec in o.dump_mem:
            sel, off, ln = (int(v, 16) for v in spec.split(":"))
            data = bytes(e.rb(sel, off, ln))
            (out / f"mem_{sel:04x}_{off:04x}.bin").write_bytes(data)

    def on_play_frame(e):
        # The recorder (DemoRecordFrame 1098:04ec) samples on the level frame counter
        # DAT_1170_617e, which GameFrameInGame increments after this frame's input read:
        # CHECK[k] is RandSeed at the input read of game frame 35k+1.
        st["played"] += 1
        n = e.rd(DS, LEVEL_FRAMES)
        if n % CHECK_PERIOD:
            return
        k = n // CHECK_PERIOD
        seed = e.rd(DS, RAND_SEED)
        want = demo.checks[k] if k < len(demo.checks) else None
        ok = seed == want
        if k == 0:
            print(f"CHECK[0] (after level setup): RandSeed {seed:#010x} {'ok' if ok else f'!= {want:#010x}'}")
            return
        st["windows"].append((k, st["calls"], seed, want, ok))
        st["calls"] = []
        st["window"] = k + 1
        mark = "ok" if ok else f"MISMATCH: want {want:#010x}" + (f" ({expected[k - 1]} calls)" if k <= len(expected) else "")
        print(f"window {k:4}: {len(st['windows'][-1][1]):4} calls  RandSeed {seed:#010x}  {mark}", flush=True)
        if not ok and not o.keep_going:
            stop(f"RandSeed differs from CHECK[{k}]")
        elif o.windows and k >= o.windows:
            stop(f"{k} windows done")

    def on_demo_stop(e):
        if st["started"]:
            stop("demo finished")

    def on_random_next(e):
        if not st["started"]:
            return
        ss, sp = e["ss"], e["sp"]
        near = e.rw(ss, sp)
        if near in RANDOM_ENTRIES:  # Random variants call RandomNext first: the far caller is next
            cs, ip = e.rfar(ss, sp + 2)
            kind = RANDOM_ENTRIES[near]
            n = e.rw(ss, sp + 6) if kind == "Random" else 0
        else:
            cs, ip, kind, n = 0x1168, near, "RandomNext", 0
        nxt = (e.rd(DS, RAND_SEED) * 0x08088405 + 1) & 0xFFFFFFFF
        result = (nxt * n) >> 32 if kind == "Random" else nxt
        caller = syms.name(cs, ip - 5)  # the call instruction, so a call in tail position maps to its function
        st["calls"].append((st["frame"], caller))
        calls_f.write(f"{st['window']}\t{st['frame']}\t{caller}\t{cs:04x}:{ip:04x}\t{kind}\t{n}\t{result}\t{nxt:#010x}\n")

    if o.pre_seed is not None:
        emu.hook(*HERO_RESET, lambda e: e.wd(DS, RAND_SEED, o.pre_seed))
    emu.hook(*GAME_FRAME, on_game_frame)
    emu.hook(*GAME_FRAME_IN_GAME, on_in_game)
    emu.hook(*DEMO_LOAD, on_demo_load)
    emu.hook(*DEMO_PLAY_FRAME, on_play_frame)
    emu.hook(*DEMO_STOP, on_demo_stop)
    emu.hook(*RANDOM_NEXT, on_random_next)

    if writes_f:
        from unicorn import UC_HOOK_MEM_WRITE
        from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_IP

        writes_f.write("frame\tvar\tsite\tcaller\tvalue\n")
        by_addr = {a: w for w, a in writes}

        def on_write(uc, access, address, size, value, data):
            if not st["started"]:
                return
            w = by_addr.get(address) or by_addr.get(address - 1)
            if w is None:
                return
            cs, ip = uc.reg_read(UC_X86_REG_CS), uc.reg_read(UC_X86_REG_IP)
            v = value & 0xFFFF if size >= 2 else value & 0xFF
            writes_f.write(f"{st['frame']}\t{w}\t{cs:04x}:{ip:04x}\t{syms.name(cs, ip)}\t{v - 0x10000 if v & 0x8000 else v}\n")

        for _, a in writes:
            emu.uc.hook_add(UC_HOOK_MEM_WRITE, on_write, begin=a, end=a + 1)

    entries_f = open(out / "entries.txt", "w") if o.entry else None
    for spec in o.entry:
        if ":" in spec:
            sel, off = (int(v, 16) for v in spec.split(":"))
        else:
            sel, off = (int(v, 16) for v in names[spec].split(":"))

        def on_entry(e, spec=spec):
            if not st["started"]:
                return
            ss, sp, bp = e["ss"], e["sp"], e["bp"]
            # At entry the return address is on top: try far (ip, cs) first.
            ip, cs = e.rw(ss, sp), e.rw(ss, sp + 2)
            chain = [syms.name(cs, ip - 5) if 0x1000 <= cs < 0x1170 else syms.name(e["cs"], ip - 3)]
            for _ in range(8):
                if not bp:
                    break
                ip, cs = e.rw(ss, bp + 2), e.rw(ss, bp + 4)
                chain.append(syms.name(cs, ip - 5) if 0x1000 <= cs < 0x1170 else syms.name(e["cs"], ip - 3) + "?")
                bp = e.rw(ss, bp)
            line = f"{st['frame']}\t{spec}\t" + " <- ".join(chain)
            if o.self:
                so, sg = e.rw(ss, sp + 4), e.rw(ss, sp + 6)
                raw = bytes(e.rb(sg, so, o.self))
                words = [int.from_bytes(raw[i:i + 2], "little", signed=True) for i in range(0, len(raw) - 1, 2)]
                line += f"\tself={sg:04x}:{so:04x}\t" + " ".join(str(w) for w in words)
            entries_f.write(line + "\n")

        emu.hook(sel, off, on_entry)

    t = time.time()
    emu.run()
    calls_f.close()
    if watch_f:
        watch_f.close()
    if writes_f:
        writes_f.close()
    if plat_f:
        plat_f.close()
    if entries_f:
        entries_f.close()
    why =st["done"] or emu.error or "stopped"
    print(f"{why}; {st['played']} demo frames, {len(st['windows'])} windows in {time.time() - t:.1f} s")
    if st["calls"]:
        print(f"{len(st['calls'])} calls after the last CHECK sample are in calls.tsv only")
    if emu.messages:
        print("message boxes:", emu.messages)

    with open(out / "windows.txt", "w") as f:
        good = 0
        for k, calls, seed, want, ok in st["windows"]:
            good += ok and good == k - 1
            f.write(f"window {k}: {len(calls)} calls (CHECK says {expected[k - 1] if k - 1 < len(expected) else '?'}), "
                    f"RandSeed {seed:#010x} {'ok' if ok else f'!= CHECK {want:#010x}'}\n")
            by_frame = {}
            for frame, caller in calls:
                by_frame.setdefault(frame, []).append(caller)
            for frame, cs in by_frame.items():
                f.write(f"  frame {frame:5}: {len(cs):3}  " + ", ".join(f"{c} x{m}" if m > 1 else c for c, m in Counter(cs).items()) + "\n")
    totals = Counter(c.split("+")[0] for _, calls, *_ in st["windows"] for _, c in calls)
    with open(out / "callers.txt", "w") as f:
        for name, n in totals.most_common():
            f.write(f"{n:8}  {name}\n")
    print(f"CHECK matched for the first {good} windows; wrote {out}/calls.tsv, windows.txt, callers.txt")
    return 0 if st["windows"] and all(w[4] for w in st["windows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
