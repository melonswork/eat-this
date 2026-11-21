//! The hovering machine (record type 0x13f6, FUN_1070_30b6) in three stages (+0x32), and the
//! wreck it leaves (VMT 0xcdc, FUN_1070_390f). Position: x and y are longs in 1/16 px at +0xe and
//! +0x12 (FUN_1070_1487 moves them between limits +0x28..+0x2e); +0x16/+0x18 is where it is
//! drawn and hit (FUN_1070_2dd7 bobs it).

use crate::actors::Env;
use crate::level::Level;
use crate::objects::Obj;
use crate::sprites;
use crate::walker::plates;

pub const WRECK: u16 = 0xcdc;

fn puff(ps: &mut crate::particles::Particles, y: i32, x: i32) {
    ps.spawn_seq(0x6800, 2, 0xf, y, x, 1);
}

impl Level {
    /// FUN_1070_1487: drift between the limits, turning at each end.
    fn boss_drift(&mut self, id: usize) {
        let o = self.o(id);
        if o.w(0x20) > 0 {
            o.set(0x20, o.w(0x20) - 1);
            return;
        }
        let (lim_l, lim_r, lim_u, lim_d) = (o.w(0x28), o.w(0x2c), o.w(0x2a), o.w(0x2e));
        if o.w(0x24) != 0 {
            let vy = o.w(0x24);
            o.set_l(0x12, o.l(0x12).wrapping_add(vy));
            let py = o.l(0x12) / 0x10;
            if vy >= 0 {
                if lim_d <= py {
                    o.set(0x24, 0);
                }
            } else if lim_u >= py {
                o.set(0x24, 0);
            }
            return;
        }
        let vx = o.w(0x22);
        o.set_l(0xe, o.l(0xe).wrapping_add(vx));
        let px = o.l(0xe) / 0x10;
        let stop = if vx >= 0 { lim_r <= px } else { px <= lim_l };
        if stop {
            o.set(0x22, -vx);
            o.set(0x20, 0x78);
            let py = o.l(0x12) / 0x10;
            o.set(0x24, if lim_d <= py { -5 } else { 5 });
        }
    }

    /// FUN_1070_2dd7.
    fn boss_move(&mut self, id: usize) {
        let now = self.particles.now as i32;
        if self.o(id).w(0x3c) == 0 {
            self.boss_drift(id);
            let o = self.o(id);
            o.set(0x16, o.l(0xe) / 0x10);
            if o.w(0x24) != 0 {
                if o.w(0x3a) > 0 {
                    o.set(0x3a, o.w(0x3a) - 1);
                }
            } else if o.w(0x3a) < 0x80 {
                o.set(0x3a, o.w(0x3a) + 1);
            }
        }
        let o = self.o(id);
        let idx = (now % 0x3c) * 6 * 0x10;
        let bob = (crate::trig::sin(idx) * o.w(0x3a)) / 0x80000;
        let mut y = bob + o.l(0x12) / 0x10;
        if o.w(0x3c) > 0x44 {
            y -= o.w(0x3c) - 0x44;
        }
        o.set(0x18, y);
    }

    /// FUN_1070_2eaa: the first stage of damage.
    fn boss_stage1(&mut self, id: usize, env: &mut Env) {
        let (x, y) = (self.o(id).w(0x16), self.o(id).w(0x18));
        self.o(id).set(0x32, 1);
        let ps = &mut self.particles;
        let n = env.rng.random(3) + 2;
        plates(ps, env.rng, 0xa0, n, y - 0xd, x);
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.flags = 0x80;
        ps.p.on_death = 0x1c;
        ps.spawn_seq(0x5024, 1, 0x24, y - 0xd, x, 0);
        let r = env.rng.random(0x61);
        ps.throw(9, 0x50, r + 0x590);
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.flags = 0x84;
        ps.p.group = 1;
        ps.p.colour = 0x10;
        crate::particles::smoke(ps, env.rng, y - 0xf, x);
        ps.p.colour = 0;
        env.fx.shakes.push((0x1c, 0x10));
        crate::sound::play_at(0x3d, y, x);
    }

