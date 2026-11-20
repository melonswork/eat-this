# Front end (segment 1080)

The menus, the level start, the stats screen and the settings. Port: `port/src/frontend/`
(`mod.rs` state machine + window loop, `menu.rs` menu objects, `gfx.rs` drawing and text,
`texts.rs` words and EatThis.ini). `eatthis` with no `--map`/`--demo` runs it.

## Boot (FUN_1008_0c73 -> FUN_1088_d60e)

1. `FUN_1080_336d`: loads `PICTURES/SLIDERBC.PCX` into the 64000-byte buffer 1170:4720 and keeps
   its palette at 1170:4710 (**the menu palette**: every menu screen is shown with it). RandSeed 0,
   then 512 x `Random(8)` into 1170:494e (filled from index 0x1ff down: the noise table). Tint table
   rows 6..9 from the menu palette (below). Sprite groups: `PPFONT3.SPR` slot 0x23 (fitted to the
   palette, FUN_1088_454d), `ETCIFER3.SPR` 0x27, `MENUSL.SPR` 0x24, `EPISPIC.SPR` 0x29,
   `EXITPIC.SPR` 0x2a (not fitted, FUN_10f0_1e01: drawn in the menu palette as they are).
2. `FUN_1088_d04c` (empty), `d073` (PPGLIGHT/SVN/FACBROCK, game groups), `b96b`.
3. `FUN_1080_3719`: the menus (below), texts from `MAPS/ETMENU<Language>.SCR`.
4. `PICTURES/MENUBACK.LBM` into 1170:4720: the menu background.
5. Main menu (`FUN_1080_05fb(472c)`), palette set, then `VIDEOS/INTRO.AVI` (FUN_1008_1685); its
   end callback 1080:17be plays the level music if any (none at boot: the menus are silent).

`FUN_1088_d4f2` (startup before that) reads EatThis.ini (FUN_1038_03f5), scans the user maps
(FUN_1030_04d3), sets up the sound and allocates the screens.

## A menu frame (GameFrame -> GameFrameMenu 1080:2c70)

- Pointer position (FUN_1088_42a3), 1170:108a = 0x7fff.
- If a demo plays behind the menu (`gDemoPlaying`, FUN_1080_0195): one game frame drawn into 4720
  with the game's RandSeed and palette swapped in, then greyed (FUN_1080_0002). ETMAIN.SCR has
  `Demo=\DemoBroyeur\broyeur.map`, but that folder is not on the CD: nothing plays. Not ported.
- `FUN_1080_002a`: screen = 4720 + noise: for each of 250 rows of 256 bytes, 64 dword adds of the
  noise table from `Random(0x101)` on (carries cross bytes). The pictures are dark and the palette
  is arranged in ramps, so this is a shimmer. The menus' Random state is their own (RandSeed swaps
  at 1170:4718/471c in FUN_1080_02b3); the game's never sees it.
- `ReadPlayerKeys` (arrows/numpad 8 2 4 6, Space/Enter/numpad 0, plus the joystick 1a54..1a59).
- Cursor sprite 1170:494c = 0x241e; FUN_1080_120f glides the pointer to an item a key selected.
- 1170:1080 > 0 (a menu change): -0x60 per frame from 0xb40 (30 frames); with
  `a = sin(1080 + 0x5a0) * 0x5a0 / 0x4000`, `f = sin(0xb40 - a) * 0xa0 / 0x4000` (sin table
  1170:6bb8, 0x1680 = a turn) the new menu is at `x0 + (0xa0 - f)`, the old one at
  `x0 + 0x140 - (0xa0 - f)`; only the old one's draw runs, cursor 0x2420. Else the current menu's
  update (VMT +0x10).
- Not in game: the current menu's draw (VMT +0x14), the cursor sprite at the pointer (mode 4).
- 1086 = 108a; key repeat 108c -= 1. GameFrame then turns mouse buttons 1 into 2.

## Menu objects

Menu (FUN_1080_0eed): +0xe x at rest, +0x10 x drawn, +0x12 y of the first item, +0x14 spacing
(0x11), +0x16 selected (-1), +0x18 last, +0x1a key request, +0x1c default (selected when shown),
+0x1e..22 pointer glide, +0x24 parent (GET BACK / Escape target; set by 05fb when 108e = 1),
+0x28 title sprite (0x7fff none) at (x + +0x2a, +0x2c) on a band darkened through tint row 7,
+0x2e/+0x30 menu specific. VMT: +0xc on show, +0x10 update, +0x14 draw.

