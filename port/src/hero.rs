//! Leon: HeroUpdate (ETLAUNCH 1088:7592) in the original's order, for the states ported so far:
//! standing, walking, turning, idling, jumping, falling, crouching, landing and being knocked down.
//! Every Random() call the original makes in those states is made here too, in the same order,
//! because the recorded demos check RandSeed (see re/demo.md). Ladders, pipes, ledges, slopes,
//! platforms, levers, doors and weapons are not ported yet.

use crate::level::{Level, Platform};
use crate::rng::PascalRandom;
use crate::world::{MAIN_PLANE, OUTSIDE, World, solid};

/// gInput indices (ReadGameInput, 1088:53a5).
pub const UP: usize = 0;
pub const DOWN: usize = 1;
pub const LEFT: usize = 2;
pub const RIGHT: usize = 3;
pub const LEFT_SHOOT: usize = 4;
pub const RIGHT_SHOOT: usize = 5;
/// 1170:4f06: the left button may fire (cleared by the crouch-flip move, not ported).
const FIRE_OK: bool = true;

/// gHeroState values.
pub const GROUND: i32 = 0;
pub const FALL: i32 = 1;
pub const JUMP: i32 = 5;
pub const CROUCH: i32 = -40;
pub const LEDGE: i32 = -4;
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
    nudge: i32,
    hang_x: i32,       // 1170:4e52
    hang_y: i32,       // 1170:4e54
    climb: i32,        // 1170:4e0e: 0 hanging, 1..0x1b pulling up
    push_timer: i32,   // 1170:4e12 (gHeroShootWalkTimer): frames since pushing a wall
    prev_blocked: i32, // 1170:4e64: last frame's gHeroBlockedDX
    /// 1170:4e9e after the collision: -1 hit a ceiling, 1 landed (read next frame as 4e66).
    ceiling: i32,
    head_blocked: i32, // 1170:4e68
    pub crushed: bool,
    /// Sensitive-lever tiles touched this frame (HeroTouchTiles), as (ty, tx), for the level.
    pub touched: Vec<(i32, i32)>, // 1170:4e06: HeroPushOutOfWalls' push, becomes next frame's starting dx (FUN_1088_532c)
    released: [bool; 9],
    input_cache: [i8; 9],
    pub height: i32,
    half_width: i32,
    /// ScreenShake calls made this frame, in order: (strength, amplitude).
    pub shakes: Vec<(i32, i32)>,
    /// Changes to the camera's resting point (1170:4e34/4e36) made this frame.
    pub cam_nudge: (i32, i32),
    pub gun: Gun,
}

