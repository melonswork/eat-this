//! Level logic around the tiles: lever/sector scripts (OBJECTS 0x44c/0x44d, FUN_1088_a9af) and the
//! moving platforms they create (map areas cut into their own planes, FUN_1078_*). See
//! re/level.md and tools/scripts.py.

use crate::world::{FIRST_PLATFORM_PLANE, MAIN_PLANE, PLANE_SLOTS, Plane, World, func};

/// Lever tiles a sector's icon cell is set to when the hero enters / leaves it (FUN_1088_1280).
pub const SECTOR_ON_TILE: u16 = 0x308;
pub const SECTOR_OFF_TILE: u16 = 0x312;

/// One moving platform (the game's struct behind gPlatforms[i], word offsets in comments).
/// The platform sound sets (1170:0ec4): start, point reached, end, moving loop, shake amount.
const PLATFORM_SOUNDS: [[i32; 5]; 10] = [
    [0x7fff, 0x7fff, 0x7fff, 0x7fff, 4],
    [48, 51, 50, 49, 4],
    [0x7fff, 0x7fff, 62, 0x7fff, 4],
    [5, 51, 127, 49, 4],
    [146, 51, 139, 147, 4],
    [5, 0x7fff, 0x7fff, 0x7fff, 4],
    [146, 51, 139, 9, 4],
    [0x7fff, 0x7fff, 50, 0x7fff, 4],
    [5, 0x7fff, 50, 0x7fff, 4],
    [5, 0x7fff, 127, 156, 3],
];

#[derive(Clone, Debug)]
pub struct Platform {
    pub x: i32,                       // w0
    pub y: i32,                       // w1
    pub prev_x: i32,                  // w2
    pub prev_y: i32,                  // w3
    pub x16: i32,                     // w4:w5, x in 1/16 px
    pub y16: i32,                     // w6:w7
    pub width: i32,                   // w9, px
    pub height: i32,                  // w10
    pub sound: i32,                   // w11: sound set (1170:0ec0 when created)
    pub plane: usize,                 // w13
    pub flags: i32, // w14 (1170:0ebe): 1/2 bounds from the start/end point, 4 accelerate, 8 shake, 0x10 dust
    pub volume: i32, // w16 (1170:0ec2)
    pub accel: i32, // w17
    pub sector: usize, // w18:w19: the sector of the last point; the platform moves only while the hero is in it
    pub cam_lock: i32, // w20
    pub bounds: (i32, i32, i32, i32), // w21..w24: where the hero can stand on it
    pub start: (i32, i32), // w25, w26
    pub point: usize,  // w28: index of the point it is heading for
    pub points: Vec<(i32, i32, i32)>, // w29..: (x, y, speed in 1/16 px per frame)
}

/// The level's counters for the front end's stats screen (1080:1ca3): bios spheres taken
/// (1170:4ea8), hidden areas found (4eac), enemies killed (4eb0). The simulation only adds to
/// them: every `Effects` hands in its kills and secrets when it is dropped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub bios: i32,
    pub secrets: i32,
    pub kills: i32,
}

thread_local! {
    pub static TALLY: std::cell::Cell<Tally> = const { std::cell::Cell::new(Tally { bios: 0, secrets: 0, kills: 0 }) };
}

/// Changes the tally (the front end resets it when a level starts).
pub fn tally(f: impl FnOnce(&mut Tally)) {
    TALLY.with(|t| {
        let mut v = t.get();
        f(&mut v);
        t.set(v);
    });
}

impl Drop for Effects {
    fn drop(&mut self) {
        let (k, s) = (self.kills, self.secrets);
        if k != 0 || s != 0 {
            tally(|t| {
                t.kills += k;
                t.secrets += s;
            });
        }
    }
}

/// Things a script asks of the rest of the game.
#[derive(Debug, Default)]
pub struct Effects {
    pub damage_hero: Vec<i32>,
    pub grinder_platform: Option<usize>,
    pub grinder_slow: bool,
    /// ScreenShake calls (strength, amplitude), in order.
    pub shakes: Vec<(i32, i32)>,
    /// FUN_1050_5c7d calls (strength, amplitude): sideways shakes.
    pub side_shakes: Vec<(i32, i32)>,
    /// FUN_1088_3a81: the hero bounced off an enemy's head (HeroStartJump(0, -50, 10)).
    pub hero_bounce: bool,
    /// A springboard under him: (vy, jump kind or -1 for none, y, whether the Up key is used up).
    pub hero_spring: Option<(i32, i32, i32, bool)>,
    /// What stops him walking on (1170:4e16 / 4e18 for the next frame).
    pub hero_lim: (Option<i32>, Option<i32>),
    /// 1170:4e26: the hero stands by an enemy he can kick (1, or 2 with the pointer on it).
    pub hero_by_enemy: i32,
    /// Enemies killed (1170:4eb0).
    pub kills: i32,
    /// Hidden areas found (script command 0x18: 1170:4eac += 1).
    pub secrets: i32,
    /// Object tiles a script placed (FUN_1088_0d5a): (tile, cell), spawned by `Level::flush_spawns`.
    pub spawns: Vec<(u16, u16)>,
    /// Effects a script asked for (command 2, FUN_1040_60bb): (id, y, x).
    pub effects: Vec<(u16, i32, i32)>,
    /// A bot carried the hero sideways (gHeroX += dx).
    pub hero_shift: i32,
    /// Script command 0x1c: launch the hero (the START intro).
    pub hero_launch: bool,
    /// Script commands 0x0d / 0x16 (the storm), run with the Random() generator later.
    pub storm_cmds: Vec<u8>,
    /// Negative tiles (FUN_1088_1364): explode the cell with this strength at (y, x).
    pub explodes: Vec<(i32, i32, i32)>,
    /// FUN_1070_00b6: the hero stands on a moving top at this y (gHeroGround = 0x16).
    pub hero_stand: Option<i32>,
    /// Bonuses the hero caught in the air: (tile, y, x).
    pub bonus_pickups: Vec<(u16, i32, i32)>,
}

