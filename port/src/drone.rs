//! The hero's drone (pick-up effect 6, FUN_1088_03a3): it follows the hero (FUN_1088_9d34, after
//! the actors), turns towards the target the hit tests picked and shoots at it. Globals
//! 1170:4d9e..4dbc. A second pick-up upgrades it (4dbc).

use crate::hero::Hero;
use crate::level::Level;
use crate::particles::Particles;
use crate::rng::PascalRandom;
use crate::trig;
use crate::world::World;

/// "No target" in `dist` (1170:4db4).
const NO_TARGET: i32 = 0x120;

/// What `hit_test` found (FUN_1050_568b's drone part), judged in `Level::drone_update` because
/// the line of sight needs the world.
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    /// The box centre.
    cx: i32,
    cy: i32,
    /// The drone's position when it was looked at (4d9e / 4da0).
    x: i32,
    y: i32,
    dist: i32,
    level: i32,
}

#[derive(Debug, Default)]
pub struct Drone {
    pub x: i32,      // 4d9e
    pub y: i32,      // 4da0
    bob_y: i32,      // 4da2: y with the hovering added
    fx: i32,         // 4da6:4da4 (x << 16 + fraction)
    fy: i32,         // 4daa:4da8
    pub angle: i32,  // 4dac
    pub active: i32, // 4dae
    target: (i32, i32), // 4db0 / 4db2 (relative until the next update)
    dist: i32,       // 4db4
    prio: i32,       // 4db6
    reload: i32,     // 4db8
    burst: i32,      // 4dba
    upgraded: bool,  // 4dbc
    cands: Vec<Candidate>,
}

impl Drone {
    /// 1170:4da2, the hovering y the drone is drawn at.
    pub fn bob_y(&self) -> i32 {
        self.bob_y
    }

    /// FUN_1088_03a3(y, x).
    pub fn pickup(&mut self, y: i32, x: i32) {
        if std::env::var("ET_DEBUG").is_ok() {
            eprintln!("drone pick-up at {y},{x}");
        }
        if self.active < 1 {
            self.active = 1;
            self.angle = 0;
            self.dist = NO_TARGET;
            self.prio = 0;
        } else {
            self.upgraded = true;
        }
        self.reload = 0;
        self.burst = 0;
        self.x = x;
        self.y = y;
        self.fx = x << 16;
        self.fy = y << 16;
    }

    /// FUN_1088_0c20: the hero is placed (a teleport): the drone goes to him at once.
    pub fn place(&mut self, y: i32, x: i32) {
        self.x = x;
        self.y = y - 0x15;
        self.fx = x << 16;
        self.fy = self.y << 16;
    }

    /// The drone part of FUN_1050_568b / 51aa: a box the target may be.
    #[track_caller]
    pub fn consider(&mut self, bottom: i32, right: i32, top: i32, left: i32, level: i32) {
        if self.active <= 0 {
            return;
        }
        let (cx, cy) = ((left + right) / 2, (top + bottom) / 2);
        let (dx, dy) = (cx - self.x, cy - self.y);
        let dist = ((dx as f64).powi(2) + (dy as f64).powi(2)).sqrt().round_ties_even() as i32;
        if std::env::var("ET_DRONE").is_ok() {
            eprintln!("CAND box l{left} t{top} r{right} b{bottom} level {level} dist {dist} {}", std::panic::Location::caller());
        }
        self.cands.push(Candidate { cx, cy, x: self.x, y: self.y, dist, level });
    }

    /// FUN_1050_0a63 (called by the enemy rockets homing in on it): a decoy flare flies off.
    pub fn flare_due(&self) -> bool {
        self.active > 0
    }
}

