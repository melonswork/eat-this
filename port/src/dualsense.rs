//! Extras: the DualSense's adaptive triggers. What the two shoot buttons do now becomes a trigger
//! effect: the break of a gun, the stiffness of a launcher, the rattle of something firing, a kick as a
//! rocket leaves, slack when the weapon is empty. Reads the game only; other controllers never see it.
//!
//! The effects are the controller firmware's own modes (0x21 feedback, 0x25 weapon, 0x26 vibration),
//! set through SDL_GameControllerSendEffect's 47-byte state (DS5EffectsState_t), of which only the two
//! trigger blocks are marked valid: rumble and lights stay SDL's.

use crate::pad;

pub const SONY: u16 = 0x054c;
/// DualSense, DualSense Edge.
pub const PRODUCTS: [u16; 2] = [0x0ce6, 0x0df2];

/// What a trigger does. Zones are tenths of its travel (0..=9), strengths 1..=8.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Effect {
    #[default]
    Off,
    /// Stiff from zone `from` on.
    Feedback { from: u8, strength: u8 },
    /// Stiff from `start` (2..=7) until it gives way at `end` (after `start`, up to 8).
    Weapon { start: u8, end: u8, strength: u8 },
    /// Shakes from zone `from` on, `hz` times a second.
    Vibration { from: u8, strength: u8, hz: u8 },
}

impl Effect {
    /// The same at `percent` strength; nothing at 0.
    pub fn scaled(self, percent: i32) -> Effect {
        let k = |s: u8| ((s as i32 * percent + 99) / 100).clamp(1, 8) as u8;
        match self {
            _ if percent <= 0 => Effect::Off,
            Effect::Off => Effect::Off,
            Effect::Feedback { from, strength } => Effect::Feedback { from, strength: k(strength) },
            Effect::Weapon { start, end, strength } => Effect::Weapon { start, end, strength: k(strength) },
            Effect::Vibration { from, strength, hz } => Effect::Vibration { from, strength: k(strength), hz },
        }
    }

    /// The 11 bytes of a trigger block.
    pub fn bytes(self) -> [u8; 11] {
        let mut b = [0u8; 11];
        // A bit per zone from `from` on, and 3 bits of strength for each.
        let zones = |b: &mut [u8; 11], from: u8, strength: u8| {
            let s = strength.clamp(1, 8) as u32 - 1;
            let (mut active, mut force) = (0u16, 0u32);
            for z in from.min(9)..10 {
                active |= 1 << z;
                force |= s << (3 * z);
            }
            b[1..3].copy_from_slice(&active.to_le_bytes());
            b[3..7].copy_from_slice(&force.to_le_bytes());
        };
        match self {
            Effect::Off => b[0] = 0x05,
            Effect::Feedback { from, strength } => {
                b[0] = 0x21;
                zones(&mut b, from, strength);
            }
            Effect::Weapon { start, end, strength } => {
                let start = start.clamp(2, 7);
                let end = end.clamp(start + 1, 8);
                b[0] = 0x25;
                b[1..3].copy_from_slice(&((1u16 << start) | (1 << end)).to_le_bytes());
                b[3] = strength.clamp(1, 8) - 1;
            }
            Effect::Vibration { from, strength, hz } => {
                b[0] = 0x26;
                zones(&mut b, from, strength);
                b[9] = hz;
            }
        }
        b
    }
}

/// The controller state setting the (left, right) triggers and nothing else.
pub fn packet(t: [Effect; 2]) -> [u8; 47] {
    let mut p = [0u8; 47];
    p[0] = 0x04 | 0x08;
    p[10..21].copy_from_slice(&t[1].bytes());
    p[21..32].copy_from_slice(&t[0].bytes());
    p
}

/// Frames a kick lasts.
const KICK: u8 = 9;

/// The triggers' memory: what the last frame had.
#[derive(Clone, Copy, Default, Debug)]
pub struct Haptics {
    /// The weapon and its ammo.
    last: Option<(i32, i32)>,
    next_shot: u32,
    kick: u8,
}

