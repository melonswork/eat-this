//! The particle engine (segment 1050): bullets, debris, sparks, critters, smoke. An array of
//! 0x50-byte records (1170:25b0, count 1170:0918) that FUN_1050_4ca5(group) moves, collides with
//! the map and finishes with a "death" effect. Records are kept as raw memory like the objects
//! (see objects.rs) so the decompiled code translates field by field:
//!
//! | off  | field |
//! |------|-------|
//! | 0    | group: FUN_1050_4ca5(group) updates only its own (0 before the actors, 1 after) |
//! | 2    | 1: sprite advances with the frame counter (FUN_1050_3de2's last argument) |
//! | 4, 6 | start x, y (movers count from here) |
//! | 8, a | x, y |
//! | c, e | last frame's x, y |
//! | 10,12| where it hit something (death effects play there) |
//! | 14   | sprite (< 0: an animation) / 16 the frame drawn |
//! | 18,1a| draw mode / 20 colour |
//! | 22   | lifetime / 24 age / 26 frames per sprite step |
//! | 28   | effect on spawn / 2a effect on death |
//! | 2c..32 | hit box (left, top, right, bottom) |
//! | 34   | what it hits (masks of FUN_1050_5960; 0x8000: only walls) |
//! | 36   | homing mask / 38 damage / 3a flags / 3c tile mask |
//! | 3e   | 1 alive without hits; -1 hit a wall; -10 left the screen; else hits on objects |
//! | 42,44| per-frame procedure (far pointer; replaces the plain draw) |
//! | 46   | far pointer to the motion record, 4a its mover, 4c its size |
//! | 4e   | water line for splashes |
//!
//! Flags (+0x3a): 1 keeps going through objects, 2 no collision, 4 dies quietly off screen,
//! 8 ignores platforms, 0x10 keeps its own hit box, 0x20 damage by FUN_1050_4b17.

use crate::effects;
use crate::level::{Effects, Level};
use crate::objects::Obj;
use crate::rng::PascalRandom;
use crate::trig;
use crate::world::{OUTSIDE, World};

/// FUN_1050_2aa4: just ages.
pub const MOVE_NONE: u16 = 0x2aa4;
/// FUN_1050_2ade: a straight line (FUN_1050_43c0).
pub const MOVE_LINE: u16 = 0x2ade;
/// FUN_1050_2c17: a throw under gravity (FUN_1050_44da).
pub const MOVE_THROW: u16 = 0x2c17;
/// FUN_1050_2d37: a bouncing throw (FUN_1050_457e).
pub const MOVE_BOUNCE: u16 = 0x2d37;
/// FUN_1050_36a9: a running critter (FUN_1050_4622).
pub const MOVE_CRITTER: u16 = 0x36a9;

/// Per-frame procedures (segment 1050 offsets).
pub const CB_BULLET: u16 = 0x126;
pub const CB_CRITTER: u16 = 0x1656;
/// FUN_1050_21c5: a shot-away wall tile that grows back when the particle's life ends.
pub const CB_WALL_REGROW: u16 = 0x21c5;

/// The spawn parameters, globals at 1170:091a..0940 that callers set around FUN_1050_3f77 and
/// put back afterwards.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub group: i32,      // 091a
    pub delay: i32,      // 091c: spawn later (FUN_10a0_0161)
    pub on_spawn: i32,   // 091e
    pub on_death: i32,   // 0920
    pub draw: i32,       // 0922
    pub draw2: i32,      // 0924
    pub colour: i32,     // 0926
    pub callback: u16,   // 0928 (segment 1050 at 092a)
    pub bounce: i32,     // 0930
    pub hits: i32,       // 0932
    pub homing: i32,     // 0934
    pub damage: i32,     // 0936
    pub flags: i32,      // 0938
    pub tiles: i32,      // 093a
    pub aim_level: i32,  // 093c
    pub aim_level2: i32, // 093e
    pub mover: u16,      // 0940
}

impl Default for Params {
    fn default() -> Params {
        Params {
            group: 1,
            delay: 0,
            on_spawn: 0,
            on_death: 0,
            draw: 0,
            draw2: 0,
            colour: 0,
            callback: 0,
            bounce: 5,
            hits: 0,
            homing: 0,
            damage: 0x10,
            flags: 0x84,
            tiles: 0x43,
            aim_level: 0x100,
            aim_level2: 0x100,
            mover: MOVE_NONE,
        }
    }
}

