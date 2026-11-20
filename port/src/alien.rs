//! The small alien soldier (record type 0x1428, smallali.spr in sprite slot 0x57): its update
//! FUN_1070_60ca with the procedures nested in it (FUN_1070_5911..5fda), and its move hook
//! FUN_1070_586f. Translated field by field:
//!
//! | off | field |
//! |-----|-------|
//! | 0a  | flags: 1 drops on the hero from above |
//! | 12  | bonus carried |
//! | 14,16 | patrol range |
//! | 20  | facing |
//! | 26  | voice pitch |
//! | 28  | state (below) |
//! | 2a  | hit flash frames / 2c, 2e, 30 counters / 32 aim (signed: which way it looks) |
//! | 34  | sprite / 36 hit points (<= 0 dead) / 38 shots fired / 3a shooting pause |
//! | 3c  | alerted / 3e half width / 40 last y / 42 stomped / 44 corpse facing |
//! | 46  | 0..4 normal, -1 coward, 0xff no more bleeding |
//! | 48  | bleeding frames / 4a sees the hero / 4b keeps facing him / 4c corpse kind |
//!
//! States (+0x28): -10 idle, -0x14 walking, -500 on guard, -0x1fe / -0x28 shooting standing /
//! crouched, -0x96 / -600 hit, -0x21c jumping down, -0x212 throwing a grenade, -0x208 getting
//! up, -0x210 waiting, -200 dead, -0xd2 a corpse to be shot, 10 falling.

use crate::actors::Env;
use crate::level::Level;
use crate::sprites;
use crate::trig;

/// 1170:1aca / 1ace: the screen's x range in which aliens shoot.
const SHOOT_X: (i32, i32) = (0, 0x13f);

/// The parent's locals the nested procedures read.
#[derive(Default)]
struct Look {
    eye_y: i32,    // [BP-0x14]
    target_y: i32, // [BP-0x12] (FUN_1070_5ca6)
    angle: i32,    // [BP-0xa]
    dir: i32,      // [BP-0xe]: -1 / 1 towards the hero
}

