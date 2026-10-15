"""Lever/sector scripts: map OBJECTS records of type 0x44c (run when the conditions hold) and
0x44d (run when they don't). Interpreter: FUN_1088_a9af.

The game copies a record into a buffer of u16 words: w[0] = word count, w[1] = type, then the
record's words. Here `w` is that buffer.

    w[2] low byte  = number of conditions n; high byte = 1: delete the script after it ran
    w[3 + 2i]      = condition i: a map cell (index into plane 1) holding a "lever" tile,
                     function 0 in 6000..6009 = state 0..9 (FUN_1088_140b)
    w[4 + 2i]      = the state it must have
    then commands from lc = 2 + 2n while lc < w[0]: command = low byte of w[lc+1], its plane
    (gTilePlane) = high byte, arguments w[lc+2] ...

Entering a sector sets its SECTOR icon cell to tile 0x308 (state 1), leaving sets 0x312
(state 0); changing any lever cell runs the scripts that list it (FUN_1088_1364 -> 12f9).

    scripts.py MAP...   prints every script of the maps
"""

import sys
from pathlib import Path

from formats import Map

# (name, words consumed including the command word). Command 3 is variable.
COMMANDS = {
    0x00: ("set_tile cell tile", 3),
    0x01: ("fill_area cell1 cell2 tile", 4),
    0x02: ("effect a b c", 4),
    0x03: ("platform", None),
    0x04: ("platform_flag |= 1", 1),
    0x05: ("platform_speed_scale v", 2),
    0x06: ("platform_flag |= 2", 1),
    0x07: ("abort_if_cell cell tile", 3),
    0x08: ("platform_flags = v", 2),
    0x09: ("platform_param v", 2),
    0x0A: ("enemy a b c", 4),
    0x0B: ("damage_hero n", 2),
    0x0C: ("platform_bounds cell1 cell2", 3),
    0x0D: ("cmd0d", 1),
    0x0E: ("copy_area cell1 cell2 dest", 4),
    0x0F: ("platform_bounds_px x1 y1 x2 y2", 5),
    0x10: ("spawn_icon cell tile", 3),
    0x11: ("cd_track track loop", 3),
    0x12: ("volume v", 2),
    0x13: ("grinder_platform", 1),
    0x14: ("platform_28 v", 2),
    0x15: ("background_procs back fore", 3),
    0x16: ("cmd16 (Random x2)", 1),
    0x17: ("set_tile_alt a b", 3),
    0x18: ("counter_4eac++", 1),
    0x19: ("explosion cell1 cell2", 3),
    0x1A: ("platform_sector cell", 2),
    0x1B: ("cmd1b", 1),
    0x1C: ("hero_launch", 1),
    0x1D: ("grinder_slow", 1),
}


def decode(rec: list[int]) -> tuple[list, bool, list]:
    """rec is Map.object_records() form: rec[0] = type. Returns (conditions, one_shot, commands)."""
    w = [len(rec)] + [v & 0xFFFF for v in rec]
    n, one_shot = w[2] & 0xFF, w[2] >> 8 == 1
    conds = [(w[3 + 2 * i], w[4 + 2 * i]) for i in range(n)]
    cmds, lc = [], 2 + 2 * n
    while lc < w[0]:
        op, plane = w[lc + 1] & 0xFF, w[lc + 1] >> 8
        if op == 0x03:
            count = w[lc + 6]
            points = [(w[lc + 7 + 3 * i], w[lc + 8 + 3 * i], w[lc + 9 + 3 * i]) for i in range(count - 1)]
            cmds.append(("platform", plane, {"area": (w[lc + 2], w[lc + 3]), "start": (w[lc + 4], w[lc + 5]), "points": points}))
            lc += 6 + 3 * (count - 1)
            continue
        name, size = COMMANDS.get(op, (f"?{op:#x}", 1))
        cmds.append((name, plane, w[lc + 2 : lc + 1 + size]))
        lc += size
    return conds, one_shot, cmds


if __name__ == "__main__":
    for path in sys.argv[1:]:
        m = Map.load(Path(path))
        print(f"# {path} ({m.width}x{m.height})")
        for rec in m.object_records():
            if rec[0] not in (0x44C, 0x44D):
                continue
            conds, one_shot, cmds = decode(rec)
            cs = " and ".join(f"cell {c} (tx {c % m.width}, ty {c // m.width}) == {s}" for c, s in conds)
            print(f"{'unless' if rec[0] == 0x44D else 'when'} {cs}{' (once)' if one_shot else ''}:")
            for name, plane, args in cmds:
                print(f"    {name}{f' [plane {plane}]' if plane != 1 else ''}: {args}")
