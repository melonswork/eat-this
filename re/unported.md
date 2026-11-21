# Unported gameplay features (census 2026-11-21)

Episode chains (ETMENU NEW GAME → each level script's `NextLevelMap=`, 8.3-truncated):

1. START CITY01 CITY03 CITYRAIN BUTCHERY GRANNY CITY_I04 CITY_LIB CITY_I06 HYDRO_01 FOREST02 → NEXT EPISODE
2. FOREST04 FOREST03 FOREST05 STONES CITY02 CITYHS BOMS02 CITY_I02 RADIO_AC FALL CYBORG → NEXT EPISODE
3. FORPLANE HYDRO_02 CITY_I01 CITY_ROL CITY_I03 CITY_I05 CITY_I00 CITY_I07 CITY_I08 BROYEUR F_JUMP F_LAST

How: load-time warnings of `./target/release/eatthis --headless --map ../work/cd/BIN/MAPS/X.MAP`, and
`ET_KINDS=1` (every object the factory creates, kind + VMT) checked against `Level::update_object`.
VMT method table (DS:VMT, see `mem_1170_0900.bin`): +0xc = update, +0x10 = hit.

## Priority list (feature → episode maps)

| # | feature | original | episode maps | notes |
|---|---|---|---|---|
| 1 | ~~OBJECTS 0x1482~~ the granny (VMT 0xe0c, update FUN_1070_cb38) | EnemyFactory | GRANNY 45, CITYHS 7, BUTCHERY 4, CITY02 1 | DONE (`granny.rs`, GRANNY1); her burnt/frozen deaths and frozen corpse (-0xd2) still warn |
| 2 | ~~carrier 0x14be~~ (icon 61600, VMT 0xc84, update FUN_1070_0cce, move hook 0b02) | | CITY02, CITY_I00, CITY_I06, FOREST03, FOREST05, STONES | DONE (`carrier.rs`, CARRIER1: flight, drops, shot down, crash); + splashes (FUN_1070_0105 for aliens/big aliens, the hero's in HeroUpdate), effects 7/23 in the table. Not exercised: blowing up in the air (0b70), the hero riding it |
| 3 | ~~OBJECTS 0x13ec~~ the gun turret (VMT 0xcbc, FUN_1070_2550) | EnemyFactory | CITYRAIN, FOREST02, FOREST05, CITY02, CITY_I01 | DONE (`wallgun.rs`, WALLGUN1: firing cycle, killed, broken apart; effect 65 = FUN_1040_105d). Not exercised: aiming at a visible hero (FOREST02's sits in the trunk), the lever (CITY02/CITY_I01) |
| 4 | ~~OBJECTS type 100~~ path platforms (FUN_1060_0166 kind 0, update FUN_1060_0819) | | CITY_I04 5, CITYHS 6, CITY_I01 5, GRANNY 2 | DONE (`PlatObj::path`, `platobj::update_path`, PATHPLAT1). Still warn if met: modes 0x4000 (bobbing), 0x680 (driven by the hero: 5123/5124/4e5a), 0x8000 (slowing at the ends); the lever (GRANNY's) is in but untested; kinds 2/3 (falling, sinking) unported |
| 5 | ~~icons 61000/61001~~ falling weights (FUN_1070_fa6a, kind 0x13a6, VMT 0xbe8, FUN_1068_215e) | SpawnFromIconTiles | CITY02, CITYHS, CITY_I03 | DONE (`weight.rs`, WEIGHT1: drop, chain bits, crush = hero hit kind 2, planks = effect 50 with its nested negative-tile blasts; the hero now falls from any state < 1 but -1..-5). Not exercised: the press (61001: animation, standing on it, FUN_1040_677d), shooting a landed weight broken |
| 6 | ~~OBJECTS 0x146e~~ the tank (VMT 0xdf8 shared with 0x1478, FUN_1070_b9d3) | EnemyFactory | CITY_I00, F_LAST | DONE (`tank.rs`, TANK1/TANK2: patrol, grenades, arm/claw/turret/hatch/hull destroyed, the wreck; hero state -3 = held by the claw in `Hero` (HELD), hit kinds 0 and 3). Not exercised: the claw grabbing the hero, the hero standing on it |
| 7 | ~~OBJECTS 0x1414~~ the meat hooks (VMT 0xd04, FUN_1070_4b9b) | EnemyFactory | BUTCHERY | DONE (`meathook.rs`, MEATHOOK1); FUN_1070_4b4e = Random(100) per hook at setup |
| 8 | ~~OBJECTS 0x14a0~~ the falling sign (VMT 0xbd8, FUN_1068_1f3c) | EnemyFactory | GRANNY | DONE (`weight.rs` SLAB, SLAB1: chain cleared (5c10/5b33), falls on the hero = hit kind 2, lands, stood on with the original's mixed-up FUN_1070_00b6 arguments) |
| 9 | ~~OBJECTS 0x2715~~ | LevelInit | GRANNY | DONE (`collapse.rs`, COLLAPSE1): the background building collapse FUN_1088_a481 (run by background proc 15 = FUN_1090_1f6f). Its draw (the building rotated, FUN_10f0_513c/545b on extra map planes 4/5) is not ported |
| 10 | ~~OBJECTS 0x14b4~~ the pumping door (VMT 0xd44, FUN_1070_a0bd -> FUN_10a8_11e9) | EnemyFactory | CITY_I04 | DONE (`Level::door_pump`, PUMPDOOR1). Weak check: no Random involved; the hero's path through it matches |
| 11 | ~~OBJECTS 0x14e6~~ (VMT 0xda0) | EnemyFactory | FOREST05 | DONE (`spikeball.rs`, SPIKE1). A spiked ball on a chain (FUN_1070_9a66, nested 9a16): swings once its lever (+0x1c) is on or it is shot; the quake (45ba/45bc/45c0) pushes it (Random(0x18), Random(2)); hits the hero with kind 3 (0xc0, sound 0xf), drips blood (callback at 1170:23b7). Seven long divisions: disassemble before porting |
| 12 | OBJECTS 0x2716 | LevelInit | FORPLANE | draw only: 1170:534e/5350 for the background proc FUN_1090_178e (renderer's) |
| — | OBJECTS 0x272e (decor layer, VMT 0xaf8, draw procs 1088:a25d/a371/...) | LevelInit | many | drawn by main.rs; no update method: warning silenced |

Also from AGENTS.md (not counted per map yet): riding the smallbot (FUN_1070_a18e), levers needing a key,
pick-up effects 3/74, particle spawn effects 30064/30065/30070, mover 1050:19f8, platform shake/dust (flags 8),
frozen/burnt deaths, barrels >= 0xb, the hero on the big alien's shield, wall chunks 60308/60304 (45db),
bonus 0xd10, callers of hero hit kinds 2/3 (FUN_1068_1f3c, 215e, FUN_1070_9a66, b9d3).

## Left (2026-11-21), in priority order

1. ~~OBJECTS 0x146e~~ the tank: DONE (`tank.rs`, TANK1, TANK2).
2. ~~OBJECTS 0x14e6~~ the spiked ball: DONE (`spikeball.rs`, SPIKE1).
3. ~~OBJECTS 0x1414~~ the meat hooks: DONE (`meathook.rs`, MEATHOOK1).
4. ~~OBJECTS 0x2715~~ (GRANNY): the collapse: DONE (`collapse.rs`, COLLAPSE1); the rotated building's draw is left.
5. Draw only: 0x2716 (FORPLANE, FUN_1090_178e's 534e/5350) for the renderer.
6. From AGENTS.md, not seen on the episode maps' load: riding the smallbot (FUN_1070_a18e), levers needing a
   key, pick-up effects 3/74, particle spawn effects 30064/30065/30070, mover 1050:19f8, platform shake/dust
   (flags 8), frozen/burnt deaths (FUN_1070_564d / 544e: aliens, big aliens, the granny), barrels >= 0xb, the
   hero on the big alien's shield, wall chunks 60308/60304 (45db), bonus 0xd10, path-platform modes
   0x4000/0x680/0x8000 and platform kinds 2/3, the press (icon 61001) and the carrier's mid-air blast are
   ported but not exercised by a scenario.

## Sounds the new objects play (for the sound session)

(filled in as each object is ported)

- Tank (FUN_1070_b9d3): wired in `tank.rs` (sounddiff TANK1/TANK2: all its plays matched): engine loop 0x36 on
  +0x4b (priority 0x11f4, pitch 0x100 / 0x11e turning at volume 0xaa) while the hull lives, then 0x1e at 0x80;
  0x30 arm shot / claw cycle, 0x1d gun, 0x15/0x14 gun recoil, 0x51 hatch, 0x50 claw hits a wall, 0x1a + 0x16
  grab, 0x26 claw off, 0x4a arm off, 0x17 turret off, 0x4e wreck.
- Meat hooks (FUN_1070_4b9b): wired in `meathook.rs`: 0x33 at speed 0x30 going up, 0x30 going down, the motor loop
  0x2b on +0x26 (volume (speed*256/full)*2/3, pitch 0x100 - that/2, priority 0x11f4), 0x4b a piece shot apart.
- Collapse (FUN_1088_a481): wired in `collapse.rs`: 0x4b per debris burst (at the background position), the rumble
  loop 0x53 on 4ed2 (priority 0x13e8, volumes min(0x42a - t, 0x100)).
- Spiked ball (FUN_1070_9a66): wired in `spikeball.rs`: 0xf when it hurts the hero, 1 when shot (sounddiff SPIKE1).

- Granny (FUN_1070_cb38): 0x9d when hit alive, pitch `0x100 - Random(0x20)` (FUN_10e8_03ae; the Random is
  already drawn in `granny.rs`); 0x25 at half volume (1ab8 = 0x80) when she lands (state -5, counter 0x18);
  0xf when the hero stomps her lying and the stomp returns > 1; burnt 0x71, frozen 0x21 (shared deaths).
- Carrier (FUN_1070_0cce): 0x85 when it starts, at x + dir*0x60; while flying and audible (10e0:0073) the loop
  helper 0408 on key +0x60 with sound 0x29 at pitch 0x11f4 (10e8:03f2); the wreck under the water line loops
  0x1e the same way; 1 when a wall stops it (if its loop played); crash 0x4a, 0x4e, 0x26; blown up 0x4a, 0x18;
  the destructor 0a06 / release 0c6d stop the loop (10e0:0339).
- Gun turret (FUN_1070_2550): 0x65 each shot; killed 100; hit 1 (dead) or 0x49 alive, unless the hit mask is
  0x48 or it still flashes (+0x1c >= 0x7e); broken apart 0x4a. Effect 65 (its pieces): 0x4b at pitch
  0xf6 + Random(0x14) (drawn).
- Weights (FUN_1068_215e): 0x19 on the drop; landing 0x18 (weight) / 0x1c (press); the press 0x1f each cycle
  ((0x1a % ((frames-1)*speed)) == 0, type 1). Effect 50 (planks, FUN_1040_50cf): 0x26 and 0x1c at half volume.
- Falling sign (FUN_1068_1f3c): 0x19 when it drops, 1 and 0x17 when it lands, all at x + 0x34.
- Pumping door (FUN_10a8_11e9): 0x81 at (y + 0x20, x + 0x10) when (frame*0x60) % 0x1680 == 0xb40.
- Splashes (FUN_1070_0105 / HeroUpdate): 0x6f at (water line, x), pitch 0xd0 + Random(0x50) (drawn in
  `Level::splash` / `Hero::update`), volume 1ab8 = 0xaa (alien), 0x100 (big alien), 0x180 (215e), the hero
  min(fall*2 + 0x80, 0x100).
