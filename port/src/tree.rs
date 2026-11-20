//! Trees (record 0x1446, icons 62000/62001, FUN_1070_9420): a trunk that sways when the screen
//! shakes, takes shots in stages (table at 1170:0e98: flags, hit points, sprite) and falls when
//! it is cut through.

use crate::actors::Env;
use crate::level::Level;
use crate::objects::Obj;
use crate::sprites;
use crate::world::{World, ds_word};

pub const KIND: u16 = 0x1446;

/// 1170:0e98 + 4 * stage: (flags, hit points, sprite).
fn stage(k: i32) -> (i32, i32, i32) {
    let w = ds_word(0xe98 + 4 * k as usize);
    (w & 0xff, (w >> 8) & 0xff, ds_word(0xe9a + 4 * k as usize))
}

impl Level {
    /// EnemyFactory 0x1446 with the record [_, stage, x, y, length].
    pub fn make_tree(&mut self, world: &World, k: i32, x: i32, y: i32, len: i32) {
        let mut o = Obj::new(KIND, 0xd90, 0x22);
        o.set(0xc, 0);
        o.set(10, k);
        o.set(0xe, x);
        o.set(0x10, y);
        o.set(0x14, stage(k).1);
        o.set(0x16, 0);
        o.set(0x1a, len);
        o.set(0x1c, 0);
        o.set(0x1e, sprites::hitboxes(stage(k).2).iter().find(|b| b.kind == 3).map_or(0, |b| b.rect.1));
        o.set(0x20, if k < 5 { 1 } else { 0xef });
        o.set(2, x - len - 0x18);
        o.set(4, y);
        o.set(6, x + len + 0x18);
        o.set(8, y + len + 0x30);
        self.add_actor(world, o, y, x);
    }

    /// FUN_1070_93d0: start (or deepen) the swaying.
    fn tree_sway(&mut self, id: usize, strength: i32) {
        let p = strength.min(0xa0);
        let o = self.o(id);
        if o.w(0x1c) == 0 {
            o.set(0x18, 1);
        }
        if o.w(0x1c) < p {
            o.set(0x1c, p);
        }
        for (off, v) in [(2, -0x7fff), (4, -0x7fff), (6, 0x7fff), (8, 0x7fff)] {
            o.set(off, v);
        }
    }

