//! The camera, as the original moves it every frame (GameFrameInGame): FUN_1088_0a3b (target and
//! dead zone, mouse look, the grinder's scripted camera, horizontal shake) → FUN_1088_087b (at most
//! 15 px a frame, never past a STOP tile or the map edge) → FUN_1088_099d (vertical shake) →
//! FUN_1088_0c82 (move). LevelFirstFrame instead jumps straight to the target and clamps it to the
//! sector (FUN_1088_06f3). Objects only update while their box meets the camera's rectangle, so
//! this has to be exact; `--dump` compares it with the tracer.

use crate::world::{OUTSIDE, World, func};

const SCREEN: (i32, i32) = (320, 200);
/// Where on screen the hero rests (1170:4e34/4e36 drift back to this by 1 px a frame).
const REST: (i32, i32) = (0xa0, 0x7d);

/// A shake (1170:45ba vertical, 45c2 horizontal): frames left, the starting count, the amount
/// last applied, the amplitude.
#[derive(Clone, Copy, Default)]
struct Shake {
    count: i32,
    strength: i32,
    amount: i32,
    amplitude: i32,
}

impl Shake {
    /// ScreenShake (1050:5c4a) / FUN_1050_5c7d: a new shake wins unless the running one is
    /// still bigger.
    fn start(&mut self, strength: i32, amplitude: i32) {
        if self.count < 1 || (self.amplitude * self.count) / self.strength <= amplitude {
            self.amplitude = amplitude;
            self.strength = strength;
            self.count = strength;
        }
    }
}

pub struct Camera {
    pub x: i32, // gCameraX
    pub y: i32,
    /// The hero's resting point on screen (4e34/4e36); the hero code nudges it.
    pub offset: (i32, i32),
    target: (i32, i32), // 4e38/4e3a
    step: (i32, i32),   // 1af4/1af6: this frame's move
    /// Mouse look (4e4c/4e4e): how far the mouse is past the screen edge.
    pub look: (i32, i32),
    vertical: Shake,
    horizontal: Shake,
    sin: Vec<i32>, // 1170:6bb8: round(sin(i / 16 deg) * 0x4000)
}

impl Camera {
    /// HeroReset's camera: at 0, the hero resting at (0xa0, 0x7d).
    pub fn new() -> Camera {
        Camera {
            x: 0,
            y: 0,
            offset: REST,
            target: (0, 0),
            step: (0, 0),
            look: (0, 0),
            vertical: Shake::default(),
            horizontal: Shake::default(),
            sin: (0..5760)
                .map(|i| ((i as f64 / 16.0).to_radians().sin() * 16384.0).round() as i32)
                .collect(),
        }
    }

    /// ScreenShake(strength, amplitude).
    pub fn shake(&mut self, strength: i32, amplitude: i32) {
        self.vertical.start(strength, amplitude);
    }

    /// FUN_1050_5c7d.
    pub fn shake_sideways(&mut self, strength: i32, amplitude: i32) {
        self.horizontal.start(strength, amplitude);
    }

    /// LevelFirstFrame (1088:b4db): FUN_1088_0a3b(0), FUN_1088_06f3, FUN_1088_0c82.
    pub fn first_frame(&mut self, world: &World, hero: (i32, i32), grinder: Option<(i32, i32)>) {
        self.follow(hero, false, grinder);
        self.sector_clamp(world, hero);
        self.x += self.step.0;
        self.y += self.step.1;
    }

    /// One frame of GameFrameInGame. `follow` is false while riding (DAT_1170_5127 == 1).
    pub fn update(
        &mut self,
        world: &World,
        hero: (i32, i32),
        follow: bool,
        grinder: Option<(i32, i32)>,
    ) {
        if follow {
            self.follow(hero, true, grinder);
        }
        self.limit(world, hero);
        // FUN_1088_099d: the vertical shake, then the limits again.
        let v = &mut self.vertical;
        if v.count < 1 {
            v.amount = 0;
        } else {
            let i = pingpong(v.count, 4);
            v.amount = (((3 - i) * v.amplitude * v.count) / v.strength) / 3;
            self.step.1 -= v.amount;
            v.count -= 1;
        }
        if self.vertical.count > 0 {
            self.step.1 = Self::limit_axis(self.step.1, self.y, SCREEN.1, |p| {
                world.tile_function_at(p, hero.0)
            });
        }
        self.x += self.step.0;
        self.y += self.step.1;
    }

