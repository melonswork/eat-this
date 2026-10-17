//! Eat This, a source port. Bring your own CD: point --data at its BIN directory.
//!
//!   eatthis [--data DIR] [--map FILE.MAP]          play a map with the keyboard
//!   eatthis [--data DIR] --demo USERMAPS/X.DMO      watch a recorded demo
//!   eatthis [--data DIR] --headless --demo X.DMO    replay it as fast as possible and compare
//!                                                   RandSeed with the demo's CHECK entries

mod actors;
mod ambient;
mod alien;
mod bigali;
mod archive;
mod bg;
mod camera;
mod data;
mod decals;
mod doors;
mod drone;
mod demo;
mod effects;
mod factory;
mod gunship;
mod hero;
mod level;
mod objects;
mod particles;
mod palette;
mod platobj;
mod picture;
mod rng;
mod sentry;
mod smallbot;
mod steam;
mod storm;
mod sprites;
mod tileanim;
mod trig;
mod boss;
mod tree;
mod turret;
mod walker;
mod world;

use camera::Camera;
use data::{Palette, Script, SpriteGroup, TILE};
use demo::{CHECK_PERIOD, Demo};
use hero::Hero;
use level::{Effects, Level};
use palette::{DEFAULT_FOREGROUND, LevelPalette};
use picture::Picture;
use rng::{PascalRandom, steps_between};
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use world::{MAIN_PLANE, World};

const W: i32 = 320;
const H: i32 = 200;
const HZ: u32 = 70;

struct Options {
    data: PathBuf,
    map: Option<PathBuf>,
    demo: Option<PathBuf>,
    headless: bool,
    scale: u32,
    shot: Option<(u32, PathBuf)>,
    trace: Option<(u32, u32)>,
    dump: Option<PathBuf>,
    camera: Option<(i32, i32)>,
    bg_only: bool,
}

fn options() -> Result<Options, String> {
    let mut o = Options {
        data: PathBuf::from("../work/cd/BIN"),
        map: None,
        demo: None,
        headless: false,
        scale: 3,
        shot: None,
        trace: None,
        dump: None,
        camera: None,
        bg_only: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--data" => o.data = value()?.into(),
            "--map" => o.map = Some(value()?.into()),
            "--demo" => o.demo = Some(value()?.into()),
            "--scale" => o.scale = value()?.parse().map_err(|e| format!("--scale: {e}"))?,
            "--headless" => o.headless = true,
            "--dump" => o.dump = Some(value()?.into()),
            "--trace" => {
                let from = value()?.parse().map_err(|e| format!("--trace FROM TO: {e}"))?;
                o.trace = Some((from, value()?.parse().map_err(|e| format!("--trace FROM TO: {e}"))?));
            }
            "--bg-only" => o.bg_only = true,
            "--camera" => {
                let x = value()?.parse().map_err(|e| format!("--camera X Y: {e}"))?;
                o.camera = Some((x, value()?.parse().map_err(|e| format!("--camera X Y: {e}"))?));
            }
            "--shot" => {
                let frame = value()?.parse().map_err(|e| format!("--shot FRAME FILE.ppm: {e}"))?;
                o.shot = Some((frame, value()?.into()));
            }
            "-h" | "--help" => return Err("usage: eatthis [--data BIN] [--map FILE.MAP | --demo DIR.DMO] [--headless] [--scale N] [--shot FRAME FILE.ppm]".into()),
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    Ok(o)
}

/// A .DMO folder holds the level's .MAP and the .REC recording.
fn find_in(dir: &Path, ext: &str) -> Result<PathBuf, String> {
    std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case(ext)))
        .ok_or(format!("no .{ext} in {}", dir.display()))
}

/// Per-key hold counters: KeyPoll (1160:0002). -1 up, 0 released now, 1 pressed now, 2 held.
fn key_poll(counter: &mut i8, down: bool) -> i8 {
    *counter = match (down, *counter > 0) {
        (true, true) => 2,
        (true, false) => 1,
        (false, true) => 0,
        (false, false) => -1,
    };
    *counter
}

struct Game {
    world: World,
    level: Level,
    hero: Hero,
    camera: Camera,
    rng: PascalRandom,
    frame: u32,
    demo: Option<Demo>,
    report: CheckReport,
}

#[derive(Default)]
struct CheckReport {
    /// Whether RandSeed matched CHECK[0], the seed after level setup.
    setup: Option<bool>,
    windows: u32,
    /// Random() calls before the current window.
    calls_before: u32,
    matched: u32,
    /// Windows 1..=leading all matched: the demo replays exactly up to frame 35 * leading.
    leading: u32,
    first_mismatch: Option<(u32, String)>,
    /// (Random() calls in the original, in the port) per window.
    rows: Vec<(Option<u32>, u32)>,
}

