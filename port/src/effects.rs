//! Tile explosions and the effects table (FUN_1040_60bb). Only their Random() use and their
//! changes to the map matter to the simulation; particles aren't drawn yet.

use crate::level::{Effects, Level};
use crate::rng::PascalRandom;
use crate::sound;
use crate::world::{MAIN_PLANE, OUTSIDE, World};

/// The effect procedures whose sound needs no Random(): (effect, sound, pitch, volume 1ab8,
/// stop that sample's other instances first (FUN_10e8_05e0)). Pick-ups (1, 6, 67, 70, 74) too.
const PLAIN_SOUNDS: [(u16, usize, i32, i32, bool); 13] = [
    (1, 0x86, 0x100, 0x100, true),     // FUN_1040_014a: a heart
    (3, 0x87, 0x100, 0x100, false),    // FUN_1040_0264
    (4, 0x38, 0x100, 0x100, false),    // FUN_1040_0571
    (6, 0x88, 0x100, 0x100, false),    // FUN_1040_050e: the drone
    (26, 0x4e, 0x100, 0x100, false),   // FUN_1040_2f7b
    (41, 0x4a, 0x100, 0x100, false),   // FUN_1040_44ef
    (50, 0x26, 0x100, 0x80, false),    // FUN_1040_50cf (and 0x1c below)
    (52, 0x5b, 0x100, 0x100, false),   // FUN_1040_54b2
    (67, 0x88, 0x100, 0x100, true),    // FUN_1040_0312: a weapon
    (70, 0x88, 0x100, 0x100, false),   // FUN_1040_043c: a key
    (74, 0x86, 0x100, 0x100, true),    // FUN_1040_01d9
    (0, 0, 0, 0, false),
    (0, 0, 0, 0, false),
];

/// The sound of an effect procedure that draws no Random() for it (`PLAIN_SOUNDS`).
pub fn plain_sound(id: u16, y: i32, x: i32) {
    if let Some(&(_, s, p, v, discard)) = PLAIN_SOUNDS.iter().find(|e| e.0 == id && e.0 != 0) {
        if discard {
            sound::discard(s);
        }
        sound::set_volume(v);
        if p != 0x100 {
            sound::pitch(p);
        }
        sound::play_at(s, y, x);
        if id == 50 {
            sound::play_at(0x1c, y, x);
        }
        sound::set_volume(0x100);
    }
}

