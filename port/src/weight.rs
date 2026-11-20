//! Falling weights (icons 61000/61001, kind 0x13a6, VMT 0xbe8, FUN_1068_215e): type 0 a weight
//! hanging on a chain (sprites 0x4700), type 1 a press (0x4800, animated, the hero can stand on it).
//! When the hero walks under it, it shakes and drops (hero hit kind 2 for 0xa0 if he is under it),
//! breaks through explodable tiles, lands (type 0: maybe broken; a 7010 tile under it steps on), and
//! once landed can be shot to its broken sprite. The table at 1170:0c28 (0x12 bytes a type):
//! half width, height, first sprite, frames, frames per sprite, broken sprite, trigger depth and
//! half width, the icon's offset (x, y bytes).
//!
//! | off | field |
//! |-----|-------|
//! | 14  | state: 0 hanging, 3 about to drop, 2 falling, 1 landed |
//! | 16  | countdown (0x17: idle) / fall time / 12 the y the fall started at |
//! | 18  | sprite / 1a animation counter |

use crate::actors::Env;
use crate::level::Level;
use crate::sprites;
use crate::world::ds_word;

pub const KIND: u16 = 0x13a6;

/// Word `i` of the type's row in 1170:0c28.
pub fn table(t: i32, i: usize) -> i32 {
    ds_word(0xc28 + (t as usize) * 0x12 + i * 2)
}

impl Level {
    /// FUN_1068_215e.
    pub(crate) fn weight(&mut self, id: usize, env: &mut Env) {
        let h = env.h;
        let t = self.o(id).w(10);
        let (half, height, base, frames, speed, broken) =
            (table(t, 0), table(t, 1), table(t, 2), table(t, 3), table(t, 4), table(t, 5));
        let state = self.o(id).w(0x14);
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        if state == 1 {
            let o = self.o(id);
            if o.w(0x18) == broken {
                if o.w(0x16) < 0x1e {
                    o.set(0x16, o.w(0x16) + 1);
                }
            } else {
                let s = o.w(0x18);
                self.particles.p.aim_level2 = -1;
                let d = match sprites::hit_rect(s, y, x, false) {
                    Some((b, r, tp, l)) => self.particles.hit_test(b, r, tp, l, 0x14),
                    None => 0,
                };
                self.particles.p.aim_level2 = 0x100;
                if d > 0x5f {
                    self.o(id).set(0x18, broken);
                    env.fx.shakes.push((0x20, 0xc));
                }
            }
        } else if state == 2 {
            let o = self.o(id);
            o.set(0x16, o.w(0x16) + 1);
            let prev = o.w(0x10);
            let t2 = (o.w(0x16) as u16).wrapping_mul(o.w(0x16) as u16) as i16 as i32;
            o.set(0x10, t2 / 7 + o.w(0x12));
            let y = o.w(0x10);
            // FUN_1088_52d6: the hero under it (not while invulnerable).
            if !env.hero_obj.invulnerable()
                && ranges(h.x + h.half_width, h.x - h.half_width, x + half, x - half)
                && ranges(h.y, (h.y - h.height) + 1, y - 8, y - 0x10)
            {
                env.hero_obj.crushed(0xa0, env.rng);
            }
            if env.world.solidity_at(y, x) & 0x80 != 0 {
                crate::effects::explode_tile(self, env.world, 0x60, 4, y, x, env.rng, env.fx);
                let o = self.o(id);
                o.set(0x12, y);
                o.set(0x16, 0);
            }
            self.splash(env, y, prev, x); // at volume 0x180
            if env.world.solidity_at(y, x) & 0x201 != 0 {
                let y = (y & !0xf) - 1;
                {
                    let o = self.o(id);
                    o.set(0x10, y);
                    o.set(0x14, 1);
                    o.set(0x16, 0);
                }
                env.fx.shakes.push((0x1e, 0xc));
                if t == 0 {
                    // sound 0x18
                    crate::alien::dust(&mut self.particles, y, x);
                    if env.rng.random(2) == 0 {
                        self.o(id).set(0x18, broken);
                    }
                } else if t == 1 {
                    // sound 0x1c
                    self.press_bits(env, y, x);
                    crate::alien::dust(&mut self.particles, y - 0xe, x);
                    self.o(id).set(0x18, broken);
                }
                if env.world.tile_function_at(y + 1, x) == 7010 {
                    let w = env.world.main().width;
                    let cell = (((y + 1) as u16 >> 4) as i32 * w + (x as u16 >> 4) as i32) as u16;
                    let tile = env.world.main().cells[cell as usize];
                    self.set_cell(env.world, tile.wrapping_add(1), cell, env.fx);
                }
                let o = self.o(id);
                o.set(2, (x - half) - 0x10);
                o.set(6, x + half + 0x10);
                o.set(4, (y - height) - 0x10);
                o.set(8, y + 0x10);
            }
        } else {
            let s = self.o(id).w(0x18);
            self.particles.p.aim_level2 = -1;
            let d = match sprites::hit_rect(s, y, x, false) {
                Some((b, r, tp, l)) => self.particles.hit_test(b, r, tp, l, 0x11b7),
                None => 0,
            };
            self.particles.p.aim_level2 = 0x100;
            if d > 0 {
                env.fx.shakes.push((0xc, 4));
                if self.particles.hit_index >= 0 {
                    let m = &self.particles.list[self.particles.hit_index as usize].m;
                    let (py, px) = (m.w(0x12), m.w(0x10));
                    self.particles.p.colour = 0x10;
                    crate::particles::smoke(&mut self.particles, env.rng, py, px);
                    self.particles.p.colour = 0;
                }
                if self.o(id).w(0x16) == 0x17 {
                    self.o(id).set(0x16, 0x16);
                }
            }
            if t == 1 && self.stand_on(env, y - height, (y - height) - 1, x + half, x - half) {
                if self.o(id).w(0x16) == 0x17 {
                    self.o(id).set(0x16, 0x16);
                }
            } else {
                let o = self.o(id);
                let c = o.uw(0x1a) as i32;
                // (type 1: sound 0x1f when c % ((frames - 1) * speed) == 0)
                if frames > 1 {
                    let k = ((c / speed) % (frames * 2 - 2) - frames + 1).abs();
                    o.set(0x18, base + frames - 1 - k);
                }
                o.set(0x1a, o.w(0x1a).wrapping_add(1));
            }
            let dist = (h.x - x).abs();
            let o = self.o(id);
            if y <= h.y && h.y <= y + table(t, 6) {
                if dist < table(t, 7) {
                    o.set(0x16, 0);
                } else if o.w(0x14) != 3 && dist < table(t, 7) + 0x20 {
                    o.set(0x16, 0xb);
                }
            }
            if o.w(0x16) < 0x17 {
                if o.w(0x16) < 0xc && o.w(0x14) != 3 {
                    o.set(0x10, o.w(0x10) + 1);
                    o.set(0x14, 3);
                } else if o.w(0x16) < 1 {
                    o.set(0x14, 2);
                    o.set(0x12, o.w(0x10));
                    crate::actors::unbounded(o);
                    // sound 0x19
                    if t == 0 {
                        self.chain_bits(env, y, x);
                    }
                }
                let o = self.o(id);
                o.set(0x16, o.w(0x16) - 1);
            }
        }
    }

