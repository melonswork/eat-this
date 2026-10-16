//! Things that lie around: blood drops on the floor (0xbb0, FUN_1068_06e9), shell casings
//! (0xbc4, FUN_1068_0852) and blood splats on walls (0x976, FUN_1058_0301). They live in their
//! sector's list +0x5a (`list::L5A`), at most 0x20 of them: a new one evicts the first that is
//! off screen (FUN_1068_04d0). FUN_1070_fb30 updates the hero's sector's list.

use crate::actors::Env;
use crate::level::Level;
use crate::objects::{Obj, list};
use crate::rng::PascalRandom;
use crate::world::{MAIN_PLANE, World};

/// Object kinds (their VMT offsets).
pub const DROP: u16 = 0xbb0;
pub const SHELL: u16 = 0xbc4;
pub const SPLAT: u16 = 0x976;

/// FUN_1058_016f(y, x): the 2x3 tiles at (x - 8 ..) are all wall (flag 0x800), or have a back
/// wall behind (plane 0 solid 0x800) and a main-plane tile whose record starts with 4 (the cell
/// is the one TileSolidityAt looked up first).
fn wall_behind(world: &World, mut y: i32, x: i32) -> bool {
    for _ in 0..3 {
        for col in 0..2 {
            let a = x - 8 + 16 * col;
            if world.solidity_at(y, a) & 0x800 == 0 {
                let back = world.solidity_in(0, y, a);
                if back & 0x800 == 0 || world.cell_byte0(MAIN_PLANE, y, a) != Some(4) {
                    return false;
                }
            }
        }
        y += 16;
    }
    true
}

impl Level {
    /// FUN_1068_0551 + 0663/06a6: a drop or a shell lands where there is floor under both
    /// edges. Returns whether it was made.
    pub fn lay_decal(&mut self, world: &World, kind: u16, draw: i32, sprite: i32, y: i32, x: i32) -> bool {
        for dx in [2, -2] {
            let s = world.solidity_at(y + 1, x + dx);
            if s & 0x201 == 0 || s & 0x20 != 0 || world.solidity_at(y, x + dx) & 0x20 != 0 {
                return false;
            }
        }
        let mut o = Obj::new(kind, kind, 0x24);
        o.set(0xc, 1);
        o.set(0x1a, 0);
        o.set(0x1c, 0);
        o.set(0xe, x);
        o.set(0x10, y);
        o.set(0x20, sprite);
        o.set(0x22, draw);
        self.add_decal(world, o, (x - 0x20, y - 0x10, x + 0x20, y + 0x30));
        true
    }

    /// FUN_1058_0218(sprite, y, x): a blood splat on the wall behind.
    pub fn blood_splat(&mut self, world: &World, sprite: i32, y: i32, x: i32) -> bool {
        let ok = wall_behind(world, y, x);
        if std::env::var("ET_SPLAT").is_ok() {
            eprintln!("SPLAT {} sprite {sprite:#x} at ({x},{y}) wall {ok}", self.particles.now);
        }
        if !ok {
            return false;
        }
        let mut o = Obj::new(SPLAT, SPLAT, 0x14);
        o.set(10, x);
        o.set(0xc, y);
        o.set(0xe, sprite);
        o.set_l(0x10, self.particles.now as i32);
        self.add_decal(world, o, (x - 0x20, y - 0x10, x + 0x20, y + 0x30));
        true
    }

    fn add_decal(&mut self, world: &World, mut o: Obj, (x1, y1, x2, y2): (i32, i32, i32, i32)) {
        o.set(2, x1);
        o.set(4, y1);
        o.set(6, x2);
        o.set(8, y2);
        let s = world.sector_at(y1, x1);
        if self.lists[s][list::L5A].len() > 0x1f {
            // FUN_1068_04d0: the first one that is off screen goes, else the oldest.
            let (l, t, r, b) = (self.cam.0, self.cam.1, self.cam.0 + 0x13f, self.cam.1 + 199);
            let at = self.lists[s][list::L5A]
                .iter()
                .position(|&id| {
                    let (a, c, d, e) = self.objects[id].as_ref().unwrap().bbox();
                    d < l || e < t || r < a || b < c
                })
                .unwrap_or(0);
            let id = self.lists[s][list::L5A].remove(at);
            self.objects[id] = None;
        }
        self.add_to(s, list::L5A, o);
    }

