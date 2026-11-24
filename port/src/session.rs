//! Extras (docs/extras.md): a level's input log, save states, rewind and recordings.
//!
//! The simulation is deterministic, so a moment of a level is its map, its first RandSeed and the
//! inputs since. A save state on disk is just that, replayed on load (over 200 000 frames a
//! second); the log also becomes a recording in the original's demo format. Snapshots (a clone of
//! the game every `EVERY` frames) make rewinding fast. Nothing here runs while a demo plays.

use crate::level::{TALLY, Tally};
use crate::{Game, demo, load_world, sound};
use std::collections::VecDeque;
use std::error::Error;
use std::path::{Path, PathBuf};

/// Frames between two snapshots, and how many are kept (two minutes).
const EVERY: usize = 140;
const KEPT: usize = 60;

/// Randomize: a seed from the clock.
pub fn clock_seed() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_secs() as u32)
}

/// What the simulation is given in one frame of live play.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Input {
    pub keys: [i8; 9],
    pub mouse: (i16, i16),
    /// A weapon key (`Game::weapon_key`), -1 none.
    pub weapon: i8,
    /// `Game::wide_actors`.
    pub wide: u8,
}

#[derive(Clone)]
pub struct Log {
    pub map: PathBuf,
    pub seed: u32,
    pub tries: i32,
    /// The run is being recorded: RandSeed 0 and the cursor at the start, as DemoStartRecording.
    pub record: Option<(i32, i32)>,
    pub frames: Vec<Input>,
}

struct Snap {
    at: usize,
    game: Game,
    tally: Tally,
}

pub struct Session {
    pub log: Log,
    snaps: VecDeque<Snap>,
    pub keep_snaps: bool,
}

fn apply(game: &mut Game, i: &Input) {
    game.live_mouse = Some((i.mouse.0 as i32, i.mouse.1 as i32));
    game.weapon_key = (i.weapon >= 0).then_some(i.weapon as usize);
    game.wide_actors = i.wide as i32;
    game.step(i.keys);
}

impl Log {
    /// The level's start.
    pub fn start(&self, data: &Path) -> Result<Game, Box<dyn Error>> {
        let world = load_world(data, &self.map)?;
        TALLY.with(|t| t.set(Tally::default()));
        let mut g = Game::start(world, None, self.seed, self.record);
        g.level.tries = self.tries;
        Ok(g)
    }

