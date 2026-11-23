//! Drawing sprites the way the original does (segment 10f0). Nothing here touches the simulation:
//! the draw pass reads the game state after the frame's update.
//!
//! The original draws every sprite through `FUN_10f0_2670` (or `FUN_10f0_2a11` mirrored; the pair
//! sits in the table at 1170:1b54, indexed by the flip byte 1ae5). 1ae6 flips vertically. Each
//! sprite row is a list of runs; the run handlers come from the table at 1170:1b8a, indexed by the
//! draw mode 1ae0 (two handlers per mode: a run with transparent pixels, a solid run). What the
//! modes do (from their disassembly, 10f0:23b6..2648):
//!
//! | 1ae0 | effect |
//! |------|--------|
//! | 0    | copy, only over pixels <= 1ae2 (the background range: foreground tiles stay in front) |
//! | 4    | plain copy (colour 0 skipped) |
//! | 0x20 | opaque copy of each row's span (colour 0 inside it written) |
//! | 8    | fill with colour 1ae3, behind the foreground |
//! | 0xc, 0x18 | through the 256-byte table 1170:678c (not ported: plain copy / skipped) |
//! | 0x10 | MIX[src][dst], behind the foreground (translucent) |
//! | 0x14 | copy only over colour 1ae4 |
//! | 0x1c | dst = dst[x + 1], behind the foreground (heat haze) |
//! | 0x24 | MIX[1ae3][src], behind the foreground (the hit flash, 1ae3 = 1170:1b1b) |
//! | 0x28 | LIT[src][dst], behind the foreground (glows, PalMixLight) |
//! | 0x2c | MIX[1ae3][src] |
//! | 0x30 | MIX[src][dst] |
//! | 0x34 | MIX[TINT[1ae3][src]][dst], behind the foreground |
//! | 0x38 | MIX[1ae3][dst] where the sprite has pixels |
//!
//! MIX is the table 1b0c points at: normally the level's PalMix, swapped for the tint table
//! (4c26, computed from the palette) or PalMixLight (1b10) around a few calls. PalMixSea (16da)
//! only serves the water (`draw::water`).

use crate::data::SpriteGroup;
use crate::palette::LevelPalette;
use crate::{H, W};
use std::cell::RefCell;
use std::collections::HashMap;

/// Sprite groups by slot, fitted to the level palette as they load.
pub struct Bank {
    level: LevelPalette,
    groups: RefCell<HashMap<u16, Option<SpriteGroup>>>,
}

impl Bank {
    pub fn new(level: LevelPalette) -> Bank {
        Bank {
            level,
            groups: RefCell::new(HashMap::new()),
        }
    }

    pub fn foreground(&self) -> u8 {
        self.level.foreground.min(255) as u8
    }

    /// Calls `f` with sprite `id` (slot * 256 + frame), if it exists.
    fn with<R>(&self, id: i32, f: impl FnOnce(&crate::data::Sprite) -> R) -> Option<R> {
        let slot = (id as u16) >> 8;
        let mut groups = self.groups.borrow_mut();
        let g = groups.entry(slot).or_insert_with(|| {
            let mut g = crate::sprites::path(slot).and_then(|p| SpriteGroup::load(&p).ok());
            if let Some(g) = &mut g {
                if !crate::sprites::raw(slot) {
                    g.fit_to(&self.level);
                }
            }
            g
        });
        let s = g.as_ref()?.sprites.get((id & 0xff) as usize)?.as_ref()?;
        Some(f(s))
    }

    /// The number of sprites in a slot (FUN_10f0_143c).
    pub fn count(&self, slot: i32) -> i32 {
        self.with(slot << 8, |_| ());
        self.groups.borrow().get(&(slot as u16)).and_then(|g| g.as_ref()).map_or(0, |g| g.sprites.len() as i32)
    }

    /// Sprite `id`'s (width, height) (FUN_10f0_1f5f / 1f8e).
    pub fn size(&self, id: i32) -> Option<(i32, i32)> {
        self.with(id, |s| (s.width, s.height))
    }

    /// Row `row` of sprite `id`'s pixels (a texture).
    pub fn row(&self, id: i32, row: i32) -> Option<Vec<u8>> {
        self.with(id, |s| {
            let r = row.clamp(0, s.height - 1);
            s.pixels[(r * s.width) as usize..((r + 1) * s.width) as usize].to_vec()
        })
    }
}

