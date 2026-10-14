//! Leon: HeroUpdate (ETLAUNCH 1088:7592) in the original's order, for the states ported so far:
//! standing, walking, turning, idling, jumping, falling, crouching, landing and being knocked down.
//! Every Random() call the original makes in those states is made here too, in the same order,
//! because the recorded demos check RandSeed (see re/demo.md). Ladders, pipes, ledges, slopes,
//! platforms, levers, doors and weapons are not ported yet.

use crate::rng::PascalRandom;
use crate::world::{World, solid};

/// gInput indices (ReadGameInput, 1088:53a5).
pub const UP: usize = 0;
pub const DOWN: usize = 1;
pub const LEFT: usize = 2;
pub const RIGHT: usize = 3;

/// gHeroState values.
pub const GROUND: i32 = 0;
pub const FALL: i32 = 1;
pub const JUMP: i32 = 5;
pub const CROUCH: i32 = -40;
pub const KNOCKED_DOWN: i32 = -200;

/// gHeroGround kind for an ordinary floor (HeroReset sets it; the heavy-landing test checks it).
const FLAT_FLOOR: i32 = 9;
const NONE: i32 = 0x7fff;
const NO_TURN: i32 = 0xff;
const ON_GROUND_VY: i32 = 0x7fff; // HeroResetFallSpeed
const HEIGHT: i32 = 0x2a;
const WALK_LIMIT: i32 = 7;
const WALK_STEP: i32 = 1; // 1170:1608 (8 in ETFLY mode)
const FATAL_FALL: i32 = 0x120;
/// Tile function 13100, "Send leaves": SpawnDust only does something on these.
const LEAVES: u16 = 0x332c;

pub struct Hero {
    pub x: i32, // feet centre, px
    pub y: i32,
    pub facing: i32,
    pub state: i32,
    pub frame: i32, // sprite index over PPHERO/PPHERO2/PPHERO3 (256 each); NONE = not drawn
    prev_state: i32,
    state_before_jump: i32, // 4dd2
    vx: i32,
    vy: i32, // 1/10 px per frame; ON_GROUND_VY on the ground
    vy0: i32,
    fall: i32,
    anim: i32,
    crouch: i32,
    crouch_max: i32,
    turn: i32,
    turn_kind: i32,
    stand_frame: i32, // 1170:127a
    idle_timer: i32,
    idle_next: i32,
    ground: i32, // gHeroGround from the last collision
    released: [bool; 9],
    pub height: i32,
    half_width: i32,
    /// Screen shake requested this frame: (strength, duration).
    pub shake: Option<(i32, i32)>,
}

impl Hero {
    /// HeroReset (1088:0442) followed by the level's placement. HeroReset's four Random() calls
    /// happen before DemoLoad zeroes RandSeed, so their results are not reproducible from a demo:
    /// `idle_timer` and `idle_next` take mid-range values instead.
    pub fn new((x, y): (i32, i32), facing: i32) -> Hero {
        Hero {
            x,
            y,
            facing,
            state: GROUND,
            frame: 0x26f,
            prev_state: GROUND,
            state_before_jump: GROUND,
            vx: 0,
            vy: ON_GROUND_VY,
            vy0: 0,
            fall: 0,
            anim: 6,
            crouch: 0,
            crouch_max: 0,
            turn: NO_TURN,
            turn_kind: 0,
            stand_frame: 0x84,
            idle_timer: -0x10 - 4,
            idle_next: 0xd2 + 0x1a4 / 2,
            ground: FLAT_FLOOR,
            released: [true; 9],
            height: HEIGHT,
            half_width: 6,
            shake: None,
        }
    }

    /// HeroStartJump (1088:39d4).
    fn start_jump(&mut self, vy: i32, kind: i32) {
        self.state_before_jump = self.state;
        if vy < 1 || self.state < 1 {
            self.fall = 0;
        }
        self.state = kind;
        self.vy = vy;
        self.vy0 = vy;
        self.anim = 0;
    }

    /// HeroCrouch (1088:32d1).
    fn crouch_down(&mut self) {
        if self.state != CROUCH {
            self.state = CROUCH;
            self.crouch = 0;
            self.crouch_max = 0;
            self.anim = 0;
            self.turn = NO_TURN;
        }
    }

    /// HeroStand (1088:0432).
    fn stand(&mut self) {
        self.state = GROUND;
        self.anim = 6;
    }

