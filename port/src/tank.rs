//! The tank (OBJECTS 0x146e, VMT 0xdf8, FUN_1070_b9d3, sprites 0x5900..): a big tank patrolling
//! its range (+0x14..+0x16) with a turret (+0x32 hit points), a hull (+0x2e), a grenade launcher
//! arm (+0x30: while positive it lobs grenades after each turn; shot to 0 it drops off and a claw
//! comes out that grabs the hero, hero state -3) and a hatch (+0x34). Shot to pieces part by
//! part; the hull's death leaves a burning wreck. Running into the hero hurts him (hit kind 3),
//! the hero can stand on it.
//!
//! | off | field |
//! |-----|-------|
//! | 20  | direction (-1/1) / 22 state: 0 driving, 1 turning (+0x2c frames), -1 turret gone, -2 wreck |
//! | 24/26/28 | hit flashes: hull, arm/claw, turret |
//! | 2a  | the track position (wraps; the track frame and the step +0x38 come from it) |
//! | 2e/30/32/34 | hit points: hull, arm (claw: 0..-0x400), turret, hatch |
//! | 36  | last frame's body y / 38 this frame's step / 3a recoil of the arm / 3c of the gun |
//! | 3e  | the gun's timer (fires at 0..0x50 every 4 frames) |
//! | 40  | the arm's timer / 42 speed / 44 turns made / 46 the claw's reach / 48 its frame |
//! | 4a  | (byte) the claw hit a wall last frame / 4b the engine loop's handle |

use crate::actors::Env;
use crate::level::Level;
use crate::sound;
use crate::sprites;

pub const KIND: u16 = 0x146e;

