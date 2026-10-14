# Who calls Random, and when

Working notes for matching the demos' CHECK windows (see [demo.md](demo.md)). 168 functions call
`Random` directly; `python tools/rngreach.py NAME [depth]` prints the part of a call tree that
can reach it. Static reach is incomplete: enemies and objects are Pascal objects with virtual
methods, and their calls go through VMTs and function pointers (`(*DAT_1170_4c32)()`,
`(*DAT_1170_4c36)()`, `(*DAT_1170_4dee)()`).

## Order around a level start (GameFrame state 10)

1. `HeroReset` (1088:0442): `Random(8)`, `Random(0x1a4)`, `Random(0x230)`, `Random(0x230)` for the
   idle timers and two other countdowns. This runs **before** `DemoLoad` zeroes RandSeed, so
   these four values depend on whatever happened before the demo and can't be reproduced.
   Luckily they only time the idle animation and two countdowns.
2. `FUN_1088_d0d6` → `LevelLoad` (1088:cf5c): loads the map (`FUN_10f0_5dc1`), places the hero
   at MAP HEADER (+6, +8), then `DemoLoad` (RandSeed := 0), then `LevelInit`.
3. `LevelInit` (1088:1952): `Randomize` unless a demo is involved, then spawns from
   - the icon tiles in planes 2 and 1 (`FUN_1088_1797`): function 0 ≥ 61000 are spawners,
     the rest goes through `FUN_1088_0d5a`,
   - the map's OBJECTS records (`FUN_10f0_5d1e` iterates them; `Map.object_records()` in
     tools/formats.py decodes them): types 0x13xx–0x14xx go to the enemy factory
     `FUN_1070_d6be`, whose constructors draw Random (e.g. 0x1428: 5 calls); 1000, 0x44c/0x44d
     (lever scripts), 0x4b0, 0x2715–0x2724, 0x272e are handled inline.
4. `FUN_1088_b4db`: a first update and draw without the hero.

Window 1 starts counting at step 2. GRINDER (no enemies) makes 0 calls in it; CITY01 (47 enemy
records) makes 79.

## HeroUpdate

Ported in `port/src/hero.rs`, in the original's order:

- `Random(8)` once per frame whenever the state isn't 0 (air, crouch, knocked down, ...), and
  once per frame while walking or pushing a wall,
- none while standing or during the 9-frame turn-around,
- `Random(3)` + `Random(0x1a4)` when the idle timer reaches its threshold,
- on landing after a fall over 0x1f px: `Random(min(fall/16+1, 8))`, plus 5 per particle in
  `SpawnDust`, but only on "Send leaves" tiles (function 13100).

## The floor (HeroFindFloor 1088:304c, from its disassembly)

- Only one row is tested: `y_new + 1` (or `+ |dx|` on platforms), in the two columns `x ± half`.
- Solidity is plane 1 **OR plane 2**; a floor is `& 0x2201`, or a ladder top (function 10).
- A candidate `(row & ~15) - 1` counts if it is within `[y_old - 1 - e, y_new + e]`, with
  e = 2 when standing (ground kind ≤ 9 and state ≤ 0), else 0 (range test `FUN_1118_02ac`).
- Ground kind: 9 floor, 5 conveyor (function 11000), 0x14 ladder top while falling fast.
- Moving platforms (`gPlatforms`, extra maps such as GRINDER's `LoadExtraMap=broyback.map`) are
  scanned the same way in their own coordinates (`FUN_1088_2db9`).
- Slopes (solidity 0x20) only when flag 2 is set (`FUN_1088_2f6b`, tables at 1170:1280/1290).

## GRINDER, window by window

| window | original | port | why |
|---|---|---|---|
| 1 | 0 | 0 | the hero stands; no enemies |
| 2 | 30 | 29 | walking from frame 41 is 29 calls; the 30th comes from somewhere else (unknown) |
| 3–7 | 35 each | 35, 36, 35, 35, 35 | 1/frame (walk, jump, fall); the port's extra one in 4 is a landing at the map bottom |
| 8 | 296 | 25 | the grinder |

The grinder is object 0x2724 (`FUN_1090_1a91`, called through a function pointer): a moving
extra map whose top `DAT_1170_4ee0` starts at the record's y (1997) and, once triggered
(`FUN_1088_140b(record[4])`), climbs with speed `DAT_1170_4eea` up to `DAT_1170_4eec` = 0x180.
It crushes the ledge rows whose tiles have function 15000 (`FUN_1050_5a33` explosions,
`Random(0x50)` for the sound pitch, screen shake) and kills the hero when it reaches him at full
speed. It is also the platform the hero lands on, which the port doesn't have yet, so the port's
hero falls to the map bottom instead.

## Next

Matching more than the first windows needs the systems that draw Random: the enemy factory and
enemy AI (CITY01 window 1: 79, window 2: 12), the grinder and extra maps (GRINDER windows 4, 8),
particles (`FUN_1050_5a33`, `FUN_1050_3f77`, `FUN_1050_47e7`). Working them out statically is
slow, because the calls hide behind VMTs. A tracer that runs the original code (an x86 emulator
plus Win16 stubs) and logs every `Random` call with its call site and frame would turn each
window into an exact list.
