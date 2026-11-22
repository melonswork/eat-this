//! The carrier (icon 61600, kind 0x14be, VMT 0xc84, FUN_1070_0cce, sprites 0x5cxx): a hovering
//! platform that flies towards the hero with the aliens and bots of the icons to its right on
//! board, dropping each when the hero comes within its distance (or stands on it). Shot down or
//! stopped by a wall it falls; landing it crashes (dust, plates, blast), hitting something hard
//! on the way it blows up.
//!
//! | off | field |
//! |-----|-------|
//! | 20  | 0 flying, 1 crashed / 22 frame counter / 24 direction (0xff: not yet) |
//! | 26  | hit points (0xc0; <= 0 falling) / 28 hit flash / 2a passengers |
//! | 2c + 10 i | passenger i: kind (-1 gone), dx, dy, sprite, drop distance |
//! | 5e  | the y it is drawn at (bobs while flying) / 60 its sound loop |

use crate::actors::Env;
use crate::level::Level;
use crate::sprites;

pub const KIND: u16 = 0x14be;

impl Level {
    /// FUN_1070_0cce.
    pub(crate) fn carrier(&mut self, id: usize, env: &mut Env) {
        let h = env.h;
        let prev_top;
        let sprite;
        {
            let o = self.o(id);
            if o.w(0x28) > 0 {
                o.set(0x28, o.w(0x28) - 1);
            }
            prev_top = o.w(0x5e);
            o.set(0x5e, o.w(0x10));
        }
        if self.o(id).w(0x20) == 0 {
            let x_moved;
            {
                let o = self.o(id);
                if o.w(0x24) == 0xff {
                    o.set(0x24, if h.x - o.w(0xe) < 0 { -1 } else { 1 });
                    crate::sound::play_at(0x85, o.w(0x10), o.w(0x24) * 0x60 + o.w(0xe));
                }
                o.set(0x22, o.w(0x22) + 1);
                let c = o.w(0x22);
                // FUN_10f0_1317(c, 1, 6, 4): back and forth over four frames.
                let t = (c / 6) % 6 - 3;
                sprite = 0x5c00 + 3 - t.abs();
                o.set(0xe, o.w(0xe) + o.w(0x24) * 2);
                x_moved = o.w(0xe);
            }
            if self.o(id).w(0x26) > 0 {
                let o = self.o(id);
                let a = (o.w(0x22) as i16).wrapping_mul(10) % 0x168;
                if a < 0 {
                    eprintln!("warning: carrier bob index {a} (the original reads before the sine table)");
                }
                o.set(0x5e, o.w(0x5e) + (crate::trig::sin(a as i32 * 16) * 3) / 0x4000);
                let mut d = o.w(0xe) - h.x;
                if o.w(0x24) > 0 {
                    d = -d;
                }
                for i in 0..o.w(0x2a) {
                    if d <= self.o(id).w(0x34 + 10 * i as usize) {
                        self.carrier_drop(id, i, env);
                    }
                }
            }
            if self.actor_physics(id, env, 0x18, 0x1b, 9) > 0 {
                self.carrier_explodes(id, env);
                return;
            }
            let (x, top) = (self.o(id).w(0xe), self.o(id).w(0x5e));
            if self.stand_on(env, top - 9, prev_top - 9, x + 0x1d, x - 0x1d) {
                // (It also adds its step to gHeroDX, which HeroUpdate resets before using it.)
                for i in (0..self.o(id).w(0x2a)).rev() {
                    self.carrier_drop(id, i, env);
                }
                self.particles.p.aim_level2 = -1;
            }
            let o = self.o(id);
            let (x, y) = (o.w(0xe), o.w(0x10));
            if o.w(0x26) < 1 {
                if o.w(0x22) % 6 == 0 {
                    let ps = &mut self.particles;
                    ps.p.delay = 3;
                    ps.p.colour = 0x10;
                    crate::effects::effect(self, env.world, 0x17, y - 0x10, x, env.rng, env.fx);
                    let ps = &mut self.particles;
                    ps.p.colour = 0;
                    ps.p.delay = 0;
                }
            } else {
                if crate::sound::audible(y, x) {
                    crate::sound::priority(0x11f4);
                    crate::sound::keep(crate::sound::key::obj(id, 0x60), 0x29);
                }
                let d = match sprites::hit_rect(sprite, y, x, false) {
                    Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
                    None => 0,
                };
                if d > 0 {
                    let o = self.o(id);
                    o.set(0x26, o.w(0x26) - d);
                    o.set(0x28, 3);
                    if o.w(0x26) < 1 {
                        self.carrier_release(id, env);
                    }
                }
            }
            self.particles.p.aim_level2 = 0x100;
            let o = self.o(id);
            if x_moved != o.w(0xe) || o.w(0x1c) == 5 {
                o.set(0x26, 0);
                env.fx.shakes.push((0x20, 10));
                if crate::sound::looping(crate::sound::key::obj(id, 0x60)) {
                    crate::sound::play_at(1, o.w(0x10), o.w(0xe));
                }
                self.carrier_release(id, env);
            }
            crate::actors::unbounded(self.o(id));
        } else {
            // sprite 0x5c05: only the draw uses it here
            {
                let o = self.o(id);
                let (x, y) = (o.w(0xe), o.w(0x10));
                o.set(2, x - 0x160);
                o.set(4, y - 0x9c);
                o.set(6, x + 0x160);
                o.set(8, y + 0x9c);
            }
            if self.actor_physics(id, env, 0x18, 0x1b, 9) > 0 {
                self.carrier_explodes(id, env);
                return;
            }
            if self.o(id).w(0x1c) == 5 {
                self.slide_on_slope(id);
            }
            // Under water it bubbles (handle +0x60).
            let water = env.world.sectors[self.sector.unwrap_or(0)].water[1];
            let o = self.o(id);
            if o.w(0x5e) < water && crate::sound::audible(o.w(0x10) - 0x10, o.w(0xe)) {
                crate::sound::priority(0x11f4);
                crate::sound::keep(crate::sound::key::obj(id, 0x60), 0x1e);
            }
        }
    }

