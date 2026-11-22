//! Sound effects and music (re/sound.md).
//!
//! The original plays WAVs through the DiamondWare Sound ToolKit (DWSW16.DLL): `LoadWAV=` lines
//! of MAPS/ETMAIN.SCR give sound ids 0.. in order (SoundLoadWav 10e8:01a8), every play fills a
//! DWS_DPLAY record at 1170:5d7e (flags, lvol 5d8e, rvol 5d90, pitch 5d92; 0x100 = unchanged)
//! that SoundPlay (10e8:04d1) hands to DWS_DPLAY and clears. Positions become volumes against
//! the listener (10e0:0002, the camera's centre) in FUN_10e0_0073 (a point) and FUN_10e0_016b
//! (a box); a sound too faint in both channels (<= 0x10) is not played. Looping sounds belong
//! to a handle that their owner refreshes every frame (FUN_10e0_0408); GameFrame drops the ones
//! not refreshed (FUN_10e0_04a4). Music is CD audio (`CDTrack=N,loop` plays CD track N).
//!
//! Nothing here feeds back into the simulation: the volume math is pure, and everything else is
//! a no-op unless a mixer, a WAV capture or the ET_SOUND_TSV log is attached (never in a
//! plain `--headless` run).

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Output sample rate (the CD's, so music needs no resampling).
pub const RATE: u32 = 44100;
/// Mixing voices (the original's INI default: 8, 1170:1ac4).
const VOICES: usize = 8;
/// DWS_DPLAY's default priority when none is given (the loops use 0x11f4.. so they win).
const DEFAULT_PRIORITY: i32 = 0x1000;

/// Loop handles of owners that aren't objects (objects use `key::obj`).
pub mod key {
    pub const GRINDER: u64 = 1 << 40; // 1170:4ee8
    pub const RAIN: u64 = (1 << 40) + 1; // 1170:4de0
    pub const ENGINE: u64 = (1 << 40) + 2; // 1170:4e92, the vehicle
    pub const COLLAPSE: u64 = (1 << 40) + 3; // 1170:4ed2
    /// An object's handle field at `off` (objects are numbered by their record index).
    pub fn obj(index: usize, off: u32) -> u64 {
        (2 << 40) | ((index as u64) << 8) | off as u64
    }
    /// A lift's handle (record +0x1e), by its x and top stop.
    pub fn lift(x: i32, top: i32) -> u64 {
        (4 << 40) | ((x as u16 as u64) << 16) | top as u16 as u64
    }
    /// A platform's handle (record word 0xf).
    pub fn platform(index: usize) -> u64 {
        (3 << 40) | index as u64
    }
}

/// What the game asks the mixer to do.
#[derive(Clone, Debug)]
pub enum Ev {
    /// Start sample `id` on voice handle `voice` (looping forever if `looped`).
    Play { voice: u32, id: usize, l: i32, r: i32, pitch: i32, prio: i32, looped: bool },
    /// DWS_DSETINFO: a playing voice's new volumes and pitch (None: unchanged).
    Set { voice: u32, l: i32, r: i32, pitch: Option<i32> },
    Stop { voice: u32 },
    /// DWS_DDISCARDAO: every playing instance of sample `id` stops.
    Discard { id: usize },
    StopAll,
    /// CD track N from its start (0 or a missing track: silence).
    Music { track: u32, looped: bool },
    /// A video's sound track (VIDEOS/*.AVI) on voice handle `voice`, centred, at full volume.
    Pcm { voice: u32, sample: Arc<Sample> },
}

/// The DWS_DPLAY fields a caller set before the play (1170:5d7e flags).
#[derive(Default, Clone, Copy)]
struct Pending {
    vol: Option<(i32, i32)>,
    pitch: Option<i32>,
    prio: Option<i32>,
}

struct Loop {
    key: u64,
    voice: u32,
    fresh: bool,
}

struct Front {
    on: bool,
    listener: (i32, i32),
    /// 1170:1ab8: the reach/volume of the next positional sound (0x100 normally).
    vol: i32,
    pend: Pending,
    loops: Vec<Loop>,
    next_voice: u32,
    events: Vec<Ev>,
    frame: u32,
    log: Option<std::io::BufWriter<std::fs::File>>,
}

thread_local! {
    static FRONT: RefCell<Front> = RefCell::new(Front {
        on: false,
        listener: (0xa0, 100),
        vol: 0x100,
        pend: Pending::default(),
        loops: Vec::new(),
        next_voice: 1,
        events: Vec::new(),
        frame: 0,
        log: std::env::var_os("ET_SOUND_TSV").and_then(|p| {
            let mut f = std::io::BufWriter::new(std::fs::File::create(p).ok()?);
            writeln!(f, "frame\tkind\tid\tlvol\trvol\tpitch").ok()?;
            Some(f)
        }),
    });
}

fn with<R>(f: impl FnOnce(&mut Front) -> R) -> R {
    FRONT.with(|s| f(&mut s.borrow_mut()))
}

/// Turn the event stream on (a mixer or a capture listens).
pub fn enable() {
    with(|s| s.on = true);
}

fn active(s: &Front) -> bool {
    s.on || s.log.is_some()
}

/// FUN_10e0_0002 (GameFrameInGame's start): the listener is the camera's centre.
pub fn listener(x: i32, y: i32) {
    with(|s| s.listener = (x, y));
}

/// The frame label for the ET_SOUND_TSV log (the tracer's sounds.tsv numbering).
pub fn begin_frame(frame: u32) {
    with(|s| s.frame = frame);
}

/// 1170:1ab8, the volume the next positional sound starts from (callers put back 0x100).
pub fn set_volume(v: i32) {
    with(|s| s.vol = v);
}

/// FUN_10e8_03ae: the next play's pitch (0x100 = as recorded, higher plays faster).
pub fn pitch(p: i32) {
    with(|s| s.pend.pitch = Some(p));
}

/// FUN_10e8_037f: the next play's volumes.
pub fn volumes(l: i32, r: i32) {
    with(|s| s.pend.vol = Some((l, r)));
}

/// FUN_10e8_03f2: the next play's priority.
pub fn priority(p: i32) {
    with(|s| s.pend.prio = Some(p));
}

/// Distance attenuation and panning: `vol - dist / 2`, then the far channel scaled by
/// 0x100 / (0x100 + |dx|). None if both channels are <= 0x10 (inaudible).
fn pan(vol: i32, dist: i32, dx: i32) -> Option<(i32, i32)> {
    let v = (vol - dist / 2).max(0);
    let (mut l, mut r) = (v, v);
    if dx > 0 {
        l = (v << 8) / (dx + 0x100);
    } else if dx < 0 {
        r = (v << 8) / (0x100 - dx);
    }
    (l > 0x10 || r > 0x10).then_some((l, r))
}

/// Trunc(Sqrt(dx*dx + dy*dy)) of FUN_10e0_0015 (the 8087 rounds to nearest).
fn dist(dx: i32, dy: i32) -> i32 {
    ((dx as i64 * dx as i64 + dy as i64 * dy as i64) as f64).sqrt().round_ties_even() as i32
}

/// FUN_10e0_0073 without side effects: the volumes of a sound at (x, y) heard from `listener`.
/// Vertical distance counts double.
pub fn point_volumes(listener: (i32, i32), vol: i32, y: i32, x: i32) -> Option<(i32, i32)> {
    let (dx, dy) = (x - listener.0, (y - listener.1) * 2);
    pan(vol, dist(dx, dy), dx)
}

/// FUN_10e0_016b without side effects: the volumes of a sound filling the box (x1, y1)-(x2, y2):
/// the distance to its nearest point, panned only when the listener is beside it.
pub fn box_volumes(listener: (i32, i32), vol: i32, (x1, y1, x2, y2): (i32, i32, i32, i32)) -> Option<(i32, i32)> {
    let (lx, ly) = listener;
    let corner = |py: i32, px: i32| (dist(px - lx, (py - ly) * 2), px - lx);
    let (d, dx) = if lx < x1 {
        if ly < y1 {
            corner(y1, x1)
        } else if y2 < ly {
            corner(y2, x1)
        } else {
            (x1 - lx, x1 - lx)
        }
    } else if x2 < lx {
        if ly < y1 {
            corner(y1, x2)
        } else if y2 < ly {
            corner(y2, x2)
        } else {
            (-(x2 - lx), x2 - lx)
        }
    } else if ly < y1 {
        ((y1 - ly) * 2, 0)
    } else if y2 < ly {
        ((ly - y2) * 2, 0)
    } else {
        (0, 0)
    };
    pan(vol, d, dx)
}

/// FUN_10e0_0073: is a sound at (x, y) audible? If so its volumes become the next play's.
pub fn audible(y: i32, x: i32) -> bool {
    with(|s| match point_volumes(s.listener, s.vol, y, x) {
        Some(v) => {
            s.pend.vol = Some(v);
            true
        }
        None => false,
    })
}

/// FUN_10e0_016b: is a sound filling the box audible? If so its volumes become the next play's.
pub fn audible_box(rect: (i32, i32, i32, i32)) -> bool {
    with(|s| match box_volumes(s.listener, s.vol, rect) {
        Some(v) => {
            s.pend.vol = Some(v);
            true
        }
        None => false,
    })
}

fn log(s: &mut Front, kind: &str, id: i64, vol: Option<(i32, i32)>, pitch: Option<i32>) {
    let frame = s.frame;
    if let Some(f) = s.log.as_mut() {
        let (l, r) = vol.map_or(("-".into(), "-".into()), |(l, r)| (l.to_string(), r.to_string()));
        let p = pitch.map_or("-".into(), |p| p.to_string());
        let _ = writeln!(f, "{frame}\t{kind}\t{id}\t{l}\t{r}\t{p}");
    }
}

fn start(s: &mut Front, id: usize, looped: bool) -> u32 {
    let p = std::mem::take(&mut s.pend);
    log(s, "play", id as i64, p.vol, p.pitch);
    let voice = s.next_voice;
    s.next_voice += 1;
    if s.on {
        let (l, r) = p.vol.unwrap_or((0x100, 0x100));
        s.events.push(Ev::Play {
            voice,
            id,
            l,
            r,
            pitch: p.pitch.unwrap_or(0x100),
            prio: p.prio.unwrap_or(DEFAULT_PRIORITY),
            looped,
        });
    }
    voice
}

/// SoundPlay (10e8:04d1, FUN_1050_6038): sample `id` with what the caller set, then the
/// DWS_DPLAY record is cleared.
pub fn play(id: usize) {
    with(|s| {
        if active(s) {
            start(s, id, false);
        }
        s.pend = Pending::default();
    });
}

/// SoundPlayAt (1050:5fe2): `id` at (x, y) if audible.
pub fn play_at(id: usize, y: i32, x: i32) {
    if audible(y, x) {
        play(id);
    } else {
        with(|s| s.pend = Pending::default());
    }
}

/// FUN_10e8_03ae + SoundPlayAt (e.g. FUN_1050_6047 with its Random already drawn).
pub fn play_at_pitch(id: usize, pitch_: i32, y: i32, x: i32) {
    pitch(pitch_);
    play_at(id, y, x);
}

/// SoundPlayAt at the listener's height, `dx` beside it (SelectWeapon's click).
pub fn play_at_listener(id: usize, dx: i32) {
    let (lx, ly) = with(|s| s.listener);
    play_at(id, ly, lx + dx);
}

/// SoundPlayAt with 1170:1ab8 = `vol` around it.
pub fn play_at_vol(id: usize, vol: i32, y: i32, x: i32) {
    set_volume(vol);
    play_at(id, y, x);
    set_volume(0x100);
}

/// FUN_10e0_0408: keep looping sound `id` for `key` (the owner's handle) going this frame:
/// the first call starts it, later ones update its volumes (DWS_DSETINFO). Call after
/// `audible`/`audible_box` returned true; a loop not kept for a frame stops (FUN_10e0_04a4).
pub fn keep(key: u64, id: usize) {
    with(|s| {
        if !active(s) {
            s.pend = Pending::default();
            return;
        }
        if let Some(i) = s.loops.iter().position(|l| l.key == key) {
            let p = std::mem::take(&mut s.pend);
            s.loops[i].fresh = true;
            let voice = s.loops[i].voice;
            log(s, "set", id as i64, p.vol, p.pitch);
            if s.on {
                let (l, r) = p.vol.unwrap_or((0x100, 0x100));
                s.events.push(Ev::Set { voice, l, r, pitch: p.pitch });
            }
        } else {
            let voice = start(s, id, true);
            s.loops.push(Loop { key, voice, fresh: true });
        }
    });
}

/// Does `key`'s loop play (its handle is not -1)?
pub fn looping(key: u64) -> bool {
    with(|s| s.loops.iter().any(|l| l.key == key))
}

/// FUN_10e0_0339: stop the loop of `key` now.
pub fn stop(key: u64) {
    with(|s| {
        if let Some(i) = s.loops.iter().position(|l| l.key == key) {
            let l = s.loops.swap_remove(i);
            log(s, "stop", 0, None, None);
            if s.on {
                s.events.push(Ev::Stop { voice: l.voice });
            }
        }
    });
}

/// FUN_10e8_05e0: stop every playing instance of sample `id` (before playing it again).
pub fn discard(id: usize) {
    with(|s| {
        if s.on {
            s.events.push(Ev::Discard { id });
        }
    });
}

/// FUN_10e0_03d5 (level start/end): every loop stops, all sounds too.
pub fn stop_all() {
    with(|s| {
        s.loops.clear();
        if s.on {
            s.events.push(Ev::StopAll);
        }
    });
}

/// FUN_10e0_03d5: every loop stops (LevelFirstFrame, after a teleport too).
pub fn stop_loops() {
    with(|s| {
        for l in std::mem::take(&mut s.loops) {
            log(s, "stop", 0, None, None);
            if s.on {
                s.events.push(Ev::Stop { voice: l.voice });
            }
        }
    });
}

/// CdPlayTrack (10c8:02e0).
pub fn music(track: u32, looped: bool) {
    with(|s| {
        if s.on {
            s.events.push(Ev::Music { track, looped });
        }
    });
}

/// GameFrame after the update: FUN_10e0_04a4 (loops not kept this frame stop), then the frame's
/// events for the mixer.
pub fn end_frame() -> Vec<Ev> {
    with(|s| {
        let mut i = 0;
        while i < s.loops.len() {
            if s.loops[i].fresh {
                s.loops[i].fresh = false;
                i += 1;
            } else {
                let l = s.loops.swap_remove(i);
                log(s, "stop", 0, None, None);
                if s.on {
                    s.events.push(Ev::Stop { voice: l.voice });
                }
            }
        }
        if let Some(f) = s.log.as_mut() {
            let _ = f.flush();
        }
        std::mem::take(&mut s.events)
    })
}

// ---- data ---------------------------------------------------------------------------------------

/// A decoded sample: mono, -1..1, at `rate` Hz.
pub struct Sample {
    pub data: Vec<f32>,
    pub rate: u32,
}

impl std::fmt::Debug for Sample {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Sample({} at {} Hz)", self.data.len(), self.rate)
    }
}