impl Game {
    fn new(mut world: World, demo: Option<Demo>) -> Game {
        // LevelInit calls Randomize unless a demo plays; DemoLoad zeroes RandSeed.
        let seed = if demo.is_some() {
            0
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_secs() as u32)
        };
        let mut rng = PascalRandom::new(seed);
        let mut level = Level::new(&mut world);
        let mut hero = Hero::new(world.map.start, 1);
        // LevelInit: icons and OBJECTS records become objects (their constructors draw Random).
        level.spawn_all(&mut world, hero.x, &mut rng);
        // LevelFirstFrame (1088:b4db): enter the start sector (its scripts run), face its far side.
        let mut fx = Effects::default();
        let sector = world.sector_at(hero.y, hero.x);
        level.enter_sector(&mut world, sector, &mut fx);
        if let (Some(g), Some(i)) = (level.grinder.as_mut(), fx.grinder_platform) {
            g.platform = Some(i);
        }
        if std::mem::take(&mut fx.hero_launch) {
            hero.start_launch();
        }
        if level.vehicle {
            hero.start_vehicle();
        }
        let mut camera = Camera::new();
        let grinder = level.grinder.as_ref().map(|g| (g.y, g.cam_offset));
        let cam = world.sectors[sector].cam;
        hero.facing = if hero.x - cam.0 < cam.2 - hero.x {
            1
        } else {
            -1
        };
        camera.first_frame(&world, (hero.x, hero.y), grinder);
        let mut g = Game {
            camera,
            world,
            level,
            hero,
            rng,
            frame: 0,
            demo,
            report: CheckReport::default(),
        };
        // LevelFirstFrame runs a frame without the hero (as after a teleport).
        let hero_at = (g.hero.x, g.hero.y);
        g.level.doors_near_hero(&mut g.world, hero_at, true);
        let mut cx = factory::Ctx { hero_x: g.hero.x, rng: &mut g.rng };
        g.level.update_doors(&mut g.world, hero_at, true, &mut cx);
        let mut noise = g.camera.shake_started() as i32;
        if std::mem::take(&mut g.level.door_noise) {
            noise = 3;
        }
        let window = (g.camera.x, g.camera.y, g.camera.x + 0x13f, g.camera.y + 199);
        g.world_update(&mut fx, noise, false, window);
        // The end of LevelFirstFrame: the hero may start inside a wall that blows up.
        if g.world.solidity_at(g.hero.y - 0x10, g.hero.x) & 0x80 != 0 {
            let (y, x) = (g.hero.y - 0x10, g.hero.x);
            crate::effects::explode_tile(&mut g.level, &mut g.world, 0x60, 0x104, y, x, &mut g.rng, &mut fx);
        }
        for &(a, b) in &fx.shakes {
            g.camera.shake(a, b);
        }
        g
    }

    /// HeroTouchTiles (1088:5847): a sensitive lever at his body height goes off. It comes early
    /// in HeroUpdate (before the rest of it draws its Random() calls); the vehicle does it after
    /// moving.
    fn touch_tiles(&mut self, fx: &mut Effects) {
        let (hy, hx) = (self.hero.y, self.hero.x);
        for dy in [0x1c, 0xc] {
            if self.world.tile_function_at(hy - dy, hx) == level::SENSITIVE_LEVER {
                self.level.touch_sensitive(&mut self.world, (hy - dy) >> 4, hx >> 4, &mut self.rng, fx);
            }
        }
        self.flush_spawns(fx);
    }

    fn demo_done(&self) -> bool {
        self.demo.as_ref().is_some_and(|d| self.frame >= d.frames)
    }

    /// A pick-up's effect (function 4 of its tile).
    fn pick_effect(&mut self, effect: u16, f3: u16, y: i32, x: i32) {
        if effect == hero::WEAPON_PICKUP {
            self.hero.weapon_pickup(f3);
            self.level.particles.weapon_pickup(f3 as i32, y, x);
        } else if effect == 6 {
            self.level.particles.drone.pickup(y, x);
        } else if effect == 1 {
            // FUN_1040_014a: health +10 (1170:4e6c, a heart), a counter 4ea8 += 1, sparks.
            self.hero.health += 10;
            self.level.particles.bonus_pickup(y, x);
        } else if effect == 70 {
            // FUN_1040_043c: a key (1170:5104 + f3), sparks like a weapon's.
            self.hero.keys[(f3 as usize) & 0xf] = true;
            self.level.particles.weapon_pickup(f3 as i32, y, x);
        } else if effect != 0 && effect < 30000 {
            eprintln!("warning: pick-up effect {effect} not ported");
        }
    }

    fn flush_spawns(&mut self, fx: &mut Effects) {
        if std::mem::take(&mut fx.hero_launch) {
            self.hero.start_launch();
        }
        let mut cx = factory::Ctx { hero_x: self.hero.x, rng: &mut self.rng };
        self.level.flush_spawns(&mut self.world, &mut cx, fx);
    }

    /// One game frame. `keys` is ignored while a demo plays.
    fn step(&mut self, keys: [i8; 9]) {
        self.rng.frame = self.frame + 1;
        self.rng.log = std::env::var("ET_RNG").ok().and_then(|v| v.parse::<u32>().ok()) == Some(self.frame);
        // GameFrameInGame order: platforms (FUN_1078_0aef), then the hero (input read inside).
        self.level.now = self.frame;
        let mut fx = Effects::default();
        self.level.cam = (self.camera.x, self.camera.y);
        if let Some(o) = self.level.update_platforms(&mut self.world, self.camera.y) {
            self.camera.offset.1 = o;
        }
        for cell in std::mem::take(&mut self.level.pasted) {
            self.level.run_scripts(&mut self.world, cell, &mut fx);
        }
        self.flush_spawns(&mut fx);
        // FUN_10a0_023f: delayed particles due this frame.
        self.level.particles.now = self.frame;
        self.level.particles.release_due();
        {
            let mut fx = Effects::default();
            self.level.release_tile_events(&mut self.world, &mut fx, self.hero.x);
            self.flush_spawns(&mut fx);
            for &(a, b) in &fx.shakes {
                self.camera.shake(a, b);
            }
            // (465e is set afterwards, forgetting the door's.)
            self.level.door_noise = false;
        }
        // 1170:465e: a screen shake just started (or the hero fires, below).
        let mut noise = self.camera.shake_started() as i32;
        let mut mouse_now = self.hero.gun_mouse();
        let input = match &self.demo {
            Some(d) => {
                let (k, mouse) = d.input(self.frame);
                mouse_now = (mouse.0 as i32, mouse.1 as i32);
                // FUN_1088_4340: the mouse past the screen edge looks around.
                let look = |m: i32, max: i32| {
                    if m < 0 {
                        m
                    } else if m <= max {
                        0
                    } else {
                        m - max
                    }
                };
                self.camera.look = (look(mouse.0 as i32, 0x13f), look(mouse.1 as i32, 199));
                // DemoRecordFrame tests the level frame counter, which GameFrameInGame bumps at
                // its very end: CHECK[k] is RandSeed at this input read of 0-based frame 35k.
                if self.frame % CHECK_PERIOD == 0 {
                    self.check(self.frame / CHECK_PERIOD);
                }
                k
            }
            None => keys,
        };
        if std::env::var("ET_PART").ok().and_then(|v| v.parse::<u32>().ok()).is_some_and(|f| self.frame >= f && self.frame < f + 12) {
            for p in &self.level.particles.list {
                if p.m.w(0x4a) == particles::MOVE_ROCKET as i32 || std::env::var("ET_ALLPART").is_ok() {
                    eprintln!("PART frame {} pos {},{} age {} life {} state {} mask {} cb {:x} box {},{},{},{}", self.frame, p.m.w(8), p.m.w(0xa), p.m.w(0x24), p.m.w(0x22), p.m.w(0x3e), p.m.w(0x34), p.m.w(0x42), p.m.w(0x2c), p.m.w(0x2e), p.m.w(0x30), p.m.w(0x32));
                }
            }
        }
        if std::env::var("ET_HERO").ok().and_then(|v| v.parse::<u32>().ok()).is_some_and(|f| self.frame >= f && self.frame < f + 4) {
            let (hx, hy) = (self.hero.x, self.hero.y);
            eprintln!("HERO frame {} at {hx},{hy} state {} height {} ground {} {:?}", self.frame + 1, self.hero.state, self.hero.height, self.hero.ground(), self.level.plat_objs.iter().map(|p| (p.kind, p.x, p.y)).collect::<Vec<_>>());
            for dx in [0, -6, -9, -12] {
                for dy in -40..=8 {
                    eprint!(" {:x}", hero::Hero::solidity_all(&self.world, &self.level, hy + dy, hx + dx));
                }
                eprintln!();
            }
            for p in self.level.platforms.iter().flatten() {
                eprintln!("  platform {},{} {}x{}", p.x, p.y, p.width, p.height);
            }
        }
        if std::env::var("ET_TILE").ok().and_then(|v| v.parse::<u32>().ok()) == Some(self.frame) {
            for (y, x) in [(304, 1464), (304, 1460), (304, 1456), (288, 1460), (320, 1460)] {
                let c = self.world.cell_at(1, y, x).unwrap_or(0);
                eprintln!("TILE ({y},{x}) cell {c:#x} f0 {} f1 {:#x} f2 {} f3 {}", self.world.cell_function(c, 0), self.world.cell_function(c, 1), self.world.cell_function(c, 2), self.world.cell_function(c, 3));
            }
        }
        let cam = (self.camera.x, self.camera.y);
        if std::env::var("ET_CAM").is_ok() {
            eprintln!("CAM {} {} {}", self.frame, cam.0, cam.1);
        }
        if !self.hero.vehicle {
            self.touch_tiles(&mut fx);
        }
        let mut ps = std::mem::take(&mut self.level.particles);
        let mut plat_objs = std::mem::take(&mut self.level.plat_objs);
        self.hero.update(
            &self.world,
            &self.level,
            &input,
            mouse_now,
            cam,
            &mut ps,
            &mut self.rng,
            &mut plat_objs,
        );
        self.level.plat_objs = plat_objs;
        self.level.particles = ps;
        if self.hero.vehicle {
            self.touch_tiles(&mut fx);
        }
        if let Some((y, x)) = self.hero.death.take() {
            self.level.gibs_rng(y - 0x19, x, 0x20, true, &mut self.rng);
            alien::smoke_sprite(&mut self.level.particles, y, x);
            let r = self.rng.random(5);
            self.level.blood_splat(&self.world, 0x531e + r, y - 0x24, x);
        }
        for (sprite, y, x) in std::mem::take(&mut self.hero.splats) {
            self.level.blood_splat(&self.world, sprite, y, x);
        }
        self.level.hero_x = self.hero.x;
        if self.hero.veh_fired {
            noise = 2;
        }
        if let Some((y, x, aim, parity)) = self.hero.gun.shot {
            self.level.particles.hero_fire(y, x, aim, parity);
            noise = 2;
        }
        if let Some((y, x, a)) = self.hero.gun.rockets {
            self.level.particles.rockets(a, y, x);
            noise = 2;
        }
        if let Some((y, x, a)) = self.hero.gun.flame {
            self.level.particles.flame(y, x, a);
            noise = 2;
        }
        if let Some((y, x, a, parity)) = self.hero.gun.rapid {
            self.level.particles.rapid(y, x, a, parity);
            noise = 2;
        }
        if let Some((y, x, a)) = self.hero.gun.shell7 {
            self.level.particles.shell7(a, y, x);
            noise = 2;
        }
        if let Some((y, x, a)) = self.hero.gun.charge {
            self.level.particles.mine(a, y, x);
            noise = 2;
        }
        if let Some((y, x, a)) = self.hero.gun.grenade {
            self.level.particles.grenade(0x40, a, y, x, &mut self.rng);
            noise = 2;
        }
        for ((tx, ty), effect, f3, y, x) in std::mem::take(&mut self.hero.pickups) {
            // FUN_1088_5551: the pick-up leaves plane 2, then its effect (FUN_1040_60bb).
            self.world.planes[2].as_mut().unwrap().set(tx, ty, 0);
            self.pick_effect(effect, f3, y, x);
        }
        for cell in std::mem::take(&mut self.hero.script_cells) {
            self.level.run_scripts(&mut self.world, cell, &mut fx);
        }
        for cell in std::mem::take(&mut self.hero.cell_steps) {
            let v = self.world.main().cells[cell as usize];
            let off = self.world.cell_function(v, 2);
            self.level.set_cell(&mut self.world, v.wrapping_add(off), cell, &mut fx);
        }
        for (idx, y, x, pull) in std::mem::take(&mut self.hero.levers) {
            self.level.work_lever(&mut self.world, idx, y, x, pull, &mut self.rng, &mut fx);
        }
        if let Some((dir, y, x)) = self.hero.push {
            let hy = self.hero.y;
            self.level.push_block(&mut self.world, hy, dir, y, x, &mut fx);
        }
        self.flush_spawns(&mut fx);
        // After HeroUpdate: its camera nudges and shakes, then FUN_1088_0a3b(1) .. FUN_1088_0c82.
        self.camera.offset.0 += self.hero.cam_nudge.0;
        self.camera.offset.1 += self.hero.cam_nudge.1;
        for &(a, b) in &self.hero.shakes {
            self.camera.shake(a, b);
        }
        let grinder = self.level.grinder.as_ref().map(|g| (g.y, g.cam_offset));
        self.camera
            .update(&self.world, (self.hero.x, self.hero.y), true, grinder);
        // FUN_10a8_1311 (doors opening by themselves), then after the level's background
        // procedure FUN_10a8_0dab (the moving doors).
        let hero_at = (self.hero.x, self.hero.y);
        if std::env::var("ET_DOOR").is_ok() && self.frame >= 16490 && self.frame < 16508 {
            eprintln!("DOORS frame {} hero {:?}", self.frame + 1, hero_at);
        }
        self.level.cam = (self.camera.x, self.camera.y);
        self.level.doors_near_hero(&mut self.world, hero_at, false);
        let mut cx = factory::Ctx { hero_x: self.hero.x, rng: &mut self.rng };
        self.level.update_doors(&mut self.world, hero_at, false, &mut cx);
        self.hero.hide_in_exit();
        if std::mem::take(&mut self.level.door_noise) {
            noise = 3;
        }
        // FUN_1070_faec: the actors whose box meets the camera.
        let window = (
            self.camera.x,
            self.camera.y,
            self.camera.x + 0x13f,
            self.camera.y + 199,
        );
        self.world_update(&mut fx, noise, input[hero::UP] > 0, window);
        for (tile, y, x) in std::mem::take(&mut fx.bonus_pickups) {
            let (effect, f3) = (self.world.cell_function(tile, 4), self.world.cell_function(tile, 3));
            self.pick_effect(effect, f3, y, x);
        }
        for &(a, b) in &fx.shakes {
            self.camera.shake(a, b);
        }
        // The end of GameFrameInGame: 1170:4e62 counts down to the teleport.
        if self.hero.exit_timer > 0 {
            self.hero.exit_timer -= 1;
            if self.hero.exit_timer == 0 {
                self.teleport();
            }
        }
        self.frame += 1;
    }

    /// GameFrameInGame from the particles of group 0 on (also LevelFirstFrame's): group 0,
    /// the actors (FUN_1070_faec), group 1, the level's foreground procedure.
    fn world_update(&mut self, fx: &mut Effects, noise: i32, up: bool, window: (i32, i32, i32, i32)) {
        self.level.hero_x = self.hero.x;
        // FUN_1070_fb30: the things lying around.
        let cam = (self.camera.x, self.camera.y);
        self.level.cam = cam;
        {
            let mut env = actors::Env {
                world: &mut self.world,
                rng: &mut self.rng,
                fx,
                hero: (self.hero.x, self.hero.y, self.hero.height),
                frame: self.frame,
                h: self.hero.view(up),
                noise,
                camera: cam,
                hero_obj: &mut self.hero,
                shake_started: self.camera.shake_started(),
                shake_amp: self.camera.shake_amplitude(),
            };
            self.level.update_decals(&mut env, window);
        }
        // FUN_1060_14a7: the platform objects' second pass (lifts shed dust).
        {
            let sector = self.level.sector.unwrap_or(0);
            let objs = std::mem::take(&mut self.level.plat_objs);
            for p in objs.iter().filter(|p| p.kind == 4 && (p.sector == 0 || p.sector == sector)) {
                let (x1, y1, x2, y2) = p.bbox;
                if x1 <= window.2 && y1 <= window.3 && window.0 <= x2 && window.1 <= y2 {
                    platobj::lift_dust(p, &self.world, &mut self.level.particles, &mut self.rng);
                }
            }
            self.level.plat_objs = objs;
        }
        // FUN_1050_4ca5(0): debris and critters, before the actors.
        let mut pf = particles::Frame {
            world: &mut self.world,
            rng: &mut self.rng,
            fx,
            camera: cam,
            hero: (self.hero.x, self.hero.y, self.hero.height, self.hero.half_width),
        };
        self.level.update_particles(0, &mut pf);
        let mut env = actors::Env {
            world: &mut self.world,
            rng: &mut self.rng,
            fx,
            hero: (self.hero.x, self.hero.y, self.hero.height),
            frame: self.frame,
            h: self.hero.view(up),
            noise,
            camera: cam,
            hero_obj: &mut self.hero,
            shake_started: self.camera.shake_started(),
            shake_amp: self.camera.shake_amplitude(),
        };
        self.level.update_actors(&mut env, window);
        if std::mem::take(&mut fx.hero_bounce) {
            self.hero.bounce(-0x32, 10);
        }
        self.hero.x += std::mem::take(&mut fx.hero_shift);
        if let Some(y) = fx.hero_stand.take() {
            self.hero.stand_on(y);
        }
        if let Some((vy, kind, y, release)) = fx.hero_spring.take() {
            self.hero.spring(vy, kind, y, release);
        }
        if let Some(v) = fx.hero_lim.0.take() {
            self.hero.lim.0 = v;
        }
        if let Some(v) = fx.hero_lim.1.take() {
            self.hero.lim.1 = v;
        }
        // FUN_1088_9d34: the drone follows the hero and shoots.
        self.level.drone_update(&self.world, &self.hero, self.frame);
        // FUN_1050_4ca5(1): the hero's shots and their sparks.
        let mut pf = particles::Frame {
            world: &mut self.world,
            rng: &mut self.rng,
            fx,
            camera: cam,
            hero: (self.hero.x, self.hero.y, self.hero.height, self.hero.half_width),
        };
        self.level.update_particles(1, &mut pf);
        // The level's foreground procedure ((*DAT_1170_4c36)()) runs last: the grinder in GRINDER.
        let h = &self.hero;
        let hero = (h.x, h.y, h.crouch_level(), false);
        let cam_x = self.camera.x;
        self.level
            .update_grinder(&mut self.world, hero, cam_x, &mut self.rng, fx);
        self.level.storm_frame(&mut self.rng);
        self.level.water_frame(&self.world);
        // START's foreground (FUN_1090_0ace): the ship the hero was thrown from is a platform
        // (FUN_1070_00b6 on each of sprite 0x3700's plain boxes).
        if self.hero.ship != (0, 0) {
            if std::env::var_os("ET_SHIP").is_some() {
                eprintln!("SHIP {} at {:?} hero {},{} boxes {:?}", self.frame, self.hero.ship, self.hero.x, self.hero.y, sprites::hitboxes(0x3700));
            }
            for b in sprites::hitboxes(0x3700).iter().filter(|b| b.kind == 1) {
                let top = self.hero.ship.1 + b.rect.1 - 1;
                let (right, left) = (self.hero.ship.0 + b.rect.2, self.hero.ship.0 + b.rect.0);
                let v = self.hero.view(false);
                let ok = (v.state >= 1 && v.vy >= 0) || (v.state < 1 && !(-5..=-1).contains(&v.state));
                let (lo, hi) = (v.y.min(v.prev_y), v.y.max(v.prev_y));
                if left <= v.x && v.x <= right && ok && lo <= top && top <= hi {
                    self.hero.stand_on(top);
                }
            }
        }
    }

    /// FUN_1088_517a: through the exit to the other end of its teleport record, then
    /// LevelFirstFrame(0) there.
    fn teleport(&mut self) {
        let old = self.level.sector.unwrap_or(0);
        if !self.hero.teleport(&self.world, &self.level.teleports) {
            return;
        }
        self.level.particles.drone.place(self.hero.y, self.hero.x);
        let mut fx = Effects::default();
        // FUN_1068_08ff: the old sector's triggers see him go.
        self.level
            .run_triggers(old, &mut self.world, &mut fx, self.hero.x, self.hero.y);
        // LevelFirstFrame: enter the sector (FUN_1088_1280), face its far side, the camera,
        // then a frame without the hero.
        let mut noise = self.camera.shake_started() as i32;
        let sector = self.world.sector_at(self.hero.y, self.hero.x);
        self.level.enter_sector(&mut self.world, sector, &mut fx);
        let cam = self.world.sectors[sector].cam;
        self.hero.facing = if self.hero.x - cam.0 < cam.2 - self.hero.x { 1 } else { -1 };
        let grinder = self.level.grinder.as_ref().map(|g| (g.y, g.cam_offset));
        self.camera
            .first_frame(&self.world, (self.hero.x, self.hero.y), grinder);
        let hero_at = (self.hero.x, self.hero.y);
        self.level.doors_near_hero(&mut self.world, hero_at, true);
        let mut cx = factory::Ctx { hero_x: self.hero.x, rng: &mut self.rng };
        self.level.update_doors(&mut self.world, hero_at, true, &mut cx);
        if std::mem::take(&mut self.level.door_noise) {
            noise = 3;
        }
        let window = (self.camera.x, self.camera.y, self.camera.x + 0x13f, self.camera.y + 199);
        self.world_update(&mut fx, noise, false, window);
        if self.world.solidity_at(self.hero.y - 0x10, self.hero.x) & 0x80 != 0 {
            let (y, x) = (self.hero.y - 0x10, self.hero.x);
            crate::effects::explode_tile(&mut self.level, &mut self.world, 0x60, 0x104, y, x, &mut self.rng, &mut fx);
        }
        for &(a, b) in &fx.shakes {
            self.camera.shake(a, b);
        }
    }

    fn check(&mut self, k: u32) {
        let d = self.demo.as_ref().unwrap();
        let Some(&want) = d.checks.get(k as usize) else {
            return;
        };
        if k == 0 {
            // The seed after level setup. Its calls stay in window 1, which starts at
            // DemoLoad (RandSeed := 0), as in the tracer.
            self.report.setup = Some(self.rng.seed == want);
            self.rng.seed = want;
            return;
        }
        // Window k: the calls from sample k - 1 to sample k.
        let ours = self.rng.calls() - self.report.calls_before;
        self.report.calls_before = self.rng.calls();
        let prev = if k == 1 { 0 } else { d.checks[k as usize - 1] };
        self.report.windows += 1;
        let original = steps_between(prev, want, 1_000_000);
        self.report.rows.push((original, ours));
        if self.rng.seed == want {
            self.report.matched += 1;
            if self.report.first_mismatch.is_none() {
                self.report.leading = k;
            }
        } else if self.report.first_mismatch.is_none() {
            let expected = original.map_or("?".into(), |n| n.to_string());
            self.report.first_mismatch = Some((
                k,
                format!(
                    "window {k} (frames {}..{}): original made {expected} Random() calls, the port {ours}; hero at ({}, {}) state {}",
                    (k - 1) * CHECK_PERIOD + 1,
                    k * CHECK_PERIOD,
                    self.hero.x,
                    self.hero.y,
                    self.hero.state
                ),
            ));
        }
        if self.rng.seed != want {
            self.rng.seed = want; // resync, so later windows are judged on their own
        }
    }
}

