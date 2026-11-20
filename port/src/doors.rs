//! Doors (segment 10a8): blocks of function-7050 tiles. A door being opened or closed is a record
//! in the 1170:53d2 list (count 19d8) that animates `progress` from `full` (closed) to 0 (open);
//! the tiles switch at once when it starts opening (each cell plus its function 3) and back when
//! it has closed. Doors with a 7060 marker in plane 2 left of their top open by themselves near
//! the hero; the others are shot open: plane 2 holds their hit points (negative) right of the
//! marker column, one row down. A plane-2 function >= 61000 two rows above the bottom is an
//! alien waiting behind: it comes out when the door is open.

use crate::factory::{Ctx, Rec};
use crate::level::{Effects, Level};
use crate::world::{MAIN_PLANE, World};

/// Tile function of door cells.
pub const DOOR: u16 = 7050;
/// FUN_1040_60bb with a door type's effect: they are all sounds (30000 + id) or none.
fn door_sound(effect: u16, y: i32, x: i32) {
    if (30000..0x8000).contains(&effect) {
        crate::sound::play_at(effect as usize - 30000, y, x);
    }
}

/// Plane-2 marker of the doors that open by themselves.
const AUTO_MARK: u16 = 7060;

/// The door types (1170:19dc, 0xd bytes): (full, effect when it starts moving, effect when it
/// has closed, step per frame). The effects are all sounds. Each type's procedure (19de) only
/// draws, except type 3's dust (FUN_10a8_02ce).
const TYPES: [(i32, u16, u16, i32); 9] = [
    (32, 30053, 30040, 2),
    (16, 30053, 30052, 1),
    (16, 30126, 0, 1),
    (32, 30125, 30040, 1),
    (32, 30143, 30052, 1),
    (32, 30143, 30052, 1),
    (32, 30125, 30052, 1),
    (32, 30053, 30052, 1),
    (21, 30143, 0, 1),
];

/// A door record (0x15 bytes).
#[derive(Clone, Debug)]
pub struct Door {
    pub x: i32,        // +0: left, tiles
    pub y: i32,        // +2: top, tiles
    pub cell: i32,     // +4: top-left cell index
    pub w: i32,        // +6: tiles
    pub h: i32,        // +8
    pub progress: i32, // +0xa: full = closed, 0 = open
    pub monster: i32,  // +0xc: the plane-2 function (>= 61000) of the alien behind it, or 0
    pub hero_left: i32, // +0xe
    pub cx: i32,       // +0x10: middle, px
    pub kind: usize,   // +0x12: function 4 of its cells
    pub open: i32,     // +0x13: 1 opening, 0 closing
    pub auto: bool,    // +0x14
}

fn fn_at(w: &World, plane: usize, y: i32, x: i32) -> u16 {
    w.tile_function_in(plane, y, x)
}

/// The top-left corner of the door block around (y, x).
fn top_left(w: &World, mut y: i32, mut x: i32) -> (i32, i32) {
    while fn_at(w, MAIN_PLANE, y, x - 0x10) == DOOR {
        x -= 0x10;
    }
    while fn_at(w, MAIN_PLANE, y - 0x10, x) == DOOR {
        y -= 0x10;
    }
    (y, x)
}

fn cell_index(w: &World, y: i32, x: i32) -> i32 {
    (y >> 4) * w.main().width + (x >> 4)
}

impl Level {
    /// FUN_10a8_0d16(y, x): the state of the door here (1 opening, 0 closing), -1 if it isn't
    /// moving.
    pub fn door_state(&self, w: &World, y: i32, x: i32) -> i32 {
        let (y, x) = top_left(w, y, x);
        let idx = cell_index(w, y, x);
        self.doors.iter().find(|d| d.cell == idx).map_or(-1, |d| d.open)
    }

