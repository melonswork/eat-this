# Notes for agents working on Eat This

Goal: a faithful Rust+SDL2 port (`port/`) of the 1998 Win16 game (Borland Pascal 7). Fidelity
oracle: the recorded demos' CHECK entries (RandSeed every 35 frames). Human handbook:
`docs/handbook.md`. Domain notes: `re/*.md` (demo, rng, hero, level, tracer).

## Daily loop

```
mise run port -- --headless --demo ../work/cd/BIN/USERMAPS/LEVEL1_2.DMO   # CITY01 (from port/)
mise run trace -- work/cd/BIN/USERMAPS/LEVEL1_2.DMO --windows N             # original's calls
python3 tools/windiff.py LEVEL1_2 <window>                                   # per-frame diff
python3 tools/compare_watch.py work/tracer/LEVEL1_2/watch.tsv work/port_dump.tsv --from F
```
Always also run GRINDER.DMO: it must stay 113/113. Commit after each green step.

## New oracle demos (tools/mkdemo.py, 2026-11-20)

The seven CD demos never exercise many features. `tools/mkdemo.py` makes NEW ones: the original
game (the tracer's emulator) records them with its OWN recorder from a scenario script, so the
.REC carries the original's CHECK entries. Scenarios live in git (`tools/scenarios/*.txt`), the
demos they make in `work/demos/BIN/USERMAPS/<NAME>.DMO` (regenerate them, ~5 s each):

```
mise run mkdemo -- tools/scenarios/WEAPON5.txt                         # -> work/demos/BIN/USERMAPS/WEAPON5.DMO
mise run trace -- work/demos/BIN/USERMAPS/WEAPON5.DMO --overlay work/demos --watch gHeroX,gHeroY,gHeroState
(cd port && ./target/release/eatthis --headless --demo ../work/demos/BIN/USERMAPS/WEAPON5.DMO)
python3 tools/windiff.py WEAPON5 4 ; python3 tools/valdiff.py WEAPON5   # both look in work/demos too
```
Script lines: `level GRINDER` (a CD demo folder) or `level MAPS/BOMS02.MAP` (any map under
work/cd/BIN), `name X`, `mouse X Y` (game-screen pixels, from the next frame on), `N keys...` (hold
these for N frames; none = all up). Keys: up down left right fire alt key6 key7 switch (gKeyMap),
kp8 kp2 kp4 kp6 kp0 (numpad: these report 0 on release; BROYEUR was played with them), lmb rmb.
Script frame i = game frame i+1 = port frame i. `mkdemo.py LEVEL --from-rec X.REC` prints a script
replaying a recording: re-recording BROYEUR, HYDRO_02 and CITY01 that way gives their exact input
streams and EVERY CHECK, so the method is the original's own.
- How: the demo folder's .SCR gets `PlayRecord=<name>.rec`; the file is missing, so LevelLoad calls
  DemoStartRecording (RandSeed := 0) instead of DemoLoad. GetKeyState is answered from the script
  (VKs read from gKeyMap), the mouse buttons are the globals 1a5c/1a5e (0 up, 1 pressed, GameFrame
  turns 1 into 2), the cursor goes through GetCursorPos (FUN_1088_42a3 scales by 319/screen width,
  inverted in mkdemo). After the last frame mkdemo calls DemoSave.
- The game stops recording by itself when the level ends or restarts (GameFrame: 4e60 > 0 ->
  DemoSave): a demo can show the hero dying but not the restart after it.
- Digit/PgUp/PgDn weapon keys aren't replayed by DemoPlayFrame: get weapons from pick-ups
  (`ET_PICKUPS=1 ./target/release/eatthis --headless --map ../work/cd/BIN/MAPS/X.MAP` lists them).
- A frame count that is a multiple of 35 leaves the last CHECK unsampled (garbage); mkdemo pads one.
- The port must run from `port/` (default data path `../work/cd/BIN`).

| scenario | level | what it checks | port |
|---|---|---|---|
| `DEATH1` | GRINDER | BROYEUR's first 300 frames, then waiting: the grinder hurts (FUN_1088_3c32 kind 1, returns 1 → FUN_1090_1a70) and kills the hero (frame 680, state 100, gibs/smoke/splat) | exact |
| `WEAPON5` | BOMS02 | weapon-5 pick-up, four rockets (FUN_1088_6deb, callback 0xd83 = the torpedo's 0xf9f without bubbles, effect 14, rubble effect 75) | exact |

## Tools (non-obvious usage)

- `tools/fn.py show NAME...` prints decompiled functions from `work/decomp/ETLAUNCH.EXE.c`
  (run from repo root). Names come from `re/names.csv`; unnamed: `FUN_1070_7eb2`.
- `mise run disasm -- NAME...` → `work/decomp/NAME.asm`. Use it wherever the decomp calls
  `FUN_1168_0fdd/0fa0/109c/1083` (long div/mul/shl/sar) — Ghidra drops their arguments.
- Tracer: `--watch NAME|SEL:OFF,...`, `--writes SEL:OFF,...` (any selector, e.g. heap
  `26a0:4a4e`), `--entry SEL:OFF --self LEN` (logs a method's object fields per call; use
  SEL:OFF for unnamed functions), `--dump-mem SEL:OFF:LEN` (LEN is hex!). Each run overwrites
  `work/tracer/<demo>/` files — rerun with the columns you need.
- Port: `--trace A B` (per-frame keys/state/random count), `--dump FILE` (hero watch columns),
  env `ET_DEBUG=1` (debug prints in particles/alien).
- Effect ids → procedures: `work/tracer/LEVEL1_2/mem_1170_07b2.bin` (word table, id*2).
  VMTs/DS globals at load: `mem_1170_0900.bin` (offset − 0x900).

## Hard-won facts

- Frame alignment: port `--trace` line N ↔ tracer frame N for hero calls; object calls are
  labelled one frame earlier by the tracer. windiff handles this; equal per-window counts can
  hide per-frame differences that cancel.
- Ghidra param order: `param_1` = last pushed = last Pascal source argument. Nested procs get
  the parent BP as param_1; parent locals are `[BP-n]` — check asm, Ghidra merges locals.
- Particles: 0x50-byte records, see header of `port/src/particles.rs`. Group 0 updated before
  actors, group 1 after. Spawn params are globals 091a..0940 set/restored around each spawn
  (`ps.p`). Delayed spawns go through the 10a0 event queue (`pending`). A death effect may keep
  the particle alive (keep_dying). Hit tests: 568b (objects, first type-0 box), 51aa (hero, all
  boxes of his sprites, slots 0–2 = PPHERO*). Homing data lives in the velocity record.
- Sprite slot → file table in `port/src/sprites.rs`; hitboxes are 16-byte records after a u16
  count in the sprite's extra block; box at record+8.
- 465e "noise" = shake just started (camera vertical strength-1 == count) or hero fired (2);
  enemies hear it.
- 4f0c = 0 and detail 16d9 = 3 during demos (lying smoke never ages; no flash).
- Hero: slopes (gHeroPlatform 4e22 is the slope id, 511b flag), levers (5123/4e5a/4e3c),
  pushable blocks (record 0x4b0 copied from word 2), knock-back state 0x10 → knocked down flat.
  No turning while shooting. Health 0x2c6, invulnerability 0x52 frames.
- Object removal during the walk: `remove_current` (FUN_1070_0002) fixes `walk_index`.
- Argument order: a callee's `[BP+6]` is the LAST pushed argument. When Ghidra's call or a
  nested-proc decompile looks odd, check the callee's asm (e.g. TileFunctionAt: [BP+6]=y,
  [BP+8]=x, so Ghidra shows `TileFunctionAt(y, x)`). Ghidra was right about door markers; I
  "fixed" it wrongly once — verify against the callee before overriding the decompile.
- Some drawing code calls Random and then restores RandSeed (big alien shield, 1070:8c72):
  the tracer counts those calls; windiff's IGNORE set skips such sites.
- Object boxes: actors update only while their box (+2..+8) meets the camera. FUN_1058_0002
  makes it unbounded (falling, hit, ...); moving platforms do it for actors they touch
  (FUN_1058_0114, `wake_actors`). Unknown kinds print `warning: object kind ... not ported`.
- 465e noise: 1 shake started, 2 hero/big-alien fire, 3 a door started moving (FUN_10a8_0975,
  even in LevelFirstFrame). Aliens hearing it go on guard (-500) with a Random(2).
- Weapons: table 1170:1656 (0xd bytes), pick-ups = plane-2 function 30000 (effect 67), right
  button = weapon proc table 188a (only grenades ported). Exits = function 9000 (state -100),
  teleport records 1000; after a teleport LevelFirstFrame runs a frame without the hero
  (`world_update`). Doors: `doors.rs` (segment 10a8).

## Renderer (2026-11-20)

`port/src/render.rs` (sprite bank by slot, 10f0:2670's draw modes 1ae0, tables) and `port/src/draw.rs`
(the frame's draw order and each object kind's draw tail); main.rs `Graphics::draw` runs it after the
update, so it never touches the simulation. Check with `--shot FRAME FILE.ppm` against
`uv run python tools/dumpframe.py DEMO FRAME` (the original's frame; frame numbers line up).
- Draw modes (1ae0) come from the run handlers at 1170:1b8a; mode 0 only paints over colours <= 1ae2
  (ForegroundColors), which is how foreground tiles hide sprites. MIX tables: PalMix (`.MIX`, 256 rows),
  PalMixLight (`.LIT`, 129 rows: light levels), PalMixSea (`.SEA`, 80 rows: the water), and the tint table
  4c26 COMPUTED from the palette (`LevelPalette::tint_table`: row 0 red = hurt hero, 10 = shield).
- Groups loaded by FUN_1088_4518 (PPGLIGHT 0x1b, FACBROCK 0x73, SVN 0x75), SEATILE 0x7c and script groups
  flagged `,N` are NOT fitted to the palette: their pixels index the mix tables (`sprites::raw`).
- Long divisions of 617e in draw code (`lcall` with cx = divisor) are frame/N animations: disassemble
  (capstone: segment for selector S is NE segment (S-0x1000)/8+1) rather than guess.
- FUN_10f0_302b scales with source STEPS (8.8, 0x100 = 1:1, 0x200 = half size); FUN_1040_6315/6720 wrap it.
- Updates that compute their sprites in locals record them in `Obj::looks` (walker, gunship, boss, turret,
  tree); nothing in the simulation reads it.
- Extra maps (`LoadExtraMap=`, slots 8..) keep only PLANE 0 and use the main map's tile groups; 0x272e
  records are parallax decor layers (depth < 0x100 behind the tiles, else over everything).
- Not drawn yet: the city background's extra parts (FUN_1090_0292/04d7, the START bridge), the forest
  background's two parallax layers (proc 4), messages/tips (FUN_1088_2187), the kill/secret counters
  (totals 4eb2/4eae/4eaa unknown in the port), platform objects of unported kinds sit at their start.
  Debug: `ET_DRAWLOG=1` lists every sprite drawn, `ET_DRAW=1` the actor kinds without a draw, `ET_DECOR=1`.

## State (2026-10-17, end)

GRINDER 113/113, CITY01 (LEVEL1_2) 847/847, FOREST04 (LEVEL2_1) 616/616, FOREST03 (LEVEL2_2) 770/770,
FORPLANE (LEVEL3_1) 352/352, START (LEVEL1_1) 679/679 and HYDRO_02 (LEVEL3_2) 173/173 windows, and for these seven `python3 tools/valdiff.py <DEMO>` (after a full
`mise run trace -- <demo>`) reports 0 value mismatches over EVERY Random() call. ALL SEVEN DEMOS are exact (every window, every Random() result via `tools/valdiff.py`, and the hero's x/y/state
frame by frame via the tracer's `--watch gHeroX,gHeroY,gHeroState`): GRINDER, CITY01, FOREST04, FOREST03, FORPLANE,
START (launch, ship platform, steam emitter 0x141e, storm in `storm.rs`, lift dust in the platform objects'
second pass) and HYDRO_02 (the vehicle: `Hero::update_vehicle` = HeroUpdateVehicle, OBJECTS 0x271a starts it,
0x2718/0x2719 give a sector its water line (`Level::water` = 4ec8, waves in `storm.rs::water_frame`), bubbles =
callback 0x30f, the torpedo = `Particles::torpedo` + callback 0xf9f, the sea plants `plant.rs`).

LESSONS (the expensive ones):
- A matching RandSeed per window proves only the call COUNT (the LCG step ignores the range). Two calls swapped
  inside a window, or a setup that draws the right number of values for the wrong objects, leave all windows green
  and wrong. `tools/valdiff.py` compares ranges AND results; run it whenever a demo is "exact".
- `CHECK[0] (after level setup): differs` means the setup Randoms are wrong; the port then ADOPTS the demo's seed
  and everything downstream looks fine until a value (an alien's hit points) matters. Fix the setup first
  (`ET_ICON=1` lists the icons in creation order; the frame-0 calls of calls.tsv are the setup). Icons that
  FUN_1068 constructors consume (the carrier 0x14be eats the aliens/bots to its right) must be cleared in order.
- Tile cells: `cell >> 8` is the group number and goes beyond 15 (LEVEL3_1 has 17 groups); never mask it.
- The tracer labels hero calls with the port's frame and object calls one frame earlier; `ET_RNG_TSV` + valdiff
  sidestep labels. LevelFirstFrame also runs a world update without the hero (Game::new, like `teleport`).
- HeroTouchTiles (sensitive levers) is EARLY in HeroUpdate: its scripts/spawns draw Randoms before the hero's idle
  Random(8) at the end (main.rs runs it before `hero.update`). Other level events recorded by `Hero::update`
  (levers, pushes, pick-ups) are still processed after it: if a Random order swap shows up around those, split
  the update there too.
- Hero walk limits (4e16/4e18, springboards, bots) apply AFTER the platform objects, not at the top; spring parity
  is `(frame & 1)`; weapons 2/8 also add to the shot counter (4e6a, it feeds the flame's wobble) and set the
  4e56 pause of 10; weapon 1 = flame thrower (FUN_1088_678b, callback 0x4ff, death effect 37 = smoke when 617e is even).
- Under a saved RandSeed (drawing code, the plants' burst) use `PascalRandom::scoped(seed, ..)`: it restores the seed AND the
  call counter the CHECK windows compare. Draw-time Randoms the port doesn't make are skipped by the IGNORE sites in
  windiff.py/valdiff.py (1070:8c72, 34db, 091f, 1068:3112).
- Platform draw routines live in a table at 1170:09ce (filled at run time, not in the EXE's data): rows 0 and 3 (draw
  1060:1cee) shed dust from lifts.
- Ghidra's `FUN_x(a,b,c,d)` call text lists the parameters in [BP+6].. order = REVERSED Pascal order.

Added after the CITY01 pass (all from the FOREST04 loop): ambient sound emitters (`ambient.rs`: audibility of
FUN_10e0_016b needs the listener = camera centre; the first deadlines 4ec0/4ebc are the boot-seed values 509/329, the
hero's idle values -20/590 too); sprite groups by slot from the script's `LoadSpriteGroup=` lines (slots 0x32..) plus
the fixed slots in sprites.rs; walking gunner 0x14d2 (`turret.rs`); platform objects 100..103 / lifts 0x14c8
(`platobj.rs`, run from inside HeroUpdate before the hero moves, like FUN_1060_145a); script commands fill_area (0x01)
and copy_area (0x0e); levers with key slots (tile param 2 flag 4: `Hero::keys`, `script_cells`, `cell_steps`);
weapons 2 (rapid gun, effect 81), 7 (napalm shell, effects 44/18, fire crawlers FUN_1050_14bd), 8 (three homing
rockets, callback 0x7bf, effect 57); the switch-weapon key (input 8); bonus drops (`Level::bonus` = FUN_1040_6b03,
callback 0x2588, effect 66: they fly, the hero can catch them, else they land as plane-2 tiles); pipes (states -1
hanging from 60800, -2 climbing out of 60600); red gibs FUN_1068_186c for barrels.

Ported since the 610 mark: ladders (HeroUpdate state -5: detection loop before the exits, grab before the
Down-crouch, the -5 block before the levers); the hero's hit boxes are ALWAYS mirrored (the rec word is gHeroFacing
= +-1 and FUN_10f0_2240 only tests != 0; mirrored rects swap their edges); platform camera lock (script command 0x14 =
platform w20: 4e36 = (w20 + 0x7d) - platform.y + camY each frame while it is in the hero's sector); script commands
that spawn enemies (command 0 with an object tile, command 0x10 = 0d5a at the row above the camera;
`Effects.spawns` + `Level::flush_spawns`); decals (`decals.rs`: blood drops 0xbb0, shell casings 0xbc4, wall splats
0x976 in sector list L5A, updated by FUN_1070_fb30 BEFORE particle group 0; eviction at > 0x1f; FUN_1058_016f takes
byte 0 of the MAIN-plane cell looked up first); the walker with a gunner (0x140a, `walker.rs`); the hovering machine
(0x13f6) and its wreck (VMT 0xcdc, move hook FUN_1070_36e7) in `boss.rs`; effects 7, 9, 28, 29; crouch-slide down
slopes (4e20 = `down_timer`, 4e24 = `sliding`, 4f03 = `released[DOWN]`, FUN_1088_29c8 = `slope_run`) and the
knocked-down hero sliding too; `Env.hero_obj` lets objects hurt the hero on the spot (`Hero::hurt`),
`Effects.hero_stand` = FUN_1070_00b6 (hero carried on a moving top, gHeroGround 0x16).
Facts: gHeroDX set by objects after HeroUpdate is lost (4e06 resets it next frame); the 1168:1381 set test at
DS:160a is "-state in 1..5" (hero hanging/climbing ignores platforms); Pascal calls push left to right, so Ghidra's
param_1 is the LAST source argument (3de2(sprite, steps, frames, y, x, animate)); a long divisor with CX=0, BX=8 is
0x80000 (BX is the high word). Particle spawn globals that a caller forgets to reset stay set (FUN_1058_0301 leaves
091c = its last Random(4)).
Debug aids added: `ET_CAM`, `ET_AIM`, `ET_CMD`, `ET_FACT`, `ET_SPLAT`, `ET_BOSS`. Tracer `--entry SEL:OFF` works for
plain functions (`--entry 1050:5c4a` finds who shakes the screen) but hooking EnemyFactory aborts the run;
`--particles` tsv frame N ~ port ET_PART frame N-1. windiff IGNOREs the draw-time Randoms under a saved seed
(1070:8c72, 1070:34db). Earlier aids: `ET_RNG=<frame-1>`, `ET_RNG_TSV` + `tools/rngdiff.py`, `ET_DRONE` +
`tools/dronediff.py`, `ET_PART=F` (+`ET_ALLPART=1`), `ET_HERO=F`, `ET_OBJ=1`, `ET_DEBUG=1`, `ET_HIT=1`, `ET_DOOR=1`,
`ET_AREA=1`, `ET_CORPSE=1`, `ET_SPARK=1`.

Still unported (warnings in the port run): the hero riding a smallbot (FUN_1070_a18e), the hero's death
outside the grinder (kinds 2/3 of FUN_1088_3c32 untested), the restart, objects of VMT 0xc84 (kind 0x14be, icon 61600, FUN_1070_0cce), kinds 0x14d2/0x14f0 icons,
effects 23 (FUN_1050_4a29 part)/53/54, bonus 0xd10 (6b03), levers needing a key, weapons 3, 6, 9 (pick-ups: BOMS02 3 at (1303,263) near the start (1255,366), CITYRAIN 6 at
(167,103) (start (79,799)), CITY02 9 at (1223,1719); make a scenario for each), sideways
shake 5c7d, frozen/burnt deaths, barrels >= 0xb, level-leaving teleports, hero on the big alien's shield (00b6 is
now `Level::stand_on`, wire it there), wall chunks 60308/60304 (45db), 30070 particle spawn.
Next: nothing is left in the recorded demos; write scenarios (tools/scenarios) for the unported features. The game is not complete: see the unported items below (grep the
port run for `warning:`), and the SDL front end/menus/sound. and run the daily loop on it.