/// FUN_1040_60bb: effect `id` at (y, x); ids >= 30000 are plain sounds. The effect procedures
/// are the table at 1170:07b2 (filled at run time; `mise run trace -- ... --dump-mem 1170:07b2:200`).
pub fn effect(lv: &mut Level, world: &mut World, id: u16, y: i32, x: i32, rng: &mut PascalRandom, fx: &mut Effects) {
    plain_sound(id, y, x);
    match id {
        0 => {}
        7 => {
            // FUN_1040_0897: an explosion rising.
            lv.particles.spawn_seq(0x6400, 2, 0x24, y, x, 1);
            lv.particles.line(2, 0x5a0);
        }
        9 => {
            // FUN_1040_08e2: a small explosion.
            lv.particles.spawn_seq(0x6500, 2, 0x14, y, x, 1);
        }
        50 => {
            // FUN_1040_50cf: a plank bridge (tile function 60300) collapses, left of x and from x
            // to the right, plank by plank (FUN_1040_4fd9). Sounds 0x26, 0x1c at half volume.
            fx.shakes.push((0x18, 0xc));
            let ps = &mut lv.particles;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            ps.p.callback = 0x17f4;
            let mut px = x;
            loop {
                px -= 0x10;
                if world.tile_function_at(y, px) != 60300 {
                    break;
                }
                plank_breaks(lv, world, y, px, rng, fx);
            }
            let mut px = x;
            loop {
                plank_breaks(lv, world, y, px, rng, fx);
                px += 0x10;
                if world.tile_function_at(y, px) != 60300 {
                    break;
                }
            }
            let ps = &mut lv.particles;
            ps.p.delay = 0;
            ps.p.draw = 0;
            ps.p.callback = 0;
            ps.p.hits = 0;
            ps.p.on_death = 0;
        }
        23 => {
            // FUN_1040_2af4: a flash.
            lv.particles.spawn_seq(0x6900, 2, 0x19, y, x, 1);
        }
        34 => explosion(lv, world, y, x, rng, fx),
        69 => {
            // FUN_1040_5c06: a big wall collapses: the tiles above step on, dust and rubble fly.
            let (x1, y1, x2, y2) = clear_block(world, y, x);
            let width = world.main().width;
            for i in 0..4 {
                let cell = (((y1 - 0x10) >> 4) * width + (x1 >> 4) + i) as usize;
                if let Some(&v) = world.main().cells.get(cell) {
                    let off = world.cell_function(v, 2);
                    lv.set_cell(world, v.wrapping_add(off), cell as u16, fx);
                }
            }
            let cx = ((x1 + x2 + 0xf) as u16 >> 1) as i32;
            let cy = ((y1 + y2 + 0xf) as u16 >> 1) as i32;
            let ps = &mut lv.particles;
            ps.p.colour = 0x30;
            for r in 0..2 {
                for c in -1..=1 {
                    ps.p.delay = rng.random(4);
                    let px = c * 0x18 + cx - 6 + rng.random(0xc);
                    let py = cy + r * -0x30 + 0x12 + rng.random(0xc);
                    ps.spawn_seq(0x6d00, 2, 0x13, py, px, 1);
                    let a = (rng.random(0x61) + 0x570) - c * 0xc0;
                    let speed = (r * 2 + 2) * 0x10 + rng.random(0x11);
                    ps.throw(0xe, speed, a);
                }
            }
            ps.p.callback = 0x186e;
            ps.p.colour = 4;
            for r in -2..=2 {
                for c in -1..=1 {
                    ps.p.delay = rng.random(0xc);
                    ps.p.draw = rng.random(2);
                    let px = c * 0x18 + cx - 0xc + rng.random(0x18);
                    let py = r * 0x11 + cy - 8 + rng.random(0x10);
                    let steps = rng.random(6) + 2;
                    ps.spawn(0x3302, py, px);
                    ps.last_mut().m.set(0x26, steps);
                    let a = (rng.random(0xc1) + 0x540) - c * 0xc0;
                    let speed = rng.random(0x31) + 0x30;
                    let g = rng.random(4) + 0xe;
                    ps.throw(g, speed, a);
                    let t = rng.random(0x201) + 0x100;
                    ps.last_mut().m.set(0x40, t);
                }
            }
            ps.p.delay = 0;
            ps.p.draw = 0;
            ps.p.callback = 0;
            ps.p.colour = 0;
            fx.shakes.push((0x20, 0x10));
            sound::play_at(0x47, cy, cx);
            sound::play_at(0x4a, cy, cx);
        }
        18 => {
            // FUN_1040_11b0: a tile falls to bits.
            fx.shakes.push((0x18, 8));
            let ps = &mut lv.particles;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            ps.spawn(0x331b, y, x);
            let r = rng.random(0xc0);
            ps.throw(7, 0x40, 0x560 - r);
            ps.spawn(0x331b, y, x);
            let r = rng.random(0xc0);
            ps.throw(7, 0x58, r + 0x5e0);
            ps.p.on_death = 0;
            ps.p.hits = 0;
        }
        43 => wall_chunks(lv, world, y, x, rng, fx),
        72 => rubble(lv, 0x3301, y, x, rng),
        75 => {
            rubble(lv, 0x520a, y, x, rng);
            let p = rng.random(0x28); // the crash's pitch
            sound::set_volume(0xaa);
            sound::play_at_pitch(0x4a, 0x100 - p, y, x);
            sound::set_volume(0x100);
            fx.shakes.push((0x18, 10));
        }
        92 => {
            // FUN_1040_3325: a blast of burning bits, and a steam emitter where it happened
            // (the START intro's ship).
            let ps = &mut lv.particles;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            ps.p.callback = 0x177a;
            ps.p.colour = 0x10;
            for i in 0..=0x14 {
                ps.p.delay = rng.random(5);
                let px = x - 4 + (i >> 1);
                let py = y - rng.random(6);
                let steps = rng.random(2) + 3;
                ps.spawn(0x7500, py, px);
                ps.last_mut().m.set(0x26, steps);
                let v = rng.random(0x21) + 0x20;
                ps.throw(7, v, i * 0x30);
            }
            ps.p.colour = 0;
            ps.p.callback = 0;
            ps.p.on_death = 0;
            ps.p.hits = 0;
            ps.p.delay = 0;
            for s in [0x7b, 1, 0x1a] {
                sound::play_at(s, y, x);
            }
            fx.shakes.push((0x20, 0x10));
            let rec = [0i16, 2, 0x172, 0x223];
            let mut cx = crate::factory::Ctx { hero_x: lv.hero_x, rng };
            lv.factory(world, &mut cx, 0x141e, &crate::factory::Rec { w: &rec, count: 4 });
        }
        88 => {
            // FUN_1040_2a14: every 0x58th frame a spark column and, a while later, a bit that falls
            // and goes off (effect 87).
            if lv.now % 0x58 == 0 {
                let ps = &mut lv.particles;
                ps.p.group = 0;
                ps.spawn_seq(0x5210, 0x1a, 2, y, x, 1);
                ps.p.delay = 0x34;
                ps.p.hits = 3;
                ps.p.damage = 0;
                ps.p.on_death = 0x57;
                ps.p.callback = 0x1c92;
                ps.spawn(0x520c, y + 1, x);
                ps.throw(0x1c, 0, 0x10e0);
                ps.p.callback = 0;
                ps.p.on_death = 0;
                ps.p.damage = 0x10;
                ps.p.hits = 0;
                ps.p.delay = 0;
                ps.p.group = 1;
            }
        }
        87 => {
            // FUN_1040_2ab0: a spark and its sound.
            let ps = &mut lv.particles;
            ps.p.group = 0;
            ps.spawn_seq(0x520d, 4, 3, y, x, 1);
            ps.p.group = 1;
            let p = rng.random(0x80); // the sound's pitch
            sound::play_at_pitch(99, p + 0xe0, y, x);
        }
        37 => {
            // FUN_1040_3fff: a puff of smoke every other frame (1170:617e is even).
            if lv.now % 2 == 0 {
                let ps = &mut lv.particles;
                ps.p.colour = 0x10;
                ps.spawn_seq(0x5111, 3, 9, y, x, 1);
                let a = rng.random(0x81) + 0x560;
                let speed = rng.random(0x10) + 0x10;
                ps.line(speed, a);
                ps.p.colour = 0;
            }
        }
        82 => {
            // FUN_1040_328f: chips here and, over a neighbouring cracked tile (60302 / 60301),
            // there too; that tile steps on.
            chips(lv, rng, y, x);
            let width = world.main().width;
            for (dx, want) in [(0x10, 60302u16), (-0x10, 60301)] {
                if world.tile_function_at(y, x + dx) == want {
                    chips(lv, rng, y, x + dx);
                    let c = (y >> 4) * width + ((x + dx) >> 4);
                    let v = world.main().cells[c as usize];
                    let off = world.cell_function(v, 2);
                    lv.set_cell(world, v.wrapping_add(off), c as u16, fx);
                }
            }
            let p = rng.random(0x1f); // the sound's pitch
            sound::play_at_pitch(0x7b, p + 0xf1, y, x);
            fx.shakes.push((0xc, 7));
        }
        30 => {
            // FUN_1040_3683: a gate blows: the two tiles beside the cell step on and bits fly
            // out of both sides.
            let width = world.main().width;
            let mut c = (y >> 4) * width + (x >> 4);
            let f = world.cell_function(world.main().cells[(c - 1) as usize], 0);
            let step = |lv: &mut Level, world: &mut World, c: i32, fx: &mut Effects| {
                let v = world.main().cells[c as usize];
                let off = world.cell_function(v, 2);
                lv.set_cell(world, v.wrapping_add(off), c as u16, fx);
            };
            if f == 60300 || f == 60601 {
                step(lv, world, c - 1, fx);
                c -= 2;
                step(lv, world, c, fx);
            } else {
                step(lv, world, c + 1, fx);
                step(lv, world, c + 2, fx);
            }
            let px = (c % width) * 0x10;
            let ps = &mut lv.particles;
            ps.p.group = 0;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            ps.p.callback = 0x177a;
            ps.p.colour = 0x10;
            for (dir, bx) in [(-1, px + 7), (1, px + 0x27)] {
                for i in 0..=5 {
                    ps.p.delay = rng.random(4);
                    let py_x = bx - 2 + rng.random(6);
                    let py = y + rng.random(4);
                    ps.spawn(0x7500, py, py_x);
                    ps.last_mut().m.set(0x26, 3);
                    let a = 0x10e0 + dir * (i * 0x80 - 0xc0 + rng.random(0x71));
                    ps.throw(7, 0x20, a);
                }
            }
            ps.p.colour = 0;
            ps.p.callback = 0;
            ps.p.on_death = 0;
            ps.p.hits = 0;
            ps.p.group = 1;
            ps.p.delay = 0;
            let p = rng.random(0x1f); // the sound's pitch
            sound::play_at_pitch(0x51, p + 0xf1, y, x + 0x10);
            fx.shakes.push((0xc, 7));
        }
        83 => {
            // FUN_1040_34a7: chips (FUN_1040_31d4), a sound, a shake and maybe a late spark.
            let ps = &mut lv.particles;
            ps.p.group = 0;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            ps.p.callback = 0x177a;
            ps.p.colour = 0x10;
            for i in 0..=4 {
                ps.p.delay = rng.random(4);
                let px = x - 6 + i * 3;
                let py = y + rng.random(4);
                let steps = rng.random(2) + 3;
                ps.spawn(0x7500, py, px);
                ps.last_mut().m.set(0x26, steps);
                ps.throw(7, 0x20, i * 0x80 + 0xfe0);
            }
            ps.p.colour = 0;
            ps.p.callback = 0;
            ps.p.on_death = 0;
            ps.p.hits = 0;
            ps.p.group = 1;
            ps.p.delay = 0;
            let p = rng.random(0x29); // the sound's pitch
            sound::play_at_pitch(0x7b, p + 0xec, y, x);
            fx.shakes.push((0xc, 7));
            if rng.random(2) != 0 {
                ps.p.delay = rng.random(0x50) + 0x28;
                ps.p.group = 0;
                ps.p.on_spawn = 0x7576;
                ps.spawn_seq(0x1928, 3, 4, y + 8, x, 1);
                ps.p.on_spawn = 0;
                ps.p.group = 1;
                ps.p.delay = 0;
            }
        }
        84 => {
            // FUN_1040_34e3: chips thrown up (FUN_1040_31d4), maybe a late spark, a sound.
            let ps = &mut lv.particles;
            ps.p.group = 0;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            ps.p.callback = 0x177a;
            ps.p.colour = 0x10;
            for i in 0..=4 {
                ps.p.delay = rng.random(4);
                let px = x - 6 + i * 3;
                let py = y - 2 + rng.random(4);
                let steps = rng.random(2) + 3;
                ps.spawn(0x7500, py, px);
                ps.last_mut().m.set(0x26, steps);
                ps.throw(7, 0x20, i * 0x80 + 0xfe0);
            }
            ps.p.colour = 0;
            ps.p.callback = 0;
            ps.p.on_death = 0;
            ps.p.hits = 0;
            ps.p.group = 1;
            ps.p.delay = 0;
            if rng.random(2) != 0 {
                ps.p.delay = rng.random(0x50) + 0x28;
                ps.p.group = 0;
                ps.p.on_spawn = 0x7576;
                ps.spawn_seq(0x1928, 3, 4, y - 1, x, 1);
                ps.p.on_spawn = 0;
                ps.p.group = 1;
                ps.p.delay = 0;
            }
            let p = rng.random(0x29); // the sound's pitch
            let v = rng.random(0x40);
            sound::set_volume(0x100 - v);
            sound::play_at_pitch(0x7b, p + 0xec, y, x);
            sound::set_volume(0x100);
            fx.shakes.push((0xc, 7));
        }
        85 => {
            // FUN_1040_008d: a handful of sparks, each spawned after a delay.
            let ps = &mut lv.particles;
            ps.p.hits = 0x8000;
            ps.p.callback = 0x2946;
            let n = rng.random(2) + 5;
            for _ in 0..=n {
                ps.p.delay = rng.random(0x18);
                let px = x - 8 + rng.random(0x10);
                let py = y - rng.random(0xc);
                let s = rng.random(3) * 2 + 0x331e;
                ps.spawn(s, py, px);
                let a = rng.random(0x500) + 800;
                let speed = rng.random(10) + 8;
                ps.line(speed, a);
            }
            ps.p.delay = 0;
            ps.p.callback = 0;
            ps.p.hits = 0;
        }
        // Seen in GRINDER with no Random() calls in the tracer.
        76 => sound::play_at_pitch(0x91, 0xce, y, x),
        53 => {
            // FUN_1040_5503: the tile `function - 60701` tiles to the right blows up if it is a
            // 60705 (FUN_1050_5a33(0x60, 4)).
            let f = world.tile_function_at(y, x) as i16 as i32;
            let x = x + (f + 0x12e3) * 0x10;
            if world.tile_function_at(y, x) as i16 == -0x12df {
                explode_tile(lv, world, 0x60, 4, y, x, rng, fx);
            }
        }
        54 => {
            // FUN_1040_553e: with no floor tile above, the main-plane cell steps on by 2
            // (FUN_10f0_5b14).
            if world.solidity_at(y - 0x10, x) & 2 == 0 && world.cell_at(MAIN_PLANE, y, x).is_some() {
                let w = world.main().width;
                let idx = ((y >> 4) * w + (x >> 4)) as usize;
                let c = &mut world.main_mut().cells[idx];
                *c = c.wrapping_add(2);
            }
        }
        31 | 32 => {
            // FUN_1040_37aa / 38a9: chips thrown left / right, a crack, a shake.
            let ps = &mut lv.particles;
            ps.p.group = 0;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            ps.p.callback = 0x177a;
            ps.p.colour = 0x10;
            for i in 0..=4 {
                ps.p.delay = rng.random(4);
                let px = if id == 31 { x + 4 - i * 3 } else { x - 4 + i * 3 };
                let py = y + 2 + rng.random(4);
                ps.spawn(0x7500, py, px); // FUN_1050_4106: its own frame step
                ps.last_mut().m.set(0x26, 3);
                let r = rng.random(0x71);
                let a = if id == 31 { 0x10e0 - (i * 0x80 - 0xc0 + r) } else { i * 0x80 + r + 0x1020 };
                ps.throw(7, 0x20, a);
            }
            ps.p = crate::particles::Params::default();
            let p = rng.random(0x15);
            sound::pitch(p + 0xf6);
            sound::play_at(0x7b, y + 2, x + 0x10);
            fx.shakes.push((0xc, 5));
        }
        33 => {
            // FUN_1040_39a1: the block of the tile's function sinks out of sight: it leaves the
            // main plane and a particle draws it going down (callback 0x1e16, mover 0x3ab8 = age
            // only; record: top-left tile, x, y, columns, rows); then FUN_1088_12f9 for the cell
            // under its left end.
            let f = world.tile_function_at(y, x);
            let (mut lx, mut ty) = (x, y);
            while world.tile_function_at(y, lx - 0x10) == f {
                lx -= 0x10;
            }
            while world.tile_function_at(ty - 0x10, lx) == f {
                ty -= 0x10;
            }
            let w = world.main().width;
            let base = world.cell_at(MAIN_PLANE, ty, lx).unwrap_or(0) as i32;
            let (x1, y1, x2, y2) = clear_block(world, ty & 0xfff0, lx & 0xfff0);
            let ps = &mut lv.particles;
            ps.p.group = 0;
            ps.p.callback = 0x1e16;
            let steps = rng.random(0x18) + 5;
            ps.spawn_seq(0, steps, (y2 - y1) + 0x10, y2 + 0xf, x1, 0);
            {
                let p = ps.last_mut();
                p.m.set(0x4a, crate::particles::MOVE_AGE as i32);
                for (k, v) in [base, x1, y1, (x2 - x1) >> 4, (y2 - y1) >> 4].into_iter().enumerate() {
                    p.v.set(k * 2, v);
                }
            }
            ps.p.callback = 0;
            ps.p.group = 1;
            let below = ((y2 >> 4) + 1) * w + (x1 >> 4);
            if below >= 0 && (below as usize) < world.main().cells.len() {
                lv.cell_changed(world, below, fx);
            }
        }
        35 => {
            // FUN_1040_3c53: the cells left, right and above go, bits fly, a steam emitter
            // (0x141e kind 0) takes its place (FUN_1068_17f8), a big shake.
            let w = world.main().width;
            let c = (y >> 4) * w + (x >> 4);
            for d in [1, -1, -w] {
                if let Some(v) = world.main_mut().cells.get_mut((c + d) as usize) {
                    *v = 0;
                }
            }
            chip_pairs(&mut lv.particles, rng, 0x7516, y, x);
            // FUN_1068_17f8: made at once (a blast's object pass right after hits it).
            let mut cx = crate::factory::Ctx { hero_x: 0, rng: &mut *rng };
            lv.factory(world, &mut cx, 0x141e, &crate::factory::Rec { w: &[0, 0, x as i16, (y + 8) as i16], count: 4 });
            fx.shakes.push((0x2a, 0x10));
            sound::play_at(0x4a, y, x);
        }
        80 => {
            // FUN_1040_3d9e: bits fly, a steam emitter (0x141e kind 1) takes its place.
            chip_pairs(&mut lv.particles, rng, 0x7514, y, x);
            // FUN_1068_17f8: made at once (a blast's object pass right after hits it).
            let mut cx = crate::factory::Ctx { hero_x: 0, rng: &mut *rng };
            lv.factory(world, &mut cx, 0x141e, &crate::factory::Rec { w: &[0, 1, x as i16, (y + 7) as i16], count: 4 });
            fx.shakes.push((0x18, 10));
            sound::play_at(0x4a, y, x);
        }
        68 => {
            // FUN_1040_56ef: function-60300 cells above / below step on, a bonus 0xd10 one time
            // in three, six bits of wood, a shake.
            let w = world.main().width;
            let fy = (y & 0xfff0) + 0xf;
            let mut bx = x;
            if world.tile_function_at(fy, x - 0x10) as i16 == -0x1474 {
                step_cell(lv, world, ((fy >> 4) * w + ((x - 0x10) >> 4)) as u16, rng, fx);
                bx -= 8;
            }
            if world.tile_function_at(fy, x + 0x10) as i16 == -0x1474 {
                bx += 8;
                step_cell(lv, world, ((fy >> 4) * w + ((x + 0x10) >> 4)) as u16, rng, fx);
            }
            if rng.random(3) == 0 {
                lv.bonus(world, 0xd10, fy, bx);
            }
            let ps = &mut lv.particles;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            for i in 0..=5 {
                let px = bx - 10 + i * 4;
                let py = ((y & 0xfff0) + 0xb) - rng.random(8);
                let k = rng.random(2);
                ps.spawn(k + 0x3300, py, px);
                let a = i * -0x30 + 0x5d8 + rng.random(0x81);
                let v = rng.random(0x20);
                ps.throw(9, v + 0x40, a);
            }
            ps.p.on_death = 0;
            ps.p.hits = 0;
            fx.shakes.push((0x18, 0xe));
            sound::play_at(0x4a, fy, bx);
        }
        79 => {
            // FUN_1040_538f: a back-plane tile of function 60300 there steps on (plane 0), four
            // leaves (0x5925) flutter down.
            if world.tile_function_in(0, y, x) as i16 == -0x1474 {
                if let Some(Some(p)) = world.planes.get(0) {
                    let (tx, ty) = (x >> 4, y >> 4);
                    let t = p.get(tx, ty);
                    let next = (t as i32 + world.cell_function(t, 2) as i16 as i32) as u16;
                    if (next as i16) >= 0 {
                        world.planes[0].as_mut().unwrap().set(tx, ty, next);
                    }
                }
                sound::discard(0x51);
                let p = rng.random(0x28);
                sound::pitch(0x100 - p);
                sound::play_at(0x51, y, x);
                let ps = &mut lv.particles;
                ps.p.group = 0;
                ps.p.hits = 0x8000;
                ps.p.on_death = 0x10;
                ps.p.callback = 0x177a;
                ps.p.colour = 0x10;
                for _ in 0..=3 {
                    ps.p.delay = rng.random(0x10);
                    let px = x - 7 + rng.random(0x10);
                    let py = y - 7 + rng.random(0x10);
                    let steps = rng.random(3) + 2;
                    ps.spawn(0x5925, py, px);
                    ps.last_mut().m.set(0x26, steps);
                    let a = rng.random(0x321) + 0xf50;
                    let v = rng.random(0x11);
                    ps.throw(7, v + 0x10, a);
                }
                ps.p = crate::particles::Params::default();
            }
        }
        45 => {
            // FUN_1040_48b8: the function-60300 cells above and below step on by 1
            // (FUN_1088_13dc); with a strength-4 blast or one time in four the cell steps on by
            // its function 4; smoke, a spark later, a shake.
            let w = world.main().width;
            let cell_of = |yy: i32| ((yy >> 4) * w + (x >> 4)) as u16;
            let mut ly = y - 0x10;
            while world.tile_function_at(ly, x) as i16 == -0x1474 {
                let c = cell_of(ly);
                let v = world.main().cells[c as usize];
                lv.set_cell(world, v.wrapping_add(1), c, fx);
                ly -= 0x10;
            }
            let mut ly = y + 0x10;
            while world.tile_function_at(ly, x) as i16 == -0x1474 {
                let c = cell_of(ly);
                let v = world.main().cells[c as usize];
                lv.set_cell(world, v.wrapping_add(1), c, fx);
                ly += 0x10;
            }
            if lv.blast_strength == 4 || rng.random(4) == 0 {
                let c = cell_of(y) as usize;
                let v = world.main().cells[c];
                let n = world.cell_function(v, 4);
                if n != 0 {
                    world.main_mut().cells[c] = v.wrapping_add(n);
                }
            }
            let ps = &mut lv.particles;
            ps.p.colour = 0x10;
            ps.p.group = 0;
            crate::particles::smoke(ps, rng, y, x);
            ps.p.colour = 0;
            ps.p.delay = 0xa0;
            // FUN_1040_3440: a spark (group 0, a sound when it appears).
            ps.p.group = 0;
            ps.p.on_spawn = 0x7576;
            ps.spawn_seq(0x1928, 3, 4, y + 2, x - 3, 1);
            ps.p.on_spawn = 0;
            ps.p.group = 1;
            ps.p.delay = 0;
            fx.shakes.push((0x12, 6));
            sound::play_at(0x49, y, x);
        }
        46 => {
            // FUN_1040_4a06: every cell of the function-60309 cross around steps on by its
            // function 2; nine splinters, dust, a shake.
            let f = 0xeb95u16;
            let (x0, y0) = (x & 0xfff0, y & 0xfff0);
            let (mut x1, mut y1, mut x2, mut y2) = (x0, y0, x0, y0);
            while world.tile_function_at(y0, x1 - 0x10) == f {
                x1 -= 0x10;
            }
            while world.tile_function_at(y1 - 0x10, x0) == f {
                y1 -= 0x10;
            }
            while world.tile_function_at(y0, x2 + 0x10) == f {
                x2 += 0x10;
            }
            while world.tile_function_at(y2 + 0x10, x0) == f {
                y2 += 0x10;
            }
            let w = world.main().width;
            let mut cx = x1;
            while cx <= x2 {
                let mut cy = y1;
                while cy <= y2 {
                    if cx >= 0 && cy >= 0 {
                        let c = ((cy >> 4) * w + (cx >> 4)) as usize;
                        if let Some(&v) = world.main().cells.get(c) {
                            let d = world.cell_function(v, 2);
                            world.main_mut().cells[c] = v.wrapping_add(d);
                        }
                    }
                    cy += 0x10;
                }
                cx += 0x10;
            }
            let ps = &mut lv.particles;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            ps.p.group = 0;
            ps.p.callback = 0x17f4;
            for i in 0..=8 {
                ps.p.delay = rng.random(8);
                ps.p.draw = rng.random(2);
                let px = x1 + 4 + i * 3;
                let py = y1 + 8 + rng.random(0x11);
                let steps = rng.random(3) + 2;
                ps.spawn(0x7552, py, px); // FUN_1050_4106: its own frame step
                ps.last_mut().m.set(0x26, steps);
                let a = (0x720 - rng.random(0x81)) - i * 0x50;
                let v = rng.random(0x10);
                ps.throw(9, v + 0x40, a);
            }
            ps.p = crate::particles::Params::default();
            crate::alien::dust(ps, y1 + 0x12, x1 + 0x10);
            fx.shakes.push((0x20, 0xc));
            sound::play_at(0x49, y1 + 0x10, x1 + 0x10);
        }
        51 => {
            // FUN_1040_517c: the block of functions 60300/60301 around goes, cell by cell, each
            // in four chips fluttering down.
            fx.shakes.push((0x20, 0xe));
            let p = rng.random(100);
            sound::pitch(p + 0xec);
            sound::play_at(0x59, y, x);
            let isb = |w: &World, yy: i32, xx: i32| (w.tile_function_at(yy, xx) as i16 as i32 + 0x1474) < 2 && (w.tile_function_at(yy, xx) as i16 as i32 + 0x1474) >= 0;
            let (x0, y0) = (x & 0xfff0, y & 0xfff0);
            let mut right = x0;
            while isb(world, y0, right + 0x10) {
                right += 0x10;
            }
            let mut bottom = y0;
            while isb(world, bottom + 0x10, right) {
                bottom += 0x10;
            }
            let mut left = x0;
            while isb(world, y0, left - 0x10) {
                left -= 0x10;
            }
            let mut top = y0;
            while isb(world, top - 0x10, left) {
                top -= 0x10;
            }
            let ps = &mut lv.particles;
            ps.p.group = 0;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            ps.p.callback = 0x177a;
            ps.p.colour = 0x10;
            let w = world.main().width;
            let mut cy = top;
            for _ in 0..=((bottom - top) >> 4) {
                let mut cx = left;
                for _ in 0..=((right - left) >> 4) {
                    let mut s = if world.tile_function_at(cy, cx) as i16 == -0x1474 { 0x7500 } else { 0x755a };
                    for k in 0..=3 {
                        if k == 2 {
                            s = 0x7500;
                        }
                        ps.p.delay = rng.random(0x10);
                        let px = cx + rng.random(0x10);
                        let py = cy + rng.random(0x10);
                        ps.spawn(s, py, px);
                        ps.last_mut().m.set(0x26, 3);
                        let a = rng.random(0x321) + 0xf50;
                        let v = rng.random(0x11);
                        ps.throw(7, v + 0x10, a);
                    }
                    if cx >= 0 && cy >= 0 {
                        if let Some(c) = world.main_mut().cells.get_mut(((cy >> 4) * w + (cx >> 4)) as usize) {
                            *c = 0;
                        }
                    }
                    cx += 0x10;
                }
                cy += 0x10;
            }
            ps.p = crate::particles::Params::default();
        }
        36 => {
            // FUN_1040_3f9c: the back plane's cell goes, chips burst (FUN_1040_3eb4).
            if let Some(Some(p)) = world.planes.get_mut(0) {
                if x >= 0 && y >= 0 && (x >> 4) < p.width && (y >> 4) < p.height {
                    p.set(x >> 4, y >> 4, 0);
                }
            }
            lv.chip_burst(y, x, rng, fx);
        }
        49 => {
            // FUN_1040_5608: the block goes (FUN_1040_3aed), smoke, six bits of rubble.
            let (x1, y1, x2, y2) = clear_block(world, y, x);
            let cx = ((x1 + x2 + 0xf) as u16 >> 1) as i32;
            let cy = ((y1 + y2 + 0xf) as u16 >> 1) as i32;
            let ps = &mut lv.particles;
            crate::alien::smoke_sprite(ps, y2 + 0xf, cx);
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            for i in 0..=5 {
                let px = (cx - 10) + i * 4;
                let py = cy - rng.random(0x14);
                let s = rng.random(2) + 0x7534;
                ps.spawn(s, py, px);
                let a = i * -0x40 + 0x600 + rng.random(0x81);
                let v = rng.random(0x20);
                ps.throw(9, v + 0x40, a);
            }
            ps.p.on_death = 0;
            ps.p.hits = 0;
            fx.shakes.push((0x18, 0xe));
            sound::play_at(0x4a, cy, cx);
        }
        58 => {
            // FUN_1040_5e4e: a crash, smoke, a spray of bits (FUN_1040_6941); an explodable tile
            // above blows up 0x19 frames later.
            let p = rng.random(0x14);
            sound::pitch(0x100 - p);
            sound::play_at(0x62, y, x);
            lv.particles.p.colour = 0x10;
            crate::particles::smoke(&mut lv.particles, rng, y - 0x10, x);
            lv.particles.p.colour = 0;
            spray(&mut lv.particles, rng, y - 9, x);
            if world.solidity_at(y - 0x10, x) & 0x80 != 0 {
                let w = world.main().width;
                lv.queue_tile_event(0x19, 0xffff, (((y - 0x10) >> 4) * w + (x >> 4)) as u16);
            }
        }
        61 => {
            // FUN_1040_600e: the cell blows up 0x14 frames later (tile -1).
            let w = world.main().width;
            lv.queue_tile_event(0x14, 0xffff, ((y >> 4) * w + (x >> 4)) as u16);
        }
        71 => {
            // FUN_1040_6861 (from 4bdf): the block goes, bits fly up (FUN_1040_677d), a splash
            // rising, smoke, a crash and a big shake.
            let (x1, _, x2, y2) = clear_block(world, y & 0xfff0, x & 0xfff0);
            let cx = ((x1 + x2) as u16 >> 1) as i32 + 7;
            let ps = &mut lv.particles;
            crate::weight::press_bits(ps, rng, y2 + 0xf, cx);
            ps.p.colour = 0x10;
            ps.spawn_seq(0x6d00, 3, 0x13, y, cx, 1);
            ps.line(8, 0x5a0);
            ps.spawn_seq(0x6c00, 4, 0x19, y2 + 7, cx, 1); // FUN_1040_66fa
            ps.p.colour = 0;
            crate::alien::smoke_sprite(ps, y2 + 0xf, cx);
            sound::play_at(0x26, y, cx);
            sound::play_at(0x1c, y, cx);
            fx.shakes.push((0x2a, 0x10));
        }
        86 => {
            // FUN_1040_3557: the 2x2 block of function 60309 it belongs to steps on by 2
            // (FUN_1040_0002, FUN_10f0_5b14), dust, a crash.
            let f = 0xeb95u16;
            let (mut x1, mut y1) = (x, y);
            while world.tile_function_at(y, x1 - 0x10) == f {
                x1 -= 0x10;
            }
            while world.tile_function_at(y1 - 0x10, x) == f {
                y1 -= 0x10;
            }
            let w = world.main().width;
            let mut c = ((y1 >> 4) * w + (x1 >> 4)) as usize;
            for _ in 0..2 {
                for k in [1, 0] {
                    if let Some(v) = world.main_mut().cells.get_mut(c + k) {
                        *v = v.wrapping_add(2);
                    }
                }
                c += w as usize;
            }
            crate::alien::dust(&mut lv.particles, y1 + 0xe, x1 + 8);
            sound::play_at(0x4a, y1 + 0xe, x1 + 8);
        }
        90 => {
            // FUN_1040_3471: a post bursts: rubble (FUN_1040_31d4), a crack, a shake.
            chips(lv, rng, y, x);
            let p = rng.random(0x29);
            sound::pitch(p + 0xec);
            sound::play_at(0x7b, y, x);
            fx.shakes.push((0xc, 7));
        }
        93 => {
            // FUN_1040_341a: on the first two tries of a level the tile goes.
            if lv.tries < 2 && world.cell_at(MAIN_PLANE, y, x).is_some() {
                let w = world.main().width;
                world.main_mut().cells[((y >> 4) * w + (x >> 4)) as usize] = 0;
            }
        }
        id if (30000..0x8000).contains(&id) => crate::sound::play_at(id as usize - 30000, y, x),
        id => eprintln!("warning: effect {id} not ported"),
    }
}

