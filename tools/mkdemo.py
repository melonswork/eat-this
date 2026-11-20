"""Make a NEW oracle demo: the original game (under the tracer's emulator) records it from a
scripted input, with its own recorder (DemoStartRecording / DemoRecordFrame / DemoSave), so the
.REC it writes has the original's CHECK entries (RandSeed every 35 frames).

    uv run python tools/mkdemo.py tools/scenarios/DEATH1.txt [--out work/demos]

writes work/demos/BIN/USERMAPS/<NAME>.DMO/{<MAP>.MAP,<MAP>.SCR,<NAME>.REC}. Replay it with the
tracer (`--overlay work/demos`) and the port (`--demo ../work/demos/BIN/USERMAPS/<NAME>.DMO`).

How: the level script's `PlayRecord=<NAME>.REC` names a file that doesn't exist yet, so LevelLoad
calls DemoStartRecording (RandSeed := 0) instead of DemoLoad. Every frame ReadGameInput then polls
the keyboard (GetKeyState, answered from the script), the mouse buttons (DAT_1170_1a5c/1a5e,
set between frames like the WM_*BUTTON* handlers 1008:1c0e.. do) and the cursor (GetCursorPos),
and DemoRecordFrame stores the result. After the last scripted frame the tool calls DemoSave.

Script (one command per line, '#' starts a comment):

    level GRINDER          a CD demo folder (BIN/USERMAPS/GRINDER.DMO: its .MAP/.SCR are copied)
                           or a map under work/cd/BIN, e.g. MAPS/CITY01.MAP
    name DEATH1            output folder/REC name (default: the script's file name)
    start 940 191          move the hero's start (MAP HEADER) in the demo's copy of the map
    cell 2 61 9 0x1234     set a cell (plane, tile x, tile y, value) in the copy (tools/mapedit.py)
    mouse 160 100          mouse position in game-screen pixels from the next frame on
    40 right fire          hold these keys for 40 frames (nothing listed = all released)

Keys: up down left right fire alt key6 key7 switch (the game's key map gKeyMap 1170:1640, i.e.
gInput 0..8), kp8 kp2 kp4 kp6 kp0 (the numpad fallbacks; they report 0 on release where the
mapped keys are overwritten by the numpad's -1), lmb rmb (mouse buttons -> LeftShoot/RightShoot).
Script frame i is game frame i+1 (the port's frame i). Digit/PgUp/PgDn weapon keys are not
offered: DemoPlayFrame doesn't replay them.
"""

import argparse
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from win16 import Win16  # noqa: E402
from win16api import FileSystem  # noqa: E402

CD = Path("work/cd")
DS = 0x1170
GAME_FRAME = (0x1080, 0x309A)
START_LEVEL = (0x1080, 0x17FA)
READ_GAME_INPUT = (0x1088, 0x53A5)
DEMO_START_RECORDING = (0x1098, 0x0002)
DEMO_SAVE = (0x1098, 0x00FF)
GDEMO = 0x5374
KEY_MAP = 0x1640
MOUSE_BUTTONS = (0x1A5C, 0x1A5E)
GDEMO_RECORDING = 0x1952

KEYS = ["up", "down", "left", "right", "fire", "alt", "key6", "key7", "switch"]
NUMPAD = {"kp8": 0x68, "kp2": 0x62, "kp4": 0x64, "kp6": 0x66, "kp0": 0x60}
BUTTONS = ["lmb", "rmb"]


