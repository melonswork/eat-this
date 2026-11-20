//! The front end (segment 1080, re/frontend.md): the menus, the level start through them, the
//! stats screen after a level, the next level / episode screen, the in-game menu, settings in
//! EatThis.ini and the "savings" (the next level of a finished one, under START USER MAP).
//! It only drives the simulation the way the window loop does: a level starts with
//! `Game::new` (RandSeed 0x3e00 + HeroReset, FUN_1088_d4b2) and runs with `Game::step`.

pub mod gfx;
pub mod menu;
pub mod texts;

use crate::level::{self, Tally};
use crate::palette::Pal6;
use crate::picture::Picture;
use crate::{Game, Graphics, sound};
use gfx::{Gfx, MENU_FONT, Mode, W, H};
use menu::{Id, Kind, Menu, Ui};
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use texts::{MainScript, MenuTexts, Settings, find_ci, localized, script_pairs};

/// The episodes' first maps (1170:066c, 0x11 bytes each).
const EPISODE_MAPS: [&str; 3] = ["START.MAP", "FOREST04.MAP", "FORPLANE.MAP"];

/// Key for the sound menu's looping test sound (FUN_10e0_0408 with the menu's +0x2e).
const SOUND_TEST: u64 = (1 << 41) + 0x4842;

/// A user map (FUN_1030_00f6): a folder under USERMAPS (or the savings) with a script and map.
struct UserMap {
    map: PathBuf,
    title: String,
    comments: String,
}

/// The level being played.
struct Running {
    game: Game,
    gfx: Graphics,
    map: PathBuf,
    /// 1170:4eaa, 4eae, 4eb2: bios spheres, hidden areas and enemies the level has.
    totals: Tally,
}

pub struct FrontEnd {
    data: PathBuf,
    settings: Settings,
    main: MainScript,
    menu_texts: Vec<String>,
    gfx: Gfx,
    ui: Ui,
    menus: HashMap<Id, Menu>,
    cur: Id,
    old: Id,
    /// 1170:1080: the slide between two menus (0xb40 down to 0).
    slide: i32,
    /// 1170:108e: a menu shown remembers the one it came from.
    link_parent: bool,
    /// gInGame (1170:0f28).
    in_game: bool,
    /// 1170:0f2a: 0 no level, 2 a level is loaded, -1 its stats were just closed.
    level_state: i32,
    level: Option<Running>,
    /// 1170:1106: the map asked for (kept while "THIS WILL END YOUR CURRENT GAME" asks).
    pending: Option<PathBuf>,
    /// 1170:4720 is a game frame (else a picture is loaded there).
    back_is_snapshot: bool,
    /// The game's last frame (indexed) for the in-game menu.
    game_fb: Vec<u8>,
    user_maps: Vec<UserMap>,
    /// KeyPoll counters for the game's nine keys and the menus' keys.
    game_keys: [i8; 9],
    menu_keys: [i8; 9],
    /// The stats screen's shake (1170:45ba..45c0): count, start count, amplitude.
    shake: (i32, i32, i32),
    /// gFrameCounter (1170:0f2c).
    frame: u32,
    quit: bool,
}

fn pcx_palette(path: &Path) -> Option<Pal6> {
    let b = std::fs::read(path).ok()?;
    if b.len() < 769 || b[b.len() - 769] != 12 {
        return None;
    }
    let mut p = [[0u8; 3]; 256];
    for (i, c) in p.iter_mut().enumerate() {
        for k in 0..3 {
            c[k] = b[b.len() - 768 + i * 3 + k] >> 2;
        }
    }
    Some(p)
}

fn load_picture(data: &Path, name: &str) -> Option<Picture> {
    let p = find_ci(&data.join("PICTURES"), name)?;
    Picture::load(&p).ok().filter(|p| p.width == 320 && p.height >= 200)
}

impl FrontEnd {
    /// FUN_1088_d60e: the menu palette (SLIDERBC.PCX's, kept at 1170:4710), the menu
    /// background (MENUBACK.LBM), the fonts and menu sprites, the menus (FUN_1080_3719).
    pub fn new(data: &Path) -> FrontEnd {
        let settings = Settings::load();
        let menu_back = load_picture(data, "MENUBACK.LBM");
        let palette = find_ci(&data.join("PICTURES"), "SLIDERBC.PCX")
            .and_then(|p| pcx_palette(&p))
            .or(menu_back.as_ref().map(|p| p.palette))
            .unwrap_or([[0; 3]; 256]);
        let mut gfx = Gfx::new(&data.join("SPRITES"), palette);
        if let Some(p) = &menu_back {
            gfx.back.copy_from_slice(&p.pixels[..(W * H) as usize]);
        }
        let main = MainScript::load(data, settings.language);
        crate::messages::LANGUAGE.store(settings.language, std::sync::atomic::Ordering::Relaxed);
        let mut texts = MenuTexts::load(data, settings.language);
        let menu_texts = texts.take(11);
        let mut fe = FrontEnd {
            data: data.to_path_buf(),
            settings,
            main,
            menu_texts,
            gfx,
            ui: Ui::new(),
            menus: HashMap::new(),
            cur: Id::Main,
            old: Id::Main,
            slide: 0,
            link_parent: true,
            in_game: false,
            level_state: 0,
            level: None,
            pending: None,
            back_is_snapshot: false,
            game_fb: vec![0; (W * H) as usize],
            user_maps: Vec::new(),
            shake: (0, 1, 0),
            game_keys: [-1; 9],
            menu_keys: [-1; 9],
            frame: 0,
            quit: false,
        };
        fe.build_menus(&mut texts);
        sound::listener(0xa0, 100);
        let m = fe.menus.get_mut(&Id::Main).unwrap();
        m.a = 0x8c;
        m.b = 0x2409;
        let ui = &fe.ui;
        m.select(m.default, ui);
        fe
    }

    fn table(&self, first: usize, i: i32) -> String {
        self.menu_texts.get(first + i.max(0) as usize).cloned().unwrap_or_default()
    }

