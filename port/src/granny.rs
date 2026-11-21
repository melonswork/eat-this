//! The granny (OBJECTS 0x1482, VMT 0xe0c, FUN_1070_cb38, sprites 0x5axx = GRANNY): knits while
//! she shuffles along her range, stops to watch the hero, and when shot is knocked over (her
//! hat flies off), lies there and gets up hit points back. Only blasts (or enough fire) kill her.
//!
//! | off | field |
//! |-----|-------|
//! | 14,16 | range (equal: she only stands and watches) / 20 facing |
//! | 28  | state: 0 knitting, -2 watching, -5 knocked over, -6 lying, -0xd2 frozen |
//! | 2c  | animation counter / 2e knocked-over counter / 30 health (< 0, recovers) |
//! | 32  | knitting pause / 34 where she was hit (y offset, 0x7fff none) / 36 hurt flash |
//! | 3a  | sprite |

use crate::actors::Env;
use crate::level::Level;
use crate::sprites;

pub const KIND: u16 = 0x1482;

impl Level {
    /// FUN_1070_cb38.
    pub(crate) fn granny(&mut self, id: usize, env: &mut Env) {
        let h = env.h;
        {
            let o = self.o(id);
            if o.w(0x30) < 0 {
                o.set(0x30, o.w(0x30) + 1);
            }
        }
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        self.patrol_box(id, y + 0x10, x + 0x40, y - 0x40, x - 0x40);
        if self.actor_physics(id, env, 0x24, 0xf, 4) > 0 {
            if self.o(id).w(0x28) > -5 {
                env.fx.kills += 1;
            }
            env.fx.shakes.push((0x20, 0x10));
            let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
            self.gibs_kind(y - 0x19, x, 0x20, true, env);
            self.remove_current(id);
            return;
        }
        let state = self.o(id).w(0x28);
        match state {
            0 => {
                let o = self.o(id);
                o.set(0x3a, o.w(0x2c) / 6 + 0x5a0a);
                if o.w(0x32) < 1 {
                    o.set(0x2c, (o.w(0x2c) + 1) % 0x60);
                    if o.w(0x2c) % 0xc == 0 {
                        let dir = o.w(0x20);
                        self.patrol_step(id, env, 0xf, dir);
                        let o = self.o(id);
                        if dir != o.w(0x20) {
                            o.set(0x20, dir);
                            o.set(0x28, -2);
                            o.set(0x2c, 0);
                        }
                    }
                    let o = self.o(id);
                    if o.w(0x2c) % 0x30 == 0 {
                        o.set(0x32, 4);
                    }
                } else {
                    let o = self.o(id);
                    o.set(0x32, o.w(0x32) - 1);
                }
            }
            -2 => {
                let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
                let t = self.o(id).w(0x2c);
                if t < 0x23 {
                    let o = self.o(id);
                    o.set(0x3a, t / 7 + 0x5a47);
                    o.set(0x2c, t + 1);
                } else if h.y - 0x50 < y
                    && y < h.y + 0x50
                    && self.line_of_sight(env, h.y - (h.height >> 1), h.x, y - 0x1e, x)
                {
                    let o = self.o(id);
                    let mut d = ((h.x - x) * o.w(0x20)) as i16 as i32 / 0xc;
                    if d.abs() > 5 {
                        d = d.signum() * 5;
                    }
                    o.set(0x3a, d + 0x5a54);
                } else if self.o(id).w(0x3a) == 0x5a4b {
                    self.o(id).set(0x3a, 0x5a54);
                }
            }
            -5 => {
                let o = self.o(id);
                o.set(0x3a, (o.w(0x2e) / 2 + 0x5a30).min(0x5a3b));
                o.set(0x2e, o.w(0x2e) + 1);
                if o.w(0x2e) % 3 == 0 {
                    o.set(0xe, o.w(0xe) - o.w(0x20));
                }
                if o.w(0x2e) > 0x20 {
                    o.set(0x28, -6);
                }
                if o.w(0x2e) == 0x18 {
                    env.fx.shakes.push((8, 4)); // sound 0x25 at half volume
                }
                if o.w(0x34) != 0x7fff {
                    let (t, f) = (o.w(0x2e), o.w(0x20));
                    let (yy, xx) = (o.w(0x10) + o.w(0x34), o.w(0xe) - f * 0xc);
                    self.granny_bleeds(t, f, yy, xx, env);
                }
            }
            -6 => {
                let o = self.o(id);
                if o.w(0x36) < 1 {
                    o.set(0x3a, 0x5a3b);
                } else {
                    o.set(0x36, o.w(0x36) - 1);
                    o.set(0x3a, 0x5a3f);
                }
                let (s, x, y, flip) = (o.w(0x3a), o.w(0xe), o.w(0x10), o.w(0x20) < 0);
                let r = self.stomp(s, y, x, flip, env);
                if r > 0 {
                    if r > 1 {
                        env.fx.shakes.push((5, 2)); // sound 0xf
                    }
                    self.o(id).set(0x36, 0xc);
                }
            }
            -0xd2 => {
                // Frozen: a statue until shot, then it shatters.
                if self.frozen_statue(id, env, 0x5d02) {
                    let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
                    crate::deaths::frozen(&mut self.particles, env.rng, y - 0x12, x);
                    env.fx.shakes.push((0x20, 0xe));
                    self.remove_current(id);
                }
                return;
            }
            _ => {}
        }
        // Her own hit test (the drone leaves her alone: 093e = -1).
        let o = self.o(id);
        let (s, x, y, flip) = (o.w(0x3a), o.w(0xe), o.w(0x10), o.w(0x20) < 0);
        self.particles.p.aim_level2 = -1;
        let d = match sprites::hit_rect(s, y, x, flip) {
            Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11ff),
            None => 0,
        };
        self.particles.p.aim_level2 = 0x100;
        if d <= 0 {
            return;
        }
        if self.o(id).w(0x28) > -5 {
            env.fx.kills += 1;
            env.rng.random(0x20); // the pitch of her cry (sound 0x9d)
        }
        let mask = self.particles.hit_mask;
        if (mask & 0x14 == 0 || d < 0x31) && self.o(id).w(0x30) > -0x51 {
            if self.o(id).w(0x28) == -6 {
                let o = self.o(id);
                o.set(0x36, 0xc);
                if mask & 0x48 != 0 {
                    o.set(0x30, o.w(0x30) - 3);
                }
                env.fx.shakes.push((8, 4));
                return;
            }
            let dir = self.particles.hit_direction();
            if dir != 0 {
                self.o(id).set(0x20, -dir);
            }
            if self.o(id).w(0x28) == -5 {
                self.o(id).set(0x2e, 2);
            } else {
                let o = self.o(id);
                o.set(0x28, -5);
                o.set(0x2e, 0);
                let (x, y, f) = (o.w(0xe), o.w(0x10), o.w(0x20));
                // Her hat flies off.
                let ps = &mut self.particles;
                ps.p.group = 0;
                ps.p.hits = 0x8000;
                ps.p.on_death = 0x10;
                ps.p.delay = 3;
                ps.spawn(0x510a, y - 0x14, f * 0xc + x);
                ps.throw(9, 0x30, (f * -0x140 + 0x5a0) as i16 as i32);
                ps.p.on_death = 0;
                ps.p.hits = 0;
                ps.p.group = 1;
                ps.p.delay = 0;
                env.fx.shakes.push((0xc, 4));
            }
            let hy = (self.particles.hit_index >= 0)
                .then(|| self.particles.list[self.particles.hit_index as usize].m.w(0x12));
            let o = self.o(id);
            o.set(0x3a, o.w(0x2e) / 2 + 0x5a30);
            o.set(0x34, 0x7fff);
            if mask & 0x48 == 0 {
                if let Some(hy) = hy {
                    let mut d = hy - o.w(0x10);
                    if d < -3 {
                        if d < -0x1e {
                            d = -0x1e;
                        }
                    } else {
                        d = -4;
                    }
                    o.set(0x34, d);
                }
            } else {
                o.set(0x30, o.w(0x30) - 3);
            }
        } else if mask & 0x400 == 0 {
            let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
            if mask & 8 == 0 {
                self.gibs_kind(y - 0x19, x, 0x20, true, env);
                crate::alien::smoke_sprite(&mut self.particles, y, x);
                let r = env.rng.random(8);
                let by = (y - 0x20) - r;
                let s = env.rng.random(5) + 0x531e;
                self.blood_splat(env.world, s, by, x);
            } else {
                crate::deaths::burnt(&mut self.particles, env.rng, y - 0x19, x);
                crate::alien::smoke_sprite(&mut self.particles, y, x);
                self.particles.last_mut().m.set(0x1e, 0x180);
            }
            self.remove_current(id);
        } else if self.o(id).w(0x28) == -6 {
            let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
            crate::deaths::frozen(&mut self.particles, env.rng, y - 0x14, x);
            self.remove_current(id);
        } else {
            self.o(id).set(0x28, -0xd2);
        }
    }

    /// FUN_1070_ca36 (nested in cb38): blood from where she was hit while she falls over.
    fn granny_bleeds(&mut self, t: i32, facing: i32, y: i32, x: i32, env: &mut Env) {
        let ps = &mut self.particles;
        let puff = |ps: &mut crate::particles::Particles, y, x| {
            // FUN_1070_ca10
            ps.p.colour = 0x10;
            ps.spawn_seq(0x5300, 4, 5, y, x, 1);
            ps.p.colour = 0;
        };
        match t {
            0 => puff(ps, y - 2, x),
            6 => puff(ps, y + 4, x + 2),
            0xf => puff(ps, y - 6, x - 2),
            0xc => {
                ps.p.group = 0;
                ps.p.hits = 0x8000;
                ps.p.on_death = 0x16;
                for i in 1..=3 {
                    ps.p.delay = env.rng.random(5);
                    let s = env.rng.random(2) + 0x5307;
                    ps.spawn(s, y - 5, x);
                    let r = env.rng.random(0x141);
                    let a = (facing * (r + 0x280) + 0x5a0) as i16 as i32;
                    ps.throw(7, (i * 0x18) / 3 + 0x10, a);
                }
                ps.p.on_death = 0;
                ps.p.hits = 0;
                ps.p.group = 1;
                ps.p.delay = 0;
            }
            _ => {}
        }
    }
}
