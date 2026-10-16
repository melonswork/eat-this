//! Platform objects (OBJECTS records 100..103, FUN_1060_0166 / FUN_1060_0819): platforms that
//! follow a path (0), swing on an arc (1), fall (2) or sink (3). Only the swinging kind is ported.
//! They are updated from inside HeroUpdate (FUN_1060_145a), before the hero moves, and carry him.

use crate::trig;
use crate::world::ds_word;

#[derive(Clone, Debug)]
pub struct PlatObj {
    pub kind: i32,
    pub sector: usize,
    /// Where its top left is, and where it was last frame (+0x10, +0x12, +0x14).
    pub x: i32,
    pub y: i32,
    pub prev_y: i32,
    /// +0x16 flags, +0x18 angle, +0x1a last angle.
    pub flags: i32,
    pub angle: i32,
    pub prev_angle: i32,
    /// The row in the platform table at 1170:09d0 (+0xe).
    pub row: usize,
    pub speed: i32,
    pub mode: i32,       // +0x24, from the table at 0a68
    pub mode_alt: i32,   // +0x26
    pub count: i32,      // +0x2d
    pub radius: i32,     // +0x2e
    pub lever: (i32, i32), // +0x2c, +0x2a
    pub points: Vec<(i32, i32)>,
    pub bbox: (i32, i32, i32, i32),
    /// Lifts (record 0x14c8, FUN_1060_16ef): the stops (+0x26, +0x28), the direction (+0x24, 0 waiting)
    /// and the speed counter (+0x2e).
    pub stops: (i32, i32),
    pub dir: i32,
    pub speed_count: i32,
    pub age: i32,
}

impl PlatObj {
    fn width(&self) -> i32 {
        ds_word(0x9d0 + self.row * 14)
    }

    fn height(&self) -> i32 {
        ds_word(0x9d6 + self.row * 14) & 0xff
    }

    /// FUN_1060_0166 for a swinging platform (type 1): `rec` is the record's words.
    pub fn swing(rec: &[i16], sector_of: impl Fn(i32, i32) -> usize) -> Option<PlatObj> {
        let n = rec.len();
        let byte = |off: usize| -> i32 {
            let w = rec.get(off / 2).copied().unwrap_or(0) as u16;
            if off % 2 == 0 { (w & 0xff) as i32 } else { (w >> 8) as i32 }
        };
        let word = |i: usize| rec.get(i).copied().unwrap_or(0) as i32;
        let mut count = n as i32;
        let row = byte(4) as usize;
        let key_row = ds_word(0x9d6 + row * 14) >> 8 & 0xff;
        let mut o = PlatObj {
            kind: 1,
            sector: 0,
            x: 0,
            y: 0,
            prev_y: 0,
            flags: 0,
            angle: 0,
            prev_angle: 0,
            row,
            speed: byte(6) as i8 as i32,
            mode: ds_word(0xa68 + byte(2) as usize * 4),
            mode_alt: ds_word(0xa6a + byte(2) as usize * 4),
            count: 0,
            radius: 0,
            lever: (0, 0),
            points: Vec::new(),
            bbox: (0, 0, 0, 0),
            stops: (0, 0),
            dir: 0,
            speed_count: 0,
            age: 0,
        };
        let mut key = -1;
        if key_row != 0xff {
            key = word(n - 1);
            count -= 1;
        }
        let _ = key;
        o.lever.0 = byte(7);
        if o.lever.0 != 0 {
            o.lever.1 = word(4);
        }
        let skip = (o.lever.0 != 0) as i32 + 4;
        o.count = (count - skip - 1) / 2;
        let at = |w: i32| word(w as usize);
        o.radius = byte(((o.lever.0 != 0) as usize + 6) * 2);
        let p0 = (at(skip), at(skip + 1));
        o.points.push(p0);
        let l = (o.lever.0 != 0) as i32;
        if o.count == 2 {
            let p1 = (at(l + 7), at(l + 8));
            o.points.push(p1);
            o.angle = (p1.0 + p1.1) / 2;
        } else if count - skip == 4 {
            o.angle = at(l + 7);
        } else {
            o.angle = 0x5a0;
        }
        o.place(o.angle);
        let r = ds_word(0x9d6 + row * 14) as i32 & 0xff;
        let r = r * o.radius;
        let (cx, cy) = p0;
        // FUN_1060_14f4 adds the platform's width to the right edge, then 0x40 all round.
        o.bbox = (cx - r - 0x40, cy - r - 0x40, cx + r + o.width() + 0x40, cy + r + 0x40);
        o.prev_angle = o.angle;
        o.sector = sector_of(o.y, o.x);
        Some(o)
    }

