//! Updating the game objects: the collection walk (FUN_1058_008f), the actor physics shared by
//! the walking enemies (FUN_1068_119e), and the classes' update methods (VMT +0xc), translated
//! from the decompilation field by field. Drawing is left out.

use crate::level::{Effects, Level};
use crate::objects::list;
use crate::rng::PascalRandom;
use crate::world::{OUTSIDE, World};

/// The frame state the object methods read (globals in the original).
pub struct Env<'a> {
    pub world: &'a mut World,
    pub rng: &'a mut PascalRandom,
    pub fx: &'a mut Effects,
    pub hero: (i32, i32, i32), // gHeroX, gHeroY, gHeroHeight
    pub frame: u32,            // DAT_1170_617e, the level frame counter
}

impl Level {
    /// FUN_1070_faec: the hero's sector's actors (list +0x36) and triggers (+0x4e), then the
    /// default sector's actors. Objects update only while their box meets `window` (the camera
    /// rectangle, 1170:45ca..45d0).
    pub fn update_actors(&mut self, env: &mut Env, window: (i32, i32, i32, i32)) {
        let sector = self.sector.unwrap_or(0);
        self.walk(sector, list::ACTORS, env, window);
        self.update_triggers(sector, env);
        if sector != 0 {
            self.walk(0, list::ACTORS, env, window);
        }
    }

    /// FUN_1058_008f. Objects removed during the walk keep the walk's position right
    /// (the original's 1170:45d6 bookkeeping).
    fn walk(&mut self, sector: usize, l: usize, env: &mut Env, window: (i32, i32, i32, i32)) {
        let mut i = 0;
        while i < self.lists[sector][l].len() {
            let id = self.lists[sector][l][i];
            let active = self.objects[id].as_ref().is_some_and(|o| {
                let (x1, y1, x2, y2) = o.bbox();
                x1 <= window.2 && y1 <= window.3 && window.0 <= x2 && window.1 <= y2
            });
            if active {
                self.walk_index = i as i32;
                self.update_object(id, env);
                i = (self.walk_index + 1) as usize;
            } else {
                i += 1;
            }
        }
    }

    /// FUN_1068_08ff: rectangle triggers set a lever cell when the hero enters / leaves them.
    fn update_triggers(&mut self, sector: usize, env: &mut Env) {
        let ids: Vec<usize> = self.lists[sector][list::TRIGGERS]
            .iter()
            .rev()
            .copied()
            .collect();
        let (hx, hy, _) = env.hero;
        for id in ids {
            let Some(o) = self.objects[id].as_ref() else {
                continue;
            };
            let inside = o.w(2) <= hx && o.w(4) <= hy && hx <= o.w(6) && hy <= o.w(8);
            let (cell, out_tile, in_tile, was_in) =
                (o.uw(10), o.uw(0xc), o.uw(0xe), o.b(0x10) != 0);
            if !was_in && inside {
                self.set_cell(env.world, in_tile, cell, env.fx);
                self.objects[id].as_mut().unwrap().set_b(0x10, 1);
            } else if was_in && !inside {
                self.set_cell(env.world, out_tile, cell, env.fx);
                self.objects[id].as_mut().unwrap().set_b(0x10, 0);
            }
        }
    }

    fn update_object(&mut self, id: usize, env: &mut Env) {
        let kind = self.objects[id].as_ref().unwrap().kind;
        match kind {
            0x145a => self.smallbot(id, env),
            0x14aa => {} // FUN_1070_a06a: draws a sprite
            0x13c4 => {} // FUN_1068_315a: plain sound emitters (kinds < 10000) don't draw Random
            _ => {}
        }
    }

    fn o(&mut self, id: usize) -> &mut crate::objects::Obj {
        self.objects[id].as_mut().unwrap()
    }

    /// FUN_1068_1765: grow the activity box around the actor's patrol range.
    fn patrol_box(&mut self, id: usize, bottom: i32, right: i32, top: i32, left: i32) {
        let o = self.o(id);
        if o.w(0x14) == -0x7fff || o.w(2) == -0x7fff {
            o.set(2, left);
            o.set(6, right);
        } else {
            if left < o.w(2) {
                o.set(2, left);
            }
            if o.w(6) < right {
                o.set(6, right);
            }
        }
        o.set(4, top);
        o.set(8, bottom);
    }

