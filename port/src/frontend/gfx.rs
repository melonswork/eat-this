//! The menu screens' drawing (re/frontend.md): the 64000-byte background buffer (1170:4720)
//! with the shimmer FUN_1080_002a adds every frame, the menu sprite groups, the tint table rows
//! the menus compute (FUN_1080_336d) and the text routines of segment 1028.

use crate::data::SpriteGroup;
use crate::palette::{LevelPalette, Pal6};
use crate::rng::PascalRandom;
use std::collections::HashMap;
use std::path::Path;

pub const W: i32 = 320;
pub const H: i32 = 200;

/// Sprite slots (slot * 256 + frame) and their files (FUN_1080_336d, FUN_1080_172a).
pub const FONT: u16 = 0x23; // PPFONT3, fitted (FUN_1088_454d)
pub const MENU: u16 = 0x24; // MENUSL: logo, sliders, titles, cursors
pub const DIGITS: u16 = 0x27; // ETCIFER3
pub const STATS: u16 = 0x28; // STATSOBJ, fitted
pub const EPISODES: u16 = 0x29; // EPISPIC
pub const EXIT: u16 = 0x2a; // EXITPIC

fn slot_file(slot: u16) -> Option<(&'static str, bool)> {
    Some(match slot {
        FONT => ("PPFONT3", true),
        MENU => ("MENUSL", false),
        DIGITS => ("ETCIFER3", false),
        STATS => ("STATSOBJ", true),
        EPISODES => ("EPISPIC", false),
        EXIT => ("EXITPIC", false),
        _ => return None,
    })
}

/// How a sprite is drawn (1ae0 with the menu's MIX = the tint table, 1b0c = 4c26).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// 4: copy.
    Copy,
    /// 0x2c: tint[row][src].
    Tint(u8),
    /// 0x38: tint[row][dst] where the sprite has pixels (the selected item's glow).
    Glow(u8),
}

pub struct Gfx {
    pub fb: Vec<u8>,
    /// 1170:4720: what the menus are drawn over (a picture, or the game's last frame).
    pub back: Vec<u8>,
    pub palette: Pal6,
    /// The tint table 4c26 as the menus use it: rows 5..9.
    pub tint: Vec<u8>,
    /// 1170:494e: 512 values Random(8) drawn with RandSeed 0 (FUN_1080_336d).
    noise: Vec<u8>,
    /// The menus' own Random state (RandSeed swapped at 1170:4718/471c around the game's).
    pub rng: PascalRandom,
    /// Clip rectangle (left, top, right, bottom), inclusive (1170:1aca..1ad0).
    pub clip: (i32, i32, i32, i32),
    dir: std::path::PathBuf,
    groups: HashMap<u16, Option<SpriteGroup>>,
}

pub const SCREEN: (i32, i32, i32, i32) = (0, 0, W - 1, H - 1);

impl Gfx {
    pub fn new(sprites: &Path, palette: Pal6) -> Gfx {
        let mut rng = PascalRandom::new(0);
        let mut noise = vec![0u8; 0x200];
        for i in (0..0x200).rev() {
            noise[i] = rng.random(8) as u8;
        }
        let mut g = Gfx {
            fb: vec![0; (W * H) as usize],
            back: vec![0; (W * H) as usize],
            palette,
            tint: Vec::new(),
            noise,
            rng,
            clip: SCREEN,
            dir: sprites.to_path_buf(),
            groups: HashMap::new(),
        };
        g.set_palette(palette);
        g
    }

    /// A new menu palette: the tint rows are computed again and fitted groups reload.
    pub fn set_palette(&mut self, palette: Pal6) {
        self.palette = palette;
        self.tint = menu_tints(&palette);
        self.groups.clear();
    }

    /// FUN_1080_002a: the screen is the background plus 4-byte words of the noise table, each
    /// row of 256 bytes from a random place in it (32-bit adds: carries cross into the next byte).
    pub fn shimmer(&mut self) {
        for row in 0..250usize {
            let off = self.rng.random(0x101) as usize;
            for i in 0..64 {
                let at = row * 256 + i * 4;
                let b = u32::from_le_bytes(self.back[at..at + 4].try_into().unwrap());
                let n = u32::from_le_bytes(self.noise[off + i * 4..off + i * 4 + 4].try_into().unwrap());
                self.fb[at..at + 4].copy_from_slice(&b.wrapping_add(n).to_le_bytes());
            }
        }
    }

    fn group(&mut self, slot: u16) -> Option<&SpriteGroup> {
        let palette = self.palette;
        let dir = self.dir.clone();
        self.groups
            .entry(slot)
            .or_insert_with(|| {
                let (name, fit) = slot_file(slot)?;
                let mut g = SpriteGroup::load(&dir.join(format!("{name}.SPR"))).ok()?;
                if fit {
                    // FUN_10f0_1a99 with 1ae2 = 0xff: every colour may be used.
                    g.boundary = 255;
                    g.fit_to(&LevelPalette { colors: palette, foreground: 255 });
                }
                Some(g)
            })
            .as_ref()
    }