#[derive(Clone)]
pub struct Level {
    /// 1170:0f2a is 2 (a level running) rather than 1 (its setup): EnemyFactory's big aliens growl.
    pub in_play: bool,
    /// 1170:4dc2: restarts of this level (GameFrame 4e60 = 10 counts them; a start from the menu
    /// or a level end resets it). Effect 93 only clears its tile on the first two tries.
    pub tries: i32,
    /// 1170:4f0e: the wind (OBJECTS 0x2717), an angle offset for drifting smoke.
    pub wind: i32,
    /// 1170:25ae: the strength of the last tile blast whose effect ran (FUN_1050_5a33).
    pub blast_strength: i32,
    /// Script buffers as the game keeps them (1170:513c): w[0] = word count, w[1] = type, ...
    pub scripts: Vec<Vec<u16>>,
    pub platforms: Vec<Option<Platform>>,
    /// The hero's sector (DAT_1170_4656); None before the first frame.
    pub sector: Option<usize>,
    pub grinder: Option<Grinder>,
    /// The game objects (EnemyFactory & co.), and each sector's seven collections of them.
    pub objects: Vec<Option<crate::objects::Obj>>,
    pub lists: Vec<[Vec<usize>; crate::objects::list::COUNT]>,
    /// OBJECTS records type 1000 (teleport destinations, the 1170:5130 collection).
    pub teleports: Vec<Vec<i16>>,
    /// The moving doors (1170:53d2).
    pub doors: Vec<crate::doors::Door>,
    /// The doors as FUN_10a8_0dab draws them: before it moves them (only the draw reads it).
    pub doors_drawn: Vec<crate::doors::Door>,
    /// A door started moving this frame (1170:465e = 3).
    pub door_noise: bool,
    /// FUN_10a0_009b's event queue: (due frame, cell, tile, plane), a later one after an equal due.
    /// Cells a finished platform pasted that may be levers (FUN_1088_12f9), for their scripts.
    pub pasted: Vec<u16>,
    pub tile_events: Vec<(u32, u16, u16, usize)>,
    /// START's storm.
    pub storm: crate::storm::Storm,
    /// OBJECTS 0x271a: the hero rides a vehicle (FUN_1088_191d); Game::new passes it on.
    pub vehicle: bool,
    /// 1170:4ec8: the water line (the hero's sector, with its waves), computed by the foreground.
    pub water: i32,
    pub now: u32,
    /// Script command 0x1b (4ef6 = 0): the frame the skyline building of background proc 12
    /// started falling; its draw (FUN_1090_0dd0) counts 4ef6 up by 4 a frame from there.
    pub skyline_fall: Option<u32>,
    /// EnemyFactory's part of 1170:4eb2 (the kill total): enemies constructed so far (only the
    /// level setup's count, while 0f2a = 1).
    pub enemies_made: i32,
    /// Script command 0x15: the background and foreground procs (1170:189e / 18e2 indices) in
    /// place of the script's until the hero changes sector (FUN_1088_1280 restores 4c2a..4c30).
    /// The renderer and the storm follow it (`bg_proc`), the collapse doesn't.
    pub procs: Option<(u32, u32)>,
    /// gHeroX as the objects and effects see it this frame.
    pub hero_x: i32,
    /// gCameraX/Y as the scripts and decals see them.
    pub cam: (i32, i32),
    /// OBJECTS 0x2716 (FORPLANE): the quad background proc 5 fills (1170:534e / 5350).
    pub quad: Option<(i32, i32)>,
    /// GRANNY's collapsing building (OBJECTS 0x2715).
    pub collapse: Option<crate::collapse::Collapse>,
    /// The platform objects (records 100..103).
    pub plat_objs: Vec<crate::platobj::PlatObj>,
    /// The ambient sound emitters' next times (1170:4ec0, 4ebc, 4ec4).
    pub amb: [u32; 3],
    /// FUN_1058_008f's position in the collection being walked (1170:45d6).
    pub walk_index: i32,
    /// The (sector, list) being walked, for FUN_1070_0002.
    pub walking: (usize, usize),
    /// The particle engine (bullets, debris, ...): see particles.rs.
    pub particles: crate::particles::Particles,
    // Script globals.
    platform_flags: i32,  // 1170:0ebe
    platform_sound: i32,  // 1170:0ec0
    platform_volume: i32, // 1170:0ec2
}

impl Level {
    pub fn new(world: &mut World) -> Level {
        let scripts = world
            .map
            .object_records()
            .into_iter()
            .filter(|r| r[0] == 0x44c || r[0] == 0x44d)
            .map(|r| {
                std::iter::once(r.len() as u16)
                    .chain(r.iter().map(|&v| v as u16))
                    .collect::<Vec<u16>>()
            })
            .collect();
        // SpawnFromIconTiles (FUN_1088_1797), the part that matters here: SECTOR icons, plane 2 then 1.
        for plane in [2, MAIN_PLANE] {
            let Some(p) = world.planes[plane].clone() else {
                continue;
            };
            for (i, &c) in p.cells.iter().enumerate() {
                if c != 0 && c & 0x8000 == 0 && world.cell_function(c, 0) == func::SECTOR_ICON {
                    let (y, x) = ((i as i32 / p.width) * 16, (i as i32 % p.width) * 16);
                    let s = world.sector_at(y, x);
                    world.sectors[s].icon = i as i32;
                }
            }
        }
        let grinder = world
            .map
            .object_records()
            .iter()
            .find(|r| r[0] == 0x2724)
            .map(|r| Grinder::from_record(r));
        Level {
            in_play: false,
            tries: 0,
            wind: 0,
            blast_strength: 0,
            scripts,
            platforms: Vec::new(),
            sector: None,
            grinder,
            objects: Vec::new(),
            lists: vec![Default::default(); world.sectors.len()],
            teleports: Vec::new(),
            doors: Vec::new(),
            doors_drawn: Vec::new(),
            door_noise: false,
            pasted: Vec::new(),
            tile_events: Vec::new(),
            storm: Default::default(),
            vehicle: false,
            water: 0x7fff,
            now: 0,
            skyline_fall: None,
            enemies_made: 0,
            procs: None,
            hero_x: 0,
            cam: (0, 0),
            plat_objs: Vec::new(),
            collapse: None,
            quad: None,
            amb: [509, 329, 0], // HeroReset's Random(0x230) calls ran before DemoLoad zeroed the seed: the same in every demo
            walk_index: 0,
            walking: (0, 0),
            particles: Default::default(),
            platform_flags: 0,
            platform_sound: 0,
            platform_volume: 0x100,
        }
    }

    /// FUN_1088_140b: state 0..9 of a lever cell, or -1.
    pub fn lever_state(world: &World, cell: u16) -> i32 {
        let w = world.main().width;
        let f = world.tile_function_at((cell as i32 / w) * 16, (cell as i32 % w) * 16);
        if (func::LEVER_FIRST..=func::LEVER_LAST).contains(&f) {
            (f - func::LEVER_FIRST) as i32
        } else {
            -1
        }
    }

