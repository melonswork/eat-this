//! The burning wreck (record type 0x1400, VMT 0xbf8, FUN_1068_27e7) that tile effects 24/25
//! (FUN_1040_2b0f/2d5c) leave: it burns for 0xaf0 frames from its making (+0x14/+0x16 = the
//! frame counter then), smoking every 0x12 frames after 0xb4, every 0x24 after 0x690; a shot
//! (mask 0x14) makes it jump (+0x12 = 0x14) and burns it down to its last stage.

use crate::actors::Env;
use crate::level::Level;
use crate::objects::Obj;

pub const KIND: u16 = 0x1400;

/// EnemyFactory 0x1400: byte 2 of the record is the kind (0 = effect 24, 1 = effect 25).
pub(crate) fn make(o: &mut Obj, kind: i32, x: i32, y: i32, now: u32) {
    o.set(10, kind);
    o.set(0xe, x);
    o.set(0x10, y);
    o.set(0x12, 0x16);
    o.set(0x14, (now & 0xffff) as i32);
    o.set(0x16, (now >> 16) as i32);
    o.set(0x18, kind + 0x5500);
    o.set(0x1a, 0x18);
    o.set(2, x - 0xe0);
    o.set(4, y - 0xe0);
    o.set(6, x + 0xe0);
    o.set(8, y + 0xe0);
}

fn stamp(o: &Obj) -> i64 {
    (o.uw(0x14) as u32 | (o.uw(0x16) as u32) << 16) as i32 as i64
}

impl Level {
    pub(crate) fn burnwreck(&mut self, id: usize, env: &mut Env) {
        let now = env.frame as i32 as i64;
        let (x, y, sprite) = (self.o(id).w(0xe), self.o(id).w(0x10), self.o(id).w(0x18));
        if std::env::var_os("ET_WRECK").is_some() {
            eprintln!("WRECK {} w12 {} stamp {}", env.frame, self.o(id).w(0x12), stamp(self.o(id)));
        }
        if self.o(id).w(0x12) < 1 {
            self.particles.p.aim_level = -1;
            self.particles.p.aim_level2 = -1;
            let d = match crate::sprites::hit_rect(sprite, y, x, false) {
                Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x14),
                None => 0,
            };
            self.particles.p.aim_level = 0x100;
            self.particles.p.aim_level2 = 0x100;
            if d > 0 {
                if std::env::var_os("ET_WRECK").is_some() {
                    eprintln!("WRECK hit {} id {id}", env.frame);
                }
                let o = self.o(id);
                o.set(0x12, 0x14);
                o.set(0x1a, 0xc);
                if stamp(o) + 0x690 > now {
                    let s = (now - 0x690) as u32;
                    o.set(0x14, (s & 0xffff) as i32);
                    o.set(0x16, (s >> 16) as i32);
                }
                env.fx.shakes.push((0x26, 0x10));
            }
        } else {
            let o = self.o(id);
            o.set(0x12, o.w(0x12) - 1);
        }
        let o = self.o(id);
        let (st, kind) = (stamp(o), o.w(10));
        if st + 0xb4 == now {
            env.fx.shakes.push((0x10, 6));
        }
        if now < st + 0xaf0 && st + 0xb4 < now {
            let div = if st + 0x690 <= now { 0x24 } else { 0x12 };
            if (now as i32) % div == 0 {
                // Smoke drifting up, swaying with the frame counter.
                let a = ((now as i32).wrapping_mul(0x40)) % 0x1680;
                let off = crate::trig::sin(a) * 8 / 0x4000;
                let ps = &mut self.particles;
                if kind != 0 {
                    ps.p.group = 0;
                }
                ps.p.callback = 0x1cef;
                let dx = if kind == 0 { 4 } else { 0x1a };
                ps.spawn_seq(0x510b, 1, 0x6b, y - 0xd, x + dx + off, 0);
                ps.line(0x18, 0x680);
                ps.p.callback = 0;
                ps.p.group = 1;
            }
        }
    }
}

/// FUN_1068_27e7's draw (while +0x12 < 0x15): the wreck (+0x18) bobbing after a hit, and its
/// flames (FLAME5), growing after 0x690 frames.
pub fn draw(p: &mut crate::render::Painter, o: &Obj) {
    let (x, y0) = (o.w(0xe), o.w(0x10));
    if o.w(0x12) >= 0x15 {
        return;
    }
    let mut y = y0;
    if o.w(0x12) > 0 {
        let a = (o.w(0x12) * 0x2d * 16) / 6;
        y -= crate::trig::sin(a) * o.w(0x1a) / 0x4000;
    }
    p.at(o.w(0x18), x, y);
    let now = p.frame;
    let age = now as i64 - stamp(o);
    let grown = age >= 0x690;
    let scale = if grown { ((((age - 0x690) / if o.w(10) == 0 { 4 } else { 3 }) + 0x100) as u16).min(0x200) as i32 } else { 0x100 };
    if o.w(10) == 0 {
        p.flame(scale, scale, now, x + 2, y - 2);
    } else {
        p.flame(scale, scale, now, x + 0x1e, if grown { y } else { y0 });
        p.flame(0x400, 700, now, x - 0x16, y0);
    }
}