/// RIFF WAVE: 8-bit unsigned or 16-bit signed PCM, mono or stereo (stereo is mixed down).
pub fn decode_wav(b: &[u8]) -> Option<(Vec<f32>, u32, u16)> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let (mut fmt, mut data) = (None, None);
    let mut p = 12;
    while p + 8 <= b.len() {
        let len = u32::from_le_bytes(b[p + 4..p + 8].try_into().ok()?) as usize;
        let body = &b[p + 8..(p + 8 + len).min(b.len())];
        match &b[p..p + 4] {
            b"fmt " if body.len() >= 16 => fmt = Some(body),
            b"data" => data = Some(body),
            _ => {}
        }
        p += 8 + len + (len & 1);
    }
    let (fmt, data) = (fmt?, data?);
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]).max(1);
    let rate = u32::from_le_bytes(fmt[4..8].try_into().ok()?);
    let bits = u16::from_le_bytes([fmt[14], fmt[15]]);
    let mono: Vec<f32> = match bits {
        8 => data.chunks_exact(channels as usize).map(|c| c.iter().map(|&v| (v as f32 - 128.0) / 128.0).sum::<f32>() / channels as f32).collect(),
        16 => data
            .chunks_exact(2 * channels as usize)
            .map(|c| c.chunks_exact(2).map(|v| i16::from_le_bytes([v[0], v[1]]) as f32 / 32768.0).sum::<f32>() / channels as f32)
            .collect(),
        _ => return None,
    };
    Some((mono, rate, channels))
}