    /// The game after all the log's frames, silently.
    pub fn replay(&self, data: &Path) -> Result<Game, Box<dyn Error>> {
        let mut g = self.start(data)?;
        run(&mut g, &self.frames);
        sound::stop_loops();
        Ok(g)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut b = b"EATTHIS STATE 1\n".to_vec();
        let name = self.map.to_string_lossy().into_owned().into_bytes();
        b.extend((name.len() as u32).to_le_bytes());
        b.extend(name);
        b.extend(self.seed.to_le_bytes());
        b.extend(self.tries.to_le_bytes());
        let (rx, ry) = self.record.unwrap_or((0x7fff, 0));
        b.extend((rx as i16).to_le_bytes());
        b.extend((ry as i16).to_le_bytes());
        b.extend((self.frames.len() as u32).to_le_bytes());
        // Runs of equal frames: count, then the frame.
        let mut k = 0;
        while k < self.frames.len() {
            let f = self.frames[k];
            let n = self.frames[k..].iter().take(0xffff).take_while(|g| **g == f).count();
            b.extend((n as u16).to_le_bytes());
            b.extend(f.keys.map(|v| v as u8));
            b.extend(f.mouse.0.to_le_bytes());
            b.extend(f.mouse.1.to_le_bytes());
            b.extend([f.weapon as u8, f.wide]);
            k += n;
        }
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        // Never leave half a state in the slot.
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, b)?;
        std::fs::rename(tmp, path)
    }

    pub fn load(path: &Path) -> std::io::Result<Log> {
        let b = std::fs::read(path)?;
        Self::parse(&b).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}: not a save state", path.display())))
    }

    fn parse(b: &[u8]) -> Option<Log> {
        struct R<'a>(&'a [u8]);
        impl<'a> R<'a> {
            fn take(&mut self, n: usize) -> Option<&'a [u8]> {
                let (a, rest) = self.0.split_at_checked(n)?;
                self.0 = rest;
                Some(a)
            }
            fn u32(&mut self) -> Option<u32> {
                Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
            }
            fn i16(&mut self) -> Option<i16> {
                Some(i16::from_le_bytes(self.take(2)?.try_into().ok()?))
            }
        }
        let mut r = R(b.strip_prefix(b"EATTHIS STATE 1\n")?);
        let n = r.u32()? as usize;
        let map = PathBuf::from(String::from_utf8(r.take(n)?.to_vec()).ok()?);
        let seed = r.u32()?;
        let tries = r.u32()? as i32;
        let (rx, ry) = (r.i16()?, r.i16()?);
        let count = r.u32()? as usize;
        let mut frames = Vec::new();
        while frames.len() < count {
            let n = r.i16()? as u16 as usize;
            if n == 0 || frames.len() + n > count {
                return None;
            }
            let mut keys = [0i8; 9];
            for (k, &v) in keys.iter_mut().zip(r.take(9)?) {
                *k = v as i8;
            }
            let mouse = (r.i16()?, r.i16()?);
            let t = r.take(2)?;
            if t[0] as i8 >= 12 {
                return None;
            }
            frames.extend(std::iter::repeat_n(Input { keys, mouse, weapon: t[0] as i8, wide: t[1] }, n));
        }
        let record = (rx != 0x7fff).then_some((rx as i32, ry as i32));
        Some(Log { map, seed, tries, record, frames })
    }
}

fn run(g: &mut Game, frames: &[Input]) {
    for f in frames {
        apply(g, f);
        g.sound.clear();
    }
}

impl Session {
    pub fn new(log: Log) -> Session {
        Session { log, snaps: VecDeque::new(), keep_snaps: false }
    }

    /// One frame of live play.
    pub fn step(&mut self, game: &mut Game, i: Input) {
        if self.keep_snaps && self.log.frames.len() % EVERY == 0 {
            self.snaps.push_back(Snap { at: self.log.frames.len(), game: game.clone(), tally: TALLY.with(|t| t.get()) });
            if self.snaps.len() > KEPT {
                self.snaps.pop_front();
            }
        }
        self.log.frames.push(i);
        apply(game, &i);
    }

    /// Goes `n` frames back (to the oldest snapshot at most). None: nothing older is kept.
    pub fn rewind(&mut self, n: usize) -> Option<Game> {
        let want = self.log.frames.len().saturating_sub(n).max(self.snaps.front()?.at);
        if want >= self.log.frames.len() {
            return None;
        }
        while self.snaps.back().is_some_and(|s| s.at > want) {
            self.snaps.pop_back();
        }
        let s = self.snaps.back()?;
        let mut g = s.game.clone();
        TALLY.with(|t| t.set(s.tally));
        run(&mut g, &self.log.frames[s.at..want]);
        self.log.frames.truncate(want);
        sound::stop_loops();
        Some(g)
    }
}

/// What two equal games have in common (for the checks).
pub fn fingerprint(g: &Game) -> String {
    let t = TALLY.with(|t| t.get());
    format!(
        "frame {} seed {:#x} cam ({}, {}) hero ({}, {}) state {} health {} {t:?} {:?} {:?} {:?} {:?}",
        g.frame, g.rng.seed, g.camera.x, g.camera.y, g.hero.x, g.hero.y, g.hero.state, g.hero.health,
        g.level.objects, g.level.doors, g.level.plat_objs,
        g.level.particles.list.iter().map(|p| (p.m.w(8), p.m.w(0xa), p.m.w(0x16))).collect::<Vec<_>>(),
    )
}

