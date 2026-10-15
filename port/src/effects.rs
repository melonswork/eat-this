//! Tile explosions and the effects table (FUN_1040_60bb). Only their Random() use and their
//! changes to the map matter to the simulation; particles aren't drawn yet.

use crate::level::{Effects, Level};
use crate::rng::PascalRandom;
use crate::world::{OUTSIDE, World};

/// FUN_1040_60bb: effect `id` at (y, x); ids >= 30000 are plain sounds. The effect procedures
/// are the table at 1170:07b2 (filled at run time; `mise run trace -- ... --dump-mem 1170:07b2:200`).
pub fn effect(world: &World, id: u16, y: i32, x: i32, rng: &mut PascalRandom, fx: &mut Effects) {
    match id {
        0 => {}
        34 => explosion(world, y, x, rng, fx),
        72 => rubble(rng),
        // Seen in GRINDER with no Random() calls in the tracer.
        76 => {}
        id if id >= 30000 => {}
        id => eprintln!("warning: effect {id} not ported"),
    }
}

/// FUN_1040_3b1f: a wall blowing up. The fire, debris and sprite objects it spawns aren't
/// ported; their Random() calls are.
fn explosion(world: &World, y: i32, x: i32, rng: &mut PascalRandom, fx: &mut Effects) {
    if rng.random(4) != 0 && rng.random(3) == 0 {
        // FUN_1040_6b03(0xd10, ...): spawns an object (not ported).
    }
    falling_debris(world, y + 0xf, x, rng);
    for _ in 0..4 {
        rng.random(2);
        rng.random(0x81);
        rng.random(0x20);
    }
    fx.shakes.push((0x18, 0xe));
}

/// FUN_1040_1d72: sometimes a crate falls from the blast.
fn falling_debris(_world: &World, _y: i32, _x: i32, rng: &mut PascalRandom) {
    if rng.random(5) == 0 {
        eprintln!("warning: FUN_1040_1d72's falling object is not ported");
    }
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
    if world.cell_function(original, 3) & mask == 0 {
        return;
    }
    let mut cell = original;
    let mut left = (strength / 0x10).max(1);
    loop {
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
    let id = world.cell_function(original, 4);
    if id as i16 > 0 {
        effect(world, id, (y & !15) + 7, (x & !15) + 7, rng, fx);
    }
}
