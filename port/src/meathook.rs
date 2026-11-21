//! The meat hooks (OBJECTS 0x1414, VMT 0xd04, FUN_1070_4b9b, sprites 0x7503..): a vertical chain
//! of hooks (one per 32 px from +0x16 to +0x18) running upwards at a speed that ramps up to +0x1c
//! while its lever (+0x1e, 0: none) is on, each with a piece of meat (FUN_1070_4b4e: 0x7504..0x7507,
//! 0x7507 a head) of 0x10 hit points that shots blow apart. The hero can ride the hooks.
//!
//! | off | field |
//! |-----|-------|
//! | e   | (long) the run position, mod hooks * 0x2000 / 14 it / 0x100 (px) |
//! | 12  | x / 16 top / 18 bottom (+7) / 1a speed / 1c full speed / 1e the lever's map cell |
//! | 20  | hooks / 22 (far pointer) the hooks: `Obj::items` / 26 the motor loop's handle |

use crate::actors::Env;
use crate::level::Level;
use crate::rng::PascalRandom;
use crate::sound;

pub const KIND: u16 = 0x1414;

/// FUN_1070_4b4e: a piece of meat (Random(100)).
pub fn meat(rng: &mut PascalRandom) -> i32 {
    match rng.random(100) {
        0..=0x3b => 0x7504,
        0x3c..=0x5b => 0x7505,
        0x5c..=0x62 => 0x7506,
        _ => 0x7507,
    }
}

impl Level {
    /// FUN_1070_4b9b.
    pub(crate) fn meathook(&mut self, id: usize, env: &mut Env) {
        let key = sound::key::obj(id, 0x26);
        let (x, top, bottom) = (self.o(id).w(0x12), self.o(id).w(0x16), self.o(id).w(0x18));
        let lever = self.o(id).uw(0x1e);
        let o = self.o(id);
        if lever == 0 || Self::lever_state(env.world, lever) == 1 {
            if o.w(0x1a) == 0x30 {
                sound::play_at(0x33, bottom, x);
            }
            if o.w(0x1a) < o.w(0x1c) {
                o.set(0x1a, o.w(0x1a) + 1);
            }
        } else if o.w(0x1a) > 0 {
            o.set(0x1a, o.w(0x1a) - 1);
            if o.w(0x1a) == 0x30 {
                sound::play_at(0x30, bottom, x);
            }
        }
        let o = self.o(id);
        if o.w(0x1a) < 1 {
            if o.w(0x26) != -1 {
                sound::stop(key);
                o.set(0x26, -1);
            }
        } else {
            let v = ((o.w(0x1a) as i64) << 8) / o.w(0x1c) as i64;
            sound::set_volume(((v << 1) / 3) as i32);
            if sound::audible_box((x - 0xf, top, x, bottom)) {
                sound::priority(0x11f4);
                sound::pitch(0x100 - ((v as u16) >> 1) as i32);
                sound::keep(key, 0x2b);
                o.set(0x26, id as i32);
            }
            sound::set_volume(0x100);
        }
        let (cx1, cy1, cx2, cy2) = (env.camera.0, env.camera.1, env.camera.0 + 0x13f, env.camera.1 + 199);
        if !(x - 0x14 <= cx2 && cx1 <= x && top <= cy2 && cy1 <= bottom) {
            return;
        }
        let o = self.o(id);
        let n = o.w(0x20);
        let pos = ((o.uw(0xe) as u32 | (o.uw(0x10) as u32) << 16) as i32 as i64 + o.w(0x1a) as i64)
            % (n as i64 * 0x2000);
        o.set(0xe, pos as i32 & 0xffff);
        o.set(0x10, (pos >> 16) as i32);
        let old = o.w(0x14);
        o.set(0x14, (pos / 0x100) as i32);
        // The hero riding a hook.
        let h = env.h;
        let hs = env.hero_obj.state;
        let can = (hs >= 1 && h.vy >= 0) || (hs < 1 && !(-5..=-1).contains(&hs));
        if x - 0xf < h.x + h.half_width && h.x - h.half_width < x && h.y <= bottom && top <= h.y && can {
            let o14 = self.o(id).w(0x14);
            let d = (-((h.y - 0x801) - (bottom - o14))) % 0x20 - 1;
            let target = h.prev_y + d;
            let mut lo = h.y.min(h.prev_y) - 1;
            let mut hi = h.prev_y.max(h.y);
            if hi < lo {
                std::mem::swap(&mut lo, &mut hi);
            }
            if lo <= target && target <= hi {
                env.hero_obj.set_ground(0x16);
                env.hero_obj.carry.1 = d;
            }
        }
        let first = (n - 1) - old / 0x20;
        if old % 0x20 == 0 {
            let m = meat(env.rng);
            self.o(id).items[first as usize] = (0x10, m, 0);
        }
        self.particles.p.aim_level = -1;
        self.particles.p.aim_level2 = -1;
        let mut i = first;
        let mut y = bottom - old % 0x20;
        while top < y {
            let (hp, sprite, _) = self.o(id).items[i as usize];
            if hp > 0 {
                let d = match crate::sprites::hit_rect(0x7504, y, x, false) {
                    Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x1059),
                    None => 0,
                };
                if d > 0 {
                    self.o(id).items[i as usize].0 = hp - d;
                    if hp - d < 1 {
                        self.o(id).items[i as usize].0 = 0;
                        self.meat_breaks(env, sprite, y, x);
                    }
                }
            }
            i = (i + 1) % n;
            y -= 0x20;
        }
        self.particles.p.aim_level = 0x100;
        self.particles.p.aim_level2 = 0x100;
        // The draw's fading flash of a broken piece (it counts the hit points on down to -6).
        let mut i = first;
        let mut y = bottom - old % 0x20;
        while top < y {
            let it = &mut self.o(id).items[i as usize];
            it.2 = 0;
            if it.0 < 1 && -6 < it.0 {
                it.2 = it.0 * 10 + 0x7c;
                it.0 -= 1;
            }
            i = (i + 1) % n;
            y -= 0x20;
        }
    }

    /// A piece of meat is shot apart: four bits (a head: one is the skull), flames, a crackle.
    fn meat_breaks(&mut self, env: &mut Env, sprite: i32, y: i32, x: i32) {
        let ps = &mut self.particles;
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x10;
        let r = env.rng.random(2);
        ps.spawn(r + 0x7508, y - 4, x - 10);
        let a = env.rng.random(0xa0);
        ps.throw(9, 0x38, a + 0x690);
        if sprite == 0x7507 {
            ps.spawn(0x750a, y - 4, x - 8);
        } else {
            let r = env.rng.random(2);
            ps.spawn(r + 0x7508, y - 4, x - 8);
        }
        let a = env.rng.random(0xa0);
        ps.throw(9, 0x40, a + 0x5f0);
        let r = env.rng.random(2);
        ps.spawn(r + 0x7508, y - 4, x - 6);
        let a = env.rng.random(0xa0);
        ps.throw(9, 0x48, 0x550 - a);
        let r = env.rng.random(2);
        ps.spawn(r + 0x7508, y - 4, x - 4);
        let a = env.rng.random(0xa0);
        ps.throw(9, 0x30, 0x4b0 - a);
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
        ps.p.colour = 0x10;
        for i in 0..6 {
            ps.p.delay = i * 6;
            let fx_ = x - 0xf + env.rng.random(0xb);
            let fy = (y - 2) - env.rng.random(8);
            if i < 3 {
                ps.spawn_seq(0x6800, 2, 0xf, fy, fx_, 1);
            } else {
                ps.spawn_seq(0x5111, 4, 9, fy, fx_, 1); // FUN_1040_645a
            }
            ps.line(0x10, 0x660);
        }
        ps.p.delay = 0;
        ps.p.colour = 0;
        sound::play_at(0x4b, y, x);
        env.fx.shakes.push((0xc, 6));
    }
}