/// The pistol (weapon 2's left button; HeroUpdate's end block and HeroFireWeapon 1088:6437).
#[derive(Default)]
pub struct Gun {
    pub muzzle: (i32, i32), // gHeroMuzzleX/Y (4de8/4dea): offset of the gun from its rest point
    pub aim: i32,           // gHeroAimAngle (4e46)
    pub shooting: bool,     // gHeroShooting (5121): aiming pose; set by moving the mouse
    armed: bool,            // 5122: this frame's pose can fire
    pose: i32,              // 4e10: frames into the aiming pose (0..10)
    cooldown: i32,          // 4e56: frames until the next shot, also the recoil
    pub shots: i32,         // 4e6a
    pub frame2: i32,        // gHeroFrame2 (4de4): the arm
    mouse: (i32, i32),      // gMouseX/Y last frame
    cam: (i32, i32),        // gCameraX/Y this frame (before the camera moves)
    /// HeroFireWeapon's call this frame: (y, x, angle, sprite parity).
    pub shot: Option<(i32, i32, i32, i32)>,
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
            vy: 0,
            vy0: 0,
            fall: 0,
            anim: 6,
            crouch: 0,
            crouch_max: 0,
            turn: NO_TURN,
            turn_kind: 0,
            stand_frame: 0x84,
            idle_timer: -0x10 - 3,
            idle_next: 0xd2 + 0x1a4 / 2,
            ground: FLAT_FLOOR,
            nudge: 0,
            hang_x: 0,
            hang_y: 0x7fff,
            climb: 0,
            push_timer: 0,
            prev_blocked: 0,
            ceiling: 0,
            head_blocked: 0,
            crushed: false,
            touched: Vec::new(),
            released: [true; 9],
            input_cache: [0; 9],
            height: HEIGHT,
            half_width: 6,
            shakes: Vec::new(),
            cam_nudge: (0, 0),
            gun: Gun::default(),
        }
    }

    /// Names and values to compare with the tracer's --watch (tools/compare_watch.py).
    pub const WATCH: &'static str = "gHeroX,gHeroY,gHeroState,gHeroVX,gHeroVY,gHeroFallDistance,gHeroAnimTimer,gHeroFrame,gHeroFacing";

    pub fn crouch_level(&self) -> i32 {
        self.crouch
    }

    pub fn watch(&self) -> [i32; 9] {
        let vy = if self.vy == ON_GROUND_VY {
            0x7fff
        } else {
            self.vy
        };
        [
            self.x,
            self.y,
            self.state,
            self.vx,
            vy,
            self.fall,
            self.anim,
            self.frame,
            self.facing,
        ]
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

    pub fn gun_mouse(&self) -> (i32, i32) {
        self.gun.mouse
    }

    /// HeroAimAtMouse (1088:420b).
    fn aim_at_mouse(&mut self) {
        let g = &mut self.gun;
        g.aim = crate::trig::atan2(
            g.mouse.1 - ((self.y + g.muzzle.1 - 0x16) - g.cam.1),
            g.mouse.0 - ((self.x + g.muzzle.0) - g.cam.0),
        );
    }

    /// HeroAimFixed (1088:4242): aim, and the arm's frame for the angle.
    fn aim_fixed(&mut self) {
        self.aim_at_mouse();
        let g = &mut self.gun;
        g.frame2 = ((g.aim * 0x20 + 0xb40) / 0x1680) % 0x20 + 0x19b;
        g.armed = true;
    }

    /// `mouse` is gMouseX/Y as the demo sets it; `cam` is the camera before this frame's move.
    pub fn update(
        &mut self,
        w: &World,
        lv: &Level,
        input: &[i8; 9],
        mouse: (i32, i32),
        cam: (i32, i32),
        rng: &mut PascalRandom,
    ) {
        // ReadGameInput: moving the mouse raises the gun.
        self.gun.shot = None;
        if mouse != self.gun.mouse {
            self.gun.shooting = true;
        }
        self.gun.mouse = mouse;
        self.gun.cam = cam;
        self.gun.frame2 = NONE;
        self.gun.armed = false;
        if (input[LEFT_SHOOT] > 0 && FIRE_OK) || input[RIGHT_SHOOT] > 0 {
            self.gun.shooting = true;
        }
        let mut input = *input;
        let prev_facing = self.facing;
        self.frame = NONE;
        self.prev_state = self.state;
        self.half_width = 6;
        self.height = HEIGHT - (self.ground == 1) as i32;
        self.shakes.clear();
        self.cam_nudge = (0, 0);
        let mut friction = true;
        let mut walking = false;
        let (mut dx, mut dy) = (std::mem::take(&mut self.nudge), 0);
        for (r, &v) in self.released.iter_mut().zip(&input) {
            if v < 1 {
                *r = true;
            }
        }
        if input[LEFT] > 0 && input[RIGHT] > 0 {
            input[LEFT] = 0;
            input[RIGHT] = 0;
        }
        self.input_cache = input;
        if self.push_timer > 0 {
            self.push_timer -= 1;
        }
        let held = |k: usize| input[k] > 0;

        // A ceiling right above the head forces a crouch.
        if w.solidity_at(self.y - 0x2b, self.x) & solid::WALL != 0 {
            self.head_blocked = 1;
        }
        if self.head_blocked > 0 && self.state == GROUND {
            self.crouch_down();
        }

        // HeroTouchTiles (1088:5847).
        self.touched.clear();
        if w.tile_function_at(self.y - 0x1c, self.x) == crate::level::SENSITIVE_LEVER {
            self.touched.push(((self.y - 0x1c) >> 4, self.x >> 4));
        }
        if w.tile_function_at(self.y - 0xc, self.x) == crate::level::SENSITIVE_LEVER {
            self.touched.push(((self.y - 0xc) >> 4, self.x >> 4));
        }

        // Landing: the previous frame's collision found a floor.
        if self.ground > 0 && self.vy >= 0 && self.state >= 0 {
            self.vy = ON_GROUND_VY;
            if self.state > 0 {
                self.land(w, rng);
            }
        }

        if self.state == KNOCKED_DOWN {
            self.gun.shooting = false;
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
            self.gun.shooting = false;
            if self.crouch < 8 {
                self.crouch = (self.crouch + 1).abs();
            } else if self.ground != 0x15 {
                self.half_width = 9;
            }
            self.frame = self.crouch + 0xa0;
            let ix = self.facing * self.half_width + self.x;
            if self.head_blocked == 0
                && (w.solidity_at(self.y - 0x20, ix) & solid::WALL == 0
                    || w.solidity_at(self.y - 0x10, ix) & solid::WALL != 0)
            {
                self.crouch_max = self.crouch_max.max(self.crouch);
                if !held(DOWN) {
                    self.crouch = -self.crouch;
                    self.turn = NO_TURN;
                }
                self.height = HEIGHT - self.crouch.abs() * 0x12 / 8;
            } else {
                self.height = 0x18;
            }
            if self.crouch == -1 {
                self.stand();
            } else if self.crouch > 7 {
                // Crouch-walk.
                let mut moving = false;
                if !held(LEFT) {
                    if held(RIGHT) {
                        dx += 1;
                        self.facing = 1;
                        moving = true;
                    }
                } else {
                    moving = !held(RIGHT);
                    if moving {
                        dx -= 1;
                        self.facing = -1;
                    }
                }
                if self.facing != prev_facing {
                    self.turn = if self.turn == NO_TURN {
                        1
                    } else {
                        self.turn.signum() * 0xd - self.turn
                    };
                }
                if self.turn == NO_TURN {
                    let next = if moving && self.prev_blocked == 0 {
                        self.anim + 1
                    } else {
                        let i = (self.anim / 2 + 0x19) % 0x14;
                        let mut l = i - 5;
                        if l.abs() > 5 {
                            l = i - 0xf;
                        }
                        ((self.anim / 2 - l.signum()) + 0x14) * 2
                    };
                    self.anim = next % 0x28;
                    self.frame = self.anim / 2 + 0xac;
                } else {
                    self.turn += if self.turn < 0 { -1 } else { 1 };
                    self.frame = 0xe4 - self.turn.abs() / 2;
                    if self.turn.abs() > 0xc {
                        self.turn = NO_TURN;
                        self.anim = 0;
                    }
                }
            }
        }

        if held(UP) && self.released[UP] && self.state == GROUND {
            // Reach for a ledge above first (FUN_1088_3e0d), else jump.
            let (ndx, ndy) = self.grab_ledge(w, lv, self.y - 0x30, dx, dy);
            (dx, dy) = (ndx, ndy);
            if self.state != LEDGE {
                let mut vy = if self.prev_state == CROUCH {
                    -(self.crouch_max >> 1) - 0x32
                } else {
                    -0x32
                };
                if self.ground == 0x17 {
                    vy = (vy << 3) / 7;
                }
                self.start_jump(vy, JUMP);
                self.released[UP] = false;
            }
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
                    self.cam_nudge.1 -= 8;
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
                if self.gun.shooting {
                    self.gun.muzzle = (if self.facing < 1 { 4 } else { 0 }, 8);
                    if self.state == JUMP || self.state == 10 {
                        self.frame += 0xf0;
                        self.aim_fixed();
                    } else {
                        self.aim_at_mouse();
                        self.gun.armed = true;
                    }
                }
            }
            if self.vy < 1 {
                if self.vy < 0 && self.ceiling < 0 {
                    self.vy = 0;
                }
            } else {
                self.fall += self.vy / 10;
                let (ndx, ndy) = self.grab_ledge(w, lv, self.y - 0x20, dx, dy);
                (dx, dy) = (ndx, ndy);
            }
            self.vy = self.vy.min(0x32);
        }

        let mut collide_flags = 3;
        if self.state == LEDGE {
            let (ndx, ndy) = self.hang(w, lv, &input, dx, dy);
            (dx, dy) = (ndx, ndy);
            collide_flags = 1;
        }
        let _ = collide_flags;

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
        self.head_blocked = 0;
        let (blocked, step_h) = self.move_and_collide(w, lv, dx, dy);
        self.prev_blocked = blocked;
        self.push_out_of_walls(w);
        let posed = self.end_of_frame(walking, blocked, step_h, rng);
        self.gun_end(posed, &input);
    }

    /// The end of HeroUpdate: the aiming pose, the left button's shot, the recoil.
    fn gun_end(&mut self, posed: bool, input: &[i8; 9]) {
        if posed && self.gun.shooting {
            if self.frame < 0x81 || self.frame > 0x94 {
                let l = self.gun.pose / 4;
                self.gun.muzzle = (if self.facing > 0 { -l } else { l } + 2, 0);
                self.aim_fixed();
                self.gun.muzzle.1 += l - 2;
                self.frame = self.gun.pose / 2 + 0x1e3;
                if self.gun.pose < 10 {
                    self.gun.pose += 1;
                }
            } else {
                self.gun.muzzle = (2, 0);
                self.aim_fixed();
                self.gun.muzzle.1 -= ((self.frame - 0x7e) % 10 > 4) as i32 + 1;
                self.frame += 0x14a;
            }
        }
        // LAB_1088_9a0d
        if self.frame < 0x1e3 || self.frame > 0x1e8 {
            self.gun.pose = 0;
        }
        let g = &mut self.gun;
        let (c, s) = (crate::trig::cos(g.aim), crate::trig::sin(g.aim));
        if g.armed && input[LEFT_SHOOT] > 0 && FIRE_OK && g.cooldown < 1 {
            g.cooldown = 10;
            let x = self.x + g.muzzle.0 + (0x17 * c) / 0x4000;
            let y = self.y + g.muzzle.1 - 0x16 - (0x17 * s) / 0x4000;
            g.shot = Some((y, x, g.aim, g.shots & 1));
            g.shots += 1;
            g.shooting = true;
        }
        // (*1170:4dee)(): the right button's weapon (table 1170:188a). With no ammo (FUN_1088_5914),
        // as at the start of a level, it does nothing; ammo pick-ups aren't ported.
        if g.cooldown > 0 {
            g.muzzle.0 -= (g.cooldown * c) / 0xc000;
            g.muzzle.1 += (g.cooldown * s) / 0xc000;
            g.cooldown -= 1;
        }
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
            self.shakes.push((0x18, 0xe));
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
        if p < 2 && self.head_blocked == 0 {
            self.stand();
        } else {
            self.crouch_down();
            self.crouch = p;
        }
        if d > 0x60 {
            self.shakes.push((8, (d / 0x10 - 4).min(8)));
        }
    }

    /// The tail of HeroUpdate after the collision: animation frames and the Random() calls.
    /// Returns false where the original skips the aiming pose (goto LAB_1088_9a0d).
    fn end_of_frame(&mut self, walking: bool, blocked: i32, step_h: i32, rng: &mut PascalRandom) -> bool {
        if self.state != GROUND {
            if self.state != CROUCH {
                self.turn = NO_TURN;
            }
            self.idle_timer = -0x10 - rng.random(8);
            return false;
        }
        if self.turn != NO_TURN {
            let t = self.turn.abs();
            self.frame = if self.turn_kind == 2 {
                0x66 - t
            } else {
                (t + 8) % 0x14 + 0x5b
            };
            self.turn += self.turn.signum();
            if self.turn.abs() < 0xb && !self.gun.shooting {
                return false;
            }
            self.turn = NO_TURN;
            self.anim = 6;
            if !self.gun.shooting {
                return false;
            }
        }
        if walking && (blocked == 0 || step_h > 0x13) {
            if blocked != 0 && blocked.signum() == self.facing {
                // Pushing against a wall.
                if self.push_timer == 0 {
                    self.anim = 0x1c;
                }
                self.anim = (self.anim + 1) % 0x54;
                self.frame = self.anim / 7 + 0x227;
                self.gun.shooting = false;
                self.gun.armed = true;
                self.gun.muzzle = (0, 0);
                self.aim_at_mouse();
                self.vx = self.facing * 7;
                self.push_timer = 2;
            } else {
                if self.push_timer > 0 {
                    self.anim = 6;
                    self.vx = 0;
                }
                self.frame = self.anim / 2 + 0x81;
                self.anim = (self.anim + 1) % 0x28;
            }
            self.idle_timer = -0x10 - rng.random(8);
            return true;
        }
        // Standing: ease the walk cycle back to a standing pose, then idle.
        if self.push_timer > 0 {
            self.anim = 6;
            self.vx = 0;
        }
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
            return true;
        }
        if self.frame != NONE {
            return true;
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
        true
    }

    /// HeroMoveAndCollide (1088:3322) with HeroFindFloor (1088:304c): platforms first, then
    /// tiles, horizontally; then the ceiling or the floor. Returns (gHeroBlockedDX 4e9c = what
    /// of dx was refused, the height of what blocked it 4ea6).
    fn move_and_collide(&mut self, w: &World, lv: &Level, mut dx: i32, dy: i32) -> (i32, i32) {
        let (h, hw) = (self.height, self.half_width);
        let (x0, y0, dx_req) = (self.x, self.y, dx);
        let head = (y0 - h) + 1;
        let mut step_h = 0;
        let wall = |plane: usize, y: i32, x: i32| {
            let s = w.solidity_in(plane, y, x);
            s != OUTSIDE && s & solid::WALL != 0
        };
        // Platforms.
        let (right, left, rows) = (x0 + dx + hw, x0 + dx - hw, (h + 15) >> 4);
        for p in lv.platforms.iter().rev().flatten() {
            if !overlaps(p, y0, right, y0 - 0x29, left) {
                continue;
            }
            let (lx, rx) = (left - p.x, right - p.x);
            let mut ly = head - p.prev_y;
            let (mut hit_left, mut hit_right) = (false, false);
            let mut k = rows;
            loop {
                if wall(p.plane, ly, lx) {
                    dx = ((lx & !15) + 16 + hw + p.x) - x0;
                    hit_left = true;
                }
                if wall(p.plane, ly, rx) {
                    hit_right = true;
                    break;
                }
                if hit_left {
                    step_h = (y0 - p.prev_y) - ly;
                    break;
                }
                ly = if k == 1 { y0 - p.prev_y } else { ly + 16 };
                if k == 0 {
                    break;
                }
                k -= 1;
            }
            if hit_right {
                if hit_left {
                    dx = dx_req;
                    if wall(p.plane, (y0 - (h >> 1)) - p.prev_y, (x0 + dx) - p.x) {
                        self.crushed = true; // FUN_1088_3c32(0x8e)
                    }
                } else {
                    dx = (((rx & !15) - 1) - hw + p.x) - x0;
                }
                step_h = (y0 - p.prev_y) - ly;
            }
            if wall(p.plane, (y0 - 0x29) - p.prev_y, x0 - p.x) {
                self.head_blocked = 2;
            }
        }
        // Tiles.
        if dx != 0 {
            let s = dx.signum();
            let edge = x0 + dx + s * hw;
            let mut k = (h + 15) >> 4;
            let mut ly = head;
            let mut hit = false;
            loop {
                if w.solidity_at(ly, edge) & solid::WALL != 0 {
                    hit = true;
                    break;
                }
                ly = if k == 1 { y0 } else { ly + 16 };
                if k == 0 {
                    break;
                }
                k -= 1;
            }
            if hit {
                step_h = y0 - ly;
                self.x = ((edge & !15) + if s == -1 { 15 } else { 0 }) - (hw + 1) * s;
            } else {
                self.x += dx;
            }
        }
        let blocked = dx_req - (self.x - x0);
        self.ceiling = 0;
        if dy < 0 {
            let new_head = head + dy;
            let mut y = y0 + dy;
            let cols = (hw * 2 + 15) >> 4;
            let row_hit = |plane: usize, ry: i32, xl: i32, xr: i32| {
                let (mut cx, mut k, mut any) = (xl, cols, false);
                loop {
                    if w.solidity_in(plane, ry, cx) & solid::WALL != 0 {
                        any = true;
                    }
                    cx = if k == 1 { xr } else { cx + 16 };
                    if k == 0 {
                        break any;
                    }
                    k -= 1;
                }
            };
            if (((new_head - dy) + 16) as u32 >> 4) != ((new_head + 16) as u32 >> 4)
                && row_hit(MAIN_PLANE, new_head, self.x - hw, self.x + hw)
            {
                let v = (new_head & !15) + 0xf + h;
                if v >= y {
                    y = v;
                    self.ceiling = -1;
                }
            }
            for p in lv.platforms.iter().rev().flatten() {
                if overlaps(p, new_head, self.x + hw, new_head, self.x - hw)
                    && row_hit(
                        p.plane,
                        new_head - p.y,
                        self.x - hw - p.x,
                        self.x + hw - p.x,
                    )
                {
                    let v = ((new_head - p.y) & !15) + 0xf + h + p.y;
                    if v >= y {
                        y = v;
                        self.ceiling = -1;
                    }
                }
            }
            self.y = y;
        } else {
            let (floor, carry) = self.find_floor(w, lv, self.x - x0, y0, y0 + dy);
            if floor == NONE {
                self.y = y0 + dy;
            } else {
                self.y = floor;
                if dy > 0 {
                    self.ceiling = 1;
                }
                if blocked.signum() != carry.signum() {
                    self.x += carry;
                }
            }
        }
        (blocked, step_h)
    }

    /// HeroFindFloor (1088:304c, see re/rng.md): the highest floor in the row under y_new.
    /// Returns (new y or NONE, the carrying platform's dx).
    fn find_floor(&mut self, w: &World, lv: &Level, dx: i32, y_old: i32, y_new: i32) -> (i32, i32) {
        let hw = self.half_width;
        let x = self.x;
        let e = if self.ground <= 9 && self.state <= 0 {
            2
        } else {
            0
        };
        let cols = (hw * 2 + 15) >> 4;
        let _ = dx; // tolerance |dx| only applies right after riding an object platform (4ea4)
        let mut best = NONE;
        let mut carry = 0;
        let mut ground = self.ground;
        // One column test (FUN_1088_2cff + 2c97). `offset` and `pdy` are 0 for the map.
        let mut column = |func0: u16,
                          sol: u16,
                          sy: i32,
                          sx_abs: (usize, i32),
                          offset: i32,
                          pdy: i32,
                          best: &mut i32,
                          ground: &mut i32|
         -> bool {
            let solid_floor = sol & solid::HERO_FLOOR != 0;
            let ladder_top = func0 == 10
                && self.state != -5
                && (sol & 0x10 != 0 || w.tile_function_in(sx_abs.0, sy - 6, sx_abs.1) != 10);
            if !(solid_floor || ladder_top) {
                return false;
            }
            let cand = ((sy & !15) - 1) + offset;
            let moving_down = pdy < 0;
            let found = if !moving_down || y_old != cand {
                cand <= *best && ranges_overlap(cand, cand + pdy, y_old - e - 1, y_new + e)
            } else {
                true
            };
            if found {
                *best = cand;
                *ground = if !solid_floor && *ground > 9 {
                    0x14
                } else if func0 == 11000 {
                    5
                } else if *ground != 5 {
                    9
                } else {
                    *ground
                };
            }
            found
        };
        let sy = y_new + 1;
        let (mut sx, mut k) = (x - hw, cols);
        loop {
            let func0 = w.tile_function_at(sy, sx);
            let sol = w.solidity_in(2, sy, sx) | w.solidity_in(MAIN_PLANE, sy, sx);
            column(
                func0,
                sol,
                sy,
                (MAIN_PLANE, sx),
                0,
                0,
                &mut best,
                &mut ground,
            );
            sx = if k == 1 { x + hw } else { sx + 16 };
            if k == 0 {
                break;
            }
            k -= 1;
        }
        for p in lv.platforms.iter().rev().flatten() {
            if !overlaps(p, y_new + 16, x + hw, y_new, x - hw) {
                continue;
            }
            let b = p.bounds;
            if !(b.0 <= x + hw && b.1 <= y_new + 16 && x - hw <= b.2 && y_new <= b.3) {
                continue;
            }
            let pdy = p.prev_y - p.y;
            let tol = if pdy < 0 { 1 - pdy } else { 1 };
            let mut sy = (y_new - p.y) + 1;
            if self.state != -4 {
                sy += tol;
            }
            let (mut sx, mut k) = ((x - hw) - p.x, cols);
            loop {
                let func0 = w.tile_function_in(p.plane, sy, sx);
                if func0 != OUTSIDE {
                    let sol = w.solidity_in(p.plane, sy, sx);
                    if column(
                        func0,
                        sol,
                        sy,
                        (p.plane, sx),
                        p.y,
                        pdy,
                        &mut best,
                        &mut ground,
                    ) {
                        carry = p.x - p.prev_x;
                    }
                }
                sx = if k == 1 { (x + hw) - p.x } else { sx + 16 };
                if k == 0 {
                    break;
                }
                k -= 1;
            }
        }
        if best != NONE {
            self.ground = ground;
        }
        (best, carry)
    }
}

