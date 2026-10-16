//! Updating the game objects: the collection walk (FUN_1058_008f), the actor physics shared by
//! the walking enemies (FUN_1068_119e), and the classes' update methods (VMT +0xc), translated
//! from the decompilation field by field. Drawing is left out.

use crate::level::{Effects, Level};
use crate::objects::list;
use crate::rng::PascalRandom;
use crate::world::{OUTSIDE, World, ds_word};

/// The frame state the object methods read (globals in the original).
pub struct Env<'a> {
    pub world: &'a mut World,
    pub rng: &'a mut PascalRandom,
    pub fx: &'a mut Effects,
    pub hero: (i32, i32, i32), // gHeroX, gHeroY, gHeroHeight
    pub frame: u32,            // DAT_1170_617e, the level frame counter
    pub h: HeroView,
    /// 1170:465e: 2 the frame the hero fired, 1 when a screen shake just started.
    pub noise: i32,
    pub camera: (i32, i32),
}

/// The hero globals the objects read.
#[derive(Clone, Copy, Default, Debug)]
pub struct HeroView {
    pub x: i32,
    pub y: i32,
    pub prev_x: i32,
    pub prev_y: i32,
    pub height: i32,
    pub state: i32,
    pub vy: i32,
    pub fall: i32,
    pub up: bool,
    /// 1170:4e2a: the hero's invulnerability frames.
    pub hurt: i32,
    /// 1170:4660: what hit him this frame.
    pub hit_mask: i32,
}

