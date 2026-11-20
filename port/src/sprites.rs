//! The game's sprite groups by slot (DAT_1170_6188, filled by FUN_1088_c98f): sprite id
//! `slot * 256 + frame`. Only the hit boxes are needed by the simulation; groups load on first
//! use from BIN/SPRITES.

use crate::data::SpriteGroup;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub static DIR: OnceLock<PathBuf> = OnceLock::new();
/// The level script's LoadSpriteGroup= names (slots 0x32..).
pub static LEVEL_GROUPS: OnceLock<Vec<String>> = OnceLock::new();
/// Those with the `,N` flag: loaded without fitting them to the level palette (their pixels
/// index the mix tables, like PPGLIGHT's).
pub static LEVEL_RAW: OnceLock<Vec<bool>> = OnceLock::new();

/// Slot -> file, as FUN_1088_c98f loads them (seen with the tracer's --dump-groups).
const SLOTS: &[(u16, &str)] = &[
    (0x00, "PPHERO"), (0x01, "PPHERO2"), (0x02, "PPHERO3"),
    (0x06, "PI2ANIM"), (0x09, "FIREDRO2"), (0x18, "PPWEAPON"), (0x19, "PPWEAP02"),
    (0x1a, "PPFLAMER"), (0x1f, "ETSCORE"), (0x20, "PPFONT3"), (0x21, "BONUSANI"),
    (0x22, "ETBONUS"), (0x23, "PPFONT3"), (0x24, "MENUSL"), (0x27, "ETCIFER3"), (0x29, "EPISPIC"), (0x2a, "EXITPIC"), (0x28, "STATSOBJ"), (0x47, "BOWL"), (0x48, "CRATE"), (0x4c, "MINE2"),
    (0x4d, "FLYDROID"), (0x4e, "FLYPART"), (0x4f, "WALLG5"), (0x50, "BMBDROID"),
    (0x51, "ENEMYANI"), (0x52, "ENMYANI2"), (0x53, "REDBLD"), (0x54, "GREENBLD"), (0x55, "CARS"),
    (0x56, "WALKER"), (0x57, "SMALLALI"), (0x58, "SMALLBOT"), (0x59, "CATERP"), (0x5a, "GRANNY"),
    (0x5b, "BIGALI"), (0x5c, "ALIPLAT"), (0x5d, "JELLY"), (0x5e, "JELLYBLD"), (0x64, "EXP03"),
    (0x65, "DOTEXP"), (0x67, "SMDUST"), (0x68, "REDBEXP"), (0x69, "2BOWLEX2"), (0x6a, "FLAME5"),
    (0x6b, "SMOKECOL"), (0x6c, "SMOKEP6"), (0x6d, "DUST"), (0x6e, "ATEXP07"), (0x6f, "BIGBOOM3"),
    (0x70, "BOWLEXP"), (0x75, "SVN"), (0x78, "MPOINTER"), (0x7d, "MOVIE"),
];

/// A hit box record (sprite +0xc + 0x10 * i): type, value, (left, top, right, bottom).
#[derive(Clone, Copy, Debug)]
pub struct HitBox {
    pub kind: i32,
    pub value: i32,
    pub rect: (i32, i32, i32, i32),
}

#[derive(Clone, Debug)]
struct Frame {
    height: i32,
    width: i32,
    hotspot: (i32, i32),
    boxes: Vec<HitBox>,
}

/// Per slot and frame.
type Boxes = Vec<Option<Frame>>;
static GROUPS: OnceLock<Mutex<HashMap<u16, Boxes>>> = OnceLock::new();

fn i16_at(b: &[u8], at: usize) -> i32 {
    i16::from_le_bytes([b[at], b[at + 1]]) as i32
}

/// Slots only the drawing code uses (FUN_1088_d073 loads them). Kept apart from SLOTS so that
/// hit-box lookups behave exactly as before.
const DRAW_SLOTS: &[(u16, &str)] = &[(0x1b, "PPGLIGHT"), (0x73, "FACBROCK")];

