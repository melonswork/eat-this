"""Run the original game under the emulator on a demo and dump what it draws: the 320x200 8-bit
frame buffer (_DAT_1170_4b76, what VideoPresent blits) and the 256-entry 6-bit palette
(DAT_1170_6688), at a given demo frame.

    uv run python tools/dumpframe.py work/cd/BIN/USERMAPS/LEVEL1_2.DMO FRAME [FRAME ...] [--out DIR]

Writes DIR/frame<N>.png (default work/frames/<DEMO>/) plus .bin (64000 pixels), .bg (the same buffer
after the background proc, before tiles), .pal (768 bytes), .cam (camera x y, hero x y, previous camera, level frame counter 617e).
Frame N is the state after N calls of GameFrameInGame since DemoLoad, as in tools/tracer.py.
"""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from tracer import (  # noqa: E402
    CD, DEMO_LOAD, DEMO_STOP, DS, GAME_FRAME, GAME_FRAME_IN_GAME, START_LEVEL, demo_files,
)
from win16 import Win16  # noqa: E402
from win16api import FileSystem  # noqa: E402

VIDEO_PRESENT = (0x1008, 0x0363)
BG_OBJECTS = (0x1068, 0x09BF)  # first call per frame: the background proc has drawn its picture and sprites, no tiles yet
FRAME_BUFFER = 0x4B76  # far pointer, 64000 bytes
PALETTE = 0x6688  # far pointer, 768 bytes, 6-bit
CAMERA = 0x1AF0  # gCameraX, gCameraY (int16)
RESERVED = 0x668C  # 256 flags, 1 = colour not in the background range (see re/graphics.md)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("demo")
    ap.add_argument("frames", type=int, nargs="+")
    ap.add_argument("--out")
    ap.add_argument("--camera-trace", metavar="FILE", help="also write frame, camera x y, hero x y for every frame up to the last FRAME")
    ap.add_argument("--menu-frames", type=int, default=5)
    ap.add_argument("--poke", action="append", default=[], metavar="OFF=BYTE",
                    help="set DS:OFF (hex) to BYTE (hex) when the demo starts, e.g. 1a94=1 for the automap")
    ap.add_argument("--overlay", help="folder over work/cd (default work/tracer/overlay; work/demos when the demo is there)")
    ap.add_argument("--peek", action="append", default=[], metavar="OFF:LEN",
                    help="print LEN bytes (hex) of DS:OFF (hex) at each dumped frame; @PTR+ADD:LEN reads through the far pointer at DS:PTR")
    ap.add_argument("--draws", action="store_true", help="write every sprite drawn in each dumped frame to frame<N>.draws: routine, sprite, screen x y, mode (1ae0), colour (1ae3), steps of the scaled ones, caller (selector:ip)")
    o = ap.parse_args()

    game_path, _ = demo_files(o.demo)
    out = Path(o.out or f"work/frames/{Path(o.demo).name.split('.')[0]}")
    out.mkdir(parents=True, exist_ok=True)
    emu = Win16(CD / "ETLAUNCH.EXE", log=lambda *a: None)
    overlay = o.overlay or ("work/demos" if Path(o.demo).resolve().is_relative_to(Path("work/demos").resolve()) else "work/tracer/overlay")
    emu.setup_process(FileSystem(CD, Path(overlay)))
    st = {"menu": 0, "started": False, "frame": 0, "todo": sorted(o.frames)}

    def far(e, off):
        o_, s_ = e.rw(DS, off), e.rw(DS, off + 2)
        return s_, o_

    def on_game_frame(e):
        if not st["started"]:
            st["menu"] += 1
            if st["menu"] == o.menu_frames:
                s = e.scratch
                e.wb(s, 0x800, bytes([len(game_path)]) + game_path.encode())
                e.call_far(*START_LEVEL, (s, 0x800), regs={"ds": DS})

    def on_bg(e):
        if st["started"] and st["todo"] and st["frame"] >= st["todo"][0] and not st.get("bg"):
            st["bg"] = e.rb(*far(e, FRAME_BUFFER), 64000)

    trace = open(o.camera_trace, "w") if o.camera_trace else None

    def on_present(e):
        if trace and st["started"]:
            trace.write(f"{e.rw(DS, 0x617E)} {e.rw(DS, CAMERA)} {e.rw(DS, CAMERA + 2)} {e.rw(DS, 0x4DC4)} {e.rw(DS, 0x4DC6)}\n")
        if not st["started"] or not st["todo"] or st["frame"] < st["todo"][0]:
            st["bg"] = None
            return
        n = st["todo"].pop(0)
        if o.draws:
            (out / f"frame{n}.draws").write_text("\n".join(draws) + "\n")
            draws.clear()
        fb = e.rb(*far(e, FRAME_BUFFER), 64000)
        pal = e.rb(*far(e, PALETTE), 768)
        (out / f"frame{n}.bin").write_bytes(fb)
        if st.get("bg"):
            (out / f"frame{n}.bg").write_bytes(st["bg"])
        (out / f"frame{n}.pal").write_bytes(pal)
        print("camera", [e.rw(DS, CAMERA), e.rw(DS, CAMERA + 2)], "hero", [e.rw(DS, 0x4DC4), e.rw(DS, 0x4DC6)])
        for pk in o.peek:
            if pk.startswith("@"):  # @PTR+ADD:LEN: through the far pointer at DS:PTR
                at, ln = pk[1:].split(":")
                ptr, add = (int(v, 16) for v in (at.split("+") + ["0"])[:2])
                sel, off = e.rw(DS, ptr + 2), e.rw(DS, ptr) + add
                print(f"{sel:04x}:{off:04x}", e.rb(sel, off, int(ln, 16)).hex(" "))
                continue
            off, ln = (int(v, 16) for v in pk.split(":"))
            print(f"DS:{off:04x}", e.rb(DS, off, ln).hex(" "))
        print("bgproc %04x:%04x fgproc %04x:%04x" % (e.rw(DS, 0x4C34), e.rw(DS, 0x4C32), e.rw(DS, 0x4C38), e.rw(DS, 0x4C36)))
        (out / f"frame{n}.cam").write_text(f"{e.rw(DS, CAMERA)} {e.rw(DS, CAMERA + 2)} {e.rw(DS, 0x4DC4)} {e.rw(DS, 0x4DC6)} {e.rw(DS, 0x667C)} {e.rw(DS, 0x667E)} {e.rw(DS, 0x617E)}\n")
        (out / f"frame{n}.res").write_bytes(e.rb(DS, RESERVED, 256))
        from PIL import Image
        im = Image.frombytes("P", (320, 200), fb)
        im.putpalette([(v << 2) | (v >> 4) for v in pal])
        im.save(out / f"frame{n}.png")
        st["bg"] = None
        print(f"frame {n} -> {out}/frame{n}.png", flush=True)
        if not st["todo"]:
            e.uc.emu_stop()

    draws = []
    # 10f0:2670, mirrored (the table 1b54; upside down = 1ae6), the half-size one, the scaled ones.
    for off, name, skip in [(0x2670, "plain", 0), (0x2A11, "hflip", 0), (0x380B, "b54.2", 0), (0x3BF8, "b54.3", 0),
                            (0x3D94, "b54.4", 0), (0x3E68, "b54.5", 0), (0x2D8E, "half", 0), (0x302B, "scaled", 8), (0x3356, "scaledh", 8)]:
        def on_draw(e, name=name, skip=skip):
            if not st["started"] or not st["todo"] or st["frame"] < st["todo"][0]:
                return
            ss, sp = e["ss"], e["sp"]
            i16 = lambda v: v - 0x10000 if v & 0x8000 else v
            w = [e.rw(ss, sp + 2 * k) for k in range(2 + skip // 2 + 3)]
            sid, y, x = w[2 + skip // 2:]
            steps = f" steps {w[2]:#x} {w[4]:#x}" if skip else ""
            draws.append(f"{name} {sid:#06x} at ({i16(x)}, {i16(y)}) mode {e.rb(DS, 0x1AE0, 1)[0]:#x} colour {e.rb(DS, 0x1AE3, 1)[0]:#x} flips {int(name in ('hflip', 'scaledh'))}{int(e.rb(DS, 0x1AE6, 1)[0] != 0)}{steps} from {w[1]:04x}:{w[0]:04x}")
        if o.draws:
            emu.hook(0x10F0, off, on_draw)

    emu.hook(*GAME_FRAME, on_game_frame)
    emu.hook(*GAME_FRAME_IN_GAME, lambda e: st.update(frame=st["frame"] + st["started"]))
    def on_demo_load(e):
        st.update(started=True, frame=0)
        for p in o.poke:
            off, val = p.split("=")
            e.wb(DS, int(off, 16), bytes([int(val, 16)]))

    emu.hook(*DEMO_LOAD, on_demo_load)
    emu.hook(*DEMO_STOP, lambda e: e.uc.emu_stop())
    emu.hook(*BG_OBJECTS, on_bg)
    emu.hook(*VIDEO_PRESENT, on_present)
    emu.run()
    if emu.error:
        print("emulator:", emu.error)


if __name__ == "__main__":
    main()
