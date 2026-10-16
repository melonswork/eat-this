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

## State (2026-10-17)

CITY01 exact for 610 windows (710/847 total), GRINDER 113/113. Debug aids: `ET_RNG=<frame-1>`
(every port Random() with its Rust caller), `ET_RNG_TSV=file` + `tools/rngdiff.py` (first call whose
range/result differs over the whole run; frame shifts of ~1 in object code show up as noise),
`ET_DRONE=1` + `tools/dronediff.py` (drone vs tracer `--watch 1170:4d9e,4da0,4da2,4dac,4dae,
4db0,4db2,4db4,4db6,4db8,4dba` into work/tracer_drone2), `ET_PART=F` (+`ET_ALLPART=1`) particles of
12 frames vs the tracer's `--particles A-B` (particles.tsv: frame, idx, x, y, age, life, mask +34,
state +3e, callback +42, mover +4a), `ET_HERO=F`, `ET_OBJ=1` (gunship), `ET_DEBUG=1` (aliens,
bullet hits), `ET_HIT=1`, `ET_DOOR=1`, `ET_AREA=1`, `ET_CORPSE=1`, `ET_SPARK=1`. Always give the
tracer enough `--windows`: windiff shows "original 0" past the traced windows, and the tracer's
`--entry SEL:OFF --self LEN` dumps a method's object per call (use it to compare objects).

Drone: `port/src/drone.rs` (pick-up effect 6, FUN_1088_03a3/9d34, flares 0a63/09f8/274f, effect
64, a teleport puts it beside the hero: 0c20). The hit tests only record candidate boxes
(`Drone::consider`); `Level::drone_update` replays them with line of sight (FUN_1068_0a54) before
following the hero. 1170:093e (`aim_level2`) is the drone's priority: -1 for wrecks (`hit_test_low`),
the big alien's shield and dead aliens whose +0x46 != -1, 0xf6 for the gunship's body, 0x100
otherwise (the dying big alien keeps 0x100: 7eb2 only sets 093c). Mind such per-site wrappers: the
decompile of each hit test site says exactly what 093c/093e it sets.

Also ported this round: pick-up 1 (health +10, 4e6c is the HEALTH) and 70 (key + sparks);
springboards (kind 0x148c, FUN_1070_d23b) with the hero's carry limits 4e16/4e18; conveyor belts
(ground 5, nudge = tile param 3, walking off throws him vx +-0x17); somersault poses (states 7, 6);
script command 10 (delayed tile/door events, queue in `Level::tile_events`) and platforms running
the lever scripts of the cells they paste; weapon 4 (FUN_1088_6f4d, a bouncing charge) with effects
77 (188c), 78 (0cf8, sparks) and 43 (45db, wall chunks); callback 0x229; objects sliding down slopes
(17d6); ledge pull-up collision flags; hero body hit boxes mirrored only facing left.

Next: window 611. At frame 21323 the original hero enters the use pose (state -30) because
FUN_1060_0819 (a platform the hero rides/controls: sets 1170:5123 = 2 and 5124 = 4 from the
platform's path and his position, lines 12612..13100 of the decompile) says so; the port jumps.
Then: levers needing a key (HeroUpdate, tile param 2 flag 4: keys[tile function - 13980], port has
`Hero::keys` indexed by the pick-up's f3), the arm hit-box flip in `take_hits` (still `facing`,
unverified), effects 23/53/54, bonuses (6b03), script object tiles (0d5a), other weapons (1..3, 5..9),
sideways shake 5c7d, frozen/burnt deaths, barrels of type >= 0xb (186c), level-leaving teleports,
hero standing on the big alien's shield (00b6), wall chunks of tile functions 60308/60304 (45db).