    /// FUN_1070_2f6e: it falls apart and leaves a wreck.
    fn boss_dies(&mut self, id: usize, env: &mut Env) {
        let (x, y) = (self.o(id).w(0x16), self.o(id).w(0x18));
        self.o(id).set(0x32, 2);
        env.fx.kills += 1;
        let ps = &mut self.particles;
        puff(ps, y - 0x14, x - 6);
        for (d, dy, dx) in [(0x1e, -10, 6), (0xb, -0xc, -2), (7, -0x16, 2)] {
            ps.p.delay = d;
            puff(ps, y + dy, x + dx);
        }
        ps.p.delay = 0;
        env.fx.shakes.push((0x18, 0xc));
        crate::sound::play_at(0x48, y, x);
        // FUN_1040_6156.
        ps.p.colour = 0x28;
        ps.p.group = 0;
        ps.spawn_seq(0x1b04, 4, 2, y - 0xc, x, 1);
        ps.p.colour = 0;
        ps.p.group = 1;
        let mut w = Obj::new(WRECK, WRECK, 0x26);
        w.set(0xc, 1);
        w.set(0x1a, 0);
        w.set(0x1c, 0);
        w.set(0xe, x);
        w.set(0x10, y);
        w.set(0x20, 0x101);
        w.set(0x22, 0x5027);
        w.set(0x24, 0);
        for (off, v) in [(2, -0x7fff), (4, -0x7fff), (6, 0x7fff), (8, 0x7fff)] {
            w.set(off, v);
        }
        let wid = self.objects.len();
        self.objects.push(Some(w));
        let (s, l) = self.walking;
        self.lists[s][l].push(wid);
        self.remove_current(id);
    }

