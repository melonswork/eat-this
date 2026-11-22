//! The gunship (record type 0x13e2, VMT 0xc98): FUN_1070_1b00 with FUN_1070_1487/1634 (flying
//! its rectangle), 16ee (hit hard: smoking), 17f7 (falling), 18e0 (crashed: a wreck, VMT 0xca8,
//! FUN_1070_243c/2379). Fields:
//!
//! | off | field |
//! |-----|-------|
//! | 0a  | armed (record flags) |
//! | 0e, 12 | x, y in 1/16 px (longs) / 16, 18 x, y |
//! | 20  | pause at a turn / 22, 24 horizontal, vertical speed (1/16 px) |
//! | 26  | rotor sprite / 28..2e the rectangle it flies (left, top, right, bottom) |
//! | 32  | state: 0 flying, 1 smoking, 2 falling, 3 crashed |
//! | 34  | hit flash / 36 smoking time / 38 hit points / 3a bobbing |
//! | 3c  | gun angle / 3e the angle to the hero / 40 burst counter (negative: pause) |

use crate::actors::Env;
use crate::level::Level;
use crate::objects::Obj;
use crate::sprites;
use crate::trig;

/// The wreck's kind (it has no record type: FUN_1068_111e(0, 0, 0xca8)).
pub const WRECK: u16 = 0xca8;