/// Finds `name` in `dir` ignoring case (the script says `citybac.lbm`, the CD has `CITYBAC.LBM`).
fn find_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| {
            p.file_name()
                .is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name))
        })
}

/// The frame is one 320x200 indexed buffer and the level has one palette, as in the original:
/// tiles and the background picture are drawn as they are, sprites are fitted into the palette
/// when they load (palette.rs). `present` turns the indices into RGB.
struct Graphics {
    background: bg::Background,
    /// Debug: fixed camera position (--camera X Y) instead of following the hero.
    camera: Option<(i32, i32)>,
    /// Debug (--bg-only): draw nothing but the background.
    bg_only: bool,
    palette: Palette,
    hero: Vec<SpriteGroup>,
}

impl Graphics {
    fn load(data: &Path, world: &World, map_path: &Path) -> Result<Graphics, Box<dyn Error>> {
        // The script holds ForegroundColors and names the background; a map without one gets
        // the map's own background slot and the default split.
        let script = find_ci(map_path.parent().unwrap_or(Path::new(".")), &{
            let mut n = map_path.file_stem().unwrap_or_default().to_os_string();
            n.push(".SCR");
            n.to_string_lossy().into_owned()
        })
        .and_then(|p| std::fs::read(p).ok())
        .map(|t| Script::parse(&String::from_utf8_lossy(&t)));
        let background = script
            .as_ref()
            .and_then(|s| s.background.clone())
            .unwrap_or_else(|| world.map.background.clone());
        let picture = find_ci(&data.join("PICTURES"), &background)
            .map(|p| Picture::load(&p))
            .transpose()?;
        let colors = match &picture {
            Some(p) => p.palette,
            None => {
                // No picture: borrow the first tile group's palette.
                let g = world
                    .groups
                    .iter()
                    .flatten()
                    .next()
                    .ok_or("map has no tile groups")?;
                g.palette.map(|c| c.map(|v| v >> 2))
            }
        };
        let level = LevelPalette {
            colors,
            foreground: script
                .as_ref()
                .and_then(|s| s.foreground_colors)
                .unwrap_or(DEFAULT_FOREGROUND),
        };
        let mix = script
            .as_ref()
            .and_then(|s| s.palmix.as_deref())
            .and_then(|n| find_ci(&data.join("PALS"), n))
            .and_then(|p| data::load_palmix(&p).ok());
        let groups = script
            .as_ref()
            .map(|s| s.sprite_groups.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|name| {
                let mut g =
                    find_ci(&data.join("SPRITES"), name).and_then(|p| SpriteGroup::load(&p).ok());
                if let Some(g) = &mut g {
                    g.fit_to(&level);
                }
                g
            })
            .collect();
        let mut hero: Vec<SpriteGroup> = ["PPHERO.SPR", "PPHERO2.SPR", "PPHERO3.SPR"]
            .iter()
            .map(|f| SpriteGroup::load(&data.join("SPRITES").join(f)))
            .collect::<Result<_, _>>()?;
        for g in &mut hero {
            g.fit_to(&level);
        }
        Ok(Graphics {
            background: bg::Background {
                proc_: script.map_or(0, |s| s.background_proc),
                picture,
                groups,
                mix,
            },
            camera: None,
            bg_only: false,
            palette: level.rgb(),
            hero,
        })
    }

