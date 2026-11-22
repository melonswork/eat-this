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
use crate::sound;
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
/// FUN_1050_3044: a ball that bounces, rolls on and climbs one-tile steps (FUN_1050_46d9).
pub const MOVE_ROLL: u16 = 0x3044;
/// FUN_1050_3c4f: a homing rocket in 16.16 fixed point (FUN_1050_4870).
pub const MOVE_ROCKET: u16 = 0x3c4f;
/// FUN_1050_2ac9: just ages (no sprite frame either).
pub const MOVE_AGE: u16 = 0x2ac9;
/// FUN_1050_36a9: a running critter (FUN_1050_4622).
pub const MOVE_CRITTER: u16 = 0x36a9;
/// FUN_1050_3d06: a throw in the half-speed background layer (no walls; gone below 0x1c8 + camY / 2).
pub const MOVE_BG_THROW: u16 = 0x3d06;

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
    /// The hero's drone (FUN_1088_03a3).
    pub drone: crate::drone::Drone,
    /// 1170:4e42 / 4e44: the pointer in the world (set at the end of the camera update).
    pub mouse: (i32, i32),
    /// 1170:4e50: what the pointer is over, set by enemy hit tests (mask & 0x59) whose box holds
    /// `mouse`: 2 an enemy, 3 an alien to kick (`kick_pointer`); 0 none. Only the HUD reads it
    /// (FUN_1088_4b64); main clears it every frame.
    pub pointer: i32,
    /// 1170:163e = 3 around the alien's corpse test (FUN_1070_60ca), else 2.
    pub kick_pointer: bool,
}

impl Default for Particle {
    fn default() -> Particle {
        Particle {
            m: Obj::new(0, 0, 0x50),
            v: Obj::new(0, 0, 0x20),
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
            v: Obj::new(0, 0, 0x20),
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
            let m = &self.list[self.last].m;
            spawn_effect(p.on_spawn, m.w(0xa), m.w(8));
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
                spawn_effect(p.m.w(0x28), p.m.w(0xa), p.m.w(8));
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

    /// FUN_1050_4999(gravity, speed, angle): a throw in the background layer (GRANNY's collapse).
    pub fn bg_throw(&mut self, gravity: i32, speed: i32, angle: i32) {
        let bounce = self.p.bounce;
        let p = self.last_mut();
        p.m.set(0x4a, MOVE_BG_THROW as i32);
        p.m.set(0x4c, 0xe);
        let v = &mut p.v;
        v.set_l(0, trig::cos(angle) * speed);
        v.set_l(4, trig::sin(angle) * speed);
        v.set(8, 0);
        v.set(0xa, gravity);
        v.set(0xc, bounce);
    }

    /// FUN_1050_2a6c(group): every particle of the group goes.
    pub fn remove_group(&mut self, group: i32) {
        self.list.retain(|p| p.m.w(0) != group);
    }

    /// FUN_1040_621f(speed, angle, y, x): the hero's grenade (effect 21 when it hits or its
    /// fuse of 0x78 frames runs out).
    pub fn grenade(&mut self, speed: i32, angle: i32, y: i32, x: i32, rng: &mut PascalRandom) {
        self.p.hits = 0x8000;
        self.p.flags = 0x80;
        self.p.damage = 0xc0;
        self.p.callback = 0x12f6;
        self.p.on_death = 0x15;
        self.p.draw = (0x5a0..=0x10df).contains(&angle) as i32;
        let s = 0x195a + rng.random(2) * 4;
        self.spawn_seq(s, 1, 0x78, y, x, 0);
        self.bounce(7, speed, angle);
        // FUN_1050_44ae: the water line of its sector (none in the levels ported so far).
        self.p.draw = 0;
        self.p.on_death = 0;
        self.p.callback = 0;
        self.p.damage = 0x10;
        self.p.flags = 0x84;
        self.p.hits = 0;
    }

    /// FUN_1088_655f: weapon 7's shell (effect 44 when it hits slowly or its fuse runs out).
    pub fn shell7(&mut self, angle: i32, y: i32, x: i32) {
        self.p.group = 0;
        self.p.hits = 0x8000;
        self.p.flags = 0x80;
        self.p.damage = 0x60;
        self.p.callback = 0x13c3;
        self.p.on_death = 0x2c;
        self.p.draw = (0x5a0..=0x10df).contains(&angle) as i32;
        self.spawn_seq(0x1866, 6, 0x1e, y, x, 0);
        self.bounce(7, 0x40, angle);
        self.p.draw = 0;
        self.p.on_death = 0;
        self.p.callback = 0;
        self.p.damage = 0x10;
        self.p.flags = 0x84;
        self.p.hits = 0;
        self.p.group = 1;
    }

    /// FUN_1088_6649's ball (weapon 9): bounces, then rolls along the floor and up steps,
    /// hurting what it meets for its first frames after each bounce (effect 56 at its end).
    pub fn ball9(&mut self, y: i32, x: i32, angle: i32, water: i32) {
        self.p.group = 0;
        self.p.hits = 0x8000;
        self.p.flags = 0x90;
        self.p.damage = 0;
        self.p.callback = 0x16cc;
        self.p.on_death = 0x38;
        self.p.draw = (0x5a0..=0x10df).contains(&angle) as i32;
        self.p.bounce = 2;
        self.spawn_seq(0x1818, 1, 0x1e0, y, x, 0);
        self.roll(7, 0x20, angle, water);
        self.p.bounce = 5;
        self.p.draw = 0;
        self.p.on_death = 0;
        self.p.callback = 0;
        self.p.damage = 0x10;
        self.p.flags = 0x84;
        self.p.hits = 0;
        self.p.group = 1;
    }

    /// FUN_1050_46d9(gravity, speed, angle): a rolling ball (FUN_1050_3044); `water` is the
    /// spawn sector's water line (FUN_1050_44ae).
    pub fn roll(&mut self, gravity: i32, speed: i32, angle: i32, water: i32) {
        self.throw(gravity, speed, angle);
        let p = self.last_mut();
        p.m.set(0x4a, MOVE_ROLL as i32);
        if p.v.l(0) == 0 {
            p.v.set_l(0, 1);
        }
        p.m.set(0x4e, water);
    }

    /// FUN_1088_6f4d: weapon 4's bouncing charge (effect 77 when it hits or its fuse runs out).
    pub fn mine(&mut self, angle: i32, y: i32, x: i32) {
        self.p.hits = 0x8000;
        self.p.flags = 0x80;
        self.p.callback = 0x137a;
        self.p.on_death = 0x4d;
        self.p.draw = (0x5a0..=0x10df).contains(&angle) as i32;
        self.spawn_seq(0x1964, 1, 0x78, y, x, 0);
        self.bounce(6, 0x50, angle);
        self.p.draw = 0;
        self.p.on_death = 0;
        self.p.callback = 0;
        self.p.flags = 0x84;
        self.p.hits = 0;
    }

    /// The particles of FUN_1040_0312 (a weapon picked up, `f3` its tile's function 3): two
    /// sparkles and the weapon's icon flying up.
    pub fn weapon_pickup(&mut self, f3: i32, y: i32, x: i32) {
        self.p.group = 0;
        self.spawn_seq(0x2106, 6, 5, y - 1, x - 2, 1);
        self.throw(0x1c, 0x10, 0x6e0);
        self.spawn_seq(0x2106, 6, 5, y + 1, x + 2, 1);
        self.throw(0x1c, 0x10, 0x460);
        self.p.group = 1;
        self.p.colour = 4;
        self.p.callback = 0x253f;
        self.spawn_seq(f3 + 0x2100, 0x1a, 1, y, x, 1);
        self.line(0x10, 0x5a0);
        self.p.callback = 0;
        self.p.colour = 0;
    }

    /// The particles of FUN_1040_014a (a bonus picked up): sparks flying off.
    pub fn bonus_pickup(&mut self, y: i32, x: i32) {
        self.sparks_pickup(0x2100, y, x);
    }

    /// FUN_1040_014a / 01d9's sparks, of sprite `s`.
    pub fn sparks_pickup(&mut self, s: i32, y: i32, x: i32) {
        self.p.group = 0;
        for i in 0..2 {
            self.spawn_seq(s, 9, 4, y, x, 1);
            self.throw(0x1c, 0x10, i * 0x200 + 0x4a0);
            self.spawn_seq(s, 7, 4, y, x, 1);
            self.throw(0x1c, 8, i * 0xb40);
        }
        self.p.group = 1;
    }

    /// FUN_1050_457e(gravity, speed, angle): a bouncing throw (FUN_1050_2d37).
    pub fn bounce(&mut self, gravity: i32, speed: i32, angle: i32) {
        self.throw(gravity, speed, angle);
        self.last_mut().m.set(0x4a, MOVE_BOUNCE as i32);
    }

    /// FUN_1050_4870(speed, angle): a rocket that steers towards targets found by the hit tests
    /// (FUN_1050_0b2a). It starts one step back.
    pub fn rocket(&mut self, speed: i32, angle: i32) {
        let p = self.last_mut();
        p.m.set(0x4a, MOVE_ROCKET as i32);
        p.m.set(0x4c, 0x1c);
        let v = &mut p.v;
        v.set_l(0, p.m.w(8) << 16);
        v.set_l(4, p.m.w(0xa) << 16);
        v.set_l(8, (trig::cos(angle) * speed) / 4);
        v.set_l(0xc, (trig::sin(angle) * speed) / -4);
        v.set(0x10, angle);
        v.set(0x12, speed);
        v.set(0x18, 0x140);
        v.set(0x1a, 0);
        p.m.set(8, v.l(0).wrapping_sub(v.l(8)) >> 16);
        p.m.set(0xa, v.l(4).wrapping_sub(v.l(0xc)) >> 16);
    }

    /// FUN_1050_4296(life, damage, hits, bottom, right, top, left, y, x): a blast hurting what
    /// overlaps its box, less towards the edges (flag 0x20: FUN_1050_4b17).
    #[allow(clippy::too_many_arguments)]
    pub fn blast(&mut self, life: i32, damage: i32, hits: i32, bottom: i32, right: i32, top: i32, left: i32, y: i32, x: i32) {
        let p = self.p;
        let mut m = Obj::new(0, 0, 0x50);
        for off in [4, 8, 0xc] {
            m.set(off, x);
            m.set(off + 2, y);
        }
        m.set(0x22, life);
        m.set(0x34, hits);
        m.set(0x36, p.homing);
        m.set(0x3a, p.flags | 0x3b);
        m.set(0x38, damage);
        m.set(0x2c, x + left);
        m.set(0x2e, y + top);
        m.set(0x30, x + right);
        m.set(0x32, y + bottom);
        m.set(0x4a, MOVE_AGE as i32);
        m.set(0x42, 0x121);
        let particle = Particle {
            m,
            v: Obj::new(0, 0, 0x20),
        };
        if p.delay != 0 {
            let due = self.now.wrapping_add(p.delay as u32);
            let at = self.pending.iter().position(|(d, _)| due < *d).unwrap_or(self.pending.len());
            self.pending.insert(at, (due, particle));
            self.last = at;
            self.last_pending = true;
        } else {
            self.list.push(particle);
            self.last = self.list.len() - 1;
            self.last_pending = false;
        }
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

    /// FUN_1088_6cf0: weapon 8's three homing rockets (effect 57 where they burst).
    pub fn rockets(&mut self, aim: i32, y: i32, x: i32) {
        self.p.hits = 0x10;
        self.p.damage = 0x60;
        self.p.homing = 0x10;
        self.p.tiles = 0x143;
        self.p.on_death = 0x39;
        self.p.callback = 0x7bf;
        let (mut ly, mut lx) = (y, x);
        for a in [(aim + 0x1560) % 0x1680, (aim + 0x120) % 0x1680, aim] {
            lx = x + (0x17 * trig::cos(a)) / 0x4000;
            ly = y - (0x17 * trig::sin(a)) / 0x4000;
            self.spawn_seq(0x186e, 1, 0x100, ly, lx, 0);
            self.rocket(0x40, a);
        }
        self.p.callback = 0;
        self.p.on_death = 0;
        self.p.tiles = 0x43;
        self.p.homing = 0;
        self.p.damage = 0x10;
        self.p.hits = 0;
        self.spawn_seq(0x190a, 2, 2, ly, lx, 1);
    }

    /// FUN_1088_598e's torpedo: a homing missile that starts at rest and speeds up (callback 0xf9f).
    pub fn torpedo(&mut self, y: i32, x: i32, angle: i32) {
        self.p.group = 0;
        self.p.hits = 0x10;
        self.p.damage = 0xa0;
        self.p.flags = 0x284;
        self.p.homing = 0x10;
        self.p.tiles = 0x143;
        self.p.on_death = 0xe;
        self.p.callback = 0xf9f;
        self.spawn_seq(0x1932, 1, 0x100, y, x, 0);
        self.rocket(0, angle);
        self.last_mut().v.set(0x18, 0x100);
        self.p.callback = 0;
        self.p.on_death = 0;
        self.p.tiles = 0x43;
        self.p.flags = 0x84;
        self.p.homing = 0;
        self.p.damage = 0x10;
        self.p.hits = 0;
        self.p.group = 1;
    }

    /// FUN_1088_6deb's rocket (weapon 5): like the torpedo, it speeds up from 0x10 and leaves
    /// smoke (callback 0xd83); effect 14 when it hits.
    pub fn rocket5(&mut self, y: i32, x: i32, angle: i32) {
        self.p.hits = 0x410;
        self.p.damage = 0xa0;
        self.p.flags = 0x284;
        self.p.homing = 0x10;
        self.p.tiles = 0x143;
        self.p.on_death = 0xe;
        self.p.callback = 0xd83;
        self.spawn_seq(0x193c, 1, 0x100, y, x, 0);
        self.rocket(0x10, angle);
        self.last_mut().v.set(0x18, 0x100);
        self.p.callback = 0;
        self.p.on_death = 0;
        self.p.tiles = 0x43;
        self.p.flags = 0x84;
        self.p.homing = 0;
        self.p.damage = 0x10;
        self.p.hits = 0;
        self.spawn_seq(0x190a, 2, 2, y, x, 1);
    }

    /// FUN_1088_6b1f's rocket (weapon 3): homing at a steady speed of 0x30, leaving smoke
    /// (callback 0x586); effect 14 when it hits.
    pub fn rocket3(&mut self, y: i32, x: i32, angle: i32) {
        self.p.hits = 0x10;
        self.p.damage = 0x80;
        self.p.homing = 0x10;
        self.p.tiles = 0x143;
        self.p.on_death = 0xe;
        self.p.callback = 0x586;
        self.spawn_seq(0x1932, 1, 0x100, y, x, 0);
        self.rocket(0x30, angle);
        self.p.callback = 0;
        self.p.on_death = 0;
        self.p.tiles = 0x43;
        self.p.homing = 0;
        self.p.damage = 0x10;
        self.p.hits = 0;
        self.spawn_seq(0x190a, 2, 2, y, x, 1);
    }

    /// FUN_1088_678b's flame: a short-lived fire that follows its line and goes out in smoke
    /// (effect 37).
    pub fn flame(&mut self, y: i32, x: i32, angle: i32) {
        self.p.hits = 0x48;
        self.p.damage = 4;
        self.p.on_death = 0x25;
        self.p.flags = 0x94;
        self.p.tiles = 0x413;
        self.p.callback = 0x4ff;
        self.spawn_seq(0x1a00, 1, 0x11, y, x, 0);
        self.line(0x60, angle);
        self.p.callback = 0;
        self.p.tiles = 0x43;
        self.p.flags = 0x84;
        self.p.on_death = 0;
        self.p.damage = 0x10;
        self.p.hits = 0;
    }

    /// FUN_1088_69d8: weapon 2's bullet (effect 81 where it lands).
    pub fn rapid(&mut self, y: i32, x: i32, aim: i32, parity: i32) {
        self.p.hits = 0x3000;
        self.p.damage = 0x10;
        self.p.flags = 0x95;
        self.p.on_death = 0x51;
        self.p.callback = 0x1a3;
        // FUN_1088_63c9: which of the ten pictures for the angle.
        if (aim + 0x5a0) % 0x1680 > 0xb3f {
            self.p.draw = 1;
        }
        if aim > 0xb3f {
            self.p.draw2 = 1;
        }
        let q = (aim % 0x5a0 + 0x50) / 0xa0;
        let idx = if self.p.draw == self.p.draw2 { q } else { 9 - q };
        self.spawn(0x1852 + idx, y, x);
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
        self.spawn_seq(0x1952, 2, 2, y, x, 1);
    }

    /// FUN_1088_70b8 (weapon 6) from the muzzle (y0, x0): a flash, three bullets
    /// (FUN_1088_7020: at aim - 0x80, aim + 0x80 and aim, 0x17 px out, effect 62) and a second flash.
    pub fn spread(&mut self, y0: i32, x0: i32, aim: i32) {
        let out = |a: i32| {
            (
                y0 - (0x17 * trig::sin(a)) / 0x4000,
                x0 + (0x17 * trig::cos(a)) / 0x4000,
            )
        };
        let (y, x) = out(aim);
        self.p.colour = 0x28;
        self.spawn_seq(0x1b04, 4, 2, y, x, 1);
        self.p.colour = 0;
        self.p.hits = 1;
        self.p.damage = 0x10;
        self.p.on_death = 0x3e;
        for a in [(aim + 0x1600) % 0x1680, (aim + 0x80) % 0x1680, aim] {
            let (by, bx) = out(a);
            // FUN_1088_63c9: which of the ten pictures for the angle.
            if (a + 0x5a0) % 0x1680 > 0xb3f {
                self.p.draw = 1;
            }
            if a > 0xb3f {
                self.p.draw2 = 1;
            }
            let q = (a % 0x5a0 + 0x50) / 0xa0;
            let idx = if self.p.draw == self.p.draw2 { q } else { 9 - q };
            self.spawn(0x1848 + idx, by, bx);
            self.line(0x60, a);
            self.p.draw = 0;
            self.p.draw2 = 0;
        }
        self.p.on_death = 0;
        self.p.damage = 0x10;
        self.p.hits = 0;
        self.spawn_seq(0x190a, 2, 1, y, x, 1);
    }

    /// `hit_test` with the homing priority 1170:093c at 0 (wrecks, corpses, crates: rockets
    /// only steer at them when nothing better is around) and the drone's 093e at -1 (it never
    /// picks them), restored to 0x100 after.
    #[track_caller]
    pub fn hit_test_low(&mut self, bottom: i32, right: i32, top: i32, left: i32, mask: i32) -> i32 {
        self.p.aim_level = 0;
        self.p.aim_level2 = -1;
        let d = self.hit_test(bottom, right, top, left, mask);
        self.p.aim_level = 0x100;
        self.p.aim_level2 = 0x100;
        d
    }

    /// FUN_1050_568b(bottom, right, top, left, mask): the damage of the particles hitting this
    /// box. A hit particle stops (unless flag 1) and remembers where it hit.
    #[track_caller]
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
                    damage += blast_damage(m, bottom, right, top, left);
                } else {
                    damage += m.w(0x38);
                }
                if std::env::var_os("ET_HIT2").is_some() {
                    eprintln!("HIT2 {} box {left},{top},{right},{bottom} mask {mask:x} by {} <- {}", self.now, i, std::panic::Location::caller());
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
                let level = self.p.aim_level;
                let p = &mut self.list[i];
                let dx = (left + right) / 2 - p.m.w(8);
                let dy = (top + bottom) / 2 - p.m.w(0xa);
                aim_at(p, level, dx, dy);
            }
        }
        if mask & 0x59 != 0 {
            self.drone.consider(bottom, right, top, left, self.p.aim_level2);
            let (mx, my) = self.mouse;
            if left <= mx && top <= my && mx <= right && my <= bottom {
                self.pointer = if self.kick_pointer { 3 } else { 2 };
            }
        }
        damage
    }