impl Level {
    pub fn gunship(&mut self, id: usize, env: &mut Env) {
        let now = env.frame as i32;
        {
            let o = self.o(id);
            if o.w(0x34) > 0 {
                o.set(0x34, o.w(0x34) - 1);
            }
        }
        if std::env::var("ET_OBJ").is_ok() {
            let o = self.o(id);
            let ws: Vec<String> = (0..0x20).map(|i| o.w(i * 2).to_string()).collect();
            eprintln!("OBJ {} {}", env.frame + 1, ws.join(" "));
        }
        let state = self.o(id).w(0x32);
        let sprite;
        match state {
            0 => {
                self.ship_fly(id, now);
                let o = self.o(id);
                o.set(0x26, (now / 4) % 0x15 + 0x4d02);
                if o.w(10) == 0 {
                    sprite = o.w(0x26) + 0x1e;
                } else {
                    sprite = self.ship_aim_and_fire(id, env);
                }
                ship_loop(id, self.o(id), 0x2e);
            }
            1 => {
                self.ship_fly(id, now);
                let o = self.o(id);
                if o.w(0x38) < 0x71 {
                    if o.w(0x36) > 0x46 && o.w(0x36) % 0xc == 0 && env.rng.random(4) == 0 {
                        let (x, y) = (o.w(0x16), o.w(0x18));
                        let r = env.rng.random(4);
                        self.particles.spawn_seq(0x1928, 3, 4, (y - 0x12) - r, x, 1);
                        let p = env.rng.random(0x14);
                        crate::sound::play_at_pitch(0x46, p + 0x100, y, x);
                    }
                    let o = self.o(id);
                    o.set(0x36, o.w(0x36) + 1);
                    if o.w(0x36) > 0x45f {
                        self.ship_falls(id, env);
                        self.o(id).set(0x38, 0x40);
                    }
                }
                let o = self.o(id);
                o.set(0x26, (now / 2) % 0x15 + 0x4d02);
                sprite = 0x4e09;
                ship_loop(id, self.o(id), 0x2e);
            }
            2 => {
                let o = self.o(id);
                if o.w(0x22) == 0 {
                    o.set_l(0x12, o.l(0x12).wrapping_add(4));
                } else {
                    o.set_l(0xe, o.l(0xe).wrapping_add(o.w(0x22)));
                    o.set_l(0x12, o.l(0x12).wrapping_add(2));
                }
                o.set(0x16, o.l(0xe) / 0x10);
                o.set(0x18, o.l(0x12) / 0x10);
                o.set(0x26, now % 0x15 + 0x4d02);
                let (x, y, vx) = (o.w(0x16), o.w(0x18), o.w(0x22));
                if now % 8 == 0 {
                    let ps = &mut self.particles;
                    if vx == 0 {
                        let r = env.rng.random(0xb);
                        ps.spawn_seq(0x6801, 5, 0xe, y - 0x15, x - 5 + r, 1);
                        let a = env.rng.random(0x161) + 0x4f0;
                        ps.line(4, a);
                    } else {
                        let r = env.rng.random(0xc);
                        ps.spawn_seq(0x6801, 5, 0xe, y - 0x17 + r, x, 1);
                    }
                }
                sprite = 0x4e09;
                let px = vx.signum() * 6 + x;
                let solid = |y: i32, x: i32| crate::hero::Hero::solidity_all(env.world, self, y, x);
                if solid(y, x) & 0x201 != 0 || solid(y - 0xc, px) & 2 != 0 || solid(y + 4, px) & 2 != 0 {
                    self.ship_crash(id, env);
                    return;
                }
                ship_loop(id, self.o(id), 0x2c);
            }
            _ => sprite = 0x7fff,
        }
        // For the draw pass: the rotor (+0x26) and the gun (the flash is applied when drawing).
        {
            let o = self.o(id);
            let (x, y) = (o.w(0x16), o.w(0x18));
            let falling = o.w(0x32) == 2;
            let mut looks = vec![Obj::look(o.w(0x26), x, y)];
            if sprite != 0x7fff {
                looks.push(Obj::look(sprite, x, y));
            }
            if falling {
                // Mode 0x24 with colour 1170:1b14 (0 at level start): darkened while it falls.
                for l in &mut looks {
                    l.mode = 0x24;
                }
            }
            o.looks = looks;
        }
        // The shots hitting it (FUN_1050_5960 with its gun's or its body's sprite).
        let (x, y) = (self.o(id).w(0x16), self.o(id).w(0x18));
        if self.o(id).w(0x1c) == 2 {
            self.particles.p.aim_level2 = 0xf6;
        }
        let d = match sprites::hit_rect(sprite, y, x, false) {
            Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x1059),
            None => 0,
        };
        self.particles.p.aim_level2 = 0x100;
        if d <= 0 {
            return;
        }
        let o = self.o(id);
        o.set(0x38, o.w(0x38) - d);
        if o.w(0x32) < 1 && o.w(0x38) < 0x81 {
            self.ship_smokes(id, env);
        }
        let o = self.o(id);
        if o.w(0x32) < 2 && o.w(0x38) < 0x41 {
            self.ship_falls(id, env);
        }
        if self.o(id).w(0x38) < 1 {
            self.ship_crash(id, env);
            return;
        }
        let mask = self.particles.hit_mask;
        let o = self.o(id);
        if mask != 0x48 && o.w(0x34) < 1 {
            let vx = o.w(0x22);
            let ps = &mut self.particles;
            // FUN_1040_6120 x3: smoke puffs.
            ps.spawn_seq(0x6800, 2, 0xf, y - 0x10, x, 1);
            ps.p.delay = 0x24;
            ps.spawn_seq(0x6800, 2, 0xf, y - 0xe, (vx * 0x3a) / 0x10 + x, 1);
            ps.p.delay = 0x18;
            ps.spawn_seq(0x6800, 2, 0xf, y - 0x1a, (vx * 0x2a) / 0x10 + x, 1);
            ps.p.delay = 0;
            env.fx.shakes.push((0x10, 8));
            crate::sound::play_at(0x49, y, x);
        }
        let o = self.o(id);
        if o.w(0x38) < 0x71 {
            unbounded(o);
        }
        o.set(0x34, 3);
    }

    /// FUN_1070_1634 (with FUN_1070_1487): along the rectangle's edges, pausing 0x78 frames at
    /// each end, bobbing (+0x3a) while flying sideways.
    fn ship_fly(&mut self, id: usize, now: i32) {
        let o = self.o(id);
        if o.w(0x20) < 1 {
            if o.w(0x24) == 0 {
                o.set_l(0xe, o.l(0xe).wrapping_add(o.w(0x22)));
                let x = o.l(0xe) / 0x10;
                let turn = if o.w(0x22) < 0 { x <= o.w(0x28) } else { o.w(0x2c) <= x };
                if turn {
                    o.set(0x22, -o.w(0x22));
                    o.set(0x20, 0x78);
                    let y = o.l(0x12) / 0x10;
                    o.set(0x24, if o.w(0x2e) <= y { -5 } else { 5 });
                }
            } else {
                o.set_l(0x12, o.l(0x12).wrapping_add(o.w(0x24)));
                let y = o.l(0x12) / 0x10;
                let stop = if o.w(0x24) < 0 { y <= o.w(0x2a) } else { o.w(0x2e) <= y };
                if stop {
                    o.set(0x24, 0);
                }
            }
        } else {
            o.set(0x20, o.w(0x20) - 1);
        }
        o.set(0x16, o.l(0xe) / 0x10);
        if o.w(0x24) != 0 {
            if o.w(0x3a) > 0 {
                o.set(0x3a, o.w(0x3a) - 1);
            }
        } else if o.w(0x3a) < 0x80 {
            o.set(0x3a, o.w(0x3a) + 1);
        }
        let a = (now % 0x3c) * 6 * 0x10;
        let bob = (trig::sin(a) * o.w(0x3a)) / 0x80000;
        o.set(0x18, o.l(0x12) / 0x10 + bob);
    }

    /// The armed gunship turns its gun (0x30 a frame at most) towards the hero while it sees
    /// him, and fires bursts of three. Returns the gun's sprite (its hit box).
    fn ship_aim_and_fire(&mut self, id: usize, env: &mut Env) -> i32 {
        let h = env.h;
        let (x, y) = (self.o(id).w(0x16), self.o(id).w(0x18));
        let eye = y - 0x17;
        let (sx, sy) = (x - env.camera.0, eye - env.camera.1);
        let target = h.y - (h.height >> 1);
        let sees = (0..=0x13f).contains(&sx)
            && (0..=199).contains(&sy)
            && self.line_of_sight(env, target, h.x, eye, x);
        let o = self.o(id);
        if sees {
            o.set(0x3e, trig::atan2(target - eye, h.x - x));
        }
        let d = ((o.w(0x3e) - o.w(0x3c)) + 0x21c0) % 0x1680;
        if d < 0xb40 {
            let step = 0x30.min(0xb40 - d);
            o.set(0x3c, ((o.w(0x3c) + 0x1680) - step) % 0x1680);
        } else {
            let step = 0x30.min(d - 0xb40);
            o.set(0x3c, (step + o.w(0x3c)) % 0x1680);
        }
        o.set(0x40, o.w(0x40) + 1);
        // FUN_10f8_00bf: the angle still to turn.
        let off = ((o.w(0x3c) - o.w(0x3e)) + 0x21c0) % 0x1680 - 0xb40;
        if (!sees || off.abs() > 0x20) && o.w(0x40) == 1 {
            o.set(0x40, 0);
        }
        if o.w(0x40) == -0x50 {
            crate::sound::play_at(0x84, y, x);
        }
        if o.w(0x40) > 0 && o.w(0x40) % 7 == 1 {
            let a = o.w(0x3c);
            let mx = x + (0x13 * trig::cos(a)) / 0x4000;
            let my = eye - (0x13 * trig::sin(a)) / 0x5000;
            let ps = &mut self.particles;
            ps.p.group = 0;
            ps.p.hits = 0x1a2;
            ps.p.damage = 0x20;
            ps.p.on_death = 0x3e;
            ps.p.callback = 0x2d2;
            ps.spawn(0x191e, my, mx);
            ps.line(0x50, a);
            ps.p.callback = 0;
            ps.p.on_death = 0;
            ps.p.damage = 0x10;
            ps.p.hits = 0;
            ps.p.group = 1;
            ps.p.colour = 0x28;
            ps.spawn_seq(0x1b04, 2, 2, my, mx, 1);
            ps.p.colour = 0;
            ps.spawn_seq(0x190a, 2, 2, my, mx, 1);
            let sa = env.rng.random(0x181) + 0x4e0;
            crate::bigali::shell_of(ps, 0x1924, 0x40, sa, y - 0x1b, x);
            crate::sound::play_at(0x80, my, mx);
            env.fx.shakes.push((4, 3));
            let o = self.o(id);
            if o.w(0x40) > 0xe {
                o.set(0x40, -0x8c);
            }
        }
        let o = self.o(id);
        0x4d4a - ((o.w(0x3c) + 0x510) % 0x1680) / 0x120
    }

    /// FUN_1070_16ee: hit hard: bits fly off, smoke.
    fn ship_smokes(&mut self, id: usize, env: &mut Env) {
        let o = self.o(id);
        o.set(0x32, 1);
        let (x, y) = (o.w(0x16), o.w(0x18));
        env.fx.kills += 1;
        let ps = &mut self.particles;
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x11;
        ps.spawn(-8, y - 0x15, x - 3);
        let a = env.rng.random(0x140) + 0x640;
        ps.throw(9, 0x40, a);
        ps.p.draw = 1;
        ps.spawn(-8, y - 0x15, x + 3);
        let a = 0x500 - env.rng.random(0x140);
        ps.throw(9, 0x40, a);
        ps.p.draw = 0;
        ps.spawn(0x4e08, y - 0x15, x);
        let a = 0x640 - env.rng.random(0x141);
        ps.throw(10, 0x48, a);
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
        ps.p.colour = 0x10;
        crate::particles::smoke(ps, env.rng, y - 0x16, x);
        ps.p.colour = 0;
        env.fx.shakes.push((0x18, 0xc));
        crate::sound::play_at(0x3d, y, x);
    }

    /// FUN_1070_17f7: it starts falling (straight down if it was climbing or sinking).
    fn ship_falls(&mut self, id: usize, env: &mut Env) {
        crate::sound::stop(crate::sound::key::obj(id, 0x30));
        let o = self.o(id);
        o.set(0x32, 2);
        let (x, y) = (o.w(0x16), o.w(0x18));
        let ps = &mut self.particles;
        for (delay, dy, dx) in [(0x1e, -10, 6), (0xb, -0xc, -2), (7, -0x16, 2), (0, -0x14, -6)] {
            ps.p.delay = delay;
            ps.spawn_seq(0x6800, 2, 0xf, y + dy, x + dx, 1);
        }
        ps.p.delay = 0;
        env.fx.shakes.push((0x18, 0xc));
        crate::sound::play_at(0x48, y, x);
        // FUN_1040_6156: a flash.
        ps.p.colour = 0x28;
        ps.p.group = 0;
        ps.spawn_seq(0x1b04, 4, 2, y - 0xc, x, 1);
        ps.p.colour = 0;
        ps.p.group = 1;
        let o = self.o(id);
        if o.w(0x24) != 0 {
            o.set(0x22, 0);
        }
    }

    /// FUN_1070_18e0: down: smoke, wreckage, a blast, and a wreck object in its place.
    fn ship_crash(&mut self, id: usize, env: &mut Env) {
        let o = self.o(id);
        o.set(0x32, 3);
        let (x, y) = (o.w(0x16), o.w(0x18));
        let ps = &mut self.particles;
        // FUN_1040_6405: four puffs, each later.
        let puffs = |ps: &mut crate::particles::Particles, y: i32, x: i32| {
            ps.spawn_seq(0x6800, 2, 0xf, y, x - 4, 1);
            ps.p.delay += 7;
            ps.spawn_seq(0x6800, 2, 0xf, y - 5, x + 1, 1);
            ps.p.delay += 4;
            ps.spawn_seq(0x6800, 2, 0xf, y + 5, x - 2, 1);
            ps.p.delay += 0x13;
            ps.spawn_seq(0x6800, 2, 0xf, y - 1, x, 1);
        };
        ps.p.delay = 0x14;
        puffs(ps, y - 7, x);
        // FUN_1070_1428.
        for (delay, dy, dx) in [(100, -0xf, 2), (0x3c, -1, 10), (0x28, -9, -0x10)] {
            ps.p.delay = delay;
            puffs(ps, y - 8 + dy, x + dx);
        }
        ps.p.delay = 0;
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x11;
        ps.spawn(-9, y - 0xe, x - 3);
        let a = env.rng.random(0x140) + 0x640;
        ps.throw(9, 0x40, a);
        ps.spawn(-9, y - 0x10, x);
        let a = 0x6e0 - env.rng.random(0x280);
        ps.throw(9, 0x50, a);
        ps.p.draw = 1;
        ps.spawn(-9, y - 0xe, x + 3);
        let a = 0x500 - env.rng.random(0x140);
        ps.throw(9, 0x40, a);
        ps.p.draw = 0;
        ps.spawn(0x4e08, y - 0x10, x);
        let a = 0x6e0 - env.rng.random(0x280);
        ps.throw(10, 0x50, a);
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
        // Effect 7 (FUN_1040_0897).
        ps.spawn_seq(0x6400, 2, 0x24, y - 0x16, x, 1);
        ps.line(2, 0x5a0);
        // FUN_1050_5be8(0x60, 5, 4, ...): the tiles around, and a blast.
        self.explode_area(env.world, 0x60, 4, y + 0x16, x + 0x16, y - 0x16, x - 0x16, env.rng, env.fx);
        self.particles.blast(2, 0x60, 5, 0x10, 0x16, -0x1c, -0x16, y + 6, x);
        env.fx.shakes.push((0x2a, 0x10));
        crate::sound::play_at(0x4a, y, x);
        crate::sound::play_at(0x3d, y, x);
        let mut w = Obj::new(WRECK, 0xca8, 0x20);
        w.set(0xc, 1);
        w.set(0xe, x);
        w.set(0x10, y);
        unbounded(&mut w);
        let wid = self.objects.len();
        self.objects.push(Some(w));
        let (s, l) = self.walking;
        self.lists[s][l].push(wid);
        self.remove_current(id);
    }

    /// FUN_1070_243c: the wreck. Falls; shot (more than 3) or stood on, it goes up in smoke.
    pub fn wreck(&mut self, id: usize, env: &mut Env) {
        let o = self.o(id);
        let (x, y) = (o.w(0xe), o.w(0x10));
        o.set(2, x - 0x20);
        o.set(6, x + 0x20);
        o.set(4, y - 0x10);
        o.set(8, y + 0x10);
        if self.actor_physics(id, env, 4, 7, 5) >= 1 {
            self.wreck_burns(id, env);
            return;
        }
        if self.o(id).w(0x1a) < 1 {
            let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
            let d = match sprites::hit_rect(0x4e13, y, x, false) {
                Some((b, r, t, l)) => self.particles.hit_test_low(b, r, t, l, 0x11fb),
                None => 0,
            };
            if d > 3 || self.stomp(0x4e13, y, x, false, env) > 0 {
                self.wreck_burns(id, env);
                return;
            }
            if self.o(id).w(0x1c) == 5 {
                self.slide_on_slope(id);
            }
        }
    }

    /// FUN_1070_23db.
    fn wreck_burns(&mut self, id: usize, env: &mut Env) {
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let ps = &mut self.particles;
        crate::alien::smoke_sprite(ps, y, x);
        ps.p.colour = 0x10;
        crate::particles::smoke(ps, env.rng, y - 7, x);
        ps.p.colour = 0;
        crate::sound::play_at(0x4b, y, x);
        env.fx.shakes.push((0x18, 8));
        self.remove_current(id);
    }

    /// FUN_1070_2379 (the wreck's move hook): landing after a fall raises dust (effect 2).
    /// FUN_1070_d183: the springboard's move hook, a wreck's without the sound-less puff of 66d4.
    pub fn spring_after_move(&mut self, id: usize) {
        let o = self.o(id);
        if o.w(0x1c) == 0 {
            if o.w(0x1a) == 0 {
                o.set(0x18, o.w(0x10));
                o.set(0x14, -0x7fff);
                o.set(0x16, 0x7fff);
            }
            o.set(0x1a, o.w(0x1a) + 1);
            unbounded(o);
        } else {
            if o.w(0x1a) > 0xc {
                let (x, y) = (o.w(0xe), o.w(0x10));
                // FUN_1040_66d4: dust where it landed.
                let ps = &mut self.particles;
                ps.p.colour = 0x10;
                ps.spawn_seq(0x6d00, 2, 0x13, y, x, 1);
                ps.p.colour = 0;
            }
            self.o(id).set(0x1a, 0);
        }
    }

    pub fn wreck_after_move(&mut self, id: usize) {
        let o = self.o(id);
        if o.w(0x1c) == 0 {
            if o.w(0x1a) == 0 {
                o.set(0x18, o.w(0x10));
                o.set(0x14, -0x7fff);
                o.set(0x16, 0x7fff);
            }
            o.set(0x1a, o.w(0x1a) + 1);
            unbounded(o);
        } else {
            if o.w(0x1a) > 0xc {
                let (x, y) = (o.w(0xe), o.w(0x10));
                // Effect 2 (FUN_1040_054b).
                let ps = &mut self.particles;
                ps.p.colour = 0x10;
                ps.spawn_seq(0x65a, 4, 7, y, x, 1);
                ps.p.colour = 0;
            }
            self.o(id).set(0x1a, 0);
        }
    }
}

/// The rotor's loop (handle +0x30) while it is in earshot.
fn ship_loop(id: usize, o: &Obj, snd: usize) {
    if crate::sound::audible(o.w(0x18), o.w(0x16)) {
        crate::sound::priority(0x11f4);
        crate::sound::keep(crate::sound::key::obj(id, 0x30), snd);
    }
}

pub(crate) fn unbounded(o: &mut Obj) {
    o.set(2, -0x7fff);
    o.set(4, -0x7fff);
    o.set(6, 0x7fff);
    o.set(8, 0x7fff);
}