    /// Indexed frame -> RGB24.
    fn present(&self, fb: &[u8], rgb: &mut [u8]) {
        for (px, &i) in rgb.chunks_exact_mut(3).zip(fb) {
            px.copy_from_slice(&self.palette[i as usize]);
        }
    }

    fn draw(&self, g: &Game, fb: &mut [u8]) {
        fb.fill(0);
        let (cam_x, cam_y) = self.camera.unwrap_or((g.camera.x, g.camera.y));
        // The level frame counter as the original's draw sees it: frames completed before this one.
        let frame = g.frame.saturating_sub(1);
        self.background.draw(fb, (cam_x, cam_y), frame);
        if self.bg_only {
            return;
        }
        let map = &g.world.map;
        // The map planes, then the platforms (their own planes, at their positions).
        let mut layers: Vec<(usize, i32, i32)> = vec![(0, 0, 0), (MAIN_PLANE, 0, 0)];
        layers.extend(
            g.level
                .platforms
                .iter()
                .flatten()
                .map(|p| (p.plane, p.x, p.y)),
        );
        for (plane, ox, oy) in layers {
            let Some(pl) = g.world.planes[plane].as_ref() else {
                continue;
            };
            for ty in 0..pl.height {
                let sy = oy + ty * TILE - cam_y;
                if sy <= -TILE || sy >= H {
                    continue;
                }
                for tx in 0..pl.width {
                    let sx = ox + tx * TILE - cam_x;
                    if sx <= -TILE || sx >= W {
                        continue;
                    }
                    let cell = pl.get(tx, ty);
                    if cell == 0 || cell & 0x8000 != 0 {
                        continue;
                    }
                    let gi = (cell >> 8) as usize;
                    if gi >= map.groups.len() {
                        continue;
                    }
                    let hidden = map.groups[gi]
                        .as_deref()
                        .is_some_and(|n| n.contains("ICON"));
                    let Some(group) = &g.world.groups[gi] else {
                        continue;
                    };
                    if hidden || (cell & 0xff) as usize >= group.len() {
                        continue;
                    }
                    let t = tileanim::tile(
                        &group.infos[(cell & 0xff) as usize * 16..][..16],
                        (cell & 0xff) as u8,
                        frame,
                    );
                    blit(fb, group.tile(t as usize), TILE, TILE, sx, sy, false);
                }
            }
        }
        let f = g.hero.frame;
        if let Some(Some(s)) = self
            .hero
            .get((f >> 8) as usize)
            .and_then(|grp| grp.sprites.get((f & 0xff) as usize))
        {
            let flip = g.hero.facing < 0;
            let x = if flip {
                g.hero.x - s.hotspot.0 - s.width + 1
            } else {
                g.hero.x + s.hotspot.0
            };
            blit(
                fb,
                &s.pixels,
                s.width,
                s.height,
                x - cam_x,
                g.hero.y + s.hotspot.1 - cam_y,
                flip,
            );
        }
    }
}

