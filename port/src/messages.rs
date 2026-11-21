//! The message box at the top of the screen (FUN_1088_0fa4 adds a message, FUN_1088_4b64 types
//! it out and scrolls the lines away, FUN_1088_0f2e drops the first line). The messages are
//! ETMAIN.SCR's `AddMsg=` lines of the language (`HeroDamage(id)` = FUN_1088_1171(id): id -1 is
//! the last). Markup: `^|` new line, `^Cxx` tint xx (hex), `^O` plain again, `^Bxx` blink.
//! Only the HUD reads this; nothing in the simulation does.

use std::sync::atomic::{AtomicI32, Ordering};

/// The language of the messages (EatThis.ini's Language; the front end sets it).
pub static LANGUAGE: AtomicI32 = AtomicI32::new(0);

thread_local! {
    /// The level's map (its script's `AddMsg=` lines follow ETMAIN's: ids 0, 1, ...).
    static LEVEL: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// The map being loaded (main's `load_world`).
pub fn set_level(map: &std::path::Path) {
    LEVEL.with(|l| *l.borrow_mut() = Some(map.to_path_buf()));
}

/// The line separator FUN_1088_0fa4 puts between two messages (1170:1704).
const NEW_LINE: &[u8] = b"^|";

#[derive(Default, Clone)]
pub struct MessageBox {
    /// 1170:4f10: the lines still shown, `^|` between them.
    pub text: Vec<u8>,
    /// 5110: how many bytes are typed out so far.
    pub shown: i32,
    /// 5112: the number of lines.
    pub lines: i32,
    /// 5114: frames left before the first line scrolls away (0: no box).
    pub timer: i32,
    /// The state FUN_1088_4b64 drew this frame (before its update): text, shown, lines, timer.
    pub drawn: (Vec<u8>, i32, i32, i32),
    /// The messages, and how many of them are ETMAIN's (5116).
    texts: Option<(Vec<String>, i32)>,
}

impl MessageBox {
    /// HeroDamage(id) (1088:1109): message `id` of ETMAIN (negative, -1 = the last) or, from 0
    /// on, of the level's script (script command 0x0b).
    /// With `unless_last` (FUN_1088_10e1), not when it is already the last line.
    pub fn show_id(&mut self, id: i32, unless_last: bool) {
        let (texts, main) = self.texts.get_or_insert_with(|| {
            let data = crate::sprites::DIR.get().and_then(|d| d.parent()).map(|d| d.to_path_buf()).unwrap_or_default();
            let lang = LANGUAGE.load(Ordering::Relaxed);
            let mut v = crate::frontend::texts::MainScript::load(&data, lang).messages;
            let main = v.len() as i32;
            if let Some(map) = LEVEL.with(|l| l.borrow().clone()) {
                for (k, val) in crate::script_lines(&map) {
                    if let Some((t, l)) = val.rsplit_once(',') {
                        if k.eq_ignore_ascii_case("AddMsg") && l.trim().parse::<i32>().ok() == Some(lang) {
                            v.push(t.to_string());
                        }
                    }
                }
            }
            (v, main)
        });
        let n = *main + id;
        if let Some(t) = (n >= 0).then(|| texts.get(n as usize)).flatten() {
            let t = t.clone().into_bytes();
            let last = match self.text.windows(2).rposition(|w| w == NEW_LINE) {
                Some(at) => &self.text[at + 2..],
                None => &self.text[..],
            };
            if !(unless_last && last == t.as_slice()) {
                self.show(&t);
            }
        }
    }

    /// FUN_1088_0fa4: add a message (a new box, or a new line under the others).
    pub fn show(&mut self, msg: &[u8]) {
        if self.text.is_empty() {
            self.timer = 0xd2;
            self.shown = 0;
        } else {
            if self.lines > 2 && self.timer > 0xd {
                self.timer = 0xd;
            }
            while !self.text.is_empty() && self.text.len() + msg.len() >= 0x1fd {
                self.drop_line();
            }
            if !self.text.is_empty() {
                self.text.extend_from_slice(NEW_LINE);
            }
        }
        self.text.extend_from_slice(msg);
        self.shown = skip_markup(&self.text, self.shown);
        self.lines += 1 + count(msg, NEW_LINE);
    }

    /// FUN_1088_0f2e: the first line goes.
    fn drop_line(&mut self) {
        match find(&self.text, NEW_LINE) {
            None => self.text.clear(),
            Some(at) => {
                self.shown = (self.shown - (at as i32 + 2)).max(0);
                self.text.drain(..at + 2);
                self.timer = if self.lines < 5 { 0x69 } else { 0xd };
            }
        }
        self.lines -= 1;
    }

    /// The update half of FUN_1088_4b64's message box, once a frame (after `drawn` is kept).
    pub fn frame(&mut self) {
        self.drawn = (self.text.clone(), self.shown, self.lines, self.timer);
        if self.timer <= 0 {
            return;
        }
        if (self.shown as usize) < self.text.len() {
            self.shown += 1;
            self.shown = skip_markup(&self.text, self.shown);
        }
        self.timer -= 1;
        if self.timer == 4 {
            self.drop_line();
        }
    }
}

/// FUN_1028_01f8: the index past the markup codes at `i`.
pub fn skip_markup(t: &[u8], mut i: i32) -> i32 {
    while t.get(i as usize) == Some(&b'^') {
        match t.get(i as usize + 1) {
            Some(b'|') | Some(b'O') => i += 2,
            Some(b'C') | Some(b'B') => i += 4,
            _ => break,
        }
    }
    i
}

fn find(t: &[u8], pat: &[u8]) -> Option<usize> {
    t.windows(pat.len()).position(|w| w == pat)
}

/// FUN_1028_0241 minus one: the line breaks in `t`.
fn count(t: &[u8], pat: &[u8]) -> i32 {
    t.windows(pat.len()).filter(|w| *w == pat).count() as i32
}
