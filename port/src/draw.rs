//! The draw pass after a frame's update, in the original's order (GameFrameInGame): the
//! sector's background objects and the tiles (4c32), the platforms (FUN_1078_0abd), the decals
//! (FUN_1070_fb30), the platform objects (FUN_1060_14a7), particle group 0, the actors
//! (FUN_1070_faec: their update methods draw them), the drone (FUN_1088_9d34), particle group 1,
//! then the HUD (FUN_1088_4b64). Each object kind's drawing is its update method's draw tail,
//! read from the decompilation; it only reads the state.
//!
//! Known gaps: a particle that dies this frame was still drawn by the original that frame (it
//! is gone from the port's state when the draw runs). Scaled sprites (`FUN_10f0_302b`) use
//! nearest-pixel sampling; the original's run handlers may round differently.

use crate::level::Level;
use crate::objects::{Look, Obj, list};
use crate::particles::Particle;
use crate::render::{Mix, Painter};

/// The hit flash colour 1170:1b1b (its value at level start).
pub const FLASH: u8 = 7;

/// The level frame counter divided by `d` (the long divisions the draw code does with 617e).
fn per(p: &Painter, d: u32) -> i32 {
    (p.frame / d) as i32
}

/// FUN_1050_4ca5's drawing for `group`: the record's flips (+0x18, +0x1a) and mode (+0x20),
/// its sprite +0x16 at (+8, +0xa), or its per-frame procedure's own drawing.
pub fn particles(p: &mut Painter, level: &Level, world: &crate::world::World, group: i32) {
    for part in &level.particles.list {
        if part.m.w(0) != group {
            continue;
        }
        particle(p, part, world);
        p.reset();
    }
}

fn particle(p: &mut Painter, part: &Particle, world: &crate::world::World) {
    let m = &part.m;
    let (x, y) = (m.w(8), m.w(0xa));
    p.hflip = m.b(0x18) != 0;
    p.vflip = m.b(0x1a) != 0;
    p.mode = m.w(0x20);
    let age = m.w(0x24);
    let step = m.w(0x26).max(1);
    let n = age / step;
    let mut sprite = m.w(0x16);
    // The tumbling debris procedures: every 4th step flips both ways.
    let tumble = |p: &mut Painter| {
        if n & 4 != 0 {
            p.hflip = !p.hflip;
            p.vflip = true;
        }
    };
    match m.uw(0x42) {
        0x126 => sprite = 0x5638 + ((age + m.w(0x40)) & 1),
        0x1a3 | 0x30f => p.mode = 0x10,
        0x229 | 0x17f4 => {
            tumble(p);
            sprite += n & 3;
        }
        0x177a => {
            tumble(p);
            sprite += n & 1;
        }
        0x13c3 => {
            tumble(p);
            sprite = m.w(0x14) + (n & 3);
        }
        0x2d2 => sprite = m.w(0x14) + ((age >> 1) & 3),
        0x586 | 0x7bf | 0xb2a | 0xd83 | 0xf9f => {
            // The rockets and the torpedo: one of 10 sprites per quarter turn of the angle
            // (motion record +0x10), mirrored into the other quarters.
            let a = part.v.w(0x10);
            p.vflip = a > 0xb3f;
            let b = a + 0x5a0;
            p.hflip = b > 0xb3f && b < 0x1680;
            let k = (a.rem_euclid(0x5a0) + 0x50) / 0xa0;
            sprite = if p.hflip == p.vflip {
                k + m.w(0x14)
            } else {
                m.w(0x14) + 9 - k
            };
        }
        0x1cef => {
            p.mode = 0x10;
            sprite = age / 0x12 + m.w(0x14);
        }
        0x2299 => sprite += (age / 4) % 2,
        0x2459 | 0x2497 | 0x2946 => sprite += (age >> 2) & 1,
        0x232e => {
            // Far away: half the camera's scroll.
            p.sprite(sprite, x - (p.cam.0 >> 1), y - (p.cam.1 >> 1));
            return;
        }
        0x236b => {
            p.mode = 0x34;
            p.colour = 2;
        }
        0x21c5 => sprite = m.w(0x14),
        // Burning ground (napalm and the like): a flame scaled by +0x1e / +0x1c, cut at the
        // ground. (The original grows it while it dies out.)
        0x18e6 | 0x19f8 | 0x1b90 => {
            p.clip.3 = (y - p.cam.1).min(p.clip.3);
            match m.uw(0x42) {
                0x18e6 => p.flame(m.w(0x1e), m.w(0x1c), age as u32, x, y),
                0x19f8 => {
                    // A column of smoke over it.
                    p.scaled_at(0x100, 0x400, (age / 4) % 0xd + 0x6b00, x, y);
                    p.flame(0x300, 0x200, age as u32, x, y);
                }
                _ => p.flame(0xa00, 0x755, age as u32, x, y),
            }
            return;
        }
        // Smoke and debris drawn scaled: +0x40 both ways (with an animation of 8), or
        // +0x1c across and +0x1e down.
        0x186e => {
            let k = m.w(0x40);
            p.scaled_at(k, k, sprite + (n & 7), x, y);
            return;
        }
        0x2148 => {
            p.scaled_at(m.w(0x1e), m.w(0x1c), sprite, x, y);
            return;
        }
        0x2192 => {
            p.scaled_at(0x200, 0x200, sprite, x, y);
            return;
        }
        0x2588 => {
            // A bonus flying out (FUN_1088_022b on its tile +0x40).
            let kind = world.cell_function(m.uw(0x40), 2) as i32;
            p.reset();
            pickup(p, kind, x - p.cam.0, y - p.cam.1);
            return;
        }
        0x1da7 => {
            // A burning thing: its flame behind it, translucent.
            p.mode = 0x10;
            p.flame(0x200, 0x400, age as u32, x, y - 1);
            p.mode = 0;
        }
        _ => {}
    }
    p.at(sprite, x, y);
}

