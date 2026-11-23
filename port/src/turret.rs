//! The walking gunner (record type 0x14d2, icon 62100, FUN_1070_af70): patrols, turns, and when
//! the hero is in sight raises a gun and fires bursts. When shot it falls over and lies there
//! (state 2) soaking up damage until it blows up.

use crate::actors::Env;
use crate::level::Level;
use crate::objects::Obj;
use crate::sprites;
use crate::walker::plates;

pub const KIND: u16 = 0x14d2;

impl Level {
    /// FUN_1070_aec6: it blows up.
    fn turret_explodes(&mut self, id: usize, env: &mut Env) {
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let ps = &mut self.particles;
        ps.p.colour = 0x10;
        ps.spawn_seq(0x6e00, 3, 0x13, y, x, 1); // FUN_1040_690b
        let r = env.rng.random(0x40);
        let m = &mut ps.last_mut().m; // FUN_1050_4a29
        m.set(0x1c, r + 800);
        m.set(0x1e, r + 600);
        m.set(0x42, 0x2148);
        ps.p.colour = 0;
        crate::sound::play_at(0x26, y, x);
        ps.p.group = 0;
        let n = env.rng.random(4) + 3;
        plates(ps, env.rng, 0xc0, n, y, x);
        ps.p.group = 1;
        env.fx.shakes.push((0x20, 0x10));
        crate::alien::smoke_sprite(ps, y, x);
        self.remove_current(id);
    }

