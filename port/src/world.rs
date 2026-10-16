//! A loaded level: its tile planes (mutable, as the game edits them), tile groups, sectors, and
//! the tile queries the game logic uses.

use crate::data::{Map, TILE, TileGroup};
use std::{io, path::Path};

/// What TileSolidityAt/TileFunctionAt return outside a plane (ETLAUNCH 10f0:58ec): solid to all.
pub const OUTSIDE: u16 = 0xa75f;

/// Solidity bits (TILE INFOS function 1; see BIN/TILEMAKE.KIT/TM/TILES.TXT).
#[allow(dead_code)]
pub mod solid {
    pub const FLOOR: u16 = 0x0001;
    pub const WALL: u16 = 0x0002;
    pub const SLOPE: u16 = 0x0020;
    pub const FLOOR_NOT_BULLETS: u16 = 0x0200;
    pub const FLOOR_HERO_ONLY: u16 = 0x2000;
    /// "Any floor the hero stands on", the mask HeroUpdate tests at 1088:7592.
    pub const HERO_FLOOR: u16 = FLOOR | FLOOR_NOT_BULLETS | FLOOR_HERO_ONLY;
}

/// Tile function 0 values the level logic reacts to (TILES.TXT).
pub mod func {
    pub const LEVER_FIRST: u16 = 6000; // 6000..=6009: a "lever" cell in state 0..9
    pub const LEVER_LAST: u16 = 6009;
    pub const NOT_CUT: u16 = 7021; // "Wall that won't be cut when selecting a map area"
    pub const AREA_PARAMS: u16 = 7040; // "Specify parameters for map areas"
    pub const STOP: u16 = 7100; // "Stop scrolling": sector borders
    pub const SECTOR_ICON: u16 = 61700; // the SECTOR icon tile in a sector
}

/// Plane slots: 0 back, 1 main (gTilePlane 1, collision), 2 bonus/icons; platforms get their own
/// slots from 16 up (FUN_1078_02e5), like the game's 1170:64ca table of 48.
pub const MAIN_PLANE: usize = 1;
pub const PLANE_SLOTS: usize = 0x30;
pub const FIRST_PLATFORM_PLANE: usize = 0x10;

#[derive(Clone)]
pub struct Plane {
    pub width: i32,
    pub height: i32,
    pub cells: Vec<u16>,
}

impl Plane {
    pub fn get(&self, tx: i32, ty: i32) -> u16 {
        self.cells[(ty * self.width + tx) as usize]
    }

    pub fn set(&mut self, tx: i32, ty: i32, v: u16) {
        self.cells[(ty * self.width + tx) as usize] = v;
    }
}

/// A sector (FUN_1068_0002): the rectangle between STOP tiles that the camera and the level logic
/// work in.
#[derive(Clone, Debug)]
pub struct Sector {
    /// Containment bounds (+2..+8): borders at the map edge are opened to +/-0x7fff.
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    /// The rectangle as found (+10..+0x10): the camera limits.
    pub cam: (i32, i32, i32, i32),
    /// Map cell of the sector's SECTOR icon (+0x12), or -1.
    pub icon: i32,
}

pub struct World {
    pub map: Map,
    pub groups: Vec<Option<TileGroup>>,
    pub planes: Vec<Option<Plane>>,
    /// The whole-map default sector (1170:45d8) is `sectors[0]`; the flood-filled ones follow
    /// (the 1170:464a list).
    pub sectors: Vec<Sector>,
}

impl World {
    pub fn load(map_path: &Path, tiles_dir: &Path) -> io::Result<World> {
        let map = Map::load(map_path)?;
        let groups = map
            .groups
            .iter()
            .map(|g| {
                g.as_ref()
                    .map(|name| TileGroup::load(&tiles_dir.join(name)))
                    .transpose()
            })
            .collect::<io::Result<_>>()?;
        let mut planes: Vec<Option<Plane>> = vec![None; PLANE_SLOTS];
        for (i, p) in map.planes.iter().enumerate() {
            planes[i] = Some(Plane {
                width: map.width,
                height: map.height,
                cells: p.clone(),
            });
        }
        let mut w = World {
            map,
            groups,
            planes,
            sectors: Vec::new(),
        };
        w.build_sectors();
        Ok(w)
    }

