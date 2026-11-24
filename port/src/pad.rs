//! Extras: game controllers. A controller becomes the inputs the original reads (the nine keys,
//! the cursor, the weapon keys), so the simulation can't tell the difference.

use sdl2::controller::{Axis, Button as B, GameController};

pub const BUTTONS: [&str; 12] = ["NONE", "A", "B", "X", "Y", "LB", "RB", "LT", "RT", "L3", "R3", "BACK"];
pub const ACTIONS: [&str; 8] = ["FIRE", "SECOND FIRE", "JUMP", "CROUCH", "PREVIOUS WEAPON", "NEXT WEAPON", "REWIND", "SWITCH WEAPON"];
pub const DEFAULTS: [i32; ACTIONS.len()] = [8, 7, 1, 2, 5, 6, 11, 4];

pub const FIRE: usize = 0;
pub const ALT: usize = 1;
pub const JUMP: usize = 2;
pub const CROUCH: usize = 3;
pub const PREV: usize = 4;
pub const NEXT: usize = 5;
pub const REWIND: usize = 6;
pub const SWITCH: usize = 7;

/// A controller's state, sticks in -1..1.
#[derive(Clone, Copy, Default, Debug)]
pub struct State {
    /// By `BUTTONS` index.
    pub held: [bool; BUTTONS.len()],
    pub start: bool,
    /// Up, down, left, right (d-pad or left stick).
    pub dirs: [bool; 4],
    pub aim: (f32, f32),
}

impl State {
    pub fn action(&self, binds: &[i32; ACTIONS.len()], a: usize) -> bool {
        self.held.get(binds[a].max(0) as usize).copied().unwrap_or(false)
    }

    /// The first button held, for remapping.
    pub fn any(&self) -> Option<i32> {
        self.held.iter().position(|h| *h).map(|i| i as i32)
    }

    /// The game's nine keys (the second pair mirrors the shoot keys, as EatThis.ini's do).
    pub fn keys(&self, binds: &[i32; ACTIONS.len()]) -> [bool; 9] {
        let a = |n| self.action(binds, n);
        let (fire, alt) = (a(FIRE), a(ALT));
        [
            self.dirs[0] || a(JUMP),
            self.dirs[1] || a(CROUCH),
            self.dirs[2],
            self.dirs[3],
            fire,
            alt,
            fire,
            alt,
            a(SWITCH),
        ]
    }
}

/// The cursor moved by the right stick: `hero` is the hero on the screen, `lo`..`hi` the columns the
/// cursor may be in. None while the stick rests (a resting cursor must not count as moved).
pub fn aim(s: &State, circle: bool, at: (i32, i32), hero: (i32, i32), lo: i32, hi: i32) -> Option<(i32, i32)> {
    let (x, y) = s.aim;
    let len = (x * x + y * y).sqrt();
    if len < 0.3 {
        return None;
    }
    let m = if circle {
        (hero.0 + (x / len * 80.0) as i32, hero.1 - 0x16 + (y / len * 80.0) as i32)
    } else {
        let v = (len.min(1.0) - 0.3) / 0.7 * 5.0 + 0.6;
        (at.0 + (x / len * v).round() as i32, at.1 + (y / len * v).round() as i32)
    };
    Some((m.0.clamp(lo, hi), m.1.clamp(0, crate::H - 1)))
}

pub struct Pad {
    sys: Option<sdl2::GameControllerSubsystem>,
    ctl: Option<GameController>,
    /// No more rumbling than needed: the last strength.
    last: (u16, u16),
}

impl Pad {
    pub fn new(sdl: &sdl2::Sdl) -> Pad {
        let sys = sdl.game_controller().map_err(|e| eprintln!("no controllers: {e}")).ok();
        let mut p = Pad { sys, ctl: None, last: (0, 0) };
        p.find();
        p
    }

    fn find(&mut self) {
        let Some(s) = &self.sys else { return };
        self.ctl = (0..s.num_joysticks().unwrap_or(0)).filter(|&i| s.is_game_controller(i)).find_map(|i| s.open(i).ok());
    }