/// The colour tables the modes use.
#[derive(Default)]
pub struct Tables {
    /// PalMix (1b0c's usual target, 6992).
    pub mix: Option<Vec<u8>>,
    /// PalMixLight (1b10): rows are light levels.
    pub light: Option<Vec<u8>>,
    /// PalMixSea (16da): the water's tints, rows indexed by the SEATILE texture.
    pub sea: Option<Vec<u8>>,
    /// The tint table 4c26 (`LevelPalette::tint_table`), 256 rows.
    pub tint: Vec<u8>,
}

/// Which table MIX (1b0c) points at.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mix {
    #[default]
    Level,
    /// The tint table 4c26.
    Tint,
    Light,
}

/// The drawing state: the frame buffer plus the globals the original's draw calls read.
pub struct Painter<'a> {
    pub fb: &'a mut [u8],
    pub bank: &'a Bank,
    pub tables: &'a Tables,
    pub cam: (i32, i32),
    /// 1ae2: the last colour of the background range.
    pub fg: u8,
    /// 1ae0, 1ae3, 1ae4.
    pub mode: i32,
    pub colour: u8,
    pub key: u8,
    /// 1ae5 / 1ae6.
    pub hflip: bool,
    pub vflip: bool,
    pub mix: Mix,
    /// The level frame counter as the draw code sees it (DAT_1170_617e).
    pub frame: u32,
    /// The hit flash colour, 1170:1b1b (VGA colour 7 of the level).
    pub flash: u8,
    /// 1170:1b14 (VGA colour 0 of the level).
    pub black: u8,
    /// 1170:1b14..1b23: the level's colours nearest to the 16 VGA colours.
    pub vga: [u8; 16],
    /// 1170:1aca..1ad0: the clip rectangle (left, top, right, bottom), inclusive.
    pub clip: (i32, i32, i32, i32),
}

/// The full screen as a clip rectangle.
pub const SCREEN: (i32, i32, i32, i32) = (0, 0, W - 1, H - 1);