    /// FUN_1080_3719.
    fn build_menus(&mut self, t: &mut MenuTexts) {
        let g = &mut self.gfx;
        let s = &self.settings;
        let mut main = Menu::new(Id::Main, 0x8b, 0xa0);
        main.spacing = 0xe;
        for _ in 0..4 {
            main.label(g, &t.next(), 6);
        }
        let mut options = Menu::new(Id::Options, 0x62, 0xa0);
        options.title = Some((0x2419, 0, 0x62 - 0x1e));
        options.spacing = 0x10;
        options.default = 3;
        for _ in 0..4 {
            options.label(g, &t.next(), 6);
        }
        let mut display = Menu::new(Id::Display, 0x3f, 0xa0);
        display.title = Some((0x241a, 0, 0x3f - 0x1e));
        display.slider(g, &t.next(), s.gamma, 0xf, 1, 2);
        display.slider(g, &t.next(), s.detail, 4, 1, 1);
        display.align(0, 1);
        display.label(g, &self.menu_texts[s.display_mode.clamp(0, 2) as usize], 6);
        let size = texts::SIZES.iter().position(|&w| w == s.window).unwrap_or(5) as i32;
        display.slider(g, &t.next(), size, 5, 1, 1);
        display.align(0, 3);
        display.label(g, &self.menu_texts[3 + s.frame_rate.clamp(0, 3) as usize], 6);
        display.label(g, &self.menu_texts[7 + s.wait_retrace.clamp(0, 1) as usize], 6);
        display.label(g, &t.next(), 6);
        display.label(g, &t.next(), 6);
        let mut snd = Menu::new(Id::Sound, 0x4a, 0xa0);
        snd.title = Some((0x241b, 0, 0x4a - 0x1e));
        snd.default = -1;
        snd.slider(g, &t.next(), s.volume, 0x100, 8, 0x20);
        snd.slider(g, &t.next(), s.voices - 2, 0xe, 1, 2);
        snd.align(0, 1);
        let buffer = if s.buffer_length < 0x200 { 0x400 } else { s.buffer_length };
        snd.slider(g, &t.next(), buffer - 0x200, 0xe00, 0x40, 0x100);
        snd.align(0, 2);
        snd.label(g, &self.menu_texts[9 + s.stereo.clamp(0, 1) as usize], 6);
        snd.slider(g, &t.next(), s.cd_volume, 0x100, 8, 0x20);
        snd.align(0, 4);
        snd.label(g, &t.next(), 6);
        snd.label(g, &t.next(), 6);
        // FUN_1080_18ee: the two questions, YEP then NOPE (global messages -4, -5).
        let yes = self.main.msg(-4);
        let no = self.main.msg(-5);
        let mut restart = Menu::new(Id::Restart, 0x78, 0xa0);
        restart.label(g, &yes, 6);
        restart.label(g, &no, 6);
        let mut quit = Menu::new(Id::Quit, 0x72, 0x4c);
        quit.label(g, &yes, 6);
        quit.label(g, &no, 6);
        let mut stats = Menu::new(Id::Stats, 0xa6, 0xa0);
        stats.label(g, &t.next(), 6);
        let mut art = Menu::new(Id::Artwork, 0x5a, 0xa0);
        art.label(g, &t.next(), 6);
        art.slider(g, &t.next(), 0, 0, 1, 1);
        art.label(g, &t.next(), 6);
        art.label(g, &t.next(), 6);
        let mut episode = Menu::new(Id::Episode, 0x6c, 0xa3);
        episode.centered = false;
        for _ in 0..3 {
            episode.label(g, &t.next(), 6);
        }
        episode.a = 0;
        let mut user = Menu::new(Id::UserMaps, 0x22, 0x14);
        user.title = Some((0x241c, 0x8c, 0xf));
        user.spacing = 0xc;
        user.centered = false;
        for m in [main, options, display, snd, restart, quit, stats, art, episode, user] {
            self.menus.insert(m.id, m);
        }
        self.scan_user_maps();
    }

    fn menu(&mut self, id: Id) -> &mut Menu {
        self.menus.get_mut(&id).unwrap()
    }