    fn with<R>(&mut self, id: i32, f: impl FnOnce(&crate::data::Sprite) -> R) -> Option<R> {
        let g = self.group((id >> 8) as u16)?;
        Some(f(g.sprites.get((id & 0xff) as usize)?.as_ref()?))
    }

    pub fn width(&mut self, id: i32) -> i32 {
        self.with(id, |s| s.width).unwrap_or(0)
    }

    pub fn height(&mut self, id: i32) -> i32 {
        self.with(id, |s| s.height).unwrap_or(0)
    }

    /// Whether sprite `id` exists (a font has glyphs only for some characters).
    pub fn has(&mut self, id: i32) -> bool {
        self.with(id, |_| ()).is_some()
    }

    /// The sprite's rectangle on screen when drawn at (x, y) (FUN_10f0_1fbe).
    pub fn rect(&mut self, id: i32, x: i32, y: i32) -> (i32, i32, i32, i32) {
        self.with(id, |s| (x + s.hotspot.0, y + s.hotspot.1, x + s.hotspot.0 + s.width - 1, y + s.hotspot.1 + s.height - 1))
            .unwrap_or((x, y, x, y))
    }

    /// FUN_10f0_2670: sprite `id` with its hotspot at (x, y).
    pub fn sprite(&mut self, id: i32, x: i32, y: i32, mode: Mode) {
        let clip = self.clip;
        let tint = std::mem::take(&mut self.tint);
        let mut fb = std::mem::take(&mut self.fb);
        self.with(id, |s| {
            let (left, top) = (x + s.hotspot.0, y + s.hotspot.1);
            for row in 0..s.height {
                let sy = top + row;
                if sy < clip.1 || sy > clip.3 {
                    continue;
                }
                for col in 0..s.width {
                    let sx = left + col;
                    if sx < clip.0 || sx > clip.2 {
                        continue;
                    }
                    let p = s.pixels[(row * s.width + col) as usize];
                    if p == 0 {
                        continue;
                    }
                    let d = &mut fb[(sy * W + sx) as usize];
                    *d = match mode {
                        Mode::Copy => p,
                        Mode::Tint(r) => tint[r as usize * 256 + p as usize],
                        Mode::Glow(r) => tint[r as usize * 256 + *d as usize],
                    };
                }
            }
        });
        self.fb = fb;
        self.tint = tint;
    }

    /// FUN_10f0_0113 with flags 0x11: the rectangle through tint row `row` (row 7 darkens).
    pub fn shade(&mut self, row: u8, x1: i32, y1: i32, x2: i32, y2: i32) {
        let (x1, y1) = (x1.max(self.clip.0), y1.max(self.clip.1));
        let (x2, y2) = (x2.min(self.clip.2), y2.min(self.clip.3));
        for y in y1..=y2 {
            for x in x1..=x2 {
                let d = &mut self.fb[(y * W + x) as usize];
                *d = self.tint[row as usize * 256 + *d as usize];
            }
        }
    }
}

/// FUN_1080_336d: tint rows 6..9 from the menu palette. 6: a light yellow of the colour's
/// brightness l (the selected item), 7: three steps darker, 8: orange red (^C08), 9: brighter.
/// The ramps 0xc0..0xcf, 0xd0..0xef and 0xf0..0xfe move three entries within the ramp instead.
fn menu_tints(pal: &Pal6) -> Vec<u8> {
    let lp = LevelPalette { colors: *pal, foreground: 255 };
    let mut t: Vec<u8> = (0..10).flat_map(|_| 0..=255u8).collect();
    let c6 = |r: i32, g: i32, b: i32| [r, g, b].map(|v| v.clamp(0, 63) as u8);
    for c in 0..256usize {
        let [r, g, b] = pal[c].map(i32::from);
        let l = (r * 0x12b + g * 0x24b + b * 0x72) / 1000;
        t[0x600 + c] = lp.nearest(c6(l * 20 / 9, l * 20 / 9, l * 4 / 3), 1..=255);
        let ramp = match c {
            0xc0..=0xcf => Some((0xc0, 0xcf)),
            0xd0..=0xef => Some((0xd0, 0xef)),
            0xf0..=0xfe => Some((0xf0, 0xfe)),
            _ => None,
        };
        match ramp {
            Some((lo, hi)) => {
                t[0x700 + c] = (c as i32 - 3).max(lo) as u8;
                t[0x900 + c] = (c as i32 + 3).min(hi) as u8;
            }
            None => {
                t[0x700 + c] = lp.nearest(c6(r - 3, g - 3, b - 3), 1..=255);
                t[0x900 + c] = lp.nearest(c6(r + 5, g + 3, b + 5), 1..=255);
            }
        }
        t[0x800 + c] = lp.nearest(c6(l * 2, l / 3, 0), 1..=255);
    }
    t
}