    pub fn update(&mut self, w: &World, input: &[i8; 9], rng: &mut PascalRandom) {
        let mut input = *input;
        let prev_facing = self.facing;
        self.frame = NONE;
        self.prev_state = self.state;
        self.half_width = 6;
        self.height = HEIGHT - (self.ground == 1) as i32;
        self.shake = None;
        let mut friction = true;
        let mut walking = false;
        let (mut dx, mut dy) = (0, 0);
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

        // Landing: the previous frame's collision found a floor.
        if self.ground > 0 && self.vy >= 0 && self.state >= 0 {
            self.vy = ON_GROUND_VY;
            if self.state > 0 {
                self.land(w, rng);
            }
        }

        if self.state == KNOCKED_DOWN {
            self.frame = (self.anim / 0x1e) % 2 + 0x208;
            self.height = 0x1a;
            if self.anim < 1 {
                self.crouch_down();
                self.crouch = 8;
            } else {
                self.anim -= 1;
            }
        }

        if held(DOWN) && self.state == GROUND {
            self.crouch_down();
        }

        if self.state == CROUCH {
            if self.crouch < 8 {
                self.crouch = (self.crouch + 1).abs();
            } else if self.crouch > 7 && (held(LEFT) || held(RIGHT)) {
                // Crouch-walk: one pixel a frame.
                let dir = if held(LEFT) { -1 } else { 1 };
                dx += dir;
                self.facing = dir;
                walking = true;
            }
            self.frame = self.crouch.abs() + 0xa0;
            self.crouch_max = self.crouch_max.max(self.crouch);
            if !held(DOWN) {
                self.crouch = -self.crouch;
                self.turn = NO_TURN;
            }
            self.height = HEIGHT - self.crouch.abs() * 0x12 / 8;
            if self.crouch == -1 {
                self.stand();
            }
        }

        if held(UP) && self.released[UP] && self.state == GROUND {
            let vy = if self.prev_state == CROUCH {
                -(self.crouch_max >> 1) - 0x32
            } else {
                -0x32
            };
            self.start_jump(vy, JUMP);
            self.released[UP] = false;
        }

        if self.state == GROUND && (held(LEFT) || held(RIGHT)) {
            self.facing = if held(LEFT) { -1 } else { 1 };
            self.vx = (self.vx + WALK_STEP * self.facing).clamp(-WALK_LIMIT, WALK_LIMIT);
            dx += (self.vx.abs() / 4 + 1) * self.vx.signum();
            if self.vx.signum() == self.facing {
                friction = false;
            }
            walking = true;
        }

        // Walked off an edge.
        if self.ground == 0 && self.state < 1 && matches!(self.state, GROUND | CROUCH) {
            self.start_jump(1, FALL);
        }

        if self.state == GROUND && self.facing != prev_facing {
            if self.turn == NO_TURN {
                self.turn = 2;
                self.turn_kind = 4;
            } else {
                self.turn = self.turn.signum() * 0xd - self.turn;
            }
        }

        if self.state > 0 {
            if held(LEFT) {
                dx -= 2;
                self.facing = -1;
            } else if held(RIGHT) {
                dx += 2;
                self.facing = 1;
            }
            let vy_before = self.vy;
            self.vy += if held(UP) { 3 } else { 10 };
            dy += self.vy / 10;
            if self.state_before_jump < 1 {
                if self.state < 0x10 {
                    dy -= 8;
                }
                self.state_before_jump = 1;
            }
            if self.state == FALL {
                self.frame = 0x110;
                self.height = if self.vy < 0x1f { 0x1f } else { 0x21 };
            } else {
                self.frame = (self.vy - self.vy0 - 0x31) / 5 + 0x10e;
                if !(0x105..=0x10e).contains(&self.frame) {
                    self.frame = 0x10e;
                }
                self.height = if vy_before == self.vy0 { 0x21 } else { 0x1f };
            }
            if self.vy >= 1 {
                self.fall += self.vy / 10;
            }
            self.vy = self.vy.min(0x32);
        }

        if self.vx != 0 && friction {
            let s = self.vx.signum();
            let step = self.vx.abs() / 8 * s;
            dx += if step == 0 { s } else { step };
            self.vx = if self.vx.abs() < 5 {
                0
            } else {
                self.vx - 4 * s
            };
        }

        self.ground = 0;
        let blocked = self.move_and_collide(w, dx, dy);
        self.end_of_frame(walking, blocked, rng);
    }

    /// The landing block at the top of HeroUpdate.
    fn land(&mut self, w: &World, rng: &mut PascalRandom) {
        let d = self.fall;
        if d > 0x1f {
            let n = rng.random((d / 16 + 1).min(8) as u16);
            spawn_dust(w, n, self.y, self.x, rng);
        }
        if d >= FATAL_FALL {
            self.state = KNOCKED_DOWN;
            self.anim = ((d - FATAL_FALL) >> 1) + 100;
            self.anim = self.anim.min(0xaa);
            self.shake = Some((0x18, 0xe));
            return;
        }
        let p = if w.solidity_at(self.y - 0x20, self.facing * self.half_width + self.x)
            & solid::WALL
            != 0
        {
            8
        } else if d < 7 {
            4 - (d >> 1)
        } else if self.prev_state == 7 {
            (d / 0x18 + 1).min(8)
        } else {
            (d / 0x20 + 1).min(8)
        };
        if p < 2 {
            self.stand();
        } else {
            self.crouch_down();
            self.crouch = p;
        }
        if d > 0x60 {
            self.shake = Some((8, (d / 0x10 - 4).min(8)));
        }
    }