/// FUN_1050_5a07(cell): a main-plane cell steps on by its function 2 (FUN_1088_1364: a negative
/// result blasts the cell right here, nested, not queued).
fn step_cell(lv: &mut Level, world: &mut World, cell: u16, rng: &mut PascalRandom, fx: &mut Effects) {
    let w = world.main().width;
    let t = world.main().cells[cell as usize];
    let next = (t as i32 + world.cell_function(t, 2) as i16 as i32) as u16;
    if (next as i16) < 0 {
        if world.cell_function(t, 1) & 0x80 != 0 {
            let strength = -(next as i16 as i32) * 0x10;
            explode_tile(lv, world, strength, 0xffff, (cell as i32 / w) << 4, (cell as i32 % w) << 4, rng, fx);
        }
    } else {
        lv.set_cell(world, next, cell, fx);
    }
}

/// FUN_1040_3c53 / 3d9e's bits: three pairs of chips (sprites `s`, `s` + 1) thrown up and out.
fn chip_pairs(ps: &mut crate::particles::Particles, rng: &mut PascalRandom, s: i32, y: i32, x: i32) {
    ps.p.group = 0;
    ps.p.hits = 0x8000;
    ps.p.on_death = 0x10;
    for i in 0..=2 {
        for v0 in [0x50, 0x70] {
            let px = x - 4 + i * 4;
            let k = rng.random(2);
            ps.spawn(k + s, y, px);
            let a = i * -0xb0 + 0x610 + rng.random(0x81);
            let v = rng.random(0x10);
            ps.throw(9, v + v0, a);
        }
    }
    ps.p.on_death = 0;
    ps.p.hits = 0;
    ps.p.group = 1;
}

