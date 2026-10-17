//! Level setup's object creation: LevelInit's icon scan (SpawnFromIconTiles 1088:1797 +
//! FUN_1088_0d5a) and OBJECTS loop, and EnemyFactory (1070:d6be), the constructors of the classes
//! the demos use. Translated field by field; the Random() calls are the original's, in order.

use crate::level::Level;
use crate::objects::{Obj, list};
use crate::rng::PascalRandom;
use crate::world::{MAIN_PLANE, World};

/// What the factory sees: the game's `_DAT_1170_6676` words and `DAT_1170_667a` (their count).
/// For a map record w[0] is the type; the icon spawners build 4..6-word records on the stack
/// whose w[0] is garbage.
pub struct Rec<'a> {
    pub w: &'a [i16],
    pub count: i32,
}

impl Rec<'_> {
    /// The word at byte offset `off` (`*(int *)(_DAT_1170_6676 + off)`).
    fn at(&self, off: usize) -> i32 {
        self.w.get(off / 2).copied().unwrap_or(0) as i32
    }

    /// `*(byte *)(_DAT_1170_6676 + 2)`.
    fn byte2(&self) -> i32 {
        (self.w.get(1).copied().unwrap_or(0) as u16 & 0xff) as i32
    }
}

pub struct Ctx<'a> {
    pub hero_x: i32,
    pub rng: &'a mut PascalRandom,
}

/// FUN_1058_0064 / FUN_1068_111e / FUN_1068_0ec3: the base constructors.
fn base(kind: u16, vmt: u16, size: usize) -> Obj {
    let mut o = Obj::new(kind, vmt, size);
    o.set(0xc, 0);
    o
}

fn actor(kind: u16, vmt: u16, size: usize) -> Obj {
    let mut o = base(kind, vmt, size);
    o.set(0xc, 1);
    o.set(0x1a, 0);
    o.set(0x1c, 0);
    o
}

fn walker(kind: u16, vmt: u16, size: usize) -> Obj {
    let mut o = actor(kind, vmt, size);
    o.set(0x22, 1);
    o.set(0x24, 0xf0);
    o
}

fn facing_hero(hero_x: i32, x: i32) -> i32 {
    if hero_x - x < 0 { -1 } else { 1 }
}

impl Level {
    /// FUN_1058_0023: put an object into list +0x36 of the sector at (x, y) (0x7fff: the default).
    pub fn add_actor(&mut self, world: &World, o: Obj, y: i32, x: i32) -> usize {
        let s = if x == 0x7fff {
            0
        } else {
            world.sector_at(y, x)
        };
        self.add_to(s, list::ACTORS, o)
    }

    pub fn add_to(&mut self, sector: usize, l: usize, o: Obj) -> usize {
        let id = self.objects.len();
        self.objects.push(Some(o));
        self.lists[sector][l].push(id);
        id
    }

    /// Fill the map fields from the record and insert (FUN_1068_0c7a).
    fn from_rec(&mut self, world: &World, mut o: Obj, r: &Rec) -> usize {
        o.set(10, r.byte2());
        o.set(0xe, r.at(4));
        o.set(0x10, r.at(6));
        let (y, x) = (o.w(0x10), o.w(0xe));
        self.add_actor(world, o, y, x)
    }

    fn obj(&mut self, id: usize) -> &mut Obj {
        self.objects[id].as_mut().unwrap()
    }

    /// FUN_1068_0cbf: an object standing under a plane-2 "carries a bonus" tile takes it.
    fn take_bonus(&mut self, world: &mut World, id: usize) {
        let (y, x) = (self.obj(id).w(0x10) - 0x10, self.obj(id).w(0xe));
        let mut bonus = 0;
        if world.tile_function_in(2, y, x) == 30000 {
            let c = world.cell_at(2, y, x).unwrap_or(0) as i32;
            if c != 0xd00 && c != 0xd10 {
                let w = world.planes[2].as_ref().unwrap().width;
                world.planes[2].as_mut().unwrap().cells[((y >> 4) * w + (x >> 4)) as usize] = 0;
                bonus = c;
            }
        }
        self.obj(id).set(0x12, bonus);
    }