    /// FUN_1070_fb30.
    pub fn update_decals(&mut self, env: &mut Env, window: (i32, i32, i32, i32)) {
        let sector = self.sector.unwrap_or(0);
        self.walk(sector, list::L5A, env, window);
    }

    /// A blood drop (FUN_1068_06e9): falls; the hero landing on it squashes it.
    pub(crate) fn drop_decal(&mut self, id: usize, env: &mut Env) {
        {
            let o = self.o(id);
            let (x, y) = (o.w(0xe), o.w(0x10));
            o.set(2, x - 0x20);
            o.set(4, y - 0x10);
            o.set(6, x + 0x20);
            o.set(8, y + 0x30);
        }
        if self.actor_physics(id, env, 2, 2, 2) >= 1 {
            self.remove_current(id);
            return;
        }
        let (x, y, sprite) = (self.o(id).w(0xe), self.o(id).w(0x10), self.o(id).w(0x20));
        let h = env.h;
        let landed = h.state >= 1 && h.vy >= 0 && y > h.prev_y && h.y >= y && x + 5 >= h.x && h.x >= x - 5;
        if landed {
            env.rng.random(0x40); // the sound's pitch
            self.particles.spawn_seq((sprite & 0xff00) + 0x19, 4, 5, y, x, 1);
            self.remove_current(id);
        }
    }

    /// A shell casing (FUN_1068_0852).
    pub(crate) fn shell_decal(&mut self, id: usize, env: &mut Env) {
        {
            let o = self.o(id);
            let (x, y) = (o.w(0xe), o.w(0x10));
            o.set(2, x - 0x20);
            o.set(4, y - 0x10);
            o.set(6, x + 0x20);
            o.set(8, y + 0x30);
        }
        if self.actor_physics(id, env, 1, 1, 1) >= 1 {
            self.remove_current(id);
        }
    }

    /// A blood splat (FUN_1058_0301): once the wall behind it is gone it bursts into drops; the
    /// first kind drips now and then.
    pub(crate) fn splat(&mut self, id: usize, env: &mut Env) {
        let (x, y, sprite, born) = {
            let o = self.o(id);
            (o.w(10), o.w(0xc), o.w(0xe), o.l(0x10) as u32)
        };
        let ps = &mut self.particles;
        if std::env::var("ET_SPLAT").is_ok() {
            eprintln!("SPLAT update {} at ({x},{y}) wall {}", ps.now, wall_behind(env.world, y, x));
        }
        if !wall_behind(env.world, y, x) {
            ps.p.group = 0;
            ps.spawn_seq(sprite & 0xff00, 4, 5, y + 0xe, x, 1);
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x16;
            ps.p.callback = 0x23b7;
            for i in 0..4 {
                ps.p.delay = env.rng.random(4);
                let r = env.rng.random(3);
                ps.spawn((sprite & 0xff00) + 6 + r, y + 0xe, x - 6 + i * 4);
                ps.throw(7, 0x10, i * 0x140 + 0xf00);
            }
            ps.p.callback = 0;
            ps.p.on_death = 0;
            ps.p.hits = 0;
            ps.p.group = 1;
            self.remove_current(id);
        } else {
            let now = ps.now;
            let age = now.wrapping_sub(born);
            let t = 0x200i32.wrapping_sub((age >> 1) as i32);
            if sprite == 0x531e && t > 0x180 && now % 0x46 == 0 {
                ps.p.group = 0;
                ps.p.hits = 0x8000;
                ps.p.on_death = 0x16;
                ps.p.callback = 0x23b7;
                ps.spawn(0x5309, y + 0x18, x);
                ps.throw(7, 0, 0x10e0);
                ps.p.callback = 0;
                ps.p.on_death = 0;
                ps.p.hits = 0;
                ps.p.group = 1;
            }
        }
    }
}

/// Splat sprite for a hit: FUN_1058_0218(Random(5) + 0x531e, ...).
#[allow(dead_code)]
pub fn splat_sprite(rng: &mut PascalRandom) -> i32 {
    rng.random(5) + 0x531e
}

#[allow(dead_code)]
const _: usize = MAIN_PLANE;
