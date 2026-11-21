//! The lamp-post sentry (record type 0x14f0, icon 62200, FUN_1070_06f6, sprite 0x5212): it
//! takes shots (+0x20 hit points) and glows (+0x24) while something hits its tall beam box.
//! The flicker it draws under a saved RandSeed doesn't touch the simulation.

use crate::actors::Env;
use crate::level::Level;
use crate::sprites;

pub const KIND: u16 = 0x14f0;

impl Level {
    /// FUN_1070_06f6.
    pub(crate) fn sentry(&mut self, id: usize, env: &mut Env) {
        {
            let o = self.o(id);
            if o.w(0x22) > 0 {
                o.set(0x22, o.w(0x22) - 1);
            }
            if o.w(0x24) > 0 {
                o.set(0x24, o.w(0x24) - 1);
            }
            let (x, y) = (o.w(0xe), o.w(0x10));
            o.set(2, x - 0x40);
            o.set(6, x + 0x40);
            o.set(4, y - 0x60);
            o.set(8, y + 0x10);
        }
        if self.actor_physics(id, env, 8, 5, 4) >= 1 {
            self.sentry_dies(id, env);
            return;
        }
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        let d = match sprites::hit_rect(0x5212, y, x, false) {
            Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
            None => 0,
        };
        if d > 0 {
            let push = self.particles.hit_direction();
            let o = self.o(id);
            o.set(0xe, o.w(0xe) + push);
            o.set(0x20, o.w(0x20) - d);
            if o.w(0x20) < 1 {
                self.sentry_dies(id, env);
                return;
            }
            if self.particles.hit_mask & 0x48 == 0 {
                env.fx.shakes.push((0xc, 6));
            }
            self.o(id).set(0x22, 3);
        }
        let (x, y) = (self.o(id).w(0xe), self.o(id).w(0x10));
        self.particles.p.aim_level = -1;
        self.particles.p.aim_level2 = -1;
        // (163e = 4e50 around this test: it leaves the pointer as it is.)
        let pointer = self.particles.pointer;
        let d = self.particles.hit_test(y - 9, x + 4, y - 0x51, x - 4, 0x59);
        self.particles.pointer = pointer;
        self.particles.p.aim_level = 0x100;
        self.particles.p.aim_level2 = 0x100;
        if d > 0 {
            if self.particles.hit_mask & 0x48 == 0 && env.rng.random(2) == 0 {
                let p = env.rng.random(0x40); // a ricochet
                crate::sound::play_at_pitch(0x7c, 0x100 - p, y - 0x20, x);
            }
            self.o(id).set(0x24, 6);
        }
    }

    /// FUN_1070_0636: it blows up.
    fn sentry_dies(&mut self, id: usize, env: &mut Env) {
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
        crate::sound::play_at(0x47, y, x);
        crate::sound::play_at(0x75, y, x);
        ps.p.group = 0;
        let n = env.rng.random(4) + 4;
        crate::walker::plates(ps, env.rng, 0xc0, n, y, x);
        ps.p.group = 1;
        env.fx.shakes.push((0x20, 0x10));
        crate::alien::smoke_sprite(ps, y, x);
        self.remove_current(id);
    }
}