    fn save_dir() -> PathBuf {
        std::env::var_os("ET_SAVINGS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("SAVINGS"))
    }

    /// FUN_1030_04d3 + FUN_1080_31ea: every folder of USERMAPS (and the savings) with a
    /// script and its map; "Title=HIDDEN" ones are left out. One label per map.
    fn scan_user_maps(&mut self) {
        let mut dirs: Vec<PathBuf> = Vec::new();
        for root in [self.data.join("USERMAPS"), Self::save_dir()] {
            if let Ok(rd) = std::fs::read_dir(&root) {
                let mut v: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
                v.sort();
                dirs.extend(v);
            }
        }
        let lang = self.settings.language;
        self.user_maps.clear();
        for d in dirs {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            let mut scrs: Vec<PathBuf> = rd
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("scr")))
                .collect();
            scrs.sort();
            for scr in scrs {
                let stem = scr.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let Some(map) = find_ci(&d, &format!("{stem}.MAP")) else { continue };
                let pairs = script_pairs(&scr);
                let title = localized(&pairs, "Title", lang).unwrap_or_else(|| stem.clone());
                if title.eq_ignore_ascii_case("HIDDEN") {
                    continue;
                }
                let comments = localized(&pairs, "Comments", lang).unwrap_or_default();
                self.user_maps.push(UserMap { map, title, comments });
            }
        }
        let titles: Vec<String> = if self.user_maps.is_empty() {
            vec!["NO USERMAP FOUND...".to_string()]
        } else {
            self.user_maps.iter().map(|u| u.title.clone()).collect()
        };
        let g = &mut self.gfx;
        let m = self.menus.get_mut(&Id::UserMaps).unwrap();
        m.items.clear();
        for t in &titles {
            m.label(g, t, 5);
            let it = m.items.last_mut().unwrap();
            it.bx.2 = it.bx.2.min(0x116 - 0x14 - it.x);
        }
        m.scroll = 0;
        m.scroll_max = ((titles.len() as i32 - 0xb) * 0xc).max(0);
    }

    // ---- switching menus -------------------------------------------------------------

    /// FUN_1080_05fb.
    fn show(&mut self, id: Id) {
        if self.in_game {
            self.enter_menu_mode();
            let m = self.menu(id);
            m.parent = None;
            m.x = m.home_x;
        } else {
            if self.cur == id {
                return;
            }
            let old = self.cur;
            self.menu(old).sel = -1;
            if self.link_parent {
                self.menu(id).parent = Some(old);
            }
            self.old = old;
            self.slide = 0xb40;
            sound::volumes(0x80, 0x80);
            sound::pitch(0x55);
            sound::discard(0x85);
            sound::play(0x85);
        }
        self.cur = id;
        self.on_show(id);
        if self.ui.input.lmb < 1 && self.ui.input.rmb < 1 {
            let ui = &self.ui;
            let m = self.menus.get_mut(&id).unwrap();
            let d = if m.default < 0 { m.items.len() as i32 - 1 } else { m.default };
            m.select(d, ui);
        }
        self.ui.drag = (0x7fff, 0);
    }

    /// FUN_1080_02b3(1, 0): the game stops, its last frame (greyed through tint row 5) becomes
    /// the background, the music plays at half volume.
    fn enter_menu_mode(&mut self) {
        if !self.in_game {
            return;
        }
        self.in_game = false;
        if !self.back_is_snapshot {
            if let Some(r) = &self.level {
                let lp: [[u8; 3]; 256] = r.gfx.palette.map(|c| c.map(|v| v >> 2));
                let row = gfx::grey_row(&lp);
                for (b, &p) in self.gfx.back.iter_mut().zip(&self.game_fb) {
                    *b = row[p as usize];
                }
                self.back_is_snapshot = true;
            }
        }
        sound::listener(0xa0, 100);
        let m = self.menu(Id::Main);
        m.a = 0x8c;
        m.b = 0x2409;
    }

    /// FUN_1080_02b3(1, 1): back to the game (or to the stats if they are still open).
    fn resume(&mut self) {
        if self.menu(Id::Stats).b != 0x7fff {
            self.show(Id::Stats);
            return;
        }
        if self.level.is_none() {
            return;
        }
        self.in_game = true;
    }

    /// FUN_1080_1192: GET BACK.
    fn back(&mut self) {
        match self.menu(self.cur).parent {
            None => self.resume(),
            Some(p) => {
                self.link_parent = false;
                self.show(p);
                self.link_parent = true;
            }
        }
    }

    /// The menus' "on show" methods (VMT +0xc).
    fn on_show(&mut self, id: Id) {
        match id {
            Id::Display => self.display_texts(),
            Id::Artwork => {
                // FUN_1080_1f4d: the juke box goes up to the last CD track title.
                let n = self.main.track_titles.len() as i32 - 1;
                if let Kind::Slider { max, .. } = &mut self.menu(Id::Artwork).items[1].kind {
                    *max = n.max(0);
                }
            }
            Id::Stats => {
                // FUN_1080_1a6a: the counters start after 40 frames, lower for each one missing.
                let totals = self.level.as_ref().map(|r| r.totals).unwrap_or_default();
                let m = self.menu(Id::Stats);
                if m.b == 0x7fff {
                    m.b = -40;
                    m.a = 0x4c + 10 * [totals.bios, totals.secrets, totals.kills].iter().filter(|&&t| t == 0).count() as i32;
                }
            }
            _ => {}
        }
    }

    /// FUN_1080_264b.
    fn display_texts(&mut self) {
        let s = self.settings.clone();
        let mode = self.table(0, s.display_mode);
        let tempo = self.table(3, s.frame_rate);
        let sync = self.table(7, s.wait_retrace);
        let g = &mut self.gfx;
        let m = self.menus.get_mut(&Id::Display).unwrap();
        m.items[0].set_value(s.gamma);
        m.retext(g, 2, &mode);
        m.retext(g, 4, &tempo);
        m.retext(g, 5, &sync);
    }

    // ---- levels ------------------------------------------------------------------------

    /// FUN_1080_17fa: the map is remembered; with a level running, ask first.
    fn start_map(&mut self, map: PathBuf) {
        self.pending = Some(map.clone());
        if self.level_state >= 1 {
            self.show(Id::Restart);
            return;
        }
        if let Err(e) = self.load_level(&map) {
            eprintln!("can't start {}: {e}", map.display());
            self.level_state = 0;
            self.show(Id::Main);
        }
    }

    /// FUN_1088_d3e7: the level stops.
    fn stop_level(&mut self) {
        sound::stop_all();
        self.level = None;
        self.level_state = 0;
    }

    /// FUN_1088_bc00 + FUN_1080_16b7: load the map, then the level starts (RandSeed 0x3e00,
    /// HeroReset: `Game::new`), its music plays (FUN_1080_167b).
    fn load_level(&mut self, map: &Path) -> Result<(), Box<dyn Error>> {
        sound::stop_all();
        let world = crate::load_world(&self.data, map)?;
        let mut gfx = Graphics::load(&self.data, &world, map)?;
        gfx.show_hud = true;
        gfx.hud_level = self.settings.game_stats;
        crate::messages::LANGUAGE.store(self.settings.language, std::sync::atomic::Ordering::Relaxed);
        // A user map with PlayRecord= plays its recording (LevelLoad -> DemoLoad).
        let pairs = script_pairs(&script_of(map));
        let demo = pairs
            .iter()
            .rfind(|(k, _)| k.eq_ignore_ascii_case("PlayRecord"))
            .and_then(|(_, v)| find_ci(map.parent()?, v))
            .and_then(|p| crate::demo::Demo::load(&p).ok());
        let mut totals = level_totals(&world);
        level::tally(|t| *t = Tally::default());
        let game = Game::new(world, demo);
        totals.kills += game.level.objects.iter().flatten().filter(|o| COUNTED_KINDS.contains(&o.kind)).count() as i32;
        crate::level_music(map);
        self.level = Some(Running { game, gfx, map: map.to_path_buf(), totals });
        self.menu(Id::Stats).b = 0x7fff;
        self.level_state = 2;
        self.in_game = true;
        self.back_is_snapshot = false;
        Ok(())
    }

    /// GameFrame 4e60 = 100 -> FUN_1080_172a: the stats screen over the script's EndPic,
    /// CD track 6, and the next map goes to the savings (FUN_1080_2fc4).
    fn level_over(&mut self) {
        let Some(r) = &self.level else { return };
        let pairs = script_pairs(&script_of(&r.map));
        let pic = pairs.iter().rfind(|(k, _)| k.eq_ignore_ascii_case("EndPic")).map(|(_, v)| v.clone());
        let next = crate::next_level(&self.data, &r.map);
        self.show(Id::Stats);
        let pic = pic.and_then(|p| load_picture(&self.data, &p)).or_else(|| load_picture(&self.data, "STATSB01.LBM"));
        if let Some(p) = pic {
            self.gfx.back.copy_from_slice(&p.pixels[..(W * H) as usize]);
        }
        self.back_is_snapshot = false;
        sound::stop_all();
        sound::music(6, false);
        self.save_next(next);
    }

    /// FUN_1080_2fc4: a copy of the next level's map and script in SAVINGS/<NAME>/, so START
    /// USER MAP can start there (only maps of MAPS/ are saved).
    fn save_next(&mut self, next: Option<PathBuf>) {
        let Some(next) = next else { return };
        let maps = self.data.join("MAPS");
        if next.parent().map(|p| p.canonicalize().ok()) != Some(maps.canonicalize().ok()) {
            return;
        }
        let stem = next.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let dir = Self::save_dir().join(&stem);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let _ = std::fs::copy(&next, dir.join(format!("{stem}.MAP")));
        let scr = script_of(&next);
        if scr.exists() {
            let _ = std::fs::copy(&scr, dir.join(format!("{stem}.SCR")));
        }
        self.scan_user_maps();
    }

    /// FUN_1080_1ab2's CONTINUE: the next map, the episode screen or the main menu.
    fn stats_continue(&mut self) {
        let next_name = self
            .level
            .as_ref()
            .and_then(|r| script_pairs(&script_of(&r.map)).into_iter().rfind(|(k, _)| k.eq_ignore_ascii_case("NextLevelMap")))
            .map(|(_, v)| v);
        let next = self.level.as_ref().and_then(|r| crate::next_level(&self.data, &r.map));
        self.stop_level();
        self.level_state = -1;
        self.menu(Id::Stats).b = 0x7fff;
        match (next_name, next) {
            (Some(n), _) if n.eq_ignore_ascii_case("NEXT EPISODE") => {
                self.show(Id::Episode);
                self.level_state = 0;
            }
            (_, Some(map)) => self.start_map(map),
            _ => {
                self.show(Id::Main);
                self.level_state = 0;
            }
        }
    }

    // ---- one frame -----------------------------------------------------------------------

    /// GameFrameMenu (1080:2c70).
    fn menu_frame(&mut self) {
        self.ui.drag_next = 0x7fff;
        self.gfx.clip = gfx::SCREEN;
        self.gfx.shimmer();
        self.ui.cursor = 0x241e;
        let cur = self.cur;
        let ui = &self.ui;
        if let Some((x, y)) = self.menus.get_mut(&cur).unwrap().glide_pointer(ui) {
            self.ui.input.mx = x;
            self.ui.input.my = y;
        }
        if self.slide > 0 {
            self.slide -= 0x60;
            let a = crate::trig::sin(self.slide + 0x5a0) * 0x5a0 / 0x4000;
            let f = crate::trig::sin(0xb40 - a) * 0xa0 / 0x4000;
            let m = self.menu(cur);
            m.x = m.home_x + (0xa0 - f);
            let old = self.old;
            let o = self.menu(old);
            o.x = o.home_x + 0x140 - (0xa0 - f);
            self.draw_menu(old);
            self.ui.cursor = 0x2420;
        } else {
            self.update_menu(cur);
        }
        if !self.in_game && !self.quit {
            let cur = self.cur;
            self.draw_menu(cur);
            let (mx, my, c) = (self.ui.input.mx, self.ui.input.my, self.ui.cursor);
            self.gfx.sprite(c, mx, my, Mode::Copy);
        }
        self.ui.drag.0 = self.ui.drag_next;
        if self.ui.repeat > 0 {
            self.ui.repeat -= 1;
        }
    }

    /// The menus' updates (VMT +0x10).
    fn update_menu(&mut self, id: Id) {
        let ui = &mut self.ui;
        let g = &mut self.gfx;
        let m = self.menus.get_mut(&id).unwrap();
        if id == Id::Stats {
            // FUN_1080_1ab2: nothing to choose until the clock reaches 0.
            ui.cursor = 0x2422;
            if m.b < 0x4000 {
                m.b += 1;
            }
            if m.b < 0 {
                return;
            }
        }
        if id == Id::Main && m.a > 0 {
            // FUN_1080_2270: the logo's EAT changes after 140 frames, with a bang.
            m.a -= 1;
            if m.a == 0 {
                m.b += 1;
                sound::volumes(0x80, 0x80);
                sound::pitch(0xaa);
                sound::play(1);
            }
        }
        if id == Id::UserMaps {
            m.y = 0x22 - m.scroll;
            g.clip = (0, 0x1b, W - 1, 0xa1);
        }
        let title_back = m.update_common(g, ui);
        g.clip = gfx::SCREEN;
        let sel = m.sel;
        let count = m.items.len() as i32;
        let act = ui.act;
        if matches!(id, Id::Stats | Id::Sound | Id::Artwork | Id::Episode | Id::Restart | Id::Quit) {
            ui.cursor = 0x2422;
        }
        if title_back {
            self.back();
            return;
        }
        match id {
            Id::Main if act => match sel {
                0 => self.show(Id::Episode),
                1 => self.show(Id::UserMaps),
                2 => self.show(Id::Options),
                3 => self.show(Id::Quit),
                _ => {}
            },
            Id::Options if act => match sel {
                0 => self.show(Id::Display),
                1 => self.show(Id::Sound),
                2 => self.show(Id::Artwork),
                _ if sel == count - 1 => self.back(),
                _ => {}
            },
            Id::Display => self.update_display(sel, act),
            Id::Sound => self.update_sound(sel, act),
            Id::Artwork => {
                // FUN_1080_1f8f: the juke box plays CD track value + 2.
                let it = &self.menu(Id::Artwork).items[1];
                if sel == 1 && (it.changed() || act) {
                    sound::music(it.value() as u32 + 2, false);
                }
                if act && sel == count - 1 {
                    self.back();
                }
                // Items 0 and 2 play INTRO.AVI / CREDITS.AVI: videos are not decoded.
            }
            Id::Episode if act && sel >= 0 => {
                if let Some(p) = find_ci(&self.data.join("MAPS"), EPISODE_MAPS[sel as usize % 3]) {
                    self.start_map(p);
                }
            }
            Id::Restart if act => {
                // FUN_1080_1963 / 18bf: YEP ends the game and starts the remembered map.
                if sel == 0 {
                    self.stop_level();
                    if let Some(p) = self.pending.clone() {
                        self.start_map(p);
                    }
                } else if sel == 1 {
                    self.back();
                }
            }
            Id::Quit if act => {
                if sel == 0 {
                    self.quit = true;
                } else if sel == 1 {
                    self.back();
                }
            }
            Id::Stats if act => self.stats_continue(),
            Id::UserMaps => self.update_user_maps(sel, act),
            _ => {}
        }
    }

    /// FUN_1080_270b.
    fn update_display(&mut self, sel: i32, act: bool) {
        let m = self.menu(Id::Display);
        let (gamma, detail, size) = (m.items[0].value(), m.items[1].value(), m.items[3].value());
        let s = &mut self.settings;
        if gamma != s.gamma {
            s.gamma = gamma;
        }
        s.detail = detail;
        let wh = texts::SIZES[size.clamp(0, 4) as usize];
        if wh != s.window {
            s.window = wh;
        }
        if !act {
            return;
        }
        match sel {
            2 => {
                s.display_mode = (s.display_mode + 1) % 3;
            }
            4 => s.frame_rate = (s.frame_rate + 1) % 4,
            5 => s.wait_retrace ^= 1,
            6 => {
                s.detail = 3;
                s.frame_rate = 0;
                s.wait_retrace = 1;
                self.menu(Id::Display).items[1].set_value(3);
            }
            7 => self.back(),
            _ => {}
        }
        if matches!(sel, 2 | 4 | 5 | 6) {
            self.display_texts();
        }
    }

    /// FUN_1080_2486.
    fn update_sound(&mut self, sel: i32, act: bool) {
        let m = self.menu(Id::Sound);
        let vals: Vec<i32> = m.items.iter().map(|i| i.value()).collect();
        let s = &mut self.settings;
        s.volume = vals[0];
        s.voices = vals[1] + 2;
        s.buffer_length = vals[2] + 0x200;
        if vals[4] != s.cd_volume {
            s.cd_volume = vals[4];
        }
        if act {
            match sel {
                3 => {
                    s.stereo = (s.stereo == 0) as i32;
                    let t = self.table(9, self.settings.stereo);
                    let g = &mut self.gfx;
                    self.menus.get_mut(&Id::Sound).unwrap().retext(g, 3, &t);
                    menu::confirm_sound(&self.ui);
                }
                5 => {
                    s.voices = 8;
                    s.stereo = 1;
                    s.buffer_length = 0x400;
                    let t = self.table(9, 1);
                    let g = &mut self.gfx;
                    let m = self.menus.get_mut(&Id::Sound).unwrap();
                    m.items[1].set_value(6);
                    m.items[2].set_value(0x200);
                    m.retext(g, 3, &t);
                    menu::confirm_sound(&self.ui);
                }
                6 => {
                    self.back();
                    return;
                }
                _ => {}
            }
        }
        // The test sound loops at the pointer while this menu is open.
        let (mx, my) = (self.ui.input.mx, self.ui.input.my);
        if sound::audible(my, mx) {
            sound::priority(0x11f4);
            sound::keep(SOUND_TEST, 0x2c);
        }
    }

    /// FUN_1080_2a95: the list scrolls by whole items (12 px), PgUp/PgDn by ten.
    fn update_user_maps(&mut self, sel: i32, act: bool) {
        let i = self.ui.input;
        let m = self.menus.get_mut(&Id::UserMaps).unwrap();
        if sel >= 0 {
            let y = sel * 0xc;
            if y < m.scroll {
                m.scroll = y;
            } else if m.scroll + 0x78 < y {
                m.scroll = y - 0x78;
            }
        }
        if self.ui.repeat(8, i.pgup as i32) > 0 {
            m.scroll -= 0x78;
        } else if i.my < 1 || (i.lmb > 0 && i.mx >= 0x10c && (0x7d..=0x8e).contains(&i.my)) {
            m.scroll_speed = -2;
        }
        if self.ui.repeat(8, i.pgdn as i32) > 0 {
            m.scroll += 0x78;
        } else if i.my > 0xc6 || (i.lmb > 0 && i.mx >= 0x10c && (0x8f..=0x9f).contains(&i.my)) {
            m.scroll_speed = 2;
        }
        m.scroll += m.scroll_speed;
        if m.scroll % 0xc == 0 {
            m.scroll_speed = 0;
        }
        m.scroll = m.scroll.clamp(0, m.scroll_max);
        if act && sel >= 0 {
            if let Some(u) = self.user_maps.get(sel as usize) {
                let p = u.map.clone();
                self.start_map(p);
            }
        }
    }

    // ---- drawing -------------------------------------------------------------------------

    /// The menus' draws (VMT +0x14).
    fn draw_menu(&mut self, id: Id) {
        let x = self.menu(id).x;
        let y = self.menu(id).y;
        match id {
            Id::Main => {
                self.draw_items(id);
                let b = self.menu(id).b;
                for s in [0x2407, 0x2408, b] {
                    self.gfx.sprite(s, x, 0, Mode::Copy);
                }
            }
            Id::Restart => {
                self.draw_items(id);
                let t = self.main.msg(-7);
                self.gfx.text_centered(&MENU_FONT, t.as_bytes(), x, y - 0x20, Mode::Copy);
            }
            Id::Quit => {
                self.draw_items(id);
                let t = self.main.msg(-6);
                self.gfx.text_centered(&MENU_FONT, t.as_bytes(), x, y - 0x16, Mode::Copy);
                self.gfx.sprite(0x2a00, x - 0x4c, 0, Mode::Copy);
            }
            Id::Episode => {
                self.gfx.shade(7, x - 0xa3, 0, x + 0x9c, 0x20);
                self.gfx.shade(7, x - 0xa3, 0xa7, x + 0x9c, 199);
                let m = self.menu(id);
                if m.sel >= 0 {
                    m.a = m.sel;
                }
                let (a, sp) = (m.a, m.spacing);
                self.gfx.sprite(0x2900 + a, x - 1, y - 0x36, Mode::Copy);
                let t = self.main.msg(-0x24);
                self.gfx.text(&MENU_FONT, t.as_bytes(), x, y - sp, Mode::Copy);
                self.draw_items(id);
            }
            Id::Artwork => {
                self.draw_items(id);
                let m = self.menu(id);
                if m.sel == 1 {
                    let v = m.items[1].value();
                    let t = self.main.track_titles.get(v as usize).cloned().unwrap_or_default();
                    self.gfx.text_centered(&MENU_FONT, t.as_bytes(), x, 0xb8, Mode::Tint(6));
                }
            }
            Id::Stats => self.draw_stats(),
            Id::UserMaps => self.draw_user_maps(),
            _ => self.draw_items(id),
        }
    }

    fn draw_items(&mut self, id: Id) {
        let g = &mut self.gfx;
        self.menus.get_mut(&id).unwrap().draw_items(g);
    }

    /// FUN_1080_2927.
    fn draw_user_maps(&mut self) {
        let m = self.menu(Id::UserMaps);
        let (x, sel) = (m.x, m.sel);
        m.y = 0x22 - m.scroll;
        self.gfx.clip = (0, 0x1b, W - 1, 0xa1);
        self.draw_items(Id::UserMaps);
        self.gfx.clip = gfx::SCREEN;
        self.gfx.sprite(0x240c, 0x116, 0x87, Mode::Copy);
        self.gfx.sprite(0x240e, 0x116, 0x97, Mode::Copy);
        self.gfx.shade(7, x - 0x14, 0xa3, x + 299, 199);
        if let Some(u) = (sel >= 0).then(|| self.user_maps.get(sel as usize)).flatten() {
            let c = u.comments.clone();
            self.gfx.text(&MENU_FONT, c.as_bytes(), 10, 0xae, Mode::Tint(6));
        }
    }

    /// FUN_1080_1ca3: the level's end message, then one counter every 20 frames with a bang
    /// and a shake: bios spheres, hidden areas, kills (those the level has), the time.
    fn draw_stats(&mut self) {
        let (x, b, a) = {
            let m = self.menu(Id::Stats);
            (m.x, m.b, m.a)
        };
        if b < 0 {
            return;
        }
        self.draw_items(Id::Stats);
        // FUN_1050_0017: the shake's offset.
        let (count, init, amp) = self.shake;
        let off = if count < 1 {
            0
        } else {
            let r = 3 - ((count % 6) - 3).abs();
            self.shake.0 -= 1;
            ((3 - r) * amp * count / init.max(1)) / 3
        };
        self.gfx.shade(7, x - 0x78, 0, x + 0x78, 199);
        let (end_msg, totals) = match &self.level {
            Some(r) => {
                let pairs = script_pairs(&script_of(&r.map));
                (localized(&pairs, "EndMessage", self.settings.language).unwrap_or_default(), r.totals)
            }
            None => (String::new(), Tally::default()),
        };
        self.gfx.text_centered(&MENU_FONT, end_msg.as_bytes(), x, off + 0x1c, Mode::Tint(6));
        let got = level::TALLY.with(|t| t.get());
        let f = self.frame as i32;
        let mut y = a;
        let mut thr = 0x28;
        let counters = [
            (0x2824 + ((f / 3) & 7), self.main.msg(-8), got.bios, totals.bios),
            (0x280c + (f / 9) % 12, self.main.msg(-9), got.secrets, totals.secrets),
            (0x2800 + (f / 9) % 12, self.main.msg(-10), got.kills, totals.kills),
        ];
        let mut shown_all = true;
        for (icon, label, value, total) in counters {
            if b == thr {
                self.bang(y);
            }
            if b >= thr && total > 0 {
                self.gfx.text(&MENU_FONT, label.as_bytes(), x - 0x4b, y, Mode::Copy);
                menu::draw_count(&mut self.gfx, value, total, x + 0x5c, y);
                self.gfx.sprite(icon, x - 0x50, y - 1 - (icon < 0x2824) as i32, Mode::Copy);
                y += 0x14;
                thr += 0x14;
            } else if b < thr {
                shown_all = false;
            }
        }
        if shown_all && b >= thr {
            if b == thr {
                self.bang(y);
            }
            let secs = self.level.as_ref().map(|r| r.game.frame / crate::HZ).unwrap_or(0) as i32;
            let (h, mm, ss) = (secs / 3600, secs / 60 % 60, secs % 60);
            let t = if h > 0 { format!("{h}:{mm:02}:{ss:02}") } else { format!("{mm:02}:{ss:02}") };
            let label = self.main.msg(-11);
            self.gfx.text(&MENU_FONT, label.as_bytes(), x - 0x4b, y, Mode::Copy);
            self.gfx.text_right(&gfx::DIGIT_FONT, t.as_bytes(), x + 0x5c, y, Mode::Copy);
            self.gfx.sprite(0x2818 + (f / 9) % 12, x - 0x50, y - 2, Mode::Copy);
        }
    }

    /// FUN_1080_1bab: sound 0 at a random pitch, ScreenShake(0xe, 6).
    fn bang(&mut self, y: i32) {
        let p = self.gfx.rng.random(10);
        sound::pitch(0x100 - p);
        sound::play_at(0, y, 0xa0);
        self.shake = (0xe, 0xe, 6);
    }
}

