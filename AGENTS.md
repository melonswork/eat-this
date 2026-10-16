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

## State (2026-10-17, later)

CITY01 exact for 721 windows (800/847 total), GRINDER 113/113. Done since the 610 mark:
ladders (HeroUpdate state -5: detection loop before the exits, grab before the Down-crouch, the -5 block before
the levers; collision already knew -5), the hero's hit boxes are ALWAYS mirrored (the rec word is gHeroFacing =
+-1 and FUN_10f0_2240 only tests != 0; mirrored rects swap their edges), platform camera lock (script command
0x14 = platform w20, 4e36 = (w20 + 0x7d) - platform.y + camY every frame while it is in the hero's sector),
script commands that spawn enemies (command 0 with an object tile and command 0x10 = 0d5a at the row above the
camera; `Effects.spawns` + `Level::flush_spawns`), decals (`decals.rs`: blood drops 0xbb0, shell casings 0xbc4,
wall splats 0x976 in sector list L5A, updated by FUN_1070_fb30 BEFORE particle group 0; eviction FUN_1068_04d0 at
> 0x1f; FUN_1058_016f takes the byte0 of the MAIN-plane cell looked up first, not plane 0's).
Debug aids added: `ET_CAM` (camera per frame, compare with tracer `--watch gCameraX,gCameraY`), `ET_AIM`,
`ET_CMD` (script commands / SETCELL / SCRIPT), `ET_FACT` (unported factory kinds), `ET_SPLAT`.
Tracer: `--entry` works with SEL:OFF (`--entry 1068:182c`) but hooking EnemyFactory aborts the run; `--particles`
tsv frame N ~ port ET_PART frame N-1 (the tracer samples before the update).
Earlier aids (still valid): `ET_RNG=<frame-1>`, `ET_RNG_TSV=file` + `tools/rngdiff.py`, `ET_DRONE=1` +
`tools/dronediff.py`, `ET_PART=F` (+`ET_ALLPART=1`), `ET_HERO=F`, `ET_OBJ=1`, `ET_DEBUG=1`, `ET_HIT=1`,
`ET_DOOR=1`, `ET_AREA=1`, `ET_CORPSE=1`, `ET_SPARK=1`.

Next: window 722. Frame 25257: object kind 0x140a (walker with a turret rider; update FUN_1070_4051, constructor in
`factory.rs`, port prints `object kind 0x140a not ported`; helpers 1765, 3d4c, 3d85, 3f4d, 0dd8, 00b6, 3a81).
Also unported and seen in CITY01: kind 0x13f6 (stationary boss-like object, update FUN_1070_30b6, 790 updates),
the hero riding a smallbot (FUN_1070_a18e), the hero's death (FUN_1068_186c splats), objects of VMT 0xc84
(kind 0x14be, icon 61600, FUN_1070_0cce), kinds 0x14d2/0x14f0 icons, effects 23/53/54, bonus 0xd10 (6b03),
levers needing a key, other weapons (1..3, 5..9), sideways shake 5c7d, frozen/burnt deaths, barrels >= 0xb,
level-leaving teleports, hero on the big alien's shield (00b6), wall chunks 60308/60304 (45db).
