//! The smoke/steam emitter (record type 0x141e, FUN_1070_5280): while its box meets the screen it
//! puffs; byte 2 of its record picks the kind (0 a spray of bits, 1 and 2 smoke on even frames).

use crate::actors::Env;
use crate::level::Level;

impl Level {
    pub(crate) fn steam(&mut self, id: usize, env: &mut Env) {
        let (x, y, kind) = (self.o(id).w(0xe), self.o(id).w(0x10), self.o(id).w(10));
        // Its hiss (handle +0x14), quieter than most.
        crate::sound::set_volume(0xaa);
        if crate::sound::audible(y, x) {
            crate::sound::priority(0x11f4);
            crate::sound::keep(crate::sound::key::obj(id, 0x14), if kind == 0 { 0x69 } else { 0xc });
        }
        crate::sound::set_volume(0x100);
        let (cx, cy) = env.camera;
        if !(x - 0x18 <= cx + 0x13f && cx <= x + 0x18 && y - 0x80 <= cy + 199 && cy <= y + 0x10) {
            return;
        }
        let ps = &mut self.particles;
        ps.p.colour = 0x10;
        if kind == 0 {
            let dy = env.rng.random(4);
            ps.spawn_seq(0x750c, 8, 3, y + dy, x, 1);
            let a = 0x5d0 - env.rng.random(0x61);
            let v = env.rng.random(8) + 0x40;
            ps.throw(9, v, a);
        } else if env.frame & 1 == 0 {
            if kind == 1 {
                ps.spawn_seq(0x6c00, 1, 0x19, y, x, 1);
                let a = env.rng.random(0x101) + 0x520;
                ps.line(0x20, a);
                let b = env.rng.random(0x41) + 0x2a8;
                let c = env.rng.random(0x41);
                let m = &mut ps.last_mut().m; // FUN_1050_4a29(c + 0x1e0, b)
                m.set(0x1c, b);
                m.set(0x1e, c + 0x1e0);
                m.set(0x42, 0x2148);
            } else {
                ps.p.group = 0;
                ps.spawn_seq(0x6c00, 1, 0x19, y, x, 1);
                let a = env.rng.random(0x101) + 0x520;
                ps.line(0x10, a);
                let b = env.rng.random(0x41) + 0x570;
                let c = env.rng.random(0x41);
                let m = &mut ps.last_mut().m;
                m.set(0x1c, b);
                m.set(0x1e, c + 0x3e0);
                m.set(0x42, 0x2148);
                ps.p.group = 1;
            }
        }
        ps.p.colour = 0;
    }
}