    /// Function n of a cell value (TileParamOfLastCell, 10f0:5a8c): 0 for empty/negative cells.
    pub fn cell_function(&self, cell: u16, n: usize) -> u16 {
        if cell == 0 || cell & 0x8000 != 0 {
            return 0;
        }
        match &self.groups[(cell >> 8) as usize & 15] {
            Some(g) if ((cell & 0xff) as usize) < g.len() => g.function((cell & 0xff) as usize, n),
            _ => 0,
        }
    }

    /// The first byte of the TILE INFOS record of the tile at (y, x) in a plane.
    pub fn cell_byte0(&self, plane: usize, y: i32, x: i32) -> Option<u8> {
        let c = self.cell_at(plane, y, x)?;
        if c & 0x8000 != 0 {
            return None;
        }
        let g = self.groups[(c >> 8) as usize & 15].as_ref()?;
        g.infos.get((c & 0xff) as usize * 16).copied()
    }

    /// The cell under pixel (x, y) of a plane, or None outside it.
    pub fn cell_at(&self, plane: usize, y: i32, x: i32) -> Option<u16> {
        let p = self.planes.get(plane)?.as_ref()?;
        if x < 0 || y < 0 || x >> 4 >= p.width || y >> 4 >= p.height {
            return None;
        }
        Some(p.get(x >> 4, y >> 4))
    }

    fn function_in(&self, plane: usize, y: i32, x: i32, n: usize) -> u16 {
        match self.cell_at(plane, y, x) {
            None => OUTSIDE,
            Some(c) => self.cell_function(c, n),
        }
    }

    /// TileSolidityAt (10f0:58ec) on the main plane.
    pub fn solidity_at(&self, y: i32, x: i32) -> u16 {
        self.function_in(MAIN_PLANE, y, x, 1)
    }

    /// TileSolidityAt with gTilePlane = `plane`.
    pub fn solidity_in(&self, plane: usize, y: i32, x: i32) -> u16 {
        self.function_in(plane, y, x, 1)
    }

    /// TileFunctionAt (10f0:587c) on the main plane: function 0 (10 ladder, 60600 pipe, ...).
    pub fn tile_function_at(&self, y: i32, x: i32) -> u16 {
        self.function_in(MAIN_PLANE, y, x, 0)
    }

    pub fn tile_function_in(&self, plane: usize, y: i32, x: i32) -> u16 {
        self.function_in(plane, y, x, 0)
    }

    pub fn main(&self) -> &Plane {
        self.planes[MAIN_PLANE].as_ref().unwrap()
    }

    pub fn main_mut(&mut self) -> &mut Plane {
        self.planes[MAIN_PLANE].as_mut().unwrap()
    }

    pub fn pixel_size(&self) -> (i32, i32) {
        (self.map.width * TILE, self.map.height * TILE)
    }

    /// FUN_1068_021c: the last-built sector containing (x, y), else the default sector 0.
    pub fn sector_at(&self, y: i32, x: i32) -> usize {
        (1..self.sectors.len())
            .rev()
            .find(|&i| {
                let s = &self.sectors[i];
                s.x1 <= x && s.y1 <= y && x <= s.x2 && y <= s.y2
            })
            .unwrap_or(0)
    }

