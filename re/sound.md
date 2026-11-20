# Sound and music

## Data

- **Effects**: `work/cd/BIN/SOUNDS/*.WAV`, 177 files, RIFF PCM 8-bit unsigned mono 11025 Hz.
  Sound ids are the `LoadWAV=` lines of `MAPS/ETMAIN.SCR` in order, from 0 (`LoadWAV=statsl`)
  to 174 (`shit`); comment lines don't count, blank ones (`LoadWAV=`) do. The loader
  (1088:bf00's caller, ~1088 LevelLoad script loop) builds `sounds\` + name + `.wav` and calls
  SoundLoadWav(id, path) (10e8:01a8: DWS_WAV2DWD into a far block, table DS:553e, 8 bytes per id).
  `python3 -c` over ETMAIN.SCR or `port/src/sound.rs::sound_names` lists them.
- **Music**: CD audio, tracks 2..7 (`BIN/TRACKS.TXT`: Forest1, Sea, Cityviol, Forestdi, Stats,
  Palpitation). `tools/extract_audio.py "iso/Eat This.cue" work/cd/AUDIO` writes
  `TRACK02.WAV`..`TRACK07.WAV` (44.1 kHz 16-bit stereo, little-endian, from INDEX 01 to the next
  track's INDEX 00). A level's script says `CDTrack=N,loop` (FUN_1088_bf00 -> 1170:4d98 = N,
  4d9a = loop > 0); the level start (FUN_1080_167b) calls CdPlayTrack(4d98, 4d9a) (10c8:02e0:
  MCI `play cdaudio from N to N+1`, tmsf); N is the CD track number itself (CITY = 4 Cityviol,
  FOREST0x = 2, rain levels 3; FOREST.SCR's `CDTrack=1` names the data track: silence). The
  stats screen plays track 6 (CdPlayTrack(0, 6)), the menu jukebox entry k plays k + 2, script
  command 0x11 switches track in a level (only if different). When a track ends, 10c8:03cf
  replays it if the loop flag is set.

## DiamondWare Sound ToolKit (DWSW16.DLL)

The game fills one DWS_DPLAY record at DS:5d7e and hands it to DWS_DPLAY:

| offset | field | set by | flag (high word of 5d7e) |
|---|---|---|---|
| 5d82 | far ptr to the DWD | SoundPlay | 0x8000 |
| 5d86 | count (0 = loop forever) | FUN_10e8_03d0 | 0x4000 |
| 5d88 | priority | FUN_10e8_03f2 (loops: 0x11f4, grinder 0x11fe) | 0x2000 |
| 5d8c | soundnum (the playing instance; DWS writes it back) | SoundSetInfo | 0x0800 |
| 5d8e | lvol | FUN_10e8_037f(l, r) | 0x0400 |
| 5d90 | rvol | FUN_10e8_037f | 0x0200 |
| 5d92 | pitch | FUN_10e8_03ae(p) | 0x0100 |

0x100 is "unchanged" for volume and pitch (dws_IDENTITY). The port plays pitch p at
rate * p / 0x100 (higher = faster/higher); the callers' values sit around 0x100 (0xec..0x120
typically), so the direction is the only assumption and only shifts pitch slightly.

- `SoundPlay(id)` (10e8:04d1; FUN_1050_6038 is a thin wrapper): DWS_DPLAY with the flags set so
  far, then 5d7e/5d80 = 0 (everything the caller set is used once). A missing sample shows
  "ERROR - ATTEMPTING TO PLAY A NON EXISTING SOUND". If 1ac0 > 0 (the game running behind the
  menu: 6, in game 0) both volumes are divided by it (FUN_10e8_0414).
- `SoundSetInfo(soundnum)` (10e8:0575): DWS_DSETINFO, a playing loop's new volumes/pitch.
- FUN_10e8_05e0(id): DWS_DDISCARDAO, every playing instance of sample id stops (pick-ups and
  a few cries do it before playing again). FUN_10e8_05ca: DWS_DDISCARD (one instance).
- SoundUpdate (DWS_UPDATE) runs once per GameFrame.

## Position -> volume (segment 10e0)

- Listener: FUN_10e0_0002(y, x) = DS:5538/5536. Set at the end of the camera update
  FUN_1088_0c82 (after HeroUpdate): (gCameraX + 0xa0, gCameraY + 100), the screen's centre.
  The hero's own sounds in HeroUpdate therefore use the previous frame's camera.
- DS:1ab8 is the base volume of the next positional sound (0x100; callers set it, play, put 0x100
  back: a quieter cry, a moving platform's w16, the grinder's speed...).
- FUN_10e0_0073(y, x), a point: dx = x - lx, dy = (y - ly) * 2 (vertical counts double),
  d = round(sqrt(dx^2 + dy^2)) (8087 FILD/FSQRT/FISTP), v = max(0, 1ab8 - d/2); the far channel is
  scaled: dx > 0 -> lvol = v * 256 / (dx + 256), dx < 0 -> rvol = v * 256 / (256 - dx). If both
  are <= 0x10 it returns 0 (not played) else sets lvol/rvol (FUN_10e8_037f) and returns 1.
- FUN_10e0_016b(y2, x2, y1, x1), a box: the same from the box's nearest point (a corner with
  the full distance, an edge with the straight distance, inside = 0); panned only from a side.
- SoundPlayAt(id, y, x) (1050:5fe2) = 0073 then SoundPlay, or clear the record. FUN_1050_600a =
  the box version. FUN_1050_6047(id, y, x, base, n) = pitch base + Random(n), SoundPlayAt.
  FUN_1070_584e(obj, id, y, x) = pitch from obj+0x26 (aliens' voice), SoundPlayAt.
- **Audibility gates Random()**: many callers draw the pitch only `if (0073/016b(...))`. The port
  must compute it (pure functions `sound::point_volumes`/`box_volumes`) wherever that happens,
  even with the sound off.

## Loops (handles)

FUN_10e0_0408(&handle, id): if handle == -1 it allocates a 14-byte record (table DS:5434, count
1ab6) pointing back at the handle, plays (count 0 = forever when 1aba = 1, the default) and
stores soundnum; else it marks the record kept (+0xc = 1) and SoundSetInfo's the new volumes.
GameFrame calls FUN_10e0_04a4 after the update: a record not kept this frame stops (handle = -1).
FUN_10e0_0339(i) stops one, 03d5 stops all (level start/end). Owners: platforms (record w15),
the grinder (DS:4ee8), objects (fields like +0x1e, +0x26, +0x30), weather (4ed2).

## Platforms

FUN_1078_0002/0048 play a platform's sound set w11 (table DS:0ec4, 5 words per set: start,
point reached, end, moving loop, shake amount; 0x7fff = none) over its box (x, y, x + w, y + h)
at volume w16: start in FUN_1078_05a0, the loop every moving frame, point/end on arrival (the
end also stops the loop, FUN_1078_00ae). Sets 0..9 are in `port/src/level.rs` PLATFORM_SOUNDS.

## Checking: the tracer's sounds.tsv

`mise run trace` writes `work/tracer/<DEMO>/sounds.tsv`: every SoundPlay (10e8:04d1),
SoundSetInfo (0575) and loop stop (10e0:0339) with the record's lvol/rvol/pitch, the flags and
the caller chain. `python3 tools/sounddiff.py DEMO` runs the port with `ET_SOUND_TSV` and matches
plays by id within +-1 frame, then compares volumes and pitch (only fields whose flag is set).

## Port

`port/src/sound.rs`: the DWS-like front end (thread-local; pure volume math; play/keep/stop
events, no-ops unless a mixer, `--wav` or ET_SOUND_TSV listens), the mixer (8 voices,
priority/age stealing, linear interpolation, 44.1 kHz stereo f32) and the music player.
Window mode opens an SDL2 audio device (`--mute` to skip); `--headless --demo X --wav out.wav`
renders the demo's sound offline (630 samples per 70 Hz frame).