def parse(path):
    """-> (level, name, frames, edits): frames[i] = (set of key names, (mouse x, y))."""
    level, name, frames, mouse = None, Path(path).stem.upper(), [], (160, 100)
    edits = []
    for n, line in enumerate(Path(path).read_text().splitlines(), 1):
        words = line.split("#")[0].split()
        if not words:
            continue
        cmd = words[0].lower()
        if cmd == "level":
            level = words[1]
        elif cmd == "name":
            name = words[1].upper()
        elif cmd == "start":
            edits += ["--start", words[1], words[2]]
        elif cmd == "cell":
            edits += ["--cell", *words[1:5]]
        elif cmd == "mouse":
            mouse = (int(words[1]), int(words[2]))
        elif cmd.isdigit():
            keys = {w.lower() for w in words[1:]}
            bad = keys - set(KEYS) - set(NUMPAD) - set(BUTTONS)
            if bad:
                raise SystemExit(f"{path}:{n}: unknown keys {sorted(bad)}")
            frames += [(keys, mouse)] * int(cmd)
        else:
            raise SystemExit(f"{path}:{n}: unknown command {cmd!r}")
    if not level:
        raise SystemExit(f"{path}: no 'level' line")
    if not frames:
        raise SystemExit(f"{path}: no frames")
    if len(frames) % 35 == 0:
        # DemoSave writes frames/35 + 1 CHECKs; the last one would be sampled at the input read
        # of frame `frames` + 1, which never comes, and stay garbage.
        frames.append(frames[-1])
    return level, name, frames, edits


def make_folder(level, name, root, edits=()):
    """Copy the level into root/BIN/USERMAPS/<name>.DMO with PlayRecord=<name>.REC; -> (folder, game path)."""
    src = CD / "BIN" / "USERMAPS" / f"{level.upper()}.DMO"
    if src.is_dir():
        files = [p for p in src.iterdir() if p.suffix.upper() != ".REC"]
    else:
        m = CD / "BIN" / level
        files = [p for p in m.parent.iterdir() if p.stem.upper() == m.stem.upper() and p.suffix.upper() in (".MAP", ".SCR")]
    scr = next(p for p in files if p.suffix.upper() == ".SCR")
    folder = root / "BIN" / "USERMAPS" / f"{name}.DMO"
    if folder.exists():
        shutil.rmtree(folder)
    folder.mkdir(parents=True)
    for p in files:
        if p != scr:
            shutil.copy(p, folder / p.name.upper())
    lines = [ln for ln in scr.read_bytes().split(b"\n") if not ln.strip().lower().startswith(b"playrecord=")]
    lines.insert(0, f"PlayRecord={name.lower()}.rec\r".encode())
    (folder / scr.name.upper()).write_bytes(b"\n".join(lines))
    if edits:
        import subprocess

        m = next(folder / p.name.upper() for p in files if p.suffix.upper() == ".MAP")
        subprocess.run([sys.executable, str(Path(__file__).parent / "mapedit.py"), str(m), str(m), *edits], check=True)
    return folder, f".\\Bin\\usermaps\\{folder.name}\\{scr.stem}.map"


