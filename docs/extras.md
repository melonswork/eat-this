# Extras on top of the port

Features the original never had. The rules for all of them:

- **Opt-in.** The defaults play exactly like the original.
- **Kept out of the simulation.** `tools/checkall.sh` must stay exact. Extras read game state; they never add or move a `Random()` or change what an update does.
- **The one exception** is widescreen option B below, which changes behaviour on purpose. It is off in demos and in `checkall`.

**Status (2026-11-24): all seven are in** (OPTIONS > EXTRAS; settings in `EatThisExtras.ini`).
Where the result differs from the plan below:

- **Upscalers:** xBRZ is GPL and ScaleFX is another author's shader, so there are Scale4x (Scale2x twice) and an
  xBR-style shader written here instead. GPU path: OpenGL 3.3 through `glow`; without it, SDL's renderer and no shaders.
- **Glow and light** run on the CPU at the original's resolution (2-3.5 ms), from a mask the sprite drawing fills;
  the light comes from the lit pixels, not from particle records. START's lightning doesn't glow.
- **Save states** are the level's seed and inputs, replayed on load; they only load in a build whose simulation is the same.
- **16:10** is the original's own shape, so widescreen is 16:9 (+36 columns) or 21:9 (+148). Columns outside the hero's
  sector are black. The view moves over at a sector's edge instead.
- **Recordings** hold no weapon keys (1-0, PgUp, PgDn), because the original's format doesn't: Tab switches weapons.
- **DualSense triggers** (added later): `dualsense.rs` turns the two shoot buttons' state into the controller's own trigger
  effects (USB or Bluetooth, DualSense and DualSense Edge; Options > Extras > Controls sets the strength, 0 is off). Guns
  break, launchers are stiff and kick, the flame thrower and the automatic weapons rattle while they fire, an empty weapon
  is slack. Not felt on a real controller: the bytes are unit-tested and `ET_HAPTICS=1` with `--rerecord` lists the effects.
- Not tried with a physical controller: its inputs were tested through the scripted front end, the rumble not at all.

Code: `session.rs` (input log, states, rewind, recordings), `gpu.rs`, `scale.rs`, `light.rs`, `pad.rs`, `extras.rs`,
the end of `frontend/mod.rs`, and the view in `render.rs`. Checks: `--check-states --demo X.DMO`,
`--rerecord DIR --demo X.DMO` (then play it with `mise run trace`), `--wide N --shot F FILE`,
`ET_GLOW="60 50"` with `--shot`, `ET_GL_SHOT="in.ppm out.ppm UPSCALE CRT INTEGER W H"`, and `ET_FE_SHOT`'s
`set Name=value`, `pad BUTTON... [aim X Y]` and `print`.

Planned order: 1, 2, 3, 4, 5, 6 and the recordings (7) last.

## 1. Controller

- SDL2 GameController: standard layouts for Xbox, PlayStation, Switch and Steam Deck controllers; hot-plugging.
- Left stick or d-pad: move, jump, crouch. Right stick: the aiming cursor, either free or locked to a circle around the hero (a setting). Triggers: primary and secondary fire. Bumpers: previous and next weapon. Start: menu.
- Feeds the same inputs ReadGameInput reads (keys, cursor, mouse buttons), so the simulation can't tell the difference.
- The menus work with the controller too.
- Rumble on screen shakes, explosions near the hero, and the hero getting hurt or dying; strength scaled by distance and size.

## 2. GPU output path and shaders

- Replace the final SDL2 texture copy with a GPU path: `wgpu` (preferred) or OpenGL through `glow`. The software renderer still produces the 320×200 indexed frame; the GPU only does output.
- Shaders, each a post-process on the finished frame:
  - **Sharp bilinear:** crisp pixels at any window size, without uneven pixel widths.
  - **Pixel-art upscalers:** xBRZ, ScaleFX.
  - **CRT:** scanlines, curvature, phosphor mask, a little bloom.
- Picked in the settings (6); shaders can be chained.

## 3. Glow and light

- **Glow:** a mask of emissive palette colours (fire, flashes, lasers, lightning, the HUD), blurred and added: explosions, muzzle flashes, burning wrecks, START's lightning.
- **Light from particles:** each particle knows its type and position, so explosions and fire cast coloured light on nearby tiles and sprites.
- Drawing only; built on 2.

## 4. Save states and rewind

- Quick save and load at any moment (several slots), on top of the original's own save between levels.
- Rewind: a ring buffer of snapshots, held down to go back.
- Needs the whole game state (Game, Level, particles, the RNG, sound loops) to be snapshot-able: Clone or serialisation.
- Check: save, load and replay a demo from the loaded state; it must stay exact.

## 5. Widescreen

Two separate options:

- **A. Extra columns only.** Draw extra columns of the level on both sides to fill a 16:9 or 16:10 window. The internal camera stays 320 pixels wide, so the simulation is unchanged and demos stay exact. Downside: enemies in the extra columns stand still until they come into the original 320-pixel view (actors only update while their box meets the camera).
- **B. Wide simulation.** The camera that wakes actors is as wide as the screen too, so enemies at the edges act normally. This changes the game: enemies wake earlier and from further away. Off in demos; `checkall` runs without it.
- The HUD, message box and menus stay in the 320-pixel centre, or are anchored to the edges.

## 6. Settings

- Key remapping, controller remapping.
- Fullscreen, window size, integer scaling.
- The shader picker (2, 3), the widescreen mode (5), and rumble strength.
- Stored in a separate file next to `EatThis.ini`, so the original's ini stays as it was.

## 7. Recordings (last)

- Record a run in the original's demo format (the recorder already exists in the tooling) and play it back from the menu.
- Players can share their runs; the files also work as oracle demos for the original game.