impl Level {
    /// FUN_1050_5960(sprite, y, x, 0x1059): the shots hitting this sprite's box.
    fn tank_hit(&mut self, sprite: i32, y: i32, x: i32) -> i32 {
        match sprites::hit_rect(sprite, y, x, false) {
            Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x1059),
            None => 0,
        }
    }

    /// FUN_1070_b9d3.
    pub(crate) fn tank(&mut self, id: usize, env: &mut Env) {
        let key = sound::key::obj(id, 0x4b);
        for f in [0x24, 0x26, 0x28] {
            let o = self.o(id);
            if o.w(f) > 0 {
                o.set(f, o.w(f) - 1);
            }
        }
        {
            let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
            self.patrol_box(id, y + 0x10, x + 0x80, y - 0x60, x - 0x80);
        }
        self.actor_physics(id, env, 0x10, 0x50, 0x30);
        let mut ly = self.o(id).w(0x10) + 2;
        let old_dir = self.o(id).w(0x20);
        let step = self.o(id).w(0x38);
        self.patrol_step(id, env, 0x50, step);
        {
            let o = self.o(id);
            o.set(0x38, 0);
            for f in [0x3a, 0x3c] {
                if o.w(f) != 0 && o.w(f) < 0x14 {
                    o.set(f, o.w(f) + 1);
                }
            }
        }
        let state = self.o(id).w(0x22);
        if state == 0 {
            let o = self.o(id);
            if o.w(0x32) > 0 {
                let c = ((o.uw(0x2a) >> 3) & 1 == 1 && o.b(0x4a) == 0) as i32;
                ly = o.w(0x10) - c;
            }
            let before = (o.uw(0x2a) / 3) as i32;
            o.set(0x2a, o.w(0x2a) - o.w(0x20) * o.w(0x42));
            o.set(0x38, before - (o.uw(0x2a) / 3) as i32);
            let (x, y) = (o.w(0xe), o.w(0x10));
            if env.world.solidity_at(y, x - 0x32) & 0x80 != 0 {
                crate::effects::explode_tile(self, env.world, 0x60, 0x104, y, x - 0x32, env.rng, env.fx);
            }
            if self.o(id).w(0x20) != old_dir {
                let o = self.o(id);
                o.set(0x22, 1);
                o.set(0x44, o.w(0x44) + 1);
                if o.w(0x40) < 1 || o.w(0x30) < 1 {
                    o.set(0x2c, 0x38);
                } else {
                    o.set(0x2c, 0xaf);
                    recoil(o, 0x3a, false);
                    o.set(0x40, 0);
                    if o.w(0x2e) > 0x400 {
                        // FUN_1040_66fa: the smoke of the shot the arm fires on the turn.
                        let sx = x + 0xd + o.w(0x38);
                        let ps = &mut self.particles;
                        ps.p.callback = 0x2192;
                        ps.p.colour = 0x10;
                        ps.spawn_seq(0x6c00, 4, 0x19, ly - 0x49, sx, 1);
                        ps.p.colour = 0;
                        ps.p.callback = 0;
                        sound::play_at(0x30, y, x);
                    }
                }
                self.o(id).set(0x42, 1);
            }
            let o = self.o(id);
            if (o.w(0x20) < 0 || o.w(0x30) < -0x3ff)
                && ((o.w(0x14) + o.w(0x16)) as u16 >> 1) == o.uw(0xe)
                && o.w(0x3e) < -0x1d
            {
                self.tank_gun_up(id);
            }
        } else if state == 1 {
            ly = self.o(id).w(0x10);
            let o = self.o(id);
            o.set(0x2c, o.w(0x2c) - 1);
            if o.w(0x2c) < 1 {
                o.set(0x22, 0);
                env.fx.shakes.push((0x10, 6));
                let o = self.o(id);
                if o.w(0x20) < 1 {
                    if o.w(0x44) % 6 == 0 && env.h.x < o.w(0xe) - 0x20 {
                        o.set(0x42, 6);
                    }
                } else if o.w(0x30) > 0 {
                    o.set(0x40, 0xc1e);
                    recoil(o, 0x3a, true);
                }
            }
        } else if state == -1 && self.o(id).w(0x3e) < -0x87 {
            self.tank_gun_up(id);
            let o = self.o(id);
            o.set(0x44, (o.w(0x44) + 1) & 1);
        }
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let last = self.o(id).w(0x36);
        // The hero standing on it moves with it (gHeroDX += +0x38).
        let step = self.o(id).w(0x38);
        if self.o(id).w(0x2e) < 1 {
            if self.stand_on(env, ly - 0x24, last - 0x24, x + 0x2d, x - 0xc) {
                env.hero_obj.carry.0 += step;
            }
            let o = self.o(id);
            if o.w(0x40) < 0x58 {
                o.set(0x40, o.w(0x40) + 1);
            }
        } else {
            if self.stand_on(env, ly - 0x44, last - 0x44, x + 0x19, x - 0x10)
                || self.stand_on(env, ly - 0x27, last - 0x27, x + 0x31, x + 0x18)
            {
                env.hero_obj.carry.0 += step;
            }
            let turning = self.o(id).w(0x22) != 0;
            if turning {
                sound::set_volume(0xaa);
            }
            if sound::audible(y, x) {
                sound::priority(0x11f4);
                sound::pitch(if turning { 0x11e } else { 0x100 });
                sound::keep(key, 0x36);
                self.o(id).set(0x4b, id as i32);
            }
            sound::set_volume(0x100);
        }
        // Running into the hero (FUN_1088_52d6).
        if self.o(id).w(0x38) != 0 && !env.hero_obj.invulnerable() {
            let h = env.h;
            if ranges(h.x + h.half_width, h.x - h.half_width, x + 0x28, x - 0x2c)
                && ranges(h.y, (h.y - h.height) + 1, y, y - 0x14)
            {
                env.hero_obj.hurt_kind(0x80, 3, env.rng);
                self.o(id).set(0x42, 1);
            }
        }
        // The hatch.
        if self.o(id).w(0x34) > 0 {
            let d = self.tank_hit(0x5923, ly, x);
            if d > 0 {
                let o = self.o(id);
                o.set(0x34, o.w(0x34) - d);
                if o.w(0x34) < 0x10 {
                    self.tank_hatch_off(id, env, ly);
                    let ps = &mut self.particles;
                    ps.p.group = 0;
                    ps.p.colour = 0x10;
                    crate::particles::smoke(ps, env.rng, y - 0x2d, x + 0x20);
                    ps.p.colour = 0;
                    ps.p.group = 1;
                    sound::play_at(0x51, y, x);
                }
                env.fx.shakes.push((0xc, 5));
            }
        }
        // The hull and its gun.
        if self.o(id).w(0x2e) > 0 {
            let o = self.o(id);
            o.set(0x3e, o.w(0x3e) - 1);
            let t = o.w(0x3e);
            if t > -0x1f {
                if (0..0x51).contains(&t) && t % 4 == 0 {
                    let (st, turns) = (o.w(0x22), o.w(0x44));
                    let ps = &mut self.particles;
                    ps.p.hits = 2;
                    ps.p.damage = 0x20;
                    ps.p.on_death = 0x3e;
                    if st == -1 {
                        if t % 0x10 == 0 {
                            sound::play_at(0x1d, ly, x);
                            for i in 0..8 {
                                ps.spawn(-1, ly - 0x2f, x + 5);
                                ps.line(0x20, ((turns * 0x2d0) as u16 >> 1) as i32 + (i * 0x2d0) % 0x1680);
                            }
                        }
                    } else {
                        sound::play_at(0x1d, ly, x);
                        ps.spawn(-1, ly - 0x2f, x + 5);
                        ps.line(0x20, ((t / 4) * -0xd8 + 0x2490) % 0x1680);
                    }
                    ps.p.on_death = 0;
                    ps.p.damage = 0x10;
                    ps.p.hits = 0;
                }
                if t < -0x1d {
                    self.tank_gun(id, false);
                }
            }
            let d = self.particles.hit_test_sprite((0x590b, x, ly, 0, 0), 0x1059);
            if d > 0 {
                let o = self.o(id);
                o.set(0x2e, o.w(0x2e) - d);
                if o.w(0x2e) < 1 {
                    self.tank_wreck(id, env, ly, key);
                } else {
                    env.fx.shakes.push((0xc, 5));
                }
                self.o(id).set(0x24, 3);
            }
        }
        let hit_wall = self.o(id).b(0x4a) != 0;
        self.o(id).set_b(0x4a, 0);
        let arm = self.o(id).w(0x30);
        if arm >= -0x3ff {
            if arm > 0 {
                // The grenade launcher.
                let o = self.o(id);
                if o.w(0x40) > 0 && env.world.solidity_at(y, x - 0x70) & 2 == 0 {
                    let t = o.w(0x40);
                    if t < 0xc01 && t % 0x30 == 0 {
                        let r = env.rng.random(0x31);
                        self.grenade(0x30, 0xb40 - r, ly - 0x25, x - 0x20);
                    }
                    let o = self.o(id);
                    o.set(0x40, o.w(0x40) - 1);
                }
                let d = self.tank_hit(0x590a, ly, x);
                if d > 0 {
                    let o = self.o(id);
                    o.set(0x30, o.w(0x30) - d);
                    if o.w(0x30) < 1 {
                        self.tank_arm_off(id, env, ly);
                    } else {
                        env.fx.shakes.push((0xc, 5));
                        self.o(id).set(0x26, 3);
                    }
                }
            } else {
                self.tank_claw(id, env, ly, hit_wall);
                let d = {
                    let reach = self.o(id).w(0x46);
                    self.tank_hit(0x593c, ly, x - reach)
                };
                if d > 0 {
                    let o = self.o(id);
                    o.set(0x30, o.w(0x30) - d);
                    if o.w(0x30) < -0x3ff {
                        self.tank_claw_off(id, env, ly);
                    } else {
                        env.fx.shakes.push((0xc, 5));
                        self.o(id).set(0x26, 3);
                    }
                }
            }
        }
        // The turret.
        if self.o(id).w(0x32) < 1 {
            sound::set_volume(0x80);
            if sound::audible(y - 0x1e, x) {
                sound::priority(0x11f4);
                sound::keep(key, 0x1e);
                self.o(id).set(0x4b, id as i32);
            }
            sound::set_volume(0x100);
        } else {
            let d = self.tank_hit(0x5900, y, x);
            if d > 0 {
                let o = self.o(id);
                o.set(0x32, o.w(0x32) - d);
                if o.w(0x32) < 1 {
                    self.tank_turret_off(id);
                    let o = self.o(id);
                    o.set(0x22, -1);
                    env.fx.shakes.push((0x1e, 0x10));
                    o.set(0x3e, if o.w(0x3e) < -0x1d { -0x1e } else { -0x16 });
                    sound::play_at(0x17, y, x);
                } else {
                    env.fx.shakes.push((0xc, 5));
                }
                self.o(id).set(0x28, 3);
            }
        }
        self.o(id).set(0x36, ly);
        if std::env::var_os("ET_TANK").is_some() {
            let o = self.o(id);
            eprintln!("TANK {} x {} y {} st {} dir {} hp {} {} {} {} t3e {} t40 {} reach {}", env.frame, o.w(0xe), o.w(0x10), o.w(0x22), o.w(0x20), o.w(0x2e), o.w(0x30), o.w(0x32), o.w(0x34), o.w(0x3e), o.w(0x40), o.w(0x46));
        }
    }

    /// The claw (arm hit points 0..-0x400): reaches out on its timer, grabs the hero.
    fn tank_claw(&mut self, id: usize, env: &mut Env, ly: i32, hit_wall: bool) {
        let x = self.o(id).w(0xe);
        let h = env.h;
        let o = self.o(id);
        o.set(0x40, o.w(0x40) - 1);
        if o.w(0x40) < 1 {
            if o.w(0x40) < -0x16 {
                if o.w(0x48) > 0 {
                    o.set(0x48, o.w(0x48) - 1);
                }
            } else {
                o.set(0x48, crate::alien::pingpong(0xc - o.w(0x40), 1, 0xc));
            }
            if o.w(0x40) < -0x54 {
                // FUN_1070_b9b0.
                o.set(0x40, 0x60);
                let y = o.w(0x10);
                sound::play_at(0x30, y, x);
            }
        } else {
            self.tank_reach(id, env, ly, hit_wall);
        }
        // Standing on the claw's arm (FUN_1070_0078, then one of its four segments).
        let hs = env.hero_obj.state;
        let vy = env.h.vy;
        let can = (hs >= 1 && vy >= 0) || (hs < 1 && !(-5..=-1).contains(&hs));
        if can {
            let o = self.o(id);
            let mut lx = x - 0xc;
            let step = o.w(0x46) / 4 + 4;
            // (the original takes the smaller of an x and a y here)
            let top = lx.min(o.w(0x36)) - 0x30;
            let bottom = o.w(0x36).max(ly) - 0x2e;
            for i in 0..4 {
                let left = (lx - step) + if i == 3 { -0xc } else { 0 };
                if h.x >= left && h.x <= lx && ranges(bottom + i, top + i, h.y, h.prev_y) {
                    env.fx.hero_stand = Some(ly - 0x2f + i);
                    env.hero_obj.carry.0 += o.w(0x38);
                    break;
                }
                lx -= step;
            }
        }
    }

    /// The claw's reach while its timer runs: out, back, stopped by walls; it grabs the hero.
    fn tank_reach(&mut self, id: usize, env: &mut Env, ly: i32, hit_wall: bool) {
        let x = self.o(id).w(0xe);
        let h = env.h;
        let o = self.o(id);
        if o.w(0x40) < 0x1d {
            o.set(0x46, o.w(0x46) - 4);
            if o.w(0x46) < 1 {
                o.set(0x46, 0);
                o.set(0x40, 0);
            }
        } else {
            o.set(0x46, (o.w(0x46) + 4).min(0x70));
        }
        let lx = (x - 0x29) - o.w(0x46);
        let ly2 = ly - 0x2d;
        if env.world.solidity_at(ly2, lx) & 2 != 0 {
            let o = self.o(id);
            o.set(0x46, (x - 0x29) - ((lx & !0xf) + 0x10));
            if !hit_wall {
                env.fx.shakes.push((8, 4));
                sound::play_at(0x50, ly2, lx);
            }
            o.set_b(0x4a, 1);
            if o.w(0x40) > 0x45 {
                o.set(0x40, 0x45);
            }
        }
        if lx < h.x + h.half_width
            && h.x - h.half_width < lx + 0xd
            && h.y - h.height < ly2 + 0xb
            && ly2 < h.y - 5
            && self.o(id).w(0x40) != 0x1d
        {
            // Held: pulled towards the claw (gHeroDX/DY), gHeroCrouch = 1.
            let mut held = env.hero_obj.state == crate::hero::HELD;
            if !held && env.hero_obj.hurt_kind(0x60, 0, env.rng) == 1 {
                let hero = &mut *env.hero_obj;
                sound::play_at(0x1a, hero.y, hero.x);
                sound::play_at(0x16, hero.y, hero.x);
                hero.grab();
                env.fx.shakes.push((0x18, 0xe));
                held = true;
            }
            if held {
                let step = self.o(id).w(0x38);
                let hero = &mut *env.hero_obj;
                let dx = (lx + step + 5) - h.x;
                hero.set_dx(dx.signum() * dx.abs().min(4));
                let dy = (ly2 + 0x25) - h.y;
                hero.carry.1 = dy.signum() * dy.abs().min(2);
                hero.hold();
            }
        }
        let held = env.hero_obj.state == crate::hero::HELD;
        let o = self.o(id);
        if held || o.w(0x46) > 0x6f {
            o.set(0x48, (o.w(0x48) - 2).max(0));
        } else if o.w(0x48) < 0xb {
            o.set(0x48, o.w(0x48) + 1);
        }
    }

    /// FUN_1070_b993: the gun swings up (FUN_1070_b936(1)), +0x3e = 0x83.
    fn tank_gun_up(&mut self, id: usize) {
        self.tank_gun(id, true);
        self.o(id).set(0x3e, 0x83);
    }

    /// FUN_1070_b936: the gun's recoil, with sound 0x15 (down) or 0x14 (up).
    fn tank_gun(&mut self, id: usize, up: bool) {
        let o = self.o(id);
        recoil(o, 0x3c, up);
        let (x, y) = (o.w(0xe), o.w(0x10));
        sound::play_at(0x15 - up as usize, y, x);
    }

    /// FUN_1070_b5e6: the hatch flies off in six pieces.
    fn tank_hatch_off(&mut self, id: usize, env: &mut Env, _ly: i32) {
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let ps = &mut self.particles;
        ps.p.group = 0;
        ps.p.colour = 0x10;
        ps.p.on_death = 0x10;
        ps.p.callback = 0x177a;
        ps.p.hits = 0x8000;
        for i in 0..6 {
            let px = x + 0x25 - i * 2;
            let py = (y - 0x2d) - env.rng.random(4);
            let s = env.rng.random(2);
            ps.spawn(0x5925, py, px);
            ps.last_mut().m.set(0x26, s + 2);
            let a = 0x5a0 - (i * 0x80 - 0xd0 + env.rng.random(0x61));
            let sp = env.rng.random(0x10);
            ps.throw(7, sp + 0x50, a);
        }
        ps.p.hits = 0;
        ps.p.callback = 0;
        ps.p.on_death = 0;
        ps.p.colour = 0;
        ps.p.group = 1;
        self.o(id).set(0x34, 0);
    }

    /// FUN_1070_b6db: the claw is shot off.
    fn tank_claw_off(&mut self, id: usize, env: &mut Env, ly: i32) {
        let o = self.o(id);
        let (x, reach) = (o.w(0xe), o.w(0x46));
        let (py, px) = (ly - 0x2d, (x - 0x22) - reach);
        let ps = &mut self.particles;
        ps.p.on_death = 0x10;
        crate::particles::fan(ps, env.rng, 0, 0x5949, 0x10, 0x40, 0, 0xf0, py, px, 3);
        ps.p.colour = 0x10;
        crate::particles::smoke(ps, env.rng, py, px);
        ps.p.colour = 0;
        ps.p.hits = 0x8000;
        for i in 0..4 {
            ps.spawn(0x5948, py, (x - 0xc) - ((i * 2 + 1) * reach) / 8);
            let a = env.rng.random(0xa1) + 0x500;
            ps.throw(9, 0x40, a);
        }
        ps.p.hits = 0;
        ps.p.on_death = 0;
        sound::play_at(0x26, py, px);
        env.fx.shakes.push((0x1e, 0xc));
    }

    /// FUN_1070_b7cd: the grenade launcher is shot off (the claw comes out next).
    fn tank_arm_off(&mut self, id: usize, env: &mut Env, ly: i32) {
        let o = self.o(id);
        o.set(0x30, 0);
        let (lx, py) = (o.w(0xe) - 0x22, ly - 0x2c);
        let ps = &mut self.particles;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x2f;
        ps.p.group = 0;
        ps.spawn(0x5932, py + 0x12, lx - 0xc);
        ps.last_mut().m.set(0x26, 4);
        ps.throw(9, 0x20, 0x6e0);
        ps.p.hits = 0;
        ps.p.on_death = 0;
        ps.p.colour = 0x10;
        crate::effects::effect(self, env.world, 9, py, lx - 8, env.rng, env.fx);
        let ps = &mut self.particles;
        ps.line(8, 0x5a0);
        ps.p.colour = 0;
        ps.p.on_death = 0x2f;
        ps.p.hits = 0x8000;
        ps.p.group = 1;
        ps.spawn(0x5933, py + 0x12, lx + 8);
        ps.last_mut().m.set(0x26, 4);
        ps.throw(9, 0x20, 0x460);
        ps.p.hits = 0;
        ps.p.on_death = 0;
        sound::play_at(0x4a, py, lx);
        env.fx.shakes.push((0x1e, 0xc));
        self.o(id).set(0x40, 0);
    }

    /// FUN_1070_b8d3: the turret is shot off: dust (FUN_1040_66d4 + FUN_1050_4a29(0x100, 0x80)).
    fn tank_turret_off(&mut self, id: usize) {
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let ps = &mut self.particles;
        ps.p.colour = 0x10;
        ps.spawn_seq(0x6d00, 2, 0x13, y, x, 1);
        ps.p.colour = 0;
        let m = &mut ps.last_mut().m;
        m.set(0x1c, 0x80);
        m.set(0x1e, 0x100);
        m.set(0x42, 0x2148);
    }

    /// The hull shot to 0: everything left comes off, plates fly, the wreck burns.
    fn tank_wreck(&mut self, id: usize, env: &mut Env, ly: i32, key: u64) {
        env.fx.kills += 1;
        if self.o(id).w(0x4b) != -1 {
            sound::stop(key);
            self.o(id).set(0x4b, -1);
        }
        if self.o(id).w(0x34) > 0 {
            self.tank_hatch_off(id, env, ly);
        }
        if self.o(id).w(0x32) > 0 {
            self.tank_turret_off(id);
            self.o(id).set(0x32, 0);
        }
        if self.o(id).w(0x30) > 0 {
            self.tank_arm_off(id, env, ly);
        }
        if self.o(id).w(0x30) > -0x400 {
            self.tank_claw_off(id, env, ly);
            self.o(id).set(0x30, -0x400);
        }
        let o = self.o(id);
        o.set(0x3c, 0);
        o.set(0x40, 0);
        o.set(0x22, -2);
        let (x, y) = (o.w(0xe), o.w(0x10));
        let ps = &mut self.particles;
        ps.p.on_death = 0x11;
        ps.p.hits = 0x8000;
        for i in 0..3 {
            ps.p.draw = env.rng.random(2);
            let px = x - 4 + i * 10;
            let s = env.rng.random(2);
            ps.spawn(s + 0x591f, ly - 0x34, px);
            let a = (i * -0x140 + 0x710) - env.rng.random(0x61);
            let sp = env.rng.random(8);
            ps.throw(9, sp + 0x40, a);
        }
        ps.p.draw = 0;
        ps.p.on_death = 0x10;
        crate::particles::fan(ps, env.rng, 0, 0x5921, 0x10, 0x50, 0, 0x168, ly - 0x3e, x + 6, 4);
        ps.p.on_death = 0;
        ps.p.hits = 0;
        env.fx.shakes.push((0x2a, 0x10));
        sound::play_at(0x4e, y, x);
    }
}