    /// EnemyFactory (1070:d6be). Returns false for types it doesn't make.
    pub fn factory(&mut self, world: &mut World, cx: &mut Ctx, kind: u16, r: &Rec) -> bool {
        let rng = &mut *cx.rng;
        match kind {
            0x1428 => {
                let mut o = walker(kind, 0xd54, 0x4d);
                o.set(10, r.byte2());
                o.set(0xe, r.at(4));
                o.set(0x10, r.at(6));
                o.set(0x40, o.w(0x10));
                o.set(0x28, -10);
                o.set(0x2e, 0xb4);
                o.set(0x2a, 0);
                o.set(0x2c, 0);
                o.set(0x32, 0);
                o.set(0x36, rng.random(0x21) + 0x40);
                o.set(0x20, facing_hero(cx.hero_x, o.w(0xe)));
                o.set(0x30, 0);
                o.set(0x38, rng.random(0x10));
                if o.w(0x38) > 3 {
                    o.set(0x38, 0);
                }
                o.set(0x3a, 0);
                o.set(0x3e, 9);
                o.set_b(0x4a, 0);
                o.set_b(0x4b, 0);
                o.set(0x3c, 0);
                o.set(0x34, 0x5703);
                o.set(0x42, 0);
                o.set(0x48, 0);
                o.set(0x26, rng.random(0x14) + 0xf6);
                o.set(0x46, rng.random(6) - 1);
                if o.w(0x46) == -1 {
                    o.set(0x36, 0x10);
                }
                if r.count >= 7 {
                    o.set(0x28, -200);
                    o.set_b(0x4c, 1);
                    o.set(0x2e, 0x17);
                    o.set(0x46, 0);
                    o.set(0x36, 0);
                }
                o.set(0x14, r.at(8));
                o.set(0x16, r.at(10));
                if o.w(0x16) - 0xf == o.w(0x14) {
                    o.set(0x14, o.w(0xe));
                    o.set(0x16, o.w(0xe) + 1);
                }
                o.set(2, o.w(0x14) - 0x40);
                o.set(4, o.w(0x10) - 0x40);
                o.set(6, o.w(0x16) + 0x40);
                o.set(8, o.w(0x10) + 0x10);
                let (y, x) = (o.w(0x10), o.w(0xe));
                let id = self.add_actor(world, o, y, x);
                self.take_bonus(world, id);
            }
            0x1432 => {
                let mut o = walker(kind, 0xd68, 0x46);
                o.set(10, r.byte2());
                o.set(0xe, r.at(4));
                o.set(0x10, r.at(6));
                o.set(0x42, o.w(0x10));
                o.set(0x28, -20);
                o.set(0x2e, 0xb4);
                o.set(0x2c, 0);
                o.set(0x3e, 0);
                o.set(0x2a, 0);
                o.set(0x36, rng.random(0x51) + 0x140);
                o.set(0x38, o.w(0x36));
                o.set(0x20, facing_hero(cx.hero_x, o.w(0xe)));
                o.set(0x30, 0);
                o.set(0x3a, 0);
                o.set(0x40, 0);
                o.set(0x34, 0x5b01);
                o.set(0x3c, 0);
                o.set(0x32, 0);
                let under =
                    world.tile_function_in(MAIN_PLANE, o.w(0x10) - 0x10, o.w(0xe)) == 0x334a;
                o.set_b(0x44, under as u8);
                o.set_b(0x45, 0);
                o.set(0x26, rng.random(0x14) + 0xf6);
                o.set(0x14, -0x7fff);
                o.set(0x16, 0x7fff);
                o.set(2, r.at(8) - 0x80);
                o.set(4, o.w(0x10) - 0x60);
                o.set(6, r.at(10) + 0x80);
                o.set(8, o.w(0x10) + 0x30);
                let (y, x) = (o.w(0x10), o.w(0xe));
                let id = self.add_actor(world, o, y, x);
                // DAT_1170_0f2a is 1 after HeroReset: just counts the enemy.
                self.take_bonus(world, id);
            }
            0x145a => {
                let mut o = actor(kind, 0xdd0, 0x36);
                o.set(10, r.byte2());
                o.set(0xe, r.at(4));
                o.set(0x10, r.at(6));
                o.set(0x14, r.at(8));
                o.set(0x16, r.at(10));
                o.set(0x26, 0);
                o.set(0x28, 0);
                o.set(0x2a, 0);
                o.set(0x2c, 0x60);
                o.set(0x20, facing_hero(cx.hero_x, o.w(0xe)));
                o.set_b(0x34, 0);
                o.set(0x30, 0);
                if o.w(10) == 1 {
                    o.set(0x32, rng.random(3) + 0x5805);
                    o.set(0x24, 0);
                    o.set(0x2e, 0x2f);
                } else {
                    if o.w(10) == 2 {
                        o.set(0x32, 0x5823);
                        o.set(0x24, 1);
                    } else {
                        o.set(0x32, 0x582d);
                        o.set(0x24, 0);
                    }
                    o.set(0x2e, 0);
                }
                o.set(2, o.w(0x14) - 0x30);
                o.set(4, o.w(0x10) - 0x40);
                o.set(6, o.w(0x16) + 0x30);
                o.set(8, o.w(0x10) + 0x20);
                let (y, x) = (o.w(0x10), o.w(0xe));
                let id = self.add_actor(world, o, y, x);
                self.take_bonus(world, id);
            }
            0x13c4 => {
                let mut o = base(kind, 0xc18, 0x24);
                o.set(10, r.byte2());
                o.set(0x16, r.at(0xc));
                o.set(0x18, r.at(0xe));
                o.set(0x1a, -1);
                o.set(0xe, r.at(4));
                o.set(0x10, r.at(6));
                o.set(0x12, r.at(8));
                o.set(0x14, r.at(10));
                o.set(0x22, 0x100);
                if r.count < 9 {
                    o.set(0x1e, 0);
                } else {
                    o.set(0x1e, r.at(0x10));
                    o.set(0x1c, r.at(0x12));
                    if r.count > 10 {
                        o.set(0x22, r.at(0x14));
                    }
                }
                // FUN_10e0_02fd with DAT_1170_1ab8 = 0x100 (the box grows by the sound's reach).
                let v = 0x100;
                o.set(2, o.w(0xe) - v * 2);
                o.set(4, o.w(0x10) - (v & 0x7fff));
                o.set(6, o.w(0x12) + v * 2);
                o.set(8, o.w(0x14) + (v & 0x7fff));
                o.set(2, o.w(2) + 0xa0);
                o.set(4, o.w(4) + 100);
                o.set(6, o.w(6) - 0xa0);
                o.set(8, o.w(8) - 100);
                let (y, x) = (o.w(0x10), o.w(0xe));
                self.add_actor(world, o, y, x);
            }
            0x13d8 => {
                let o = actor(kind, 0xc5c, 0x24);
                let id = self.from_rec(world, o, r);
                let o = self.obj(id);
                o.set(0x22, 0);
                o.set(0x20, 0);
                o.set(2, o.w(0xe) - 0x50);
                o.set(4, o.w(0x10) - 0x20);
                o.set(6, o.w(0xe) + 0x50);
                o.set(8, o.w(0x10) + 0x10);
            }
            0x13e2 | 0x13f6 => {
                let (vmt, size) = if kind == 0x13e2 {
                    (0xc98, 0x42)
                } else {
                    (0xccc, 0x40)
                };
                let mut o = base(kind, vmt, size);
                o.set(10, r.byte2());
                o.set_l(0xe, (r.at(4) as u16 as i32) * 16);
                o.set_l(0x12, (r.at(6) as u16 as i32) * 16);
                o.set(0x1c, 0);
                o.set(0x1e, 0);
                o.set(0x22, 8);
                o.set(0x24, 0);
                o.set(0x32, 0);
                o.set(0x34, 0);
                o.set(0x36, 0);
                o.set(0x38, if kind == 0x13e2 { 0xc0 } else { 0x241 });
                o.set(0x20, 0);
                o.set(0x3a, 0x80);
                o.set(0x30, -1);
                if kind == 0x13e2 {
                    o.set(0x40, 0);
                }
                o.set(0x3c, 0);
                if kind == 0x13e2 {
                    o.set(0x3e, 0);
                }
                o.set(0x28, r.at(8));
                o.set(0x2a, r.at(10));
                o.set(0x2c, r.at(0xc));
                o.set(0x2e, r.at(0xe));
                o.set(2, o.w(0x28) - 0x80);
                o.set(4, o.w(0x2a) - 0x80);
                o.set(6, o.w(0x2c) + 0x80);
                o.set(8, o.w(0x2e) + 0x80);
                let (y, x) = (o.w(0x2a), o.w(0x28));
                self.add_actor(world, o, y, x);
            }
            0x140a => {
                let mut o = actor(kind, 0xcf0, 0x3d);
                o.set(10, r.byte2());
                o.set(0xe, r.at(4));
                o.set(0x10, r.at(6));
                o.set(0x22, o.w(0x10) - 0x2c);
                for f in [0x24, 0x26, 0x28, 0x2c, 0x2a] {
                    o.set(f, 0);
                }
                o.set(0x32, 0x180);
                o.set(0x34, 0xc0);
                o.set(0x20, facing_hero(cx.hero_x, o.w(0xe)));
                o.set(0x36, 0xb40);
                o.set(0x38, rng.random(2) + 0x5620);
                o.set(0x2e, 0);
                o.set(0x3a, 0);
                o.set_b(0x3c, 0);
                o.set(0x14, r.at(8));
                o.set(0x16, r.at(10));
                o.set(2, o.w(0x14) - 0x40);
                o.set(4, o.w(0x10) - 0x60);
                o.set(6, o.w(0x16) + 0x40);
                o.set(8, o.w(0x10) + 0x40);
                let (y, x) = (o.w(0x10), o.w(0xe));
                self.add_actor(world, o, y, x);
            }
            0x143c => {
                let o = actor(kind, 0xd7c, 0x28);
                let id = self.from_rec(world, o, r);
                let rnd = rng.random(100);
                let o = self.obj(id);
                o.set(0x20, if o.w(10) < 1 { 0x60 } else { 0x30 });
                o.set(0x22, 0);
                o.set(0x24, rnd + 0x15e);
                if o.w(10) == 0 {
                    o.set(2, o.w(0xe) - 0xb0);
                    o.set(6, o.w(0xe) + 0xb0);
                    o.set(4, o.w(0x10) - 0x4e);
                    o.set(8, o.w(0x10) + 0x4e);
                    o.set(0x26, -1);
                } else {
                    o.set(2, o.w(0xe) - 0x30);
                    o.set(6, o.w(0xe) + 0x30);
                    o.set(4, o.w(0x10) - 0x40);
                    o.set(8, o.w(0x10) + 0x30);
                }
                self.take_bonus(world, id);
            }
            0x148c => {
                let o = actor(kind, 0xe20, 0x22);
                let id = self.from_rec(world, o, r);
                let o = self.obj(id);
                o.set(0x20, 0);
                o.set(2, o.w(0xe) - 0x20);
                o.set(4, o.w(0x10) - 0x2e);
                o.set(6, o.w(0xe) + 0x20);
                o.set(8, o.w(0x10) + 0x10);
            }
            0x14aa => {
                let o = base(kind, 0xd24, 0x18);
                let id = self.from_rec(world, o, r);
                let o = self.obj(id);
                o.set(0x14, r.at(8));
                o.set(0x16, r.at(10));
                o.set(2, o.w(0xe));
                o.set(4, o.w(0x10));
                // FUN_10f0_1fbe: the box of sprite +0x14. Drawn only; not needed for the logic yet.
                o.set(6, o.w(0xe));
                o.set(8, o.w(0x10));
            }
            0x1478 => {
                let mut o = actor(kind, 0xdf8, 0x4d);
                for (k, off) in [2, 4, 6, 8, 10, 0xc, 0xe].into_iter().enumerate() {
                    o.set(off, r.at(4 + 2 * k));
                }
                o.set_b(0x10, 0);
                let s = world.sector_at(o.w(4), o.w(2));
                self.add_to(s, list::TRIGGERS, o);
            }
            0x13ba => {}
            0x14d2 => {
                let mut o = actor(kind, 0xde4, 0x34);
                o.set(10, r.byte2());
                o.set(0xe, r.at(4));
                o.set(0x10, r.at(6) + 9);
                o.set(0x14, r.at(8));
                o.set(0x16, r.at(10));
                for f in [0x26, 0x28, 0x2a, 0x2e, 0x24, 0x30, 0x32] {
                    o.set(f, 0);
                }
                o.set(0x2c, 0x40);
                o.set(0x20, facing_hero(cx.hero_x, o.w(0xe)));
                o.set(2, o.w(0x14) - 0x30);
                o.set(4, o.w(0x10) - 0x40);
                o.set(6, o.w(0x16) + 0x30);
                o.set(8, o.w(0x10) + 0x20);
                let (y, x) = (o.w(0x10), o.w(0xe));
                self.add_actor(world, o, y, x);
            }
            _ => {
                if std::env::var("ET_FACT").is_ok() {
                    eprintln!("FACTORY unported kind {kind:#x}");
                }
                return false;
            }
        }
        true
    }