fn find_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name)))
}

/// The sound ids: the `LoadWAV=` lines of MAPS/ETMAIN.SCR in order (comment lines don't count,
/// blank values do), each SOUNDS/<name>.WAV.
pub fn sound_names(data: &Path) -> Vec<String> {
    let Some(text) = find_ci(&data.join("MAPS"), "ETMAIN.SCR").and_then(|p| std::fs::read(p).ok()) else {
        return Vec::new();
    };
    String::from_utf8_lossy(&text)
        .lines()
        .filter(|l| !l.starts_with('\''))
        .filter_map(|l| l.split_once('='))
        .filter(|(k, _)| k.trim().eq_ignore_ascii_case("LoadWAV"))
        .map(|(_, v)| v.split('\'').next().unwrap_or("").trim().to_string())
        .collect()
}

pub struct Bank {
    pub samples: Vec<Option<Arc<Sample>>>,
    #[allow(dead_code)]
    pub names: Vec<String>,
    /// The CD's audio tracks, extracted by tools/extract_audio.py (work/cd/AUDIO/TRACKnn.WAV).
    audio_dir: PathBuf,
}

impl Bank {
    pub fn load(data: &Path) -> Bank {
        let names = sound_names(data);
        let dir = data.join("SOUNDS");
        let samples = names
            .iter()
            .map(|n| {
                if n.is_empty() {
                    return None;
                }
                let b = std::fs::read(find_ci(&dir, &format!("{n}.WAV"))?).ok()?;
                let (data, rate, _) = decode_wav(&b)?;
                Some(Arc::new(Sample { data, rate }))
            })
            .collect();
        Bank { samples, names, audio_dir: data.join("../AUDIO") }
    }

