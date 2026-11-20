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
pub fn actors(p: &mut Painter, level: &Level, world: &crate::world::World) {
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
                actor(p, o, world);
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
            1 => Mix::Tint,
            2 => Mix::Light,
            _ => Mix::Level,
        };
        p.at(l.sprite, l.x, l.y);
        p.reset();
    }
}

fn actor(p: &mut Painter, o: &Obj, world: &crate::world::World) {
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
        // FUN_1070_7eb2: the big alien; its shield tints it yellow (tint table row 10).
        0x1432 => {
            p.hflip = o.w(0x20) < 0;
            if o.b(0x44) != 0 {
                p.mix = Mix::Tint;
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
        // It is dark while the lever at cell +0x1e is at 0 (FUN_1088_140b: function 6000).
        0x1450 if o.w(0x1e) != 0 && {
            let c = o.uw(0x1e) as i32;
            let wd = world.main().width;
            world.tile_function_at((c / wd) << 4, (c % wd) << 4) == 6000
        } => {}
        0x1450 => {
            if std::env::var("ET_DRAW").is_ok() {
                eprintln!("light 14 {:#x} 16 {:#x} 18 {:#x} 1e {}", o.w(0x14), o.w(0x16), o.w(0x18), o.w(0x1e));
            }
            p.mode = o.w(0x18);
            let t = (o.w(0x1a) as u32).wrapping_add(p.frame);
            p.flame(o.w(0x16), o.w(0x14), t, x, y);
        }
        // FUN_1070_9420: a tree: the trunk as a line (colour +0x20) to its swaying tip, then the
        // crown of its stage (1170:0e9a), with a glow for stages flagged 2; a burning stump once
        // cut (+0x14 < 1). The tip comes from the update.
        crate::tree::KIND => {
            let (Some(tip), Some(crown)) = (o.looks.first(), o.looks.get(1)) else { return };
            let (cx, cy) = p.cam;
            p.line((x - cx, y - cy), (tip.x - cx, tip.y - cy), o.b(0x20));
            if o.w(0x14) < 1 {
                p.mode = 0x10;
                p.flame(0x600, 0x700, p.frame, tip.x, tip.y);
                p.mode = 0;
            } else {
                let k = o.w(10) as usize;
                let flags = crate::world::ds_word(0xe98 + 4 * k) & 0xff;
                if flags & 2 == 2 {
                    p.at_mode(0x28, 0x1b04, tip.x, crown.y + 7);
                }
                p.at_flash(o.w(0x16) > 0, crate::world::ds_word(0xe9a + 4 * k), tip.x, crown.y);
            }
        }
        // FUN_1068_2ee8: a sea plant: its stalk's segments (picked with Random(2) under its own
        // seed +0x1a, as the burst does), the bulb at the tip, the root.
        0x14fa => {
            let segs = o.w(0x14);
            if o.w(0x16) > 0 && segs > 0 {
                let (phase, seed) = (o.w(0x18), o.w(0x1a));
                let now = p.frame as i64;
                let t = ((now * 0x20 + phase as i64) % 0x1680) as i32;
                let idx = (((segs << 9) as i16 as i32) + t) % 0x1680;
                let tip_x = x + (crate::trig::sin(idx) * 6) / 0x4000;
                let idx2 = ((now * 28 + 0x5a0 + phase as i64) % 0x1680) as i32;
                let rise = (segs << 3) + (crate::trig::sin(idx2) * 6) / 0x4000;
                let mut rng = crate::rng::PascalRandom::new(seed as i16 as i32 as u32);
                for k in 1..segs {
                    let off = (crate::trig::sin(((k << 9) + t) % 0x1680) * ((k * 6) / segs)) / 0x4000;
                    let s = rng.random(2) + 0x3301;
                    p.at(s, x + off, y - (k * rise) / segs);
                }
                p.at(0x3300, tip_x, y - rise - 8);
            }
            p.at(0x3303, x, y);
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
/// Blinking after a hit tints him red (tint table row 0); the gun's recoil lights him
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
        p.mix = Mix::Tint;
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

/// FUN_1060_14a7: the platform objects whose box meets the screen, the default sector's then
/// the hero's sector's, each through the draw procedure of its row in the table at 1170:09ce
/// (14-byte rows: sprite, width, _, link sprite, height | key << 8, _, procedure). The
/// procedure column is filled at run time; these are its values (seen in CITY01).
pub fn platform_objects(p: &mut Painter, level: &Level) {
    use crate::world::ds_word;
    let sector = level.sector.unwrap_or(0);
    let (cx, cy) = p.cam;
    let mut order: Vec<&crate::platobj::PlatObj> = level.plat_objs.iter().filter(|o| o.sector == 0).collect();
    if sector != 0 {
        order.extend(level.plat_objs.iter().filter(|o| o.sector == sector));
    }
    for o in order {
        let (x1, y1, x2, y2) = o.bbox;
        if !(x1 <= cx + 0x13f && y1 <= cy + 199 && cx <= x2 && cy <= y2) {
            continue;
        }
        let row = o.row;
        let at = |k: usize| ds_word(0x9ce + row * 14 + k);
        let (sprite, link, h) = (at(0), at(6), at(8) & 0xff);
        const PROCS: [u16; 11] = [0x1cee, 0x1a25, 0x1a25, 0x1cee, 0x1a25, 0x1e13, 0x1e88, 0x1e13, 0x2242, 0x2079, 0x1e13];
        match PROCS.get(row).copied().unwrap_or(0x1a25) {
            0x1cee => {
                // FUN_1060_1cee: a light under it (unless it is sprite 0x3600), then itself.
                if sprite != 0x3600 {
                    p.at_mode(0x28, 0x1b01, o.x, o.y);
                }
                p.at(sprite, o.x, o.y);
            }
            0x1e13 => {
                // FUN_1060_1e13: blinks while it moves.
                let mut s = sprite;
                if o.angle != o.prev_angle {
                    s += (o.angle >> (h & 0x1f)) & 1;
                }
                p.at(s, o.x, o.y);
            }
            0x2242 => {
                // FUN_1060_2242: a lift hanging from two cables, a chain between them.
                let (top, bottom) = (o.bbox.1 + 0x10 - cy, o.bbox.3 - 0x10 - cy);
                let phase = 0xf - ((o.stops.1 - o.y) & 0xf);
                rope(p, 8, 0, phase, top, bottom, o.x + 7 - cx);
                rope(p, 8, 0, phase, top, bottom, o.x + 0x17 - cx);
                rope(p, 0xc, 1, ((o.stops.1 - o.y) >> 2) & 0xf, top, bottom, o.x + 0xf - cx);
                p.at(sprite, o.x, o.y);
            }
            _ => {
                // FUN_1060_1a25 (and the turning ones, 1e88 / 2079, drawn plainly): the chain
                // of a swinging platform from its pivot, then the platform.
                if o.kind == 1 && link != 0x7fff {
                    if let Some(&(px, py)) = o.points.first() {
                        let a = o.angle.rem_euclid(0x1680);
                        let (c, sn) = (crate::trig::cos(a), crate::trig::sin(a));
                        for k in 0..o.radius {
                            p.at(link, px + (c * h * k) / 0x4000, py - (sn * h * k) / 0x4000);
                        }
                    }
                }
                p.at(sprite, o.x, o.y);
            }
        }
        p.reset();
    }
}

/// FUN_1050_5cb0(w, kind, phase, top, bottom, x): a vertical rope (kind's sprites in the table
/// at 1170:0942: segment, end, spacing) from `bottom` (+ `phase`) up to `top`, the last `w`
/// pixels in the end sprite. Screen coordinates.
fn rope(p: &mut Painter, w: i32, kind: usize, phase: i32, top: i32, bottom: i32, x: i32) {
    use crate::world::ds_word;
    if top > bottom {
        return;
    }
    let seg = ds_word(0x942 + kind * 6);
    let end = ds_word(0x944 + kind * 6);
    let step = ds_word(0x946 + kind * 6) & 0xff;
    if step <= 0 {
        return;
    }
    let save = p.clip;
    let mut y = bottom + phase;
    p.clip.3 = bottom.min(save.3);
    p.clip.1 = (top + w).max(save.1);
    while y >= p.clip.1 {
        p.sprite(seg, x, y);
        y -= step;
    }
    if w > 0 && end != 0x7fff && end >= 0 {
        p.clip.3 = (top + w - 1).min(save.3);
        p.clip.1 = top.max(save.1);
        y += step;
        while y >= p.clip.1 {
            p.sprite(end, x, y);
            y -= step;
        }
    }
    p.clip = save;
}

/// FUN_10a8_0dab's drawing: each moving door through its type's procedure (1170:19da + 0xd *
/// type: sprite, full, procedure, sounds, top and bottom piece heights; these are the values
/// the table holds at run time). The door's own cells are cleared while it moves.
pub fn doors(p: &mut Painter, level: &Level) {
    // (sprite, full, procedure, top piece, bottom piece)
    const T: [(i32, i32, u16, i32, i32); 8] = [
        (0x3400, 32, 0x0002, 8, 1),
        (0x340b, 16, 0x01f5, 0, 0),
        (0x340d, 16, 0x01f5, 0, 0),
        (0x340f, 32, 0x02ce, 0, 0),
        (0x3400, 32, 0x0537, 0, 0),
        (0x3403, 32, 0x07c1, 0, 0),
        (0x3412, 32, 0x0454, 0, 0),
        (0x3405, 32, 0x0665, 0, 0),
    ];
    for d in &level.doors {
        let Some(&(base, full, proc_, top, bot)) = T.get(d.kind) else { continue };
        let (x, y) = (d.x * 16 - p.cam.0, d.y * 16 - p.cam.1);
        let prog = d.progress;
        let bottom = y + d.h * 16;
        // The swing of the hinged ones: amp * sin(0x5a0 - progress * 0x5a0 / 32).
        let swing = |amp: i32| {
            let a = 0x5a0 - ((((prog * 0x5a0) as u16) >> 5) as i32);
            (amp * crate::trig::sin(a.rem_euclid(0x1680))) / 0x4000
        };
        let save = p.clip;
        match proc_ {
            0x0002 => {
                // FUN_10a8_0002: slides up into its frame (top piece), a sill below.
                if top != 0 {
                    p.clip.1 = y.max(save.1);
                    p.clip.3 = (y + top - 1).min(save.3);
                    p.sprite(base + 2, x, y + prog);
                    p.clip = save;
                }
                p.clip.1 = (y + top).max(save.1);
                p.sprite(base, x, y + prog);
                p.clip = save;
                if bot != 0 {
                    p.clip.3 = (bottom - 1).min(save.3);
                    p.sprite(base + 1, x, bottom - prog);
                }
            }
            0x01f5 | 0x02ce => {
                // FUN_10a8_01f5 / 02ce: two leaves part sideways (02ce: and a shutter drops).
                p.clip.0 = x.max(save.0);
                p.clip.2 = (x + d.w * 16 - 1).min(save.2);
                let u = if proc_ == 0x01f5 { full - prog } else { ((full - prog) as u16 >> 1) as i32 };
                p.sprite(base, x - u, y);
                p.sprite(base + 1, x + u, y);
                p.clip = save;
                if proc_ == 0x02ce {
                    p.clip.1 = y.max(save.1);
                    p.sprite(base + 2, x, y - 0x40 + swing(0x40));
                }
            }
            0x0454 => {
                // FUN_10a8_0454: a folding grille, a light over it.
                p.clip.1 = y.max(save.1);
                let i4 = y - 0x22 + prog;
                for k in 0..=10 {
                    p.sprite(base + 1, x, (k * (prog + 0x21)) / 0xb + i4);
                }
                p.sprite(base, x, i4 + prog + 0x21);
                p.clip = save;
                p.mode = 0x28;
                p.sprite(0x1b09, x, y);
            }
            0x0537 | 0x07c1 => {
                // FUN_10a8_0537 / 07c1: jaws opening up and down, a light over them.
                let k = swing(0x20);
                p.clip.1 = (y + if proc_ == 0x0537 { 3 } else { 0xb }).max(save.1);
                p.sprite(base, x, y - k);
                p.clip = save;
                p.clip.3 = (bottom - 1).min(save.3);
                p.sprite(base + 1, x, y + k);
                p.clip = save;
                if proc_ == 0x0537 {
                    p.sprite(base + 2, x, y);
                }
                p.mode = 0x28;
                p.sprite(if proc_ == 0x0537 { 0x1b02 } else { 0x1b03 }, x, y);
            }
            _ => {
                // FUN_10a8_0665: a lid up, two leaves sideways, a light.
                p.clip.1 = y.max(save.1);
                p.sprite(base, x, y - swing(0x20));
                p.clip = save;
                p.clip.0 = (x + 1).max(save.0);
                p.sprite(base + 1, x - (0x20 - prog), y);
                p.clip = save;
                p.clip.2 = (x + 0x1e).min(save.2);
                p.sprite(base + 2, x + (0x20 - prog), y);
                p.clip = save;
                p.mode = 0x28;
                p.sprite(0x1b09, x, y);
            }
        }
        p.reset();
    }
}

/// FUN_1088_4b64's pointer and FUN_1088_2501's status bar (HUD level 1: health, keys left out,
/// the right button's weapon and its ammo). Screen coordinates; ETSCORE is slot 0x1f.
pub fn hud(p: &mut Painter, hero: &crate::hero::Hero) {
    use crate::world::ds_word;
    p.reset();
    let byte = |a: usize| ds_word(a) & 0xff;
    let w = hero.weapon;
    let rec = 0x1656 + (w.max(0) as usize) * 0xd;
    // The pointer: two frames blinking every 12, plus the weapon's own (record +5).
    let mut ptr = 0x7800 + (per(p, 12) & 1);
    if w >= 0 {
        ptr += byte(rec + 5);
    }
    let (mx, my) = hero.gun_mouse();
    p.mode = 0x30;
    p.sprite(ptr, mx, my);
    // Health: the frame, then the bar cut at its length (health * 0x47 / 0x2c6); red when low.
    let len = hero.health.clamp(0, 0x2c6) * 0x47 / 0x2c6;
    p.sprite(0x1f0d, 4, 0xc6);
    p.mode = 4;
    if len < 9 {
        p.mix = Mix::Light;
        p.mode = 0x2c;
        p.colour = ((per(p, 4) & 1) * 5 + 0x1f) as u8;
    }
    p.clip.2 = len + 0x1b;
    p.sprite(0x1f0b, 4, 0xc6);
    p.reset();
    let mut x = 4;
    p.mode = 4;
    // The keys (1170:5118..511a; the port keeps them at `keys[(0x14 + k) & 0xf]`).
    for k in 0..3 {
        if hero.keys[(0x14 + k) & 0xf] {
            if x == 4 {
                p.sprite(0x1f28, 4, 0xc6);
            }
            p.sprite(0x1f2a + k as i32, x, 0xc6);
            x += 9;
        }
    }
    if x != 4 {
        p.sprite(0x1f29, x, 0xc6);
        x += 6;
    }
    p.sprite(0x1f0a, x, 0xc6);
    if w >= 0 {
        let wp = &hero.weapons[w as usize];
        let x2 = x + 99;
        if byte(rec + 4) == 1 {
            // Six pips.
            let n = (wp.ammo * 6 + wp.max - 5) / (wp.max + 1);
            for k in 0..n.min(6) {
                p.sprite(0x1f0e, k * 2 + x2, 0xbe);
            }
            for k in n.max(0)..6 {
                p.sprite(0x1f0f, k * 2 + x2, 0xbe);
            }
        } else {
            p.sprite(wp.ammo / 10 + 0x1f00, x2, 0xbe);
            p.sprite(wp.ammo % 10 + 0x1f00, x + 0x68, 0xbe);
        }
    }
    p.mode = 0x30;
    p.sprite(0x1f0c, x, 0xc6);
    // The weapon's icon; with none chosen yet (-1) this is 0x1f13, the pistol's.
    p.mode = 4;
    p.sprite(0x1f14 + w, x, 0xc6);
    p.reset();
}

/// FUN_1090_009c (the foreground procs of the levels with water): below the hero's sector's
/// water line (`Level::water`, waves included) every screen row is tinted through PalMixSea,
/// the row picked by a 128-pixel SEATILE texture (slot 0x7c) plus a strength that fades over
/// the 32 rows under the surface (0x30 on the surface row itself) (FUN_1090_0002). The rows
/// sway with a sine of their height and drift with the sector's current (+0x1a).
pub fn water(p: &mut Painter, level: &Level, world: &crate::world::World) {
    let Some(sea) = p.tables.sea.as_deref() else { return };
    let sector = level.sector.unwrap_or(0);
    let Some(sec) = world.sectors.get(sector) else { return };
    if sec.water[1] == 0x7fff || level.water == 0x7fff {
        return;
    }
    let (cx, cy) = p.cam;
    let frame = p.frame as i32;
    let surface = level.water - cy;
    let start = surface.max(0);
    if start > 199 {
        return;
    }
    let drift = ((cx - frame.wrapping_mul(sec.water[3])) & 0x7f) + 0x80;
    let rise = if sec.water[0] == 1 { 0 } else { (cy - frame / 2) & 0x7f };
    for y in start..=199 {
        let a = ((y + cy) * 6 + frame * 0x10).rem_euclid(0x1680);
        let x0 = -((crate::trig::sin(a) / 0xaaa + drift) & 0x7f);
        let mut row = (rise + y) & 0x7f;
        let strength = if y == surface {
            row = (row + frame) & 0x7f;
            0x30
        } else {
            ((surface + 0x20 - y) / 2).max(0)
        };
        let Some(tex) = p.bank.row(0x7c00, row) else { return };
        let mut x = x0;
        while x < crate::W {
            for (i, &t) in tex.iter().enumerate() {
                let sx = x + i as i32;
                if (0..crate::W).contains(&sx) {
                    let at = (y * crate::W + sx) as usize;
                    let k = (t as i32 + strength) & 0xff;
                    p.fb[at] = sea[(k as usize) * 256 + p.fb[at] as usize];
                }
            }
            x += 0x80;
        }
    }
}