    /// FUN_1068_03e8: flood a sector from the four neighbours of every STOP tile.
    fn build_sectors(&mut self) {
        let (w, h) = (self.map.width, self.map.height);
        self.sectors = vec![Self::make_sector(w, h, 0, 0, w * 16 - 1, h * 16 - 1)];
        for i in 0..w * h {
            let cell = self.main().cells[i as usize];
            if self.cell_function(cell, 0) == func::STOP {
                let (y, x) = ((i / w) * 16, (i % w) * 16);
                for (dy, dx) in [(0, -16), (0, 16), (-16, 0), (16, 0)] {
                    self.flood_sector(y + dy, x + dx);
                }
            }
        }
    }

    /// FUN_1068_0002: map-edge borders open up so the hero can't leave through them.
    fn make_sector(w: i32, h: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> Sector {
        Sector {
            x1: if x1 < 0x11 { -0x7fff } else { x1 },
            y1: if y1 < 0x11 { -0x7fff } else { y1 },
            x2: if x2 < 0 || x2 >= w * 16 - 1 {
                0x7fff
            } else {
                x2
            },
            y2: if y2 < 0 || y2 >= h * 16 - 1 {
                0x7fff
            } else {
                y2
            },
            cam: (x1, y1, x2, y2),
            icon: -1,
        }
    }

    /// FUN_1068_0295.
    fn flood_sector(&mut self, y: i32, x: i32) {
        let f = |w: &World, y: i32, x: i32| w.tile_function_at(y, x);
        let first = f(self, y, x);
        if first == func::STOP || first == OUTSIDE || self.sector_at(y, x) != 0 {
            return;
        }
        let mut left = x;
        loop {
            let v = f(self, y, left - 16);
            if v == func::STOP || v == OUTSIDE {
                break;
            }
            left -= 16;
        }
        let mut top = y;
        loop {
            let v = f(self, top - 16, left & !15);
            if v == func::STOP || v == OUTSIDE {
                break;
            }
            top -= 16;
        }
        let mut right = x;
        loop {
            right += 16;
            let v = f(self, y, right);
            if v == func::STOP || v == OUTSIDE {
                break;
            }
        }
        let x2 = (right & !15) - 1;
        let mut bottom = y;
        loop {
            bottom += 16;
            let v = f(self, bottom, x2);
            if v == func::STOP || v == OUTSIDE {
                break;
            }
        }
        let y2 = (bottom & !15) - 1;
        let s = Self::make_sector(
            self.map.width,
            self.map.height,
            left & !15,
            top & !15,
            x2,
            y2,
        );
        self.sectors.push(s);
    }
}

/// The original's initialised data segment (ETLAUNCH.EXE's autodata segment), read from the
/// player's CD at start-up: a few tables live there (slopes at 1170:1280, ...).
pub static GAME_DS: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

/// Slope heights: 0x16-byte rows indexed by a slope tile's function 2 (1170:1280).
pub static SLOPES: std::sync::LazyLock<&'static [u8]> =
    std::sync::LazyLock::new(|| &GAME_DS.get().expect("load_game_ds first")[0x1280..]);

/// Find ETLAUNCH.EXE's autodata segment (NE header) and keep it.
pub fn load_game_ds(exe: &Path) -> io::Result<()> {
    let d = std::fs::read(exe)?;
    let u16_at = |a: usize| u16::from_le_bytes([d[a], d[a + 1]]) as usize;
    let ne = u16_at(0x3c);
    let (count, auto) = (u16_at(ne + 0x1c), u16_at(ne + 0x0e));
    let (table, shift) = (ne + u16_at(ne + 0x22), u16_at(ne + 0x32));
    if auto == 0 || auto > count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "no autodata segment",
        ));
    }
    let e = table + (auto - 1) * 8;
    let (off, len) = (u16_at(e) << shift, u16_at(e + 2));
    let mut ds = d[off..off + len].to_vec();
    ds.resize(0x10000, 0);
    let _ = GAME_DS.set(ds);
    Ok(())
}

/// A word of the game's data segment.
pub fn ds_word(off: usize) -> i32 {
    let d = GAME_DS.get().expect("load_game_ds first");
    i16::from_le_bytes([d[off], d[off + 1]]) as i32
}
