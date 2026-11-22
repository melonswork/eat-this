//! GRANNY's collapsing building (OBJECTS 0x2715 from LevelInit; FUN_1088_a481, run by the level's
//! background proc 15 = FUN_1090_1f6f while 1170:4eca != 0x7fff). Its lever (4ed4) starts it: for
//! 0x42a frames the building (extra map plane 4, drawn in the half-speed background layer) tilts
//! over, shaking the screen and shedding debris (particle group 2, mover FUN_1050_3d06, drawn by
//! callback 0x232e) with a rumble loop (0x53). Afterwards the debris goes and it stops.
//! The building is drawn by main.rs (`Graphics::building`, from SVEN2BAC.MAP = planes 4/5).

use crate::level::Level;
use crate::rng::PascalRandom;
use crate::sound;
use crate::trig::sin;

pub const KIND: u16 = 0x2715;

/// 1170:4eca..4ed8.
#[derive(Clone, Debug)]
pub struct Collapse {
    /// 4eca / 4ecc: the building's pivot in the background layer (x / 2, y / 2 - 0x90).
    pub x: i32,
    pub y: i32,
    /// 4ece: frames since it started (0: waiting for the lever).
    pub t: i32,
    /// 4ed0: frames to the next debris.
    pub next: i32,
    /// 4ed4: the lever's map cell.
    pub lever: u16,
    /// 4ed6/4ed8: its sector (the draw only shows it there).
    #[allow(dead_code)]
    pub sector: usize,
}

impl Level {
    /// FUN_1088_a481's simulation. Returns the screen shakes it made (they act at once).
    pub fn collapse_frame(&mut self, world: &crate::world::World, rng: &mut PascalRandom, cam: (i32, i32)) -> Vec<(i32, i32)> {
        let mut shakes = Vec::new();
        let Some(mut c) = self.collapse.clone() else {
            return shakes;
        };
        if c.x == 0x7fff {
            return shakes;
        }
        let y2 = c.t / 2 + c.y;
        let a = (0x1680 - c.t / 2) % 0x1680;
        let (s, co) = (sin(a), sin((a + 0x5a0) % 0x1680));
        if c.t < 1 {
            if Self::lever_state(world, c.lever) > 0 {
                c.t = 1;
            }
        } else {
            c.next -= 1;
            if c.next < 1 && c.t < 0x38a {
                let r1 = rng.random(0x120) + 0x120;
                let r2 = rng.random(0x1c0) + 0xf0;
                let px = c.x + ((r1 * co) / 0x4000 + (r2 * s) / 0x4000 - (s * 0x230) / 0x4000) - 0xa0;
                let py = y2 + ((r2 * co) / 0x4000 - (r1 * s) / 0x4000 + (s * 0x148) / 0x4000) - 0x80;
                let ps = &mut self.particles;
                ps.p.group = 2;
                ps.p.callback = 0x232e;
                ps.p.colour = 4;
                for i in 0..7 {
                    let k = rng.random(2);
                    ps.spawn(k + 0x7550, py, px);
                    ps.bg_throw(0xe, 0x20, i * 0xc0 + 0x300);
                }
                let (hx, hy) = (((cam.0 as u16) >> 1) as i32, ((cam.1 as u16) >> 1) as i32);
                sound::play_at(0x4b, (py - hy) + cam.1, (px - hx) + cam.0);
                ps.p.colour = 0x30;
                ps.spawn_seq(0x7000, 3, 0x18, py, px, 1);
                ps.p.callback = 0;
                ps.p.colour = 0;
                ps.p.group = 1;
                shakes.push((0xc, 8));
                c.next = rng.random(0x40) + 8;
            } else if c.t % 6 == 0 {
                shakes.push((0x10, 4));
            }
            sound::priority(0x13e8);
            let v = (0x42a - c.t).min(0x100);
            sound::volumes(v, v);
            sound::keep(sound::key::COLLAPSE, 0x53);
            c.t += 1;
            if c.t > 0x42a {
                c.x = 0x7fff;
                self.particles.remove_group(2);
                self.collapse = Some(c);
                return shakes;
            }
        }
        self.collapse = Some(c);
        shakes
    }
}