impl Haptics {
    /// The (left, right) triggers after a frame of live play. A trigger not bound to a shoot action
    /// is slack.
    pub fn update(&mut self, g: &crate::Game, s: &pad::State, binds: &[i32; pad::ACTIONS.len()]) -> [Effect; 2] {
        let h = &g.hero;
        let ammo = |w: i32| usize::try_from(w).ok().and_then(|w| h.weapons.get(w)).map_or(0, |w| w.ammo);
        // A shot: the weapon of the last frame has less (it may have changed by running out), or the
        // vehicle's next torpedo was put off.
        let shot = if h.vehicle {
            h.veh.next_shot > self.next_shot
        } else {
            self.last.is_some_and(|(w, n)| ammo(w) < n)
        };
        let fired = self.last.map_or(-1, |l| l.0);
        let auto = !h.vehicle && matches!(fired, 1 | 2 | 6);
        self.last = Some((h.weapon, ammo(h.weapon)));
        self.next_shot = h.veh.next_shot;
        self.kick = if shot && !auto { KICK } else { self.kick.saturating_sub(1) };
        if h.health <= 0 {
            *self = Haptics::default();
            return [Effect::Off; 2];
        }
        let main = self.main(h.vehicle, s.action(binds, pad::FIRE) && h.gun.cooldown() > 0);
        let alt = self.alt(h.vehicle, h.weapon, ammo(h.weapon), s.action(binds, pad::ALT));
        let of = |button: i32| {
            if binds[pad::FIRE] == button {
                main
            } else if binds[pad::ALT] == button {
                alt
            } else {
                Effect::Off
            }
        };
        [of(pad::LT), of(pad::RT)]
    }

    /// The pistol: a light break; firing (a shot every 10 frames), it knocks along.
    fn main(&self, vehicle: bool, firing: bool) -> Effect {
        match (vehicle, firing) {
            (true, _) => Effect::Off, // the vehicle has only the torpedo
            (_, true) => Effect::Vibration { from: 3, strength: 4, hz: 7 },
            _ => Effect::Weapon { start: 2, end: 4, strength: 3 },
        }
    }

    /// The selected weapon's button.
    fn alt(&self, vehicle: bool, weapon: i32, ammo: i32, held: bool) -> Effect {
        if self.kick > 0 {
            return Effect::Vibration { from: 0, strength: 8, hz: 30 };
        }
        if vehicle {
            return Effect::Weapon { start: 2, end: 4, strength: 7 };
        }
        if ammo < 1 {
            return Effect::Off; // empty: slack
        }
        match weapon {
            // The flame thrower roars, a valve rather than a trigger.
            1 if held => Effect::Vibration { from: 2, strength: 5, hz: 28 },
            1 => Effect::Feedback { from: 2, strength: 3 },
            // The rapid gun (a shot every 13 frames) and the three bullets (every 10).
            2 if held => Effect::Vibration { from: 3, strength: 6, hz: 5 },
            2 => Effect::Weapon { start: 2, end: 4, strength: 4 },
            6 if held => Effect::Vibration { from: 3, strength: 7, hz: 7 },
            6 => Effect::Weapon { start: 2, end: 4, strength: 5 },
            // The launchers are the stiffest.
            3 | 5 | 8 => Effect::Weapon { start: 2, end: 4, strength: 8 },
            // What is thrown or laid: a steady pull.
            _ => Effect::Feedback { from: 2, strength: 4 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_encode_as_the_firmware_expects() {
        assert_eq!(Effect::Off.bytes(), [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        // Zones 2 and 5, full strength.
        assert_eq!(Effect::Weapon { start: 2, end: 5, strength: 8 }.bytes(), [0x25, 0x24, 0, 7, 0, 0, 0, 0, 0, 0, 0]);
        // Zones 8 and 9, strength 8: bits 8, 9; 3 bits each from bit 24.
        assert_eq!(Effect::Feedback { from: 8, strength: 8 }.bytes(), [0x21, 0, 3, 0, 0, 0, 0x3f, 0, 0, 0, 0]);
        assert_eq!(Effect::Vibration { from: 0, strength: 8, hz: 30 }.bytes(), [0x26, 0xff, 3, 0xff, 0xff, 0xff, 0x3f, 0, 0, 30, 0]);
        // Out of range values stay valid.
        assert_eq!(Effect::Weapon { start: 9, end: 1, strength: 99 }.bytes()[..4], [0x25, 0x80, 1, 7]);
    }

    #[test]
    fn the_packet_touches_only_the_triggers() {
        let p = packet([Effect::Weapon { start: 2, end: 4, strength: 1 }, Effect::Off]);
        assert_eq!((p[0], p[1]), (0x0c, 0));
        assert_eq!((p[10], p[21]), (5, 0x25)); // right, left
        assert!(p[2..10].iter().chain(&p[32..]).all(|&b| b == 0));
    }

    #[test]
    fn strength_scales_down_to_nothing() {
        let e = Effect::Feedback { from: 2, strength: 8 };
        assert_eq!(e.scaled(100), e);
        assert_eq!(e.scaled(50), Effect::Feedback { from: 2, strength: 4 });
        assert_eq!(e.scaled(1), Effect::Feedback { from: 2, strength: 1 });
        assert_eq!(e.scaled(0), Effect::Off);
    }
}
