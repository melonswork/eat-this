//! The menu objects of segment 1080 (re/frontend.md): a menu is a list of items placed one
//! under the other around (x, y); items are labels (VMT 1170:0f3c) or sliders (0f54). The
//! common update FUN_1080_1330 moves the selection (arrows, Home/End, the mouse over an item),
//! the selected item's update reports a click/Enter in `act` (1170:494a); each menu's own update
//! then acts on it.

use super::gfx::{DIGIT_FONT, Gfx, MENU_FONT, Mode};
use crate::sound;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Id {
    Main,     // 1170:472c, VMT 0f84
    Options,  // 475e, 0f9c
    Display,  // 47e6, 0fcc
    Sound,    // 4814, 0fb4
    UserMaps, // 478c, 0fe4
    Restart,  // 4844, 0ffc: "THIS WILL END YOUR CURRENT GAME"
    Quit,     // 4872, 1014
    Stats,    // 48a0, 102c
    Artwork,  // 48d2, 1044
    Episode,  // 4900, 105c
}

#[derive(Clone, Debug)]
pub enum Kind {
    Label,
    /// value +0x40, max +0x42, old value +0x44, step +0x46, page +0x48, lit knob +0x4a.
    Slider { value: i32, max: i32, old: i32, step: i32, page: i32, lit: bool },
}

#[derive(Clone, Debug)]
pub struct Item {
    pub text: String,
    pub kind: Kind,
    /// +2, +4: position in the menu.
    pub x: i32,
    pub y: i32,
    /// +6..+0xc: the mouse box around (x, y).
    pub bx: (i32, i32, i32, i32),
    /// +0x3e: where a slider's bar starts, from x.
    pub bar: i32,
}

impl Item {
    pub fn value(&self) -> i32 {
        match self.kind {
            Kind::Slider { value, .. } => value,
            Kind::Label => 0,
        }
    }

    pub fn changed(&self) -> bool {
        matches!(self.kind, Kind::Slider { value, old, .. } if value != old)
    }

    /// FUN_1080_0daa.
    pub fn set_value(&mut self, v: i32) {
        if let Kind::Slider { value, old, .. } = &mut self.kind {
            *value = v;
            *old = v;
        }
    }
}

/// Input as GameFrameMenu sees it. Keys are KeyPoll counters (-1 up, 1 pressed, 2 held, 0
/// released); mouse buttons 0 up, 1 pressed this frame, 2 held (1170:1a5c / 1a5e).
#[derive(Clone, Copy, Default, Debug)]
pub struct Input {
    pub mx: i32,
    pub my: i32,
    pub lmb: i32,
    pub rmb: i32,
    pub up: i8,
    pub down: i8,
    pub left: i8,
    pub right: i8,
    pub enter: i8,
    pub home: i8,
    pub end: i8,
    pub pgup: i8,
    pub pgdn: i8,
}

/// The globals the menu code shares.
pub struct Ui {
    pub input: Input,
    /// 1170:494a: the selected item was clicked / Enter.
    pub act: bool,
    /// 1170:494c: the mouse pointer sprite.
    pub cursor: i32,
    /// 1170:108c: key repeat countdown (FUN_1080_05be).
    pub repeat: i32,
    /// 1170:1086/1088: the box the held button went down in (0x7fff none); 108a this frame's.
    pub drag: (i32, i32),
    pub drag_next: i32,
}

impl Ui {
    pub fn new() -> Ui {
        Ui { input: Input::default(), act: false, cursor: 0x241e, repeat: 0, drag: (0x7fff, 0), drag_next: 0x7fff }
    }

    /// FUN_1080_05be: 1 when the key goes down (repeats start after 0x12 frames), 2 on a repeat
    /// (every `rate` frames), else 0.
    pub fn repeat(&mut self, rate: i32, key: i32) -> i32 {
        if key == 1 {
            self.repeat = 0x12;
            1
        } else if key < 1 || self.repeat != 0 {
            0
        } else {
            self.repeat = rate;
            2
        }
    }

    /// FUN_1080_053e: 1 when a button goes down in the box, 2 while it stays down after that.
    pub fn press(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        let i = self.input;
        if i.lmb < 2 && i.rmb < 2 {
            if (i.lmb == 1 || i.rmb == 1) && x1 <= i.mx && y1 <= i.my && i.mx <= x2 && i.my <= y2 {
                self.drag = (x1, y1);
                self.drag_next = x1;
                return 1;
            }
            0
        } else if self.drag == (x1, y1) {
            self.drag_next = x1;
            2
        } else {
            0
        }
    }