    /// FUN_1070_0bed (nested): passenger `i` gets off where it stands.
    fn carrier_drop(&mut self, id: usize, i: i32, env: &mut Env) {
        let b = 0x2c + 10 * i as usize;
        let o = self.o(id);
        let kind = o.w(b);
        if kind < 0 {
            return;
        }
        let x = o.w(0xe) + o.w(b + 2);
        let y = o.w(0x5e) + o.w(b + 4);
        o.set(b, -1);
        let w = [0, 0, x as i16, y as i16, (x - 0xc0) as i16, (x + 0xc0) as i16];
        let mut cx = crate::factory::Ctx { hero_x: env.h.x, rng: &mut *env.rng };
        self.factory(env.world, &mut cx, kind as u16, &crate::factory::Rec { w: &w, count: 6 });
    }

    /// FUN_1070_0c6d (nested): it stops flying: everyone gets off and it falls.
    fn carrier_release(&mut self, id: usize, env: &mut Env) {
        self.o(id).set(0x60, -1); // its sound loop stops
        crate::sound::stop(crate::sound::key::obj(id, 0x60));
        for i in (0..self.o(id).w(0x2a)).rev() {
            self.carrier_drop(id, i, env);
        }
        self.o(id).set(0xc, 1);
    }

    /// FUN_1070_0b70 (nested): it blows up in the air.
    fn carrier_explodes(&mut self, id: usize, env: &mut Env) {
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        crate::effects::effect(self, env.world, 7, y - 0x10, x, env.rng, env.fx);
        let r = env.rng.random(4);
        crate::walker::plates(&mut self.particles, env.rng, 400, r + 10, y - 8, x);
        env.fx.shakes.push((0x2a, 0x10));
        crate::sound::play_at(0x4a, y, x);
        crate::sound::play_at(0x18, y, x);
        self.remove_current(id);
    }

    /// FUN_1070_0b02: the move hook: falling once shot down, crashing on landing.
    pub(crate) fn carrier_after_move(&mut self, id: usize, env: &mut Env) {
        let o = self.o(id);
        if o.w(0x1c) == 0 {
            if o.w(0x26) < 1 {
                if o.w(0x1a) == 0 {
                    o.set(0x18, o.w(0x10));
                    o.set(0x14, -0x7fff);
                    o.set(0x16, 0x7fff);
                }
                o.set(0x1a, o.w(0x1a) + 1);
                crate::actors::unbounded(o);
            }
        } else {
            if o.w(0x26) < 1 && o.w(0x20) != 1 {
                o.set(0x20, 1);
                self.carrier_crash(id, env);
            }
            self.o(id).set(0x1a, 0);
        }
    }

    /// FUN_1070_0a34: it hits the ground.
    fn carrier_crash(&mut self, id: usize, env: &mut Env) {
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let ps = &mut self.particles;
        // FUN_1040_66d4: dust, then FUN_1050_4a29(0x100, 0x80).
        ps.p.colour = 0x10;
        ps.spawn_seq(0x6d00, 2, 0x13, y, x, 1);
        ps.p.colour = 0;
        let m = &mut ps.last_mut().m;
        m.set(0x1c, 0x80);
        m.set(0x1e, 0x100);
        m.set(0x42, 0x2148);
        self.particles.p.colour = 0x10;
        crate::effects::effect(self, env.world, 7, y - 0x10, x, env.rng, env.fx);
        self.particles.p.colour = 0;
        let r = env.rng.random(4);
        crate::walker::plates(&mut self.particles, env.rng, 400, r + 10, y - 0xe, x);
        env.fx.shakes.push((0x2a, 0x10));
        self.explode_area(env.world, 0x60, 4, y + 0x16, x + 0x26, y - 0x16, x - 0x26, env.rng, env.fx);
        for s in [0x4a, 0x4e, 0x26] {
            crate::sound::play_at(s, y, x);
        }
    }
}
