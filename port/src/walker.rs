//! The walker with a gunner on top (record type 0x140a, FUN_1070_4051): a mount (+0x34 hit
//! points) that patrols and a rider (+0x32) whose gun follows the hero. Either can be shot away.

use crate::actors::Env;
use crate::level::Level;
use crate::particles::Particles;
use crate::rng::PascalRandom;
use crate::sprites;

/// FUN_1040_6120: a puff of smoke with the current delay.
fn puff(ps: &mut Particles, y: i32, x: i32) {
    ps.spawn_seq(0x6800, 2, 0xf, y, x, 1);
}

/// FUN_1070_01ae(spread, count, y, x): armour plates in a fan.
pub fn plates(ps: &mut Particles, rng: &mut PascalRandom, spread: i32, count: i32, y: i32, x: i32) {
    ps.p.hits = 0x8000;
    ps.p.on_death = 0x10;
    let n = count - 1;
    for i in 0..=n {
        let s = rng.random(3) + 0x5137;
        ps.spawn(s, y, x);
        let a = i * ((spread << 1) / n) + (0x560 - spread) + rng.random(0x81);
        let v = rng.random(0x11) + 0x40;
        ps.throw(9, v, a);
    }
    ps.p.on_death = 0;
    ps.p.hits = 0;
}

fn visible(env: &Env, x: i32) -> bool {
    0 < (x + 0x20) - env.camera.0 && (x - 0x20) - env.camera.0 < 0x13f
}

impl Level {
    /// FUN_1070_3d4c: turn round.
    fn walker_turn(&mut self, id: usize, dir: i32) {
        let o = self.o(id);
        if o.w(0x20) != dir {
            o.set(0x20, dir);
            o.set(0x2c, 0x2e);
            o.set(0x2a, 0);
        }
    }

    /// FUN_1070_3f4d: the mount blows up.
    fn walker_mount_dies(&mut self, id: usize, env: &mut Env) {
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let ps = &mut self.particles;
        let n = env.rng.random(4) + 3;
        plates(ps, env.rng, 0x140, n, y - 10, x);
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x11;
        ps.spawn(0x562f, y - 10, x);
        ps.last_mut().m.set(0x26, 4);
        ps.throw(9, 0x60, 0x5c0);
        ps.p.draw = 1;
        ps.spawn(0x5630, y - 6, x + 8);
        ps.throw(9, 0x70, 0x500);
        ps.p.draw = 0;
        ps.spawn(0x5630, y - 6, x - 8);
        ps.throw(9, 0x70, 0x640);
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.spawn_seq(0x6900, 2, 0x19, y - 8, x, 1); // effect 23
        env.fx.shakes.push((0x24, 0xc));
    }

    /// FUN_1070_3d85: the rider blows up at `y`.
    fn walker_rider_dies(&mut self, id: usize, y: i32, env: &mut Env) {
        let x = self.o(id).w(0xe);
        let solid = crate::hero::Hero::solidity_all(env.world, self, y + 1, x);
        let ps = &mut self.particles;
        if solid & 0x201 == 0 {
            ps.spawn_seq(0x6400, 2, 0x24, y - 6, x, 1); // effect 7
            ps.line(2, 0x5a0);
        } else {
            ps.p.colour = 0x10;
            ps.spawn_seq(0x6f00, 3, 0x18, y, x, 1);
            ps.p.colour = 0;
        }
        // FUN_1070_1428: three puffs of smoke.
        for (d, dy, dx) in [(100, -0xf, 2), (0x3c, -1, 10), (0x28, -9, -0x10)] {
            ps.p.delay = d;
            let (py, px) = (y + dy, x + dx);
            puff(ps, py, px - 4);
            ps.p.delay += 7;
            puff(ps, py - 5, px + 1);
            ps.p.delay += 4;
            puff(ps, py + 5, px - 2);
            ps.p.delay += 0x13;
            puff(ps, py - 1, px);
        }
        ps.p.delay = 0;
        let n = env.rng.random(4) + 2;
        plates(ps, env.rng, 0xf0, n, y - 8, x);
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x1d;
        ps.p.callback = 0x1da7;
        ps.spawn(0x5637, y - 8, x - 2);
        ps.throw(9, 0x70, 0x5f0);
        ps.p.on_death = 0x11;
        ps.p.callback = 0x17f4;
        ps.spawn(0x5633, y - 8, x + 2);
        ps.last_mut().m.set(0x26, 4);
        ps.throw(9, 0x70, 0x580);
        ps.p.callback = 0;
        ps.p.draw = 1;
        ps.spawn(0x5631, y - 2, x + 8);
        ps.throw(9, 0x60, 0x520);
        ps.p.draw = 0;
        ps.spawn(0x5632, y - 2, x - 8);
        ps.throw(9, 0x60, 0x620);
        ps.p.on_death = 0;
        ps.p.hits = 0;
        env.fx.shakes.push((0x24, 0xe));
        env.fx.kills += 1;
    }

