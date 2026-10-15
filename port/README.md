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
| `data.rs` | `.TIL`, `.SPR`, `.MAP` (start position from the map header, as `LevelLoad` does), the few `.SCR` keys used |
| `palette.rs` | the level palette and the sprite remap (`FUN_10f0_1970`) |
| `camera.rs` | the scrolling camera (dead zone, 15 px/frame, stops at the map edge) |
| `bg.rs` | what is behind the planes: BackgroundProc 0 (city) and 4 (forest, first layer) |
| `picture.rs` | IFF `.LBM` pictures (PBM and ILBM, ByteRun1) |
| `world.rs` | `TileSolidityAt` / `TileFunctionAt` on the main plane, `0xa75f` outside the map |
| `rng.rs` | Borland Pascal `Random` (the LCG the demos' CHECK entries sample) |
| `demo.rs` | `.REC` playback: key runs, mouse runs, CHECK |
| `hero.rs` | Leon per [re/hero.md](../re/hero.md): stand, walk (accelerate to 7, `|vx|/4+1` px), jump (−50, +3/+10 gravity, cap 50), fall, crouch, the landing table, knocked down after 0x120 px; walls, ceilings and floors |
| `main.rs` | 70 Hz loop, `KeyPoll` counters, 320×200 frame scaled by SDL, demo player, headless checker, PPM screenshots |

## Graphics

The frame is one 320×200 indexed buffer with one palette, like the original's. The level palette is
the CMAP of the picture named by the script's `Background=` (8-bit in the file, the game keeps
the top 6 bits); `ForegroundColors=N` splits it into background colours 1..=N and foreground colours
N+1..=255 (208 without the key). **Tiles are authored in that palette and are drawn raw** (the
`PALETTE` inside a `.TIL` is just the editor's copy; where it differs, e.g. SVEN3's magenta marker
colours, the level's wins). **Sprite groups are remapped at load**: each colour marked in `USED CLRS`
becomes the nearest level colour, colours up to the header's boundary word (SPR-HEADER word 5: 255
for most groups, 175 or 0 for some) from the background range, the others from the foreground range.
The metric is `(dr²+dg²)² + db⁴` on 6-bit values, first minimum wins (`FUN_10f0_6039`).
Verified against the original: `tools/dumpframe.py` runs it under the emulator and dumps its frame
buffer and palette; on CITY01 (demo frame 300, same camera) 89 % of the port's non-black pixels equal
the original's index for index, the rest being the missing background, objects, lights and a few
tiles the original doesn't draw (the `60308` "dumpster" tiles).
Not verified: the remap against a frame with a sprite group other than Leon's, and PalMix/light
tables (the `.MIX`/`.LIT` shading) which are not ported. `--camera X Y` fixes the camera for
comparisons.

### Background

`Background=` is only the palette and the source of the picture; what is drawn behind the planes is
chosen by `BackgroundProc=N`, one of the routines in segment 1090 (`bg.rs`). Proc 0, CITY01's
(`FUN_1090_0797`): the sky is a 320×120 sprite (rows 0..119 of the picture) drifting left one pixel
per 8 frames; two rows at the horizon scroll at camera/16; the water rows below scroll faster with
every row (`acc/248`, fixed point); the skyline strip rides at the horizon's speed, its reflection
(sprite 2) is drawn through the PalMix table. Against the original's background (tools/dumpframe.py
saves it before the tiles are drawn) the port is identical in 99.84 % of the pixels on four frames
of the CITY01 demo; the rest is a small animated sprite (a screen on a building) that isn't drawn.
Proc 4 (forest) draws its first layer, a 320×200 sprite tiled at camera/16, but not the two parallax
tile layers over it (65 % identical). The camera follows the original's (`camera.rs`); against the
original's camera over 3000 CITY01 frames it differs on 7 %, all while Leon idles (the hero code
nudges the screen offset).

## Determinism check

`--headless --demo` replays a demo as fast as it can. `CHECK[k]` is `RandSeed` at the input read
of frame 35k + 1 (1-based; the recorder tests the level frame counter, which is bumped at the end
of the frame), and `CHECK[0]` is the seed after level setup. The port compares at the same point,
before the hero update. Window k is the calls from sample k − 1 to sample k; window 1 starts at
`DemoLoad` (seed 0), so it includes the level setup, as in the tracer. On a mismatch it reports how many `Random()` calls the original made in that window
against the port's count, then resyncs the seed so each later window is judged on its own.

`--trace FROM TO` prints, for the frames FROM..TO, the inputs, the hero's state and position and
how many Random() calls each frame made.

Current state ([re/rng.md](../re/rng.md)): in the ported states the hero draws Random where the decompiled code
does (checked only against GRINDER). GRINDER, which has no enemies, matches windows 1–3 and 5–7 (58 of 113 overall);
window 4 lands at the map bottom because the grinder platform is missing, and window 8 is the
grinder itself. The other demos spend 60–400 calls in window 1 on level setup (the enemy
factory and objects), which isn't ported.

## Not yet

- Ladders, pipes, ledges, slopes and stairs, moving platforms. In the CITY01 demo Leon gets
  stuck at the first staircase.
- Background routines other than 0 and 4 (a guess is drawn instead), the forest's parallax tile layers, the TV on CITY01's skyline, lights, map objects, enemies, weapons, sound, CD music,
  the level script (`.SCR`), menus.