pub struct Particle {
    pub m: Obj,
    /// The motion record at +0x46 (its layout depends on the mover).
    pub v: Obj,
}

#[derive(Default)]
pub struct Particles {
    pub list: Vec<Particle>,
    pub p: Params,
    /// 1170:45b4: the particle being updated.
    pub current: usize,
    /// 1170:45b0: the last one spawned (in `pending` when `last_pending`).
    pub last: usize,
    last_pending: bool,
    /// Spawns waiting for their frame (FUN_10a0_0161's event queue 1170:19d2), by due frame,
    /// later ones after earlier ones with the same frame.
    pub pending: Vec<(u32, Particle)>,
    /// DAT_1170_617e, the level frame counter.
    pub now: u32,
    /// 1170:45b6: cleared by a death effect that keeps its particle alive.
    keep_dying: bool,
    /// 1170:45b8: the side of the wall hit, 0/1 horizontal (1: moving right), 2 up, 3 down.
    pub hit_side: i32,
    /// 1170:25aa / 25ac: what FUN_1050_568b last found.
    pub hit_mask: i32,
    pub hit_index: i32,
}

impl Default for Particle {
    fn default() -> Particle {
        Particle {
            m: Obj::new(0, 0, 0x50),
            v: Obj::new(0, 0, 0x10),
        }
    }
}

impl Particles {
    /// FUN_1050_3f77(sprite, y, x) and FUN_1050_3de2(sprite, steps, frames, y, x, animate):
    /// a new particle from the current parameters. `seq` is (steps, frames per step, animate)
    /// for 3de2 (lifetime steps * frames); 3f77 lives "forever" (0x7ffe frames).
    fn new_particle(&mut self, sprite: i32, y: i32, x: i32, seq: Option<(i32, i32, i32)>) -> usize {
        let p = self.p;
        let mut m = Obj::new(0, 0, 0x50);
        m.set(0, p.group);
        for off in [4, 8, 0xc] {
            m.set(off, x);
            m.set(off + 2, y);
        }
        match seq {
            None => {
                m.set(2, 0);
                m.set(0x22, 0x7ffe);
                m.set(0x26, 1);
            }
            Some((steps, frames, animate)) => {
                m.set(2, animate);
                m.set(0x22, frames * steps);
                m.set(0x26, steps);
            }
        }
        m.set(0x24, 0);
        m.set(0x14, sprite);
        m.set(0x18, p.draw);
        m.set(0x1a, p.draw2);
        m.set(0x20, p.colour);
        m.set(0x34, p.hits);
        m.set(0x3a, p.flags);
        m.set(0x3e, (p.hits == 0) as i32);
        if p.hits != 0 {
            m.set(0x36, p.homing);
            m.set(0x38, p.damage);
            if p.hits as i16 == -0x8000 {
                m.set(0x3a, p.flags | 0x10);
                m.set(0x3c, 0x13);
            } else {
                m.set(0x3c, p.tiles);
            }
            m.set(0x2c, x);
            m.set(0x2e, y);
            m.set(0x30, x);
            m.set(0x32, y);
        }
        m.set(0x28, p.on_spawn);
        m.set(0x2a, p.on_death);
        m.set(0x4c, 0);
        m.set(0x4a, p.mover as i32);
        m.set(0x42, p.callback as i32);
        let particle = Particle {
            m,
            v: Obj::new(0, 0, 0x10),
        };
        if p.delay != 0 {
            let due = self.now.wrapping_add(p.delay as u32);
            let at = self.pending.iter().position(|(d, _)| due < *d).unwrap_or(self.pending.len());
            self.pending.insert(at, (due, particle));
            self.last = at;
            self.last_pending = true;
            return at;
        }
        self.list.push(particle);
        self.last = self.list.len() - 1;
        self.last_pending = false;
        if p.on_spawn != 0 {
            eprintln!("warning: particle spawn effect {} not ported", p.on_spawn);
        }
        self.last
    }

    pub fn spawn(&mut self, sprite: i32, y: i32, x: i32) -> usize {
        self.new_particle(sprite, y, x, None)
    }

    pub fn spawn_seq(&mut self, sprite: i32, steps: i32, frames: i32, y: i32, x: i32, animate: i32) -> usize {
        self.new_particle(sprite, y, x, Some((steps, frames, animate)))
    }