/// The map's script beside it.
fn script_of(map: &Path) -> PathBuf {
    let stem = map.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let dir = map.parent().unwrap_or(Path::new("."));
    find_ci(dir, &format!("{stem}.SCR")).unwrap_or_else(|| dir.join(format!("{stem}.SCR")))
}

/// SpawnFromIconTiles' totals (FUN_1088_1797): plane-2 cells 0xd00 (bios spheres), function
/// 61800 (hidden areas), 61900 (enemies). The constructors that also add to the enemy total
/// (EnemyFactory during LevelInit) are counted from the objects the setup made.
pub fn level_totals(world: &crate::world::World) -> Tally {
    let mut t = Tally::default();
    if let Some(Some(p)) = world.planes.get(2) {
        let main = world.main();
        for (i, &c) in p.cells.iter().enumerate() {
            if c == 0 {
                continue;
            }
            let f = world.cell_function(c, 0);
            // FUN_1088_0d5a: an alien icon (61400, 61300) over a wall hole (main-plane function
            // 7050) makes no object yet but counts as an enemy.
            if (f == 61400 || f == 61300) && main.cells.get(i).is_some_and(|&m| m > 0 && world.cell_function(m, 0) == 7050) {
                t.kills += 1;
            }
            if f < 61000 {
                t.bios += (c == 0xd00) as i32;
            } else if f == 61800 {
                t.secrets += 1;
            } else if f == 61900 {
                t.kills += 1;
            }
        }
    }
    t
}