    /// FUN_1068_119e(obj, height, half_width, foot_half_width): fall, land on platforms and
    /// tiles (slopes too), stop at side walls, then the class's move hook (VMT +0x10).
    /// Returns 1 if squeezed or inside a wall, 2 on a trap tile.
    pub fn actor_physics(
        &mut self,
        id: usize,
        env: &mut Env,
        height: i32,
        half: i32,
        foot: i32,
    ) -> i32 {
        let w: &World = env.world;
        let mut result = 0;
        let mut side = 0;
        let mut carry = 0;
        let y_old;
        {
            let o = self.o(id);
            o.set(0x1c, 0);
            y_old = o.w(0x10);
            if o.w(0x1a) > 0 {
                let t = o.w(0x1a);
                o.set(0x10, (t * t) / 8 + o.w(0x18));
            }
        }
        // Platforms.
        for pi in (0..self.platforms.len()).rev() {
            let Some(p) = self.platforms[pi].clone() else {
                continue;
            };
            let o = self.objects[id].as_ref().unwrap();
            let (x, y) = (o.w(0xe), o.w(0x10));
            let (right, left, top) = (x + half, x - half, y - height);
            let overlaps =
                !(right < p.x || y + 3 < p.y || p.x + p.width <= left || p.y + p.height <= top);
            if !overlaps {
                continue;
            }
            let mut ly = (y + 1) - p.y;
            if p.prev_y - p.y < 0 {
                ly -= p.prev_y - p.y;
            }
            let x_end = x + foot;
            let mut lx = (x - foot) - 0x10;
            loop {
                lx += 0x10;
                if x_end < lx {
                    lx = x_end;
                }
                let s = w.solidity_in(p.plane, ly, lx - p.x);
                if s != OUTSIDE && s & 0x211 != 0 {
                    let o = self.o(id);
                    o.set(0x10, (ly & !15) + p.y - 1);
                    o.set(0x1c, 1);
                    carry = p.x - p.prev_x;
                    o.set(2, -0x7fff);
                    o.set(4, -0x7fff);
                    o.set(6, 0x7fff);
                    o.set(8, 0x7fff);
                    o.set(0x14, -0x7fff);
                    o.set(0x16, 0x7fff);
                    break;
                }
                if lx >= x_end {
                    break;
                }
            }
            let mut ly = self.objects[id].as_ref().unwrap().w(0x10);
            loop {
                ly -= 0x10;
                if ly < top {
                    ly = top;
                }
                let s = w.solidity_in(p.plane, ly - p.y, left - p.x);
                if s != OUTSIDE && s & 8 != 0 {
                    self.o(id)
                        .set(0xe, p.x + ((left - p.x) & !15) + 0x10 + half);
                    env.fx.shakes.push((4, 4));
                    if side < 0 { result = 1 } else { side = 1 }
                }
                let s = w.solidity_in(p.plane, ly - p.y, right - p.x);
                if s != OUTSIDE && s & 8 != 0 {
                    self.o(id)
                        .set(0xe, p.x + (((right - p.x) & !15) - 1 - half));
                    env.fx.shakes.push((4, 4));
                    if side < 1 { side = -1 } else { result = 1 }
                }
                if top >= ly {
                    break;
                }
            }
        }
        {
            let o = self.o(id);
            o.set(0xe, o.w(0xe) + carry);
        }
        carry = 0;
        let (x, y) = {
            let o = self.objects[id].as_ref().unwrap();
            (o.w(0xe), o.w(0x10))
        };
        // Slopes.
        let mut slope_y = 0x7fff;
        let s = w.solidity_at(y + 1, x);
        if s & 0x20 != 0 {
            slope_y = slope_height(w, x, w.cell_at(1, y + 1, x).unwrap_or(0)) + ((y + 1) & !15);
        } else if w.solidity_at(y - 1, x) & 0x20 != 0 {
            slope_y = slope_height(w, x, w.cell_at(1, y - 1, x).unwrap_or(0)) + ((y - 1) & !15);
        }
        if y + 2 < slope_y {
            let u = y + 1;
            let x_end = x + foot;
            let top = y - height;
            let falling = self.objects[id].as_ref().unwrap().w(0x1a);
            if falling < 1 || (y_old & !15) != (u & !15) {
                let mut lx = (x - foot) - 0x10;
                loop {
                    lx += 0x10;
                    if x_end < lx {
                        lx = x_end;
                    }
                    let s = w.solidity_at(u, lx);
                    let floor = (u & !15) - 1;
                    if s & 0x211 != 0 && y_old - 8 <= floor {
                        let o = self.o(id);
                        o.set(0x10, floor);
                        o.set(0x1c, 1);
                        if w.tile_function_at(u, lx) == 11000 {
                            carry =
                                w.cell_function(w.cell_at(1, u, lx).unwrap_or(0), 3) as i16 as i32;
                        }
                        break;
                    }
                    if lx >= x_end {
                        break;
                    }
                }
            }
            let mut ly = self.objects[id].as_ref().unwrap().w(0x10) + 0x10;
            loop {
                ly -= 0x10;
                if ly < top {
                    ly = top;
                }
                if w.solidity_at(ly, x - half) & 8 != 0 {
                    self.o(id).set(0xe, ((x - half) & !15) + 0x10 + half);
                    if side < 0 { result = 1 } else { side = 1 }
                }
                if w.solidity_at(ly, x + half) & 8 != 0 {
                    self.o(id).set(0xe, ((x + half) & !15) - 1 - half);
                    if side < 1 { side = -1 } else { result = 1 }
                }
                if top >= ly {
                    break;
                }
            }
        } else {
            let o = self.o(id);
            o.set(0x10, slope_y);
            o.set(0x1c, 5);
            let c = w.cell_at(1, y + 1, x).unwrap_or(0);
            o.set(0x1e, w.cell_function(c, 2) as i16 as i32);
        }
        let (x, y) = {
            let o = self.objects[id].as_ref().unwrap();
            (o.w(0xe), o.w(0x10))
        };
        if w.solidity_at(y - 8, x) & 2 != 0 {
            result = 1;
        }
        let f = w.tile_function_at(y - 8, x);
        if f > 0x1f3f && f < 0x1fa4 {
            result = 2;
        }
        {
            let o = self.o(id);
            o.set(0xe, o.w(0xe) + carry);
        }
        self.after_move(id);
        result
    }