/// `--check-states` (with --headless --demo): plays the demo's input live, and every few hundred
/// frames compares the game with one loaded from a saved log and one rewound and played again.
pub fn check(data: &Path, map: &Path, d: &demo::Demo) -> Result<bool, Box<dyn Error>> {
    let input = |f: u32| {
        let (keys, mouse) = d.input(f);
        Input { keys, mouse, weapon: if f % 500 == 250 { (f / 500 % 12) as i8 } else { -1 }, wide: (f / 1000 % 2 * 18) as u8 }
    };
    let log = Log { map: map.to_path_buf(), seed: 12345, tries: 0, record: None, frames: Vec::new() };
    let mut game = log.start(data)?;
    let mut s = Session::new(log);
    s.keep_snaps = true;
    let path = std::env::temp_dir().join(format!("eatthis-check-{}.ets", std::process::id()));
    let (mut ok, mut n) = (true, 0);
    for f in 0..d.frames {
        s.step(&mut game, input(f));
        game.sound.clear();
        if game.restart || game.level_over {
            break;
        }
        if f % 777 == 776 {
            let (now, tally) = (fingerprint(&game), TALLY.with(|t| t.get()));
            s.log.save(&path)?;
            let l = Log::load(&path)?;
            let loaded = fingerprint(&l.replay(data)?);
            // Rewinding 333 frames and playing them again.
            TALLY.with(|t| t.set(tally));
            let tail = s.log.frames[s.log.frames.len() - 333..].to_vec();
            let mut g = s.rewind(333).ok_or("nothing to rewind")?;
            for i in tail {
                s.step(&mut g, i);
                g.sound.clear();
            }
            let rewound = fingerprint(&g);
            n += 1;
            if loaded != now || rewound != now {
                ok = false;
                println!("frame {}: loaded {} rewound {}", f + 1, loaded == now, rewound == now);
            }
            game = g;
        }
    }
    let _ = std::fs::remove_file(&path);
    println!("{n} states saved, loaded and rewound: {}", if ok { "all equal" } else { "DIFFERENT" });
    Ok(ok)
}

/// `--rerecord DIR` (with --demo): the demo's input played live as a recorded run, saved under DIR
/// the way the front end does.
pub fn rerecord(data: &Path, map: &Path, d: &demo::Demo, root: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let m = d.input(0).1;
    let log = Log { map: map.to_path_buf(), seed: 0, tries: 0, record: Some((m.0 as i32, m.1 as i32)), frames: Vec::new() };
    let mut game = log.start(data)?;
    let mut s = Session::new(log);
    for f in 0..d.frames {
        let (keys, mouse) = d.input(f);
        s.step(&mut game, Input { keys, mouse, weapon: -1, wide: 0 });
        game.sound.clear();
        if game.restart || game.level_over {
            break;
        }
    }
    Ok(save_recording(root, &s.log, &game.checks)?)
}

/// GIF-style LZW with literals only (a clear code every 200 bytes keeps the codes 9 bits wide).
fn lzw_literal(data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_le_bytes().to_vec();
    let (mut bits, mut n) = (0u32, 0);
    let mut put = |c: u32, out: &mut Vec<u8>| {
        bits |= c << n;
        n += 9;
        while n >= 8 {
            out.push(bits as u8);
            bits >>= 8;
            n -= 8;
        }
    };
    for chunk in data.chunks(200) {
        put(0x100, &mut out);
        for &b in chunk {
            put(b as u32, &mut out);
        }
    }
    put(0x101, &mut out);
    if n > 0 {
        out.push(bits as u8);
    }
    out
}

