//! Tile explosions and the effects table (FUN_1040_60bb). Only their Random() use and their
//! changes to the map matter to the simulation; particles aren't drawn yet.

use crate::level::{Effects, Level};
use crate::rng::PascalRandom;
use crate::world::{OUTSIDE, World};

/// FUN_1040_60bb: effect `id` at (y, x); ids >= 30000 are plain sounds. The effect procedures
/// are the table at 1170:07b2 (filled at run time; `mise run trace -- ... --dump-mem 1170:07b2:200`).
pub fn effect(lv: &mut Level, world: &mut World, id: u16, y: i32, x: i32, rng: &mut PascalRandom, fx: &mut Effects) {
    match id {
        0 => {}
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
        72 => rubble(rng),
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
            rng.random(0x80); // the sound's pitch
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
            rng.random(0x1f); // the sound's pitch
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
            rng.random(0x29); // the sound's pitch
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
            rng.random(0x29); // the sound's pitch
            rng.random(0x40);
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
        76 => {}
        id if id >= 30000 => {}
        id => eprintln!("warning: effect {id} not ported"),
    }
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
}

/// Effect 43, FUN_1040_45db(y, x): a wall tile bursts into chunks (what flies depends on the
/// function of the tile that took its place); the block it belonged to goes with it.
fn wall_chunks(lv: &mut Level, world: &mut World, y: i32, x: i32, rng: &mut PascalRandom, fx: &mut Effects) {
    let f = world.tile_function_at(y, x);
    let (mut y, mut x) = (y, x);
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
        x = ((x1 + x2 + 0xf) as u16 >> 1) as i32;
        y = ((y1 + y2 + 0xf) as u16 >> 1) as i32;
    }
    let ps = &mut lv.particles;
    ps.p.group = 0;
    ps.p.hits = 0x8000;
    ps.p.on_death = 0x10;
    match f as i16 {
        -0x146c | -0x1470 => eprintln!("warning: wall chunks of tile function {f} (FUN_1040_45db) not ported"),
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
        eprintln!("warning: a wall's bonus 0xd10 (FUN_1040_6b03) not ported");
    }
    fx.shakes.push((0x18, 0xe));
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

/// FUN_1040_5833: six rubble particles, four Random() calls each.
fn rubble(rng: &mut PascalRandom) {
    for _ in 0..6 {
        rng.random(8);
        rng.random(2);
        rng.random(0x81);
        rng.random(0x20);
    }
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
        eprintln!("EXPL strength {strength} mask {mask:x} at {y},{x} cell {original:#x} f1 {:#x} f2 {} f3 {} f4 {}", world.cell_function(original, 1), world.cell_function(original, 2) as i16, world.cell_function(original, 3), world.cell_function(original, 4));
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
    match id as i16 {
        i if i <= 0 => {}
        39 => lv.breakable_wall(world, strength, ey, ex, rng, fx),
        73 => lv.tile_burst(ey, ex, rng, fx),
        _ => effect(lv, world, id, ey, ex, rng, fx),
    }
}