    /// FUN_10a8_0975(auto, open, y, x): start a door opening (`open` 1) or closing. `first`
    /// is 1170:19d6 (LevelFirstFrame: no effects, already at the end). Sets 1170:465e = 3.
    pub fn door_set(&mut self, w: &mut World, auto: bool, open: i32, y: i32, x: i32, first: bool, hero_x: i32) {
        let width = w.main().width;
        let c = w.main().cells[((y as u16 >> 4) as i32 * width + (x as u16 >> 4) as i32) as usize];
        let closed = w.cell_function(c, 2) == 0;
        if open == 0 && closed {
            return;
        }
        let (y0, x0) = (y, x);
        let (y, x) = top_left(w, y, x);
        if auto && open == 1 && fn_at(w, 2, y, x - 0x10) != AUTO_MARK {
            return;
        }
        let idx = cell_index(w, y, x);
        self.door_noise = true;
        if let Some(d) = self.doors.iter_mut().find(|d| d.cell == idx) {
            d.open = open;
            return;
        }
        if !closed && open == 1 {
            return;
        }
        let mut x1 = x0;
        while fn_at(w, MAIN_PLANE, y0, x1 + 0x10) == DOOR {
            x1 += 0x10;
        }
        let mut y1 = y0;
        while fn_at(w, MAIN_PLANE, y1 + 0x10, x1) == DOOR {
            y1 += 0x10;
        }
        let kind = w.cell_function(c, 4) as usize;
        let (dw, dh) = (((x1 - x) >> 4) + 1, ((y1 - y) as u16 >> 4) as i32 + 1);
        let (tx, ty) = (idx % width, idx / width);
        let (full, _, _, speed) = TYPES[kind];
        let mut d = Door {
            x: tx,
            y: ty,
            cell: idx,
            w: dw,
            h: dh,
            progress: speed,
            monster: 0,
            hero_left: 0,
            cx: tx * 0x10 + dw * 8,
            kind,
            open,
            auto,
        };
        if !first {
            d.progress = if closed { full - speed } else { speed };
            // FUN_1040_60bb: the type's start sound at the door's middle.
            door_sound(TYPES[kind].1, d.y * 0x10 + d.h * 8, d.cx);
        }
        if closed {
            let p2 = w.planes[2].as_ref().unwrap().cells[(idx + (dh - 2) * width) as usize];
            let m = w.cell_function(p2, 0) as i32;
            if m > 60999 {
                d.monster = m;
                d.hero_left = (hero_x < d.cx) as i32;
            }
            for r in 0..dh {
                for col in (0..dw).rev() {
                    let i = (idx + r * width + col) as usize;
                    let v = w.main().cells[i];
                    w.main_mut().cells[i] = v.wrapping_add(w.cell_function(v, 3));
                }
            }
        }
        self.doors.push(d);
    }

    /// FUN_10a8_1311: the doors marked to open by themselves within 0x40 px of the hero
    /// (on his row, with no door 0x20 below).
    pub fn doors_near_hero(&mut self, w: &mut World, hero: (i32, i32), first: bool) {
        let (hx, hy) = hero;
        for k in 0..9 {
            let x = hx - 0x40 + k * 0x10;
            if fn_at(w, MAIN_PLANE, hy, x) == DOOR && fn_at(w, MAIN_PLANE, hy + 0x20, x) != DOOR {
                self.door_set(w, true, 1, hy, x, first, hx);
            }
        }
    }

    /// FUN_10a8_1369(damage, y, x): a shot or a blast at a closed door wears its hit points down;
    /// at none it opens.
    pub fn door_hit(&mut self, w: &mut World, damage: i32, y: i32, x: i32, hero_x: i32, fx: &mut Effects) {
        let c = w.cell_at(MAIN_PLANE, y, x).unwrap_or(0);
        if w.cell_function(c, 2) != 0 {
            return;
        }
        let (y, x) = top_left(w, y, x);
        let (hy, hx) = (y + 0x10, x - 0x10);
        let Some(v) = w.cell_at(2, hy, hx) else {
            return;
        };
        let hp = if (v as i16) < 0 { -(v as i16 as i32) } else { 0 };
        if hp <= 0 {
            return;
        }
        let width = w.main().width;
        let i = ((hy >> 4) * width + (hx >> 4)) as usize;
        if hp - damage < 1 {
            w.planes[2].as_mut().unwrap().cells[i] = 0;
            self.door_set(w, false, 1, y, x, false, hero_x);
            fx.shakes.push((0xe, 0xc));
        } else {
            w.planes[2].as_mut().unwrap().cells[i] = (-(hp - damage)) as i16 as u16;
            fx.shakes.push((0xc, 4));
        }
    }

