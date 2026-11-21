//! The gun turret (OBJECTS 0x13ec, VMT 0xcbc, FUN_1070_2550, sprites 0x4f00..): a twin gun on a
//! spinning base that turns towards the hero while it sees him and fires both barrels on a fixed
//! cycle; tied to a lever it fires only while the lever is on. Shot to 0 hit points it smokes,
//! spins down and sheds its gun; shot to below -0x80 it breaks apart.
//!
//! | off | field |
//! |-----|-------|
//! | 14  | the firing cycle (frames) / 16 its phase: t = (phase + frame) mod cycle, fires at t = 0, 0xc |
//! | 18  | recoil / 1a hit points (0xc0) / 1c hit flash (0x80) and dead spin |
//! | 1e  | the gun's angle / 20 the angle to the hero / 22 the lever's map cell (0: none) |

use crate::actors::Env;
use crate::level::Level;
use crate::sprites;
use crate::trig::sin;

pub const KIND: u16 = 0x13ec;

impl Level {
    /// FUN_1070_2550.
    pub(crate) fn wallgun(&mut self, id: usize, env: &mut Env) {
        let h = env.h;
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let mut t = {
            let o = self.o(id);
            let cycle = o.w(0x14) as i64;
            if cycle == 0 { 0 } else { ((o.w(0x16) as i64 + env.frame as i64) % cycle) as i32 }
        };
        {
            let o = self.o(id);
            if o.w(0x1c) > 0 {
                o.set(0x1c, o.w(0x1c) - 1);
            }
        }
        let lever = self.o(id).w(0x22);
        if self.o(id).w(0x1a) < 0x41 {
            let o = self.o(id);
            if o.w(0x1a) < 1 {
                o.set(0x1e, ((o.w(0x1e) + 0x1680) - (o.w(0x1c) << 4) / 3) % 0x1680);
            } else {
                o.set(0x1e, (o.w(0x1e) + 0x15d0) % 0x1680);
                if lever != 0 && Self::lever_state(env.world, lever as u16) < 1 {
                    let o = self.o(id);
                    o.set(0x16, o.w(0x16) - (t - 0x16));
                    t = -1;
                }
            }
        } else if lever == 0 {
            if t > 0x15 {
                let hy = h.y - (h.height >> 1);
                if !self.line_of_sight(env, hy, h.x, y, x) {
                    if t > 0xc {
                        let o = self.o(id);
                        o.set(0x16, o.w(0x16) - ((t - 0x16) - (o.uw(0x14) >> 2) as i32));
                        t = -1;
                    }
                } else {
                    self.o(id).set(0x20, crate::trig::atan2(hy - y, h.x - x));
                }
                let o = self.o(id);
                let d = ((o.w(0x20) - o.w(0x1e)) + 0x21c0) % 0x1680;
                if d < 0xb40 {
                    let step = (0xb40 - d).min(0x10);
                    o.set(0x1e, ((o.w(0x1e) + 0x1680) - step) % 0x1680);
                } else {
                    let step = (d - 0xb40).min(0x10);
                    o.set(0x1e, (step + o.w(0x1e)) % 0x1680);
                }
            }
        } else if Self::lever_state(env.world, lever as u16) < 1 {
            let o = self.o(id);
            o.set(0x16, o.w(0x16) - (t - 0x16));
            t = -1;
        }
        if self.o(id).w(0x1a) > 0 {
            if t == 0 || t == 0xc {
                let dir = ((self.o(id).w(0x1e) + 0x16da) % 0x1680) / 0xb4;
                let a = (dir * 0x1680) / 0x20 + if t == 0 { 0x15c0 } else { 0x1740 };
                let bx = x + (0x14 * sin((a + 0x5a0) % 0x1680)) / 0x4000;
                let by = y - (0x14 * sin(a % 0x1680)) / 0x4000;
                let angle = self.o(id).w(0x1e);
                let ps = &mut self.particles;
                ps.p.hits = 2;
                ps.p.damage = 0x40;
                ps.p.on_death = 0x3e;
                ps.p.callback = 0x2d2;
                ps.spawn(0x191e, by, bx);
                ps.line(0x60, angle);
                ps.p.callback = 0;
                ps.p.on_death = 0;
                ps.p.damage = 0x10;
                ps.p.hits = 0;
                ps.p.colour = 0x28;
                ps.spawn_seq(0x1b04, 4, 2, by, bx, 1);
                ps.p.colour = 0;
                ps.spawn_seq(0x190a, 3, 2, by, bx, 1);
                crate::sound::play_at(0x65, by, bx);
                self.o(id).set(0x18, 10);
            }
            let o = self.o(id);
            if o.w(0x18) > 0 {
                o.set(0x18, o.w(0x18) - 1);
            }
        }
        let dead = self.o(id).w(0x1a) < 1;
        if dead {
            self.particles.p.aim_level = 0;
            self.particles.p.aim_level2 = -1;
        }
        let d = match sprites::hit_rect(0x4f22, y, x, false) {
            Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x1059),
            None => 0,
        };
        self.particles.p.aim_level = 0x100;
        self.particles.p.aim_level2 = 0x100;
        if d <= 0 {
            return;
        }
        let before = self.o(id).w(0x1a);
        self.o(id).set(0x1a, before - d);
        if self.o(id).w(0x1a) < 1 && before > 0 {
            env.fx.kills += 1;
            let ps = &mut self.particles;
            ps.p.delay = 0x5a;
            crate::boss::smoke4(ps, y - 0xb, x + 4);
            ps.p.delay = 0x32;
            crate::boss::smoke4(ps, y + 1, x + 10);
            ps.p.delay = 0x1e;
            crate::boss::smoke4(ps, y - 9, x - 10);
            ps.p.delay = 0;
            crate::effects::effect(self, env.world, 7, y, x, env.rng, env.fx);
            let ps = &mut self.particles;
            ps.p.group = 0;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x11;
            ps.p.callback = 0x17f4;
            // FUN_1050_4106: the gun's pieces, each with its own frame step.
            for (step, dx, draw, speed, base, sign, range) in
                [(4, -3, 0, 0x40, 0x640, 1, 0x140), (2, 0, 0, 0x50, 0x6e0, -1, 0x280), (3, 3, 1, 0x40, 0x500, -1, 0x140)]
            {
                ps.p.draw = draw;
                ps.spawn(0x4f44, y, x + dx);
                ps.last_mut().m.set(0x26, step);
                let r = env.rng.random(range);
                ps.throw(9, speed, base + sign * r);
            }
            ps.p.draw = 0;
            ps.p.callback = 0;
            for sign in [1, -1] {
                ps.spawn(0x4e08, y, x);
                let r = env.rng.random(0x140);
                ps.throw(10, 0x50, 0x5a0 + sign * r);
            }
            ps.p.on_death = 0;
            ps.p.hits = 0;
            ps.p.group = 1;
            env.fx.shakes.push((0x24, 0xe));
            let o = self.o(id);
            crate::sound::play_at(100, o.w(0x10), o.w(0xe));
        } else {
            env.fx.shakes.push((0xc, 4));
            let mask = self.particles.hit_mask;
            let o = self.o(id);
            if mask != 0x48 && o.w(0x1c) < 0x7e {
                // Dead already: a thud; else the hit.
                crate::sound::play_at(if o.w(0x1a) < 0 { 1 } else { 0x49 }, o.w(0x10), o.w(0xe));
            }
        }
        if self.o(id).w(0x1a) < -0x80 {
            let ps = &mut self.particles;
            crate::boss::smoke4(ps, y - 5, x);
            ps.p.delay = 0;
            let r = env.rng.random(3);
            crate::walker::plates(ps, env.rng, 0xf0, r + 2, y, x);
            ps.p.group = 0;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x41;
            ps.spawn(0x4f48, y - 5, x + 6);
            ps.throw(7, 0x30, 0x500);
            ps.p.draw = 1;
            ps.spawn(0x4f48, y - 5, x - 6);
            ps.throw(7, 0x30, 0x640);
            ps.p.draw2 = 1;
            ps.spawn(0x4f48, y + 5, x - 6);
            ps.throw(7, 0x20, 0x8c0);
            ps.p.draw = 0;
            ps.spawn(0x4f48, y + 5, x + 6);
            ps.throw(7, 0x20, 0x280);
            ps.p.draw2 = 0;
            ps.p.on_death = 0;
            ps.p.hits = 0;
            ps.p.group = 1;
            env.fx.shakes.push((0x18, 8));
            let o = self.o(id);
            crate::sound::play_at(0x4a, o.w(0x10), o.w(0xe));
            self.remove_current(id);
            return;
        }
        self.o(id).set(0x1c, 0x80);
    }
}