    pub fn last_mut(&mut self) -> &mut Particle {
        let i = self.last;
        if self.last_pending {
            &mut self.pending[i].1
        } else {
            &mut self.list[i]
        }
    }

    /// FUN_10a0_023f at the start of a frame: the spawns that are due join the array.
    pub fn release_due(&mut self) {
        while self.pending.first().is_some_and(|(d, _)| *d <= self.now) {
            let (_, p) = self.pending.remove(0);
            if p.m.w(0x28) != 0 {
                eprintln!("warning: particle spawn effect {} not ported", p.m.w(0x28));
            }
            self.list.push(p);
        }
    }

    /// FUN_1050_43c0(speed, angle): fly straight; 1/16 px per frame per unit of speed. It starts
    /// half a step back.
    pub fn line(&mut self, speed: i32, angle: i32) {
        let p = self.last_mut();
        p.m.set(0x4a, MOVE_LINE as i32);
        p.m.set(0x4c, 0x10);
        let v = &mut p.v;
        v.set_l(0, trig::cos(angle) * speed);
        v.set_l(4, trig::sin(angle) * speed);
        v.set(8, angle);
        v.set(0xa, speed);
        v.set_l(0xc, 0);
        let (vx, vy) = (v.l(0), v.l(4));
        p.m.set(8, p.m.w(4) - vx / 0x20000);
        p.m.set(0xa, p.m.w(6) + vy / 0x20000);
        p.m.set(0x2c, 0x3fff);
        p.m.set(0x30, 0x3fff);
    }

    /// FUN_1050_44da(gravity, speed, angle): a throw; falls t*t/gravity px after t frames.
    pub fn throw(&mut self, gravity: i32, speed: i32, angle: i32) {
        let bounce = self.p.bounce;
        let p = self.last_mut();
        p.m.set(0x4a, MOVE_THROW as i32);
        p.m.set(0x4c, 0xe);
        let v = &mut p.v;
        v.set_l(0, trig::cos(angle) * speed);
        v.set_l(4, trig::sin(angle) * speed);
        v.set(8, 0);
        v.set(0xa, gravity);
        v.set(0xc, bounce);
        p.m.set(0x4e, 0x7fff);
        p.m.set(0x3c, p.m.w(0x3c) | 0x200);
    }

    /// FUN_1050_457e(gravity, speed, angle): a bouncing throw (FUN_1050_2d37).
    pub fn bounce(&mut self, gravity: i32, speed: i32, angle: i32) {
        self.throw(gravity, speed, angle);
        self.last_mut().m.set(0x4a, MOVE_BOUNCE as i32);
    }

    /// FUN_1050_4622(gravity, speed, angle): a critter running along the floor. Its mover
    /// (FUN_1050_36a9) isn't ported: it stays where it is.
    pub fn critter(&mut self, gravity: i32, speed: i32, angle: i32) {
        let bounce = self.p.bounce;
        let p = self.last_mut();
        p.m.set(0x4a, MOVE_CRITTER as i32);
        p.m.set(0x4c, 0xe);
        let v = &mut p.v;
        v.set_l(0, trig::cos(angle) * speed);
        if v.l(0) == 0 {
            v.set_l(0, 1);
        }
        v.set_l(4, trig::sin(angle) * speed);
        v.set(8, 0);
        v.set(0xa, gravity);
        v.set(0xc, bounce);
        // FUN_1050_44ae: the water line of the start's sector (none in the levels ported so far).
        p.m.set(0x4e, 0x7fff);
        p.m.set(0x3c, p.m.w(0x3c) | 0x200);
    }

    /// HeroFireWeapon (1088:6437) after its sound: the bullet, then two muzzle flashes.
    pub fn hero_fire(&mut self, y: i32, x: i32, aim: i32, parity: i32) {
        self.p.hits = 1;
        self.p.damage = 0x10;
        self.p.flags = 0x94;
        self.p.on_death = 62;
        self.p.callback = CB_BULLET;
        self.spawn(0x5638, y, x);
        self.last_mut().m.set(0x40, parity);
        self.line(0xa0, aim);
        self.p.callback = 0;
        self.p.on_death = 0;
        self.p.flags = 0x84;
        self.p.damage = 0x10;
        self.p.hits = 0;
        self.p.draw = 0;
        self.p.draw2 = 0;
        self.p.colour = 0x28;
        self.spawn_seq(0x1b04, 4, 2, y, x, 1);
        self.p.colour = 0;
        self.spawn_seq(0x190a, 2, 2, y, x, 1);
    }