/// Draws indexed pixels (0 = transparent) into the indexed frame.
fn blit(fb: &mut [u8], px: &[u8], w: i32, h: i32, x0: i32, y0: i32, flip: bool) {
    for y in 0..h {
        let sy = y0 + y;
        if !(0..H).contains(&sy) {
            continue;
        }
        for x in 0..w {
            let sx = x0 + x;
            let c = px[(y * w + if flip { w - 1 - x } else { x }) as usize];
            if c != 0 && (0..W).contains(&sx) {
                fb[(sy * W + sx) as usize] = c;
            }
        }
    }
}

fn run_headless(mut game: Game, trace: Option<(u32, u32)>, dump: Option<&Path>) {
    use std::io::Write;
    let t = Instant::now();
    let mut dump =
        dump.map(|p| std::io::BufWriter::new(std::fs::File::create(p).expect("--dump file")));
    if let Some(f) = dump.as_mut() {
        writeln!(
            f,
            "frame\t{}\t1170:4ee0\t1170:4eea\tgCameraX\tgCameraY",
            Hero::WATCH.replace(',', "\t")
        )
        .unwrap();
    }
    while !game.demo_done() {
        let count = game.frame + 1;
        if let Some(f) = dump.as_mut() {
            // Like the tracer's --watch: the values at the start of the frame.
            let v: Vec<String> = game.hero.watch().iter().map(|v| v.to_string()).collect();
            let g = game
                .level
                .grinder
                .as_ref()
                .map_or((-1, 0), |g| (g.y, g.speed));
            writeln!(
                f,
                "{count}\t{}\t{}\t{}\t{}\t{}",
                v.join("\t"),
                g.0,
                g.1,
                game.camera.x,
                game.camera.y
            )
            .unwrap();
        }
        let traced = trace.is_some_and(|(a, b)| (a..=b).contains(&count));
        let before = game.rng.calls();
        game.step([0; 9]);
        if traced {
            let (keys, _) = game.demo.as_ref().unwrap().input(count - 1);
            println!(
                "frame {count:5}  keys {keys:?}  state {:4} frame {:#05x} ({:5},{:5})  random {}",
                game.hero.state,
                game.hero.frame,
                game.hero.x,
                game.hero.y,
                game.rng.calls().wrapping_sub(before)
            );
        }
    }
    let r = &game.report;
    if let Some(ok) = r.setup {
        println!(
            "CHECK[0] (after level setup): {}",
            if ok { "ok" } else { "differs" }
        );
    }
    println!("window  original  port");
    for (k, (orig, ours)) in r.rows.iter().enumerate().take(12) {
        let o = orig.map_or("?".into(), |n| n.to_string());
        let mark = if orig == &Some(*ours) { "" } else { "  <--" };
        println!("{:>6}  {o:>8}  {ours:>4}{mark}", k + 1);
    }
    println!(
        "exact for the first {} windows ({} frames)",
        r.leading,
        r.leading * CHECK_PERIOD
    );
    println!(
        "{} frames in {:.2?}; RandSeed matched in {}/{} windows",
        game.frame,
        t.elapsed(),
        r.matched,
        r.windows
    );
    match &r.first_mismatch {
        Some((_, why)) => println!("first divergence: {why}"),
        None => println!("deterministic against this demo"),
    }
    println!(
        "hero ended at ({}, {}), state {}",
        game.hero.x, game.hero.y, game.hero.state
    );
}