fn slot_name(slot: u16, draw: bool) -> Option<String> {
    let level = (slot >= 0x32)
        .then(|| LEVEL_GROUPS.get().and_then(|g| g.get(slot as usize - 0x32)))
        .flatten()
        .filter(|n| !n.is_empty())
        .map(|n| n.trim_end_matches(".spr").trim_end_matches(".SPR").to_ascii_uppercase());
    let fixed = SLOTS
        .iter()
        .chain(if draw { DRAW_SLOTS } else { &[] })
        .find(|(s, _)| *s == slot)
        .map(|&(_, n)| n.to_string());
    fixed.or(level)
}

/// Groups FUN_1088_d073 loads through FUN_1088_4518, which skips the palette fitting
/// (FUN_10f0_1a99): their pixels are raw indices, mostly into the mix tables.
pub fn raw(slot: u16) -> bool {
    matches!(slot, 0x1b | 0x73 | 0x75)
        || (slot >= 0x32
            && LEVEL_RAW
                .get()
                .and_then(|r| r.get(slot as usize - 0x32))
                .copied()
                .unwrap_or(false))
}

/// The file behind sprite group `slot`, for drawing.
pub fn path(slot: u16) -> Option<PathBuf> {
    Some(DIR.get()?.join(format!("{}.SPR", slot_name(slot, true)?)))
}

fn load(slot: u16) -> Boxes {
    let Some(name) = slot_name(slot, false) else {
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
            let s = s.as_ref()?;
            let e = &s.extra;
            let n = if e.len() >= 2 { i16_at(e, 0).max(0) as usize } else { 0 };
            let boxes = (0..n)
                .filter(|i| 2 + i * 0x10 + 0x10 <= e.len())
                .map(|i| {
                    let b = 2 + i * 0x10;
                    HitBox {
                        kind: i16_at(e, b),
                        value: i16_at(e, b + 2),
                        rect: (i16_at(e, b + 8), i16_at(e, b + 10), i16_at(e, b + 12), i16_at(e, b + 14)),
                    }
                })
                .collect();
            Some(Frame { height: s.height, width: s.width, hotspot: s.hotspot, boxes })
        })
        .collect()
}

/// Sprite `id`'s hit box relative to its anchor: (left, top, right, bottom) = sprite
/// +0x14, +0x16, +0x18, +0x1a, when its first box is a plain one (count +0xa != 0, type +0xc == 0).
pub fn hitbox(id: i32) -> Option<(i32, i32, i32, i32)> {
    let f = frame(id)?;
    let b = f.boxes.first()?;
    (b.kind == 0).then_some(b.rect)
}

/// All the sprite's hit boxes (sorted by type, as the files keep them).
pub fn hitboxes(id: i32) -> Vec<HitBox> {
    frame(id).map_or(Vec::new(), |f| f.boxes)
}

fn frame(id: i32) -> Option<Frame> {
    let slot = (id as u16) >> 8;
    let mut m = GROUPS.get_or_init(Default::default).lock().unwrap();
    let boxes = m.entry(slot).or_insert_with(|| load(slot));
    boxes.get((id & 0xff) as usize).cloned().flatten()
}

/// FUN_10f0_1fbe's extent of sprite `id` drawn at the origin: (left, top, right, bottom).
pub fn draw_box(id: i32) -> Option<(i32, i32, i32, i32)> {
    let f = frame(id)?;
    let (l, t) = f.hotspot;
    Some((l, t, l + f.width - 1, t + f.height - 1))
}

/// FUN_10f0_1f8e: the sprite's height.
pub fn height(id: i32) -> i32 {
    frame(id).map_or(0, |f| f.height)
}

/// FUN_1050_5960's box for a sprite drawn at (x, y), mirrored when `flip` (1170:1ae5 = 1):
/// (bottom, right, top, left), the order FUN_1050_568b takes them.
pub fn hit_rect(id: i32, y: i32, x: i32, flip: bool) -> Option<(i32, i32, i32, i32)> {
    let (l, t, r, b) = hitbox(id)?;
    Some(if flip {
        (y + b, x - l, y + t, x - r)
    } else {
        (y + b, x + r, y + t, x + l)
    })
}