    /// EnemyFactory 0x14c8 (FUN_1060_14f4 after it): a lift between two stops.
    pub fn lift(rec: &[i16], sector_of: impl Fn(i32, i32) -> usize) -> PlatObj {
        let n = rec.len();
        let word = |i: usize| rec.get(i).copied().unwrap_or(0) as i32;
        let row = word(2) as usize;
        let x = word(3);
        let y = word(4);
        let top = word(5);
        let bottom = word(6) + 1;
        let mut a = top - 0x40;
        let mut b = bottom + 0xf;
        if n > 8 {
            a = word(8);
        }
        if n > 9 {
            b = word(9);
        }
        let mut o = PlatObj {
            kind: 4,
            sector: sector_of(y, x),
            x,
            y,
            prev_y: y,
            flags: 0,
            angle: 0,
            prev_angle: 0,
            row,
            speed: 0,
            mode: 0,
            mode_alt: 0,
            count: 0,
            radius: 0,
            lever: (0, 0),
            points: Vec::new(),
            bbox: (0, 0, 0, 0),
            stops: (top, bottom),
            dir: 0,
            speed_count: 0,
            age: 0,
        };
        o.bbox = (x - 0x10, a - 0x10, x + 0x10 + o.width(), b + 0x10);
        o
    }

    /// FUN_1060_0002: the top left for an angle.
    fn place(&mut self, angle: i32) {
        let (px, py) = self.points[0];
        let h = self.height();
        let r = self.radius * h;
        let half = self.width() / 2;
        self.x = px + ((r * trig::cos(angle)) / 0x4000 - half) + 1;
        self.y = py - ((r * trig::sin(angle)) / 0x4000 + h / 2);
        self.prev_y = self.y;
    }
}

/// What the platform objects see of the hero and change in him.
pub struct HeroSide {
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
    pub vy: i32,
    pub state: i32,
    pub half_width: i32,
    pub prev_ground: i32,
    /// Set to 0x15 when he stands on one.
    pub ground: Option<i32>,
}

fn ranges_overlap(a: i32, b: i32, c: i32, d: i32) -> bool {
    let (a1, a2) = (a.min(b), a.max(b));
    let (c1, c2) = (c.min(d), c.max(d));
    a1 <= c2 && c1 <= a2
}

/// FUN_1060_0819 for a swinging platform.
pub fn update(p: &mut PlatObj, hero: &mut HeroSide, now: u32) {
    let old_x = p.x;
    p.prev_y = p.y;
    let mut active = true;
    let mode = p.mode;
    if p.lever.0 != 0 {
        eprintln!("warning: a platform object with a lever (FUN_1060_0819) not ported");
    }
    if mode & 0x40 != 0 {
        active = true;
    }
    let (mut nx, mut ny, mut angle) = (p.x, p.y, p.angle);
    if active || p.flags & 8 != 0 {
        if mode & 3 == 0 {
            angle = p.angle + p.speed;
        }
        if p.count == 2 {
            // FUN_1060_00c7: swings between the two angles.
            let (w0, w1) = (p.points[1].0, p.points[1].1);
            let d = (w1 - w0).abs();
            let t = (p.speed as i64 * now as i64).rem_euclid(0x1680) as i32;
            let s = trig::sin(t) + 0x4000;
            angle = w0 + ((d as i64 * s as i64) / 0x8000) as i32;
        } else {
            angle = (angle + 0x1680) % 0x1680;
        }
        let mut q = p.clone();
        q.place(angle);
        nx = q.x;
        ny = q.y;
    }
    let w = p.width();
    let rel = (hero.x + hero.dx) - old_x;
    let (lo, hi) = (-hero.half_width, w - 1 + hero.half_width);
    let inside = rel >= lo.min(hi) && rel <= lo.max(hi);
    let mut on = inside && ranges_overlap(ny - 1, p.prev_y - 1, hero.y + hero.dy, hero.y);
    if on {
        let climbing = hero.state < 0 && (-5..=-1).contains(&hero.state);
        on = !climbing
            && (hero.state < 1
                || hero.vy >= 0
                || (hero.y <= p.prev_y - 1 && ny - 1 < hero.y + hero.dy));
    }
    if p.flags & 0x10 != 0 {
        p.flags &= !0x10;
        if p.prev_y < ny {
            active = false;
            angle = p.prev_angle;
            nx = old_x;
            ny = p.prev_y;
        } else {
            p.prev_angle = p.angle;
        }
    } else {
        p.prev_angle = p.angle;
    }
    if std::env::var("ET_PLAT").is_ok() {
        eprintln!("PLAT {now} at ({nx},{ny}) old ({old_x},{}) angle {angle} hero ({},{}) d({},{}) vy {} state {} on {on} w {w}", p.prev_y, hero.x, hero.y, hero.dx, hero.dy, hero.vy, hero.state);
    }
    if on {
        if active && mode & 3 != 0 && hero.prev_ground != 0x15 && mode & 0x80 == 0 {
            p.flags |= 0x10;
        }
        hero.dy = (ny - 1) - hero.y;
        if mode & 8 == 0 {
            hero.dx += nx - old_x;
        }
        hero.ground = Some(0x15);
    }
    if !active || (!on || mode & 0x40 != 0) {
        p.flags &= !1;
    } else if p.speed.abs() < angle.abs() {
        p.flags |= 1;
    }
    if active {
        p.angle = angle;
        p.x = nx;
        p.y = ny;
    }
    if p.angle == p.prev_angle {
        p.flags &= !2;
    } else if p.x != old_x || p.y != p.prev_y {
        p.flags |= 2;
    }
}