    /// FUN_1070_9420.
    pub(crate) fn tree(&mut self, id: usize, env: &mut Env) {
        if self.o(id).w(0x16) > 0 {
            let o = self.o(id);
            o.set(0x16, o.w(0x16) - 1);
        }
        let (x, y, len) = (self.o(id).w(0xe), self.o(id).w(0x10), self.o(id).w(0x1a));
        let mut tip_angle = 0x10e0;
        if self.o(id).w(0x1c) >= 1 {
            let o = self.o(id);
            let sway = o.w(0x1c) - 1;
            o.set(0x1c, sway);
            let ang = (o.w(0x18) * 0x80) % 0x1680;
            tip_angle = 0x10e0 + (crate::trig::sin(ang) * ((sway >> 2) << 4)) / 0x4000;
            o.set(0x18, o.w(0x18) + 1);
            if sway == 0 && o.w(0x14) > 0 {
                o.set(2, x - len - 0x18);
                o.set(4, y);
                o.set(6, x + len + 0x18);
                o.set(8, y + len + 0x30);
            }
        }
        let tip_x = x + (crate::trig::sin((tip_angle + 0x5a0) % 0x1680) * len) / 0x4000;
        let rise = ((crate::trig::sin(tip_angle) * len) / 0x4000).max(1 - len);
        let tip_y = y - rise;
        let top = tip_y - self.o(id).w(0x1e);
        // For the draw pass: the trunk's tip and the crown's anchor.
        self.o(id).looks = vec![Obj::look(0, tip_x, tip_y), Obj::look(0, tip_x, top)];
        if env.shake_started {
            let strength = if self.o(id).w(0x14) < 1 {
                (env.shake_amp * 0x50) / 0x10
            } else {
                let r = env.rng.random(0x18);
                (env.shake_amp * 0x78) / 0x10 - 0xc + r
            };
            self.tree_sway(id, strength);
            if self.o(id).w(0x1c) == strength && env.rng.random(2) == 0 {
                let o = self.o(id);
                o.set(0x18, o.w(0x18) + 0xb4);
            }
        }
        let k = self.o(id).w(10);
        if self.o(id).w(0x14) < 1 {
            if self.particles.now & 0xf == 0 {
                let o = self.o(id);
                o.set(0x1a, o.w(0x1a) - 1);
                if o.w(0x1a) < 1 {
                    self.remove_current(id);
                    return;
                }
            }
        } else {
            self.particles.p.aim_level = -1;
            self.particles.p.aim_level2 = -1;
            let d = match sprites::hit_rect(stage(k).2, top, tip_x, false) {
                Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
                None => 0,
            };
            if d > 3 {
                let hp = self.o(id).w(0x14) - d;
                self.o(id).set(0x14, hp);
                self.tree_sway(id, (d * 0x78) / 0x10);
                let o = self.o(id);
                o.set(0x18, o.w(0x18) % 0x16);
                if self.particles.hit_direction() < 0 {
                    let o = self.o(id);
                    o.set(0x18, o.w(0x18) + 0x16);
                }
                self.o(id).set(0x16, 3);
                let k = self.o(id).w(10);
                if stage(k).0 == 0 || hp > 0x60 {
                    // (a sound)
                } else {
                    self.o(id).set(10, k + 1);
                    let ps = &mut self.particles;
                    ps.p.group = 0;
                    ps.p.hits = 0x8000;
                    ps.p.on_death = 0x10;
                    ps.p.callback = 0x177a;
                    ps.p.colour = 0x10;
                    for i in 0..4 {
                        ps.p.delay = env.rng.random(4);
                        let cx = tip_x - 3 + i * 2;
                        let dy = env.rng.random(4);
                        ps.spawn(0x7500, top + dy, cx);
                        ps.last_mut().m.set(0x26, 3);
                        ps.throw(7, 0x20, i * 0x80 + 0x1020);
                    }
                    ps.p.colour = 0;
                    ps.p.callback = 0;
                    ps.p.on_death = 0;
                    ps.p.hits = 0;
                    ps.p.group = 1;
                    ps.p.delay = 0;
                    env.rng.random(0x15); // the sound's pitch
                }
                let k = self.o(id).w(10);
                if hp < 1 && stage(k).1 > 0x70 {
                    self.particles.p.colour = 0x10;
                    crate::particles::smoke(&mut self.particles, env.rng, tip_y, tip_x);
                    self.particles.p.colour = 0;
                }
                env.fx.shakes.push((0xc, 5));
            }
            let d2 = self.particles.hit_test(tip_y - 1, x + 3, y, x - 3, 0x11fb);
            if d2 > 3 {
                if self.particles.hit_index >= 0 {
                    let hy = self.particles.list[self.particles.hit_index as usize].m.w(0x12);
                    let dist = hy - y;
                    let s = crate::trig::sin(tip_angle);
                    let mut l = if s != 0 { (-(dist * 0x4000)) / s } else { 1 };
                    if l < 1 {
                        l = 1;
                    } else if self.o(id).w(0x1a) < l {
                        l = self.o(id).w(0x1a);
                    }
                    self.o(id).set(0x1a, l);
                    env.rng.random(0x40); // the sound's pitch
                }
                self.o(id).set(0x14, 0);
            }
            self.particles.p.aim_level = 0x100;
            self.particles.p.aim_level2 = 0x100;
            if self.o(id).w(0x14) < 1 {
                let k = self.o(id).w(10);
                let k = if stage(k).0 != 0 { k + 1 } else { k };
                self.o(id).set(10, k);
                let ps = &mut self.particles;
                ps.p.group = 0;
                ps.p.hits = 0x8000;
                ps.p.on_death = 0x11;
                ps.spawn(stage(k).2, top, tip_x);
                ps.throw(7, 0x10, tip_angle);
                ps.p.on_death = 0;
                ps.p.hits = 0;
                ps.p.group = 1;
                let o = self.o(id);
                for (off, v) in [(2, -0x7fff), (4, -0x7fff), (6, 0x7fff), (8, 0x7fff)] {
                    o.set(off, v);
                }
            }
        }
    }
}

#[allow(dead_code)]
fn _unused(_: &World) {}