/// FUN_10f0_1317(t, 1, div, n): a ping-pong frame index 0..n-1.
fn anim_pingpong(t: i32, div: i32, n: i32) -> i32 {
    let v = ((t / div) % (n * 2 - 2) - n) + 1;
    (n - 1) - v.abs()
}

/// FUN_1078_0198: does the platform's rectangle meet [left, right] x [top, bottom]?
fn overlaps(p: &Platform, bottom: i32, right: i32, top: i32, left: i32) -> bool {
    !(right < p.x || bottom < p.y || p.x + p.width <= left || p.y + p.height <= top)
}

/// FUN_1118_02ac.
fn ranges_overlap(a: i32, b: i32, c: i32, d: i32) -> bool {
    let (lo1, hi1) = (a.min(b), a.max(b));
    let (lo2, hi2) = (c.min(d), c.max(d));
    hi1 >= lo2 && lo1 <= hi2
}

impl Hero {
    /// FUN_1078_0f1a: solidity of the main plane OR'ed with every platform covering the point.
    pub fn solidity_all(w: &World, lv: &Level, y: i32, x: i32) -> u16 {
        let mut s = w.solidity_at(y, x);
        for p in lv.platforms.iter().rev().flatten() {
            if x >= p.x && y >= p.y && x < p.x + p.width && y < p.y + p.height {
                s |= w.solidity_in(p.plane, y - p.y, x - p.x);
            }
        }
        s
    }

