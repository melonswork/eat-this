# Tracer: the original game under an emulator

`tools/tracer.py` runs the original `ETLAUNCH.EXE` (16-bit Win16, the real code, not a port) on
a recorded demo and logs every Borland Pascal `Random` call with its call site, game frame and
CHECK window. It checks RandSeed against the demo's `CHECK` entries as it goes.

```
mise run trace -- work/cd/BIN/USERMAPS/GRINDER.DMO [--windows N] [--keep-going] [--out DIR]
```

| demo | frames | CHECK windows matched | time |
|---|---|---|---|
| `LEVEL1_1.DMO` (START) | 23781 | 679 / 679 | 127 s |
| `LEVEL1_2.DMO` (CITY01) | 29661 | 847 / 847 | 72 s |
| `LEVEL2_1.DMO` (FOREST04) | 21571 | 616 / 616 | 92 s |
| `LEVEL2_2.DMO` (FOREST03) | 26969 | 770 / 770 | 68 s |
| `LEVEL3_1.DMO` (FORPLANE) | 12328 | 352 / 352 | 51 s |
| `LEVEL3_2.DMO` (HYDRO_02) | 6067 | 173 / 173 | 36 s |
| `GRINDER.DMO` (BROYEUR) | 3968 | 113 / 113 | 10 s |

All 3550 CHECK samples on the CD are reproduced, so the emulated game is exact for every
recorded frame, and each window's call list is the original's.

## Output

`work/tracer/<DEMO>/` (override with `--out`):

- `calls.tsv`: one line per Random call: `window`, `frame` (1-based `GameFrameInGame` count
  since `DemoLoad`; 0 = level setup), `caller` (function from `re/names.csv` or the
  decompilation, + offset of the `call far`), `site` (return address), `kind` (`Random(n)`,
  `RandomL`/`RandomR` for the other variants, `RandomNext` for direct calls), `n`, the `result`
  and the new `seed`.
- `callers.txt`: calls per function over the checked windows (CITY01: `FUN_1070_7eb2` 120 434,
  `HeroUpdate` 15 061, `FUN_1070_30b6` 4 993, `FUN_1040_188c` 2 988, …).
- `windows.txt`: per window, the count, the count CHECK implies and the verdict, then the calls
  grouped by frame and caller.

The console shows one line per window, and the run stops at the first mismatch unless
`--keep-going`.

Window k ends at the sample of `CHECK[k]`, at the input read of frame 35k + 1 (see below), so
the call `HeroUpdate` makes after reading input in frame 35k + 1 is in window k + 1. Window 1
starts at `DemoLoad` (RandSeed := 0) and includes the level setup, like `Demo.rng_calls()`.

## How it works

- **Loader** (`tools/ne.py`, `tools/win16.py`): the NE file is mapped into 16-bit protected
  mode on [unicorn](https://www.unicorn-engine.org/). NE segment n gets GDT selector
  0x1000 + (n−1)·8 with linear base `(sel >> 3) << 16`, so emulator addresses are Ghidra's
  addresses (1088:b5ff is `GameFrameInGame` in both) and consecutive selectors are contiguous
  (`__AHINCR` = 8). All relocations are applied; floating-point OS fixups are skipped (the CPU
  has an FPU). `GlobalAlloc` hands out fresh selectors from 0x2000.
- **Windows** (`tools/win16api.py`): each import points to a `retf N` stub; a code hook runs a
  Python handler first. About 130 APIs are stubbed: memory and selectors (`GlobalAlloc`,
  `AllocCSToDSAlias`, `PrestoChangoSelector` for OWL's instance thunks), windows (one main
  window; `CreateWindowEx` sends `WM_NCCREATE`/`WM_CREATE` and `ShowWindow` sends
  `WM_ACTIVATE`/`WM_SETFOCUS` into the program's own window procedure through nested
  `emu_start`), `PeekMessage` always empty (so OWL's idle loop runs `MainWindowIdle` →
  `GameFrame` forever), GDI/WinG/DispDib/MCI/mixer/joystick as no-ops, DiamondWare sound
  "present" but silent, `GetKeyState` = nothing pressed, `TimerCount`/`GetTickCount` a virtual
  clock that advances 1 ms per query, INI reads return the defaults (`--ini KEY=VALUE` to
  override).
- **DOS** (INT 21h): files are read from `work/cd` as drive `C:\ETHIS` (case-insensitive);
  files the game creates go to `work/tracer/overlay`. `BIN\VIDEOS` is hidden, so the intro and
  level videos are skipped the way the game skips a missing file.
- **Driving the demo**: the program boots from its NE entry point through the RTL and unit
  initialisation, the OWL application and the menu. After 5 menu frames the tracer calls
  `FUN_1080_17fa('.\Bin\usermaps\<DEMO>\<MAP>.map')`, which is what the user-maps menu does on
  a click; the level script's `PlayRecord=` makes `LevelLoad` call `DemoLoad`. From then on it
  is the game's own loop. The run ends at `DemoStop`.
- **Hooks**: `RandomNext` 1168:1570 (every `Random` variant near-calls it first, so the far
  caller is the next return address on the stack), `DemoLoad` (start of the log),
  `DemoPlayFrame` (CHECK samples), `GameFrameInGame` (frame count), `DemoStop`.

## What it found

- **The CHECK sampling point is one frame later than re/demo.md said.** `DemoRecordFrame`
  (1098:04ec) does not test its own counter: it tests the level frame counter `DAT_1170_617e`,
  which `GameFrameInGame` increments at its very end. So `CHECK[k]` is RandSeed at the input
  read of frame 35k + 1, and `CHECK[0]` is the seed after the level setup (CITY01: 0x96db1b23
  = 79 calls; it is not garbage). Sampling at frame 35k makes every window that starts or ends
  with walking look one call short (BROYEUR window 2: 29 instead of 30, which is the port's
  "unknown 30th call").
- Frame skipping really doesn't matter: clearing `gDrawThisFrame` on every frame leaves
  BROYEUR's counts unchanged. Neither do `GameDetail`, `GameStats`, `FrameRate` or
  `DisplayMode`, nor the pre-demo seed `HeroReset` draws from (`--pre-seed`), at least in
  BROYEUR's first 30 windows.
- Nothing in the recorded demos depends on the environment the stubs fake (sound, CD audio,
  clock, cursor): all of them replay exactly.

## Limits

- Only what the demos exercise is stubbed. An unimplemented API or DOS call stops the run with
  its name and caller; add it to `SPECS`/`HANDLERS` in `tools/win16api.py`.
- No sound, video, CD audio or input outside the demo. Code that reads them for game logic
  would differ (none found so far: sound playback doesn't draw Random, `SoundPlay` runs with
  the driver reported present).
- The virtual clock is not the real one. The game reads time for the frame limiter, the
  level timer (`FUN_1080_167b`) and the pause menu; none of that reaches Random in the demos.
- Speed: 190–440 game frames per second on an M-series Mac (unicorn JIT; the Python hooks
  only fire on Random, the stubs and once per frame).
- The pre-demo RandSeed (for `HeroReset`'s idle timer and two ambient-sound countdowns) is
  whatever the emulated menu left, not what the recording session had; `--pre-seed` sets it.