Items (FUN_1080_0f99 places each `spacing` below the last):
- label (VMT 0f3c): text; box `(-(w/2 + 0x16), -h, w/2 + 0x16, h)` centred (1084 = 1) or
  `(-0x16, -h, w + 0x16, h)` (1084 = 0), h = 1082 (6; 5 for the user map list). Text drawn at
  `(x + box.left + 0x16, y + 3)`; selected: FUN_1080_0722 = 8 copies at the neighbours in mode
  0x38 row 9 (brightens what is under them) then the text in mode 0x2c row 6; else mode 4.
  Update 095c: mouse button pressed or Enter -> 494a = 1 and sound 0xa1 at the pointer.
- slider (VMT 0f54, FUN_1080_0981): value +0x40, max +0x42, step +0x46, page +0x48; box widened
  0x35 each side, -7..6 high; bar at `x + box.right - 99` (+0x3e). Draw 09fe: 0x2402 (lit) clipped
  right at knob-8, 0x2403 clipped left at knob+8, knob 0x2404 (0x2406 while clicked) at
  `bar + 8 + value * 42 / max`, all at y + 6. Update 0b5f: Left/Right +-step, PgUp/PgDn +-page
  (repeat every 3 frames after 0x12), mouse in [bar, bar+0x3a]: `value = ((mx - bar - 8) *
  (max + 1) - 1) / 42`; [bar-0x13, bar-1] and [bar+0x3b, bar+0x4d] step down/up; sound 0x96.
- arrow buttons (VMT 0f6c): the user map list's scroll arrows 0x240c / 0x240e.

Common update FUN_1080_1330: Up/Down/Home/End (repeat 8) request an item; when no glide is
running the item under the pointer is selected (none: -1); a requested item is selected with the
glide and sound 0x98 at half volume, pitch `0x100 - Random(0x14)`. No selection: a click on the
title goes back. Else the selected item's update.

FUN_1080_05fb (show a menu): from the game it first switches to menu mode (FUN_1080_02b3(1, 0):
music at half volume (MixerSetVolume(1102 / 2)), the game's last frame copied to 4720 and greyed
through tint row 5 (FUN_1080_0002: row 5 of the level's tint table is `min(l / 2, 20) + 0xd0`,
a ramp of the menu palette), pointer free). Else the old menu loses its selection, the slide
starts, sound 0x85 at pitch 0x55, volumes 0x80. Then on show, the default item selected.

FUN_1080_1192 (GET BACK): the parent menu without linking, or back to the game (02b3(1, 1))
when there is none. Resuming shows the stats again if they are still open (48d0 != 0x7fff).