    /// FUN_1088_3dad: room above the ledge to pull up into.
    fn room_to_climb(&self, w: &World, lv: &Level) -> bool {
        let x2 = if self.facing < 1 {
            self.x - 7
        } else {
            self.x + 7
        };
        Self::solidity_all(w, lv, self.hang_y - 0x38, self.x) & solid::WALL == 0
            && Self::solidity_all(w, lv, self.hang_y - 0x38, x2) & solid::WALL == 0
    }

    /// FUN_1088_3e0d: while falling and pushing towards a wall, catch a ledge (solidity bit 4).
    /// Returns the frame's (dx, dy) (it zeroes dy when it grabs).
    fn grab_ledge(&mut self, w: &World, lv: &Level, y: i32, dx: i32, dy: i32) -> (i32, i32) {
        if self.state == LEDGE {
            return (dx, dy);
        }
        let probe_x = if self.facing < 1 {
            if self.input_cache[LEFT] < 1 || !self.released[LEFT] {
                return (dx, dy);
            }
            (self.x - self.half_width) - 1
        } else {
            if self.input_cache[RIGHT] < 1 || !self.released[RIGHT] {
                return (dx, dy);
            }
            self.x + self.half_width + 1
        };
        self.hang_y = 0x7fff;
        let py = y + 8;
        let mut hx = 0;
        if w.solidity_at(py, probe_x) & 4 != 0 {
            self.hang_y = (py & !15) + 0x20;
            hx = if self.facing < 1 {
                (probe_x & !15) + 0x16
            } else {
                (probe_x & !15) - 7
            };
        } else {
            for p in lv.platforms.iter().rev().flatten() {
                if p.prev_x <= probe_x
                    && p.y <= py
                    && probe_x < p.prev_x + p.width
                    && py < p.y + p.height
                {
                    if w.solidity_in(p.plane, py - p.y, probe_x - p.prev_x) & 4 != 0 {
                        self.hang_y = ((py - p.y) & !15) + 0x20 + p.y;
                        let b = ((probe_x - p.prev_x) & !15) + p.x;
                        hx = if self.facing < 1 { b + 0x16 } else { b - 7 };
                    }
                }
            }
        }
        if self.hang_y == 0x7fff
            || Self::solidity_all(w, lv, y, self.facing * 8 + probe_x) & solid::WALL != 0
            || Self::solidity_all(w, lv, y + 0x30, self.x) & solid::HERO_FLOOR != 0
        {
            return (dx, dy);
        }
        let free_below = Self::solidity_all(w, lv, y + 0x3c, self.x) & solid::HERO_FLOOR == 0;
        if !(free_below || self.room_to_climb(w, lv)) {
            return (dx, dy);
        }
        if self.state > 0 {
            self.shakes.push((8, 3));
        }
        self.vy = ON_GROUND_VY;
        self.state = LEDGE;
        self.cam_nudge.1 += self.hang_y - self.y;
        self.y = self.hang_y;
        self.x = hx;
        self.half_width = 6;
        self.released[UP] = false;
        self.vx = 0;
        if free_below {
            self.climb = 0;
            if self.fall < 0x40 {
                self.released[UP] = true;
            } else {
                self.shakes.push((0xc, 6));
            }
            self.anim = 0x37;
        } else {
            self.climb = 5;
            self.y -= 4;
        }
        (dx, 0)
    }

