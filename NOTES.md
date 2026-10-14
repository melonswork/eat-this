# Eat This (Image-Line, 1998) — reverse-engineering notes

## The disc

`iso/Eat This.img` is a CloneCD mixed-mode image: track 1 is data (MODE2/2352, ISO 9660, volume
`980306_0834`), tracks 2–7 are Red Book audio — the in-game soundtrack (see `BIN/TRACKS.TXT`).
`mise run extract` unpacks track 1 into `work/cd`.

| File | What it is |
|---|---|
| `ETLAUNCH.EXE` | **The game.** 16-bit Win16 NE, Borland C/C++ (`Portions Copyright (c) 1983,92 Borland`), ~2070 functions |
| `ET_DBG.EXE` | A debug build of the same game (different code, `DEBUG VALUE:` prints, asserts). No symbols, but a second view of the same code |
| `MENU.EXE` | Autorun menu / installer, 32-bit PE, Delphi 3. Copies WinG into place, picks language |
| `BIN/OVL/CY539BA9.EXE` + `.OVL` | Installer self-extractor; `.OVL` is a 120 MB encrypted PKZIP |
| `DWSW16*.DLL`, `DWSW32*.DLL` | DiamondWare Sound ToolKit (digital audio mixer) |
| `BIN/WING/*` | Microsoft WinG 1.0 (fast DIB blitting for Win 3.x) |
| `BIN/TILEMAKE.KIT` | Official map/tile editor kit (TILEMAKE, MLAUNCH) with docs on map scripts and objects |

Data formats, all custom: `.MAP` (levels, text scripts?), `.SPR` sprites, `.TIL` tiles, `.SCR`,
`.LBM`/`.PCX` (Deluxe Paint images), `.PAL`, `.DMO` recorded demos, plus an
"Image Line Archive File (1995)" container format referenced by the game.

## Why it doesn't run today

The game is Win16. 64-bit Windows has no NTVDM, so it can't start 16-bit programs at all; it also
depends on WinG, MMSYSTEM CD-audio via MCI, and DiamondWare's 16-bit driver.

## Paths to playable, cheapest first

1. **otvdm/winevdm** on 64-bit Windows: runs Win16 apps directly. Worth trying first just to see the game.
2. **Emulated Win9x** (86Box, DOSBox-X + Win98): faithful baseline to compare a port against.
3. **Source port**: decompile `ETLAUNCH.EXE`, rewrite the platform layer (WinG → SDL texture,
   DiamondWare → SDL audio, MCI CD tracks → ripped audio files), keep the game logic.
   Ship only our code; players supply their own CD data.

## Tooling

`mise install`, then:

- `mise run extract` — CD image → `work/cd`
- `mise run analyze` — headless Ghidra import + auto-analysis into `work/ghidra`
- `mise run ghidra` — Ghidra GUI (open project `work/ghidra/EatThis`)
- `mise run ghidra-natives` — one-off after installing Ghidra: builds the macOS arm64 decompiler,
  which the release zip doesn't ship