fn run_window(mut game: Game, gfx: Graphics, scale: u32) -> Result<(), Box<dyn Error>> {
    use sdl2::{event::Event, keyboard::Scancode, pixels::PixelFormatEnum};
    let sdl = sdl2::init()?;
    let window = sdl
        .video()?
        .window("Eat This", W as u32 * scale, H as u32 * scale)
        .position_centered()
        .resizable()
        .build()?;
    let mut canvas = window.into_canvas().present_vsync().build()?;
    canvas.set_logical_size(W as u32, H as u32)?;
    let tc = canvas.texture_creator();
    let mut tex = tc.create_texture_streaming(PixelFormatEnum::RGB24, W as u32, H as u32)?;
    let mut events = sdl.event_pump()?;
    let mut fb = vec![0u8; (W * H) as usize];
    let mut rgb = vec![0u8; (W * H * 3) as usize];
    let mut counters = [-1i8; 9];
    // gKeyMap defaults: arrows, Insert (LeftShoot), Space (RightShoot), Tab (SwitchWeapon).
    let keymap = [
        Scancode::Up,
        Scancode::Down,
        Scancode::Left,
        Scancode::Right,
        Scancode::Insert,
        Scancode::Space,
        Scancode::Insert,
        Scancode::Space,
        Scancode::Tab,
    ];
    let tick = Duration::from_secs(1) / HZ;
    let mut next = Instant::now();
    'main: loop {
        for e in events.poll_iter() {
            if let Event::Quit { .. }
            | Event::KeyDown {
                scancode: Some(Scancode::Escape),
                ..
            } = e
            {
                break 'main;
            }
        }
        while Instant::now() >= next {
            let ks = events.keyboard_state();
            let mut keys = [0i8; 9];
            for (i, sc) in keymap.iter().enumerate() {
                keys[i] = key_poll(&mut counters[i], ks.is_scancode_pressed(*sc));
            }
            if !game.demo_done() {
                game.step(keys);
            }
            next += tick;
        }
        gfx.draw(&game, &mut fb);
        gfx.present(&fb, &mut rgb);
        tex.update(None, &rgb, W as usize * 3)?;
        canvas.copy(&tex, None, None)?;
        canvas.present();
        let title = match &game.demo {
            Some(d) => format!(
                "Eat This - demo frame {}/{} - RandSeed {}/{} windows",
                game.frame, d.frames, game.report.matched, game.report.windows
            ),
            None => format!(
                "Eat This - ({}, {}) state {}",
                game.hero.x, game.hero.y, game.hero.state
            ),
        };
        canvas.window_mut().set_title(&title)?;
    }
    Ok(())
}

