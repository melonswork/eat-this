//! The big alien (record type 0x1432, bigali.spr in slot 0x5b): FUN_1070_7eb2 with its nested
//! procedures (FUN_1070_73e1 .. 7c9d) and its move hook FUN_1070_72a2. It carries a rifle and a
//! homing rocket launcher, and some wear armour (standing under a 0x334a tile). Fields:
//!
//! | off | field |
//! |-----|-------|
//! | 0a  | flags / 12 bonus / 14,16 patrol range / 20 facing |
//! | 28  | state (below) / 2a hit flash / 2c, 2e, 30 counters |
//! | 32  | dying: frames to the self-destruct / 34 sprite / 36 hit points / 38 their mark |
//! | 3a  | shooting pause / 3c shield (raised for 0x69 frames) / 3e frames walking |
//! | 40  | sees the hero: 0 never, 1 lost him, 2 yes / 42 last y |
//! | 44  | armoured (byte) / 45 has growled (byte) |
//!
//! States: -10 walking, -0x14 standing, -0x8c rifle, -0x96 rocket, -0xfa hit, -200 shield up,
//! -0x78 jumping down, -0x82 falling, -300 dying, -0x136 a corpse, -0x140 frozen.

use crate::actors::Env;
use crate::level::Level;
use crate::particles::{self, Particles};
use crate::sprites;
use crate::trig;

/// 1170:1aca / 1ace.
const SHOOT_X: (i32, i32) = (0, 0x13f);

/// The parent's locals the nested procedures read.
#[derive(Default)]
struct Look {
    dist: i32,  // [BP-0xe]: |hero x - x|
    dir: i32,   // [BP-0xc]
    angle: i32, // [BP-8]
}