/// The hero's sector's decals (list +0x5a, FUN_1070_fb30).
pub fn decals(p: &mut Painter, level: &Level) {
    let sector = level.sector.unwrap_or(0);
    for &id in &level.lists[sector][list::L5A] {
        let Some(o) = &level.objects[id] else { continue };
        match o.kind {
            crate::decals::DROP => {
                p.hflip = o.w(0x22) != 0;
                p.at(o.w(0x20), o.w(0xe), o.w(0x10));
            }
            crate::decals::SHELL => {
                p.hflip = o.w(0x22) != 0;
                p.at(o.w(0x20), o.w(0xe), o.w(0x10) - 1);
            }
            crate::decals::SPLAT => {
                p.at_mode(0x10, o.w(0xe), o.w(0xa), o.w(0xc));
            }
            _ => {}
        }
        p.reset();
    }
}

/// The actors that updated this frame (FUN_1070_faec): the hero's sector's, then sector 0's,
/// those whose box meets the camera.
pub fn actors(p: &mut Painter, level: &Level) {
    let sector = level.sector.unwrap_or(0);
    let (cx, cy) = p.cam;
    let window = (cx, cy, cx + 0x13f, cy + 199);
    let mut sectors = vec![sector];
    if sector != 0 {
        sectors.push(0);
    }
    for s in sectors {
        for &id in &level.lists[s][list::ACTORS] {
            let Some(o) = &level.objects[id] else { continue };
            let (x1, y1, x2, y2) = o.bbox();
            if x1 <= window.2 && y1 <= window.3 && window.0 <= x2 && window.1 <= y2 {
                actor(p, o);
                p.reset();
            }
        }
    }
}