/// FUN_1068_0a54(ty, tx, y0, x0): nothing solid (bit 0x40) on the line between the points,
/// sampled once per tile.
fn line_of_sight(w: &World, lv: &Level, ty: i32, tx: i32, y0: i32, x0: i32) -> bool {
    let solid = |y: i32, x: i32| Hero::solidity_all(w, lv, y, x) & 0x40 != 0;
    let (dx, dy) = (tx - x0, ty - y0);
    if dx.abs() > dy.abs() {
        let mut lx = (x0 & !0xf) + 7;
        if x0 < tx {
            while lx < tx {
                if solid(y0 + (lx - x0) * dy / dx, lx) {
                    return false;
                }
                lx += 0x10;
            }
        } else {
            while lx > tx {
                if solid(y0 + (lx - x0) * dy / dx, lx) {
                    return false;
                }
                lx -= 0x10;
            }
        }
        !solid(ty, tx)
    } else {
        let mut ly = (y0 & !0xf) + 7;
        if y0 < ty {
            while ly < ty {
                if solid(ly, x0 + (ly - y0) * dx / dy) {
                    return false;
                }
                ly += 0x10;
            }
        } else if dy != 0 {
            while ly > ty {
                if solid(ly, x0 + (ly - y0) * dx / dy) {
                    return false;
                }
                ly -= 0x10;
            }
        } else {
            return true;
        }
        !solid(ty, tx)
    }
}

/// FUN_1088_9ca4: a 9x9 spot around the point touching a wall.
fn blocked(w: &World, y: i32, x: i32) -> bool {
    [(-4, -4), (-4, 4), (4, -4), (4, 4)]
        .iter()
        .any(|&(dx, dy)| w.solidity_at(y + dy, x + dx) & 2 != 0)
}

/// One axis of the follow: a quadratic ease of the 16.16 position towards `goal`.
fn ease(fixed: i32, goal: i32) -> i32 {
    let q = (goal << 16).wrapping_sub(fixed) / 0x1000;
    fixed.wrapping_add(q.wrapping_mul(q.wrapping_abs()))
}