/// FUN_1040_6941(y, x): forty pairs of bits sprayed both ways (callback 0x26c8).
fn spray(ps: &mut crate::particles::Particles, rng: &mut PascalRandom, y: i32, x: i32) {
    ps.p.hits = 0x8000;
    ps.p.group = 0;
    ps.p.callback = 0x26c8;
    for i in 1..=0x28 {
        let base = (i * 5) % 8;
        let spread = (i * 0x1e0) / 0x28;
        for side in 0..2 {
            ps.p.delay = base + rng.random(8);
            let px = x - 6 + rng.random(0xc);
            let py = y - 6 + rng.random(0xc);
            ps.spawn_seq(if side == 0 { 0x5307 } else { 0x5306 }, 1, 0x46, py, px, 0);
            let r = rng.random(0x80);
            let a = if side == 0 { (r + 0x780) - spread } else { ((0x1a40 - r) + spread) % 0x1680 };
            let v = rng.random(0x30);
            ps.throw(0x1c, v, a);
        }
    }
    ps.p.delay = 0;
    ps.p.callback = 0;
    ps.p.group = 1;
    ps.p.hits = 0;
}

/// FUN_1040_31d4(y, x): five chips of rubble thrown up, each after a short delay.
fn chips(lv: &mut Level, rng: &mut PascalRandom, y: i32, x: i32) {
    let ps = &mut lv.particles;
    ps.p.group = 0;
    ps.p.hits = 0x8000;
    ps.p.on_death = 0x10;
    ps.p.callback = 0x177a;
    ps.p.colour = 0x10;
    for i in 0..=4 {
        ps.p.delay = rng.random(4);
        let px = x - 6 + i * 3;
        let py = y + rng.random(4);
        let steps = rng.random(2) + 3;
        ps.spawn(0x7500, py, px);
        ps.last_mut().m.set(0x26, steps);
        ps.throw(7, 0x20, i * 0x80 + 0xfe0);
    }
    ps.p.colour = 0;
    ps.p.callback = 0;
    ps.p.on_death = 0;
    ps.p.hits = 0;
    ps.p.group = 1;
    ps.p.delay = 0;
}