/// Draws the sprites an update recorded (`Obj::looks`).
fn looks(p: &mut Painter, o: &Obj) {
    for l in &o.looks {
        p.mode = l.mode;
        p.colour = if l.colour == Look::FLASH { p.flash } else { l.colour as u8 };
        p.hflip = l.hflip;
        p.mix = match l.mix {
            1 => Mix::Sea,
            2 => Mix::Light,
            _ => Mix::Level,
        };
        p.at(l.sprite, l.x, l.y);
        p.reset();
    }
}

fn actor(p: &mut Painter, o: &Obj) {
    let (x, y) = (o.w(0xe), o.w(0x10));
    match o.kind {
        // Walker (FUN_1070_4051), walking gunner (FUN_1070_af70): recorded by their updates.
        0x140a => looks(p, o),
        crate::turret::KIND => {
            if o.w(0x30) == 1 {
                p.mode = 0x10;
                p.flame(0x44c, 0x44c, p.frame, x, y - 4);
                p.mode = 0;
            }
            looks(p, o);
        }
        // FUN_1070_1b00: the gunship, rotor then gun, both lit by a hit (rotor 1 px lower, gun
        // 1 px higher); darkened while it falls (mode 0x24, colour 1b14 = 0).
        0x13e2 => {
            let flash = o.w(0x34) > 0;
            for (i, l) in o.looks.iter().enumerate() {
                if flash {
                    p.mode = 0x24;
                    p.colour = p.flash;
                } else {
                    p.mode = l.mode;
                    p.colour = 0;
                }
                let dy = if flash { if i == 0 { 1 } else { -1 } } else { 0 };
                p.at(l.sprite, l.x, l.y + dy);
            }
        }
        // FUN_1070_30b6: the hovering machine: body (shaken by a hit, plus 0x502c in its
        // first stage), then the rotor; flashing while +0x34 >= 0x22e.
        0x13f6 => {
            let (bx, by) = (o.w(0x16), o.w(0x18));
            let body = o.looks.first().map_or(0x7fff, |l| l.sprite);
            let t = o.w(0x34);
            if t < 0x22e {
                let mut yy = by;
                if t > 0x1a4 {
                    yy += 2 - (t < 0x1ac) as i32;
                }
                if body != 0x7fff {
                    p.at(body, bx, yy);
                    if o.w(0x32) < 1 {
                        p.at(0x502c, bx, yy);
                    }
                }
                p.at(o.w(0x26), bx, by);
            } else {
                p.mode = 0x24;
                p.colour = p.flash;
                p.at(o.w(0x26), bx, by - (o.w(0x32) < 3) as i32);
                if body != 0x7fff {
                    p.at(body, bx, by - 1);
                    if o.w(0x32) < 1 {
                        p.at(0x502c, bx, by - 1);
                    }
                }
            }
        }
        // FUN_1070_390f: its wreck: the rotor +0x22 and, while it still hovers, the body.
        crate::boss::WRECK => {
            let body = if o.w(0x20) < 0x101 {
                0x7fff
            } else {
                0x5025 + ((p.frame / 0xc) % 2) as i32
            };
            if o.w(0x24) < 1 {
                p.at(body, x, y);
                p.at(o.w(0x22), x, y);
            } else {
                p.mode = 0x24;
                p.colour = p.flash;
                p.at(o.w(0x22), x, y);
                p.at(body, x, y - 1);
            }
            if o.w(0x20) < 0x101 {
                p.mode = 0x10;
                p.flame(0x200, 0x200, p.frame, x, y - 0xd);
            }
        }
        // FUN_1070_a3cf: the small robot (sprite +0x32; 0x5823 animates), with its
        // jet (+0x2c > 0) and, armed, its gun.
        0x145a => {
            let s = o.w(0x32);
            if o.w(0x2c) > 0 {
                p.at(((o.uw(0x28) / 3) % 2) as i32 + 0x5800, x, y);
                if o.w(10) > 0 {
                    if s == 0x5823 {
                        let u = per(p, 6);
                        if u & 3 > 2 {
                            p.at_mode(0x28, 0x1b07, x - 1, y - 0x11);
                        }
                        p.at((u & 3) + 0x5825, x, y);
                    } else {
                        p.at(0x580f + (per(p, 3) & 1), x, y);
                        p.at_mode(0x10, 0x580c - (o.uw(0x2e) >> 4) as i32, x, y);
                    }
                }
            }
            if o.w(0x2c) < 1 {
                p.flame(800, 0x2d0, p.frame, x, y - 7);
            }
            let s = if s == 0x5823 { 0x5823 + (per(p, 3) & 1) } else { s };
            p.at_flash(o.w(0x26) > 0, s, x, y);
        }
        // FUN_1070_02b3: a mine, blinking faster once it ticks.
        0x13d8 => {
            if o.w(0x20) == 0 {
                p.at(0x4c00 + (per(p, 3) & 1), x, y);
            } else {
                p.at(0x4c02 - ((o.w(0x22) >> 3) & 1), x, y);
            }
        }
        // FUN_1070_90f1: a crate (or a box of another kind: sprite table 1170:0e4a).
        0x143c => {
            let k = o.w(10);
            let mut s = crate_sprite(k);
            if k == 5 && (p.frame & 0xf) < 8 {
                s += 1;
            }
            p.at_flash(o.w(0x22) > 0, s, x, y);
        }
        // FUN_1070_60ca: the small alien; facing from its speed (+0x20).
        0x1428 => {
            p.hflip = o.w(0x20) < 0;
            if o.w(0x28) == -500 && o.w(0x2e) > 7 {
                p.hflip = !p.hflip;
            }
            p.at_flash(o.w(0x2a) > 0, o.w(0x34), x, y);
        }
        // FUN_1070_7eb2: the big alien; its shield tints it through PalMixSea.
        0x1432 => {
            p.hflip = o.w(0x20) < 0;
            if o.b(0x44) != 0 {
                p.mix = Mix::Sea;
                p.mode = 0x24;
                p.colour = 10;
            } else if o.w(0x2a) > 0 {
                p.mode = 0x24;
                p.colour = p.flash;
            }
            p.at(o.w(0x34), x, y);
        }
        // FUN_1070_d23b: a springboard, squashed while +0x20 > 0.
        0x148c => p.at(0x7556 + (o.w(0x20) > 0) as i32, x, y),
        // FUN_1070_243c: the gunship's wreck.
        crate::gunship::WRECK => p.at(0x4e13 - (o.w(0x1a) > 0) as i32, x, y),
        // FUN_1070_a06a: a sprite (+0x14) in mode +0x16.
        0x14aa => p.at_mode(o.w(0x16), o.w(0x14), x, y),
        // FUN_1070_9f35: a flame (light) in mode +0x18, scaled by +0x16, +0x14, its phase +0x1a.
        // (The original skips it while the lever +0x1e is off.)
        0x1450 => {
            if std::env::var("ET_DRAW").is_ok() {
                eprintln!("light 14 {:#x} 16 {:#x} 18 {:#x} 1e {}", o.w(0x14), o.w(0x16), o.w(0x18), o.w(0x1e));
            }
            p.mode = o.w(0x18);
            let t = (o.w(0x1a) as u32).wrapping_add(p.frame);
            p.flame(o.w(0x16), o.w(0x14), t, x, y);
        }
        // FUN_1070_06f6: a sentry.
        crate::sentry::KIND => p.at_flash(o.w(0x22) > 0, 0x5212 + (per(p, 4) & 1), x, y),
        _ if std::env::var("ET_DRAW").is_ok() => {
            eprintln!("draw: kind {:#x} at ({x},{y}) not drawn", o.kind)
        }
        _ => {}
    }
}