    /// The chain above a dropping weight (tile function 60300) breaks into bits.
    fn chain_bits(&mut self, env: &mut Env, y: i32, x: i32) {
        let ps = &mut self.particles;
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x10;
        let mut e = ((y - 0x30) & !0xf) + 0xc;
        while env.world.tile_function_at(e, x) == 60300 {
            let w = env.world.main().width;
            let cell = ((e >> 4) * w + (x >> 4)) as usize;
            env.world.planes[crate::world::MAIN_PLANE].as_mut().unwrap().cells[cell] = 0;
            for _ in 0..2 {
                ps.spawn(0x5128, e, x);
                let a = env.rng.random(2) * 0xb40;
                let s = env.rng.random(0x11);
                ps.throw(7, s, a);
                e -= 8;
            }
        }
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
    }

    /// FUN_1040_677d: a press lands: thirteen bits fly up.
    fn press_bits(&mut self, env: &mut Env, y: i32, x: i32) {
        let ps = &mut self.particles;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x10;
        ps.p.group = 0;
        ps.p.callback = 0x17f4;
        for i in 0..=0xc {
            ps.p.delay = env.rng.random(6);
            ps.p.draw = env.rng.random(2);
            let px = x - 0xc + i * 2;
            let py = (y - 8) - env.rng.random(0x1e);
            let step = env.rng.random(3) + 2;
            ps.spawn(0x7552, py, px); // FUN_1050_4106: its own frame step
            ps.last_mut().m.set(0x26, step);
            let a = (0x7a0 - env.rng.random(0x41)) - i * 0x50;
            let s = env.rng.random(0x10) + 0x40;
            ps.throw(9, s, a);
        }
        ps.p.delay = 0;
        ps.p.draw = 0;
        ps.p.callback = 0;
        ps.p.hits = 0;
        ps.p.on_death = 0;
        ps.p.group = 1;
    }
}

/// RangesOverlap (either pair in any order).
fn ranges(a: i32, b: i32, c: i32, d: i32) -> bool {
    a.min(b) <= c.max(d) && c.min(d) <= a.max(b)
}

/// FUN_1068_215e's draw: the sprite; the broken weight (0x4702) also its swinging chain end.
pub fn draw(p: &mut crate::render::Painter, o: &crate::objects::Obj) {
    let (x, y, s) = (o.w(0xe), o.w(0x10), o.w(0x18));
    p.at(s, x, y);
    if s == 0x4702 {
        let v = crate::trig::sin(((o.w(0x16) * 3) << 4) as i16 as i32);
        p.at(s + 1, x - (v * 4) / 0x4000, y + (v * 3) / 0x4000);
    }
}