/// FUN_1070_2550's draw: alive, the spinning base under the gun (pushed back by the recoil);
/// dead, the gun alone, shaking while it spins down. Both lit by a hit.
pub fn draw(p: &mut crate::render::Painter, o: &crate::objects::Obj) {
    let (x, y) = (o.w(0xe), o.w(0x10));
    let dir = ((o.w(0x1e) + 0x16da) % 0x1680) / 0xb4;
    let lit = |p: &mut crate::render::Painter| {
        if o.w(0x1c) > 0x7d {
            p.mode = 0x24;
            p.colour = p.flash;
        }
    };
    if o.w(0x1a) > 0 {
        let base = ((o.w(0x1e) as i64 * 0xc0 + 0xb40) / 0x1680 % 0x20) as i32;
        p.at(0x4f22 + base, x, y);
        let (mut gx, mut gy) = (x, y);
        if o.w(0x18) > 0 {
            gx -= (o.w(0x18) * sin((o.w(0x1e) + 0x5a0) % 0x1680)) / 0x6000;
            gy += (o.w(0x18) * sin(o.w(0x1e))) / 0x6aaa;
        }
        lit(p);
        p.at(0x4f00 + dir, gx, gy);
    } else {
        lit(p);
        let mut gx = x;
        if o.w(0x1c) >= 0x32 {
            let a = ((o.w(0x1c) * 0x30) as i16 as i32 * 16) as i16 as i32 % 0x1680;
            gx += ((sin(a) * o.w(0x1c)) as i64 / 0xc8000) as i32;
        }
        p.at(0x4f22 + dir, gx, y);
    }
    p.mode = 0;
    p.colour = 0;
}