/// The enemy kinds whose constructors count themselves (1170:4eb2) during LevelInit.
pub const COUNTED_KINDS: [u16; 10] = [0x1428, 0x1432, 0x145a, 0x14d2, 0x13e2, 0x13ec, 0x13f6, 0x140a, 0x146e, 0x1482];

// ---- the window ------------------------------------------------------------------------------

/// Windows virtual-key codes (EatThis.ini's keys) as SDL scancodes.
fn vk_scancode(vk: i32) -> Option<sdl2::keyboard::Scancode> {
    use sdl2::keyboard::Scancode as S;
    Some(match vk {
        0x08 => S::Backspace,
        0x09 => S::Tab,
        0x0d => S::Return,
        0x10 => S::LShift,
        0x11 => S::LCtrl,
        0x12 => S::LAlt,
        0x20 => S::Space,
        0x21 => S::PageUp,
        0x22 => S::PageDown,
        0x23 => S::End,
        0x24 => S::Home,
        0x25 => S::Left,
        0x26 => S::Up,
        0x27 => S::Right,
        0x28 => S::Down,
        0x2d => S::Insert,
        0x2e => S::Delete,
        0x30..=0x39 | 0x41..=0x5a => S::from_name(&((vk as u8) as char).to_string())?,
        0x60 => S::Kp0,
        0x61..=0x69 => S::from_name(&format!("Keypad {}", vk - 0x60))?,
        0x70..=0x7b => S::from_name(&format!("F{}", vk - 0x6f))?,
        _ => return None,
    })
}

