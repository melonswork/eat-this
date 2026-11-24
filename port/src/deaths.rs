//! The shared deaths of the enemies hit by fire or ice (aliens, big aliens, grannies):
//! FUN_1070_544e burnt (ash flakes 0x5132, burning bits 0x5130 that land as effect 59, smoke)
//! and FUN_1070_564d frozen (ice splinters 0x5e08, shards 0x5e06/7 that land as effect 48).

use crate::particles::Particles;
use crate::rng::PascalRandom;
use crate::sound;

/// FUN_1070_544e(y, x): burnt to ashes.
pub fn burnt(ps: &mut Particles, rng: &mut PascalRandom, y: i32, x: i32) {
    sound::play_at(0x71, y, x);
    ps.p.hits = 0x8000;
    ps.p.callback = 0x2497;
    for i in 1..=0x20 {
        let a = (i * 0x280) / 0x20;
        let speed = (i * 0x30) / 0x20 + 0x20;
        ps.p.delay = rng.random(8);
        ps.spawn_seq(0x5132, 1, 0x46, y, x, 0);
        let r = rng.random(0x80);
        ps.throw(0xf, speed, (r + 0x820) - a);
        ps.p.delay = rng.random(8);
        ps.spawn_seq(0x5132, 1, 0x46, y, x, 0);
        let r = rng.random(0x80);
        ps.throw(0xf, speed, (a + (0x19a0 - r)) % 0x1680);
    }
    ps.p.callback = 0x2459;
    ps.p.on_death = 0x3b;
    for i in 1..=4 {
        ps.p.delay = rng.random(8) + 8;
        let speed = (i << 5) / 4 + 0x20;
        ps.spawn(0x5130, y, x);
        let a = (i * 0x3c0) / 4;
        let r = rng.random(0x80);
        ps.throw(0x12, speed, (r + 0x960) - a);
        ps.p.delay = rng.random(8) + 8;
        ps.spawn(0x5130, y, x);
        let r = rng.random(0x80);
        ps.throw(0x12, speed, (a + (0x1860 - r)) % 0x1680);
    }
    ps.p.callback = 0;
    ps.p.on_death = 0;
    ps.p.hits = 0;
    ps.p.colour = 0x10;
    crate::particles::smoke(ps, rng, y, x);
    ps.p.colour = 0;
}

/// FUN_1070_564d(y, x): frozen and shattered.
pub fn frozen(ps: &mut Particles, rng: &mut PascalRandom, y: i32, x: i32) {
    let p = rng.random(0x20);
    sound::play_at_pitch(0x21, 0x100 - p, y, x);
    ps.p.hits = 0x8000;
    ps.p.colour = 0x10;
    for i in 1..=0x1c {
        let a = (i * 0x280) / 0x1c;
        let speed = (i * 0x28) / 0x1c + 0x20;
        ps.p.delay = rng.random(8);
        ps.spawn_seq(0x5e08, 1, 0x46, y, x, 0);
        let r = rng.random(0x80);
        ps.throw(0xf, speed, (r + 0x820) - a);
        ps.p.delay = rng.random(8);
        ps.spawn_seq(0x5e08, 1, 0x46, y, x, 0);
        let r = rng.random(0x80);
        ps.throw(0xf, speed, (a + (0x19a0 - r)) % 0x1680);
    }
    ps.p.colour = 0;
    ps.p.on_death = 0x30;
    for i in 1..=4 {
        ps.p.delay = rng.random(8) + 8;
        let speed = (i * 0x18) / 4 + 0x20;
        let s = rng.random(2);
        ps.spawn(0x5e07 - s, y, x);
        let a = (i * 0x3c0) / 4;
        let r = rng.random(0x80);
        ps.throw(0x12, speed, (r + 0x960) - a);
        ps.p.delay = rng.random(8) + 8;
        let s = rng.random(2);
        ps.spawn(0x5e07 - s, y, x);
        let r = rng.random(0x80);
        ps.throw(0x12, speed, (a + (0x1860 - r)) % 0x1680);
    }
    ps.p.on_death = 0;
    ps.p.hits = 0;
    ps.p.delay = 0;
}

impl crate::level::Level {
    /// FUN_1068_0ef4(strength): wobble a frozen statue (to 0xf0).
    fn statue_push(&mut self, id: usize, n: i32) {
        let n = n.min(0xf0);
        let o = self.o(id);
        if o.w(0x24) == 0 {
            o.set(0x22, 1);
        }
        if o.w(0x24) < n {
            o.set(0x24, n);
        }
    }

    /// FUN_1068_0f32(obj, sprite): a frozen enemy standing as a statue (+0x22 wobble phase, +0x24
    /// its strength): a shake or the hero brushing past wobbles it; true when a shot hits it (the
    /// caller shatters it).
    pub(crate) fn frozen_statue(&mut self, id: usize, env: &mut crate::actors::Env, sprite: i32) -> bool {
        let (x, y, flip) = (self.o(id).w(0xe), self.o(id).w(0x10), self.o(id).w(0x20) < 0);
        self.particles.p.aim_level = 0;
        self.particles.p.aim_level2 = -1;
        let d = match crate::sprites::hit_rect(sprite, y, x, flip) {
            Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
            None => 0,
        };
        self.particles.p.aim_level = 0x100;
        self.particles.p.aim_level2 = 0x100;
        let (started, amp) = env.shake_now();
        if started {
            let n = (amp * 0x10 - 0xc + env.rng.random(0x18)).max(0x40);
            self.statue_push(id, n);
            if self.o(id).w(0x24) == n && env.rng.random(2) == 0 {
                let o = self.o(id);
                o.set(0x22, o.w(0x22) + 0xb4);
            }
        }
        let h = env.h;
        if y - 0x20 < h.y && h.y < y + 0x20 && h.x.min(h.prev_x) <= x && x <= h.x.max(h.prev_x) {
            self.statue_push(id, 0x80);
        }
        let o = self.o(id);
        if o.w(0x24) > 0 {
            o.set(0x24, o.w(0x24) - 1);
            o.set(0x22, o.w(0x22) + 1);
        }
        d > 0
    }
}

/// FUN_1068_0f32's draw: the sprite through the level mix table (glassy), squashed and stretched by the wobble.
pub fn draw_statue(p: &mut crate::render::Painter, o: &crate::objects::Obj, sprite: i32) {
    let a = ((o.w(0x22) * 0x80) as i64).rem_euclid(0x1680) as i32;
    let w = (crate::trig::sin(a) * ((o.uw(0x24) >> 1) as i32)) / 0x4000;
    p.mode = 0x10;
    p.mix = crate::render::Mix::Level; // the scaled blitter's mode 0x10 (10f0:2fbb) reads PalMix (1b0e)
    p.colour = 2;
    p.hflip = o.w(0x20) < 0;
    p.scaled_at(0x120 - (w << 1) / 3, w + 0x100, sprite, o.w(0xe), o.w(0x10));
    p.reset();
    p.colour = 0;
}
