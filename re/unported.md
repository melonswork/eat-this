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
| 2 | carrier 0x14be (icon 61600, VMT 0xc84, update FUN_1070_0cce, hit 0b02) | | CITY02, CITY_I00, CITY_I06, FOREST03, FOREST05, STONES | created already (passengers eaten), no update |
| 3 | OBJECTS 0x13ec (VMT 0xcbc) | EnemyFactory | CITYRAIN, FOREST02, FOREST05, CITY02, CITY_I01 | enemy (4eb2) |
| 4 | OBJECTS type 100 (FUN_1060_0166, platform object) | | CITY_I04 5, CITYHS 6, CITY_I01 5, GRANNY 2 | |
| 5 | icons 61000/61001 (FUN_1070_fa6a, kind 0x13a6) | SpawnFromIconTiles | CITY02, CITYHS, CITY_I03 | |
| 6 | OBJECTS 0x146e (VMT 0xdf8 shared with 0x1478, FUN_1068_0d84) | EnemyFactory | CITY_I00, F_LAST | enemy (4eb2) |
| 7 | OBJECTS 0x1414 (VMT 0xd04) | EnemyFactory | BUTCHERY | calls FUN_1070_4b4e per 32 px |
| 8 | OBJECTS 0x14a0 (VMT 0xbd8, update FUN_1068_1f3c = hero hit kind 2) | EnemyFactory | GRANNY | |
| 9 | OBJECTS 0x2715 | LevelInit | GRANNY | |
| 10 | OBJECTS 0x14b4 (VMT 0xd44) | EnemyFactory | CITY_I04 | |
| 11 | OBJECTS 0x14e6 (VMT 0xda0) | EnemyFactory | FOREST05 | |
| 12 | OBJECTS 0x2716 | LevelInit | FORPLANE | |
| — | OBJECTS 0x272e (decor layer, VMT 0xaf8, draw procs 1088:a25d/a371/...) | LevelInit | many | drawn by main.rs; no update method: warning silenced |

Also from AGENTS.md (not counted per map yet): riding the smallbot (FUN_1070_a18e), levers needing a key,
pick-up effects 3/74, particle spawn effects 30064/30065/30070, mover 1050:19f8, platform shake/dust (flags 8),
frozen/burnt deaths, barrels >= 0xb, the hero on the big alien's shield, wall chunks 60308/60304 (45db),
bonus 0xd10, callers of hero hit kinds 2/3 (FUN_1068_1f3c, 215e, FUN_1070_9a66, b9d3).

## Sounds the new objects play (for the sound session)

(filled in as each object is ported)

- Granny (FUN_1070_cb38): 0x9d when hit alive, pitch `0x100 - Random(0x20)` (FUN_10e8_03ae; the Random is
  already drawn in `granny.rs`); 0x25 at half volume (1ab8 = 0x80) when she lands (state -5, counter 0x18);
  0xf when the hero stomps her lying and the stomp returns > 1; burnt 0x71, frozen 0x21 (shared deaths).