/// FUN_1070_b8f9 / b936's recoil: |field| + 1 (to 0x14), negated unless `up`.
fn recoil(o: &mut crate::objects::Obj, f: usize, up: bool) {
    let mut v = o.w(f).abs();
    if v < 0x14 {
        v += 1;
    }
    o.set(f, if up { v } else { -v });
}

/// RangesOverlap(a, b, c, d).
fn ranges(a: i32, b: i32, c: i32, d: i32) -> bool {
    a.max(b) >= c.min(d) && a.min(b) <= c.max(d)
}

/// FUN_1070_b9d3's draw (the body at +0x36, the y the update ended with).
pub fn draw(p: &mut crate::render::Painter, o: &crate::objects::Obj) {
    let (x, ly) = (o.w(0xe), o.w(0x36));
    let f = p.frame;
    if o.w(0x2e) < 1 {
        p.mode = 0x10;
        p.scaled_at(600, 0xa0, ((f / 4) % 0xd) as i32 + 0x6b00, x + 3, ly - 0x24);
        p.flame(700, 0x100, f, x + 0x19, ly - 0x24);
        p.flame(500, 0x1c2, f, x - 7, ly - 0x20);
        p.mode = 0;
        if o.w(0x40) < 0x4c {
            p.at(o.w(0x40) / 4 + 0x6e00, x + 8, ly - 0x24);
        }
    } else if o.w(0x30) < 1 {
        p.at(((f >> 4) & 1) as i32 + 0x5946, x, ly);
    }
    // The body parts through PalMixLight row 0x80 when lit by a hit.
    p.mix = crate::render::Mix::Light;
    p.colour = 0x80;
    if o.w(0x24) > 0 {
        p.mode = 0x24;
    }
    p.at(0x590b + (o.w(0x2e) < 0x401) as i32 + (o.w(0x2e) < 1) as i32, x, ly);
    let gun = o.w(0x3c).abs() / 4;
    if gun > 0 {
        p.at(gun + 0x590f, x, ly);
    }
    p.mode = 0;
    if o.w(0x30) > -0x400 {
        if o.w(0x26) > 0 {
            p.mode = 0x24;
        }
        if o.w(0x30) < 1 {
            let reach = o.w(0x46);
            p.at((o.uw(0x48) >> 1) as i32 + 0x593c, x - reach, ly);
            let right = p.clip.2;
            p.clip.2 = right.min(x - 0xc - p.cam.0);
            p.at(0x594a, x - (reach >> 3), ly);
            for i in 0..4 {
                p.at(i + 0x5942, x - ((4 - i) * reach) / 4, ly);
            }
            p.clip.2 = right;
            p.mode = 0x28;
            p.at(0x1b08, x, ly);
        } else {
            p.at(0x590a, x, ly);
            p.at(o.w(0x3a).abs() / 4 + 0x5919, x, ly);
        }
    }
    p.mode = 0;
    let y = o.w(0x10);
    if o.w(0x32) < 1 {
        p.at(0x590e, x, y);
    } else {
        if o.w(0x28) > 0 {
            p.mode = 0x24;
        }
        p.at(((o.uw(0x2a) >> 1) % 10) as i32 + 0x5900, x, y);
        if o.w(0x32) < 0x401 {
            p.at(0x590f, x, y);
        }
    }
    p.mode = 0;
    p.mix = crate::render::Mix::Level;
    p.colour = 0;
    if o.w(0x2e) > 0 {
        p.mode = 0x10;
        p.at((o.w(0x34) < 1) as i32 + 0x5923, x, ly);
        p.mode = 0;
    }
}