impl Level {
    /// FUN_1070_7eb2.
    pub fn bigali(&mut self, id: usize, env: &mut Env) {
        if std::env::var_os("ET_DEBUG").is_some() {
            let o = self.o(id);
            let words: Vec<String> = (0..0x46).step_by(2).map(|i| o.w(i).to_string()).collect();
            eprintln!("BIG {}\t{}", env.frame + 1, words.join(" "));
        }
        let h = env.h;
        {
            let o = self.o(id);
            if o.w(0x2a) > 0 {
                o.set(0x2a, o.w(0x2a) - 1);
            }
            o.set(0x42, o.w(0x10));
        }
        let (x, y) = self.bxy(id);
        self.patrol_box(id, y + 0x30, x + 0x80, y - 0x60, x - 0x80);
        let height = sprites::height(self.o(id).w(0x34));
        if self.actor_physics(id, env, height - 3, 0xe, 8) > 0 {
            if self.o(id).w(0x36) > 0 {
                env.fx.kills += 1;
            }
            if self.o(id).w(0x28) > -0x12d {
                let y = self.o(id).w(0x10);
                self.bigali_explode(id, y - 0x1e, env);
            }
            self.big_drop_bonus(id, env.world);
            self.remove_current(id);
            return;
        }
        let mut look = Look::default();
        if self.o(id).w(0x36) > 0 {
            let (x, y) = self.bxy(id);
            {
                let o = self.o(id);
                if o.w(0x3a) > 0 {
                    o.set(0x3a, o.w(0x3a) - 1);
                }
            }
            look.dist = (h.x - x).abs();
            let eye = y - 0x25;
            let target = (h.y - h.height) + 6;
            if !self.line_of_sight(env, target, h.x, eye, x) {
                let o = self.o(id);
                if o.b(0x45) == 0 {
                    o.set_b(0x45, 1); // a growl
                }
                if o.w(0x40) != 0 {
                    o.set(0x40, 1);
                }
            } else {
                if h.hit_mask & 0x200 != 0 && env.rng.random(3) != 0 {
                    env.rng.random(2); // which growl
                }
                let o = self.o(id);
                o.set(0x40, 2);
                if look.dist < 0x200 {
                    if h.x < o.w(2) {
                        o.set(2, h.x);
                    }
                    if o.w(6) < h.x {
                        o.set(6, h.x);
                    }
                }
            }
            let o = self.o(id);
            look.angle = trig::atan2(target - eye, (h.x - x - o.w(0x20) * 0x1a).abs());
            look.dir = if h.x - x < 0 { -1 } else { 1 };
            if o.w(0x40) > 1
                && o.w(0x20) != look.dir
                && o.w(0x28) > -0x15
                && look.dist > 0x30
                && (o.w(0x28) != -10 || o.w(0x3e) > 0x40)
            {
                self.big_turn(id, look.dir);
            }
        }
        let mut mirror = self.o(id).w(0x20) < 0;
        let state = self.o(id).w(0x28);
        match state {
            -10 => {
                let o = self.o(id);
                o.set(0x3e, o.w(0x3e) + 1);
                if o.w(0x2e) < 1 {
                    o.set(0x34, (o.w(0x2c) / 4) % 0xc + 0x5b0a);
                    o.set(0x2c, o.w(0x2c) + 1);
                } else {
                    let mut f = (0x10 - o.w(0x2e)) / 2;
                    if f > 3 {
                        f = 7 - f;
                    }
                    o.set(0x34, f + 0x5b3e);
                    o.set(0x2e, o.w(0x2e) - 1);
                    if o.w(0x2e) > 7 {
                        mirror = !mirror;
                    }
                    if o.w(0x2e) < 1 || o.w(0x14) + 1 == o.w(0x16) {
                        o.set(0x2c, 4);
                    } else {
                        o.set(0x2c, o.w(0x2c) + 1);
                    }
                }
                let dir = self.o(id).w(0x20);
                self.big_walk(id, env, &look, 0xe, dir);
                let o = self.o(id);
                if o.w(0x20) != dir {
                    o.set(0x20, dir);
                    o.set(0x28, -0x14);
                    o.set(0x2e, 0xb4);
                    o.set(0x2c, 1);
                }
                if self.o(id).w(0x2e) < 1 {
                    self.big_decide(id, env, &look);
                    self.big_jump_down(id, env, &look);
                }
            }
            -0x14 => {
                let o = self.o(id);
                let mut s = ((o.w(0x2e) / 0xc) & 3) + 0x5b01;
                if s == 0x5b04 {
                    s = 0x5b02;
                }
                o.set(0x34, s);
                o.set(0x2e, o.w(0x2e) - 1);
                if o.w(0x2e) < 1 {
                    o.set(0x28, -10);
                    let d = -o.w(0x20);
                    self.big_turn(id, d);
                }
                let o = self.o(id);
                if o.w(0x2c) == 0 && o.w(0x40) == 2 {
                    self.big_go(id);
                }
                self.big_decide(id, env, &look);
                self.big_jump_down(id, env, &look);
            }
            -0x8c | -0x96 => {
                let o = self.o(id);
                o.set(0x30, o.w(0x30) + 1);
                let mut t = o.w(0x30);
                if t > 10 {
                    let rocket = state == -0x96;
                    let end = if rocket { (o.b(0x44) as i32 + 2) * 0x28 + 10 } else { 0x4a };
                    if t >= end {
                        self.big_go(id);
                    }
                    let period = if rocket { 0x28 } else { 8 };
                    let k = (t - 0xb) % period;
                    if k == 0 {
                        let o = self.o(id);
                        let (x, y, dir) = (o.w(0xe), o.w(0x10), o.w(0x20));
                        if rocket {
                            self.big_rocket(id, env, 0x80, y - 0x17, dir * 0x16 + x);
                        } else {
                            let a = ((t - 0xb) / 8) * 0xcd - 0x2d0;
                            self.big_shot(id, env, a, y - 0x17, dir * 0x18 + x);
                        }
                    }
                    t = (k < if rocket { 6 } else { 4 }) as i32 + 0xb;
                }
                self.o(id).set(0x34, t / 3 + 0x5b1f);
            }
            -0xfa => {
                let o = self.o(id);
                o.set(0x2e, o.w(0x2e) - 1);
                o.set(0x34, (o.w(0x2e) + 0x5b52).min(0x5b54));
                if o.w(0x2e) < 1 {
                    o.set(0x28, -0x14);
                    o.set(0x2e, 0xb4);
                    o.set(0x2c, 0);
                }
                if o.w(0x2e) % 2 == 0 {
                    o.set(0xe, o.w(0xe) - o.w(0x20));
                }
            }
            -200 => {
                let frame = env.frame as i32;
                let o = self.o(id);
                let mut s = ((frame / 10) & 3) + 0x5b4a;
                if s == 0x5b4d {
                    s = 0x5b4b;
                }
                o.set(0x34, s);
                if o.w(0x3c) < 1 {
                    self.big_go(id);
                }
            }
            -0x78 => {
                let o = self.o(id);
                o.set(0x34, ((o.uw(0x1a) >> 2) as i32 + 0x5b62).min(0x5b64));
                o.set(0xe, o.w(0xe) + o.w(0x20));
                if o.w(0x2e) == 2 {
                    o.set(0xe, o.w(0xe) + o.w(0x20));
                }
                o.set(0x18, o.w(0x18) - o.w(0x2e));
            }
            -0x82 => {
                let o = self.o(id);
                o.set(0x34, 0x5b5e);
                o.set(0xe, o.w(0xe) + o.w(0x2e));
                let (y, prev, x) = (o.w(0x10), o.w(0x42), o.w(0xe));
                self.splash(env, y, prev, x);
            }
            -0x136 => {
                let o = self.o(id);
                if o.w(0x1c) == 5 {
                    o.set(0xe, o.w(0xe) + if o.w(0x1e) & 0x80 == 0 { -1 } else { 1 });
                }
                o.set(0x34, 0x5200);
            }
            -0x140 => {
                eprintln!("warning: a frozen big alien (FUN_1068_0f32) not ported");
                return;
            }
            -300 => {
                if self.big_dying(id, env, mirror) {
                    return;
                }
            }
            _ => {}
        }
        // Shots while alive.
        if self.o(id).w(0x36) > 0 {
            let (x, y) = self.bxy(id);
            let sprite = self.o(id).w(0x34);
            let damage;
            if self.o(id).w(0x3c) < 1 {
                damage = match sprites::hit_rect(sprite, y, x, mirror) {
                    Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x10fb),
                    None => 0,
                };
            } else {
                {
                    let o = self.o(id);
                    o.set(2, -0x7fff);
                    o.set(4, -0x7fff);
                    o.set(6, 0x7fff);
                    o.set(8, 0x7fff);
                    o.set(0x3c, o.w(0x3c) - 1);
                }
                self.particles.p.aim_level2 = -1;
                let r = self.particles.hit_test(y + 6, x + 0x20, y - 0x3a, x - 0x20, 0x1fb);
                self.particles.p.aim_level2 = 0x100;
                let h = env.h;
                if r > 0 || (x - 0x10 <= h.x && y - 0x40 <= h.y && h.x <= x + 0x10 && h.y <= y + 0x20) {
                    self.o(id).set(0x3c, 0x69);
                }
                // FUN_1070_00b6: the hero may stand on the shield.
                if x - 0x20 <= h.x && h.x <= x + 0x20 && (h.y.min(h.prev_y)..=h.y.max(h.prev_y)).contains(&(y - 0x3a)) {
                    eprintln!("warning: the hero standing on a big alien's shield not ported");
                }
                damage = match sprites::hit_rect(sprite, y, x, mirror) {
                    Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x1000),
                    None => 0,
                };
            }
            if damage > 0 {
                let push = self.particles.hit_direction();
                let o = self.o(id);
                if o.b(0x44) == 0 {
                    o.set(0x36, o.w(0x36) - damage);
                }
                if push != 0 {
                    o.set(0x20, -push);
                }
                o.set(0x3c, 0);
                if o.w(0x36) < 1 {
                    if particles::FRAME_SLACK && env.rng.random(2) == 0 {
                        eprintln!("warning: FUN_1088_11a3 (a taunt) not ported");
                    }
                    env.fx.kills += 1;
                    self.o(id).set(0x36, 0);
                    if self.particles.hit_mask & 0x400 == 0 {
                        let (x, y) = self.bxy(id);
                        let by = (y - 0x20) - env.rng.random(8);
                        let which = env.rng.random(3);
                        self.blood_splat(env.world, 0x541e + which, by, x);
                        env.rng.random(3); // the death cry
                        let o = self.o(id);
                        o.set(0x28, -300);
                        o.set(0x2e, 0);
                        self.big_bleed(id, env, y - 0x14, x);
                    } else {
                        self.big_drop_bonus(id, env.world);
                        self.o(id).set(0x28, -0x140);
                    }
                    env.fx.shakes.push((0xc, 6));
                } else {
                    o.set(0x2a, 3);
                    if self.particles.hit_mask != 0x48 {
                        if self.o(id).b(0x44) != 0 && env.rng.random(4) == 0 {
                            env.rng.random(0x40); // a ricochet off the armour
                        }
                    }
                    let o = self.o(id);
                    if o.w(0x3a) > 0x23 {
                        o.set(0x3a, 0x23);
                    }
                    if o.w(0x1a) < 1 {
                        o.set(0x2e, 9);
                        o.set(0x28, -0xfa);
                        if o.w(0x36) <= o.w(0x38) - 0x80 {
                            o.set(0x38, o.w(0x36));
                            // FUN_1070_7bfe: the shield goes up.
                            o.set(0x28, -200);
                            o.set(0x3c, 0x69);
                        }
                    }
                }
            }
        }
    }

    fn bxy(&mut self, id: usize) -> (i32, i32) {
        let o = self.o(id);
        (o.w(0xe), o.w(0x10))
    }

    fn big_drop_bonus(&mut self, id: usize, world: &crate::world::World) {
        let (b, y, x) = (self.o(id).w(0x12), self.o(id).w(0x10), self.o(id).w(0xe));
        if b != 0 {
            self.bonus(world, b, y, x);
            self.o(id).set(0x12, 0);
        }
    }

    /// FUN_1070_790d: walk on.
    fn big_go(&mut self, id: usize) {
        let o = self.o(id);
        o.set(0x28, -10);
        o.set(0x2e, 0);
        o.set(0x2c, 4);
        o.set(0x3e, 0);
    }

    /// FUN_1070_7933: turn round (unless in the middle of turning).
    fn big_turn(&mut self, id: usize, dir: i32) {
        let o = self.o(id);
        if o.w(0x28) != -10 || o.w(0x2e) < 1 {
            o.set(0x28, -10);
            o.set(0x20, dir);
            o.set(0x2e, 0x10);
            o.set(0x3e, 0);
        }
    }

    /// FUN_1070_73e1(half, dx): patrol, or when chasing the hero (seen, facing him, he isn't
    /// above) walk anywhere short of a wall.
    fn big_walk(&mut self, id: usize, env: &mut Env, look: &Look, half: i32, dx: i32) {
        let o = self.o(id);
        if o.w(0x40) < 2 || o.w(0x20) != look.dir || env.h.y < o.w(0x10) - 0x20 {
            self.patrol_step(id, env, half, dx);
            return;
        }
        o.set(0xe, o.w(0xe) + dx);
        let (x, y) = (o.w(0xe), o.w(0x10));
        let ly = ((y + 1) & !15) - 1;
        let lx = if dx < 0 { x - half } else { x + half };
        if crate::hero::Hero::solidity_all(env.world, self, ly, lx) & 8 != 0 {
            self.o(id).set(0x20, if dx < 0 { 1 } else { -1 });
        }
    }

    /// FUN_1070_755c: in range and facing the hero: fire a rocket (close, or now and then) or
    /// the rifle (if he's not too high or low).
    fn big_decide(&mut self, id: usize, env: &mut Env, look: &Look) {
        let cam_x = env.camera.0;
        let o = self.o(id);
        if env.h.hurt >= 1
            || !(SHOOT_X.0 < o.w(0xe) - cam_x && o.w(0xe) - cam_x < SHOOT_X.1)
            || o.w(0x3a) >= 1
            || o.w(0x40) != 2
            || o.w(0x20) != look.dir
        {
            return;
        }
        if look.dist < 0x70 || env.rng.random(0x20) == 0 {
            env.rng.random(3); // a roar sometimes
            let o = self.o(id);
            o.set(0x28, -0x96);
            o.set(0x30, 0);
            o.set(0x3a, if o.b(0x44) == 0 { 0x8c } else { 0xd2 });
        } else if look.angle < 0x3c0 || look.angle > 0x12c0 {
            if env.rng.random(4) == 0 {
                env.rng.random(2);
            }
            let o = self.o(id);
            o.set(0x28, -0x8c);
            o.set(0x30, 0);
            o.set(0x3a, if o.b(0x44) == 0 { 0x8c } else { 0x50 });
        }
    }

    /// FUN_1070_7c9d: jump down (or across) towards the hero.
    fn big_jump_down(&mut self, id: usize, env: &mut Env, look: &Look) {
        let o = self.o(id);
        if o.w(0x40) != 2 || o.w(0x20) != look.dir {
            return;
        }
        let (x, y, dir, ground) = (o.w(0xe), o.w(0x10), o.w(0x20), o.w(0x1c));
        let all = |s: &Level, yy: i32, xx: i32| crate::hero::Hero::solidity_all(env.world, s, yy, xx);
        let fx = dir * 0x14 + x;
        let k = (all(self, y - 0x10, fx) & 2 != 0) as i32;
        let s = env.world.solidity_at(y - k * 0x10, fx - dir * 0x10);
        if s & 0x20 == 0
            && (k != 0 || (all(self, y, fx) & 2 != 0 && ground != 5))
            && self.big_room(env, dir, 0x30, 0x14, y - 0xf - (k + 1) * 0x10, fx)
        {
            let o = self.o(id);
            o.set(0x28, -0x78);
            o.set(0x18, y - 5);
            o.set(0x1a, 1);
            o.set(0x2e, k + 3);
            env.fx.shakes.push((8, 3));
        } else if env.h.y <= y + 0xf {
            let w = &env.world;
            let (fx2, fy) = (dir * 2 + x, y + 0xf);
            if w.solidity_at(fy, fx2) & 0x221 == 0
                && w.solidity_at(fy, dir * 0x10 + fx2) & 0x221 == 0
                && w.solidity_at(fy, dir * 0x20 + fx2) & 0x221 != 0
                && w.solidity_at(fy, x) & 0x221 != 0
                && self.big_room(env, dir, 0x50, 0x20, y, x + dir + 0x20)
            {
                let o = self.o(id);
                o.set(0x28, -0x78);
                o.set(0x18, y - 5);
                o.set(0x1a, 1);
                o.set(0x2e, 2);
                env.fx.shakes.push((8, 3));
            }
        }
    }

    /// FUN_1070_7c27(h, w, y, x): nothing solid (2 or 8) in the h x w block going up from (x, y)
    /// backwards.
    fn big_room(&self, env: &Env, dir: i32, h: i32, w: i32, mut y: i32, x: i32) -> bool {
        let mut rows = h >> 4;
        if rows == 0 {
            return true;
        }
        let mut ok = true;
        loop {
            let mut cols = w >> 4;
            loop {
                if crate::hero::Hero::solidity_all(env.world, self, y, x - cols * 0x10 * dir) & 10 != 0 {
                    ok = false;
                    break;
                }
                if cols == 0 {
                    break;
                }
                cols -= 1;
            }
            y -= 0x10;
            if rows == 1 {
                return ok;
            }
            rows -= 1;
        }
    }

    /// FUN_1070_769c(angle, y, x): a rifle shot.
    fn big_shot(&mut self, id: usize, env: &mut Env, angle: i32, y: i32, x: i32) {
        let dir = self.o(id).w(0x20);
        let a = if dir < 0 { 0xb40 - angle } else { angle };
        let ps = &mut self.particles;
        ps.p.colour = 0x28;
        ps.spawn_seq(0x1b04, 2, 2, y, x, 1);
        ps.p.colour = 0;
        ps.p.hits = 0x100;
        ps.p.damage = 0x40;
        ps.p.on_death = 0x3f;
        ps.p.callback = 0x2d2;
        ps.p.draw = (0x5a0 < a) as i32;
        ps.spawn(0x1910, y, x);
        ps.line(0x40, (a + 0x1680) % 0x1680);
        ps.p.callback = 0;
        ps.p.on_death = 0;
        ps.p.damage = 0x10;
        ps.p.hits = 0;
        ps.spawn_seq(0x1923, 2, 1, y, x, 0);
        let side = if ps.p.draw < 1 { 1 } else { -1 };
        ps.p.group = 0;
        let r = env.rng.random(0x161);
        shell(ps, side * (r + 0xa0) + 0x5a0, y - 9, x - side * 0xd);
        ps.p.group = 1;
        ps.p.draw = 0;
        env.fx.shakes.push((5, 4));
        let o = self.o(id);
        o.set(0x3a, o.w(0x3a) + 1);
    }

    /// FUN_1070_77dd(angle, y, x): a homing rocket.
    fn big_rocket(&mut self, id: usize, env: &mut Env, angle: i32, y: i32, x: i32) {
        let dir = self.o(id).w(0x20);
        // Facing left, the angle is mirrored.
        let angle = if dir < 0 { 0xb40 - angle } else { angle };
        let ps = &mut self.particles;
        ps.p.colour = 0x28;
        ps.spawn_seq(0x1b04, 2, 2, y, x, 1);
        ps.p.colour = 0;
        ps.p.hits = 0x212;
        ps.p.damage = 0x80;
        ps.p.homing = 0x200;
        ps.p.tiles = 0x143;
        ps.p.on_death = 0xe;
        ps.p.callback = 0xb2a;
        ps.spawn_seq(0x1946, 1, 0x180, y, x, 0);
        let a = (angle + 0x1680 - 0x20 + env.rng.random(0x41)) % 0x1680;
        ps.rocket(0x24, a);
        ps.p.callback = 0;
        ps.p.on_death = 0;
        ps.p.tiles = 0x43;
        ps.p.homing = 0;
        ps.p.damage = 0x10;
        ps.p.hits = 0;
        ps.p.draw2 = 0;
        ps.p.colour = 0;
        ps.spawn_seq(0x1909, 2, 3, y, x + dir, 1);
        ps.p.draw = 0;
        env.fx.shakes.push((8, 6));
        env.noise = 2;
        let o = self.o(id);
        o.set(0x3a, o.w(0x3a) + 1);
        o.set(0xe, o.w(0xe) - o.w(0x20));
    }

    /// FUN_1070_74b5(y, x): blood.
    fn big_bleed(&mut self, id: usize, env: &mut Env, y: i32, x: i32) {
        let dir = self.o(id).w(0x20);
        let ps = &mut self.particles;
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x16;
        for i in 1..=3 {
            ps.p.delay = env.rng.random(5);
            let s = env.rng.random(2) + 0x5407;
            ps.spawn(s, y, x);
            let a = dir * (env.rng.random(0x141) + 0x280) + 0x5a0;
            ps.throw(7, (i * 0x18) / 3 + 0x10, a);
        }
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
        ps.p.delay = 0;
        ps.spawn_seq(0x5400, 4, 5, y, x, 1);
    }

    /// State -300: falling dead, then lying; a while after the hero comes near, it blows up.
    /// Returns true when the object is gone.
    fn big_dying(&mut self, id: usize, env: &mut Env, mirror: bool) -> bool {
        let h = env.h;
        {
            let o = self.o(id);
            o.set(0x34, o.w(0x2e) / 6 + 0x5b2a);
        }
        if self.o(id).w(0x2e) < 0x35 {
            let o = self.o(id);
            if o.w(0x1a) < 1 || o.w(0x2e) < 0x18 {
                o.set(0x2e, o.w(0x2e) + 1);
                if o.w(0x2e) == 0x31 {
                    env.fx.shakes.push((0x18, 0xc));
                    if self.o(id).w(0x12) == 0 && env.rng.random(2) == 0 {
                        self.o(id).set(0x12, 0xd10);
                    }
                    self.big_drop_bonus(id, env.world);
                }
            }
            if env.frame & 1 == 0 {
                let o = self.o(id);
                o.set(0xe, o.w(0xe) - o.w(0x20));
            }
        } else {
            let o = self.o(id);
            if o.w(0x1c) == 5 {
                o.set(0xe, o.w(0xe) + if o.w(0x1e) & 0x80 == 0 { -1 } else { 1 });
            }
            let (x, y) = (o.w(0xe), o.w(0x10));
            if o.w(0x32) < 1 && x - 0x40 < h.x && h.x < x + 0x40 && y - 0x30 < h.y && h.y < y + 0x30 {
                o.set(0x32, 0xaf);
            }
            if self.o(id).w(0x32) > 0 {
                if self.o(id).w(0x32) == 0xaf && env.rng.random(8) != 0 {
                    env.rng.random(4); // its last words
                }
                let o = self.o(id);
                o.set(0x32, o.w(0x32) - 1);
                if o.w(0x32) < 1 {
                    let y = o.w(0x10);
                    self.bigali_explode(id, y - 8, env);
                    self.remove_current(id);
                    return true;
                }
                o.set(0x34, (o.w(0x32) < 0x9b) as i32 + 0x5b34);
                o.set(2, -0x7fff);
                o.set(4, -0x7fff);
                o.set(6, 0x7fff);
                o.set(8, 0x7fff);
            }
        }
        let (x, y) = self.bxy(id);
        let sprite = self.o(id).w(0x34);
        if self.stomp(sprite, y, x, mirror, env) > 1 {
            let o = self.o(id);
            o.set(0x10, o.w(0x10) - 3);
            env.fx.shakes.push((5, 2));
        }
        // FUN_1070_7eb2 leaves 093e alone here: the drone still shoots at the dying.
        self.particles.p.aim_level = 0;
        let damage = match sprites::hit_rect(sprite, y, x, mirror) {
            Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
            None => 0,
        };
        self.particles.p.aim_level = 0x100;
        if damage > 0 {
            let o = self.o(id);
            if o.w(0x2e) > 0x34 && o.w(0x32) < 1 {
                o.set(0x36, o.w(0x36) - damage);
                if o.w(0x36) < -0x1f {
                    o.set(0x32, 0xaf);
                }
            }
            let mask = self.particles.hit_mask;
            if mask & 0x14 != 0 && damage > 0x30 {
                self.big_drop_bonus(id, env.world);
                if mask & 0x400 == 0 {
                    let y = self.o(id).w(0x10);
                    self.bigali_explode(id, y - 8, env);
                } else {
                    eprintln!("warning: a frozen big alien (FUN_1070_564d) not ported");
                }
                let o = self.o(id);
                if o.w(0x2e) > 0x34 || o.w(0x1c) != 1 || env.rng.random(3) == 0 {
                    self.remove_current(id);
                    return true;
                }
                self.o(id).set(0x28, -0x136);
            }
            let push = self.particles.hit_direction();
            let o = self.o(id);
            o.set(0xe, o.w(0xe) + push);
        }
        false
    }

    /// FUN_1070_796f(y): blows up (the armoured ones scatter their plates).
    fn bigali_explode(&mut self, id: usize, y: i32, env: &mut Env) {
        let (x, oy) = self.bxy(id);
        if self.o(id).b(0x44) == 0 {
            self.explode_area(env.world, 0x60, 0x14, oy + 0xe, x + 0x2e, oy - 0x2e, x - 0x2e, env.rng, env.fx);
            self.particles.blast(2, 0xc0, 7, 0x1e, 0x3e, -0x3e, -0x3e, oy, x);
            let speed = env.rng.random(9) + 0x30;
            self.gibs_speed(y, x, speed, env);
            if env.rng.random(3) != 0 {
                let by = (oy - 0x20) - env.rng.random(8);
                let which = env.rng.random(3);
                self.blood_splat(env.world, 0x541e + which, by, x);
            }
            let ps = &mut self.particles;
            ps.p.delay = 0x14;
            crate::alien::smoke_sprite(ps, oy, x);
            ps.p.delay = 0;
            ps.p.colour = 0x10;
            ps.spawn_seq(0x6500, 2, 0x14, oy - 6, x, 1);
            ps.p.colour = 0;
            env.fx.shakes.push((0x2a, 0x10));
        } else {
            let ps = &mut self.particles;
            // FUN_1070_01ae(800, 0xc, y - 2, x): armour plates in a fan.
            ps.p.delay = 0x10;
            ps.p.hits = 0x8000;
            ps.p.on_death = 0x10;
            for i in 0..0xc {
                let s = env.rng.random(3) + 0x5137;
                ps.spawn(s, oy - 2, x);
                let a = i * ((800 << 1) / 0xb) + (0x560 - 800) + env.rng.random(0x81);
                let v = env.rng.random(0x11) + 0x40;
                ps.throw(9, v, a);
            }
            ps.p.on_death = 0;
            ps.p.hits = 0;
            ps.p.on_spawn = 0x7577;
            for i in 0..=8 {
                ps.p.delay = (i + 2) * i * 2;
                let px = x - 0x20 + env.rng.random(0x41);
                let py = (oy - 2) - env.rng.random(6);
                ps.spawn_seq(0x7000, 2, 0x18, py, px, 1);
                let m = &mut ps.last_mut().m;
                m.set(0x1c, 0x400);
                m.set(0x1e, 0x400);
                m.set(0x42, 0x2148);
            }
            ps.p.on_spawn = 0;
            for i in 0..=0x14 {
                ps.p.delay = ((i + 0x14) * (i + 0x14)) / 4;
                let px = x - 0x20 + env.rng.random(0x41);
                let py = (oy - 2) - env.rng.random(0xc);
                // FUN_1040_3440: a spark (group 0, a sound when it appears).
                ps.p.group = 0;
                ps.p.on_spawn = 0x7576;
                ps.spawn_seq(0x1928, 3, 4, py, px, 1);
                ps.p.on_spawn = 0;
                ps.p.group = 1;
            }
            ps.p.delay = 0;
            eprintln!("warning: FUN_1050_5c7d's sideways shake from a big alien not applied");
        }
    }

    /// FUN_1070_72a2 (VMT +0x10 after the physics): falling and landing.
    pub fn bigali_after_move(&mut self, id: usize, env: &mut Env) {
        let o = self.o(id);
        if o.w(0x1c) == 0 {
            if o.w(0x1a) == 0 {
                o.set(0x18, o.w(0x10));
                if -300 < o.w(0x28) && o.w(0x28) != -0x78 {
                    if o.w(0x28) == -10 && o.w(0x2e) < 1 {
                        o.set(0x2e, o.w(0x20));
                    } else {
                        o.set(0x2e, 0);
                    }
                    o.set(0x28, -0x82);
                    o.set(0x3c, 0);
                }
            }
            o.set(0x1a, o.w(0x1a) + 1);
            o.set(2, -0x7fff);
            o.set(4, -0x7fff);
            o.set(6, 0x7fff);
            o.set(8, 0x7fff);
            let (y, prev, x) = (o.w(0x10), o.w(0x42), o.w(0xe));
            self.splash(env, y, prev, x);
        } else if o.w(0x1a) != 0 {
            if o.w(0x28) == -0x82 {
                let fall = o.w(0x10) - o.w(0x18);
                if fall < 0x30 {
                    o.set(0x28, -10);
                    o.set(0x2e, 0);
                    o.set(0x2c, 4);
                    o.set(0x3e, 0);
                } else {
                    o.set(0x28, -0x14);
                    o.set(0x2e, 0xb4);
                    o.set(0x2c, 0);
                }
                if fall > 0x1f {
                    let (x, y) = (o.w(0xe), o.w(0x10));
                    // FUN_1040_66d4: dust.
                    let ps = &mut self.particles;
                    ps.p.colour = 0x10;
                    ps.spawn_seq(0x6d00, 2, 0x13, y, x, 1);
                    ps.p.colour = 0;
                    env.fx.shakes.push((0x10, 6));
                }
            } else if o.w(0x28) == -0x78 {
                o.set(0x28, -10);
                o.set(0x2e, 0);
                o.set(0x2c, 4);
                o.set(0x3e, 0);
            }
            self.o(id).set(0x1a, 0);
        }
    }
}

/// FUN_1070_015f(0x1914, 0x40, angle, y, x): a spent shell flying off.
pub fn shell(ps: &mut Particles, angle: i32, y: i32, x: i32) {
    shell_of(ps, 0x1914, 0x40, angle, y, x);
}

/// FUN_1070_015f(sprite, speed, angle, y, x).
pub fn shell_of(ps: &mut Particles, sprite: i32, speed: i32, angle: i32, y: i32, x: i32) {
    ps.p.hits = 0x8000;
    ps.p.on_death = 0xd;
    ps.p.callback = 0x17f4;
    ps.spawn(sprite, y, x);
    ps.last_mut().m.set(0x26, 2);
    ps.throw(7, speed, angle);
    ps.p.callback = 0;
    ps.p.on_death = 0;
    ps.p.hits = 0;
}