    /// FUN_1050_568b(bottom, right, top, left, mask): the damage of the particles hitting this
    /// box. A hit particle stops (unless flag 1) and remembers where it hit.
    pub fn hit_test(&mut self, bottom: i32, right: i32, top: i32, left: i32, mask: i32) -> i32 {
        let mut damage = 0;
        self.hit_mask = 0;
        self.hit_index = -1;
        for i in (0..self.list.len()).rev() {
            let m = &mut self.list[i].m;
            if m.w(0x34) & mask == 0 {
                continue;
            }
            if left <= m.w(0x30) && top <= m.w(0x32) && m.w(0x2c) <= right && m.w(0x2e) <= bottom {
                if m.w(0x3a) & 0x20 != 0 {
                    eprintln!("warning: particle damage FUN_1050_4b17 not ported");
                } else {
                    damage += m.w(0x38);
                }
                self.hit_mask |= m.w(0x34);
                self.hit_index = i as i32;
                if m.w(0x3e) >= 0 {
                    m.set(0x3e, m.w(0x3e) + 1);
                }
                if m.w(0x3a) & 1 == 0 {
                    m.set(0x24, m.w(0x22) - 1);
                }
                m.set(0x10, m.w(8));
                m.set(0x12, m.w(0xa));
            }
            if m.w(0x36) & mask != 0 {
                eprintln!("warning: homing particles (FUN_1050_568b) not ported");
            }
        }
        damage
    }

    /// FUN_1070_0249: which way the last particle that hit was going (-1, 0, 1).
    pub fn hit_direction(&self) -> i32 {
        if self.hit_index < 0 {
            return 0;
        }
        let m = &self.list[self.hit_index as usize].m;
        (m.w(0x10) - m.w(0xc)).signum()
    }

    /// FUN_1050_29c8.
    fn remove(&mut self, i: usize) {
        self.list.remove(i);
    }
}

/// What a particle update needs from the frame.
pub struct Frame<'a> {
    pub world: &'a mut World,
    pub rng: &'a mut PascalRandom,
    pub fx: &'a mut Effects,
    pub camera: (i32, i32),
}

impl Level {
    /// FUN_1050_4ca5(group).
    pub fn update_particles(&mut self, group: i32, f: &mut Frame) {
        let mut i = 0;
        loop {
            if i >= self.particles.list.len() {
                return;
            }
            if self.particles.list[i].m.w(0) != group {
                i += 1;
                continue;
            }
            self.particles.current = i;
            {
                let m = &mut self.particles.list[i].m;
                m.set(0xc, m.w(8));
                m.set(0xe, m.w(0xa));
            }
            self.move_particle(i, f.camera);
            let hit = self.particles.list[i].m.w(0x34) != 0 && self.collide(i, f);
            if !hit {
                self.particle_callback(i, f);
                let m = &self.particles.list[i].m;
                if m.w(0x24) < m.w(0x22) {
                    i += 1;
                    continue;
                }
            }
            // LAB_1050_5128: the end of its life.
            let m = &self.particles.list[i].m;
            let (death, flags, state) = (m.w(0x2a), m.w(0x3a), m.w(0x3e));
            if death == 0 || (flags & 4 != 0 && (state < -9 || state == 0)) {
                self.particles.remove(i);
            } else {
                self.particles.keep_dying = true;
                let (hy, hx) = (m.w(0x12), m.w(0x10));
                self.particle_effect(death, hy, hx, f);
                if self.particles.keep_dying {
                    self.particles.remove(i);
                } else {
                    i += 1;
                }
            }
        }
    }

