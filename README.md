# ![Eat This](https://capsule-render.vercel.app/api?type=blur&height=300&color=0:7a1f0e,100:2b2b2b&text=EAT%20THIS&textBg=false&fontColor=faf8f8&fontSize=72&desc=a%20faithful%20Rust%20port%20of%20the%201998%20Win16%20shooter&descSize=16&descAlignY=65)

[![mise](https://mise-versions.jdx.dev/badge.svg)](https://mise.jdx.dev)
![Rust](https://img.shields.io/badge/rust-1.100-b7410e?logo=rust)
![SDL2](https://img.shields.io/badge/SDL2-bundled-1d4f91)
![oracle demos](https://img.shields.io/badge/oracle%20demos-82%2F82%20exact-2e7d32)

![FOREST04, rendered by the port](docs/screenshot.png)

*Eat This* is a 1998 side-scrolling shooter for Windows 3.1/95 by Image-Line, written in Borland
Pascal 7. This repo reverse engineers it (Ghidra plus an x86 emulator that runs the original
EXE) and ports it to Rust + SDL2. It runs natively on a Mac, with no Windows and no VM.

The port aims to be exact, not merely look right. The game's recorded demos store its random
seed every 35 frames. The port replays each demo and has to match every one of those checks,
every `Random()` result, and the hero's position on every frame. New demos for features the CD
demos never show are recorded with the original game itself (`tools/mkdemo.py`).

## Progress

- **Simulation:** all 7 demos from the CD and 75 scenario demos match the original exactly.
  Every object type on the CD's episode maps is ported, and so are the platform types only a custom
  map could use. What couldn't be checked is in [`re/unported.md`](re/unported.md).
- **Graphics:** tiles, sprites, parallax backgrounds, weather, the HUD, the message box and the
  automap. All 1,722 sampled frames match the original pixel for pixel, except one where the
  original's own idle animation depends on how it was started.
- **Sound:** sound effects through an SDL2 mixer, and music from the CD's audio tracks. All
  9,238 sound plays in the 82 demos match the original's.
- **Front end:** the original's menus, episodes, the stats screen, the next level, saves and
  `EatThis.ini`. The videos play through an Indeo 3 decoder written for the port.
- **Extras:** all seven planned ones are in (see below). Controllers and rumble have only been
  tested with scripted input, not a physical controller.

## Play

**Controls:** arrows to move and jump, the mouse to aim and fire (Insert/Space fire too), Tab or
1–0 to switch weapons, Esc for the menu.

**Extras** (Options > Extras): upscaling and CRT shaders, glow, widescreen, rewind (Backspace) and
recordings, all off by default; save states (F8/F9, F10 for the slot), remappable keys, and game
controllers, which work when plugged in. See [`docs/extras.md`](docs/extras.md).

## Build and run

You need your own copy of the original CD as a BIN/CUE image. The game's data is not in this
repo.

### mise

```bash
mise install                                                       # Rust, Python/uv, cmake, 7zip, Ghidra
cp "/path/to/Eat This.img" "/path/to/Eat This.cue" iso/
mise run extract                                                   # game data -> work/cd
python3 -I tools/extract_audio.py "iso/Eat This.cue" work/cd/AUDIO # music (optional)
mise run port                                                      # build and start at the menus
```

### plain Cargo

From `port/`:

```bash
cargo run --release -- --demo ../work/cd/BIN/USERMAPS/LEVEL1_2.DMO            # watch a demo
cargo run --release -- --map ../work/cd/BIN/MAPS/CITY01.MAP                   # play one map
cargo run --release -- --headless --demo ../work/cd/BIN/USERMAPS/LEVEL1_2.DMO # check it against the original
```

The reverse-engineering workflow (tracer, diffs, scenario demos) is documented in
[`AGENTS.md`](AGENTS.md). The notes on each subsystem are in [`re/`](re/), and
[`docs/handbook.md`](docs/handbook.md) is a handbook on the method, in Polish.

`port/src/frontend/indeo3.rs` is a port of FFmpeg's Indeo 3 decoder and is under LGPL-2.1.