/// FUN_1040_3b1f: a wall blowing up. The fire, debris and sprite objects it spawns aren't
/// ported; their Random() calls are.
fn explosion(lv: &mut Level, world: &mut World, y: i32, x: i32, rng: &mut PascalRandom, fx: &mut Effects) {
    let (x1, y1, x2, y2) = clear_block(world, y, x);
    let cx = ((x1 + x2 + 0xf) as u16 >> 1) as i32;
    let cy = ((y1 + y2 + 0xf) as u16 >> 1) as i32;
    let ps = &mut lv.particles;
    if rng.random(4) == 0 {
        // FUN_1040_63c9: a long smoke.
        ps.p.group = 0;
        ps.p.callback = 0x19f8;
        ps.p.colour = 0x10;
        ps.spawn_seq(0x6a00, 1, 0x5bc, y2 + 0xf, cx, 0);
        ps.p.colour = 0;
        ps.p.callback = 0;
        ps.p.group = 1;
    } else {
        crate::alien::smoke_sprite(ps, y2 + 0xf, cx);
        if rng.random(3) == 0 {
            lv.bonus(world, 0xd10, y2, cx);
        }
    }
    lv.falling_critter_at(world, rng, y2 + 0xf, cx);
    let ps = &mut lv.particles;
    ps.p.group = 0;
    ps.p.hits = 0x8000;
    ps.p.on_death = 0x10;
    for i in 0..4 {
        let px = cx - 6 + i * 4;
        let s = rng.random(2) + 0x7514;
        ps.spawn(s, cy, px);
        let a = i * -0x50 + 0x5d0 + rng.random(0x81);
        let speed = rng.random(0x20) + 0x50;
        ps.throw(9, speed, a);
    }
    ps.p.on_death = 0;
    ps.p.hits = 0;
    ps.p.group = 1;
    fx.shakes.push((0x18, 0xe));
    sound::play_at(0x4a, cy, cx);
}