    /// The mover at +0x4a.
    fn move_particle(&mut self, i: usize, cam: (i32, i32)) {
        let p = &mut self.particles.list[i];
        sprite_frame(&mut p.m);
        let (m, v) = (&mut p.m, &mut p.v);
        let mover = m.uw(0x4a);
        let off_screen = |m: &Obj, top: i32| {
            let (dx, dy) = ((m.w(8) - cam.0) as i16, (m.w(0xa) - cam.1) as i16);
            dx < -0x100 || dy < top as i16 || dx > 0x23f || dy > 0x1c7
        };
        match mover {
            MOVE_NONE => {
                m.set(0x24, m.w(0x24) + 1);
                return;
            }
            MOVE_LINE => {
                let t = v.l(0xc);
                m.set(8, m.w(4) + v.l(0).wrapping_mul(t) / 0x40000);
                m.set(0xa, m.w(6) - v.l(4).wrapping_mul(t) / 0x40000);
                v.set_l(0xc, t.wrapping_add(1));
                if off_screen(m, -0x100) {
                    m.set(0x24, m.w(0x22));
                    m.set(0x3e, -10);
                } else {
                    m.set(0x24, m.w(0x24) + 1);
                }
            }
            MOVE_THROW => {
                let t = v.w(8);
                m.set(8, m.w(4) + v.l(0).wrapping_mul(t) / 0x40000);
                let fall = (t * t) / v.w(0xa);
                m.set(0xa, m.w(6) - v.l(4).wrapping_mul(t) / 0x40000 + fall);
                v.set(8, t + 1);
                if m.w(0x4e) != 0x7fff {
                    eprintln!("warning: particle splashes (FUN_1050_2bcf) not ported");
                }
                if off_screen(m, -0x200) {
                    m.set(0x24, m.w(0x22));
                    m.set(0x3e, -10);
                } else {
                    m.set(0x24, m.w(0x24) + 1);
                }
            }
            MOVE_CRITTER => {
                m.set(0x24, m.w(0x24) + 1);
            }
            other => {
                eprintln!("warning: particle mover 1050:{other:04x} not ported");
                m.set(0x24, m.w(0x24) + 1);
            }
        }
    }

    /// The tile and platform collision in FUN_1050_4ca5. True if it hit.
    fn collide(&mut self, i: usize, f: &mut Frame) -> bool {
        let w = &*f.world;
        let m = &mut self.particles.list[i].m;
        if m.w(0x3a) & 0x10 == 0 {
            m.set(0x2c, m.w(8));
            m.set(0x2e, m.w(0xa));
            m.set(0x30, m.w(8));
            m.set(0x32, m.w(0xa));
        }
        if m.w(0x3a) & 2 != 0 {
            return false;
        }
        let (x, y, px, py, mask) = (m.w(8), m.w(0xa), m.w(0xc), m.w(0xe), m.uw(0x3c));
        let s = w.solidity_at(y, x);
        if s & mask == 0 {
            return self.platform_hit(i, w);
        }
        let m2 = mask & 0xfdee;
        let s2 = w.solidity_at(y, px);
        if s2 & m2 == 0 && s & m2 != 0 {
            let hx = (x & !15) + if px > x { 0x10 } else { -1 };
            self.particles.hit_side = (px <= x) as i32;
            m.set(0x10, hx);
            m.set(0x12, y);
        } else {
            if s & m2 == 0 && (y as u16 >> 4) <= (py as u16 >> 4) {
                // Only a slope's or a floor's top, and not entered from above: the platform test.
                // (The original jumps back to it; with no platform hit it carries on unharmed.)
                return self.platform_hit(i, w);
            }
            if y < py {
                m.set(0x12, (y & !15) + 0x10);
                self.particles.hit_side = 2;
            } else {
                m.set(0x12, (y & !15) - 1);
                self.particles.hit_side = 3;
            }
            m.set(0x10, x);
        }
        let (strength, hits) = (m.w(0x38), m.w(0x34));
        if hits as i16 != -0x8000 {
            let func = w.tile_function_at(y, x);
            if (6000..=0x1779).contains(&func) {
                eprintln!("warning: a shot lever (FUN_1088_14a3) not ported");
            } else if func == 0x1b8a {
                eprintln!("warning: FUN_10a8_1369 (shot tile 7050) not ported");
            }
            if s & 0x80 != 0 {
                effects::explode_tile(self, f.world, strength, 3, y, x, f.rng, f.fx);
            }
        }
        self.particles.list[i].m.set(0x3e, -1);
        true
    }