/// FUN_1070_4b9b's draw: the hooks between +0x16 and +0x18 (clipped to them, right edge at x),
/// each jittered by Random(2) under RandSeed frame / 8 (0 while speeding up), a broken one lit.
pub fn draw(p: &mut crate::render::Painter, o: &crate::objects::Obj) {
    let (x, top, bottom, n) = (o.w(0x12), o.w(0x16), o.w(0x18), o.w(0x20));
    if n < 1 || o.items.len() < n as usize {
        return;
    }
    let (cx, cy) = p.cam;
    if !(x - 0x14 <= cx + 0x13f && cx <= x && top <= cy + 199 && cy <= bottom) {
        return;
    }
    // The run offset the update used (before it moved on).
    let old = {
        let pos = (o.uw(0xe) as u32 | (o.uw(0x10) as u32) << 16) as i32 as i64;
        let prev = (pos - o.w(0x1a) as i64).rem_euclid(n as i64 * 0x2000);
        (prev / 0x100) as i32
    };
    let seed = if o.w(0x1a) < o.w(0x1c) { 0 } else { p.frame / 8 };
    let mut rng = PascalRandom::plain(seed);
    let saved = p.clip;
    // FUN_10f0_12d4: narrow the clip rectangle.
    p.clip = (saved.0, saved.1.max(top - cy), saved.2.min(x - cx), saved.3.min(bottom - cy));
    if p.clip.1 > p.clip.3 || p.clip.0 > p.clip.2 {
        p.clip = saved;
        return;
    }
    let mut i = (n - 1) - old / 0x20;
    let mut y = bottom - old % 0x20;
    while top < y {
        let jx = x + rng.random(2);
        let (hp, sprite, flash) = o.items[i as usize];
        if hp < 1 {
            if flash != 0 {
                p.mode = 0x24;
                p.mix = crate::render::Mix::Light;
                p.colour = flash as u8;
            }
            p.at(0x7503, jx, y);
            p.mode = 0;
            p.mix = crate::render::Mix::Level;
            p.colour = 0;
        } else {
            p.at(0x7503, jx, y);
            p.at(sprite, jx, y);
        }
        i = (i + 1) % n;
        y -= 0x20;
    }
    p.clip = saved;
}