impl Level {
    /// FUN_1070_faec: the hero's sector's actors (list +0x36) and triggers (+0x4e), then the
    /// default sector's actors. Objects update only while their box meets `window` (the camera
    /// rectangle, 1170:45ca..45d0).
    pub fn update_actors(&mut self, env: &mut Env, window: (i32, i32, i32, i32)) {
        let sector = self.sector.unwrap_or(0);
        if std::env::var("ET_DEBUG2").ok().and_then(|v| v.parse::<u32>().ok()) == Some(env.frame as u32) {
            for (s, l) in self.lists.iter().enumerate() {
                for (li, ids) in l.iter().enumerate() {
                    for &id in ids {
                        if let Some(o) = &self.objects[id] {
                            eprintln!("LIST sector {s} list {li} id {id} kind {:#x} bbox {:?} pos ({},{})", o.kind, o.bbox(), o.w(0xe), o.w(0x10));
                        }
                    }
                }
            }
            eprintln!("LIST hero sector {sector} window {window:?}");
        }
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
                self.walking = (sector, l);
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
            0x143c => self.crate_box(id, env),
            0x1428 => self.alien(id, env),
            0x1432 => self.bigali(id, env),
            0x14aa => {} // FUN_1070_a06a: draws a sprite
            0x13c4 => {} // FUN_1068_315a: plain sound emitters (kinds < 10000) don't draw Random
            _ => {}
        }
    }

    /// FUN_1070_0002: delete the object being updated from the collection being walked.
    pub(crate) fn remove_current(&mut self, id: usize) {
        let (s, l) = self.walking;
        let i = self.walk_index as usize;
        debug_assert_eq!(self.lists[s][l][i], id);
        self.lists[s][l].remove(i);
        self.walk_index -= 1;
        self.objects[id] = None;
    }

    /// FUN_1070_90f1: a crate (0x143c). Shots push it and wear it down.
    fn crate_box(&mut self, id: usize, env: &mut Env) {
        let t = {
            let o = self.o(id);
            if o.w(0x22) > 0 {
                o.set(0x22, o.w(0x22) - 1);
            }
            let (x, y) = (o.w(0xe), o.w(0x10));
            if o.w(10) == 0 {
                // (FUN_10e0_0073 and friends keep its humming sound going.)
                o.set(2, x - 0xb0);
                o.set(6, x + 0xb0);
                o.set(4, y - 0x4e);
                o.set(8, y + 0x4e);
            } else {
                o.set(2, x - 0x30);
                o.set(6, x + 0x30);
                o.set(4, y - 0x40);
                o.set(8, y + 0x30);
            }
            o.w(10)
        };
        // 1170:0e4a: sprite, half width, height per crate type.
        let row = 0xe4a + (t * 6) as usize;
        let (sprite, half, height) = (ds_word(row), ds_word(row + 2), ds_word(row + 4));
        if self.actor_physics(id, env, height - 1, half, 4) >= 1 {
            self.crate_destroy(id, env);
            return;
        }
        if self.o(id).w(0x1c) == 5 {
            eprintln!("warning: crate sliding on a slope (FUN_1068_17d6) not ported");
        }
        let (x, y) = {
            let o = self.o(id);
            (o.w(0xe), o.w(0x10))
        };
        let damage = match crate::sprites::hitbox(sprite) {
            Some((l, tp, r, b)) => self.particles.hit_test_low(y + b, x + r, y + tp, x + l, 0x11fb),
            None => 0,
        };
        if damage > 0 {
            let damage = if damage == 4 && t == 0 { 1 } else { damage };
            let push = self.particles.hit_direction();
            let o = self.o(id);
            o.set(0xe, o.w(0xe) + push);
            o.set(0x20, o.w(0x20) - damage);
            if o.w(0x20) < 1 {
                self.crate_destroy(id, env);
                return;
            }
            o.set(0x22, 3);
            env.fx.shakes.push((0xc, 5));
        }
        // The hero standing on it.
        let o = self.o(id);
        let top = o.w(0x10) - height;
        let (hx, hy, _) = env.hero;
        if o.w(0xe) - half - 9 <= hx && hx <= o.w(0xe) + half + 10 && (top..top + 0x10).contains(&hy) {
            eprintln!("warning: the hero standing on a crate not ported");
        }
    }

    /// FUN_1070_8cd7: a crate breaks.
    fn crate_destroy(&mut self, id: usize, env: &mut Env) {
        let (x, y, t) = {
            let o = self.o(id);
            if o.w(0x12) == 0 && env.rng.random(4) == 0 {
                o.set(0x12, 0xd10);
            }
            (o.w(0xe), o.w(0x10), o.w(10))
        };
        // FUN_1068_10f3: the bonus inside.
        if self.o(id).w(0x12) != 0 {
            eprintln!("warning: a crate's bonus {:#x} (FUN_1040_6b03) not ported", self.o(id).w(0x12));
            self.o(id).set(0x12, 0);
        }
        if t < 1 {
            let ps = &mut self.particles;
            ps.p.group = 0;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            for i in 0..4 {
                let px = x - 6 + i * 4;
                let py = (y - 6) - env.rng.random(0xe);
                ps.spawn(29999, py, px);
                let angle = i * -0x50 + 0x5d0 + env.rng.random(0x81);
                let speed = env.rng.random(0x21) + 0x50;
                ps.throw(7, speed, angle);
            }
            ps.p.on_death = 0;
            ps.p.hits = 0;
            ps.p.group = 1;
            // FUN_1040_637a: smoke sprites (no Random()).
            env.fx.shakes.push((0x18, 0xe));
        } else {
            eprintln!("warning: crate type {t} breaking not ported");
        }
        self.falling_critter(y, x, env);
        self.remove_current(id);
    }

    /// FUN_1040_1d72(y, x): one time in five a critter runs out, if there's floor to run on.
    pub fn falling_critter(&mut self, y: i32, x: i32, env: &mut Env) {
        if env.rng.random(5) != 0 {
            return;
        }
        let dir = if env.hero.0 - x < 0 { -1 } else { 1 };
        let mut lx = x;
        for _ in 0..4 {
            if crate::hero::Hero::solidity_all(env.world, self, y + 0xf, lx) & 0x201 == 0 {
                return;
            }
            if crate::hero::Hero::solidity_all(env.world, self, y, lx) & 2 != 0 {
                return;
            }
            lx -= dir * 0x10;
        }
        let angle = env.rng.random(2) * 0x5a0 + 0x2d0;
        // FUN_1070_f9c7(0x40, angle, y, x).
        let ps = &mut self.particles;
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.callback = crate::particles::CB_CRITTER;
        ps.p.on_death = 0x5e;
        ps.p.draw = (0x5a0..=0x10df).contains(&angle) as i32;
        ps.p.bounce = 1;
        ps.spawn(0x5215, y, x);
        ps.critter(7, 0x40, angle);
        ps.p = crate::particles::Params::default();
        env.rng.random(0x40); // the squeak's pitch
    }

    pub(crate) fn o(&mut self, id: usize) -> &mut crate::objects::Obj {
        self.objects[id].as_mut().unwrap()
    }

    /// FUN_1068_1765: grow the activity box around the actor's patrol range.
    pub(crate) fn patrol_box(&mut self, id: usize, bottom: i32, right: i32, top: i32, left: i32) {
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
        self.after_move(id, env);
        result
    }

    /// VMT +0x10 after the physics. FUN_1068_1155 is the default.
    fn after_move(&mut self, id: usize, env: &mut Env) {
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
            0x1428 => self.alien_after_move(id),
            0x1432 => self.bigali_after_move(id, env),
            k => eprintln!("warning: move hook of {k:#x} not ported"),
        }
    }
}

/// FUN_1088_2c17: the height of a slope tile at x (table at 1170:1280).
pub(crate) fn slope_height(w: &World, x: i32, cell: u16) -> i32 {
    let p = w.cell_function(cell, 2);
    let mut i = x & 0xf;
    if p & 0x80 != 0 {
        i = 0xf - i;
    }
    crate::world::SLOPES[((p & 0xff7f) as usize) * 0x16 + i as usize] as i32
}
