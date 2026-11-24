//! The menus' words (MAPS/ETMENU<language>.SCR, read by FUN_1080_3719), the global messages
//! (ETMAIN.SCR's AddMsg lines, negative ids, FUN_1088_1171) and the settings the original keeps
//! in EatThis.ini, section [Eat This] (FUN_1038_03f5 reads, FUN_1038_012b writes).

use std::path::{Path, PathBuf};

fn read_lines(path: &Path) -> Vec<String> {
    std::fs::read(path)
        .map(|b| b.split(|&c| c == b'\n').map(|l| latin1(l).trim_end_matches('\r').to_string()).collect())
        .unwrap_or_default()
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

pub fn find_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    crate::find_ci(dir, name)
}

/// ETMENU<n>.SCR: the lines that are neither empty nor comments (FUN_1080_36be), in order.
pub struct MenuTexts {
    lines: Vec<String>,
    next: usize,
}

impl MenuTexts {
    pub fn load(data: &Path, language: i32) -> MenuTexts {
        let maps = data.join("MAPS");
        let path = find_ci(&maps, &format!("ETMENU{language}.SCR")).or_else(|| find_ci(&maps, "ETMENU0.SCR"));
        let lines = path
            .map(|p| read_lines(&p))
            .unwrap_or_default()
            .into_iter()
            .filter(|l| !l.is_empty() && !l.starts_with('\''))
            .collect();
        MenuTexts { lines, next: 0 }
    }

    pub fn next(&mut self) -> String {
        let s = self.lines.get(self.next).cloned().unwrap_or_default();
        self.next += 1;
        s
    }

    pub fn take(&mut self, n: usize) -> Vec<String> {
        (0..n).map(|_| self.next()).collect()
    }
}

/// `Key=value,language` lines of a script: the value for `language` (the last one wins), else
/// the first one.
pub fn localized(lines: &[(String, String)], key: &str, language: i32) -> Option<String> {
    let mut first = None;
    let mut found = None;
    for (k, v) in lines {
        if !k.eq_ignore_ascii_case(key) {
            continue;
        }
        let (text, lang) = match v.rsplit_once(',') {
            Some((t, l)) if l.trim().parse::<i32>().is_ok() => (t.to_string(), l.trim().parse::<i32>().unwrap()),
            _ => (v.clone(), -1),
        };
        first.get_or_insert(text.clone());
        if lang == language || lang == -1 {
            found = Some(text);
        }
    }
    found.or(first)
}