impl Level {
    /// FUN_1070_60ca.
    pub fn alien(&mut self, id: usize, env: &mut Env) {
        if std::env::var_os("ET_DEBUG").is_some() {
            let o = self.o(id);
            let words: Vec<String> = (0..0x4c).step_by(2).map(|i| o.w(i).to_string()).collect();
            eprintln!("OBJ {}\t{}", env.frame + 1, words.join(" "));
        }
        let h = env.h;
        {
            let o = self.o(id);
            if o.w(0x2a) > 0 {
                o.set(0x2a, o.w(0x2a) - 1);
            }
            if o.w(0x3a) > 0 {
                o.set(0x3a, o.w(0x3a) - 1);
            }
            o.set(0x40, o.w(0x10));
        }
        let (x, y) = self.xy(id);
        self.patrol_box(id, y + 0x10, x + 0x40, y - 0x40, x - 0x40);
        let height = sprites::height(self.o(id).w(0x34));
        let half = self.o(id).w(0x3e);
        if self.actor_physics(id, env, height, half, 3) > 0 {
            if self.o(id).w(0x36) > 0 {
                env.fx.kills += 1;
            }
            self.drop_bonus(id, env.world);
            env.fx.shakes.push((0x20, 0x10));
            let (x, y) = self.xy(id);
            self.gibs(y - 0x19, x, env);
            self.remove_current(id);
            return;
        }
        let mut look = Look::default();
        if self.o(id).w(0x36) > 0 {
            let (x, y) = self.xy(id);
            let o = self.o(id);
            look.eye_y = y + if o.w(0x28) < -499 { -0x15 } else { -0x25 };
            look.target_y = aim_y(&h, look.eye_y);
            let sees = self.line_of_sight(env, look.target_y, h.x, look.eye_y, x);
            let o = self.o(id);
            o.set_b(0x4a, sees as u8);
            look.angle = trig::atan2(look.target_y - look.eye_y, (h.x - x - o.w(0x20) * 0x1a).abs());
            look.dir = if h.x - x < 0 { -1 } else { 1 };
            if o.w(0x20) != look.dir && o.w(0x28) != -0x208 {
                let d = (h.x - x).abs();
                let sees = o.b(0x4a) != 0;
                if env.noise > 0
                    || (d > 0xc
                        && ((o.b(0x4b) != 0 && sees)
                            || (sees && d < 0x40 && y - 0x40 <= h.y && h.y - 0x40 <= y)))
                {
                    self.turn(id, look.dir);
                }
                let o = self.o(id);
                let keep = !(o.b(0x4a) == 0 || (o.w(0x20) != look.dir && d > 0xb));
                o.set_b(0x4b, keep as u8);
            }
            let o = self.o(id);
            if o.w(10) & 1 != 0 && y < h.y && x - 0x60 < h.x && h.x < x + 0x60 {
                self.drop_down(id, env);
            }
        }
        let mut mirror = self.o(id).w(0x20) < 0;
        let state = self.o(id).w(0x28);
        let done = match state {
            -500 => {
                self.on_guard(id, env, &look);
                true
            }
            -0x14 => {
                self.walking(id, env, &look, &mut mirror);
                true
            }
            -10 => {
                self.idle(id, env, &look);
                true
            }
            -0x1fe | -0x28 => {
                self.shooting(id, env, &look, state == -0x28);
                true
            }
            -0x96 | -600 => {
                let o = self.o(id);
                o.set(0x2e, o.w(0x2e) - 1);
                let (base, top) = if state == -0x96 { (0x573e, 0x5740) } else { (0x577a, 0x577c) };
                o.set(0x34, (o.w(0x2e) + base).min(top));
                if o.w(0x2e) < 1 {
                    if state == -0x96 {
                        o.set(0x28, -10);
                        o.set(0x2e, 0xb4);
                    } else {
                        o.set(0x28, -500);
                    }
                }
                if o.w(0x2e) % 2 == 0 {
                    o.set(0xe, o.w(0xe) - o.w(0x20));
                }
                true
            }
            -200 => false,
            -0xd2 => {
                eprintln!("warning: alien corpse state -0xd2 (FUN_1068_0f32) not ported");
                return;
            }
            -0x208 => {
                let o = self.o(id);
                o.set(0x2e, o.w(0x2e) - 1);
                let mut f = o.w(0x2e) / 2 - 0x10;
                if f < 5 {
                    if f < 0 {
                        f = 0;
                    } else if o.w(0x2e) == 0x28 {
                        o.set(0x38, 0);
                    }
                } else {
                    f = 4;
                }
                o.set(0x34, (0x5772 - f).max(0x576f));
                if o.w(0x2e) < 1 {
                    o.set(0x28, -500);
                }
                o.set_b(0x4b, 1);
                true
            }
            -0x21c => {
                let o = self.o(id);
                o.set(0x34, o.w(0x2e) / 2 + 0x5798);
                o.set(0x2e, o.w(0x2e) + 1);
                if o.w(0x2e) > 0x21 {
                    o.set(0x28, -500);
                    env.fx.shakes.push((4, 4));
                }
                if o.w(0x2e) == 0xe {
                    env.fx.shakes.push((4, 4));
                    if env.rng.random(2) == 0 {
                        let (x, y) = self.xy(id);
                        let ps = &mut self.particles;
                        ps.p.group = 0;
                        ps.p.callback = 0x2192;
                        dust(ps, y, x);
                        ps.p.callback = 0;
                        ps.p.group = 1;
                    }
                }
                let (half, dir) = (self.o(id).w(0x3e), self.o(id).w(0x20));
                self.patrol_step(id, env, half, dir * 2);
                self.o(id).set_b(0x4b, 1);
                true
            }
            -0x212 => {
                let o = self.o(id);
                o.set(0x2e, o.w(0x2e) - 1);
                if o.w(0x2e) == 0x18 {
                    let mut power = 0x3c0;
                    if h.y > o.w(0x10) {
                        power = (power - (h.y - o.w(0x10)) * 10).max(0x1c0);
                    }
                    let gx = o.w(0x20) * 0x15 + o.w(0xe);
                    let gy = o.w(0x10) - 0xf;
                    let dir = o.w(0x20);
                    let r = env.rng.random(0x101);
                    let angle = 0x5a0 - dir * (power + r);
                    self.grenade(0x30, angle, gy, gx);
                }
                let o = self.o(id);
                let f = (o.w(0x2e) / 3 - 8).clamp(0, 4);
                o.set(0x34, (0x5790 - f).max(0x578d));
                if o.w(0x2e) < -7 {
                    o.set(0x28, -500);
                }
                true
            }
            10 => {
                self.o(id).set(0x34, 0x5783);
                true
            }
            _ => {
                eprintln!("warning: alien state {state} not ported");
                true
            }
        };
        if !done {
            if self.dead(id, env, &mut mirror) {
                return;
            }
        }
        // LAB_1070_704d: shots that hit it while alive.
        if self.o(id).w(0x36) > 0 {
            let (x, y) = self.xy(id);
            let damage = match sprites::hit_rect(self.o(id).w(0x34), y, x, mirror) {
                Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x117b),
                None => 0,
            };
            if damage > 0 {
                if std::env::var_os("ET_ALIENHIT").is_some() {
                    eprintln!("ALIENHIT {} id {id} damage {damage} hp {}", env.frame, self.o(id).w(0x36));
                }
                let push = self.particles.hit_direction();
                let o = self.o(id);
                o.set(0x36, o.w(0x36) - damage);
                if push != 0 {
                    o.set(0x20, -push);
                }
                if o.w(0x36) < 1 {
                    env.fx.kills += 1;
                    let mask = self.particles.hit_mask;
                    if mask & 0x400 == 0 {
                        if mask & 8 != 0 || (mask & 0x14 != 0 && damage > 0x30) {
                            let y = self.o(id).w(0x10);
                            self.blown_apart(id, y - 0x1e, env);
                            return;
                        }
                        let _voice = env.rng.random(3);
                        let o = self.o(id);
                        o.set(0x28, -200);
                        o.set(0x2e, 0);
                        o.set(0x2c, 0);
                        if o.w(0x46) < 0 {
                            o.set(0x14, -0x7fff);
                            o.set(0x16, 0x7fff);
                            o.set_b(0x4c, 1);
                            o.set(0x44, if h.x - o.w(0xe) < 0 { -1 } else { 1 });
                        } else {
                            let kind = env.rng.random(3);
                            o.set_b(0x4c, kind as u8);
                            let water = self.sector_water(env);
                            let o = self.o(id);
                            if water < o.w(0x10) - 4 {
                                o.set(0x46, 0xff);
                            } else {
                                let v = env.rng.random(2);
                                self.o(id).set(0x46, v);
                            }
                            let (x, y) = self.xy(id);
                            let by = (y - 0x20) - env.rng.random(8);
                            let which = env.rng.random(3);
                            self.blood_splat(env.world, 0x541e + which, by, x);
                            // FUN_1070_59d5: a puff of blood (group 0).
                            self.particles.p.group = 0;
                            blood_puff(&mut self.particles, y - 0x14, x);
                            self.particles.p.group = 1;
                            if env.rng.random(3) == 0 {
                                let t = env.rng.random(6) + 0x1e;
                                self.o(id).set(0x48, t);
                            }
                        }
                    } else {
                        self.o(id).set(0x28, -0xd2);
                        self.drop_bonus(id, env.world);
                    }
                    self.o(id).set(0x36, 0);
                    env.fx.shakes.push((0xc, 6));
                } else {
                    let o = self.o(id);
                    o.set(0x2a, 3);
                    o.set(0x2e, 9);
                    o.set(0x28, if o.w(0x28) < -499 { -600 } else { -0x96 });
                }
            }
        }
    }

    fn xy(&mut self, id: usize) -> (i32, i32) {
        let o = self.o(id);
        (o.w(0xe), o.w(0x10))
    }

    /// FUN_1068_10f3: let go of the bonus it carries.
    fn drop_bonus(&mut self, id: usize, world: &crate::world::World) {
        let (b, y, x) = (self.o(id).w(0x12), self.o(id).w(0x10), self.o(id).w(0xe));
        if b != 0 {
            self.bonus(world, b, y, x);
            self.o(id).set(0x12, 0);
        }
    }

    /// The water line of the hero's sector (DAT_1170_4656 + 0x16).
    fn sector_water(&self, _env: &Env) -> i32 {
        0x7fff
    }

    /// FUN_1070_5e4a: turn towards `dir` (or just look the other way on guard).
    fn turn(&mut self, id: usize, dir: i32) {
        let o = self.o(id);
        if o.w(0x28) == -0x1fe {
            o.set(0x28, -500);
        }
        if o.w(0x20) != dir {
            if o.w(0x28) == -500 {
                o.set(0x32, -o.w(0x32).abs());
            } else if o.w(0x28) > -0x96 {
                o.set(0x28, -0x14);
                o.set(0x20, dir);
                o.set(0x2e, 0x10);
                o.set(0x2c, 0);
            }
        }
    }

    /// FUN_1070_5c4b: let itself drop through the floor onto the hero.
    fn drop_down(&mut self, id: usize, env: &mut Env) {
        let (x, y) = self.xy(id);
        if env.world.solidity_at(y + 1, x) & 2 == 0 {
            let o = self.o(id);
            o.set(0x10, o.w(0x10) + 1);
            o.set(0x18, o.w(0x10));
            o.set(0x1a, 1);
            o.set(0x14, -0x7fff);
            o.set(0x16, 0x7fff);
        }
        let o = self.o(id);
        o.set(10, o.w(10) & !1);
    }

    /// FUN_1070_59f2: sees the hero ahead, roughly level.
    fn in_sight(&mut self, id: usize, look: &Look) -> bool {
        let o = self.o(id);
        o.b(0x4a) != 0 && o.w(0x20) == look.dir && (look.angle < 0x140 || look.angle > 0x1540)
    }

    /// FUN_1070_5a31: worth throwing a grenade at.
    fn grenade_target(&mut self, id: usize, env: &mut Env, look: &Look) -> bool {
        let h = env.h;
        let cam_x = env.camera.0;
        let o = self.o(id);
        if !(o.b(0x4a) != 0
            && o.w(0x20) == look.dir
            && o.w(0x3a) < 1
            && (look.angle > 0xb40 || look.angle < 0xf0))
        {
            return false;
        }
        let (x, y) = (o.w(0xe), o.w(0x10));
        if !(SHOOT_X.0 < x - cam_x && x - cam_x < SHOOT_X.1) {
            return false;
        }
        if !(y + 0x10 < h.y || env.rng.random(0xc) == 0) {
            return false;
        }
        let d = isqrt_round(look.target_y - look.eye_y, h.x - x);
        d > 0x3f && (h.x - x).abs() < 0xb0
    }

    /// FUN_1070_5b08: jump down a ledge ahead towards the hero if there's floor below.
    fn jump_down(&mut self, id: usize, env: &mut Env) -> bool {
        let h = env.h;
        let o = self.o(id);
        if o.b(0x4a) == 0 || o.w(0x3a) >= 1 {
            return false;
        }
        let d = (h.x - o.w(0xe)).abs();
        if !(d < 0x21 || d > 0xa3) {
            return false;
        }
        if o.w(0x14) != -0x7fff {
            let room = if o.w(0x20) == 1 { o.w(0x16) - o.w(0xe) } else { o.w(0xe) - o.w(0x14) };
            if room < 0x44 {
                return false;
            }
        }
        let (dir, y) = (o.w(0x20), o.w(0x10));
        let mut lx = dir * 0x44 + o.w(0xe);
        for _ in 0..4 {
            let s = |yy| crate::hero::Hero::solidity_all(env.world, self, yy, lx);
            if s(y + 0xf) & 0x201 == 0 || s(y) & 0x2a != 0 || s(y - 0x18) & 0x2a != 0 {
                return false;
            }
            lx -= dir * 0x10;
        }
        let o = self.o(id);
        o.set(0x28, -0x21c);
        o.set(0x2e, 0);
        true
    }

    /// FUN_1070_5fc0: out of ammo, it waits.
    fn wait(&mut self, id: usize) {
        let o = self.o(id);
        o.set(0x28, -0x208);
        o.set(0x2e, 0x50);
    }

    /// FUN_1070_5eb7: heard something: on guard.
    fn alert(&mut self, id: usize, env: &mut Env) {
        if self.o(id).w(0x3c) == 0 {
            let _shout = env.rng.random(2) == 0; // FUN_1070_001a + a sound
        }
        let o = self.o(id);
        o.set(0x28, -500);
        o.set(0x32, o.w(0x32).abs());
        o.set(0x3c, 10);
    }

    /// FUN_1070_5f30: alerted by noise, or by seeing the hero.
    fn listen(&mut self, id: usize, env: &mut Env, look: &Look) {
        let h = env.h;
        let o = self.o(id);
        if env.noise < 1 && o.w(0x3c) < 10 {
            if std::env::var_os("ET_LISTEN").is_some() {
                eprintln!("LISTEN {} id {id} sees {} at {},{} hero {},{} face {} 3c {}", env.frame, o.b(0x4a), o.w(0xe), o.w(0x10), h.x, h.y, o.w(0x20), o.w(0x3c));
            }
            if o.b(0x4a) != 0 {
                let a = trig::atan2(h.y - o.w(0x10), h.x - o.w(0xe));
                let hears = if o.w(0x20) < 1 { a < 0xe60 && a > 0x820 } else { a < 800 || a > 0x1360 };
                if hears {
                    self.alert(id, env);
                }
            }
        } else {
            self.alert(id, env);
        }
        let _ = look;
    }

    /// State -500.
    fn on_guard(&mut self, id: usize, env: &mut Env, look: &Look) {
        let cam_x = env.camera.0;
        let o = self.o(id);
        if o.w(0x32) < 10 {
            o.set(0x32, o.w(0x32) + 1);
            if o.w(0x32) == 0 {
                o.set(0x28, -10);
                o.set(0x2e, 0xb4);
                if o.b(0x4a) != 0 {
                    self.turn(id, look.dir);
                }
            }
        } else if o.w(0x38) < 4 {
            let x = o.w(0xe);
            let facing = o.w(0x20);
            if !self.jump_down(id, env) && self.grenade_target(id, env, look) {
                let o = self.o(id);
                o.set(0x28, -0x212);
                o.set(0x2e, 0x40);
                o.set(0x3a, 0x8c);
            } else if facing == look.dir
                && SHOOT_X.0 < x - cam_x
                && x - cam_x < SHOOT_X.1
                && (look.angle < 0x140 || look.angle > 0x1540)
            {
                let o = self.o(id);
                if o.b(0x4a) == 0 {
                    let y = o.w(0x10);
                    let ty = aim_y(&env.h, y - 0x25);
                    if self.line_of_sight(env, ty, env.h.x, y - 0x25, x) {
                        let o = self.o(id);
                        o.set(0x3c, 1);
                        o.set(0x32, -o.w(0x32).abs());
                    }
                } else {
                    o.set(0x30, 0);
                    o.set(0x28, -0x1fe);
                }
            }
        } else {
            self.wait(id);
        }
        let o = self.o(id);
        o.set(0x34, o.w(0x32).abs() / 2 + 0x570f);
    }

    /// State -0x14.
    fn walking(&mut self, id: usize, env: &mut Env, look: &Look, mirror: &mut bool) {
        let o = self.o(id);
        if o.w(0x2e) < 1 {
            o.set(0x34, (o.w(0x2c) / 5) % 0x10 + 0x57d4);
            if o.w(0x2c) % 0x28 == 0 {
                env.rng.random(0x20); // footstep pitch
            }
            o.set(0x2c, o.w(0x2c) + 1);
        } else {
            let mut f = (0x10 - o.w(0x2e)) / 2;
            if f > 3 {
                f = 7 - f;
            }
            o.set(0x34, f + 0x5708);
            o.set(0x2e, o.w(0x2e) - 1);
            if o.w(0x2e) > 7 {
                *mirror = !*mirror;
            }
            if o.w(0x2e) < 1 || o.w(0x14) + 1 == o.w(0x16) {
                o.set(0x2c, 0x19);
            } else {
                o.set(0x2c, o.w(0x2c) + 1);
            }
        }
        if self.o(id).w(0x2c) % 2 == 0 {
            let (half, dir) = (self.o(id).w(0x3e), self.o(id).w(0x20));
            self.patrol_step(id, env, half, dir);
            let o = self.o(id);
            if o.w(0x20) != dir {
                o.set(0x20, dir);
                o.set(0x28, -10);
                o.set(0x2e, 0xb4);
            }
        }
        self.listen(id, env, look);
    }

    /// State -10.
    fn idle(&mut self, id: usize, env: &mut Env, look: &Look) {
        let cam_x = env.camera.0;
        let o = self.o(id);
        o.set(0x34, 0x5703);
        if o.w(0x2e) >= 0 && o.w(0x2e) < 0x3c {
            o.set(0x34, 0x5702);
        }
        if o.w(0x2e) > 0x41 && o.w(0x2e) < 0x74 {
            o.set(0x34, 0x5704);
        }
        o.set(0x2e, o.w(0x2e) - 1);
        if o.w(0x2e) < 1 {
            if o.w(0x3c) < 1 {
                o.set(0x28, -0x14);
                let d = -o.w(0x20);
                self.turn(id, d);
            } else {
                o.set(0x3c, 10);
            }
        }
        let x = self.o(id).w(0xe);
        if self.o(id).w(0x3c) > 0
            && self.in_sight(id, look)
            && SHOOT_X.0 < x - cam_x
            && x - cam_x < SHOOT_X.1
        {
            let o = self.o(id);
            o.set(0x30, 0);
            o.set(0x28, -0x28);
        }
        if self.o(id).w(0x28) == -10 {
            self.listen(id, env, look);
        }
    }

    /// States -0x1fe (standing, `crouched` false) and -0x28.
    fn shooting(&mut self, id: usize, env: &mut Env, look: &Look, crouched: bool) {
        let o = self.o(id);
        o.set(0x30, o.w(0x30) + 1);
        let mut t = o.w(0x30);
        let (aim_at, fire_at, again_at, back) = if crouched { (8, 0xc, 0x19, 8) } else { (5, 9, 0x13, 5) };
        if t > aim_at {
            t -= 0x1e;
            if t == fire_at {
                let (x, y, dir) = (o.w(0xe), o.w(0x10), o.w(0x20));
                let gy = y - if crouched { 0x25 } else { 0x15 };
                self.alien_shot(id, env, look.angle, gy, dir * 0x1a + x);
            }
            if t > again_at {
                if self.in_sight(id, look) {
                    self.o(id).set(0x30, back);
                } else {
                    self.o(id).set(0x28, -500);
                }
            }
            t = if t < fire_at + 1 { fire_at } else if crouched { 0x27 - t } else { 0x1e - t };
        }
        let o = self.o(id);
        if crouched {
            o.set(0x34, (t / 3 + 0x572a).min(0x5731));
            if o.w(0x38) > 3 {
                o.set(0x28, -500);
            }
        } else {
            o.set(0x34, t / 3 + 0x5716);
            if o.w(0x38) < 4 {
                self.jump_down(id, env);
            } else {
                self.wait(id);
            }
        }
    }

    /// FUN_1070_5ce1(angle, y, x): a shot, its muzzle flash and a shell.
    fn alien_shot(&mut self, id: usize, env: &mut Env, angle: i32, y: i32, x: i32) {
        let mut a = if angle > 0xb40 { angle - 0x1680 } else { angle };
        a = a.clamp(-0x140, 0x140);
        let dir = self.o(id).w(0x20);
        if dir < 0 {
            a = 0xb40 - a;
        }
        let ps = &mut self.particles;
        ps.p.colour = 0x28;
        ps.spawn_seq(0x1b04, 4, 2, y, x, 1);
        ps.p.colour = 0;
        ps.p.hits = 0x80;
        ps.p.damage = 0x40;
        ps.p.on_death = 0x3f;
        ps.p.callback = CB_ALIEN_SHOT;
        ps.p.draw = (a > 0x5a0) as i32;
        ps.spawn(0x1910, y, x);
        ps.line(0x60, (a + 0x1680) % 0x1680);
        ps.p.callback = 0;
        ps.p.on_death = 0;
        ps.p.damage = 0x10;
        ps.p.hits = 0;
        ps.p.colour = 0;
        ps.spawn_seq(0x190f, 5, 1, y, x, 0);
        let side = if ps.p.draw < 1 { 1 } else { -1 };
        ps.p.group = 0;
        let (sx, sy) = (x - side * 0x10, y - 10);
        let r = env.rng.random(0x141);
        // FUN_1070_015f: the shell.
        ps.p.hits = 0x8000;
        ps.p.on_death = 0xd;
        ps.p.callback = 0x17f4;
        ps.spawn(0x1914, sy, sx);
        ps.last_mut().m.set(0x26, 2);
        ps.throw(7, 0x40, side * (r + 0x80) + 0x5a0);
        ps.p.callback = 0;
        ps.p.on_death = 0;
        ps.p.hits = 0;
        ps.p.group = 1;
        ps.p.draw = 0;
        env.fx.shakes.push((6, 4));
        let o = self.o(id);
        o.set(0x38, o.w(0x38) + 1);
    }

    /// FUN_1040_6187(speed, angle, y, x): a grenade.
    fn grenade(&mut self, speed: i32, angle: i32, y: i32, x: i32) {
        let ps = &mut self.particles;
        ps.p.group = 0;
        ps.p.hits = 0x8000;
        ps.p.flags = 0x80;
        ps.p.damage = 0xc0;
        ps.p.callback = 0x11e6;
        ps.p.on_death = 0x15;
        ps.p.draw = !(angle < 0x5a0 || angle > 0x10df) as i32;
        ps.spawn_seq(0x1818, 1, 0x78, y, x, 0);
        ps.bounce(7, speed, angle);
        ps.p = crate::particles::Params::default();
    }

    /// The dead branch of FUN_1070_60ca (the corpse kinds by +0x4c). Returns true when the
    /// object is gone.
    fn dead(&mut self, id: usize, env: &mut Env, mirror: &mut bool) -> bool {
        let h = env.h;
        let frame = env.frame;
        // Bleeding.
        {
            let o = self.o(id);
            if o.w(0x48) > 0 {
                o.set(0x48, o.w(0x48) - 1);
                if o.w(0x48) % 2 == 0 {
                    let delay = env.rng.random(2);
                    let o = self.o(id);
                    let (x, y, dir, t) = (o.w(0x20) * 4 + o.w(0xe), o.w(0x10) - 0x20, o.w(0x20), o.w(0x48));
                    let r = env.rng.random(0x81);
                    let angle = 0x5a0 - dir * ((0x2d0 - (t << 4)) - r);
                    let ps = &mut self.particles;
                    ps.p.delay = delay;
                    ps.p.hits = 0x8000;
                    ps.p.on_death = 0x16;
                    ps.spawn(0x5408, y, x);
                    ps.throw(7, 0x40, angle);
                    ps.p.on_death = 0;
                    ps.p.hits = 0;
                    ps.p.delay = 0;
                }
            }
        }
        let kind = self.o(id).b(0x4c);
        let (frames_base, limit) = match kind {
            0 => (0x5748, 0x1a),
            1 => (0x575d, 0x17),
            _ => (0x57b6, 0x23),
        };
        let mut threshold = limit;
        {
            let o = self.o(id);
            o.set(0x34, o.w(0x2e) / if kind >= 2 { 4 } else { 3 } + frames_base);
        }
        if self.o(id).w(0x2e) < limit {
            let settle = match kind { 0 => 0xc, 1 => 0xf, _ => 0x14 };
            let landed_at = match kind { 0 => 0x19, 1 => 0x16, _ => 0x21 };
            let o = self.o(id);
            if o.w(0x1a) < 1 || o.w(0x2e) < settle {
                o.set(0x2e, o.w(0x2e) + 1);
                if o.w(0x2e) == landed_at {
                    self.thud(id, env);
                }
            }
            let o = self.o(id);
            if kind == 0 {
                o.set(0x3e, (o.w(0x2e) << 1) / 5 + 10);
                o.set(0xe, o.w(0xe) - o.w(0x20));
            } else {
                o.set(0x3e, o.w(0x2e) / 6 + 10);
                if frame & 1 == 0 {
                    o.set(0xe, o.w(0xe) - o.w(0x20));
                }
            }
        } else {
            if self.o(id).w(0x1c) == 5 {
                let o = self.o(id);
                o.set(0xe, o.w(0xe) + if o.w(0x1e) & 0x80 == 0 { -1 } else { 1 });
            }
            let o = self.o(id);
            match kind {
                0 => {
                    if o.w(0x42) < 1 {
                        if o.w(0x46) == 0 && o.w(0x2c) < 0x72c {
                            if o.w(0x2c) % 0x11 < 6 && o.w(0x2c) % 0x99 < 0x33 {
                                o.set(0x34, 0x5755);
                            }
                            o.set(0x2c, o.w(0x2c) + 1);
                        }
                    } else {
                        o.set(0x34, o.w(0x34) + 2);
                    }
                }
                1 => {
                    if o.w(0x42) > 0 {
                        o.set(0x34, o.w(0x34) + 2);
                    }
                    if o.w(0x46) == -1 {
                        let a = o.w(0xe) + o.w(0x44) * -0x10;
                        let b = o.w(0xe) + o.w(0x44) * -0x40;
                        let (lo, hi) = (a.min(b), a.max(b));
                        let near = lo <= h.x && h.x <= hi;
                        if (near && o.w(0x10) - 0x30 < h.y && h.y < o.w(0x10) + 0x30) || o.w(0x42) == 1 {
                            o.set(0x46, 0xff);
                            threshold = 0x7fff;
                            let hp = env.rng.random(0x11) + 0x20;
                            let o = self.o(id);
                            o.set(0x36, hp);
                            o.set(0x20, o.w(0x44));
                            o.set(0x28, -500);
                            o.set(0x32, 10);
                            o.set_b(0x4b, 1);
                            o.set_b(0x4a, 1);
                            self.jump_down(id, env);
                            env.fx.kills -= 1;
                        }
                    }
                }
                _ => {
                    if o.w(0x42) < 1 {
                        if o.w(0x46) == 0 {
                            if o.w(0x2c) > 0x9f {
                                let i = o.w(0x2c) % 0xa0;
                                if i < 0x70 {
                                    o.set(0x34, pingpong(i, 0x10, 4) + 0x57c4);
                                } else {
                                    let lx = o.w(0xe) + o.w(0x20) * -0x26;
                                    let (y, dir) = (o.w(0x10), o.w(0x20));
                                    let wall = crate::hero::Hero::solidity_all(env.world, self, y, lx) & 2 != 0;
                                    let w = &env.world;
                                    if wall
                                        || w.solidity_at(y + 1, lx) & 0x201 == 0
                                        || w.solidity_at(y + 1, dir * 0x10 + lx) & 0x201 == 0
                                    {
                                        self.o(id).set(0x46, 0xff);
                                    }
                                }
                                let o = self.o(id);
                                if o.w(0x2c) % 0xe == 0 && o.w(0x2c) % 0xa0 < 0x41 {
                                    o.set(0xe, o.w(0xe) - o.w(0x20));
                                }
                                if o.w(0x2c) > 0x9fe {
                                    o.set(0x46, 0xff);
                                }
                            }
                            let o = self.o(id);
                            o.set(0x2c, o.w(0x2c) + 1);
                        }
                    } else {
                        o.set(0x34, o.w(0x34) + 2);
                    }
                }
            }
        }
        // The corpse can still be shot and stomped on.
        if threshold <= self.o(id).w(0x2e) {
            *mirror = self.o(id).w(0x20) < 0;
            let (x, y) = self.xy(id);
            let sprite = self.o(id).w(0x34);
            // (The drone ignores it unless +0x46 is -1.)
            let hidden = self.o(id).w(0x46) != -1;
            self.particles.p.aim_level = 0;
            if hidden {
                self.particles.p.aim_level2 = -1;
            }
            if std::env::var("ET_CORPSE").is_ok() {
                eprintln!("CORPSE {} sprite {:x} at {x},{y} mirror {} rect {:?}", env.frame + 1, sprite, *mirror, sprites::hit_rect(sprite, y, x, *mirror));
            }
            let damage = match sprites::hit_rect(sprite, y, x, *mirror) {
                Some((b, r, t, l)) => self.particles.hit_test(b, r, t, l, 0x11fb),
                None => 0,
            };
            self.particles.p.aim_level = 0x100;
            self.particles.p.aim_level2 = 0x100;
            let o = self.o(id);
            if o.w(0x42) > 0 {
                o.set(0x42, o.w(0x42) - 1);
            }
            let stomp = self.stomp(sprite, y, x, *mirror, env);
            if stomp > 0 {
                let o = self.o(id);
                o.set(0x42, 0xc);
                if o.b(0x4c) != 1 && env.rng.random(2) == 0 {
                    self.o(id).set(0x46, 0xff);
                }
            }
            let o = self.o(id);
            if o.w(0x36) < 0 {
                o.set(0x36, o.w(0x36) + 1);
            }
            if damage > 0 {
                let mask = self.particles.hit_mask;
                if mask & 0x48 == 0 {
                    let push = self.particles.hit_direction();
                    let o = self.o(id);
                    o.set(0x42, 8);
                    o.set(0xe, o.w(0xe) + push);
                } else {
                    let o = self.o(id);
                    o.set(0x36, o.w(0x36) - 3);
                    o.set(0x2a, 3);
                }
                if (mask & 0x14 != 0 && damage > 0x30) || self.o(id).w(0x36) < -0x50 {
                    let y = self.o(id).w(0x10);
                    self.blown_apart(id, y - 8, env);
                    return true;
                }
                self.o(id).set(0x46, 0xff);
            }
        }
        false
    }

    /// FUN_1070_5911: the body hits the floor.
    fn thud(&mut self, id: usize, env: &mut Env) {
        env.fx.shakes.push((0xc, 6));
        let (x, y) = self.xy(id);
        dust(&mut self.particles, y, x);
        let ps = &mut self.particles;
        let m = &mut ps.last_mut().m;
        m.set(0x1c, 0x100);
        m.set(0x1e, 0x200);
        m.set(0x42, 0x2148);
        if self.o(id).w(0x12) == 0 && env.rng.random(2) == 0 && self.o(id).w(0x46) != -1 {
            self.o(id).set(0x12, 0xd10);
        }
        self.drop_bonus(id, env.world);
    }

    /// FUN_1088_3a81(sprite, y, x): the hero standing on the corpse (4e26), or landing on its
    /// head holding up (a bounce). Returns 1 on a bounce.
    pub(crate) fn stomp(&mut self, sprite: i32, y: i32, x: i32, flip: bool, env: &mut Env) -> i32 {
        let h = env.h;
        let Some((l, t, r, b)) = sprites::hitbox(sprite) else {
            return 0;
        };
        let (left, right) = if flip { (x - r, x - l) } else { (x + l, x + r) };
        let (top, bottom) = (y + t, y + b);
        if !(left < h.x && h.x < right) {
            return 0;
        }
        if h.state < 1 {
            if y - 2 < h.y && h.y <= bottom {
                env.fx.hero_by_enemy = 1;
            }
            0
        } else {
            let top = top - 4;
            if h.state != 10 && h.vy >= 0 && h.fall > 0x16 && h.prev_y < top && top <= h.y && h.up {
                env.fx.hero_bounce = true;
                env.fx.shakes.push((0xc, 4));
                1
            } else {
                0
            }
        }
    }

    /// FUN_1070_5fda(y): torn apart by an explosion.
    fn blown_apart(&mut self, id: usize, y: i32, env: &mut Env) {
        self.drop_bonus(id, env.world);
        let mask = self.particles.hit_mask;
        if mask & 0x400 == 0 {
            let x = self.o(id).w(0xe);
            if mask & 8 == 0 {
                self.gibs(y, x, env);
                let (x, oy) = self.xy(id);
                smoke_sprite(&mut self.particles, oy, x);
                if env.rng.random(3) != 0 {
                    let by = (oy - 0x20) - env.rng.random(8);
                    let which = env.rng.random(3);
                    self.blood_splat(env.world, 0x541e + which, by, x);
                }
            } else {
                eprintln!("warning: alien burnt (FUN_1070_544e) not ported");
            }
        } else {
            eprintln!("warning: alien frozen (FUN_1070_564d) not ported");
        }
        self.remove_current(id);
    }

    /// FUN_1068_1bdd(y, x): an alien bursts into gibs: 64 bits of flesh, 8 bones, 16 drops of
    /// blood, then four puffs, all spread over the next frames.
    fn gibs(&mut self, y: i32, x: i32, env: &mut Env) {
        self.gibs_speed(y, x, GIB_SPEED, env);
    }

    /// FUN_1068_1bdd with 1170:127c (the base speed) set to `base`.
    pub(crate) fn gibs_speed(&mut self, y: i32, x: i32, base: i32, env: &mut Env) {
        self.gibs_kind(y, x, base, false, env);
    }

    /// FUN_1068_186c is the same burst in red (`red`): the hero's, a barrel's.
    pub(crate) fn gibs_kind(&mut self, y: i32, x: i32, base: i32, red: bool, env: &mut Env) {
        self.gibs_rng(y, x, base, red, &mut *env.rng);
    }

    pub(crate) fn gibs_rng(&mut self, y: i32, x: i32, base: i32, red: bool, rng: &mut crate::rng::PascalRandom) {
        let (flesh, bones, nbones, blood) = if red { (0x5307, 0x530f, 5, 0x5300) } else { (0x5407, 0x540f, 3, 0x5400) };
        let ps = &mut self.particles;
        // Each step throws one piece left and one right, both spreading further each step:
        // (delay, sprite) then the angle's Random(0x80).
        let left = |r: &mut crate::rng::PascalRandom, spread: i32| (r.random(0x80) + 0x960) - spread;
        let right = |r: &mut crate::rng::PascalRandom, spread: i32| ((0x1860 - r.random(0x80)) + spread) % 0x1680;
        ps.p.hits = 0x8000;
        ps.p.on_death = 0x16;
        ps.p.callback = 0x23b7;
        ps.p.flags = 0x8c;
        for i in 1..=0x20 {
            let (speed, spread) = ((i << 6) / 0x20 + base, (i * 0x3c0) / 0x20);
            ps.p.delay = rng.random(8);
            let s = (rng.random(4) == 0) as i32 + flesh;
            ps.spawn(s, y, x);
            let a = left(rng, spread);
            ps.throw(0xb, speed, a);
            ps.p.delay = rng.random(8);
            let s = (rng.random(4) == 0) as i32 * 2 + flesh - 1;
            ps.spawn(s, y, x);
            let a = right(rng, spread);
            ps.throw(0xb, speed, a);
        }
        ps.p.callback = 0;
        ps.p.flags = 0x84;
        ps.p.on_death = 0x30;
        for i in 1..=4 {
            let (speed, spread) = ((i * 0x30) / 4 + base, (i * 0x3c0) / 4);
            for side in 0..2 {
                ps.p.delay = rng.random(8) + 8;
                let s = rng.random(nbones) + bones;
                ps.spawn(s, y, x);
                let a = if side == 0 { left(rng, spread) } else { right(rng, spread) };
                ps.throw(0xe, speed, a);
            }
        }
        ps.p.on_death = 0;
        for i in 1..=8 {
            let (speed, spread) = ((i << 5) / 8 + base, (i * 0x3c0) / 8);
            for side in 0..2 {
                ps.p.delay = rng.random(8) + 8;
                ps.spawn_seq(blood, 4, 5, y, x, 1);
                let a = if side == 0 { left(rng, spread) } else { right(rng, spread) };
                ps.throw(0xb, speed, a);
            }
        }
        ps.p.hits = 0;
        for (d, dy, dx) in [(0x1e, 6, 1), (0x19, 0xc, 4), (0x14, -5, -5), (0xf, 5, 0)] {
            ps.p.delay = d;
            ps.spawn_seq(blood, 4, 5, y + dy, x + dx, 1);
        }
        ps.p.delay = 0;
    }

    /// FUN_1068_0dd8(half, dx): walk dx along the patrol range; turn at its ends, walls and
    /// drops.
    pub(crate) fn patrol_step(&mut self, id: usize, env: &mut Env, half: i32, dx: i32) {
        let o = self.o(id);
        o.set(0xe, o.w(0xe) + dx);
        let (ya, yb) = if o.w(0x1c) == 5 {
            (((o.w(0x10) + 1) & !15) - 1, (o.w(0x10) + 0xf) & !15)
        } else {
            (o.w(0x10), o.w(0x10) + 1)
        };
        let (x, lo, hi) = (o.w(0xe), o.w(0x14), o.w(0x16));
        let s = |yy, xx| crate::hero::Hero::solidity_all(env.world, self, yy, xx);
        if dx < 0 {
            if x <= lo || s(ya, x - half) & 8 != 0 || s(yb, x - half) & 0x231 == 0 {
                self.o(id).set(0x20, 1);
            }
        } else if hi <= x || s(ya, x + half) & 8 != 0 || s(yb, x + half) & 0x231 == 0 {
            self.o(id).set(0x20, -1);
        }
    }

    /// FUN_1068_0a54(y2, x2, y1, x1): nothing with solidity 0x40 on the line, tested once a tile.
    pub(crate) fn line_of_sight(&self, env: &Env, y2: i32, x2: i32, y1: i32, x1: i32) -> bool {
        let (dx, dy) = (x2 - x1, y2 - y1);
        let s = |y: i32, x: i32| {
            let v = crate::hero::Hero::solidity_all(env.world, self, y, x);
            if std::env::var_os("ET_LOS").is_some() && v & 0x40 != 0 {
                eprintln!("LOS {} blocked at y={y} x={x} solidity {v:#x} tilefn {} cell {:#x} f1 {:#x} f2 {} f3 {} f4 {}", env.frame, env.world.tile_function_at(y, x), env.world.cell_at(1, y, x).unwrap_or(0), env.world.cell_function(env.world.cell_at(1, y, x).unwrap_or(0), 1), env.world.cell_function(env.world.cell_at(1, y, x).unwrap_or(0), 2), env.world.cell_function(env.world.cell_at(1, y, x).unwrap_or(0), 3), env.world.cell_function(env.world.cell_at(1, y, x).unwrap_or(0), 4));
            }
            v & 0x40 != 0
        };
        if dy.abs() < dx.abs() {
            let mut lx = (x1 & !15) + 7;
            if x1 < x2 {
                while lx < x2 {
                    if s(y1 + (dy * (lx - x1)) / dx, lx) {
                        return false;
                    }
                    lx += 0x10;
                }
            } else {
                while x2 < lx {
                    if s(y1 + (dy * (lx - x1)) / dx, lx) {
                        return false;
                    }
                    lx -= 0x10;
                }
            }
            !s(y2, x2)
        } else {
            let mut ly = (y1 & !15) + 7;
            if y1 < y2 {
                while ly < y2 {
                    if s(ly, x1 + (dx * (ly - y1)) / dy) {
                        return false;
                    }
                    ly += 0x10;
                }
            } else if dy != 0 {
                while y2 < ly {
                    if s(ly, x1 + (dx * (ly - y1)) / dy) {
                        return false;
                    }
                    ly -= 0x10;
                }
            } else {
                return true;
            }
            !s(y2, x2)
        }
    }

    /// FUN_1070_586f (VMT +0x10 after the physics): falling and landing.
    pub fn alien_after_move(&mut self, id: usize, env: &mut Env) {
        let o = self.o(id);
        if o.w(0x1c) == 0 {
            if o.w(0x1a) == 0 {
                o.set(0x18, o.w(0x10));
                o.set(0x14, -0x7fff);
                o.set(0x16, 0x7fff);
            }
            o.set(0x1a, o.w(0x1a) + 1);
            o.set(2, -0x7fff);
            o.set(4, -0x7fff);
            o.set(6, 0x7fff);
            o.set(8, 0x7fff);
            if o.w(0x28) != -200 {
                o.set(0x28, 10);
            }
            let (y, prev, x) = (o.w(0x10), o.w(0x40), o.w(0xe));
            self.splash(env, y, prev, x);
        } else if o.w(0x1a) != 0 {
            if o.w(0x28) == 10 {
                o.set(0x28, -500);
                o.set(0x32, 10);
                if o.w(0x1a) > 0xc {
                    let (x, y) = (o.w(0xe), o.w(0x10));
                    dust(&mut self.particles, y, x);
                }
            }
            self.o(id).set(0x1a, 0);
        }
    }
}