fn rgb_with_gamma(pal6: &Pal6, gamma: i32) -> [[u8; 3]; 256] {
    // VideoSetPalette (1008): (c + gamma) * 4, at most 255; 0 black and 255 white are Windows'.
    let mut out = [[0u8; 3]; 256];
    for (i, o) in out.iter_mut().enumerate() {
        *o = match i {
            0 => [0, 0, 0],
            255 => [255, 255, 255],
            _ => pal6[i].map(|c| ((c as i32 + gamma) * 4).min(255) as u8),
        };
    }
    out
}

/// `eatthis` without --map/--demo: the original's menus in a window.
pub fn run(data: &Path, scale: u32, mute: bool) -> Result<(), Box<dyn Error>> {
    use sdl2::{event::Event, keyboard::Scancode, mouse::MouseButton, pixels::PixelFormatEnum};
    if let Ok(script) = std::env::var("ET_FE_SHOT") {
        return shot(data, &script);
    }
    let sdl = sdl2::init()?;
    struct Out(sound::Mixer);
    impl sdl2::audio::AudioCallback for Out {
        type Channel = f32;
        fn callback(&mut self, out: &mut [f32]) {
            self.0.render(out);
        }
    }
    let mut audio = if mute {
        None
    } else {
        let want = sdl2::audio::AudioSpecDesired { freq: Some(sound::RATE as i32), channels: Some(2), samples: Some(1024) };
        match sdl.audio().and_then(|a| a.open_playback(None, &want, |_| Out(sound::Mixer::new(data)))) {
            Ok(dev) => {
                sound::enable();
                dev.resume();
                Some(dev)
            }
            Err(e) => {
                eprintln!("no sound: {e}");
                None
            }
        }
    };
    let (sfx_gain, music_gain) = audio
        .as_mut()
        .map(|d| {
            let m = d.lock();
            (m.0.sfx_gain, m.0.music_gain)
        })
        .unwrap_or((1.0, 0.45));
    let mut fe = FrontEnd::new(data);
    let (ww, wh) = fe.settings.window;
    let s = scale.max(1);
    let window = sdl
        .video()?
        .window("Eat This", (ww as u32).max(W as u32 * s / 2), (wh as u32).max(H as u32 * s / 2))
        .position_centered()
        .resizable()
        .build()?;
    let mut canvas = window.into_canvas().present_vsync().build()?;
    canvas.set_logical_size(W as u32, H as u32)?;
    let tc = canvas.texture_creator();
    let mut tex = tc.create_texture_streaming(PixelFormatEnum::RGB24, W as u32, H as u32)?;
    let mut tex2 = tc.create_texture_streaming(PixelFormatEnum::RGB24, W as u32, 2 * H as u32)?;
    sdl.mouse().show_cursor(false);
    let mut events = sdl.event_pump()?;
    let mut rgb = vec![0u8; (W * H * 3) as usize];
    let mut rgb2 = vec![0u8; (W * H * 6) as usize];
    let (mut lmb, mut rmb) = (false, false);
    let tick = std::time::Duration::from_secs(1) / crate::HZ;
    let mut next = std::time::Instant::now();
    let mut ticks = 0u32;
    let mut applied_gains = (-1.0f32, -1.0f32);
    let mut applied_window: Option<(i32, (i32, i32))> = None;
    'main: loop {
        for e in events.poll_iter() {
            match e {
                Event::Quit { .. } => break 'main,
                Event::KeyDown { scancode: Some(sc), repeat: false, .. } => fe.key_down(sc),
                Event::MouseMotion { x, y, .. } => {
                    fe.ui.input.mx = x.clamp(0, W - 1);
                    fe.ui.input.my = y.clamp(0, H - 1);
                }
                Event::MouseButtonDown { mouse_btn, .. } => match mouse_btn {
                    MouseButton::Left => lmb = true,
                    MouseButton::Right => rmb = true,
                    _ => {}
                },
                Event::MouseButtonUp { mouse_btn, .. } => match mouse_btn {
                    MouseButton::Left => lmb = false,
                    MouseButton::Right => rmb = false,
                    _ => {}
                },
                _ => {}
            }
        }
        while std::time::Instant::now() >= next {
            next += tick;
            ticks += 1;
            let ks = events.keyboard_state();
            let mut evs = Vec::new();
            fe.tick(&|sc: Scancode| ks.is_scancode_pressed(sc), lmb, rmb, &mut evs);
            if let Some(dev) = audio.as_mut() {
                let mut m = dev.lock();
                for e in evs {
                    m.0.apply(e);
                }
            }
            if fe.quit {
                break 'main;
            }
            let s = &fe.settings;
            // MixerSetVolume: half the CD volume in the menus, all of it in a level or the stats.
            let half = if fe.in_game || fe.cur == Id::Stats { 1.0 } else { 0.5 };
            let gains = (sfx_gain * s.volume as f32 / 256.0, music_gain * s.cd_volume as f32 / 256.0 * half);
            if gains != applied_gains {
                applied_gains = gains;
                if let Some(dev) = audio.as_mut() {
                    let mut m = dev.lock();
                    (m.0.sfx_gain, m.0.music_gain) = gains;
                }
            }
            let want = (s.display_mode, s.window);
            if applied_window != Some(want) {
                applied_window = Some(want);
                let win = canvas.window_mut();
                let full = s.display_mode == 2;
                let _ = win.set_fullscreen(if full { sdl2::video::FullscreenType::Desktop } else { sdl2::video::FullscreenType::Off });
                if !full {
                    let _ = win.set_size(s.window.0.max(W) as u32, s.window.1.max(H) as u32);
                }
            }
        }
        // FrameRate: auto/70 draw every frame, 35 every second, 23 every third.
        let every = match fe.settings.frame_rate {
            2 => 2,
            3 => 3,
            _ => 1,
        };
        if ticks % every != 0 {
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        }
        fe.render(&mut rgb);
        if fe.settings.display_mode == 1 {
            // Scanlines: every other line of a double-height picture is black.
            let row = W as usize * 3;
            for y in 0..H as usize {
                rgb2[2 * y * row..(2 * y + 1) * row].copy_from_slice(&rgb[y * row..(y + 1) * row]);
                rgb2[(2 * y + 1) * row..(2 * y + 2) * row].fill(0);
            }
            tex2.update(None, &rgb2, W as usize * 3)?;
            canvas.copy(&tex2, None, None)?;
        } else {
            tex.update(None, &rgb, W as usize * 3)?;
            canvas.copy(&tex, None, None)?;
        }
        canvas.present();
    }
    fe.settings.save();
    Ok(())
}

