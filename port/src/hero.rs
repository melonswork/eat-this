//! Leon. A first cut of HeroUpdate (ETLAUNCH 1088:7592) following re/hero.md: standing, walking,
//! jumping, falling, crouching, landing and being knocked down by a long fall. Ladders, pipes,
//! ledges, slopes, platforms, weapons and the idle animations are not ported yet.

use crate::rng::PascalRandom;
use crate::world::{World, solid};

/// gInput indices (ReadGameInput, 1088:53a5).
pub const UP: usize = 0;
pub const DOWN: usize = 1;
pub const LEFT: usize = 2;
pub const RIGHT: usize = 3;

/// gHeroState values.
pub const GROUND: i32 = 0;
pub const JUMP: i32 = 5;
pub const FALL: i32 = 1;
pub const CROUCH: i32 = -40;
pub const KNOCKED_DOWN: i32 = -200;

const HEIGHT: i32 = 42;
const HALF_WIDTH: i32 = 6;
const WALK_LIMIT: i32 = 7; // HeroAccelerate(7, gWalkStep * facing)
const WALK_STEP: i32 = 1; // gWalkStep, 1170:1608 (8 in ETFLY mode)
const STAND_FRAME: i32 = 0x84; // 1170:127a
const FATAL_FALL: i32 = 0x120;

pub struct Hero {
    pub x: i32, // feet centre, px
    pub y: i32,
    pub facing: i32,
    pub state: i32,
    pub frame: i32, // sprite index over PPHERO/PPHERO2/PPHERO3 (256 each)
    vx: i32,
    vy: i32, // 1/10 px per frame
    vy0: i32,
    lift_off: bool,
    fall: i32,
    anim: i32,
    crouch: i32,
    crouch_max: i32,
    timer: i32,
    released: [bool; 9],
    pub height: i32,
    /// Fired effects for the frame, for the caller to show: (screen shake strength, duration).
    pub shake: Option<(i32, i32)>,
}

impl Hero {
    pub fn new((x, y): (i32, i32)) -> Hero {
        Hero {
            x,
            y,
            facing: 1,
            state: FALL,
            frame: STAND_FRAME,
            vx: 0,
            vy: 1,
            vy0: 1,
            lift_off: false,
            fall: 0,
            anim: 6,
            crouch: 0,
            crouch_max: 0,
            timer: 0,
            released: [true; 9],
            height: HEIGHT,
            shake: None,
        }
    }

    /// HeroStartJump (1088:39d4).
    fn start_jump(&mut self, vy: i32, kind: i32) {
        if vy < 1 || self.state < 1 {
            self.fall = 0;
        }
        self.state = kind;
        self.vy = vy;
        self.vy0 = vy;
        self.lift_off = vy < 0;
        self.anim = 0;
    }

    fn crouch_down(&mut self, progress: i32) {
        self.state = CROUCH;
        self.crouch = progress;
        self.crouch_max = progress;
    }

    fn stand(&mut self) {
        self.state = GROUND;
        self.anim = 6;
    }

    fn on_floor(&self, w: &World) -> bool {
        [-HALF_WIDTH, HALF_WIDTH]
            .iter()
            .any(|&o| w.solidity_at(self.y + 1, self.x + o) & solid::HERO_FLOOR != 0)
    }

