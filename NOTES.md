# Eat This (Image-Line, 1998) — reverse-engineering notes

## The disc

`iso/Eat This.img` is a CloneCD mixed-mode image: track 1 is data (MODE2/2352, ISO 9660, volume
`980306_0834`), tracks 2–7 are Red Book audio — the in-game soundtrack (see `BIN/TRACKS.TXT`).
`mise run extract` unpacks track 1 into `work/cd`.

| File | What it is |
|---|---|
| `ETLAUNCH.EXE` | **The game.** 16-bit Win16 NE, Borland Pascal 7 + ObjectWindows (string literals live in code segments, WinCrt and OWL units linked in), ~2070 functions |
| `ET_DBG.EXE` | A debug build of the same game (different code, `DEBUG VALUE:` prints, asserts). No symbols, but a second view of the same code |
| `MENU.EXE` | Autorun menu / installer, 32-bit PE, Delphi 3. Copies WinG into place, picks language |
| `BIN/OVL/CY539BA9.EXE` + `.OVL` | Installer self-extractor; `.OVL` is a 120 MB encrypted PKZIP |
| `DWSW16*.DLL`, `DWSW32*.DLL` | DiamondWare Sound ToolKit (digital audio mixer) |
| `BIN/WING/*` | Microsoft WinG 1.0 (fast DIB blitting for Win 3.x) |
| `BIN/TILEMAKE.KIT` | The official DOS tile/map editor by Didier Dambrin (who later wrote Fruity Loops), with docs on tile functions (`TM/TILES.TXT`) and map objects (`TM/OBJECTS.TXT`, `TM/MMDOC.TXT`) |

## Data formats (`tools/`)

Every data file is an **Image Line Archive File (1995)** (`tools/ilaf.py`): a 31-byte magic, a
directory of `{u32 offset, u32 size, u8 flags, string[12] name}` entries, Pascal strings
throughout. Entries with `flags=1` are `u32 unpacked size` + **GIF-style LZW** (9–12 bit codes,
LSB-first, 0x100 clear, 0x101 end; `tools/lzw.py`, lifted from `LzwDecompress` at `1108:00d6`).
All 3987 entries on the disc decode.

| Archive | Entries | Decoded |
|---|---|---|
| `.TIL` "VGA TILE GROUP" | `TILE DATA` 16×16 8-bit tiles, `TILE INFOS` 16 bytes/tile, `PALETTE` | yes |
| `.SPR` "VGA SPRITE GROUP" | `SPR-HEADER`, `SPRITE n` (or `EMPTY`), `PALETTE`, `USED CLRS` | yes: u16 w, h; i16 hotspot x, y; u16 n + n bytes of hitboxes; then per row u16 span|0x8000, u16 first x, w pixels |
| `.MAP` "TILE MAP FILE" | `MAP HEADER` (u16 @2 width, @4 height), `PLANE 0..2` u16 cells (hi = tile group slot, lo = tile), `OBJECTS`, `FILENAMES` (background `.LBM` + 16 `.TIL` slots, string[12]) | planes yes, objects no |
| `.LIT` `.MIX` `.SEA` "VGA PALETTE … MIXAGE" | `PALMIX`: lookup tables for light, translucency and water | not yet |
| `.REC` "EAT THIS DEMO" (in `USERMAPS/*.DMO/`) | recorded input: `REC HEADER`, `MOUSE`, `KEY #0..8`, `CHECK` | yes: [re/demo.md](re/demo.md); `CHECK` holds RandSeed every 35 frames |

Palettes are 6-bit VGA. Colour index 0 is transparent in tiles and sprites.
`mise run render -- map work/cd/BIN/MAPS/CITY01.MAP out.png` renders a level.

Open questions: which palette a map really uses (probably the background `.LBM`'s; tile group
palettes look darker), plane order (0 looks like the back plane), the `OBJECTS` script format.

## How the game runs

- **Main loop**: OWL idle hook `MainWindowIdle` (`1008:05cc`) → `GameFrame` (`1080:309a`) then
  `VideoPresent` (`1008:0363`), every idle tick while the window is active.
- **Video**: the game draws into a **320×200 8-bit framebuffer** (64000 bytes, mode 13h style).
  `VideoPresent` limits the frame rate with TOOLHELP `TimerCount`, optionally busy-waits on VGA
  port `0x3DA` (privileged I/O, faults on NT), then WinGBitBlt/StretchBlt to the window or
  DispDib fullscreen.
- **Input**: `KeyPoll` (`1160:0002`) wraps `GetKeyState` into per-key hold counters; joystick via
  `joyGetPos`.
- **Sound**: DiamondWare `DWS_*` (`10e8:*`), WAVs converted at load time.
- **Music**: MCI `play cdaudio` strings (`10c8:02e0`), mixer API for volume.

Function names live in `re/names.csv`; `mise run names` applies them in Ghidra and re-exports
`work/decomp/ETLAUNCH.EXE.c`. `work/decomp/ETLAUNCH.EXE.imports.txt` maps every Win16 import to
its callers.

## Why it doesn't run today

Win16 binary (no NTVDM on 64-bit Windows, nothing on macOS), WinG, the 16-bit DiamondWare driver,
CD audio over MCI, and direct VGA port I/O.

## Plan: source port

1. Formats ✔ — palettes, tiles, sprites, map planes. Demos ✔. Next: map objects, light/mix tables.
2. Platform layer ✔ mapped. It is narrow: framebuffer present, palette, key state, timer, sound
   play/update, CD track play.
3. Decompile the game proper (`1088` is the 64 KB core segment, `1080` the frame/menu logic),
   naming as we go, using `ET_DBG.EXE` as a cross-check. In progress: hero movement is
   documented in [re/hero.md](re/hero.md).
4. Port ([port/](port/README.md), Rust + SDL2, started): data loading, the main-plane tile
   queries, Leon's core movement, a 70 Hz loop, demo playback and the headless RandSeed check
   work. Original plan: game logic in a modern language over SDL (texture for the framebuffer, SDL audio for
   samples, ripped CD tracks as files). Ship only our code; players bring their own CD.
   Verification: the 7 recorded demos (124 345 frames) carry the RNG seed every 35 frames, which
   gives the exact `Random()` call count per window. A port that matches it is deterministic
   (see [re/demo.md](re/demo.md)).

## Tooling

`mise install`, then:

- `mise run extract` — CD image → `work/cd`
- `mise run analyze` — headless Ghidra import + auto-analysis into `work/ghidra`
- `mise run names` — apply `re/names.csv` and re-export decompiled C into `work/decomp`
- `mise run decomp` — re-export only
- `mise run render -- {tiles|sprites|map} <file> <out.png>`
- `uv run python tools/frames.py SPRITES/PPHERO.SPR out.png 0x81-0x94`: a strip of numbered frames
- `python tools/fn.py {show|tree|callers} NAME`: browse the exported decompilation
- `mise run trace -- work/cd/BIN/USERMAPS/GRINDER.DMO` — run the original game under an x86
  emulator on a demo and log every `Random` call ([re/tracer.md](re/tracer.md))
- `mise run ghidra` — Ghidra GUI (open project `work/ghidra/EatThis`)
- `mise run ghidra-natives` — one-off after installing Ghidra: builds the macOS arm64 decompiler,
  which the release zip doesn't ship
