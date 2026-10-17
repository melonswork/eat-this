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
}

impl Level {
    /// FUN_1088_11a3: thunder of the given kind.
    fn thunder(&mut self, rng: &mut PascalRandom, kind: i32) {
        self.storm.kind = kind;
        self.storm.flash = if kind == 0 { 0xf } else { 0x24 };
        rng.random(0x96); // the pitch
        rng.random(100);
        rng.random(100);
        rng.random(2); // which sound
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
