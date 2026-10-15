//! The game's sprite groups by slot (DAT_1170_6188, filled by FUN_1088_c98f): sprite id
//! `slot * 256 + frame`. Only the hit boxes are needed by the simulation; groups load on first
//! use from BIN/SPRITES.

use crate::data::SpriteGroup;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Slot -> file, as FUN_1088_c98f loads them (seen with the tracer's --dump-groups).
const SLOTS: &[(u16, &str)] = &[
    (0x06, "PI2ANIM"), (0x09, "FIREDRO2"), (0x18, "PPWEAPON"), (0x19, "PPWEAP02"),
    (0x1a, "PPFLAMER"), (0x1f, "ETSCORE"), (0x20, "PPFONT3"), (0x21, "BONUSANI"),
    (0x22, "ETBONUS"), (0x28, "STATSOBJ"), (0x47, "BOWL"), (0x48, "CRATE"), (0x4c, "MINE2"),
    (0x4d, "FLYDROID"), (0x4e, "FLYPART"), (0x4f, "WALLG5"), (0x50, "BMBDROID"),
    (0x51, "ENEMYANI"), (0x52, "ENMYANI2"), (0x53, "REDBLD"), (0x54, "GREENBLD"), (0x55, "CARS"),
    (0x56, "WALKER"), (0x57, "SMALLALI"), (0x58, "SMALLBOT"), (0x59, "CATERP"), (0x5a, "GRANNY"),
    (0x5b, "BIGALI"), (0x5c, "ALIPLAT"), (0x5d, "JELLY"), (0x5e, "JELLYBLD"), (0x64, "EXP03"),
    (0x65, "DOTEXP"), (0x67, "SMDUST"), (0x68, "REDBEXP"), (0x69, "2BOWLEX2"), (0x6a, "FLAME5"),
    (0x6b, "SMOKECOL"), (0x6c, "SMOKEP6"), (0x6d, "DUST"), (0x6e, "ATEXP07"), (0x6f, "BIGBOOM3"),
    (0x70, "BOWLEXP"), (0x75, "SVN"), (0x78, "MPOINTER"), (0x7d, "MOVIE"),
];

/// Per slot: each frame's first hit box when it is a plain box (type 0), else None.
type Boxes = Vec<Option<(i32, i32, i32, i32)>>;
static GROUPS: OnceLock<Mutex<HashMap<u16, Boxes>>> = OnceLock::new();

fn i16_at(b: &[u8], at: usize) -> i32 {
    i16::from_le_bytes([b[at], b[at + 1]]) as i32
}

fn load(slot: u16) -> Boxes {
    let Some(&(_, name)) = SLOTS.iter().find(|(s, _)| *s == slot) else {
        eprintln!("warning: sprite slot {slot:#x} unknown");
        return Vec::new();
    };
    let path = DIR.get().expect("sprites::DIR").join(format!("{name}.SPR"));
    let Ok(g) = SpriteGroup::load(&path) else {
        eprintln!("warning: can't load {}", path.display());
        return Vec::new();
    };
    g.sprites
        .iter()
        .map(|s| {
            let e = &s.as_ref()?.extra;
            (e.len() >= 18 && i16_at(e, 0) != 0 && i16_at(e, 2) == 0)
                .then(|| (i16_at(e, 10), i16_at(e, 12), i16_at(e, 14), i16_at(e, 16)))
        })
        .collect()
}

/// Sprite `id`'s hit box relative to its anchor: (left, top, right, bottom) = sprite
/// +0x14, +0x16, +0x18, +0x1a, when its first box is a plain one (count +0xa != 0, type +0xc == 0).
pub fn hitbox(id: i32) -> Option<(i32, i32, i32, i32)> {
    let slot = (id as u16) >> 8;
    let mut m = GROUPS.get_or_init(Default::default).lock().unwrap();
    let boxes = m.entry(slot).or_insert_with(|| load(slot));
    boxes.get((id & 0xff) as usize).copied().flatten()
}