impl Level {
    /// FUN_1088_9d34. Returns whether the drone fired (1170:465e = 2).
    pub fn drone_update(&mut self, w: &World, hero: &Hero, now: u32) -> bool {
        // The hit tests' targets, in the order they were found.
        let cands = std::mem::take(&mut self.particles.drone.cands);
        for c in cands {
            let d = &self.particles.drone;
            if d.prio > c.level || !(c.dist < d.dist || d.prio < c.level) {
                continue;
            }
            if line_of_sight(w, self, c.cy, c.cx, c.y, c.x) {
                let d = &mut self.particles.drone;
                d.prio = c.level;
                d.target = (c.cx - c.x, c.cy - c.y);
                d.dist = c.dist;
            }
        }
        let ps = &mut self.particles;
        if ps.drone.active <= 0 {
            return false;
        }
        if std::env::var("ET_DRONE").is_ok() {
            let d = &ps.drone;
            eprintln!("DRONE {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", now + 1, d.x, d.y, d.bob_y, d.angle, d.target.0, d.target.1, d.dist, d.reload, d.burst);
        }
        let d = &mut ps.drone;
        d.reload = (d.reload - 1).max(0);
        d.burst = (d.burst - 1).max(0);
        if d.dist != NO_TARGET {
            d.target.0 += d.x;
            d.target.1 += d.y;
        }
        // Where to hover: above the hero, else below, else behind, else in front.
        let (hx, hy) = (hero.x, hero.y);
        let (mut gx, mut gy) = (hx, hy - hero.height - 0x14);
        if blocked(w, gy, gx) {
            gy = hy + 0x10;
            if blocked(w, gy, gx) {
                gx = hx - hero.facing * 0x1a;
                gy = hy - hero.height / 2;
                if blocked(w, gy, gx) {
                    gx = hx + hero.facing * 0x1a;
                }
            }
        }
        if std::env::var("ET_DRONE").is_ok() {
            eprintln!("GOAL {} {gx},{gy} hero {hx},{hy} h {} f {}", now + 1, hero.height, hero.facing);
        }
        if d.x != gx {
            d.fx = ease(d.fx, gx);
        }
        if d.y != gy {
            d.fy = ease(d.fy, gy);
        }
        d.x = d.fx >> 16;
        d.y = d.fy >> 16;
        let phase = (now as i32).wrapping_mul(5 * 0x10) % trig::TURN;
        d.bob_y = trig::sin(phase) / 0x1999 + d.y;
        let mut fired = false;
        if d.dist == NO_TARGET {
            d.angle = (d.angle + 0x5a) % trig::TURN;
            return false;
        }
        let a = trig::atan2(d.target.1 - d.bob_y, d.target.0 - d.x);
        d.angle = ((a / 0xb40) * 0xb40 + d.angle % 0xb40) % trig::TURN;
        if d.angle != a {
            let diff = ((a - d.angle) + 0x21c0) % trig::TURN;
            if diff < 0xb40 {
                let step = 0x40.min(0xb40 - diff);
                d.angle = ((d.angle + trig::TURN) - step) % trig::TURN;
            } else {
                let step = 0x40.min(diff - 0xb40);
                d.angle = (step + d.angle) % trig::TURN;
            }
        }
        if d.reload < 1 {
            let off = (d.angle - a + 0x21c0) % trig::TURN - 0xb40;
            if off.abs() <= 0x20 {
                fired = true;
                if std::env::var("ET_DEBUG").is_ok() {
                    eprintln!("drone fires at frame {now} angle {} target {:?} pos {},{}", d.angle, d.target, d.x, d.y);
                }
                let (mx, my) = (d.x + 10 * trig::cos(d.angle) / 0x4000, d.bob_y - 10 * trig::sin(d.angle) / 0x4000);
                d.x += (d.x - mx) / 2;
                d.y += (d.bob_y - my) / 2;
                d.fx = (d.x << 16) | (d.fx & 0xffff);
                d.fy = (d.y << 16) | (d.fy & 0xffff);
                let (angle, upgraded) = (d.angle, d.upgraded);
                ps.p.group = 0;
                ps.p.hits = 1;
                ps.p.damage = 10;
                ps.p.on_death = 0x3e;
                ps.p.callback = 0x2d2;
                ps.spawn(if upgraded { 0x1910 } else { 0x191e }, my, mx);
                ps.line(0x60, angle);
                ps.p.callback = 0;
                ps.p.on_death = 0;
                ps.p.damage = 0x10;
                ps.p.hits = 0;
                ps.p.group = 1;
                ps.spawn_seq(if upgraded { 0x1952 } else { 0x190a }, 2, 2, my, mx, 1);
                let d = &mut ps.drone;
                if d.burst < 0x18 {
                    if !d.upgraded {
                        d.reload = 0xc;
                        d.burst += 0x18;
                    } else {
                        d.reload = 8;
                    }
                } else {
                    d.reload = 0x52;
                }
            }
        }
        let d = &mut ps.drone;
        d.dist = NO_TARGET;
        d.prio = 0;
        fired
    }
}

impl Particles {
    /// FUN_1050_0a63: the drone drops a decoy flare when a rocket homes in on it.
    pub fn drone_flare(&mut self, rng: &mut PascalRandom) {
        if self.drone.reload >= 2 {
            return;
        }
        let angle = rng.random(0x201) + 0x4a0;
        let d = &mut self.drone;
        let (mx, my) = (d.x + 10 * trig::cos(angle) / 0x4000, d.y - 10 * trig::sin(angle) / 0x4000);
        d.angle = angle;
        d.x += (d.x - mx) / 2;
        d.y += (d.y - my) / 2;
        d.fx = (d.x << 16) | (d.fx & 0xffff);
        d.fy = (d.y << 16) | (d.fy & 0xffff);
        d.reload = 0x52;
        self.flare(angle, my, mx);
    }

    /// FUN_1050_09f8(angle, y, x): the flare thrown off.
    fn flare(&mut self, angle: i32, y: i32, x: i32) {
        self.p.group = 0;
        self.p.colour = 0x28;
        self.p.callback = 0x274f;
        self.p.on_death = 0x40;
        self.p.flags = 0x80;
        self.spawn_seq(0x1950, 1, 0xd2, y, x, 0);
        self.throw(0x1c0, 4, angle);
        self.p.flags = 0x84;
        self.p.on_death = 0;
        self.p.callback = 0;
        self.p.colour = 0;
        self.p.group = 1;
    }
}