    pub fn update(&mut self, w: &World, input: &[i8; 9], rng: &mut PascalRandom) {
        let mut input = *input;
        let (mut dx, mut dy) = (0, 0);
        let mut friction = true;
        self.shake = None;
        self.height = HEIGHT;
        for (r, &v) in self.released.iter_mut().zip(&input) {
            if v < 1 {
                *r = true;
            }
        }
        if input[LEFT] > 0 && input[RIGHT] > 0 {
            input[LEFT] = 0;
            input[RIGHT] = 0;
        }
        let held = |k: usize| input[k] > 0;
        let dir = if held(LEFT) {
            -1
        } else if held(RIGHT) {
            1
        } else {
            0
        };

        match self.state {
            GROUND => {
                if held(DOWN) {
                    self.crouch_down(0);
                } else if held(UP) && self.released[UP] {
                    self.released[UP] = false;
                    self.start_jump(-50, JUMP);
                } else if dir != 0 {
                    self.facing = dir;
                    self.vx = (self.vx + WALK_STEP * dir).clamp(-WALK_LIMIT, WALK_LIMIT);
                    dx += (self.vx.abs() / 4 + 1) * self.vx.signum();
                    if self.vx.signum() == self.facing {
                        friction = false;
                    }
                    self.anim = (self.anim + 1) % 0x28;
                    self.frame = self.anim / 2 + 0x81;
                } else {
                    self.frame = STAND_FRAME;
                }
            }
            CROUCH => {
                if held(DOWN) {
                    self.crouch = (self.crouch + 1).min(8);
                    self.crouch_max = self.crouch_max.max(self.crouch);
                } else if held(UP) && self.released[UP] {
                    self.released[UP] = false;
                    let vy = -(self.crouch_max >> 1) - 50;
                    self.start_jump(vy, JUMP);
                } else if self.crouch > 0 {
                    self.crouch -= 1;
                } else {
                    self.stand();
                }
                self.height = HEIGHT - self.crouch * 18 / 8;
                self.frame = 0xa0 + self.crouch.max(1);
            }
            KNOCKED_DOWN => {
                self.height = 0x1a;
                self.frame = (self.timer / 30) % 2 + 0x208;
                self.timer -= 1;
                if self.timer < 1 {
                    self.crouch_down(8);
                }
            }
            _ => {}
        }

        if self.state > 0 {
            if dir != 0 {
                dx += 2 * dir;
                self.facing = dir;
            }
            self.vy += if held(UP) { 3 } else { 10 };
            dy += self.vy / 10;
            if self.lift_off {
                dy -= 8;
                self.lift_off = false;
            }
            if self.vy > 0 {
                self.fall += self.vy / 10;
            }
            self.vy = self.vy.min(50);
            self.frame = ((self.vy - self.vy0 - 0x31) / 5 + 0x10e).clamp(0x105, 0x10e);
            self.height = 0x21;
        }

        if self.vx != 0 && friction {
            let s = self.vx.signum();
            dx += if self.vx.abs() / 8 == 0 {
                s
            } else {
                self.vx.abs() / 8 * s
            };
            self.vx = if self.vx.abs() < 5 {
                0
            } else {
                self.vx - 4 * s
            };
        }

        self.move_and_collide(w, dx, dy, rng);
    }

    /// A simplified HeroMoveAndCollide (1088:3322) + HeroFindFloor (1088:304c).
    fn move_and_collide(&mut self, w: &World, dx: i32, dy: i32, rng: &mut PascalRandom) {
        // Horizontal: the column the leading edge moves into, every 16 px from head to feet.
        if dx != 0 {
            let s = dx.signum();
            let edge = self.x + dx + s * HALF_WIDTH;
            let head = self.y - self.height + 1;
            let mut ys: Vec<i32> = (head..self.y).step_by(16).collect();
            ys.push(self.y);
            if ys
                .iter()
                .any(|&y| w.solidity_at(y, edge) & solid::WALL != 0)
            {
                self.x = (edge & !15) + if s < 0 { 15 } else { 0 } - (HALF_WIDTH + 1) * s;
                self.vx = 0;
            } else {
                self.x += dx;
            }
        }
        // Up: the ceiling row the head moves into.
        if dy < 0 {
            let head = self.y - self.height + 1 + dy;
            let hit = (head >> 4) != ((self.y - self.height + 1) >> 4)
                && [-HALF_WIDTH, HALF_WIDTH]
                    .iter()
                    .any(|&o| w.solidity_at(head, self.x + o) & solid::WALL != 0);
            if hit {
                self.y = (head & !15) + 16 + self.height - 1;
                if self.vy < 0 {
                    self.vy = 0;
                }
            } else {
                self.y += dy;
            }
            return;
        }
        // Down: land on the first floor row under the feet.
        let mut landed = false;
        let mut row_y = self.y + 1;
        while row_y <= self.y + dy.max(0) + 1 {
            if [-HALF_WIDTH, HALF_WIDTH]
                .iter()
                .any(|&o| w.solidity_at(row_y, self.x + o) & solid::HERO_FLOOR != 0)
            {
                self.y = (row_y & !15) - 1;
                landed = true;
                break;
            }
            row_y = (row_y & !15) + 16;
        }
        if !landed {
            self.y += dy;
        }
        if self.state > 0 && landed && self.vy >= 0 {
            self.land(rng);
        } else if self.state == GROUND && !self.on_floor(w) {
            self.start_jump(1, FALL);
        }
    }

    /// The landing block of HeroUpdate (gHeroGround > 0 while airborne).
    fn land(&mut self, rng: &mut PascalRandom) {
        let d = self.fall;
        self.vy = 0;
        if d >= 0x20 {
            // SpawnDust(10, Random(min(d/16+1, 8)), y, x): only its Random call matters here.
            rng.random((d / 16 + 1).min(8) as u16);
        }
        if d >= FATAL_FALL {
            self.state = KNOCKED_DOWN;
            self.timer = ((d - FATAL_FALL) / 2 + 100).min(0xaa);
            self.shake = Some((0x18, 0xe));
            return;
        }
        let progress = if d < 7 {
            4 - d / 2
        } else {
            (d / 32 + 1).min(8)
        };
        if progress < 2 {
            self.stand()
        } else {
            self.crouch_down(progress)
        }
        if d > 0x60 {
            self.shake = Some((8, (d / 16 - 4).min(8)));
        }
    }
}