    pub fn spawn_all(&mut self, world: &mut World, hero_x: i32, rng: &mut PascalRandom) {
        let mut cx = Ctx { hero_x, rng };
        self.spawn(world, &mut cx);
    }

    /// LevelInit's spawning: icons in plane 2 then plane 1, then the OBJECTS records.
    pub fn spawn(&mut self, world: &mut World, cx: &mut Ctx) {
        for plane in [2, MAIN_PLANE] {
            self.spawn_icons(world, cx, plane);
        }
        for rec in world.map.object_records() {
            let kind = rec[0] as u16;
            let r = Rec {
                w: &rec,
                count: rec.len() as i32,
            };
            if self.factory(world, cx, kind, &r) {
                continue;
            }
            match kind {
                1000 => self.teleports.push(rec.clone()),
                0x4b0 => {
                    let mut o = Obj::new(kind, 0xaec, 0x18);
                    // LevelInit copies 0x16 bytes from the record's word 2 to +2.
                    for (k, &v) in rec.iter().skip(2).take(11).enumerate() {
                        o.set(2 + 2 * k, v as i32);
                    }
                    let s = world.sector_at(o.w(4), o.w(2));
                    self.add_to(s, list::L66, o);
                }
                0x1446 => {
                    let w = &*world;
                    let g = |i: usize| rec.get(i).copied().unwrap_or(0) as i32;
                    let (k, x, y, len) = (g(1) & 0xff, g(2), g(3), g(4));
                    let _ = w;
                    self.make_tree(world, k, x, y, len);
                }
                0x14c8 => {
                    let w = &*world;
                    self.plat_objs.push(crate::platobj::PlatObj::lift(&rec, |y, x| w.sector_at(y, x)));
                }
                101 => {
                    let w = &*world;
                    if let Some(p) = crate::platobj::PlatObj::swing(&rec, |y, x| w.sector_at(y, x)) {
                        if std::env::var("ET_PLAT").is_ok() {
                            eprintln!("PLATOBJ made {:?}", p);
                        }
                        self.plat_objs.push(p);
                    }
                }
                k if (100..=199).contains(&k) => {
                    eprintln!("warning: OBJECTS type {k} (FUN_1060_0166) not ported")
                }
                0x44c | 0x44d | 0x2724 => {}
                k => eprintln!("warning: OBJECTS type {k:#x} not ported"),
            }
        }
    }

