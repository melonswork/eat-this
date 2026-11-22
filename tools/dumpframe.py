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
                    help="print LEN bytes (hex) of DS:OFF (hex) at each dumped frame")
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
        fb = e.rb(*far(e, FRAME_BUFFER), 64000)
        pal = e.rb(*far(e, PALETTE), 768)
        (out / f"frame{n}.bin").write_bytes(fb)
        if st.get("bg"):
            (out / f"frame{n}.bg").write_bytes(st["bg"])
        (out / f"frame{n}.pal").write_bytes(pal)
        print("camera", [e.rw(DS, CAMERA), e.rw(DS, CAMERA + 2)], "hero", [e.rw(DS, 0x4DC4), e.rw(DS, 0x4DC6)])
        for pk in o.peek:
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