    /// LAB_1050_4ee6: the moving platforms, last first.
    fn platform_hit(&mut self, i: usize, w: &World) -> bool {
        let m = &mut self.particles.list[i].m;
        if self.platforms.is_empty() || m.w(0x3a) & 8 != 0 {
            return false;
        }
        let (x, y, px, py, mask) = (m.w(8), m.w(0xa), m.w(0xc), m.w(0xe), m.uw(0x3c));
        for pi in (0..self.platforms.len()).rev() {
            let Some(p) = &self.platforms[pi] else {
                continue;
            };
            // FUN_1078_01fb: inside the platform's rectangle.
            if x < p.x || y < p.y || p.x + p.width <= x || p.y + p.height <= y {
                continue;
            }
            let s = w.solidity_in(p.plane, y - p.y, x - p.x);
            if s == OUTSIDE || s & mask == 0 {
                continue;
            }
            let s2 = w.solidity_in(p.plane, (py - 1) - p.prev_y, x - p.prev_x);
            if s2 == OUTSIDE || s2 & mask == 0 {
                m.set(0x10, x);
                let mut hy = ((y - p.y) & !15) + p.y;
                if y < py {
                    hy += 0x10;
                    self.particles.hit_side = 2;
                } else {
                    hy -= 1;
                    self.particles.hit_side = 3;
                }
                m.set(0x12, hy);
            } else {
                let mut hx = ((x - p.x) & !15) + p.x;
                hx += if px > x { 0x10 } else { -1 };
                self.particles.hit_side = (px <= x) as i32;
                m.set(0x10, hx);
                m.set(0x12, y);
            }
            m.set(0x3e, -1);
            return true;
        }
        false
    }

    /// The per-frame procedure at +0x42 (the drawing otherwise).
    fn particle_callback(&mut self, i: usize, f: &mut Frame) {
        let m = &mut self.particles.list[i].m;
        match m.uw(0x42) {
            CB_WALL_REGROW => {
                if m.w(0x22) <= m.w(0x24) {
                    let (tile, y, x) = (m.uw(0x40), m.w(0xa), m.w(8));
                    let width = f.world.main().width;
                    let idx = (y / 16) * width + x / 16;
                    f.world.main_mut().cells[idx as usize] = tile;
                    self.cell_changed(f.world, idx - width, f.fx);
                }
            }
            0 => {}
            CB_BULLET => {
                // FUN_1050_0126: draws, and a 7x7 box around the bullet.
                let (x, y) = (m.w(8), m.w(0xa));
                m.set(0x2c, x - 3);
                m.set(0x2e, y - 3);
                m.set(0x30, x + 3);
                m.set(0x32, y + 3);
            }
            CB_CRITTER | 0x2d2 | 0x17f4 | 0x2148 | 0x2192 | 0x13c3 => {} // drawing only
            0x1a3 => {
                // FUN_1050_01a3: a 9x9 box.
                let (x, y) = (m.w(8), m.w(0xa));
                m.set(0x2c, x - 4);
                m.set(0x2e, y - 4);
                m.set(0x30, x + 4);
                m.set(0x32, y + 4);
            }
            0x23b7 => {
                // FUN_1050_23b7: a gib that stopped against a wall slides down it.
                let p = &mut self.particles.list[i];
                let (m, v) = (&mut p.m, &mut p.v);
                let (x, y) = (m.w(8), m.w(0xa));
                if v.l(0) == 0 && f.world.solidity_at(y, x - 1 + ((x & 0xf) > 7) as i32 * 2) & 2 != 0 {
                    v.set(8, 4);
                    m.set(6, y);
                }
                if m.w(0x24) > 0xb {
                    m.set(0x3a, 0x84);
                }
            }
            0x11e6 => {
                // FUN_1050_11e6: a grenade smokes every 5 frames.
                if m.w(0x24) % 5 == 0 {
                    let (x, y) = (m.w(8), m.w(0xa));
                    let ps = &mut self.particles;
                    ps.p.group = 0;
                    ps.p.colour = 0x10;
                    ps.p.callback = 0x2192;
                    let r = f.rng.random(9);
                    ps.spawn_seq(0x6c00, 1, 0x19, y - 5, (x - 3) - r, 1);
                    let a = f.rng.random(0x141) + 0x500;
                    ps.line(10, a);
                    ps.p.callback = 0;
                    ps.p.colour = 0;
                    ps.p.group = 1;
                }
            }
            0x2299 => {
                // FUN_1050_2299: a trail of smoke.
                if m.w(0x24) % 4 == 3 {
                    let (x, y) = (m.w(8), m.w(0xa));
                    let ps = &mut self.particles;
                    ps.p.delay = 3;
                    ps.p.group = 0;
                    ps.p.colour = 0x10;
                    ps.spawn_seq(0x6805, 3, 10, y, x, 1);
                    ps.p.colour = 0;
                    ps.p.group = 1;
                    ps.p.delay = 0;
                }
            }
            0x18e6 => {
                // FUN_1050_18e6: smoke lying on the floor; gone when there's no floor.
                let (x, y) = (m.w(8), m.w(0xa));
                if f.world.solidity_at(y + 1, x) & 0x211 == 0 {
                    m.set(0x24, m.w(0x22));
                } else if SMOKE_AGES && m.w(0x24) < m.w(0x22) {
                    m.set(0x24, m.w(0x24) + 1);
                }
            }
            other => eprintln!("warning: particle procedure 1050:{other:04x} not ported"),
        }
    }