impl<'a> Painter<'a> {
    fn table(&self, m: Mix) -> Option<&'a [u8]> {
        let tables: &'a Tables = self.tables;
        match m {
            Mix::Level => tables.mix.as_deref(),
            Mix::Tint => Some(tables.tint.as_slice()),
            Mix::Light => tables.light.as_deref().or(tables.mix.as_deref()),
        }
    }

    /// Resets mode, colour, flips and table, as the callers do after a special draw.
    pub fn reset(&mut self) {
        self.clip = SCREEN;
        self.mode = 0;
        self.hflip = false;
        self.vflip = false;
        self.mix = Mix::Level;
    }

    /// `FUN_10f0_2670` / `2a11`: sprite `id` with its anchor at screen (x, y).
    pub fn sprite(&mut self, id: i32, x: i32, y: i32) {
        if id < 0 || id == 0x7fff {
            return;
        }
        if std::env::var("ET_DRAWLOG").is_ok() {
            eprintln!("sprite {id:#06x} at ({x},{y}) mode {:#x} flip {}", self.mode, self.hflip);
        }
        let bank = self.bank;
        bank.with(id, |s| self.put(s, x, y));
    }

    /// The sprite at world position (x, y).
    pub fn at(&mut self, id: i32, x: i32, y: i32) {
        self.sprite(id, x - self.cam.0, y - self.cam.1);
    }

    /// `at` with a one-off mode (reset to 0 afterwards, as the callers do).
    pub fn at_mode(&mut self, mode: i32, id: i32, x: i32, y: i32) {
        self.mode = mode;
        self.at(id, x, y);
        self.mode = 0;
    }

    /// The hit flash: mode 0x24 with colour 1b1b while `flash` > 0.
    pub fn at_flash(&mut self, flash: bool, id: i32, x: i32, y: i32) {
        if flash {
            self.mode = 0x24;
            self.colour = self.flash;
        }
        self.at(id, x, y);
        self.mode = 0;
    }

    /// `FUN_10f0_302b(ystep, xstep, id, y, x)`: sprite `id` scaled, reading `xstep` / `ystep`
    /// source pixels (8.8 fixed point, 0x100 = 1:1) per screen pixel, anchor at screen (x, y).
    pub fn scaled(&mut self, id: i32, x: i32, y: i32, xstep: i32, ystep: i32) {
        if id < 0 || id == 0x7fff || xstep <= 0 || ystep <= 0 {
            return;
        }
        let bank = self.bank;
        bank.with(id, |s| {
            let w = (s.width * 0x100) / xstep;
            let h = (s.height * 0x100) / ystep;
            if w <= 0 || h <= 0 {
                return;
            }
            let mut px = vec![0u8; (w * h) as usize];
            for yy in 0..h {
                let sy0 = ((yy * ystep) >> 8).min(s.height - 1);
                for xx in 0..w {
                    let sx0 = ((xx * xstep) >> 8).min(s.width - 1);
                    px[(yy * w + xx) as usize] = s.pixels[(sy0 * s.width + sx0) as usize];
                }
            }
            let scaled = crate::data::Sprite {
                width: w,
                height: h,
                // 10f0:3080: the top is ((hot_y - 1) * 0x100) / ystep + 1.
                hotspot: ((s.hotspot.0 * 0x100) / xstep, ((s.hotspot.1 - 1) * 0x100) / ystep + 1),
                pixels: px,
                extra: Vec::new(),
            };
            self.put(&scaled, x, y);
        });
    }

    /// `FUN_1040_6720(ystep, xstep, id, y, x)` at world (x, y).
    pub fn scaled_at(&mut self, ystep: i32, xstep: i32, id: i32, x: i32, y: i32) {
        let (x, y) = (x - self.cam.0, y - self.cam.1);
        if xstep == 0x100 && ystep == 0x100 {
            self.sprite(id, x, y);
        } else {
            self.scaled(id, x, y, xstep, ystep);
        }
    }

    /// `FUN_1040_6315(ystep, xstep, t, y, x)`: a flame (FLAME5, 0x6a00 + t / 3 % 16) scaled
    /// as `scaled` does, at world (x, y). `FUN_1040_62ed` is the unscaled one.
    pub fn flame(&mut self, ystep: i32, xstep: i32, t: u32, x: i32, y: i32) {
        let id = ((t / 3) % 0x10) as i32 + 0x6a00;
        let (x, y) = (x - self.cam.0, y - self.cam.1);
        if xstep == 0x200 && ystep == 0x200 {
            self.half(id, x, y);
        } else {
            self.scaled(id, x, y, xstep, ystep);
        }
    }

    /// FUN_10f0_2d8e: sprite `id` at half size (every other row from the first, every other
    /// pixel from each row's first opaque one, the hotspot halved toward zero), anchor at
    /// screen (x, y).
    pub fn half(&mut self, id: i32, x: i32, y: i32) {
        if id < 0 || id == 0x7fff {
            return;
        }
        let bank = self.bank;
        bank.with(id, |s| {
            let (w, h) = (s.width >> 1, s.height >> 1);
            if w <= 0 || h <= 0 {
                return;
            }
            let mut px = vec![0u8; (w * h) as usize];
            for yy in 0..h {
                // The row's span (its first opaque pixel, its opaque width): screen pixel
                // start / 2 + k shows source pixel start + 2k.
                let row = &s.pixels[(yy * 2 * s.width) as usize..((yy * 2 + 1) * s.width) as usize];
                let Some(start) = row.iter().position(|&c| c != 0) else { continue };
                let end = row.iter().rposition(|&c| c != 0).unwrap_or(start) + 1;
                for k in 0..((end - start) >> 1) {
                    let xx = (start >> 1) + k;
                    if (xx as i32) < w {
                        px[(yy * w) as usize + xx] = row[start + 2 * k];
                    }
                }
            }
            let half = crate::data::Sprite {
                width: w,
                height: h,
                hotspot: (s.hotspot.0 / 2, s.hotspot.1 / 2),
                pixels: px,
                extra: Vec::new(),
            };
            self.put(&half, x, y);
        });
    }

    fn put(&mut self, s: &crate::data::Sprite, x: i32, y: i32) {
        let (w, h) = (s.width, s.height);
        let left = if self.hflip {
            x - s.hotspot.0 - w + 1
        } else {
            x + s.hotspot.0
        };
        let top = if self.vflip {
            y - s.hotspot.1 - h + 1
        } else {
            y + s.hotspot.1
        };
        let fg = self.fg;
        let mix = self.table(self.mix);
        let light = self.table(Mix::Light);
        let tint = self.table(Mix::Tint);
        let (mode, colour, key) = (self.mode, self.colour, self.key);
        let (c0, c1, c2, c3) = self.clip;
        for row in 0..h {
            let sy = top + row;
            if sy < c1 || sy > c3 {
                continue;
            }
            let srow = if self.vflip { h - 1 - row } else { row };
            // Mode 0x20 (10f0:24fd) copies the row's whole span, colour 0 inside it included.
            let span = if mode == 0x20 {
                let r = &s.pixels[(srow * w) as usize..((srow + 1) * w) as usize];
                r.iter().position(|&c| c != 0).map(|a| (a as i32, r.iter().rposition(|&c| c != 0).unwrap_or(a) as i32))
            } else {
                None
            };
            for col in 0..w {
                let sx = left + col;
                if sx < c0 || sx > c2 {
                    continue;
                }
                let scol = if self.hflip { w - 1 - col } else { col };
                let src = s.pixels[(srow * w + scol) as usize];
                let at = (sy * W + sx) as usize;
                let dst = self.fb[at];
                let behind = dst <= fg;
                let out = match mode {
                    0x20 => span.is_some_and(|(a, b)| (a..=b).contains(&scol)).then_some(src),
                    4 | 0xc => (src != 0).then_some(src),
                    0x18 => None,
                    8 => (src != 0 && behind).then_some(colour),
                    0x10 => (src != 0 && behind)
                        .then(|| mix.map_or(src, |t| t[src as usize * 256 + dst as usize])),
                    0x14 => (src != 0 && dst == key).then_some(src),
                    0x1c => (src != 0 && behind && sx + 1 < W).then(|| self.fb[at + 1]),
                    0x24 => (src != 0 && behind)
                        .then(|| mix.map_or(src, |t| t[colour as usize * 256 + src as usize])),
                    0x28 => (src != 0 && behind)
                        .then(|| light.map_or(src, |t| t[src as usize * 256 + dst as usize])),
                    0x2c => (src != 0)
                        .then(|| mix.map_or(src, |t| t[colour as usize * 256 + src as usize])),
                    0x30 => (src != 0)
                        .then(|| mix.map_or(src, |t| t[src as usize * 256 + dst as usize])),
                    0x34 => (src != 0 && behind).then(|| {
                        let c = tint.map_or(src, |t| t[colour as usize * 256 + src as usize]);
                        mix.map_or(c, |t| t[c as usize * 256 + dst as usize])
                    }),
                    0x38 => (src != 0)
                        .then(|| mix.map_or(src, |t| t[colour as usize * 256 + dst as usize])),
                    _ => (src != 0 && behind).then_some(src),
                };
                if let Some(c) = out {
                    self.fb[at] = c;
                }
            }
        }
    }

    /// `FUN_10f0_3da`: a one-pixel line in colour `c` between two screen points.
    pub fn line(&mut self, (x1, y1): (i32, i32), (x2, y2): (i32, i32), c: u8) {
        let n = (x2 - x1).abs().max((y2 - y1).abs()).max(1);
        for i in 0..=n {
            let x = x1 + (x2 - x1) * i / n;
            let y = y1 + (y2 - y1) * i / n;
            let (c0, c1, c2, c3) = self.clip;
            if x >= c0 && x <= c2 && y >= c1 && y <= c3 {
                self.fb[(y * W + x) as usize] = c;
            }
        }
    }
}

/// FUN_10d0_01e4 (plain fill): a polygon filled with colour `c`, clipped to the screen; each
/// scanline from the leftmost to the rightmost edge crossing at the row's centre.
pub fn fill_polygon(fb: &mut [u8], pts: &[(i32, i32)], c: u8) {
    let (top, bottom) = pts.iter().fold((i32::MAX, i32::MIN), |(t, b), p| (t.min(p.1), b.max(p.1)));
    for y in top.max(0)..bottom.min(H) {
        let fy = y as f64 + 0.5;
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let (ya, yb) = (a.1 as f64, b.1 as f64);
            if (ya <= fy && fy < yb) || (yb <= fy && fy < ya) {
                xs.push(a.0 as f64 + (fy - ya) * (b.0 - a.0) as f64 / (yb - ya));
            }
        }
        if xs.len() < 2 {
            continue;
        }
        let l = xs.iter().cloned().fold(f64::MAX, f64::min).round() as i32;
        let r = xs.iter().cloned().fold(f64::MIN, f64::max).round() as i32 - 1;
        for x in l.max(0)..=r.min(W - 1) {
            fb[(y * W + x) as usize] = c;
        }
    }
}