    /// FUN_1088_1280: the hero is now in `sector`.
    pub fn enter_sector(&mut self, world: &mut World, sector: usize, rng: &mut crate::rng::PascalRandom, fx: &mut Effects) {
        if self.sector == Some(sector) {
            return;
        }
        self.procs = None;
        if let Some(old) = self.sector {
            let icon = world.sectors[old].icon;
            if icon != -1 {
                self.set_cell(world, SECTOR_OFF_TILE, icon as u16, rng, fx);
            }
        }
        self.sector = Some(sector);
        let icon = world.sectors[sector].icon;
        if icon != -1 {
            self.set_cell(world, SECTOR_ON_TILE, icon as u16, rng, fx);
        }
    }

    /// FUN_1088_1364 + FUN_1088_12f9: put a tile in a main-plane cell; levers run their scripts.
    pub fn set_cell(&mut self, world: &mut World, tile: u16, cell: u16, rng: &mut crate::rng::PascalRandom, fx: &mut Effects) {
        if (tile as i16) < 0 {
            // A negative tile is a blast of strength -tile * 0x10 on a breakable cell.
            let cur = world.main().cells[cell as usize];
            // (FUN_1050_5a33 runs right here, nested.)
            if world.cell_function(cur, 1) & 0x80 != 0 {
                let w = world.main().width;
                let (row, col) = (cell as i32 / w, cell as i32 % w);
                crate::effects::explode_tile(self, world, -(tile as i16 as i32) * 0x10, 0xffff, row << 4, col << 4, rng, fx);
            }
            return;
        }
        world.main_mut().cells[cell as usize] = tile;
        if std::env::var("ET_CMD").is_ok() {
            eprintln!("SETCELL {} cell {cell} tile {tile:#x} f0 {}", self.now, world.cell_function(tile, 0));
        }
        let f = world.cell_function(tile, 0);
        if (tile as i16) > 0 && (func::LEVER_FIRST..=func::LEVER_LAST).contains(&f) {
            self.run_scripts(world, cell, rng, fx);
        } else if f == 30000 {
            // FUN_1088_12f9: the plane-2 cell is cleared, the tile flies out as a bonus.
            world.planes[2].as_mut().expect("plane 2").cells[cell as usize] = 0;
            let w = world.main().width;
            self.bonus(world, tile as i32, (cell as i32 / w) << 4, (cell as i32 % w) << 4);
        }
    }

    /// FUN_10a0_009b(delay, tile, cell) on the main plane: the cell changes later (a negative
    /// tile blows it up).
    pub fn queue_tile_event(&mut self, delay: i32, tile: u16, cell: u16) {
        let due = self.now.wrapping_add(delay as u32);
        let at = self.tile_events.iter().position(|e| due < e.0).unwrap_or(self.tile_events.len());
        self.tile_events.insert(at, (due, cell, tile, MAIN_PLANE));
    }

    /// FUN_10a0_023f's tile events that are due (FUN_10a0_00eb).
    pub fn release_tile_events(&mut self, world: &mut World, rng: &mut crate::rng::PascalRandom, fx: &mut Effects, hero_x: i32) {
        while self.tile_events.first().is_some_and(|e| e.0 <= self.now) {
            let (_, cell, tile, plane) = self.tile_events.remove(0);
            let width = world.main().width;
            let (cy, cx) = ((cell as i32 / width) << 4, (cell as i32 % width) << 4);
            if world.tile_function_at(cy, cx) == crate::doors::DOOR {
                self.door_set(world, false, (tile == 1) as i32, cy, cx, false, hero_x);
            } else if plane == MAIN_PLANE {
                self.set_cell(world, tile, cell, rng, fx);
            } else if let Some(p) = world.planes[plane].as_mut() {
                p.cells[cell as usize] = tile;
            }
        }
    }

    /// FUN_1088_a9af: run the scripts that list `cell` among their conditions and whose
    /// conditions all hold (0x44c) or don't all hold (0x44d).
    pub fn run_scripts(&mut self, world: &mut World, cell: u16, rng: &mut crate::rng::PascalRandom, fx: &mut Effects) {
        let mut i = 0;
        while i < self.scripts.len() {
            let w = self.scripts[i].clone();
            let n = (w[2] & 0xff) as usize;
            let listed = (0..n).any(|k| w[3 + 2 * k] == cell);
            if listed {
                let all = (0..n).all(|k| {
                    Self::lever_state(world, w[3 + 2 * k]) == (w[4 + 2 * k] & 0xff) as i32
                });
                let run = if w[1] == 0x44d { !all } else { all };
                if std::env::var("ET_CMD").is_ok() {
                    eprintln!("SCRIPT {} cell {cell} type {:#x} all {all} run {run} words {:x?}", self.now, w[1], w);
                }
                if run {
                    let aborted = self.run_commands(world, &w, 2 + 2 * n, rng, fx);
                    self.platform_sound = 0;
                    self.platform_volume = 0x100;
                    if w[2] >> 8 == 1 && !aborted {
                        // FUN_1088_1208: the last script moves into this slot.
                        self.scripts.swap_remove(i);
                        continue;
                    }
                }
            }
            i += 1;
        }
    }