/// The word at 1170:0e4a + kind * 6 (the box kinds' sprites).
fn crate_sprite(kind: i32) -> i32 {
    const T: [i32; 8] = [0x752c, 0x752d, 0x752e, 0x5201, 0x5202, 0x5203, 0x330e, 0x330f];
    T.get(kind as usize).copied().unwrap_or(0x752c)
}

/// FUN_1088_9d34's drawing: the drone (its sprite turns with its angle, plus a blinking light),
/// then the hero: the body (mirrored when facing left) and the arm at the muzzle offset.
/// Blinking after a hit tints him through PalMixSea (colour 0); the gun's recoil lights him
/// through PalMixLight.
pub fn hero_and_drone(p: &mut Painter, hero: &crate::hero::Hero, drone: &crate::drone::Drone) {
    if drone.active > 0 {
        let y = drone.bob_y();
        p.at(((drone.angle + 0xb4) / 0x168) % 0x10 + 0x900, drone.x, y);
        p.at(0x910 + (per(p, 4) & 1), drone.x, y);
    }
    if hero.frame == 0x7fff {
        return;
    }
    let b = hero.hurt_blink();
    if b > 0 && (b / 2) % 2 == 0 {
        p.mode = 0x24;
        p.mix = Mix::Sea;
        p.colour = 0;
    }
    let c = hero.gun.cooldown();
    if c > 0 {
        p.mode = 0x24;
        p.mix = Mix::Light;
        p.colour = (c + 0x40) as u8;
    }
    p.hflip = hero.facing == -1;
    p.at(hero.frame, hero.x, hero.y);
    p.hflip = false;
    if hero.gun.frame2 != 0x7fff {
        let (mx, my) = hero.gun.muzzle;
        p.at(hero.gun.frame2, hero.x + mx, hero.y + my);
    }
    p.reset();
}