/// Runs `frames` frames without a window and writes the last one as a binary PPM.
fn screenshot(mut game: Game, gfx: &Graphics, frames: u32, path: &Path) -> std::io::Result<()> {
    for _ in 0..frames {
        game.step([-1; 9]);
    }
    let mut fb = vec![0u8; (W * H) as usize];
    gfx.draw(&game, &mut fb);
    if path.extension().is_some_and(|e| e == "bin") {
        return std::fs::write(path, &fb); // the raw indexed frame, for comparing with tools/dumpframe.py
    }
    let mut rgb = vec![0u8; (W * H * 3) as usize];
    gfx.present(&fb, &mut rgb);
    let mut out = format!("P6\n{W} {H}\n255\n").into_bytes();
    out.extend(&rgb);
    std::fs::write(path, out)
}

fn main() -> Result<(), Box<dyn Error>> {
    let o = options()?;
    let (map_path, demo) = match &o.demo {
        Some(dir) => (
            find_in(dir, "MAP")?,
            Some(Demo::load(&find_in(dir, "REC")?)?),
        ),
        None => (
            o.map.clone().unwrap_or(o.data.join("MAPS/CITY01.MAP")),
            None,
        ),
    };
    world::load_game_ds(&o.data.join("../ETLAUNCH.EXE"))?;
    let _ = sprites::DIR.set(o.data.join("SPRITES"));
    // The script's LoadSpriteGroup= lines fill slots 0x32.. in order (an empty one skips a slot).
    let groups: Vec<String> = find_ci(map_path.parent().unwrap_or(Path::new(".")), &{
        let mut n = map_path.file_stem().unwrap_or_default().to_os_string();
        n.push(".SCR");
        n.to_string_lossy().into_owned()
    })
    .and_then(|p| std::fs::read(p).ok())
    .map(|t| {
        String::from_utf8_lossy(&t)
            .lines()
            .filter_map(|l| {
                let l = l.trim();
                let (k, v) = l.split_once('=')?;
                k.eq_ignore_ascii_case("LoadSpriteGroup").then(|| v.split(',').next().unwrap_or("").trim().to_string())
            })
            .collect()
    })
    .unwrap_or_default();
    let _ = sprites::LEVEL_GROUPS.set(groups);
    let world = World::load(&map_path, &o.data.join("TILES"))?;
    let game = Game::new(world, demo);
    if o.headless {
        if game.demo.is_none() {
            return Err("--headless needs --demo".into());
        }
        run_headless(game, o.trace, o.dump.as_deref());
        return Ok(());
    }
    let mut gfx = Graphics::load(&o.data, &game.world, &map_path)?;
    gfx.camera = o.camera;
    gfx.bg_only = o.bg_only;
    if let Some((frames, path)) = &o.shot {
        return Ok(screenshot(game, &gfx, *frames, path)?);
    }
    run_window(game, gfx, o.scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_poll_matches_the_original() {
        let mut c = -1;
        let seq: Vec<i8> = [false, true, true, true, false, false]
            .iter()
            .map(|&d| key_poll(&mut c, d))
            .collect();
        assert_eq!(seq, [-1, 1, 2, 2, 0, -1]);
    }
}
