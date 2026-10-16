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
/// Pulling a lever (HeroUseObject).
pub const USE: i32 = -30;
/// Walking into an exit (function 9000 tiles).
pub const EXIT: i32 = -100;
/// Tile function of exits (doors to another place, paired by the teleport records).
const EXIT_TILE: u16 = 9000;
/// Thrown back by a hit (FUN_1088_3bee).
pub const KNOCKED_BACK: i32 = 0x10;
/// Dead (FUN_1088_3c32).
pub const DEAD: i32 = 100;
/// 1170:4e6c at the start of a level.
const HEALTH: i32 = 0x2c6;
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
    /// gHeroPlatform (4e22): the slope tile's function 2 (its row in 1170:1280, 0x80 mirrored)
    /// the feet are on, -1 none.
    slope: i32,
    collide_flags: i32,
    /// gHeroPrevX/Y: where this frame's move started.
    prev: (i32, i32),
    /// FUN_1088_72f9's call this frame: (dir, y, x) of the wall pushed.
    pub push: Option<(i32, i32, i32)>,
    /// 1170:4dec: the right button's weapon (-1 none yet; its procedure 4dee is table 188a).
    pub weapon: i32,
    /// 1170:1656: the weapons.
    pub weapons: [Weapon; 10],
    /// Pick-ups taken this frame: (plane-2 cell (tx, ty), effect, its function 3, y, x).
    pub pickups: Vec<((i32, i32), u16, u16, i32, i32)>,
    /// 1170:4e62: counts down to the teleport (FUN_1088_517a) at the end of the frame it hits 0.
    pub exit_timer: i32,
    /// 1170:4e1e: the exit's kind (its function 2, low byte): 0 a door, 1 a hatch.
    exit_kind: i32,
    /// 1170:5123: 3 while a lever is in reach this frame (then 1: Up jumps again).
    lever_reach: i32,
    /// 1170:4e5a: frames the use pose has been held (to 2); the lever moves at 1.
    use_count: i32,
    /// 1170:4dfa: the use pose's frame when the lever is high up.
    use_frame: i32,
    /// 1170:4e3c: a spring lever held down (its cell), -1 none.
    held_lever: i32,
    /// The lever operations this frame, for the level: (cell, y, x, pull).
    pub levers: Vec<(i32, i32, i32, bool)>,
    /// 1170:4e6c.
    pub health: i32,
    /// 1170:5104..: the keys picked up (pick-up effect 70).
    pub keys: [bool; 16],
    /// 1170:4e2a: frames of invulnerability after a hit; 4e5e the same for the blinking.
    hurt: i32,
    hurt_blink: i32,
    /// 1170:4660: what hit the hero this frame (25aa of the hit test), cleared each frame.
    hit_mask: i32,
    /// 1170:511f: HeroStartJump's first argument (set when thrown back): falls fast even holding Up.
    knocked: bool,
    /// 1170:4de6: knocked down flat (after being thrown back) rather than crumpled (a fall).
    flat: bool,
    /// 1170:511b: on a slope, set by FUN_1088_2f6b; walls are then looked for higher up.
    on_slope: bool,
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
    /// FUN_1088_64fe's grenade this frame: (y, x, angle) for FUN_1040_621f.
    pub grenade: Option<(i32, i32, i32)>,
}

/// A weapon record (1170:1656 + 0xd * id).
#[derive(Clone, Copy, Debug)]
pub struct Weapon {
    pub pickup: i32, // +0: ammo per pick-up
    pub max: i32,    // +2
    pub next: u32,   // +6: the frame (1170:617e) it may fire again
    pub ammo: i32,   // +0xa
    pub owned: bool, // +0xc
}

/// The weapons' (ammo per pick-up, max) at load.
const WEAPON_AMMO: [(i32, i32); 10] =
    [(6, 99), (400, 799), (50, 249), (10, 99), (10, 99), (10, 99), (50, 249), (5, 99), (10, 99), (5, 99)];
