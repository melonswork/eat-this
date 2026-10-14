# Leon: hero movement (`HeroUpdate`, 1088:7592)

Reconstructed from the decompiled `ETLAUNCH.EXE`. All names come from `re/names.csv`; run
`mise run names` and read `work/decomp/ETLAUNCH.EXE.c`, or `python tools/fn.py show HeroUpdate`.
Units are pixels and frames unless noted. Screen y grows downwards. `(x, y)` is the centre of
the feet.

## Where it sits in a frame

```
MainWindowIdle (1008:05cc)
  GameFrame (1080:309a)
    GameFrameInGame (1088:b5ff)      when gInGame == 1
      ... camera, objects
      HeroUpdate (1088:7592)
        ReadGameInput (1088:53a5)    -> gInput[0..8], or the demo stream
        state machine                -> gHeroDX, gHeroDY, gHeroState, gHeroFrame
        HeroMoveAndCollide (1088:3322)
        HeroPushOutOfWalls (1088:2a78)
        SpriteQueue (10f0:2151)      Leon's sprite (+ an overlay sprite)
      ... enemies, bullets, drawing when gDrawThisFrame
  VideoPresent (1008:0363)
```

There is no fixed timestep: one call is one frame. `VideoPresent` throttles the frame rate and
sets `gDrawThisFrame` (frame skipping). The editor docs say delays are in 1/70 s, so the design
rate is 70 Hz (VGA refresh).

## Input

`ReadGameInput` polls 9 virtual keys from `gKeyMap` (1170:1640, from the INI): Up, Down, Left,
Right, LeftShoot, RightShoot (twice) and SwitchWeapon. The defaults are the arrows, Insert, Space
and Tab, with the numpad as a fallback. Each `gInput[i]` is a `KeyPoll` counter: `n > 0` means
held for n frames, so `== 1` means "just pressed". `gInputReleased[i]` is set while a key is up
and cleared when its action fires, so holding Up doesn't keep re-jumping. Left and Right held
together cancel out. The mouse (`gMouseX/Y`) aims the gun: `HeroAimAtMouse` stores
`atan2(mouse - muzzle)` in `gHeroAimAngle`. Keys 1–0 and PgUp/PgDn pick a weapon through
`gWeaponForDigitKey`.

During demo playback (`gDemoPlaying`) the whole block is replaced by `FUN_1098_06c2`, which reads
recorded input. That's why the `.DMO` files can serve as a determinism test for a port.

## State machine (`gHeroState`)

| value | meaning | entered by | notes |
|---|---|---|---|
| 0 | on the ground: stand, walk, turn, idle | `HeroStand` | walk accel 7 (3–4 on some platforms) via `HeroAccelerate` |
| > 0 | airborne; the value is the kind of jump | `HeroStartJump(flag, vy, kind)` | 5 = normal jump (wall jump possible), 7 = wall jump or a jump from a springy platform (somersault frames), 6 = reverse somersault, 1 and 0x10 = ? (poses 0x110 and 0x201–0x204) |
| −1 | hanging from a pipe (tile function 60800) | grabbing with Up while falling | Up/Left/Right jumps off with vy −50 |
| −2 | climbing up onto a pipe (tile function 60600) | rising into one with Up held | 54-frame animation, then crouch |
| −3 | ? (frames 0x23e+) | | starts a vy −20 hop |
| −4 | hanging from a ledge at (`gHeroHangX`, `gHeroHangY`), then pulling up (guess from the code: ledge check, then a 28-frame climb) | | |
| −5 | on a ladder or rope (tile function 10) | `HeroGrabLadder` | Up/Down move 2 px per frame; snaps x to the tile centre |
| −30 | using something (lever, door), facing away | | |
| −40 | crouching; `gHeroCrouch` 0..8 is the progress | `HeroCrouch` | height 42 → 24; also the landing pose |
| −80 | turned to face the camera | | frames 0x24a–0x24e |
| −100 | going through a door or teleport | Up on a door tile | `gHeroAnimTimer` runs the animation, then the level reacts |
| −200 | knocked down | heavy hit, or a fall of 0x120+ | lies for `gHeroAnimTimer` frames |
| 100 | dead | | Space or Enter restarts (`GameFrame` state 10) |

`gCheatFly > 0` (the ETFLY cheat) skips all of the above: each key moves Leon 8 px through
walls.

## Physics

- **Gravity** (state > 0): `vy += 10` each frame, or `+= 3` while Up is held. That is the
  variable jump height. `gHeroVY` is in 1/10 px/frame and capped at 50, so the terminal speed is
  5 px/frame. The fall moves `vy / 10` px each frame.
- **Jump**: `HeroStartJump(0, -50, 5)`. After a crouch the impulse grows with the crouch time:
  `-(gHeroCrouchMax / 2) - 50`. On ground kind 0x17 it is scaled by 8/7. Up needs to have been
  released since the last jump.