    /// FUN_10a8_0dab: the moving doors. Automatic ones follow the hero's distance; a door's
    /// alien comes out once it is open; finished doors leave the list (the last takes their
    /// place), a closed one getting its door tiles back.
    pub fn update_doors(&mut self, w: &mut World, hero: (i32, i32), first: bool, cx: &mut Ctx) {
        let width = w.main().width;
        let (htx, hty) = (hero.0 >> 4, hero.1 >> 4);
        let mut i = 0;
        while i < self.doors.len() {
            let d = &mut self.doors[i];
            let (full, _, _, speed) = TYPES[d.kind];
            if std::env::var("ET_DOOR").is_ok() {
                eprintln!("DOOR {:?}", (d.kind, d.auto, d.open, d.progress, d.monster, d.x, d.y, d.w, d.h));
            }
            if d.auto {
                let (a, b) = (d.x - 5, d.x + d.w + 4);
                let (c, e) = (d.y + d.h - 4, d.y + d.h);
                let near = a.min(b) <= htx && htx <= a.max(b) && c.min(e) <= hty && hty <= c.max(e);
                let want = near as i32;
                if d.open != want {
                    d.open = want;
                    if !first {
                        door_sound(TYPES[d.kind].1, d.y * 0x10 + d.h * 8, d.cx);
                    }
                }
            }
            let p = if first {
                if d.open == 1 { 0 } else { full }
            } else if d.open == 1 {
                (d.progress - speed).max(0)
            } else {
                speed + d.progress
            };
            if d.monster != 0 && p == 0 {
                let kind = if d.monster == 61400 { 0x1428 } else { 0x1432 };
                let (x, y) = (d.cx, (d.y + d.h) * 0x10 - 1);
                let rec = [0i16, 0, x as i16, y as i16, (x - 0xc0) as i16, (x + 0xc0) as i16];
                let cell = (d.y + d.h - 2) * width + d.x;
                d.monster = 0;
                self.factory(w, cx, kind, &Rec { w: &rec, count: 6 });
                w.planes[2].as_mut().unwrap().cells[cell as usize] = 0;
            }
            let d = &mut self.doors[i];
            if d.kind == 3 && full - 1 == d.progress && d.open == 0 {
                // FUN_10a8_02ce: dust where it shuts.
                let (y, x) = (d.y * 0x10 + 0x40, d.cx);
                let ps = &mut self.particles;
                ps.p.group = 0;
                ps.p.colour = 0x10;
                ps.spawn_seq(0x6d00, 2, 0x13, y, x, 1);
                ps.p.colour = 0;
                let m = &mut ps.last_mut().m;
                m.set(0x1c, 0x180);
                m.set(0x1e, 0x200);
                m.set(0x42, 0x2148);
                ps.p.group = 1;
            }
            let d = &mut self.doors[i];
            d.progress = p;
            if p < full && (d.auto || p != 0) {
                i += 1;
                continue;
            }
            if p != 0 {
                let first_cell = w.main().cells[d.cell as usize];
                let mut tile = first_cell.wrapping_add(w.cell_function(first_cell, 3));
                let mut cell = d.cell;
                for _ in 0..d.h {
                    for col in (0..d.w).rev() {
                        w.main_mut().cells[(cell + col) as usize] = tile.wrapping_add(col as u16);
                    }
                    cell += width;
                    tile = tile.wrapping_add(0x14);
                }
                d.cell = cell;
                if !first {
                    door_sound(TYPES[d.kind].2, d.y * 0x10 + d.h * 8, d.cx);
                }
            }
            self.doors.swap_remove(i);
        }
    }
}
