//! Sound emitters (record 0x13c4, FUN_1068_315a). The plain ones just play; the four special
//! ones (10000, 0x2711..0x2713: wind, birds, thunder, ...) draw Random() for their pitch and when
//! to sound next. Whether a sound is heard (FUN_10e0_016b) depends on the listener (the camera's
//! centre) and the sound's box, so it has to be computed.

use crate::actors::Env;
use crate::level::Level;

/// FUN_10e0_016b(x1, y1, x2, y2) with the volume `vol`: is the sound audible in either channel?
fn audible(env: &Env, vol: i32, (x1, y1, x2, y2): (i32, i32, i32, i32)) -> bool {
    let (lx, ly) = (env.camera.0 + 0xa0, env.camera.1 + 100);
    let corner = |py: i32, px: i32| {
        let (dx, dy) = (px - lx, (py - ly) * 2);
        (((dx * dx + dy * dy) as f64).sqrt().round_ties_even() as i32, dx)
    };
    let (dist, pan) = if lx < x1 {
        if ly < y1 {
            corner(y1, x1)
        } else if y2 < ly {
            corner(y2, x1)
        } else {
            (x1 - lx, x1 - lx)
        }
    } else if x2 < lx {
        if ly < y1 {
            corner(y1, x2)
        } else if y2 < ly {
            corner(y2, x2)
        } else {
            (-(x2 - lx), x2 - lx)
        }
    } else if ly < y1 {
        ((y1 - ly) * 2, 0)
    } else if y2 < ly {
        ((ly - y2) * 2, 0)
    } else {
        (0, 0)
    };
    let v = (vol - dist / 2).max(0);
    let (mut a, mut b) = (v, v);
    if pan > 0 {
        a = (v << 8) / (pan + 0x100);
    } else if pan < 0 {
        b = (v << 8) / (0x100 - pan);
    }
    !(a <= 0x10 && b <= 0x10)
}

impl Level {
    /// FUN_1068_315a.
    pub(crate) fn emitter(&mut self, id: usize, env: &mut Env) {
        let o = self.o(id);
        let (cond, cell, kind, vol) = (o.w(0x1e), o.w(0x1c), o.w(0x16), o.w(0x18));
        let rect = (o.w(0xe), o.w(0x10), o.w(0x12), o.w(0x14));
        if cond >= 1 {
            let state = Self::lever_state(env.world, cell as u16).max(0);
            if cond - 1 != state {
                return;
            }
        }
        if kind < 10000 {
            return;
        }
        let now = self.particles.now;
        match kind {
            10000 => {
                if self.amb[0] <= now {
                    let v = (vol - env.rng.random(200)).max(0);
                    if audible(env, v, rect) {
                        env.rng.random(0x200);
                        env.rng.random(2);
                    }
                    self.amb[0] = now + 0x14 + env.rng.random(0x230) as u32;
                }
            }
            0x2711 => {
                if audible(env, 0x100, rect) && self.amb[1] <= now {
                    env.rng.random(0x80);
                    env.rng.random(0x80);
                    env.rng.random(0x41);
                    env.rng.random(2);
                    self.amb[1] = now + 0x23 + env.rng.random(0x348) as u32;
                }
            }
            0x2712 => {
                if audible(env, vol, rect) && self.amb[2] <= now {
                    self.amb[2] = now + 0x8c + env.rng.random(0x6d6) as u32;
                    env.rng.random(0x96);
                    env.rng.random(100);
                    env.rng.random(100);
                    env.rng.random(2);
                }
            }
            0x2713 => {
                if self.amb[0] <= now {
                    let v = (vol - env.rng.random(200)).max(0);
                    let _ = audible(env, v, rect);
                    self.amb[0] = now + 0x15e + env.rng.random(0x834) as u32;
                }
            }
            _ => {}
        }
    }
}
