//! Eat This, a source port. Bring your own CD: point --data at its BIN directory.
//!
//!   eatthis [--data DIR] [--map FILE.MAP]          play a map with the keyboard
//!   eatthis [--data DIR] --demo USERMAPS/X.DMO      watch a recorded demo
//!   eatthis [--data DIR] --headless --demo X.DMO    replay it as fast as possible and compare
//!                                                   RandSeed with the demo's CHECK entries

mod archive;
mod data;
mod demo;
mod hero;
mod rng;
mod world;

use data::{Palette, SpriteGroup, TILE};
use demo::{CHECK_PERIOD, Demo};
use hero::Hero;
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
    hero: Hero,
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
    fn new(world: World, demo: Option<Demo>) -> Game {
        let hero = Hero::new(world.map.start, 1);
        // LevelInit calls Randomize unless a demo plays; DemoLoad zeroes RandSeed.
        let seed = if demo.is_some() {
            0
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_secs() as u32)
        };
        Game {
            world,
            hero,
            rng: PascalRandom::new(seed),
            frame: 0,
            demo,
            report: CheckReport::default(),
        }
    }

    fn demo_done(&self) -> bool {
        self.demo.as_ref().is_some_and(|d| self.frame >= d.frames)
    }

    /// One game frame. `keys` is ignored while a demo plays.
    fn step(&mut self, keys: [i8; 9]) {
        let input = match &self.demo {
            Some(d) => {
                let (k, _mouse) = d.input(self.frame);
                // DemoRecordFrame tests the level frame counter, which GameFrameInGame bumps at
                // its very end: CHECK[k] is RandSeed at this input read of 0-based frame 35k.
                if self.frame % CHECK_PERIOD == 0 {
                    self.check(self.frame / CHECK_PERIOD);
                }
                k
            }
            None => keys,
        };
        self.hero.update(&self.world, &input, &mut self.rng);
        self.frame += 1;
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

/// Every .TIL and .SPR carries its own 256-colour palette. The original remaps them into one
/// level palette at load time (USED CLRS); until that is ported, each layer is drawn in its own
/// colours into an RGB frame.
struct Graphics {
    sky: [u8; 3],
    hero: Vec<SpriteGroup>,
    hero_palette: Palette,
}

impl Graphics {
    fn load(data: &Path, world: &World) -> Result<Graphics, Box<dyn Error>> {
        let hero: Vec<SpriteGroup> = ["PPHERO.SPR", "PPHERO2.SPR", "PPHERO3.SPR"]
            .iter()
            .map(|f| SpriteGroup::load(&data.join("SPRITES").join(f)))
            .collect::<Result<_, _>>()?;
        let sky = world
            .groups
            .iter()
            .flatten()
            .next()
            .ok_or("map has no tile groups")?
            .palette[0];
        let hero_palette = hero
            .iter()
            .find_map(|g| g.palette)
            .ok_or("hero sprites have no palette")?;
        Ok(Graphics {
            sky,
            hero,
            hero_palette,
        })
    }

    fn draw(&self, g: &Game, fb: &mut [u8]) {
        for px in fb.chunks_exact_mut(3) {
            px.copy_from_slice(&self.sky);
        }
        let (mw, mh) = g.world.pixel_size();
        let cam_x = (g.hero.x - W / 2).clamp(0, (mw - W).max(0));
        let cam_y = (g.hero.y - H / 2 - 20).clamp(0, (mh - H).max(0));
        let map = &g.world.map;
        for plane in [0, MAIN_PLANE] {
            for ty in cam_y / TILE..=(cam_y + H) / TILE {
                for tx in cam_x / TILE..=(cam_x + W) / TILE {
                    let cell = map.cell(plane, tx, ty);
                    if cell == 0 || cell & 0x8000 != 0 {
                        continue;
                    }
                    let gi = (cell >> 8) as usize;
                    let hidden = map.groups[gi]
                        .as_deref()
                        .is_some_and(|n| n.contains("ICON"));
                    let Some(group) = &g.world.groups[gi] else {
                        continue;
                    };
                    if hidden || (cell & 0xff) as usize >= group.len() {
                        continue;
                    }
                    blit(
                        fb,
                        &group.palette,
                        group.tile((cell & 0xff) as usize),
                        TILE,
                        TILE,
                        tx * TILE - cam_x,
                        ty * TILE - cam_y,
                        false,
                    );
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
                &self.hero_palette,
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

/// Draws indexed pixels (0 = transparent) into the RGB frame.
fn blit(fb: &mut [u8], pal: &Palette, px: &[u8], w: i32, h: i32, x0: i32, y0: i32, flip: bool) {
    for y in 0..h {
        let sy = y0 + y;
        if !(0..H).contains(&sy) {
            continue;
        }
        for x in 0..w {
            let sx = x0 + x;
            let c = px[(y * w + if flip { w - 1 - x } else { x }) as usize];
            if c != 0 && (0..W).contains(&sx) {
                let at = (sy * W + sx) as usize * 3;
                fb[at..at + 3].copy_from_slice(&pal[c as usize]);
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
        writeln!(f, "frame\t{}", Hero::WATCH.replace(',', "\t")).unwrap();
    }
    while !game.demo_done() {
        let count = game.frame + 1;
        if let Some(f) = dump.as_mut() {
            // Like the tracer's --watch: the values at the start of the frame.
            let v: Vec<String> = game.hero.watch().iter().map(|v| v.to_string()).collect();
            writeln!(f, "{count}\t{}", v.join("\t")).unwrap();
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
    let mut fb = vec![0u8; (W * H * 3) as usize];
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
        tex.update(None, &fb, W as usize * 3)?;
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
    let mut fb = vec![0u8; (W * H * 3) as usize];
    gfx.draw(&game, &mut fb);
    let mut out = format!("P6\n{W} {H}\n255\n").into_bytes();
    out.extend(&fb);
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
    let world = World::load(&map_path, &o.data.join("TILES"))?;
    let game = Game::new(world, demo);
    if o.headless {
        if game.demo.is_none() {
            return Err("--headless needs --demo".into());
        }
        run_headless(game, o.trace, o.dump.as_deref());
        return Ok(());
    }
    let gfx = Graphics::load(&o.data, &game.world)?;
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