    /// FUN_1070_af70.
    pub(crate) fn turret(&mut self, id: usize, env: &mut Env) {
        if self.o(id).w(0x26) > 0 {
            let o = self.o(id);
            o.set(0x26, o.w(0x26) - 1);
        }
        let now = self.particles.now;
        let mut draw_off = 0;
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        self.o(id).set(0x22, y);
        let mut sprite = 0x1973 + ((now >> 4) & 1) as i32;
        self.patrol_box(id, y + 0x20, x + 0x30, y - 0x40, x - 0x30);
        if self.actor_physics(id, env, 9, 7, 3) > 0 {
            if self.o(id).w(0x2c) > 0 {
                env.fx.kills += 1;
            }
            self.turret_explodes(id, env);
            return;
        }
        let h = env.h;
        let (ex, ey) = (self.o(id).w(0xe) - 1, self.o(id).w(0x10) + 4);
        let hy = h.y - (env.hero.2 >> 1);
        let mut angle = crate::trig::atan2(hy - ey, h.x - ex);
        let on_screen = ex - env.camera.0 >= 0 && ey - env.camera.1 >= 0 && ex - env.camera.0 <= 0x13f && ey - env.camera.1 <= 199;
        let in_sight = on_screen && (0xb90..=0x1630).contains(&angle) && self.line_of_sight(env, hy, h.x, ey, ex);
        let state = self.o(id).w(0x24);
        let see = |s: &mut Level, env: &mut Env| {
            let _ = env;
            let o = s.o(id);
            if o.w(0x2e) < 1 && in_sight {
                o.set(0x24, 4);
                o.set(0x2a, -0x19);
                o.set(0x28, 0);
                o.set(0x2e, 0x118);
            }
        };
        match state {
            4 => {
                let o = self.o(id);
                for (off, v) in [(2, -0x7fff), (4, -0x7fff), (6, 0x7fff), (8, 0x7fff)] {
                    o.set(off, v);
                }
                let t = o.w(0x2a) + 1;
                o.set(0x2a, t);
                if t == -0x18 {
                    crate::sound::play_at_pitch(0x84, 0xc0, o.w(0x10), o.w(0xe));
                }
                if t >= 0 {
                    if t - 10 >= 0 && t < 0x27 && (t - 10) % 7 == 0 {
                        self.o(id).set(0x32, 8);
                        crate::sound::play_at(0x9b, self.o(id).w(0x10), self.o(id).w(0xe));
                        let a = (0x10e0 + angle) % 0x1680;
                        if a < 0x5f0 {
                            angle = 0xb90;
                        } else if a > 0x1090 {
                            angle = 0x1630;
                        }
                        let ps = &mut self.particles;
                        ps.p.group = 0;
                        ps.p.hits = 0x1a2;
                        ps.p.damage = 0x18;
                        ps.p.on_death = 0x3e;
                        ps.p.callback = 0x2d2;
                        ps.spawn(0x191e, ey, ex);
                        ps.line(0x28, angle);
                        ps.p.callback = 0;
                        ps.p.on_death = 0;
                        ps.p.damage = 0x10;
                        ps.p.hits = 0;
                        let r = env.rng.random(0x301);
                        crate::bigali::shell_of(ps, 0x1924, 0x10, r + 0x420, ey, ex);
                        ps.spawn_seq(0x190a, 3, 1, ey, ex, 1);
                        ps.p.group = 1;
                        ps.p.colour = 0x28;
                        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
                        self.particles.spawn_seq(0x1b04, 2, 2, y + 4, x, 1);
                        self.particles.p.colour = 0;
                    }
                    if t > 0x35 {
                        self.o(id).set(0x24, 0);
                    }
                }
            }
            3 => {
                let o = self.o(id);
                let t = o.w(0x2a) - 1;
                o.set(0x2a, t);
                if t < 1 {
                    o.set(0x24, 0);
                }
                see(self, env);
            }
            2 => {
                if self.o(id).w(0x1c) == 5 {
                    self.slide_on_slope(id);
                }
                let down = self.o(id).w(0x28) != 0;
                sprite = if down { 0x1975 } else { 0x1973 };
                let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
                self.particles.p.aim_level = 0;
                self.particles.p.aim_level2 = -1;
                let mut d = match sprites::hit_rect(sprite, y, x, false) {
                    Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
                    None => 0,
                };
                if down && self.stomp(sprite, y, x, false, env) > 0 {
                    d += 0x10;
                    self.particles.hit_mask = 0x848;
                    self.o(id).set(0x10, y - 3);
                    env.fx.shakes.push((0xc, 4));
                    crate::sound::play_at(1, y - 3, x);
                }
                self.particles.p.aim_level = 0x100;
                self.particles.p.aim_level2 = 0x100;
                if d > 0 {
                    if down {
                        let dir = self.particles.hit_direction();
                        let o = self.o(id);
                        o.set(0xe, o.w(0xe) + dir);
                    }
                    let hp = self.o(id).w(0x2c) - d;
                    self.o(id).set(0x2c, hp);
                    if hp < -0x3f {
                        if env.rng.random(3) == 0 {
                            { let (y, x) = (self.o(id).w(0x10), self.o(id).w(0xe)); self.bonus(env.world, 0xd10, y, x); }
                        }
                        self.turret_explodes(id, env);
                        return;
                    }
                    if self.particles.hit_mask & 0x48 == 0 {
                        env.fx.shakes.push((0xc, 6));
                    }
                    self.o(id).set(0x26, 3);
                }
            }
            _ => {
                let facing = self.o(id).w(0x20);
                let cnt = self.o(id).w(0x28) + 1;
                self.o(id).set(0x28, cnt);
                let mut turn = false;
                if (cnt as u16) % 3 == 0 {
                    let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
                    let px = facing * 10 + x;
                    let back = env.world.solidity_in(2, y - 4, px);
                    let all = crate::hero::Hero::solidity_all(env.world, self, y - 4, px);
                    if (back | all) & 0x2a == 0 && crate::hero::Hero::solidity_all(env.world, self, y - 0xb, px) & 0x203 != 0 {
                        self.o(id).set(0xe, x + facing);
                    } else {
                        turn = true;
                    }
                    if turn {
                        self.o(id).set(0x20, -facing);
                    }
                }
                let cnt = self.o(id).w(0x28);
                draw_off = (cnt as u16 / 10) as i32 & 1;
                if facing != self.o(id).w(0x20) {
                    let o = self.o(id);
                    o.set(0x24, 3);
                    o.set(0x2a, 0x8c);
                    o.set(0x28, 0);
                }
                see(self, env);
            }
        }
        // Being shot.
        if self.o(id).w(0x2c) > 0 {
            if self.o(id).w(0x2e) > 0 {
                let o = self.o(id);
                o.set(0x2e, o.w(0x2e) - 1);
            }
            if self.o(id).w(0x32) > 0 {
                let o = self.o(id);
                draw_off -= o.w(0x32) >> 2;
                o.set(0x32, o.w(0x32) - 1);
            }
            let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
            let d = match sprites::hit_rect(sprite, y, x, false) {
                Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x1159),
                None => 0,
            };
            if d > 0 {
                let hp = self.o(id).w(0x2c) - d;
                self.o(id).set(0x2c, hp);
                if hp < 1 {
                    let o = self.o(id);
                    o.set(0x24, 2);
                    o.set(0x28, 0);
                    o.set(0x2c, 0);
                    env.fx.kills += 1;
                    let ps = &mut self.particles;
                    let n = env.rng.random(4) + 2;
                    plates(ps, env.rng, 0x80, n, y, x);
                    env.fx.shakes.push((0x1e, 0xc));
                    let o = self.o(id);
                    crate::sound::play_at(0x26, o.w(0x10), o.w(0xe));
                } else {
                    env.fx.shakes.push((0xc, 6));
                }
                self.o(id).set(0x26, 3);
            }
        }
        // For the draw pass: the legs (1170:196e.., mirrored by the facing) and the body.
        let o = self.o(id);
        let (x, y) = (o.w(0xe), o.w(0x10));
        let legs = match o.w(0x24) {
            0 => 0x196e + ((o.w(0x28) as u16 / 10) & 3) as i32,
            2 if o.w(0x28) != 0 => 0x7fff,
            2 => 0x1972,
            _ => 0x196e,
        };
        let mut looks = Vec::new();
        if legs != 0x7fff {
            looks.push(Obj::look(legs, x, y).flip(o.w(0x20) < 0));
        }
        looks.push(Obj::look(sprite, x, y + draw_off).flash(o.w(0x26) > 0));
        o.looks = looks;
    }

    /// FUN_1070_ad87: the move hook. Falling it keeps the box open; a dead one landing flashes.
    pub(crate) fn turret_after_move(&mut self, id: usize, env: &mut Env) {
        let o = self.o(id);
        if o.w(0x1c) == 0 {
            if o.w(0x2c) < 1 {
                if o.w(0x1a) == 0 {
                    o.set(0x18, o.w(0x10));
                    o.set(0x14, -0x7fff);
                    o.set(0x16, 0x7fff);
                }
                o.set(0x1a, o.w(0x1a) + 1);
                for (off, v) in [(2, -0x7fff), (4, -0x7fff), (6, 0x7fff), (8, 0x7fff)] {
                    o.set(off, v);
                }
            }
        } else {
            o.set(0x1a, 0);
            if o.w(0x2c) < 1 && o.w(0x28) == 0 {
                o.set(0x28, 1);
                let (x, y) = (o.w(0xe), o.w(0x10));
                let r = env.rng.random(2);
                self.o(id).set(0x30, r);
                let ps = &mut self.particles;
                ps.p.colour = 0x10;
                ps.spawn_seq(0x6900, 2, 0x19, y, x - 2, 1); // effect 23
                let m = &mut ps.last_mut().m; // FUN_1050_4a29(0x19c, 0x200)
                m.set(0x1c, 0x200);
                m.set(0x1e, 0x19c);
                m.set(0x42, 0x2148);
                ps.p.colour = 0;
                env.fx.shakes.push((0x18, 8));
                let p = env.rng.random(0x3c);
                crate::sound::play_at_pitch(0x4b, 0x100 - p, y, x);
                if env.rng.random(3) == 0 {
                    let d = env.rng.random(0x1e) + 0x32;
                    let ps = &mut self.particles;
                    ps.p.delay = d;
                    ps.p.group = 0;
                    ps.p.on_spawn = 0x7576;
                    ps.spawn_seq(0x1928, 3, 4, y - 6, x, 1); // FUN_1040_3440
                    ps.p.on_spawn = 0;
                    ps.p.group = 1;
                    ps.p.delay = 0;
                }
            }
        }
    }
}
