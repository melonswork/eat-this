//! Leon: HeroUpdate (ETLAUNCH 1088:7592) in the original's order, for the states ported so far:
//! standing, walking, turning, idling, jumping, falling, crouching, landing and being knocked down.
//! Every Random() call the original makes in those states is made here too, in the same order,
//! because the recorded demos check RandSeed (see re/demo.md). Ladders, pipes, ledges, slopes,
//! platforms, levers, doors and weapons are not ported yet.

use crate::level::{Level, Platform};
use crate::rng::PascalRandom;
use crate::sound;
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
pub const LADDER: i32 = -5;
/// Held by the tank's claw (FUN_1070_b9d3).
pub const HELD: i32 = -3;
/// Hanging from a ceiling pipe (60800) and climbing up out of a pipe (60600).
pub const PIPE_HANG: i32 = -1;
pub const PIPE_CLIMB: i32 = -2;
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
    pub lim: (i32, i32),   // 1170:4e16 / 4e18
    prev_ground: i32,  // 1170:4e2e
    belt_dir: i32,     // 1170:4e08
    belt: i32,         // 1170:4ea0: the conveyor's speed under him
    climb: i32,        // 1170:4e0e: 0 hanging, 1..0x1b pulling up
    push_timer: i32,   // 1170:4e12 (gHeroShootWalkTimer): frames since pushing a wall
    prev_blocked: i32, // 1170:4e64: last frame's gHeroBlockedDX
    /// 1170:4e9e after the collision: -1 hit a ceiling, 1 landed (read next frame as 4e66).
    ceiling: i32,
    head_blocked: i32, // 1170:4e68
    pub crushed: bool,
    /// gHeroDX/DY that objects set after FUN_1088_532c (which runs right after HeroUpdate): the
    /// next HeroUpdate starts from them (the meat hooks, the tank).
    pub carry: (i32, i32),
    /// Sensitive-lever tiles touched this frame (HeroTouchTiles), as (ty, tx), for the level.
    pub touched: Vec<(i32, i32)>, // 1170:4e06: HeroPushOutOfWalls' push, becomes next frame's starting dx (FUN_1088_532c)
    released: [bool; 9],
    input_cache: [i8; 9],
    pub height: i32,
    pub half_width: i32,
    /// ScreenShake calls made this frame, in order: (strength, amplitude).
    pub shakes: Vec<(i32, i32)>,
    /// Sideways shakes (FUN_1050_5c7d) made this frame: (strength, amplitude).
    pub side_shakes: Vec<(i32, i32)>,
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
    /// 1170:4e30 / 4e32: the status bar's health length (follows the health one pixel a frame)
    /// and the weapon icon's slide-in offset (-14 on a weapon change, +2 a frame); FUN_1088_2501
    /// updates them every frame (`status_bar`). Only the HUD reads them.
    pub hud_bar: i32,
    pub hud_slide: i32,
    /// HeroDamage(id) calls this frame (1088:1109: message `id` of ETMAIN for the message box;
    /// main moves them to `Game::messages`); true = FUN_1088_10e1, only if not the last line).
    pub messages: Vec<(i32, bool)>,
    /// 1170:1656: the weapons.
    pub weapons: [Weapon; 10],
    /// Pick-ups taken this frame: (plane-2 cell (tx, ty), effect, its function 3, y, x).
    pub pickups: Vec<((i32, i32), u16, u16, i32, i32)>,
    /// 1170:4e62: counts down to the teleport (FUN_1088_517a) at the end of the frame it hits 0.
    pub exit_timer: i32,
    /// 1170:4e60: what GameFrame does after this frame (10 restart the level, 100 + n the level
    /// is over through a teleport record that leaves it, 0x1e back to the menu).
    pub level_end: i32,
    /// 1170:4e1e: the exit's kind (its function 2, low byte): 0 a door, 1 a hatch.
    exit_kind: i32,
    /// 1170:5123: 3 while a lever is in reach this frame (then 1: Up jumps again).
    lever_reach: i32,
    /// 1170:4e20: frames Down has been held while crouching still (to 0x10).
    down_timer: i32,
    /// 1170:4e24: > 0 while sliding down a slope on his belly.
    sliding: i32,
    /// 1170:4e5a: frames the use pose has been held (to 2); the lever moves at 1.
    use_count: i32,
    /// 1170:4dfa: the use pose's frame when the lever is high up.
    use_frame: i32,
    /// 1170:4e3c: a spring lever held down (its cell), -1 none.
    held_lever: i32,
    /// The lever operations this frame, for the level: (cell, y, x, pull).
    pub levers: Vec<(i32, i32, i32, bool)>,
    /// Blood splats the hero's hits ask for (FUN_1058_0218): (sprite, y, x).
    pub splats: Vec<(i32, i32, i32)>,
    pub death: Option<(i32, i32)>,
    /// The hero rides a vehicle (1170:5127 == 2, OBJECTS 0x271a): HeroUpdateVehicle.
    pub vehicle: bool,
    /// The vehicle's speeds (4e86, 4e88), sub-pixel remainders (4e8a, 4e8c), bump counters
    /// (4e8e, 4e90) and the next time it may fire (4e94).
    pub veh: Veh,
    /// The vehicle fired this frame (1170:465e = 2).
    pub veh_fired: bool,
    /// 1170:4e70: frames of the START intro's launch left (> 100: he is carried over the ship).
    pub launch: i32,
    /// 1170:4e9a: the launch's centre x. 4e96/4e98: the ship's position.
    pub launch_x0: i32,
    pub ship: (i32, i32),
    /// Cells whose scripts a keyless lever runs, and key slots a key opens (FUN_1088_13dc).
    pub script_cells: Vec<u16>,
    pub cell_steps: Vec<u16>,
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

impl Hero {
    /// 1170:4e5e, for the draw pass (the hero blinks while it is > 0).
    pub fn hurt_blink(&self) -> i32 {
        self.hurt_blink
    }
}