    /// VMT +0x10 after the physics. FUN_1068_1155 is the default.
    fn after_move(&mut self, id: usize) {
        let kind = self.objects[id].as_ref().unwrap().kind;
        match kind {
            0x145a | 0x143c => {
                let o = self.o(id);
                if o.w(0x1c) == 0 {
                    if o.w(0x1a) == 0 {
                        o.set(0x18, o.w(0x10));
                        o.set(0x14, -0x7fff);
                        o.set(0x16, 0x7fff);
                    }
                    o.set(0x1a, o.w(0x1a) + 1);
                    o.set(2, -0x7fff);
                    o.set(4, -0x7fff);
                    o.set(6, 0x7fff);
                    o.set(8, 0x7fff);
                } else {
                    o.set(0x1a, 0);
                }
            }
            k => eprintln!("warning: move hook of {k:#x} not ported"),
        }
    }

    /// FUN_1070_a3cf: the small security bot (smallbot.spr).
    fn smallbot(&mut self, id: usize, env: &mut Env) {
        {
            let o = self.o(id);
            if o.w(0x26) > 0 {
                o.set(0x26, o.w(0x26) - 1);
            }
            o.set(0x22, o.w(0x10));
        }
        let (x, y) = {
            let o = self.objects[id].as_ref().unwrap();
            (o.w(0xe), o.w(0x10))
        };
        self.patrol_box(id, y + 0x20, x + 0x30, y - 0x40, x - 0x30);
        let hit = self.actor_physics(id, env, 0xe, 7, 3);
        if hit >= 1 {
            eprintln!("warning: smallbot destroyed by the map (FUN_1068_10f3/a2e0) not ported");
            return;
        }
        let state = self.objects[id].as_ref().unwrap().w(0x24);
        match state {
            1 => {
                let o = self.o(id);
                o.set(0x2a, o.w(0x2a) + 1);
                let i = o.w(0x2a) % 0x200;
                if i == 0x1e8 || i == 0x1eb {
                    o.set(0x28, o.w(0x28) + 3);
                    o.set(0xe, o.w(0xe) + ((o.w(0x2a) / 0x200) & 1) * 2 - 1);
                }
                let i = o.w(0x2a) % 0x100;
                if i == 0x3f {
                    let (x, y) = (o.w(0xe), o.w(0x10));
                    crate::effects::smoke(y - 0xe, x + 0x10, env.rng);
                }
            }
            0 | 2 | 3 | 4 => {
                eprintln!("warning: smallbot state {state} not ported");
            }
            _ => {}
        }
        // Damage from the hero's shots (FUN_1050_5960) needs the projectiles: not ported yet.
        let o = self.objects[id].as_ref().unwrap();
        if o.w(0x2c) < 1 && o.b(0x35) != 0 && (env.frame & 7) == 0 && env.rng.random(4) != 0 {
            env.rng.random(9);
        }
    }
}

/// FUN_1088_2c17: the height of a slope tile at x (table at 1170:1280).
fn slope_height(w: &World, x: i32, cell: u16) -> i32 {
    let p = w.cell_function(cell, 2);
    let mut i = x & 0xf;
    if p & 0x80 != 0 {
        i = 0xf - i;
    }
    crate::world::SLOPES[((p & 0xff7f) as usize) * 0x16 + i as usize] as i32
}
