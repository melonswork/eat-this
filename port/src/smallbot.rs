//! The small security bot (record type 0x145a, smallbot.spr in slot 0x58): FUN_1070_a3cf with its
//! nested procedures (FUN_1070_a0e9 .. a2e0). Fields:
//!
//! | off | field |
//! |-----|-------|
//! | 0a  | 0: patrols and shoots, else only raises the alarm |
//! | 14,16 | patrol range / 20 facing / 22 last y |
//! | 24  | state: 0 patrol, 1 standing by (smoking), 2 wrecked, 3 pause, 4 shooting |
//! | 26  | hit flash / 28 step counter / 2a timer |
//! | 2c  | hit points (<= 0: wrecked; < -0x7f: blown up) / 2e shield / 30 shooting pause |
//! | 32  | sprite / 34 alarmed (byte) / 35 smoking wreck (byte) |

use crate::actors::Env;
use crate::level::Level;
use crate::particles;
use crate::sprites;

impl Level {
    /// FUN_1070_a3cf.
    pub fn smallbot(&mut self, id: usize, env: &mut Env) {
        let h = env.h;
        let x_start;
        {
            let o = self.o(id);
            if o.w(0x26) > 0 {
                o.set(0x26, o.w(0x26) - 1);
            }
            x_start = o.w(0xe);
            o.set(0x22, o.w(0x10));
        }
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        self.patrol_box(id, y + 0x20, x + 0x30, y - 0x40, x - 0x30);
        if self.actor_physics(id, env, 0xe, 7, 3) >= 1 {
            if self.o(id).w(0x2c) > 0 {
                env.fx.kills += 1;
            }
            self.bot_drop_bonus(id);
            self.bot_explode(id, env);
            return;
        }
        let state = self.o(id).w(0x24);
        match state {
            0 => {
                let dir = self.o(id).w(0x20);
                let o = self.o(id);
                if o.b(0x34) == 0 {
                    o.set(0x28, o.w(0x28) + 1);
                    if (o.uw(0x28) % 3) == 0 {
                        self.patrol_step(id, env, 0xe, dir);
                    }
                } else {
                    o.set(0x28, o.w(0x28) + 3);
                    self.patrol_step(id, env, 0xe, dir);
                }
                let o = self.o(id);
                if o.w(10) == 0 {
                    let riding = self.bot_carries_hero(id, x_start);
                    self.bot_spot(id, env, riding);
                } else if o.b(0x34) == 0 {
                    let to_hero = if h.x - o.w(0xe) < 0 { -1 } else { 1 };
                    if to_hero == o.w(0x20)
                        && o.w(0x14) < h.x
                        && h.x < o.w(0x16)
                        && o.w(0x10) - 0x30 < h.y
                        && h.y - h.height < o.w(0x10)
                    {
                        o.set_b(0x34, 1); // the alarm (sound 0x4f)
                    }
                }
                let o = self.o(id);
                if dir != o.w(0x20) {
                    o.set(0x24, 3);
                    o.set(0x2a, 0x8c);
                    o.set(0x28, 0);
                    o.set_b(0x34, 0);
                }
            }
            4 => {
                let o = self.o(id);
                o.set(2, -0x7fff);
                o.set(4, -0x7fff);
                o.set(6, 0x7fff);
                o.set(8, 0x7fff);
                self.bot_carries_hero(id, x_start);
                let o = self.o(id);
                o.set(0x2a, o.w(0x2a) + 1);
                let t = o.w(0x2a);
                if t < 0 {
                    if t > -0x18 {
                        o.set(0x32, t / 6 + 0x5830);
                    }
                } else {
                    let k = t - 10;
                    if k >= 0 && t < 0x4b && k % 8 == 0 {
                        let (x, y) = (o.w(0xe), o.w(0x10));
                        let spread = (k / 8) * 0x98;
                        let ps = &mut self.particles;
                        ps.p.group = 0;
                        ps.p.hits = 0x1a2;
                        ps.p.damage = 0x20;
                        ps.p.on_death = 0x3e;
                        ps.p.callback = 0x2d2;
                        ps.spawn(0x191e, y - 0xc, x + 2);
                        ps.line(0x30, 0x5a0 - (spread + 0x4c));
                        ps.spawn(0x191e, y - 0xc, x - 2);
                        ps.line(0x30, spread + 0x5ec);
                        ps.p.callback = 0;
                        ps.p.on_death = 0;
                        ps.p.damage = 0x10;
                        ps.p.hits = 0;
                        ps.spawn_seq(0x190a, 3, 1, y - 0xb, x, 1);
                        ps.p.group = 1;
                        ps.p.colour = 0x28;
                        ps.spawn_seq(0x1b04, 2, 2, y - 0xc, x, 1);
                        ps.p.colour = 0;
                    }
                    let o = self.o(id);
                    if t > 0x5a {
                        o.set(0x32, 0x5830 - (t - 0x5a) / 6);
                    }
                    if t > 0x70 {
                        o.set(0x24, 0);
                    }
                }
            }
            3 => {
                let o = self.o(id);
                o.set(0x2a, o.w(0x2a) - 1);
                if o.w(0x2a) < 1 {
                    o.set(0x24, 0);
                }
                if o.w(10) == 0 {
                    let riding = self.bot_carries_hero(id, x_start);
                    self.bot_spot(id, env, riding);
                }
            }
            2 => {
                let (x, y, sprite) = (self.o(id).w(0xe), self.o(id).w(0x10), self.o(id).w(0x32));
                let mut damage = match sprites::hit_rect(sprite, y, x, false) {
                    Some((b, r, t, l)) => self.particles.hit_test_low(b, r, t, l, 0x11fb),
                    None => 0,
                };
                if self.stomp(sprite, y, x, false, env) > 0 {
                    damage += 0x10;
                    self.particles.hit_mask = 0x848;
                    let o = self.o(id);
                    o.set(0x10, o.w(0x10) - 3);
                    env.fx.shakes.push((0xc, 4));
                }
                if damage > 0 {
                    let push = self.particles.hit_direction();
                    let o = self.o(id);
                    o.set(0xe, o.w(0xe) + push);
                    o.set(0x2c, o.w(0x2c) - damage);
                    if o.w(0x2c) < -0x7f {
                        if env.rng.random(2) == 0 {
                            eprintln!("warning: a bonus from a smallbot (FUN_1040_6b03) not ported");
                        }
                        self.bot_explode(id, env);
                        return;
                    }
                    if self.particles.hit_mask & 0x48 == 0 {
                        env.fx.shakes.push((0xc, 6));
                    }
                    self.o(id).set(0x26, 3);
                }
            }
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
                    let ps = &mut self.particles;
                    ps.p.group = 0;
                    ps.p.colour = 0x10;
                    particles::smoke(ps, env.rng, y - 0xe, x + 0x10);
                    ps.p.colour = 0;
                    ps.p.group = 1;
                }
            }
            _ => {}
        }
        // Shots.
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        if self.o(id).w(0x2c) < 1 {
            let o = self.o(id);
            if o.b(0x35) != 0 && env.frame & 7 == 0 && env.rng.random(4) != 0 {
                let ps = &mut self.particles;
                ps.p.group = 0;
                ps.p.colour = 0x10;
                let r = env.rng.random(9);
                ps.spawn_seq(0x5141, 0x12, 4, y - 0x11, x - 4 + r, 1);
                ps.line(5, 0x5a0);
                ps.p.colour = 0;
                ps.p.group = 1;
            }
            return;
        }
        {
            let o = self.o(id);
            if o.w(0x30) > 0 {
                o.set(0x30, o.w(0x30) - 1);
            }
        }
        let sprite = self.o(id).w(0x32);
        let mut damage = match sprites::hit_rect(sprite, y, x, false) {
            Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x1159),
            None => 0,
        };
        if self.o(id).w(0x2e) > 0 {
            let s = match sprites::hit_rect(0x580a, y, x, false) {
                Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
                None => 0,
            };
            if s > 0 {
                let o = self.o(id);
                o.set(0x2e, o.w(0x2e) - s);
                if o.w(0x2e) < 0x10 {
                    self.shield_breaks(id, env);
                    let ps = &mut self.particles;
                    ps.p.group = 0;
                    ps.p.colour = 0x10;
                    particles::smoke(ps, env.rng, y - 0xc, x);
                    ps.p.colour = 0;
                    ps.p.group = 1;
                }
                env.fx.shakes.push((0xc, 5));
                let o = self.o(id);
                if o.w(0x2e) < 0 {
                    damage -= o.w(0x2e);
                }
            }
        }
        if damage > 0 {
            let o = self.o(id);
            o.set(0x2c, o.w(0x2c) - damage);
            if o.w(0x2c) < 1 {
                self.bot_drop_bonus(id);
                let o = self.o(id);
                o.set(0x24, 2);
                o.set(0x28, 0);
                o.set(0x2c, 0);
                env.fx.kills += 1;
                if o.w(0x2e) > 0 {
                    self.shield_breaks(id, env);
                }
                let s = env.rng.random(2) + 0x5814;
                self.o(id).set(0x32, s);
                let smoking = env.rng.random(3) == 0;
                self.o(id).set_b(0x35, smoking as u8);
                let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
                let ps = &mut self.particles;
                ps.p.colour = 0x10;
                ps.spawn_seq(0x6400, 1, 0x24, y - 0x12, x, 1);
                ps.line(2, 0x5a0);
                let m = &mut ps.last_mut().m;
                m.set(0x1c, 0x200);
                m.set(0x1e, 600);
                m.set(0x42, 0x2148);
                ps.p.delay = 0x1e;
                smoke_cloud(ps, y - 0x10, x);
                ps.p.delay = 0;
                ps.p.colour = 0;
                ps.p.group = 0;
                ps.p.on_death = 0x10;
                particles::fan(ps, env.rng, 2, 0x581d, 0x10, 0x40, 0, 0xf0, y - 6, x, 5);
                ps.p.on_death = 0;
                ps.p.group = 1;
                env.fx.shakes.push((0x1e, 0xc));
            } else {
                env.fx.shakes.push((0xc, 6));
            }
            let o = self.o(id);
            o.set(0x26, 3);
            if o.w(0x24) == 1 {
                o.set(0x24, 0);
            }
        }
    }

    fn bot_drop_bonus(&mut self, id: usize) {
        let b = self.o(id).w(0x12);
        if b != 0 {
            eprintln!("warning: a bonus {b:#x} dropped (FUN_1040_6b03) not ported");
            self.o(id).set(0x12, 0);
        }
    }

    /// FUN_1070_a18e: the wreck sprite 0x582d carries the hero standing on it.
    fn bot_carries_hero(&mut self, id: usize, _x_start: i32) -> bool {
        if self.o(id).w(0x32) == 0x582d {
            eprintln!("warning: the hero riding a smallbot (FUN_1070_a18e) not ported");
        }
        false
    }

    /// FUN_1070_a0e9: the hero in range and in sight: start shooting.
    fn bot_spot(&mut self, id: usize, env: &mut Env, riding: bool) {
        let h = env.h;
        let o = self.o(id);
        let (x, y) = (o.w(0xe), o.w(0x10));
        if o.w(0x30) < 1
            && x - 0x90 < h.x
            && h.x < x + 0x90
            && y - 0x70 < h.y
            && h.y < y + 0x20
            && !riding
            && self.line_of_sight(env, h.y - (h.height >> 1), h.x, y - 0xc, x)
        {
            let o = self.o(id);
            o.set(0x24, 4);
            o.set(0x2a, -0x28);
            o.set(0x28, 0);
            o.set(0x30, 0x15e);
        }
    }

    /// FUN_1070_a1f6: the shield breaks into four pieces.
    fn shield_breaks(&mut self, id: usize, env: &mut Env) {
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let ps = &mut self.particles;
        ps.p.group = 0;
        ps.p.colour = 0x10;
        ps.p.on_death = 0x10;
        ps.p.callback = 0x17f4;
        ps.p.hits = 0x8000;
        for i in 0..4 {
            let px = x + 3 + i * -2;
            let py = y - 0x10 + env.rng.random(4);
            ps.spawn(0x5819, py, px);
            ps.last_mut().m.set(0x26, 2);
            let a = 0x5a0 - (i * 0x80 - 0xf0 + env.rng.random(0x61));
            let v = env.rng.random(0x10) + 0x50;
            ps.throw(7, v, a);
        }
        ps.p.hits = 0;
        ps.p.callback = 0;
        ps.p.on_death = 0;
        ps.p.colour = 0;
        ps.p.group = 1;
        self.o(id).set(0x2e, 0);
    }

    /// FUN_1070_a2e0: blows up, taking the nearby walls with it.
    fn bot_explode(&mut self, id: usize, env: &mut Env) {
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let ps = &mut self.particles;
        ps.p.colour = 0x10;
        // Effect 23 (FUN_1040_2af4): the blast.
        ps.spawn_seq(0x6900, 2, 0x19, y - 6, x, 1);
        ps.p.delay = 0x1e;
        smoke_cloud(ps, y - 0x10, x);
        ps.p.delay = 0;
        ps.p.colour = 0;
        ps.p.group = 0;
        ps.p.on_death = 0x10;
        particles::fan(ps, env.rng, 2, 0x581d, 0x10, 0x40, 0, 0xf0, y - 6, x, 6);
        ps.p.on_death = 0;
        ps.p.group = 1;
        env.fx.shakes.push((0x20, 0x10));
        crate::alien::smoke_sprite(&mut self.particles, y, x);
        // FUN_1050_5b61(0x60, 4, ...): the tiles around.
        let mut ty = y + 0xe;
        while (y & !15) <= ty {
            let mut tx = x + 0xe;
            while ((x - 0xe) & !15) <= tx {
                if env.world.solidity_at(ty, tx) & 0x80 != 0 {
                    crate::effects::explode_tile(self, env.world, 0x60, 4, ty, tx, env.rng, env.fx);
                }
                if env.world.tile_function_at(ty, tx) == 0x1b8a {
                    eprintln!("warning: FUN_10a8_1369 (blown tile 7050) not ported");
                }
                tx -= 0x10;
            }
            ty -= 0x10;
        }
        self.remove_current(id);
    }
}

/// FUN_1040_6405(y, x): four puffs of smoke, the first after the current delay.
fn smoke_cloud(ps: &mut particles::Particles, y: i32, x: i32) {
    for (dy, dx, more) in [(0, -4, 7), (-5, 1, 4), (5, -2, 0x13), (-1, 0, 0)] {
        ps.spawn_seq(0x6800, 2, 0xf, y + dy, x + dx, 1);
        ps.p.delay += more;
    }
}