def record(game_path, frames, root, menu_frames=5, verbose=False):
    log = print if verbose else (lambda *a: None)
    emu = Win16(CD / "ETLAUNCH.EXE", log=log)
    emu.setup_process(FileSystem(CD, root))
    st = {"menu": 0, "recording": False, "reads": 0, "done": None}
    held = set()  # VKs held during the current ReadGameInput
    emu.key_state = lambda vk: 0x80 if vk in held else 0

    def cursor_for(x, y):
        # FUN_1088_42a3: gMouseX = cursor.x * 319 / DAT_1170_1636 (screen width, a long),
        # gMouseY = cursor.y * 199 / DAT_1170_163a, truncated toward zero. Invert it.
        out = []
        for v, scale, size_at in ((x, 319, 0x1636), (y, 199, 0x163A)):
            size = emu.rd(DS, size_at)
            c = v * size // scale
            for d in range(-2, 3):
                p = (c + d) * scale
                if (abs(p) // size) * (1 if p >= 0 else -1) == v:
                    out.append(c + d)
                    break
            else:
                raise SystemExit(f"mouse coordinate {v} is unreachable with a screen size of {size}")
        return tuple(out)

    def stop(why):
        st["done"] = why
        emu.uc.emu_stop()

    def on_game_frame(e):
        if not st["recording"]:
            st["menu"] += 1
            if st["menu"] == menu_frames:
                s = e.scratch
                e.wb(s, 0x800, bytes([len(game_path)]) + game_path.encode())
                e.call_far(*START_LEVEL, (s, 0x800), regs={"ds": DS})
            elif st["menu"] > menu_frames + 400:
                stop("the level never started recording")
            return
        i = st["reads"]
        if i >= len(frames) or not e.rb(DS, GDEMO_RECORDING, 1)[0]:
            if e.rb(DS, GDEMO_RECORDING, 1)[0]:
                e.call_far(*DEMO_SAVE, (DS, GDEMO), regs={"ds": DS})
                stop(f"saved after {i} frames")
            else:
                stop(f"the game stopped recording by itself after {i} frames")
            return
        keys = frames[i][0]
        for b, off in zip(BUTTONS, MOUSE_BUTTONS):  # what WM_xBUTTONDOWN/UP leave between frames
            if b not in keys:
                e.ww(DS, off, 0)
            elif e.rw(DS, off) == 0:
                e.ww(DS, off, 1)

    def on_start_recording(e):
        st["recording"] = True
        e.cursor = cursor_for(*frames[0][1])  # LevelFirstFrame reads the cursor too

    def on_read_input(e):
        if not st["recording"]:
            return
        i = st["reads"]
        keys, mouse = frames[min(i, len(frames) - 1)]
        keymap = e.rb(DS, KEY_MAP, 9)
        held.clear()
        held.update(keymap[KEYS.index(k)] for k in keys if k in KEYS)
        held.update(NUMPAD[k] for k in keys if k in NUMPAD)
        e.cursor = cursor_for(*mouse)
        st["reads"] += 1

    emu.hook(*GAME_FRAME, on_game_frame)
    emu.hook(*DEMO_START_RECORDING, on_start_recording)
    emu.hook(*READ_GAME_INPUT, on_read_input)
    emu.run()
    return st["done"] or emu.error or "stopped"


def script_from_rec(rec, level):
    """A script that presses what a recorded .REC shows (to re-record an existing demo)."""
    from demo import Demo

    d = Demo.load(Path(rec))
    names = []
    for k, stream in enumerate(d.keys):
        if k < 4:
            names.append(list(NUMPAD)[k] if 0 in stream else KEYS[k])
        elif k < 6:
            names.append(BUTTONS[k - 4] if -1 not in stream else KEYS[k])
        else:
            names.append(KEYS[k])
    out, prev, run, mouse = [f"level {level}"], None, 0, None
    for f in range(d.frames):
        cur = (tuple(n for n, s in zip(names, d.keys) if s[f] >= 1), d.mouse[f])
        if cur != prev and run:
            out.append(f"{run} {' '.join(prev[0])}".rstrip())
            run = 0
        if cur[1] != mouse:
            out.append(f"mouse {cur[1][0]} {cur[1][1]}")
            mouse = cur[1]
        prev, run = cur, run + 1
    out.append(f"{run} {' '.join(prev[0])}".rstrip())
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("script", help="scenario script (see the module docstring)")
    ap.add_argument("--out", default="work/demos", help="overlay root for the new demo folder (default work/demos)")
    ap.add_argument("-v", "--verbose", action="store_true")
    ap.add_argument("--from-rec", metavar="REC", help="print a script that replays this .REC's input (the script argument is then the level)")
    o = ap.parse_args()
    if o.from_rec:
        print(script_from_rec(o.from_rec, o.script), end="")
        return 0
    level, name, frames, edits = parse(o.script)
    root = Path(o.out)
    folder, game_path = make_folder(level, name, root, edits)
    why = record(game_path, frames, root, verbose=o.verbose)
    rec = next((p for p in folder.iterdir() if p.suffix.upper() == ".REC"), None)
    print(f"{why}; {len(frames)} scripted frames -> {rec or 'NO .REC WRITTEN'}")
    return 0 if rec else 1


if __name__ == "__main__":
    sys.exit(main())
