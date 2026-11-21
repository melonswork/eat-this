//! The spiked ball on a chain (OBJECTS 0x14e6, VMT 0xda0, FUN_1070_9a66, ball sprite 0x3314,
//! links 0x3714..): hangs still until its lever cell (+0x1c) is on or a shot hits it, then swings;
//! screen shakes push it, swinging hard it hurts the hero (hit kind 3) and gets bloody (0x3315),
//! then drips blood for a while.
//!
//! | off | field |
//! |-----|-------|
//! | 14  | the swing phase (angle = phase * 0x50) / 16 links / 18 chain length ((links + 1) * 13) |
//! | 1a  | the swing's strength (to 0x168, -2 a frame) / 1c the lever's map cell / 1e the ball sprite |
//! | 20  | (byte) swinging / 21 bleeding frames |

use crate::actors::Env;
use crate::level::Level;
use crate::objects::Obj;
use crate::sound;
use crate::trig::sin;

pub const KIND: u16 = 0x14e6;

/// RangesOverlap(a, b, c, d).
fn ranges(a: i32, b: i32, c: i32, d: i32) -> bool {
    a.max(b) >= c.min(d) && a.min(b) <= c.max(d)
}

impl Level {
    /// FUN_1070_9a16(strength): push the swing (to 0x168 at most); the box becomes unbounded.
    fn spikeball_push(&mut self, id: usize, n: i32) {
        let n = n.min(0x168);
        let o = self.o(id);
        if o.w(0x1a) == 0 {
            o.set(0x14, 1);
        }
        if o.w(0x1a) < n {
            o.set(0x1a, n);
        }
        crate::actors::unbounded(o);
    }

    /// FUN_1070_9a66.
    pub(crate) fn spikeball(&mut self, id: usize, env: &mut Env) {
        if self.o(id).b(0x20) == 0 {
            let cell = self.o(id).uw(0x1c);
            let on = Self::lever_state(env.world, cell) > 0;
            self.o(id).set_b(0x20, on as u8);
        }
        let angle = {
            let o = self.o(id);
            if o.w(0x1a) < 1 {
                0x10e0
            } else {
                if o.b(0x20) != 0 {
                    o.set(0x1a, (o.w(0x1a) - 2).max(0));
                    o.set(0x14, o.w(0x14) + 1);
                }
                if o.w(0x1a) == 0 {
                    let (x, y, len) = (o.w(0xe), o.w(0x10), o.w(0x18));
                    o.set(2, (x - len) - 0x18);
                    o.set(4, y - 0x10);
                    o.set(6, x + len + 0x18);
                    o.set(8, y + len + 0x30);
                }
                let a = (o.w(0x14) * 0x50) % 0x1680;
                0x10e0 + (sin(a) * (((o.uw(0x1a) >> 2) << 4) as i16 as i32)) / 0x4000
            }
        };
        let (bx, by) = {
            let o = self.o(id);
            let len = o.w(0x18);
            let bx = o.w(0xe) + (sin((angle + 0x5a0) % 0x1680) * len) / 0x4000;
            let by = o.w(0x10) - ((sin(angle) * len) / 0x4000).max(1 - len);
            (bx, by)
        };
        if std::env::var_os("ET_SPIKE").is_some() {
            let o = self.o(id);
            eprintln!("SPIKE {} on {} swing {} phase {} angle {angle:#x} ball ({bx},{by})", env.frame, o.b(0x20), o.w(0x1a), o.w(0x14));
        }
        if self.o(id).b(0x20) != 0 {
            if env.shake_started && env.shake_amp > 7 {
                let r = env.rng.random(0x18);
                let n = (env.shake_amp * 0x78) / 0x10 - 0xc + r;
                self.spikeball_push(id, n);
                if self.o(id).w(0x1a) == n && env.rng.random(2) == 0 {
                    let o = self.o(id);
                    o.set(0x14, o.w(0x14) + 0xb4);
                }
            }
            let h = env.h;
            if self.o(id).w(0x1a) > 0xb4
                && !env.hero_obj.invulnerable()
                && ranges(h.x + h.half_width, h.x - h.half_width, bx + 7, bx - 7)
                && ranges(h.y, (h.y - h.height) + 1, by + 7, by - 7)
                && env.hero_obj.hurt_kind(0xc0, 3, env.rng) == 1
            {
                sound::play_at(0xf, by, bx);
                let o = self.o(id);
                if o.w(0x1e) == 0x3314 {
                    o.set(0x1e, 0x3315);
                }
                o.set(0x14, (o.w(0x14) + 0x24) % 0x48);
                o.set(0x21, 0x15e);
            }
        }
        let ps = &mut self.particles;
        ps.p.aim_level = -1;
        ps.p.aim_level2 = -1;
        let sprite = self.o(id).w(0x1e);
        let d = match crate::sprites::hit_rect(sprite, by, bx, false) {
            Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
            None => 0,
        };
        if d > 3 {
            if self.o(id).b(0x20) == 0 {
                self.o(id).set_b(0x20, 1);
            } else {
                self.spikeball_push(id, (d * 0x78) / 0x10);
                let dir = self.particles.hit_direction();
                let o = self.o(id);
                o.set(0x14, o.w(0x14) % 0x24);
                if dir < 0 {
                    o.set(0x14, o.w(0x14) + 0x24);
                }
            }
            sound::play_at(1, by, bx);
            env.fx.shakes.push((0xc, 5));
        }
        self.particles.p.aim_level = 0x100;
        self.particles.p.aim_level2 = 0x100;
        let bleed = {
            let o = self.o(id);
            if o.w(0x21) > 0 {
                o.set(0x21, o.w(0x21) - 1);
                o.w(0x21) % 0x32 == 0
            } else {
                false
            }
        };
        if bleed {
            let ps = &mut self.particles;
            ps.p.group = 0;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x16;
            ps.p.callback = 0x23b7;
            ps.spawn(0x5309, by + 6, bx - 1);
            ps.throw(7, 0, 0x10e0);
            ps.p.callback = 0;
            ps.p.on_death = 0;
            ps.p.hits = 0;
            ps.p.group = 1;
        }
        // The draw: the links along the chain (mirrored by the angle), then the ball.
        let o = self.o(id);
        let side = (angle + 0x5a0) % 0x1680;
        let (hflip, vflip) = (side >= 0xb40, angle >= 0xb40);
        let frame = (angle % 0x5a0 + 0x50) / 0xa0;
        let link = if hflip == vflip { 9 - frame } else { frame } + 0x3714;
        let (sx, sy) = (sin(side) as i64 * 0xd, sin(angle) as i64 * 0xd);
        let (x, y) = (o.w(0xe), o.w(0x10));
        let mut looks = Vec::new();
        for i in 1..=o.w(0x16) as i64 {
            let lx = x + ((i * sx) / 0x4000) as i32;
            let ly = y - ((i * sy) / 0x4000) as i32;
            let mut l = Obj::look(link, lx, ly).flip(hflip);
            l.vflip = vflip;
            looks.push(l);
        }
        looks.push(Obj::look(o.w(0x1e), bx, by));
        o.looks = looks;
    }
}