    /// FUN_1050_51aa(sprite record, mask): like `hit_test` against every type-0 hit box of a
    /// queued sprite (frame, x, y, h-flip, v-flip) (FUN_10f0_2240; a flip negates the particle's
    /// box around the anchor without reordering it).
    pub fn hit_test_sprite(&mut self, rec: (i32, i32, i32, i32, i32), mask: i32) -> i32 {
        let (frame, sx, sy, hflip, vflip) = rec;
        let boxes = crate::sprites::hitboxes(frame);
        let mut damage = 0;
        self.hit_mask = 0;
        self.hit_index = -1;
        for i in (0..self.list.len()).rev() {
            let m = &mut self.list[i].m;
            if m.w(0x34) & mask == 0 {
                continue;
            }
            let (mut l, mut t, mut r, mut b) = (m.w(0x2c) - sx, m.w(0x2e) - sy, m.w(0x30) - sx, m.w(0x32) - sy);
            if std::env::var("ET_HIT").is_ok() && m.w(0x34) == 7 {
                eprintln!("BLAST rel {l},{t},{r},{b} flags {:x} dmg {} boxes {:?}", m.w(0x3a), m.w(0x38), boxes);
            }
            if hflip != 0 {
                (l, r) = (-r, -l);
            }
            if vflip != 0 {
                (t, b) = (-b, -t);
            }
            let mut hit = false;
            for hb in &boxes {
                if hb.kind == 0 {
                    let (x1, y1, x2, y2) = hb.rect;
                    if l <= x2 && t <= y2 && x1 <= r && y1 <= b {
                        hit = true;
                        break;
                    }
                } else if hb.kind > 0 {
                    break;
                }
            }
            if hit {
                if m.w(0x3a) & 0x20 != 0 {
                    // FUN_1050_51aa: against the box that was hit, placed and flipped.
                    let hb = boxes.iter().find(|hb| {
                        let (x1, y1, x2, y2) = hb.rect;
                        hb.kind == 0 && l <= x2 && t <= y2 && x1 <= r && y1 <= b
                    });
                    let (x1, y1, x2, y2) = hb.unwrap().rect;
                    let (bl, br) = if hflip == 0 { (sx + x1, sx + x2) } else { (sx - x2, sx - x1) };
                    let (bt, bb) = if vflip == 0 { (sy + y1, sy + y2) } else { (sy - y2, sy - y1) };
                    damage += blast_damage(m, bb, br, bt, bl);
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
                let level = self.p.aim_level;
                let p = &mut self.list[i];
                for hb in boxes.iter().take_while(|hb| hb.kind <= 0).filter(|hb| hb.kind == 0) {
                    let (x1, y1, x2, y2) = hb.rect;
                    let cx = if hflip == 0 { (x1 + x2) / 2 + sx } else { sx - (x1 + x2) / 2 };
                    let cy = if vflip == 0 { (y1 + y2) / 2 + sy } else { sy - (y1 + y2) / 2 };
                    let (dx, dy) = (cx - p.m.w(8), cy - p.m.w(0xa));
                    aim_at(p, level, dx, dy);
                }
            }
        }
        if mask & 0x59 != 0 {
            // FUN_10f0_2240 with the pointer as a one-pixel box.
            let (mx, my) = self.mouse;
            let (mut px, mut py) = (mx - sx, my - sy);
            if hflip != 0 {
                px = -px;
            }
            if vflip != 0 {
                py = -py;
            }
            let over = boxes.iter().take_while(|hb| hb.kind <= 0).any(|hb| {
                let (x1, y1, x2, y2) = hb.rect;
                hb.kind == 0 && x1 <= px && px <= x2 && y1 <= py && py <= y2
            });
            if over {
                self.pointer = if self.kick_pointer { 3 } else { 2 };
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
    /// The hero's x, y, height and half width.
    pub hero: (i32, i32, i32, i32),
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
            self.move_particle(i, f.camera, f.world, f.rng);
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
    fn move_particle(&mut self, i: usize, cam: (i32, i32), w: &World, rng: &mut PascalRandom) {
        let p = &mut self.particles.list[i];
        if p.m.uw(0x4a) == MOVE_AGE {
            p.m.set(0x24, p.m.w(0x24) + 1);
            return;
        }
        sprite_frame(&mut p.m);
        if p.m.uw(0x4a) == MOVE_CRITTER {
            self.move_critter(i, cam, w, rng);
            return;
        }
        let p = &mut self.particles.list[i];
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
            MOVE_BG_THROW => {
                let t = v.w(8);
                m.set(8, m.w(4) + v.l(0).wrapping_mul(t) / 0x40000);
                let fall = ((t * t) as i16 as i32) / v.w(0xa);
                m.set(0xa, fall + (m.w(6) - v.l(4).wrapping_mul(t) / 0x40000));
                v.set(8, t + 1);
                if (m.w(0xa) - ((cam.1 as u16) >> 1) as i32) < 0x1c8 {
                    m.set(0x24, m.w(0x24) + 1);
                } else {
                    m.set(0x24, m.w(0x22));
                    m.set(0x3e, -10);
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
            MOVE_BOUNCE => {
                let t = v.w(8);
                m.set(8, m.w(4) + v.l(0).wrapping_mul(t) / 0x40000);
                let fall = (t * t) / v.w(0xa);
                m.set(0xa, fall + (m.w(6) - v.l(4).wrapping_mul(t) / 0x40000));
                v.set(8, t + 1);
                if off_screen(m, -0x200) {
                    m.set(0x24, m.w(0x22));
                    m.set(0x3e, -10);
                    return;
                }
                m.set(0x24, m.w(0x24) + 1);
                if m.w(0x4e) != 0x7fff {
                    eprintln!("warning: particle splashes (FUN_1050_2bcf) not ported");
                }
                // Rolling down a slope: on it with no vertical speed, it speeds up downhill
                // (by half its speed, at least 1/2 px), at most 16 px per frame; else it bounces.
                let (x, y) = (m.w(8), m.w(0xa));
                if w.solidity_at(y - 1, x) & 0x20 != 0 {
                    let cell = w.cell_at(crate::world::MAIN_PLANE, y - 1, x).unwrap_or(0);
                    let ground = ((y - 1) & 0xfff0) + crate::actors::slope_height(w, x, cell);
                    if ground < y {
                        if v.l(4) == 0 {
                            let a = (v.l(0).wrapping_abs() >> 1).max(0x8000);
                            if w.cell_function(cell, 2) & 0x80 == 0 {
                                v.set_l(0, v.l(0).wrapping_sub(a).max(-0x100000));
                            } else {
                                v.set_l(0, v.l(0).wrapping_add(a).min(0x100000));
                            }
                        } else {
                            let t1 = v.w(8);
                            let t0 = t1 - 1;
                            let fall = ((t1 * t1) - (t0 * t0) as i16 as i32) / v.w(0xa);
                            let vy = fall.wrapping_mul(0x40000).wrapping_sub(v.l(4)).wrapping_mul(v.w(0xc)) / 0x10;
                            v.set_l(4, if vy < 0x20000 { 0 } else { vy });
                        }
                        v.set(8, 1);
                        m.set(4, m.w(8));
                        m.set(6, ground);
                        m.set(0xa, ground);
                        m.set(0xe, ground);
                    }
                }
                let (x, y) = (m.w(8), m.w(0xa));
                if w.tile_function_at(y + 1, x) == 11000 {
                    let cell = w.cell_at(crate::world::MAIN_PLANE, y + 1, x).unwrap_or(0);
                    m.set(4, m.w(4) + w.cell_function(cell, 3) as i16 as i32);
                }
            }
            MOVE_ROLL => {
                let t = v.w(8);
                m.set(8, m.w(4) + v.l(0).wrapping_mul(t) / 0x40000);
                let fall = (t * t) / v.w(0xa);
                m.set(0xa, fall + (m.w(6) - v.l(4).wrapping_mul(t) / 0x40000));
                v.set(8, t + 1);
                if off_screen(m, -0x200) {
                    m.set(0x24, m.w(0x22));
                    m.set(0x3e, -10);
                    return;
                }
                m.set(0x24, m.w(0x24) + 1);
                if m.w(0x4e) != 0x7fff {
                    eprintln!("warning: particle splashes (FUN_1050_2bcf) not ported");
                }
                let (x, y) = (m.w(8), m.w(0xa));
                let mut steep = false;
                if w.solidity_at(y, x) & 0x20 != 0 {
                    let cell = w.cell_at(crate::world::MAIN_PLANE, y, x).unwrap_or(0);
                    let k = (w.cell_function(cell, 2) & 0xff7f) as usize * 0x16 + 0x14;
                    steep = u16::from_le_bytes([crate::world::SLOPES[k], crate::world::SLOPES[k + 1]]) & 4 == 0;
                    let ground = (y & 0xfff0) + crate::actors::slope_height(w, x, cell) - 1;
                    if ground < y + 1 {
                        if v.l(4) == 0 {
                            v.set_l(0, if v.l(0) < 0 { -0x40000 } else { 0x40000 });
                        } else {
                            let t1 = v.w(8);
                            let t0 = t1 - 1;
                            let fall = ((t1 * t1) - (t0 * t0) as i16 as i32) / v.w(0xa);
                            let vy = fall.wrapping_mul(0x40000).wrapping_sub(v.l(4)).wrapping_mul(v.w(0xc)) / 0x10;
                            v.set_l(4, if vy < 0x20000 { 0 } else { vy });
                        }
                        v.set(8, 1);
                        m.set(4, m.w(8));
                        m.set(6, ground);
                        m.set(0xa, ground);
                        m.set(0xe, ground);
                    }
                }
                let (x, y) = (m.w(8), m.w(0xa));
                if w.tile_function_at(y + 1, x) == 11000 {
                    let cell = w.cell_at(crate::world::MAIN_PLANE, y + 1, x).unwrap_or(0);
                    m.set(4, m.w(4) + w.cell_function(cell, 3) as i16 as i32);
                }
                if v.w(8) < 5 {
                    if m.w(0x22) < 0x1e1 {
                        m.set(0x34, 1);
                    }
                    let (x, y) = (m.w(8), m.w(0xa));
                    m.set(0x2c, x);
                    m.set(0x2e, y - 0x14);
                    m.set(0x30, x);
                    m.set(0x32, y);
                    if v.l(4) == 0 && !steep {
                        let ax = if v.l(0) < 0 { x - 0x10 } else { x + 0x10 };
                        if w.solidity_at(y - 1, ax) & 2 != 0 && w.solidity_at(y - 0x11, ax) & 2 == 0 {
                            if w.solidity_at(y, x) & 2 != 0 {
                                m.set(0xa, y - 1);
                            }
                            m.set(4, m.w(8));
                            m.set(6, m.w(0xa));
                            v.set(8, 1);
                            v.set_l(4, 0xe0000);
                        }
                    }
                } else {
                    m.set(0x34, 0x8000);
                }
            }
            MOVE_ROCKET => {
                m.set(8, v.l(0) >> 16);
                m.set(0xa, v.l(4) >> 16);
                v.set_l(0, v.l(0).wrapping_add(v.l(8)));
                v.set_l(4, v.l(4).wrapping_add(v.l(0xc)));
                if off_screen(m, -0x100) {
                    m.set(0x24, m.w(0x22));
                    m.set(0x3e, -10);
                } else {
                    m.set(0x24, m.w(0x24) + 1);
                }
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
                // FUN_1088_14a3: a shot lever steps to its next state, after the last back to
                // its first (FUN_1088_1364 sets the cell: its scripts run).
                let wd = f.world.main().width;
                let idx = ((y >> 4) * wd + (x >> 4)) as u16;
                let v = f.world.main().cells[idx as usize];
                let fn0 = |t: u16| f.world.cell_function(t, 0);
                let mut t = v;
                if func != 0x1779 && fn0(v.wrapping_add(1)) == func + 1 {
                    t = v.wrapping_add(1);
                } else {
                    let mut g = func;
                    while g > 6000 && fn0(t.wrapping_sub(1)) == g - 1 {
                        g -= 1;
                        t = t.wrapping_sub(1);
                    }
                }
                self.set_cell(f.world, t, idx, f.fx);
            } else if func == crate::doors::DOOR {
                self.door_hit(f.world, strength, y, x, 0, f.fx);
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
        let cb = m.uw(0x42);
        match cb {
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
            0xf9f | 0xd83 => {
                // FUN_1050_0f9f (the torpedo) and FUN_1050_0d83 (weapon 5's rocket, the same
                // without the bubbles): it turns (at most 0x10) towards its target, speeds up,
                // and leaves a trail.
                let (cx, age) = (m.w(0x22), m.w(0x24));
                let _ = cx;
                let p = &mut self.particles.list[i];
                let (m, v) = (&mut p.m, &mut p.v);
                if v.w(0x18) != 0x100 {
                    let a = trig::atan2(v.w(0x16), v.w(0x14));
                    if v.w(0x10) != a {
                        let d = ((a - v.w(0x10)) + 0x21c0) % 0x1680;
                        let na = if d < 0xb40 {
                            ((v.w(0x10) + 0x1680) - 0x10.min(0xb40 - d)) % 0x1680
                        } else {
                            (0x10.min(d - 0xb40) + v.w(0x10)) % 0x1680
                        };
                        v.set(0x10, na);
                    }
                    v.set(0x18, 0x100);
                    v.set(0x1a, 0);
                }
                v.set(0x12, v.w(0x12) + 2);
                v.set_l(8, (trig::cos(v.w(0x10)) * v.w(0x12)) / 4);
                v.set_l(0xc, (trig::sin(v.w(0x10)) * v.w(0x12)) / -4);
                let (x, y) = (m.w(8), m.w(0xa));
                let _ = m;
                let ps = &mut self.particles;
                if age % 4 == 3 {
                    ps.p.delay = 4;
                    ps.p.group = 0;
                    ps.p.colour = 0x28;
                    ps.spawn_seq(0x1b19, 4, 5, y, x, 1);
                    ps.p.colour = 0;
                    ps.p.group = 1;
                    ps.p.delay = 0;
                }
                if cb == 0xf9f && age % 7 == 6 {
                    let sprite = f.rng.random(3) + 0x64b;
                    if self.water + 0x10 <= y {
                        let ps = &mut self.particles;
                        ps.p.callback = 0x30f;
                        ps.spawn(sprite, y, x);
                        ps.p.callback = 0;
                    }
                }
            }
            0x586 => {
                // FUN_1050_0586 (weapon 3's rocket): it turns (at most 0x40) towards its target
                // at a steady speed, leaves smoke, and goes off when its life runs out.
                let p = &mut self.particles.list[i];
                let (m, v) = (&mut p.m, &mut p.v);
                if v.w(0x18) != 0x140 {
                    let a = trig::atan2(v.w(0x16), v.w(0x14));
                    if v.w(0x10) != a {
                        let d = ((a - v.w(0x10)) + 0x21c0) % 0x1680;
                        let na = if d < 0xb40 {
                            ((v.w(0x10) + 0x1680) - 0x40.min(0xb40 - d)) % 0x1680
                        } else {
                            (0x40.min(d - 0xb40) + v.w(0x10)) % 0x1680
                        };
                        v.set(0x10, na);
                        v.set_l(8, (trig::cos(na) * v.w(0x12)) / 4);
                        v.set_l(0xc, (trig::sin(na) * v.w(0x12)) / -4);
                    }
                    v.set(0x18, 0x140);
                    v.set(0x1a, 0);
                }
                let (x, y, age, life) = (m.w(8), m.w(0xa), m.w(0x24), m.w(0x22));
                if age % 4 == 3 {
                    let ps = &mut self.particles;
                    ps.p.delay = 4;
                    ps.p.group = 0;
                    ps.spawn_seq(0x5111, 3, 9, y, x, 1);
                    ps.p.group = 1;
                    ps.p.delay = 0;
                }
                let m = &mut self.particles.list[i].m;
                if life <= age && m.w(0x3e) > -10 {
                    m.set(0x3e, -1);
                    m.set(0x10, x);
                    m.set(0x12, y);
                }
            }
            0x30f => {
                // FUN_1050_030f: a bubble rises through the water, slides round walls, and pops
                // at the surface.
                let water = self.water;
                let (x, y) = (m.w(8), m.w(0xa));
                let life = m.w(0x22);
                let sprite = m.w(0x14);
                if y < water {
                    m.set(0x24, life);
                    let ps = &mut self.particles;
                    ps.p.group = 0;
                    ps.p.colour = 0x10;
                    ps.spawn_seq(0x64e, 4, 4, water - 1, x, 1);
                    ps.p.colour = 0;
                    ps.p.group = 1;
                    if sprite == 0x64b {
                        let p = f.rng.random(0x80);
                        let v = f.rng.random(0x80);
                        crate::sound::pitch(p + 0xc0);
                        crate::sound::play_at_vol(0x3f, 0x100 - v, y, x);
                    }
                } else {
                    let w = &*f.world;
                    let mut done = false;
                    if w.solidity_at(y, x) & 0x40 == 0 {
                        m.set(0xa, y - 1);
                    } else if w.solidity_at(y + 1, x) & 0x40 != 0 {
                        m.set(0x24, life);
                        done = true;
                    } else {
                        let mut k = 0x10;
                        while k < 0x50 {
                            if w.solidity_at(y, (x + 9) - k) & 0x40 == 0 {
                                m.set(8, x - 1);
                                break;
                            }
                            if w.solidity_at(y, x + k) & 0x40 == 0 {
                                m.set(8, x + 1);
                                break;
                            }
                            k += 0x10;
                        }
                        if k > 0x4f {
                            m.set(0x24, life);
                            done = true;
                        }
                    }
                    if !done {
                        let (x, y) = (m.w(8), m.w(0xa));
                        let (cx, cy) = f.camera;
                        if x - cx < -0x100 || y - cy < -0x100 || 0x23f < x - cx || 0x1c7 < y - cy {
                            m.set(0x24, life);
                        }
                    }
                }
            }
            0x4ff => {
                // FUN_1050_04ff: the flame's picture (age / 2 into its frames) and its box.
                let (x, y) = (m.w(8), m.w(0xa));
                let frame = (m.uw(0x24) >> 1) as i32 + m.w(0x14);
                m.set(0x16, frame);
                if let Some((l, t, r, b)) = crate::sprites::draw_box(frame) {
                    m.set(0x2c, x + l);
                    m.set(0x2e, y + t);
                    m.set(0x30, x + r);
                    m.set(0x32, y + b);
                } else {
                    m.set(0x2c, x);
                    m.set(0x2e, y);
                }
            }
            CB_CRITTER => {
                // FUN_1050_1656 (drawing): its frame, which the shots' hit test uses.
                let t = self.particles.list[i].v.w(8);
                let m = &mut self.particles.list[i].m;
                m.set(0x16, if t < 5 { (m.w(0x24) / 6) % 6 + 0x5215 } else { 0x5218 });
            }
            0x16cc => {
                // FUN_1050_16cc (drawing): weapon 9's ball turns while it bounces.
                let t = self.particles.list[i].v.w(8);
                let m = &mut self.particles.list[i].m;
                m.set(0x16, if t < 5 { ((m.uw(0x24) >> 3) & 3) as i32 + 0x1900 } else { 0x1904 });
            }
            0x2d2 | 0x2946 | 0x1c92 | 0x1da7 | 0x186e | 0x137a | 0x17f4 | 0x2148 | 0x2192 | 0x13c3 | 0x121 | 0x12f6 | 0x253f | 0x177a | 0x232e | 0x2459 | 0x2497 | 0x236b => {} // drawing only
            0x2299 => {
                // FUN_1050_2299: a burning bit trails smoke.
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
            0x14bd => {
                // FUN_1050_14bd: two fires crawl along the floor either way, every 6 frames.
                if m.w(0x24) % 6 == 1 {
                    f.fx.shakes.push((0x2a, 10));
                    if m.w(0x24) % 0xc == 1 {
                        sound::play_at_vol(0x47, ((0x2a - m.w(0x24)) * 0x180) / 0x2a, m.w(0xa), m.w(8));
                    }
                    for k in 0..2usize {
                        let (mut x, mut y, mut d) = {
                            let v = &self.particles.list[i].v;
                            (v.w(6 * k), v.w(6 * k + 2), v.w(6 * k + 4))
                        };
                        let nx = x - 0x10 + d * 0x20;
                        if f.world.solidity_at(y, nx) & 2 == 0 {
                            if f.world.solidity_at(y + 0x10, nx) & 0x211 == 0 {
                                if f.world.solidity_at(y + 0x20, nx) & 0x211 == 0 {
                                    d ^= 1;
                                } else {
                                    x = nx;
                                    y = (y & !15) + 0x1f;
                                }
                            } else {
                                x = nx;
                            }
                        } else if f.world.solidity_at(y - 0x10, nx) & 2 == 0 {
                            x = nx;
                            y = (y & !15) - 1;
                        } else {
                            d ^= 1;
                        }
                        let v = &mut self.particles.list[i].v;
                        v.set(6 * k, x);
                        v.set(6 * k + 2, y);
                        v.set(6 * k + 4, d);
                        // FUN_1050_144b: a flame, tiles blown, a blast.
                        let ps = &mut self.particles;
                        ps.p.callback = 0x1b90;
                        ps.spawn_seq(0x6a00, 1, 0x46, y, x, 0);
                        ps.p.callback = 0;
                        self.explode_area(f.world, 0xc0, 4, y + 0x10, x + 0x12, y - 0x60, x - 0x12, f.rng, f.fx);
                        self.particles.blast(2, 0x60, 0xd, 0x20, 0x22, -0x60, -0x22, y, x);
                    }
                }
            }
            0x19f8 => {
                // FUN_1050_19f8: a long fire on the floor goes out without floor under it (it
                // ages only when the game runs late, 4f0c > 10).
                if f.world.solidity_at(m.w(0xa) + 1, m.w(8)) & 0x211 == 0 {
                    m.set(0x24, m.w(0x22));
                } else if FRAME_SLACK {
                    m.set(0x24, m.w(0x24) + 1);
                }
            }
            0x1b90 => {
                // FUN_1050_1b90: a flame goes out without floor under it.
                if f.world.solidity_at(m.w(0xa) + 1, m.w(8)) & 0x211 == 0 {
                    m.set(0x24, m.w(0x22));
                }
            }
            0x2588 => {
                // FUN_1050_2588: the hero catches a bonus flying out of a wall or an enemy.
                let (x, y, tile) = (m.w(8), m.w(0xa), m.w(0x40));
                let (hx, hy, hh, hw) = f.hero;
                if x <= hx + hw - 2 && (hx - hw) + 2 <= x + 0xf && y <= hy - 4 && (hy - hh) + 5 <= y + 0xf {
                    f.fx.bonus_pickups.push((tile as u16, y + 7, x + 7));
                    m.set(0x24, m.w(0x22));
                    m.set(0x2a, 0);
                }
            }
            0x7bf => {
                // FUN_1050_07bf: a rocket of the hero's: turns (at most 0x40) towards its target,
                // smokes, and blows up at the end of its fuel.
                let seeking = self.particles.list[i].v.w(0x18) != 0x140;
                let p = &mut self.particles.list[i];
                let (m, v) = (&mut p.m, &mut p.v);
                if seeking {
                    let a = trig::atan2(v.w(0x16), v.w(0x14));
                    if v.w(0x10) != a {
                        let d = ((a - v.w(0x10)) + 0x21c0) % 0x1680;
                        let na = if d < 0xb40 {
                            ((v.w(0x10) + 0x1680) - 0x40.min(0xb40 - d)) % 0x1680
                        } else {
                            (0x40.min(d - 0xb40) + v.w(0x10)) % 0x1680
                        };
                        v.set(0x10, na);
                        v.set_l(8, (trig::cos(na) * v.w(0x12)) / 4);
                        v.set_l(0xc, (trig::sin(na) * v.w(0x12)) / -4);
                    }
                    v.set(0x18, 0x140);
                    v.set(0x1a, 0);
                }
                let (x, y, age, life, state) = (m.w(8), m.w(0xa), m.w(0x24), m.w(0x22), m.w(0x3e));
                if age % 4 == 3 {
                    let ps = &mut self.particles;
                    ps.p.delay = 4;
                    ps.p.group = 0;
                    ps.spawn_seq(0x5111, 2, 9, y, x, 1);
                    ps.p.group = 1;
                    ps.p.delay = 0;
                }
                if life <= age && state > -10 {
                    let m = &mut self.particles.list[i].m;
                    m.set(0x3e, -1);
                    m.set(0x10, x);
                    m.set(0x12, y);
                }
            }
            0xb2a => {
                // FUN_1050_0b2a: the rocket turns (at most 0x20) towards the target last found,
                // smokes, and blows up at the end of its fuel.
                let (seeking, level) = {
                    let v = &self.particles.list[i].v;
                    (v.w(0x18) != 0x140, v.w(0x1a))
                };
                if seeking && level == 0x100 && self.particles.drone.flare_due() {
                    self.particles.drone_flare(f.rng);
                }
                let p = &mut self.particles.list[i];
                let (m, v) = (&mut p.m, &mut p.v);
                if seeking {
                    let a = trig::atan2(v.w(0x16), v.w(0x14));
                    if v.w(0x10) != a {
                        let d = ((a - v.w(0x10)) + 0x21c0) % 0x1680;
                        let na = if d < 0xb40 {
                            ((v.w(0x10) + 0x1680) - 0x20.min(0xb40 - d)) % 0x1680
                        } else {
                            (0x20.min(d - 0xb40) + v.w(0x10)) % 0x1680
                        };
                        v.set(0x10, na);
                        v.set_l(8, (trig::cos(na) * v.w(0x12)) / 4);
                        v.set_l(0xc, (trig::sin(na) * v.w(0x12)) / -4);
                    }
                    v.set(0x18, 0x140);
                    v.set(0x1a, 0);
                }
                let (x, y, age, life, state) = (m.w(8), m.w(0xa), m.w(0x24), m.w(0x22), m.w(0x3e));
                if life <= age && state > -10 {
                    m.set(0x3e, -1);
                    m.set(0x10, x);
                    m.set(0x12, y);
                }
                if age % 4 == 3 {
                    let ps = &mut self.particles;
                    ps.p.delay = 5;
                    ps.p.group = 0;
                    ps.p.colour = 0x10;
                    ps.spawn_seq(0x5111, 2, 9, y, x, 1);
                    ps.p.colour = 0;
                    ps.p.group = 1;
                    ps.p.delay = 0;
                }
            }
            0x274f => {
                // FUN_1050_274f: a flare sputters smoke and draws rockets to itself.
                let age = self.particles.list[i].m.w(0x24);
                if age % 8 == 0 && f.rng.random(4) == 0 {
                    let (x, y) = (self.particles.list[i].m.w(8), self.particles.list[i].m.w(0xa));
                    let ps = &mut self.particles;
                    ps.p.delay = 4;
                    ps.p.group = 0;
                    ps.p.colour = 0x28;
                    let r = f.rng.random(3);
                    let steps = f.rng.random(6) + 8;
                    ps.spawn_seq(0x1b1e + r, steps, 5 - r, y + 4, x, 1);
                    ps.throw(0x38, 0, 0);
                    ps.p.colour = 0;
                    ps.p.group = 1;
                    ps.p.delay = 0;
                }
                let (x, y) = (self.particles.list[i].m.w(8), self.particles.list[i].m.w(0xa));
                self.particles.p.aim_level = 0x200;
                let d = self.particles.hit_test(y + 3, x + 3, y - 3, x - 3, 0x200);
                self.particles.p.aim_level = 0x100;
                let m = &mut self.particles.list[i].m;
                if d > 0 {
                    m.set(0x24, m.w(0x22));
                }
                if m.w(0x22) <= m.w(0x24) {
                    m.set(0x10, x);
                    m.set(0x12, y);
                }
            }
            0x1a3 | 0x229 => {
                // FUN_1050_01a3 / 0229: a 9x9 box.
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
            0x18e6 => {
                // FUN_1050_18e6: smoke lying on the floor; gone when there's no floor.
                let (x, y) = (m.w(8), m.w(0xa));
                if f.world.solidity_at(y + 1, x) & 0x211 == 0 {
                    m.set(0x24, m.w(0x22));
                } else if SMOKE_AGES && m.w(0x24) < m.w(0x22) {
                    m.set(0x24, m.w(0x24) + 1);
                }
            }
            0x1d33 => {} // FUN_1050_1d33: a splash, drawn clipped at the water line
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
        if std::env::var_os("ET_DEBUG").is_some() {
            eprintln!("WALL {} at ({x},{y}) strength {strength}", self.particles.now);
        }
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
            // Effect 23 (FUN_1040_2af4): a flash of fire, scaled by FUN_1050_4a29(0x180, 0x400).
            ps.spawn_seq(0x6900, 2, 0x19, y - 2, x - 2, 1);
            let m = &mut ps.last_mut().m;
            m.set(0x1c, 0x400);
            m.set(0x1e, 0x180);
            m.set(0x42, 0x2148);
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
            sound::play_at(0x4b, y, x);
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
        sound::play_at(1, y, x);
    }

    /// A death effect (FUN_1040_60bb) with the dying particle current; its procedure may clear
    /// `keep_dying` to live on.
    fn particle_effect(&mut self, id: i32, y: i32, x: i32, f: &mut Frame) {
        match id {
            62 => self.bullet_hit(y, x, f),
            16 => {
                self.bounce_off(f);
            }
            44 => self.shell7_hit(f),
            56 => self.ball9_hit(f),
            66 => {
                // FUN_1040_1137: the bonus lands as a pick-up tile in plane 2.
                let m = &self.particles.list[self.particles.current].m;
                let (cy, cx, tile) = (m.w(0xa), m.w(8), m.w(0x40));
                if let Some(cur) = f.world.cell_at(2, cy, cx) {
                    let cur = cur as i16 as i32;
                    let ok = cur == 0
                        || (cur > 0 && f.world.cell_function(cur as u16, 0) == 30000 && f.world.cell_function(cur as u16, 4) != 0x46);
                    if ok {
                        let w = f.world.main().width;
                        let idx = ((cy >> 4) * w + (cx >> 4)) as usize;
                        f.world.planes[2].as_mut().unwrap().cells[idx] = tile as u16;
                    }
                }
            }
            57 => {
                // FUN_1040_556e: a rocket bursts (silently if its +0x3e is -10).
                {
                    let m = &self.particles.list[self.particles.current].m;
                    if m.w(0x3e) != -10 {
                        sound::play_at(0x49, m.w(0xa), m.w(8));
                    }
                }
                f.fx.shakes.push((0x12, 4));
                let ps = &mut self.particles;
                ps.spawn_seq(0x6900, 2, 0x19, y, x, 1); // effect 23
                let r = f.rng.random(0x40);
                let m = &mut ps.last_mut().m;
                m.set(0x1c, r + 0x1cc);
                m.set(0x1e, r + 0x180);
                m.set(0x42, 0x2148);
                self.explode_area(f.world, 0x60, 4, y + 0x20, x + 0x20, y - 0x20, x - 0x20, f.rng, f.fx);
                let n = f.rng.random(4);
                crate::hero::spawn_dust_pub(f.world, n, y - 7, x, f.rng);
            }
            81 => {
                // FUN_1040_07e0: sparks thrown back from the bullet's line.
                let v = &self.particles.list[self.particles.current].v;
                let side = if v.l(0) <= 0 { -1 } else { 1 };
                let angle = v.w(8);
                let ps = &mut self.particles;
                ps.p.colour = 0x10;
                ps.spawn_seq(0x1918, 4, 3, y, x, 1);
                ps.throw(0xe, 0x30, (angle + 0xb40 - side * 0x1e0) % 0x1680);
                ps.spawn_seq(0x1918, 5, 3, y, x, 1);
                ps.throw(0xe, 0x18, (angle + 0xb40 + side * 0x1e0) % 0x1680);
                ps.p.colour = 0;
            }
            9 => {
                self.particles.spawn_seq(0x6500, 2, 0x14, y, x, 1);
            }
            28 => {
                // FUN_1040_300c: a hull piece lands: two bits fly off, smoke and a blast.
                let m = &self.particles.list[self.particles.current].m;
                let (py, px) = (m.w(0xa), m.w(8));
                let ps = &mut self.particles;
                ps.p.group = 0;
                ps.p.hits = 0x8000;
                ps.p.on_death = 0x1b;
                ps.p.draw = 1;
                ps.spawn(0x4e08, py, px + 2);
                let r = f.rng.random(0xa0);
                ps.throw(9, 0x40, 0x500 - r);
                ps.p.on_death = 0x11;
                ps.p.draw = 0;
                ps.spawn(0x4e08, py, px - 2);
                let r = f.rng.random(0xa0);
                ps.throw(10, 0x48, r + 0x640);
                ps.p.on_death = 0;
                ps.p.hits = 0;
                ps.p.group = 1;
                ps.spawn_seq(0x6500, 2, 0x14, py, px, 1); // effect 9
                sound::play_at(0x4e, py, px);
                ps.p.delay = 0x1e;
                smoke4_at(ps, py, px + 10);
                ps.p.delay = 0x46;
                smoke4_at(ps, py - 6, px + 2);
                ps.p.delay = 0;
                f.fx.shakes.push((0x18, 8));
            }
            7 => {
                // FUN_1040_0897: a puff that drifts up.
                self.particles.spawn_seq(0x6400, 2, 0x24, y, x, 1);
                self.particles.line(2, 0x5a0);
            }
            29 => {
                // FUN_1040_3117: like 17, a heavy bit that bounces and puffs smoke.
                let v = self.bounce_off(f);
                if v >= 0 && v < 0x2aaab && self.particles.hit_side != 2 {
                    crate::alien::smoke_sprite(&mut self.particles, y, x);
                    let ps = &mut self.particles;
                    puff_at(ps, y - 2, x - 2);
                    ps.p.delay = 0xc;
                    puff_at(ps, y - 0xc, x + 3);
                    ps.p.delay = 0x12;
                    puff_at(ps, y - 0x18, x - 10);
                    ps.p.delay = 0;
                    let p = f.rng.random(0x14);
                    sound::play_at_pitch(0x4b, p + 0xf6, y, x);
                    f.fx.shakes.push((0x18, 8));
                }
            }
            17 => {
                // FUN_1040_0fcc: a heavy bit bounces; landing slowly it puffs smoke.
                let v = self.bounce_off(f);
                if v >= 0 && v < 0x2aaab && self.particles.hit_side != 2 {
                    let ps = &mut self.particles;
                    ps.spawn_seq(0x6f00, 2, 0x18, y, x, 1);
                    let r = f.rng.random(0x100);
                    let m = &mut ps.last_mut().m;
                    m.set(0x1c, r + 0x200);
                    m.set(0x1e, r + 0x240);
                    m.set(0x42, 0x2148);
                    let p = f.rng.random(0x14); // the sound's pitch
                    sound::play_at_pitch(0x4b, p + 0xf6, y, x);
                    f.fx.shakes.push((0x12, 5));
                }
            }
            13 => {
                // FUN_1040_0c4d: a shell casing tinkles; once it stops it lies on the floor (an
                // object in the list +0x5a that only draws: not kept here).
                let v = self.bounce_off(f);
                if (v >> 16) > 1 {
                    // A tinkle, louder the harder it lands.
                    sound::set_volume((((v - 0x20000) >> 13) + 0x80).min(0x100));
                    sound::play_at(if f.rng.random(8) == 0 { 0x23 } else { 0x44 }, y, x);
                    sound::set_volume(0x100);
                }
                if self.particles.keep_dying {
                    let draw = f.rng.random(2);
                    let m = &self.particles.list[self.particles.current].m;
                    let (sprite, py, px) = (m.w(0x14) + 3, m.w(0x12), m.w(0x10));
                    self.lay_decal(f.world, crate::decals::SHELL, draw, sprite, py, px);
                }
            }
            14 => self.rocket_blast(y, x, f),
            64 => {
                // FUN_1040_602a: the flare burns out in five trails of smoke (the group stays 0).
                let ps = &mut self.particles;
                ps.p.group = 0;
                ps.p.colour = 0x28;
                for i in 0..5 {
                    let r = f.rng.random(3);
                    ps.spawn_seq(0x1b1e + r, 0x10, 5 - r, y, x, 1);
                    let angle = (0x2180 - i * 0x2d0 + f.rng.random(0x81)) % 0x1680;
                    let speed = f.rng.random(8) + 8;
                    ps.throw(0x38, speed, angle);
                }
                ps.p.colour = 0;
            }
            42 => {
                // FUN_1040_45a3: a burning bit goes out.
                crate::alien::smoke_sprite(&mut self.particles, y, x);
                let p = f.rng.random(0x14);
                sound::play_at_pitch(0x4b, p + 0xf6, y, x);
                f.fx.shakes.push((0x12, 6));
            }
            21 => self.grenade_hit(f, false),
            0x4d => self.grenade_hit(f, true),
            0x4e => self.spark_bounce(f),
            94 => self.critter_hits(f),
            23 => {
                // FUN_1040_2af4: a flash of fire.
                self.particles.spawn_seq(0x6900, 2, 0x19, y, x, 1);
            }
            63 => {
                // FUN_1040_075a: an enemy bullet hits.
                let i = self.particles.current;
                let m = &self.particles.list[i].m;
                let (py, px) = (m.w(0xa), m.w(8));
                if f.world.tile_function_at(py, px) == 13000 {
                    let cell = f.world.cell_at(crate::world::MAIN_PLANE, py, px).unwrap_or(0);
                    let id = f.world.cell_function(cell, 4) as i16 as i32;
                    self.particle_effect(id, y, x, f);
                    return;
                }
                if m.w(0x3e) == -1 && f.rng.random(8) == 0 {
                    let p = f.rng.random(0x40);
                    crate::sound::play_at_pitch(0x7c, 0x100 - p, y, x);
                }
                self.sparks(0x1918, y, x);
            }
            59 => {
                // FUN_1040_5ec6: a burning bit lands: smoke against a wall, on the floor a flame
                // (one in four also a lingering smoke sprite).
                let i = self.particles.current;
                {
                    let m = &mut self.particles.list[i].m;
                    m.set(8, m.w(0x10));
                    m.set(0xa, m.w(0x12));
                }
                self.particle_callback(i, f);
                let side = self.particles.hit_side;
                let (py, px) = {
                    let m = &self.particles.list[i].m;
                    (m.w(0xa), m.w(8))
                };
                let ps = &mut self.particles;
                if side < 2 {
                    ps.p.colour = 0x10;
                    smoke(ps, f.rng, y, x);
                    ps.p.colour = 0;
                } else if side != 2 {
                    if f.rng.random(4) == 0 {
                        ps.p.callback = 0x18e6;
                        ps.p.colour = 0x10;
                        ps.spawn_seq(0x6a00, 1, 0xf2, py, px, 0);
                        let m = &mut ps.last_mut().m;
                        m.set(0x1c, 0x300);
                        m.set(0x1e, 0x300);
                        ps.p.colour = 0;
                        ps.p.callback = 0;
                    }
                    ps.spawn_seq(0x5129, 4, 6, y, x, 1);
                }
            }
            22 | 48 => {
                // FUN_1040_295e / 4ed6: a drop of blood or a bone: slides down walls, splashes
                // on the floor (bones may stay: lying objects aren't kept here).
                let i = self.particles.current;
                {
                    let m = &mut self.particles.list[i].m;
                    m.set(8, m.w(0x10));
                    m.set(0xa, m.w(0x12));
                }
                self.particle_callback(i, f);
                let side = self.particles.hit_side;
                let p = &mut self.particles.list[i];
                if side < 2 {
                    p.m.set(0x24, 0);
                    p.m.set(0x3e, 0);
                    p.m.set(4, p.m.w(0x10));
                    p.m.set(6, p.m.w(0x12));
                    p.v.set_l(0, 0);
                    p.v.set_l(4, 0);
                    p.v.set(8, 1);
                    self.particles.keep_dying = false;
                } else if side != 2 {
                    let base = p.m.w(0x14) & 0xff00;
                    let (hy, hx) = (p.m.w(0x12), p.m.w(0x10));
                    if id == 22 {
                        self.particles.spawn_seq(base + 10, 4, 4, hy, hx, 1);
                    } else {
                        self.particles.spawn_seq(base + 0x19, 4, 5, hy, hx, 1);
                        if f.rng.random(2) == 0 && (base == 0x5400 || base == 0x5300) {
                            let sprite = base + f.rng.random(3) + 0x29;
                            let draw = f.rng.random(2);
                            let (py, px) = (self.particles.list[self.particles.current].m.w(0x12), self.particles.list[self.particles.current].m.w(0x10));
                            self.lay_decal(f.world, crate::decals::DROP, draw, sprite, py, px);
                        }
                    }
                }
            }
            27 => {
                // FUN_1040_2f98: a burning bit bounces (effect 16); landing slowly (under 2.67 px
                // a frame, not on a ceiling) it flares (FUN_1040_6120) and lies smoking
                // (FUN_1040_637a).
                let v = self.bounce_off(f);
                let (hi, lo) = (v >> 16, v as u32 & 0xffff);
                if hi >= 0 && (hi < 2 || (hi < 3 && lo < 0xaaab)) && self.particles.hit_side != 2 {
                    let ps = &mut self.particles;
                    ps.spawn_seq(0x6800, 2, 0xf, y, x, 1);
                    let p = f.rng.random(0x14);
                    sound::play_at_pitch(0x4b, p + 0xf6, y, x);
                    f.fx.shakes.push((0x12, 5));
                    ps.p.callback = 0x18e6;
                    ps.p.colour = 0x10;
                    ps.spawn_seq(0x6a00, 1, 0x17e, y, x, 0);
                    let m = &mut ps.last_mut().m;
                    m.set(0x1c, 0x200);
                    m.set(0x1e, 0x200);
                    ps.p.colour = 0;
                    ps.p.callback = 0;
                }
            }
            65 => {
                // FUN_1040_105d: a heavy piece bounces (effect 16); landing slowly it thuds
                // (dust, sound 0x4b at pitch 0xf6 + Random(0x14)); hitting a ceiling it sheds
                // two bits.
                let v = self.bounce_off(f);
                let (hi, lo) = (v >> 16, v as u32 & 0xffff);
                let side = self.particles.hit_side;
                let ps = &mut self.particles;
                if hi >= 0 && (hi < 2 || (hi < 3 && lo < 0xaaab)) && side != 2 {
                    ps.spawn_seq(0x6f00, 2, 0x18, y, x, 1);
                    let r = f.rng.random(0x100);
                    let m = &mut ps.last_mut().m; // FUN_1050_4a29(r + 0x240, r + 0x200)
                    m.set(0x1c, r + 0x200);
                    m.set(0x1e, r + 0x240);
                    m.set(0x42, 0x2148);
                    let p = f.rng.random(0x14);
                    sound::play_at_pitch(0x4b, p + 0xf6, y, x);
                    f.fx.shakes.push((0x12, 5));
                } else if side == 3 {
                    ps.spawn_seq(0x1918, 3, 3, y, x, 1);
                    ps.throw(0xe, 0x30, 0x3c0);
                    ps.spawn_seq(0x1918, 4, 3, y, x, 1);
                    ps.throw(0xe, 0x20, 0x780);
                }
            }
            47 => {
                // FUN_1040_4d4c: the tank's grenade launcher lands and breaks up: plates, bits,
                // a smoke sprite and a fire flash (effect 23). Leaves 0932 = 0x8000 set.
                let g = self.particles.list[self.particles.current].m.w(0);
                let ps = &mut self.particles;
                ps.p.group = g;
                ps.p.hits = 0x8000;
                ps.p.callback = 0x17f4;
                for i in 0..4 {
                    ps.p.on_death = if f.rng.random(4) == 0 { 0x1d } else { 0x11 };
                    let px = x - 0xf + i * 10;
                    let py = (y - 4) - f.rng.random(0x18);
                    let s = f.rng.random(4);
                    ps.spawn(0x592b, py, px);
                    ps.last_mut().m.set(0x26, s + 2);
                    let a = (i * -0x1e0 + 0x8c0) - f.rng.random(0xa0);
                    let sp = f.rng.random(0x11);
                    ps.throw(9, sp + 0x40, a);
                }
                ps.p.callback = 0;
                ps.p.on_death = 0x10;
                for i in 0..5 {
                    let px = x - 0x10 + i * 8;
                    let py = (y - 4) - f.rng.random(0x18);
                    let s = f.rng.random(3);
                    ps.spawn(s + 0x5928, py, px);
                    let a = (i * -0xa0 + 0x730) - f.rng.random(0xa0);
                    let sp = f.rng.random(0x11);
                    ps.throw(9, sp + 0x50, a);
                }
                ps.p.on_death = 0;
                ps.p.hits = 0x8000;
                ps.p.group = 1;
                crate::alien::smoke_sprite(ps, y, x);
                effects::effect(self, f.world, 23, y - 0x10, x, f.rng, f.fx);
                f.fx.shakes.push((0x20, 0xc));
            }
            15 => {
                // FUN_1040_0a80: a metallic ping and sparks.
                let p = f.rng.random(0x1e);
                sound::play_at_pitch(0x45, p + 0xf6, y, x);
                self.sparks(0x191b, y, x);
            }
            _ => effects::effect(self, f.world, id as u16, y, x, f.rng, f.fx),
        }
    }

    /// Effect 16, FUN_1040_0ab0: bounce off the wall that was hit (FUN_1050_44da's throw):
    /// sideways reverses, up or down loses speed by the bounce factor (/16) and stops when too
    /// slow. Returns the new vertical speed (1170:25a6).
    fn bounce_off(&mut self, f: &mut Frame) -> i32 {
        let i = self.particles.current;
        {
            let m = &mut self.particles.list[i].m;
            m.set(8, m.w(0x10));
            m.set(0xa, m.w(0x12));
        }
        self.particle_callback(i, f); // FUN_1050_4acb
        let side = self.particles.hit_side;
        let p = &mut self.particles.list[i];
        let (m, v) = (&mut p.m, &mut p.v);
        if side <= 1 {
            m.set(0x24, 0);
            m.set(0x3e, 0);
            m.set(4, m.w(0x10) - (m.w(4) - m.w(0x10)));
            v.set_l(0, v.l(0).wrapping_neg());
            self.particles.keep_dying = false;
            return -0x7fff;
        }
        let t = v.w(8);
        let t1 = t - 1;
        let fall = (t.wrapping_mul(t) - t1.wrapping_mul(t1)) / v.w(0xa);
        let vy = fall.wrapping_mul(0x40000).wrapping_sub(v.l(4)).wrapping_mul(v.w(0xc)) / 0x10;
        v.set_l(4, vy);
        if side == 2 || (vy >> 16) > 2 || ((vy >> 16) == 2 && (vy as u32 & 0xffff) > 0xaaaa) {
            m.set(0x24, 0);
            m.set(0x3e, 0);
            v.set(8, 1);
            if f.world.solidity_at(m.w(0xe), m.w(8)) & 0x40 == 0 {
                m.set(0xc, m.w(8));
            } else {
                m.set(8, m.w(0xc));
            }
            m.set(4, m.w(0xc));
            m.set(6, m.w(0xa));
            self.particles.keep_dying = false;
        }
        vy
    }

    /// Effect 44, FUN_1040_2046: weapon 7's shell bounces; a slow impact or its fuse sets it off.
    fn shell7_hit(&mut self, f: &mut Frame) {
        let i = self.particles.current;
        let side = self.particles.hit_side;
        let p = &mut self.particles.list[i];
        let (m, v) = (&mut p.m, &mut p.v);
        if m.w(0x3e) == -10 {
            return;
        }
        if m.w(0x24) < m.w(0x22) {
            m.set(8, m.w(0x10));
            m.set(0xa, m.w(0x12));
            if side < 2 {
                if v.l(4) == 0 {
                    self.shell7_blast(f);
                    return;
                }
                m.set(0x3e, 0);
                m.set(4, m.w(0x10) - (m.w(4) - m.w(0x10)));
                v.set_l(0, v.l(0).wrapping_neg());
                sound::play_at(0x6d, m.w(0x12), m.w(0x10));
            } else {
                if v.l(4) == 0 && v.w(8) < 5 {
                    v.set_l(0, v.l(0) / 2);
                } else {
                    let t = v.w(8);
                    let fall = (t * t - (t - 1) * (t - 1)) / v.w(0xa);
                    let vy = fall.wrapping_mul(0x40000).wrapping_sub(v.l(4)).wrapping_mul(v.w(0xc)) / 0x10;
                    v.set_l(4, vy);
                    if side != 2 && (vy >> 16) < 0xc {
                        self.shell7_blast(f);
                        return;
                    }
                }
                m.set(0x3e, 0);
                v.set(8, 1);
                if f.world.solidity_at(m.w(0xe), m.w(8)) & 0x40 == 0 {
                    m.set(0xc, m.w(8));
                } else {
                    m.set(8, m.w(0xc));
                }
                m.set(4, m.w(0xc));
                m.set(6, m.w(0xa));
            }
            m.set(0x24, m.w(0x24) + 1);
            self.particles.keep_dying = false;
            return;
        }
        if m.w(0x3e) == -1 {
            m.set(8, m.w(0x10));
            m.set(0xa, m.w(0x12));
        }
        self.shell7_blast(f);
    }

    /// Effect 56, FUN_1040_22c1: weapon 9's ball bounces off walls (back the way it came) and
    /// floors (losing speed, rolling on at 4 px a frame); at the end of its life it gets 8 more
    /// frames, then goes off.
    fn ball9_hit(&mut self, f: &mut Frame) {
        let i = self.particles.current;
        let side = self.particles.hit_side;
        let p = &mut self.particles.list[i];
        let (m, v) = (&mut p.m, &mut p.v);
        if m.w(0x3e) == -10 {
            return;
        }
        if m.w(0x24) < m.w(0x22) {
            m.set(8, m.w(0x10));
            m.set(0xa, m.w(0x12));
            m.set(0x16, ((m.uw(0x24) >> 3) & 3) as i32 + 0x1900);
            if side < 2 {
                m.set(0x3e, 0);
                m.set(4, m.w(0x10) - (m.w(4) - m.w(0x10)));
                v.set_l(0, v.l(0).wrapping_neg());
                m.set(0x18, m.w(0x18) ^ 1);
            } else {
                if v.l(4) == 0 && v.w(8) < 5 {
                    v.set_l(0, if v.l(0) < 0 { -0x40000 } else { 0x40000 });
                } else {
                    let t = v.w(8);
                    let t1 = t - 1;
                    let fall = (t.wrapping_mul(t) - t1.wrapping_mul(t1)) / v.w(0xa);
                    let vy = fall.wrapping_mul(0x40000).wrapping_sub(v.l(4)).wrapping_mul(v.w(0xc)) / 0x10;
                    v.set_l(4, if side == 2 || vy >= 0x20000 { vy } else { 0 });
                    // The clonk on a floor: only a hard enough landing (or a ceiling).
                    if side == 2 || vy >= 0x20000 {
                        sound::play_at(0x44, m.w(0x12), m.w(0x10));
                    }
                }
                m.set(0x3e, 0);
                v.set(8, 1);
                if f.world.solidity_at(m.w(0xe), m.w(8)) & 0x40 == 0 {
                    m.set(0xc, m.w(8));
                } else {
                    m.set(8, m.w(0xc));
                }
                m.set(4, m.w(0xc));
                m.set(6, m.w(0xa));
            }
            m.set(0x24, m.w(0x24) + 1);
            self.particles.keep_dying = false;
            return;
        }
        if m.w(0x3e) == -1 {
            m.set(8, m.w(0x10));
            m.set(0xa, m.w(0x12));
        }
        if m.w(0x22) == 0x1e0 {
            m.set(0x34, 0x8000);
            m.set(0x22, 0x1e8);
            sound::play_at(0x3c, m.w(0xa), m.w(8));
            self.particles.keep_dying = false;
            return;
        }
        let (y, x) = (m.w(0xa), m.w(8));
        let ps = &mut self.particles;
        ps.spawn_seq(0x6e00, 3, 0x13, y, x, 1); // FUN_1040_690b
        let r = f.rng.random(0x40) + 300;
        let m = &mut ps.last_mut().m; // FUN_1050_4a29
        m.set(0x1c, r * 6 / 5);
        m.set(0x1e, r);
        m.set(0x42, 0x2148);
        sound::play_at(0x47, y, x);
        f.fx.shakes.push((0x26, 0xe));
        self.explode_area(f.world, 0xc0, 4, y + 0xe, x + 0x2e, y - 0x3e, x - 0x2e, f.rng, f.fx);
        let ps = &mut self.particles;
        ps.blast(2, 0xc0, 5, 0x1e, 0x3e, -0x4e, -0x3e, y, x);
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x10;
        ps.spawn(-0xb, y - 1, x);
        let a = f.rng.random(0xc0) + 0x5c0;
        ps.throw(9, 0x80, a);
        ps.spawn(-0xb, y - 1, x);
        let a = 0x580 - f.rng.random(0xc0);
        ps.throw(9, 0x80, a);
        if f.rng.random(2) == 0 {
            ps.spawn(-0xb, y - 1, x);
            let a = f.rng.random(0xd0) + 0x540;
            ps.throw(9, 0x80, a);
        }
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
        let n = f.rng.random(4) + 1;
        crate::hero::spawn_dust_pub(f.world, n, y, x, f.rng);
    }

    /// FUN_1040_1ec6: the shell goes off: a flash, four bits of casing, shake and dust.
    fn shell7_blast(&mut self, f: &mut Frame) {
        let i = self.particles.current;
        let (mut y, x) = (self.particles.list[i].m.w(0xa), self.particles.list[i].m.w(8));
        if (y & !15) != 0xf && f.world.solidity_at(y + 0x10, x) & 1 != 0 {
            y = (y & !15) + 0xf;
            self.particles.list[i].m.set(0xa, y);
        }
        let ps = &mut self.particles;
        ps.p.delay = 1;
        ps.spawn_seq(0x6f00, 3, 0x18, y, x, 1);
        ps.p.delay = 0;
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x10;
        ps.p.callback = 0x177a;
        for k in 0..4 {
            let steps = f.rng.random(4) + 2;
            ps.spawn(0x1824, y - 1, x);
            ps.last_mut().m.set(0x26, steps);
            let r = f.rng.random(0x61);
            ps.throw(9, 0x70, k * 0x60 + 0x4e0 + r);
        }
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
        ps.p.callback = 0x14bd;
        ps.spawn_seq(0x7fff, 1, 0x2a, y, x, 0);
        {
            // FUN_1050_4a51: two fires to crawl away from where it burst.
            let p = ps.last_mut();
            let (sx, sy) = (p.m.w(4), p.m.w(6));
            for k in 0..2 {
                p.v.set(6 * k as usize, sx);
                p.v.set(6 * k as usize + 2, sy);
                p.v.set(6 * k as usize + 4, k);
            }
        }
        ps.p.callback = 0;
        sound::play_at(0x26, y, x);
        f.fx.shakes.push((0x2a, 0x12));
        let n = f.rng.random(7) + 2;
        crate::hero::spawn_dust_pub(f.world, n, y, x, f.rng);
    }

    /// Effect 78, FUN_1040_0cf8: a spark of the charge's blast bounces (and then only hits walls).
    fn spark_bounce(&mut self, f: &mut Frame) {
        let i = self.particles.current;
        {
            let m = &mut self.particles.list[i].m;
            m.set(8, m.w(0x10));
            m.set(0xa, m.w(0x12));
        }
        self.particle_callback(i, f); // FUN_1050_4acb
        let side = self.particles.hit_side;
        let p = &mut self.particles.list[i];
        let (m, v) = (&mut p.m, &mut p.v);
        if side < 2 {
            m.set(0x24, 0);
            m.set(0x3e, 0);
            m.set(0x34, -0x8000);
            let a = ((m.w(4) - m.w(0x10)).wrapping_mul(v.w(0xc)) as i16 as i32) / 0x10;
            m.set(4, m.w(0x10) - a);
            let nv = (v.l(0).wrapping_neg() as i64 * v.w(0xc) as i64 / 0x10) as i32;
            if std::env::var("ET_SPARK").is_ok() {
                eprintln!("SPARK now {} vx {:x} nv {:x} bounce {}", self.particles.now, v.l(0), nv, v.w(0xc));
            }
            v.set_l(0, nv);
            if (nv as i64).abs() >> 16 >= 12 && f.rng.random(10) == 0 {
                let p = f.rng.random(0x40); // the sound's pitch
                sound::discard(0x7c);
                sound::play_at_pitch(0x7c, 0x100 - p, m.w(0xa), m.w(8));
            }
            self.particles.keep_dying = false;
            return;
        }
        let t = v.w(8);
        let t1 = t - 1;
        let fall = (t.wrapping_mul(t) - t1.wrapping_mul(t1)) / v.w(0xa);
        let vy = fall.wrapping_mul(0x40000).wrapping_sub(v.l(4)).wrapping_mul(v.w(0xc)) / 0x10;
        v.set_l(4, vy);
        let hi = vy >> 16;
        if hi >= 2 {
            if hi < 12 || f.rng.random(10) != 0 {
                if f.rng.random(4) == 0 {
                    // A clink 1 time in 4, louder the faster it falls.
                    sound::discard(0x44);
                    sound::set_volume((((vy - 0x20000) >> 13) + 0x80).min(0x100));
                    sound::play_at(0x44, m.w(0xa), m.w(8));
                    sound::set_volume(0x100);
                }
            } else {
                let p = f.rng.random(0x40); // the sound's pitch
                sound::discard(0x7c);
                sound::play_at_pitch(0x7c, 0x100 - p, m.w(0xa), m.w(8));
            }
        }
        if side == 2 || hi > 2 || (hi == 2 && (vy as u32 & 0xffff) > 0xaaaa) {
            m.set(0x24, 0);
            m.set(0x3e, 0);
            v.set(8, 1);
            m.set(0x34, -0x8000);
            if f.world.solidity_at(m.w(0xe), m.w(8)) & 0x40 == 0 {
                m.set(0xc, m.w(8));
            } else {
                m.set(8, m.w(0xc));
            }
            m.set(4, m.w(0xc));
            m.set(6, m.w(0xa));
            self.particles.keep_dying = false;
        }
    }

    /// FUN_1050_36a9: a critter runs: a throw that lands (running on along slopes, hopping up
    /// single steps) until a shot kills it (after 0x12 frames) or it leaves the screen.
    fn move_critter(&mut self, i: usize, cam: (i32, i32), w: &World, rng: &mut PascalRandom) {
        {
            let p = &mut self.particles.list[i];
            let (m, v) = (&mut p.m, &mut p.v);
            let t = v.w(8);
            m.set(8, m.w(4) + v.l(0).wrapping_mul(t) / 0x40000);
            let fall = (t.wrapping_mul(t) as i16 as i32) / v.w(0xa);
            m.set(0xa, fall + (m.w(6) - v.l(4).wrapping_mul(t) / 0x40000));
            v.set(8, t + 1);
        }
        if self.particles.list[i].m.w(0x24) > 0x12 {
            let m = &self.particles.list[i].m;
            let (x, y, s, flip) = (m.w(8), m.w(0xa), m.w(0x16), m.w(0x18) != 0);
            self.particles.p.aim_level = -1;
            self.particles.p.aim_level2 = 0;
            let d = match crate::sprites::hit_rect(s, y, x, flip) {
                Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x105b),
                None => 0,
            };
            if d > 0 {
                // (1170:093c/093e stay at -1/0 until something resets them.)
                critter_gibs(&mut self.particles, rng, y - 4, x);
                let m = &mut self.particles.list[i].m;
                m.set(0x24, m.w(0x22));
                m.set(0x3e, -10);
                return;
            }
            self.particles.p.aim_level = 0x100;
            self.particles.p.aim_level2 = 0x100;
        }
        let p = &mut self.particles.list[i];
        let (m, v) = (&mut p.m, &mut p.v);
        let (dx, dy) = ((m.w(8) - cam.0) as i16, (m.w(0xa) - cam.1) as i16);
        if dx < -0x100 || dy < -0x200 || dx > 0x23f || dy > 0x1c7 {
            m.set(0x24, m.w(0x22));
            m.set(0x3e, -10);
            return;
        }
        m.set(0x24, m.w(0x24) + 1);
        if m.w(0x4e) != 0x7fff {
            eprintln!("warning: particle splashes (FUN_1050_2bcf) not ported");
        }
        let (x, y) = (m.w(8), m.w(0xa));
        let mut steep = false;
        if w.solidity_at(y, x) & 0x20 != 0 {
            let cell = w.cell_at(crate::world::MAIN_PLANE, y, x).unwrap_or(0);
            let kind = w.cell_function(cell, 2) as usize & 0xff7f;
            let row = &crate::world::SLOPES[kind * 0x16..];
            steep = i16::from_le_bytes([row[0x14], row[0x15]]) & 4 == 0;
            let ground = (y & 0xfff0) + crate::actors::slope_height(w, x, cell) - 1;
            if ground < y + 1 {
                if v.l(4) == 0 {
                    v.set_l(0, if v.w(2) < 0 { -0x40000 } else { 0x40000 });
                } else {
                    let t1 = v.w(8);
                    let t0 = t1 - 1;
                    let fall = (t1 * t1 - t0 * t0) / v.w(0xa);
                    let vy = fall.wrapping_mul(0x40000).wrapping_sub(v.l(4)).wrapping_mul(v.w(0xc)) / 0x10;
                    v.set_l(4, if vy < 0x20000 { 0 } else { vy });
                }
                v.set(8, 1);
                m.set(4, m.w(8));
                m.set(6, ground);
                m.set(0xa, ground);
                m.set(0xe, ground);
            }
        }
        let (x, y) = (m.w(8), m.w(0xa));
        if w.tile_function_at(y + 1, x) == 11000 {
            let cell = w.cell_at(crate::world::MAIN_PLANE, y + 1, x).unwrap_or(0);
            m.set(4, m.w(4) + w.cell_function(cell, 3) as i16 as i32);
        }
        if v.w(8) < 5 {
            m.set(0x2c, x);
            m.set(0x2e, y - 0x14);
            m.set(0x30, x);
            m.set(0x32, y);
            if v.l(4) == 0 && !steep {
                let lx = if v.w(2) < 0 { x - 0x10 } else { x + 0x10 };
                if w.solidity_at(y - 1, lx) & 2 != 0 && w.solidity_at(y - 0x11, lx) & 2 == 0 {
                    if w.solidity_at(y, x) & 2 != 0 {
                        m.set(0xa, y - 1);
                    }
                    m.set(4, m.w(8));
                    m.set(6, m.w(0xa));
                    v.set(8, 1);
                    v.set_l(4, 0xe0000);
                }
            }
        }
    }

    /// Effect 94, FUN_1040_26fd: a critter meets a wall (turns, unless it hit low) or the floor.
    fn critter_hits(&mut self, f: &mut Frame) {
        let i = self.particles.current;
        let side = self.particles.hit_side;
        let p = &mut self.particles.list[i];
        let (m, v) = (&mut p.m, &mut p.v);
        if m.w(0x3e) == -10 || m.w(0x24) >= m.w(0x22) {
            return;
        }
        m.set(0xa, m.w(0x12));
        m.set(0x16, (m.w(0x24) / 6) % 6 + 0x5215);
        if side < 2 {
            if m.w(6) <= m.w(0xa) || v.l(4) == 0 {
                return;
            }
            m.set(4, m.w(0x10) - (m.w(4) - m.w(0x10)));
            v.set_l(0, v.l(0).wrapping_neg());
            m.set(0x18, m.w(0x18) ^ 1);
            m.set(0x3e, 0);
        } else {
            m.set(8, m.w(0x10));
            if v.l(4) == 0 && v.w(8) < 5 {
                v.set_l(0, if v.w(2) < 0 { -0x80000 } else { 0x80000 });
            } else {
                let t = v.w(8);
                let fall = (t * t - (t - 1) * (t - 1)) / v.w(0xa);
                let vy = fall.wrapping_mul(0x40000).wrapping_sub(v.l(4)).wrapping_mul(v.w(0xc)) / 0x10;
                v.set_l(4, if side != 2 && vy < 0x20000 { 0 } else { vy });
            }
            m.set(0x3e, 0);
            v.set(8, 1);
            if f.world.solidity_at(m.w(0xe), m.w(8)) & 0x40 == 0 {
                m.set(0xc, m.w(8));
            } else {
                m.set(8, m.w(0xc));
            }
            m.set(4, m.w(0xc));
            m.set(6, m.w(0xa));
        }
        m.set(0x24, m.w(0x24) + 1);
        self.particles.keep_dying = false;
    }

    /// Effect 21, FUN_1040_1481: a grenade bounces off what it hit (walls reverse it; floors and
    /// ceilings lose speed, and it skids to a halt) until its fuse (0x22) runs out, then explodes.
    fn grenade_hit(&mut self, f: &mut Frame, mine: bool) {
        let i = self.particles.current;
        let side = self.particles.hit_side;
        let p = &mut self.particles.list[i];
        let (m, v) = (&mut p.m, &mut p.v);
        if m.w(0x3e) == -10 {
            return;
        }
        if m.w(0x24) < m.w(0x22) {
            m.set(8, m.w(0x10));
            m.set(0xa, m.w(0x12));
            if side < 2 {
                m.set(0x3e, 0);
                m.set(4, m.w(0x10) - (m.w(4) - m.w(0x10)));
                v.set_l(0, v.l(0).wrapping_neg());
                sound::play_at(0x6d, m.w(0x12), m.w(0x10));
            } else {
                if v.l(4) == 0 && v.w(8) < 5 {
                    v.set_l(0, v.l(0) / 2);
                } else {
                    let t = v.w(8);
                    let fall = (t * t - (t - 1) * (t - 1)) / v.w(0xa);
                    let vy = fall.wrapping_mul(0x40000).wrapping_sub(v.l(4)).wrapping_mul(v.w(0xc)) / 0x10;
                    v.set_l(4, if side == 2 || vy >= 0x20000 { vy } else { 0 });
                    // The clonk on a floor: only a hard enough landing (or a ceiling).
                    if side == 2 || vy >= 0x20000 {
                        sound::play_at(0x6d, m.w(0x12), m.w(0x10));
                    }
                }
                m.set(0x3e, 0);
                v.set(8, 1);
                if f.world.solidity_at(m.w(0xe), m.w(8)) & 0x40 == 0 {
                    m.set(0xc, m.w(8));
                } else {
                    m.set(8, m.w(0xc));
                }
                m.set(4, m.w(0xc));
                m.set(6, m.w(0xa));
            }
            m.set(0x24, m.w(0x24) + 1);
            self.particles.keep_dying = false;
            return;
        }
        if m.w(0x3e) == -1 {
            m.set(8, m.w(0x10));
            m.set(0xa, m.w(0x12));
        }
        let (y, x, damage, big) = (m.w(0xa), m.w(8), m.w(0x38), m.w(0x14) == 0x1818);
        if mine {
            self.mine_blast(y, x, f);
            return;
        }
        let ps = &mut self.particles;
        crate::alien::smoke_sprite(ps, y, x);
        ps.spawn_seq(0x6400, 2, 0x24, y - 0xc, x, 1);
        ps.line(8, 0x5a0);
        sound::play_at(0x3b, y, x);
        f.fx.shakes.push((0x2a, 0x10));
        self.explode_area(f.world, damage, 4, y + 0xe, x + 0x2e, y - 0x3e, x - 0x2e, f.rng, f.fx);
        let ps = &mut self.particles;
        ps.blast(2, damage, if big { 7 } else { 5 }, 0x1e, 0x3e, -0x4e, -0x3e, y, x);
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x10;
        ps.spawn(-0xb, y - 1, x);
        let a = f.rng.random(0xc0) + 0x5c0;
        ps.throw(9, 0x80, a);
        ps.spawn(-0xb, y - 1, x);
        let a = 0x580 - f.rng.random(0xc0);
        ps.throw(9, 0x80, a);
        if f.rng.random(2) == 0 {
            ps.spawn(-0xb, y - 1, x);
            let a = f.rng.random(0xd0) + 0x540;
            ps.throw(9, 0x80, a);
        }
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
        let n = f.rng.random(6) + 1;
        crate::hero::spawn_dust_pub(f.world, n, y, x, f.rng);
    }

    /// The explosion half of effect 77, FUN_1040_188c: a fireball, a big blast, three chunks and
    /// 48 sparks flying.
    fn mine_blast(&mut self, y: i32, x: i32, f: &mut Frame) {
        let ps = &mut self.particles;
        ps.p.colour = 0x10;
        ps.spawn_seq(0x6e00, 3, 0x13, y, x, 1); // FUN_1040_690b
        let r = f.rng.random(0x20);
        let m = &mut ps.last_mut().m; // FUN_1050_4a29
        m.set(0x1c, r + 500);
        m.set(0x1e, r + 400);
        m.set(0x42, 0x2148);
        let p = f.rng.random(0x14);
        sound::play_at_pitch(100, 0xd8 - p, y, x);
        f.fx.shakes.push((0x20, 0xe));
        ps.p.colour = 0;
        self.explode_area(f.world, 0xc0, 4, y + 6, x + 0xe, y - 0x1e, x - 0xe, f.rng, f.fx);
        let ps = &mut self.particles;
        ps.blast(2, 0xc0, 5, 0xe, 0x16, -0x26, -0x16, y, x);
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x10;
        ps.p.callback = 0x177a;
        for (i, (dy, base)) in [(-1, 0x5c0), (-1, 0x580), (-2, 0x540)].into_iter().enumerate() {
            if i == 2 && f.rng.random(2) != 0 {
                break;
            }
            ps.spawn(0x1824, y + dy, x);
            let steps = f.rng.random(4) + 2;
            ps.last_mut().m.set(0x26, steps);
            let a = match i {
                0 => f.rng.random(0xc0) + base,
                1 => base - f.rng.random(0xc0),
                _ => f.rng.random(0xd0) + base,
            };
            ps.throw(9, 0x80, a);
        }
        ps.p.hits = 1;
        ps.p.damage = 0x20;
        ps.p.flags = 0x94;
        ps.p.on_death = 0x4e;
        ps.p.callback = 0x229;
        for i in 0..0x30 {
            ps.p.delay = f.rng.random(3);
            let px = x - 2 + f.rng.random(5);
            let steps = f.rng.random(5) + 1;
            ps.spawn(0x1965, y - 3, px);
            ps.last_mut().m.set(0x26, steps);
            let a = (i * 0x3b + 0x1670 + f.rng.random(0x61)) % 0x1680;
            let speed = f.rng.random(0x11) + 0xa0;
            ps.throw(9, speed, a);
        }
        ps.p.delay = 0;
        ps.p.callback = 0;
        ps.p.flags = 0x84;
        ps.p.damage = 0x10;
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
        let n = f.rng.random(5) + 1;
        crate::hero::spawn_dust_pub(f.world, n, y, x, f.rng);
    }

    /// Effect 73, FUN_1040_3fc4(y, x): a tile bursts (FUN_1040_3eb4: five chips thrown up,
    /// some sound), and two times in three a spark follows later.
    pub fn tile_burst(&mut self, y: i32, x: i32, rng: &mut PascalRandom, fx: &mut Effects) {
        let ps = &mut self.particles;
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x10;
        ps.p.callback = 0x177a;
        ps.p.colour = 0x10;
        for i in 0..=4 {
            ps.p.delay = rng.random(4);
            let px = x - 6 + i * 3;
            let py = y + 1 + rng.random(4);
            let steps = rng.random(2) + 3;
            ps.spawn(0x7500, py, px); // FUN_1050_4106: with its own frame step
            ps.last_mut().m.set(0x26, steps);
            ps.throw(7, 0x20, i * 0x80 + 0xfe0);
        }
        ps.p.colour = 0;
        ps.p.callback = 0;
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
        ps.p.delay = 0;
        let p = rng.random(0x1f);
        sound::play_at_pitch(0x51, p + 0xf1, y + 1, x + 0x10);
        fx.shakes.push((0xc, 5));
        if rng.random(3) != 0 {
            ps.p.delay = rng.random(0x28) + 0x28;
            // FUN_1040_3440: a spark (group 0, a sound when it appears).
            ps.p.group = 0;
            ps.p.on_spawn = 0x7576;
            ps.spawn_seq(0x1928, 3, 4, y + 2, x, 1);
            ps.p.on_spawn = 0;
            ps.p.group = 1;
            ps.p.delay = 0;
        }
    }

    /// Effect 14, FUN_1040_09d4(y, x): a rocket explodes.
    fn rocket_blast(&mut self, y: i32, x: i32, f: &mut Frame) {
        let m = &self.particles.list[self.particles.current].m;
        if m.w(0x3e) != -10 {
            crate::sound::play_at(0x49, m.w(0xa), m.w(8));
        }
        f.fx.shakes.push((0x12, 5));
        if DETAIL > 3 {
            let ps = &mut self.particles;
            ps.p.colour = 0x28;
            ps.p.group = 0;
            ps.spawn_seq(0x1b04, 4, 2, y, x, 1);
            ps.p.colour = 0;
            ps.p.group = 1;
        }
        self.particles.spawn_seq(0x6900, 2, 0x19, y, x, 1);
        let r = f.rng.random(0x20);
        let m = &mut self.particles.last_mut().m;
        m.set(0x1c, r + 0x133);
        m.set(0x1e, r + 0x100);
        m.set(0x42, 0x2148);
        self.explode_area(f.world, 0x60, 4, y + 0x20, x + 0x20, y - 0x20, x - 0x20, f.rng, f.fx);
        let n = f.rng.random(4);
        crate::hero::spawn_dust_pub(f.world, n, y - 7, x, f.rng);
    }

    /// FUN_1050_5b61(strength, mask, bottom, right, top, left): blow up every explodable tile
    /// in the rectangle (from the bottom right).
    #[allow(clippy::too_many_arguments)]
    pub fn explode_area(
        &mut self,
        world: &mut World,
        strength: i32,
        mask: u16,
        bottom: i32,
        right: i32,
        top: i32,
        left: i32,
        rng: &mut PascalRandom,
        fx: &mut Effects,
    ) {
        if std::env::var("ET_AREA").is_ok() {
            let mut ty = bottom;
            while (top & !15) <= ty {
                let mut tx = right;
                while (left & !15) <= tx {
                    eprintln!("AREA {} tile ({tx},{ty}) solid {:x} func {} cell {:x}", self.particles.now, world.solidity_at(ty, tx), world.tile_function_at(ty, tx), world.cell_at(1, ty, tx).unwrap_or(0));
                    tx -= 0x10;
                }
                ty -= 0x10;
            }
        }
        let mut ty = bottom;
        while (top & !15) <= ty {
            let mut tx = right;
            while (left & !15) <= tx {
                if world.solidity_at(ty, tx) & 0x80 != 0 {
                    effects::explode_tile(self, world, strength, mask, ty, tx, rng, fx);
                }
                if world.tile_function_at(ty, tx) == crate::doors::DOOR {
                    self.door_hit(world, strength, ty, tx, 0, fx);
                }
                tx -= 0x10;
            }
            ty -= 0x10;
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
            let p = f.rng.random(0x40); // a ricochet
            crate::sound::play_at_pitch(0x7c, 0x100 - p, y, x);
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

/// 1170:4f0c > 10 (FUN_1050_18e6): lying smoke ages. 4f0c stays 0 while a demo plays.
const SMOKE_AGES: bool = false;
/// 1170:4f0c > 10, as above.
pub const FRAME_SLACK: bool = false;
/// 1170:16d9, the detail level (3 on the demos).
const DETAIL: i32 = 3;

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

/// FUN_1040_60bb(091e, y, x) when a particle joins the array (FUN_1050_3f77, or FUN_10a0_023f for
/// a delayed one): 30000 + n plays sound n there; the procedures (< 30000) aren't used this way.
fn spawn_effect(id: i32, y: i32, x: i32) {
    let id = id as u16 as i32;
    if id >= 30000 {
        sound::play_at((id - 30000) as usize, y, x);
    } else {
        eprintln!("warning: particle spawn effect {id} not ported");
    }
}

/// FUN_1050_00a7: the frame to draw. Animations (negative sprites) aren't ported.
fn sprite_frame(m: &mut Obj) {
    let mut s = m.w(0x14);
    if m.w(2) == 1 && m.w(0x26) != 0 {
        s += m.w(0x24) / m.w(0x26);
    }
    m.set(0x16, s);
}

/// FUN_1050_4b17(particle, bottom, right, top, left): a blast's damage to a box, full at the
/// centre and none at the edge of the blast's box.
fn blast_damage(m: &Obj, bottom: i32, right: i32, top: i32, left: i32) -> i32 {
    let (px, py) = (m.w(8), m.w(0xa));
    let (x1, y1, x2, y2) = (m.w(0x2c), m.w(0x2e), m.w(0x30), m.w(0x32));
    let dist = |a: i32, b: i32| ((a as f64).powi(2) + (b as f64).powi(2)).sqrt().round_ties_even() as i32;
    let vertical = |d: i32| {
        if py < top {
            dist(((top - py) * 0x40) / (y2 - py), d)
        } else if bottom < py {
            dist(((py - bottom) * 0x40) / (py - y1), d)
        } else {
            d
        }
    };
    if std::env::var("ET_HIT").is_ok() {
        eprintln!("blast_damage p {px},{py} box {x1},{y1},{x2},{y2} target l{left} r{right} t{top} b{bottom} dmg {}", m.w(0x38));
    }
    let d = if px < left {
        vertical(((left - px) * 0x40) / (x2 - px))
    } else if right < px {
        vertical(((px - right) * 0x40) / (px - x1))
    } else if py < top {
        ((top - py) * 0x40) / (y2 - py)
    } else if bottom < py {
        ((py - bottom) * 0x40) / (py - y1)
    } else {
        0
    };
    if d < 0x40 {
        ((0x40 - d) * m.w(0x38)) / 0x40
    } else {
        0
    }
}

/// The homing part of FUN_1050_568b / 51aa: remember the nearest target (or, with flag 0x200,
/// the one most straight ahead within 0x100) of at least the current priority (1170:093c).
fn aim_at(p: &mut Particle, level: i32, dx: i32, dy: i32) {
    let v = &mut p.v;
    if v.w(0x1a) > level {
        return;
    }
    let score = if p.m.w(0x3a) & 0x200 == 0 {
        ((dx as f64).powi(2) + (dy as f64).powi(2)).sqrt().round_ties_even() as i32
    } else {
        let d = ((v.w(0x10) - trig::atan2(dy, dx)) + 0x21c0) % 0x1680 - 0xb40;
        if d.abs() >= 0x100 {
            return;
        }
        d.abs()
    };
    if score < v.w(0x18) || v.w(0x1a) < level {
        v.set(0x18, score);
        v.set(0x14, dx);
        v.set(0x16, dy);
        v.set(0x1a, level);
    }
}

/// FUN_1050_3422(y, x): a critter bursts: twelve bits of it, four bones, four drops of blood.
fn critter_gibs(ps: &mut Particles, rng: &mut PascalRandom, y: i32, x: i32) {
    sound::play_at(0x21, y, x);
    let left = |r: &mut PascalRandom, spread: i32| (r.random(0x80) + 0x960) - spread;
    let right = |r: &mut PascalRandom, spread: i32| ((0x1860 - r.random(0x80)) + spread) % 0x1680;
    ps.p.hits = 0x8000;
    ps.p.on_death = 0x16;
    ps.p.callback = 0x23b7;
    ps.p.flags = 0x8c;
    for i in 1..=6 {
        let (speed, spread) = ((i << 6) / 6 + 0x10, (i * 0x3c0) / 6);
        ps.p.delay = rng.random(8);
        let s = (rng.random(4) == 0) as i32 + 0x5307;
        ps.spawn(s, y, x);
        let a = left(rng, spread);
        ps.throw(0xb, speed, a);
        ps.p.delay = rng.random(8);
        let s = (rng.random(4) == 0) as i32 * 2 + 0x5306;
        ps.spawn(s, y, x);
        let a = right(rng, spread);
        ps.throw(0xb, speed, a);
    }
    ps.p.callback = 0;
    ps.p.flags = 0x84;
    ps.p.on_death = 0x30;
    for i in 1..=2 {
        let (speed, spread) = ((i << 5) / 2 + 0x10, (i * 0x3c0) / 2);
        for side in 0..2 {
            ps.p.delay = rng.random(8) + 8;
            let s = rng.random(5) + 0x530f;
            ps.spawn(s, y, x);
            let a = if side == 0 { left(rng, spread) } else { right(rng, spread) };
            ps.throw(0xe, speed, a);
        }
    }
    ps.p.on_death = 0;
    ps.p.hits = 0;
    for (d, dy, dx) in [(0x1e, 6, 1), (0x19, 0xc, 4), (0x14, -5, -5), (0xf, 5, 0)] {
        ps.p.delay = d;
        ps.spawn_seq(0x5300, 4, 5, y + dy, x + dx, 1); // FUN_1040_62b7
    }
    ps.p.delay = 0;
}

fn puff_at(ps: &mut Particles, y: i32, x: i32) {
    ps.spawn_seq(0x6800, 2, 0xf, y, x, 1);
}

/// FUN_1040_6405.
fn smoke4_at(ps: &mut Particles, y: i32, x: i32) {
    puff_at(ps, y, x - 4);
    ps.p.delay += 7;
    puff_at(ps, y - 5, x + 1);
    ps.p.delay += 4;
    puff_at(ps, y + 5, x - 2);
    ps.p.delay += 0x13;
    puff_at(ps, y - 1, x);
}

