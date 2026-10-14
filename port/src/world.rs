//! A loaded level: the map, its tile groups and the tile queries the game logic uses.

use crate::data::{Map, TILE, TileGroup};
use std::{io, path::Path};

/// What TileSolidityAt/TileFunctionAt return outside the map (ETLAUNCH 10f0:58ec): solid to all.
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

/// The plane the game logic reads (gTilePlane = 1).
pub const MAIN_PLANE: usize = 1;

pub struct World {
    pub map: Map,
    pub groups: Vec<Option<TileGroup>>,
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
        Ok(World { map, groups })
    }

    fn function_at(&self, y: i32, x: i32, n: usize) -> u16 {
        if x < 0 || y < 0 || x >> 4 >= self.map.width || y >> 4 >= self.map.height {
            return OUTSIDE;
        }
        let cell = self.map.cell(MAIN_PLANE, x >> 4, y >> 4);
        if cell == 0 || cell & 0x8000 != 0 {
            return 0;
        }
        match &self.groups[(cell >> 8) as usize] {
            Some(g) if ((cell & 0xff) as usize) < g.len() => g.function((cell & 0xff) as usize, n),
            _ => 0,
        }
    }

    /// TileSolidityAt (10f0:58ec): function 1 of the main-plane tile under pixel (x, y).
    pub fn solidity_at(&self, y: i32, x: i32) -> u16 {
        self.function_at(y, x, 1)
    }

    /// TileFunctionAt (10f0:587c): function 0 (10 ladder, 60600 pipe, ...).
    pub fn tile_function_at(&self, y: i32, x: i32) -> u16 {
        self.function_at(y, x, 0)
    }

    pub fn pixel_size(&self) -> (i32, i32) {
        (self.map.width * TILE, self.map.height * TILE)
    }
}