    fn track(&self, n: u32) -> Option<Arc<Vec<i16>>> {
        let b = std::fs::read(find_ci(&self.audio_dir, &format!("TRACK{n:02}.WAV"))?).ok()?;
        // 44.1 kHz 16-bit stereo: take the data chunk as is.
        let at = b.windows(4).position(|w| w == b"data")? + 8;
        Some(Arc::new(b[at..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()))
    }
}

// ---- mixer --------------------------------------------------------------------------------------

struct Voice {
    handle: u32,
    sample: Arc<Sample>,
    pos: f64,
    step: f64,
    l: f32,
    r: f32,
    looped: bool,
    prio: i32,
    age: u64,
}

struct Music {
    pcm: Arc<Vec<i16>>,
    pos: usize,
    looped: bool,
}

pub struct Mixer {
    bank: Bank,
    voices: Vec<Voice>,
    music: Option<Music>,
    tracks: Vec<Option<Arc<Vec<i16>>>>,
    age: u64,
    pub sfx_gain: f32,
    pub music_gain: f32,
}

impl Mixer {
    pub fn new(data: &Path) -> Mixer {
        Mixer {
            bank: Bank::load(data),
            voices: Vec::new(),
            music: None,
            tracks: vec![None; 100],
            age: 0,
            sfx_gain: 0.6,
            music_gain: 0.45,
        }
    }