/// Debug: `ET_FE_SHOT="steps"` runs the front end without a window and writes PPM pictures.
/// Steps (separated by ';'): `N [key...]` holds the keys (SDL scancode names, e.g. Down,
/// Return, Escape) for N frames, `move X Y`, `click X Y` (one frame down, then up), `shot FILE`, `levelend` (the
/// level in progress ends on its next frame).
fn shot(data: &Path, script: &str) -> Result<(), Box<dyn Error>> {
    use sdl2::keyboard::Scancode;
    let mut fe = FrontEnd::new(data);
    let mut rgb = vec![0u8; (W * H * 3) as usize];
    let mut evs = Vec::new();
    for step in script.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let words: Vec<&str> = step.split_whitespace().collect();
        match words[0] {
            "move" | "click" => {
                fe.ui.input.mx = words.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                fe.ui.input.my = words.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);
                if words[0] == "click" {
                    fe.tick(&|_| false, true, false, &mut evs);
                    fe.tick(&|_| false, false, false, &mut evs);
                }
            }
            "levelend" => {
                if let Some(r) = fe.level.as_mut() {
                    r.game.level_over = true;
                }
            }
            "shot" => {
                fe.render(&mut rgb);
                let mut out = format!("P6\n{W} {H}\n255\n").into_bytes();
                out.extend(&rgb);
                std::fs::write(words.get(1).copied().unwrap_or("fe.ppm"), out)?;
            }
            n => {
                let n: u32 = n.parse().map_err(|_| format!("ET_FE_SHOT: bad step {step}"))?;
                let keys: Vec<Scancode> = words[1..].iter().filter_map(|k| Scancode::from_name(k)).collect();
                for k in &keys {
                    if matches!(k, Scancode::Escape | Scancode::F3 | Scancode::F4 | Scancode::F5 | Scancode::F6 | Scancode::F7) || crate::weapon_key(*k).is_some() {
                        fe.key_down(*k);
                    }
                }
                for _ in 0..n {
                    fe.tick(&|sc| keys.contains(&sc), false, false, &mut evs);
                    evs.clear();
                }
            }
        }
    }
    Ok(())
}