/// FUN_1088_026d: the bonus pick-ups, plane-2 cells whose function 0 is 30000, each drawn by
/// the procedure its function 2 picks (table 1170:16de) with MIX pointing at PalMixLight.
pub fn pickups(p: &mut Painter, world: &crate::world::World) {
    let Some(pl) = world.planes[2].as_ref() else { return };
    let (cx, cy) = p.cam;
    let (tx0, ty0) = ((cx >> 4).max(0), (cy >> 4).max(0));
    let (tx1, ty1) = (((cx + 0x13f) >> 4).min(pl.width - 1), ((cy + 199) >> 4).min(pl.height - 1));
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            let cell = pl.get(tx, ty);
            if cell == 0 || cell & 0x8000 != 0 || world.cell_function(cell, 0) != 30000 {
                continue;
            }
            pickup(p, world.cell_function(cell, 2) as i32, tx * 16 - cx, ty * 16 - cy);
        }
    }
}

/// One pick-up of kind `k` at screen (x, y) (FUN_1088_022b's dispatch).
pub fn pickup(p: &mut Painter, k: i32, x: i32, y: i32) {
    p.mix = Mix::Light;
    let f3 = per(p, 3);
    match k {
        0 => p.sprite(0x2200 + (f3 & 7), x, y),
        1 => shine(p, 0x2208 + ((f3 & 7) < 3) as i32, x, y),
        2 => {
            p.sprite((per(p, 2) & 0xf) + 0x900, x + 7, y + 7);
            p.sprite((per(p, 4) & 1) + 0x910, x + 7, y + 7);
        }
        3 => shine(p, 0x2214, x, y),
        4 => shine(p, 0x220a + f3 % 6, x, y),
        5..=12 => shine(p, 0x2216 + (k - 5), x, y),
        13..=15 => shine(p, 0x2228 + (k - 13), x, y),
        16 => p.sprite(0x2233 - (f3 & 7), x, y),
        _ => {}
    }
    p.reset();
}

/// FUN_1088_007d: every 256 frames a pick-up shines for 16 (mode 0x24 through PalMixLight).
fn shine(p: &mut Painter, id: i32, x: i32, y: i32) {
    let t = (p.frame & 0xff) as i32;
    if t < 0x10 {
        p.mode = 0x24;
        p.colour = (0x80 - (t << 2)) as u8;
    }
    p.sprite(id, x, y);
    p.mode = 0;
}
