//! The sea plants (record type 0x14fa, FUN_1068_2ee8): a stalk of `+0x14` segments that sways
//! (phase +0x18, its own draw-time seed +0x1a) and bursts when shot hard enough or touched by the
//! hero (FUN_1068_2cf3). +0x16 is 1 while it stands.

use crate::actors::Env;
use crate::level::Level;
use crate::sprites;
use crate::trig;

fn ranges_overlap(a: i32, b: i32, c: i32, d: i32) -> bool {
    a.min(b) <= c.max(d) && c.min(d) <= a.max(b)
}

impl Level {
    pub(crate) fn plant(&mut self, id: usize, env: &mut Env) {
        let o = self.o(id);
        if o.w(0x16) <= 0 {
            return;
        }
        let (x, y, segs, phase, seed) = (o.w(0xe), o.w(0x10), o.w(0x14), o.w(0x18), o.w(0x1a));
        let now = env.frame as i64;
        let t = ((now * 0x20 + phase as i64) % 0x1680) as i32;
        let idx = (((segs << 9) as i16 as i32) + t) % 0x1680;
        let tip_x = x + (trig::sin(idx) * 6) / 0x4000;
        let idx2 = ((now * 28 + 0x5a0 + phase as i64) % 0x1680) as i32;
        let rise = (segs << 3) + (trig::sin(idx2) * 6) / 0x4000;
        let tip_y = y - rise - 8;
        let hit = match sprites::hit_rect(0x3300, tip_y, tip_x, false) {
            Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
            None => 0,
        };
        let h = env.h;
        let burst = hit >= 8
            || (ranges_overlap(tip_x - 7, tip_x + 7, h.x - h.half_width, h.x + h.half_width)
                && ranges_overlap(tip_y - 7, tip_y + 7, (h.y - h.height) + 1, h.y));
        if burst {
            self.plant_burst(id, env, (tip_y, tip_x, t, rise, segs, seed));
        }
    }

    /// FUN_1068_2cf3.
    fn plant_burst(&mut self, id: usize, env: &mut Env, (ty, tx, t, rise, segs, seed): (i32, i32, i32, i32, i32, i32)) {
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let ps = &mut self.particles;
        ps.blast(2, 0x180, 7, 0x3e, 0x3e, -0x3e, -0x3e, ty, tx);
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x10;
        let seed = seed as i16 as i32 as u32;
        env.rng.scoped(seed, |rng| {
            for k in 1..segs {
                ps.p.delay = rng.random(8);
                let off = (trig::sin(((k << 9) + t) % 0x1680) * ((k * 6) / segs)) / 0x4000;
                let sprite = rng.random(2) + 0x3301;
                ps.spawn(sprite, y - (k * rise) / segs, x + off);
                let a = rng.random(2) * 0xb40;
                let v = rng.random(0x21);
                ps.throw(0x54, v, a);
            }
        });
        ps.p.delay = 0;
        ps.p.on_death = 0;
        ps.p.hits = 0;
        // FUN_1068_2c56: a column of bubbles.
        let water = self.water;
        self.particles.p.group = 0;
        for k in 0..=0x18 {
            self.particles.p.on_spawn = if k % 0xc == 0 { env.rng.random(2) + 0x7570 } else { 0 };
            self.particles.p.delay = (k * k) / 6;
            let bx = tx - 0x10 + env.rng.random(0x21);
            let by = ty - env.rng.random(3);
            let sprite = env.rng.random(3) + 0x64b;
            if water + 0x10 <= by {
                let ps = &mut self.particles;
                ps.p.callback = 0x30f;
                ps.spawn(sprite, by, bx);
                ps.p.callback = 0;
            }
        }
        let ps = &mut self.particles;
        ps.p.on_spawn = 0;
        ps.p.delay = 0;
        ps.p.group = 1;
        env.fx.shakes.push((0x30, 0x12));
        for id in [0x9e, 0] {
            let p = env.rng.random(0x31);
            let v = env.rng.random(0x38);
            crate::sound::pitch(0x100 - p);
            crate::sound::play_at_vol(id, v + 200, ty, tx);
        }
        // FUN_1040_613b
        self.particles.spawn_seq(0x7000, 3, 0x18, ty, tx, 1);
        self.o(id).set(0x16, 0);
    }
}
