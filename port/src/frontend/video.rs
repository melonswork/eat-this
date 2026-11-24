//! Playing VIDEOS/*.AVI (FUN_1008_1365, the MCI "AVIVideo" device, alias AVI320120).
//!
//! Two ways, as the original's key handler FUN_1008_1966 has them:
//! - `Play` (1a64 = 3, "Play ... Notify"): runs at the file's rate with its sound; Escape, Space
//!   or Return stop it ("Stop AVI320120"), and the end notification runs what comes next.
//! - `Pages` (1a64 = 4: help 1a98 = 2, credits 1a98 = 1): one still frame at a time; Space,
//!   Return, Right, F1, numpad 6 the next one, Left or numpad 4 the previous one (both wrap),
//!   Escape leaves (FUN_1008_16b9).

use super::avi::Avi;
use super::helpfont::GLYPHS;
use crate::sound::Ev;
use sdl2::keyboard::Scancode;
use std::path::{Path, PathBuf};

/// The mixer voice of a video's sound (outside the game's voice handles).
const VOICE: u32 = 0xfff0_0000;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Play,
    Pages,
}

/// What the end of the video starts (the end notification's callback 1170:10fa).
#[derive(Clone, Debug)]
pub enum Then {
    /// Back to the menu shown before (intro at boot: 1080:17be, nothing to play in the menus).
    Menu,
    /// FUN_1080_16b7: the loaded level starts (its music).
    #[allow(dead_code)]
    Level(PathBuf),
    /// FUN_1080_172a: the stats screen.
    Stats,
}

pub struct Player {
    avi: Avi,
    pub kind: Kind,
    pub then: Then,
    ticks: u32,
    /// The frame shown (Pages: the page).
    page: usize,
    pub done: bool,
    started_sound: bool,
    /// The help pages' texts (none: credits, 1a98 = 1).
    pub help: Option<HelpTexts>,
}

/// The texts FUN_1008_1073 writes over ETHELP.AVI's pages (messages -0x25..-0x3e, by -id - 0x25).
pub struct HelpTexts(pub Vec<String>);

/// Page 0's labels: (message id, SetTextAlign, x, y) as FUN_1008_1073 gives them.
const KEYBOARD: [(i32, i32, i32, i32); 8] = [
    (-0x25, TA_LEFT, 0x58, 0x4d),
    (-0x26, TA_LEFT, 0xd5, 0x74),
    (-0x27, TA_RIGHT, 0x16a, 0x86),
    (-0x28, TA_RIGHT, 0x15e, 0x98),
    (-0x29, TA_RIGHT, 0x2ab, 0x8c),
    (-0x2a, TA_RIGHT, 0x279, 0x9d),
    (-0x2c, TA_RIGHT, 0x2bc, 0x1ca),
    (-0x2b, TA_LEFT, 0x1ce, 0x1ee),
];
const TA_LEFT: i32 = 0;
const TA_RIGHT: i32 = 2;
const TA_CENTER: i32 = 6;
/// SetTextColor(0xd0d0d0), transparent background.
const TEXT_COLOUR: u8 = 0xd0;
/// The main window is the screen plus 64 pixels on every side (CreateWindowEx at -64, -64, size +128):
/// the video's place in it, ((client - 640) / 2, (client - 480) / 2), is the text's origin + 64.
const MARGIN: i32 = 64;

impl HelpTexts {
    pub const FIRST: i32 = -0x25;
    pub const LAST: i32 = -0x2c - 18;

    fn msg(&self, id: i32) -> &str {
        self.0.get((Self::FIRST - id) as usize).map_or("", |s| s)
    }

    fn draw(&self, page: usize, out: &mut [u8]) {
        if page == 0 {
            for (id, align, x, y) in KEYBOARD {
                text_out(out, self.msg(id), align, x - MARGIN, y - MARGIN);
            }
        } else {
            text_out(out, self.msg(-0x2c - page as i32), TA_CENTER, 0x181 - MARGIN, 0x1b8 - MARGIN);
        }
    }
}

/// TextOut in Arial bold, cell height 16 (`helpfont`), clipped to the 640x480 picture.
fn text_out(out: &mut [u8], text: &str, align: i32, x: i32, y: i32) {
    let glyph = |c: char| &GLYPHS[(c as usize).checked_sub(0x20).filter(|&i| i < GLYPHS.len()).unwrap_or(0)];
    let width: i32 = text.chars().map(|c| glyph(c).0 as i32).sum();
    let mut x = match align {
        TA_RIGHT => x - width,
        TA_CENTER => x - width / 2,
        _ => x,
    };
    let (w, h) = (SIZE.0 as i32, SIZE.1 as i32);
    for c in text.chars() {
        let (advance, left, rows) = glyph(c);
        for (r, bits) in rows.iter().enumerate() {
            for b in (0..16).filter(|b| bits & (0x8000 >> b) != 0) {
                let (px, py) = (x + *left as i32 + b, y + r as i32);
                if (0..w).contains(&px) && (0..h).contains(&py) {
                    let at = ((py * w + px) * 3) as usize;
                    out[at..at + 3].fill(TEXT_COLOUR);
                }
            }
        }
        x += *advance as i32;
    }
}

