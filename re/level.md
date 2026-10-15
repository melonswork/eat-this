# Level logic: sectors, levers, scripts, platforms, the grinder

Ported in `port/src/world.rs`, `level.rs` and `effects.rs`, and checked against the original
with the tracer (`mise run trace -- ... --watch/--writes/--platforms`). The GRINDER demo replays
exactly: all 113 CHECK windows, and the hero and grinder match frame by frame.

## Sectors (FUN_1068_03e8)

The level is cut into rectangles by STOP tiles (function 0 = 7100). From each STOP tile the
game floods its four neighbours: from the neighbour it goes left, then up (from the left
edge), then right, then down (on the right edge), until it meets a STOP tile or the map edge.
A sector keeps:

- that rectangle (+10..+0x10), which bounds the camera,
- a containment rectangle (+2..+8) whose map-edge sides are opened to ±0x7fff,
- the cell of its SECTOR icon (function 61700, found in planes 2 and 1 by
  `SpawnFromIconTiles`).

`FUN_1068_021c` returns the last-built sector that contains a point, or the whole-map default
sector.

## Levers and scripts

A main-plane cell whose tile has function 0 in 6000..6009 is a *lever* in state 0..9
(`FUN_1088_140b`). Putting a new tile in a cell (`FUN_1088_1364`) runs the scripts if the new
tile is a lever (`FUN_1088_12f9` → `FUN_1088_a9af(cell)`).

- **Sectors:** entering a sector sets its icon cell to tile 0x308 (a lever in state 1), and
  leaving sets 0x312 (state 0). This happens at the level start (`LevelFirstFrame`) and on
  doors.
- **Sensitive levers:** touching a "Sensitive lever" tile (7030) with the hero's body
  (`HeroTouchTiles`) changes the whole rectangle of that same cell value to `tile + function 2`
  (`FUN_1088_1536`).

Scripts are OBJECTS records 0x44c (run when all conditions hold) and 0x44d (run when they
don't). The layout and command list are in `tools/scripts.py`
(`uv run python tools/scripts.py MAP` decodes them). Ported commands:

- 0 set tile,
- 3 platform,
- 4, 5, 6, 8, 9 platform settings,
- 7 abort,
- 0x0b hurt hero (recorded only),
- 0x13 make the last platform the grinder's,
- 0x1d slow the grinder.

The others are skipped by their size.

## Platforms (FUN_1078_02e5, 03bc, 05a0, 0aef, 0765, 00ae)

Script command 3 does the following:

- **Cut:** it cuts a rectangle of main-plane cells (two cell indices) into a new plane slot
  (≥ 16). Cells with "won't be cut" (7021) stay behind.
- **Path:** it gives the platform a start point and a path of (x, y, speed in 1/16 px per frame)
  points.
- **Reverse:** a platform whose path is this one reversed is reversed instead of being created
  again.

Platforms move only while the hero is in the sector of their last point:

- first x, then y towards the next point, at that point's speed (with flag 4, accelerating),
- position kept in 1/16 px.

At the end of the path the cells are pasted back into the main plane and the platform is freed.
GRINDER's sliding wall works this way, and it is what pushes Leon right at the start.

Hero collision with platforms is in `HeroMoveAndCollide`:

- horizontal: in platform coordinates, using the platform's *previous* y,
- ceiling: per platform,
- floor: `FUN_1088_2db9`, with the platform's motion widening the candidate range and its dx
  carrying the hero.

Ledges on platforms (`FUN_1088_3e0d`, `4116`) and `FUN_1078_0f1a` (tile solidity OR'ed with
every platform under the point) are ported too.

## The grinder (OBJECTS 0x2724, GrinderUpdate 1090:1a91)

The record is `[0x2724, ?, y, top, trigger cell, camera offset]`.

- **Trigger:** the grinder waits (speed −1) until its trigger cell is a lever in a state > 0.
  In GRINDER that is the column of sensitive-lever icons the player jumps through.
- **Climb:** it then climbs, its y kept in 1/512 px. The speed grows by 2 per frame up to
  0x180 (0x100 after script command 0x1d), and it stops at `top`.
- **Platform:** every frame it puts its platform's y at its own y.
- **Crushing:** it crushes the row at `y − 6` in 16 columns from the camera's x + 0x20. Each
  tile with function 15000 is blown up by `FUN_1050_5a33`: it is stepped through its function-2
  "next" tiles and then its function-4 effect plays (72 = rubble, 6 particles × 4 Random
  calls).
- **After a crush:** `FUN_1090_1a70` brakes it by `Random(0x10) + 0x60`, and the crunch sound
  draws `Random(0x50)` for its pitch.
- **Hero:** when it catches the hero at speed > 0xbf it hurts him (`FUN_1088_3c32`), which isn't
  ported yet.

## Not yet

- The other script commands and effects.
- Doors and teleports.
- Hurting the hero (the grinder's catch, script 0x0b).
- The `4e16`/`4e18` dx limits: GRINDER's last frames differ by 3 px at the right edge, without
  any effect on Random.
- Enemies and the enemy factory (CITY01 and the others).