    /// The command loop of FUN_1088_a9af. Returns true if command 7 aborted it.
    fn run_commands(
        &mut self,
        world: &mut World,
        w: &[u16],
        mut lc: usize,
        rng: &mut crate::rng::PascalRandom,
        fx: &mut Effects,
    ) -> bool {
        let mut last_platform = usize::MAX;
        let width = world.main().width;
        while lc < w[0] as usize {
            let (op, plane) = (w[lc + 1] & 0xff, (w[lc + 1] >> 8) as usize);
            let arg = |k: usize| w[lc + 1 + k];
            match op {
                0x00 => {
                    let (cell, v) = (arg(1) as i32, arg(2));
                    if std::env::var("ET_CMD").is_ok() {
                        eprintln!("CMD0 {} cell {cell} v {v:#x} f0 {} plane {plane}", self.now, world.cell_function(v, 0));
                    }
                    let (cy, cx) = ((cell / width) << 4, (cell % width) << 4);
                    if plane == MAIN_PLANE && world.tile_function_at(cy, cx) == crate::doors::DOOR {
                        // FUN_10a8_0975(0, v == 1, ...): a door opens (1) or shuts.
                        self.door_set(world, false, (v == 1) as i32, cy, cx, false, 0);
                    } else if (v as i16) > 0 && world.cell_function(v, 0) >= 61000 {
                        fx.spawns.push((v, arg(1)));
                    } else if plane == MAIN_PLANE {
                        self.set_cell(world, arg(2), arg(1), rng, fx);
                    } else if let Some(p) = world.planes[plane].as_mut() {
                        p.cells[arg(1) as usize] = arg(2);
                    }
                    lc += 3;
                }
                0x03 => {
                    let count = arg(5) as usize;
                    let start = (arg(3) as i16 as i32, arg(4) as i16 as i32);
                    let last = if count >= 2 {
                        let at = lc + 6 + (count - 2) * 3;
                        (w[at + 1] as i16 as i32, w[at + 2] as i16 as i32)
                    } else {
                        start
                    };
                    let (c1, c2) = (arg(1) as i32, arg(2) as i32);
                    let existing = self.platforms.iter().position(|p| {
                        p.as_ref().is_some_and(|p| {
                            p.points.last().map(|q| (q.0, q.1)) == Some(start) && p.start == last
                        })
                    });
                    let idx = match existing {
                        Some(i) => i,
                        None => {
                            let i = self.create_platform(world, count - 1);
                            let area = (c1 % width, c1 / width, c2 % width, c2 / width);
                            self.cut_area(world, i, area);
                            i
                        }
                    };
                    {
                        let p = self.platforms[idx].as_mut().unwrap();
                        p.start = start;
                    }
                    lc += 6;
                    let mut pts = Vec::new();
                    for _ in 0..count.saturating_sub(1) {
                        let speed = w[lc + 3] as i16 as i32;
                        pts.push((
                            w[lc + 1] as i16 as i32,
                            w[lc + 2] as i16 as i32,
                            if speed == 0 { 1 } else { speed },
                        ));
                        lc += 3;
                    }
                    self.platforms[idx].as_mut().unwrap().points = pts;
                    self.start_platform(world, idx, existing.is_some());
                    self.platform_flags = 0;
                    last_platform = idx;
                }
                0x04 => {
                    self.platform_flags |= 1;
                    lc += 1;
                }
                0x05 => {
                    self.platform_volume = arg(1) as i16 as i32;
                    lc += 2;
                }
                0x06 => {
                    self.platform_flags |= 2;
                    lc += 1;
                }
                0x07 => {
                    let c = world.planes[plane]
                        .as_ref()
                        .map_or(0, |p| p.cells[arg(1) as usize]);
                    if c & 0x8000 == 0 && c == arg(2) {
                        return true;
                    }
                    lc += 3;
                }
                0x08 => {
                    self.platform_flags = arg(1) as i16 as i32;
                    lc += 2;
                }
                0x09 => {
                    self.platform_sound = arg(1) as i16 as i32;
                    lc += 2;
                }
                0x0b => {
                    fx.damage_hero.push(arg(1) as i16 as i32);
                    lc += 2;
                }
                0x13 => {
                    fx.grinder_platform = Some(last_platform);
                    if let Some(Some(p)) = self.platforms.get_mut(last_platform) {
                        if let Some(first) = p.points.first_mut() {
                            first.2 = 0;
                        }
                    }
                    lc += 1;
                }
                0x14 => {
                    if let Some(Some(p)) = self.platforms.get_mut(last_platform) {
                        p.cam_lock = arg(1) as i16 as i32;
                    }
                    lc += 2;
                }
                0x10 => {
                    // FUN_1088_0d5a(tile, column of the cell + the row above the camera).
                    let row = ((self.cam.1 - 15) as u16 >> 4) as i32;
                    fx.spawns.push((arg(2), (arg(1) as i32 % width + row * width) as u16));
                    lc += 3;
                }
                0x0d | 0x16 => {
                    fx.storm_cmds.push(op as u8);
                    lc += 1;
                }
                0x1c => {
                    fx.hero_launch = true;
                    lc += 1;
                }
                0x1d => {
                    fx.grinder_slow = true;
                    lc += 1;
                }
                0x0a => {
                    // FUN_10a0_009b(delay, tile, cell): the tile changes (or the door moves) later.
                    let due = self.now.wrapping_add(arg(3) as u32);
                    if std::env::var("ET_DOOR").is_ok() {
                        eprintln!("EVENT at {} due {due} cell {} tile {:x}", self.now, arg(1), arg(2));
                    }
                    let at = self.tile_events.iter().position(|e| due < e.0).unwrap_or(self.tile_events.len());
                    self.tile_events.insert(at, (due, arg(1), arg(2), plane));
                    lc += 4;
                }
                0x01 | 0x0e => {
                    // fill_area / copy_area: FUN_1088_a96b's rectangle of cells (rows outer), each
                    // put through FUN_1088_a93c.
                    let (c1, c2) = (arg(1) as i32, arg(2) as i32);
                    let (x1, y1, x2, y2) = (c1 % width, c1 / width, c2 % width, c2 / width);
                    for row in y1..=y2 {
                        for col in x1..=x2 {
                            let src = (col + row * width) as usize;
                            let (tile, dest) = if op == 0x01 {
                                (arg(3), src as i32)
                            } else {
                                let cur = world.planes[plane].as_ref().map_or(0, |p| p.cells[src]);
                                (cur, src as i32 + (arg(3) as i32 - c1))
                            };
                            if (tile as i16) < 0 {
                                continue;
                            }
                            if plane == MAIN_PLANE {
                                self.set_cell(world, tile, dest as u16, rng, fx);
                            } else if let Some(p) = world.planes[plane].as_mut() {
                                p.cells[dest as usize] = tile;
                            }
                        }
                    }
                    lc += 4;
                }
                0x02 => {
                    fx.effects.push((arg(3), arg(2) as i16 as i32, arg(1) as i16 as i32));
                    lc += 4;
                }
                // The last platform's rectangle (w21..w24) in pixels.
                0x0f => {
                    if let Some(p) = self.platforms.get_mut(last_platform).and_then(|p| p.as_mut()) {
                        p.bounds = (arg(1) as i16 as i32, arg(2) as i16 as i32, arg(3) as i16 as i32, arg(4) as i16 as i32);
                    }
                    lc += 5;
                }
                // The same, from its corner cells.
                0x0c => {
                    let (c1, c2) = (arg(1) as i32, arg(2) as i32);
                    if let Some(p) = self.platforms.get_mut(last_platform).and_then(|p| p.as_mut()) {
                        p.bounds = (c1 % width * 16, c1 / width * 16, c2 % width * 16 + 15, c2 / width * 16 + 15);
                    }
                    lc += 3;
                }
                // FUN_10f0_5af5: a cell set as it is (no lever, no pick-up).
                0x17 => {
                    if let Some(c) = world.planes[plane].as_mut().and_then(|p| p.cells.get_mut(arg(1) as usize)) {
                        *c = arg(2);
                    }
                    lc += 3;
                }
                // A blast over the box of two cells (FUN_1050_4296).
                0x19 => {
                    let (c1, c2) = (arg(1) as i32, arg(2) as i32);
                    let (x1, y1) = (c1 % width * 16, c1 / width * 16);
                    let (hx, hy) = ((c2 % width * 16 + 15 - x1) >> 1, (c2 / width * 16 + 15 - y1) >> 1);
                    self.particles.blast(2, 0x600, 5, hy, hx, -hy, -hx, y1 + hy, x1 + hx);
                    lc += 3;
                }
                // The sector in which the last platform moves (w18:w19).
                0x1a => {
                    let c = arg(1) as i32;
                    let sector = world.sector_at(c / width * 16, c % width * 16);
                    if let Some(p) = self.platforms.get_mut(last_platform).and_then(|p| p.as_mut()) {
                        p.sector = sector;
                    }
                    lc += 2;
                }
                // The level's CD track.
                0x11 => {
                    crate::sound::music(arg(1) as u32, arg(2) != 0);
                    lc += 3;
                }
                0x15 => {
                    self.procs = Some((u32::from(arg(1)), u32::from(arg(2))));
                    if std::env::var("ET_CMD").is_ok() {
                        eprintln!("PROCS {} {:?}", self.now, self.procs);
                    }
                    lc += 3;
                }
                // The music's volume (MixerSetVolume), not ported.
                0x12 => lc += 2,
                // FUN_1088_a9af: a hidden area found (1170:4eac, the stats screen's count).
                0x18 => {
                    fx.secrets += 1;
                    lc += 1;
                }
                // 4ef6 = 0: background proc 12's building falls (only the draw reads it).
                0x1b => {
                    self.skyline_fall = Some(self.now);
                    lc += 1;
                }
                _ => lc += 1,
            }
        }
        false
    }

