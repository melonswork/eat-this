//! The level palette and how assets are fitted into it (ETLAUNCH `FUN_10f0_1970` / `FUN_10f0_6039`).
//!
//! A level has one 256-colour palette: the CMAP of its background picture (`Background=` in the
//! `.SCR`), 6-bit, shifted down from the file's 8-bit values. `ForegroundColors=N` splits it:
//! colours 1..=N are the background range, N+1..=255 the foreground range, 0 is transparent.
//! Tiles are authored in that palette and drawn as they are. Sprite groups carry their own
//! palette; at load time every colour listed in their `USED CLRS` entry is replaced by the nearest
//! level colour, those up to the group's header boundary (SPR-HEADER word 5) from the background
//! range, the rest from the foreground range.

use crate::data::Palette;
use std::ops::RangeInclusive;

/// 6-bit VGA colours.
pub type Pal6 = [[u8; 3]; 256];

/// ForegroundColors when the script has none (`DAT_1170_1ae2`'s default).
pub const DEFAULT_FOREGROUND: usize = 0xd0;

pub struct LevelPalette {
    pub colors: Pal6,
    pub foreground: usize,
}

impl LevelPalette {
    /// 8-bit RGB for display (VGA DAC value v -> v << 2 | v >> 4).
    pub fn rgb(&self) -> Palette {
        let mut p = [[0; 3]; 256];
        for (o, c) in p.iter_mut().zip(&self.colors) {
            for k in 0..3 {
                o[k] = (c[k] << 2) | (c[k] >> 4);
            }
        }
        p
    }

    /// `FUN_10f0_6039`: the colour in `free` closest to `c`. The metric is the original's:
    /// (dr² + dg²)² + db⁴, the first of equals wins, an exact match returns at once.
    pub(crate) fn nearest(&self, c: [u8; 3], free: RangeInclusive<usize>) -> u8 {
        let [r, g, b] = c.map(|v| i64::from(v.min(63)));
        let mut best = (i64::MAX, 0);
        for i in free {
            let p = self.colors[i];
            let (dr, dg, db) = (
                r - i64::from(p[0]),
                g - i64::from(p[1]),
                b - i64::from(p[2]),
            );
            let rg = dr * dr + dg * dg;
            let d = rg * rg + db * db * db * db;
            if d < best.0 {
                best = (d, i as u8);
                if d == 0 {
                    break;
                }
            }
        }
        best.1
    }

    /// `FUN_10f0_1970`: maps each colour of a sprite group's palette that `used` marks to a level
    /// colour. Colours 1..=`boundary` go to the background range, the rest to the foreground range.
    pub fn remap_table(&self, pal: &Pal6, used: &[bool; 256], boundary: usize) -> [u8; 256] {
        let mut t = [0u8; 256];
        let bg = 1..=self.foreground;
        let fg = self.foreground + 1..=255;
        for c in 1..=255usize {
            if used[c] {
                let range = if c <= boundary {
                    bg.clone()
                } else {
                    fg.clone()
                };
                t[c] = self.nearest(pal[c], range);
            }
        }
        t
    }
}

impl LevelPalette {
    /// `FUN_10f0_6142`: a colour's brightness, (299 r + 587 g + 114 b) / 1000.
    fn luma(&self, c: usize) -> i32 {
        let [r, g, b] = self.colors[c].map(i32::from);
        (r * 0x12b + g * 0x24b + b * 0x72) / 1000
    }

    /// The tint table at 1170:4c26 (built when a level starts, 1088:d0d6): row k maps each
    /// colour to the nearest one of a tint of its brightness l: 0 red (the hurt hero), 1 green,
    /// 2 fire, 3 negative, 4 dark brown, 10 yellow white (the big alien's shield). Background
    /// colours map into the background range (1..=fg), the rest into fg+2..=255. Rows 5..9 are
    /// not colour maps or belong to other screens; they stay "no change".
    pub fn tint_table(&self) -> Vec<u8> {
        let fg = self.foreground.min(253);
        let mut t: Vec<u8> = (0..11).flat_map(|_| 0..=255u8).collect();
        let c6 = |r: i32, g: i32, b: i32| [r, g, b].map(|v| v.clamp(0, 63) as u8);
        for c in 0..256usize {
            let range = if c <= fg { 1..=fg } else { fg + 2..=255 };
            let l = self.luma(c);
            let mut put = |row: usize, rgb: [u8; 3]| t[row * 256 + c] = self.nearest(rgb, range.clone());
            put(0, c6(l * 3 / 2, 0, 0));
            put(1, c6(l * 4 / 5, l, l / 2));
            let k = l / ((0x40 - l) / 8 + 1);
            put(2, c6(l * 2 + 0xc, k * 3 + 8, k / 2 + 2));
            put(3, c6(0x3f - l, 0x3f - l, 0x3f - l));
            let m = (0x39 - l).clamp(6, 0x22);
            put(4, c6(m / 2, m / 3, 0));
            put(10, c6(l * 3 / 2, l * 3 / 2, l));
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(foreground: usize) -> LevelPalette {
        let mut colors = [[0u8; 3]; 256];
        for (i, c) in colors.iter_mut().enumerate() {
            *c = [(i / 4) as u8, (i / 4) as u8, (i / 4) as u8];
        }
        LevelPalette { colors, foreground }
    }

    #[test]
    fn exact_match_takes_the_lowest_index() {
        let lvl = gradient(175);
        // colours 8..=11 all hold (2, 2, 2); the first one wins
        assert_eq!(lvl.nearest([2, 2, 2], 1..=175), 8);
    }

    #[test]
    fn ranges_are_respected() {
        let lvl = gradient(175);
        let mut used = [false; 256];
        used[10] = true;
        used[200] = true;
        let mut pal = [[0; 3]; 256];
        pal[10] = [60, 60, 60]; // bright, but boundary 100 keeps it in the background range
        pal[200] = [1, 1, 1]; // dark, forced into the foreground range
        let t = lvl.remap_table(&pal, &used, 100);
        assert_eq!(t[10], 172); // 43 is as bright as the background range gets
        assert_eq!(t[200], 176); // 44 is as dark as the foreground range gets
    }
}
