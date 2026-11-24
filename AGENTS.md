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
- dumpframe.py takes `--overlay` (work/demos by default for demos there). The tracer's `--particles A-B` works
  (GRINDER just has no particles in its first ~300 frames); a row's frame N = port `ET_PART` frame N-1.

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
| `FREEZE1` | GRANNY, weapon-1 and weapon-5 pick-ups at (18,30), (19,30) | weapon 5's rockets (hit mask 0x400) freeze grannies (FUN_1070_564d, `deaths.rs`) into statues (state -0xd2, FUN_1068_0f32: wobble, tint row 2) that the pistol shatters (shards land as effect 48) | exact |
| `BURN1` | GRANNY, weapon-1 pick-up at (19,30) | the flamethrower (mask 8) burns grannies (FUN_1070_544e: ash 0x5132, burning bits 0x5130 landing as effect 59 = FUN_1040_5ec6, smoke); the same deaths are wired for aliens and big aliens | exact |
| `BG_STONES` `BG_CITY_I06` `BG_FALL` `BG_CITY03` | those maps, walking right | background procs 12, 13, 14 and (switched by command 0x15) 15 | exact |
| `SLIP1` | CITY_I04, start on the dirt pile (1020,790) | slippery slopes (slope row 30, flags 0xe: HeroUpdate row flag 2): he slides down both sides, dust every 16 frames one time in four (flag 8) | exact |
| `HARM1` | CITY_LIB, start above the strip at (632..728, 1600) | tile function 8000 (FUN_1088_55bd from 55f0, main plane and platforms): FUN_1088_3c32(100), sound 0x16, sideways shake | exact |
| `SPLASH1` | CITY02, grenade pick-up on the start | particle splashes (FUN_1050_2bcf) through sector 2's water line; thrown grenades/shells/charges keep their water line (FUN_1050_44ae) | exact |
| `SWINGLEVER1/2` | STONES, start on the platform at (720,110); 2: lever (52,20) set on (0xf58) | swinging platforms obey their lever (FUN_1060_0819 +0x2c/+0x26): off it hangs still under him, on it swings him round | exact |
| `VEHICLE1` | LEVEL3_2 to frame 3600, then down-left | the vehicle destroyed by sea-plant bursts (FUN_1088_4a81 -> 4a34: bubbles, pieces FUN_1088_46e9, blast) | exact |
| `TANKGRAB1` | TANK2's first 740 frames, then walk up | the tank's claw holds the hero (state -3) until he dies | exact |
| `PRESS1` | CITY_I03, start left of the press (2850,897) | the press (icon 61001, weight type 1): drop, landing bits, pumping | exact |
| `CHUNKS1` | CITY01, weapon-3 pick-up at (1360,1231) | wall chunks (effect 43, FUN_1040_45db) from rockets | exact |
| `SHOTLEVER1` | CITYHS, start right of the posts at (5..8,70) | shot levers (FUN_1088_14a3) on explodable posts: effect 90 (FUN_1040_3471) | exact |
| `FX1`..`FX15` | CITY_I01, FOREST04, FOREST02, CITY02, GRANNY, CITYRAIN, CITYHS, CITY_I02, CITY_I06 (start moved, weapon-3 pick-ups copied) | tile effects of explodable tiles: 31/32 (FX1), 49 (FX2), 68 (FX4), 79 (FX5), 80 (FX6: a steam emitter made at once by EnemyFactory), 45 (FX7), 51 (FX9), 33 (FX10: the sinking block, drifting smoke mover 0x3acd, wind OBJECTS 0x2717), 35 (FX11), 40 (FX12/FX13: CITYHS's gate; its script starts an accelerating platform that squeezes a granny: FUN_10a0_023f runs before the platforms), 24/25 (FX14/FX15: the wrecks, burning wreck 0x1400 `burnwreck.rs`, spawn effect 26) | exact |

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
  to compare with ffmpeg (all 27 match). Help pages' texts (FUN_1008_1073): `HelpTexts` in video.rs,
  `helpfont.rs` made by tools/helpfont.py.

## Extras (2026-11-24, docs/extras.md)

Not the original's; all off by default and outside the simulation (the exception: `Game::wide_actors`, 0 in demos).
Screen code must use `render::at/on/left/right/screen()` instead of 0..320 and `y * W + x` (the view may be wider), and
sprites that give off light belong in `render::emission`. New state in `Game` must stay Clone and deterministic:
`./target/release/eatthis --check-states --demo X.DMO` saves, loads and rewinds all along a demo.

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
  value mismatches (only fields the original's flags set; same-frame plays of one id pair by value); the missing
  list by caller is the to-do list; port plays in a demo's last frame (the tracer stops inside it) aren't
  counted. State (2026-11-23): all 55 oracle demos play exactly what the original plays (every play
  matched, no extra, the same volumes and pitch). The last mismatch (LEVEL1_2 frame 14720, a far ricochet) was the
  mouse: FUN_1088_4340 clamps gMouseX/Y to 0..319/0..199 after the look-around, so a demo mouse at x 320 aims like
  319 (bullet angle 308, not 306). Every enemy, object, effect, the vehicle, lifts, hurt
  cries, the message blip, START's train and rain are wired; a sound call sits where the original's is, under
  the same conditions, with the Random() the port already drew.
- Lessons from closing the last differences: volumes off by 1-5 mean the listener (camera) of another moment:
  FUN_10a8_1311 (doors opening by themselves) runs BEFORE the camera update; shakes of scripts
  run before it (a landed platform's, levers', pushes', touched tiles') move this frame's camera (`Game::apply_shakes`); object hits on the hero shake the
  screen at once (`hero.shakes` from an object are applied after the actors, not lost); `--watch gCameraX,gCameraY`
  in the tracer shows the original's camera (watch row N+1 = after frame N). 1ab8 is global state: FUN_1040_34e3
  leaves it at 0x100 - Random(0x40), the next emitter (315a) resets it. Big aliens made during play (0f2a = 2,
  `Level::in_play`) growl at once. Scripts' comments are whole lines starting with ' (AddMsg=IT'S UP MATE!).
  Not in any demo, so unchecked: the frozen/burnt aliens' sounds, other unported objects. `ET_SNDPOS=1` prints
  every positional play with the listener.
  Sound helpers for objects: `sound::play_at`/`play_at_pitch`/`play_at_vol`, loops `audible` + `priority(0x11f4)`
  + `keep(key::obj(id, handle_offset), id)`, `looping(key)` for code that tests a handle (the carrier),
  `stop_loops` = 03d5 (LevelFirstFrame). The pitch direction (higher = faster) is assumed, see re/sound.md.

## Renderer (2026-11-20)

`port/src/render.rs` (sprite bank by slot, 10f0:2670's draw modes 1ae0, tables) and `port/src/draw.rs`
(the frame's draw order and each object kind's draw tail); main.rs `Graphics::draw` runs it after the
update, so it never touches the simulation. Check with `--shot FRAME FILE.ppm` against
`uv run python tools/dumpframe.py DEMO FRAME` (the original's frame; frame numbers line up).
- Draw modes (1ae0) come from the run handlers at 1170:1b8a; mode 0 only paints over colours <= 1ae2
  (ForegroundColors), which is how foreground tiles hide sprites. MIX tables: PalMix (`.MIX`, 256 rows),
  PalMixLight (`.LIT`, 129 rows: light levels), PalMixSea (`.SEA`, 80 rows: the water), and the tint table
  4c26 COMPUTED from the palette (`LevelPalette::tint_table`: row 0 red = hurt hero, 10 = shield).
- Groups loaded by FUN_1088_4518 (PPGLIGHT 0x1b, FACBROCK 0x73, MOVIE 0x7d), SEATILE 0x7c and script groups
  flagged `,N` are NOT fitted to the palette: their pixels index the mix tables (`sprites::raw`). SVN 0x75 is
  raw only at boot (FUN_1088_d073); every level reloads it fitted (FUN_1088_c98f -> 458b).
- Long divisions of 617e in draw code (`lcall` with cx = divisor) are frame/N animations: disassemble
  (capstone: segment for selector S is NE segment (S-0x1000)/8+1) rather than guess.
- FUN_10f0_302b scales with source STEPS (8.8, 0x100 = 1:1, 0x200 = half size); FUN_1040_6315/6720 wrap it.
- Updates that compute their sprites in locals record them in `Obj::looks` (walker, gunship, boss, turret,
  tree); nothing in the simulation reads it.
- Extra maps (`LoadExtraMap=`, slots 8..) keep only PLANE 0 and use the main map's tile groups; 0x272e
  records are parallax decor layers (depth < 0x100 behind the tiles, else over everything).
- Background procs (`BackgroundProc=N` -> table 1170:189e, 17 entries; `ForegroundProc` -> 1170:18e2):
  script command 0x15 switches both until the hero changes sector (FUN_1088_1280 restores 4c2a..4c30;
  `Level::procs`, only the renderer follows it: the procs' simulation parts, storm/collapse/rain, are not
  switched). Used a lot: CITY01's start sector switches to 1 on frame 0 (that is why it shows the city),
  indoor areas to 0, 7, 9. 0 = 1090:075c (decor only: no backdrop), 1 = 0a8c and
  16 = 0ace (START: the city 0797, the bridge 0292 with its train when the sector's +0x1c >= 0 (OBJECTS
  0x2719), the rain 0418 while the storm 4eda runs, decor; 16 also the truck), 3 = 147b, 4 = 1629 (+ extra
  maps 9 and 8 at camera/4 and /2, `FUN_1088_a401` = repeated both ways), 5 = 178e (FORPLANE), 6 = 1a06.
  7 = 1d4f (0x3300 tiled at camera/2), 8 = 11ce (city + 0x3300), 9 = 16e8 (0x3800 tiled at camera/2),
  10 = 1742 (forest above the line 0x290 - camY, 9 below), 11 = 1219 (city + rain), 12 = 0dd0 (sky above
  y2, the 14-building skyline table 1170:18fe at camera/10, its building 0x3214 topples after script command
  0x1b (4ef6, counted up by the draw: `Level::skyline_fall`), tree line 0x3200 at camera/6), 13 = 0bbe and 15 =
  1f6f/1d8f (sky, 13 a window frame over colour 0xa8, the picture's rows faster with each row, skyline +
  reflection; 15 also the bridge, rain and GRANNY's building), 14 = 1026 (FALL: a 0x20..0x11f view). All in
  `Graphics::svn_background`; checked pixel-exact on BG_STONES/BG_FALL/BG_CITY_I06/BG_CITY03/MEATHOOK1/GRANNY1.
  2 (123e) is never selected by any map. Scenarios BG_* (tools/scenarios) just walk those levels.
- The city's movie screen (end of 0797): MOVIE.SPR (slot 0x7d, raw, loaded for every level by c98f), frame
  617e/4 % count at (0xf9 - horizon, 0x68). GRANNY's building: LevelLoad loads SVEN2BAC.MAP into planes 4/5
  when OBJECTS 0x2715 exists; FUN_1088_a481 draws plane 4 upright (513c) or plane 5 turned (FUN_10f0_545b:
  16.16 steps, tile group 8 only, the cell's low byte) in the half-speed layer (`Graphics::building`,
  tools/scenarios/draw/COLLAPSE2.txt; the port's collapse t equals the original's 4ece at draw time). That
  scenario is NOT an oracle: a481 runs from the background proc, which the original's frame skipping can leave
  out, and the rotated draw is slow under the emulator, so its recordings differ run to run; COLLAPSE1 (camera
  far away) is not affected.
- Automap (FUN_1088_2187, 1a94): typed ETSCAN (`Game::type_key`: FUN_1088_2845 keeps the last 40 letters,
  FUN_1088_28c9 finds "ET"+word: ALIEN MOVIE STUFF DRONE BOOM RESTART WIN MAPS SCAN; only SCAN ported).
  `draw::automap`; colours 1b14..1b23 = nearest of the 16 VGA colours among 1..=ForegroundColors (FUN_10f0_183d
  reserves the rest; the port's older `Graphics::black` still searches all 256: 1b14 is 1 in GRANNY, not 0;
  check the FORPLANE quad and the falling gunship's darkening, which assume 0). Pixel-exact with
  `dumpframe.py --poke 1a94=1` vs `ET_AUTOMAP=1`.
- START's storm: script command 0x16 darkens the palette once (FUN_1088_293a: each colour 3/4 of the way to
  its grey; `Graphics::palette_of`), rain drops come from a table drawn at boot under RandSeed 1000
  (`storm::rain_drops`, 4f0c = 4eda / 32 of them behind the tiles, the same count from drop 0x1e in front
  (FUN_1090_0567, which also spawns splash particles 0x615)), lightning kind 1 lights the sky (PalMixLight
  row 0x80), kind 0 tints the screen (tint row 3) three frames in six.
- HUD (FUN_1088_4b64 -> 2501): GameStats (1a92) 1 = status bar, 2 = + the kill/hidden-area/bios counters
  (FUN_1088_248e: 4eb0/4eb2 kills at the bottom, 4eac/4eae, 4ea8/4eaa; icons STATSOBJ 0x2800 + f/6 % 12,
  0x280c + f/9 % 12, 0x2824 + f/3 & 7). Totals: `Game::totals` (`frontend::level_totals`: plane-2 icons and
  alien icons over wall holes, + `Level::enemies_made`: EnemyFactory counts 10 kinds while 0f2a = 1, i.e. only
  during the level setup (FUN_1088_d4b2), the alien only from a record of < 7 words, the carrier its riders).
  Checked against 4eb2 (`dumpframe.py --peek 4eb2:2`, port `ET_TOTALS=1`) on 9 levels. The bar follows health a pixel a frame (4e30), the weapon icon slides in
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
- Fixed 2026-11-22: burning ground FUN_1050_1b90 (flame pulsing with |sin|), weapon 9's balls' lights
  (FUN_1050_16cc), bios spheres / pick-up 16 drawn 45be/2 higher during a vertical shake.
- Fixed 2026-11-23 (branch claude/render-gaps2, all scenario demos compared with dumpframe): water tint
  (rise frame >> 2, drift frame >> +0x1a: 1168:1083 is a SHIFT, 109c shl), short 0x2718 records read past their end,
  LevelFirstFrame runs the background proc (collapse) and the HUD update, particles drawn on the frame they end,
  doors and the falling sign drawn with their state before the update, tile draw kinds (513c: 2 opaque, 6 MIX,
  8 LIGHT), mode 0x20 = opaque span copy, 302b top = ((hot_y-1)*256)/ystep+1, half-size 2d8e, 1b1b/1b14 = VGA
  colours 7/0, crates from table 0e4a (13 kinds), grinder teeth, grenades, critters, splashes, robot muzzle light,
  barrel flame/fire, wall regrow frames, door kind 3 shutter, swing chain first link, hidden areas of sensitive
  levers. `--shot F1,F2 file_%d.ppm`, `dumpframe.py --peek @PTR+ADD:LEN`, `ET_TILEAT=x,y`, `ET_PARTDRAW=1`.
- Fixed 2026-11-23 (second pass): small alien's turn flip (state -0x14), particle callbacks that draw through
  10f0:2670 directly ignore the record's flips (1b54 is the flip table), negative particle sprites = animation rows
  of 1170:0918 (FUN_1040_6bd4, evaluated with 617e = age; the tank's fireballs), sprayed bits 26c8 (VGA pixels),
  bubbles' wobble, platforms clipped to w21..w24 (FUN_1078_0968), door kind 7's lid 0x26, START's rain layers
  (0x1e.. at 617e*4, 0x3c.. at 617e*5; the splash test uses the *4 layer), smoke 2192 half size, sinking blocks 1e16
  (`draw::tile_at` = FUN_10f0_445a). `--shot` frames, the Painter's `vga` array, `ET_DOORDRAW`/`ET_DOORNOW`.
- Fixed 2026-11-23 (third pass, branch claude/render-gaps3): particles are drawn only if 4ca5 drew them this frame
  (`Particle::drawn`: not one spawned after its group's update, nor one that hit something), bounces draw where they
  hit (`draw_now`/`draw_bounce`/`draw_rolling`: FUN_1050_4acb and the death effects' own draws); 1168:1083 of 617e is
  `shr`, not a division (`draw::shr`: pick-ups, mines, drone light, sentry, small robot); script commands 0x0c/0x0f
  (platform rectangle), 0x11, 0x17, 0x19, 0x1a (0x12, the music volume, is still skipped); SpawnDust's leaves are
  real particles; walking into exits: the hero drawn by 0dab (`Hero::door_frame`), the screen's pixelation
  (`Hero::fade`, 1170:536a, FUN_1080_007c = `draw::pixelate`), aliens lit behind opening doors; the line routine
  03da, half-size sprites at the screen's sides, rotating chains 1e88/2079, the movie screen's wrapped copy (half
  the film ahead), carrier light/flames; a death by an actor's hit spawns its gibs at once; a landed platform's
  scripts shake at once.
- `python3 tools/drawdiff.py DEMO FRAME` = the sprites the original draws in a frame (dumpframe `--draws`, cached in
  work/frames/<DEMO>/frame<N>.draws) against the port's ET_DRAWLOG: the quickest way to name a pixel difference.
  It lists the sky (bg.rs) and platform/tile draws as one-sided; pixels, lines and polygons aren't sprites.
  Also from a sweep of the CD demos (every ~300 frames): the hero's health (4e6c) is now the original's frame by frame in
  all seven demos (FUN_1050_4b17 divides by the WRONG edge when the target is left of the blast: more than full damage);
  objects on a slope keep `gLastTileCell` (the y-1 cell when y+1 isn't one); messages split at the FIRST comma with a bad
  language = 0; a teleport runs an extra HUD update; foreground procs (1170:18e2) by number, 6 = girders (1090:1d2a);
  beams 465a (`Painter::reds`: hovering machine, sentry, big alien's shield, the last under the running RandSeed);
  running blood splats; landing dust; FUN_10d0_01e4's polygon filler; 302b mirrored.
- State: every sampled frame is pixel-exact: all scenarios (5 frames each) and ~790 frames of the seven CD demos, but for
  one pixel of a flame (LEVEL2_1 11467 and 11778, at (186, 154..159)). Not drawn/ported: script command 0x12 (music
  volume), the typed cheats but SCAN, the MOVIE cheat's movie2.spr; GRANNY's building (COLLAPSE2) isn't an oracle.
  Debug: `ET_DRAWLOG=1` lists every sprite drawn, `ET_DRAW=1` the actor kinds without a draw, `ET_DECOR=1`,
  `ET_MSG=1` the messages shown, `ET_HEALTH=1` the health each frame, `ET_NOPROCS=1` ignores command 0x15,
  `ET_AUTOMAP=1`, `ET_TOTALS=1`; `--bg-only` stops where dumpframe's .bg does (first FUN_1068_09bf), but the
  .bg dump does not line up with the port's yet (compare full frames).

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

Ported on 2026-11-21 (branch claude/objects-rest-c3eb40): the tank 0x146e, the spiked ball 0x14e6, the meat
hooks 0x1414, GRANNY's collapse 0x2715, FORPLANE's quad 0x2716, burnt/frozen deaths and the frozen statue,
platform shake/dust, wall chunks, shot levers (scenarios TANK1/2, SPIKE1, MEATHOOK1, COLLAPSE1, BURN1, FREEZE1).
Still unported (warnings in the port run): see `re/unported.md` (census of the three episodes' maps and the
priority list: the tank 0x146e of CITY_I00/F_LAST, the spiked ball 0x14e6, 0x1414, GRANNY's 0x2715). Ported on
2026-11-21 (branch claude/gameplay-objects-9a00ca): granny 0x1482, carrier 0x14be, gun turret 0x13ec, path
platforms (type 100), falling weights (icons 61000/61001) and sign 0x14a0, pumping door 0x14b4, water splashes,
effects 7/23/50/65; `ET_KINDS=1` lists every object the factory makes.
Next: nothing is left in the recorded demos; write scenarios (tools/scenarios) for the unported features. The game is not complete: see the unported items below (grep the
port run for `warning:`), and the SDL front end/menus/sound. and run the daily loop on it.