    /// FUN_1088_12f9 for the cell's current tile: levers run their scripts; bonus tiles
    /// (function 30000) turn into bonus objects.
    fn cell_changed(&mut self, world: &mut World, idx: i32, fx: &mut Effects) {
        let tile = world.main().cells[idx as usize];
        if tile as i16 <= 0 {
            return;
        }
        let f0 = world.cell_function(tile, 0);
        if (crate::world::func::LEVER_FIRST..=crate::world::func::LEVER_LAST).contains(&f0) {
            self.run_scripts(world, idx as u16, fx);
        } else if f0 == 30000 {
            eprintln!("warning: a bonus tile uncovered (FUN_1040_6b03) not ported");
        }
    }

    /// Effect 39, FUN_1040_40fb: a wall tile that takes shots. Its hit points are kept negated
    /// in plane 2 (FUN_10f0_59a0); while it has some left it only goes away for 0x8c frames.
    pub fn breakable_wall(
        &mut self,
        world: &mut World,
        strength: i32,
        y: i32,
        x: i32,
        rng: &mut PascalRandom,
        fx: &mut Effects,
    ) {
        let width = world.main().width;
        let idx = ((y / 16) * width + x / 16) as usize;
        let left = {
            let p2 = world.planes[2].as_mut().expect("plane 2");
            let v = p2.cells[idx] as i16 as i32;
            let left = if v < 0 { -v } else { 0 } - strength;
            p2.cells[idx] = if left < 1 { 0 } else { (-left) as u16 };
            left
        };
        let ps = &mut self.particles;
        ps.p.group = 0;
        if left < 1 {
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            for i in 0..3 {
                ps.spawn(0x7528, y, x - 4 + i * 4);
                let angle = i * -0x70 + 0x5f0 + rng.random(0x41);
                let speed = rng.random(0x10) + 0x30;
                ps.throw(9, speed, angle);
            }
            ps.p.on_death = 0;
            ps.p.hits = 0;
            eprintln!("warning: effect 23 (FUN_1040_2af4) and its FUN_1050_4a29 not ported");
            fx.shakes.push((0x18, 0xc));
            // FUN_1050_5a07: the tile below steps on (function 2), FUN_1088_1364.
            let below = idx + width as usize;
            let t = world.main().cells[below];
            let next = (t as i32 + world.cell_function(t, 2) as i16 as i32) as u16;
            if (next as i16) < 0 {
                eprintln!("warning: FUN_1088_1364 with a negative tile not ported");
            } else {
                world.main_mut().cells[below] = next;
                self.cell_changed(world, below as i32, fx);
            }
        } else {
            ps.p.callback = CB_WALL_REGROW;
            ps.spawn_seq(0x7518, 1, 0x8c, y, x, 0);
            let tile = world.main().cells[idx];
            ps.last_mut().m.set(0x40, tile as i32);
            ps.p.callback = 0;
            fx.shakes.push((0x12, 8));
        }
        self.particles.p.group = 1;
        world.main_mut().cells[idx] = 0;
    }

    /// A death effect (FUN_1040_60bb) with the dying particle current; its procedure may clear
    /// `keep_dying` to live on.
    fn particle_effect(&mut self, id: i32, y: i32, x: i32, f: &mut Frame) {
        match id {
            62 => self.bullet_hit(y, x, f),
            15 => {
                // FUN_1040_0a80: a metallic ping and sparks.
                f.rng.random(0x1e);
                self.sparks(0x191b, y, x);
            }
            // FUN_1040_0ab0: debris bounces off the floor. No Random(); not ported, it just ends.
            16 => {}
            _ => effects::effect(f.world, id as u16, y, x, f.rng, f.fx),
        }
    }

