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
}

impl Obj {
    /// A fresh object of `size` bytes (the VMT's size word) with its VMT pointer at +0.
    pub fn new(kind: u16, vmt: u16, size: usize) -> Obj {
        let mut o = Obj { kind, mem: vec![0; size.max(2)] };
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

    /// The activity box (+2..+8) every object has: it only updates while this meets the camera.
    pub fn bbox(&self) -> (i32, i32, i32, i32) {
        (self.w(2), self.w(4), self.w(6), self.w(8))
    }
}