    /// FUN_1078_02e5.
    /// The hero working a lever (HeroUpdate): its sound (function 4), then FUN_1088_14a3 steps it to
    /// its next state (from the last back to the first); letting go of a spring lever steps it
    /// back (FUN_1088_1364(cell - 1)).
    pub fn work_lever(&mut self, world: &mut World, idx: i32, y: i32, x: i32, pull: bool, rng: &mut crate::rng::PascalRandom, fx: &mut Effects) {
        let width = world.main().width;
        let tile = world.main().cells[idx as usize];
        if pull {
            let id = world.cell_function(tile, 4);
            if id as i16 > 0 {
                crate::effects::effect(self, world, id, y, x, rng, fx);
            }
            let f = world.cell_function(tile, 0) as i32;
            if !(6000..=6009).contains(&f) {
                return;
            }
            let next_f = |t: u16| world.cell_function(t, 0) as i32;
            let mut t = tile;
            if f == 6009 || next_f(t + 1) != f + 1 {
                let mut ff = f;
                while ff > 6000 && next_f(t - 1) == ff - 1 {
                    t -= 1;
                    ff -= 1;
                }
            } else {
                t += 1;
            }
            self.set_cell(world, t, idx as u16, rng, fx);
        } else {
            let (cy, cx) = ((idx / width) * 16 + 7, (idx % width) * 16 + 7);
            let id = world.cell_function(tile, 4);
            if id as i16 > 0 {
                crate::effects::effect(self, world, id, cy, cx, rng, fx);
            }
            self.set_cell(world, tile - 1, idx as u16, rng, fx);
        }
    }

    /// FUN_1088_72f9(dir, y, x): the hero pushes against the wall at (x, y): a pushable block
    /// (OBJECTS record 0x4b0 in the sector's list +0x66) with room behind it slides one tile as a
    /// platform; its cell (+0x16) is touched either way.
    pub fn push_block(&mut self, world: &mut World, hero_y: i32, dir: i32, y: i32, x: i32, rng: &mut crate::rng::PascalRandom, fx: &mut Effects) {
        let wall = |yy| world.solidity_at(yy, x) & 2 != 0;
        if !((wall(hero_y - 0x10) || wall(hero_y - 0x20))
            && world.tile_function_at(hero_y - 0x10, x) != func::NOT_CUT
            && world.tile_function_at(hero_y - 0x20, x) != func::NOT_CUT)
        {
            return;
        }
        let sector = self.sector.unwrap_or(0);
        let ids: Vec<usize> = self.lists[sector][crate::objects::list::L66].clone();
        for &id in ids.iter().rev() {
            let o = self.objects[id].as_ref().unwrap();
            if !(o.w(2) <= x && o.w(4) <= y && x <= o.w(6) && y <= o.w(8)) {
                continue;
            }
            if o.w(0x12) != 0 {
                let (x1, y1, x2, y2, speed, sound) = (o.w(0xa), o.w(0xc), o.w(0xe), o.w(0x10), o.w(0x12), o.w(0x14));
                // Already sliding?
                if self.platforms.iter().flatten().any(|p| {
                    p.start.1 == y1 && p.points.last().is_some_and(|q| q.0 == x1)
                }) {
                    return;
                }
                let beyond = if dir < 0 { x1 - 1 } else { x2 + 1 };
                if blocked_behind(world, dir, y2, y1, beyond) {
                    return;
                }
                let step = dir * 0x10;
                let saved = self.platform_sound;
                self.platform_sound = sound;
                let i = self.create_platform(world, 1);
                self.platform_sound = saved;
                self.cut_area(world, i, (x1 >> 4, y1 >> 4, x2 >> 4, y2 >> 4));
                {
                    let p = self.platforms[i].as_mut().unwrap();
                    p.start = (x1, y1);
                    p.points = vec![(x1 + step, y1, speed)];
                }
                self.start_platform(world, i, false);
                let o = self.objects[id].as_mut().unwrap();
                for off in [2, 6, 0xa, 0xe] {
                    o.set(off, o.w(off) + step);
                }
            }
            let cell = self.objects[id].as_ref().unwrap().w(0x16);
            if cell != -1 {
                let tile = world.main().cells[cell as usize];
                if tile as i16 > 0 {
                    let f0 = world.cell_function(tile, 0);
                    if (func::LEVER_FIRST..=func::LEVER_LAST).contains(&f0) {
                        self.run_scripts(world, cell as u16, rng, fx);
                    } else if f0 == 30000 {
                        // FUN_1088_12f9: the plane-2 cell is cleared, the tile flies out as a bonus.
                        world.planes[2].as_mut().expect("plane 2").cells[cell as usize] = 0;
                        let w = world.main().width;
                        self.bonus(world, tile as i32, (cell / w) << 4, (cell % w) << 4);
                    }
                }
            }
        }
    }