impl Gun {
    /// 1170:4e56, for the draw pass (the gun glows while it is > 0).
    pub fn cooldown(&self) -> i32 {
        self.cooldown
    }
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
    /// FUN_1088_6f4d's charge: (y, x, angle).
    pub charge: Option<(i32, i32, i32)>,
    /// FUN_1088_655f's shell: (y, x, angle).
    pub shell7: Option<(i32, i32, i32)>,
    /// FUN_1088_70b8's three bullets: (muzzle y, muzzle x, angle).
    pub spread: Option<(i32, i32, i32)>,
    /// FUN_1088_6649's ball: (y, x, angle).
    pub ball9: Option<(i32, i32, i32)>,
    /// FUN_1088_69d8's bullet: (y, x, angle, parity).
    pub rapid: Option<(i32, i32, i32, i32)>,
    /// FUN_1088_6cf0's three rockets: (y, x, angle).
    pub rockets: Option<(i32, i32, i32)>,
    /// FUN_1088_6deb's accelerating rocket: (y, x, angle).
    pub rocket5: Option<(i32, i32, i32)>,
    /// Weapon 3's homing rocket (y, x, angle): FUN_1088_6b1f.
    pub rocket3: Option<(i32, i32, i32)>,
    /// FUN_1088_678b's flame: (y, x, angle).
    pub flame: Option<(i32, i32, i32)>,
    flame_aim: i32, // 4e4a: the aim the flame follows
    /// 1170:4dbe: the flame's roar plays again at this level frame (sound only).
    flame_sound: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Veh {
    pub vx: i32,
    pub vy: i32,
    pub rx: i32,
    pub ry: i32,
    pub bump_x: i32,
    pub bump_y: i32,
    pub next_shot: u32,
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
pub const WEAPON_FOR_DIGIT: [i32; 10] = [2, 6, 1, 5, 3, 8, 0, 4, 7, 9];
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
            // HeroReset's Random(8) = 4 and Random(0x1a4) = 380 (the boot seed is the same in every run).
            idle_timer: -0x10 - 4,
            idle_next: 0xd2 + 380,
            ground: FLAT_FLOOR,
            nudge: 0,
            hang_x: 0,
            hang_y: 0x7fff,
            belt: 0,
            prev_ground: 0,
            lim: (-0x7fff, 0x7fff),
            belt_dir: 0,
            climb: 0,
            push_timer: 0,
            prev_blocked: 0,
            ceiling: 0,
            head_blocked: 0,
            crushed: false,
            carry: (0, 0),
            touched: Vec::new(),
            released: [true; 9],
            input_cache: [0; 9],
            height: HEIGHT,
            half_width: 6,
            shakes: Vec::new(),
            side_shakes: Vec::new(),
            cam_nudge: (0, 0),
            gun: Gun::default(),
            // HeroReset (FUN_1088_06b8) ends with SelectWeapon(gWeaponForDigitKey[0]).
            weapon: WEAPON_FOR_DIGIT[0],
            hud_bar: 0,
            hud_slide: -14,
            messages: Vec::new(),
            weapons: WEAPON_AMMO.map(|(pickup, max)| Weapon { pickup, max, next: 0, ammo: 0, owned: false }),
            pickups: Vec::new(),
            exit_timer: 0,
            level_end: 0,
            exit_kind: 0,
            slope: -1,
            collide_flags: 3,
            prev: (x, y),
            push: None,
            lever_reach: 0,
            down_timer: 0,
            sliding: 0,
            use_count: 0,
            use_frame: 0x7fff,
            held_lever: -1,
            levers: Vec::new(),
            splats: Vec::new(),
            death: None,
            vehicle: false,
            veh: Veh::default(),
            veh_fired: false,
            launch: 0,
            launch_x0: 0,
            ship: (0, 0),
            script_cells: Vec::new(),
            cell_steps: Vec::new(),
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
            self.down_timer = 0;
        }
        self.sliding = 0;
    }

    /// FUN_1070_00b6: lifted onto a moving top.
    pub fn stand_on(&mut self, y: i32) {
        self.y = y;
        self.ground = 0x16;
    }

    /// gHeroGround set by an object (0x1414 carrying him: 0x16).
    pub fn set_ground(&mut self, g: i32) {
        self.ground = g;
    }

    pub fn ground(&self) -> i32 {
        self.ground
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
            facing: self.facing,
            half_width: self.half_width,
            up,
            hurt: self.hurt,
            hit_mask: self.hit_mask,
        }
    }

    /// A springboard (FUN_1070_d23b) under him: the jump (kind <= 0: none), standing on it.
    pub fn spring(&mut self, vy: i32, kind: i32, y: i32, release: bool) {
        if release {
            self.released[UP] = false;
        }
        if kind > 0 {
            self.start_jump(vy, kind);
        }
        self.ground = 0x17;
        self.y = y;
    }

    /// HeroStartJump(0, vy, kind) from outside HeroUpdate.
    pub fn bounce(&mut self, vy: i32, kind: i32) {
        self.start_jump(vy, kind);
    }

    pub fn gun_mouse(&self) -> (i32, i32) {
        self.gun.mouse
    }

    pub fn set_mouse(&mut self, m: (i32, i32)) {
        self.gun.mouse = m;
    }

    /// HeroAimAtMouse (1088:420b).
    fn aim_at_mouse(&mut self) {
        let g = &mut self.gun;
        if std::env::var("ET_AIM").is_ok() {
            eprintln!("AIM hero {},{} muzzle {:?} mouse {:?} cam {:?}", self.x, self.y, g.muzzle, g.mouse, g.cam);
        }
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
        plat_objs: &mut [crate::platobj::PlatObj],
    ) {
        // ReadGameInput: moving the mouse raises the gun.
        self.gun.shot = None;
        self.gun.grenade = None;
        self.gun.charge = None;
        self.gun.shell7 = None;
        self.gun.ball9 = None;
        self.gun.spread = None;
        self.gun.rapid = None;
        self.gun.rockets = None;
        self.gun.rocket5 = None;
        self.gun.rocket3 = None;
        self.gun.flame = None;
        self.push = None;
        if std::env::var_os("ET_MOUSE").is_some() { eprintln!("MOUSE {} {:?} {:?} cam {:?}", ps.now, mouse, self.gun.mouse, cam); }
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
        if input[8] == 1 {
            // ReadGameInput: the switch-weapon key selects the next weapon by digit key.
            let at = (0..10).rev().find(|&d| WEAPON_FOR_DIGIT[d] == self.weapon).unwrap_or(0);
            self.select_weapon(WEAPON_FOR_DIGIT[(at + 1) % 10]);
        }
        if std::env::var("ET_W").is_ok() {
            eprintln!("WEAPON {} {} sw {}", ps.now, self.weapon, input[8]);
        }
        let prev_facing = self.facing;
        self.frame = NONE;
        self.prev_state = self.state;
        self.half_width = 6;
        self.height = HEIGHT - (self.ground == 1) as i32;
        self.shakes.clear();
        self.side_shakes.clear();
        self.cam_nudge = (0, 0);
        self.hit_mask = 0;
        let mut friction = true;
        let mut walking = false;
        let (cx, cy) = std::mem::take(&mut self.carry);
        let (mut dx, mut dy) = (std::mem::take(&mut self.nudge) + cx, cy);
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
        if self.vehicle {
            self.update_vehicle(w, lv, &input, mouse, cam, ps, rng);
            return;
        }
        let cleared = std::cell::Cell::new(usize::MAX);
        let held = |k: usize| input[k] > 0 && cleared.get() != k;

        if self.state == DEAD {
            // HeroUpdate's state 100: the death timer runs out into a HeroDamage call.
            if self.anim > 0 {
                self.anim -= 1;
                if self.anim == 0 {
                    let r = rng.random(6);
                    self.messages.push((-0xe - r, false));
                }
            }
            // Inputs 6 or 7 (or Space/Enter) just pressed: restart the level.
            if input[6] == 1 || input[7] == 1 {
                self.level_end = 10;
            }
            self.run_platforms(w, lv, &input, cam, ps, rng, plat_objs, 0, 0);
            return;
        }
        // A ceiling right above the head forces a crouch.
        if w.solidity_at(self.y - 0x2b, self.x) & solid::WALL != 0 {
            self.head_blocked = 1;
        }
        if self.head_blocked > 0 && self.state == GROUND {
            self.crouch_down();
        }

        // Landing: the previous frame's collision found a floor.
        if self.ground > 0 && self.vy >= 0 && self.state >= 0 {
            self.vy = ON_GROUND_VY;
            if self.state > 0 {
                self.land(w, rng);
            }
        }

        // Ladder tiles (function 10) along the body: their kind (param 2) and flags (param 3).
        let (mut ladder_kind, mut ladder_flags) = (-1i32, 0i32);
        for k in 0..=((self.height - 1) >> 4) {
            let ty = self.y - k * 16;
            if w.tile_function_at(ty, self.x) == 10 {
                let cell = w.cell_at(MAIN_PLANE, ty, self.x).unwrap_or(0);
                ladder_kind = w.cell_function(cell, 2) as i16 as i32;
                let f3 = w.cell_function(cell, 3) as i32;
                ladder_flags = if k == 0 { f3 } else { ladder_flags | f3 };
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
                if self.slope != -1 && slope_word(self.slope, 0x12) != 0x7fff {
                    self.sliding = 1;
                    self.facing = if self.slope & 0x80 == 0 { -1 } else { 1 };
                }
            }
            if self.anim < 1 {
                self.crouch_down();
                self.crouch = 8;
            } else {
                self.anim -= 1;
            }
            if self.sliding > 0 {
                if self.slope == -1 || slope_word(self.slope, 0x12) == 0x7fff {
                    self.sliding = 0;
                    if self.vx.abs() < 0x14 {
                        self.vx = if self.vx < 0 { -0x14 } else { 0x14 };
                    }
                } else {
                    self.vx = (self.vx + (self.facing << 1)).clamp(-0x1d, 0x1d);
                    dx += (self.vx.abs() / 10 + 1) * self.vx.signum();
                    friction = false;
                }
            }
        }

        // Grabbing a ladder (HeroGrabLadder): Up on the way in, Down from above.
        let mut down_ladder = false;
        if self.state >= 0 || matches!(self.state, LADDER | USE | CROUCH) {
            if ladder_kind == -1 || !held(UP) || self.state == LADDER {
                if held(DOWN) && w.tile_function_at(self.y + 1, self.x) == 10 {
                    if ladder_kind == -1 {
                        let cell = w.cell_at(MAIN_PLANE, self.y + 1, self.x).unwrap_or(0);
                        ladder_kind = w.cell_function(cell, 2) as i16 as i32;
                        ladder_flags = w.cell_function(cell, 3) as i32;
                    }
                    down_ladder = true;
                    if self.state != LADDER
                        && (self.ground == 0
                            || (self.state == CROUCH && 8 - (self.head_blocked == 0) as i32 == self.crouch)
                            || w.tile_function_at(self.y - 0x10, self.x) == 10)
                    {
                        self.state = LADDER;
                        self.anim = 9;
                    }
                }
            } else if self.ground == 0 || self.use_count == 1 {
                self.state = LADDER;
                self.anim = 9;
            } else {
                self.lever_reach = 2;
                self.released[UP] = true;
            }
        }

        if held(DOWN) && self.state == GROUND {
            self.crouch_down();
        }

        if self.state == CROUCH {
            self.gun.shooting = false;
            if self.crouch < 8 {
                self.crouch = (self.crouch + 1).abs();
            } else {
                if self.ground != 0x15 {
                    self.half_width = 9;
                }
                // Down held long enough on a slope: slide down it (or through an exit).
                let dirn = if self.slope & 0x80 == 0 { -1 } else { 1 };
                if self.sliding == 0 {
                    if self.slope != -1
                        && self.down_timer < 1
                        && held(DOWN)
                        && self.released[DOWN]
                        && slope_run(w, 3, dirn, self.y, self.x) > 2
                        && slope_word(self.slope, 0x12) != 0x7fff
                    {
                        self.sliding = 1;
                        self.facing = dirn;
                        self.down_timer = 0;
                        sound::play_at(0x58, self.y, self.x);
                    }
                    if !held(DOWN) {
                        self.down_timer = 0;
                    } else if self.down_timer < 0x10 {
                        self.down_timer += 1;
                    } else {
                        let e = exit_at(w, true, self.y, self.x);
                        if (e >> 8) & 0xff == 1 {
                            self.state = EXIT;
                            self.crouch = 0;
                            self.exit_kind = e & 0xff;
                            self.frame = 0xa8;
                            self.anim = 0;
                        }
                    }
                }
            }
            if self.state == CROUCH && self.sliding > 0 {
                let dirbit = self.slope != -1 && self.slope & 0x80 != 0;
                let ceiling_ok = w.tile_function_at(self.y - 0x10, self.x) != 7110;
                if self.slope == -1
                    || slope_word(self.slope, 0x12) == 0x7fff
                    || (self.head_blocked == 0 && ceiling_ok && (input[LEFT] == 1 || input[RIGHT] == 1 || input[UP] == 1))
                    || self.prev_blocked != 0
                    || ((self.facing < 0) != !dirbit)
                {
                    self.sliding = 0;
                    self.crouch = 7;
                    self.frame = 0xa8;
                    self.released[DOWN] = false;
                    if self.vx.abs() < 0x14 {
                        self.vx = if self.vx < 0 { -0x14 } else { 0x14 };
                    }
                } else {
                    if self.sliding % 4 == 2 {
                        ps.p.colour = 0x10;
                        ps.p.group = 0;
                        ps.spawn_seq(0x638, 2, 0xc, self.y, self.x, 1);
                        ps.p.group = 1;
                        ps.p.colour = 0;
                    }
                    self.sliding += 1;
                    self.frame = slope_word(self.slope, 0x12);
                    self.vx = (self.vx + (self.facing << 1)).clamp(-0x1d, 0x1d);
                    dx += (self.vx.abs() / 10 + 1) * self.vx.signum();
                    friction = false;
                }
                self.height = 0x16;
                self.half_width = 8;
            }
            if self.state == CROUCH && self.sliding < 1 {
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
                        self.down_timer = 1;
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
        }

        if self.state == LADDER {
            if ladder_kind == -1 || (self.ceiling > 0 && !down_ladder) {
                self.released[UP] = false;
                if w.tile_function_at(self.y + 2, self.x) == 10 {
                    dy = ((self.y & !15) + 0xf) - self.y;
                    self.ground = 9;
                    self.crouch_down();
                    self.crouch = (self.head_blocked != 0) as i32 + 6;
                    self.frame = 0xa8;
                    self.height = 0x18;
                } else {
                    self.stand();
                }
            } else {
                if !held(UP) {
                    if held(DOWN) {
                        dy += 2;
                        self.anim -= 1;
                    }
                } else if !held(DOWN) && ladder_flags & 2 == 0 {
                    dy -= 2;
                    if self.ceiling < 0 {
                        self.anim &= !1;
                    } else {
                        self.anim += 1;
                    }
                }
                if ladder_flags & 1 == 0 && (self.ground != 9 || dy < 1) {
                    if !held(LEFT) {
                        if held(RIGHT) {
                            dx += 1;
                        }
                    } else {
                        dx -= 1;
                    }
                }
                if ladder_kind == 0 {
                    self.anim = (self.anim + 0x20) % 0x20;
                    self.frame = self.anim / 2 + 0x3a;
                } else if ladder_kind == 1 || ladder_kind == 9 {
                    self.anim = (self.anim + 0x20) % 0x20;
                    self.frame = 200 + 8 - ((self.anim / 2) % 16 - 8).abs();
                }
                self.height = 0x29;
                if w.tile_function_at(self.y + 1, self.x) == 10 {
                    let mut k = 5;
                    while w.tile_function_at(self.y - (k + 1) * 6, self.x) != 10 {
                        self.frame = 0x11d - k;
                        if self.frame == 0x118 {
                            self.frame = 0x119;
                        }
                        self.anim = 9;
                        self.height = (6 - k) * -3 + 0x29;
                        if k == 0 {
                            break;
                        }
                        k -= 1;
                    }
                }
                if dx == 0 {
                    dx = (((self.x & !15) + 7 + (self.facing > 0) as i32) - self.x).signum();
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
                    let mut skip = false;
                    if self.use_count == 1 && f2 & 4 != 0 {
                        // A lever with a key slot above it.
                        let above = w.tile_function_at(ly - 0x10, self.x) as i32;
                        let key = ((above - 13980) as usize) & 0xf;
                        let width = w.main().width;
                        if !self.keys[key] {
                            if w.cell_at(MAIN_PLANE, ly - 0x20, self.x) == Some(0x308) {
                                self.script_cells.push((((ly - 0x20) >> 4) * width + (self.x >> 4)) as u16);
                            } else {
                                // THIS POD NEEDS A ... KEYCARD (FUN_1088_10e1).
                                self.messages.push((-1 - (above - 14000), true));
                            }
                            skip = true;
                        } else {
                            self.messages.push((-20, false)); // - UNLOCKED -
                            sound::play_at(0x91, ly, (self.x & !15) + 7);
                            self.keys[key] = false;
                            self.cell_steps.push((((ly - 0x10) >> 4) * width + (self.x >> 4)) as u16);
                        }
                    }
                    if self.use_count == 1 && !skip {
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
        if std::env::var_os("ET_EXIT").is_some() && held(UP) {
            eprintln!("EXIT at {},{} e {e:#x} door {} state {} use {}", self.x, self.y, lv.door_state(w, self.y, self.x), self.state, self.use_count);
        }
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
                // Slippery: slides downhill, shedding dust (every 16 frames a 1-in-4 chance with
                // flag 8, else whenever the previous state was > 0).
                let (dir, angle) = if self.slope & 0x80 == 0 { (-1, 0xb40) } else { (1, 0) };
                dx += dir;
                if !walking {
                    // HeroAccelerate(limit 8, dir)
                    self.vx += dir;
                    if self.vx.abs() > 8 {
                        self.vx = self.vx.signum() * 8;
                    }
                    friction = false;
                }
                let dust = if flags & 8 != 0 && ps.now % 0x10 == 0 && rng.random(4) == 0 {
                    true
                } else {
                    self.prev_state >= 1
                };
                if dust {
                    ps.p.group = 0;
                    ps.p.hits = 0x8000;
                    ps.p.on_death = 0x10;
                    let s = rng.random(2);
                    ps.spawn(s + 0x755c, self.y - 1, self.x);
                    ps.bounce(9, 0x10, angle);
                    if rng.random(2) == 0 {
                        ps.p.delay = 5;
                        let s = rng.random(2);
                        ps.spawn(s + 0x755c, self.y, self.x);
                        ps.bounce(9, 0x10, angle);
                    }
                    ps.p.on_death = 0;
                    ps.p.hits = 0;
                    ps.p.group = 1;
                    ps.p.delay = 0;
                }
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
        // (Not while hanging or climbing: the 1170:160a set test is -state in 1..5.)
        if self.ground == 0 && self.state < 1 && !(-5..=-1).contains(&self.state) {
            self.start_jump(1, FALL);
            if self.prev_ground == 5 {
                // Off the end of a conveyor belt: thrown along its way.
                self.vx = if self.belt_dir < 0 { -0x17 } else { 0x17 };
                cleared.set(if self.belt_dir < 0 { RIGHT } else { LEFT });
            }
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

        if self.state == PIPE_CLIMB {
            self.frame = self.crouch / 3 + 0x254;
            self.height = HEIGHT - self.crouch / 3;
            if self.crouch < 0x36 {
                if self.crouch < 0x12 {
                    if w.tile_function_at(self.y - 0x2f, self.x) & 1 == 1 {
                        dx += if self.x & 0xf < 8 { -1 } else { 1 };
                    }
                } else if self.crouch % 3 == 0 {
                    dy -= 4;
                    self.cam_nudge.1 -= 3;
                }
                self.crouch += 1;
            } else {
                self.crouch_down();
                self.crouch = 8;
                self.frame = 0xa8;
                self.shakes.push((8, 4));
                sound::play_at(0x24, self.y, self.x);
                let n = rng.random(6) + 1;
                spawn_dust(w, n, self.y, self.x, rng);
            }
        }
        if self.state == PIPE_HANG {
            self.height = 0x20;
            let t = self.anim / 4 % 20;
            self.frame = 10 - (t - 10).abs() + 0x21;
            self.anim += 1;
            dy += (self.anim > 0x117) as i32 + ((self.hang_y - self.y) + 0x1d).signum();
            dx += ((self.hang_x - self.facing * 9) - self.x).signum();
            if (!held(DOWN) || held(UP))
                && w.tile_function_at(self.hang_y, self.hang_x) == 60800
                && self.anim < 0x140
            {
                if (held(UP) && self.released[UP]) || input[LEFT] == 1 || input[RIGHT] == 1 {
                    self.released[UP] = false;
                    self.start_jump(-0x32, 6);
                    self.knocked = true; // HeroStartJump(1, ..)
                    dx += self.facing * 4;
                    self.shakes.push((0xe, 4));
                }
            } else {
                self.start_jump(1, FALL);
                self.state_before_jump = 5;
            }
        }
        if self.state > 0 && self.launch > 0x64 {
            self.launch -= 1;
            let k = 0x82 - self.launch;
            self.x = self.launch_x0 - 0xa0 + (crate::trig::sin(k << 5) * 0xa0) / 0x4000;
            self.frame = NONE;
            self.ship.0 = self.x;
            if ps.now & 3 == 0 {
                self.shakes.push((0x10, 3));
            }
        } else if self.state > 0 {
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
                if self.state == 7 && self.frame > 0x105 {
                    // Somersault (a springboard throw).
                    self.frame = (self.anim >> 1) % 0x10 + 300;
                    self.anim += 1;
                    self.height = 0x1b;
                    if self.launch > 0 {
                        self.launch -= 1;
                        dx = 2;
                    }
                } else if self.state == 6 {
                    self.frame = 0x13b - (self.anim >> 1) % 0x10;
                    self.anim += 1;
                    self.height = 0x1b;
                }
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
                    sound::play_at(7, self.y, self.x);
                }
            } else {
                self.fall += self.vy / 10;
                let (ndx, ndy) = self.grab_ledge(w, lv, self.y - 0x20, dx, dy);
                (dx, dy) = (ndx, ndy);
                if held(UP) {
                    // Reaching up to a pipe.
                    let hx = self.facing * 4 + self.x;
                    let row = ((self.y - 0x20) & !15) + 7;
                    if (self.state != 1 || self.prev.1 - 0x20 < row) && w.tile_function_at(row, hx) == 60800 {
                        self.state = PIPE_HANG;
                        self.anim = 0x28;
                        self.hang_x = (hx & !15) + 7;
                        self.released[UP] = false;
                        self.hang_y = row;
                        sound::play_at(0x39, self.y, self.x);
                        sound::play_at(0x22, self.y, self.x);
                        self.shakes.push((0xc, 6));
                        return;
                    }
                    if (self.prev.1 - 0x2f) >> 4 < (self.y - 0x2f) >> 4
                        && w.tile_function_at(self.y - 0x2f, self.x) & 0xfffe == 60600
                    {
                        self.state = PIPE_CLIMB;
                        self.crouch = 0;
                        dy = (((self.y - 0x2f) & !15) + 0x2f) - self.y;
                        self.released[UP] = false;
                        self.shakes.push((8, 3));
                        sound::play_at(0x22, self.y, self.x);
                        sound::play_at(0x39, self.y, self.x);
                    }
                }
            }
            // Kicking off a wall: up and away from it while the climb is slowing (FUN_1088_753e).
            if self.state == 5 && self.vy > -0x21 && input[UP] > 0 {
                let wall = |x: i32| {
                    Self::solidity_all(w, lv, self.y - 2, x) & 2 != 0 || Self::solidity_all(w, lv, self.y - 0x12, x) & 2 != 0
                };
                if (input[LEFT] > 0 && wall(self.x + self.half_width + 5))
                    || (input[RIGHT] > 0 && wall((self.x - self.half_width) - 5))
                {
                    self.start_jump(-0x32, 7);
                    sound::play_at(0x22, self.y, self.x);
                    self.shakes.push((0xe, 4));
                }
            }
            // Falling through the water line of his sector: a splash (sound 0x6f, louder the
            // longer the fall, pitch 0xd0 + Random(0x50); FUN_1040_6ac8).
            let line = w.sectors[lv.sector.unwrap_or(0)].water[1];
            if self.prev.1 - 4 < line && line <= self.y - 4 {
                let p = rng.random(0x50);
                sound::pitch(p + 0xd0);
                sound::play_at_vol(0x6f, (self.fall * 2 + 0x80).min(0x100), line, self.x);
                ps.p.callback = 0x1d33;
                ps.p.colour = 0x34;
                ps.spawn_seq(0x6d00, 2, 0x12, line - 1, self.x, 1);
                ps.p.colour = 0;
                ps.p.callback = 0;
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

        if self.state == HELD {
            // Held by the tank's claw (FUN_1070_b9d3 sets gHeroCrouch = 1 every frame it holds him).
            let t = (ps.now as i32 / 8) % 6;
            self.frame = (if t < 4 { t } else { 6 - t }) + 0x23e;
            self.half_width = 8;
            bleed(ps, rng, self.anim, self.y - 0x1e, self.x);
            self.anim += 1;
            if self.crouch == 0 {
                self.start_jump(-0x14, KNOCKED_BACK);
                self.anim = 0x400;
            }
            self.crouch = 0;
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

        self.prev_ground = self.ground;
        self.ground = 0;
        self.head_blocked = 0;
        self.lever_reach &= 1;
        (dx, dy) = self.run_platforms(w, lv, &input, cam, ps, rng, plat_objs, dx, dy);
        // Something solid beside him (a springboard he walked into) caps the carry.
        if dx < self.lim.0 {
            dx = self.lim.0;
            self.lim.0 = -1;
        } else if self.lim.1 < dx {
            dx = self.lim.1;
            self.lim.0 = 1;
        }
        let (blocked, step_h) = self.move_and_collide(w, lv, dx, dy);
        self.prev_blocked = blocked;
        self.push_out_of_walls(w);
        if self.ground == 5 {
            // A conveyor belt (tile function 11000) carries him next frame (1170:4e06).
            self.nudge = self.belt;
            self.belt_dir = self.belt;
        }
        let posed = self.end_of_frame(walking, blocked, step_h, rng);
        self.lim = (-0x7fff, 0x7fff);
        self.gun_end(posed, &input, rng, ps.now);
        self.pick_up(w, lv, rng);
        self.take_hits(ps, rng);
    }

    /// FUN_1060_145a: the platform objects of the default sector and of his own, which carry him.
    fn run_platforms(
        &mut self,
        w: &World,
        lv: &Level,
        input: &[i8; 9],
        cam: (i32, i32),
        ps: &mut crate::particles::Particles,
        rng: &mut PascalRandom,
        plat_objs: &mut [crate::platobj::PlatObj],
        mut dx: i32,
        mut dy: i32,
    ) -> (i32, i32) {
        // FUN_1060_145a: the platform objects of the default sector and of his own, which carry him.
        {
            let window = (cam.0, cam.1, cam.0 + 0x13f, cam.1 + 199);
            let mut side = crate::platobj::HeroSide {
                x: self.x,
                y: self.y,
                dx,
                dy,
                vy: self.vy,
                state: self.state,
                half_width: self.half_width,
                prev_ground: self.prev_ground,
                ground: None,
            };
            let hero_sector = lv.sector.unwrap_or(0);
            let sectors: &[usize] = if hero_sector == 0 { &[0] } else { &[0, hero_sector] };
            for &sector in sectors {
                for p in plat_objs.iter_mut().filter(|p| p.sector == sector) {
                    let (x1, y1, x2, y2) = p.bbox;
                    if x1 <= window.2 && y1 <= window.3 && window.0 <= x2 && window.1 <= y2 {
                        if p.kind == 4 {
                            let mut released = self.released[DOWN];
                            crate::platobj::update_lift(p, &mut side, w, rng, input[DOWN] > 0, &mut released, &mut self.shakes);
                            self.released[DOWN] = released;
                        } else if p.kind == 0 {
                            crate::platobj::update_path(p, &mut side, w);
                        } else {
                            crate::platobj::update(p, &mut side, ps.now);
                        }
                    }
                }
            }
            (dx, dy) = (side.dx, side.dy);
            if let Some(g) = side.ground {
                self.ground = g;
            }
        }
        (dx, dy)
    }

    /// FUN_1088_191d (OBJECTS 0x271a): the hero rides a vehicle.
    pub fn start_vehicle(&mut self) {
        self.veh = Veh::default();
        self.select_weapon(3);
        self.vehicle = true;
    }

    /// FUN_1040_60e9(sprite, y, x): a bubble, if it is under the water (with the current group).
    fn bubble(ps: &mut crate::particles::Particles, water: i32, sprite: i32, y: i32, x: i32) {
        if water + 0x10 <= y {
            ps.p.callback = 0x30f;
            ps.spawn(sprite, y, x);
            ps.p.callback = 0;
        }
    }

    /// HeroUpdateVehicle (1088:5b62).
    fn update_vehicle(
        &mut self,
        w: &World,
        lv: &Level,
        input: &[i8; 9],
        mouse: (i32, i32),
        cam: (i32, i32),
        ps: &mut crate::particles::Particles,
        rng: &mut PascalRandom,
    ) {
        let water = lv.water;
        let now = ps.now;
        self.veh_fired = false;
        // HeroUpdateVehicle keeps the weapon icon hidden (4e32 = -16).
        self.hud_slide = -16;
        let old_facing = self.facing;
        self.half_width = 0x14;
        self.height = 0x1e;
        self.gun.aim = crate::trig::atan2(mouse.1 - ((self.y - 0xd) - cam.1), mouse.0 - (self.x - cam.0));
        self.gun.mouse = mouse;
        self.gun.cam = cam;
        let aim = self.gun.aim;
        if self.state == DEAD {
            if self.anim > 0 {
                self.anim -= 1;
                if self.anim == 0 {
                    self.messages.push((-13, false));
                }
                if self.anim > 0x9d && self.anim % 0xc == 0 {
                    let t = 0x15e - self.anim;
                    let v = rng.random(0x20);
                    let p = rng.random(0x40);
                    sound::pitch(0x100 - p);
                    sound::play_at_vol(0x47, (0x100 - t / 2) - v, self.y, self.x);
                    let px = self.x - 0x10 + rng.random(0x21);
                    let py = (self.y - 0x16 - 0x10) + rng.random(0x21);
                    ps.spawn_seq(0x7000, 2, 0x18, py, px, 1);
                    let b = rng.random(100) + t * 2 + 400;
                    let m = &mut ps.last_mut().m; // FUN_1050_4a29(b, b)
                    m.set(0x1c, b);
                    m.set(0x1e, b);
                    m.set(0x42, 0x2148);
                }
            }
            if input[6] == 1 || input[7] == 1 {
                self.level_end = 10;
            }
        } else {
            self.frame = 0x3400;
            self.vehicle_weapon(input, ps, rng, now);
            if input[LEFT_SHOOT] > 0 {
                self.veh.vx += (crate::trig::cos(aim) * 4) / 0x4000;
                self.veh.vy -= (crate::trig::sin(aim) * 4) / 0x4000;
                self.facing = if self.x - cam.0 < mouse.0 { 1 } else { -1 };
            }
            if input[LEFT] >= 1 {
                if input[RIGHT] <= 0 {
                    self.veh.vx -= 4;
                    self.facing = -1;
                }
            } else if input[RIGHT] >= 1 {
                self.veh.vx += 4;
                self.facing = 1;
            }
            if input[UP] >= 1 {
                if input[DOWN] <= 0 {
                    self.veh.vy -= 4;
                }
            } else if input[DOWN] >= 1 {
                self.veh.vy += 4;
            }
            let mut kind = 0;
            if self.prev_blocked != 0 {
                if self.veh.bump_x >= 2 {
                    kind = 1;
                    self.veh.vx = -self.veh.vx;
                }
                self.veh.bump_x = 0;
            }
            if self.ceiling != 0 {
                if self.veh.bump_y > 2 {
                    kind = 2;
                    self.veh.vy = (-self.veh.vy * 2) / 3;
                }
                self.veh.bump_y = 0;
            }
            if kind > 0 {
                if kind != 2 {
                    ps.p.group = 0;
                    let n = rng.random(4);
                    for _ in 0..n {
                        let r = rng.random(10);
                        let px = self.x + self.facing * (0x10 - r);
                        let py = (self.y - 6) - rng.random(0x10);
                        let sprite = rng.random(3) + 0x64b;
                        Self::bubble(ps, water, sprite, py, px);
                    }
                    ps.p.group = 1;
                }
                sound::play_at_vol(1, if kind == 2 { 0xaa } else { 0x100 }, self.y, self.x);
                let old = self.health;
                self.health = (self.health - 10).max(0);
                // FUN_1088_5b41: health falling under 0x8f shows WARNING - SITUATION CRITICAL.
                if old > 0x8e && self.health < 0x8f {
                    self.messages.push((-12, false));
                }
                self.shakes.push((10, 4));
            }
        }
        if self.veh.vx != 0 {
            if self.veh.vx.abs() <= 0x80 {
                self.veh.bump_x = 0;
            } else {
                self.veh.vx = self.veh.vx.signum() << 7;
                self.veh.bump_x += 1;
            }
            self.veh.rx += self.veh.vx;
            self.frame = (self.veh.vx.abs() * 5) / 0x80 + 0x3418;
            self.veh.vx -= if self.veh.vx < 0 { -1 } else { 1 };
        }
        let dx = self.veh.rx / 0x80;
        self.veh.rx %= 0x80;
        if self.veh.vy != 0 {
            if self.veh.vy.abs() <= 0x55 {
                self.veh.bump_y = 0;
            } else {
                self.veh.vy = if self.veh.vy < 0 { -1 } else { 1 } * 0x55;
                self.veh.bump_y += 1;
            }
            self.veh.ry += self.veh.vy;
            self.veh.vy -= if self.veh.vy < 0 { -1 } else { 1 };
        }
        let dy = (self.veh.ry + 0x10) / 0x80;
        self.veh.ry = (self.veh.ry + 0x10) % 0x80;
        if self.facing != old_facing {
            if self.turn == NO_TURN {
                self.turn = 1;
            } else {
                let s = if self.turn < 0 { -1 } else if self.turn < 1 { 0 } else { 1 };
                self.turn = s * 0x22 - self.turn;
            }
        }
        if self.turn != NO_TURN {
            self.frame = self.turn.abs() / 2 + 0x3403;
            self.turn += if self.turn < 0 { -1 } else { 1 };
            if self.turn.abs() > 0x21 {
                self.turn = NO_TURN;
            }
        }
        if self.state != DEAD {
            // The engine (handle 4e92), pitched down with the speed.
            sound::priority(0x13e8);
            sound::audible(self.y, self.x);
            sound::pitch(0x100 - (self.veh.vx.abs() * 0x40) / 0x80);
            sound::keep(sound::key::ENGINE, 0x29);
        }
        if self.state == DEAD {
            self.frame = 0x3401;
        } else if self.veh.vx.abs() > 100 && now % 6 == 0 {
            ps.p.colour = 0x28;
            ps.p.group = 0;
            ps.spawn_seq(0x1b0a, 3, 3, self.y - 0x19, self.x + self.facing * -0x17, 1);
            ps.p.group = 1;
            ps.p.colour = 0;
        }
        self.prev = (self.x, self.y);
        let saved = (self.state, self.collide_flags);
        self.state = 0x78;
        self.collide_flags = 3;
        let (blocked, _) = self.move_and_collide(w, lv, dx, dy);
        self.state = saved.0;
        self.collide_flags = saved.1;
        self.push_out_of_walls(w);
        self.prev_blocked = blocked;
        // The water line: he can't leave it, and breaking through it splashes.
        let prev_y = self.prev.1;
        if self.y < water + 0x12 {
            self.y = water + 0x12;
        }
        if water + 0x1c <= prev_y && self.y < water + 0x1c {
            let p = rng.random(0x50);
            sound::pitch(p + 0xd0);
            sound::play_at_vol(0x6f, 0x55, water, self.x);
        }
        if self.state != DEAD {
            let v = self.veh;
            if v.vy > 0x20 && v.bump_y > 2 && v.bump_y < 0x40 && v.bump_y % 3 == 0 {
                if v.bump_y == 3 {
                    let id = rng.random(2) + 0x40;
                    let p = rng.random(0x50);
                    sound::play_at_pitch(id as usize, p + 0xd0, self.y, self.x);
                }
                ps.p.group = 0;
                let px = (self.x - 0xc) + rng.random(0x18);
                let py = (self.y - 10) - rng.random(0x1c);
                let sprite = rng.random(3) + 0x64b;
                Self::bubble(ps, water, sprite, py, px);
                ps.p.group = 1;
            }
            if now % 0xf0 < 0x14 && now % 5 == 0 {
                if now % 0xf0 == 0 {
                    let v = rng.random(0x80);
                    let p = rng.random(0x60);
                    sound::pitch(p + 0xd0);
                    sound::play_at_vol(0x41, 0xc0 - v, self.y, self.x);
                }
                ps.p.group = 0;
                let px = (self.x - 8) + rng.random(0x10);
                let py = (self.y - 0x18) - rng.random(2);
                let sprite = rng.random(3) + 0x64b;
                Self::bubble(ps, water, sprite, py, px);
                ps.p.group = 1;
            }
            let d = ps.hit_test_sprite((self.frame, self.x, self.y, 1, 0), 0x3a2);
            if d > 0 {
                self.vehicle_hit(d, rng);
                if ps.hit_index >= 0 {
                    let m = &ps.list[ps.hit_index as usize].m;
                    let a = crate::trig::atan2(m.w(0xa) - self.y, m.w(8) - self.x);
                    self.veh.vx -= (crate::trig::cos(a) * 0x100) / 0x4000;
                    self.veh.vy += (crate::trig::sin(a) * 0xaa) / 0x4000;
                }
                self.veh.bump_x = 0;
                self.veh.bump_y = 0;
            }
        }
    }

    /// FUN_1088_598e: the right button fires a homing torpedo every 12 frames.
    fn vehicle_weapon(
        &mut self,
        input: &[i8; 9],
        ps: &mut crate::particles::Particles,
        rng: &mut PascalRandom,
        now: u32,
    ) {
        if !(input[RIGHT_SHOOT] > 0 && self.released[RIGHT_SHOOT] && self.veh.next_shot <= now) {
            return;
        }
        self.veh.next_shot = now.wrapping_add(0xc);
        let aim = self.gun.aim;
        let x = self.x + (10 * crate::trig::cos(aim)) / 0x4000;
        let y = (self.y - 0xd) - (10 * crate::trig::sin(aim)) / 0x4000;
        let a2 = (aim + 0x1680 - 0x20 + rng.random(0x41)) % 0x1680;
        ps.torpedo(y, x, a2);
        sound::play_at(0x52, y, x);
        self.veh_fired = true;
        self.released[RIGHT_SHOOT] = false;
        self.veh.vx -= (crate::trig::cos(aim) * 0x40) / 0x4000;
        self.veh.vy += (crate::trig::sin(aim) * 0x2a) / 0x4000;
    }

    /// FUN_1088_4a81: the vehicle takes damage.
    fn vehicle_hit(&mut self, damage: i32, _rng: &mut PascalRandom) {
        if self.hurt < 1 {
            let left = self.health - damage;
            if left < 1 {
                eprintln!("warning: the vehicle's destruction (FUN_1088_4a34) not ported");
                self.hurt = 0x7fff;
                self.anim = 0x15e;
                self.state = DEAD;
                self.turn = NO_TURN;
                self.health = left;
            } else {
                if self.health > 0x8e && left < 0x8f {
                    self.messages.push((-12, false));
                }
                self.health = left;
                self.hurt = 0x52;
                self.hurt_blink = 0x52;
            }
        }
    }

    /// Script command 0x1c: the START intro throws him out of the ship.
    pub fn start_launch(&mut self) {
        self.start_jump(-0x32, 7);
        self.knocked = true; // HeroStartJump's first argument (1170:511f)
        self.vy += 10;
        self.launch = 0x82;
        self.launch_x0 = self.x + 0x16;
        self.x -= 0x8a;
        self.ship = (self.x, self.y);
    }

    /// FUN_1088_55f0's end: shots against the body's and the arm's sprites (mask 0x3a2), the
    /// worse one counts (FUN_1088_3c32).
    fn take_hits(&mut self, ps: &mut crate::particles::Particles, rng: &mut PascalRandom) {
        // The record's hflip word is gHeroFacing itself (1 or -1) and the test only asks for
        // non-zero: the hit boxes are mirrored whichever way he faces.
        let flip = 1;
        let d1 = if self.frame != NONE {
            ps.hit_test_sprite((self.frame, self.x, self.y, flip, 0), 0x3a2)
        } else {
            0
        };
        let mask1 = ps.hit_mask;
        if std::env::var("ET_HIT").is_ok() {
            eprintln!("boxes {:?}", crate::sprites::hitboxes(self.frame));
            eprintln!("TAKEHITS {} d1 {d1} frame {:x} at {},{} frame2 {:x}", ps.now, self.frame, self.x, self.y, self.gun.frame2);
        }
        let mut damage = d1;
        if self.gun.frame2 != NONE {
            let d2 = ps.hit_test_sprite((self.gun.frame2, self.x, self.y, self.facing, 0), 0x3a2);
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

    /// FUN_1088_3c32(damage) from an object: a shot-like hit. Returns 0 (invulnerable),
    /// 1 (hurt) or 2 (killed).
    pub fn hurt(&mut self, damage: i32, rng: &mut PascalRandom) -> i32 {
        self.hit(damage, 1, rng)
    }

    /// FUN_1088_3c32(damage) with 1170:162a = 2: crushed (a falling weight).
    pub fn crushed(&mut self, damage: i32, rng: &mut PascalRandom) -> i32 {
        self.hit(damage, 2, rng)
    }

    /// FUN_1088_3c32(damage) with 1170:162a = `kind` from an object (0: no reaction, 3: thrown
    /// back with a sideways shake).
    pub fn hurt_kind(&mut self, damage: i32, kind: i32, rng: &mut PascalRandom) -> i32 {
        self.hit(damage, kind, rng)
    }

    /// The tank's claw grabs him (FUN_1070_b9d3): state -3, held while `hold` runs each frame.
    pub fn grab(&mut self) {
        self.state = HELD;
        self.anim = 0;
    }

    /// gHeroDX = v from an object (after FUN_1088_532c gave it 4e06): the next frame starts at v.
    pub fn set_dx(&mut self, v: i32) {
        self.carry.0 = v - self.nudge;
    }

    /// gHeroCrouch = 1: the claw still holds him.
    pub fn hold(&mut self) {
        self.crouch = 1;
    }

    /// 1170:4e2a >= 1: invulnerable after a hit.
    pub fn invulnerable(&self) -> bool {
        self.hurt >= 1
    }

    /// FUN_1088_3c32(damage) with 1170:162a = `kind` (1 shot, 4 burnt): 0 invulnerable, 1 hurt,
    /// 2 killed.
    fn hit(&mut self, damage: i32, kind: i32, rng: &mut PascalRandom) -> i32 {
        if std::env::var("ET_HIT").is_ok() {
            eprintln!("HIT damage {damage} kind {kind} health {} hurt {} mask {:x}", self.health, self.hurt, self.hit_mask);
        }
        if self.hurt >= 1 {
            return 0;
        }
        self.health -= damage;
        if self.health < 1 {
            self.cry(rng);
            self.death = Some((self.y, self.x)); // main: the gibs, smoke and a splat (FUN_1068_186c, 637a, 0218)
            self.hurt = 0x7fff;
            self.anim = 0x8c;
            self.state = DEAD;
            return 2;
        }
        self.hurt = 0x52;
        match kind {
            1 => {
                self.cry(rng);
                self.throw_back();
                self.shakes.push((0x10, 8));
                if damage > 0x4f && rng.random(3) != 0 {
                    let sprite = rng.random(5) + 0x531e;
                    self.splats.push((sprite, self.y - 0x24, self.x));
                }
            }
            4 => {
                self.cry(rng);
                self.hurt = 1;
                if self.state != KNOCKED_BACK {
                    self.throw_back();
                }
                self.shakes.push((0x10, 8));
            }
            // Kinds 2 and 3 (FUN_1068_1f3c/215e, FUN_1070_9a66/b9d3, none ported yet: untested).
            2 => {
                self.cry(rng);
                self.shakes.push((0x10, 8));
                if self.state < 1 {
                    self.state = KNOCKED_DOWN;
                    self.anim = 0x3c;
                    self.flat = true;
                } else {
                    self.throw_back();
                }
            }
            3 => {
                self.cry(rng);
                self.throw_back();
                self.side_shakes.push((0x32, 0x12));
            }
            0 => {}
            k => eprintln!("warning: hero hit kind {k} not ported"),
        }
        self.hurt_blink = self.hurt;
        1
    }

    /// FUN_1088_3c05: the hero's cry (any earlier one cut short).
    fn cry(&self, rng: &mut PascalRandom) {
        let p = rng.random(0x20);
        sound::discard(0x1a);
        sound::play_at_pitch(0x1a, p + 0xf0, self.y, self.x);
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
            if std::env::var_os("ET_SHOT").is_some() {
                eprintln!("SHOT {now} hero {},{} facing {} muzzle {:?} aim {} pose {} -> {x},{y}", self.x, self.y, self.facing, g.muzzle, g.aim, g.pose);
            }
            g.shot = Some((y, x, g.aim, g.shots & 1));
            sound::discard(0x73); // HeroFireWeapon
            sound::play_at(0x73, y, x);
            g.shots += 1;
            if std::env::var_os("ET_SHOT").is_some() {
                eprintln!("SHOTS {now} -> {}", g.shots);
            }
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
                // The level is over (GameFrame: 4e60 = 100 + ...), the frame isn't drawn.
                if self.level_end != 0x1e {
                    self.level_end = leave as i32 + 99;
                }
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
            // FUN_1088_678b: the flame thrower follows the aim a little behind.
            1 => {
                let g = &mut self.gun;
                if !g.armed || input[RIGHT_SHOOT] < 1 {
                    g.flame_aim = g.aim;
                } else {
                    if g.aim != g.flame_aim {
                        let d = (g.aim - g.flame_aim + 0x1680 + 0xb40) % 0x1680 - 0xb40;
                        if d.abs() > 0x3c0 {
                            g.flame_aim = g.aim;
                        } else if d < 0 {
                            g.flame_aim = (g.flame_aim + 0x1680 - (-d).min(0x40)) % 0x1680;
                        } else {
                            g.flame_aim = (d.min(0x40) + g.flame_aim) % 0x1680;
                        }
                    }
                    if self.weapons[1].ammo > 0 {
                        let g = &mut self.gun;
                        let k = g.shots % 6 + 0x17;
                        let a = g.flame_aim;
                        let x = self.x + g.muzzle.0 + (k * crate::trig::cos(a)) / 0x4000;
                        let y = self.y + g.muzzle.1 - 0x16 - (k * crate::trig::sin(a)) / 0x4000;
                        let wobble = (crate::trig::sin(g.shots * 0x100 % 0x1680) * 0x40) / 0x4000;
                        if std::env::var_os("ET_FLAME").is_some() {
                            eprintln!("FLAME {now} aim {} a4a {a} shots {} muzzle {:?} hero {},{} -> {x},{y} angle {}", g.aim, g.shots, g.muzzle, self.x, self.y, (a + wobble + 0x1680) % 0x1680);
                        }
                        g.flame = Some((y, x, (a + wobble + 0x1680) % 0x1680));
                        if g.flame_sound <= now {
                            sound::play_at(0x72, y, x);
                            g.flame_sound = now + 0x10;
                        }
                        g.shots += 1;
                        g.shooting = true;
                        self.weapons[1].ammo -= 1;
                        if self.weapons[1].ammo < 1 {
                            self.next_weapon();
                        }
                    }
                }
            }
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
            // FUN_1088_6f4d: like the grenade, a bouncing charge.
            4 => {
                if self.use_ammo(0xc, input, now) {
                    let (y, x) = (self.y - 0x1a, self.x);
                    let a = (self.gun.aim + 0x1680 - 0x50 + rng.random(0xa1)) % 0x1680;
                    self.gun.charge = Some((y, x, a));
                    self.gun.pose = 4;
                    self.gun.shooting = true;
                    self.released[RIGHT_SHOOT] = false;
                }
            }
            // FUN_1088_6cf0: three homing rockets.
            8 => {
                if std::env::var("ET_W").is_ok() {
                    let w = &self.weapons[8];
                    eprintln!("W8 {} ammo {} armed {} rs {} rel {} next {} now {}", self.weapon, w.ammo, self.gun.armed, input[RIGHT_SHOOT], self.released[RIGHT_SHOOT], w.next, now);
                }
                if self.use_ammo(8, input, now) {
                    let g = &mut self.gun;
                    let (y, x) = (self.y + g.muzzle.1 - 0x16, self.x + g.muzzle.0);
                    g.rockets = Some((y, x, g.aim));
                    let p = rng.random(0x30); // the sound's pitch
                    sound::play_at_pitch(0x5c, 0x100 - p, y, x);
                    g.pose = 4;
                    g.shots += 1;
                    g.cooldown = 10;
                    g.shooting = true;
                    self.released[RIGHT_SHOOT] = false;
                }
            }
            // FUN_1088_6deb: a rocket that speeds up, aimed 0x20 either side of the aim.
            5 => {
                if self.use_ammo(8, input, now) {
                    let g = &mut self.gun;
                    let (c, s) = (crate::trig::cos(g.aim), crate::trig::sin(g.aim));
                    let x = self.x + g.muzzle.0 + (0x17 * c) / 0x4000;
                    let y = self.y + g.muzzle.1 - 0x16 - (0x17 * s) / 0x4000;
                    let a = (g.aim + 0x1680 - 0x20 + rng.random(0x41) as i32) % 0x1680;
                    g.rocket5 = Some((y, x, a));
                    sound::play_at(0x52, y, x);
                    g.pose = 4;
                    g.shots += 1;
                    g.cooldown = 10;
                    g.shooting = true;
                    self.released[RIGHT_SHOOT] = false;
                }
            }
            // FUN_1088_6b1f: a homing rocket at a steady speed, aimed 0x20 either side of the aim.
            3 => {
                if self.use_ammo(8, input, now) {
                    let g = &mut self.gun;
                    let (c, s) = (crate::trig::cos(g.aim), crate::trig::sin(g.aim));
                    let x = self.x + g.muzzle.0 + (0x17 * c) / 0x4000;
                    let y = self.y + g.muzzle.1 - 0x16 - (0x17 * s) / 0x4000;
                    let a = (g.aim + 0x1680 - 0x20 + rng.random(0x41) as i32) % 0x1680;
                    g.rocket3 = Some((y, x, a));
                    sound::play_at(0x52, y, x);
                    g.pose = 4;
                    g.shots += 1;
                    g.cooldown = 10;
                    g.shooting = true;
                    self.released[RIGHT_SHOOT] = false;
                }
            }
            // FUN_1088_69d8: a rapid-fire bullet.
            2 => {
                if self.use_ammo(0xd, input, now) {
                    let g = &mut self.gun;
                    let (c, s) = (crate::trig::cos(g.aim), crate::trig::sin(g.aim));
                    let x = self.x + g.muzzle.0 + (0x17 * c) / 0x4000;
                    let y = self.y + g.muzzle.1 - 0x16 - (0x17 * s) / 0x4000;
                    g.rapid = Some((y, x, g.aim, g.shots & 1));
                    sound::discard(0x1b);
                    sound::play_at(0x1b, y, x);
                    g.shots += 1;
                    g.cooldown = 10;
                    g.shooting = true;
                }
            }
            // FUN_1088_655f: a shell that goes off on a slow impact.
            7 => {
                if self.use_ammo(0xc, input, now) {
                    let (y, x) = (self.y - 0x1a, self.x);
                    let a = (self.gun.aim + 0x1680 - 0x50 + rng.random(0xa1)) % 0x1680;
                    self.gun.shell7 = Some((y, x, a));
                    self.gun.pose = 4;
                    self.gun.shooting = true;
                    self.released[RIGHT_SHOOT] = false;
                }
            }
            // FUN_1088_70b8: three bullets, 0x80 either side of the aim and on it.
            6 => {
                if self.use_ammo(10, input, now) {
                    let g = &mut self.gun;
                    g.spread = Some((self.y + g.muzzle.1 - 0x16, self.x + g.muzzle.0, g.aim));
                    sound::play_at(0x95, self.y + g.muzzle.1 - 0x16, self.x + g.muzzle.0);
                    g.shots += 1;
                    g.cooldown = 10;
                    g.shooting = true;
                }
            }
            // FUN_1088_6649: a ball that bounces and rolls (FUN_1050_46d9); crouched, it is laid
            // on the floor and may fire in any pose.
            9 => {
                if self.state == CROUCH {
                    self.gun.armed = true;
                }
                if self.use_ammo(0xc, input, now) {
                    let aim = self.gun.aim;
                    let (y, a) = if self.state == CROUCH && self.crouch > 7 {
                        (self.y, if (0x5a0..0x10e0).contains(&aim) { 0xb40 } else { 0 })
                    } else {
                        (self.y - 0x1a, (aim + 0x1680 - 0x50 + rng.random(0xa1)) % 0x1680)
                    };
                    self.gun.ball9 = Some((y, self.x, a));
                    sound::play_at(0x5e, y, self.x);
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

    /// FUN_1088_0688: the current weapon's digit-key slot (0 = key 1, 9 = key 0).
    pub fn weapon_slot(&self) -> usize {
        (0..10).rev().find(|&d| WEAPON_FOR_DIGIT[d] == self.weapon).unwrap_or(0)
    }

    /// SelectWeapon (1088:71ef). (4e32 = -14 starts the weapon display, 4e4a keeps the aim.)
    pub fn select_weapon(&mut self, w: i32) {
        // A click from the weapon's place on the HUD (FUN_1088_0688: its slot), at the listener.
        if w != self.weapon {
            self.weapon = w;
            sound::discard(0x88);
            sound::play_at_listener(0x88, -200 + (self.weapon_slot() as i32 * 400) / 9);
            self.hud_slide = -14;
        }
        self.weapon = w;
    }

    /// The update half of FUN_1088_2501 (the status bar), once a frame (the end of the frame).
    /// It also keeps the health (4e6c) within 0..=0x2c6: hearts can't push it over.
    pub fn status_bar(&mut self) {
        self.health = self.health.clamp(0, 0x2c6);
        if std::env::var_os("ET_HEALTH").is_some() {
            eprintln!("health {}", self.health);
        }
        let target = self.health * 0x47 / 0x2c6;
        self.hud_bar = if self.hud_bar == 0x7fff { target } else { self.hud_bar + (target - self.hud_bar).signum() };
        if self.hud_slide < 0 {
            self.hud_slide += 2;
        }
    }

    /// The start of FUN_1088_55f0: pick-ups (function 30000 in plane 2) under the hero's two
    /// columns (x-4, x+4) from his feet to his head (FUN_1088_5551). Each is cleared and its
    /// effect (function 4) runs at the tile's centre; main applies both.
    fn pick_up(&mut self, w: &World, lv: &Level, rng: &mut PascalRandom) {
        let rows = (self.height + 7) >> 4;
        let (feet, head) = (self.y - 4, (self.y - self.height) + 5);
        let mut y = feet;
        for r in (0..=rows).rev() {
            for x in [self.x - 4, self.x + 4] {
                let Some(cell) = w.cell_at(2, y, x) else {
                    self.harmful_tile(w.tile_function_at(y, x), rng);
                    continue;
                };
                let t = (x >> 4, y >> 4);
                if !self.pickups.iter().any(|p| p.0 == t) && w.cell_function(cell, 0) == PICKUP {
                    let (fx, f3) = (w.cell_function(cell, 4), w.cell_function(cell, 3));
                    self.pickups.push((t, fx, f3, (y & !15) + 7, (x & !15) + 7));
                }
                self.harmful_tile(w.tile_function_at(y, x), rng);
            }
            y = if r != 1 { y - 0x10 } else { head };
        }
        // The platforms the hero's column meets (FUN_1078_0198), last first: their planes'
        // tiles under x from the feet up.
        for p in lv.platforms.iter().rev().flatten() {
            if self.x < p.x || feet < p.y || p.x + p.width <= self.x || p.y + p.height <= head {
                continue;
            }
            let x = self.x - p.x;
            let mut y = feet - p.y;
            for r in (0..=rows).rev() {
                self.harmful_tile(w.tile_function_in(p.plane, y, x), rng);
                y = if r != 1 { y - 0x10 } else { head - p.y };
            }
        }
    }

    /// FUN_1088_55bd: tile function 8000 hurts (FUN_1088_3c32(100)); when that hurt him, sound
    /// 0x16 and a sideways shake.
    fn harmful_tile(&mut self, f: u16, rng: &mut PascalRandom) {
        if f == 8000 && self.hit(100, 1, rng) == 1 {
            sound::play_at(0x16, self.y, self.x);
            self.side_shakes.push((0x24, 0xe));
        }
    }

    /// FUN_1040_0312 (effect 67) for the hero's side: a weapon (function 3 - 0x1e) and its ammo.
    pub fn weapon_pickup(&mut self, f3: u16) {
        let i = f3 as i32 - 0x1e;
        let w = &mut self.weapons[i as usize];
        let select = if !w.owned {
            w.owned = true;
            self.messages.push((f3 as i32 - 0x3f, false));
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
        if d >= FATAL_FALL || self.prev_state == KNOCKED_BACK || self.launch > 0 {
            self.state = KNOCKED_DOWN;
            if d < FATAL_FALL {
                if self.launch < 1 {
                    self.anim = 0x14;
                } else {
                    self.launch = 0;
                    self.anim = 0x46;
                    sound::play_at(0x25, self.y, self.x);
                }
                self.flat = true;
            } else {
                self.anim = ((d - FATAL_FALL) >> 1) + 100;
                self.anim = self.anim.min(0xaa);
                self.shakes.push((0x18, 0xe));
                self.flat = false;
                sound::play_at(0x25, self.y, self.x);
                sound::play_at(0x1a, self.y, self.x);
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
        // A thud, or on a soft floor (gHeroGround 9) from high up a splash (and a puff, 66d4).
        sound::play_at(if d < 0x40 || self.ground != 9 { 8 } else { 4 }, self.y, self.x);
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
        let (dir, step_h) = if self.lim.0 == self.facing {
            (self.facing, 0x14)
        } else {
            (blocked.signum(), step_h)
        };
        if walking && (blocked == 0 || step_h > 0x13) {
            if dir == self.facing {
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
        let mut belt = None;
        // One column test (FUN_1088_2cff + 2c97). `offset` and `pdy` are 0 for the map.
        let mut column = |func0: u16,
                          sol: u16,
                          sy: i32,
                          sx_abs: (usize, i32),
                          offset: i32,
                          pdy: i32,
                          best: &mut i32,
                          ground: &mut i32,
                          belt: &mut Option<(usize, i32, i32)>|
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
                    *belt = Some(sx_abs.0).map(|pl| (pl, sy, sx_abs.1));
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
                &mut belt,
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
                        &mut belt,
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
        if let Some((plane, sy, sx)) = belt {
            if let Some(cell) = w.cell_at(plane, sy, sx) {
                self.belt = w.cell_function(cell, 3) as i16 as i32;
            }
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
            sound::play_at(0x22, self.y, self.x);
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
                sound::play_at(0x39, self.y, self.x);
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
/// FUN_1088_29c8(n, dir, y, x): how many slope tiles (up to n) run on from here, stepping
/// down by each one's drop and sideways by `dir` tiles.
fn slope_run(w: &World, n: i32, dir: i32, mut y: i32, mut x: i32) -> i32 {
    let mut c = 0;
    if w.solidity_at(y - 8, x) & 0x20 == 0 {
        y += 8;
    }
    loop {
        if w.solidity_at(y, x) & 0x20 == 0 {
            return c;
        }
        let cell = w.cell_at(MAIN_PLANE, y, x).unwrap_or(0);
        let id = w.cell_function(cell, 2) as usize & 0xff7f;
        y += crate::world::SLOPES[id * 0x16 + 0x11] as i32 * 0x10;
        x += dir * 0x10;
        c += 1;
        if c >= n {
            return c;
        }
    }
}

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