| menu | DS | VMT | (x, y) | items | update / draw |
|---|---|---|---|---|---|
| main | 472c | 0f84 | (0xa0, 0x8b), spacing 14 | START NEW GAME, START USER MAP, OPTIONS, QUIT | 2270: episode (or 1170:11a6's map), user maps, options, quit. Logo: 0x2407 THIS, 0x2408 fire, +0x30 EAT at (x, 0); +0x2e counts 0x8c frames from entering menu mode, then EAT becomes 0x240a with sound 1 (pitch 0xaa) |
| options | 475e | 0f9c | (0xa0, 0x62), spacing 16, default 3, title 0x2419 | DISPLAY/SOUND SETTINGS, ARTWORK, GET BACK | 2391 |
| display | 47e6 | 0fcc | (0xa0, 0x3f), title 0x241a | gamma slider (0..15), detail slider (0..4), display mode text, size slider (0..5, table 1170:1a70 320x200..640x400), tempo text, vsync text, RESET, GET BACK | 270b applies values live; mode cycles 3 (VideoDispDibShow), tempo 4, vsync 2; reset = detail 3, tempo 0, vsync 1 |
| sound | 4814 | 0fb4 | (0xa0, 0x4a), title 0x241b | FX volume (0..0x100 step 8), mixed channels (2..16), mixing buffer (0x200..0x1000 step 0x40), mono/stereo text, CD volume, RESET, GET BACK | 2486: a looping test sound 0x2c plays at the pointer (FUN_10e0_0408) |
| user maps | 478c | 0fe4 | (0x14, 0x22), spacing 12, title 0x241c at (+0x8c, 0xf) | one label per map, left aligned, clipped to y 0x1b..0xa1 | 2a95 scrolls (+0x54) by 12, PgUp/PgDn 0x78, the arrows or the pointer at the screen edge; 2927 draws the selected map's Comments (y 0xae) and CommentPic (bottom right) on a dark band |
| restart | 4844 | 0ffc | (0xa0, 0x78) | YEP, NOPE (messages -4, -5) | 1963: YEP stops the level (FUN_1088_d3e7) and loads 1170:1106; text -7 above |
| quit | 4872 | 1014 | (0x4c, 0x72) | YEP, NOPE | 19d2: YEP quits; text -6, EXITPIC 0x2a00 at (x - 0x4c, 0) |
| stats | 48a0 | 102c | (0xa0, 0xa6) | CONTINUE | see below |
| artwork | 48d2 | 1044 | (0xa0, 0x5a) | REPLAY INTRO, JUKE BOX slider (0..CD titles-1), CREDITS, GET BACK | 1f8f: INTRO.AVI, CD track value+2, CREDITS.AVI; the track title (ETMAIN CDTrackTitle=) at y 0xb8 |
| episode | 4900 | 105c | (0xa3, 0x6c), left aligned | INVASION, KICKING THEIR BUTT, LAST ASSAULT | 2151: loads `MAPS\` + 1170:066c[i] (START.MAP, FOREST04.MAP, FORPLANE.MAP); draw 219f: bands y 0..0x20 and 0xa7..199 darkened, EPISPIC 0x2900+i at (x-1, y-0x36), message -0x24 "SELECT YOUR EPISODE" at (x, y - 17) |

Cursor 0x2422 in stats, sound, artwork, episode, restart and quit.

Keys (FUN_1008_1966, WM_KEYDOWN): Escape = main menu when no level runs, in game, or on the
stats, else back to the level; F1 ETHELP.AVI; F3 user maps; F4 next display mode; F5 options;
F6 display; F7 sound; F11 gamma + 1; numpad +/- GameStats (1a92, the HUD).

## Texts

`ETMENU<n>.SCR`: the lines that are not empty and do not start with `'`, in order: 3 display
modes (46e4), 4 tempos (46f0), 2 vsync (4700), 2 mono/stereo (4708), then the items in menu
order (main 4, options 4, display 5, sound 6, stats 1, artwork 4, episode 3).
Global messages (FUN_1088_1171, negative ids): ETMAIN.SCR's `AddMsg=text,language` lines of the
current language; id -1 is the last (-4 YEP, -5 NOPE, -6 LEAVING SO SOON?, -7 THIS WILL END YOUR
CURRENT GAME..., -8 BIOS SPHERES, -9 HIDDEN AREAS, -10 KILLS, -11 TIME, -0x24 SELECT YOUR EPISODE).

Text (FUN_1028_006e, font set by FUN_1028_03bc(gap 2, spacing 0, slot)): glyph `slot*256 + c`
at its hotspot, advance = width + spacing; `^Cnn` = mode 0x2c through tint row nn (not while
glowing), `^O` back, `^|` new line (height of glyph '0' + gap), `^Bnn` blink. 03eb centres each
line, 0492 right-aligns. PPFONT3 has no lower case: lower-case letters are skipped (the user maps'
"Eat This original map" shows as "E T").

Tint rows (MIX = 1b0c points at the tint table 4c26 in the menus), from the menu palette with
`l = (299 r + 587 g + 114 b) / 1000`: 6 = nearest(l*20/9, l*20/9, l*4/3) (selected text),
7 = nearest(c - 3) (darker bands), 8 = nearest(2l, l/3, 0) (`^C08`), 9 = nearest(r+5, g+3, b+5);
in the ramps 0xc0..cf, 0xd0..ef, 0xf0..fe rows 7/9 move 3 entries inside the ramp.

## Levels

FUN_1080_17fa(map): 1106 := map; a level running (0f2a >= 1) -> the restart question. Else:
weapons' digit keys reset (06b8, not after stats), 4720 filled with colour 1, LevelLoad
(FUN_1088_bc00), then the script's start video (`Video=X.avi,0`, collection 4b5e, FUN_1080_161b(0))
whose end calls 16b7, or 16b7 at once. FUN_1080_16b7: menu mode -> game mode, FUN_1088_d4b2
(RandSeed 0x3e00, HeroReset), 4720 freed, FUN_1080_167b (play time zeroed, MixerSetVolume(1102),
CD track `CDTrack=`), LevelFirstFrame. The port: `Game::new` after `load_world`, `level_music`.

GameFrame after the update, 4e60 > 0: a demo playing or recording stops (DemoStop/DemoSave) and
4e60 becomes 0x1e = main menu (so a demo that reaches its level end shows no stats);
10 = restart (HeroReset, map reloaded, 4dc2 += 1); 100 = `Video=X.avi,1` then 1080:172a.

## Stats (FUN_1080_172a, menu 48a0)

Shows the stats menu, loads the script's `EndPic=` (default 1170:be06, statsb01.lbm) into 4720,
1ae2 = 0xff, `STATSOBJ.SPR` into slot 0x28 (fitted), MixerSetVolume(0xff), CD track 6 (WELL
DONE), FUN_1080_2fc4 (savings, below), 4dc2 = 0.
On show (1a6a): +0x30 = -40, +0x2e = 0x4c + 10 for each counter whose total is 0.
Update (1ab2): +0x30 counts up to 0x4000; from 0 the menu works; CONTINUE: FUN_1088_d3e7, 0f2a = -1,
48d0 = 0x7fff, then "NEXT EPISODE" (4d97) -> episode menu, else NextLevelMap (4c3a) -> 17fa,
else the main menu.
Draw (1ca3), from +0x30 >= 0: the items; FUN_1050_0017 (the shake: offset =
`(|count % 6 - 3| * amplitude * count / start) / 3`); x +-0x78 darkened through row 7; EndMessage
(4c8a) centred at y = shake + 0x1c (mode 0x2c row 6); then at y = +0x2e, threshold 40:
bios spheres (4ea8 / total 4eaa, icon 0x2824 + (frame/3 & 7)), hidden areas (4eac / 4eae,
0x280c + frame/9 % 12), kills (4eb0 / 4eb2, 0x2800 + frame/9 % 12), each once +0x30 reaches the
threshold and only if its total > 0: label at (x - 0x4b, y), "n/total" in ETCIFER3 ending at
x + 0x5c, icon at (x - 0x50, y - 1 - (icon < 0x2824)); y and threshold += 20. Then TIME (4eb4 ms,
`m:ss`, hours in front when > 0) with icon 0x2818 + frame/9 % 12 at y - 2. When the clock reaches a
threshold exactly: sound 0 at pitch 0x100 - Random(10) and ScreenShake(0xe, 6).

Totals (SpawnFromIconTiles 1088:1797): plane-2 cells 0xd00 (bios), function 61800 (hidden areas),
61900 (enemies) and every enemy EnemyFactory makes during LevelInit (kinds 1428 1432 145a 14d2
13e2 13ec 13f6 140a 146e 1482, 14be adds its passengers). Counts: 4ea8 FUN_1040_014a (heart
pick-up), 4eac script command 0x18 (FUN_1088_a9af) and clearing a 0x316 cell, 4eb0 the kills.
Port: `level::Tally` (passive counters the simulation adds to) and `frontend::level_totals`.

## Savings (FUN_1080_2fc4)

After a level of `MAPS\`, the NEXT level's `.map` and `.scr` are copied to
`USERMAPS\SAVINGS\<name>\` (2554 = the user map folder): START USER MAP can start there later.
This is the game's only save. Port: `SAVINGS/<NAME>/` in the working directory (`ET_SAVINGS`).

User maps (FUN_1030_04d3 / 00f6): every folder under the user map folder; each `*.scr` with a
`.map` beside it is an entry: `Title=` (an item; `HIDDEN` drops it), `Comments=`, `CommentPic=`
(a picture of PICTURES or the folder, made into sprite 0x2500). A script with `PlayRecord=` plays
that recording (the CD's demos in USERMAPS).

## Settings: EatThis.ini, section [Eat This] (WritePrivateProfileString)

Read at startup (FUN_1038_03f5), written by FUN_1038_012b:
`GameDetail` (16d9, 3), `GameStats` (1a92, 2), `WindowW`/`WindowH` (1a6c/1a6e, 640x400),
`DisplayMode` (1a66, 0 window / 1 scanlines / 2 full screen; >= 3 read as 0), `WaitForRetrace`
(1a8a, 1), `FrameRate` (1a62, 0 auto / 70 / 35 / 23), `GammaCorrection` (1a88, 4), `BufferLength`
(1ac2, -1), `Volume` (1ac8, 0x100), `nVoices` (1ac4, 16), `Stereo` (1ac6, 1), `Language` (1a96,
0..5: ETMENU<n>.SCR and the `,n` of script texts), `HDDir` (read only: the user map folder),
`Up Down Left Right LeftShoot RightShoot SwitchWeapon` (gKeyMap 1640.., VK codes 0x26 0x28 0x25
0x27 0x2d 0x20 0x09). There is no key setup screen: keys are only in the file.
Gamma (VideoSetPalette, 1008): `min((c + gamma) * 4, 255)` per 6-bit component; entries 0 and 255
are black and white.

## In the port

- Videos (VIDEOS/*.AVI: intro, help, credits, level start/end) are not decoded: skipped.
- CommentPic of the user map list is not drawn (PCX into a sprite); the list's title sprite is
  clipped away as in the original's code.
- Stored but without effect: GameDetail (the simulation keeps the demos' detail 3), mixed
  channels, mixing buffer, mono/stereo, wait for vsync. Applied: gamma, display mode (scanlines =
  black odd lines, full screen = desktop full screen), display size, frame rate (draw every 1/2/3
  frames), FX and CD volume, GameStats (the HUD), keys, language.
- The demo behind the menu (Demo= in ETMAIN.SCR) and the joystick are not ported.
- The kill total counts the enemy objects the port's setup made (unported kinds are missing).
- Debug: `ET_FE_SHOT="steps"` runs the front end without a window (see `frontend::shot`),
  `ET_INI` / `ET_SAVINGS` move the settings file and the savings folder.
