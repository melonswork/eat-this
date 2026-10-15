"""Run the original game under the emulator on a demo and dump what it draws: the 320x200 8-bit
frame buffer (_DAT_1170_4b76, what VideoPresent blits) and the 256-entry 6-bit palette
(DAT_1170_6688), at a given demo frame.

    uv run python tools/dumpframe.py work/cd/BIN/USERMAPS/LEVEL1_2.DMO FRAME [FRAME ...] [--out DIR]

Writes DIR/frame<N>.png (default work/frames/<DEMO>/) plus .bin (64000 pixels) and .pal (768 bytes).
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
FRAME_BUFFER = 0x4B76  # far pointer, 64000 bytes
PALETTE = 0x6688  # far pointer, 768 bytes, 6-bit
CAMERA = 0x1AF0  # gCameraX, gCameraY (int16)
RESERVED = 0x668C  # 256 flags, 1 = colour not in the background range (see re/graphics.md)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("demo")
    ap.add_argument("frames", type=int, nargs="+")
    ap.add_argument("--out")
    ap.add_argument("--menu-frames", type=int, default=5)
    o = ap.parse_args()

    game_path, _ = demo_files(o.demo)
    out = Path(o.out or f"work/frames/{Path(o.demo).name.split('.')[0]}")
    out.mkdir(parents=True, exist_ok=True)
    emu = Win16(CD / "ETLAUNCH.EXE", log=lambda *a: None)
    emu.setup_process(FileSystem(CD, Path("work/tracer/overlay")))
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

    def on_present(e):
        if not st["started"] or not st["todo"] or st["frame"] < st["todo"][0]:
            return
        n = st["todo"].pop(0)
        fb = e.rb(*far(e, FRAME_BUFFER), 64000)
        pal = e.rb(*far(e, PALETTE), 768)
        (out / f"frame{n}.bin").write_bytes(fb)
        (out / f"frame{n}.pal").write_bytes(pal)
        print("camera", [e.rw(DS, CAMERA), e.rw(DS, CAMERA + 2)], "hero", [e.rw(DS, 0x4DC4), e.rw(DS, 0x4DC6)])
        print("bgproc %04x:%04x fgproc %04x:%04x" % (e.rw(DS, 0x4C34), e.rw(DS, 0x4C32), e.rw(DS, 0x4C38), e.rw(DS, 0x4C36)))
        (out / f"frame{n}.cam").write_text(f"{e.rw(DS, CAMERA)} {e.rw(DS, CAMERA + 2)} {e.rw(DS, 0x4DC4)} {e.rw(DS, 0x4DC6)} {e.rw(DS, 0x667C)} {e.rw(DS, 0x667E)}\n")
        (out / f"frame{n}.res").write_bytes(e.rb(DS, RESERVED, 256))
        from PIL import Image
        im = Image.frombytes("P", (320, 200), fb)
        im.putpalette([(v << 2) | (v >> 4) for v in pal])
        im.save(out / f"frame{n}.png")
        print(f"frame {n} -> {out}/frame{n}.png", flush=True)
        if not st["todo"]:
            e.uc.emu_stop()

    emu.hook(*GAME_FRAME, on_game_frame)
    emu.hook(*GAME_FRAME_IN_GAME, lambda e: st.update(frame=st["frame"] + st["started"]))
    emu.hook(*DEMO_LOAD, lambda e: st.update(started=True, frame=0))
    emu.hook(*DEMO_STOP, lambda e: e.uc.emu_stop())
    emu.hook(*VIDEO_PRESENT, on_present)
    emu.run()
    if emu.error:
        print("emulator:", emu.error)


if __name__ == "__main__":
    main()