    fn create_platform(&mut self, world: &mut World, points: usize) -> usize {
        let i = self
            .platforms
            .iter()
            .position(|p| p.is_none())
            .unwrap_or_else(|| {
                self.platforms.push(None);
                self.platforms.len() - 1
            });
        let plane = (FIRST_PLATFORM_PLANE..PLANE_SLOTS)
            .find(|&s| world.planes[s].is_none())
            .unwrap_or(PLANE_SLOTS - 1);
        world.planes[plane] = Some(Plane {
            width: 0,
            height: 0,
            cells: Vec::new(),
        });
        self.platforms[i] = Some(Platform {
            x: 0,
            y: 0,
            prev_x: 0,
            prev_y: 0,
            x16: 0,
            y16: 0,
            width: 0,
            height: 0,
            sound: self.platform_sound,
            plane,
            flags: self.platform_flags,
            volume: self.platform_volume,
            accel: 0,
            sector: 0,
            cam_lock: 0x7fff,
            bounds: (0, 0, 0, 0),
            start: (0, 0),
            point: 0,
            points: Vec::with_capacity(points),
        });
        i
    }

    /// FUN_1078_03bc(cut = 1, plane 1, ...): move a rectangle of main-plane cells into the
    /// platform's plane. "Won't be cut" tiles stay; an "area parameters" tile sets the sound.
    fn cut_area(&mut self, world: &mut World, i: usize, (x1, y1, x2, y2): (i32, i32, i32, i32)) {
        let (w, h) = (x2 - x1 + 1, y2 - y1 + 1);
        let mut cells = vec![0u16; (w * h).max(0) as usize];
        let mw = world.main().width;
        for ty in y1..=y2 {
            for tx in x1..=x2 {
                let at = (ty * mw + tx) as usize;
                let mut c = world.main().cells[at];
                let f = world.cell_function(c, 0);
                if f == func::NOT_CUT {
                    c = 0;
                } else {
                    world.main_mut().cells[at] = 0;
                    let p = self.platforms[i].as_mut().unwrap();
                    if p.sound == 0 && f == func::AREA_PARAMS {
                        p.sound = world.cell_function(c, 2) as i16 as i32;
                    }
                }
                cells[((ty - y1) * w + tx - x1) as usize] = c;
            }
        }
        let p = self.platforms[i].as_mut().unwrap();
        p.width = w * 16;
        p.height = h * 16;
        world.planes[p.plane] = Some(Plane {
            width: w,
            height: h,
            cells,
        });
    }

    /// FUN_1078_0002 (`slot` 0 start, 1 a point reached, 2 the end) and FUN_1078_0048 (`slot` 3,
    /// the loop while it moves): the platform's sound set (1170:0ec4, 10 bytes per set; 0x7fff =
    /// none) played over its box at its volume.
    fn platform_sound(&self, i: usize, slot: usize) {
        let Some(p) = self.platforms[i].as_ref() else {
            return;
        };
        let Some(&id) = PLATFORM_SOUNDS.get(p.sound as usize).map(|s| &s[slot]) else {
            return;
        };
        if id == 0x7fff {
            return;
        }
        crate::sound::set_volume(p.volume);
        if crate::sound::audible_box((p.x, p.y, p.x + p.width, p.y + p.height)) {
            if slot == 3 {
                crate::sound::priority(0x11f4);
                crate::sound::keep(crate::sound::key::platform(i), id as usize);
            } else {
                crate::sound::play(id as usize);
            }
        }
        crate::sound::set_volume(0x100);
    }

    /// FUN_1078_05a0.
    fn start_platform(&mut self, world: &World, i: usize, reverse: bool) {
        let p = self.platforms[i].as_mut().unwrap();
        if reverse {
            p.point = p.points.len().saturating_sub(1) - p.point;
            return;
        }
        (p.x, p.y) = p.start;
        p.x16 = p.x << 4;
        p.y16 = p.y << 4;
        (p.prev_x, p.prev_y) = (p.x, p.y);
        p.point = 0;
        p.accel = 1;
        self.platform_sound(i, 0);
        let p = self.platforms[i].as_mut().unwrap();
        let last = *p.points.last().unwrap_or(&(p.x, p.y, 0));
        p.sector = world.sector_at(last.1, last.0);
        p.bounds = if p.flags & 3 == 0 {
            world.sectors[p.sector].cam
        } else {
            let (bx, by) = if p.flags & 1 == 0 {
                (p.x, p.y)
            } else {
                (last.0, last.1)
            };
            (bx, by, bx + p.width - 1, by + p.height - 1)
        };
    }

