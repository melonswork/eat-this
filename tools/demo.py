"""Eat This recorded demos (.REC inside the USERMAPS/*.DMO folders, archive "EAT THIS DEMO").

Recorder: FUN_1098_033a, called from ReadGameInput once per game frame. Player: FUN_1098_06c2,
which replaces ReadGameInput's key/mouse polling. Loader: FUN_1098_0537. Saver: FUN_1098_00ff.

Entries:
    REC HEADER  u32 frames, u16 0, u16 version (3; anything else makes the game warn that the demo
                "IS FROM ANOTHER VERSION AND WILL CERTAINLY NOT PLAY CORRECTLY")
    MOUSE       u16 pairs: (x, y) = mouse position for one frame, or (0x8001, n) = the previous
                position for n more frames
    KEY #0..8   3-byte runs (u16 extra, i8 value): `value` for extra+1 frames. Values are KeyPoll
                states: -1 up, 0 just released, 1 just pressed, 2 held (0/1/2 for the mouse
                buttons merged into LeftShoot/RightShoot). Run 0 is a priming run the loader
                consumes one frame of before the first game frame.
    CHECK       u32 per 35 frames: Borland Pascal RandSeed, sampled in the recorder when the
                level frame counter DAT_1170_617e is 35*k, i.e. at the input read of game frame
                35*k+1 (1-based). CHECK[0] is the seed after the level setup.
                The game never reads it back, so it is a free determinism oracle for a port.

RandSeed is zeroed when recording and playback start.
"""

import struct
import sys
from dataclasses import dataclass
from pathlib import Path

from ilaf import Archive
from formats import entry

KEYS = ["Up", "Down", "Left", "Right", "LeftShoot", "RightShoot", "Key6", "Key7", "SwitchWeapon"]
CHECK_PERIOD = 35
REPEAT = 0x8001


class PascalRandom:
    """Borland Pascal 7 System.Random: 32-bit LCG, RandSeed * 0x08088405 + 1."""

    def __init__(self, seed: int = 0):
        self.seed = seed & 0xFFFFFFFF

    def next(self) -> int:
        self.seed = (self.seed * 0x08088405 + 1) & 0xFFFFFFFF
        return self.seed

    def random(self, n: int) -> int:
        """Random(n) for 0 < n < 65536 (1168:1513): high word of seed * n."""
        return (self.next() * n) >> 32


def unrun_keys(raw: bytes) -> list[int]:
    out = []
    for at in range(0, len(raw), 3):
        extra, value = struct.unpack_from("<Hb", raw, at)
        out += [value] * (extra + 1)
    return out


def unrun_mouse(raw: bytes) -> list[tuple[int, int]]:
    out: list[tuple[int, int]] = []
    for at in range(0, len(raw), 4):
        a, b = struct.unpack_from("<Hh", raw, at)
        if a == REPEAT:
            out += [out[-1]] * b
        else:
            out.append((struct.unpack_from("<h", raw, at)[0], b))
    return out


@dataclass
class Demo:
    frames: int
    version: int
    keys: list[list[int]]  # keys[k][frame], already past the priming run
    mouse: list[tuple[int, int]]  # mouse[frame]
    checks: list[int]  # checks[k] = RandSeed at the input read of frame 35*k+1

    @classmethod
    def load(cls, path: Path) -> "Demo":
        a = Archive.open(str(path))
        frames, _, version = struct.unpack("<IHH", entry(a, "REC HEADER"))
        keys = []
        for i in range(9):
            stream = unrun_keys(entry(a, f"KEY #{i}"))
            if len(stream) != frames + 1:
                raise ValueError(f"KEY #{i}: {len(stream)} frames, header says {frames} (+1 priming)")
            keys.append(stream[1:])
        mouse = unrun_mouse(entry(a, "MOUSE"))
        if len(mouse) < frames:
            raise ValueError(f"MOUSE: {len(mouse)} frames, header says {frames}")
        raw = entry(a, "CHECK")
        checks = list(struct.unpack(f"<{len(raw) // 4}I", raw))
        if len(checks) != frames // CHECK_PERIOD + 1:
            raise ValueError(f"CHECK: {len(checks)} samples for {frames} frames")
        return cls(frames, version, keys, mouse[:frames], checks)

    def rng_calls(self, limit: int = 1_000_000) -> list[int]:
        """Random() calls the original made in each 35-frame window: the number of LCG steps from
        CHECK[k-1] to CHECK[k] (from seed 0 for k=1). All 3550 samples on the CD are reachable."""
        calls, seed = [], 0
        for target in self.checks[1:]:
            rng, n = PascalRandom(seed), 0
            while rng.seed != target:
                if n == limit:
                    raise ValueError(f"CHECK sample {len(calls) + 1} is not {limit} LCG steps from the previous one")
                rng.next()
                n += 1
            calls.append(n)
            seed = target
        return calls

    def input_at(self, frame: int) -> dict:
        """What ReadGameInput sees on game frame `frame` (0-based)."""
        return {"keys": [k[frame] for k in self.keys], "mouse": self.mouse[frame]}


def summary(path: Path) -> None:
    d = Demo.load(path)
    print(f"{path}: {d.frames} frames ({d.frames / 70:.0f} s at 70 Hz), version {d.version}")
    for name, k in zip(KEYS, d.keys):
        presses = sum(1 for v in k if v == 1)
        held = sum(1 for v in k if v > 0)
        print(f"  {name:<12} {presses:4} presses, held {held:5} frames")
    xs, ys = [m[0] for m in d.mouse], [m[1] for m in d.mouse]
    print(f"  mouse        x {min(xs)}..{max(xs)}  y {min(ys)}..{max(ys)}")
    calls = d.rng_calls()
    print(f"  RandSeed     {len(calls)} samples, all reachable; Random() calls per window {min(calls)}..{max(calls)}, total {sum(calls)}")


if __name__ == "__main__":
    for p in sys.argv[1:]:
        summary(Path(p))