/// Effect 43, FUN_1040_45db(y, x): a wall tile bursts into chunks (what flies depends on the
/// function of the tile that took its place); the block it belonged to goes with it.
fn wall_chunks(lv: &mut Level, world: &mut World, y: i32, x: i32, rng: &mut PascalRandom, fx: &mut Effects) {
    let f = world.tile_function_at(y, x);
    let (mut y, mut x) = (y, x);
    let mut bottom = y; // 1170:6686
    if f == 0 {
        // (The plane-2 cell is cleared; FUN_10f0_3f68.)
        if let Some(p) = world.planes[2].as_mut() {
            let w = p.width;
            let (tx, ty) = (x >> 4, y >> 4);
            if tx >= 0 && ty >= 0 && tx < w && ty < p.height {
                p.cells[(ty * w + tx) as usize] = 0;
            }
        }
    } else {
        let (x1, y1, x2, y2) = clear_block(world, y, x);
        bottom = y2;
        x = ((x1 + x2 + 0xf) as u16 >> 1) as i32;
        y = ((y1 + y2 + 0xf) as u16 >> 1) as i32;
    }
    let ps = &mut lv.particles;
    ps.p.group = 0;
    ps.p.hits = 0x8000;
    ps.p.on_death = 0x10;
    match f as i16 {
        -0x146c => {
            // 60308: three chunks and a puff of smoke (FUN_1040_66fa).
            for i in 0..3 {
                ps.spawn(0x7531, y, (x - 4) + i * 4);
                let a = i * -0x50 + 0x5b0 + rng.random(0x81);
                let speed = rng.random(0x10);
                ps.throw(9, speed + 0x50, a);
            }
            ps.p.callback = 0x236b;
            ps.p.colour = 0x10;
            ps.spawn_seq(0x6c00, 4, 0x19, y, x, 1);
            ps.p.colour = 0;
            ps.p.callback = 0;
        }
        -0x1470 => {
            // 60304: four bits (30000), half of them mirrored.
            for i in 0..4 {
                ps.p.draw = rng.random(2);
                let px = (x - 6) + i * 4;
                let py = (y + 8) - rng.random(0x10);
                ps.spawn(30000, py, px);
                let a = i * -0x50 + 0x5d0 + rng.random(0x81);
                let speed = rng.random(0x20);
                ps.throw(9, speed + 0x40, a);
            }
            ps.p.draw = 0;
        }
        _ => {
            for i in 0..2 {
                ps.spawn(29999, y, x - 4 + i * 8);
                let a = i * -0x80 + 0x5c0 + rng.random(0x41);
                let speed = rng.random(0x10) + 0x40;
                ps.throw(9, speed, a);
            }
        }
    }
    if f as i16 == -0x146b {
        for i in 0..5 {
            ps.p.on_death = 0x16;
            ps.p.callback = 0x23b7;
            ps.spawn(0x5409, y, x - 4 + i * 2);
            let a = i * -0x10 + 0x5a0 + rng.random(0x41);
            let speed = rng.random(0x20) + 0x40;
            ps.throw(0xb, speed, a);
            ps.p.callback = 0;
        }
    }
    ps.p.on_death = 0;
    ps.p.hits = 0;
    ps.p.group = 1;
    if rng.random(3) == 0 {
        lv.bonus(world, 0xd10, bottom, x);
    }
    fx.shakes.push((0x18, 0xe));
    sound::play_at(0x4a, y, x);
    if f as i16 == -0x1470 {
        crate::alien::smoke_sprite(&mut lv.particles, y + 0x10, x);
    }
}

