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

## State (2026-10-16)

CITY01 exact for ~106 windows. In progress: big alien class 0x1432 (`port/src/bigali.rs`
written, NOT yet wired: add `mod bigali;` in main.rs, dispatch `0x1432 => self.bigali` in
actors.rs `update_object`, move hook FUN_1070_72a2 in `after_move`; HeroView needs `hurt`
(4e2a) and `hit_mask` (4660); crate/alien/smallbot tests should set `ps.p.aim_level = 0`
where the original sets 093c=0). Not ported yet: grenade mover 2d37 and effect 21 (1481),
lying shells/splat objects (no RNG), bonuses (6b03), FUN_10a8_1369 (tile 7050), sideways shake
5c7d, frozen/burnt deaths.