/// 1170:162c: the weapon for each digit key.
const WEAPON_FOR_DIGIT: [i32; 10] = [2, 6, 1, 5, 3, 8, 0, 4, 7, 9];
/// Tile function of the pick-ups (in plane 2).
const PICKUP: u16 = 30000;
/// The effect of a weapon pick-up (FUN_1040_0312).
pub const WEAPON_PICKUP: u16 = 67;

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
            weapon: -1,
            weapons: WEAPON_AMMO.map(|(pickup, max)| Weapon { pickup, max, next: 0, ammo: 0, owned: false }),
            pickups: Vec::new(),
            exit_timer: 0,
            exit_kind: 0,
            slope: -1,
            collide_flags: 3,
            prev: (x, y),
            push: None,
            lever_reach: 0,
            use_count: 0,
            use_frame: 0x7fff,
            held_lever: -1,
            levers: Vec::new(),
            health: HEALTH,
            keys: [false; 16],
            hurt: 0,
            hurt_blink: 0,
            hit_mask: 0,
            knocked: false,
            flat: false,
            on_slope: false,
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
        self.knocked = false;
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

    /// The globals the objects read.
    pub fn view(&self, up: bool) -> crate::actors::HeroView {
        crate::actors::HeroView {
            x: self.x,
            y: self.y,
            prev_x: self.prev.0,
            prev_y: self.prev.1,
            height: self.height,
            state: self.state,
            vy: self.vy,
            fall: self.fall,
            up,
            hurt: self.hurt,
            hit_mask: self.hit_mask,
        }
    }

    /// HeroStartJump(0, vy, kind) from outside HeroUpdate.
    pub fn bounce(&mut self, vy: i32, kind: i32) {
        self.start_jump(vy, kind);
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
        ps: &mut crate::particles::Particles,
        rng: &mut PascalRandom,
    ) {
        // ReadGameInput: moving the mouse raises the gun.
        self.gun.shot = None;
        self.gun.grenade = None;
        self.push = None;
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
        self.hit_mask = 0;
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
        if self.hurt < 1 {
            self.hurt_blink = 0;
        } else {
            self.hurt -= 1;
            if self.hurt_blink > 0 {
                self.hurt_blink -= 1;
            }
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
            if self.flat {
                self.frame = 0x20b;
                self.height = 0x14;
            } else {
                self.frame = (self.anim / 0x1e) % 2 + 0x208;
                self.height = 0x1a;
            }
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

        // Levers in reach (6000..6099 cells not marked 2), pulled on the use pose's first frame.
        self.levers.clear();
        let mut ly = (self.y & !15) - 0x28;
        for k in (0..=2).rev() {
            let f = w.tile_function_at(ly, self.x);
            if (6000..0x17d4).contains(&f) {
                let cell = w.cell_at(MAIN_PLANE, ly, self.x).unwrap_or(0);
                let f2 = w.cell_function(cell, 2);
                if f2 & 2 == 0 {
                    self.lever_reach = 3;
                    if self.use_count == 1 {
                        if f2 & 4 != 0 {
                            eprintln!("warning: levers needing a key (HeroUpdate, flag 4) not ported");
                        }
                        let idx = (ly >> 4) * w.main().width + (self.x >> 4);
                        if f2 & 1 != 0 {
                            self.held_lever = idx;
                        }
                        self.levers.push((idx, ly, (self.x & !15) + 7, true));
                    }
                    if k == 2 {
                        self.use_frame = 0x122;
                    }
                }
            }
            ly += 0x10;
        }
        if self.held_lever != -1 && self.use_count < 1 {
            self.levers.push((self.held_lever, 0, 0, false));
            self.held_lever = -1;
        }

        // Exits: function 2's high byte 0x10 leaves at once; 0 when Up is held standing (the
        // use pose first, walking to the exit's middle, then into it).
        let e = exit_at(w, true, self.y, self.x);
        if (e >> 8) & 0xff == 0x10 && self.exit_timer < 1 {
            self.exit_timer = 2;
        } else if (e >> 8) & 0xff == 0
            && held(UP)
            && (self.state == GROUND || self.state == USE)
            && lv.door_state(w, self.y, self.x) != 0
        {
            if self.use_count == 1 {
                self.state = EXIT;
                self.anim = 0;
                self.exit_kind = e & 0xff;
            } else {
                self.lever_reach = 2;
            }
            if self.state == USE {
                self.x += (exit_middle(w, self.y, self.x) + (self.facing == -1) as i32 - self.x).signum();
            }
        }
        if self.state == EXIT {
            let len = match self.exit_kind {
                0 => {
                    if self.anim < 0x28 {
                        self.frame = self.anim / 5 + 0x300;
                    }
                    0x28
                }
                1 => {
                    if self.anim < 0x20 {
                        self.frame = self.anim / 4 + 0x30e;
                    }
                    self.facing = 1;
                    self.height = 0x18;
                    0x20
                }
                _ => 0,
            };
            if len - 1 == self.anim {
                self.exit_timer = 2;
            }
            // (len - anim == 0x1d: 1170:536a = -0x1e starts the fade.)
            self.turn = NO_TURN;
            self.anim += 1;
            self.x += (exit_middle(w, self.y, self.x) + (self.facing == -1) as i32 - self.x).signum();
            self.hurt = 1;
        }

        if held(UP) && self.released[UP] && self.lever_reach >= 2 {
            // HeroUseObject(0, 5123).
            let kind = self.lever_reach & 0xfe;
            if self.state == GROUND || (self.state == USE && kind != self.turn_kind) {
                self.state = USE;
                self.turn_kind = kind;
                self.turn = if kind == 2 { -11 } else { 2 };
                self.use_count = 0;
                self.released[UP] = false;
            }
        } else if held(UP) && self.released[UP] && self.state == GROUND {
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
                // Out of a crouch on a slope marked 1: kind 7 (a roll).
                let kind = if self.prev_state == CROUCH && self.slope != -1 && slope_word(self.slope, 0x14) & 1 != 0 {
                    7
                } else {
                    JUMP
                };
                self.start_jump(vy, kind);
                self.released[UP] = false;
            }
        }

        if self.state == GROUND && (held(LEFT) || held(RIGHT)) {
            self.facing = if held(LEFT) { -1 } else { 1 };
            // Up the steep slopes (row word 0x12 == 0xe7) slower.
            let limit = if self.on_slope && slope_word(self.slope, 0x12) == 0xe7 && self.push_timer < 1 {
                3 + (self.vx.abs() == 3) as i32
            } else {
                WALK_LIMIT
            };
            self.vx = (self.vx + WALK_STEP * self.facing).clamp(-limit, limit);
            dx += (self.vx.abs() / 4 + 1) * self.vx.signum();
            if self.vx.signum() == self.facing {
                friction = false;
            }
            walking = true;
        }

        if self.slope != -1 {
            let flags = slope_word(self.slope, 0x14);
            if flags & 2 != 0 {
                eprintln!("warning: slippery slopes (HeroUpdate, row flag 2) not ported");
            }
            if flags & 1 != 0 {
                if !walking {
                    self.vx = 0;
                }
                if self.vx == 0 && dx == 0 && ((((self.slope & 0x80) != 0) as i32 + self.x) & 7) != 0 {
                    dx = self.facing;
                }
            }
        }

        // Walked off an edge.
        if self.ground == 0 && self.state < 1 && matches!(self.state, GROUND | CROUCH) {
            self.start_jump(1, FALL);
        }

        if self.state == USE {
            if self.turn.abs() != 7 {
                self.turn += 1;
            }
            if self.turn.abs() == 7 && self.use_count < 2 {
                self.use_count += 1;
            }
            if self.frame == NONE {
                if self.use_count < 1 || self.use_frame == 0x7fff {
                    let t = self.turn.abs();
                    self.frame = if self.turn_kind == 2 { 0x66 - t } else { (0x16 - t) % 0x14 + 0x5b };
                    self.use_frame = 0x7fff;
                } else {
                    self.frame = self.use_frame;
                }
            }
            if !held(UP) || self.lever_reach == 0 {
                self.stand();
            }
            self.gun.shooting = false;
        } else {
            self.use_count = 0;
        }

        if self.state == GROUND && self.facing != prev_facing && !self.gun.shooting {
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
            self.vy += if !held(UP) && !self.knocked { 10 } else { 3 };
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
                if self.gun.shooting {
                    self.gun.muzzle = (if self.facing < 1 { 5 } else { 0 }, 6);
                    self.frame = 0x1f2;
                    self.aim_fixed();
                }
            } else if self.state == KNOCKED_BACK {
                bleed(ps, rng, self.anim, self.y - 0x1e, self.x);
                self.frame = (self.anim / 2 + 0x201).min(0x204);
                self.anim += 1;
                self.height = 0x27;
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
            // Only the hanging branch (not the pull-up) sets local_1e = 1.
            let hanging = self.climb == 0;
            let (ndx, ndy) = self.hang(w, lv, &input, dx, dy);
            (dx, dy) = (ndx, ndy);
            if hanging {
                collide_flags = 1;
            }
        }
        self.collide_flags = collide_flags;

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
        self.lever_reach &= 1;
        let (blocked, step_h) = self.move_and_collide(w, lv, dx, dy);
        self.prev_blocked = blocked;
        self.push_out_of_walls(w);
        let posed = self.end_of_frame(walking, blocked, step_h, rng);
        self.gun_end(posed, &input, rng, ps.now);
        self.pick_up(w);
        self.take_hits(ps, rng);
    }

    /// FUN_1088_55f0's end: shots against the body's and the arm's sprites (mask 0x3a2), the
    /// worse one counts (FUN_1088_3c32).
    fn take_hits(&mut self, ps: &mut crate::particles::Particles, rng: &mut PascalRandom) {
        let flip = self.facing;
        let d1 = if self.frame != NONE {
            ps.hit_test_sprite((self.frame, self.x, self.y, flip, 0), 0x3a2)
        } else {
            0
        };
        let mask1 = ps.hit_mask;
        let mut damage = d1;
        if self.gun.frame2 != NONE {
            let d2 = ps.hit_test_sprite((self.gun.frame2, self.x, self.y, flip, 0), 0x3a2);
            if d2 < 0 || d2 <= d1 {
                ps.hit_mask = mask1;
            } else {
                damage = d2;
            }
        }
        if damage != 0 {
            self.hit_mask = ps.hit_mask;
            let kind = if ps.hit_mask & 0x20 != 0 { 4 } else { 1 };
            self.hit(damage, kind, rng);
        }
    }

    /// FUN_1088_3c32(damage) with 1170:162a = `kind` (1 shot, 4 burnt).
    fn hit(&mut self, damage: i32, kind: i32, rng: &mut PascalRandom) {
        if self.hurt >= 1 {
            return;
        }
        self.health -= damage;
        if self.health < 1 {
            rng.random(0x20); // FUN_1088_3c05: the cry's pitch
            eprintln!("warning: the hero's death (FUN_1068_186c, splats) not ported");
            rng.random(5);
            self.hurt = 0x7fff;
            self.anim = 0x8c;
            self.state = DEAD;
            return;
        }
        self.hurt = 0x52;
        match kind {
            1 => {
                rng.random(0x20);
                self.throw_back();
                self.shakes.push((0x10, 8));
                if damage > 0x4f && rng.random(3) != 0 {
                    rng.random(5);
                    eprintln!("warning: blood splats (FUN_1058_0218) not ported");
                }
            }
            4 => {
                rng.random(0x20);
                self.hurt = 1;
                if self.state != KNOCKED_BACK {
                    self.throw_back();
                }
                self.shakes.push((0x10, 8));
            }
            k => eprintln!("warning: hero hit kind {k} not ported"),
        }
        self.hurt_blink = self.hurt;
    }

    /// FUN_1088_3bee: thrown back unless the head is against a ceiling.
    fn throw_back(&mut self) {
        if self.head_blocked == 0 {
            self.start_jump(-0x28, KNOCKED_BACK);
            self.knocked = true;
        }
    }

    /// The end of HeroUpdate: the aiming pose, the left button's shot, the recoil.
    fn gun_end(&mut self, posed: bool, input: &[i8; 9], rng: &mut PascalRandom, now: u32) {
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
        // (*1170:4dee)(): the right button's weapon (table 1170:188a).
        self.right_weapon(input, rng, now);
        let g = &mut self.gun;
        if g.cooldown > 0 {
            g.muzzle.0 -= (g.cooldown * c) / 0xc000;
            g.muzzle.1 += (g.cooldown * s) / 0xc000;
            g.cooldown -= 1;
        }
    }

    /// FUN_1088_517a, the hero's part: from the exit's left end to the other end of its
    /// teleport record (records 1000: byte 2 two-way, byte 3 leaves the level, words 2 and 3
    /// the cells). Returns true when he moved (LevelFirstFrame follows).
    pub fn teleport(&mut self, w: &World, teleports: &[Vec<i16>]) -> bool {
        while exit_at(w, false, self.y, self.x - 0x10) != -1 {
            self.x -= 0x10;
        }
        let width = w.main().width;
        let cell = (self.y / 16) * width + self.x / 16;
        let mut moved = false;
        for r in teleports.iter().rev() {
            let (two_way, leave) = (r[1] & 0xff != 0, (r[1] >> 8) & 0xff);
            let (a, b) = (r[2] as u16 as i32, r.get(3).map_or(0, |&v| v as u16 as i32));
            if cell != a && !(two_way && cell == b) {
                continue;
            }
            if leave != 0 {
                eprintln!("warning: a level exit (teleport record leaving the level) not ported");
                continue;
            }
            let to = if cell == a { b } else { a };
            self.y = (to / width) * 0x10 + 0xf;
            self.x = exit_middle(w, self.y, (to % width) << 4) + (self.facing == -1) as i32;
            // FUN_1088_0c20: there, with the camera snapped (LevelFirstFrame redoes it).
            self.prev = (self.x, self.y);
            self.stand();
            self.frame = 0x26f;
            self.gun.shooting = false;
            if w.solidity_at(self.y - 0x20, self.x) & 2 != 0 {
                self.crouch_down();
                self.crouch = 8;
                self.frame = 0xa8;
            }
            moved = true;
        }
        moved
    }

    /// The start of FUN_10a8_0dab: walking into an exit, the door module draws him.
    pub fn hide_in_exit(&mut self) {
        if self.state == EXIT && self.frame != NONE {
            self.frame = NONE;
            self.gun.frame2 = NONE;
        }
    }

    /// (*1170:4dee)(): the selected weapon's procedure.
    fn right_weapon(&mut self, input: &[i8; 9], rng: &mut PascalRandom, now: u32) {
        match self.weapon {
            -1 => {}
            // FUN_1088_64fe: a grenade, thrown 0x50 either side of the aim.
            0 => {
                if self.use_ammo(0xc, input, now) {
                    let (y, x) = (self.y - 0x1a, self.x);
                    let a = (self.gun.aim + 0x1680 - 0x50 + rng.random(0xa1)) % 0x1680;
                    self.gun.grenade = Some((y, x, a));
                    self.gun.pose = 4;
                    self.gun.shooting = true;
                    self.released[RIGHT_SHOOT] = false;
                }
            }
            w => eprintln!("warning: weapon {w} (1088:{:04x}) not ported", [0x64fe, 0x678b, 0x69d8, 0x6b1f, 0x6f4d, 0x6deb, 0x70b8, 0x655f, 0x6cf0, 0x6649][w as usize]),
        }
    }

    /// FUN_1088_5914(delay): the selected weapon may fire (ammo, armed, the right button
    /// pressed anew, its pause over); takes one round and sets the pause.
    fn use_ammo(&mut self, delay: u32, input: &[i8; 9], now: u32) -> bool {
        let w = &mut self.weapons[self.weapon as usize];
        if w.ammo < 1 || !self.gun.armed || input[RIGHT_SHOOT] < 1 || !self.released[RIGHT_SHOOT] {
            return false;
        }
        if w.next > now {
            return false;
        }
        w.next = now.wrapping_add(delay);
        w.ammo -= 1;
        if w.ammo < 1 {
            self.next_weapon();
        }
        true
    }

    /// FUN_1088_58c1: out of ammo, the next weapon by digit key that has some.
    fn next_weapon(&mut self) {
        let mut d = (0..10).rev().find(|&d| WEAPON_FOR_DIGIT[d] == self.weapon).unwrap_or(0);
        for _ in 0..10 {
            d = (d + 1) % 10;
            if self.weapons[WEAPON_FOR_DIGIT[d] as usize].ammo > 0 {
                self.select_weapon(WEAPON_FOR_DIGIT[d]);
                self.released[RIGHT_SHOOT] = false;
                return;
            }
        }
    }

    /// SelectWeapon (1088:71ef). (4e32 = -14 starts the weapon display, 4e4a keeps the aim.)
    pub fn select_weapon(&mut self, w: i32) {
        self.weapon = w;
    }

    /// The start of FUN_1088_55f0: pick-ups (function 30000 in plane 2) under the hero's two
    /// columns (x-4, x+4) from his feet to his head (FUN_1088_5551). Each is cleared and its
    /// effect (function 4) runs at the tile's centre; main applies both.
    fn pick_up(&mut self, w: &World) {
        let rows = (self.height + 7) >> 4;
        let (feet, head) = (self.y - 4, (self.y - self.height) + 5);
        let mut y = feet;
        for r in (0..=rows).rev() {
            for x in [self.x - 4, self.x + 4] {
                if (8000..8100).contains(&w.tile_function_at(y, x)) {
                    eprintln!("warning: FUN_1088_55bd (tile functions 8000..8099) not ported");
                }
                let Some(cell) = w.cell_at(2, y, x) else {
                    continue;
                };
                let t = (x >> 4, y >> 4);
                if self.pickups.iter().any(|p| p.0 == t) || w.cell_function(cell, 0) != PICKUP {
                    continue;
                }
                let (fx, f3) = (w.cell_function(cell, 4), w.cell_function(cell, 3));
                self.pickups.push((t, fx, f3, (y & !15) + 7, (x & !15) + 7));
            }
            y = if r != 1 { y - 0x10 } else { head };
        }
    }

    /// FUN_1040_0312 (effect 67) for the hero's side: a weapon (function 3 - 0x1e) and its ammo.
    pub fn weapon_pickup(&mut self, f3: u16) {
        let i = f3 as i32 - 0x1e;
        let w = &mut self.weapons[i as usize];
        let select = if !w.owned {
            w.owned = true; // and HeroDamage(f3 - 0x3f): score
            true
        } else {
            w.ammo == 0
        };
        if select {
            self.select_weapon(i);
        }
        let w = &mut self.weapons[i as usize];
        w.ammo = (w.ammo + w.pickup).min(w.max);
    }

    /// The landing block at the top of HeroUpdate.
    fn land(&mut self, w: &World, rng: &mut PascalRandom) {
        let d = self.fall;
        if d > 0x1f {
            let n = rng.random((d / 16 + 1).min(8) as u16);
            spawn_dust(w, n, self.y, self.x, rng);
        }
        if d >= FATAL_FALL || self.prev_state == KNOCKED_BACK {
            self.state = KNOCKED_DOWN;
            if d < FATAL_FALL {
                self.anim = 0x14;
                self.flat = true;
            } else {
                self.anim = ((d - FATAL_FALL) >> 1) + 100;
                self.anim = self.anim.min(0xaa);
                self.shakes.push((0x18, 0xe));
                self.flat = false;
            }
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
            if self.state != CROUCH && self.state != USE {
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
                // Pushing against a wall (a block may give way).
                self.push = Some((self.facing, self.y - step_h, (self.half_width + 1) * self.facing + self.x));
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
        self.prev = (x0, y0);
        let head = (y0 - h) + 1;
        let prev_slope = std::mem::replace(&mut self.slope, -1); // 1170:4ea4
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
            // On a slope the feet are below the tile edge: start the wall test higher.
            let lift = if self.on_slope { dx.abs() + hw } else { 0 };
            let mut k = (h + 15 - lift) >> 4;
            let mut ly = head;
            let mut hit = false;
            loop {
                if w.solidity_at(ly, edge) & solid::WALL != 0 {
                    hit = true;
                    break;
                }
                ly = if k == 1 { y0 - lift } else { ly + 16 };
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
            let (floor, carry) = self.find_floor(w, lv, self.x - x0, y0, y0 + dy, prev_slope);
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
        if self.slope == -1 {
            self.on_slope = false;
        }
        (blocked, step_h)
    }

    /// HeroFindFloor (1088:304c, see re/rng.md): the highest floor in the row under y_new.
    /// Returns (new y or NONE, the carrying platform's dx).
    fn find_floor(
        &mut self,
        w: &World,
        lv: &Level,
        dx: i32,
        y_old: i32,
        y_new: i32,
        prev_slope: i32,
    ) -> (i32, i32) {
        if std::env::var("ET_FLOOR").is_ok() {
            eprintln!("FLOOR x {} dx {dx} y_old {y_old} y_new {y_new} prev_slope {prev_slope} flags {} state {} ground {} hw {}", self.x, self.collide_flags, self.state, self.ground, self.half_width);
        }
        let hw = self.half_width;
        let x = self.x;
        let e = if self.ground <= 9 && self.state <= 0 {
            2
        } else {
            0
        };
        let cols = (hw * 2 + 15) >> 4;
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
        // Coming off a slope, look |dx| deeper.
        let deeper = if self.collide_flags & 2 != 0 && prev_slope != -1 { dx.abs() } else { 1 };
        let sy = y_new + deeper;
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
        if self.collide_flags & 2 == 0 {
            return (best, carry);
        }
        // Slopes (solidity 0x20) at the feet - 8 and + 8: they win over a floor while walking.
        let state = if self.state == -2 { 1 } else { self.state };
        let adx = dx.abs();
        let mut ly = y_new - 8;
        for _ in 0..2 {
            if w.solidity_at(ly, x) & 0x20 != 0 {
                let cell = w.cell_at(MAIN_PLANE, ly, x).unwrap_or(0);
                let (kind, height) = self.slope_probe(w, cell, x, dx);
                let h = (ly & !15) + height;
                let ok = if state < 1 {
                    if self.on_slope && dx != 0 {
                        h <= y_new && y_old - adx <= h
                    } else {
                        y_old - adx + (adx > 0) as i32 <= h && h <= y_new + hw
                    }
                } else {
                    h <= best && y_old - adx <= h && h <= y_new + e
                };
                if ok {
                    self.ground = 1;
                    self.slope = kind;
                    return (h, carry);
                }
            }
            ly += 16;
        }
        (best, carry)
    }

    /// FUN_1088_2f6b: the slope's height at x (1170:1280 row by function 2, mirrored with 0x80),
    /// and whether the hero is on it going its way (1170:511b; byte 0x10 of the row is the split).
    fn slope_probe(&mut self, w: &World, cell: u16, x: i32, dx: i32) -> (i32, i32) {
        let kind = w.cell_function(cell, 2) as i16 as i32;
        let row = &crate::world::SLOPES[((kind as u16 & 0xff7f) as usize) * 0x16..];
        let mut i = x & 0xf;
        let split = row[0x10] as i32;
        self.on_slope = if kind & 0x80 != 0 {
            i = 0xf - i;
            dx == 0 || ((split > i) != (dx > 0))
        } else {
            dx == 0 || ((split > i) != (dx < 0))
        };
        (kind, row[i as usize] as i32)
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
        let (mut ly, rows) = if self.slope == -1 {
            (self.y, (h + 15) >> 4)
        } else {
            (self.y - hw, (h + 15 - hw) >> 4)
        };
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
/// SpawnDust for the other modules.
pub fn spawn_dust_pub(w: &World, count: i32, y: i32, x: i32, rng: &mut PascalRandom) {
    spawn_dust(w, count, y, x, rng);
}

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

/// FUN_1088_5082(door, y, x): an exit's function 2 (main plane, else plane 2), -1 for none or
/// (with `door`) a closed door (function 7050 whose function 2 isn't 1) in front of it.
fn exit_at(w: &World, door: bool, y: i32, x: i32) -> i32 {
    if w.tile_function_at(y, x) == EXIT_TILE {
        let cell = w.cell_at(MAIN_PLANE, y, x).unwrap_or(0);
        return w.cell_function(cell, 2) as i32;
    }
    if w.tile_function_in(2, y, x) != EXIT_TILE {
        return -1;
    }
    let cell = w.cell_at(2, y, x).unwrap_or(0);
    let mut r = w.cell_function(cell, 2) as i32;
    if door && w.tile_function_at(y, x) == 7050 {
        let main = w.cell_at(MAIN_PLANE, y, x).unwrap_or(0);
        if w.cell_function(main, 2) != 1 {
            r = -1;
        }
    }
    r
}

/// FUN_1088_510f(y, x): the middle of the exit around x (its 16 px cells left and right).
fn exit_middle(w: &World, y: i32, x: i32) -> i32 {
    let mut x0 = x & 0xfff0;
    let mut width: i8 = 0x10;
    while exit_at(w, false, y, width as i32 + x0) != -1 {
        width = width.wrapping_add(0x10);
    }
    while exit_at(w, false, y, x0 - 0x10) != -1 {
        x0 -= 0x10;
        width = width.wrapping_add(0x10);
    }
    (x0 - 1) + width as i32 / 2
}

/// A word of the slope table row (1170:1280 + 0x16 * (kind & 0xff7f)): 0x12 its shape id,
/// 0x14 flags (1 stop on it, 2 slippery, 8 sliding dust).
fn slope_word(kind: i32, off: usize) -> i32 {
    let row = &crate::world::SLOPES[((kind as u16 & 0xff7f) as usize) * 0x16..];
    i16::from_le_bytes([row[off], row[off + 1]]) as i32
}

/// FUN_1088_1f41(y, x) while thrown back: blood puffs, and at frame 12 a spray of drops.
fn bleed(ps: &mut crate::particles::Particles, rng: &mut PascalRandom, anim: i32, y: i32, x: i32) {
    let puff = |ps: &mut crate::particles::Particles, y: i32, x: i32| {
        // FUN_1040_62b7.
        ps.spawn_seq(0x5300, 4, 5, y, x, 1);
    };
    match anim {
        0 => puff(ps, y, x),
        5 => puff(ps, y - 10, x - 5),
        10 => puff(ps, y + 8, x + 4),
        0xc => {
            ps.p.group = 0;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x16;
            ps.p.callback = 0x23b7;
            for i in 1..=3 {
                let speed = (i << 5) / 3;
                ps.p.delay = rng.random(5);
                let s = rng.random(2) + 0x5307;
                ps.spawn(s, y - 5, x);
                let a = (rng.random(0xb1) + 0xb40) - i * 0xf0;
                ps.throw(7, speed, a);
                ps.p.delay = rng.random(5);
                let s = rng.random(2) * 2 + 0x5306;
                ps.spawn(s, y - 5, x);
                let a = ((0x1680 - rng.random(0xb1)) + i * 0xf0) % 0x1680;
                ps.throw(7, speed, a);
            }
            ps.p.callback = 0;
            ps.p.on_death = 0;
            ps.p.hits = 0;
            ps.p.group = 1;
            ps.p.delay = 0;
        }
        0xf => puff(ps, y + 1, x + 1),
        _ => {}
    }
}