impl FrontEnd {
    /// A key going down (FUN_1008_1966): Escape, the F keys, numpad +/-.
    fn key_down(&mut self, sc: sdl2::keyboard::Scancode) {
        use sdl2::keyboard::Scancode;
        match sc {
            Scancode::Escape => {
                // The main menu, or back to the level the menu is over.
                if self.level_state == 0 || self.in_game || self.cur == Id::Stats {
                    self.link_parent = true;
                    self.show(Id::Main);
                } else {
                    self.resume();
                }
            }
            Scancode::Num1 | Scancode::Num2 | Scancode::Num3 | Scancode::Num4 | Scancode::Num5 | Scancode::Num6
            | Scancode::Num7 | Scancode::Num8 | Scancode::Num9 | Scancode::Num0 | Scancode::PageUp | Scancode::PageDown
                if self.in_game =>
            {
                if let Some(r) = self.level.as_mut() {
                    r.game.weapon_key = crate::weapon_key(sc);
                }
            }
            Scancode::F3 => self.hotkey(Id::UserMaps),
            Scancode::F5 => self.hotkey(Id::Options),
            Scancode::F6 => self.hotkey(Id::Display),
            Scancode::F7 => self.hotkey(Id::Sound),
            Scancode::F4 => {
                self.settings.display_mode = (self.settings.display_mode + 1) % 3;
                self.display_texts();
            }
            Scancode::F11 => {
                self.settings.gamma = (self.settings.gamma + 1) % 0x10;
                self.display_texts();
            }
            Scancode::KpPlus => self.settings.game_stats = (self.settings.game_stats + 1).min(2),
            Scancode::KpMinus => self.settings.game_stats = (self.settings.game_stats - 1).max(0),
            _ => {}
        }
    }

    /// One 70 Hz frame (GameFrame): the level or the menus; the sound events go to `evs`.
    fn tick(&mut self, pressed: &dyn Fn(sdl2::keyboard::Scancode) -> bool, lmb: bool, rmb: bool, evs: &mut Vec<sound::Ev>) {
        use sdl2::keyboard::Scancode;
        self.frame += 1;
        // The mouse buttons as GameFrame counts them: 1 the frame they go down, then 2.
        let i = &mut self.ui.input;
        i.lmb = if lmb { if i.lmb >= 1 { 2 } else { 1 } } else { 0 };
        i.rmb = if rmb { if i.rmb >= 1 { 2 } else { 1 } } else { 0 };
        if self.in_game {
            let mut keys = [0i8; 9];
            let k = self.settings.keys;
            let vks = [k[0], k[1], k[2], k[3], k[4], k[5], k[4], k[5], k[6]];
            for (n, vk) in vks.iter().enumerate() {
                let down = vk_scancode(*vk).is_some_and(|sc| pressed(sc));
                keys[n] = crate::key_poll(&mut self.game_keys[n], down);
            }
            // ReadGameInput: the mouse buttons fire when the shoot keys are up; the cursor
            // (x * 319 / width, FUN_1088_42a3) aims.
            let i = &self.ui.input;
            if keys[4] < 1 {
                keys[4] = i.lmb as i8;
            }
            if keys[5] < 1 {
                keys[5] = i.rmb as i8;
            }
            let mouse = (i.mx * 319 / W, i.my * 199 / H);
            let r = self.level.as_mut().unwrap();
            r.game.live_mouse = Some(mouse);
            if !r.game.demo_done() {
                r.game.step(keys);
            }
            evs.extend(r.game.sound.drain(..));
            if std::mem::take(&mut r.game.restart) {
                // GameFrame 4e60 = 10: HeroReset, the level again (LevelInit randomizes).
                let tries = r.game.level.tries + 1;
                let map = r.map.clone();
                if self.load_level(&map).is_ok() {
                    self.level.as_mut().unwrap().game.level.tries = tries;
                }
            } else if std::mem::take(&mut r.game.level_over) {
                self.level_over();
            } else if r.game.demo_done() {
                // A demo that ends goes back to the menu (GameFrame 4e60 = 0x1e).
                self.stop_level();
                self.show(Id::Main);
            }
            if let Some(r) = self.level.as_mut() {
                r.gfx.hud_level = self.settings.game_stats;
            }
        } else {
            let sets: [&[Scancode]; 9] = [
                &[Scancode::Up, Scancode::Kp8],
                &[Scancode::Down, Scancode::Kp2],
                &[Scancode::Left, Scancode::Kp4],
                &[Scancode::Right, Scancode::Kp6],
                &[Scancode::Space, Scancode::Return, Scancode::Kp0, Scancode::KpEnter],
                &[Scancode::Home],
                &[Scancode::End],
                &[Scancode::PageUp],
                &[Scancode::PageDown],
            ];
            let mut v = [0i8; 9];
            for (n, s) in sets.iter().enumerate() {
                v[n] = crate::key_poll(&mut self.menu_keys[n], s.iter().any(|k| pressed(*k)));
            }
            let i = &mut self.ui.input;
            (i.up, i.down, i.left, i.right, i.enter, i.home, i.end, i.pgup, i.pgdn) = (v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8]);
            self.menu_frame();
            evs.extend(sound::end_frame());
        }
    }

    /// The frame as RGB: the level's picture or the menus', through the gamma correction.
    fn render(&mut self, rgb: &mut [u8]) {
        let mut fb = std::mem::take(&mut self.game_fb);
        let pal = if self.in_game {
            let r = self.level.as_ref().unwrap();
            r.gfx.draw(&r.game, &mut fb);
            r.gfx.palette.map(|c| c.map(|v| v >> 2))
        } else {
            self.gfx.palette
        };
        let src = if self.in_game { &fb } else { &self.gfx.fb };
        let colours = rgb_with_gamma(&pal, self.settings.gamma);
        for (px, &i) in rgb.chunks_exact_mut(3).zip(src) {
            px.copy_from_slice(&colours[i as usize]);
        }
        self.game_fb = fb;
    }
}

impl FrontEnd {
    /// F3/F5/F6/F7 (FUN_1008_1966): straight to a menu.
    fn hotkey(&mut self, id: Id) {
        self.link_parent = true;
        self.show(id);
    }
}

