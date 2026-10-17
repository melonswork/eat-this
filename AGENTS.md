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

## State (2026-10-17, end)

CITY01 (LEVEL1_2.DMO) is exact for ALL 847 windows, GRINDER 113/113. FOREST04 (LEVEL2_1.DMO) is exact for ALL 616 windows (trees, pipes, ladders, death gibs and the dead hero's state 100 included).
LEVEL1_1 (0 windows: the START intro, script command 0x1c launches the hero out
of a ship, 4e70/4e96..4e9a), LEVEL2_2 43, LEVEL3_1 34, LEVEL3_2 0 (level setup draws 402 Randoms) still need their own
`windiff` loops (`/tmp`-free recipe: `mise run trace -- work/cd/BIN/USERMAPS/<DEMO>.DMO --windows N+2`, then
`python3 tools/windiff.py <DEMO> N`, then `ET_RNG=<frame-1>` in the port). A matching RandSeed does not prove the hero is
in the right place: also compare `--dump` with the tracer's `--watch gHeroX,gHeroY,gHeroState`.

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
(FUN_1068_186c splats), objects of VMT 0xc84 (kind 0x14be, icon 61600, FUN_1070_0cce), kinds 0x14d2/0x14f0 icons,
effects 23 (FUN_1050_4a29 part)/53/54, bonus 0xd10 (6b03), levers needing a key, other weapons (1..3, 5..9), sideways
shake 5c7d, frozen/burnt deaths, barrels >= 0xb, level-leaving teleports, hero on the big alien's shield (00b6 is
now `Level::stand_on`, wire it there), wall chunks 60308/60304 (45db), 30070 particle spawn.
Next: LEVEL2_2 (window 46: original 86 calls, port 45) and run the daily loop on it.
