# Recorded demos: format and the determinism oracle

`BIN/USERMAPS/*.DMO/` are folders, one per demo level, each holding the level's `.MAP`, its
`.SCR` script and a `.REC` recording. The script points at the recording with
`PlayRecord=<name>.rec`. Decoder: `tools/demo.py` (`uv run python tools/demo.py <.REC>...`).

| demo | level | frames | ≈ time at 70 Hz |
|---|---|---|---|
| `LEVEL1_1.DMO/START.REC` | START | 23781 | 5:40 |
| `LEVEL1_2.DMO/CITY01.REC` | CITY01 | 29661 | 7:04 |
| `LEVEL2_1.DMO/FOREST04.REC` | FOREST04 | 21571 | 5:08 |
| `LEVEL2_2.DMO/FOREST03.REC` | FOREST03 | 26969 | 6:25 |
| `LEVEL3_1.DMO/FORPLANE.REC` | FORPLANE | 12328 | 2:56 |
| `LEVEL3_2.DMO/HYDRO_02.REC` | HYDRO_02 | 6067 | 1:27 |
| `GRINDER.DMO/BROYEUR.REC` | BROYEUR | 3968 | 0:57 |

That makes 124 345 frames, almost half an hour of recorded play.

## Code

The demo state is a record at 1170:5374 (`gDemo`). Its fields:

- `+0x00` nine `{far ptr runs, u16 index, u16 handle}`, one per key,
- `+0x48` mouse runs, `+0x4c` mouse index,
- `+0x50` checksum buffer,
- `+0x54` u32 frame counter,
- `+0x5a` version.

| function | role |
|---|---|
| `DemoStartRecording` 1098:0002 | allocates the buffers (48000 B per key = 16000 runs, 64000 B of mouse = 16000 entries); **RandSeed := 0** |
| `DemoRecordFrame` 1098:033a | called from `ReadGameInput` after the keys are read; extends the runs; frame counter += 1; every 35th frame stores RandSeed |
| `DemoSave` 1098:00ff | writes `USER<n>.REC` (entries 0..11) when recording stops or a buffer fills |
| `DemoLoad` 1098:0537 | reads entries 0–10 (CHECK is **not** read); **RandSeed := 0**; warns if the version isn't 3 |
| `DemoPlayFrame` 1098:06c2 | replaces all of `ReadGameInput`'s polling: `gInput[0..8]`, `gMouseX/Y`; frame counter −= 1; at 0 it calls `DemoStop` |
| `DemoStop` 1098:0668 | frees everything; `GameFrame` state 0x1e returns to the menu |

## Entries (archive "EAT THIS DEMO", all LZW-compressed)

| # | name | layout |
|---|---|---|
| 0 | `REC HEADER` | u32 frames, u16 0, u16 version = 3 |
| 1 | `MOUSE` | 4-byte entries: `(i16 x, i16 y)` = the mouse position for one frame, or `(0x8001, u16 n)` = the previous position for n more frames. Coordinates are in the 320×200 game screen and can fall outside it. |
| 2–10 | `KEY #0`..`KEY #8` | 3-byte runs `(u16 extra, i8 value)`: `value` for `extra + 1` frames. The order is gInput's: Up, Down, Left, Right, LeftShoot, RightShoot, 6, 7, SwitchWeapon. Values are `KeyPoll` states: −1 up, 0 released this frame, 1 pressed this frame, 2 held. Mouse buttons arrive as 0/1/2 in LeftShoot/RightShoot. Each stream covers `frames + 1` frames: the first frame is the priming run, which `DemoLoad` consumes before the first game frame. |
| 11 | `CHECK` | u32 × (frames/35 + 1): `CHECK[k]` is RandSeed when the recorder's frame counter reached 35·k. `CHECK[0]` is never written (it holds whatever was in memory). |

## RandSeed is the oracle

Borland Pascal's `Random` is a 32-bit LCG, `RandSeed = RandSeed * 0x08088405 + 1` (1168:1570;
the multiplier's low word 0x8405 sits at 1168:15a6), and `Random(n) = (RandSeed * n) >> 32`.
Recording and playback both start from RandSeed = 0.

**All 3550 CHECK samples on the CD are successive states of that LCG starting from 0.** For each
35-frame window this gives the exact number of `Random()` calls the original made
(`Demo.rng_calls()`). For example, BROYEUR makes 0, 30, 35, 35, 35, 35, 35, 296, … calls.

A port is deterministic against a demo when:

1. it starts the level from the same state (the level `.SCR` and `.MAP`, the default player
   state, RandSeed = 0),
2. it feeds `Demo.input_at(frame)` into the hero instead of the keyboard,
3. it uses the same LCG and calls `Random` in the same order, and
4. RandSeed after each 35th frame's input read equals `CHECK[k]`.

The first window whose seed differs points at the bug: compare the call counts to see whether
the port makes too few or too many calls in that window.

The game was built for this. `LevelInit` (1088:1952) calls `Randomize` (1168:15a8, a seed from
the DOS clock) only when no demo is playing or recording, and the game logic has no floating
point at all: no x87 emulator interrupts anywhere, and `__FPMATH` appears only in the runtime's
startup. Matching inputs plus a matching RNG stream should therefore mean matching games.

Caveats:

- The sample is taken inside `ReadGameInput`, so window k covers the end of frame 35(k−1) and
  the start of frame 35k up to the hero's input read. `GameFrameInGame` calls a few systems
  (`FUN_10a0_023f`, `FUN_1078_0aef`) before `HeroUpdate`; a port has to keep that order.
- Anything else that draws on `Random` counts too: particles, idle animations, screen shake,
  sounds picked at random. Cosmetic code isn't exempt, because it advances the same seed.
- The frame-skipper in `VideoPresent` decides whether to *draw*, not whether to *update*. The
  draw-only calls in `GameFrameInGame` (`FUN_1078_0abd`, `FUN_1088_026d`, `FUN_1088_20bc`)
  never reach `Random`, so frame skipping doesn't change the RNG stream.
- `LevelInit` runs after `DemoLoad` has zeroed the seed, so the first window includes the
  level's setup calls (START: 290 calls in window 1).