    /// FUN_1078_0aef: move the platforms of the hero's sector one step along their paths.
    /// Screen shakes go to `shakes` (they act at once: the caller applies them).
    pub fn update_platforms(&mut self, world: &mut World, cam_y: i32, rng: &mut crate::rng::PascalRandom, shakes: &mut Vec<(i32, i32)>) -> Option<i32> {
        let mut cam_offset = None;
        for i in 0..self.platforms.len() {
            let Some(p) = self.platforms[i].as_mut() else {
                continue;
            };
            if Some(p.sector) != self.sector {
                continue;
            }
            (p.prev_x, p.prev_y) = (p.x, p.y);
            if p.cam_lock != 0x7fff {
                // 1170:4e36, where the hero rests on screen, pinned to the platform.
                cam_offset = Some((p.cam_lock + 0x7d) - p.y + cam_y);
            }
            let Some(&(px, py, speed)) = p.points.get(p.point) else {
                continue;
            };
            let (tx, ty) = (px << 4, py << 4);
            let mut arrived = false;
            if p.x16 == tx {
                let dir = if ty - p.y16 < 0 { -1 } else { 1 };
                if p.flags & 4 == 0 {
                    p.y16 += dir * speed;
                } else {
                    p.y16 += p.accel;
                    p.accel += if p.accel >> 2 == 0 { 1 } else { p.accel >> 2 };
                    p.accel = p.accel.min(speed);
                }
                let passed = p.y16 == ty || (p.y16 - ty).signum() == dir;
                if passed {
                    p.y16 = ty;
                    arrived = p.point + 1 < p.points.len() || (p.prev_y << 4) == ty;
                }
            } else {
                let dir = if tx - p.x16 < 0 { -1 } else { 1 };
                p.x16 += dir * speed;
                if p.x16 == tx || (p.x16 - tx).signum() == dir {
                    p.x16 = tx;
                    arrived = true;
                }
            }
            p.x = p.x16 >> 4;
            p.y = p.y16 >> 4;
            if std::env::var_os("ET_PLATMOVE").is_some() {
                eprintln!("PLATMOVE {} #{i} x {} y {} accel {} point {} arrived {arrived} flags {}", self.now, p.x, p.y, p.accel, p.point, p.flags);
            }
            let (flags, set) = (p.flags, p.sound);
            let last = p.points.last().copied().unwrap_or((0, 0, 0));
            let (pw, ph) = (p.width, p.height);
            let (sector, rect) = (p.sector, (p.x, p.y, p.x + p.width - 1, p.y + p.height - 1));
            let mut done = false;
            if arrived {
                p.point += 1;
                done = p.point == p.points.len();
            }
            self.platform_sound(i, 3);
            // Flags 8: it shakes the screen every 6 frames (its sound set's amount); 0x10: dust
            // falls from under its last point every 8 frames.
            if flags & 8 != 0 && self.now % 6 == 0 {
                let amp = PLATFORM_SOUNDS.get(set as usize).map_or(4, |r| r[4]);
                shakes.push((0x10, amp));
            }
            if flags & 0x10 != 0 && self.now % 8 == 0 {
                let dy = last.1 + ph - 1;
                let dx = last.0 + rng.random(pw as u16);
                let ps = &mut self.particles;
                ps.p.colour = 0x10;
                ps.spawn_seq(0x6d00, 2, 0x13, dy, dx, 1); // FUN_1040_66d4
                ps.p.colour = 0;
                let m = &mut ps.last_mut().m; // FUN_1050_4a29(0x200, 0x100)
                m.set(0x1c, 0x100);
                m.set(0x1e, 0x200);
                m.set(0x42, 0x2148);
            }
            self.wake_actors(sector, rect);
            if done && flags & 4 != 0 {
                shakes.push((0x1e, 10));
            }
            if arrived {
                self.platform_sound(i, if done { 2 } else { 1 });
            }
            if done {
                crate::sound::stop(crate::sound::key::platform(i)); // FUN_1078_00ae
                self.paste_back(world, i);
            }
        }
        cam_offset
    }

    /// FUN_1058_0114: actors of the sector whose box meets the rect get an unbounded box, so
    /// they update wherever the camera is (riding a platform). Clears all but bit 0 of +0xc.
    fn wake_actors(&mut self, sector: usize, (left, top, right, bottom): (i32, i32, i32, i32)) {
        for &id in &self.lists[sector][crate::objects::list::ACTORS] {
            let o = self.objects[id].as_mut().unwrap();
            o.set(0xc, o.w(0xc) & 1);
            if o.w(0xc) != 0 && o.w(2) <= right && o.w(4) <= bottom && left <= o.w(6) && top <= o.w(8) {
                o.set(2, -0x7fff);
                o.set(4, -0x7fff);
                o.set(6, 0x7fff);
                o.set(8, 0x7fff);
            }
        }
    }

    /// End of a path: FUN_1078_0765(1, i) pastes the platform's cells into the main plane, then
    /// FUN_1078_00ae frees it.
    fn paste_back(&mut self, world: &mut World, i: usize) {
        let p = self.platforms[i].take().unwrap();
        if let Some(src) = world.planes[p.plane].take() {
            let (mw, mh) = (world.main().width, world.main().height);
            let (x0, y0) = (p.x / 16, p.y / 16);
            let lo = ((x0).max(p.bounds.0 >> 4), (y0).max(p.bounds.1 >> 4));
            let hi = (
                (x0 + src.width - 1).min(p.bounds.2 >> 4),
                (y0 + src.height - 1).min(p.bounds.3 >> 4),
            );
            for ty in lo.1..=hi.1 {
                for tx in lo.0..=hi.0 {
                    if tx < 0 || ty < 0 || tx >= mw || ty >= mh {
                        continue;
                    }
                    let c = src.get(tx - x0, ty - y0);
                    if c != 0 {
                        world.main_mut().cells[(ty * mw + tx) as usize] = c;
                    }
                }
            }
            // FUN_1088_12f9 on each cell of the rectangle: levers run their scripts.
            for ty in lo.1..=hi.1 {
                for tx in lo.0..=hi.0 {
                    if tx < 0 || ty < 0 || tx >= mw || ty >= mh {
                        continue;
                    }
                    let cell = (ty * mw + tx) as usize;
                    let t = world.main().cells[cell];
                    let f = world.cell_function(t, 0);
                    if (t as i16) > 0 && (func::LEVER_FIRST..=func::LEVER_LAST).contains(&f) {
                        self.pasted.push(cell as u16);
                    }
                }
            }
        }
        while self.platforms.last().is_some_and(|p| p.is_none()) {
            self.platforms.pop();
        }
    }
}

/// The grinder (OBJECTS type 0x2724, GrinderUpdate 1090:1a91): a platform that climbs the shaft
/// once its trigger lever is on, crushing every row of "grinder food" tiles (function 15000).
#[derive(Clone, Debug)]
pub struct Grinder {
    pub y: i32,                  // 1170:4ee0: the top of the grinder
    top: i32,                    // 4ee2: where it stops
    y8: i32,                     // 4ef2:4ef4: y in 1/512 px
    pub speed: i32,              // 4eea: -1 until triggered, then 1/512 px per frame
    max_speed: i32,              // 4eec
    trigger: u16,                // 4eee: lever cell
    pub cam_offset: i32,         // 4ee6: the camera sits this far above it
    anim: i32,                   // 4ef0
    pub platform: Option<usize>, // 4ee4
    /// (4ef0, 4ee0) as GrinderUpdate draws the teeth: before its own update.
    drawn: (i32, i32),
}

/// Grinder food (TILES.TXT has no name for it; GrinderUpdate tests it).
pub const GRINDER_FOOD: u16 = 15000;
/// Tile function "Sensitive lever": touching it advances the whole block of it (FUN_1088_1536).
pub const SENSITIVE_LEVER: u16 = 7030;

impl Grinder {
    /// LevelInit, record type 0x2724: [type, ?, y, top, trigger cell, camera offset].
    pub fn from_record(r: &[i16]) -> Grinder {
        let y = r[2] as i32;
        Grinder {
            y,
            top: r[3] as i32,
            y8: y << 9,
            speed: -1,
            max_speed: 0x180,
            trigger: r[4] as u16,
            cam_offset: r[5] as i32,
            anim: 0x400,
            platform: None,
            drawn: (0x400, y),
        }
    }
}