    pub fn clicked(&self) -> bool {
        self.input.lmb == 1 || self.input.rmb == 1 || self.input.enter == 1
    }
}

/// FUN_1080_08cf: the "chosen" sound at the pointer.
pub fn confirm_sound(ui: &Ui) {
    sound::play_at(0xa1, ui.input.my, ui.input.mx);
}

fn click_sound(ui: &Ui) {
    if ui.input.lmb == 1 || ui.input.rmb == 1 {
        sound::discard(0x96);
        sound::play_at(0x96, ui.input.my, ui.input.mx);
    }
}

pub struct Menu {
    pub id: Id,
    /// +0xe: the menu's x at rest; +0x10 the x it is drawn at (they differ while sliding).
    pub home_x: i32,
    pub x: i32,
    /// +0x12, +0x14: the first item's y and the distance between items.
    pub y: i32,
    pub spacing: i32,
    pub items: Vec<Item>,
    /// +0x16 selected (-1 none), +0x18 last selected, +0x1a requested by keys, +0x1c default.
    pub sel: i32,
    pub last: i32,
    pub next: Option<i32>,
    pub default: i32,
    /// +0x1e..+0x22: the pointer glides to the item a key selected (0x5a0 down to 0).
    pub glide: i32,
    pub glide_from: (i32, i32),
    /// +0x24: the menu GET BACK returns to (none: back to the game).
    pub parent: Option<Id>,
    /// +0x28..+0x2c: the title sprite (0x7fff none), its x offset and y.
    pub title: Option<(i32, i32, i32)>,
    /// +0x2e, +0x30: menu specific (the logo's countdown and sprite, the stats' clock, ...).
    pub a: i32,
    pub b: i32,
    /// FUN_1080_0824's 1084: boxes centred on x (else from x).
    pub centered: bool,
    /// User maps: +0x54 scroll, +0x56 its limit, +0x58 speed.
    pub scroll: i32,
    pub scroll_max: i32,
    pub scroll_speed: i32,
}

impl Menu {
    /// FUN_1080_0eed.
    pub fn new(id: Id, y: i32, x: i32) -> Menu {
        Menu {
            id,
            home_x: x,
            x,
            y,
            spacing: 0x11,
            items: Vec::new(),
            sel: -1,
            last: 0,
            next: None,
            default: 0,
            glide: 0,
            glide_from: (0, 0),
            parent: None,
            title: None,
            a: 0,
            b: 0x7fff,
            centered: true,
            scroll: 0,
            scroll_max: 0,
            scroll_speed: 0,
        }
    }

    /// FUN_1080_08e4/0824 + FUN_1080_0f99: a label under the last item. `half` is 1082.
    pub fn label(&mut self, gfx: &mut Gfx, text: &str, half: i32) {
        let w = gfx.text_width(&MENU_FONT, text.as_bytes());
        let bx = if self.centered {
            let h = (w >> 1) + 0x16;
            (-h, -half, h, half)
        } else {
            (-0x16, -half, w + 0x16, half)
        };
        let y = match self.items.last() {
            Some(l) => l.y + self.spacing,
            None => 0,
        };
        self.items.push(Item { text: text.to_string(), kind: Kind::Label, x: 0, y, bx, bar: 0 });
    }

    /// FUN_1080_0981 + 0f99 + 0d81: a slider (its box 0x35 wider each side, -7..6 high).
    pub fn slider(&mut self, gfx: &mut Gfx, text: &str, value: i32, max: i32, step: i32, page: i32) {
        self.label(gfx, text, 6);
        let it = self.items.last_mut().unwrap();
        it.kind = Kind::Slider { value, max, old: value, step, page, lit: false };
        it.bx = (it.bx.0 - 0x35, -7, it.bx.2 + 0x35, 6);
        it.bar = it.bx.2 - 99;
    }

    /// FUN_1080_1010: item `b` moves sideways so its bar lines up with item `a`'s.
    pub fn align(&mut self, a: usize, b: usize) {
        let x = self.items[a].bar + self.items[a].x - self.items[b].bar;
        self.items[b].x = x;
    }