/// FUN_1060_16ef (with 1689, 15a6, 152f): a lift. Returns the Random() calls it makes through
/// `rng` (a shake when it passes a marker tile) and whether the Down key was used.
pub fn update_lift(
    p: &mut PlatObj,
    hero: &mut HeroSide,
    world: &crate::world::World,
    rng: &mut crate::rng::PascalRandom,
    down: bool,
    released_down: &mut bool,
    shakes: &mut Vec<(i32, i32)>,
) {
    p.prev_y = p.y;
    let rel = (hero.x + hero.dx) - p.x;
    let w = p.width();
    let (top, bottom) = p.stops;
    let mut start = |p: &mut PlatObj, dir: i32| {
        if p.dir == 0 {
            p.dir = dir;
        }
    };
    if p.dir == 0 {
        p.age = 0;
        if -0x18 < rel && rel < w + 0x17 {
            if p.y == bottom {
                if (hero.y - top).abs() < 0x18 {
                    start(p, -1);
                }
            } else if (hero.y - bottom).abs() < 0x18 {
                start(p, 1);
            }
        }
    } else {
        if p.y < top + 0x24 {
            p.speed_count += p.dir;
            if p.speed_count < 1 {
                p.dir = 0;
            }
            p.y = top + (p.speed_count * p.speed_count) / 0x1c;
        } else if bottom - 0x24 < p.y {
            p.speed_count -= p.dir;
            if p.speed_count < 1 {
                p.dir = 0;
            }
            p.y = bottom - (p.speed_count * p.speed_count) / 0x1c;
        } else {
            p.y += p.dir * 3;
            p.speed_count = 0x1f;
        }
        p.age += 1;
        if (p.prev_y >> 4) != (p.y >> 4) {
            let mut x = p.x + 7;
            for _ in 0..=((w - 1) >> 4) {
                if world.tile_function_in(2, p.y, x) == 0x3336 {
                    shakes.push((0x10, 4));
                    rng.random(0x18);
                }
                x += 0x10;
            }
        }
    }
    if rel <= -hero.half_width || w + hero.half_width - 1 <= rel {
        return;
    }
    if hero.state < 1 || (hero.vy < 0 && hero.y <= p.y - 1) {
        if hero.state > 0 || (-5..=-1).contains(&hero.state) {
            return;
        }
    }
    let mut stand = |p: &mut PlatObj, hero: &mut HeroSide| {
        // FUN_1060_1689: Down (pressed anew) rides it the other way.
        if down && *released_down && hero.prev_ground != 9 {
            if p.dir == 0 {
                p.dir = if top < p.y { -1 } else { 1 };
            } else {
                p.dir = -p.dir;
            }
            *released_down = false;
        }
        hero.ground = Some(0x15);
    };
    if ranges_overlap(p.y - 1, p.prev_y - 1, hero.y + hero.dy, hero.y) {
        let d = (p.y - 1) - hero.y;
        if -1 < d || d < hero.dy {
            hero.dy = d;
        }
        stand(p, hero);
    }
    let pillar = ds_word(0x9d8 + p.row * 14);
    if pillar != 0 && ranges_overlap(p.y + pillar, p.prev_y + pillar, hero.y + hero.dy, hero.y) {
        hero.dy = (p.y + pillar) - hero.y;
        stand(p, hero);
    }
}