    /// FUN_1070_4051.
    pub(crate) fn walker(&mut self, id: usize, env: &mut Env) {
        for off in [0x28, 0x26] {
            let o = self.o(id);
            if o.w(off) > 0 {
                o.set(off, o.w(off) - 1);
            }
        }
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let mut muzzle = if self.o(id).w(0x34) > 0 { y - 0x24 } else { y - 8 };
        self.patrol_box(id, y + 0x40, x + 0x40, y - 0x60, x - 0x40);
        if self.actor_physics(id, env, 0x10, 0x1b, 8) >= 1 {
            if self.o(id).w(0x32) > 0 {
                self.walker_rider_dies(id, muzzle, env);
            }
            if self.o(id).w(0x34) > 0 {
                self.walker_mount_dies(id, env);
            }
            self.remove_current(id);
            return;
        }
        let h = env.h;
        let (hx, hy, hh) = (h.x, h.y, env.hero.2);
        let mut rider = self.o(id).w(0x38);
        let mut mount = 0x5602;
        let mut aim = 0x7fff;
        // The rider (and what he shoots at).
        if self.o(id).w(0x32) < 1 {
            if self.o(id).w(0x2c) < 0x15 {
                self.o(id).set(0x2c, 0);
            }
            if self.o(id).w(0x2a) % 0x10 == 1 && env.rng.random(6) == 0 {
                let dy = env.rng.random(5);
                self.particles.spawn_seq(0x1928, 3, 4, (y - 0x20) - dy, x, 1);
                env.rng.random(0x14);
            }
        } else {
            let (facing, turn) = (self.o(id).w(0x20), self.o(id).w(0x2c));
            let (ex, ey) = (x, muzzle);
            let ty = hy - (hh >> 4);
            let target;
            if ((facing * 0x1a + ex < hx) == (facing > 0))
                && turn < 1
                && self.line_of_sight(env, ty, hx, ey, ex)
            {
                let mut a = crate::trig::atan2(ty - ey, hx - ex);
                if facing > 0 {
                    a = (0x21c0 - a) % 0x1680;
                }
                let mut t = a;
                if a < 0x871 {
                    t = 0x871;
                } else if self.o(id).w(0x34) < 1 {
                    if a > 0xb40 {
                        if a < 0xcc0 {
                            a = 0xb40;
                        }
                        t = 0xb40;
                    }
                } else if a > 0xe10 {
                    t = 0xe10;
                }
                aim = a;
                target = t;
            } else {
                target = 0xb40;
            }
            let o = self.o(id);
            let mut g = o.w(0x36);
            if target < g {
                g -= 0x10;
                if g < target {
                    g = target;
                }
            } else if g < target {
                g += 0x10;
                if target < g {
                    g = target;
                }
            }
            o.set(0x36, g);
            rider = ((0xe10 - g) * 9) / 0x5a0 + 0x5617;
        }
        // The mount.
        let mut off = 0;
        let mut flip_m = false;
        if self.o(id).w(0x34) > 0 {
            let turn = self.o(id).w(0x2c);
            if turn < 1 {
                let a = (self.o(id).w(0x2a) + 1) % 0x50;
                self.o(id).set(0x2a, a);
                mount = a / 4 + 0x5602;
                if a % 0x28 == 0 && self.o(id).w(0x32) > 0 && visible(env, x) {
                    env.fx.shakes.push((6, 2));
                }
                if a % 2 == 0 {
                    let old = self.o(id).w(0x20);
                    self.patrol_step(id, env, 0x1b, old);
                    if old != self.o(id).w(0x20) {
                        self.o(id).set(0x2c, 0x2e);
                        self.o(id).set(0x2a, 0);
                    }
                }
            } else {
                self.o(id).set(0x2c, turn - 1);
                let t = turn - 1;
                if t < 0x2f && 0x1a < t {
                    let mut m = (0x2e - t) / 2;
                    if 4 < m {
                        m = 9 - m;
                    }
                    mount = m + 0x5624;
                } else {
                    mount = 0x5602;
                }
                if t < 0x14 {
                    let mut m = (0x13 - t) / 2;
                    if 4 < m {
                        m = 9 - m;
                    }
                    rider = m + 0x562a;
                }
            }
            let (facing, turn) = (self.o(id).w(0x20), self.o(id).w(0x2c));
            flip_m = facing < 0;
            if turn > 0x24 {
                flip_m = !flip_m;
            }
            off = sprites::hitboxes(mount)
                .iter()
                .find(|b| b.kind == 3)
                .map_or(0, |b| b.rect.1);
            let (y, x) = (self.o(id).w(0x10), self.o(id).w(0xe));
            self.particles.p.aim_level = 0;
            self.particles.p.aim_level2 = -1;
            let d = match sprites::hit_rect(mount, y, x, flip_m) {
                Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
                None => 0,
            };
            self.particles.p.aim_level = 0x100;
            self.particles.p.aim_level2 = 0x100;
            if d > 0 {
                let hp = self.o(id).w(0x34) - d;
                self.o(id).set(0x34, hp);
                if hp < 1 {
                    self.walker_mount_dies(id, env);
                    if self.o(id).w(0x32) < 1 {
                        self.remove_current(id);
                        return;
                    }
                    let o = self.o(id);
                    o.set(0x2c, 0);
                    o.set(0x2a, 0);
                    o.set(0x10, y + off);
                    off = 0;
                } else {
                    if self.particles.hit_mask != 0x48 && self.o(id).w(0x28) < 1 {
                        let facing = self.o(id).w(0x20);
                        let ps = &mut self.particles;
                        ps.p.delay = 0x1a;
                        puff(ps, y - 0xc, facing * 0x14 + x);
                        ps.p.delay = 0xc;
                        puff(ps, y - 0x10, x);
                        ps.p.delay = 0;
                        env.fx.shakes.push((0xc, 4));
                    }
                    self.o(id).set(0x28, 3);
                    if self.o(id).w(0x2c) < 1 && self.o(id).w(0x32) > 0 {
                        if hx < x - 0x12 {
                            self.walker_turn(id, -1);
                        } else if x + 0x12 < hx {
                            self.walker_turn(id, 1);
                        }
                    }
                }
            }
        }
        if self.o(id).w(0x3a) > 0 {
            let o = self.o(id);
            o.set(0x3a, o.w(0x3a) - 1);
        }
        let mut flip_r = false;
        if self.o(id).w(0x32) >= 1 {
            let (facing, turn) = (self.o(id).w(0x20), self.o(id).w(0x2c));
            flip_r = facing < 0;
            if turn > 9 {
                flip_r = !flip_r;
            }
            let (y, x) = (self.o(id).w(0x10), self.o(id).w(0xe));
            let mut d = match sprites::hit_rect(rider, y + off, x, flip_r) {
                Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
                None => 0,
            };
            if self.o(id).w(0x34) < 1 && self.stomp(rider, y + off, x, flip_r, env) > 0 {
                d += 0x10;
                self.particles.hit_mask = 0x48;
                self.o(id).set(0x10, y - 3);
                env.fx.shakes.push((0xc, 4));
            }
            if d > 0 {
                let hp = self.o(id).w(0x32) - d;
                self.o(id).set(0x32, hp);
                muzzle = self.o(id).w(0x10) + off;
                if hp < 1 {
                    self.walker_rider_dies(id, muzzle, env);
                    if self.o(id).w(0x34) < 1 {
                        if env.rng.random(3) == 0 {
                            let (y, x) = (self.o(id).w(0x10), self.o(id).w(0xe));
                            // FUN_1040_63c9: a flash that lingers.
                            self.particles.p.callback = 0x19f8;
                            self.particles.p.colour = 0x10;
                            self.particles.spawn_seq(0x6a00, 1, 0x5bc, y, x, 0);
                            self.particles.p.colour = 0;
                            self.particles.p.callback = 0;
                        } else {
                            let (y, x) = (self.o(id).w(0x10), self.o(id).w(0xe));
                            crate::alien::smoke_sprite(&mut self.particles, y, x);
                        }
                        self.remove_current(id);
                        return;
                    }
                } else {
                    let mask = self.particles.hit_mask;
                    let facing = self.o(id).w(0x20);
                    if self.o(id).w(0x34) > 0 {
                        if mask != 0x48 && self.o(id).w(0x26) < 1 {
                            let ps = &mut self.particles;
                            ps.p.delay = 0x1a;
                            puff(ps, muzzle - 5, facing * 0x1e + x);
                            ps.p.delay = 0xc;
                            if env.rng.random(2) == 0 {
                                puff(ps, muzzle - 0xf, facing * 0xc + x);
                            } else {
                                puff(ps, muzzle - 9, x);
                            }
                            ps.p.delay = 0;
                        }
                        if self.o(id).w(0x2c) < 1 {
                            if hx < x - 0x12 {
                                self.walker_turn(id, -1);
                            } else if x + 0x12 < hx {
                                self.walker_turn(id, 1);
                            }
                        }
                    }
                    if mask != 0x48 && self.o(id).w(0x26) < 1 {
                        env.fx.shakes.push((0xc, 4));
                    }
                    self.o(id).set(0x26, 3);
                }
            }
            // Firing.
            let g = self.o(id).w(0x36);
            if aim <= g && g - 0x180 <= aim {
                if self.o(id).b(0x3c) == 0 {
                    self.o(id).set_b(0x3c, 1);
                    self.o(id).set(0x3a, 0x6e);
                }
                let cd = self.o(id).w(0x3a);
                let now = self.particles.now;
                if visible(env, x) && (cd < 1 || cd > 0x4c) && now % 8 == 0 {
                    let facing = self.o(id).w(0x20);
                    let mut a = g;
                    if facing > 0 {
                        a = (0x21c0 - a) % 0x1680;
                    }
                    let sx = x + (0x1c * crate::trig::sin((a + 0x5a0) % 0x1680)) / 0x4000;
                    let sy = (self.o(id).w(0x10) + off - 8) - (0x1c * crate::trig::sin(a % 0x1680)) / 0x4000;
                    let ps = &mut self.particles;
                    ps.p.hits = 0x20;
                    ps.p.damage = 0x10;
                    ps.p.on_death = 0x3e;
                    ps.spawn(0x5638 + (now % 0x10 == 0) as i32, sy, sx);
                    ps.line(0x60, a);
                    ps.p.on_death = 0;
                    ps.p.damage = 0x10;
                    ps.p.hits = 0;
                    ps.p.colour = 0x28;
                    ps.spawn_seq(0x1b04, 2, 2, sy, sx, 1);
                    ps.p.colour = 0;
                    ps.spawn_seq(0x190a, 3, 2, sy, sx, 1);
                    let dir = if facing < 0 { -1 } else { 1 };
                    ps.p.group = 0;
                    let r = env.rng.random(0x101);
                    crate::bigali::shell_of(ps, 0x1924, 0x50, dir * (r + 0x80) + 0x5a0, sy, sx);
                    ps.p.group = 1;
                    self.o(id).set(0x2e, 4);
                    if self.o(id).w(0x3a) == 0 {
                        self.o(id).set(0x3a, 0x8c);
                    }
                }
            }
            // The hero standing on the gunner is carried (FUN_1070_00b6).
            let (facing, x) = (self.o(id).w(0x20), self.o(id).w(0xe));
            let cx = facing * 3 + x;
            let top = self.o(id).w(0x10) + off - 0x10;
            let last = self.o(id).w(0x22);
            self.stand_on(env, top, last, cx + 0x12, cx - 0x12);
            self.o(id).set(0x22, top);
        }
        if self.o(id).w(0x2e) > 0 {
            let o = self.o(id);
            o.set(0x2e, o.w(0x2e) - 1);
        }
        let _ = (flip_m, flip_r);
    }

    /// FUN_1070_00b6(top, last top, right, left): the hero landing on a moving top stays on it.
    pub(crate) fn stand_on(&mut self, env: &mut Env, top: i32, last: i32, right: i32, left: i32) -> bool {
        let h = env.h;
        let ok = (h.state >= 1 && h.vy >= 0) || (h.state < 1 && !(-5..=-1).contains(&h.state));
        let (lo, hi) = (h.y.min(h.prev_y), h.y.max(h.prev_y));
        let (lo2, hi2) = (top.min(last), top.max(last));
        if left <= h.x && h.x <= right && ok && lo <= hi2 && lo2 <= hi {
            env.fx.hero_stand = Some(top);
            return true;
        }
        false
    }
}