    /// The gHeroState == -4 block of HeroUpdate: hang, pull up, or let go.
    fn hang(
        &mut self,
        w: &World,
        lv: &Level,
        input: &[i8; 9],
        mut dx: i32,
        mut dy: i32,
    ) -> (i32, i32) {
        let mut hx = if self.facing < 1 {
            self.x - 7
        } else {
            self.x + 7
        };
        self.height = 0x20;
        // FUN_1088_4116: ride a moving platform's ledge.
        let py = (self.hang_y - 0x21) + 4;
        let probe = self.facing * 2 + hx;
        for p in lv.platforms.iter().rev().flatten() {
            if probe >= p.x && py >= p.y && probe < p.x + p.width && py < p.y + p.height {
                let s = w.solidity_in(p.plane, py - p.y, probe - p.x);
                if s != OUTSIDE && s & 4 != 0 {
                    dy = (((py - p.y) & !15) - 1 + p.y) - (self.hang_y - 0x21);
                    dx = p.x - p.prev_x;
                    self.hang_y += dy;
                }
            }
        }
        let wall_here = Self::solidity_all(w, lv, self.hang_y - 0x21, hx);
        hx += dx;
        let y_next = self.y + dy;
        if self.climb == 0 {
            let back = self.facing * -0x20;
            let open = (0..2).all(|k| {
                Self::solidity_all(w, lv, k * 16 + self.hang_y - 8, hx) & solid::WALL == 0
                    && Self::solidity_all(w, lv, k * 16 + self.hang_y - 8, hx + back) & solid::WALL
                        == 0
            });
            self.frame = if open {
                anim_pingpong(self.anim, 5, 0xb) + 0x21
            } else {
                0x2d
            };
            self.anim += 1;
            if input[UP] < 1 || !self.released[UP] || !self.room_to_climb(w, lv) {
                let limit = if wall_here & 0x20 == 0 {
                    let l = if open { 0x14f } else { 0x497 };
                    if l - 0x28 == self.anim {
                        dy += 1;
                    }
                    l
                } else {
                    if self.anim == 0x41 {
                        dy += 1;
                    }
                    0x50
                };
                let away = input[if self.facing < 0 { RIGHT } else { LEFT }];
                if input[DOWN] == 1
                    || away > 0
                    || y_next < self.hang_y
                    || self.hang_y + 1 < y_next
                    || limit < self.anim
                {
                    self.start_jump(1, FALL);
                    self.state_before_jump = 5;
                    self.released[if self.facing > 0 { RIGHT } else { LEFT }] = false;
                }
            } else {
                self.climb = 1;
                self.released[UP] = false;
                self.y = self.hang_y - dy;
            }
        } else if self.climb < 0x1c {
            self.frame = self.climb / 2 + 0xf1;
            dy -= 1;
            self.climb += 1;
        } else {
            dx = self.facing * 0xc;
            self.y -= 6;
            self.cam_nudge.0 += dx;
            self.cam_nudge.1 -= 6;
            self.ground = 9;
            self.crouch_down();
            self.crouch = 8;
            self.frame = 0xa8;
            self.released[UP] = false;
        }
        if self.state == LEDGE {
            if wall_here & solid::WALL != 0
                || Self::solidity_all(w, lv, self.hang_y - 0x11, hx) & 4 == 0
            {
                self.start_jump(1, FALL);
                self.state_before_jump = 5;
            }
        }
        (dx, dy)
    }

