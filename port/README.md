# Eat This: source port (skeleton)

A Rust + SDL2 reimplementation built from the decompiled `ETLAUNCH.EXE`. It ships no game data:
point it at the `BIN` directory of your own CD (`mise run extract` puts it in `work/cd/BIN`, the
default).

```bash
mise run port                                                    # CITY01 with the keyboard
mise run port -- --map ../work/cd/BIN/MAPS/FOREST02.MAP
mise run port -- --demo ../work/cd/BIN/USERMAPS/LEVEL1_2.DMO     # watch a recorded demo
mise run check-demo -- ../work/cd/BIN/USERMAPS/GRINDER.DMO       # headless RandSeed check
mise run port -- --demo ../work/cd/BIN/USERMAPS/LEVEL1_2.DMO --shot 2000 ../work/out/f.ppm
```

Controls follow the game's default key map: arrows, Insert, Space, Tab; Esc quits. The game runs
at 70 frames per second, the VGA refresh rate it was designed for.

## What's there

| module | from |
|---|---|
| `archive.rs` | Image Line archives + LZW (1108:00d6) |
| `data.rs` | palettes, `.TIL`, `.SPR`, `.MAP` (start position from the map header, as `LevelLoad` does) |
| `world.rs` | `TileSolidityAt` / `TileFunctionAt` on the main plane, `0xa75f` outside the map |
| `rng.rs` | Borland Pascal `Random` (the LCG the demos' CHECK entries sample) |
| `demo.rs` | `.REC` playback: key runs, mouse runs, CHECK |
| `hero.rs` | Leon per [re/hero.md](../re/hero.md): stand, walk (accelerate to 7, `|vx|/4+1` px), jump (−50, +3/+10 gravity, cap 50), fall, crouch, the landing table, knocked down after 0x120 px; walls, ceilings and floors |
| `main.rs` | 70 Hz loop, `KeyPoll` counters, 320×200 frame scaled by SDL, demo player, headless checker, PPM screenshots |

## Determinism check

`--headless --demo` replays a demo as fast as it can. Every 35th frame, right after the frame's
input is read (where the original recorder samples it), it compares `RandSeed` with the demo's
CHECK entry. On a mismatch it reports how many `Random()` calls the original made in that window
against the port's count, then resyncs the seed so each later window is judged on its own.

`--trace FROM TO` prints, for the frames FROM..TO, the inputs, the hero's state and position and
how many Random() calls each frame made.

Current state ([re/rng.md](../re/rng.md)): the hero draws Random exactly where the original
does. GRINDER, which has no enemies, matches window 1 and windows 3, 5, 6 and 7, but window 2 is
one call short (source unknown) and window 4 lands at the map bottom because the grinder
platform is missing. The other demos spend 60–400 calls in window 1 on level setup (the enemy
factory and objects), which isn't ported.

## Not yet

- Ladders, pipes, ledges, slopes and stairs, moving platforms. In the CITY01 demo Leon gets
  stuck at the first staircase.
- The level palette. Every `.TIL`/`.SPR` has its own palette and the original remaps them into
  one per level; for now each layer is drawn in its own colours.
- Background picture (`.LBM`), animated tiles, map objects, enemies, weapons, sound, CD music,
  the level script (`.SCR`), menus.