    /// SpawnFromIconTiles (1088:1797) for one plane.
    fn spawn_icons(&mut self, world: &mut World, cx: &mut Ctx, plane: usize) {
        let Some(p) = world.planes[plane].as_ref() else {
            return;
        };
        let (w, n) = (p.width, (p.width * p.height) as usize);
        for i in 0..n {
            let c = world.planes[plane].as_ref().unwrap().cells[i];
            if c == 0 || c & 0x8000 != 0 {
                continue;
            }
            let f = world.cell_function(c, 0);
            let (col, row) = (i as i32 % w, i as i32 / w);
            let clear = match f {
                f if f < 61000 => false,
                61700 => false, // sector icons: Level::new
                62000 | 62001 => {
                    let k = (f as i32 - 62000) * 2 + 5;
                    let x = (f as i32 - 62000) + col * 16 + 7;
                    self.make_tree(world, k, x, row * 16 + 4, 9);
                    false
                }
                61800 => false,
                61900 => true,
                _ => self.icon(world, cx, c, f, row, col),
            };
            if clear {
                world.planes[plane].as_mut().unwrap().cells[i] = 0;
            }
        }
    }

    /// Spawn what scripts asked for (FUN_1088_0d5a): enemy icons in `fx.spawns`.
    pub fn flush_spawns(&mut self, world: &mut World, cx: &mut Ctx, fx: &mut crate::level::Effects) {
        for (tile, cell) in std::mem::take(&mut fx.spawns) {
            let w = world.main().width;
            let f = world.cell_function(tile, 0);
            self.icon(world, cx, tile, f, cell as i32 / w, cell as i32 % w);
        }
    }

