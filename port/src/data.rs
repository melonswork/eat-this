//! Game data: palettes, tile groups (.TIL), sprite groups (.SPR) and maps (.MAP). See tools/formats.py.

use crate::archive::Archive;
use crate::palette::{LevelPalette, Pal6};
use std::{io, path::Path};

pub const TILE: i32 = 16;

pub type Palette = [[u8; 3]; 256];

fn palette(raw: &[u8]) -> Palette {
    let mut p = [[0; 3]; 256];
    for (i, c) in p.iter_mut().enumerate() {
        for k in 0..3 {
            let v = raw[i * 3 + k] & 63;
            c[k] = (v << 2) | (v >> 4);
        }
    }
    p
}

/// 6-bit VGA colours as stored in the files.
pub fn palette6(raw: &[u8]) -> Pal6 {
    let mut p = [[0; 3]; 256];
    for (i, c) in p.iter_mut().enumerate() {
        for k in 0..3 {
            c[k] = raw[i * 3 + k] & 63;
        }
    }
    p
}

fn missing(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("missing entry {what}"))
}

fn i16_at(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

pub struct TileGroup {
    pub pixels: Vec<u8>, // 256 bytes per tile
    pub infos: Vec<u8>,  // 16 bytes per tile; +6 function 0, +8 function 1 (solidity)
    pub palette: Palette,
}

impl TileGroup {
    pub fn load(path: &Path) -> io::Result<TileGroup> {
        let a = Archive::open(path)?;
        Ok(TileGroup {
            pixels: a.get("TILE DATA").ok_or_else(|| missing("TILE DATA"))?,
            infos: a.get("TILE INFOS").ok_or_else(|| missing("TILE INFOS"))?,
            palette: palette(&a.get("PALETTE").ok_or_else(|| missing("PALETTE"))?),
        })
    }

    pub fn len(&self) -> usize {
        self.pixels.len() / 256
    }

    pub fn tile(&self, i: usize) -> &[u8] {
        &self.pixels[i * 256..i * 256 + 256]
    }

    /// Function n (0..4) of tile i: a u16 at +6 + 2n of its TILE INFOS record.
    pub fn function(&self, i: usize, n: usize) -> u16 {
        u16_at(&self.infos, i * 16 + 6 + 2 * n)
    }
}

pub struct Sprite {
    pub width: i32,
    pub height: i32,
    pub hotspot: (i32, i32),
    pub pixels: Vec<u8>, // 0 = transparent
}

pub struct SpriteGroup {
    pub sprites: Vec<Option<Sprite>>,
    /// The group's own 6-bit palette, `USED CLRS` (all colours when the entry is missing) and the
    /// header's foreground boundary (SPR-HEADER word 5): see palette.rs.
    pub palette: Option<Pal6>,
    pub used: [bool; 256],
    pub boundary: usize,
}

impl SpriteGroup {
    pub fn load(path: &Path) -> io::Result<SpriteGroup> {
        let a = Archive::open(path)?;
        let names: Vec<String> = a
            .names()
            .filter(|n| n.starts_with("SPRITE ") || *n == "EMPTY")
            .map(String::from)
            .collect();
        let sprites = names
            .iter()
            .map(|n| {
                let d = a.get(n).filter(|d| !d.is_empty())?;
                let (w, h) = (u16_at(&d, 0) as usize, u16_at(&d, 2) as usize);
                let mut at = 10 + u16_at(&d, 8) as usize;
                let mut pixels = Vec::with_capacity(w * h);
                for _ in 0..h {
                    pixels.extend_from_slice(&d[at + 4..at + 4 + w]);
                    at += 4 + w;
                }
                Some(Sprite {
                    width: w as i32,
                    height: h as i32,
                    hotspot: (i16_at(&d, 4) as i32, i16_at(&d, 6) as i32),
                    pixels,
                })
            })
            .collect();
        let mut used = [true; 256];
        if let Some(u) = a.get("USED CLRS") {
            for (flag, &b) in used.iter_mut().zip(&u) {
                *flag = b != 0;
            }
        }
        let boundary = a
            .get("SPR-HEADER")
            .filter(|h| h.len() >= 12)
            .map_or(255, |h| u16_at(&h, 10) as usize);
        Ok(SpriteGroup {
            sprites,
            palette: a.get("PALETTE").map(|p| palette6(&p)),
            used,
            boundary,
        })
    }

    /// Rewrites every sprite's pixels into the level palette (what the original does as it loads
    /// the group). Colour 0 stays transparent.
    pub fn fit_to(&mut self, level: &LevelPalette) {
        let Some(pal) = &self.palette else { return };
        let table = level.remap_table(pal, &self.used, self.boundary);
        for s in self.sprites.iter_mut().flatten() {
            for p in &mut s.pixels {
                *p = table[*p as usize];
            }
        }
    }
}

pub struct Map {
    pub width: i32,
    pub height: i32,
    pub start: (i32, i32),
    /// PLANE 0 back, PLANE 1 main (collision; the game's gTilePlane 1), PLANE 2 bonus/enemy icons.
    pub planes: Vec<Vec<u16>>,
    /// MAP FILENAMES slot 0: the picture (.LBM) behind the planes; the level script's
    /// `Background=` names the same file.
    pub background: String,
    pub groups: Vec<Option<String>>, // 16 slots; cell >> 8 indexes this
}

impl Map {
    pub fn load(path: &Path) -> io::Result<Map> {
        let a = Archive::open(path)?;
        let h = a.get("MAP HEADER").ok_or_else(|| missing("MAP HEADER"))?;
        let (width, height) = (u16_at(&h, 2) as i32, u16_at(&h, 4) as i32);
        let start = (i16_at(&h, 6) as i32, i16_at(&h, 8) as i32); // LevelLoad -> FUN_1088_0c20(y, x)
        let planes = (0..3)
            .map_while(|i| a.get(&format!("PLANE {i}")))
            .map(|raw| {
                raw.chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect()
            })
            .collect();
        let names = a.get("FILENAMES").ok_or_else(|| missing("FILENAMES"))?;
        let slots: Vec<String> = names
            .chunks(13)
            .map(|c| {
                c[1..1 + (c[0] as usize).min(c.len() - 1)]
                    .iter()
                    .map(|&b| b as char)
                    .collect()
            })
            .collect();
        Ok(Map {
            width,
            height,
            start,
            planes,
            background: slots[0].clone(),
            groups: slots[1..]
                .iter()
                .map(|s| (!s.is_empty()).then(|| s.clone()))
                .collect(),
        })
    }

    pub fn cell(&self, plane: usize, tx: i32, ty: i32) -> u16 {
        if tx < 0 || ty < 0 || tx >= self.width || ty >= self.height {
            return 0;
        }
        self.planes
            .get(plane)
            .map_or(0, |p| p[(ty * self.width + tx) as usize])
    }
}

/// The few `.SCR` keys the port reads (`FUN_1088_bf00` parses the whole script, one `Key=value`
/// per line, `'` starts a comment).
pub struct Script {
    pub background: Option<String>,
    pub foreground_colors: Option<usize>,
    /// `BackgroundProc=`: which of the original's background routines runs (see bg.rs).
    pub background_proc: u32,
    /// `LoadSpriteGroup=` lines in order, blank ones included (they still take a slot; the first
    /// slot is group 0x32 in the original's numbering).
    pub sprite_groups: Vec<String>,
    /// `PalMix=`: the 256x256 translucency table in PALS/ (mix[src * 256 + dst]).
    pub palmix: Option<String>,
}

impl Script {
    pub fn parse(text: &str) -> Script {
        let mut s = Script {
            background: None,
            foreground_colors: None,
            background_proc: 0,
            sprite_groups: Vec::new(),
            palmix: None,
        };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "background" if !value.is_empty() => s.background = Some(value.to_string()),
                "backgroundproc" => s.background_proc = value.parse().unwrap_or(0),
                "palmix" if !value.is_empty() => s.palmix = Some(value.to_string()),
                "loadspritegroup" => s.sprite_groups.push(value.to_string()),
                "foregroundcolors" => s.foreground_colors = value.parse().ok().filter(|&n| n < 256),
                _ => {}
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_keys() {
        let s = Script::parse("' Foreground colors\nForegroundColors=175\nBackground=citybac.lbm\n'Background=x.lbm\n");
        assert_eq!(s.foreground_colors, Some(175));
        assert_eq!(s.background.as_deref(), Some("citybac.lbm"));
    }
}

/// `PALS/*.MIX`: result colour of drawing colour `src` over colour `dst`, as a 256x256 table.
pub fn load_palmix(path: &Path) -> io::Result<Vec<u8>> {
    let t = Archive::open(path)?.get("PALMIX").ok_or_else(|| missing("PALMIX"))?;
    if t.len() != 256 * 256 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "PALMIX is not 256x256"));
    }
    Ok(t)
}