    /// Hot-plugging.
    pub fn event(&mut self, e: &sdl2::event::Event) {
        use sdl2::event::Event;
        match e {
            Event::ControllerDeviceAdded { .. } if self.ctl.is_none() => self.find(),
            Event::ControllerDeviceRemoved { .. } if self.ctl.as_ref().is_some_and(|c| !c.attached()) => {
                self.ctl = None;
                self.find();
            }
            _ => {}
        }
    }

    pub fn state(&self) -> State {
        let Some(c) = &self.ctl else { return State::default() };
        let ax = |a| c.axis(a) as f32 / 32767.0;
        let (lx, ly) = (ax(Axis::LeftX), ax(Axis::LeftY));
        let b = |k| c.button(k);
        let mut held = [false; BUTTONS.len()];
        let list = [B::A, B::B, B::X, B::Y, B::LeftShoulder, B::RightShoulder];
        for (i, k) in list.into_iter().enumerate() {
            held[1 + i] = b(k);
        }
        held[7] = ax(Axis::TriggerLeft) > 0.4;
        held[8] = ax(Axis::TriggerRight) > 0.4;
        held[9] = b(B::LeftStick);
        held[10] = b(B::RightStick);
        held[11] = b(B::Back);
        State {
            held,
            start: b(B::Start),
            dirs: [
                b(B::DPadUp) || ly < -0.55,
                b(B::DPadDown) || ly > 0.55,
                b(B::DPadLeft) || lx < -0.4,
                b(B::DPadRight) || lx > 0.4,
            ],
            aim: (ax(Axis::RightX), ax(Axis::RightY)),
        }
    }

    /// Rumbles (low, high motor, 0..1) for `ms`.
    pub fn rumble(&mut self, low: f32, high: f32, ms: u32) {
        let v = ((low.clamp(0.0, 1.0) * 65535.0) as u16, (high.clamp(0.0, 1.0) * 65535.0) as u16);
        if v == (0, 0) && self.last == (0, 0) {
            return;
        }
        self.last = v;
        if let Some(c) = &mut self.ctl {
            let _ = c.set_rumble(v.0, v.1, ms);
        }
    }
}

/// What the game feels like this frame, from the state alone: (low, high) 0..1. `before` is the last
/// frame's health.
pub fn feel(g: &crate::Game, before: i32) -> (f32, f32) {
    let shake = g.camera.rumble() as f32 / 10.0;
    let hurt = (before - g.hero.health).max(0) as f32;
    let mut low = shake.min(0.7);
    let mut high = if hurt > 0.0 { (0.25 + hurt / 120.0).min(1.0) } else { 0.0 };
    if before > 0 && g.hero.health <= 0 {
        (low, high) = (1.0, 1.0);
    }
    // Explosions starting near the hero, stronger the nearer and the bigger.
    for p in &g.level.particles.list {
        let slot = (p.m.w(0x16) as u16) >> 8;
        let size = match slot {
            0x6f | 0x6e => 1.0,
            0x64 | 0x65 | 0x70 => 0.8,
            0x69 => 0.5,
            _ => continue,
        };
        if p.m.w(0x24) > 10 {
            continue;
        }
        let (dx, dy) = ((p.m.w(8) - g.hero.x) as f32, (p.m.w(0xa) - g.hero.y) as f32);
        low = low.max(size * (1.0 - (dx * dx + dy * dy).sqrt() / 180.0));
    }
    (low.clamp(0.0, 1.0), high)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticks_and_buttons_become_the_original_inputs() {
        let mut s = State::default();
        assert_eq!(s.keys(&DEFAULTS), [false; 9]);
        s.held[8] = true;
        s.dirs[3] = true;
        assert_eq!(s.keys(&DEFAULTS), [false, false, false, true, true, false, true, false, false]);
        assert_eq!(aim(&s, true, (5, 5), (160, 125), 0, 319), None);
        s.aim = (1.0, 0.0);
        assert_eq!(aim(&s, true, (5, 5), (160, 125), 0, 319), Some((240, 103)));
        assert_eq!(aim(&s, false, (318, 5), (160, 125), 0, 319), Some((319, 5)));
    }
}
