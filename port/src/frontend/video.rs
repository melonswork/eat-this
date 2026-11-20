//! Playing VIDEOS/*.AVI (FUN_1008_1365, the MCI "AVIVideo" device, alias AVI320120).
//!
//! Two ways, as the original's key handler FUN_1008_1966 has them:
//! - `Play` (1a64 = 3, "Play ... Notify"): runs at the file's rate with its sound; Escape, Space
//!   or Return stop it ("Stop AVI320120"), and the end notification runs what comes next.
//! - `Pages` (1a64 = 4: help 1a98 = 2, credits 1a98 = 1): one still frame at a time; Space,
//!   Return, Right, F1, numpad 6 the next one, Left or numpad 4 the previous one (both wrap),
//!   Escape leaves (FUN_1008_16b9).

use super::avi::Avi;
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
        Some(Player { avi, kind, then, ticks: 0, page: 0, done: false, started_sound: false })
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
