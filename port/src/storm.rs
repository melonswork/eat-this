//! START's weather: the storm that script command 0x16 starts (1170:4eda counts it up, 4edc/4ede
//! are the lightning flash and its kind, 4ec4/4ec6 the next thunder). Only the Random() calls
//! and the counters matter here; the rain and the flashes are drawing.

use crate::level::Level;
use crate::rng::PascalRandom;

#[derive(Clone, Copy, Debug, Default)]
pub struct Storm {
    pub level: i32,
    pub flash: i32,
    pub kind: i32,
    pub due: u32,
    /// The flash (4edc) as the background proc saw it this frame (only drawing reads it).
    pub sky_flash: i32,
}

/// The rain (1170:512c): 90 drops (x < 0x148, y < 0xd0) drawn at boot under RandSeed 1000.
/// Drops 0.. fall behind the tiles (FUN_1090_0418), drops 0x1e.. in front (FUN_1090_0567).
pub fn rain_drops() -> &'static [(i32, i32)] {
    static DROPS: std::sync::OnceLock<Vec<(i32, i32)>> = std::sync::OnceLock::new();
    DROPS.get_or_init(|| {
        // Pascal's Random (as rng.rs) from 1000, without touching the game's generator's dump.
        let mut seed: u32 = 1000;
        let mut random = |n: u64| {
            seed = seed.wrapping_mul(0x0808_8405).wrapping_add(1);
            ((seed as u64 * n) >> 32) as i32
        };
        (0..0x5a).map(|_| (random(0x148), random(0xd0))).collect()
    })
}

/// Where drop `d` is on the screen when the level frame counter is `now` (before the -3 of
/// FUN_1090_0390, which draws it from 3 rows above, going down-left).
pub fn drop_at(d: (i32, i32), now: u32) -> (i32, i32) {
    let t = (now as u16).wrapping_mul(3);
    ((0x148u16.wrapping_sub(t) as i32 + d.0) % 0x148, (t as i32 + d.1) % 0xd0)
}

impl Level {
    /// FUN_1090_0567 (START's foreground, while the storm runs): the front drops that meet the
    /// top of a solid tile splash there (sprite 0x615, colour 0x10).
    pub fn rain_splashes(&mut self, world: &crate::world::World, cam: (i32, i32)) {
        let n = self.storm.level / 0x20;
        if self.storm.level <= 0 || n <= 0 {
            return;
        }
        for k in (0x1e..0x1e + n as usize).rev() {
            let (sx, sy) = drop_at(rain_drops()[k], self.now);
            let (x, y) = (sx + cam.0 - 4, sy + cam.1);
            if world.solidity_at(y, x) & 0x201 != 0 && world.solidity_at(y - 4, x) & 0x201 == 0 {
                self.particles.p.colour = 0x10;
                self.particles.spawn_seq(0x615, 4, 3, y & !15, x, 1);
                self.particles.p.colour = 0;
            }
        }
    }

    /// FUN_1088_11a3: thunder of the given kind.
    fn thunder(&mut self, rng: &mut PascalRandom, kind: i32) {
        self.storm.kind = kind;
        self.storm.flash = if kind == 0 { 0xf } else { 0x24 };
        let p = rng.random(0x96);
        let l = 0x100 - rng.random(100);
        let r = 0x100 - rng.random(100);
        let which = rng.random(2);
        crate::sound::pitch(p + 0xce);
        crate::sound::volumes(l, r);
        crate::sound::play(which as usize + 0xd);
    }

    /// Script commands 0x0d (thunder) and 0x16 (start the storm).
    pub fn storm_command(&mut self, rng: &mut PascalRandom, cmd: u8) {
        if cmd == 0x0d {
            if self.storm.level > 0 {
                self.thunder(rng, 0);
            }
            return;
        }
        if self.storm.level == 0 {
            self.storm.level = 0x20;
        }
        let k = rng.random(2);
        self.thunder(rng, k);
        self.storm.due = self.now.wrapping_add(0x8c + rng.random(0x578) as u32);
    }

    /// FUN_1090_04d7 (from START's foreground): the storm ages and thunders when it is due.
    pub fn storm_frame(&mut self, rng: &mut PascalRandom) {
        // The background proc (the city's sky) drew the flash as it was before this.
        self.storm.sky_flash = self.storm.flash;
        if self.storm.level <= 0 {
            return;
        }
        if self.storm.flash > 0 {
            self.storm.flash -= 1;
        }
        if self.storm.level < 0x3bf {
            self.storm.level += 1;
        }
        if self.storm.due <= self.now {
            self.storm.due = self.now.wrapping_add(0x8c + rng.random(0x6d6) as u32);
            let k = rng.random(3);
            self.thunder(rng, (k != 0) as i32);
        }
    }
}

impl Level {
    /// FUN_1090_009c (the water foreground): the water line of the hero's sector plus its waves.
    pub fn water_frame(&mut self, world: &crate::world::World) {
        let s = &world.sectors[self.sector.unwrap_or(0)];
        let line = s.water[1];
        if line == 0x7fff {
            return;
        }
        let mut water = line;
        if s.water[2] > 0 {
            let idx = self.now.wrapping_mul(2).wrapping_mul(0x10) % 0x1680;
            water += (crate::trig::sin(idx as i32) * s.water[2]) / 0x4000;
        }
        self.water = water as i16 as i32;
    }
}

thread_local! {
    /// The level script's `BackgroundProc=` (1170:189e's index), for the sounds of START's backdrop.
    static BG_PROC: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

pub fn set_background_proc(n: u32) {
    BG_PROC.with(|c| c.set(n));
}

/// FUN_1090_0292's sound (procs 1 and 16): every 0x46 frames the train on the bridge rumbles
/// over its length (sound 0x13, box from its first car), seen at half the camera's speed.
pub fn bridge_sound(world: &crate::world::World, sector: usize, cam: (i32, i32), now: u32) {
    if !matches!(BG_PROC.with(|c| c.get()), 1 | 16) || now % 0x46 != 0 {
        return;
    }
    let by = world.sectors.get(sector).map_or(-1, |s| s.water[4]);
    if by < 0 {
        return;
    }
    let span = world.main().width * 16 + 0x750;
    let x = (cam.0 >> 1) + ((now as i64 * 3) % span as i64) as i32 - 0x2be;
    let y = (cam.1 >> 1) + by;
    if crate::sound::audible_box((x, y - 0x20, x + 0x1d4, y)) {
        crate::sound::play(0x13);
    }
}