    /// FUN_1088_0d5a: one enemy icon. Returns whether the icon is consumed.
    fn icon(
        &mut self,
        world: &mut World,
        cx: &mut Ctx,
        c: u16,
        f: u16,
        row: i32,
        col: i32,
    ) -> bool {
        let x = col * 16;
        let (xc, y, yb) = (x + 7, row * 16, row * 16 + 0xf);
        let param = world.cell_function(c, 2) as i16;
        let main_func = world.cell_function(world.main().get(col, row), 0);
        // FUN_1068_182c / FUN_1068_17f8 build a record on the stack (w[0] unused).
        let six = |p5: i16, p4: i32, p3: i32| {
            [
                0,
                p5,
                p4 as i16,
                p3 as i16,
                (x - 0xb9) as i16,
                (x + 199) as i16,
            ]
        };
        let four = |p3: i16, p2: i32, p1: i32| [0, p3, p2 as i16, p1 as i16];
        match f {
            61400 | 61300 => {
                if main_func == 7050 {
                    return false;
                }
                let kind = if f == 61400 { 0x1428 } else { 0x1432 };
                let w = six(param, xc, yb);
                self.factory(world, cx, kind, &Rec { w: &w, count: 6 });
            }
            61100 => {
                let w = six(param, xc, yb);
                self.factory(world, cx, 0x145a, &Rec { w: &w, count: 6 });
            }
            62100 => {
                let w = six(param, xc, y);
                self.factory(world, cx, 0x14d2, &Rec { w: &w, count: 6 });
            }
            61200 => {
                let w = six(param, xc, yb);
                self.factory(world, cx, 0x140a, &Rec { w: &w, count: 6 });
            }
            61600 => {
                let w = four(param, xc, yb);
                self.factory(world, cx, 0x14be, &Rec { w: &w, count: 4 });
            }
            61500..=61599 => {
                let w = four((f - 61500) as i16, xc, yb);
                self.factory(world, cx, 0x13d8, &Rec { w: &w, count: 4 });
            }
            62200 => {
                let w = four(param, xc, yb);
                self.factory(world, cx, 0x14f0, &Rec { w: &w, count: 4 });
            }
            61000..=61099 => eprintln!("warning: icon {f} (FUN_1070_fa6a, 0x13a6) not ported"),
            _ => {}
        }
        true
    }
}