    /// FUN_1088_0a3b: where the camera wants to be.
    fn follow(&mut self, hero: (i32, i32), smooth: bool, grinder: Option<(i32, i32)>) {
        match grinder {
            Some((gy, offset)) => {
                self.target = (0, gy - offset);
            }
            None => {
                let want = (hero.0 - self.offset.0, hero.1 - self.offset.1);
                let zone = |want: i32, target: i32, z: i32| {
                    let d = want - target;
                    if !smooth {
                        want
                    } else if d.abs() > z {
                        want - d.signum() * z
                    } else {
                        target
                    }
                };
                self.target = (
                    zone(want.0, self.target.0, 0x28),
                    zone(want.1, self.target.1, 0x1e),
                );
                if self.offset.0 != REST.0 {
                    self.offset.0 += if REST.0 - self.offset.0 < 0 { -1 } else { 1 };
                }
                if self.offset.1 != REST.1 {
                    self.offset.1 += if REST.1 - self.offset.1 < 0 { -1 } else { 1 };
                }
                let look = |look: i32, target: &mut i32, want: i32, z: i32| {
                    if look != 0 {
                        *target += look;
                        if look < 0 {
                            *target = (*target).max(want - z);
                        } else {
                            *target = (*target).min(want + z);
                        }
                    }
                };
                look(self.look.0, &mut self.target.0, want.0, 0x28);
                look(self.look.1, &mut self.target.1, want.1, 0x1e);
            }
        }
        self.step = (self.target.0 - self.x, self.target.1 - self.y);
        let h = &mut self.horizontal;
        if h.count > 0 {
            let i = ((h.count * 0x400) % 0x1680) as usize;
            let k = ((h.amplitude * h.count) as i16 as i32 / h.strength) / 3;
            self.step.0 += (self.sin[i] * k) / 0x4000;
            h.count -= 1;
        }
    }

    /// FUN_1088_087b.
    fn limit(&mut self, world: &World, hero: (i32, i32)) {
        if self.step.0 != 0 {
            self.step.0 = Self::limit_axis(self.step.0, self.x, SCREEN.0, |p| {
                world.tile_function_at(hero.1, p)
            });
        }
        if self.step.1 != 0 {
            self.step.1 = Self::limit_axis(self.step.1, self.y, SCREEN.1, |p| {
                world.tile_function_at(p, hero.0)
            });
        }
    }

    /// At most 15 px; a move into a STOP tile or off the map stops at the tile edge.
    fn limit_axis(mut d: i32, pos: i32, screen: i32, f: impl Fn(i32) -> u16) -> i32 {
        let edge = if d < 1 { pos } else { pos + screen };
        d = d.clamp(-0xf, 0xf);
        let v = f(edge + d);
        if v == func::STOP || v == OUTSIDE {
            d = ((((edge + d) as u16 >> 4) as i32) + (d < 0) as i32) * 0x10 - edge;
        }
        d
    }

    /// FUN_1088_06f3: keep the first view inside the hero's sector.
    fn sector_clamp(&mut self, world: &World, hero: (i32, i32)) {
        let stop = |v: u16| v == func::STOP || v == OUTSIDE;
        let (hx, hy) = ((hero.0 as u16 >> 4) as i32, (hero.1 as u16 >> 4) as i32);
        for c in ((self.x + self.step.0) >> 4)..=hx {
            if stop(world.tile_function_at(hero.1, c << 4)) {
                self.step.0 = (c + 1) * 16 - self.x;
            }
        }
        let mut c = ((self.x + SCREEN.0 + self.step.0) as u16 >> 4) as i32;
        while c >= hx {
            if stop(world.tile_function_at(hero.1, c << 4)) {
                self.step.0 = c * 16 - (self.x + SCREEN.0);
            }
            c -= 1;
        }
        for r in ((self.y + self.step.1) >> 4)..=hy {
            if stop(world.tile_function_at(r << 4, hero.0)) {
                self.step.1 = (r + 1) * 16 - self.y;
            }
        }
        let mut r = ((self.y + SCREEN.1 + self.step.1) as u16 >> 4) as i32;
        while r >= hy {
            if stop(world.tile_function_at(r << 4, hero.0)) {
                self.step.1 = r * 16 - (self.y + SCREEN.1);
            }
            r -= 1;
        }
    }
}

/// FUN_10f0_1317(t, 1, 1, n).
fn pingpong(t: i32, n: i32) -> i32 {
    let v = (t % (n * 2 - 2) - n) + 1;
    (n - 1) - v.abs()
}