    fn step(&self, s: &Sample, pitch: i32) -> f64 {
        s.rate as f64 / RATE as f64 * pitch.max(1) as f64 / 256.0
    }

    pub fn apply(&mut self, ev: Ev) {
        match ev {
            Ev::Play { voice, id, l, r, pitch, prio, looped } => {
                let Some(Some(sample)) = self.bank.samples.get(id).cloned() else {
                    return;
                };
                if self.voices.len() >= VOICES {
                    // No free voice: the lowest priority (then the oldest) gives way if it isn't
                    // more important than the new sound.
                    let (i, v) = self.voices.iter().enumerate().min_by_key(|(_, v)| (v.prio, v.age)).unwrap();
                    if v.prio > prio {
                        return;
                    }
                    self.voices.swap_remove(i);
                }
                self.age += 1;
                let step = self.step(&sample, pitch);
                self.voices.push(Voice {
                    handle: voice,
                    sample,
                    pos: 0.0,
                    step,
                    l: l as f32 / 256.0,
                    r: r as f32 / 256.0,
                    looped,
                    prio,
                    age: self.age,
                });
            }
            Ev::Set { voice, l, r, pitch } => {
                if let Some(i) = self.voices.iter().position(|v| v.handle == voice) {
                    let step = pitch.map(|p| self.step(&self.voices[i].sample, p));
                    let v = &mut self.voices[i];
                    v.l = l as f32 / 256.0;
                    v.r = r as f32 / 256.0;
                    if let Some(s) = step {
                        v.step = s;
                    }
                }
            }
            Ev::Pcm { voice, sample } => {
                self.voices.retain(|v| v.handle != voice);
                self.age += 1;
                let step = self.step(&sample, 0x100);
                self.voices.push(Voice { handle: voice, sample, pos: 0.0, step, l: 1.0, r: 1.0, looped: false, prio: i32::MAX, age: self.age });
            }
            Ev::Stop { voice } => self.voices.retain(|v| v.handle != voice),
            Ev::StopAll => self.voices.clear(),
            Ev::Discard { id } => {
                if let Some(Some(sample)) = self.bank.samples.get(id) {
                    self.voices.retain(|v| !Arc::ptr_eq(&v.sample, sample));
                }
            }
            Ev::Music { track, looped } => {
                let t = track as usize;
                if t < self.tracks.len() && self.tracks[t].is_none() {
                    self.tracks[t] = self.bank.track(track);
                    if self.tracks[t].is_none() && track > 1 {
                        eprintln!("sound: no CD track {track} (run tools/extract_audio.py)");
                    }
                }
                self.music = self.tracks.get(t).cloned().flatten().map(|pcm| Music { pcm, pos: 0, looped });
            }
        }
    }

