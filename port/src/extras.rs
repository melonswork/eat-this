//! Extras' settings (docs/extras.md): EatThisExtras.ini in the working directory (`ET_EXTRAS_INI`),
//! apart from the original's EatThis.ini. The defaults change nothing.

use crate::gpu::Look;
use crate::pad;
use std::path::PathBuf;

/// The extra keys: save state, load state, next slot, rewind (VK codes).
pub const EXTRA_KEYS: [&str; 4] = ["SaveState", "LoadState", "NextSlot", "Rewind"];
const GAME_KEYS: [&str; 7] = ["Up", "Down", "Left", "Right", "LeftShoot", "RightShoot", "SwitchWeapon"];

#[derive(Clone, Debug, PartialEq)]
pub struct Extras {
    /// gpu.rs: plain, sharp, Scale4x, xBR.
    pub upscale: i32,
    pub crt: i32,
    pub integer: i32,
    /// Percentages.
    pub glow: i32,
    pub light: i32,
    /// 0 off, 1 16:9, 2 21:9.
    pub wide: i32,
    /// Widescreen option B: actors in the extra columns act.
    pub wide_sim: i32,
    /// The right stick: 0 a free cursor, 1 a circle around the hero.
    pub aim: i32,
    /// Percentage.
    pub rumble: i32,
    pub rewind: i32,
    /// Record every level started.
    pub record: i32,
    pub slot: i32,
    /// The game's keys, over EatThis.ini's (0: EatThis.ini's).
    pub keys: [i32; 7],
    pub extra_keys: [i32; 4],
    /// pad::Action -> pad::Button.
    pub pad: [i32; pad::ACTIONS.len()],
}

impl Default for Extras {
    fn default() -> Extras {
        Extras {
            upscale: 0,
            crt: 0,
            integer: 0,
            glow: 0,
            light: 0,
            wide: 0,
            wide_sim: 0,
            aim: 1,
            rumble: 60,
            rewind: 0,
            record: 0,
            slot: 1,
            keys: [0; 7],
            extra_keys: [0x77, 0x78, 0x79, 0x08],
            pad: pad::DEFAULTS,
        }
    }
}

impl Extras {
    pub fn path() -> PathBuf {
        std::env::var_os("ET_EXTRAS_INI").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("EatThisExtras.ini"))
    }

    /// Every setting with its name and its range.
    fn fields(&mut self) -> Vec<(String, &mut i32, i32, i32)> {
        let mut v: Vec<(String, &mut i32, i32, i32)> = vec![
            ("Upscale".into(), &mut self.upscale, 0, 3),
            ("Crt".into(), &mut self.crt, 0, 1),
            ("IntegerScaling".into(), &mut self.integer, 0, 1),
            ("Glow".into(), &mut self.glow, 0, 100),
            ("Light".into(), &mut self.light, 0, 100),
            ("Widescreen".into(), &mut self.wide, 0, 2),
            ("WideSimulation".into(), &mut self.wide_sim, 0, 1),
            ("Aim".into(), &mut self.aim, 0, 1),
            ("Rumble".into(), &mut self.rumble, 0, 100),
            ("Rewinding".into(), &mut self.rewind, 0, 1),
            ("Record".into(), &mut self.record, 0, 1),
            ("Slot".into(), &mut self.slot, 1, 9),
        ];
        for (n, k) in GAME_KEYS.iter().zip(&mut self.keys) {
            v.push((format!("Key{n}"), k, 0, 255));
        }
        for (n, k) in EXTRA_KEYS.iter().zip(&mut self.extra_keys) {
            v.push((format!("Key{n}"), k, 0, 255));
        }
        for (n, k) in pad::ACTIONS.iter().zip(&mut self.pad) {
            v.push((format!("Pad{}", n.replace(' ', "")), k, 0, pad::BUTTONS.len() as i32 - 1));
        }
        v
    }

    pub fn parse(text: &str) -> Extras {
        let mut x = Extras::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let Ok(v) = v.trim().parse::<i32>() else { continue };
            for (n, f, lo, hi) in x.fields() {
                if n.eq_ignore_ascii_case(k.trim()) {
                    *f = v.clamp(lo, hi);
                }
            }
        }
        x
    }

    pub fn load() -> Extras {
        std::fs::read_to_string(Self::path()).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn text(&self) -> String {
        let mut x = self.clone();
        x.fields().iter().map(|(n, v, ..)| format!("{n}={v}\n")).collect()
    }

    /// Written only once something differs from the file.
    pub fn save(&self) {
        if *self != Self::load() {
            if let Err(e) = std::fs::write(Self::path(), self.text()) {
                eprintln!("can't write {}: {e}", Self::path().display());
            }
        }
    }

    pub fn look(&self) -> Look {
        Look { upscale: self.upscale, crt: self.crt != 0, integer: self.integer != 0 }
    }

    /// Extra columns on each side (a 200-line picture at 16:9 and 21:9).
    pub fn margin(&self) -> i32 {
        [0, 18, 74][self.wide.clamp(0, 2) as usize]
    }

    /// `Game::wide_actors`: the view may lie up to twice the margin to one side.
    pub fn wide_actors(&self) -> i32 {
        if self.wide_sim != 0 { self.margin() * 2 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_bad_values_are_clamped() {
        let mut x = Extras::default();
        x.glow = 55;
        x.keys[2] = 0x41;
        x.pad[1] = 3;
        assert_eq!(Extras::parse(&x.text()), x);
        let y = Extras::parse("Upscale=99\nWidescreen=-4\nGlow=nope\nSlot=0\n");
        assert_eq!((y.upscale, y.wide, y.glow, y.slot), (3, 0, 0, 1));
    }
}