/// The aliens' shot (FUN_1050_02d2 per frame).
pub const CB_ALIEN_SHOT: u16 = 0x2d2;
/// 1170:127c: the base speed of gibs.
const GIB_SPEED: i32 = 0x20;

/// FUN_1070_5ca6: where to aim at the hero: his chest, within his height.
fn aim_y(h: &crate::actors::HeroView, eye_y: i32) -> i32 {
    let t = h.y - 4;
    if eye_y <= t {
        let t2 = (h.y - h.height) + 6;
        if t2 <= eye_y {
            return eye_y;
        }
        return t2;
    }
    t
}

/// FUN_10f8_00db: round(sqrt(dx*dx + dy*dy)).
fn isqrt_round(dy: i32, dx: i32) -> i32 {
    ((dx as f64).powi(2) + (dy as f64).powi(2)).sqrt().round_ties_even() as i32
}

/// FUN_10f0_1317(t, 1, n, k): ping-pong over 0..k-1 every n frames.
fn pingpong(t: i32, n: i32, k: i32) -> i32 {
    let v = (t / n) % (k * 2 - 2) - k + 1;
    (k - 1) - v.abs()
}

impl Level {
    /// FUN_1070_0105(volume, y, previous y, x): something falling through the water line of the
    /// hero's sector (1170:4656 +0x16) splashes (sound 0x6f, pitch 0xd0 + Random(0x50)).
    pub(crate) fn splash(&mut self, env: &mut Env, y: i32, prev: i32, x: i32) {
        let line = env.world.sectors[self.sector.unwrap_or(0)].water[1];
        if prev - 4 < line && line <= y - 4 {
            env.rng.random(0x50);
            // FUN_1040_6ac8: the splash, drawn clipped at the line (callback 1050:1d33).
            let ps = &mut self.particles;
            ps.p.callback = 0x1d33;
            ps.p.colour = 0x34;
            ps.spawn_seq(0x6d00, 2, 0x12, line - 1, x, 1);
            ps.p.colour = 0;
            ps.p.callback = 0;
        }
    }
}

/// FUN_1040_66d4: landing dust.
pub(crate) fn dust(ps: &mut crate::particles::Particles, y: i32, x: i32) {
    ps.p.colour = 0x10;
    ps.spawn_seq(0x6d00, 2, 0x13, y, x, 1);
    ps.p.colour = 0;
}

/// FUN_1040_62d2: a puff of blood.
fn blood_puff(ps: &mut crate::particles::Particles, y: i32, x: i32) {
    ps.spawn_seq(0x5400, 4, 5, y, x, 1);
}

/// FUN_1040_637a: a lingering smoke sprite.
pub(crate) fn smoke_sprite(ps: &mut crate::particles::Particles, y: i32, x: i32) {
    ps.p.callback = 0x18e6;
    ps.p.colour = 0x10;
    ps.spawn_seq(0x6a00, 1, 0x17e, y, x, 0);
    let m = &mut ps.last_mut().m;
    m.set(0x1c, 0x200);
    m.set(0x1e, 0x200);
    ps.p.colour = 0;
    ps.p.callback = 0;
}