    /// Mixes into `out` (interleaved stereo at RATE).
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        if let Some(m) = self.music.as_mut() {
            for o in out.iter_mut() {
                if m.pos >= m.pcm.len() {
                    if !m.looped {
                        break;
                    }
                    m.pos = 0;
                }
                *o += m.pcm[m.pos] as f32 / 32768.0 * self.music_gain;
                m.pos += 1;
            }
        }
        let g = self.sfx_gain;
        self.voices.retain_mut(|v| {
            let d = &v.sample.data;
            for o in out.chunks_exact_mut(2) {
                let mut i = v.pos as usize;
                if i >= d.len() {
                    if !v.looped || d.is_empty() {
                        return false;
                    }
                    v.pos -= d.len() as f64;
                    i = v.pos as usize;
                }
                let f = (v.pos - i as f64) as f32;
                let s = d[i] + (d.get(i + 1).copied().unwrap_or(if v.looped { d[0] } else { 0.0 }) - d[i]) * f;
                o[0] += s * v.l * g;
                o[1] += s * v.r * g;
                v.pos += v.step;
            }
            true
        });
        for o in out.iter_mut() {
            *o = o.clamp(-1.0, 1.0);
        }
    }
}

/// `--wav FILE`: the mixer rendered offline, 630 stereo frames per 70 Hz game frame.
pub struct Capture {
    mixer: Mixer,
    pcm: Vec<i16>,
    path: PathBuf,
}

impl Capture {
    pub fn new(data: &Path, path: &Path) -> Capture {
        enable();
        Capture { mixer: Mixer::new(data), pcm: Vec::new(), path: path.to_path_buf() }
    }

    pub fn frame(&mut self, events: Vec<Ev>, hz: u32) {
        for e in events {
            self.mixer.apply(e);
        }
        let mut buf = vec![0f32; (RATE / hz) as usize * 2];
        self.mixer.render(&mut buf);
        self.pcm.extend(buf.iter().map(|&v| (v * 32767.0) as i16));
    }

    pub fn save(&self) -> std::io::Result<()> {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&self.path)?);
        let len = self.pcm.len() as u32 * 2;
        f.write_all(b"RIFF")?;
        f.write_all(&(36 + len).to_le_bytes())?;
        f.write_all(b"WAVEfmt ")?;
        for v in [16u32.to_le_bytes(), [1, 0, 2, 0], RATE.to_le_bytes(), (RATE * 4).to_le_bytes(), [4, 0, 16, 0]] {
            f.write_all(&v)?;
        }
        f.write_all(b"data")?;
        f.write_all(&len.to_le_bytes())?;
        for s in &self.pcm {
            f.write_all(&s.to_le_bytes())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grinder_volumes() {
        // GRINDER frame 130 (tracer sounds.tsv): hero at x 0x..; volumes fall off with distance
        // and the far channel loses more.
        assert_eq!(point_volumes((160, 100), 0x100, 100, 160), Some((256, 256)));
        let (l, r) = point_volumes((160, 100), 0x100, 100, 200).unwrap();
        assert!(l < r);
        assert_eq!(point_volumes((0, 0), 0x100, 0, 1000), None);
    }
}
