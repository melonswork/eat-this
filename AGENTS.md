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

## New oracle demos (tools/mkdemo.py, 2026-11-20)

The seven CD demos never exercise many features. `tools/mkdemo.py` makes NEW ones: the original
game (the tracer's emulator) records them with its OWN recorder from a scenario script, so the
.REC carries the original's CHECK entries. Scenarios live in git (`tools/scenarios/*.txt`), the
demos they make in `work/demos/BIN/USERMAPS/<NAME>.DMO` (regenerate them, ~5 s each):

```
mise run mkdemo -- tools/scenarios/WEAPON5.txt                         # -> work/demos/BIN/USERMAPS/WEAPON5.DMO
mise run trace -- work/demos/BIN/USERMAPS/WEAPON5.DMO --overlay work/demos --watch gHeroX,gHeroY,gHeroState
(cd port && ./target/release/eatthis --headless --demo ../work/demos/BIN/USERMAPS/WEAPON5.DMO)
python3 tools/windiff.py WEAPON5 4 ; python3 tools/valdiff.py WEAPON5   # both look in work/demos too
tools/checkall.sh [DEMO...]   # every oracle demo: windows, where it ended, valdiff, hero x/y/state
```
`tools/checkall.sh` needs a full `mise run trace -- ... --watch gHeroX,gHeroY,gHeroState` of each demo
(work/tracer/<DEMO>/); rerun the trace whenever a scenario changes.
Script lines: `level GRINDER` (a CD demo folder) or `level MAPS/BOMS02.MAP` (any map under
work/cd/BIN), `start X Y` / `cell PLANE TX TY VALUE` (patch the demo's COPY of the map: the hero's
start in MAP HEADER, a cell; tools/mapedit.py, a literal-only LZW the original reads fine), `name X`, `mouse X Y` (game-screen pixels, from the next frame on), `N keys...` (hold
these for N frames; none = all up). Keys: up down left right fire alt key6 key7 switch (gKeyMap),
kp8 kp2 kp4 kp6 kp0 (numpad: these report 0 on release; BROYEUR was played with them), lmb rmb.
Script frame i = game frame i+1 = port frame i. `mkdemo.py LEVEL --from-rec X.REC` prints a script
replaying a recording: re-recording BROYEUR, HYDRO_02 and CITY01 that way gives their exact input
streams and EVERY CHECK, so the method is the original's own.
- How: the demo folder's .SCR gets `PlayRecord=<name>.rec`; the file is missing, so LevelLoad calls
  DemoStartRecording (RandSeed := 0) instead of DemoLoad. GetKeyState is answered from the script
  (VKs read from gKeyMap), the mouse buttons are the globals 1a5c/1a5e (0 up, 1 pressed, GameFrame
  turns 1 into 2), the cursor goes through GetCursorPos (FUN_1088_42a3 scales by 319/screen width,
  inverted in mkdemo). After the last frame mkdemo calls DemoSave.
- The game stops recording by itself when the level ends or restarts (GameFrame: 4e60 > 0 ->
  DemoSave): a demo can show the hero dying but not the restart after it.
- Digit/PgUp/PgDn weapon keys aren't replayed by DemoPlayFrame: get weapons from pick-ups
  (`ET_PICKUPS=1 ./target/release/eatthis --headless --map ../work/cd/BIN/MAPS/X.MAP` lists them).
- A frame count that is a multiple of 35 leaves the last CHECK unsampled (garbage); mkdemo pads one.
- The port must run from `port/` (default data path `../work/cd/BIN`).
- Finding a route: `ET_PICKUPS=1 ./target/release/eatthis --headless --map ../work/cd/BIN/MAPS/X.MAP` lists the
  start, weapon pick-ups and exits (plane-2 function 9000; a door tile 7050 in the main plane must be open,
  its function 2 = 1); `ET_TELE=1` lists teleport records (two-way, leave, both ends in pixels);
  `mise run render -- map work/cd/BIN/MAPS/X.MAP out.png` + crop to see it. Up = jump (crouch first for a
  higher one), Up against a ladder tile (function 10) climbs. To start somewhere a CD demo goes, cut the
  `--from-rec` script of that demo at a frame (its hero positions: work/tracer/<DEMO>/watch.tsv) and append.
- A demo starts with gMouseX/Y = its first mouse entry (DemoLoad): frame 0 is not a mouse move.
- Nothing carries over between levels: every level start from the menu (FUN_1080_16b7 -> FUN_1088_d4b2)
  sets RandSeed 0x3e00 and runs HeroReset (health 0x2c6, every weapon's ammo/owned zeroed, the kill/secret
  counters). A demo or a recording always stops at the level end, so no oracle crosses a level change.
  The NEXT level goes the same way: 4e60 = 100 -> stats screen 1080:172a (4dc2 = 0) -> its handler 1080:1ab2
  -> FUN_1080_17fa(4c3a = the script's NextLevelMap=, beside the map else MAPS/, 8.3-truncated) -> 16b7 ->
  d4b2, then LevelInit's Randomize: the original's state after the change is time-seeded, not reproducible.
  "NEXT EPISODE" (4d97) shows the episode screen, an empty name goes to the menu. Port: `next_level` in
  main.rs, the window loop loads it; `ET_NEXT=N` plays N idle frames of the next level after a demo that
  ends its level (e.g. EXIT1 -> F_JUMP).
- To reach a pick-up no route leads to, move the start (`start X Y`): ET_PICKUPS lists pick-ups, start,
  exits and NEXT; CITY_I04 has weapons 6 and 9 on crates in small rooms at x 983..999.
- dumpframe.py takes `--overlay` (work/demos by default for demos there). The tracer's `--particles` writes
  nothing now (1170:0918 reads 0 at its hook, even in GRINDER): fix before relying on it.

| scenario | level | what it checks | port |
|---|---|---|---|
| `DEATH1` | GRINDER | BROYEUR's first 300 frames, then waiting: the grinder hurts (FUN_1088_3c32 kind 1, returns 1 → FUN_1090_1a70) and kills the hero (frame 680, state 100, gibs/smoke/splat) | exact |
| `WEAPON3` | BOMS02 | climb the ladder at x 1336 to the ledge, weapon-3 pick-up, five homing rockets (FUN_1088_6b1f, callback 0x586, effect 14) | exact |
| `EXIT1` | GRINDER | BROYEUR's own input + 100 frames: the recorder stops itself at the level exit (4e60 = 100, frame 3968); the port must stop there too | exact |
| `DEATH2` | GRINDER | DEATH1 + key 6 while dead: the restart (4e60 = 10) stops the recording at frame 1801 | exact |
| `WEAPON5` | BOMS02 | weapon-5 pick-up, four rockets (FUN_1088_6deb, callback 0xd83 = the torpedo's 0xf9f without bubbles, effect 14, rubble effect 75) | exact |
| `WEAPON6` | CITY_I04, start moved to (940,95) | weapon-6 pick-up on the crate, step down (too low to stand there: crouched the hero can't fire), six spreads of three bullets (FUN_1088_70b8/7020, effect 62); no Random: checked with dumpframe | exact |
| `WEAPON9` | CITY_I04, start moved to (940,191) | weapon-9 pick-up, balls standing (Random(0xa1)) and crouched (laid flat), mover 1050:3044, callback 16cc, effect 56 (bounce, +8 frames, blast) | exact |
| `GRANNY1` | GRANNY, weapon-3 pick-up copied to (19,30) | the granny 0x1482 (FUN_1070_cb38, `granny.rs`): pistol hits knock her over (hat 0x510a, bleeding ca36), lying, stomps; rockets blow two apart (red gibs, smoke, splat) | exact |
| `CARRIER1` | CITY02, start moved to (1020,1039) | the carrier 0x14be (FUN_1070_0cce, `carrier.rs`): flies bobbing, drops its three aliens by distance, shot down it falls and crashes (0a34: dust, plates, blast); water splashes of aliens and the hero (FUN_1070_0105) | exact |
| `WALLGUN1` | FOREST02, start moved to (650,1340) | the gun turret 0x13ec (FUN_1070_2550, `wallgun.rs`) by the trunk: fires on its cycle, ~110 pistol shots kill it (smoke, gun pieces) and break it (-0x80: plates, pieces with effect 65 = FUN_1040_105d) | exact |
| `PATHPLAT1` | CITY_I01, start moved onto a platform (2680,1135) | path platforms (OBJECTS type 100, kind 0, `platobj::update_path` = FUN_1060_0819): mode 4 loops round the rectangle carrying the hero, jumps on it, steps off | exact |
| `WEIGHT1` | CITY_I03, start moved to (1960,881) | the falling weight (icon 61000, kind 0x13a6, FUN_1068_215e, `weight.rs`): drops on the hero (hit kind 2, knocked flat), breaks the plank bridge (effect 50, nested FUN_1088_1364 negative-tile blasts), he falls through (any state < 1 but -1..-5 falls) | exact |
| `PUMPDOOR1` | CITY_I04, start moved to (380,863) | the pumping door 0x14b4 (FUN_1070_a0bd, `Level::door_pump` = FUN_10a8_11e9); no Random involved, only the hero's walk through it checks it | exact |
| `SLAB1` | GRANNY, start moved to (1060,975) | the falling sign 0x14a0 (FUN_1068_1f3c, `Level::slab`): walks under it, it drops and knocks him flat (hit kind 2), he climbs on it | exact |
| `TANK1` | F_LAST, start moved to (2060,735), two weapon-3 pick-ups | the tank 0x146e (FUN_1070_b9d3, `tank.rs`): patrols 2144..2511, turns, lobs grenades from its arm (FUN_1040_6187), knocks the hero back | exact |
| `TANK2` | F_LAST, start (2050,735), six weapon-3 pick-ups | the tank shot to pieces: arm off (b7cd, effects 9 and 47 = FUN_1040_4d4c), claw out, turret off (b8d3), gun bursts (state -1), hull: the wreck (plates, fan, burning); also: a group-0 ScreenShake acts before the actors (trees) | exact |
| `SPIKE1` | FOREST05, lever cell (138,91) set on (0x308), start on the branch (2318,1407) | the spiked ball 0x14e6 (FUN_1070_9a66, `spikeball.rs`): swings from the lever, hits the hero standing where it swings up (hit kind 3, bloody 0x3315, drips), two pistol shots push it. Not exercised: shakes pushing it (amp > 7) | exact |
| `MEATHOOK1` | BUTCHERY, lever (158,48) set on (0xf58), start (2575,817) | the meat hooks 0x1414 (FUN_1070_4b9b, `meathook.rs`): speed ramps up, meat shot apart, the knocked-down hero carried up (gHeroDY set after FUN_1088_532c carries into the next HeroUpdate: `Hero::carry`); its draw's Random(2) under a saved seed is IGNOREd (1070:51af) | exact |
| `COLLAPSE1` | GRANNY, lever (146,30) set on (0xfd8) | the collapsing building 0x2715 (FUN_1088_a481 from background proc 15, `collapse.rs`): 0x42a frames of shakes and debris (particle group 2, mover FUN_1050_3d06 = `MOVE_BG_THROW`, callback 0x232e), the rumble loop; the tilted building (extra map plane 4 rotated, FUN_10f0_545b) is not drawn | exact |

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
- Argument order: a callee's `[BP+6]` is the LAST pushed argument. When Ghidra's call or a
  nested-proc decompile looks odd, check the callee's asm (e.g. TileFunctionAt: [BP+6]=y,
  [BP+8]=x, so Ghidra shows `TileFunctionAt(y, x)`). Ghidra was right about door markers; I
  "fixed" it wrongly once — verify against the callee before overriding the decompile.
- Some drawing code calls Random and then restores RandSeed (big alien shield, 1070:8c72):
  the tracer counts those calls; windiff's IGNORE set skips such sites.
- Object boxes: actors update only while their box (+2..+8) meets the camera. FUN_1058_0002
  makes it unbounded (falling, hit, ...); moving platforms do it for actors they touch
  (FUN_1058_0114, `wake_actors`). Unknown kinds print `warning: object kind ... not ported`.
- 465e noise: 1 shake started, 2 hero/big-alien fire, 3 a door started moving (FUN_10a8_0975,
  even in LevelFirstFrame). Aliens hearing it go on guard (-500) with a Random(2).
- Weapons: table 1170:1656 (0xd bytes), pick-ups = plane-2 function 30000 (effect 67), right
  button = weapon proc table 188a (only grenades ported). Exits = function 9000 (state -100),
  teleport records 1000; after a teleport LevelFirstFrame runs a frame without the hero
  (`world_update`). Doors: `doors.rs` (segment 10a8).

## Front end (2026-11-20, re/frontend.md)

`eatthis` without `--map`/`--demo` (from `port/`: `cargo run --release`) runs the original's menus:
`port/src/frontend/` (`mod.rs` state machine + window loop, `menu.rs` the menu objects of segment 1080,
`gfx.rs` drawing/text/tint rows, `texts.rs` ETMENUn.SCR, ETMAIN messages, EatThis.ini). `--map`/`--demo`/
`--headless` still use `run_window`/`run_headless` unchanged.
- Levels start like the menu's 17fa -> 16b7: `load_world` + `Game::new` (RandSeed 0x3e00 + HeroReset) + `level_music`.
  Level end (4e60 = 100) -> stats (1080:172a) -> CONTINUE: NextLevelMap / NEXT EPISODE (episode menu) / main menu.
  A demo (user map with PlayRecord=) that ends goes to the main menu, no stats (as GameFrame does).
- Stats counters: `level::Tally` (thread-local; `Effects` add kills/secrets when dropped, command 0x18 = hidden
  area, heart pick-up = bios sphere); totals from plane 2 + the setup's enemy objects (`frontend::level_totals`).
- Savings (the original's only save): the next level of a finished MAPS/ level is copied to `SAVINGS/<NAME>/`
  (cwd, `ET_SAVINGS`) and listed under START USER MAP. Settings: `EatThis.ini` in the cwd (`ET_INI`).
- Live input (ReadGameInput): the cursor aims (`Game::live_mouse`, x*319/320; moving it sets gHeroShooting), the mouse
  buttons fill LeftShoot/RightShoot when those keys are up, keys 1..9 0 / PgUp / PgDn select weapons
  (`Game::weapon_key`, gWeaponForDigitKey; like the original they select weapons not owned too). Same in `--map`.
- Test without a window: `ET_FE_SHOT="30;2 Return;50;shot a.ppm;levelend;300;shot b.ppm" ./target/release/eatthis`
  (`N keys...` = SDL scancode names held N frames, `move/click X Y`, `levelend`). The intro plays first: start
  scripts with `1 Return;` to skip it; `shot` during a video writes its 640x480 frame.
- Videos (VIDEOS/*.AVI, Indeo 3 + PCM): `frontend/indeo3.rs` (port of FFmpeg's indeo3.c, LGPL), `avi.rs`, `video.rs`;
  intro, level start/end (`Video=X.avi,0/1`), credits, F1 help; skip = Escape/Space/Return. `ET_AVI_YUV` dumps yuv410p
  to compare with ffmpeg (all 27 match). Help pages' GDI texts not drawn.

## Sound (2026-11-20, re/sound.md)

Effects are SOUNDS/*.WAV (ids = ETMAIN.SCR's `LoadWAV=` lines in order), played through DiamondWare
(DWS_DPLAY record at DS:5d7e: lvol/rvol/pitch, 0x100 = unchanged). Music = CD audio tracks:
`python3 -I tools/extract_audio.py "iso/Eat This.cue" work/cd/AUDIO` once (TRACK02..07.WAV; the port reads
`<data>/../AUDIO`); a level's `CDTrack=N,loop` plays CD track N.
- Port: `port/src/sound.rs` = front end (`play_at`, `play_at_pitch`, `set_volume` = 1ab8, `audible`/`audible_box`
  = 0073/016b, `keep(key, id)` = the loop helper 0408, `discard` = 05e0, `stop`), mixer, music, `--wav`.
  Sound calls only READ simulation state; every Random they use was already drawn by the port (`let p =
  rng.random(n); sound::play_at_pitch(id, p + base, y, x)`). Never add or move a Random for sound.
  The listener is set at the end of the camera update (FUN_1088_0c82), not at frame start.
- Off in `--headless` (no events) unless `--wav FILE` (offline render) or `ET_SOUND_TSV=file` (log).
  Window mode opens SDL2 audio (`--mute` to skip). Captures to listen to: `work/audio/CITY01_excerpt.wav`
  (60 s of LEVEL1_2 with Cityviol), `work/audio/GRINDER.wav`.
- Oracle: every `mise run trace` now writes `work/tracer/<DEMO>/sounds.tsv` (each SoundPlay/SoundSetInfo/loop stop
  with lvol/rvol/pitch and the caller). `python3 tools/sounddiff.py DEMO [--all]`: matched/missing/extra plays and
  value mismatches; the missing list by caller is the to-do list. State: GRINDER 98/99 exact, LEVEL1_2 1276/1801,
  LEVEL2_1 539/905, 0 extras. Done: grinder, platforms (sets at 1170:0ec4), doors, hero moves/landing/ledges,
  weapons fire + switch, pick-ups and plain effect sounds (`effects::PLAIN_SOUNDS`), effects 75/82/84/87/30/83,
  particle hits (0fcc, 0c4d, 45a3, 0a80, 556e, 1481/188c bounces, 0cf8 sparks), the drone, ambient emitters
  (plain loops, 10000, birds 0x2711, rain/thunder 0x2712, 0x2713), the hero's death cry.
- Left (biggest first in sounddiff): enemies in segment 1070 (1b00, 4051, 30b6, 7eb2, a3cf, 60ca, 90f1, 5911,
  8cd7, 5ce1, 5b08, 02b3, af70, the voices FUN_1070_584e = pitch from obj+0x26), the hero's hurt cry (3c05 from
  other callers), effects 06d4/075a (bullet hits, Random(0xc) gate), 2046/22c1 (napalm), 1050_14bd fire crawlers,
  1060 lifts (152f/15a6), the HUD message blip (FUN_1088_0fa4, 87 when 16d8), vehicle (HeroUpdateVehicle),
  START's background (FUN_1090_0292). The pitch direction (higher = faster) is assumed, see re/sound.md.

## Renderer (2026-11-20)

`port/src/render.rs` (sprite bank by slot, 10f0:2670's draw modes 1ae0, tables) and `port/src/draw.rs`
(the frame's draw order and each object kind's draw tail); main.rs `Graphics::draw` runs it after the
update, so it never touches the simulation. Check with `--shot FRAME FILE.ppm` against
`uv run python tools/dumpframe.py DEMO FRAME` (the original's frame; frame numbers line up).
- Draw modes (1ae0) come from the run handlers at 1170:1b8a; mode 0 only paints over colours <= 1ae2
  (ForegroundColors), which is how foreground tiles hide sprites. MIX tables: PalMix (`.MIX`, 256 rows),
  PalMixLight (`.LIT`, 129 rows: light levels), PalMixSea (`.SEA`, 80 rows: the water), and the tint table
  4c26 COMPUTED from the palette (`LevelPalette::tint_table`: row 0 red = hurt hero, 10 = shield).
- Groups loaded by FUN_1088_4518 (PPGLIGHT 0x1b, FACBROCK 0x73, SVN 0x75), SEATILE 0x7c and script groups
  flagged `,N` are NOT fitted to the palette: their pixels index the mix tables (`sprites::raw`).
- Long divisions of 617e in draw code (`lcall` with cx = divisor) are frame/N animations: disassemble
  (capstone: segment for selector S is NE segment (S-0x1000)/8+1) rather than guess.
- FUN_10f0_302b scales with source STEPS (8.8, 0x100 = 1:1, 0x200 = half size); FUN_1040_6315/6720 wrap it.
- Updates that compute their sprites in locals record them in `Obj::looks` (walker, gunship, boss, turret,
  tree); nothing in the simulation reads it.
- Extra maps (`LoadExtraMap=`, slots 8..) keep only PLANE 0 and use the main map's tile groups; 0x272e
  records are parallax decor layers (depth < 0x100 behind the tiles, else over everything).
- Background procs (`BackgroundProc=N` -> table 1170:189e, 17 entries; `ForegroundProc` -> 1170:18e2):
  0 = 1090:075c (decor only, yet CITY01 shows the city: the port draws `bg::city` for it), 1 = 0a8c and
  16 = 0ace (START: the city 0797, the bridge 0292 with its train when the sector's +0x1c >= 0 (OBJECTS
  0x2719), the rain 0418 while the storm 4eda runs, decor; 16 also the truck), 3 = 147b, 4 = 1629 (+ extra
  maps 9 and 8 at camera/4 and /2, `FUN_1088_a401` = repeated both ways), 5 = 178e (FORPLANE), 6 = 1a06.
  Not ported: 12..15 (BUTCHERY/CITY_NET/RADIO_AC/STONES, CITY_I06, FALL, GRANNY; no demo shows them).
- START's storm: script command 0x16 darkens the palette once (FUN_1088_293a: each colour 3/4 of the way to
  its grey; `Graphics::palette_of`), rain drops come from a table drawn at boot under RandSeed 1000
  (`storm::rain_drops`, 4f0c = 4eda / 32 of them behind the tiles, the same count from drop 0x1e in front
  (FUN_1090_0567, which also spawns splash particles 0x615)), lightning kind 1 lights the sky (PalMixLight
  row 0x80), kind 0 tints the screen (tint row 3) three frames in six.
- HUD (FUN_1088_4b64 -> 2501): GameStats (1a92) 1 = status bar, 2 = + the kill/hidden-area/bios counters
  (FUN_1088_248e: 4eb0/4eb2 kills at the bottom, 4eac/4eae, 4ea8/4eaa; icons STATSOBJ 0x2800 + f/6 % 12,
  0x280c + f/9 % 12, 0x2824 + f/3 & 7). Totals: `Game::totals` (`frontend::level_totals` + COUNTED_KINDS +
  alien icons over wall holes). The bar follows health a pixel a frame (4e30), the weapon icon slides in
  after a change (4e32 = -14, -16 in the vehicle); 2501 also CLAMPS health to 0..0x2c6 (`Hero::status_bar`).
  HeroReset selects gWeaponForDigitKey[0] (weapon 2). The pointer (4e50, `Particles::pointer`): enemy hit
  tests (mask & 0x59) whose box holds the pointer (4e42/4e44 = mouse + camera, set after the camera update)
  make it red (0x7802+weapon), the alien's corpse test KICK/ME (0x7803/04, 163e = 3).
- Message box (`messages.rs`, 4f10/5110/5112/5114): HeroDamage(id) (1088:1109) = message id of ETMAIN's
  AddMsg lines (negative, -1 the last) or, from 0, of the level script's (script command 0x0b); sources:
  weapon/keycard/drone pick-ups, death (-14 - Random(6)), the vehicle (-12 under 0x8f health, -13), key slots
  (-20, "needs a keycard" only if not the last line). Typed a byte a frame in PPFONT3 (slot 0x20) through the
  tint table over rows darkened by PalMixLight 0x20/0x24; markup ^| ^Cxx ^O ^Bxx. Pixel-exact on CITY01.
- HeroClothes= in a script: the hero is PPHEROA/A2/A3 and slot 3 PPDOORA (`sprites::set_clothes`).
- Still different: FUN_1088_2187 is the automap (1a94, toggled by a typed cheat), not drawn; the
  little movie screen on START's skyline (4d9c, slot 0x7d MOVIE); GRANNY's tilting building (collapse.rs
  simulates it, FUN_10f0_545b draws it); the HUD message blip's sound; platform objects of unported kinds sit at their start;
  some kills totals are off by one (forest: other EnemyFactory counting); a few actor frames (FORPLANE's
  pumpkin turret, WEAPON9's mines' lights) and the flamethrower's burning ground in START differ.
  Debug: `ET_DRAWLOG=1` lists every sprite drawn, `ET_DRAW=1` the actor kinds without a draw, `ET_DECOR=1`,
  `ET_MSG=1` the messages shown, `ET_HEALTH=1` the health each frame.

## State (2026-11-20: + WEAPON6/WEAPON9, next level; all oracle demos exact via tools/checkall.sh)

GRINDER 113/113, CITY01 (LEVEL1_2) 847/847, FOREST04 (LEVEL2_1) 616/616, FOREST03 (LEVEL2_2) 770/770,
FORPLANE (LEVEL3_1) 352/352, START (LEVEL1_1) 679/679 and HYDRO_02 (LEVEL3_2) 173/173 windows, and for these seven `python3 tools/valdiff.py <DEMO>` (after a full
`mise run trace -- <demo>`) reports 0 value mismatches over EVERY Random() call. ALL SEVEN DEMOS are exact (every window, every Random() result via `tools/valdiff.py`, and the hero's x/y/state
frame by frame via the tracer's `--watch gHeroX,gHeroY,gHeroState`): GRINDER, CITY01, FOREST04, FOREST03, FORPLANE,
START (launch, ship platform, steam emitter 0x141e, storm in `storm.rs`, lift dust in the platform objects'
second pass) and HYDRO_02 (the vehicle: `Hero::update_vehicle` = HeroUpdateVehicle, OBJECTS 0x271a starts it,
0x2718/0x2719 give a sector its water line (`Level::water` = 4ec8, waves in `storm.rs::water_frame`), bubbles =
callback 0x30f, the torpedo = `Particles::torpedo` + callback 0xf9f, the sea plants `plant.rs`).

LESSONS (the expensive ones):
- A matching RandSeed per window proves only the call COUNT (the LCG step ignores the range). Two calls swapped
  inside a window, or a setup that draws the right number of values for the wrong objects, leave all windows green
  and wrong. `tools/valdiff.py` compares ranges AND results; run it whenever a demo is "exact".
- `CHECK[0] (after level setup): differs` means the setup Randoms are wrong; the port then ADOPTS the demo's seed
  and everything downstream looks fine until a value (an alien's hit points) matters. Fix the setup first
  (`ET_ICON=1` lists the icons in creation order; the frame-0 calls of calls.tsv are the setup). Icons that
  FUN_1068 constructors consume (the carrier 0x14be eats the aliens/bots to its right) must be cleared in order.
- Tile cells: `cell >> 8` is the group number and goes beyond 15 (LEVEL3_1 has 17 groups); never mask it.
- The tracer labels hero calls with the port's frame and object calls one frame earlier; `ET_RNG_TSV` + valdiff
  sidestep labels. LevelFirstFrame also runs a world update without the hero (Game::new, like `teleport`).
- HeroTouchTiles (sensitive levers) is EARLY in HeroUpdate: its scripts/spawns draw Randoms before the hero's idle
  Random(8) at the end (main.rs runs it before `hero.update`). Other level events recorded by `Hero::update`
  (levers, pushes, pick-ups) are still processed after it: if a Random order swap shows up around those, split
  the update there too.
- Hero walk limits (4e16/4e18, springboards, bots) apply AFTER the platform objects, not at the top; spring parity
  is `(frame & 1)`; weapons 2/8 also add to the shot counter (4e6a, it feeds the flame's wobble) and set the
  4e56 pause of 10; weapon 1 = flame thrower (FUN_1088_678b, callback 0x4ff, death effect 37 = smoke when 617e is even).
- Under a saved RandSeed (drawing code, the plants' burst) use `PascalRandom::scoped(seed, ..)`: it restores the seed AND the
  call counter the CHECK windows compare. Draw-time Randoms the port doesn't make are skipped by the IGNORE sites in
  windiff.py/valdiff.py (1070:8c72, 34db, 091f, 1068:3112).
- Platform draw routines live in a table at 1170:09ce (filled at run time, not in the EXE's data): rows 0 and 3 (draw
  1060:1cee) shed dust from lifts.
- Ghidra's `FUN_x(a,b,c,d)` call text lists the parameters in [BP+6].. order = REVERSED Pascal order.

Added after the CITY01 pass (all from the FOREST04 loop): ambient sound emitters (`ambient.rs`: audibility of
FUN_10e0_016b needs the listener = camera centre; the first deadlines 4ec0/4ebc are the boot-seed values 509/329, the
hero's idle values -20/590 too); sprite groups by slot from the script's `LoadSpriteGroup=` lines (slots 0x32..) plus
the fixed slots in sprites.rs; walking gunner 0x14d2 (`turret.rs`); platform objects 100..103 / lifts 0x14c8
(`platobj.rs`, run from inside HeroUpdate before the hero moves, like FUN_1060_145a); script commands fill_area (0x01)
and copy_area (0x0e); levers with key slots (tile param 2 flag 4: `Hero::keys`, `script_cells`, `cell_steps`);
weapons 2 (rapid gun, effect 81), 7 (napalm shell, effects 44/18, fire crawlers FUN_1050_14bd), 8 (three homing
rockets, callback 0x7bf, effect 57); the switch-weapon key (input 8); bonus drops (`Level::bonus` = FUN_1040_6b03,
callback 0x2588, effect 66: they fly, the hero can catch them, else they land as plane-2 tiles); pipes (states -1
hanging from 60800, -2 climbing out of 60600); red gibs FUN_1068_186c for barrels.

Ported since the 610 mark: ladders (HeroUpdate state -5: detection loop before the exits, grab before the
Down-crouch, the -5 block before the levers); the hero's hit boxes are ALWAYS mirrored (the rec word is gHeroFacing
= +-1 and FUN_10f0_2240 only tests != 0; mirrored rects swap their edges); platform camera lock (script command 0x14 =
platform w20: 4e36 = (w20 + 0x7d) - platform.y + camY each frame while it is in the hero's sector); script commands
that spawn enemies (command 0 with an object tile, command 0x10 = 0d5a at the row above the camera;
`Effects.spawns` + `Level::flush_spawns`); decals (`decals.rs`: blood drops 0xbb0, shell casings 0xbc4, wall splats
0x976 in sector list L5A, updated by FUN_1070_fb30 BEFORE particle group 0; eviction at > 0x1f; FUN_1058_016f takes
byte 0 of the MAIN-plane cell looked up first); the walker with a gunner (0x140a, `walker.rs`); the hovering machine
(0x13f6) and its wreck (VMT 0xcdc, move hook FUN_1070_36e7) in `boss.rs`; effects 7, 9, 28, 29; crouch-slide down
slopes (4e20 = `down_timer`, 4e24 = `sliding`, 4f03 = `released[DOWN]`, FUN_1088_29c8 = `slope_run`) and the
knocked-down hero sliding too; `Env.hero_obj` lets objects hurt the hero on the spot (`Hero::hurt`),
`Effects.hero_stand` = FUN_1070_00b6 (hero carried on a moving top, gHeroGround 0x16).
Facts: gHeroDX/DY set by objects after HeroUpdate carry into the next one (GameFrameInGame runs FUN_1088_532c = DX 4e06, DY 0 right AFTER HeroUpdate; `Hero::carry`, used by the meat hooks and the tank; older objects may still drop them); the 1168:1381 set test at
DS:160a is "-state in 1..5" (hero hanging/climbing ignores platforms); Pascal calls push left to right, so Ghidra's
param_1 is the LAST source argument (3de2(sprite, steps, frames, y, x, animate)); a long divisor with CX=0, BX=8 is
0x80000 (BX is the high word). Particle spawn globals that a caller forgets to reset stay set (FUN_1058_0301 leaves
091c = its last Random(4)).
Debug aids added: `ET_CAM`, `ET_AIM`, `ET_CMD`, `ET_FACT`, `ET_SPLAT`, `ET_BOSS`. Tracer `--entry SEL:OFF` works for
plain functions (`--entry 1050:5c4a` finds who shakes the screen) but hooking EnemyFactory aborts the run;
`--particles` tsv frame N ~ port ET_PART frame N-1. windiff IGNOREs the draw-time Randoms under a saved seed
(1070:8c72, 1070:34db). Earlier aids: `ET_RNG=<frame-1>`, `ET_RNG_TSV` + `tools/rngdiff.py`, `ET_DRONE` +
`tools/dronediff.py`, `ET_PART=F` (+`ET_ALLPART=1`), `ET_HERO=F`, `ET_OBJ=1`, `ET_DEBUG=1`, `ET_HIT=1`, `ET_DOOR=1`,
`ET_AREA=1`, `ET_CORPSE=1`, `ET_SPARK=1`.

Done since (2026-11-20, scenarios): weapon 3 (WEAPON3), the level end through a leaving teleport record and
the restart key while dead (EXIT1, DEATH2: `Hero::level_end` = 4e60; a demo stops, window mode reloads the map
on 10, stops on 100; the end-of-level video/stats screen 1080:172a and the menu's next level are front end).

Done 2026-11-20 (second pass): weapons 6 and 9 (WEAPON6/WEAPON9), the next level after a level end, effects 27
(FUN_1040_2f98), 53/54 (tile steps), 93 (FUN_1040_341a, `Level::tries` = 4dc2), the wall's effect-23 flash, hero
hit kinds 2/3 in `Hero::hit` (kind 2 tested by WEIGHT1; their callers FUN_1068_1f3c, FUN_1068_215e (kind 2), FUN_1070_9a66,
FUN_1070_b9d3 (kind 3, b9d3 also kind 0) are objects not ported yet; port one with a scenario to test them).

Still unported (warnings in the port run): see `re/unported.md` (census of the three episodes' maps and the
priority list: the tank 0x146e of CITY_I00/F_LAST, the spiked ball 0x14e6, 0x1414, GRANNY's 0x2715). Ported on
2026-11-21 (branch claude/gameplay-objects-9a00ca): granny 0x1482, carrier 0x14be, gun turret 0x13ec, path
platforms (type 100), falling weights (icons 61000/61001) and sign 0x14a0, pumping door 0x14b4, water splashes,
effects 7/23/50/65; `ET_KINDS=1` lists every object the factory makes.
Next: nothing is left in the recorded demos; write scenarios (tools/scenarios) for the unported features. The game is not complete: see the unported items below (grep the
port run for `warning:`), and the SDL front end/menus/sound. and run the daily loop on it.