/// FUN_1040_3aed(y, x) (FUN_10f0_5c10/5b33): clear the block of tiles joined to (x, y) that
/// share its function; returns its extent (left, top, right, bottom tile corners).
fn clear_block(world: &mut World, y: i32, x: i32) -> (i32, i32, i32, i32) {
    let (x0, y0) = (x & 0xfff0, y & 0xfff0);
    let f = world.tile_function_at(y0, x0);
    let mut b = (x0, y0, x0, y0);
    if f == 0 || f == OUTSIDE {
        return b;
    }
    let width = world.main().width;
    let mut stack = vec![(x0, y0)];
    // The original recurses left, right, up, down; the extent doesn't depend on the order.
    while let Some((cx, cy)) = stack.pop() {
        if world.tile_function_at(cy, cx) != f {
            continue;
        }
        world.main_mut().cells[((cy >> 4) * width + (cx >> 4)) as usize] = 0;
        stack.extend([(cx - 0x10, cy), (cx + 0x10, cy), (cx, cy - 0x10), (cx, cy + 0x10)]);
        b = (b.0.min(cx), b.1.min(cy), b.2.max(cx), b.3.max(cy));
    }
    b
}

/// FUN_1040_6577: a puff of smoke, six particles, two Random() calls each.
pub fn smoke(_y: i32, _x: i32, rng: &mut PascalRandom) {
    for _ in 0..6 {
        rng.random(0xb);
        rng.random(8);
    }
}