    /// Effect 62, FUN_1040_06d4: a bullet hits.
    fn bullet_hit(&mut self, y: i32, x: i32, f: &mut Frame) {
        let i = self.particles.current;
        let m = &self.particles.list[i].m;
        if std::env::var_os("ET_DEBUG").is_some() {
            eprintln!("DBG {} bullet hit ({x},{y}) at ({},{}) state {} age {}", self.particles.now, m.w(8), m.w(0xa), m.w(0x3e), m.w(0x24));
        }
        let (py, px) = (m.w(0xa), m.w(8));
        if f.world.tile_function_at(py, px) == 13000 {
            // A tile with its own sound of being hit (function 4).
            let cell = f.world.cell_at(crate::world::MAIN_PLANE, py, px).unwrap_or(0);
            let id = f.world.cell_function(cell, 4) as i16 as i32;
            self.particle_effect(id, y, x, f);
            return;
        }
        if m.w(0x3e) == -1 && f.rng.random(0xc) == 0 {
            f.rng.random(0x40); // the ricochet sound's pitch
        }
        self.sparks(0x190c, y, x);
    }

    /// FUN_1040_0628(sprite, y, x): two sparks thrown back from the current particle's line.
    fn sparks(&mut self, sprite: i32, y: i32, x: i32) {
        let v = &self.particles.list[self.particles.current].v;
        let side = if v.l(0) <= 0 { -1 } else { 1 };
        let angle = v.w(8);
        let ps = &mut self.particles;
        ps.spawn_seq(sprite, 3, 3, y, x, 1);
        ps.throw(0xe, 0x20, (angle + 0xb40 - side * 0x1e0) % 0x1680);
        ps.spawn_seq(sprite, 4, 3, y, x, 1);
        ps.throw(0xe, 0x10, (angle + 0xb40 + side * 0x1e0) % 0x1680);
    }
}

/// 1170:4f0c > 10 (FUN_1050_18e6): lying smoke ages (the frame rate's headroom, as measured).
const SMOKE_AGES: bool = true;

/// FUN_1040_6577(y, x): a puff of smoke rising, six particles over 40 frames (with the current
/// group and colour).
pub fn smoke(ps: &mut Particles, rng: &mut PascalRandom, y: i32, x: i32) {
    for i in 0..6 {
        ps.p.delay = i << 3;
        let px = x - 5 + rng.random(0xb);
        let py = y - rng.random(8);
        if i < 3 {
            ps.spawn_seq(0x6800, 2, 0xf, py, px, 1);
        } else {
            ps.spawn_seq(0x5111, 4, 9, py, px, 1);
        }
        ps.line(8, 0x5a0);
    }
    ps.p.delay = 0;
}

/// FUN_1040_6623(kinds, sprite, speed range, speed, angle range, spread, y, x, n): n pieces
/// thrown in a fan (gravity 9).
#[allow(clippy::too_many_arguments)]
pub fn fan(
    ps: &mut Particles,
    rng: &mut PascalRandom,
    kinds: i32,
    sprite: i32,
    speed_range: i32,
    speed: i32,
    angle_range: i32,
    spread: i32,
    y: i32,
    x: i32,
    n: i32,
) {
    ps.p.hits = 0x8000;
    let last = n - 1;
    if last >= 0 {
        for i in 0..=last {
            let s = sprite + rng.random(kinds as u16) as i32;
            ps.spawn(s, y, x);
            let r = rng.random((angle_range + 1) as u16) as i32;
            let a = i * ((spread << 1) / last) + ((0x5a0 - spread) - (angle_range >> 1)) + r;
            let v = speed + rng.random(speed_range as u16) as i32;
            ps.throw(9, v, a);
        }
    }
    ps.p.hits = 0;
}

/// FUN_1050_00a7: the frame to draw. Animations (negative sprites) aren't ported.
fn sprite_frame(m: &mut Obj) {
    let mut s = m.w(0x14);
    if m.w(2) == 1 && m.w(0x26) != 0 {
        s += m.w(0x24) / m.w(0x26);
    }
    m.set(0x16, s);
}