impl Level {
    /// FUN_1088_1536: the hero touched a sensitive-lever tile at (tx, ty): the whole rectangle of
    /// that same cell value becomes `tile + function 2`, cell by cell (levers run their scripts).
    pub fn touch_sensitive(
        &mut self,
        world: &mut World,
        ty: i32,
        tx: i32,
        rng: &mut crate::rng::PascalRandom,
        fx: &mut Effects,
    ) {
        let (w, h) = (world.main().width, world.main().height);
        let at = |world: &World, r: i32, c: i32| world.main().cells[(r * w + c) as usize];
        let tile = at(world, ty, tx);
        // HeroTouchTiles tests its two points one after the other; the first may have changed
        // the second already.
        if world.cell_function(tile, 0) != SENSITIVE_LEVER {
            return;
        }
        let step = world.cell_function(tile, 2) as i16 as i32;
        crate::effects::effect(
            self,
            world,
            world.cell_function(tile, 4),
            ty << 4,
            tx << 4,
            rng,
            fx,
        );
        let mut left = tx;
        while left > 0 && at(world, ty, left - 1) == tile {
            left -= 1;
        }
        let mut top = ty;
        while top >= 1 && at(world, top - 1, left) == tile {
            top -= 1;
        }
        let mut right = tx;
        while right < w - 1 && at(world, ty, right + 1) == tile {
            right += 1;
        }
        let mut bottom = ty;
        while bottom < h - 1 && at(world, bottom + 1, right) == tile {
            bottom += 1;
        }
        let new = (tile as i32 + step) as u16;
        for r in top..=bottom {
            for c in left..=right {
                self.set_cell(world, new, (r * w + c) as u16, rng, fx);
            }
        }
        // Plane 2's cell at the block's last corner = 0x316: a hidden area found (4eac), once.
        if let Some(cell) = world.planes[2].as_mut().and_then(|p| p.cells.get_mut((bottom * w + right) as usize)) {
            if *cell == 0x316 {
                *cell = 0;
                fx.secrets += 1;
            }
        }
    }

    /// GrinderUpdate (1090:1a91), the parts that change the game: trigger, climb, platform,
    /// hurting the hero, crushing rows. Returns true if it reached the hero at full speed.
    pub fn update_grinder(
        &mut self,
        world: &mut World,
        hero: (i32, i32, i32, bool),
        cam_x: i32,
        rng: &mut crate::rng::PascalRandom,
        fx: &mut Effects,
    ) -> bool {
        let Some(g) = self.grinder.as_mut() else {
            return false;
        };
        let (hero_x, hero_y, hero_crouch, _) = hero;
        g.drawn = (g.anim, g.y);
        if g.speed < 0 {
            if Self::lever_state(world, g.trigger) > 0 {
                g.speed = 2;
                if crate::sound::audible_box((cam_x + 0x20, g.y, cam_x + 0x120, g.y)) {
                    crate::sound::play(0x33);
                }
            }
        } else {
            if g.top < g.y {
                if g.speed < g.max_speed {
                    g.speed += 2;
                }
                let climb = hero_y < g.y - 1 || {
                    let s = world.solidity_at(hero_y - 0x18, hero_x);
                    s & crate::world::solid::WALL == 0 || hero_crouch < 8
                };
                if climb {
                    g.y8 -= g.speed;
                }
                g.y = g.y8 >> 9;
            } else if g.speed > 0 {
                g.speed -= 1;
            }
            g.anim += g.speed;
        }
        if let Some(Some(p)) = g.platform.and_then(|i| self.platforms.get_mut(i)) {
            p.prev_y = p.y;
            p.y = g.y;
            p.y16 = g.y << 4;
        }
        let g = self.grinder.as_mut().unwrap();
        let mut hit_hero = false;
        if g.y - 10 <= hero_y && g.speed > 0xbf {
            hit_hero = true; // FUN_1088_3c32(0xc0); on a kill also FUN_1090_1a70
        }
        let y = g.y - 6;
        let mut crushed = false;
        for k in (0..=15).rev() {
            let x = k * 16 + 0x20 + cam_x;
            if world.tile_function_at(y, x) == GRINDER_FOOD {
                crate::effects::explode_tile(self, world, 0x60, 0x80, y, x, rng, fx);
                crushed = true;
            }
        }
        if crushed {
            let g = self.grinder.as_mut().unwrap();
            g.slow_down(rng);
            // The crunch sound's pitch, drawn when it is audible (always so far: the camera
            // follows the grinder).
            let p = rng.random(0x50);
            crate::sound::set_volume(0xaa);
            if crate::sound::audible_box((cam_x + 0x20, g.y, cam_x + 0x120, g.y)) {
                crate::sound::pitch(0x100 - p);
                crate::sound::play(0x4a);
            }
            fx.shakes.push((0x18, 10));
        }
        // The engine's hum (handle 4ee8): louder and lower as it speeds up.
        let g = self.grinder.as_ref().unwrap();
        let vol = ((g.speed as i64) << 8) / g.max_speed as i64;
        crate::sound::set_volume(vol as i32);
        if crate::sound::audible_box((cam_x + 0x20, g.y - 0x40, cam_x + 0x120, g.y)) {
            crate::sound::pitch(0x180 - ((vol as i32 as u16) >> 1) as i32);
            crate::sound::priority(0x11fe);
            crate::sound::keep(crate::sound::key::GRINDER, 0x93);
        }
        crate::sound::set_volume(0x100);
        hit_hero
    }
}

impl Grinder {
    /// 1170:4ef0 and 4ee0 as the teeth are drawn (GrinderUpdate draws them first).
    pub fn drawn(&self) -> (i32, i32) {
        self.drawn
    }

    /// FUN_1090_1a70.
    pub fn slow_down(&mut self, rng: &mut crate::rng::PascalRandom) {
        self.speed -= rng.random(0x10) + 0x60;
        if self.speed < 0 {
            self.speed = 0;
        }
    }
}

/// FUN_1088_7251: something solid in the column the block would move into.
fn blocked_behind(world: &World, dir: i32, y2: i32, mut y: i32, x: i32) -> bool {
    while y < y2 {
        let f = world.tile_function_at(y, x);
        if f == func::NOT_CUT || world.tile_function_at(y, x - dir) == func::NOT_CUT {
            let mut lx = x - dir;
            while world.tile_function_at(y, lx) == func::NOT_CUT {
                lx -= dir * 0x10;
            }
            if world.cell_at(crate::world::MAIN_PLANE, y, lx).unwrap_or(0) != 0 {
                return true;
            }
        } else if world.solidity_at(y, x) & 0x22 != 0 || f == 0xec54 {
            return true;
        }
        y += 0x10;
    }
    false
}