    /// FUN_1080_0824 on an existing item: new text, new box.
    pub fn retext(&mut self, gfx: &mut Gfx, i: usize, text: &str) {
        let w = gfx.text_width(&MENU_FONT, text.as_bytes());
        let it = &mut self.items[i];
        it.text = text.to_string();
        let half = it.bx.3;
        it.bx = if self.centered {
            let h = (w >> 1) + 0x16;
            (-h, -half, h, half)
        } else {
            (-0x16, -half, w + 0x16, half)
        };
    }

    /// FUN_1080_11e2: select item `i` and glide the pointer there.
    pub fn select(&mut self, i: i32, ui: &Ui) {
        self.sel = i;
        self.last = i;
        self.glide = 0x5a0;
        self.glide_from = (ui.input.mx, ui.input.my);
    }

    /// FUN_1080_120f: while gliding, the pointer goes from where it was to the item's centre.
    pub fn glide_pointer(&mut self, ui: &Ui) -> Option<(i32, i32)> {
        if ui.drag.0 != 0x7fff {
            return None;
        }
        if self.sel < 0 {
            self.glide = 0;
        }
        if self.glide <= 0 {
            return None;
        }
        self.glide -= 0x50;
        let it = self.items.get(self.sel as usize)?;
        // The item's right end (less 4) with the menu at rest, eased by sin.
        let tx = self.home_x + it.x + it.bx.2 - 4;
        let ty = self.y + it.y;
        let (fx, fy) = self.glide_from;
        let k = crate::trig::sin(0x5a0 - self.glide);
        Some((fx + (tx - fx) * k / 0x4000, fy + (ty - fy) * k / 0x4000))
    }

    /// FUN_1080_1330. Returns true when the title was clicked (GET BACK).
    pub fn update_common(&mut self, gfx: &mut Gfx, ui: &mut Ui) -> bool {
        ui.act = false;
        let i = ui.input;
        if ui.drag.0 == 0x7fff {
            if ui.repeat(8, i.up as i32) > 0 {
                self.next = Some(self.last - 1);
            }
            if ui.repeat(8, i.down as i32) > 0 {
                self.next = Some(self.last + 1);
            }
            if ui.repeat(8, i.home as i32) > 0 {
                self.next = Some(0);
            }
            if ui.repeat(8, i.end as i32) > 0 {
                self.next = Some(-1);
            }
            if self.glide < 1 {
                if self.sel != -1 {
                    self.last = self.sel;
                    self.sel = -1;
                }
                for (k, it) in self.items.iter().enumerate().rev() {
                    let top = self.y + it.y;
                    if top + it.bx.1 <= gfx.clip.3 - 2
                        && gfx.clip.1 + 2 <= top + it.bx.3
                        && self.x + it.x + it.bx.0 <= i.mx
                        && top + it.bx.1 <= i.my
                        && i.mx <= self.x + it.x + it.bx.2
                        && i.my <= top + it.bx.3
                    {
                        self.sel = k as i32;
                    }
                }
            }
            if let Some(mut n) = self.next {
                let count = self.items.len() as i32;
                if n < 0 {
                    n = count - 1;
                } else if n >= count {
                    n = 0;
                }
                if n != self.sel {
                    self.select(n, ui);
                    // 1ab8 = 0x80 around it: the move sound at half volume, a random pitch.
                    let p = gfx.rng.random(0x14);
                    sound::pitch(0x100 - p);
                    sound::discard(0x98);
                    let it = &self.items[n as usize];
                    sound::play_at_vol(0x98, 0x80, self.y + it.y, self.x + it.x);
                }
            }
        }
        let mut back = false;
        if self.sel < 0 {
            if let Some((id, dx, y)) = self.title {
                let r = gfx.rect(id, self.x + dx, y);
                if ui.press(r.0, r.1, r.2, r.3) > 0 {
                    back = true;
                }
            }
        } else {
            let (mx, my) = (self.x, self.y);
            let sel = self.sel as usize;
            let it = &mut self.items[sel];
            match &mut it.kind {
                Kind::Label => {
                    // FUN_1080_095c.
                    if ui.clicked() {
                        ui.act = true;
                        confirm_sound(ui);
                    }
                }
                Kind::Slider { .. } => slider_update(it, ui, mx, my),
            }
        }
        self.next = None;
        back
    }