    /// HeroPushOutOfWalls (1088:2a78): if a wall overlaps the body, find the nearer free side
    /// and move the hero 1 px towards it when his centre is in the wall, else nudge next frame.
    fn push_out_of_walls(&mut self, w: &World) {
        let (h, hw) = (self.height, self.half_width);
        let cols = (hw * 2 + 15) >> 4;
        // No platform support yet: gHeroPlatform is always -1 here.
        let (mut ly, rows) = (self.y, (h + 15) >> 4);
        for e in 0..=rows {
            let mut lx = self.x - hw;
            for c in 0..=cols {
                if w.solidity_at(ly, lx) & solid::WALL != 0 {
                    let dir = if lx > self.x {
                        -1
                    } else {
                        let mut step = 16;
                        loop {
                            let r = w.solidity_at(ly, lx + step);
                            if r & solid::WALL == 0 {
                                break 1;
                            }
                            if r == crate::world::OUTSIDE {
                                break -1;
                            }
                            let l = w.solidity_at(ly, lx - step);
                            if l & solid::WALL == 0 {
                                break -1;
                            }
                            if l == crate::world::OUTSIDE {
                                break 1;
                            }
                            step += 16;
                        }
                    };
                    if w.solidity_at(ly, self.x) & solid::WALL != 0 {
                        self.x += dir;
                    } else {
                        self.nudge = dir;
                    }
                    return;
                }
                lx = if c == cols - 1 { self.x + hw } else { lx + 16 };
            }
            ly = if e == rows - 1 {
                self.y - h + 1
            } else {
                ly - 16
            };
        }
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