/// A script as `Key=value` pairs (lines starting with ' are comments; Latin-1).
pub fn script_pairs(path: &Path) -> Vec<(String, String)> {
    read_lines(path)
        .iter()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with('\'') {
                return None;
            }
            let (k, v) = l.split_once('=')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

/// ETMAIN.SCR: the global messages of one language (id -1 is the last) and the CD track titles.
pub struct MainScript {
    pub messages: Vec<String>,
    pub track_titles: Vec<String>,
}

impl MainScript {
    pub fn load(data: &Path, language: i32) -> MainScript {
        let pairs = find_ci(&data.join("MAPS"), "ETMAIN.SCR").map(|p| script_pairs(&p)).unwrap_or_default();
        let mut messages = Vec::new();
        let mut track_titles = Vec::new();
        for (k, v) in &pairs {
            if k.eq_ignore_ascii_case("AddMsg") {
                if let Some((t, l)) = v.rsplit_once(',') {
                    if l.trim().parse::<i32>().ok() == Some(language) {
                        // LoadMain: only the first 0x19 (the help pages') keep their case.
                        messages.push(if messages.len() < 0x19 { t.to_string() } else { t.to_ascii_uppercase() });
                    }
                }
            } else if k.eq_ignore_ascii_case("CDTrackTitle") {
                track_titles.push(v.clone());
            }
        }
        MainScript { messages, track_titles }
    }

    /// FUN_1088_1171 with a negative id.
    pub fn msg(&self, id: i32) -> String {
        let n = self.messages.len() as i32 + id;
        if n >= 0 { self.messages.get(n as usize).cloned().unwrap_or_default() } else { String::new() }
    }
}

/// EatThis.ini, [Eat This]. Defaults are the original's initial values.
#[derive(Clone, Debug)]
pub struct Settings {
    pub detail: i32,          // GameDetail (16d9)
    pub game_stats: i32,      // GameStats (1a92): the HUD, 0..2
    pub window: (i32, i32),   // WindowW/WindowH (1a6c/1a6e)
    pub display_mode: i32,    // DisplayMode (1a66): window, scanlines, full screen
    pub wait_retrace: i32,    // WaitForRetrace (1a8a)
    pub frame_rate: i32,      // FrameRate (1a62): auto, 70, 35, 23
    pub gamma: i32,           // GammaCorrection (1a88)
    pub buffer_length: i32,   // BufferLength (1ac2)
    pub volume: i32,          // Volume (1ac8), 0..0x100
    pub voices: i32,          // nVoices (1ac4)
    pub stereo: i32,          // Stereo (1ac6)
    pub language: i32,        // Language (1a96)
    pub keys: [i32; 7],       // Up Down Left Right LeftShoot RightShoot SwitchWeapon (VK codes)
    /// CD volume (1aa4); not in the file.
    pub cd_volume: i32,
}

const KEY_NAMES: [&str; 7] = ["Up", "Down", "Left", "Right", "LeftShoot", "RightShoot", "SwitchWeapon"];

/// The display sizes of the DISPLAY SIZE slider (1170:1a70).
pub const SIZES: [(i32, i32); 5] = [(320, 200), (400, 250), (480, 300), (560, 350), (640, 400)];

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            detail: 3,
            game_stats: 2,
            window: (640, 400),
            display_mode: 0,
            wait_retrace: 1,
            frame_rate: 0,
            gamma: 4,
            buffer_length: -1,
            volume: 0x100,
            voices: 16,
            stereo: 1,
            language: 0,
            keys: [0x26, 0x28, 0x25, 0x27, 0x2d, 0x20, 0x09],
            cd_volume: 0x100,
        }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        std::env::var_os("ET_INI").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("EatThis.ini"))
    }

    pub fn load() -> Settings {
        let mut s = Settings::default();
        let Ok(text) = std::fs::read(Self::path()) else { return s };
        let text = latin1(&text);
        let mut in_section = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_section = line.eq_ignore_ascii_case("[Eat This]");
                continue;
            }
            let Some((k, v)) = line.split_once('=').filter(|_| in_section) else { continue };
            let Ok(v) = v.trim().parse::<i32>() else { continue };
            let k = k.trim();
            let field: Option<&mut i32> = match k.to_ascii_lowercase().as_str() {
                "gamedetail" => Some(&mut s.detail),
                "gamestats" => Some(&mut s.game_stats),
                "windoww" => Some(&mut s.window.0),
                "windowh" => Some(&mut s.window.1),
                "displaymode" => Some(&mut s.display_mode),
                "waitforretrace" => Some(&mut s.wait_retrace),
                "framerate" => Some(&mut s.frame_rate),
                "gammacorrection" => Some(&mut s.gamma),
                "bufferlength" => Some(&mut s.buffer_length),
                "volume" => Some(&mut s.volume),
                "nvoices" => Some(&mut s.voices),
                "stereo" => Some(&mut s.stereo),
                "language" => Some(&mut s.language),
                _ => KEY_NAMES.iter().position(|n| n.eq_ignore_ascii_case(k)).map(|i| &mut s.keys[i]),
            };
            if let Some(f) = field {
                *f = v;
            }
        }
        // FUN_1038_03f5: display mode 3 or more becomes 0.
        if !(0..3).contains(&s.display_mode) {
            s.display_mode = 0;
        }
        s
    }

    pub fn save(&self) {
        let mut out = String::from("[Eat This]\r\n");
        let vals = [
            ("GameDetail", self.detail),
            ("GameStats", self.game_stats),
            ("WindowW", self.window.0),
            ("WindowH", self.window.1),
            ("DisplayMode", self.display_mode),
            ("WaitForRetrace", self.wait_retrace),
            ("FrameRate", self.frame_rate),
            ("GammaCorrection", self.gamma),
            ("BufferLength", self.buffer_length),
            ("Volume", self.volume),
            ("nVoices", self.voices),
            ("Stereo", self.stereo),
            ("Language", self.language),
        ];
        for (k, v) in vals {
            out += &format!("{k}={v}\r\n");
        }
        for (k, v) in KEY_NAMES.iter().zip(self.keys) {
            out += &format!("{k}={v}\r\n");
        }
        if let Err(e) = std::fs::write(Self::path(), out) {
            eprintln!("can't write {}: {e}", Self::path().display());
        }
    }
}
