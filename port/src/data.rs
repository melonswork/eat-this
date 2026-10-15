//! Game data: palettes, tile groups (.TIL), sprite groups (.SPR) and maps (.MAP). See tools/formats.py.

use crate::archive::Archive;
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
    #[allow(dead_code)] // not yet: which palette a level uses
    pub palette: Option<Palette>,
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
        Ok(SpriteGroup {
            sprites,
            palette: a.get("PALETTE").map(|p| palette(&p)),
        })
    }
}

pub struct Map {
    pub width: i32,
    pub height: i32,
    pub start: (i32, i32),
    /// PLANE 0 back, PLANE 1 main (collision; the game's gTilePlane 1), PLANE 2 bonus/enemy icons.
    pub planes: Vec<Vec<u16>>,
    #[allow(dead_code)] // not yet: the .LBM picture behind the planes
    pub background: String,
    pub groups: Vec<Option<String>>, // 16 slots; cell >> 8 indexes this
    pub header: Vec<u8>,
    pub objects: Vec<u8>,
}

impl Map {
    /// OBJECTS: MAP HEADER u16 @10 records of {u8 n; i16 words[n + 1]}, words[0] = type
    /// (FUN_10f0_5d1e; tools/formats.py has the same).
    pub fn object_records(&self) -> Vec<Vec<i16>> {
        let count = u16_at(&self.header, 10) as usize;
        let mut out = Vec::with_capacity(count);
        let mut at = 0;
        for _ in 0..count {
            let Some(&n) = self.objects.get(at) else {
                break;
            };
            let n = n as usize + 1;
            out.push(
                (0..n)
                    .map(|i| i16_at(&self.objects, at + 1 + 2 * i))
                    .collect(),
            );
            at += 1 + 2 * n;
        }
        out
    }

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
            objects: a.get("OBJECTS").unwrap_or_default(),
            header: h,
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