    /// FUN_1070_30b6.
    pub(crate) fn boss(&mut self, id: usize, env: &mut Env) {
        if self.o(id).w(0x34) > 0 {
            let o = self.o(id);
            o.set(0x34, o.w(0x34) - 1);
        }
        let now = self.particles.now as i32;
        let state = self.o(id).w(0x32);
        let sprite;
        if state == 0 {
            self.boss_move(id);
            self.o(id).set(0x26, (now / 8) % 0x18 + 0x5000);
            sprite = (now / 2) % 0xc + 0x5018;
            rotor_loop(id, self.o(id));
        } else {
            self.boss_move(id);
            rotor_loop(id, self.o(id));
            let (x, y) = (self.o(id).w(0x16), self.o(id).w(0x18));
            let o = self.o(id);
            if o.w(0x38) < 0x142 && o.w(0x34) > 0 && o.w(0x34) % 0x20 == 0 {
                let px = x - 4 + env.rng.random(9);
                let py = (y + 2) - env.rng.random(0x12);
                let ps = &mut self.particles;
                ps.spawn_seq(0x5111, 4, 9, py, px, 1);
                ps.line(8, 0x5a0);
            }
            self.o(id).set(0x26, (now / 3) % 0x18 + 0x5000);
            sprite = (now / 0xc) % 2 + 0x5025;
        }
        let (x, y) = (self.o(id).w(0x16), self.o(id).w(0x18));
        let top = y + 7;
        let mut floor = (top & !15) + 0xf;
        while Self::hero_solid(self, env, floor + 1, x) & 2 == 0 {
            floor += 0x10;
        }
        // (The sector's +0x16 limit is 0x7fff.)
        if self.o(id).w(0x3c) > 0 {
            let o = self.o(id);
            o.set(0x3c, o.w(0x3c) - 1);
        }
        let h = env.h;
        let cd = self.o(id).w(0x3c);
        if std::env::var("ET_BOSS").is_ok() {
            { let o = self.o(id); eprintln!("BOSSF vx {} vy {} wait {} lims l{} r{} u{} d{} X {} Y {}", o.w(0x22), o.w(0x24), o.w(0x20), o.w(0x28), o.w(0x2c), o.w(0x2a), o.w(0x2e), o.l(0xe), o.l(0x12)); }
            eprintln!("BOSS {} at ({x},{y}) top {top} floor {floor} hero ({},{}) hw {} hp {} state {state} cd {}", self.particles.now, h.x, h.y, h.half_width, env.hero_obj.health, cd);
        }
        if h.x - h.half_width <= x && x < h.x + h.half_width && h.y > top && h.y <= floor && env.hero_obj.health > 0 {
            if self.o(id).w(0x3c) < 0x30 {
                let p = env.rng.random(0x20);
                crate::sound::play_at_pitch(0x6c, 0x100 - p, y, x);
                env.hero_obj.hurt(0x80, env.rng);
                self.o(id).set(0x3c, 0x48);
            }
        }
        // For the draw pass: the body (the rotor +0x26 and the flash are added when drawing).
        self.o(id).looks = vec![Obj::look(sprite, x, y)];
        let d = match sprites::hit_rect(sprite, y, x, false) {
            Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x1059),
            None => 0,
        };
        if d <= 0 {
            return;
        }
        let hp = self.o(id).w(0x38) - d;
        self.o(id).set(0x38, hp);
        if state < 1 && hp <= 0x181 {
            self.boss_stage1(id, env);
        }
        if self.o(id).w(0x32) < 2 && hp <= 0x101 {
            self.boss_dies(id, env);
            return;
        }
        if self.particles.hit_mask != 0x48 && self.o(id).w(0x34) < 0x22e {
            let (x, y, vx) = (self.o(id).w(0x16), self.o(id).w(0x18), self.o(id).w(0x22));
            let ps = &mut self.particles;
            puff(ps, y - 8, x);
            ps.p.delay = 0x18;
            puff(ps, y - 0x10, (vx * 0x2a) / 0x10 + x);
            ps.p.delay = 0x24;
            puff(ps, y - 6, (vx * 0x3a) / 0x10 + x);
            ps.p.delay = 0;
            env.fx.shakes.push((0x10, 8));
            crate::sound::play_at(0x49, y, x);
        }
        self.o(id).set(0x34, 0x230);
    }

    fn hero_solid(&self, env: &Env, y: i32, x: i32) -> u16 {
        crate::hero::Hero::solidity_all(env.world, self, y, x)
    }

    /// FUN_1070_37c7: the wreck goes up.
    fn boss_wreck_blows(&mut self, id: usize, env: &mut Env) {
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        if env.rng.random(2) == 0 {
            self.bonus(env.world, 0xd10, y, x);
        }
        crate::alien::smoke_sprite(&mut self.particles, y, x);
        let ps = &mut self.particles;
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x11;
        ps.spawn(0x502a, y - 2, x - 3);
        ps.throw(9, 0x68, 0x620);
        ps.p.draw = 1;
        ps.spawn(0x5029, y - 2, x + 3);
        ps.throw(9, 0x70, 0x550);
        ps.p.draw = 0;
        ps.spawn(0x502b, y - 3, x);
        ps.throw(9, 0x80, 0x580);
        ps.spawn(0x5029, y - 3, x);
        ps.throw(9, 0x70, 0x5f0);
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
        ps.spawn_seq(0x6400, 2, 0x24, y - 0xc - 6, x, 1); // effect 7 (at y - 0xc)
        ps.line(2, 0x5a0);
        env.fx.shakes.push((0x24, 0xe));
        crate::sound::play_at(0x4a, y, x);
        self.remove_current(id);
    }

    /// FUN_1070_390f.
    pub(crate) fn boss_wreck(&mut self, id: usize, env: &mut Env) {
        if self.o(id).w(0x24) > 0 {
            let o = self.o(id);
            o.set(0x24, o.w(0x24) - 1);
        }
        let y0 = self.o(id).w(0x10);
        {
            let o = self.o(id);
            let (x, y) = (o.w(0xe), o.w(0x10));
            o.set(2, x - 0x20);
            o.set(6, x + 0x20);
            o.set(4, y - 0x30);
            o.set(8, y + 0x10);
        }
        if self.actor_physics(id, env, 0xd, 0xe, 8) >= 1 {
            self.boss_wreck_blows(id, env);
            return;
        }
        if self.o(id).w(0x20) < 0x101 {
            if self.o(id).w(0x1a) < 1 {
                let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
                self.particles.p.aim_level = 0;
                self.particles.p.aim_level2 = -1;
                let mut d = match sprites::hit_rect(0x5027, y, x, false) {
                    Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
                    None => 0,
                };
                if self.stomp(0x5027, y, x, false, env) > 0 {
                    d += 0x10;
                    self.particles.hit_mask = 0x48;
                    self.o(id).set(0x10, y - 3);
                    env.fx.shakes.push((0xc, 4));
                    crate::sound::play_at(1, y - 3, x);
                }
                self.particles.p.aim_level = 0x100;
                self.particles.p.aim_level2 = 0x100;
                if d > 0 {
                    let hp = self.o(id).w(0x20) - d;
                    self.o(id).set(0x20, hp);
                    if hp < 0 {
                        self.boss_wreck_blows(id, env);
                        return;
                    }
                    if hp < 0x81 && 0x80 < hp + d {
                        self.o(id).set(0x22, 0x5028);
                        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
                        let ps = &mut self.particles;
                        smoke4(ps, y - 0xb, x + 6);
                        ps.p.delay = 0x18;
                        smoke4(ps, y - 0x13, x - 0xc);
                        ps.p.delay = 0;
                        let n = env.rng.random(3) + 2;
                        plates(ps, env.rng, 0xf0, n, y - 4, x);
                        env.fx.shakes.push((0x10, 10));
                        self.o(id).set(0x10, y - 8);
                    }
                    if self.particles.hit_mask != 0x48 && self.o(id).w(0x24) < 1 {
                        let p = env.rng.random(0x20);
                        let o = self.o(id);
                        crate::sound::play_at_pitch(0x4b, 0x100 - p, o.w(0x10), o.w(0xe));
                        env.fx.shakes.push((0xc, 4));
                    }
                    self.o(id).set(0x24, 3);
                }
                let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
                let hw = env.h.half_width;
                self.stand_on(env, y - 0xc, y0 - 0xc, x + 0xc + hw, (x - 0xc) - hw);
                if self.o(id).w(0x1c) == 5 {
                    self.slide_on_slope(id);
                }
            }
        } else if self.particles.now % 3 == 0 {
            let r = env.rng.random(0xb);
            let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
            let ps = &mut self.particles;
            ps.p.group = 0;
            ps.spawn_seq(0x6801, 5, 0xe, y - 0x15, x - 5 + r, 1);
            ps.p.group = 1;
        }
    }
}

/// Its rotor's loop (handle +0x30) while in earshot.
fn rotor_loop(id: usize, o: &Obj) {
    if crate::sound::audible(o.w(0x18), o.w(0x16)) {
        crate::sound::priority(0x11f4);
        crate::sound::keep(crate::sound::key::obj(id, 0x30), 0x2d);
    }
}

/// FUN_1040_6405: four puffs of smoke.
pub(crate) fn smoke4(ps: &mut crate::particles::Particles, y: i32, x: i32) {
    puff(ps, y, x - 4);
    ps.p.delay += 7;
    puff(ps, y - 5, x + 1);
    ps.p.delay += 4;
    puff(ps, y + 5, x - 2);
    ps.p.delay += 0x13;
    puff(ps, y - 1, x);
}
