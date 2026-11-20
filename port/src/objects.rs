//! The original's game objects (enemies, emitters, triggers, ...): Borland Pascal objects whose
//! first word is the VMT, kept here as raw memory so the decompiled methods translate field by
//! field (`o.w(0x0e)` is `*(int *)(obj + 0xe)`). They live in per-sector collections; see
//! re/objects.md.

/// A sector's seven collections (FUN_1068_0002), by byte offset in the sector object.
pub mod list {
    pub const L1E: usize = 0;
    pub const L2A: usize = 1;
    /// +0x36: enemies and most actors; updated by FUN_1070_faec through VMT +0xc.
    pub const ACTORS: usize = 2;
    /// +0x42: things the hero code updates (FUN_1060_145a).
    pub const L42: usize = 3;
    /// +0x4e: rectangle triggers (record 0x1478), FUN_1068_08ff.
    pub const TRIGGERS: usize = 4;
    /// +0x5a: updated by FUN_1070_fb30.
    pub const L5A: usize = 5;
    /// +0x66: record 0x4b0 data objects.
    pub const L66: usize = 6;
    pub const COUNT: usize = 7;
}

#[derive(Clone, Debug)]
pub struct Obj {
    /// The record type the factory made it from (EnemyFactory's switch), for dispatch.
    pub kind: u16,
    pub mem: Vec<u8>,
    /// What the update method drew this frame (the original draws at the end of the update
    /// from values it computed on the way). Only the draw pass reads it.
    pub looks: Vec<Look>,
}

/// One sprite an object's update drew: world position, mode (1ae0), colour (1ae3; `FLASH` is
/// the hit flash colour 1170:1b1b), mirrored (1ae5), and the table MIX points at.
#[derive(Clone, Copy, Debug, Default)]
pub struct Look {
    pub sprite: i32,
    pub x: i32,
    pub y: i32,
    pub mode: i32,
    pub colour: i32,
    pub hflip: bool,
    /// 0 PalMix, 1 the tint table (4c26), 2 PalMixLight.
    pub mix: u8,
}

impl Look {
    /// Colour value standing for 1170:1b1b.
    pub const FLASH: i32 = -1;

    /// The hit flash (mode 0x24, colour 1b1b) when `on`.
    pub fn flash(mut self, on: bool) -> Look {
        if on {
            self.mode = 0x24;
            self.colour = Look::FLASH;
        }
        self
    }

    pub fn flip(mut self, f: bool) -> Look {
        self.hflip = f;
        self
    }

    pub fn mode(mut self, m: i32) -> Look {
        self.mode = m;
        self
    }
}

impl Obj {
    /// A fresh object of `size` bytes (the VMT's size word) with its VMT pointer at +0.
    pub fn new(kind: u16, vmt: u16, size: usize) -> Obj {
        let mut o = Obj {
            kind,
            mem: vec![0; size.max(2)],
            looks: Vec::new(),
        };
        o.set(0, vmt as i32);
        o
    }

    pub fn vmt(&self) -> u16 {
        self.uw(0)
    }

    /// Signed 16-bit field.
    pub fn w(&self, off: usize) -> i32 {
        i16::from_le_bytes([self.mem[off], self.mem[off + 1]]) as i32
    }

    pub fn uw(&self, off: usize) -> u16 {
        u16::from_le_bytes([self.mem[off], self.mem[off + 1]])
    }

    pub fn set(&mut self, off: usize, v: i32) {
        self.mem[off..off + 2].copy_from_slice(&(v as u16).to_le_bytes());
    }

    pub fn b(&self, off: usize) -> u8 {
        self.mem[off]
    }

    pub fn set_b(&mut self, off: usize, v: u8) {
        self.mem[off] = v;
    }

    /// 32-bit field (a Pascal longint, low word first).
    pub fn l(&self, off: usize) -> i32 {
        i32::from_le_bytes(self.mem[off..off + 4].try_into().unwrap())
    }

    pub fn set_l(&mut self, off: usize, v: i32) {
        self.mem[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// A sprite at (x, y) for the draw pass (see `looks`).
    pub fn look(sprite: i32, x: i32, y: i32) -> Look {
        Look { sprite, x, y, ..Look::default() }
    }

    /// The activity box (+2..+8) every object has: it only updates while this meets the camera.
    pub fn bbox(&self) -> (i32, i32, i32, i32) {
        (self.w(2), self.w(4), self.w(6), self.w(8))
    }
}