/// Row 5 of the level's tint table (1088:d0d6): min(l / 2, 20) - 0x30, i.e. 0xd0.. of the menu
/// palette. FUN_1080_0002 passes the game's last frame through it when the menu opens over it.
pub fn grey_row(level: &[[u8; 3]; 256]) -> [u8; 256] {
    let mut t = [0u8; 256];
    for (c, o) in t.iter_mut().enumerate() {
        let [r, g, b] = level[c].map(i32::from);
        let l = (r * 0x12b + g * 0x24b + b * 0x72) / 1000;
        *o = ((l / 2).min(0x14) + 0xd0) as u8;
    }
    t
}

/// The text routines of segment 1028: a font slot, extra pixels between glyphs (01c8) and
/// between lines (01ca).
pub struct Font {
    pub slot: u16,
    pub spacing: i32,
    pub gap: i32,
}

pub const MENU_FONT: Font = Font { slot: FONT, spacing: 0, gap: 2 };
pub const DIGIT_FONT: Font = Font { slot: DIGITS, spacing: 0, gap: 2 };

fn hex2(b: &[u8]) -> u8 {
    let h = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
    h(*b.first().unwrap_or(&b'0')) * 16 + h(*b.get(1).unwrap_or(&b'0'))
}

impl Gfx {
    /// FUN_1028_03bc's 2442: the height of glyph '0' plus the line gap.
    pub fn line_height(&mut self, f: &Font) -> i32 {
        self.height((f.slot as i32) * 256 + 0x30) + f.gap
    }

    /// FUN_1028_0293: the widest line's width (^ codes skipped, ^| ends a line).
    pub fn text_width(&mut self, f: &Font, s: &[u8]) -> i32 {
        let (mut w, mut best, mut i) = (0, 0, 0);
        while i < s.len() {
            if s[i] == b'^' {
                match s.get(i + 1) {
                    Some(b'|') => {
                        best = best.max(w);
                        w = 0;
                        i += 2;
                    }
                    Some(b'C') | Some(b'B') => i += 4,
                    _ => i += 2,
                }
                continue;
            }
            let id = (f.slot as i32) * 256 + s[i] as i32;
            if self.has(id) {
                w += self.width(id) + f.spacing;
            }
            i += 1;
        }
        let w = w.max(best);
        if w > 0 { w - f.spacing } else { w }
    }

    /// FUN_1028_006e: `s` from (x, y); ^Cnn tints the rest through row nn (mode 0x2c) unless
    /// the text is a glow, ^O goes back, ^| starts a new line, ^Bnn blinks (drawn always here).
    pub fn text(&mut self, f: &Font, s: &[u8], x: i32, y: i32, mode: Mode) {
        let lh = self.line_height(f);
        let (mut cx, mut cy, mut m, mut i) = (x, y, mode, 0);
        while i < s.len() {
            if s[i] == b'^' {
                match s.get(i + 1) {
                    Some(b'|') => {
                        cx = x;
                        cy += lh;
                        i += 2;
                    }
                    Some(b'C') => {
                        if !matches!(mode, Mode::Glow(_)) {
                            m = Mode::Tint(hex2(&s[(i + 2).min(s.len())..]));
                        }
                        i += 4;
                    }
                    Some(b'O') => {
                        m = mode;
                        i += 2;
                    }
                    Some(b'B') => i += 4,
                    _ => i += 2,
                }
                continue;
            }
            let id = (f.slot as i32) * 256 + s[i] as i32;
            if self.has(id) {
                self.sprite(id, cx, cy, m);
                cx += self.width(id) + f.spacing;
            }
            i += 1;
        }
    }

    /// FUN_1028_03eb: every line centred on x.
    pub fn text_centered(&mut self, f: &Font, s: &[u8], x: i32, y: i32, mode: Mode) {
        let lh = self.line_height(f);
        for (k, line) in split_lines(s).iter().enumerate() {
            let w = self.text_width(f, line);
            self.text(f, line, x - (w >> 1), y + k as i32 * lh, mode);
        }
    }

    /// FUN_1028_0492: ending at x.
    pub fn text_right(&mut self, f: &Font, s: &[u8], x: i32, y: i32, mode: Mode) {
        let w = self.text_width(f, s);
        self.text(f, s, x - w, y, mode);
    }

    /// FUN_1080_0722: the selected item, eight copies brightening what is under them (row 9),
    /// then the text itself through row 6 (FUN_1080_0712).
    pub fn glow_text(&mut self, f: &Font, s: &[u8], x: i32, y: i32) {
        for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0)] {
            self.text(f, s, x + dx, y + dy, Mode::Glow(9));
        }
        self.text(f, s, x, y, Mode::Tint(6));
    }
}

/// Splits at ^| (FUN_1028_03eb looks for '^' followed by '|').
pub fn split_lines(s: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i + 1 < s.len() {
        if s[i] == b'^' && s[i + 1] == b'|' {
            out.push(&s[start..i]);
            start = i + 2;
            i += 2;
        } else {
            i += 1;
        }
    }
    out.push(&s[start..]);
    out
}