    /// The tail of HeroUpdate after the collision: animation frames and the Random() calls.
    fn end_of_frame(&mut self, walking: bool, blocked: i32, rng: &mut PascalRandom) {
        if self.state != GROUND {
            if self.state != CROUCH {
                self.turn = NO_TURN;
            }
            self.idle_timer = -0x10 - rng.random(8);
            return;
        }
        if self.turn != NO_TURN {
            let t = self.turn.abs();
            self.frame = if self.turn_kind == 2 {
                0x66 - t
            } else {
                (t + 8) % 0x14 + 0x5b
            };
            self.turn += self.turn.signum();
            if self.turn.abs() < 0xb {
                return;
            }
            self.turn = NO_TURN;
            self.anim = 6;
            return;
        }
        if walking {
            if blocked != 0 && blocked.signum() == self.facing {
                // Pushing against a wall.
                self.anim = (self.anim + 1) % 0x54;
                self.frame = self.anim / 7 + 0x227;
                self.vx = self.facing * 7;
            } else {
                self.frame = self.anim / 2 + 0x81;
                self.anim = (self.anim + 1) % 0x28;
            }
            self.idle_timer = -0x10 - rng.random(8);
            return;
        }
        // Standing: ease the walk cycle back to a standing pose, then idle.
        let i = ((self.anim / 2 - (self.stand_frame - 0x81)) + 0x19) % 0x14;
        let mut l = i - 5;
        if l.abs() > 5 {
            self.stand_frame = (self.stand_frame - 0x77) % 0x14 + 0x81;
            l = i - 0xf;
        }
        if l != 0 {
            self.anim = ((self.anim / 2 - l.signum() + 0x14) * 2) % 0x28;
            if self.frame == NONE {
                self.frame = self.anim / 2 + 0x81;
            }
            return;
        }
        if self.frame != NONE {
            return;
        }
        self.frame = 0x26f;
        self.idle_timer += 1;
        if self.idle_timer > 0 {
            if self.idle_next <= self.idle_timer {
                self.idle_timer = if rng.random(3) == 0 { 0x25 } else { 0 };
                self.idle_next = rng.random(0x1a4) + 0xd2;
            }
            let t = self.idle_timer;
            self.frame = match t {
                0..=6 => 0x26f - t / 3,
                7..=0x1e => 0x26d,
                0x1f..=0x2b => (t - 0x1f) / 3 + 0x26d,
                0x2c..=0x6b => 0x271,
                0x6c..=0x71 => 0x271 - (t - 0x6b) / 3,
                _ => 0x26f,
            };
        }
    }

    /// A simplified HeroMoveAndCollide (1088:3322) + HeroFindFloor (1088:304c). Returns how much
    /// of dx was blocked (gHeroBlockedDX, 1170:4e9c).
    fn move_and_collide(&mut self, w: &World, dx: i32, dy: i32) -> i32 {
        let hw = self.half_width;
        let mut moved = dx;
        if dx != 0 {
            let s = dx.signum();
            let edge = self.x + dx + s * hw;
            let head = self.y - self.height + 1;
            let mut ys: Vec<i32> = (head..self.y).step_by(16).collect();
            ys.push(self.y);
            if ys
                .iter()
                .any(|&y| w.solidity_at(y, edge) & solid::WALL != 0)
            {
                let x = (edge & !15) + if s < 0 { 15 } else { 0 } - (hw + 1) * s;
                moved = x - self.x;
                self.x = x;
            } else {
                self.x += dx;
            }
        }
        if dy < 0 {
            let head = self.y - self.height + 1 + dy;
            let hit = (head >> 4) != ((self.y - self.height + 1) >> 4)
                && [-hw, hw]
                    .iter()
                    .any(|&o| w.solidity_at(head, self.x + o) & solid::WALL != 0);
            if hit {
                self.y = (head & !15) + 16 + self.height - 1;
                self.vy = 0;
            } else {
                self.y += dy;
            }
        } else {
            let mut row_y = self.y + 1;
            while row_y <= self.y + dy + 1 {
                if [-hw, hw]
                    .iter()
                    .any(|&o| w.solidity_at(row_y, self.x + o) & solid::HERO_FLOOR != 0)
                {
                    self.y = (row_y & !15) - 1;
                    self.ground = FLAT_FLOOR;
                    break;
                }
                row_y = (row_y & !15) + 16;
            }
            if self.ground == 0 {
                self.y += dy;
            }
        }
        dx - moved
    }
}

/// SpawnDust (1040:1e04): `count` leaf particles, five Random() calls each, on "Send leaves" tiles.
fn spawn_dust(w: &World, count: i32, y: i32, x: i32, rng: &mut PascalRandom) {
    if w.tile_function_at(y, x) != LEAVES {
        return;
    }
    for _ in 0..count {
        rng.random(5);
        rng.random(0x501);
        rng.random(2);
        rng.random(0x11);
        rng.random(10);
    }
}