/// FUN_1040_5833 (sprites 0x3301/2) and FUN_1040_58d4 (0x520a/b): six bits of rubble thrown
/// up and out, bouncing off walls (effect 16).
fn rubble(lv: &mut Level, sprite: i32, y: i32, x: i32, rng: &mut PascalRandom) {
    let ps = &mut lv.particles;
    ps.p.hits = 0x8000;
    ps.p.on_death = 0x10;
    for i in 0..6 {
        let px = x - 10 + i * 4;
        let py = (y - 4) - rng.random(8);
        let s = rng.random(2);
        ps.spawn(s + sprite, py, px);
        let a = i * -0x30 + 0x5d8 + rng.random(0x81);
        let speed = rng.random(0x20) + 0x20;
        ps.throw(9, speed, a);
    }
    ps.p.on_death = 0;
    ps.p.hits = 0;
}

/// FUN_1040_4fd9 (nested in 50cf): one plank steps on (FUN_1050_5a07) and two splinters fly.
fn plank_breaks(lv: &mut Level, world: &mut World, y: i32, x: i32, rng: &mut PascalRandom, fx: &mut Effects) {
    if std::env::var_os("ET_PLANK").is_some() { let c = world.cell_at(crate::world::MAIN_PLANE, y, x).unwrap_or(0); eprintln!("PLANK y {y} x {x} tile {c:#x} f2 {} f1 {:#x}", world.cell_function(c, 2) as i16, world.cell_function(c, 1)); }
    let w = world.main().width;
    let cell = ((y >> 4) * w + (x as u16 >> 4) as i32) as u16;
    step_cell(lv, world, cell, rng, fx);
    let ps = &mut lv.particles;
    ps.p.delay = rng.random(4);
    ps.p.draw = rng.random(2);
    let step = rng.random(3) + 2;
    ps.spawn(0x7552, y - 8, x); // FUN_1050_4106: its own frame step
    ps.last_mut().m.set(0x26, step);
    let side = rng.random(2);
    let a = (side * 0x28 + 0x46) * 0x10 - rng.random(0x41);
    let s = rng.random(0x10) + 0x30;
    ps.throw(9, s, a);
    let step = rng.random(3) + 2;
    ps.spawn(0x7552, y + 2, x);
    ps.last_mut().m.set(0x26, step);
    let a = ((side ^ 1) * 0x28 + 0x46) * 0x10 - rng.random(0x41);
    let s = rng.random(0x10) + 0x30;
    ps.throw(9, s, a);
}

/// FUN_1050_5a33(strength, mask, y, x): blow up the tile at (x, y) if its function 3 matches
/// `mask`: step it to its "next" tiles (function 2 offsets) as long as `strength` lasts, then
/// play its function-4 effect.
pub fn explode_tile(
    lv: &mut Level,
    world: &mut World,
    strength: i32,
    mask: u16,
    y: i32,
    x: i32,
    rng: &mut PascalRandom,
    fx: &mut Effects,
) {
    if world.tile_function_at(y, x) == OUTSIDE {
        return;
    }
    let w = world.main().width;
    let cell_index = ((y >> 4) * w + (x >> 4)) as u16;
    let original = world.main().cells[cell_index as usize];
    if std::env::var_os("ET_EXPL").is_some() {
        eprintln!("EXPL {} strength {strength}", lv.now); eprintln!("  strength {strength} mask {mask:x} at {y},{x} cell {original:#x} f1 {:#x} f2 {} f3 {} f4 {}", world.cell_function(original, 1), world.cell_function(original, 2) as i16, world.cell_function(original, 3), world.cell_function(original, 4));
    }
    if world.cell_function(original, 3) & mask == 0 {
        return;
    }
    let mut cell = original;
    let mut before;
    let mut left = (strength / 0x10).max(1);
    loop {
        before = cell;
        let off = world.cell_function(cell, 2);
        cell = if off == 0x7fff {
            0
        } else {
            (cell as i32 + off as i16 as i32) as u16
        };
        let f3 = world.cell_function(cell, 3);
        let more = f3 & 3 != 0 && left - 1 > 0 && world.cell_function(cell, 1) & 0x80 != 0;
        if !more {
            break;
        }
        left -= 1;
        if f3 & mask == 0 {
            break;
        }
    }
    lv.set_cell(world, cell, cell_index, fx);
    // The effect of the tile before the last step (the original's tile when it didn't step).
    let id = world.cell_function(before, 4);
    let (ey, ex) = ((y & !15) + 7, (x & !15) + 7);
    if id as i16 > 0 {
        lv.blast_strength = strength;
    }
    match id as i16 {
        i if i <= 0 => {}
        39 => lv.breakable_wall(world, strength, ey, ex, rng, fx),
        73 => lv.tile_burst(ey, ex, rng, fx),
        _ => effect(lv, world, id, ey, ex, rng, fx),
    }
}