- **Wall jump**: in state 5, with Up held and vy > −33, pressing away from a wall that
  `FUN_1088_753e` detects within `halfwidth + 5` px gives a new −50 jump (state 7).
- **Air control**: ±2 px/frame from Left/Right.
- **Walking**: `HeroAccelerate(limit, step)` does `vx += step` and clamps it to `±limit`. With
  `vx = gHeroVX` the frame moves `|vx| / 4 + 1` px in the direction of vx. Friction: when
  `gHeroVXFriction` is set, `|vx| > 4` loses 4 per frame and anything smaller stops.
- **Landing** (`gHeroFallDistance` accumulates `vy / 10`):
  - under 7 px: crouch progress `4 - d/2`, so a tiny dip,
  - otherwise progress `d/32 + 1` (`d/24 + 1` after a flip), capped at 8,
  - 0x20 px or more: dust (`SpawnDust`),
  - more than 0x60: screen shake `(8, d/16 - 4)`,
  - 0x40 or more on ladder-top ground: a heavier landing sound and a tile effect,
  - 0x120 px (18 tiles) or more: knocked down for `(d - 0x120)/2 + 100` frames, capped at 170.
    That is the fatal fall.
- **Hitbox**: half-width `gHeroHalfWidth` (6, 8–9 in some poses), height `gHeroHeight` (42
  standing, 24 crouched, 33/31 in the air, 26 lying).

## Collision

Tile queries take pixel coordinates `(y, x)` and read the `TILE INFOS` record (16 bytes per tile)
of the map cell in `gTilePlane`:

- `TileFunctionAt` → offset 6, function 0 (what the tile *does*: 10 ladder, 7030 sensitive lever,
  7140 crumbling, 10000 end of level, 14000+ keyholes, 60600 pipe, 60800 hang pipe, …; see
  `BIN/TILEMAKE.KIT/TM/TILES.TXT`),
- `TileSolidityAt` → offset 8, function 1 (bit 1 floor, bit 2 wall, 0x20 slope,
  0x200 floor that bullets pass through, 0x2000 floor for the hero only; `0xa75f` outside the
  map),
- `TileParamOfLastCell(n, …)` → function n of the cell that was just looked up.

`HeroMoveAndCollide` runs once per frame on the frame's `(dx, dy)`:

1. Horizontal: test the column at `x + dx ± halfwidth` every 16 px up the body height for
   solidity bit 2. On a hit, clamp x to the tile edge minus `halfwidth + 1`. Moving platforms
   (`gPlatforms`, `gPlatformCount`) are tested the same way in their own coordinates.
2. Vertical, going up: test the row at the head for bit 2 and clamp below the ceiling.
3. Vertical, going down: `HeroFindFloor` scans under both feet. Floors come from the main plane
   and plane 2. Slopes (bit 0x20) give a height inside the tile (`FUN_1088_2f6b`). The result is
   either the new y or 0x7fff (no floor). Standing on a platform carries Leon along with it
   (`gHeroPlatform`, a table of 0x16-byte records at 1170:1292).

`HeroPushOutOfWalls` then moves x by 1 px if Leon still overlaps a wall.

## Animation frames (`gHeroFrame`)

Index = `entry + 256 * file`, over `PPHERO.SPR`, `PPHERO2.SPR`, `PPHERO3.SPR`. The `PPHEROA*`
files are an alternative set with the same layout. Checked by rendering
(`tools/frames.py`):

| frames | animation |
|---|---|
| 0x5b–0x6e | turning around (`gHeroTurn`) |
| 0x81–0x94 | walk cycle, 20 frames, `gHeroAnimTimer / 2 + 0x81` |
| 0xa1–0xa8 | crouch, `0xa0 + gHeroCrouch` |
| 0xac–0xbf | crouch-walk |
| 0x105–0x10e | jump/fall, `(vy - vy0 - 0x31) / 5 + 0x10e` |
| 0x110 | airborne state 1 |
| 0x12c–0x13b | flips (states 6/7) |
| 0x1e3–0x1e8 | shooting while standing |
| 0x201–0x204 | airborne state 0x10 |
| 0x208–0x20b | lying knocked down |
| 0x227–0x232 | shooting while walking |
| 0x24a–0x24e | facing the camera |
| 0x254+ | climbing onto a pipe |
| 0x26d–0x271 | idle look-around (random every 210–630 frames) |
| 0x300+ | going through a door |

## Open

- States −3, −4 (ledge pull-up) and −30 in detail; `gHeroGround` kinds; `FUN_1088_2f6b` slope
  heights.
- Everything that hangs off the weapon (`HeroFireWeapon`, `gHeroMuzzleX/Y`, `(*DAT_1170_4dee)()`
  is the current weapon's update).
- `HeroUpdateVehicle` (`FUN_1088_5b62`, about 300 lines), used when `DAT_1170_5127 == 2`.