    /// FUN_1080_1064: the title (on a darkened band) and the items, the selected one lit.
    pub fn draw_items(&mut self, gfx: &mut Gfx) {
        for k in (0..self.items.len()).rev() {
            let it = self.items[k].clone();
            let selected = k as i32 == self.sel;
            let (x, y) = (self.x + it.x, self.y + it.y);
            let tx = x + it.bx.0 + 0x16;
            if selected {
                gfx.glow_text(&MENU_FONT, it.text.as_bytes(), tx, y + 3);
            } else {
                gfx.text(&MENU_FONT, it.text.as_bytes(), tx, y + 3, Mode::Copy);
            }
            if let Kind::Slider { value, max, lit, .. } = it.kind {
                // FUN_1080_09fe: the lit part of the bar left of the knob, the dark part right.
                let (bx, by) = (x + it.bar, y + 6);
                let knob = bx + 8 + if max > 0 { value * 0x2a / max } else { 0 };
                let clip = gfx.clip;
                gfx.clip.2 = (knob - 8).min(clip.2);
                gfx.sprite(0x2402, bx, by, Mode::Copy);
                gfx.clip.2 = clip.2;
                gfx.clip.0 = (knob + 8).max(clip.0);
                gfx.sprite(0x2403, bx, by, Mode::Copy);
                gfx.clip.0 = clip.0;
                gfx.sprite(if lit { 0x2406 } else { 0x2404 }, knob, by, Mode::Copy);
                if let Kind::Slider { lit, .. } = &mut self.items[k].kind {
                    *lit = false;
                }
            }
        }
        if let Some((id, dx, y)) = self.title {
            let half = (gfx.width(id) >> 1) - 0x12;
            let x = self.x + dx;
            gfx.shade(7, x - half, y - 7, x + half, y + 7);
            gfx.sprite(id, x, y, Mode::Copy);
        }
    }
}

/// FUN_1080_0b5f: arrows/PgUp/PgDn change the value, the mouse drags the knob or clicks the
/// ends of the bar.
fn slider_update(it: &mut Item, ui: &mut Ui, mx: i32, my: i32) {
    let sx = mx + it.x + it.bar;
    let sy = my + it.y;
    let i = ui.input;
    let Kind::Slider { value, max, old, step, page, lit } = &mut it.kind else { return };
    *old = *value;
    let key = |ui: &mut Ui, k: i8| {
        let r = ui.repeat(3, k as i32);
        if r == 1 {
            sound::discard(0x96);
            sound::play_at(0x96, sy, sx);
        }
        r > 0
    };
    if key(ui, i.left) {
        *value -= *step;
    }
    if key(ui, i.right) {
        *value += *step;
    }
    if key(ui, i.pgup) {
        *value -= *page;
    }
    if key(ui, i.pgdn) {
        *value += *page;
    }
    if ui.press(sx, sy - 7, sx + 0x3a, sy + 6) > 0 {
        if i.lmb > 0 || i.rmb == 1 {
            *value = ((i.mx - sx - 8) * (*max + 1) - 1) / 0x2a;
            click_sound(ui);
            *lit = true;
        }
        if i.rmb > 0 {
            *lit = true;
        }
    } else if ui.press(sx - 0x13, sy - 7, sx - 1, sy + 6) > 0 {
        if ui.repeat(3, i.lmb) > 0 {
            *value -= *step;
        }
        if ui.repeat(3, i.rmb) > 0 {
            *value -= *page;
        }
        click_sound(ui);
    } else if ui.press(sx + 0x3b, sy - 7, sx + 0x4d, sy + 6) > 0 {
        if ui.repeat(3, i.lmb) > 0 {
            *value += *step;
        }
        if ui.repeat(3, i.rmb) > 0 {
            *value += *page;
        }
        click_sound(ui);
    } else if ui.clicked() {
        ui.act = true;
    }
    *value = (*value).clamp(0, (*max).max(0));
}

/// The stats counters' "value/total" in the digit font (FUN_1080_1b38), ending at x.
pub fn draw_count(gfx: &mut Gfx, value: i32, total: i32, x: i32, y: i32) {
    let s = format!("{value}/{total}");
    gfx.text_right(&DIGIT_FONT, s.as_bytes(), x, y, Mode::Copy);
}