/// The log as a .REC (re/demo.md); `checks` are RandSeed at frames 35k.
pub fn recording(log: &Log, checks: &[u32]) -> Vec<u8> {
    // The original samples CHECK[frames / 35] too; a whole number of windows has none to give.
    let frames = if log.frames.len() % 35 == 0 { log.frames.len().saturating_sub(1) } else { log.frames.len() };
    let f = &log.frames[..frames];
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut h = (frames as u32).to_le_bytes().to_vec();
    h.extend([0, 0, 3, 0]);
    entries.push(("REC HEADER".into(), h));
    let mut mouse: Vec<u8> = Vec::new();
    let mut k = 0;
    while k < f.len() {
        let m = f[k].mouse;
        mouse.extend(m.0.to_le_bytes());
        mouse.extend(m.1.to_le_bytes());
        let same = f[k + 1..].iter().take(0x7fff).take_while(|i| i.mouse == m).count();
        if same > 0 {
            mouse.extend(0x8001u16.to_le_bytes());
            mouse.extend((same as u16).to_le_bytes());
        }
        k += 1 + same;
    }
    entries.push(("MOUSE".into(), mouse));
    for key in 0..9 {
        // The first value is the priming frame DemoLoad consumes.
        let v: Vec<i8> = std::iter::once(-1).chain(f.iter().map(|i| i.keys[key])).collect();
        let mut out = Vec::new();
        let mut k = 0;
        while k < v.len() {
            let n = v[k..].iter().take(0xffff).take_while(|x| **x == v[k]).count();
            out.extend(((n - 1) as u16).to_le_bytes());
            out.push(v[k] as u8);
            k += n;
        }
        entries.push((format!("KEY #{key}"), out));
    }
    let n = frames / 35 + 1;
    entries.push(("CHECK".into(), checks.iter().copied().chain(std::iter::repeat(0)).take(n).flat_map(|c| c.to_le_bytes()).collect()));

    let mut b = b"Image Line Archive File (1995)\x1a".to_vec();
    b.resize(0x5a, 0);
    let mut dir = Vec::new();
    for (name, data) in &entries {
        let packed = lzw_literal(data);
        dir.extend((b.len() as u32).to_le_bytes());
        dir.extend((packed.len() as u32).to_le_bytes());
        dir.push(1);
        dir.push(name.len() as u8);
        let mut n = name.clone().into_bytes();
        n.resize(12, 0);
        dir.extend(n);
        b.extend(packed);
    }
    let at = b.len() as u32;
    b[0x1f..0x23].copy_from_slice(&at.to_le_bytes());
    b[0x25..0x27].copy_from_slice(&(entries.len() as u16).to_le_bytes());
    let what = b"EAT THIS DEMO";
    b[0x29] = what.len() as u8;
    b[0x2a..0x2a + what.len()].copy_from_slice(what);
    b.extend(dir);
    b
}

/// Writes a recorded run as a user map of its own (map, script with PlayRecord=, recording).
/// Returns its folder.
pub fn save_recording(root: &Path, log: &Log, checks: &[u32]) -> std::io::Result<PathBuf> {
    let stem = log.map.file_stem().unwrap_or_default().to_string_lossy().to_ascii_uppercase();
    let (n, dir) = (1..).map(|n| (n, root.join(format!("{stem}_{n}")))).find(|(_, d)| !d.exists()).unwrap();
    std::fs::create_dir_all(&dir)?;
    let src = log.map.parent().unwrap_or(Path::new("."));
    std::fs::copy(&log.map, dir.join(format!("{stem}.MAP")))?;
    let rec = format!("{stem}.REC");
    std::fs::write(dir.join(&rec), recording(log, checks))?;
    // The level's script without its own title, videos and recording.
    let mut scr = Vec::new();
    if let Some(p) = crate::find_ci(src, &format!("{stem}.SCR")) {
        for line in std::fs::read(p)?.split(|&c| c == b'\n') {
            let l: String = line.iter().map(|&c| c as char).collect();
            let key = l.split('=').next().unwrap_or("").trim().to_ascii_lowercase();
            if !matches!(key.as_str(), "playrecord" | "video" | "title" | "comments") {
                scr.extend(line.strip_suffix(b"\r").unwrap_or(line));
                scr.extend(b"\r\n");
            }
        }
    }
    let secs = log.frames.len() / 70;
    scr.extend(format!("PlayRecord={rec}\r\nTitle=RECORDING: {stem} {n} - {}:{:02},0\r\n", secs / 60, secs % 60).into_bytes());
    std::fs::write(dir.join(format!("{stem}.SCR")), scr)?;
    Ok(dir)
}