impl Player {
    /// VIDEOS/<name>, or None if it is missing or broken (FUN_1100_0131 fails: the callback
    /// runs at once).
    pub fn open(data: &Path, name: &str, kind: Kind, then: Then) -> Option<Player> {
        let path = super::texts::find_ci(&data.join("VIDEOS"), name)?;
        let mut avi = match Avi::open(&path) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("video {name}: {e}");
                return None;
            }
        };
        if let Err(e) = avi.seek(0) {
            eprintln!("video {name}: {e}");
            return None;
        }
        Some(Player { avi, kind, then, ticks: 0, page: 0, done: false, started_sound: false, help: None })
    }

    /// One 70 Hz frame.
    pub fn tick(&mut self, evs: &mut Vec<Ev>) {
        if self.done || self.kind == Kind::Pages {
            return;
        }
        if !self.started_sound {
            self.started_sound = true;
            if let Some(s) = self.avi.audio.clone() {
                evs.push(Ev::Pcm { voice: VOICE, sample: s });
            }
        }
        self.ticks += 1;
        let us = self.ticks as u64 * 1_000_000 / crate::HZ as u64;
        let n = (us / self.avi.usec_per_frame as u64) as usize;
        if n >= self.avi.frame_count() {
            self.done = true;
            return;
        }
        if let Err(e) = self.avi.seek(n) {
            eprintln!("video: {e}");
            self.done = true;
        }
    }

    /// A key going down while the video shows (FUN_1008_1966, 1a64 = 3 or 4).
    pub fn key(&mut self, sc: Scancode, evs: &mut Vec<Ev>) {
        match self.kind {
            Kind::Play => {
                if matches!(sc, Scancode::Escape | Scancode::Space | Scancode::Return | Scancode::KpEnter) {
                    self.stop(evs);
                }
            }
            Kind::Pages => {
                let n = self.avi.frame_count().max(1);
                let page = match sc {
                    Scancode::Escape => {
                        self.done = true;
                        return;
                    }
                    Scancode::Space | Scancode::Return | Scancode::KpEnter | Scancode::Right | Scancode::F1 | Scancode::Kp6 => {
                        (self.page + 1) % n
                    }
                    Scancode::Left | Scancode::Kp4 => (self.page + n - 1) % n,
                    _ => return,
                };
                self.page = page;
                if let Err(e) = self.avi.seek(page) {
                    eprintln!("video: {e}");
                    self.done = true;
                }
            }
        }
    }

    pub fn stop(&mut self, evs: &mut Vec<Ev>) {
        self.done = true;
        evs.push(Ev::Stop { voice: VOICE });
    }

    /// The picture as RGB24 at 640x480 (a 640x240 video line-doubled, centred like
    /// "Put AVI320120 Destination").
    pub fn render(&self, out: &mut [u8]) {
        const OW: usize = 640;
        const OH: usize = 480;
        out.fill(0);
        let (w, h) = self.avi.size();
        if !self.avi.have_frame() || w == 0 || w > OW || h > OH {
            return;
        }
        let mut pic = vec![0u8; w * h * 3];
        self.avi.rgb(&mut pic);
        let rep = if h * 2 <= OH { 2 } else { 1 };
        let (x0, y0) = ((OW - w) / 2, (OH - h * rep) / 2);
        for y in 0..h * rep {
            let src = &pic[(y / rep) * w * 3..(y / rep + 1) * w * 3];
            let at = ((y0 + y) * OW + x0) * 3;
            out[at..at + w * 3].copy_from_slice(src);
        }
        if let Some(t) = &self.help {
            t.draw(self.page, out);
        }
    }
}

/// The video a level script names (`Video=NAME.avi,N`, collection 4b5e; FUN_1080_161b takes
/// the last entry with that N).
pub fn script_video(pairs: &[(String, String)], n: i32) -> Option<String> {
    pairs.iter().rev().filter(|(k, _)| k.eq_ignore_ascii_case("Video")).find_map(|(_, v)| {
        let (name, flag) = v.split_once(',')?;
        (flag.trim().parse::<i32>().ok()? == n).then(|| name.trim().to_string())
    })
}

pub const SIZE: (u32, u32) = (640, 480);
